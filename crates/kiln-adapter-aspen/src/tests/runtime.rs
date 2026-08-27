use kiln_core::types::ContentId;
use kiln_provider_api::{
    EventStorePort, PortFailure, RemoteLifecyclePort, RemoteTransportObservation,
    WorkflowExchangeOperation, WorkflowExchangePort, WorkflowExchangeRequest,
    WorkflowExchangeResponse,
};

use super::super::*;
use super::support::{
    OVERSIZED_LOG_BYTES, TEST_MAXIMUM_BYTES, TEST_MAXIMUM_RECORDS, admitted, event_request,
    reference,
};

// r[verify kiln.aspen_runtime.durability]
#[test]
fn journal_and_outbox_commit_atomically_and_conflicts_change_nothing() {
    let runtime = InMemoryAspenDurability::default();
    let mut store = AspenEventStore::new(profile_fixture(), runtime.clone())
        .unwrap_or_else(|_| unreachable!("store"));
    let request = event_request();
    assert_eq!(store.append(&request), Ok(1));
    let snapshot = store
        .load(&request.stream, TEST_MAXIMUM_RECORDS)
        .unwrap_or_else(|_| unreachable!("load"));
    assert_eq!(snapshot.records, request.records);
    assert_eq!(snapshot.outbox, request.outbox);

    let mut conflicting_request = request.clone();
    let first_record = conflicting_request
        .records
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture record"));
    first_record.event_identity = reference("kiln", "event-blake3", "different-event");
    let conflict = store.append(&conflicting_request);
    assert!(matches!(conflict, Err(PortFailure::Domain(_))));
    let after = store
        .load(&request.stream, TEST_MAXIMUM_RECORDS)
        .unwrap_or_else(|_| unreachable!("load"));
    assert_eq!(after, snapshot);
}

// r[verify kiln.aspen_runtime.durability] r[verify kiln.aspen_runtime.failure]
#[test]
fn lost_commit_acknowledgement_reconciles_without_duplicate_mutation() {
    let runtime = InMemoryAspenDurability::default();
    runtime
        .set_next_fault(DurabilityFault::UnknownAfterCommit)
        .unwrap_or_else(|_| unreachable!("fault"));
    let mut store = AspenEventStore::new(profile_fixture(), runtime.clone())
        .unwrap_or_else(|_| unreachable!("store"));
    let request = event_request();
    let operation = plan_atomic_append(&admitted(), &request)
        .unwrap_or_else(|_| unreachable!("plan"))
        .operation_id;
    assert!(matches!(
        store.append(&request),
        Err(PortFailure::Unknown(_))
    ));
    let receipt = store
        .reconcile(&operation)
        .unwrap_or_else(|_| unreachable!("reconcile"))
        .unwrap_or_else(|| unreachable!("receipt exists"));
    assert_eq!(receipt.committed_revision, 1);

    let restarted = AspenEventStore::new(profile_fixture(), runtime)
        .unwrap_or_else(|_| unreachable!("restart"));
    let snapshot = restarted
        .load(&request.stream, TEST_MAXIMUM_RECORDS)
        .unwrap_or_else(|_| unreachable!("load"));
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.outbox.len(), 1);
}

// r[verify kiln.aspen_runtime.failure]
#[test]
fn timeout_and_cancellation_do_not_commit() {
    for (fault, expected_timeout) in [
        (DurabilityFault::TimeoutBeforeCommit, true),
        (DurabilityFault::CancelBeforeCommit, false),
    ] {
        let runtime = InMemoryAspenDurability::default();
        runtime
            .set_next_fault(fault)
            .unwrap_or_else(|_| unreachable!("fault"));
        let mut store = AspenEventStore::new(profile_fixture(), runtime.clone())
            .unwrap_or_else(|_| unreachable!("store"));
        let request = event_request();
        let result = store.append(&request);
        if expected_timeout {
            assert!(matches!(result, Err(PortFailure::TimedOut(_))));
        } else {
            assert!(matches!(result, Err(PortFailure::Cancelled(_))));
        }
        assert_eq!(runtime.revision(&request.stream), Ok(0));
    }
}

// r[verify kiln.aspen_runtime.content]
#[test]
fn content_mapping_rejects_missing_corrupt_and_overbound_objects() {
    let port = InMemoryAspenContent::default();
    let mut adapter = AspenContentAdapter::new(admitted(), port.clone());
    let mapped = adapter
        .store(KilnArtifactRole::Output, "kiln.output.v1", b"output", None)
        .unwrap_or_else(|_| unreachable!("store"));
    assert_eq!(
        adapter.load(&mapped, TEST_MAXIMUM_BYTES),
        Ok(b"output".to_vec())
    );
    port.replace_unchecked(&mapped.reference, b"corrupt".to_vec())
        .unwrap_or_else(|_| unreachable!("replace"));
    assert!(adapter.load(&mapped, TEST_MAXIMUM_BYTES).is_err());
    port.remove(&mapped.reference)
        .unwrap_or_else(|_| unreachable!("remove"));
    assert!(adapter.load(&mapped, TEST_MAXIMUM_BYTES).is_err());

    let oversized_log = vec![0; OVERSIZED_LOG_BYTES];
    assert!(
        adapter
            .store(KilnArtifactRole::Log, "kiln.log.v1", &oversized_log, None,)
            .is_err()
    );
}

#[derive(Clone, Debug)]
struct WorkflowFixture {
    outcome: AspenWorkflowOutcome,
}

impl AspenWorkflowTransportPort for WorkflowFixture {
    fn exchange(
        &mut self,
        _profile: &AdmittedAspenRuntimeProfile,
        _request: &AspenWorkflowExchangeRequest,
    ) -> AspenWorkflowOutcome {
        self.outcome.clone()
    }
}

// r[verify kiln.aspen_runtime.effects] r[verify kiln.aspen_runtime.failure]
#[test]
fn workflow_exchange_preserves_response_and_unknown_acceptance() {
    let body = b"workflow".to_vec();
    let body_identity = ContentId::from_blake3_hex(blake3::hash(&body).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!("identity"));
    let request = WorkflowExchangeRequest {
        operation: WorkflowExchangeOperation::Dispatch,
        body_identity,
        body,
        max_response_bytes: TEST_MAXIMUM_BYTES,
    };
    let response = WorkflowExchangeResponse {
        body: b"accepted".to_vec(),
        log_bytes: Vec::new(),
        disconnected_after_acceptance: false,
    };
    let mut adapter = AspenWorkflowExchange::new(
        admitted(),
        WorkflowFixture {
            outcome: AspenWorkflowOutcome::Response(response.clone()),
        },
    );
    assert_eq!(adapter.exchange(request.clone()), Ok(response));

    let operation = reference("molten", "operation", "workflow-1");
    let mut unknown = AspenWorkflowExchange::new(
        admitted(),
        WorkflowFixture {
            outcome: AspenWorkflowOutcome::DisconnectedAfterAcceptance {
                operation_id: operation,
            },
        },
    );
    assert!(matches!(
        unknown.exchange(request),
        Err(PortFailure::Unknown(_))
    ));
}

#[derive(Clone, Debug)]
struct RemoteFixture;

impl AspenRemoteTransportPort for RemoteFixture {
    fn dispatch(
        &mut self,
        _profile: &AdmittedAspenRuntimeProfile,
        _request: &AspenRemoteRequest,
    ) -> AspenRemoteOutcome {
        AspenRemoteOutcome::DisconnectedAfterAcceptance {
            operation_id: Some(reference("molten", "operation", "remote-accepted")),
        }
    }
}

// r[verify kiln.aspen_runtime.failure]
#[test]
fn remote_disconnect_after_acceptance_remains_unknown() {
    let operation = reference("molten", "operation", "remote-1");
    let mut adapter = AspenRemoteAdapter::new(admitted(), RemoteFixture);
    let observation = adapter
        .poll(&operation)
        .unwrap_or_else(|_| unreachable!("typed unknown observation"));
    assert!(matches!(
        observation,
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. }
    ));
}

const UNIX_TEST_TIMEOUT_SECONDS: u64 = 5;

// r[verify kiln.aspen_runtime.ingress] r[verify kiln.aspen_runtime.composition]
#[test]
fn unix_aspen_transport_accepts_exact_response_and_denies_missing_endpoint() {
    use std::io::Read;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;

    let profile = admitted();
    let ingress = build_ingress(&profile, IngressSource::Radicle, trigger_request())
        .unwrap_or_else(|_| unreachable!("ingress"));
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary socket root"));
    let socket_path = directory.path().join("aspen.sock");
    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|_| unreachable!("bind socket"));
    let profile_identity = profile.profile_identity.clone();
    let generation = profile.profile.generation;
    let operation_id = ingress.operation_id.clone();
    let trigger_identity = ingress.trigger_identity.clone();
    let run_id = kiln_core::types::RunId::new("kiln-aspen-unix-run")
        .unwrap_or_else(|_| unreachable!("run id"));
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
        let mut request = Vec::new();
        stream
            .read_to_end(&mut request)
            .unwrap_or_else(|_| unreachable!("read"));
        assert!(!request.is_empty());
        let ack = AspenServiceAck {
            schema: SERVICE_ACK_SCHEMA.to_string(),
            operation_id: operation_id.clone(),
            generation,
            trigger_identity,
            run_id: Some(run_id),
            decision: AspenServiceDecision::Accepted,
            receipt_ref: reference("molten", "service-receipt", "unix-accepted"),
        };
        let response = encode_exchange_response(
            profile_identity,
            operation_id,
            ServiceTransportOutcome::Acknowledged(ack),
        );
        stream
            .write_all(&response)
            .unwrap_or_else(|_| unreachable!("write"));
    });
    let mut transport = UnixAspenServiceTransport::new(
        socket_path,
        profile.profile.bounds.max_callback_input_bytes,
        Duration::from_secs(UNIX_TEST_TIMEOUT_SECONDS),
    )
    .unwrap_or_else(|_| unreachable!("transport"));
    assert!(matches!(
        transport.submit(&profile, &ingress),
        ServiceTransportOutcome::Acknowledged(_)
    ));
    server.join().unwrap_or_else(|_| unreachable!("server"));

    let missing = directory.path().join("missing.sock");
    let mut unavailable = UnixAspenServiceTransport::new(
        missing,
        profile.profile.bounds.max_callback_input_bytes,
        Duration::from_secs(UNIX_TEST_TIMEOUT_SECONDS),
    )
    .unwrap_or_else(|_| unreachable!("missing transport"));
    assert_eq!(
        unavailable.submit(&profile, &ingress),
        ServiceTransportOutcome::Unavailable,
    );
}

fn trigger_request() -> kiln_provider_api::TriggerRequest {
    kiln_provider_api::TriggerRequest {
        repository: reference("radicle", "rid", "rad:aspen-unix"),
        actor: reference("radicle", "did", "did:key:aspen-unix"),
        before: Some(reference(
            "radicle",
            "oid",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )),
        after: reference("radicle", "oid", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        branch: None,
        patch: None,
        event: kiln_provider_api::TriggerEventKind::Push,
    }
}
