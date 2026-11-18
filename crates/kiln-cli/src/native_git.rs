//! Explicit operator-driven native-Git qualification runner.
//! The request file contains only non-secret identifiers/configuration; publisher
//! credentials are acquired from the operator's existing Git credential helper
//! or SSH agent and never serialized or passed to implementation workers.

use std::io::Read;
use std::path::Path;

use git2::{Cred, CredentialType, Error, Repository};
use kiln_adapter_git::native_publication::{
    NativeRemoteProfile, NativeTransport, PushObservation, push_exact_ref, read_remote_ref,
    remote_identity,
};
use kiln_core::implementation::{Enrollment, NativeGitObservation, NativeGitPlan};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use kiln_implementer::native_publication::{
    NATIVE_GIT_LIFECYCLE_REQUEST_SCHEMA, NATIVE_GIT_QUALIFICATION_REQUEST_SCHEMA,
    NativeGitDispatch, NativeGitEffectKind, NativeGitEffectReservation, NativeGitEffectSelection,
    NativeGitLifecycleRequest, NativeGitQualificationRequest, NativeGitReviewAuthority,
};
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const RESULT_SCHEMA: &str = "kiln.native-git-qualification-result.v1";
/// Independently observe native lifecycle evidence and retain its receipt.
pub fn record_lifecycle(
    request_path: &Path,
    enrollment_path: &Path,
    service_profile_path: &Path,
    review_profile_path: &Path,
    repository_path: &Path,
    report_path: &Path,
) -> Result<String, String> {
    let bytes = read_bounded(
        request_path,
        MAX_REQUEST_BYTES,
        "native Git lifecycle request",
    )?;
    let request: NativeGitLifecycleRequest<NativeRemoteProfile> =
        serde_json::from_slice(&bytes).map_err(text)?;
    if request.schema != NATIVE_GIT_LIFECYCLE_REQUEST_SCHEMA {
        return Err("unsupported native Git lifecycle request schema".into());
    }
    if request.remote.transport == NativeTransport::LocalBareFixture {
        return Err("local bare Git cannot qualify lifecycle completion".into());
    }
    let enrollment = load_enrollment(enrollment_path)?;
    if !enrollment.actors.contains(&request.actor) {
        return Err("lifecycle actor is not in trusted enrollment".into());
    }
    let mut engine = Implementer::open(&request.database, enrollment.clone())?;
    let ledger = engine.work()?;
    let work = ledger
        .current()
        .find(|work| work.identity().ok().as_ref() == Some(&request.work))
        .cloned()
        .ok_or("requested lifecycle work is not current")?;
    if work.actor != request.actor || !enrollment.repositories.contains(&work.repository) {
        return Err("lifecycle actor or repository differs from trusted enrollment".into());
    }
    let native = engine
        .native_git_receipt(&request.attempt)?
        .ok_or("accepted native Git receipt is absent")?;
    if native.work_identity != request.work
        || native.attempt != request.attempt
        || native.work_revision != work.revision
        || native.fence != request.fence
        || remote_identity(&request.remote).map_err(text)? != native.remote_identity
    {
        return Err("lifecycle request does not match retained native receipt authority".into());
    }
    let attempt = engine
        .attempt(&request.attempt)?
        .ok_or("native Git attempt is absent")?;
    let verification = attempt
        .verification
        .as_ref()
        .ok_or("native Git verification is absent")?;
    let mut completion_checks = verification
        .run
        .checks
        .iter()
        .filter(|check| check.name == "cairn-authoritative-completion");
    let completion = completion_checks
        .next()
        .ok_or("authoritative Cairn completion check is absent")?;
    if completion_checks.next().is_some() {
        return Err("multiple authoritative Cairn completion checks are ambiguous".into());
    }
    if completion.outcome != "pass" || completion.kind != "cairn-agent-completion-v2" {
        return Err("retained authoritative Cairn completion check is not passing".into());
    }
    let completion_id = ContentId::from_blake3_hex(
        completion
            .artifact_digest
            .as_deref()
            .ok_or("completion artifact digest is absent")?
            .to_owned(),
    )
    .map_err(diag)?;
    let source = work
        .aliases
        .iter()
        .find(|source| source.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
        .cloned()
        .ok_or("native lifecycle work has no Cairn task alias")?;
    let binding = kiln_implementer::publication_effects::CairnLifecycleBinding {
        source,
        handoff: native.handoff.clone(),
        verification: native.verification.clone(),
        authoritative_completion: completion_id,
        candidate: native.candidate.clone(),
        action: request.action,
    };
    let repo = Repository::open(repository_path).map_err(text)?;
    let config = repo.config().map_err(text)?;
    let credential = |url: &str, username: Option<&str>, allowed: CredentialType| {
        if url != request.remote.remote_url {
            return Err(Error::from_str(
                "credential URL differs from enrolled remote",
            ));
        }
        match request.remote.transport {
            NativeTransport::Https if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) => {
                Cred::credential_helper(&config, url, username)
            }
            NativeTransport::SshStrictHostKey if allowed.contains(CredentialType::SSH_KEY) => {
                Cred::ssh_key_from_agent(username.unwrap_or("git"))
            }
            _ => Err(Error::from_str(
                "transport does not permit this lifecycle credential",
            )),
        }
    };
    let branch = read_remote_ref(
        &repo,
        &request.remote,
        &native.remote_identity,
        &native.branch_ref,
        &credential,
    )
    .map_err(text)?;
    if branch != Some(git2::Oid::from_str(native.candidate.native_id.as_str()).map_err(text)?) {
        return Err("candidate branch no longer names the exact native Git receipt".into());
    }
    let target = read_remote_ref(
        &repo,
        &request.remote,
        &native.remote_identity,
        &native.target_ref,
        &credential,
    )
    .map_err(text)?;
    if target != Some(git2::Oid::from_str(native.candidate.native_id.as_str()).map_err(text)?) {
        return Err("native target no longer names the accepted candidate".into());
    }
    let observation = kiln_adapter_cairn::lifecycle_observation::observe_native_git_lifecycle(
        repository_path,
        &native,
        &binding,
        &request.receipt_path,
        report_path,
    )?;
    let branch = read_remote_ref(
        &repo,
        &request.remote,
        &native.remote_identity,
        &native.branch_ref,
        &credential,
    )
    .map_err(text)?;
    if branch != Some(git2::Oid::from_str(native.candidate.native_id.as_str()).map_err(text)?) {
        return Err("candidate branch changed during lifecycle observation".into());
    }
    let target = read_remote_ref(
        &repo,
        &request.remote,
        &native.remote_identity,
        &native.target_ref,
        &credential,
    )
    .map_err(text)?;
    if target != Some(git2::Oid::from_str(native.candidate.native_id.as_str()).map_err(text)?) {
        return Err("native target changed during lifecycle observation".into());
    }
    let review = kiln_web_service::code_review::observe_native_git_authority_from_profiles(
        service_profile_path,
        review_profile_path,
        &request.change_id,
        &request.actor,
    )?;
    let receipt = engine.record_native_cairn_lifecycle(
        &work,
        &request.attempt,
        request.fence,
        &observation,
        &review,
    )?;
    serde_json::to_string_pretty(&receipt).map_err(text)
}

/// Execute exactly one operator-authorized native Git qualification request.
/// The caller must pass `--execute`; this command can perform real ref updates.
// r[impl kiln.implementation.native_git]
pub fn run(
    request_path: &Path,
    enrollment_path: &Path,
    service_profile_path: &Path,
    review_profile_path: &Path,
    execute: bool,
) -> Result<String, String> {
    if !execute {
        return Err("native Git qualification requires the explicit --execute flag".into());
    }
    let bytes = read_bounded(
        request_path,
        MAX_REQUEST_BYTES,
        "native Git qualification request",
    )?;
    let mut request: NativeGitQualificationRequest<NativeRemoteProfile> =
        serde_json::from_slice(&bytes).map_err(text)?;
    if request.schema != NATIVE_GIT_QUALIFICATION_REQUEST_SCHEMA {
        return Err("unsupported native Git qualification request schema".into());
    }
    if request.remote.transport == NativeTransport::LocalBareFixture {
        return Err(
            "local bare Git is a test fixture only and cannot qualify native remote completion"
                .into(),
        );
    }
    if remote_identity(&request.remote).map_err(text)? != request.plan.remote_identity {
        return Err(
            "remote locator/transport does not match the plan's enrolled remote identity".into(),
        );
    }
    let enrollment = load_enrollment(enrollment_path)?;
    if !enrollment.actors.contains(&request.actor)
        || !enrollment.repositories.contains(&request.plan.repository)
    {
        return Err("qualification actor/repository is not in trusted enrollment".into());
    }
    let mut engine = Implementer::open(&request.database, enrollment)?;
    let ledger = engine.work()?;
    let work = ledger
        .current()
        .chain(ledger.superseded().iter())
        .find(|work| work.identity().ok().as_ref() == Some(&request.work))
        .cloned()
        .ok_or("requested work is not retained in Implementer state")?;
    if work.actor != request.actor || work.repository != request.plan.repository {
        return Err("qualification actor/repository does not match retained work".into());
    }
    let attempt = engine
        .attempt(&request.attempt)?
        .ok_or("attempt is not retained")?;
    if attempt.work_identity != request.work || attempt.work_revision != work.revision {
        return Err("qualification attempt does not own requested work".into());
    }
    let repository = Repository::open(&request.repository_path).map_err(text)?;
    let config = repository.config().map_err(text)?;
    let credential = |url: &str, username: Option<&str>, allowed: CredentialType| {
        if url != request.remote.remote_url {
            return Err(Error::from_str(
                "credential request URL differs from enrolled remote",
            ));
        }
        match request.remote.transport {
            NativeTransport::Https if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) => {
                Cred::credential_helper(&config, url, username)
            }
            NativeTransport::SshStrictHostKey if allowed.contains(CredentialType::SSH_KEY) => {
                Cred::ssh_key_from_agent(username.unwrap_or("git"))
            }
            _ => Err(Error::from_str(
                "transport does not permit the requested credential mechanism",
            )),
        }
    };

    let review = current_review(
        service_profile_path,
        review_profile_path,
        &request.change_id,
        &request.actor,
    )?;
    let plan_id = engine.prepare_native_git(
        &work,
        &request.attempt,
        request.fence,
        &mut request.plan,
        &review,
    )?;

    let branch = engine.consume_native_git_effect_with_current_authority(
        NativeGitEffectSelection {
            work: &work,
            attempt: &request.attempt,
            fence: request.fence,
            plan_identity: &plan_id,
            kind: NativeGitEffectKind::BranchPublication,
        },
        || {
            current_review(
                service_profile_path,
                review_profile_path,
                &request.change_id,
                &request.actor,
            )
        },
        |mode, plan| {
            if mode == NativeGitDispatch::FirstDispatch {
                let remote_target = read_remote_ref(
                    &repository,
                    &request.remote,
                    &plan.remote_identity,
                    &plan.target_ref,
                    &credential,
                )
                .map_err(text)?;
                if remote_target
                    != Some(
                        git2::Oid::from_str(plan.expected_old_target.native_id.as_str())
                            .map_err(text)?,
                    )
                {
                    return Ok(Some(NativeGitObservation::Rejected {
                        operation: plan.branch_operation.clone(),
                        code: "target_advanced".into(),
                        reason: "remote target advanced before candidate branch publication".into(),
                    }));
                }
                let result = push_exact_ref(
                    &repository,
                    &request.remote,
                    &plan.remote_identity,
                    git2::Oid::from_str(plan.candidate.native_id.as_str()).map_err(text)?,
                    &plan.branch_ref,
                    git2::Oid::ZERO_SHA1,
                    &credential,
                )
                .map_err(text)?;
                Ok(Some(branch_observation(plan, result)))
            } else {
                Ok(Some(read_branch_observation(
                    &repository,
                    &request.remote,
                    plan,
                    &credential,
                )?))
            }
        },
    )?;

    if !matches!(branch.observed, Some(NativeGitObservation::Branch { .. })) {
        return serde_json::to_string_pretty(&QualificationResult {
            schema: RESULT_SCHEMA,
            state: "branch_pending_or_blocked",
            branch,
            integration: None,
            receipt: None,
        })
        .map_err(text);
    }
    let integration = engine.consume_native_git_effect_with_current_authority(
        NativeGitEffectSelection {
            work: &work,
            attempt: &request.attempt,
            fence: request.fence,
            plan_identity: &plan_id,
            kind: NativeGitEffectKind::TargetIntegration,
        },
        || {
            current_review(
                service_profile_path,
                review_profile_path,
                &request.change_id,
                &request.actor,
            )
        },
        |mode, plan| {
            if let Some(blocked) =
                branch_guard_observation(&repository, &request.remote, plan, &credential)?
            {
                return Ok(Some(blocked));
            }
            if mode == NativeGitDispatch::FirstDispatch {
                let result = push_exact_ref(
                    &repository,
                    &request.remote,
                    &plan.remote_identity,
                    git2::Oid::from_str(plan.candidate.native_id.as_str()).map_err(text)?,
                    &plan.target_ref,
                    git2::Oid::from_str(plan.expected_old_target.native_id.as_str())
                        .map_err(text)?,
                    &credential,
                )
                .map_err(text)?;
                if let Some(blocked) =
                    branch_guard_observation(&repository, &request.remote, plan, &credential)?
                {
                    return Ok(Some(blocked));
                }
                Ok(Some(target_observation(plan, result)))
            } else {
                Ok(Some(read_target_observation(
                    &repository,
                    &request.remote,
                    plan,
                    &credential,
                )?))
            }
        },
    )?;
    let branch_still_exact = matches!(
        read_branch_observation(&repository, &request.remote, &integration.plan, &credential)?,
        NativeGitObservation::Branch { ref remote, ref branch_ref, ref commit }
            if remote == &integration.plan.remote_identity
                && branch_ref == &integration.plan.branch_ref
                && commit == &integration.plan.candidate,
    );
    let target_still_exact = matches!(
        read_target_observation(&repository, &request.remote, &integration.plan, &credential)?,
        NativeGitObservation::Target { ref remote, ref target_ref, ref resulting, .. }
            if remote == &integration.plan.remote_identity
                && target_ref == &integration.plan.target_ref
                && resulting == &integration.plan.candidate,
    );
    let mut integration = integration;
    let receipt = if branch_still_exact && target_still_exact {
        integration.receipt.clone()
    } else {
        None
    };
    if receipt.is_none() {
        integration.receipt = None;
    }
    let state = if receipt.is_some() {
        "confirmed"
    } else {
        "integration_pending_or_blocked"
    };
    serde_json::to_string_pretty(&QualificationResult {
        schema: RESULT_SCHEMA,
        state,
        branch,
        integration: Some(integration),
        receipt,
    })
    .map_err(text)
}

#[derive(serde::Serialize)]
struct QualificationResult<'a> {
    schema: &'static str,
    state: &'a str,
    branch: NativeGitEffectReservation,
    integration: Option<NativeGitEffectReservation>,
    receipt: Option<kiln_core::implementation::NativeGitReceipt>,
}

fn current_review(
    service_profile_path: &Path,
    review_profile_path: &Path,
    change_id: &str,
    actor: &str,
) -> Result<NativeGitReviewAuthority, String> {
    kiln_web_service::code_review::observe_native_git_authority_from_profiles(
        service_profile_path,
        review_profile_path,
        change_id,
        actor,
    )
}

fn branch_observation(plan: &NativeGitPlan, result: PushObservation) -> NativeGitObservation {
    match result {
        PushObservation::Confirmed { .. } => NativeGitObservation::Branch {
            remote: plan.remote_identity.clone(),
            branch_ref: plan.branch_ref.clone(),
            commit: plan.candidate.clone(),
        },
        PushObservation::Rejected { reason } => NativeGitObservation::Rejected {
            operation: plan.branch_operation.clone(),
            code: "remote_rejected".into(),
            reason,
        },
        PushObservation::Unknown { reason } => NativeGitObservation::Unknown {
            operation: plan.branch_operation.clone(),
            reason,
        },
    }
}

fn target_observation(plan: &NativeGitPlan, result: PushObservation) -> NativeGitObservation {
    match result {
        PushObservation::Confirmed { .. } => NativeGitObservation::Target {
            remote: plan.remote_identity.clone(),
            target_ref: plan.target_ref.clone(),
            previous: plan.expected_old_target.clone(),
            resulting: plan.candidate.clone(),
        },
        PushObservation::Rejected { reason } => NativeGitObservation::Rejected {
            operation: plan.integration_operation.clone(),
            code: "remote_protection_or_race".into(),
            reason,
        },
        PushObservation::Unknown { reason } => NativeGitObservation::Unknown {
            operation: plan.integration_operation.clone(),
            reason,
        },
    }
}

fn read_branch_observation(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    plan: &NativeGitPlan,
    credential: &impl Fn(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<NativeGitObservation, String> {
    let observed = read_remote_ref(
        repository,
        profile,
        &plan.remote_identity,
        &plan.branch_ref,
        credential,
    )
    .map_err(text)?;
    match observed {
        Some(oid) if oid.to_string() == plan.candidate.native_id.as_str() => {
            Ok(NativeGitObservation::Branch {
                remote: plan.remote_identity.clone(),
                branch_ref: plan.branch_ref.clone(),
                commit: plan.candidate.clone(),
            })
        }
        Some(oid) => Ok(NativeGitObservation::Rejected {
            operation: plan.branch_operation.clone(),
            code: "branch_changed".into(),
            reason: format!("candidate branch now points to {oid}"),
        }),
        None => Ok(NativeGitObservation::Unknown {
            operation: plan.branch_operation.clone(),
            reason: "candidate branch is absent on remote".into(),
        }),
    }
}

fn branch_guard_observation(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    plan: &NativeGitPlan,
    credential: &impl Fn(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<Option<NativeGitObservation>, String> {
    match read_branch_observation(repository, profile, plan, credential)? {
        NativeGitObservation::Branch {
            remote,
            branch_ref,
            commit,
        } if remote == plan.remote_identity
            && branch_ref == plan.branch_ref
            && commit == plan.candidate =>
        {
            Ok(None)
        }
        NativeGitObservation::Branch { commit, .. } => Ok(Some(NativeGitObservation::Rejected {
            operation: plan.integration_operation.clone(),
            code: "candidate_branch_changed".into(),
            reason: format!(
                "candidate branch no longer names the exact candidate: {}",
                commit.native_id.as_str()
            ),
        })),
        NativeGitObservation::Rejected { reason, .. } => Ok(Some(NativeGitObservation::Rejected {
            operation: plan.integration_operation.clone(),
            code: "candidate_branch_changed".into(),
            reason,
        })),
        NativeGitObservation::Unknown { reason, .. } => Ok(Some(NativeGitObservation::Unknown {
            operation: plan.integration_operation.clone(),
            reason,
        })),
        NativeGitObservation::Target { .. } => {
            Err("branch read returned a target observation".into())
        }
    }
}

fn read_target_observation(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    plan: &NativeGitPlan,
    credential: &impl Fn(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<NativeGitObservation, String> {
    let observed = read_remote_ref(
        repository,
        profile,
        &plan.remote_identity,
        &plan.target_ref,
        credential,
    )
    .map_err(text)?;
    match observed {
        Some(oid) if oid.to_string() == plan.candidate.native_id.as_str() => {
            Ok(NativeGitObservation::Target {
                remote: plan.remote_identity.clone(),
                target_ref: plan.target_ref.clone(),
                previous: plan.expected_old_target.clone(),
                resulting: plan.candidate.clone(),
            })
        }
        Some(oid) => Ok(NativeGitObservation::Unknown {
            operation: plan.integration_operation.clone(),
            reason: format!("target now points to {oid}; exact operation result is unresolved"),
        }),
        None => Ok(NativeGitObservation::Unknown {
            operation: plan.integration_operation.clone(),
            reason: "target ref is absent on remote".into(),
        }),
    }
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn diag(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

fn read_bounded(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(text)?
        .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() > maximum {
        return Err(format!("{label} exceeds its size bound"));
    }
    Ok(bytes)
}

fn load_enrollment(path: &Path) -> Result<Enrollment, String> {
    let bytes = read_bounded(path, 64 * 1024, "trusted enrollment")?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("trusted enrollment is invalid: {error}"))
}

const NATIVE_GIT_TIMEOUT_MS: u64 = 120_000;
const NATIVE_GIT_STDOUT_BYTES: usize = 1024 * 1024;
const NATIVE_GIT_STDERR_BYTES: usize = 64 * 1024;
const NATIVE_GIT_POLL_MS: u64 = 25;
const NATIVE_GIT_TEARDOWN_MS: u64 = 2_000;

/// Supervise the complete owner operation, including credential helpers and
/// libgit2 read-back. A killed child leaves committed intents for reconciliation;
/// killing a thread would let an unacknowledged remote effect continue locally.
pub fn supervise(arguments: impl Iterator<Item = String>) -> Result<String, String> {
    use bounded_exec::{
        CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy,
        RunRequest, TerminationScope,
    };
    use std::ffi::OsString;
    use std::sync::{Arc, atomic::AtomicBool};
    let cancellation = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&cancellation)).map_err(text)?;
    }
    let output = bounded_exec::run_with_cancellation(
        RunRequest {
            command: CommandSpec {
                program: std::env::current_exe().map_err(text)?,
                args: std::iter::once(OsString::from(super::COMMAND_NATIVE_GIT_WORKER))
                    .chain(arguments.map(OsString::from))
                    .collect(),
                current_dir: std::env::current_dir().map_err(text)?,
                environment_mode: EnvironmentMode::Inherit,
                environment: Vec::new(),
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms: NATIVE_GIT_TIMEOUT_MS,
                stdin_max_bytes: 0,
                stdout_max_bytes: NATIVE_GIT_STDOUT_BYTES,
                stderr_max_bytes: NATIVE_GIT_STDERR_BYTES,
                poll_interval_ms: NATIVE_GIT_POLL_MS,
                teardown_timeout_ms: NATIVE_GIT_TEARDOWN_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|error| {
                format!("native Git supervisor policy admission failed: {error:?}")
            })?,
        },
        &cancellation,
    )
    .map_err(|error| {
        format!("native Git supervisor failed; possible effects remain unknown: {error}")
    })?;
    match output.disposition {
        Disposition::Succeeded => String::from_utf8(output.stdout.bytes).map(|mut value| {
            if value.ends_with('\n') { value.pop(); }
            value
        }).map_err(text),
        Disposition::ExitFailed => {
            let error = String::from_utf8(output.stderr.bytes).map_err(text)?;
            if error.is_empty() {
                Err("native Git child failed without an acknowledgement; reconcile the exact request".into())
            } else {
                Err(error)
            }
        }
        Disposition::TimedOut => Err("native Git deadline expired; possible effects remain unknown; reconcile the exact request".into()),
        Disposition::Cancelled => Err("native Git cancelled; possible effects remain unknown; reconcile the exact request".into()),
        Disposition::OutputLimitExceeded(_) => Err("native Git output exceeded its bound; possible effects remain unknown; reconcile the exact request".into()),
    }
}
