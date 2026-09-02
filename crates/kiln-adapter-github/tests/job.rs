#![cfg(feature = "protocol-fixtures")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use git2::{Repository, Signature};
use kiln_adapter_github::job::{JOB_SCHEMA_VERSION, JobProfile};
use kiln_core::source::GitSnapshotLimits;

fn profile() -> JobProfile {
    JobProfile {
        schema_version: JOB_SCHEMA_VERSION, owner: "example".to_owned(), repository: "project".to_owned(),
        program_path: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_owned(), "IFS= read -r value < source.txt; test \"$value\" = 'selected exact commit' && test ! -e .git && test -z \"${GITHUB_TOKEN+x}\" && test -z \"${HOME+x}\"".to_owned()],
        environment: BTreeMap::new(), source_secret: None,
        snapshot_limits: GitSnapshotLimits { max_entries: 32, max_path_bytes: 256, max_file_bytes: 4096, max_total_bytes: 8192 },
        maximum_transfer_bytes: 1_048_576, acquisition_timeout_ms: 5000,
        command_timeout_ms: 2000, maximum_output_bytes: 1024, teardown_grace_ms: 1000,
    }
}

fn sleep_program() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_else(|| {
        unreachable!("GitHub process fixtures require PATH for coreutils lookup")
    }))
    .map(|directory| directory.join("sleep"))
    .find(|program| program.is_file())
    .unwrap_or_else(|| unreachable!("the integration fixture requires coreutils sleep on PATH"))
}

fn commit(repository: &Repository, content: &[u8], parent: Option<git2::Oid>) -> git2::Oid {
    let blob = repository.blob(content).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let mut builder = repository.treebuilder(None).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    builder
        .insert("source.txt", blob, 0o100_644)
        .unwrap_or_else(|error| {
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
    let parent = parent.map(|id| {
        repository.find_commit(id).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        })
    });
    let parents: Vec<_> = parent.iter().collect();
    repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "fixture",
            &tree,
            &parents,
        )
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        })
}

fn invoke(
    profile: &JobProfile,
    directory: &std::path::Path,
    source: &std::path::Path,
    revision: &str,
    repository: &str,
    wrong_digest: bool,
) -> Output {
    let bytes = serde_json::to_vec(profile).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let path = directory.join("profile.json");
    fs::write(&path, &bytes).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let identity = if wrong_digest {
        "0".repeat(64)
    } else {
        blake3::hash(&bytes).to_hex().to_string()
    };
    Command::new(env!("CARGO_BIN_EXE_kiln-github-job"))
        .args([
            "--profile",
            path.to_str()
                .unwrap_or_else(|| unreachable!("GitHub job fixture path must be UTF-8")),
            "--identity",
            &identity,
            "--fixture-source",
            source
                .to_str()
                .unwrap_or_else(|| unreachable!("GitHub job fixture path must be UTF-8")),
        ])
        .env_clear()
        .env("LATTICE_SOURCE_PROVIDER", "github")
        .env("LATTICE_SOURCE_REPOSITORY", repository)
        .env("LATTICE_SOURCE_REVISION", revision)
        .env("GITHUB_TOKEN", "must-not-reach-command")
        .output()
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        })
}

// r[verify kiln.github.source]
#[test]
fn actual_helper_acquires_selected_commit_not_branch_tip_and_propagates_failure() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let source_path = root.path().join("source.git");
    let source = Repository::init_bare(&source_path).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let selected = commit(&source, b"selected exact commit\n", None);
    let _new_tip = commit(&source, b"unselected branch tip\n", Some(selected));
    let mut profile = profile();
    let success = invoke(
        &profile,
        root.path(),
        &source_path,
        &selected.to_string(),
        "example/project",
        false,
    );
    assert_eq!(
        success.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert_eq!(success.stdout, b"");
    *profile.args.get_mut(1).unwrap_or_else(|| {
        unreachable!("reviewed shell fixture must declare its script argument")
    }) = "printf secret-looking-output; printf secret-looking-error >&2; exit 23".to_owned();
    let failure = invoke(
        &profile,
        root.path(),
        &source_path,
        &selected.to_string(),
        "example/project",
        false,
    );
    assert_eq!(failure.status.code(), Some(23));
    assert_eq!(failure.stdout, b"");
    assert_eq!(failure.stderr, b"");
}

// r[verify kiln.github.source] r[verify kiln.github.profile]
#[test]
fn actual_helper_rejects_wrong_source_identity_missing_commit_and_transfer_bound() {
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let source_path = root.path().join("source.git");
    let source = Repository::init_bare(&source_path).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let selected = commit(&source, b"selected exact commit\n", None).to_string();
    let mut profile = profile();
    for (revision, repository, wrong_digest) in [
        (selected.as_str(), "other/project", false),
        (selected.as_str(), "example/project", true),
        ("main", "example/project", false),
        (
            "1234567890abcdef1234567890abcdef12345678",
            "example/project",
            false,
        ),
    ] {
        let rejected = invoke(
            &profile,
            root.path(),
            &source_path,
            revision,
            repository,
            wrong_digest,
        );
        assert_eq!(rejected.status.code(), Some(1));
        assert_eq!(rejected.stdout, b"");
        assert_eq!(rejected.stderr, b"GitHub job failed\n");
    }
    profile.maximum_transfer_bytes = 1;
    let rejected = invoke(
        &profile,
        root.path(),
        &source_path,
        &selected,
        "example/project",
        false,
    );
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(rejected.stderr, b"GitHub job failed\n");
}

// r[verify kiln.github.source]
#[test]
fn actual_helper_cancellation_terminates_owned_command_descendants() {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let source_path = root.path().join("source.git");
    let source = Repository::init_bare(&source_path).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let selected = commit(&source, b"selected exact commit\n", None).to_string();
    let started = root.path().join("started");
    let escaped = root.path().join("escaped");
    let mut profile = profile();
    profile.command_timeout_ms = 5000;
    *profile.args.get_mut(1).unwrap_or_else(|| {
        unreachable!("reviewed shell fixture must declare its script argument")
    }) = format!(
        "( printf ready > '{}'; '{}' 0.4; printf escaped > '{}' ) & wait",
        started.display(),
        sleep_program().display(),
        escaped.display()
    );
    let bytes = serde_json::to_vec(&profile).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let path = root.path().join("profile.json");
    fs::write(&path, &bytes).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let identity = blake3::hash(&bytes).to_hex().to_string();
    let child = Command::new(env!("CARGO_BIN_EXE_kiln-github-job"))
        .args([
            "--profile",
            path.to_str()
                .unwrap_or_else(|| unreachable!("GitHub job fixture path must be UTF-8")),
            "--identity",
            &identity,
            "--fixture-source",
            source_path
                .to_str()
                .unwrap_or_else(|| unreachable!("GitHub job fixture path must be UTF-8")),
        ])
        .env_clear()
        .env("LATTICE_SOURCE_PROVIDER", "github")
        .env("LATTICE_SOURCE_REPOSITORY", "example/project")
        .env("LATTICE_SOURCE_REVISION", selected)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    let until = Instant::now() + Duration::from_secs(5);
    while !started.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    // Explicit operator SIGTERM exercises graceful cleanup; the outer-workflow
    // cases below exercise the actual executor's abrupt SIGKILL path.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap_or_else(|error| {
            unreachable!("fixture child PID must fit the platform signal interface: {error}")
        })),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap_or_else(|error| unreachable!("GitHub source/job fixture operation failed: {error:?}"));
    let output = child.wait_with_output().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    assert!(
        started.exists(),
        "command did not start: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(1));
    std::thread::sleep(Duration::from_millis(450));
    assert!(
        !escaped.exists(),
        "owned descendant survived workflow cancellation"
    );
}

fn outer_workflow_kill_cleans_descendants(cancel: bool) {
    use bounded_exec::{
        CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy,
        RunRequest, TerminationScope,
    };
    use std::ffi::OsString;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let source_path = root.path().join("source.git");
    let source = Repository::init_bare(&source_path).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let selected = commit(&source, b"selected exact commit\n", None).to_string();
    let started = root.path().join("started");
    let checkout_marker = root.path().join("checkout-path");
    let release = root.path().join("release");
    let escaped = root.path().join("escaped");
    let mut profile = profile();
    profile.command_timeout_ms = 10_000;
    *profile.args.get_mut(1).unwrap_or_else(|| {
        unreachable!("reviewed shell fixture must declare its script argument")
    }) = format!(
        "( pwd > '{}'; printf ready > '{}'; while test ! -e '{}'; do '{}' 0.01; done; printf escaped > '{}' ) & wait",
        checkout_marker.display(),
        started.display(),
        release.display(),
        sleep_program().display(),
        escaped.display()
    );
    let bytes = serde_json::to_vec(&profile).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let path = root.path().join("profile.json");
    fs::write(&path, &bytes).unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    let identity = blake3::hash(&bytes).to_hex().to_string();
    let cancellation = Arc::new(AtomicBool::new(false));
    let cancel_thread = if cancel {
        let flag = cancellation.clone();
        let started = started.clone();
        Some(std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !started.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            flag.store(true, Ordering::Release);
        }))
    } else {
        None
    };
    let request = RunRequest {
        command: CommandSpec {
            // This is the actual saved-shell nesting: the outer Bounded Exec
            // group contains both shell and helper, then dies by SIGKILL.
            program: PathBuf::from("/bin/sh"),
            args: vec![
                OsString::from("-c"),
                OsString::from(format!(
                    "'{}' --profile '{}' --identity '{}' --fixture-source '{}'",
                    env!("CARGO_BIN_EXE_kiln-github-job"),
                    path.display(),
                    identity,
                    source_path.display()
                )),
            ],
            current_dir: root.path().to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: vec![
                (
                    OsString::from("LATTICE_SOURCE_PROVIDER"),
                    OsString::from("github"),
                ),
                (
                    OsString::from("LATTICE_SOURCE_REPOSITORY"),
                    OsString::from("example/project"),
                ),
                (
                    OsString::from("LATTICE_SOURCE_REVISION"),
                    OsString::from(selected),
                ),
                (OsString::from("TMPDIR"), root.path().as_os_str().to_owned()),
            ],
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: if cancel { 10_000 } else { 2000 },
            stdin_max_bytes: 1,
            stdout_max_bytes: 1024,
            stderr_max_bytes: 1024,
            poll_interval_ms: 5,
            teardown_timeout_ms: 1000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        }),
    };
    let output =
        bounded_exec::run_with_cancellation(request, &cancellation).unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    if let Some(thread) = cancel_thread {
        thread.join().unwrap_or_else(|error| {
            unreachable!("GitHub source/job fixture operation failed: {error:?}")
        });
    }
    assert_eq!(
        output.disposition,
        if cancel {
            Disposition::Cancelled
        } else {
            Disposition::TimedOut
        }
    );
    assert!(
        started.exists(),
        "reviewed command did not start before outer kill: {}",
        String::from_utf8_lossy(&output.stderr.bytes)
    );
    let checkout = PathBuf::from(
        fs::read_to_string(&checkout_marker)
            .unwrap_or_else(|error| {
                unreachable!("GitHub source/job fixture operation failed: {error:?}")
            })
            .trim(),
    );
    let acquisition_root = checkout.parent().unwrap_or_else(|| {
        unreachable!("GitHub job checkout fixture must have an owned parent directory")
    });
    let lifelines_exist = || {
        fs::read_dir(root.path())
            .unwrap_or_else(|error| {
                unreachable!("GitHub source/job fixture operation failed: {error:?}")
            })
            .any(|entry| {
                entry
                    .unwrap_or_else(|error| {
                        unreachable!("GitHub source/job fixture operation failed: {error:?}")
                    })
                    .file_name()
                    .to_string_lossy()
                    .starts_with("kiln-github-lifeline-")
            })
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while (acquisition_root.exists() || lifelines_exist()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    // Release any buggy survivor even on a failing-before run, rather than
    // leaving a background process waiting after the regression test fails.
    fs::write(&release, b"release").unwrap_or_else(|error| {
        unreachable!("GitHub source/job fixture operation failed: {error:?}")
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !escaped.exists(),
        "command descendant survived outer workflow SIGKILL"
    );
    assert!(
        !acquisition_root.exists(),
        "supervisor did not clean its owned checkout"
    );
    assert!(
        !lifelines_exist(),
        "supervisor did not clean its owned socket root"
    );
}

// r[verify kiln.github.source]
#[test]
fn outer_workflow_timeout_sigkill_cleans_source_job_process_tree() {
    outer_workflow_kill_cleans_descendants(false);
}

// r[verify kiln.github.source]
#[test]
fn outer_workflow_cancellation_sigkill_cleans_source_job_process_tree() {
    outer_workflow_kill_cleans_descendants(true);
}
