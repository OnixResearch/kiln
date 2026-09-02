// Test harness: shared checked fixtures keep positive and negative protocol cases exact.
#![allow(
    unknown_lints,
    clippy::expect_used,
    assertion_density,
    excessive_file_length,
    non_trait_imports,
    ambient_clock,
    reason = "closed local transport fixtures keep shared contract setup beside positive and negative protocol cases"
)]

use std::io::Cursor;

use lattice_workflow_contract::*;

use crate::*;

const TEST_MAXIMUM_NODES: u32 = 8;
const TEST_MAXIMUM_EDGES: u32 = 16;
const TEST_MAXIMUM_LOG_REFERENCES: u32 = 8;
const TEST_MAXIMUM_LOG_READ_BYTES: u64 = 4_096;
const TEST_MAXIMUM_TOTAL_LOG_BYTES: u64 = 16_384;
const TEST_FRAME_BYTES: u32 = 1_048_576;
const TEST_LOG_LIMIT_BYTES: u32 = 65_536;
const TEST_CONNECTIONS: u32 = 1;
const TEST_DEADLINE_MILLISECONDS: u64 = 5_000;
const TEST_SEQUENCE: u64 = 7;
const TEST_LOG_BYTES: &[u8] = b"log\n";
const TEST_IDENTITY_DIGIT_COUNT: usize = 64;
const TEST_PROFILE_USER_ID: u32 = 1_000;

fn identity(digit: char) -> Identity {
    let hex = digit.to_string().repeat(TEST_IDENTITY_DIGIT_COUNT);
    Identity::new(format!("{BLAKE3_PREFIX}{hex}")).expect("test identity is canonical")
}

fn label(value: &str) -> Label {
    Label::new(value.to_string()).expect("test label is canonical")
}

fn contract_profile() -> CapabilityProfile {
    CapabilityProfile {
        schema: PROFILE_SCHEMA.to_string(),
        graph_schema: GRAPH_SCHEMA.to_string(),
        platforms: vec![label("linux-x86_64")],
        node_kinds: vec![label("build")],
        effect_classes: vec![EffectClass::Build],
        adapter_kinds: vec![label("local")],
        controls: ControlCapabilities {
            cancel: true,
            pause: true,
            resume: true,
        },
        limits: ContractLimits {
            maximum_nodes: TEST_MAXIMUM_NODES,
            maximum_edges: TEST_MAXIMUM_EDGES,
            maximum_log_references: TEST_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: TEST_MAXIMUM_TOTAL_LOG_BYTES,
        },
        non_claims: lattice_workflow_contract::REQUIRED_NON_CLAIMS.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn workflow_request(profile: &CapabilityProfile) -> WorkflowRequest {
    let mut graph = WorkflowGraph {
        schema: GRAPH_SCHEMA.to_string(),
        identity: identity('0'),
        nodes: vec![WorkflowNode {
            id: label("build"),
            kind: label("build"),
            effect_class: EffectClass::Build,
            adapter_kind: label("local"),
            operation_identity: identity('1'),
        }],
        edges: Vec::new(),
    };
    graph.identity = graph_identity(&graph);
    let mut request = WorkflowRequest {
        schema: REQUEST_SCHEMA.to_string(),
        identity: identity('0'),
        idempotency_key: identity('2'),
        profile_identity: profile_identity(profile),
        platform: label("linux-x86_64"),
        kiln: KilnLinkage {
            effect_identity: identity('3'),
            attempt_identity: identity('4'),
            candidate_identity: identity('5'),
            pipeline_revision_identity: identity('6'),
            workflow_policy_identity: identity('7'),
        },
        source: None,
        graph,
        controls: RequestedControls {
            cancel: true,
            pause: true,
            resume: true,
        },
        observation: ObservationPolicy {
            maximum_log_references: TEST_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: TEST_MAXIMUM_TOTAL_LOG_BYTES,
        },
    };
    request.identity = lattice_workflow_contract::request_identity(&request);
    request
}

fn accepted(request: &WorkflowRequest) -> WorkflowAccepted {
    WorkflowAccepted {
        schema: ACCEPTED_SCHEMA.to_string(),
        request_identity: request.identity.clone(),
        idempotency_key: request.idempotency_key.clone(),
        workflow_identity: identity('8'),
        execution_identity: identity('9'),
    }
}

fn observation(request: &WorkflowRequest, accepted: &WorkflowAccepted) -> ExecutionObservation {
    let log_bytes = u64::try_from(TEST_LOG_BYTES.len()).expect("test log length fits u64");
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0'),
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: TEST_SEQUENCE,
        state: ExecutionState::Completed,
        terminal: true,
        successful: true,
        nodes: vec![NodeObservation {
            node_id: request.graph.nodes.first().expect("test node exists").id.clone(),
            state: NodeState::Completed,
            terminal: true,
            successful: true,
            adapter_identity: identity('a'),
            receipt_identity: Some(identity('b')),
        }],
        logs: vec![LogReference {
            node_id: request.graph.nodes.first().expect("test node exists").id.clone(),
            stream: LogStream::Stdout,
            reference_identity: identity('c'),
            cursor: 0,
            bytes: log_bytes,
            truncated: false,
            gap_before: false,
            retained: true,
        }],
        receipt_identity: Some(identity('d')),
    };
    observation.identity = observation_identity(&observation);
    observation
}

fn transport_profile(user_id: u32) -> TransportProfile {
    TransportProfile {
        schema: TRANSPORT_PROFILE_SCHEMA.to_string(),
        limits: TransportLimits {
            maximum_frame_bytes: TEST_FRAME_BYTES,
            maximum_log_bytes: TEST_LOG_LIMIT_BYTES,
            maximum_connections: TEST_CONNECTIONS,
            read_deadline_milliseconds: TEST_DEADLINE_MILLISECONDS,
            write_deadline_milliseconds: TEST_DEADLINE_MILLISECONDS,
        },
        peers: PeerPolicy {
            allowed_user_ids: vec![user_id],
        },
        non_claims: crate::REQUIRED_NON_CLAIMS.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn dispatch_request() -> ExchangeRequest {
    let profile = contract_profile();
    let request = workflow_request(&profile);
    new_request(ExchangeCommand::Dispatch { profile, request }).expect("dispatch request is admitted")
}

#[derive(Debug)]
struct FixtureHandler {
    calls: usize,
}

impl WorkflowExchangeHandler for FixtureHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        self.calls = self.calls.checked_add(1).expect("fixture call count remains bounded");
        match command {
            ExchangeCommand::Dispatch { request, .. } | ExchangeCommand::RecoverDispatch { request, .. } => {
                Ok(ExchangeSuccess::Accepted(accepted(request)))
            }
            ExchangeCommand::Observe { request, accepted, .. }
            | ExchangeCommand::Control { request, accepted, .. }
            | ExchangeCommand::Reconcile { request, accepted, .. } => {
                Ok(ExchangeSuccess::Observation(observation(request, accepted)))
            }
            ExchangeCommand::ReadLog { request, accepted, .. } => Ok(ExchangeSuccess::Log {
                observation: observation(request, accepted),
                bytes: TEST_LOG_BYTES.to_vec(),
            }),
        }
    }
}

// r[verify lattice.workflow_interchange.transport_identity.scenario.equal]
// r[verify lattice.workflow_interchange.transport_identity.scenario.changed]
#[test]
fn request_identity_is_deterministic_and_body_bound() {
    let request = dispatch_request();
    let equal = dispatch_request();
    assert_eq!(request.identity, equal.identity);
    assert_eq!(validate_exchange_request(&request), Ok(()));

    let mut changed = request;
    if let ExchangeCommand::Dispatch { request, .. } = &mut changed.command {
        request.kiln.effect_identity = identity('e');
    }
    assert_eq!(validate_exchange_request(&changed), Err(AdmissionError::RequestIdentity));
}

// r[verify lattice.workflow_interchange.transport_handler.scenario.dispatch]
#[test]
fn service_admits_dispatch_and_validates_handler_result() {
    let request = dispatch_request();
    let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
    let response = service.exchange(&request).expect("service response is encodable");

    assert_eq!(validate_exchange_response(&request, &response), Ok(()));
    assert!(matches!(
        response.result,
        ExchangeResult::Success(success) if matches!(success.as_ref(), ExchangeSuccess::Accepted(_))
    ));
    assert_eq!(service.handler().calls, 1);
}

// r[verify lattice.workflow_interchange.transport_handler.scenario.stale]
#[test]
fn stale_control_is_rejected_before_handler_invocation() {
    let profile = contract_profile();
    let request = workflow_request(&profile);
    let accepted = accepted(&request);
    let control = ControlRequest {
        schema: CONTROL_SCHEMA.to_string(),
        request_identity: accepted.request_identity.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        expected_sequence: TEST_SEQUENCE.saturating_add(1),
        action: ControlAction::Cancel,
    };
    let exchange = new_request(ExchangeCommand::Control {
        profile,
        request,
        accepted,
        current_sequence: TEST_SEQUENCE,
        control,
    })
    .expect("stale command remains encodable");
    let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
    let response = service.exchange(&exchange).expect("rejection is encodable");

    assert!(matches!(response.result, ExchangeResult::Rejected(_)));
    assert_eq!(service.handler().calls, 0);
}

// r[verify lattice.workflow_interchange.transport_bounds.scenario.oversized]
#[test]
fn frame_reader_rejects_oversized_declaration_before_body_read() {
    let declared = TEST_FRAME_BYTES.saturating_add(1).to_be_bytes();
    let mut cursor = Cursor::new(declared);
    assert_eq!(read_frame(&mut cursor, TEST_FRAME_BYTES), Err(FrameFailure::TooLarge));
}

// r[verify lattice.workflow_interchange.transport_envelope.scenario.decode]
// r[verify lattice.workflow_interchange.transport_bounds.scenario.admitted]
#[test]
fn frame_round_trip_preserves_exchange_request() {
    let request = dispatch_request();
    let frame = encode_frame(&request, TEST_FRAME_BYTES).expect("request frame is bounded");
    let mut cursor = Cursor::new(frame);
    let body = read_frame(&mut cursor, TEST_FRAME_BYTES).expect("request frame is readable");
    let decoded: ExchangeRequest = serde_json::from_slice(&body).expect("request JSON is valid");
    assert_eq!(decoded, request);
}

// r[verify lattice.workflow_interchange.transport_envelope.scenario.unknown]
#[test]
fn unknown_wire_operation_is_rejected_before_admission() {
    let request = dispatch_request();
    let encoded = serde_json::to_string(&request).expect("request JSON is valid");
    let unknown = encoded.replace("\"operation\":\"dispatch\"", "\"operation\":\"future\"");
    let decoded = serde_json::from_str::<ExchangeRequest>(&unknown);
    assert!(decoded.is_err());
}

mod protocol;
#[cfg(target_os = "linux")]
mod socket;
