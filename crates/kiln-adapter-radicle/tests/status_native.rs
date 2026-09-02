#![cfg(feature = "status-native-fixtures")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use kiln_adapter_radicle::status::{
    CI_JOB_TYPE_NAME, PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, RadicleStatusProfile,
    RadicleStatusShell, ResolvedSigner, SecretSpecDeclaration, StatusDispatch, StatusOutcome,
    StatusPlan, StatusUnknownObservation, StatusWriteObservation,
};
use kiln_core::types::{ExternalRef, RunId};
use kiln_provider_api::{PortFailure, PortResult};
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::{TypeName, change::Storage as _};

const FIXTURE_RID: &str = "rad:zFixtureRepository";
const PRIMARY_PROVIDER: &str = "fixture-primary";
const SECONDARY_PROVIDER: &str = "fixture-secondary";
const MISSING_PROVIDER: &str = "fixture-missing";
const DEFAULT_PAYLOAD_BOUND: usize = 65_536;
const MISSING_OBJECT_ID: &str = "1111111111111111111111111111111111111111";
#[cfg(unix)]
const READ_EXECUTE_MODE: u32 = 0o500;
#[cfg(unix)]
const OWNER_FULL_MODE: u32 = 0o700;

fn external(provider: &str, kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new(provider, kind, id).unwrap_or_else(|_| unreachable!())
}

fn executable_identity(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!());
    blake3::hash(&bytes).to_string()
}

fn profile(project_root: &Path, executable: &Path, provider: &str) -> RadicleStatusProfile {
    RadicleStatusProfile::pinned(
        BTreeSet::from([FIXTURE_RID.to_string()]),
        SecretSpecDeclaration {
            project_root: project_root.to_path_buf(),
            project: "kiln-native-fixture".to_string(),
            profile: "fixture".to_string(),
            secret_key: "RADICLE_SIGNER_SEED".to_string(),
            provider: Some(provider.to_string()),
        },
        PinnedSecretSpecExecutable {
            path: executable.to_path_buf(),
            blake3: executable_identity(executable),
            version: PINNED_SECRETSPEC_VERSION.to_string(),
        },
    )
}

fn plan() -> StatusPlan {
    StatusPlan {
        repository: external("radicle", "repository", FIXTURE_RID),
        object_id: None,
        expected_head: None,
        run_id: RunId::new("run-native-fixture").unwrap_or_else(|_| unreachable!()),
        idempotency_key: external("kiln", "idempotency-key", "status-native-fixture"),
        outcome: StatusOutcome::Success,
        max_payload_bytes: DEFAULT_PAYLOAD_BOUND,
    }
}

fn port_error<T>(result: PortResult<T>) -> PortFailure {
    match result {
        Err(error) => error,
        Ok(_) => unreachable!(),
    }
}

fn port_ok<T: std::fmt::Debug>(result: PortResult<T>) -> T {
    assert!(
        result.is_ok(),
        "unexpected port failure: {:?}",
        result.as_ref().err()
    );
    result.unwrap_or_else(|_| unreachable!())
}

fn written(dispatch: StatusDispatch) -> StatusWriteObservation {
    match dispatch {
        StatusDispatch::Written(observation) => observation,
        StatusDispatch::Unknown(_) => unreachable!(),
    }
}

fn unknown(dispatch: StatusDispatch) -> StatusUnknownObservation {
    match dispatch {
        StatusDispatch::Unknown(observation) => observation,
        StatusDispatch::Written(_) => unreachable!(),
    }
}

fn domain_code(error: PortFailure) -> String {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code,
        _ => unreachable!(),
    }
}

fn shell(temporary: &tempfile::TempDir, provider: &str) -> (RadicleStatusShell, PathBuf) {
    let storage_root = temporary.path().join("storage");
    std::fs::create_dir_all(&storage_root).unwrap_or_else(|_| unreachable!());
    let repository_path = storage_root.join(FIXTURE_RID);
    if !repository_path.exists() {
        cob_git2::Repository::init_bare(&repository_path).unwrap_or_else(|_| unreachable!());
    }
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_secretspec_fixture"));
    let shell = RadicleStatusShell::new_fixture(
        storage_root,
        profile(temporary.path(), &executable, provider),
    )
    .unwrap_or_else(|_| unreachable!());
    (shell, executable)
}

fn existing_status_reference(repository: &cob_git2::Repository) -> (String, cob_git2::Oid) {
    let mut references = repository
        .references_glob("refs/namespaces/*/refs/cobs/xyz.radicle.ci-job/*")
        .unwrap_or_else(|_| unreachable!());
    let reference = references
        .next()
        .unwrap_or_else(|| unreachable!())
        .unwrap_or_else(|_| unreachable!());
    (
        reference
            .name()
            .unwrap_or_else(|_| unreachable!())
            .to_string(),
        reference.target().unwrap_or_else(|| unreachable!()),
    )
}

// r[verify kiln.radicle.status_store_fixtures]
// r[verify kiln.radicle.status_conformance]
// r[verify kiln.radicle.status_conformance.scenario.repeat]
// r[verify kiln.radicle.shell_status.scenario.success]
#[test]
fn native_first_write_repeat_and_reconcile_are_equal() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let plan = plan();

    let first = port_ok(shell.publish(&plan, "publish native fixture"));
    let first = written(first);
    assert_eq!(first.repository, plan.repository);
    assert_eq!(first.run_id, plan.run_id.as_str());
    assert_eq!(first.outcome, StatusOutcome::Success);

    let second = shell
        .publish(&plan, "repeat native fixture")
        .unwrap_or_else(|_| unreachable!());
    let second = written(second);
    assert_eq!(second, first);

    let reconciled = shell
        .reconcile(&plan, "reconcile native fixture")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(reconciled, first);
}

// r[verify kiln.radicle.status_store_fixtures]
#[test]
fn exact_object_update_is_repeatable() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let first_plan = plan();
    let first = written(
        shell
            .publish(&first_plan, "create ci-job fixture")
            .unwrap_or_else(|_| unreachable!()),
    );

    let mut update_plan = first_plan;
    update_plan.object_id = Some(first.object_id.clone());
    update_plan.expected_head = Some(first.head.clone());
    update_plan.run_id = RunId::new("run-native-update").unwrap_or_else(|_| unreachable!());
    update_plan.idempotency_key = external("kiln", "idempotency-key", "status-native-update");
    update_plan.outcome = StatusOutcome::Failure;

    let updated = shell
        .publish(&update_plan, "update ci-job fixture")
        .unwrap_or_else(|_| unreachable!());
    let updated = written(updated);
    assert_eq!(updated.object_id, first.object_id);
    assert_eq!(updated.outcome, StatusOutcome::Failure);

    let repeated = shell
        .publish(&update_plan, "repeat ci-job update")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(repeated, StatusDispatch::Written(updated));
}

// r[verify kiln.radicle.status_allowlist.scenario.foreign]
#[test]
fn foreign_signer_head_is_rejected_before_write() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (primary, executable) = shell(&temporary, PRIMARY_PROVIDER);
    let plan = plan();
    let _ = primary
        .publish(&plan, "create primary fixture")
        .unwrap_or_else(|_| unreachable!());

    let storage_root = temporary.path().join("storage");
    let foreign = RadicleStatusShell::new_fixture(
        storage_root,
        profile(temporary.path(), &executable, SECONDARY_PROVIDER),
    )
    .unwrap_or_else(|_| unreachable!());
    let error = port_error(foreign.publish(&plan, "attempt foreign write"));
    assert_eq!(domain_code(error), "status_foreign_head");
}

#[test]
fn stale_expected_head_is_rejected_before_update() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let first_plan = plan();
    let first = written(port_ok(
        shell.publish(&first_plan, "create stale-head fixture"),
    ));

    let mut stale = first_plan;
    stale.object_id = Some(first.object_id);
    stale.expected_head = Some(external("radicle", "head", MISSING_OBJECT_ID));
    stale.run_id = RunId::new("run-stale-head").unwrap_or_else(|_| unreachable!());
    stale.idempotency_key = external("kiln", "idempotency-key", "status-stale-head");
    let error = port_error(shell.publish(&stale, "reject stale head"));
    assert_eq!(domain_code(error), "status_stale_head");
}

#[test]
fn replayed_key_with_different_outcome_is_rejected() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let first = plan();
    let _ = shell
        .publish(&first, "create idempotency fixture")
        .unwrap_or_else(|_| unreachable!());

    let mut conflicting = first;
    conflicting.outcome = StatusOutcome::Failure;
    let error = port_error(shell.publish(&conflicting, "conflict idempotency fixture"));
    assert_eq!(domain_code(error), "status_idempotency_conflict");
}

// r[verify kiln.radicle.status_signer.scenario.missing]
// r[verify kiln.radicle.status_conformance.scenario.redaction]
#[test]
fn unresolved_secret_fails_without_secret_material() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, MISSING_PROVIDER);
    let error = port_error(shell.publish(&plan(), "resolve missing fixture"));
    let rendered = format!("{error:?}");
    assert_eq!(domain_code(error), "radicle_status_signer_unresolved");
    assert!(!rendered.contains("0101010101010101"));
    assert!(!rendered.contains("0202020202020202"));
}

#[test]
fn declared_missing_object_is_rejected_instead_of_recreated() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let mut missing = plan();
    missing.object_id = Some(external("radicle", "cob", MISSING_OBJECT_ID));
    let error = port_error(shell.publish(&missing, "reject missing object"));
    assert_eq!(domain_code(error), "status_object_missing");
}

#[test]
fn object_discovery_enforces_its_named_bound() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (unbounded, executable) = shell(&temporary, PRIMARY_PROVIDER);
    let first = plan();
    let _ = unbounded
        .publish(&first, "create first scan fixture")
        .unwrap_or_else(|_| unreachable!());
    let mut second = first;
    second.run_id = RunId::new("run-second-object").unwrap_or_else(|_| unreachable!());
    second.idempotency_key = external("kiln", "idempotency-key", "status-second-object");
    let _ = unbounded
        .publish(&second, "create second scan fixture")
        .unwrap_or_else(|_| unreachable!());

    let mut limited_profile = profile(temporary.path(), &executable, PRIMARY_PROVIDER);
    limited_profile.max_objects_scanned = 1;
    let limited =
        RadicleStatusShell::new_fixture(temporary.path().join("storage"), limited_profile)
            .unwrap_or_else(|_| unreachable!());
    let mut third = second;
    third.run_id = RunId::new("run-third-object").unwrap_or_else(|_| unreachable!());
    third.idempotency_key = external("kiln", "idempotency-key", "status-third-object");
    let error = port_error(limited.publish(&third, "enforce scan bound"));
    assert_eq!(domain_code(error), "radicle_object_scan_bound");
}

#[test]
fn authenticated_two_namespace_heads_reject_single_writer_not_object_bound() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, executable) = shell(&temporary, PRIMARY_PROVIDER);
    let first = written(port_ok(shell.publish(&plan(), "create branch source")));
    let repository = cob_git2::Repository::open(temporary.path().join("storage").join(FIXTURE_RID))
        .unwrap_or_else(|_| unreachable!());
    let (_, target) = existing_status_reference(&repository);
    let mut config = repository
        .config()
        .unwrap_or_else(|_| unreachable!())
        .open_level(cob_git2::ConfigLevel::Local)
        .unwrap_or_else(|_| unreachable!());
    config
        .set_str("user.name", "COB fixture")
        .unwrap_or_else(|_| unreachable!());
    config
        .set_str("user.email", "fixture@example.invalid")
        .unwrap_or_else(|_| unreachable!());
    let foreign = ResolvedSigner::from_resolved_secret(
        "0202020202020202020202020202020202020202020202020202020202020202",
    )
    .unwrap_or_else(|_| unreachable!());
    let branch = GitCobStore::new(&repository)
        .store(
            None,
            Vec::new(),
            foreign.signing_key(),
            radicle_cob::change::Template {
                tips: vec![target.into()],
                type_name: TypeName::from_str(CI_JOB_TYPE_NAME).unwrap_or_else(|_| unreachable!()),
                message: "independent authenticated head".to_string(),
                embeds: Vec::new(),
                contents: NonEmpty::new(b"other branch".to_vec()),
            },
        )
        .unwrap_or_else(|_| unreachable!());
    repository
        .reference(
            &format!(
                "refs/namespaces/{}/refs/cobs/{CI_JOB_TYPE_NAME}/{}",
                foreign.public_key(),
                first.object_id.native_id.as_str()
            ),
            branch.id.into(),
            false,
            "authenticated second namespace",
        )
        .unwrap_or_else(|_| unreachable!());

    let mut limited_profile = profile(temporary.path(), &executable, PRIMARY_PROVIDER);
    limited_profile.max_objects_scanned = 1;
    let limited =
        RadicleStatusShell::new_fixture(temporary.path().join("storage"), limited_profile)
            .unwrap_or_else(|_| unreachable!());
    let mut replay = plan();
    replay.run_id = RunId::new("run-two-heads").unwrap_or_else(|_| unreachable!());
    replay.idempotency_key = external("kiln", "idempotency-key", "status-two-heads");
    let error = port_error(limited.publish(&replay, "reject multi-head not bound"));
    assert_eq!(domain_code(error), "status_multi_head");
}

#[test]
fn malformed_status_ref_is_rejected_before_replay() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let _ = port_ok(shell.publish(&plan(), "create source ref"));
    let repository = cob_git2::Repository::open(temporary.path().join("storage").join(FIXTURE_RID))
        .unwrap_or_else(|_| unreachable!());
    let (existing_ref, target) = existing_status_reference(&repository);
    let (prefix, _) = existing_ref
        .rsplit_once('/')
        .unwrap_or_else(|| unreachable!());
    repository
        .reference(
            &format!("{prefix}/bad-cob-id"),
            target,
            false,
            "invalid fixture ref",
        )
        .unwrap_or_else(|_| unreachable!());

    let mut replay = plan();
    replay.run_id = RunId::new("run-malformed-ref").unwrap_or_else(|_| unreachable!());
    replay.idempotency_key = external("kiln", "idempotency-key", "status-malformed-ref");
    let error = port_error(shell.publish(&replay, "deny invalid COB ref"));
    assert_eq!(domain_code(error), "radicle_ref_unparsable");
}

#[test]
fn signed_author_namespace_mismatch_is_unreadable() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let _ = port_ok(shell.publish(&plan(), "create signed source ref"));
    let repository = cob_git2::Repository::open(temporary.path().join("storage").join(FIXTURE_RID))
        .unwrap_or_else(|_| unreachable!());
    let (existing_ref, target) = existing_status_reference(&repository);
    let object_id = existing_ref
        .rsplit_once('/')
        .unwrap_or_else(|| unreachable!())
        .1;
    let foreign = ResolvedSigner::from_resolved_secret(
        "0202020202020202020202020202020202020202020202020202020202020202",
    )
    .unwrap_or_else(|_| unreachable!());
    repository
        .reference(
            &format!(
                "refs/namespaces/{}/refs/cobs/xyz.radicle.ci-job/{object_id}",
                foreign.public_key()
            ),
            target,
            false,
            "mismatched signed author fixture",
        )
        .unwrap_or_else(|_| unreachable!());

    let mut replay = plan();
    replay.run_id = RunId::new("run-mismatched-author").unwrap_or_else(|_| unreachable!());
    replay.idempotency_key = external("kiln", "idempotency-key", "status-mismatched-author");
    let error = port_error(shell.publish(&replay, "deny mismatched author"));
    assert_eq!(domain_code(error), "radicle_refs_unreadable");
}

// r[verify kiln.radicle.shell_status.scenario.unknown]
#[cfg(unix)]
#[test]
fn ambiguous_ref_publication_returns_unknown_without_redispatch() {
    use std::os::unix::fs::PermissionsExt as _;

    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let refs = temporary
        .path()
        .join("storage")
        .join(FIXTURE_RID)
        .join("refs");
    std::fs::set_permissions(&refs, std::fs::Permissions::from_mode(READ_EXECUTE_MODE))
        .unwrap_or_else(|_| unreachable!());
    let dispatch = shell
        .publish(&plan(), "classify uncertain fixture")
        .unwrap_or_else(|_| unreachable!());
    std::fs::set_permissions(&refs, std::fs::Permissions::from_mode(OWNER_FULL_MODE))
        .unwrap_or_else(|_| unreachable!());

    let unknown = unknown(dispatch);
    assert_eq!(unknown.object_id, None);
    assert_eq!(unknown.idempotency_key, "status-native-fixture");
    assert!(
        shell
            .reconcile(&plan(), "reconcile uncertain fixture")
            .unwrap_or_else(|_| unreachable!())
            .is_none()
    );
}
