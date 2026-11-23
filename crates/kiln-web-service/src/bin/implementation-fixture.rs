use kiln_adapter_git::native_publication::{NativeRemoteProfile, NativeTransport};
use kiln_core::fair_admission::{
    AdmissionPhase, AdmissionPolicy, AdmissionReason, Amounts, BudgetAccount, ChargeKind,
    EligibilityInput, OwnerReply, QueueItem, UsageObservation,
};
use kiln_core::implementation::{
    DISPATCH_PLAN_SCHEMA, DispatchExecutionBindings, Enrollment, ImplementationDispatchPlan,
    acceptance_digest, admit_dispatch_plan,
};
use kiln_core::inventory::{AttemptBinding, FactRef, ResourceRequirement, WorkNeeds};
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::projection::materialization::ProjectedWorkspaceRoute;
use kiln_core::types::{Alias, ContentId, ProviderProfileId};
use kiln_implementer::{
    Implementer,
    admission::{AdmissionCommand, AdmissionOwnerPortV1, AdmissionResult, OwnerPortFailure},
    verification::VerificationProfile,
};
use kiln_web_service::{engine::Engine, profile::ServiceProfile};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_INPUT: u64 = 16 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const USAGE: &str = "usage: kiln-implementation-fixture prepare-ci --verification-profile ABS_PROFILE --service-profile-template ABS_SEED | prepare-intake --database DB --enrollment FILE | bind-execution --database DB --enrollment FILE --attempt ID --fence N --worker-request FILE --dispatch-plan-template FILE [--projected-workspace-route FILE] | prepare-remote --profile ABS_NATIVE_REMOTE_JSON | prepare-remote-template --profile ABS_NATIVE_REMOTE_JSON";

fn diagnostic(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    if !path.is_absolute() {
        return Err("input path must be absolute".into());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_INPUT {
        return Err("input must be a bounded ordinary file".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err("input exceeds byte bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn required(args: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    let p = args.get(key).ok_or_else(|| format!("{key} required"))?;
    let p = PathBuf::from(p);
    if !p.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(p)
}
fn parse(args: impl Iterator<Item = String>) -> Result<(String, BTreeMap<String, String>), String> {
    let mut args = args;
    let op = args.next().ok_or(USAGE)?;
    if ![
        "prepare-ci",
        "prepare-intake",
        "bind-execution",
        "prepare-remote",
        "prepare-remote-template",
    ]
    .contains(&op.as_str())
    {
        return Err(USAGE.into());
    }
    let mut flags = BTreeMap::new();
    while let Some(k) = args.next() {
        if !k.starts_with("--") {
            return Err(format!("unexpected argument {k}"));
        }
        let v = args
            .next()
            .ok_or_else(|| format!("missing value for {k}"))?;
        if flags.insert(k.clone(), v).is_some() {
            return Err(format!("duplicate {k}"));
        }
    }
    Ok((op, flags))
}
fn exact_flags(flags: &BTreeMap<String, String>, expected: &[&str]) -> Result<(), String> {
    if flags.len() != expected.len() || expected.iter().any(|k| !flags.contains_key(*k)) {
        return Err(USAGE.into());
    }
    Ok(())
}
fn clock_ms() -> Result<u64, String> {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    u64::try_from(n).map_err(|e| e.to_string())
}
fn fixture_scope(
    work: &kiln_core::implementation::ImplementationWork,
    enrollment: &Enrollment,
) -> Result<(), String> {
    if work.repository != "qualification/disposable"
        || work.project != "qualification"
        || work.actor != "operator"
        || enrollment.repositories != BTreeSet::from(["qualification/disposable".to_owned()])
        || enrollment.actors != BTreeSet::from(["operator".to_owned()])
    {
        return Err("fixture work/enrollment must be exactly qualification/disposable, qualification, operator".into());
    }
    Ok(())
}

fn local_provider() -> Result<ProviderProfileId, String> {
    ProviderProfileId::new("scripted-local-omp").map_err(|e| format!("{}: {}", e.code, e.message))
}
fn local_amounts() -> Result<Amounts, String> {
    Ok(BTreeMap::from([(
        Alias::new("worker_requests").map_err(diagnostic)?,
        1,
    )]))
}
fn local_policy(
    work: &kiln_core::implementation::ImplementationWork,
) -> Result<AdmissionPolicy, String> {
    let identity = work
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let campaign = Alias::new("scripted_local").map_err(diagnostic)?;
    Ok(AdmissionPolicy {
        version: 1,
        accounting_basis: identity,
        max_items: 1,
        max_bytes: 1_048_576,
        max_wait_ms: 180_000,
        max_observation_age_ms: 180_000,
        max_retry_after_ms: 30_000,
        max_children: 1,
        max_active: 1,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(campaign, 1)]),
        group_limits: Vec::new(),
        serving: None,
    })
}

// A finite local-only request ledger. It is not a physical/model capacity grant.
fn setup_local_admission(
    owner: &mut Implementer,
    work: &kiln_core::implementation::ImplementationWork,
    now: u64,
) -> Result<(), String> {
    let identity = work
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let provider = local_provider()?;
    let campaign = Alias::new("scripted_local").map_err(diagnostic)?;
    let tenant = Alias::new("scripted_local").map_err(diagnostic)?;
    let dimension = Alias::new("worker_requests").map_err(diagnostic)?;
    // 180s pre-claim wait plus the 180s worker expiry window.
    let deadline = now.saturating_add(360_000);
    let policy = local_policy(work)?;
    let before = owner.admission_snapshot()?;
    if before.policy.is_some()
        || !before.items.is_empty()
        || !before.accounts.is_empty()
        || !before.reservations.is_empty()
        || !before.observations.is_empty()
        || !before.eligibility.is_empty()
    {
        return Err("refusing to overwrite or extend unrelated admission ledger state".into());
    }
    owner.admission_apply(
        AdmissionCommand::Configure {
            policy: policy.clone(),
        },
        now,
    )?;
    owner.admission_apply(
        AdmissionCommand::Root {
            account: BudgetAccount {
                identity: identity.clone(),
                parent: None,
                parent_attempt: None,
                campaign: campaign.clone(),
                tenant: tenant.clone(),
                deadline_ms: deadline,
                available: local_amounts()?,
                consumed: Amounts::new(),
                children: 0,
            },
        },
        now,
    )?;
    let accounting_basis = policy.accounting_basis.clone();
    let usage = TenantUsage {
        tenant: tenant.clone(),
        share: 1,
        provider: provider.clone(),
        interval_start: now.saturating_sub(180_000),
        interval_end: now,
        observed_at: now,
        resources: BTreeMap::from([(
            dimension.clone(),
            UsageValue {
                amount: 0,
                capacity: Some(1),
            },
        )]),
    };
    owner.admission_apply(
        AdmissionCommand::Observe {
            observation: UsageObservation {
                usage,
                generation: 1,
                accounting_basis,
                covered: BTreeMap::new(),
                complete: true,
            },
        },
        now,
    )?;
    owner.admission_apply(
        AdmissionCommand::Enqueue {
            item: QueueItem {
                work_version: identity.clone(),
                account: identity.clone(),
                tenant,
                campaign,
                repository: Alias::new("qualification-disposable").map_err(diagnostic)?,
                priority: work.priority,
                bytes: 1,
                enqueued_at_ms: now,
                deadline_ms: deadline,
                eligible_at_ms: now,
                claim: None,
                phase: AdmissionPhase::PreClaim,
                reason: AdmissionReason::Waiting,
                order: 0,
                required_owners: BTreeMap::from([(provider.clone(), ChargeKind::WorkerRequest)]),
                shared_allocations: Vec::new(),
                budget: local_amounts()?,
            },
        },
        now,
    )?;
    owner.admission_apply(
        AdmissionCommand::Select {
            inputs: vec![EligibilityInput {
                work_version: identity,
                reason: AdmissionReason::Eligible,
                provider,
                generation: 1,
                observed_at_ms: now,
                valid_until_ms: deadline,
            }],
        },
        now,
    )?;
    Ok(())
}

struct LocalFixtureOwner;
impl LocalFixtureOwner {
    fn reply(
        request: &kiln_core::fair_admission::OwnerReservation,
        state: kiln_core::fair_admission::ReservationState,
    ) -> OwnerReply {
        OwnerReply {
            version: 1,
            operation: request.operation.clone(),
            provider: request.provider.clone(),
            generation: request.generation,
            receipt: request.operation.clone(),
            state,
            retry_at_ms: None,
            serving_bounds: None,
        }
    }
}
impl AdmissionOwnerPortV1 for LocalFixtureOwner {
    fn reserve(
        &mut self,
        request: &kiln_core::fair_admission::OwnerReservation,
    ) -> Result<OwnerReply, OwnerPortFailure> {
        Ok(Self::reply(
            request,
            kiln_core::fair_admission::ReservationState::Committed,
        ))
    }
    fn reconcile(
        &mut self,
        request: &kiln_core::fair_admission::OwnerReservation,
    ) -> Result<OwnerReply, OwnerPortFailure> {
        Ok(Self::reply(
            request,
            kiln_core::fair_admission::ReservationState::Committed,
        ))
    }
    fn cancel(
        &mut self,
        _request: &kiln_core::fair_admission::OwnerReservation,
    ) -> Result<OwnerReply, OwnerPortFailure> {
        Err(OwnerPortFailure::CancellationObserved)
    }
}

// This owner makes only a one-request local fixture reservation; no provider is contacted or qualified.
fn reserve_local_fixture(
    owner: &mut Implementer,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    plan: &ImplementationDispatchPlan,
    now: u64,
) -> Result<(), String> {
    let mut ledger = owner.admission_snapshot()?;
    let policy = ledger
        .policy
        .as_ref()
        .ok_or("fixture admission policy missing")?;
    let item = ledger
        .items
        .get(&attempt.work_identity)
        .ok_or("fixture work is not queued")?;
    if item.claim.as_ref()
        != Some(&kiln_core::fair_admission::ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        })
    {
        return Err("fixture queue claim differs from actual attempt/fence".into());
    }
    let input = ledger
        .eligibility
        .get(&attempt.work_identity)
        .cloned()
        .ok_or("fixture eligibility observation missing")?;
    if input.provider != local_provider()?
        || input.reason != AdmissionReason::Eligible
        || input.valid_until_ms <= now
    {
        return Err("fixture eligibility is stale or belongs to another owner".into());
    }
    if item.required_owners.len() != 1
        || !item.required_owners.contains_key(&input.provider)
        || item.required_owners.get(&input.provider) != Some(&ChargeKind::WorkerRequest)
    {
        return Err("fixture queue requires an unsupported owner grant".into());
    }
    let operation = attempt.identity.clone();
    let profile_revision = plan
        .execution
        .as_ref()
        .ok_or("execution bindings missing")?
        .requirements
        .needs
        .profile_revision
        .clone();
    let subject = ContentId::from_blake3_hex(profile_revision)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let reservation = kiln_core::fair_admission::OwnerReservation {
        version: 1,
        operation: operation.clone(),
        provider: input.provider.clone(),
        generation: input.generation,
        work_version: attempt.work_identity.clone(),
        claim: kiln_core::fair_admission::ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        },
        account: item.account.clone(),
        kind: ChargeKind::WorkerRequest,
        subject,
        accounting_basis: policy.accounting_basis.clone(),
        serving_bounds: None,
        amounts: local_amounts()?,
        expires_at_ms: item.deadline_ms.min(attempt.lease_until_ms),
        state: kiln_core::fair_admission::ReservationState::Tentative,
        receipt: None,
    };
    let outcome = owner.admission_apply(
        AdmissionCommand::Request {
            reservation: reservation.clone(),
            inputs: vec![input],
        },
        now,
    )?;
    let mut local = LocalFixtureOwner;
    let reply = match outcome {
        AdmissionResult::Retained {
            reservation: retained,
        } if retained.state == kiln_core::fair_admission::ReservationState::Committed => {
            return Ok(());
        }
        AdmissionResult::Retained {
            reservation: retained,
        } => local
            .reconcile(&retained)
            .map_err(|_| "local fixture reservation reconciliation failed")?,
        AdmissionResult::Updated => local
            .reserve(&reservation)
            .map_err(|_| "local fixture reservation failed")?,
        _ => {
            return Err(
                "local fixture admission did not request its bounded owner reservation".into(),
            );
        }
    };
    owner.admission_apply(AdmissionCommand::Reply { reply }, now)?;
    ledger = owner.admission_snapshot()?;
    if !ledger
        .reservations
        .get(&operation)
        .is_some_and(|r| r.state == kiln_core::fair_admission::ReservationState::Committed)
    {
        return Err("local fixture owner reservation was not retained as committed".into());
    }
    Ok(())
}
fn prepare_ci(flags: &BTreeMap<String, String>) -> Result<Value, String> {
    exact_flags(
        flags,
        &["--verification-profile", "--service-profile-template"],
    )?;
    let verification: VerificationProfile = read_json(&required(flags, "--verification-profile")?)?;
    let seed: ServiceProfile = read_json(&required(flags, "--service-profile-template")?)?;
    let mut service = seed;
    verification.identity()?;
    if verification.repository != "qualification/disposable" {
        return Err("verification profile is not bound to qualification/disposable".into());
    }
    if !matches!(service.projects.as_slice(), [only] if only.id == "qualification") {
        return Err("CI service seed must contain only the qualification project".into());
    }
    let project = service
        .projects
        .first_mut()
        .ok_or("service template requires one project")?;
    for check in &verification.checks {
        let node = project
            .workflow
            .nodes
            .iter_mut()
            .find(|n| n.id == check.id)
            .ok_or_else(|| format!("service template lacks verification node {}", check.id))?;
        node.kind = check.kind.clone();
        node.operation_identity =
            Engine::implementation_verification_operation(&verification, &check.id)?
                .as_str()
                .to_owned();
    }
    if verification.cairn.is_some() {
        let id = "cairn-authoritative-completion";
        let node = project
            .workflow
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or("service template lacks native Cairn verification node")?;
        node.kind = "cairn-agent-completion-v2".into();
        node.operation_identity = Engine::implementation_verification_operation(&verification, id)?
            .as_str()
            .to_owned();
    }
    Engine::validate_implementation_verification_project(&verification, project)?;
    service.admit()?;
    Ok(
        json!({"schema":"kiln.scripted-ci-binding.v1","scope":"synthetic-local-only","service_profile":service}),
    )
}
fn prepare_intake(flags: &BTreeMap<String, String>) -> Result<Value, String> {
    exact_flags(flags, &["--database", "--enrollment"])?;
    let enrollment: Enrollment = read_json(&required(flags, "--enrollment")?)?;
    let mut owner = Implementer::open(&required(flags, "--database")?, enrollment.clone())?;
    let work = owner.work()?;
    let ranked = work.ranked();
    let current: Vec<_> = work
        .current()
        .filter(|w| {
            fixture_scope(w, &enrollment).is_ok()
                && w.identity()
                    .is_ok_and(|id| ranked.iter().any(|candidate| candidate.identity == id))
        })
        .collect();
    let [current] = current.as_slice() else {
        return Err(format!(
            "expected exactly one current ready fixture work, found {}",
            current.len()
        ));
    };
    let current = *current;
    let winner = current
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let now = clock_ms()?;
    let mut ledger = owner.admission_snapshot()?;
    if ledger.policy.is_none() {
        if !ledger.items.is_empty()
            || !ledger.accounts.is_empty()
            || !ledger.reservations.is_empty()
            || !ledger.observations.is_empty()
            || !ledger.eligibility.is_empty()
        {
            return Err("refusing to overwrite unrelated partial admission state".into());
        }
        setup_local_admission(&mut owner, current, now)?;
        ledger = owner.admission_snapshot()?;
    }
    if ledger.policy.as_ref() != Some(&local_policy(current)?) {
        return Err("admission policy differs from bounded scripted-local fixture policy".into());
    }
    if !ledger.items.contains_key(&winner) {
        return Err(
            "admission policy is configured without the current fixture work queue row".into(),
        );
    }
    let fresh: Vec<EligibilityInput> = ledger
        .eligibility
        .values()
        .filter(|i| i.valid_until_ms >= now)
        .cloned()
        .collect();
    let selected = owner.admission_apply(AdmissionCommand::Select { inputs: fresh }, now)?;
    if !matches!(&selected,AdmissionResult::Selected{work_version:Some(id)} if id==&winner) {
        return Err("native admission selector disagrees with unique ready fixture work".into());
    }
    let after = owner.admission_snapshot()?;
    let item = after
        .items
        .get(&winner)
        .ok_or("native admission policy lost the fixture queue row")?;
    let provider = local_provider()?;
    if after.items.len() != 1
        || item.phase != AdmissionPhase::PreClaim
        || item.reason != AdmissionReason::Eligible
        || item.claim.is_some()
        || item.account != winner
        || item.campaign.as_str() != "scripted_local"
        || item.tenant.as_str() != "scripted_local"
        || item.repository.as_str() != "qualification-disposable"
        || item.required_owners != BTreeMap::from([(provider.clone(), ChargeKind::WorkerRequest)])
        || item.budget != local_amounts()?
    {
        return Err(
            "native admission queue row is not the exact pre-claim synthetic fixture row".into(),
        );
    }
    if !after.eligibility.get(&winner).is_some_and(|input| {
        input.provider == provider
            && input.reason == AdmissionReason::Eligible
            && input.valid_until_ms > now
    }) {
        return Err("fixture work lacks current local eligibility observation".into());
    }
    Ok(
        json!({"schema":"kiln.scripted-intake-binding.v1","scope":"synthetic-local-only","work":winner}),
    )
}
fn prepare_remote(flags: &BTreeMap<String, String>) -> Result<Value, String> {
    exact_flags(flags, &["--profile"])?;
    let profile: NativeRemoteProfile = read_json(&required(flags, "--profile")?)?;
    if profile.transport != NativeTransport::LocalBareFixture
        || !profile.remote_url.starts_with('/')
        || !Path::new(&profile.remote_url).is_absolute()
    {
        return Err("fixture remote must be an absolute local bare fixture path".into());
    }
    let path = Path::new(&profile.remote_url);
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("fixture remote must be a non-symlink local bare directory".into());
    }
    let repository = git2::Repository::open_bare(path).map_err(|e| e.to_string())?;
    if !repository.is_bare() {
        return Err("fixture remote is not a bare Git repository".into());
    }
    let identity = kiln_adapter_git::native_publication::remote_identity(&profile)
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"schema":"kiln.scripted-remote-binding.v1","scope":"synthetic-local-only","remote_identity":identity}),
    )
}
fn prepare_remote_template(flags: &BTreeMap<String, String>) -> Result<Value, String> {
    exact_flags(flags, &["--profile"])?;
    let profile: NativeRemoteProfile = read_json(&required(flags, "--profile")?)?;
    let path = Path::new(&profile.remote_url);
    if profile.transport != NativeTransport::LocalBareFixture
        || !path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(
            "fixture remote template must name an absolute normalized local-only path".into(),
        );
    }
    let identity = kiln_adapter_git::native_publication::remote_identity(&profile)
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"schema":"kiln.scripted-remote-binding.v1","scope":"synthetic-local-template","remote_identity":identity}),
    )
}

// r[impl kiln.source.projection.route.scenario.dispatch]
fn bind_execution(flags: &BTreeMap<String, String>) -> Result<Value, String> {
    const ROUTE: &str = kiln_adapter_lattice::projected_workspace::ROUTE_FLAG;
    let mut expected = vec![
        "--database",
        "--enrollment",
        "--attempt",
        "--fence",
        "--worker-request",
        "--dispatch-plan-template",
    ];
    if flags.contains_key(ROUTE) {
        expected.push(ROUTE);
    }
    exact_flags(flags, &expected)?;
    // A projected workspace replaces the fixture's fixed source paths with
    // the exact route of the plan's pinned overlay.
    let projected: Option<ProjectedWorkspaceRoute> = match flags.get(ROUTE) {
        Some(path) => {
            let route: ProjectedWorkspaceRoute = read_json(Path::new(path))?;
            route.verify().map_err(|diagnostics| {
                format!("projected workspace route is invalid: {diagnostics:?}")
            })?;
            Some(route)
        }
        None => None,
    };
    let enrollment: Enrollment = read_json(&required(flags, "--enrollment")?)?;
    let attempt = ContentId::from_blake3_hex(flags["--attempt"].clone())
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let fence = flags["--fence"]
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("fence must be positive")?;
    let mut owner = Implementer::open(&required(flags, "--database")?, enrollment.clone())?;
    let now = clock_ms()?;
    let actual = owner.authorize_current(&attempt, fence, now)?;
    let work = owner
        .work()?
        .current()
        .find(|w| w.identity().is_ok_and(|id| id == actual.work_identity))
        .cloned()
        .ok_or("attempt work is not current")?;
    fixture_scope(&work, &enrollment)?;
    let acceptance = acceptance_digest(&work).map_err(|e| format!("{}: {}", e.code, e.message))?;
    let mut request: Value = read_json(&required(flags, "--worker-request")?)?;
    if request.get("schema").and_then(Value::as_str)
        != Some("lattice.implementation-attempt-run.v3")
    {
        return Err(
            "worker request must be a native prepared lattice.implementation-attempt-run.v3 request"
                .into(),
        );
    }
    if request.get("dispatch_plan").is_some() {
        return Err("prepared fixture request already names a dispatch plan; bind it once".into());
    }
    let envelope = request
        .get("envelope")
        .and_then(Value::as_object)
        .ok_or("request envelope missing")?;
    for (key, expected) in [
        ("work_id", actual.work_identity.as_str().to_owned()),
        ("work_version", actual.work_revision.to_string()),
        ("attempt_id", attempt.as_str().to_owned()),
        ("fence", fence.to_string()),
        ("repository", work.repository.clone()),
        ("task_spec_digest", acceptance.as_str().to_owned()),
        (
            "verification_profile",
            work.verification_profile.as_str().to_owned(),
        ),
    ] {
        let got = envelope
            .get(key)
            .ok_or_else(|| format!("request envelope lacks {key}"))?;
        let matches = if key == "work_version" || key == "fence" {
            got.as_u64().is_some_and(|n| n.to_string() == expected)
        } else {
            got.as_str() == Some(expected.as_str())
        };
        if !matches {
            return Err(format!(
                "prepared request {key} differs from actual admitted work/attempt"
            ));
        }
    }
    let source = work
        .aliases
        .iter()
        .find(|s| matches!(s.kind, kiln_core::implementation::WorkSourceKind::CairnTask))
        .ok_or("scripted fixture requires canonical Cairn source")?;
    for (k, v) in [
        ("source_kind", "cairn_task".to_string()),
        ("source_id", source.native_id.clone()),
        ("readiness_snapshot", source.version.as_str().to_owned()),
    ] {
        if envelope.get(k).and_then(Value::as_str) != Some(v.as_str()) {
            return Err(format!("request {k} differs from admitted source"));
        }
    }
    let base = work
        .base
        .as_ref()
        .ok_or("fixture work lacks its exact source base")?;
    if envelope.get("base").and_then(Value::as_str) != Some(base.native_id.as_str()) {
        return Err("prepared request base differs from actual canonical work".into());
    }
    let expires = request
        .get("expires_at_ms")
        .and_then(Value::as_u64)
        .ok_or("prepared request expiry missing")?;
    let expected_expiry = actual
        .lease_until_ms
        .min(actual.claimed_at_ms.saturating_add(180_000));
    if expires != expected_expiry || expires <= now {
        return Err(
            "prepared request expiry differs from current claim lease and fixture window".into(),
        );
    }
    let exact_strings = |field: &str, expected: &[&str]| {
        request
            .get(field)
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<Vec<_>>>()
                    .is_some_and(|got| got == expected)
            })
    };
    // The brokered OMP route has no network device: only its attempt-local
    // model broker reaches the scripted loopback endpoint.
    let broker = request
        .pointer("/omp/model_broker")
        .and_then(Value::as_object)
        .ok_or("prepared request lacks its model broker")?;
    let routes = request
        .pointer("/execution_binding/model_routes/routes")
        .and_then(Value::as_array)
        .ok_or("prepared request lacks its model routes")?;
    let [route] = routes.as_slice() else {
        return Err("prepared request must declare exactly the primary model route".into());
    };
    let route_identity = route
        .get("route_identity")
        .and_then(Value::as_str)
        .filter(|_| route.get("name").and_then(Value::as_str) == Some("primary"))
        .ok_or("prepared request must declare exactly the primary model route")?
        .to_owned();
    let host_local = json!({"kind": "host_local"});
    let strings = |field: &str| -> Option<Vec<String>> {
        request
            .get(field)?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect()
    };
    let paths_bound = match &projected {
        Some(route) => match (
            request.get("workspace_root").and_then(Value::as_str),
            strings("read_paths"),
            strings("write_paths"),
        ) {
            (Some(root), Some(read), Some(write)) => route.admits(root, &read, &write),
            _ => false,
        },
        None => {
            exact_strings("read_paths", &["src", ".cairn"])
                && exact_strings("write_paths", &["src/scripted_model_result.txt"])
        }
    };
    if !paths_bound
        || request.get("network").and_then(Value::as_bool) != Some(false)
        || request.get("network_namespace").is_some()
        || request.get("worker_compute") != Some(&host_local)
        || request
            .pointer("/execution_binding/schema")
            .and_then(Value::as_str)
            != Some("lattice.execution-binding.v1")
        || request.pointer("/execution_binding/compute") != Some(&host_local)
        || broker.get("schema").and_then(Value::as_str) != Some("lattice.candidate-model-broker.v1")
        || broker.get("model").and_then(Value::as_str) != Some("scripted-v1")
        || !broker.get("credential").is_some_and(Value::is_null)
        || !broker
            .get("endpoint")
            .and_then(Value::as_str)
            .is_some_and(|endpoint| endpoint.starts_with("http://127.0.0.1:"))
        || request.get("output_limit_bytes").and_then(Value::as_u64) != Some(1_048_576)
        || request.get("allow_unchanged").and_then(Value::as_bool) != Some(false)
    {
        return Err(
            "prepared request differs from immutable fixture filesystem/compute/broker/output grants"
                .into(),
        );
    }
    let budget = envelope
        .get("budget")
        .and_then(Value::as_object)
        .ok_or("prepared request budget missing")?;
    if budget.get("max_turns").and_then(Value::as_u64) != Some(4)
        || budget.get("max_seconds").and_then(Value::as_u64) != Some(90)
        || budget.get("max_cost_micros").and_then(Value::as_u64) != Some(0)
    {
        return Err("prepared request differs from exact scripted fixture budget".into());
    }
    let admission = owner.admission_snapshot()?;
    if admission.policy.as_ref() != Some(&local_policy(&work)?) {
        return Err("current admission policy is not the synthetic fixture policy".into());
    }
    let policy = admission.policy_ref().map_err(|e| format!("{e:?}"))?;
    if envelope.get("policy_digest").and_then(Value::as_str) == Some(policy.as_str()) {
        return Err(
            "prepared policy_digest is already bound; refusing non-corrective fixture binding"
                .into(),
        );
    }
    request
        .get_mut("envelope")
        .and_then(Value::as_object_mut)
        .ok_or("request envelope missing")?
        .insert(
            "policy_digest".into(),
            Value::String(policy.as_str().to_owned()),
        );
    let mut plan: ImplementationDispatchPlan =
        read_json(&required(flags, "--dispatch-plan-template")?)?;
    if plan.schema != DISPATCH_PLAN_SCHEMA
        || plan.work_identity != actual.work_identity
        || plan.work_revision != actual.work_revision
        || plan.attempt_identity != attempt
        || plan.fence != fence
        || plan.repository != work.repository
        || plan.base != work.base
        || plan.provider_profile.as_str() != "scripted-local-omp"
        || match &projected {
            Some(route) => {
                plan.permitted_paths != work.permitted_paths
                    || plan.workspace.projection.as_ref() != Some(&route.pin)
                    || plan.workspace.identity != route.pin.overlay
            }
            None => plan.permitted_paths != vec!["src/scripted_model_result.txt".to_string()],
        }
        || plan.permitted_effects != BTreeSet::from(["write_source".to_string()])
        || plan.permitted_effects != work.permitted_effects
        || plan.deadline_minutes != 2
        || plan.invocation_budget != 4
        || plan.token_budget != 4096
        || plan.workspace.network
        || plan.workspace.credentials
    {
        return Err("dispatch template differs from exact scripted local work/profile/workspace/budget contract".into());
    }
    let envelope = request
        .get("envelope")
        .and_then(Value::as_object)
        .ok_or("request envelope missing")?;
    if request.get("network").and_then(Value::as_bool) != Some(false)
        || envelope.get("workspace_id").and_then(Value::as_str)
            != Some(plan.workspace.identity.as_str())
    {
        return Err("prepared request differs from immutable dispatch workspace/network".into());
    }
    let worker_profile = envelope
        .get("worker_profile")
        .and_then(Value::as_str)
        .ok_or("prepared worker profile missing")?
        .to_string();
    let model = request
        .pointer("/omp/model")
        .and_then(Value::as_str)
        .ok_or("prepared request model declaration missing")?;
    if model != "lattice/scripted-v1" {
        return Err("prepared request is not the reviewed scripted local model".into());
    }
    let requirements = kiln_core::inventory::WorkNeedsAttachment::new(
        actual.work_identity.clone(),
        actual.work_revision,
        policy.clone(),
        WorkNeeds {
            work_version: actual.work_identity.as_str().to_owned(),
            task_family: "scripted_implementation".into(),
            output_contract: plan.expected_output.as_str().to_owned(),
            definition: FactRef {
                source: "scripted-local-fixture".into(),
                id: "implementation-contract".into(),
            },
            profile_revision: worker_profile.clone(),
            policy_ref: policy.as_str().to_owned(),
            harness: "omp".into(),
            tools: vec!["write".into()],
            sandbox: "isolated_snapshot".into(),
            execution_backend: "native_lattice_local".into(),
            model_backend: "scripted_local".into(),
            context_tokens: 4096,
            execution_resources: vec![ResourceRequirement {
                unit: "worker_requests".into(),
                amount: 1,
            }],
            modalities: vec!["text".into()],
            model_artifact: None,
            qualification_subject: "scripted-local-only".into(),
            qualification_corpus: "qualification-disposable".into(),
            qualification_protocol: "scripted-local-omp-v1".into(),
        },
    )
    .map_err(str::to_string)?;
    let local_source = "scripted-local-fixture".to_string();
    let execution_grant = attempt.as_str().to_string();
    let native_profile = ContentId::from_blake3_hex(worker_profile.clone())
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let refs = AttemptBinding {
        work_version: actual.work_identity.as_str().to_owned(),
        attempt: attempt.as_str().to_owned(),
        fence,
        worker: FactRef {
            source: local_source.clone(),
            id: worker_profile,
        },
        execution_grant: FactRef {
            source: local_source.clone(),
            id: execution_grant,
        },
        model_services: vec![FactRef {
            source: local_source.clone(),
            id: model.to_string(),
        }],
        request_admissions: vec![FactRef {
            source: local_source,
            id: route_identity,
        }],
    };
    plan.execution = Some(DispatchExecutionBindings {
        requirements: requirements.clone(),
        attempt: refs,
        native_request_profile: Some(native_profile),
    });
    let plan_identity = admit_dispatch_plan(&work, &actual, &plan)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    // The worker profile does not frame per-attempt envelope values or the
    // dispatch plan, so preparing this seed again keeps the bound profile.
    let seed = request
        .as_object_mut()
        .ok_or("prepared request is not an object")?;
    seed.insert(
        "dispatch_plan".into(),
        Value::String(plan_identity.as_str().to_owned()),
    );
    seed.insert("admitted_envelope".into(), Value::String(String::new()));
    owner.attach_work_needs(&requirements)?;
    let retained = owner
        .work_needs(&actual.work_identity, &policy)?
        .ok_or("native work needs attachment not retained")?;
    if retained != requirements {
        return Err("dispatch work needs differs from retained native attachment".into());
    }
    reserve_local_fixture(&mut owner, &actual, &plan, clock_ms()?)?;
    let now = clock_ms()?;
    let observations: Vec<EligibilityInput> = owner
        .admission_snapshot()?
        .eligibility
        .into_values()
        .filter(|i| i.valid_until_ms >= now)
        .collect();
    let mut binding = json!({"schema":"kiln.scripted-execution-binding.v2","scope":"synthetic-local-only","dispatch_plan":plan,"dispatch_plan_identity":plan_identity,"admission_observations":observations,"worker_seed":request});
    if let (Some(route), Some(fields)) = (projected, binding.as_object_mut()) {
        fields.insert(
            "projected_workspace_route".into(),
            serde_json::to_value(route).map_err(|e| e.to_string())?,
        );
    }
    Ok(binding)
}
fn run() -> Result<(), String> {
    let (op, flags) = parse(std::env::args().skip(1))?;
    let result = match op.as_str() {
        "prepare-ci" => prepare_ci(&flags)?,
        "prepare-intake" => prepare_intake(&flags)?,
        "bind-execution" => bind_execution(&flags)?,
        "prepare-remote" => prepare_remote(&flags)?,
        "prepare-remote-template" => prepare_remote_template(&flags)?,
        _ => return Err(USAGE.into()),
    };
    let bytes = serde_json::to_vec(&result).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_OUTPUT {
        return Err("helper output exceeds 16 MiB bound".into());
    }
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("kiln-implementation-fixture: {e}\n{USAGE}");
        std::process::exit(2)
    }
}
