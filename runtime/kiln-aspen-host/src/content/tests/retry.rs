use kiln_adapter_aspen::AspenContentPort;
use kiln_core::types::ExternalRef;
use kiln_provider_api::PortFailure;

use super::{port, reference};
use crate::content::{INDEX_FILE, MoltenContentPort, fault::IndexFailure};

fn assert_exact(port: &MoltenContentPort, reference: &ExternalRef, bytes: &[u8]) {
    assert!(matches!(port.get_exact(reference, 1024), Ok(loaded) if loaded == bytes));
}

fn assert_publication_failed(port: &mut MoltenContentPort, reference: &ExternalRef, bytes: &[u8]) {
    assert!(matches!(
        port.put_exact(reference, bytes),
        Err(PortFailure::Infrastructure(failure)) if failure.code == "aspen_content_index_publish"
    ));
}

fn reopen(directory: &tempfile::TempDir) -> MoltenContentPort {
    let Ok(port) = MoltenContentPort::open(directory.path(), 64) else {
        unreachable!("content port must reopen");
    };
    port
}

// r[verify kiln.aspen_runtime.content_index_retry.scenario.fault]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.recovery]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.preserved]
#[test]
fn obstructed_index_stage_cannot_be_acknowledged_by_equal_retry() {
    let (directory, mut port) = port();
    let previous = b"previously published content";
    let previous_ref = reference(previous);
    assert!(port.put_exact(&previous_ref, previous).is_ok());
    let stage = directory.path().join(format!("{INDEX_FILE}.tmp"));
    assert!(std::fs::create_dir(&stage).is_ok());
    let bytes = b"pending content behind obstructed stage";
    let pending = reference(bytes);
    const RETRIES_WHILE_OBSTRUCTED: usize = 3;
    for _ in 0..RETRIES_WHILE_OBSTRUCTED {
        assert_publication_failed(&mut port, &pending, bytes);
        assert_exact(&port, &previous_ref, previous);
    }
    let observed = reopen(&directory);
    assert_exact(&observed, &previous_ref, previous);
    assert!(matches!(
        observed.get_exact(&pending, 1024),
        Err(PortFailure::Infrastructure(failure)) if failure.code == "aspen_content_missing"
    ));
    drop(observed);
    assert!(stage.is_dir());
    assert!(std::fs::remove_dir(&stage).is_ok());
    assert!(port.put_exact(&pending, bytes).is_ok());
    drop(port);
    let observed = reopen(&directory);
    assert_exact(&observed, &pending, bytes);
    assert_exact(&observed, &previous_ref, previous);
}

fn failed_index_publication_recovers(failure: IndexFailure, populated: bool) {
    let (directory, mut port) = port();
    let previous = b"previously published content";
    let previous_ref = reference(previous);
    if populated {
        assert!(port.put_exact(&previous_ref, previous).is_ok());
    }
    let bytes = b"exact content requiring index publication";
    let pending = reference(bytes);
    port.index_failure = Some(failure);
    const RETRIES_WHILE_FAILED: usize = 3;
    for _ in 0..RETRIES_WHILE_FAILED {
        assert_publication_failed(&mut port, &pending, bytes);
        if populated {
            assert_exact(&port, &previous_ref, previous);
        }
    }
    drop(port);
    let mut port = reopen(&directory);
    port.index_failure = Some(failure);
    assert_publication_failed(&mut port, &pending, bytes);
    if populated {
        assert_exact(&port, &previous_ref, previous);
    }
    port.index_failure = None;
    assert!(port.put_exact(&pending, bytes).is_ok());
    drop(port);
    let observed = reopen(&directory);
    assert_exact(&observed, &pending, bytes);
    if populated {
        assert_exact(&observed, &previous_ref, previous);
    }
    println!("content-index failure={failure:?} populated={populated} retries=4 recovered=true");
}

// r[verify kiln.aspen_runtime.content_index_retry]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.fault]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.recovery]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.preserved]
// r[verify kiln.aspen_runtime.content_index_retry.scenario.barrier]
#[test]
fn publication_failures_stay_pending_until_the_barrier_recovers() {
    for failure in [
        IndexFailure::Create,
        IndexFailure::Write,
        IndexFailure::PayloadSync,
        IndexFailure::Rename,
        IndexFailure::ParentSync,
    ] {
        failed_index_publication_recovers(failure, false);
        failed_index_publication_recovers(failure, true);
    }
}
