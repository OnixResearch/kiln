use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_core::types::{CoreResult, Diagnostic, ExternalRef, RunId};
use kiln_provider_api::{PortFailure, PortResult};

use super::plan::{
    DEFAULT_MAX_JOB_PAYLOAD_BYTES, DEFAULT_MAX_LOG_URL_BYTES, JobPlan, JobState, admit_job_plan,
    required_actions, run_uuid,
};
use super::profile::{PINNED_JOB_PROFILE_ID, RadicleJobProfile};
use super::reducer::{JobAction, JobReason, JobRun};
use crate::status::ResolvedSigner;
use crate::status::profile::{
    PINNED_SECRETSPEC_EXECUTABLE_BLAKE3, PINNED_SECRETSPEC_EXECUTABLE_PATH,
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, SecretSpecDeclaration,
};

const ALLOWED_RID: &str = "rad:zFixtureRepository";
const FOREIGN_RID: &str = "rad:zForeignRepository";
const SEED: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const UUID: &str = "00000000-0000-0000-0000-000000000000";
const LOG: &str = "https://ci.example.invalid/runs/1";

fn external(provider: &str, kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new(provider, kind, id).unwrap_or_else(|_| unreachable!())
}

fn declaration() -> SecretSpecDeclaration {
    SecretSpecDeclaration {
        project_root: PathBuf::from("/var/lib/kiln/secrets"),
        project: "kiln".to_string(),
        profile: "production".to_string(),
        secret_key: "RADICLE_SIGNER_SEED".to_string(),
        provider: Some("keyring".to_string()),
    }
}

fn executable() -> PinnedSecretSpecExecutable {
    PinnedSecretSpecExecutable {
        path: PathBuf::from(PINNED_SECRETSPEC_EXECUTABLE_PATH),
        blake3: PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.to_string(),
        version: PINNED_SECRETSPEC_VERSION.to_string(),
    }
}

fn profile() -> RadicleJobProfile {
    RadicleJobProfile::pinned(
        BTreeSet::from([ALLOWED_RID.to_string()]),
        external(
            "radicle",
            "node",
            ResolvedSigner::from_resolved_secret(SEED)
                .unwrap_or_else(|_| unreachable!())
                .public_key(),
        ),
        declaration(),
        executable(),
    )
}

fn plan(repository: &str) -> JobPlan {
    JobPlan {
        repository: external("radicle", "repository", repository),
        revision: external("radicle", "revision", &"a".repeat(40)),
        object_id: None,
        run_id: RunId::new("run-fixture").unwrap_or_else(|_| unreachable!()),
        log_url: LOG.to_string(),
        state: JobState::Finished { succeeded: true },
        max_payload_bytes: DEFAULT_MAX_JOB_PAYLOAD_BYTES,
        max_log_url_bytes: DEFAULT_MAX_LOG_URL_BYTES,
    }
}

fn core_error<T>(result: CoreResult<T>) -> Diagnostic {
    match result {
        Err(error) => error,
        Ok(_) => unreachable!(),
    }
}

fn port_error<T>(result: PortResult<T>) -> PortFailure {
    match result {
        Err(error) => error,
        Ok(_) => unreachable!(),
    }
}

fn domain_code(error: PortFailure) -> String {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code,
        _ => unreachable!(),
    }
}

#[test]
fn job_profile_is_pinned_to_job_semantics() {
    let profile = profile();
    let admitted = profile.admit().unwrap_or_else(|_| unreachable!());
    assert!(admitted.contains(ALLOWED_RID));
    assert_eq!(profile.id.as_str(), PINNED_JOB_PROFILE_ID);
    assert_eq!(profile.allowed_object_type, super::profile::JOB_TYPE_NAME);
}

#[test]
fn job_profile_rejects_status_identity_and_type() {
    let mut candidate = profile();
    candidate.inner_mut().id = kiln_core::types::ProviderProfileId::new("radicle-status-v2")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        domain_code(port_error(candidate.admit())),
        "radicle_status_profile"
    );
    let mut candidate = profile();
    candidate.inner_mut().allowed_object_type = "xyz.radicle.ci-job".to_string();
    assert_eq!(
        domain_code(port_error(candidate.admit())),
        "radicle_status_profile"
    );
}

#[test]
fn admitted_plan_accepts_exact_revision_and_bounded_log() {
    let admitted = profile().admit().unwrap_or_else(|_| unreachable!());
    admit_job_plan(&plan(ALLOWED_RID), &admitted).unwrap_or_else(|_| unreachable!());
    let error = core_error(admit_job_plan(&plan(FOREIGN_RID), &admitted));
    assert_eq!(error.code, "job_repository_not_admitted");
}

#[test]
fn plan_rejects_malformed_revision_and_object() {
    let admitted = profile().admit().unwrap_or_else(|_| unreachable!());
    for revision in [
        external("git", "revision", &"a".repeat(40)),
        external("radicle", "branch", &"a".repeat(40)),
        external("radicle", "revision", &"A".repeat(40)),
        external("radicle", "revision", "main"),
    ] {
        let mut candidate = plan(ALLOWED_RID);
        candidate.revision = revision;
        assert_eq!(
            core_error(admit_job_plan(&candidate, &admitted)).code,
            "status_revision_identity"
        );
    }
    let mut candidate = plan(ALLOWED_RID);
    candidate.object_id = Some(external("radicle", "head", &"a".repeat(40)));
    assert_eq!(
        core_error(admit_job_plan(&candidate, &admitted)).code,
        "job_object_identity"
    );
}

// r[verify kiln.radicle.job.bounds]
#[test]
fn plan_rejects_unbounded_or_malformed_log_urls() {
    let admitted = profile().admit().unwrap_or_else(|_| unreachable!());
    for url in [
        "",
        "ftp://ci.example.invalid/log",
        "https:///log",
        "https://user@ci.example.invalid/log",
        "https://ci.example.invalid/log with space",
        "http:///",
    ] {
        let mut candidate = plan(ALLOWED_RID);
        candidate.log_url = url.to_string();
        assert_eq!(
            core_error(admit_job_plan(&candidate, &admitted)).code,
            "job_log_url_invalid",
            "{url}"
        );
    }
    let mut candidate = plan(ALLOWED_RID);
    candidate.log_url = format!("https://ci.example.invalid/{}", "a".repeat(2_048));
    assert_eq!(
        core_error(admit_job_plan(&candidate, &admitted)).code,
        "job_log_url_invalid"
    );
    let mut candidate = plan(ALLOWED_RID);
    candidate.max_payload_bytes = 0;
    assert_eq!(
        core_error(admit_job_plan(&candidate, &admitted)).code,
        "job_payload_bound"
    );
    let mut candidate = plan(ALLOWED_RID);
    candidate.max_log_url_bytes = 0;
    assert_eq!(
        core_error(admit_job_plan(&candidate, &admitted)).code,
        "job_log_url_bound"
    );
}

#[test]
fn run_uuid_is_deterministic_and_scoped() {
    let plan = plan(ALLOWED_RID);
    let first = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
    let second = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
    assert_eq!(first, second);
    assert_eq!(first.len(), 36);
    assert!(uuid::Uuid::parse_str(&first).is_ok());
    let mut other = plan.clone();
    other.revision = external("radicle", "revision", &"b".repeat(40));
    assert_ne!(
        first,
        run_uuid(&other.repository, &other.revision, &other.run_id)
    );
    let mut other = plan.clone();
    other.run_id = RunId::new("run-other").unwrap_or_else(|_| unreachable!());
    assert_ne!(
        first,
        run_uuid(&other.repository, &other.revision, &other.run_id)
    );
}

// r[verify kiln.radicle.job.replay]
#[test]
fn replay_actions_cover_start_finish_and_conflicts() {
    let plan = plan(ALLOWED_RID);
    let uuid = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
    let run = JobRun {
        log: LOG.to_string(),
        finished: None,
        timestamp: 1,
    };

    let finished = JobPlan {
        state: JobState::Finished { succeeded: true },
        ..plan.clone()
    };
    let actions = required_actions(&finished, None).unwrap_or_else(|_| unreachable!());
    assert_eq!(actions.len(), 2);
    assert!(matches!(actions.first(), Some(JobAction::Run { uuid: id, .. }) if id == &uuid));
    assert!(matches!(
        actions.get(1),
        Some(JobAction::Finished { uuid: id, reason: JobReason::Succeeded }) if id == &uuid
    ));

    let started = JobPlan {
        state: JobState::Started,
        ..plan.clone()
    };
    let actions = required_actions(&started, Some(&run)).unwrap_or_else(|_| unreachable!());
    assert!(actions.is_empty());
    let actions = required_actions(&finished, Some(&run)).unwrap_or_else(|_| unreachable!());
    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions.first(),
        Some(JobAction::Finished { uuid: id, reason: JobReason::Succeeded }) if id == &uuid
    ));

    let finished_run = JobRun {
        finished: Some(JobReason::Succeeded),
        ..run.clone()
    };
    assert!(
        required_actions(&finished, Some(&finished_run))
            .unwrap_or_else(|_| unreachable!())
            .is_empty()
    );
    assert_eq!(
        core_error(required_actions(&started, Some(&finished_run))).code,
        "job_replay_start_after_finish"
    );
    let failed = JobPlan {
        state: JobState::Finished { succeeded: false },
        ..plan.clone()
    };
    assert_eq!(
        core_error(required_actions(&failed, Some(&finished_run))).code,
        "job_replay_reason_conflict"
    );
    let mut other_log = plan.clone();
    other_log.log_url = "https://ci.example.invalid/other".to_string();
    assert_eq!(
        core_error(required_actions(&other_log, Some(&run))).code,
        "job_replay_log_conflict"
    );
}

// r[verify kiln.radicle.job]
#[test]
fn actions_serialize_like_the_shared_radicle_job_fixture() {
    let fixture: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/radicle/job-actions.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    let decoded: Vec<JobAction> = serde_json::from_value(serde_json::Value::Array(fixture.clone()))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded.len(), 3);
    for (action, expected) in decoded.iter().zip(&fixture) {
        assert_eq!(
            &serde_json::to_value(action).unwrap_or_else(|_| unreachable!()),
            expected
        );
    }
    assert!(
        serde_json::from_value::<JobAction>(serde_json::json!({
            "Run": {"uuid": UUID, "log": LOG, "extra": true}
        }))
        .is_err()
    );
}
