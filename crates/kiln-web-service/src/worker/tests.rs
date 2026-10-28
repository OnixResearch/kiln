use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::atomic::AtomicBool;

use git2::{Repository, Signature};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::source::GitSnapshotLimits;
use kiln_core::types::{ExternalRef, ProviderProfileId};

use super::{Step, WorkerProfile, client, files, process, source};

fn fixture() -> (tempfile::TempDir, WorkerProfile, source::SourceContext) {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture root", error));
    let repository_path = root.path().join("repository");
    fs::create_dir(&repository_path)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "repository directory", error));
    let repository = Repository::init(&repository_path)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture repository", error));
    fs::write(repository_path.join("input.txt"), b"committed source\n")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "source file", error));
    fs::write(repository_path.join("reviewed.sh"), b"IFS= read -r input < input.txt\ntest \"$input\" = 'committed source' || exit 29\ntest -z \"$LATTICE_SOURCE_REVISION$KILN_WORKER_TOKEN\" || exit 30\nprintf '%s\\n' \"$input\" > product.txt\nprintf 'actual stdout\\n'\nprintf 'actual stderr\\n' >&2\n").unwrap_or_else(|error| unreachable!("{}: {:?}", "source command", error));
    let mut index = repository
        .index()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "index", error));
    index
        .add_path(std::path::Path::new("input.txt"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "index input", error));
    index
        .add_path(std::path::Path::new("reviewed.sh"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "index script", error));
    let tree_id = index
        .write_tree()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "tree", error));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "tree object", error));
    let signature = Signature::now("Worker fixture", "worker@example.invalid")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "signature", error));
    let oid = repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "reviewed source",
            &tree,
            &[],
        )
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "commit", error));
    let work_root = root.path().join("work");
    fs::create_dir(&work_root)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "work directory", error));
    fs::set_permissions(&work_root, fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "private work directory", error));
    let mut profile = WorkerProfile {
        schema: "kiln.web-worker.v2".into(),
        project_id: "fixture".into(),
        source_repository: String::new(),
        repository_path,
        service_origin: "https://localhost:9443".into(),
        token_file: root.path().join("worker.token"),
        tls_ca_path: root.path().join("ca.pem"),
        work_root,
        snapshot_limits: GitSnapshotLimits {
            max_entries: 128,
            max_path_bytes: 1024,
            max_file_bytes: 8192,
            max_total_bytes: 65536,
        },
        command_timeout_ms: 2000,
        maximum_output_bytes: 4096,
        teardown_timeout_ms: 1000,
        poll_interval_ms: 10,
        http_timeout_ms: 1000,
        lookup_timeout_ms: 1000,
        maximum_response_bytes: 65536,
        maximum_artifact_bytes: 8192,
        maximum_total_artifact_bytes: 16384,
        maximum_artifacts: 4,
        steps: vec![Step {
            node_id: "build".into(),
            program: "/bin/sh".into(),
            argv: vec!["reviewed.sh".into()],
            environment: BTreeMap::new(),
            artifacts: vec!["product.txt".into()],
            reports: vec![],
        }],
    };
    let id = ProviderProfileId::new("fixture-git")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "provider id", error));
    let adapter = LocalGitAdapter::new(
        profile.repository_path.clone(),
        oid.to_string(),
        GitSourceProfile::pinned_local(id.clone(), profile.snapshot_limits.clone()),
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "adapter", error));
    let resolution = resolve_and_acquire(&adapter, id)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "source resolution", error));
    profile.source_repository = resolution.repository.native_id.as_str().into();
    let context = source::SourceContext {
        provider: "git".into(),
        repository: resolution.repository.native_id.as_str().into(),
        revision: oid.to_string(),
    };
    (root, profile, context)
}

fn independent_clone_fixture() -> (tempfile::TempDir, WorkerProfile, source::SourceContext) {
    let (root, mut profile, context) = fixture();
    let clone = root.path().join("worker-clone");
    Repository::clone(
        profile
            .repository_path
            .to_str()
            .unwrap_or_else(|| unreachable!()),
        &clone,
    )
    .unwrap_or_else(|error| unreachable!("independent clone: {error:?}"));
    profile.repository_path = clone;
    (root, profile, context)
}

fn binding_fixture(
    profile: &WorkerProfile,
    source: &kiln_core::source::SourceResolution,
) -> client::RunBinding {
    let repository = ExternalRef::new("git", "repository", &profile.source_repository)
        .unwrap_or_else(|error| unreachable!("repository: {error:?}"));
    let execution = ExternalRef::new("lattice", "execution", "native-1")
        .unwrap_or_else(|error| unreachable!("execution: {error:?}"));
    client::RunBinding {
        id: "run-fixture".into(),
        project_id: profile.project_id.clone(),
        repository: serde_json::to_value(repository)
            .unwrap_or_else(|error| unreachable!("repository DTO: {error:?}")),
        source_revision: source.exact_revision.native_id.as_str().into(),
        canonical_tree: source.canonical_tree.as_str().into(),
        attempt_id: "attempt-1".into(),
        execution: serde_json::to_value(execution)
            .unwrap_or_else(|error| unreachable!("execution DTO: {error:?}")),
        outcome: "Pending".into(),
        nodes: vec![client::Node { id: "build".into() }],
        log_bounds: client::LogBounds {
            maximum_chunk_bytes: 4,
            maximum_total_bytes: 4096,
            maximum_chunks: 128,
        },
    }
}

// r[verify kiln.web.remote_workers.source]
// r[verify kiln.web.remote_workers.source.scenario.clone]
#[test]
fn independent_clone_executes_declared_source_without_relabeling_local_acquisition() {
    let (_root, profile, context) = independent_clone_fixture();
    fs::write(
        profile.repository_path.join("input.txt"),
        b"worker-local mutation",
    )
    .unwrap_or_else(|error| unreachable!("dirty clone: {error:?}"));
    fs::write(profile.repository_path.join("reviewed.sh"), b"exit 99")
        .unwrap_or_else(|error| unreachable!("dirty clone command: {error:?}"));
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("clone checkout: {error:?}"));
    let canonical = profile
        .repository_path
        .canonicalize()
        .unwrap_or_else(|error| unreachable!("clone canonical path: {error:?}"));
    let local_identity = kiln_codec::identity_from_bytes(
        canonical
            .to_str()
            .unwrap_or_else(|| unreachable!())
            .as_bytes(),
    )
    .unwrap_or_else(|error| unreachable!("clone identity: {error:?}"));
    assert_eq!(
        prepared.resolution.repository.native_id.as_str(),
        local_identity.as_str()
    );
    assert_ne!(local_identity.as_str(), profile.source_repository);
    assert_eq!(
        prepared.resolution.exact_revision.native_id.as_str(),
        context.revision
    );
    let binding = binding_fixture(&profile, &prepared.resolution);
    assert_eq!(
        client::admit_binding(&profile, &prepared.resolution, &binding),
        Ok(())
    );
    let mut local_callback = binding.clone();
    local_callback.repository = serde_json::to_value(&prepared.resolution.repository)
        .unwrap_or_else(|error| unreachable!("local repository DTO: {error:?}"));
    assert!(client::admit_binding(&profile, &prepared.resolution, &local_callback).is_err());
    let output = process::execute(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
    )
    .unwrap_or_else(|error| unreachable!("clone execution: {error:?}"));
    assert_eq!(output.code, 0);
    assert_eq!(output.stdout.bytes, b"actual stdout\n");
    assert_eq!(output.stderr.bytes, b"actual stderr\n");
    assert_eq!(
        files::artifact(
            &prepared.checkout,
            std::path::Path::new("product.txt"),
            8192
        )
        .unwrap_or_else(|error| unreachable!("clone artifact: {error:?}")),
        b"committed source\n"
    );
    assert!(!profile.repository_path.join("product.txt").exists());
}

// r[verify kiln.web.remote_workers.source]
// r[verify kiln.web.remote_workers.source.scenario.denied]
#[test]
fn old_missing_and_malformed_profiles_are_rejected_before_source_effects() {
    let (root, profile, context) = fixture();
    let original = serde_json::to_value(&profile)
        .unwrap_or_else(|error| unreachable!("profile DTO: {error:?}"));
    let path = root.path().join("profile.json");
    let mut rejected = Vec::new();
    let mut old = original.clone();
    *old.get_mut("schema").unwrap_or_else(|| unreachable!()) = "kiln.web-worker.v1".into();
    rejected.push(old);
    let mut missing = original.clone();
    missing
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .remove("source_repository");
    rejected.push(missing);
    for identity in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        format!("b3:{}", "a".repeat(64)),
        "/controller/repository".into(),
    ] {
        let mut malformed = original.clone();
        *malformed
            .get_mut("source_repository")
            .unwrap_or_else(|| unreachable!()) = identity.into();
        rejected.push(malformed);
    }
    for value in rejected {
        let bytes = serde_json::to_vec(&value)
            .unwrap_or_else(|error| unreachable!("profile encoding: {error:?}"));
        fs::write(&path, &bytes).unwrap_or_else(|error| unreachable!("profile file: {error:?}"));
        let identity = blake3::hash(&bytes).to_hex().to_string();
        assert!(super::load_profile(&path, &identity).is_err());
        if let Ok(invalid) = serde_json::from_value::<WorkerProfile>(value) {
            assert!(source::prepare(&invalid, &context).is_err());
        }
        assert_eq!(
            fs::read_dir(&profile.work_root)
                .unwrap_or_else(|error| unreachable!("work directory: {error:?}"))
                .count(),
            0
        );
        assert!(!profile.repository_path.join("product.txt").exists());
    }
}

#[test]
fn exact_commit_runs_reviewed_command_and_produces_actual_artifact_not_mutable_worktree() {
    let (_root, profile, context) = fixture();
    fs::write(
        profile.repository_path.join("input.txt"),
        b"unreviewed change",
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "dirty worktree", error));
    fs::write(profile.repository_path.join("reviewed.sh"), b"exit 99")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "dirty script", error));
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "admitted checkout", error));
    assert_eq!(
        fs::read(prepared.checkout.join("input.txt"))
            .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"committed source\n"
    );
    let output = process::execute(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(output.code, 0);
    assert_eq!(output.stdout.bytes, b"actual stdout\n");
    assert_eq!(output.stderr.bytes, b"actual stderr\n");
    assert!(!output.stdout.truncated);
    assert!(!output.stderr.truncated);
    assert_eq!(super::finish_step(output.code, Ok(())), Ok(0));
    assert!(super::finish_step(output.code, Err("ingress rejected".into())).is_err());
    assert_eq!(
        files::artifact(
            &prepared.checkout,
            std::path::Path::new("product.txt"),
            8192
        )
        .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"committed source\n"
    );
    assert!(!profile.repository_path.join("product.txt").exists());
}

// r[verify kiln.web.remote_workers.source.scenario.denied]
#[test]
fn wrong_source_context_and_snapshot_overflow_never_materialize() {
    let (_root, mut profile, mut context) = fixture();
    context.repository = "f".repeat(64);
    assert!(source::prepare(&profile, &context).is_err());
    assert_eq!(
        fs::read_dir(&profile.work_root)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .count(),
        0
    );
    context.repository = profile.source_repository.clone();
    context.provider = "github".into();
    assert!(source::prepare(&profile, &context).is_err());
    context.provider = "git".into();
    for revision in [String::new(), "0".repeat(40), "f".repeat(40)] {
        context.revision = revision;
        assert!(source::prepare(&profile, &context).is_err());
        assert_eq!(
            fs::read_dir(&profile.work_root)
                .unwrap_or_else(|error| unreachable!("work directory: {error:?}"))
                .count(),
            0
        );
    }
    let (_second_root, original, original_context) = fixture();
    profile = original;
    profile.snapshot_limits.max_total_bytes = 1;
    profile.snapshot_limits.max_file_bytes = 1;
    assert!(source::prepare(&profile, &original_context).is_err());
    assert_eq!(
        fs::read_dir(&profile.work_root)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .count(),
        0
    );
}

#[test]
fn symlink_source_is_rejected_instead_of_following_outside_checkout() {
    let (_root, profile, mut context) = fixture();
    let repository = Repository::open(&profile.repository_path)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "repository", error));
    let link = repository
        .blob(b"/etc/passwd")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "link blob", error));
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "treebuilder", error));
    builder
        .insert("escape", link, 0o120000)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "symlink tree entry", error));
    let tree_id = builder
        .write()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "tree", error));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "tree object", error));
    let signature = Signature::now("Fixture", "fixture@example.invalid")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "signature", error));
    let revision = repository
        .commit(None, &signature, &signature, "symlink source", &tree, &[])
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "commit", error));
    context.revision = revision.to_string();
    assert!(source::prepare(&profile, &context).is_err());
    assert_eq!(
        fs::read_dir(&profile.work_root)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .count(),
        0
    );
}

#[test]
fn child_environment_is_explicit_and_failure_deadline_and_cancellation_are_not_success() {
    let (_root, mut profile, context) = fixture();
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("checkout: {error:?}"));
    let step = profile.steps.first_mut().unwrap_or_else(|| unreachable!());
    step.argv = vec![
        "-c".into(),
        "test -z \"$HOME$LATTICE_SOURCE_PROVIDER$KILN_WORKER_TOKEN\" && test \"$REVIEWED\" = yes || exit 31; printf failed; printf error >&2; exit 17".into(),
    ];
    step.environment.insert("REVIEWED".into(), "yes".into());
    let execute = |profile: &WorkerProfile, cancelled| {
        process::execute(
            profile,
            profile.steps.first().unwrap_or_else(|| unreachable!()),
            &prepared.checkout,
            &AtomicBool::new(cancelled),
        )
        .unwrap_or_else(|error| unreachable!("execution: {error:?}"))
    };
    let failed = execute(&profile, false);
    assert_eq!(failed.code, 17);
    assert_eq!(failed.stdout.bytes, b"failed");
    assert_eq!(failed.stderr.bytes, b"error");
    assert_eq!(
        super::finish_step(failed.code, Err("ingress rejected".into())),
        Ok(17)
    );
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), "while :; do :; done".into()];
    profile.command_timeout_ms = 30;
    let timed_out = execute(&profile, false);
    assert_eq!(timed_out.code, 124);
    assert!(timed_out.stdout.bytes.is_empty());
    assert!(timed_out.stderr.bytes.is_empty());
    assert_eq!(
        super::finish_step(timed_out.code, Err("ingress rejected".into())),
        Ok(124)
    );
    profile.command_timeout_ms = 2000;
    let cancelled = execute(&profile, true);
    assert_eq!(cancelled.code, 130);
    assert!(cancelled.stdout.bytes.is_empty());
    assert!(cancelled.stderr.bytes.is_empty());
    assert!(!cancelled.stdout.truncated);
    assert!(!cancelled.stderr.truncated);
    assert_eq!(
        super::finish_step(cancelled.code, Err("ingress rejected".into())),
        Ok(130)
    );
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), "printf 12345; printf abcdef >&2".into()];
    profile.maximum_output_bytes = 4;
    let limited = execute(&profile, false);
    assert_eq!(limited.code, 125);
    assert_eq!(limited.stdout.bytes, b"1234");
    assert_eq!(limited.stderr.bytes, b"abcd");
    assert!(limited.stdout.truncated);
    assert!(limited.stderr.truncated);
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), "printf 12345; exit 17".into()];
    let failed_limited = execute(&profile, false);
    assert_eq!(failed_limited.code, 17);
    assert_eq!(failed_limited.stdout.bytes, b"1234");
    assert!(failed_limited.stdout.truncated);
    assert!(failed_limited.stderr.bytes.is_empty());
    assert!(!failed_limited.stderr.truncated);
}

#[test]
fn live_log_budget_accounts_each_ingress_chunk_and_rejected_reservations_are_atomic() {
    let bounds = client::LogBounds {
        maximum_chunk_bytes: 4,
        maximum_total_bytes: 8,
        maximum_chunks: 3,
    };
    let mut progress = client::LogUploads::default();
    assert!(progress.reserve(&bounds, 5).is_err());
    assert_eq!(progress.reserve(&bounds, 4), Ok(()));
    assert_eq!(progress.reserve(&bounds, 4), Ok(()));
    assert!(progress.reserve(&bounds, 1).is_err());
    // Empty/truncated markers spend a chunk but not retained-byte budget.
    assert_eq!(progress.reserve(&bounds, 0), Ok(()));
    assert!(progress.reserve(&bounds, 0).is_err());
}

#[test]
fn real_artifacts_admit_regular_bytes_and_reject_escape_symlink_hardlink_fifo_and_bounds() {
    let (root, profile, context) = fixture();
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "checkout", error));
    let actual = prepared.checkout.join("actual");
    fs::write(&actual, b"actual product bytes")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "product", error));
    assert_eq!(
        files::artifact(&prepared.checkout, std::path::Path::new("actual"), 100)
            .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"actual product bytes"
    );
    assert!(files::artifact(&prepared.checkout, std::path::Path::new("actual"), 2).is_err());
    assert!(files::artifact(&prepared.checkout, std::path::Path::new("../outside"), 100).is_err());
    symlink(&actual, prepared.checkout.join("link"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "artifact symlink", error));
    assert!(files::artifact(&prepared.checkout, std::path::Path::new("link"), 100).is_err());
    symlink(root.path(), prepared.checkout.join("directory-link"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "parent symlink", error));
    assert!(
        files::artifact(
            &prepared.checkout,
            std::path::Path::new("directory-link/anything"),
            100
        )
        .is_err()
    );
    fs::hard_link(&actual, prepared.checkout.join("hardlink"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "hardlink", error));
    assert!(files::artifact(&prepared.checkout, std::path::Path::new("hardlink"), 100).is_err());
    nix::unistd::mkfifo(
        &prepared.checkout.join("fifo"),
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "fifo", error));
    assert!(files::artifact(&prepared.checkout, std::path::Path::new("fifo"), 100).is_err());
}

#[test]
fn source_attempt_execution_and_node_bindings_are_all_required() {
    let (_root, mut profile, context) = fixture();
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "checkout", error));
    let source = &prepared.resolution;
    let binding = binding_fixture(&profile, source);
    let mut unselected = profile
        .steps
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    unselected.node_id = "not-selected".into();
    profile.steps.push(unselected);
    client::admit_binding(&profile, source, &binding)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "exact run", error));
    client::exact_binding(&binding, &binding)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "current attempt", error));
    // A full native DAG includes peers executed by separately reviewed jobs.
    // Only the nonempty intersection with this worker profile is local work.
    let mut dag = binding.clone();
    dag.nodes.push(client::Node {
        id: "verify".into(),
    });
    assert_eq!(client::admit_binding(&profile, source, &dag), Ok(()));
    let mut verify_profile = profile.clone();
    verify_profile.steps.truncate(1);
    verify_profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .node_id = "verify".into();
    assert_eq!(client::admit_binding(&verify_profile, source, &dag), Ok(()));
    assert!(client::admit_binding(&verify_profile, source, &binding).is_err());
    let mut malformed_peer = dag.clone();
    malformed_peer
        .nodes
        .get_mut(1)
        .unwrap_or_else(|| unreachable!())
        .id = "../verify".into();
    assert!(client::admit_binding(&profile, source, &malformed_peer).is_err());
    let mut peer_only = binding.clone();
    peer_only
        .nodes
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .id = "verify".into();
    assert!(client::admit_binding(&profile, source, &peer_only).is_err());
    let mut stale = binding.clone();
    stale.attempt_id = "attempt-2".into();
    assert!(client::exact_binding(&binding, &stale).is_err());
    stale = binding.clone();
    *stale
        .execution
        .get_mut("native_id")
        .unwrap_or_else(|| unreachable!()) = "native-2".into();
    assert!(client::exact_binding(&binding, &stale).is_err());
    stale = binding.clone();
    stale.source_revision = "f".repeat(40);
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    *stale
        .repository
        .get_mut("native_id")
        .unwrap_or_else(|| unreachable!()) = "other-repository".into();
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.nodes.clear();
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.outcome = "Unknown".into();
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale.outcome = "Success".into();
    assert!(client::exact_binding(&binding, &stale).is_err());
    stale = binding.clone();
    stale.log_bounds.maximum_chunk_bytes = 0;
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.log_bounds.maximum_total_bytes = 0;
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.log_bounds.maximum_chunks = 0;
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.log_bounds.maximum_chunk_bytes = stale.log_bounds.maximum_total_bytes + 1;
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.log_bounds.maximum_chunks += 1;
    assert!(client::exact_binding(&binding, &stale).is_err());
    stale = binding.clone();
    stale.nodes.push(client::Node { id: "build".into() });
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    stale = binding.clone();
    stale.nodes.first_mut().unwrap_or_else(|| unreachable!()).id = "unreviewed".into();
    assert!(client::admit_binding(&profile, source, &stale).is_err());
    assert!(client::exact_binding(&binding, &stale).is_err());
    let mut json = serde_json::to_value(&binding)
        .unwrap_or_else(|error| unreachable!("binding JSON: {error:?}"));
    json.as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .remove("log_bounds");
    assert!(serde_json::from_value::<client::RunBinding>(json).is_err());
}

#[test]
fn worker_profile_denies_secret_environment_and_untrusted_artifact_declarations() {
    let (_root, mut profile, _) = fixture();
    profile
        .admit()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "reviewed profile", error));
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .environment
        .insert("WORKER_TOKEN".into(), "never-forward".into());
    assert!(profile.admit().is_err());
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .environment
        .clear();
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .artifacts = vec!["../../worker.token".into()];
    assert!(profile.admit().is_err());
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .artifacts = vec!["a/product.txt".into(), "b/product.txt".into()];
    assert!(profile.admit().is_err());
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .artifacts
        .clear();
    profile.service_origin = "http://localhost:9443".into();
    assert!(profile.admit().is_err());
}

#[test]
fn worker_credential_reads_owned_private_regular_file_only() {
    let (root, profile, _) = fixture();
    let token = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    fs::write(&profile.token_file, token)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "credential file", error));
    for mode in [0o400, 0o600] {
        fs::set_permissions(&profile.token_file, fs::Permissions::from_mode(mode))
            .unwrap_or_else(|error| unreachable!("private mode: {error:?}"));
        assert_eq!(
            files::regular(&profile.token_file, 64, true)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            token
        );
    }
    // Sticky exercises special-bit rejection without requiring sandbox-forbidden set-ID chmod.
    for mode in [0o644, 0o640, 0o500, 0o700, 0o1600] {
        fs::set_permissions(&profile.token_file, fs::Permissions::from_mode(mode))
            .unwrap_or_else(|error| unreachable!("unsafe mode: {error:?}"));
        assert!(files::regular(&profile.token_file, 64, true).is_err());
    }
    fs::set_permissions(&profile.token_file, fs::Permissions::from_mode(0o600))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "private mode", error));
    let alias = root.path().join("token-link");
    symlink(&profile.token_file, &alias)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "credential symlink", error));
    assert!(files::regular(&alias, 64, true).is_err());
    fs::write(&profile.token_file, [b'x'; 65])
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "oversize credential", error));
    assert!(files::regular(&profile.token_file, 64, true).is_err());
}

#[test]
fn in_flight_cancellation_stops_owned_command_and_preserves_cancelled_exit() {
    let (_root, mut profile, context) = fixture();
    let prepared = source::prepare(&profile, &context)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "checkout", error));
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec![
        "-c".into(),
        "printf 'before cancel'; : > cancellation-ready; while :; do :; done".into(),
    ];
    let cancellation = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let started = std::time::Instant::now();
            while !prepared.checkout.join("cancellation-ready").exists()
                && started.elapsed() < std::time::Duration::from_secs(1)
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            cancellation.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let output = process::execute(
            &profile,
            profile.steps.first().unwrap_or_else(|| unreachable!()),
            &prepared.checkout,
            &cancellation,
        )
        .unwrap_or_else(|error| unreachable!("execution: {error:?}"));
        assert_eq!(output.code, 130);
        assert_eq!(output.stdout.bytes, b"before cancel");
        assert!(output.stderr.bytes.is_empty());
        assert!(!output.stdout.truncated);
        assert!(!output.stderr.truncated);
    });
}

#[test]
fn output_is_ingressed_before_exit_and_each_retained_byte_is_uploaded_once() {
    let (_root, mut profile, context) = fixture();
    let prepared =
        source::prepare(&profile, &context).unwrap_or_else(|error| unreachable!("{error}"));
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(),
        "printf early-out; printf early-err >&2; while test ! -f release; do :; done; printf late-out; printf late-err >&2; : > completed".into()];
    let mut uploaded_stdout = Vec::new();
    let mut uploaded_stderr = Vec::new();
    let output = process::execute_streamed(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
        64,
        |chunk| {
            let uploaded = match chunk.stream {
                super::streaming::Stream::Stdout => &mut uploaded_stdout,
                super::streaming::Stream::Stderr => &mut uploaded_stderr,
            };
            uploaded.extend_from_slice(&chunk.bytes);
            if uploaded_stdout == b"early-out" && uploaded_stderr == b"early-err" {
                assert!(!prepared.checkout.join("completed").exists());
                fs::write(prepared.checkout.join("release"), b"go")
                    .unwrap_or_else(|error| unreachable!("{error}"));
            }
            Ok(())
        },
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(output.code, 0);
    assert_eq!(output.ingress, Ok(()));
    assert_eq!(uploaded_stdout, b"early-outlate-out");
    assert_eq!(uploaded_stderr, b"early-errlate-err");
    assert_eq!(uploaded_stdout, output.stdout.bytes);
    assert_eq!(uploaded_stderr, output.stderr.bytes);
    assert!(prepared.checkout.join("completed").exists());
}

#[test]
fn overflow_preserves_prefix_cursors_and_failed_ingress_never_retries_or_masks_exit() {
    let (_root, mut profile, context) = fixture();
    let prepared =
        source::prepare(&profile, &context).unwrap_or_else(|error| unreachable!("{error}"));
    profile.maximum_output_bytes = 4;
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), "printf abcdef; printf 123456 >&2".into()];
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let mut stdout_truncated = 0;
    let mut stderr_truncated = 0;
    let output = process::execute_streamed(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
        2,
        |chunk| {
            assert!(chunk.bytes.len() <= 2);
            let (bytes, truncated) = match chunk.stream {
                super::streaming::Stream::Stdout => (&mut stdout_bytes, &mut stdout_truncated),
                super::streaming::Stream::Stderr => (&mut stderr_bytes, &mut stderr_truncated),
            };
            bytes.extend_from_slice(&chunk.bytes);
            *truncated += usize::from(chunk.truncated);
            Ok(())
        },
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(output.code, 125);
    assert_eq!(stdout_bytes, b"abcd");
    assert_eq!(stderr_bytes, b"1234");
    assert_eq!((stdout_truncated, stderr_truncated), (1, 1));
    for (command, expected_code) in [("printf abcdef", 125), ("printf abcdef; exit 17", 17)] {
        profile
            .steps
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .argv = vec!["-c".into(), command.into()];
        let mut attempts = 0;
        let output = process::execute_streamed(
            &profile,
            profile.steps.first().unwrap_or_else(|| unreachable!()),
            &prepared.checkout,
            &AtomicBool::new(false),
            2,
            |_| {
                attempts += 1;
                Err("rejected ingress".into())
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(attempts, 1);
        assert_eq!(output.code, expected_code);
        assert_eq!(
            super::finish_step(output.code, output.ingress),
            Ok(expected_code)
        );
    }
    profile.maximum_output_bytes = 4096;
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), "printf accepted-process".into()];
    let output = process::execute_streamed(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
        2,
        |_| Err("rejected ingress".into()),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(output.code, 0);
    assert!(super::finish_step(output.code, output.ingress).is_err());
    profile
        .steps
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .argv = vec!["-c".into(), ":".into()];
    let mut stdout_empty_records = 0;
    let mut stderr_empty_records = 0;
    let output = process::execute_streamed(
        &profile,
        profile.steps.first().unwrap_or_else(|| unreachable!()),
        &prepared.checkout,
        &AtomicBool::new(false),
        2,
        |chunk| {
            assert!(chunk.bytes.is_empty());
            assert!(!chunk.truncated);
            let empty_records = match chunk.stream {
                super::streaming::Stream::Stdout => &mut stdout_empty_records,
                super::streaming::Stream::Stderr => &mut stderr_empty_records,
            };
            *empty_records += 1;
            Ok(())
        },
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(output.code, 0);
    assert_eq!(output.ingress, Ok(()));
    assert_eq!((stdout_empty_records, stderr_empty_records), (1, 1));
}

#[test]
fn streaming_cancellation_and_deadline_terminate_owned_descendants() {
    for cancelled in [false, true] {
        let (_root, mut profile, context) = fixture();
        let prepared =
            source::prepare(&profile, &context).unwrap_or_else(|error| unreachable!("{error}"));
        profile.command_timeout_ms = if cancelled { 2000 } else { 300 };
        profile
            .steps
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .argv = vec!["-c".into(),
            "/bin/sh -c 'while :; do :; done' & printf '%s' \"$!\" > descendant.pid; printf ready; while :; do :; done".into()];
        let cancellation = AtomicBool::new(false);
        let output = process::execute_streamed(
            &profile,
            profile.steps.first().unwrap_or_else(|| unreachable!()),
            &prepared.checkout,
            &cancellation,
            64,
            |chunk| {
                if cancelled && chunk.bytes == b"ready" {
                    cancellation.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(())
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(output.code, if cancelled { 130 } else { 124 });
        assert_eq!(output.stdout.bytes, b"ready");
        let pid = fs::read_to_string(prepared.checkout.join("descendant.pid"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let status = fs::read_to_string(format!("/proc/{pid}/stat"));
        assert!(
            status.is_err()
                || status.as_ref().is_ok_and(|stat| {
                    stat.rsplit_once(')')
                        .is_some_and(|(_, tail)| tail.trim_start().starts_with('Z'))
                }),
            "owned descendant remains alive"
        );
    }
}
