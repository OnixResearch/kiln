use super::*;
use git2::Signature;
use std::sync::atomic::Ordering;

fn profile(script: &str) -> JobProfile {
    JobProfile {
        schema_version: JOB_SCHEMA_VERSION,
        owner: "example".to_owned(),
        repository: "project".to_owned(),
        program_path: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_owned(), script.to_owned()],
        environment: BTreeMap::from([("CI".to_owned(), "true".to_owned())]),
        source_secret: None,
        snapshot_limits: GitSnapshotLimits {
            max_entries: 32,
            max_path_bytes: 256,
            max_file_bytes: 4096,
            max_total_bytes: 8192,
        },
        maximum_transfer_bytes: 1_048_576,
        acquisition_timeout_ms: 5000,
        command_timeout_ms: 2000,
        maximum_output_bytes: 1024,
        teardown_grace_ms: 1000,
    }
}

fn commit(repository: &Repository, name: &str, content: &[u8], mode: i32) -> String {
    let blob = repository.blob(content).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let mut builder = repository.treebuilder(None).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    builder.insert(name, blob, mode).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let tree_id = builder.write().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let tree = repository.find_tree(tree_id).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let signature = Signature::now("fixture", "fixture@example.invalid").unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    repository
        .commit(None, &signature, &signature, "fixture", &tree, &[])
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        })
        .to_string()
}

fn context(profile: &JobProfile, revision: &str) -> SourceContext {
    admit_source_context(profile, "github", "example/project", revision).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    })
}

// r[verify kiln.github.profile] r[verify kiln.github.source]
#[test]
fn rejects_wrong_source_and_profile_bytes_before_execution() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let profile = profile("exit 0");
    let revision = "1234567890abcdef1234567890abcdef12345678";
    assert!(admit_source_context(&profile, "git", "example/project", revision).is_err());
    assert!(admit_source_context(&profile, "github", "other/project", revision).is_err());
    for revision in [
        "main",
        "0000000000000000000000000000000000000000",
        "1234567890abcdef1234567890abcdef1234567Z",
    ] {
        assert!(admit_source_context(&profile, "github", "example/project", revision).is_err());
    }
    let path = root.path().join("profile.json");
    let bytes = serde_json::to_vec(&profile).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let identity = blake3::hash(&bytes).to_hex().to_string();
    fs::write(&path, &bytes).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    assert_eq!(
        load_job_profile(&path, &identity).unwrap_or_else(|error| unreachable!(
            "identity-bound fixture profile must load: {error:?}"
        )),
        profile
    );
    fs::write(&path, [bytes.as_slice(), b"\n"].concat()).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    assert!(load_job_profile(&path, &identity).is_err());
    fs::write(&path, vec![b' '; MAXIMUM_PROFILE_BYTES + 1]).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    assert!(load_job_profile(&path, &identity).is_err());
    let mut unsafe_profile = profile.clone();
    unsafe_profile
        .environment
        .insert("GITHUB_TOKEN".to_owned(), "not-forwarded".to_owned());
    assert!(admit_job_profile(&unsafe_profile).is_err());
    unsafe_profile = profile;
    unsafe_profile.program_path = PathBuf::from("sh");
    assert!(admit_job_profile(&unsafe_profile).is_err());
}

// r[verify kiln.github.source]
#[test]
fn exact_checkout_content_and_real_exit_outcome() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let repository = Repository::init_bare(root.path().join("objects")).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let revision = commit(
        &repository,
        "source.txt",
        b"exact admitted bytes\n",
        0o100_644,
    );
    let mut profile = profile(
        "IFS= read -r line < source.txt; test \"$line\" = 'exact admitted bytes' && test ! -e .git && test \"$CI\" = true && test -z \"${HOME+x}\" && test -z \"${GITHUB_TOKEN+x}\"",
    );
    let context = context(&profile, &revision);
    let checkout = root.path().join("checkout");
    let canonical =
        prepare_checkout(&profile, &revision, &repository, &checkout).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    let success = execute_command(&profile, &context, canonical.clone(), &checkout, None)
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    assert!(success.successful());
    assert_eq!(
        fs::read(checkout.join("source.txt")).unwrap_or_else(|error| unreachable!(
            "admitted source fixture must be materialized: {error}"
        )),
        b"exact admitted bytes\n"
    );
    *profile.args.get_mut(1).unwrap_or_else(|| {
        unreachable!("reviewed shell fixture must declare its script argument")
    }) = "exit 23".to_owned();
    let failure =
        execute_command(&profile, &context, canonical, &checkout, None).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    assert_eq!(
        failure.termination,
        JobTermination::Exited {
            code: Some(23),
            signal: None
        }
    );
    assert!(!failure.successful());
}

// r[verify kiln.github.source]
#[test]
fn missing_unsafe_and_oversized_tree_never_materialize() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let repository = Repository::init_bare(root.path().join("objects")).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let mut profile = profile("exit 0");
    let checkout = root.path().join("checkout");
    assert!(
        prepare_checkout(
            &profile,
            "1234567890abcdef1234567890abcdef12345678",
            &repository,
            &checkout
        )
        .is_err()
    );
    assert!(!checkout.exists());
    let symlink = commit(&repository, "escape", b"../../outside", 0o120_000);
    assert!(prepare_checkout(&profile, &symlink, &repository, &checkout).is_err());
    assert!(!checkout.exists());
    let large = commit(&repository, "large", b"oversized", 0o100_644);
    profile.snapshot_limits.max_file_bytes = 4;
    assert!(prepare_checkout(&profile, &large, &repository, &checkout).is_err());
    assert!(!checkout.exists());
    profile.snapshot_limits.max_file_bytes = 4096;
    profile.snapshot_limits.max_total_bytes = 4;
    assert!(prepare_checkout(&profile, &large, &repository, &checkout).is_err());
    assert!(!checkout.exists());
}

// r[verify kiln.github.source]
#[test]
fn deadline_cancellation_and_output_overflow_are_not_success() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let repository = Repository::init_bare(root.path().join("objects")).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let revision = commit(&repository, "source.txt", b"source", 0o100_644);
    let mut profile = profile("while :; do :; done");
    profile.command_timeout_ms = 40;
    let context = context(&profile, &revision);
    let checkout = root.path().join("checkout");
    let canonical =
        prepare_checkout(&profile, &revision, &repository, &checkout).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    let timeout = execute_command(&profile, &context, canonical.clone(), &checkout, None)
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    assert_eq!(timeout.termination, JobTermination::TimedOut);
    assert!(!timeout.successful());
    let cancellation = AtomicBool::new(true);
    let cancelled = execute_command(
        &profile,
        &context,
        canonical.clone(),
        &checkout,
        Some(&cancellation),
    )
    .unwrap_or_else(|error| unreachable!("GitHub source/job fixture operation failed: {error:?}"));
    assert_eq!(cancelled.termination, JobTermination::Cancelled);
    assert!(cancellation.load(Ordering::Relaxed));
    profile.command_timeout_ms = 2000;
    profile.maximum_output_bytes = 8;
    // Bounded Exec retains a bounded prefix while draining the stream; an
    // endless emitter reaches its deadline, and timeout correctly takes
    // precedence. A finite successful command isolates overflow rejection.
    *profile.args.get_mut(1).unwrap_or_else(|| {
        unreachable!("reviewed shell fixture must declare its script argument")
    }) = "printf '12345678'".to_owned();
    let exact_bound = execute_command(&profile, &context, canonical.clone(), &checkout, None)
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    assert!(exact_bound.successful());
    for command in ["printf 'excess output'", "printf 'excess output' >&2"] {
        *profile.args.get_mut(1).unwrap_or_else(|| {
            unreachable!("reviewed shell fixture must declare its script argument")
        }) = command.to_owned();
        let overflow = execute_command(&profile, &context, canonical.clone(), &checkout, None)
            .unwrap_or_else(|error| {
                unreachable!("GitHub source/job fixture operation failed: {error:?}")
            });
        assert_eq!(overflow.termination, JobTermination::OutputLimitExceeded);
        assert!(!overflow.successful());
    }
}
