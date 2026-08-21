use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_core::domain::Outcome;
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef, RunId};
use kiln_provider_api::{PortFailure, PortResult};

use super::plan::{
    HeadAuthor, ObservedHeads, StatusOutcome, StatusPlan, admit_expected_head,
    admit_idempotent_replay, admit_single_writer_heads, admit_status_plan, classify_status_outcome,
};
use super::profile::{
    CI_JOB_TYPE_NAME, PINNED_SECRETSPEC_EXECUTABLE_BLAKE3, PINNED_SECRETSPEC_EXECUTABLE_PATH,
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, RadicleStatusProfile,
    SecretSpecDeclaration, admit_profile,
};
use super::resolver::SignerResolver;
use super::secret::ResolvedSigner;
use super::shell::{RadicleStatusShell, StatusDispatch, StatusWriteObservation};

const ALLOWED_RID: &str = "rad:zFixtureRepository";
const FOREIGN_RID: &str = "rad:zForeignRepository";
const FIXTURE_SECRET: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const ZERO_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

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

fn profile() -> RadicleStatusProfile {
    RadicleStatusProfile::pinned(
        BTreeSet::from([ALLOWED_RID.to_string()]),
        declaration(),
        executable(),
    )
}

fn plan(repository: &str) -> StatusPlan {
    StatusPlan {
        repository: external("radicle", "repository", repository),
        object_id: None,
        expected_head: None,
        run_id: RunId::new("run-fixture").unwrap_or_else(|_| unreachable!()),
        idempotency_key: external("kiln", "idempotency-key", "status-fixture"),
        outcome: StatusOutcome::Success,
        max_payload_bytes: super::profile::DEFAULT_MAX_PAYLOAD_BYTES,
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

// r[verify kiln.radicle.status_allowlist.scenario.admitted]
#[test]
fn pinned_profile_and_allowed_plan_are_admitted() {
    let admitted = admit_profile(&profile()).unwrap_or_else(|_| unreachable!());
    assert!(admitted.contains(ALLOWED_RID));
    admit_status_plan(&plan(ALLOWED_RID), &admitted).unwrap_or_else(|_| unreachable!());
    assert_eq!(profile().allowed_object_type, CI_JOB_TYPE_NAME);
}

// r[verify kiln.radicle.status_allowlist.scenario.foreign]
#[test]
fn foreign_repository_is_rejected() {
    let admitted = admit_profile(&profile()).unwrap_or_else(|_| unreachable!());
    let error = core_error(admit_status_plan(&plan(FOREIGN_RID), &admitted));
    assert_eq!(error.code, "status_repository_not_admitted");
}

#[test]
fn zero_profile_bound_is_rejected() {
    let mut candidate = profile();
    candidate.max_objects_scanned = 0;
    let error = port_error(admit_profile(&candidate));
    assert_eq!(domain_code(error), "radicle_status_profile");
}

#[test]
fn unlisted_object_type_is_rejected() {
    let mut candidate = profile();
    candidate.allowed_object_type = "xyz.radicle.issue".to_string();
    let error = port_error(admit_profile(&candidate));
    assert_eq!(domain_code(error), "radicle_status_profile");
}

#[test]
fn missing_secret_declaration_is_rejected() {
    let mut candidate = profile();
    candidate.signer.secret_key.clear();
    let error = port_error(admit_profile(&candidate));
    assert_eq!(domain_code(error), "radicle_status_profile");
}

#[test]
fn authority_expansion_is_rejected() {
    let mut candidate = profile();
    candidate.allow_merge_authority = true;
    let error = port_error(admit_profile(&candidate));
    assert_eq!(domain_code(error), "radicle_status_profile_authority");
}

#[test]
fn terminal_outcomes_are_classified_and_unknown_is_rejected() {
    assert_eq!(
        classify_status_outcome(Outcome::Success).unwrap_or_else(|_| unreachable!()),
        StatusOutcome::Success
    );
    assert_eq!(
        classify_status_outcome(Outcome::TimedOut).unwrap_or_else(|_| unreachable!()),
        StatusOutcome::Failure
    );
    let pending = core_error(classify_status_outcome(Outcome::Pending));
    assert_eq!(pending.code, "status_outcome_unknown");
    let unknown = core_error(classify_status_outcome(Outcome::Unknown));
    assert_eq!(unknown.code, "status_outcome_unknown");
}

#[test]
fn replayed_key_requires_equal_run_and_outcome() {
    let candidate = plan(ALLOWED_RID);
    admit_idempotent_replay(&candidate, candidate.run_id.as_str(), candidate.outcome)
        .unwrap_or_else(|_| unreachable!());
    let error = core_error(admit_idempotent_replay(
        &candidate,
        "different-run",
        candidate.outcome,
    ));
    assert_eq!(error.code, "status_idempotency_conflict");
}

#[test]
fn exact_expected_head_is_admitted_and_stale_head_is_rejected() {
    let observed = external("radicle", "head", "head-current");
    let stale = external("radicle", "head", "head-stale");
    admit_expected_head(Some(&observed), Some(&observed)).unwrap_or_else(|_| unreachable!());
    let error = core_error(admit_expected_head(Some(&observed), Some(&stale)));
    assert_eq!(error.code, "status_stale_head");
    let missing = core_error(admit_expected_head(Some(&observed), None));
    assert_eq!(missing.code, "status_stale_head");
}

#[test]
fn first_write_and_own_single_head_are_admitted() {
    let signer = external("radicle", "node", "zSigner");
    admit_single_writer_heads(&ObservedHeads { heads: Vec::new() }, &signer)
        .unwrap_or_else(|_| unreachable!());
    admit_single_writer_heads(
        &ObservedHeads {
            heads: vec![HeadAuthor {
                head: external("radicle", "head", "head-a"),
                author: Some(signer.clone()),
            }],
        },
        &signer,
    )
    .unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.radicle.status_allowlist.scenario.foreign]
#[test]
fn foreign_and_multiple_heads_are_rejected() {
    let signer = external("radicle", "node", "zSigner");
    let foreign = external("radicle", "node", "zForeign");
    let foreign_error = core_error(admit_single_writer_heads(
        &ObservedHeads {
            heads: vec![HeadAuthor {
                head: external("radicle", "head", "head-a"),
                author: Some(foreign),
            }],
        },
        &signer,
    ));
    assert_eq!(foreign_error.code, "status_foreign_head");

    let multiple_error = core_error(admit_single_writer_heads(
        &ObservedHeads {
            heads: vec![
                HeadAuthor {
                    head: external("radicle", "head", "head-a"),
                    author: Some(signer.clone()),
                },
                HeadAuthor {
                    head: external("radicle", "head", "head-b"),
                    author: Some(signer.clone()),
                },
            ],
        },
        &signer,
    ));
    assert_eq!(multiple_error.code, "status_multi_head");
}

// r[verify kiln.radicle.status_signer.scenario.resolved]
// r[verify kiln.radicle.status_conformance.scenario.redaction]
#[test]
fn resolved_signer_debug_is_redacted() {
    let signer =
        ResolvedSigner::from_resolved_secret(FIXTURE_SECRET).unwrap_or_else(|_| unreachable!());
    let rendered = format!("{signer:?}");
    assert!(rendered.contains("[redacted]"));
    assert!(!rendered.contains(FIXTURE_SECRET));
}

// r[verify kiln.radicle.status_signer.scenario.missing]
#[test]
fn malformed_and_zero_signers_are_rejected() {
    let malformed = core_error(ResolvedSigner::from_resolved_secret("not-a-seed"));
    assert_eq!(malformed.code, "radicle_status_signer_unresolved");
    let zero = core_error(ResolvedSigner::from_resolved_secret(ZERO_SECRET));
    assert_eq!(zero.code, "radicle_status_signer_unresolved");
}

#[test]
fn empty_resolution_reason_fails_before_executable_access() {
    let resolver = SignerResolver::new(
        declaration(),
        PathBuf::from("/path/does/not/exist"),
        PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.to_string(),
    );
    let error = port_error(resolver.resolve(" "));
    assert_eq!(domain_code(error), "radicle_status_signer_reason");
}

#[test]
fn production_profile_rejects_executable_substitution() {
    let mut candidate = profile();
    candidate.executable.path = PathBuf::from("/tmp/substituted-secretspec");
    let error = port_error(admit_profile(&candidate));
    assert_eq!(domain_code(error), "radicle_status_profile");
}

#[test]
fn executable_identity_drift_fails_closed() {
    let temporary = tempfile::NamedTempFile::new().unwrap_or_else(|_| unreachable!());
    let resolver = SignerResolver::new(
        declaration(),
        temporary.path().to_path_buf(),
        PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.to_string(),
    );
    let error = port_error(resolver.resolve("publish fixture status"));
    assert_eq!(domain_code(error), "radicle_status_executable_drift");
}

#[test]
fn status_dispatch_round_trips_and_rejects_unknown_fields() {
    let observation = StatusWriteObservation {
        repository: external("radicle", "repository", ALLOWED_RID),
        object_id: external("radicle", "cob", "object-fixture"),
        head: external("radicle", "head", "head-fixture"),
        run_id: "run-fixture".to_string(),
        idempotency_key: "status-fixture".to_string(),
        outcome: StatusOutcome::Success,
        signer_identity: external("radicle", "node", "signer-fixture"),
    };
    let dispatch = StatusDispatch::Written(observation);
    let encoded = serde_json::to_vec(&dispatch).unwrap_or_else(|_| unreachable!());
    let decoded: StatusDispatch =
        serde_json::from_slice(&encoded).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, dispatch);

    let malformed = br#"{"disposition":"written","observation":{"unknown":true}}"#;
    assert!(serde_json::from_slice::<StatusDispatch>(malformed).is_err());
}

#[test]
fn oversized_payload_fails_before_signer_or_storage_access() {
    let shell = RadicleStatusShell::new(PathBuf::from("/path/does/not/exist"), profile())
        .unwrap_or_else(|_| unreachable!());
    let mut candidate = plan(ALLOWED_RID);
    candidate.max_payload_bytes = 1;
    let error = port_error(shell.publish(&candidate, "publish oversized fixture"));
    assert_eq!(domain_code(error), "status_payload_bound");
}
