use super::*;

const TRUNCATED_DATABASE_BYTES: u64 = 8;

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
        let fixture = ProcessFixture::new(std::time::Instant::now);
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
    let fixture = ProcessFixture::new(std::time::Instant::now);
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
