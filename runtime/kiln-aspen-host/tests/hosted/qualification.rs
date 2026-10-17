use super::process::*;
use super::*;

#[path = "qualification/durable.rs"]
mod durable_test;
#[path = "qualification/rollback.rs"]
mod rollback_test;

const LOAD_OPERATION_COUNT: usize = 4;
const REQUESTS_PER_OPERATION: usize = 3;
const RESTART_REQUESTS_PER_OPERATION: usize = 2;
const CRASH_REQUEST_ALLOWANCE: usize = 8;

// These drills qualify actual child-host/Aspen/extension persistence only. The
// closed Lattice terminal fixture does not establish real workflow/provider truth.
// r[verify kiln.aspen_host.production_evidence]
#[test]
fn child_host_crash_replays_exact_acknowledgements_without_dispatch() {
    let fixture = ProcessFixture::new(std::time::Instant::now);
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
    let fixture = ProcessFixture::new(std::time::Instant::now);
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
    let fixture = ProcessFixture::new(std::time::Instant::now);
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
    let fixture = ProcessFixture::new(std::time::Instant::now);
    let lattice = FixtureLattice::start(&fixture, false);
    let child = fixture.start(LOAD_OPERATION_COUNT * REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    let mut completed_operations = Vec::with_capacity(LOAD_OPERATION_COUNT);
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
