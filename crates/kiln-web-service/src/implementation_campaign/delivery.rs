//! Selected native publication, integration, and lifecycle campaign owner.
use std::{ffi::OsString, io::Read, path::PathBuf};

use kiln_core::{
    implementation::{
        AttemptPhase,
        campaign::{CampaignOperation, EffectState, MemberStage, RecoveryDisposition},
    },
    types::ContentId,
};
use kiln_implementer::{
    campaign::CampaignOwnerObservation, campaign_consumer::CanonicalCampaignLedgers,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeDeliveryProfile {
    pub route: NativeDeliveryRoute,
    pub github: Option<GitHubDeliveryProfile>,
    pub native_git: Option<NativeGitDeliveryProfile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeDeliveryRoute {
    GithubPullRequest,
    NativeGit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitHubDeliveryProfile {
    pub publisher: super::RoleProgram,
    pub integrator: super::RoleProgram,
    pub lifecycle_observer: super::RoleProgram,
    pub enrollment: PathBuf,
    pub secret_file: PathBuf,
    pub remote_profile: PathBuf,
    pub publication_policy: GitHubPublicationPolicy,
    pub landing_policy: GitHubLandingPolicy,
    pub lifecycle_action: kiln_implementer::publication_effects::CairnLifecycleAction,
    pub lifecycle_receipt_template: String,
    pub repository: PathBuf,
    pub authoritative_report_directory: PathBuf,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitHubPublicationPolicy {
    pub base_branch: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitHubLandingPolicy {
    pub required_checks: Vec<String>,
    pub required_approvals: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeGitDeliveryProfile {
    pub qualifier: super::RoleProgram,
    pub lifecycle_observer: super::RoleProgram,
    pub enrollment: PathBuf,
    pub service_profile: PathBuf,
    pub review_profile: PathBuf,
    pub repository_path: PathBuf,
    pub change_id: String,
    pub remote: kiln_adapter_git::native_publication::NativeRemoteProfile,
    pub provider_profile: String,
    pub expected_old_target: kiln_core::types::ExternalRef,
    pub target_ref: String,
    pub authoritative_report_directory: PathBuf,
    pub lifecycle_action: kiln_implementer::publication_effects::CairnLifecycleAction,
    pub lifecycle_receipt_template: String,
}

pub(super) fn validate(profile: &super::ImplementationCampaignDriveProfile) -> Result<(), String> {
    let delivery = &profile.delivery;
    let selected = match delivery.route {
        NativeDeliveryRoute::GithubPullRequest => delivery
            .github
            .as_ref()
            .ok_or("GitHub PR route requires its typed owner profile")?,
        NativeDeliveryRoute::NativeGit => {
            delivery
                .native_git
                .as_ref()
                .ok_or("native-Git route requires its typed owner profile")?;
            if delivery.github.is_some() {
                return Err(
                    "selected native-Git route cannot contain GitHub delivery authority".into(),
                );
            }
            return validate_native_git(
                profile,
                delivery
                    .native_git
                    .as_ref()
                    .ok_or("native-Git route profile missing")?,
            );
        }
    };
    if delivery.native_git.is_some() {
        return Err("selected GitHub PR route cannot contain native-Git delivery authority".into());
    }
    if selected.timeout_ms == 0 || selected.timeout_ms > 300_000 {
        return Err("GitHub delivery timeout must be between 1 and 300000 ms".into());
    }
    for path in [
        &selected.enrollment,
        &selected.secret_file,
        &selected.remote_profile,
        &selected.repository,
        &selected.authoritative_report_directory,
    ] {
        if !path.is_absolute() {
            return Err("GitHub delivery inputs must be absolute paths".into());
        }
    }
    if selected.publisher.uid != selected.integrator.uid {
        return Err("GitHub publisher and integration observer must use the credential-owning provider identity".into());
    }
    validate_secret_file(&selected.secret_file, selected.publisher.uid)?;
    if selected.publication_policy.base_branch.trim().is_empty()
        || selected.publication_policy.base_branch.len()
            > kiln_core::implementation::MAX_BRANCH_BYTES
        || selected.publication_policy.title.trim().is_empty()
        || selected.publication_policy.title.len() > kiln_core::implementation::MAX_TITLE_BYTES
        || selected.publication_policy.body.len() > kiln_core::implementation::MAX_BODY_BYTES
        || selected.landing_policy.required_approvals > 16
        || selected.landing_policy.required_checks.len() > 64
        || selected
            .landing_policy
            .required_checks
            .iter()
            .any(|check| check.trim().is_empty() || check.len() > 256)
        || selected
            .landing_policy
            .required_checks
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != selected.landing_policy.required_checks.len()
        || !valid_receipt_template(&selected.lifecycle_receipt_template)
    {
        return Err(
            "GitHub publication, landing, or lifecycle policy exceeds its admitted bounds".into(),
        );
    }
    for role in [
        &selected.publisher,
        &selected.integrator,
        &selected.lifecycle_observer,
    ] {
        validate_role(profile, role)?;
    }
    let excluded = [profile.roles.worker.uid, profile.roles.verifier.uid];
    if [
        selected.publisher.uid,
        selected.integrator.uid,
        selected.lifecycle_observer.uid,
    ]
    .iter()
    .any(|uid| excluded.contains(uid))
    {
        return Err(
            "GitHub delivery roles must remain outside worker and verifier identities".into(),
        );
    }
    Ok(())
}

fn validate_native_git(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &NativeGitDeliveryProfile,
) -> Result<(), String> {
    validate_role(profile, &config.qualifier)?;
    validate_role(profile, &config.lifecycle_observer)?;
    for path in [
        &config.enrollment,
        &config.service_profile,
        &config.review_profile,
        &config.repository_path,
        &config.authoritative_report_directory,
    ] {
        if !path.is_absolute() {
            return Err("native-Git delivery inputs must be absolute paths".into());
        }
    }
    if config.change_id.trim().is_empty()
        || config.change_id.len() > 256
        || config.provider_profile.trim().is_empty()
        || config.provider_profile.len() > 256
        || config.target_ref.trim().is_empty()
        || config.target_ref.len() > 1024
        || config.remote.transport
            == kiln_adapter_git::native_publication::NativeTransport::LocalBareFixture
        || !valid_receipt_template(&config.lifecycle_receipt_template)
    {
        return Err(
            "native-Git operator policy is incomplete or uses an unqualified transport".into(),
        );
    }
    kiln_adapter_git::native_publication::remote_identity(&config.remote).map_err(super::text)?;
    if [config.qualifier.uid, config.lifecycle_observer.uid]
        .iter()
        .any(|uid| *uid == profile.roles.worker.uid || *uid == profile.roles.verifier.uid)
    {
        return Err(
            "native-Git owner roles must remain outside worker and verifier identities".into(),
        );
    }
    Ok(())
}

fn valid_receipt_template(template: &str) -> bool {
    template.starts_with(".cairn/")
        && template.matches("{task}").count() == 1
        && template.len() <= 1024
        && template
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && !template.starts_with('/')
        && !template.contains('\\')
}

fn receipt_path_for_task(template: &str, native_id: &str) -> Result<String, String> {
    let task = native_id.rsplit('/').next().unwrap_or(native_id);
    if task.is_empty()
        || task == "."
        || task == ".."
        || !task
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("Cairn task identity cannot safely bind a lifecycle receipt path".into());
    }
    Ok(template.replace("{task}", task))
}

fn validate_role(
    profile: &super::ImplementationCampaignDriveProfile,
    role: &super::RoleProgram,
) -> Result<(), String> {
    if role.uid == 0
        || role.gid == 0
        || role.gid != profile.roles.worker.gid
        || !role.program.is_absolute()
        || role.blake3.as_str().len() != 64
    {
        return Err(
            "delivery role must be an absolute pinned program using a non-root isolated identity"
                .into(),
        );
    }
    Ok(())
}

fn validate_secret_file(path: &std::path::Path, uid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::symlink_metadata(path).map_err(super::text)?;
        if !metadata.is_file()
            || metadata.uid() != uid
            || metadata.nlink() != 1
            || metadata.mode() & 0o7777 != 0o400
        {
            return Err("GitHub token must be an ordinary singly-linked file owned by the isolated provider UID with mode 0400".into());
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid);
        Err("GitHub credential isolation requires Unix file ownership semantics".into())
    }
}

pub(super) fn consume(
    profile: &super::ImplementationCampaignDriveProfile,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    reconciliation: bool,
) -> Result<Option<CampaignOwnerObservation>, String> {
    if !matches!(
        effect.operation,
        CampaignOperation::Publish | CampaignOperation::Integrate | CampaignOperation::Lifecycle
    ) {
        return Err(format!(
            "delivery owner does not accept campaign operation {:?}",
            effect.operation
        ));
    }
    let expected_owner = if effect.operation == CampaignOperation::Lifecycle {
        "cairn-lifecycle-owner"
    } else {
        "publication-owner"
    };
    if effect.owner != expected_owner {
        return Err("campaign effect names a different native delivery owner".into());
    }
    let (work_id, attempt_id, fence) = match (
        &effect.work_identity,
        &effect.attempt_identity,
        effect.attempt_fence,
    ) {
        (Some(work), Some(attempt), Some(fence)) if fence > 0 => (work, attempt, fence),
        _ => return Err("delivery effect lacks canonical work, attempt, or fence binding".into()),
    };
    let before = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let work = before
        .work
        .current()
        .find(|w| w.identity().is_ok_and(|id| &id == work_id))
        .ok_or("delivery work version is no longer current")?;
    let attempt = before
        .attempts
        .attempts()
        .find(|a| &a.identity == attempt_id)
        .ok_or("delivery attempt is not retained")?;
    if attempt.work_identity != *work_id
        || attempt.fence != fence
        || attempt.handoff.is_none()
        || attempt.verification.is_none()
        || effect.policy_identity != current_member_policy(profile, &effect.member)?
    {
        return Err("delivery effect differs from the current member policy, verified candidate, or attempt fence".into());
    }
    let goal = selected_goal(profile, &effect.member)?;
    if !goal_allows_delivery(profile.delivery.route, effect.operation, goal) {
        return Err(format!(
            "selected {:?} route cannot perform {:?} for the {:?} completion goal",
            profile.delivery.route, effect.operation, goal
        ));
    }
    match profile.delivery.route {
        NativeDeliveryRoute::GithubPullRequest => {
            consume_github(profile, effect, work, attempt_id, fence, reconciliation)
        }
        NativeDeliveryRoute::NativeGit => {
            consume_native_git(profile, effect, work, attempt, fence, reconciliation)
        }
    }
}
pub(crate) fn route_supports_goal(
    route: NativeDeliveryRoute,
    goal: kiln_core::implementation::campaign::CompletionGoal,
) -> bool {
    use kiln_core::implementation::campaign::CompletionGoal as Goal;
    match route {
        NativeDeliveryRoute::GithubPullRequest => {
            matches!(goal, Goal::VerifiedCandidate | Goal::VerifiedPullRequest)
        }
        NativeDeliveryRoute::NativeGit => matches!(
            goal,
            Goal::VerifiedCandidate | Goal::IntegratedChange | Goal::NativeLifecycleComplete
        ),
    }
}

fn goal_allows_delivery(
    route: NativeDeliveryRoute,
    operation: CampaignOperation,
    goal: kiln_core::implementation::campaign::CompletionGoal,
) -> bool {
    use kiln_core::implementation::campaign::CompletionGoal as Goal;
    matches!(
        (route, operation, goal),
        (
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Publish,
            Goal::VerifiedPullRequest,
        ) | (
            NativeDeliveryRoute::NativeGit,
            CampaignOperation::Integrate,
            Goal::IntegratedChange | Goal::NativeLifecycleComplete,
        ) | (
            NativeDeliveryRoute::NativeGit,
            CampaignOperation::Lifecycle,
            Goal::NativeLifecycleComplete,
        )
    )
}

fn consume_github(
    profile: &super::ImplementationCampaignDriveProfile,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &ContentId,
    fence: u64,
    reconciliation: bool,
) -> Result<Option<CampaignOwnerObservation>, String> {
    let c = profile
        .delivery
        .github
        .as_ref()
        .ok_or("GitHub delivery profile missing")?;
    let source_profile = if effect.operation == CampaignOperation::Publish {
        Some(bound_github_source_profile(
            profile,
            c,
            attempt,
            reconciliation,
        )?)
    } else {
        None
    };
    let publication_path = if matches!(
        effect.operation,
        CampaignOperation::Publish | CampaignOperation::Integrate
    ) {
        ensure_publication_plan(profile, c, work, attempt, fence, reconciliation, effect)?
    } else {
        PathBuf::new()
    };
    let landing_path = if effect.operation == CampaignOperation::Integrate {
        ensure_landing_plan(profile, c, attempt, effect, reconciliation)?
    } else {
        PathBuf::new()
    };
    let (binding_path, report_path, receipt_path) =
        if effect.operation == CampaignOperation::Lifecycle {
            let (binding, report, receipt) = lifecycle_inputs(profile, c, work, attempt, fence)?;
            (
                write_bound_exact(profile, attempt, "cairn-binding", &binding, reconciliation)?,
                report,
                receipt,
            )
        } else {
            (PathBuf::new(), PathBuf::new(), String::new())
        };
    let mut args = vec![s(match effect.operation {
        CampaignOperation::Publish => "publish",
        CampaignOperation::Integrate => "recover-integration",
        CampaignOperation::Lifecycle => "observe-cairn-lifecycle",
        _ => unreachable!(),
    })];
    args.extend(base_args(&profile.implementation_database, &c.enrollment));
    let role = match effect.operation {
        CampaignOperation::Publish => {
            let source_profile = source_profile
                .as_ref()
                .ok_or("pinned GitHub source profile is absent")?;
            args.extend([
                s("--publication-file"),
                p(&publication_path),
                s("--secret-file"),
                p(&c.secret_file),
                s("--remote-profile"),
                p(source_profile),
                s("--timeout-ms"),
                s(&c.timeout_ms.to_string()),
            ]);
            &c.publisher
        }
        CampaignOperation::Integrate => {
            args.extend([
                s("--publication-file"),
                p(&publication_path),
                s("--landing-plan"),
                p(&landing_path),
                s("--secret-file"),
                p(&c.secret_file),
                s("--timeout-ms"),
                s(&c.timeout_ms.to_string()),
            ]);
            &c.integrator
        }
        CampaignOperation::Lifecycle => {
            args.extend([
                s("--attempt"),
                s(attempt.as_str()),
                s("--fence"),
                s(&fence.to_string()),
                s("--binding-file"),
                p(&binding_path),
                s("--repository"),
                p(&c.repository),
                s("--authoritative-report"),
                p(&report_path),
                s("--receipt-path"),
                s(&receipt_path),
            ]);
            &c.lifecycle_observer
        }
        _ => unreachable!(),
    };
    super::run_role(profile, role, &profile.bound_input_directory, &args)?;
    let work_identity = work.identity().map_err(super::show)?;
    readback(profile, effect, &work_identity, attempt, fence)
}

fn consume_native_git(
    profile: &super::ImplementationCampaignDriveProfile,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    fence: u64,
    reconciliation: bool,
) -> Result<Option<CampaignOwnerObservation>, String> {
    let c = profile
        .delivery
        .native_git
        .as_ref()
        .ok_or("native-Git delivery profile missing")?;
    let request_path = if effect.operation == CampaignOperation::Lifecycle {
        ensure_native_lifecycle_request(profile, c, work, attempt, fence, reconciliation)?
    } else {
        ensure_native_request(profile, c, work, attempt, fence, reconciliation)?
    };
    let mut args = vec![
        s("native-git"),
        s(if effect.operation == CampaignOperation::Lifecycle {
            "lifecycle-receipt"
        } else {
            "qualify"
        }),
        s("--request"),
        p(&request_path),
        s("--enrollment"),
        p(&c.enrollment),
        s("--service-profile"),
        p(&c.service_profile),
        s("--review-profile"),
        p(&c.review_profile),
    ];
    let role = if effect.operation == CampaignOperation::Lifecycle {
        let report = c
            .authoritative_report_directory
            .join(format!("{}.json", attempt.identity.as_str()));
        args.extend([
            s("--repository"),
            p(&c.repository_path),
            s("--report"),
            p(&report),
        ]);
        &c.lifecycle_observer
    } else {
        args.push(s("--execute"));
        &c.qualifier
    };
    super::run_role(profile, role, &profile.bound_input_directory, &args)?;
    readback(
        profile,
        effect,
        &work.identity().map_err(super::show)?,
        &attempt.identity,
        fence,
    )
}

fn current_member_policy(
    profile: &super::ImplementationCampaignDriveProfile,
    member: &str,
) -> Result<ContentId, String> {
    let observer =
        kiln_implementer::campaign::CampaignObservation::open(&profile.campaign_database)?;
    let (state, _) = observer
        .inspect(&profile.campaign_id)?
        .ok_or("campaign state unavailable")?;
    state
        .observations
        .get(member)
        .map(|observation| observation.policy_identity.clone())
        .ok_or_else(|| "selected campaign member has no current policy observation".into())
}

fn selected_goal(
    profile: &super::ImplementationCampaignDriveProfile,
    member: &str,
) -> Result<kiln_core::implementation::campaign::CompletionGoal, String> {
    let observer =
        kiln_implementer::campaign::CampaignObservation::open(&profile.campaign_database)?;
    let (state, _) = observer
        .inspect(&profile.campaign_id)?
        .ok_or("campaign state unavailable")?;
    let selected = state
        .contract
        .members
        .iter()
        .find(|binding| match &binding.selector {
            kiln_core::implementation::campaign::MemberSelector::WholeChange { change } => {
                member == format!("{change}/*")
            }
            kiln_core::implementation::campaign::MemberSelector::Tasks { change, tasks } => {
                member == format!("{change}/{}", tasks.join(","))
            }
        })
        .ok_or("campaign effect member is outside the frozen selected scope")?;
    Ok(selected.goal)
}

fn ensure_publication_plan(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &GitHubDeliveryProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt_id: &ContentId,
    fence: u64,
    reconciliation: bool,
    effect: &kiln_core::implementation::campaign::PendingEffect,
) -> Result<PathBuf, String> {
    let ledgers = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let attempt = ledgers
        .attempts
        .attempts()
        .find(|attempt| &attempt.identity == attempt_id)
        .ok_or("attempt is not retained")?;
    let handoff = attempt
        .handoff
        .as_ref()
        .ok_or("verified handoff is absent")?;
    let verification = attempt
        .verification
        .as_ref()
        .ok_or("canonical verification is absent")?;
    let handoff_id = kiln_core::implementation::admit_candidate_handoff(work, attempt, handoff)
        .map_err(super::show)?;
    let verification_id = kiln_core::implementation::admit_candidate_verification(
        work,
        attempt,
        handoff,
        verification,
    )
    .map_err(super::show)?;
    let plan = if let Some(record) = &attempt.publication {
        let plan = record.plan.clone();
        if plan.base_branch != config.publication_policy.base_branch
            || plan.title != config.publication_policy.title
            || plan.body != config.publication_policy.body
        {
            return Err(
                "configured GitHub publication policy differs from the retained canonical plan"
                    .into(),
            );
        }
        plan
    } else {
        if reconciliation {
            return Err("reconciliation has no retained publication plan; no publication effect may be repeated".into());
        }
        let base = handoff
            .base
            .clone()
            .ok_or("verified handoff has no publication base")?;
        let branch = kiln_core::implementation::branch_for(
            &work.identity().map_err(super::show)?,
            attempt_id,
        );
        let plan = kiln_core::implementation::PublicationPlan {
            schema: kiln_core::implementation::PUBLICATION_SCHEMA.into(),
            work_identity: work.identity().map_err(super::show)?,
            work_revision: work.revision,
            attempt: attempt_id.clone(),
            fence,
            handoff: handoff_id,
            verification: verification_id,
            repository: work.repository.clone(),
            base,
            base_branch: config.publication_policy.base_branch.clone(),
            candidate: handoff.candidate.clone(),
            tree: handoff.tree.clone(),
            branch,
            title: config.publication_policy.title.clone(),
            body: config.publication_policy.body.clone(),
        };
        let enrollment: kiln_core::implementation::Enrollment =
            read_bounded_json(&config.enrollment)?;
        let mut owner =
            kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
        owner.prepare_publication(work, attempt_id, fence, handoff, verification, &plan)?;
        let reread = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
        let retained = reread
            .attempts
            .attempts()
            .find(|row| &row.identity == attempt_id)
            .and_then(|row| row.publication.as_ref())
            .ok_or("publication prepare did not retain a canonical plan")?;
        if retained.plan != plan {
            return Err("canonical publication plan differs from the exact prepared inputs".into());
        }
        plan
    };
    let path = bound_path(profile, attempt_id, "publication");
    let bytes = serde_json::to_vec(&plan).map_err(super::text)?;
    if reconciliation {
        let enrollment: kiln_core::implementation::Enrollment =
            read_bounded_json(&config.enrollment)?;
        let mut owner =
            kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
        if owner
            .read_publication_effect(
                work,
                attempt_id,
                fence,
                kiln_implementer::publication_effects::PublicationEffectKind::Branch,
            )?
            .is_none()
            || owner
                .read_publication_effect(
                    work,
                    attempt_id,
                    fence,
                    kiln_implementer::publication_effects::PublicationEffectKind::PullRequest,
                )?
                .is_none()
        {
            return Err("publication reconciliation lacks both durable branch and pull-request intents; mutation is forbidden".into());
        }
        match std::fs::symlink_metadata(&path) {
            Ok(_) if read_bounded_bytes(&path)? == bytes => {}
            Ok(_) => {
                return Err(
                    "replayed publication input differs from the retained canonical plan".into(),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                super::write_private_idempotently(&path, &bytes)?
            }
            Err(error) => return Err(super::text(error)),
        }
    } else {
        super::write_private_idempotently(&path, &bytes)?;
    }
    let _ = effect;
    Ok(path)
}

fn ensure_landing_plan(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &GitHubDeliveryProfile,
    attempt_id: &ContentId,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    reconciliation: bool,
) -> Result<PathBuf, String> {
    let ledgers = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let attempt = ledgers
        .attempts
        .attempts()
        .find(|attempt| &attempt.identity == attempt_id)
        .ok_or("integration attempt is not retained")?;
    let publication = attempt
        .publication
        .as_ref()
        .ok_or("integration has no retained publication")?;
    let pull = publication
        .pull_request
        .as_ref()
        .ok_or("integration has no authoritative published pull request")?;
    let (repository, number) = pull
        .native_id
        .as_str()
        .rsplit_once('#')
        .ok_or("canonical pull request locator has no number")?;
    let (owner, repository_name) = repository
        .split_once('/')
        .ok_or("canonical pull request locator has no owner/repository")?;
    let pull_number = number.parse::<u64>().map_err(super::text)?;
    if owner.is_empty() || repository_name.is_empty() || pull_number == 0 {
        return Err("canonical pull request locator is invalid".into());
    }
    let plan = kiln_adapter_github::landing::LandingPlan {
        schema: kiln_adapter_github::landing::LANDING_PLAN_SCHEMA.into(),
        owner: owner.into(),
        repository: repository_name.into(),
        pull_number,
        base_ref: publication.plan.base_branch.clone(),
        base_head: publication.plan.base.native_id.as_str().into(),
        head_sha: publication.plan.candidate.native_id.as_str().into(),
        required_checks: config.landing_policy.required_checks.clone(),
        required_approvals: config.landing_policy.required_approvals,
        operation: effect.intent_identity.clone(),
    };
    let path = bound_path(profile, attempt_id, "landing");
    let bytes = serde_json::to_vec(&plan).map_err(super::text)?;
    if reconciliation {
        if read_bounded_bytes(&path)? != bytes {
            return Err(
                "retained landing policy differs from current read-only recovery request".into(),
            );
        }
    } else {
        super::write_private_idempotently(&path, &bytes)?;
    }
    Ok(path)
}

fn ensure_native_request(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &NativeGitDeliveryProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    fence: u64,
    reconciliation: bool,
) -> Result<PathBuf, String> {
    let handoff = attempt
        .handoff
        .as_ref()
        .ok_or("native-Git attempt has no canonical handoff")?;
    let verification = attempt
        .verification
        .as_ref()
        .ok_or("native-Git attempt has no canonical verification")?;
    let handoff_id = kiln_core::implementation::admit_candidate_handoff(work, attempt, handoff)
        .map_err(super::show)?;
    let verification_id = kiln_core::implementation::admit_candidate_verification(
        work,
        attempt,
        handoff,
        verification,
    )
    .map_err(super::show)?;
    let enrollment: kiln_core::implementation::Enrollment = read_bounded_json(&config.enrollment)?;
    let mut owner =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
    let policy = owner
        .admission_snapshot()?
        .policy
        .ok_or("native-Git current admission policy is absent")?
        .identity()
        .map_err(super::show)?;
    let remote_identity = kiln_adapter_git::native_publication::remote_identity(&config.remote)
        .map_err(super::text)?;
    let branch = kiln_core::implementation::branch_for(
        &work.identity().map_err(super::show)?,
        &attempt.identity,
    );
    let review = crate::code_review::observe_native_git_authority_from_profiles(
        &config.service_profile,
        &config.review_profile,
        &config.change_id,
        &work.actor,
    )?;
    let mut plan = kiln_core::implementation::NativeGitPlan {
        schema: kiln_core::implementation::NATIVE_GIT_SCHEMA.into(),
        work_identity: work.identity().map_err(super::show)?,
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence,
        handoff: handoff_id,
        verification: verification_id,
        review: review.profile_identity,
        checks: review.ci_identity,
        policy,
        repository: work.repository.clone(),
        remote_identity,
        provider_profile: config.provider_profile.clone(),
        branch_expected_old: None,
        branch_ref: format!("refs/heads/{branch}"),
        target_ref: config.target_ref.clone(),
        expected_old_target: config.expected_old_target.clone(),
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        branch_operation: attempt.identity.clone(),
        integration_operation: attempt.identity.clone(),
    };
    plan.branch_operation =
        kiln_core::implementation::native_branch_operation_id(&plan).map_err(super::show)?;
    plan.integration_operation =
        kiln_core::implementation::native_integration_operation_id(&plan).map_err(super::show)?;
    if reconciliation {
        if let Some((retained, _)) = owner.native_git_plan(&attempt.identity)? {
            if !same_native_plan_route(&plan, &retained) {
                return Err("native-Git replay policy differs from the retained operation; no new mutation is allowed".into());
            }
            let plan_identity = retained_identity(&mut owner, &attempt.identity)?;
            for kind in [
                kiln_implementer::native_publication::NativeGitEffectKind::BranchPublication,
                kiln_implementer::native_publication::NativeGitEffectKind::TargetIntegration,
            ] {
                let Some(intent) = owner.native_git_effect_reservation(&attempt.identity, kind)?
                else {
                    continue;
                };
                let expected_operation = match kind {
                    kiln_implementer::native_publication::NativeGitEffectKind::BranchPublication => &retained.branch_operation,
                    kiln_implementer::native_publication::NativeGitEffectKind::TargetIntegration => &retained.integration_operation,
                };
                if intent.plan_identity != plan_identity
                    || intent.plan != retained
                    || &intent.operation != expected_operation
                {
                    return Err("native-Git effect intent differs from the retained exact plan and operation identity".into());
                }
            }
        } else if owner
            .native_git_effect_reservation(
                &attempt.identity,
                kiln_implementer::native_publication::NativeGitEffectKind::BranchPublication,
            )?
            .is_some()
            || owner
                .native_git_effect_reservation(
                    &attempt.identity,
                    kiln_implementer::native_publication::NativeGitEffectKind::TargetIntegration,
                )?
                .is_some()
        {
            return Err("native-Git effect reservation exists without its retained plan".into());
        }
    }
    let request = kiln_implementer::native_publication::NativeGitQualificationRequest {
        schema: kiln_implementer::native_publication::NATIVE_GIT_QUALIFICATION_REQUEST_SCHEMA
            .into(),
        database: profile.implementation_database.clone(),
        repository_path: config.repository_path.clone(),
        change_id: config.change_id.clone(),
        actor: work.actor.clone(),
        work: work.identity().map_err(super::show)?,
        attempt: attempt.identity.clone(),
        fence,
        plan,
        remote: config.remote.clone(),
    };
    let path = bound_path(profile, &attempt.identity, "native-qualification");
    let bytes = serde_json::to_vec(&request).map_err(super::text)?;
    if reconciliation {
        let stored: kiln_implementer::native_publication::NativeGitQualificationRequest<
            kiln_adapter_git::native_publication::NativeRemoteProfile,
        > = read_bounded_json(&path)?;
        if stored != request {
            return Err("retained native-Git request differs from current exact candidate, policy, or operation".into());
        }
    } else {
        super::write_private_idempotently(&path, &bytes)?;
    }
    Ok(path)
}

fn ensure_native_lifecycle_request(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &NativeGitDeliveryProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    fence: u64,
    reconciliation: bool,
) -> Result<PathBuf, String> {
    let source = work
        .aliases
        .iter()
        .find(|source| source.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
        .ok_or("native lifecycle work has no retained Cairn task alias")?;
    if attempt.work_identity != work.identity().map_err(super::show)? || attempt.fence != fence {
        return Err("native lifecycle request differs from canonical work or fence".into());
    }
    let receipt_path =
        receipt_path_for_task(&config.lifecycle_receipt_template, &source.native_id)?;
    let request = kiln_implementer::native_publication::NativeGitLifecycleRequest {
        schema: kiln_implementer::native_publication::NATIVE_GIT_LIFECYCLE_REQUEST_SCHEMA.into(),
        database: profile.implementation_database.clone(),
        actor: work.actor.clone(),
        work: work.identity().map_err(super::show)?,
        attempt: attempt.identity.clone(),
        fence,
        change_id: config.change_id.clone(),
        receipt_path,
        action: config.lifecycle_action,
        remote: config.remote.clone(),
    };
    let path = bound_path(profile, &attempt.identity, "native-lifecycle");
    let bytes = serde_json::to_vec(&request).map_err(super::text)?;
    if reconciliation {
        let stored: kiln_implementer::native_publication::NativeGitLifecycleRequest<
            kiln_adapter_git::native_publication::NativeRemoteProfile,
        > = read_bounded_json(&path)?;
        if stored != request {
            return Err(
                "retained native lifecycle request differs from current exact attempt or policy"
                    .into(),
            );
        }
    } else {
        super::write_private_idempotently(&path, &bytes)?;
    }
    Ok(path)
}

fn same_native_plan_route(
    a: &kiln_core::implementation::NativeGitPlan,
    b: &kiln_core::implementation::NativeGitPlan,
) -> bool {
    a.schema == b.schema
        && a.work_identity == b.work_identity
        && a.work_revision == b.work_revision
        && a.attempt == b.attempt
        && a.fence == b.fence
        && a.handoff == b.handoff
        && a.verification == b.verification
        && a.policy == b.policy
        && a.repository == b.repository
        && a.remote_identity == b.remote_identity
        && a.provider_profile == b.provider_profile
        && a.branch_expected_old == b.branch_expected_old
        && a.branch_ref == b.branch_ref
        && a.target_ref == b.target_ref
        && a.expected_old_target == b.expected_old_target
        && a.candidate == b.candidate
        && a.tree == b.tree
}

fn retained_identity(
    owner: &mut kiln_implementer::Implementer,
    attempt: &ContentId,
) -> Result<ContentId, String> {
    owner
        .native_git_plan(attempt)?
        .map(|(_, identity)| identity)
        .ok_or_else(|| "native-Git plan identity is absent".into())
}

fn lifecycle_inputs(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &GitHubDeliveryProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &ContentId,
    fence: u64,
) -> Result<(Vec<u8>, PathBuf, String), String> {
    let ledgers = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let row = ledgers
        .attempts
        .attempts()
        .find(|row| &row.identity == attempt)
        .ok_or("lifecycle attempt is not retained")?;
    let handoff = row.handoff.as_ref().ok_or("lifecycle handoff is absent")?;
    let verification = row
        .verification
        .as_ref()
        .ok_or("lifecycle verification is absent")?;
    let completion: Vec<_> = verification
        .run
        .checks
        .iter()
        .filter(|check| check.name == "cairn-authoritative-completion")
        .collect();
    let completion = match completion.as_slice() {
        &[check] if check.outcome == "pass" && check.kind == "cairn-agent-completion-v2" => check,
        _ => {
            return Err(
                "lifecycle requires exactly one retained authoritative completion check".into(),
            );
        }
    };
    let completion_id = ContentId::from_blake3_hex(
        completion
            .artifact_digest
            .clone()
            .ok_or("authoritative completion artifact digest is absent")?,
    )
    .map_err(super::show)?;
    let binding = kiln_implementer::publication_effects::CairnLifecycleBinding {
        source: work
            .aliases
            .iter()
            .find(|source| source.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
            .cloned()
            .ok_or("lifecycle work has no retained Cairn task alias")?,
        handoff: kiln_core::implementation::admit_candidate_handoff(work, row, handoff)
            .map_err(super::show)?,
        verification: kiln_core::implementation::admit_candidate_verification(
            work,
            row,
            handoff,
            verification,
        )
        .map_err(super::show)?,
        authoritative_completion: completion_id,
        candidate: handoff.candidate.clone(),
        action: config.lifecycle_action,
    };
    if row.fence != fence {
        return Err("lifecycle request fence differs from canonical attempt".into());
    }
    let receipt = receipt_path_for_task(
        &config.lifecycle_receipt_template,
        &binding.source.native_id,
    )?;
    let report = config
        .authoritative_report_directory
        .join(format!("{}.json", attempt.as_str()));
    Ok((
        serde_json::to_vec(&binding).map_err(super::text)?,
        report,
        receipt,
    ))
}

fn bound_path(
    profile: &super::ImplementationCampaignDriveProfile,
    attempt: &ContentId,
    kind: &str,
) -> PathBuf {
    profile
        .bound_input_directory
        .join(format!("campaign-{}-{kind}.json", attempt.as_str()))
}

fn bound_github_source_profile(
    profile: &super::ImplementationCampaignDriveProfile,
    config: &GitHubDeliveryProfile,
    attempt: &ContentId,
    reconciliation: bool,
) -> Result<PathBuf, String> {
    let path = bound_path(profile, attempt, "github-source");
    if reconciliation {
        let _: kiln_adapter_github::publication::PublicationSourceProfile =
            read_bounded_json(&path)?;
        return Ok(path);
    }
    let _: kiln_adapter_github::publication::PublicationSourceProfile =
        read_bounded_json(&config.remote_profile)?;
    let bytes = read_bounded_bytes(&config.remote_profile)?;
    super::write_private_idempotently(&path, &bytes)?;
    Ok(path)
}

fn read_bounded_bytes(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(super::text)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
        return Err(
            "native delivery input must be an ordinary non-empty file within the 16 MiB bound"
                .into(),
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    std::fs::File::open(path)
        .map_err(super::text)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(super::text)?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err("native delivery input is empty or exceeds its 16 MiB bound".into());
    }
    Ok(bytes)
}

fn write_bound_exact(
    profile: &super::ImplementationCampaignDriveProfile,
    attempt: &ContentId,
    kind: &str,
    bytes: &[u8],
    reconciliation: bool,
) -> Result<PathBuf, String> {
    let path = bound_path(profile, attempt, kind);
    if reconciliation {
        if read_bounded_bytes(&path)? != bytes {
            return Err("retained bound input differs from exact reconciliation authority".into());
        }
    } else {
        super::write_private_idempotently(&path, bytes)?;
    }
    Ok(path)
}

fn readback(
    profile: &super::ImplementationCampaignDriveProfile,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    work: &ContentId,
    attempt_id: &ContentId,
    fence: u64,
) -> Result<Option<CampaignOwnerObservation>, String> {
    let ledgers = CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let work_row = ledgers
        .work
        .current()
        .find(|w| w.identity().is_ok_and(|id| &id == work))
        .ok_or("delivery work ceased to be current during owner read-back")?;
    let attempt = ledgers
        .attempts
        .attempts()
        .find(|a| &a.identity == attempt_id)
        .ok_or("delivery attempt disappeared during owner read-back")?;
    if attempt.work_identity != *work || attempt.fence != fence {
        return Err("delivery owner changed the canonical attempt binding".into());
    }
    let handoff = attempt
        .handoff
        .as_ref()
        .ok_or("delivery readback lost the candidate handoff")?;
    let verification = attempt
        .verification
        .as_ref()
        .ok_or("delivery readback lost canonical verification")?;
    kiln_core::implementation::admit_candidate_handoff(work_row, attempt, handoff)
        .map_err(super::show)?;
    kiln_core::implementation::admit_candidate_verification(
        work_row,
        attempt,
        handoff,
        verification,
    )
    .map_err(super::show)?;
    let (mut member, goal) = {
        let observer =
            kiln_implementer::campaign::CampaignObservation::open(&profile.campaign_database)?;
        let (state, _) = observer
            .inspect(&profile.campaign_id)?
            .ok_or("campaign state unavailable during delivery read-back")?;
        let goal = super::selected_member_goal(&state.contract, &effect.member)?;
        (
            state
                .observations
                .get(&effect.member)
                .cloned()
                .ok_or("campaign member observation unavailable")?,
            goal,
        )
    };
    if member.policy_identity != effect.policy_identity
        || member.subject_identity != effect.subject_identity
        || member.context_identity != effect.context_identity
    {
        return Err(
            "delivery policy, source subject, or decision context changed during owner read-back"
                .into(),
        );
    }
    member.work_identity = Some(work.clone());
    member.canonical_work_key = Some(work_row.canonical_key().map_err(super::show)?);
    member.attempt_identity = Some(attempt.identity.clone());
    member.attempt_fence = Some(fence);
    member.source_identity = work_row
        .aliases
        .iter()
        .find(|a| {
            matches!(
                a.kind,
                kiln_core::implementation::WorkSourceKind::CairnTask
                    | kiln_core::implementation::WorkSourceKind::CairnArtifact
            )
        })
        .map(|a| a.version.clone());
    member.stage = if attempt.publication.as_ref().is_some_and(|p| {
        matches!(
            p.phase,
            kiln_core::implementation::PublicationPhase::PullRequestPublished
                | kiln_core::implementation::PublicationPhase::Integrated
                | kiln_core::implementation::PublicationPhase::SourceReconciled
        )
    }) {
        MemberStage::VerifiedPullRequest
    } else if attempt.verification.is_some() {
        MemberStage::VerifiedCandidate
    } else {
        MemberStage::Blocked
    };
    member.candidate_freshness = if attempt.handoff.is_some() {
        kiln_core::implementation::campaign::Freshness::Current
    } else {
        kiln_core::implementation::campaign::Freshness::Unknown
    };
    member.verifier_freshness = if attempt.verification.is_some() {
        kiln_core::implementation::campaign::Freshness::Current
    } else {
        kiln_core::implementation::campaign::Freshness::Unknown
    };
    let native_integration = native_git_evidence(profile, attempt_id, fence, work);
    let native_lifecycle = native_lifecycle_evidence(
        profile,
        attempt_id,
        fence,
        work,
        native_integration.as_ref(),
    );
    let github_lifecycle = github_lifecycle_evidence(profile, work_row, attempt_id, fence);
    let lifecycle_identity = native_lifecycle.or(github_lifecycle);
    let publication_identity = attempt
        .publication
        .as_ref()
        .map(|record| {
            serde_json::to_vec(record)
                .map_err(super::text)
                .and_then(|bytes| super::id(blake3::hash(&bytes).to_hex().as_str()))
        })
        .transpose()?;
    if attempt
        .publication
        .as_ref()
        .and_then(|p| p.integration.as_ref())
        .is_some()
        || native_integration.is_some()
    {
        member.stage = MemberStage::Integrated;
        member.recovery = None;
    }
    if let Some(identity) = &native_integration {
        if !member.evidence.contains(identity) {
            member.evidence.push(identity.clone());
        }
    }
    if let Some(identity) = &publication_identity {
        if !member.evidence.contains(identity) {
            member.evidence.push(identity.clone());
        }
    }
    if let Some(identity) = &lifecycle_identity {
        member.stage = MemberStage::Complete;
        if !member.evidence.contains(identity) {
            member.evidence.push(identity.clone());
        }
    }
    if matches!(
        attempt.phase,
        AttemptPhase::Failed
            | AttemptPhase::Blocked
            | AttemptPhase::Cancelled
            | AttemptPhase::Superseded
    ) {
        member.recovery = Some(RecoveryDisposition::SemanticReplan);
    }
    let complete = match effect.operation {
        CampaignOperation::Publish => attempt.publication.as_ref().is_some_and(|p| {
            matches!(
                p.phase,
                kiln_core::implementation::PublicationPhase::PullRequestPublished
                    | kiln_core::implementation::PublicationPhase::Integrated
                    | kiln_core::implementation::PublicationPhase::SourceReconciled
            )
        }),
        CampaignOperation::Integrate => {
            attempt
                .publication
                .as_ref()
                .and_then(|p| p.integration.as_ref())
                .is_some()
                || native_integration.is_some()
        }
        CampaignOperation::Lifecycle => lifecycle_identity.is_some(),
        _ => false,
    };
    if !complete {
        return Ok(None);
    }
    super::retain_unit_milestone(goal, &mut member, work_row);
    let identity = lifecycle_identity
        .or(native_integration)
        .or(publication_identity)
        .ok_or("canonical delivery receipt identity is absent")?;
    Ok(Some(CampaignOwnerObservation {
        result: EffectState::ObservedComplete,
        evidence_identity: identity,
        member,
    }))
}

fn native_lifecycle_evidence(
    profile: &super::ImplementationCampaignDriveProfile,
    attempt: &ContentId,
    fence: u64,
    work: &ContentId,
    integration: Option<&ContentId>,
) -> Option<ContentId> {
    let config = profile.delivery.native_git.as_ref()?;
    let enrollment: kiln_core::implementation::Enrollment =
        read_bounded_json(&config.enrollment).ok()?;
    let mut owner =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment).ok()?;
    let receipt = owner.native_cairn_lifecycle(attempt).ok()??;
    let current_policy = owner.admission_snapshot().ok()?.policy?.identity().ok()?;
    if receipt.work_identity != *work
        || receipt.attempt != *attempt
        || receipt.fence != fence
        || Some(&receipt.integration) != integration
        || receipt.lifecycle_policy != current_policy
    {
        return None;
    }
    let bytes = serde_json::to_vec(&receipt).ok()?;
    super::id(blake3::hash(&bytes).to_hex().as_str()).ok()
}

fn github_lifecycle_evidence(
    profile: &super::ImplementationCampaignDriveProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &ContentId,
    fence: u64,
) -> Option<ContentId> {
    let config = profile.delivery.github.as_ref()?;
    let (binding_bytes, _, _) = lifecycle_inputs(profile, config, work, attempt, fence).ok()?;
    let binding: kiln_implementer::publication_effects::CairnLifecycleBinding =
        serde_json::from_slice(&binding_bytes).ok()?;
    let enrollment: kiln_core::implementation::Enrollment =
        read_bounded_json(&config.enrollment).ok()?;
    let mut owner =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment).ok()?;
    let reservation = owner
        .read_cairn_lifecycle_effect(work, attempt, fence, &binding)
        .ok()??;
    let Some(kiln_implementer::publication_effects::PublicationEffectResult::CairnLifecycle(_)) =
        reservation.observed.as_ref()
    else {
        return None;
    };
    let bytes = serde_json::to_vec(&reservation).ok()?;
    super::id(blake3::hash(&bytes).to_hex().as_str()).ok()
}

pub(super) fn native_git_evidence(
    profile: &super::ImplementationCampaignDriveProfile,
    attempt: &ContentId,
    fence: u64,
    work: &ContentId,
) -> Option<ContentId> {
    let config = profile.delivery.native_git.as_ref()?;
    let enrollment: kiln_core::implementation::Enrollment =
        read_bounded_json(&config.enrollment).ok()?;
    let mut owner =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment).ok()?;
    let current_policy = owner.admission_snapshot().ok()?.policy?.identity().ok()?;
    let receipt = owner.native_git_receipt(attempt).ok()??;
    if receipt.policy != current_policy {
        return None;
    }
    let ledgers = CanonicalCampaignLedgers::read(&profile.implementation_database).ok()?;
    let work_row = ledgers
        .work
        .current()
        .find(|row| row.identity().ok().as_ref() == Some(work))?;
    let attempt_row = ledgers
        .attempts
        .attempts()
        .find(|row| &row.identity == attempt)?;
    let handoff = attempt_row.handoff.as_ref()?;
    let verification = attempt_row.verification.as_ref()?;
    let handoff_id =
        kiln_core::implementation::admit_candidate_handoff(work_row, attempt_row, handoff).ok()?;
    let verification_id = kiln_core::implementation::admit_candidate_verification(
        work_row,
        attempt_row,
        handoff,
        verification,
    )
    .ok()?;
    if receipt.work_identity != *work
        || receipt.attempt != *attempt
        || receipt.work_revision != work_row.revision
        || receipt.fence != fence
        || receipt.handoff != handoff_id
        || receipt.verification != verification_id
        || receipt.candidate != handoff.candidate
        || receipt.tree != handoff.tree
    {
        return None;
    }
    let bytes = serde_json::to_vec(&receipt).ok()?;
    super::id(blake3::hash(&bytes).to_hex().as_str()).ok()
}

fn base_args(database: &std::path::Path, enrollment: &std::path::Path) -> Vec<OsString> {
    vec![
        s("--database"),
        p(database),
        s("--enrollment"),
        p(enrollment),
    ]
}
fn read_bounded_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let metadata = std::fs::metadata(path).map_err(super::text)?;
    if metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
        return Err("native delivery input is empty or exceeds its 16 MiB bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    std::fs::File::open(path)
        .map_err(super::text)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(super::text)?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err("native delivery input is empty or exceeds its 16 MiB bound".into());
    }
    serde_json::from_slice(&bytes).map_err(super::text)
}

fn s(value: &str) -> OsString {
    OsString::from(value)
}
fn p(value: &std::path::Path) -> OsString {
    value.as_os_str().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_receipts_are_task_bound_and_safe() {
        let template = ".cairn/lifecycle/{task}.json";
        assert!(valid_receipt_template(template));
        assert_eq!(
            receipt_path_for_task(template, "change/task-42").unwrap_or_default(),
            ".cairn/lifecycle/task-42.json"
        );
        assert!(receipt_path_for_task(template, "change/..").is_err());
        assert!(receipt_path_for_task(template, "change/bad name").is_err());
    }

    #[test]
    fn lifecycle_receipt_policy_requires_one_safe_task_slot() {
        assert!(!valid_receipt_template(".cairn/lifecycle/receipt.json"));
        assert!(!valid_receipt_template(".cairn/../{task}.json"));
        assert!(!valid_receipt_template(".cairn/{task}/{task}.json"));
        assert!(!valid_receipt_template("/repo/.cairn/{task}.json"));
    }
    #[test]
    fn selected_route_never_upgrades_a_weaker_completion_goal() {
        use kiln_core::implementation::campaign::CompletionGoal as Goal;
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::NativeGit,
            CampaignOperation::Publish,
            Goal::IntegratedChange
        ));
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::NativeGit,
            CampaignOperation::Integrate,
            Goal::VerifiedPullRequest
        ));
        assert!(goal_allows_delivery(
            NativeDeliveryRoute::NativeGit,
            CampaignOperation::Integrate,
            Goal::IntegratedChange
        ));
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Publish,
            Goal::VerifiedCandidate
        ));
        assert!(goal_allows_delivery(
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Publish,
            Goal::VerifiedPullRequest
        ));
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Integrate,
            Goal::VerifiedPullRequest
        ));
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Lifecycle,
            Goal::IntegratedChange
        ));
        assert!(!goal_allows_delivery(
            NativeDeliveryRoute::GithubPullRequest,
            CampaignOperation::Lifecycle,
            Goal::NativeLifecycleComplete
        ));
    }
}
