use super::{
    ImplementationCampaignDriveProfile, RoleProgram, id, read_json, run_role, show, text,
    write_private_idempotently,
};
use kiln_adapter_lattice::worker::{WorkerInvocation, WorkerTransportProfile};
use kiln_core::implementation::{
    ImplementationAttempt, ImplementationDispatchPlan, ImplementationWork, WorkSourceKind,
    campaign::PendingEffect,
};
use std::{ffi::OsString, fs, path::Path};

pub(super) fn restore(
    profile: &ImplementationCampaignDriveProfile,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    effect: &PendingEffect,
) -> Result<Option<WorkerInvocation>, String> {
    let mut worker = profile.worker.clone();
    let prefix = format!("campaign-{}", attempt.identity.as_str());
    worker.dispatch_plan = profile
        .bound_input_directory
        .join(format!("{prefix}.dispatch.json"));
    worker.request = profile
        .bound_input_directory
        .join(format!("{prefix}.request.json"));
    match (exists(&worker.dispatch_plan)?, exists(&worker.request)?) {
        (true, true) => {
            validate_retained(work, attempt, &worker, effect)?;
            Ok(Some(worker))
        }
        (false, false) => Ok(None),
        _ => Err(
            "incomplete retained request/plan; reconciliation cannot prepare or overwrite it"
                .into(),
        ),
    }
}

pub(super) fn bind(
    profile: &ImplementationCampaignDriveProfile,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    effect: &PendingEffect,
) -> Result<WorkerInvocation, String> {
    let mut worker = profile.worker.clone();
    let prefix = format!("campaign-{}", attempt.identity.as_str());
    let seed_path = profile
        .bound_input_directory
        .join(format!("{prefix}.seed.json"));
    let draft_path = profile
        .bound_input_directory
        .join(format!("{prefix}.draft.json"));
    worker.dispatch_plan = profile
        .bound_input_directory
        .join(format!("{prefix}.dispatch.json"));
    worker.request = profile
        .bound_input_directory
        .join(format!("{prefix}.request.json"));
    let plan_exists = exists(&worker.dispatch_plan)?;
    let request_exists = exists(&worker.request)?;
    if plan_exists {
        if !request_exists {
            return Err("retained dispatch plan lost its prepared request; do not prepare or dispatch again".into());
        }
        validate_retained(work, attempt, &worker, effect)?;
        return Ok(worker);
    }
    if attempt.dispatch.is_some() || attempt.dispatch_request_binding.is_some() {
        return Err(
            "dispatched attempt lost immutable binding files; retain unknown outcome".into(),
        );
    }
    let mut plan: ImplementationDispatchPlan = if exists(&draft_path)? {
        validate_context_scope(work, attempt, &worker, effect)?;
        read_json(&draft_path)?
    } else {
        if request_exists || exists(&seed_path)? {
            return Err(
                "incomplete preparation has no frozen plan; retain files without replacement"
                    .into(),
            );
        }
        let scope = serde_json::to_vec(&context_scope(work, attempt, effect)?).map_err(text)?;
        write_private_idempotently(&worker.request.with_extension("context.json"), &scope)?;
        let workspace = super::workspace::prepare(profile, work, attempt)?;
        let mut plan = bound_plan(profile, work, attempt)?;
        let mut request: serde_json::Value = read_json(&profile.worker.request)?;
        bind_envelope(&mut request, work, attempt, &plan)?;
        super::observation::bind_stage_request(profile, effect, work, &mut request)?;
        let object = request
            .as_object_mut()
            .ok_or("worker seed is not an object")?;
        object.insert("workspace_root".into(), serde_json::json!(workspace));
        object.insert("network".into(), serde_json::json!(plan.workspace.network));
        let coordinator_enrollment = read_json(&profile.worker.enrollment)?;
        let mut coordinator = kiln_implementer::Implementer::open(
            &profile.implementation_database,
            coordinator_enrollment,
        )?;
        let admission = coordinator.admission_snapshot()?;
        let queue = admission
            .items
            .get(&attempt.work_identity)
            .ok_or("campaign work has no canonical admission item")?;
        let expiry = attempt.lease_until_ms.min(queue.deadline_ms);
        object.insert("expires_at_ms".into(), serde_json::json!(expiry));
        let remaining_minutes = expiry.saturating_sub(super::wall_clock_ms()?) / 60_000;
        plan.deadline_minutes = plan
            .deadline_minutes
            .min(u32::try_from(remaining_minutes).map_err(text)?);
        if plan.deadline_minutes == 0 {
            return Err("claim/admission window cannot fit the bounded worker runtime".into());
        }
        let envelope = object
            .get_mut("envelope")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("worker seed lacks an envelope")?;
        let budget = envelope
            .get_mut("budget")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("worker seed lacks a bounded budget")?;
        let seconds = budget
            .get("max_seconds")
            .and_then(serde_json::Value::as_u64)
            .ok_or("worker seconds are missing")?
            .min(u64::from(plan.deadline_minutes) * 60);
        let turns = budget
            .get("max_turns")
            .and_then(serde_json::Value::as_u64)
            .ok_or("worker turns are missing")?
            .min(u64::from(plan.invocation_budget));
        if seconds == 0 || turns == 0 {
            return Err("worker budget is empty".into());
        }
        budget.insert("max_seconds".into(), seconds.into());
        budget.insert("max_turns".into(), turns.into());
        write_private_idempotently(&draft_path, &serde_json::to_vec(&plan).map_err(text)?)?;
        write_private_idempotently(&seed_path, &serde_json::to_vec(&request).map_err(text)?)?;
        plan
    };
    if !exists(&worker.request)? {
        let transport: WorkerTransportProfile = read_json(&worker.transport_profile)?;
        if transport.schema != "kiln.lattice-implementation-cli.v2" {
            return Err(
                "campaign requires the explicit request-profile transport-v2 cohort".into(),
            );
        }
        let native_prepare = RoleProgram {
            program: transport.lattice_program,
            blake3: transport.executable_blake3,
            uid: profile.roles.worker.uid,
            gid: profile.roles.worker.gid,
            arguments: vec!["implementation-attempt".into(), "prepare".into()],
        };
        // The plan digest frames the native worker profile, and the request
        // must carry that digest. A first preparation yields the profile; the
        // profile revision does not frame `dispatch_plan`, so the second
        // preparation of the planned seed keeps it.
        let probe_path = profile
            .bound_input_directory
            .join(format!("{prefix}.profile.json"));
        if !exists(&probe_path)? {
            prepare_native(profile, &native_prepare, &seed_path, &probe_path)?;
        }
        bind_native_profile(&mut plan, &read_json(&probe_path)?)?;
        let digest =
            kiln_core::implementation::admit_dispatch_plan(work, attempt, &plan).map_err(show)?;
        let planned_seed_path = profile
            .bound_input_directory
            .join(format!("{prefix}.planned-seed.json"));
        write_private_idempotently(
            &planned_seed_path,
            &serde_json::to_vec(&planned_seed(read_json(&seed_path)?, &digest)?).map_err(text)?,
        )?;
        prepare_native(
            profile,
            &native_prepare,
            &planned_seed_path,
            &worker.request,
        )?;
    }
    let prepared: serde_json::Value = read_json(&worker.request)?;
    bind_native_profile(&mut plan, &prepared)?;
    let digest =
        kiln_core::implementation::admit_dispatch_plan(work, attempt, &plan).map_err(show)?;
    if !binds_plan(&prepared, &digest) {
        return Err("prepared request does not bind its exact dispatch plan".into());
    }
    write_private_idempotently(
        &worker.dispatch_plan,
        &serde_json::to_vec(&plan).map_err(text)?,
    )?;
    validate_retained(work, attempt, &worker, effect)?;
    Ok(worker)
}

/// The seed with Kiln's dispatch-plan identity, for the second preparation.
// r[impl kiln.implementation.lattice_cli.scenario.plan]
fn planned_seed(
    mut seed: serde_json::Value,
    plan: &kiln_core::types::ContentId,
) -> Result<serde_json::Value, String> {
    seed.as_object_mut()
        .ok_or("worker seed is not an object")?
        .insert("dispatch_plan".into(), serde_json::json!(plan.as_str()));
    Ok(seed)
}

/// Whether prepared request bytes carry the exact dispatch-plan identity.
fn binds_plan(request: &serde_json::Value, plan: &kiln_core::types::ContentId) -> bool {
    request
        .get("dispatch_plan")
        .and_then(serde_json::Value::as_str)
        == Some(plan.as_str())
}

/// Run the selected Lattice program's non-dispatching preparation once.
fn prepare_native(
    profile: &ImplementationCampaignDriveProfile,
    program: &RoleProgram,
    seed: &Path,
    output: &Path,
) -> Result<(), String> {
    run_role(
        profile,
        program,
        &profile.bound_input_directory,
        &[
            OsString::from("--request"),
            seed.as_os_str().to_owned(),
            OsString::from("--output"),
            output.as_os_str().to_owned(),
        ],
    )?;
    Ok(())
}

/// Bind the native worker profile a preparation computed into the plan.
fn bind_native_profile(
    plan: &mut ImplementationDispatchPlan,
    prepared: &serde_json::Value,
) -> Result<(), String> {
    let native_profile = prepared
        .pointer("/envelope/worker_profile")
        .and_then(serde_json::Value::as_str)
        .ok_or("native preparation omitted worker profile")?;
    plan.execution
        .as_mut()
        .ok_or("prepared plan lost explicit execution bindings")?
        .native_request_profile = Some(id(native_profile)?);
    Ok(())
}

fn bound_plan(
    profile: &ImplementationCampaignDriveProfile,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
) -> Result<ImplementationDispatchPlan, String> {
    let work_identity = work.identity().map_err(show)?;
    let mut plan: ImplementationDispatchPlan = read_json(&profile.worker.dispatch_plan)?;
    plan.work_identity = work_identity.clone();
    plan.work_revision = work.revision;
    plan.attempt_identity = attempt.identity.clone();
    plan.fence = attempt.fence;
    plan.repository = work.repository.clone();
    plan.base = work.base.clone();
    plan.permitted_paths = work.permitted_paths.clone();
    plan.permitted_effects = work.permitted_effects.clone();
    plan.deadline_minutes = plan.deadline_minutes.min(work.budget.minutes_per_attempt);
    let identity = serde_json::to_vec(&(
        "kiln.campaign.workspace.v1",
        &work_identity,
        &attempt.identity,
    ))
    .map_err(text)?;
    plan.workspace.identity = id(blake3::hash(&identity).to_hex().as_str())?;
    let mut execution = plan
        .execution
        .take()
        .ok_or("campaign requires explicit execution bindings")?;
    let enrollment = read_json(&profile.worker.enrollment)?;
    let mut coordinator =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
    let admission = coordinator.admission_snapshot()?;
    let policy_identity = admission
        .policy
        .as_ref()
        .ok_or("campaign requires the canonical fair-admission policy")?
        .identity()
        .map_err(show)?;
    execution.requirements = rebound_requirements(work, policy_identity, execution.requirements)?;
    execution.attempt.work_version = work_identity.as_str().to_owned();
    execution.attempt.attempt = attempt.identity.as_str().to_owned();
    execution.attempt.fence = attempt.fence;
    execution.native_request_profile = None;
    plan.execution = Some(execution);
    Ok(plan)
}

pub(super) fn attach_work_needs(
    profile: &ImplementationCampaignDriveProfile,
    work: &ImplementationWork,
    owner: &mut kiln_implementer::Implementer,
) -> Result<(), String> {
    let template: ImplementationDispatchPlan = read_json(&profile.worker.dispatch_plan)?;
    let execution = template
        .execution
        .ok_or("campaign requires explicit execution bindings before admission")?;
    let admission = owner.admission_snapshot()?;
    let policy = admission
        .policy
        .as_ref()
        .ok_or("campaign requires the canonical fair-admission policy before claiming")?
        .identity()
        .map_err(show)?;
    let requirements = rebound_requirements(work, policy, execution.requirements)?;
    owner.attach_work_needs(&requirements)?;
    Ok(())
}

fn rebound_requirements(
    work: &ImplementationWork,
    policy: kiln_core::types::ContentId,
    mut requirements: kiln_core::inventory::WorkNeedsAttachment,
) -> Result<kiln_core::inventory::WorkNeedsAttachment, String> {
    if requirements.policy_identity != policy {
        return Err("configured resource requirements use a stale admission policy".into());
    }
    let identity = work.identity().map_err(show)?;
    requirements.needs.work_version = identity.as_str().to_owned();
    kiln_core::inventory::WorkNeedsAttachment::new(
        identity,
        work.revision,
        policy,
        requirements.needs,
    )
    .map_err(str::to_owned)
}

fn context_scope(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    effect: &PendingEffect,
) -> Result<
    (
        kiln_core::types::ContentId,
        kiln_core::types::ContentId,
        u64,
        Option<kiln_core::types::ContentId>,
    ),
    String,
> {
    Ok((
        work.identity().map_err(show)?,
        attempt.identity.clone(),
        attempt.fence,
        effect.context_identity.clone(),
    ))
}

fn validate_context_scope(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    worker: &WorkerInvocation,
    effect: &PendingEffect,
) -> Result<(), String> {
    let retained: (
        kiln_core::types::ContentId,
        kiln_core::types::ContentId,
        u64,
        Option<kiln_core::types::ContentId>,
    ) = read_json(&worker.request.with_extension("context.json"))?;
    if retained != context_scope(work, attempt, effect)? {
        return Err(
            "retained native request uses another immutable attempt decision context".into(),
        );
    }
    Ok(())
}

fn bind_envelope(
    request: &mut serde_json::Value,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    plan: &ImplementationDispatchPlan,
) -> Result<(), String> {
    let source = work
        .aliases
        .iter()
        .find(|alias| {
            matches!(
                alias.kind,
                WorkSourceKind::CairnTask | WorkSourceKind::CairnArtifact
            )
        })
        .ok_or("campaign stage has no actual Cairn source alias")?;
    let kind = match source.kind {
        WorkSourceKind::CairnTask => "cairn_task",
        WorkSourceKind::CairnArtifact => "cairn_artifact",
        WorkSourceKind::GitHubIssue => {
            return Err("campaign selected an issue source instead of Cairn stage".into());
        }
    };
    let requirements = &plan
        .execution
        .as_ref()
        .ok_or("execution binding missing")?
        .requirements;
    let work_id = work.identity().map_err(show)?;
    let acceptance = kiln_core::implementation::acceptance_digest(work).map_err(show)?;
    let envelope = request
        .get_mut("envelope")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("worker seed lacks bound envelope")?;
    envelope.insert("work_version".into(), work.revision.into());
    envelope.insert("fence".into(), attempt.fence.into());
    for (name, value) in [
        ("work_id", work_id.as_str()),
        ("attempt_id", attempt.identity.as_str()),
        ("repository", work.repository.as_str()),
        (
            "base",
            work.base
                .as_ref()
                .ok_or("exact base missing")?
                .native_id
                .as_str(),
        ),
        ("workspace_id", plan.workspace.identity.as_str()),
        ("policy_digest", requirements.policy_identity.as_str()),
        (
            "worker_profile",
            requirements.needs.profile_revision.as_str(),
        ),
        ("verification_profile", work.verification_profile.as_str()),
        ("task_spec_digest", acceptance.as_str()),
        ("source_kind", kind),
        ("source_id", source.native_id.as_str()),
        ("readiness_snapshot", source.version.as_str()),
    ] {
        envelope.insert(name.to_owned(), value.into());
    }
    Ok(())
}

fn validate_retained(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    worker: &WorkerInvocation,
    effect: &PendingEffect,
) -> Result<(), String> {
    validate_context_scope(work, attempt, worker, effect)?;
    let plan: ImplementationDispatchPlan = read_json(&worker.dispatch_plan)?;
    let plan_id =
        kiln_core::implementation::admit_dispatch_plan(work, attempt, &plan).map_err(show)?;
    let bytes = fs::read(&worker.request).map_err(text)?;
    let request: serde_json::Value = serde_json::from_slice(&bytes).map_err(text)?;
    let native_profile = plan
        .execution
        .as_ref()
        .and_then(|execution| execution.native_request_profile.as_ref())
        .ok_or("retained campaign plan has no native request profile")?;
    if request
        .pointer("/envelope/worker_profile")
        .and_then(serde_json::Value::as_str)
        != Some(native_profile.as_str())
        || request
            .pointer("/envelope/attempt_id")
            .and_then(serde_json::Value::as_str)
            != Some(attempt.identity.as_str())
        || request
            .pointer("/envelope/fence")
            .and_then(serde_json::Value::as_u64)
            != Some(attempt.fence)
        || !binds_plan(&request, &plan_id)
    {
        return Err("retained native request is crossed with its actual prepared plan".into());
    }
    if let Some(seal) = &attempt.dispatch_request_binding {
        seal.validate_for(
            &attempt.identity,
            attempt.fence,
            &plan_id,
            &id(blake3::hash(&bytes).to_hex().as_str())?,
        )
        .map_err(show)?;
    }
    Ok(())
}

fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err("campaign binding path is not an ordinary file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(text(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{binds_plan, planned_seed};
    use kiln_core::types::ContentId;

    fn plan(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.implementation.lattice_cli.scenario.plan]
    #[test]
    fn the_planned_seed_and_prepared_request_carry_the_exact_plan() {
        let seed = serde_json::json!({"schema": "lattice.implementation-attempt-run.v3", "network": false});
        let planned = planned_seed(seed.clone(), &plan('d')).unwrap_or_else(|_| unreachable!());
        assert!(binds_plan(&planned, &plan('d')));
        assert!(!binds_plan(&planned, &plan('e')));
        assert!(!binds_plan(&seed, &plan('d')));
        // Every other seed field is unchanged.
        let mut without = planned;
        if let Some(fields) = without.as_object_mut() {
            fields.remove("dispatch_plan");
        }
        assert_eq!(without, seed);
        // A replanned seed replaces, never duplicates, the identity.
        let replanned = planned_seed(
            planned_seed(seed, &plan('d')).unwrap_or_else(|_| unreachable!()),
            &plan('e'),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(binds_plan(&replanned, &plan('e')));
        assert!(planned_seed(serde_json::json!([]), &plan('d')).is_err());
        assert!(!binds_plan(
            &serde_json::json!({"dispatch_plan": 1}),
            &plan('d')
        ));
    }
}
