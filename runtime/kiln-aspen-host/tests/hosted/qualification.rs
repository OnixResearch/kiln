use super::process::*;
use super::*;

const LOAD_OPERATION_COUNT: usize = 4;
const REQUESTS_PER_OPERATION: usize = 3;
const RESTART_REQUESTS_PER_OPERATION: usize = 2;
const CRASH_REQUEST_ALLOWANCE: usize = 8;
const INCOMPATIBLE_CALLBACK_TIMEOUT_MILLISECONDS: u64 = 30_001;
const TRUNCATED_DATABASE_BYTES: u64 = 8;

// These drills qualify actual child-host/Aspen/extension persistence only. The
// closed Lattice terminal fixture does not establish real workflow/provider truth.
// r[verify kiln.aspen_host.production_evidence]
#[test]
fn child_host_crash_replays_exact_acknowledgements_without_dispatch() {
    let fixture = ProcessFixture::new();
    let lattice = FixtureLattice::start(&fixture, false);
    let ingress = fixture.ingress(0);
    let child = fixture.start(CRASH_REQUEST_ALLOWANCE);
    let ack = completed(fixture.client().submit(&fixture.profile, &ingress));
    child.crash();
    fixture.remove_crashed_socket();

    let recovered = fixture.start(RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
    assert_eq!(
        completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
        ack
    );
    recovered.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 1,
            recover: 0,
            observe: 1
        }
    );
}

#[test]
fn child_host_crash_recovers_unknown_and_duplicate_without_redispatch() {
    let fixture = ProcessFixture::new();
    // The adapter probes RecoverDispatch during submit; keep it unavailable
    // until the crashed host has lost its process state.
    let lattice = FixtureLattice::unavailable_recovery(&fixture);
    let ingress = fixture.ingress(0);
    let child = fixture.start(CRASH_REQUEST_ALLOWANCE);
    let mut client = fixture.client();
    unknown(
        client.submit(&fixture.profile, &ingress),
        &ingress.operation_id,
    );
    assert_eq!(
        lattice.counts(),
        ExchangeCounts {
            dispatch: 1,
            recover: 1,
            observe: 0
        }
    );
    child.crash();
    fixture.remove_crashed_socket();
    lattice.restore_recovery();

    let recovered = fixture.start(RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    let ack = completed(client.submit(&fixture.profile, &ingress));
    assert_eq!(ack.operation_id, ingress.operation_id);
    assert_eq!(
        completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
        ack
    );
    recovered.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 1,
            recover: 2,
            observe: 1
        }
    );
}

#[test]
fn child_host_restart_retains_unknown_on_failed_recovery_without_redispatch() {
    const REQUESTS_AFTER_RESTART: usize = 3;
    // Initial adapter recovery, duplicate, startup recovery, and three requests.
    const EXPECTED_RECOVERY_ATTEMPTS: usize = 6;
    let fixture = ProcessFixture::new();
    let lattice = FixtureLattice::unavailable_recovery(&fixture);
    let ingress = fixture.ingress(0);
    let child = fixture.start(CRASH_REQUEST_ALLOWANCE);
    let mut client = fixture.client();
    unknown(
        client.submit(&fixture.profile, &ingress),
        &ingress.operation_id,
    );
    unknown(
        client.submit(&fixture.profile, &ingress),
        &ingress.operation_id,
    );
    assert_eq!(
        lattice.counts(),
        ExchangeCounts {
            dispatch: 1,
            recover: 2,
            observe: 0
        }
    );
    child.crash();
    fixture.remove_crashed_socket();

    let recovered = fixture.start(REQUESTS_AFTER_RESTART);
    let mut client = fixture.client();
    unknown(
        client.submit(&fixture.profile, &ingress),
        &ingress.operation_id,
    );
    unknown(
        client.submit(&fixture.profile, &ingress),
        &ingress.operation_id,
    );
    unknown(
        client.reconcile(&fixture.profile, &ingress.operation_id),
        &ingress.operation_id,
    );
    recovered.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 1,
            recover: EXPECTED_RECOVERY_ATTEMPTS,
            observe: 0,
        }
    );
}

// r[verify kiln.aspen_host.production_evidence]
#[test]
fn child_host_bounded_load_preserves_every_ack_and_dispatch_count_across_restart() {
    let fixture = ProcessFixture::new();
    let lattice = FixtureLattice::start(&fixture, false);
    let child = fixture.start(LOAD_OPERATION_COUNT * REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    let mut completed_operations = Vec::new();
    for index in 0..LOAD_OPERATION_COUNT {
        let ingress = fixture.ingress(index);
        let ack = completed(client.submit(&fixture.profile, &ingress));
        assert_eq!(ack.operation_id, ingress.operation_id);
        assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
        assert_eq!(
            completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
            ack
        );
        completed_operations.push((ingress, ack));
    }
    child.finish();
    let recovered = fixture.start(LOAD_OPERATION_COUNT * RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    for (ingress, ack) in completed_operations {
        assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
        assert_eq!(
            completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
            ack
        );
    }
    recovered.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: LOAD_OPERATION_COUNT,
            recover: 0,
            observe: LOAD_OPERATION_COUNT,
        }
    );
}

#[derive(Clone, Copy, Debug)]
enum DamagedState {
    MalformedMarker,
    TruncatedLedger,
    CorruptNativeJournal,
    CorruptCallbackValue,
    MissingCallbackDirectory,
}

impl DamagedState {
    fn apply(self, fixture: &ProcessFixture) -> &'static str {
        let root = &fixture.paths.workspace;
        match self {
            Self::MalformedMarker => {
                std::fs::write(root.join("host-state.json"), b"{partial")
                    .unwrap_or_else(|_| unreachable!("malformed marker"));
                "host state marker JSON is invalid"
            }
            Self::TruncatedLedger => {
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(root.join("operation-ledger/operations.sqlite"))
                    .unwrap_or_else(|_| unreachable!("ledger to truncate"));
                file.set_len(TRUNCATED_DATABASE_BYTES)
                    .unwrap_or_else(|_| unreachable!("truncated ledger"));
                "ledger"
            }
            Self::CorruptNativeJournal => {
                std::fs::write(
                    root.join("native-journal/fabric-durability.redb"),
                    b"corrupt journal",
                )
                .unwrap_or_else(|_| unreachable!("corrupt journal"));
                "native host journal"
            }
            Self::CorruptCallbackValue => {
                let snapshot = state_snapshot(root);
                let leaf = snapshot
                    .keys()
                    .find(|path| path.starts_with("callback-values"))
                    .unwrap_or_else(|| unreachable!("persisted callback value"));
                std::fs::write(root.join(leaf), b"corrupt callback value")
                    .unwrap_or_else(|_| unreachable!("corrupt callback value"));
                "callback value store"
            }
            Self::MissingCallbackDirectory => {
                std::fs::remove_dir_all(root.join("callback-values"))
                    .unwrap_or_else(|_| unreachable!("partial durable root"));
                "callback-values"
            }
        }
    }
}

// r[verify kiln.aspen_host.production_evidence]
#[test]
fn child_host_startup_rejects_malformed_corrupt_and_partial_durable_state() {
    for damage in [
        DamagedState::MalformedMarker,
        DamagedState::TruncatedLedger,
        DamagedState::CorruptNativeJournal,
        DamagedState::CorruptCallbackValue,
        DamagedState::MissingCallbackDirectory,
    ] {
        let fixture = ProcessFixture::new();
        let lattice = FixtureLattice::start(&fixture, false);
        let child = fixture.start(1);
        completed(
            fixture
                .client()
                .submit(&fixture.profile, &fixture.ingress(0)),
        );
        child.finish();
        let expected = damage.apply(&fixture);
        println!("qualification damage={damage:?}");
        fixture.spawn(1).reject(expected);
        assert!(
            !fixture.host_socket.exists(),
            "damaged state must fail before ingress"
        );
        assert_eq!(
            lattice.finish(),
            ExchangeCounts {
                dispatch: 1,
                recover: 0,
                observe: 1
            }
        );
    }
}

// r[verify kiln.aspen_host.production_evidence]
#[test]
fn child_host_complete_quiescent_backup_restores_and_partial_restore_is_rejected() {
    let fixture = ProcessFixture::new();
    let lattice = FixtureLattice::start(&fixture, false);
    let ingress = fixture.ingress(0);
    let child = fixture.start(1);
    let ack = completed(fixture.client().submit(&fixture.profile, &ingress));
    child.finish();

    // Capture only after the real host has exited/checkpointed and released both
    // stores. Every file of all four components is copied, including sidecars.
    let prior = state_snapshot(&fixture.paths.workspace);
    let backup = tempfile::tempdir().unwrap_or_else(|_| unreachable!("quiescent backup root"));
    restore_state(backup.path(), &prior);
    let backup_snapshot = state_snapshot(backup.path());
    assert_eq!(backup_snapshot, prior);
    restore_state(&fixture.paths.workspace, &backup_snapshot);
    let restored = fixture.start(RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
    assert_eq!(
        completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
        ack
    );
    restored.finish();

    // A backup without its journal must not be accepted as an empty fresh host.
    restore_state(&fixture.paths.workspace, &backup_snapshot);
    std::fs::remove_file(
        fixture
            .paths
            .workspace
            .join("native-journal/fabric-durability.redb"),
    )
    .unwrap_or_else(|_| unreachable!("partial restore journal omission"));
    fixture.spawn(1).reject("native host journal");
    assert!(!fixture.host_socket.exists());
    // Repair from the same quiescent backup, not from a newly initialized store.
    restore_state(&fixture.paths.workspace, &backup_snapshot);
    let repaired = fixture.start(1);
    assert_eq!(
        completed(fixture.client().submit(&fixture.profile, &ingress)),
        ack
    );
    repaired.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 1,
            recover: 0,
            observe: 1
        }
    );
}

// r[verify kiln.aspen_host.production_evidence]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the rollback drill keeps both incompatible cohorts and exact prior restoration visible"
)]
fn child_host_explicit_rollback_rejects_mixed_cohort_then_restores_prior_exact_cohort() {
    let mut fixture = ProcessFixture::new();
    let lattice = FixtureLattice::start(&fixture, false);
    let ingress = fixture.ingress(0);
    let prior_host = fixture.host_executable.clone();
    let prior_host_identity = blake3::hash(
        &std::fs::read(&prior_host).unwrap_or_else(|_| unreachable!("prior host executable bytes")),
    );
    let prior_extension = fixture.paths.extension_executable.clone();
    let prior_extension_bytes =
        std::fs::read(&prior_extension).unwrap_or_else(|_| unreachable!("prior executable cohort"));
    let prior_profile_bytes = std::fs::read(&fixture.paths.runtime_profile)
        .unwrap_or_else(|_| unreachable!("prior runtime profile"));
    let child = fixture.start(1);
    let ack = completed(fixture.client().submit(&fixture.profile, &ingress));
    child.finish();
    let prior_state = state_snapshot(&fixture.paths.workspace);

    // This is an admitted but incompatible profile, not malformed JSON. State
    // identity must reject it before native recovery or provider effects.
    let mut incompatible = profile_document_fixture();
    incompatible.bounds.callback_timeout_millis = INCOMPATIBLE_CALLBACK_TIMEOUT_MILLISECONDS;
    let admitted = incompatible
        .clone()
        .into_runtime_profile()
        .and_then(|profile| admit_profile(&profile))
        .unwrap_or_else(|_| unreachable!("incompatible profile remains individually admissible"));
    assert_ne!(admitted.profile_identity, fixture.profile.profile_identity);
    write_json(&fixture.paths.runtime_profile, &incompatible);
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), prior_state);
    assert!(!fixture.host_socket.exists());

    // Independently reject executable mix-and-match against the old profile and
    // old state; altered bytes must not silently inherit the old cohort identity.
    std::fs::write(&fixture.paths.runtime_profile, &prior_profile_bytes)
        .unwrap_or_else(|_| unreachable!("restore exact prior profile"));
    let candidate_extension = fixture.directory.path().join("candidate-extension");
    std::fs::copy(&prior_extension, &candidate_extension)
        .unwrap_or_else(|_| unreachable!("candidate executable copy"));
    let mut candidate_bytes = prior_extension_bytes.clone();
    candidate_bytes.extend_from_slice(b"qualification-incompatible-cohort");
    std::fs::write(&candidate_extension, candidate_bytes)
        .unwrap_or_else(|_| unreachable!("candidate executable bytes"));
    fixture.paths.extension_executable = candidate_extension;
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), prior_state);
    assert!(!fixture.host_socket.exists());

    // Run the separately admitted candidate with its own fresh state before
    // rolling back. Reject an old-profile downgrade against that candidate state.
    let prior_workspace = fixture.paths.workspace.clone();
    let prior_profile = fixture.profile.clone();
    let candidate_state =
        tempfile::tempdir().unwrap_or_else(|_| unreachable!("candidate state root"));
    fixture.paths.workspace = candidate_state.path().to_path_buf();
    fixture.profile = admitted;
    write_json(&fixture.paths.runtime_profile, &incompatible);
    let candidate_ingress = fixture.ingress(1);
    let candidate_host = fixture.start(1);
    let candidate_ack = completed(
        fixture
            .client()
            .submit(&fixture.profile, &candidate_ingress),
    );
    assert_ne!(candidate_ack.operation_id, ack.operation_id);
    candidate_host.finish();
    let candidate_snapshot = state_snapshot(&fixture.paths.workspace);
    fixture.paths.extension_executable = prior_extension.clone();
    fixture.profile = prior_profile;
    std::fs::write(&fixture.paths.runtime_profile, &prior_profile_bytes)
        .unwrap_or_else(|_| unreachable!("select exact prior profile"));
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), candidate_snapshot);
    assert!(!fixture.host_socket.exists());
    fixture.paths.workspace = prior_workspace;

    // Explicit rollback selects all prior inputs, including the quiescent state
    // cohort; recovery then must replay the original full acknowledgement.
    fixture.host_executable = prior_host;
    assert_eq!(
        blake3::hash(
            &std::fs::read(&fixture.host_executable)
                .unwrap_or_else(|_| unreachable!("restored exact host bytes"))
        ),
        prior_host_identity
    );
    fixture.paths.extension_executable = prior_extension;
    restore_state(&fixture.paths.workspace, &prior_state);
    assert_eq!(
        std::fs::read(&fixture.paths.extension_executable)
            .unwrap_or_else(|_| unreachable!("restored executable bytes")),
        prior_extension_bytes
    );
    assert_eq!(
        std::fs::read(&fixture.paths.runtime_profile)
            .unwrap_or_else(|_| unreachable!("restored profile bytes")),
        prior_profile_bytes
    );
    let rolled_back = fixture.start(RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
    assert_eq!(
        completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
        ack
    );
    rolled_back.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 2,
            recover: 0,
            observe: 2
        }
    );
}
