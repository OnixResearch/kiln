#![cfg(feature = "status-native-fixtures")]

use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use kiln_adapter_radicle::job::{
    DEFAULT_MAX_JOB_PAYLOAD_BYTES, DEFAULT_MAX_LOG_URL_BYTES, JOB_TYPE_NAME, JobAction, JobCob,
    JobDispatch, JobObservation, JobPlan, JobReason, JobState, JobUnknownObservation,
    RadicleJobProfile, RadicleJobShell, run_uuid,
};
use kiln_adapter_radicle::status::{
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, ResolvedSigner, SecretSpecDeclaration,
};
use kiln_core::types::{ExternalRef, RunId};
use kiln_provider_api::{PortFailure, PortResult};
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::change::Storage as _;
use radicle_cob::object::Storage as _;
use radicle_cob::object::collaboration;
use radicle_cob::{Contents, Create, ObjectId, TypeName, Update, Version, change::Template};
use radicle_crypto::PublicKey;
use radicle_oid::Oid;

const FIXTURE_RID: &str = "rad:zFixtureRepository";
const PRIMARY_PROVIDER: &str = "fixture-primary";
const SECONDARY_PROVIDER: &str = "fixture-secondary";
const PRIMARY_SEED: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SECONDARY_SEED: &str = "0202020202020202020202020202020202020202020202020202020202020202";
const TERTIARY_SEED: &str = "0303030303030303030303030303030303030303030303030303030303030303";
const LOG_URL: &str = "https://ci.example.invalid/runs/job-native";
const OTHER_LOG_URL: &str = "https://ci.example.invalid/runs/job-native-other";

fn external(provider: &str, kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new(provider, kind, id).unwrap_or_else(|_| unreachable!())
}

fn executable_identity(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!());
    blake3::hash(&bytes).to_string()
}

fn signer(seed: &str) -> ResolvedSigner {
    ResolvedSigner::from_resolved_secret(seed).unwrap_or_else(|_| unreachable!())
}

fn profile_with(
    project_root: &Path,
    executable: &Path,
    provider: &str,
    signer_seed: &str,
) -> RadicleJobProfile {
    let signer = signer(signer_seed);
    RadicleJobProfile::pinned(
        BTreeSet::from([FIXTURE_RID.to_string()]),
        external("radicle", "node", signer.public_key()),
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

fn plan(state: JobState) -> JobPlan {
    JobPlan {
        repository: external("radicle", "repository", FIXTURE_RID),
        revision: external("radicle", "revision", &"a".repeat(40)),
        object_id: None,
        run_id: RunId::new("run-job-native").unwrap_or_else(|_| unreachable!()),
        log_url: LOG_URL.to_string(),
        state,
        max_payload_bytes: DEFAULT_MAX_JOB_PAYLOAD_BYTES,
        max_log_url_bytes: DEFAULT_MAX_LOG_URL_BYTES,
    }
}

fn shell(temporary: &tempfile::TempDir, provider: &str) -> (RadicleJobShell, PathBuf) {
    let storage_root = temporary.path().join("storage");
    std::fs::create_dir_all(&storage_root).unwrap_or_else(|_| unreachable!());
    let repository_path = storage_root.join(FIXTURE_RID.trim_start_matches("rad:"));
    if !repository_path.exists() {
        cob_git2::Repository::init_bare(&repository_path).unwrap_or_else(|_| unreachable!());
    }
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_secretspec_fixture"));
    let shell = RadicleJobShell::new_fixture(
        storage_root,
        profile_with(temporary.path(), &executable, provider, PRIMARY_SEED),
    )
    .unwrap_or_else(|_| unreachable!());
    (shell, executable)
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

fn written(dispatch: JobDispatch) -> JobObservation {
    match dispatch {
        JobDispatch::Written(observation) => observation,
        JobDispatch::Unknown(_) => unreachable!(),
    }
}

fn domain_code(error: PortFailure) -> String {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code,
        _ => unreachable!(),
    }
}

fn repository(temporary: &tempfile::TempDir) -> cob_git2::Repository {
    let path = temporary
        .path()
        .join("storage")
        .join(FIXTURE_RID.trim_start_matches("rad:"));
    cob_git2::Repository::open(&path).unwrap_or_else(|_| unreachable!())
}

fn type_name() -> TypeName {
    JobCob::type_name_static().clone()
}

fn parse_object_id(observation: &JobObservation) -> ObjectId {
    ObjectId::from_str(observation.object_id.native_id.as_str()).unwrap_or_else(|_| unreachable!())
}

fn reduce(repo: &cob_git2::Repository, object_id: ObjectId) -> JobCob {
    let store = GitCobStore::new(repo);
    collaboration::get::<JobCob, GitCobStore>(&store, &type_name(), &object_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!())
        .object()
        .clone()
}

fn namespace_head(
    repo: &cob_git2::Repository,
    namespace: &PublicKey,
    object_id: ObjectId,
) -> Option<Oid> {
    let store = GitCobStore::new(repo);
    let name = store
        .ref_name(namespace, &type_name(), &object_id)
        .unwrap_or_else(|_| unreachable!());
    match repo.find_reference(&name) {
        Ok(reference) => reference
            .target()
            .map(|target| Oid::from_str(&target.to_string()).unwrap_or_else(|_| unreachable!())),
        Err(error) if error.code() == cob_git2::ErrorCode::NotFound => None,
        Err(_) => unreachable!(),
    }
}

fn revision() -> Oid {
    Oid::from_str(&"a".repeat(40)).unwrap_or_else(|_| unreachable!())
}

fn contents(action: &JobAction) -> Contents {
    NonEmpty::new(serde_json::to_vec(action).unwrap_or_else(|_| unreachable!()))
}

fn direct_run(
    repo: &cob_git2::Repository,
    signer: &ResolvedSigner,
    namespace: &PublicKey,
    object_id: ObjectId,
    tips: Vec<Oid>,
    expected: Option<Oid>,
    uuid: &str,
) -> Oid {
    let store = match expected {
        Some(head) => GitCobStore::with_expected_ref(repo, head),
        None => GitCobStore::new(repo),
    };
    let entry = store
        .store(
            None,
            Vec::new(),
            signer.signing_key(),
            Template {
                type_name: type_name(),
                tips,
                message: "fixture concurrent run".to_string(),
                embeds: Vec::new(),
                contents: contents(&JobAction::Run {
                    uuid: uuid.to_string(),
                    log: LOG_URL.to_string(),
                }),
            },
        )
        .unwrap_or_else(|_| unreachable!());
    store
        .update(namespace, &type_name(), &object_id, &entry.id)
        .unwrap_or_else(|_| unreachable!());
    entry.id
}

fn direct_foreign_job(
    repo: &cob_git2::Repository,
    signer: &ResolvedSigner,
    uuid: &str,
) -> (ObjectId, Oid) {
    let store = GitCobStore::new(repo);
    let namespace = PublicKey::from_str(signer.public_key()).unwrap_or_else(|_| unreachable!());
    let created = collaboration::create::<JobCob, GitCobStore>(
        &store,
        signer.signing_key(),
        None,
        Vec::new(),
        &namespace,
        Create {
            contents: contents(&JobAction::Request { oid: revision() }),
            type_name: type_name(),
            message: "foreign job root".to_string(),
            embeds: Vec::new(),
            version: Version::default(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let object_id: ObjectId = *created.id();
    let root: Oid = *object_id;
    let write_store = GitCobStore::with_expected_ref(repo, root);
    let updated = collaboration::update::<JobCob, GitCobStore>(
        &write_store,
        signer.signing_key(),
        None,
        Vec::new(),
        &namespace,
        Update {
            changes: contents(&JobAction::Run {
                uuid: uuid.to_string(),
                log: OTHER_LOG_URL.to_string(),
            }),
            object_id,
            type_name: type_name(),
            message: "foreign run".to_string(),
            embeds: Vec::new(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(updated.parents.first(), Some(&root));
    (object_id, updated.head)
}

fn history_entries(repo: &cob_git2::Repository, object_id: ObjectId) -> Vec<radicle_cob::Entry> {
    let store = GitCobStore::new(repo);
    let object = collaboration::get::<JobCob, GitCobStore>(&store, &type_name(), &object_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let history = object.history();
    let roots = vec![*history.root().id()];
    history.traverse(
        Vec::new(),
        &roots,
        |mut entries: Vec<radicle_cob::Entry>, _id, entry| {
            entries.push(entry.clone());
            ControlFlow::Continue(entries)
        },
    )
}

fn history_has_entry(
    repo: &cob_git2::Repository,
    object_id: ObjectId,
    predicate: &dyn Fn(&radicle_cob::Entry) -> bool,
) -> bool {
    history_entries(repo, object_id).iter().any(predicate)
}

// r[verify kiln.radicle.job]
// r[verify kiln.radicle.job.scenario.fresh]
#[test]
fn fresh_publish_writes_request_run_and_finished() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let plan = plan(JobState::Finished { succeeded: true });
    let observation = written(port_ok(shell.publish(&plan, "publish job fixture")));

    assert_eq!(observation.run_id, plan.run_id.as_str());
    assert_eq!(observation.state, plan.state);
    assert_eq!(
        observation.run_uuid,
        run_uuid(&plan.repository, &plan.revision, &plan.run_id)
    );

    let repo = repository(&temporary);
    let job = reduce(&repo, parse_object_id(&observation));
    assert_eq!(job.oid(), revision());
    let run = job
        .run(
            observation.signer_identity.native_id.as_str(),
            &observation.run_uuid,
        )
        .unwrap_or_else(|| unreachable!());
    assert_eq!(run.log, LOG_URL);
    assert_eq!(run.finished, Some(JobReason::Succeeded));
    assert_eq!(job.runs().len(), 1);
    assert_eq!(
        job.runs()
            .values()
            .next()
            .map(std::collections::BTreeMap::len),
        Some(1)
    );

    let history = collaboration::get::<JobCob, GitCobStore>(
        &GitCobStore::new(&repo),
        &type_name(),
        &parse_object_id(&observation),
    )
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|| unreachable!());
    assert_eq!(history.history().len(), 3);
}

// r[verify kiln.radicle.job.replay]
// r[verify kiln.radicle.job.scenario.replay]
#[test]
fn replay_does_not_duplicate_the_run() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let plan = plan(JobState::Finished { succeeded: true });
    let first = written(port_ok(shell.publish(&plan, "publish job replay")));
    let second = written(port_ok(shell.publish(&plan, "repeat job replay")));
    assert_eq!(first, second);

    let repo = repository(&temporary);
    let object_id = parse_object_id(&first);
    let job = reduce(&repo, object_id);
    assert_eq!(
        job.runs()
            .values()
            .map(std::collections::BTreeMap::len)
            .sum::<usize>(),
        1
    );
    let history = collaboration::get::<JobCob, GitCobStore>(
        &GitCobStore::new(&repo),
        &type_name(),
        &object_id,
    )
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|| unreachable!());
    assert_eq!(history.history().len(), 3);
}

// r[verify kiln.radicle.job.scenario.replay]
#[test]
fn started_run_finishes_once_and_conflicts_fail() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let started = plan(JobState::Started);
    let first = written(port_ok(shell.publish(&started, "start job fixture")));
    assert_eq!(first.state, JobState::Started);

    let finished = plan(JobState::Finished { succeeded: true });
    let second = written(port_ok(shell.publish(&finished, "finish job fixture")));
    assert_eq!(second.object_id, first.object_id);
    assert_eq!(second.state, JobState::Finished { succeeded: true });
    let third = written(port_ok(shell.publish(&finished, "repeat job finish")));
    assert_eq!(second, third);

    let mut conflict = finished.clone();
    conflict.state = JobState::Finished { succeeded: false };
    assert_eq!(
        domain_code(port_error(shell.publish(&conflict, "conflicting finish"))),
        "job_replay_reason_conflict"
    );
    assert_eq!(
        domain_code(port_error(shell.publish(&started, "restart finished run"))),
        "job_replay_start_after_finish"
    );
    let mut other_log = finished.clone();
    other_log.log_url = OTHER_LOG_URL.to_string();
    assert_eq!(
        domain_code(port_error(shell.publish(&other_log, "conflicting log"))),
        "job_replay_log_conflict"
    );
}

// r[verify kiln.radicle.job.identity]
// r[verify kiln.radicle.job.scenario.extend]
#[test]
fn foreign_job_for_the_commit_is_extended_not_duplicated() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let repo = repository(&temporary);
    let foreign = signer(SECONDARY_SEED);
    let foreign_namespace =
        PublicKey::from_str(foreign.public_key()).unwrap_or_else(|_| unreachable!());
    let (foreign_id, foreign_head) =
        direct_foreign_job(&repo, &foreign, "11111111-1111-1111-1111-111111111111");
    let before = namespace_head(&repo, &foreign_namespace, foreign_id);
    assert_eq!(before, Some(foreign_head));

    let observation = written(port_ok(shell.publish(
        &plan(JobState::Finished { succeeded: true }),
        "extend foreign job",
    )));
    assert_eq!(parse_object_id(&observation), foreign_id);
    assert_eq!(
        namespace_head(&repo, &foreign_namespace, foreign_id),
        before
    );

    let job = reduce(&repo, foreign_id);
    assert_eq!(job.runs().len(), 2);
    assert!(
        job.run(foreign.public_key(), "11111111-1111-1111-1111-111111111111")
            .is_some()
    );
    assert!(
        job.run(
            observation.signer_identity.native_id.as_str(),
            &observation.run_uuid
        )
        .is_some()
    );

    let kiln_namespace = PublicKey::from_str(observation.signer_identity.native_id.as_str())
        .unwrap_or_else(|_| unreachable!());
    assert!(history_has_entry(&repo, foreign_id, &|entry| entry
        .parents
        .contains(&foreign_head)
        && entry.author() == &kiln_namespace));
}

// r[verify kiln.radicle.job.multi_writer]
#[test]
fn divergent_heads_are_joined_by_the_kiln_entry() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let repo = repository(&temporary);
    let signer_b = signer(SECONDARY_SEED);
    let signer_c = signer(TERTIARY_SEED);
    let namespace_b = PublicKey::from_str(signer_b.public_key()).unwrap_or_else(|_| unreachable!());
    let namespace_c = PublicKey::from_str(signer_c.public_key()).unwrap_or_else(|_| unreachable!());

    let store = GitCobStore::new(&repo);
    let created = collaboration::create::<JobCob, GitCobStore>(
        &store,
        signer_b.signing_key(),
        None,
        Vec::new(),
        &namespace_b,
        Create {
            contents: contents(&JobAction::Request { oid: revision() }),
            type_name: type_name(),
            message: "divergent root".to_string(),
            embeds: Vec::new(),
            version: Version::default(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let object_id: ObjectId = *created.id();
    let root: Oid = *object_id;

    let head_c = direct_run(
        &repo,
        &signer_c,
        &namespace_c,
        object_id,
        vec![root],
        None,
        "22222222-2222-2222-2222-222222222222",
    );
    let head_b = direct_run(
        &repo,
        &signer_b,
        &namespace_b,
        object_id,
        vec![root],
        Some(root),
        "33333333-3333-3333-3333-333333333333",
    );
    assert_ne!(head_b, head_c);

    let observation = written(port_ok(shell.publish(
        &plan(JobState::Finished { succeeded: true }),
        "join divergent heads",
    )));
    assert_eq!(parse_object_id(&observation), object_id);
    let job = reduce(&repo, object_id);
    assert_eq!(job.runs().len(), 3);

    assert!(history_has_entry(&repo, object_id, &|entry| {
        entry.parents.contains(&head_b) && entry.parents.contains(&head_c)
    }));
}

// r[verify kiln.radicle.job.scenario.mismatch]
#[test]
fn explicit_object_for_another_commit_is_rejected() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let repo = repository(&temporary);
    let foreign = signer(SECONDARY_SEED);
    let (foreign_id, _) =
        direct_foreign_job(&repo, &foreign, "44444444-4444-4444-4444-444444444444");
    let mut plan = plan(JobState::Finished { succeeded: true });
    plan.object_id = Some(external("radicle", "cob", &foreign_id.to_string()));
    plan.revision = external("radicle", "revision", &"b".repeat(40));
    assert_eq!(
        domain_code(port_error(shell.publish(&plan, "wrong commit"))),
        "job_commit_mismatch"
    );
}

// r[verify kiln.radicle.job.authority]
#[test]
fn foreign_repository_and_signer_mismatch_fail_before_write() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let mut foreign = plan(JobState::Finished { succeeded: true });
    foreign.repository = external("radicle", "repository", "rad:zForeignRepository");
    assert_eq!(
        domain_code(port_error(shell.publish(&foreign, "foreign repository"))),
        "job_repository_not_admitted"
    );

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_secretspec_fixture"));
    let mismatched = RadicleJobShell::new_fixture(
        temporary.path().join("storage"),
        profile_with(
            temporary.path(),
            &executable,
            SECONDARY_PROVIDER,
            PRIMARY_SEED,
        ),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        domain_code(port_error(mismatched.publish(
            &plan(JobState::Finished { succeeded: true }),
            "mismatched signer"
        ))),
        "radicle_job_signer_mismatch"
    );
}

// r[verify kiln.radicle.job.bounds]
#[test]
fn malformed_history_fails_closed() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_shell, _) = shell(&temporary, PRIMARY_PROVIDER);
    let repo = repository(&temporary);
    let signer = signer(SECONDARY_SEED);
    let namespace = PublicKey::from_str(signer.public_key()).unwrap_or_else(|_| unreachable!());
    let store = GitCobStore::new(&repo);
    let entry = store
        .store(
            None,
            Vec::new(),
            signer.signing_key(),
            Template {
                type_name: type_name(),
                tips: Vec::new(),
                message: "malformed root".to_string(),
                embeds: Vec::new(),
                contents: NonEmpty::new(
                    serde_json::to_vec(&serde_json::json!({
                        "Run": {"uuid": "not-a-uuid", "log": LOG_URL}
                    }))
                    .unwrap_or_else(|_| unreachable!()),
                ),
            },
        )
        .unwrap_or_else(|_| unreachable!());
    let object_id = ObjectId::from(entry.id);
    store
        .update(&namespace, &type_name(), &object_id, &entry.id)
        .unwrap_or_else(|_| unreachable!());

    assert!(collaboration::get::<JobCob, GitCobStore>(&store, &type_name(), &object_id).is_err());

    // A malformed branch on a valid root is rejected by the reducer and pruned
    // by the pinned change graph, matching heartwood's branch filtering.
    let created = collaboration::create::<JobCob, GitCobStore>(
        &store,
        signer.signing_key(),
        None,
        Vec::new(),
        &namespace,
        Create {
            contents: contents(&JobAction::Request { oid: revision() }),
            type_name: type_name(),
            message: "valid root with malformed branch".to_string(),
            embeds: Vec::new(),
            version: Version::default(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let valid_id: ObjectId = *created.id();
    let root: Oid = *valid_id;
    let branch = store
        .store(
            None,
            Vec::new(),
            signer.signing_key(),
            Template {
                type_name: type_name(),
                tips: vec![root],
                message: "malformed branch".to_string(),
                embeds: Vec::new(),
                contents: NonEmpty::new(
                    serde_json::to_vec(&serde_json::json!({
                        "Run": {"uuid": "not-a-uuid", "log": LOG_URL}
                    }))
                    .unwrap_or_else(|_| unreachable!()),
                ),
            },
        )
        .unwrap_or_else(|_| unreachable!());
    GitCobStore::with_expected_ref(&repo, root)
        .update(&namespace, &type_name(), &valid_id, &branch.id)
        .unwrap_or_else(|_| unreachable!());
    let object = collaboration::get::<JobCob, GitCobStore>(&store, &type_name(), &valid_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert!(object.object().runs().is_empty());
}

#[test]
fn unknown_observation_round_trips() {
    let observation = JobUnknownObservation {
        repository: external("radicle", "repository", FIXTURE_RID),
        revision: external("radicle", "revision", &"a".repeat(40)),
        object_id: Some(external("radicle", "cob", &"b".repeat(40))),
        run_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
        run_id: "run-job-native".to_string(),
    };
    let dispatch = JobDispatch::Unknown(observation);
    let encoded = serde_json::to_vec(&dispatch).unwrap_or_else(|_| unreachable!());
    let decoded: JobDispatch = serde_json::from_slice(&encoded).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, dispatch);
}

#[test]
fn job_type_name_is_tangled_job() {
    assert_eq!(JOB_TYPE_NAME, "xyz.radworks.job");
    assert_eq!(type_name().to_string(), JOB_TYPE_NAME);
}
