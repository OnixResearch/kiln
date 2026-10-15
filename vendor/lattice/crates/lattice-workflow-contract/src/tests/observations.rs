use super::*;

// r[verify lattice.workflow_interchange.observation]
// r[verify lattice.workflow_interchange.observation.scenario.completed]
#[test]
fn linked_terminal_observation_is_admitted() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let observation = completed_observation(&request, &accepted);

    assert_eq!(validate_observation(&profile, &request, &accepted, &observation), Ok(()));
}

// r[verify lattice.workflow_interchange.observation.scenario.paused]
#[test]
fn paused_observation_cannot_claim_terminal_success() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut observation = completed_observation(&request, &accepted);
    observation.state = ExecutionState::Paused;
    observation.identity = observation_identity(&observation);

    let error = validate_observation(&profile, &request, &accepted, &observation)
        .expect_err("paused terminal success must fail");
    assert_eq!(error.code, ErrorCode::ObservationStateMismatch);
}

// r[verify lattice.workflow_interchange.observation.scenario.stale]
#[test]
fn stale_execution_observation_is_rejected() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut observation = completed_observation(&request, &accepted);
    observation.execution_identity = identity('2');
    observation.identity = observation_identity(&observation);

    let error =
        validate_observation(&profile, &request, &accepted, &observation).expect_err("stale execution must fail");
    assert_eq!(error.code, ErrorCode::ObservationLinkageMismatch);
}

#[test]
fn missing_node_observation_is_rejected() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut observation = completed_observation(&request, &accepted);
    observation.nodes.pop();
    observation.identity = observation_identity(&observation);

    let error = validate_observation(&profile, &request, &accepted, &observation).expect_err("missing node must fail");
    assert_eq!(error.code, ErrorCode::ObservationNodeMismatch);
}

#[test]
fn oversized_log_observation_is_rejected() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut observation = completed_observation(&request, &accepted);
    observation.logs[0].bytes = TEST_MAXIMUM_LOG_READ_BYTES.saturating_add(1);
    observation.identity = observation_identity(&observation);

    let error = validate_observation(&profile, &request, &accepted, &observation).expect_err("oversized log must fail");
    assert_eq!(error.code, ErrorCode::ObservationLimit);
}

// r[verify lattice.workflow_interchange.controls_logs.scenario.gap]
#[test]
fn log_retention_gap_remains_explicit_and_admitted() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut observation = completed_observation(&request, &accepted);
    observation.logs[0].gap_before = true;
    observation.logs[0].truncated = true;
    observation.logs[0].retained = false;
    observation.identity = observation_identity(&observation);

    assert_eq!(validate_observation(&profile, &request, &accepted, &observation), Ok(()));
    assert!(observation.logs[0].gap_before);
    assert!(observation.logs[0].truncated);
    assert!(!observation.logs[0].retained);
}

#[test]
fn malformed_wire_identity_is_rejected_during_decode() {
    let decoded = serde_json::from_str::<Identity>("\"b3:not-canonical\"");
    assert!(decoded.is_err());
}

// r[verify lattice.workflow_interchange.controls_logs]
// r[verify lattice.workflow_interchange.controls_logs.scenario.cancel]
#[test]
fn controls_require_exact_execution_and_sequence() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut control = ControlRequest {
        schema: CONTROL_SCHEMA.to_string(),
        request_identity: accepted.request_identity.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        expected_sequence: TEST_SEQUENCE,
        action: ControlAction::Cancel,
    };

    assert_eq!(validate_control_request(&profile, &request, &accepted, TEST_SEQUENCE, &control), Ok(()));
    control.expected_sequence = TEST_NEXT_SEQUENCE;
    let error = validate_control_request(&profile, &request, &accepted, TEST_SEQUENCE, &control)
        .expect_err("stale sequence must fail");
    assert_eq!(error.code, ErrorCode::ControlLinkageMismatch);
}

// r[verify lattice.workflow_interchange.controls_logs.scenario.log-bound]
#[test]
fn log_reads_are_bounded_and_linked() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let mut log_read = LogReadRequest {
        schema: LOG_READ_SCHEMA.to_string(),
        request_identity: accepted.request_identity.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        node_id: request.graph.nodes[0].id.clone(),
        stream: LogStream::Stdout,
        cursor: 0,
        maximum_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
    };

    assert_eq!(validate_log_read_request(&request, &accepted, &log_read), Ok(()));
    log_read.maximum_bytes = TEST_MAXIMUM_LOG_READ_BYTES.saturating_add(1);
    let error = validate_log_read_request(&request, &accepted, &log_read).expect_err("oversized log read must fail");
    assert_eq!(error.code, ErrorCode::ObservationLimit);
}
