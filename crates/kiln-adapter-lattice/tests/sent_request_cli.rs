//! `kiln-lattice-dispatch --sent-request true` as a process: exactly one JSON
//! object on stdout on success, nothing on stdout on refusal, and no write.
#![cfg(target_os = "linux")]

use kiln_core::implementation::{
    AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, Enrollment,
    FrozenRequest, IMPLEMENTATION_WORK_SCHEMA, ImplementationDispatchPlan, ImplementationWork,
    SourceRef, TerminalPolicy, TransportLaunch, TransportStart, WorkBudget, WorkSourceKind,
    WorkspaceIsolation,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEASE_MS: u64 = 600_000;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    }
}

fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: Some(
            ExternalRef::new("git", "commit", &"a".repeat(40)).unwrap_or_else(|_| unreachable!()),
        ),
        title: "Sent request report".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#49")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#49".to_string(),
            version: content('1'),
        }],
    }
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!());
}

/// Every file under `root` with its bytes.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap_or_else(|_| unreachable!()) {
            let path = entry.unwrap_or_else(|_| unreachable!()).path();
            if path.is_dir() {
                pending.push(path.clone());
                entries.insert(path, Vec::new());
            } else {
                let bytes = std::fs::read(&path).unwrap_or_else(|_| unreachable!());
                entries.insert(path, bytes);
            }
        }
    }
    entries
}

/// Run the binary and check that it changed nothing under `root`.
fn report(root: &Path, args: &[(&str, String)]) -> Output {
    let before = tree(root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_kiln-lattice-dispatch"));
    command.env_clear();
    for (flag, value) in args {
        command.arg(flag).arg(value);
    }
    let output = command.output().unwrap_or_else(|_| unreachable!());
    assert_eq!(
        tree(root),
        before,
        "the report wrote under {}",
        root.display()
    );
    output
}

fn refused(output: &Output, reason: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty(), "stdout on refusal: {output:?}");
    assert!(stderr.contains(reason), "expected {reason:?}: {stderr}");
}

/// One claimed attempt with a reserved plan and a sealed admitted request,
/// and the query inputs and flags of the report mode under `root`.
struct Bound {
    owner: Implementer,
    attempt: kiln_core::implementation::ImplementationAttempt,
    reserved: ContentId,
    admitted: Vec<u8>,
    args: Vec<(&'static str, String)>,
}

fn bound(root: &Path) -> Bound {
    let database = root.join("kiln.sqlite");
    let work = work();
    let mut owner = Implementer::open_with_local_operator(&database, enrollment(), "operator")
        .unwrap_or_else(|error| unreachable!("{error}"));
    owner
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempt = owner
        .claim_next(1, LEASE_MS)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!());
    let plan = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('e'),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: ProviderProfileId::new("lattice-implementation")
            .unwrap_or_else(|_| unreachable!()),
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1000,
        invocation_budget: 1,
        expected_output: content('f'),
        execution: None,
    };
    let reserved = owner
        .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &plan)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .plan;
    let admitted = serde_json::to_vec(&serde_json::json!({
        "schema": "lattice.implementation-attempt-run.v3",
        "envelope": {"attempt_id": attempt.identity.as_str(), "fence": attempt.fence},
        "network": false,
    }))
    .unwrap_or_else(|_| unreachable!());
    owner
        .bind_campaign_request(&work, &attempt.identity, attempt.fence, 1, &admitted)
        .unwrap_or_else(|error| unreachable!("{error}"));

    let inputs = root.join("inputs");
    std::fs::create_dir_all(&inputs).unwrap_or_else(|_| unreachable!());
    write(&inputs.join("attempt.json"), &admitted);
    write(
        &inputs.join("enrollment.json"),
        &serde_json::to_vec(&enrollment()).unwrap_or_else(|_| unreachable!()),
    );
    write(
        &inputs.join("transport.json"),
        &serde_json::to_vec(&serde_json::json!({
            "schema": "kiln.lattice-implementation-cli.v2",
            "lattice_program": "/nix/store/00000000000000000000000000000000-lattice/bin/lattice",
            "executable_blake3": "b".repeat(64),
            "state_directory": root.join("current-state"),
            "output_directory": root.join("current-output"),
        }))
        .unwrap_or_else(|_| unreachable!()),
    );
    let args = vec![
        ("--database", database.display().to_string()),
        (
            "--enrollment",
            inputs.join("enrollment.json").display().to_string(),
        ),
        (
            "--worker-request",
            inputs.join("attempt.json").display().to_string(),
        ),
        (
            "--worker-transport-profile",
            inputs.join("transport.json").display().to_string(),
        ),
        ("--sent-request", "true".to_string()),
    ];
    Bound {
        owner,
        attempt,
        reserved,
        admitted,
        args,
    }
}

fn launch(root: &Path) -> TransportLaunch {
    TransportLaunch {
        lattice_program: "/nix/store/00000000000000000000000000000000-lattice/bin/lattice".into(),
        executable_blake3: content('b'),
        state_directory: format!("{}/recorded-state/", root.display()),
    }
}

// r[verify kiln.admission.lattice_lineage.scenario.reported]
#[test]
fn a_record_naming_other_bytes_is_crossed() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let Bound {
        mut owner,
        attempt,
        reserved,
        args,
        ..
    } = bound(root);
    // The recorded file still hashes to the recorded BLAKE3, but it is not
    // the admitted request nor that request plus one lineage.
    let other = b"{\"schema\":\"lattice.implementation-attempt-run.v3\",\"other\":true}";
    let path = root.join("other.json");
    write(&path, other);
    let sent = FrozenRequest {
        path: path.display().to_string(),
        blake3: ContentId::from_blake3_hex(blake3::hash(other).to_hex().to_string())
            .unwrap_or_else(|_| unreachable!()),
    };
    assert_eq!(
        owner.begin_dispatch_transport_sending(
            &attempt.identity,
            attempt.fence,
            1,
            &reserved,
            Some(&launch(root)),
            Some(&sent)
        ),
        Ok(TransportStart::Fresh)
    );
    drop(owner);
    refused(&report(root, &args), "(frozen_request_crossed)");
}

// r[verify kiln.admission.lattice_lineage.scenario.reported]
#[test]
fn the_process_prints_one_object_or_nothing_and_writes_nothing() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let Bound {
        mut owner,
        attempt,
        reserved,
        admitted,
        args,
    } = bound(root);
    let inputs = root.join("inputs");

    // Sealed but not dispatched.
    refused(&report(root, &args), "no dispatch transport intent");

    // The launch sent the admitted bytes, frozen in its own output directory
    // under a path given with a redundant `.` component, which is printed
    // exactly as recorded.
    let recorded = root.join("recorded-output");
    std::fs::create_dir_all(&recorded).unwrap_or_else(|_| unreachable!());
    let digest = blake3::hash(&admitted).to_hex().to_string();
    let path = format!("{}/./request-{digest}.json", recorded.display());
    write(Path::new(&path), &admitted);
    let launch = launch(root);
    let sent = FrozenRequest {
        path: path.clone(),
        blake3: ContentId::from_blake3_hex(digest.clone()).unwrap_or_else(|_| unreachable!()),
    };
    assert_eq!(
        owner.begin_dispatch_transport_sending(
            &attempt.identity,
            attempt.fence,
            1,
            &reserved,
            Some(&launch),
            Some(&sent)
        ),
        Ok(TransportStart::Fresh)
    );
    drop(owner);

    let output = report(root, &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap_or_else(|_| unreachable!());
    println!("{stdout}");
    let line = stdout
        .strip_suffix('\n')
        .unwrap_or_else(|| unreachable!("{stdout}"));
    assert!(!line.contains('\n'), "{stdout}");
    let value: serde_json::Value =
        serde_json::from_str(line).unwrap_or_else(|error| unreachable!("{error}: {line}"));
    assert_eq!(
        value,
        serde_json::json!({
            "schema": "kiln.lattice-sent-request.v1",
            "attempt": attempt.identity.as_str(),
            "fence": attempt.fence,
            "recorded": true,
            "request": path,
            "request_blake3": digest,
            "state_directory": launch.state_directory,
        })
    );

    // Every refusal leaves stdout empty.
    let mut unknown = admitted.clone();
    unknown.push(b' ');
    write(&inputs.join("attempt.json"), &unknown);
    refused(
        &report(root, &args),
        "not the request bound at bind-dispatch",
    );
    write(&inputs.join("attempt.json"), &admitted);
    std::fs::remove_file(&path).unwrap_or_else(|_| unreachable!());
    refused(&report(root, &args), "is unavailable");
    let mut absent = args.clone();
    if let Some(database) = absent.first_mut() {
        database.1 = root.join("absent.sqlite").display().to_string();
    }
    refused(&report(root, &absent), "No such file or directory");
    assert!(!root.join("absent.sqlite").exists());
}

// r[verify kiln.admission.lattice_lineage.scenario.reported]
#[test]
fn a_wal_mode_ledger_is_refused_without_creating_its_sidecars() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let Bound {
        mut owner,
        attempt,
        reserved,
        admitted,
        args,
    } = bound(root);
    let digest = blake3::hash(&admitted).to_hex().to_string();
    let path = root.join(format!("request-{digest}.json"));
    write(&path, &admitted);
    let sent = FrozenRequest {
        path: path.display().to_string(),
        blake3: ContentId::from_blake3_hex(digest).unwrap_or_else(|_| unreachable!()),
    };
    assert_eq!(
        owner.begin_dispatch_transport_sending(
            &attempt.identity,
            attempt.fence,
            1,
            &reserved,
            Some(&launch(root)),
            Some(&sent)
        ),
        Ok(TransportStart::Fresh)
    );
    drop(owner);
    // The rollback-journal ledger is reported.
    assert_eq!(report(root, &args).status.code(), Some(0));

    // The same ledger switched to WAL by a normal connection, then closed:
    // SQLite removes its sidecars when the last connection closes.
    let database = root.join("kiln.sqlite");
    {
        let connection = rusqlite::Connection::open(&database).unwrap_or_else(|_| unreachable!());
        let mode: String = connection
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(mode, "wal");
    }
    let sidecars = ["-wal", "-shm", "-journal"]
        .map(|suffix| PathBuf::from(format!("{}{suffix}", database.display())));
    assert!(sidecars.iter().all(|sidecar| !sidecar.exists()));
    // `report` checks that the whole tree, sidecars included, is unchanged.
    refused(&report(root, &args), "WAL mode");
    assert!(sidecars.iter().all(|sidecar| !sidecar.exists()));
}
