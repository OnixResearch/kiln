//! The request file and state root a launch ran: the shared resolver, and
//! the read-only `--sent-request true` report for out-of-process callers.

use super::preparation_tests::{
    content, enrollment, fresh_observations, frozen, profile, ready_with,
};
use super::{
    LaunchRequest, MAX_REQUEST_BYTES, RequestSource, SENT_REQUEST_SCHEMA, SentRequestReport,
    WorkerTransportProfile, first_launch, launch_request, prepare_launch_files, sent_request,
    sent_request_and_state,
};
use kiln_core::fair_admission::ClaimRef;
use kiln_core::implementation::{
    FrozenRequest, ImplementationAttempt, TransportLaunch, TransportStart,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn export_names_the_request_and_root_the_recorded_launch_ran() {
    let admitted = b"{\"schema\":\"x\"}";
    let digest = blake3::hash(admitted).to_hex().to_string();
    let transport = WorkerTransportProfile {
        schema: "kiln.lattice-implementation-cli.v2".into(),
        lattice_program: "/nix/store/0-lattice/bin/lattice".into(),
        executable_blake3: content('b'),
        state_directory: "/current/state".into(),
        output_directory: "/current/output".into(),
    };
    let launch = TransportLaunch {
        lattice_program: "/nix/store/0-lattice/bin/lattice".into(),
        executable_blake3: content('b'),
        state_directory: "/recorded/state".into(),
    };
    let sent = FrozenRequest {
        path: "/recorded/output/request-sent.json".into(),
        blake3: content('c'),
    };
    assert_eq!(
        sent_request_and_state(Some(&launch), Some(&sent), &transport, admitted),
        Ok((
            PathBuf::from("/recorded/output/request-sent.json"),
            PathBuf::from("/recorded/state")
        ))
    );
    // A launch recorded before its request was: it sent the admitted bytes.
    let legacy = PathBuf::from(format!("/current/output/request-{digest}.json"));
    assert_eq!(
        sent_request_and_state(Some(&launch), None, &transport, admitted),
        Ok((legacy.clone(), PathBuf::from("/recorded/state")))
    );
    assert_eq!(
        sent_request_and_state(None, None, &transport, admitted),
        Ok((legacy, PathBuf::from("/current/state")))
    );
}

/// Every file and directory under `root` with its bytes, so a comparison
/// shows that nothing was created, removed or changed.
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap_or_else(|_| unreachable!()) {
            let path = entry.unwrap_or_else(|_| unreachable!()).path();
            let metadata = std::fs::symlink_metadata(&path).unwrap_or_else(|_| unreachable!());
            if metadata.is_dir() {
                pending.push(path.clone());
                entries.insert(path, None);
            } else if metadata.is_file() {
                let bytes = std::fs::read(&path).unwrap_or_else(|_| unreachable!());
                entries.insert(path, Some(bytes));
            } else {
                entries.insert(path, Some(Vec::new()));
            }
        }
    }
    entries
}

/// The admitted request the worker route seals for `attempt`.
fn admitted_for(attempt: &ImplementationAttempt) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "lattice.implementation-attempt-run.v3",
        "envelope": {"attempt_id": attempt.identity.as_str(), "fence": attempt.fence},
        "network": false,
    }))
    .unwrap_or_else(|_| unreachable!())
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!());
}

/// The query inputs in `inputs`: the admitted request, the enrollment and
/// a transport profile whose output and state directories moved since the
/// launch. Returns the flags of the `--sent-request true` mode.
fn query(root: &Path, inputs: &Path, admitted: &[u8]) -> BTreeMap<String, String> {
    std::fs::create_dir_all(inputs).unwrap_or_else(|_| unreachable!());
    write(&inputs.join("attempt.json"), admitted);
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
            "state_directory": root.join("moved-state"),
            "output_directory": root.join("moved"),
        }))
        .unwrap_or_else(|_| unreachable!()),
    );
    BTreeMap::from([
        (
            "--database".to_string(),
            root.join("kiln.sqlite").display().to_string(),
        ),
        (
            "--enrollment".to_string(),
            inputs.join("enrollment.json").display().to_string(),
        ),
        (
            "--worker-request".to_string(),
            inputs.join("attempt.json").display().to_string(),
        ),
        (
            "--worker-transport-profile".to_string(),
            inputs.join("transport.json").display().to_string(),
        ),
        ("--sent-request".to_string(), "true".to_string()),
    ])
}

/// The one report object `output` holds, with exactly the schema's members.
fn report(output: &str) -> SentRequestReport {
    let value: serde_json::Value =
        serde_json::from_str(output).unwrap_or_else(|error| unreachable!("{error}: {output}"));
    assert_eq!(
        value.as_object().map(serde_json::Map::len),
        Some(7),
        "{output}"
    );
    serde_json::from_value(value).unwrap_or_else(|error| unreachable!("{error}: {output}"))
}

/// Run the mode and check that it changed nothing under `root`.
fn read_only(root: &Path, flags: &BTreeMap<String, String>) -> Result<String, String> {
    let before = tree(root);
    let result = sent_request(flags);
    assert_eq!(
        tree(root),
        before,
        "the report wrote under {}",
        root.display()
    );
    result
}

fn refused(result: &Result<String, String>, reason: &str) {
    assert!(
        result.as_ref().is_err_and(|error| error.contains(reason)),
        "expected a refusal naming {reason:?}: {result:?}"
    );
}

/// One admitted attempt whose first launch sent, and recorded, the admitted
/// bytes plus Kiln's budget lineage.
struct Dispatched {
    attempt: ImplementationAttempt,
    admitted: Vec<u8>,
    request: LaunchRequest,
    launch: TransportLaunch,
}

fn dispatched(root: &Path) -> Dispatched {
    let (mut owner, attempt, plan, now, admitted) = ready_with(root, admitted_for);
    let transport = profile(root, "transport", &root.join("state"));
    let claim = ClaimRef {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    let ledger = owner
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let request = launch_request(
        RequestSource::Fresh {
            ledger: &ledger,
            work: &attempt.work_identity,
            claim: &claim,
            now_ms: now,
        },
        &transport,
        &admitted,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_ne!(
        request.bytes, admitted,
        "the sent request carries a lineage"
    );
    prepare_launch_files(&transport, &request, true)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let fresh = fresh_observations(&mut owner);
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let launch = first_launch(&transport).unwrap_or_else(|error| unreachable!("{error}"));
    let sent = request
        .frozen()
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        owner.begin_dispatch_transport_sending(
            &attempt.identity,
            attempt.fence,
            now,
            &plan,
            Some(&launch),
            Some(&sent)
        ),
        Ok(TransportStart::Fresh)
    );
    Dispatched {
        attempt,
        admitted,
        request,
        launch,
    }
}

/// Make the recorded file writable again so a test can replace it.
fn writable(path: &Path) {
    let mut permissions = std::fs::metadata(path)
        .unwrap_or_else(|_| unreachable!())
        .permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.admission.lattice_lineage.scenario.reported]
#[test]
fn the_report_names_the_recorded_lineage_request_and_root_without_writing() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let dispatched = dispatched(root);
    let Dispatched {
        attempt,
        admitted,
        request,
        launch,
    } = &dispatched;
    let flags = query(root, &root.join("inputs"), admitted);

    // The recorded file and the recorded launch's root, not the current
    // profile's directories nor the admitted request's digest.
    let output = read_only(root, &flags).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(!output.contains('\n'));
    let reported = report(&output);
    assert_eq!(
        reported,
        SentRequestReport {
            schema: SENT_REQUEST_SCHEMA.into(),
            attempt: attempt.identity.as_str().into(),
            fence: attempt.fence,
            recorded: true,
            request: request.path.display().to_string(),
            request_blake3: request.blake3.as_str().into(),
            state_directory: launch.state_directory.clone(),
        }
    );
    assert_ne!(
        reported.request_blake3,
        blake3::hash(admitted).to_hex().to_string()
    );
    assert_eq!(
        std::fs::read(&reported.request).ok(),
        Some(request.bytes.clone())
    );

    // The admitted bytes must be the request sealed at bind-dispatch, for an
    // attempt and fence the ledger holds.
    let envelope = |attempt_id: &str, fence: u64| {
        serde_json::to_vec(&serde_json::json!({
            "schema": "lattice.implementation-attempt-run.v3",
            "envelope": {"attempt_id": attempt_id, "fence": fence},
            "network": false,
        }))
        .unwrap_or_else(|_| unreachable!())
    };
    let inputs = root.join("inputs");
    let refusal = |bytes: &[u8], reason: &str| {
        write(&inputs.join("attempt.json"), bytes);
        refused(&read_only(root, &flags), reason);
    };
    refusal(
        &envelope(content('7').as_str(), attempt.fence),
        "is unknown to the ledger",
    );
    refusal(
        &envelope(attempt.identity.as_str(), attempt.fence + 1),
        "is unknown to the ledger",
    );
    let mut reformatted = admitted.clone();
    reformatted.push(b'\n');
    refusal(&reformatted, "not the request bound at bind-dispatch");
    refusal(b"{\"envelope\":{}}", "attempt_id");
    write(&inputs.join("attempt.json"), admitted);

    // Only `--sent-request true` selects the mode.
    let mut other = flags.clone();
    other.insert("--sent-request".into(), "1".into());
    refused(&read_only(root, &other), "accepts only true");

    // The recorded file must still be the bounded, ordinary sent bytes.
    writable(&request.path);
    write(&request.path, admitted);
    refused(&read_only(root, &flags), "(frozen_request_crossed)");
    let oversized = vec![b' '; usize::try_from(MAX_REQUEST_BYTES + 1).unwrap_or(usize::MAX)];
    write(&request.path, &oversized);
    refused(&read_only(root, &flags), "is unavailable");
    std::fs::remove_file(&request.path).unwrap_or_else(|_| unreachable!());
    #[cfg(unix)]
    {
        let elsewhere = root.join("elsewhere.json");
        write(&elsewhere, &request.bytes);
        std::os::unix::fs::symlink(&elsewhere, &request.path).unwrap_or_else(|_| unreachable!());
        refused(&read_only(root, &flags), "is unavailable");
        std::fs::remove_file(&request.path).unwrap_or_else(|_| unreachable!());
    }
    refused(&read_only(root, &flags), "is unavailable");
    assert!(!request.path.exists(), "a missing file is never recreated");

    // A ledger that does not exist is refused and not created.
    let mut absent = flags.clone();
    let missing = root.join("absent.sqlite");
    absent.insert("--database".into(), missing.display().to_string());
    refused(&read_only(root, &absent), "unable to open");
    assert!(!missing.exists());
}

// r[verify kiln.admission.lattice_lineage.scenario.reported]
#[test]
fn a_legacy_launch_reports_the_admitted_request_and_no_intent_is_refused() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let (mut owner, attempt, plan, now, admitted) = ready_with(root, admitted_for);
    let inputs = root.join("inputs");
    let flags = query(root, &inputs, &admitted);

    // Sealed but never dispatched: nothing was sent, so nothing is named.
    refused(&read_only(root, &flags), "no dispatch transport intent");

    // An earlier binary recorded its launch without the request it sent: the
    // admitted bytes, frozen under their digest in the profile's output
    // directory, which is the directory of the profile the caller names.
    let transport = profile(root, "moved", &root.join("state"));
    let legacy = frozen(&transport, &admitted);
    prepare_launch_files(&transport, &legacy, true).unwrap_or_else(|error| unreachable!("{error}"));
    let launch = first_launch(&transport).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        owner.begin_dispatch_transport(&attempt.identity, attempt.fence, now, &plan, Some(&launch)),
        Ok(TransportStart::Fresh)
    );
    assert_eq!(owner.dispatch_request(&attempt.identity), Ok(None));
    drop(owner);
    let output = read_only(root, &flags).unwrap_or_else(|error| unreachable!("{error}"));
    let digest = blake3::hash(&admitted).to_hex().to_string();
    assert_eq!(
        report(&output),
        SentRequestReport {
            schema: SENT_REQUEST_SCHEMA.into(),
            attempt: attempt.identity.as_str().into(),
            fence: attempt.fence,
            recorded: false,
            request: root
                .join("moved")
                .join(format!("request-{digest}.json"))
                .display()
                .to_string(),
            request_blake3: digest,
            state_directory: launch.state_directory.clone(),
        }
    );

    // The legacy file is verified like a recorded one.
    writable(&legacy.path);
    write(&legacy.path, b"{\"schema\":\"other\"}");
    refused(&read_only(root, &flags), "(frozen_request_crossed)");
    std::fs::remove_file(&legacy.path).unwrap_or_else(|_| unreachable!());
    refused(&read_only(root, &flags), "is unavailable");
}
