use super::*;

// r[verify lattice.workflow_interchange.transport_dispatch_recovery.scenario.lost]
// r[verify lattice.workflow_interchange.transport_dispatch_recovery_result.scenario.accepted]
#[test]
fn dispatch_recovery_returns_the_exact_linked_acceptance() {
    let profile = contract_profile();
    let request = workflow_request(&profile);
    let exchange =
        new_request(ExchangeCommand::RecoverDispatch { profile, request }).expect("recovery request is encodable");
    let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
    let response = service.exchange(&exchange).expect("recovery response is encodable");

    assert_eq!(validate_exchange_response(&exchange, &response), Ok(()));
    assert!(matches!(
        response.result,
        ExchangeResult::Success(success) if matches!(success.as_ref(), ExchangeSuccess::Accepted(_))
    ));
}

// r[verify lattice.workflow_interchange.transport_dispatch_recovery_result.scenario.observed]
#[test]
fn dispatch_recovery_rejects_an_observation_without_the_exact_acceptance() {
    struct AlternateHandler;

    impl WorkflowExchangeHandler for AlternateHandler {
        fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
            let ExchangeCommand::RecoverDispatch { request, .. } = command else {
                return Err(HandlerError::Rejected);
            };
            let accepted = accepted(request);
            Ok(ExchangeSuccess::Observation(observation(request, &accepted)))
        }
    }

    let profile = contract_profile();
    let request = workflow_request(&profile);
    let exchange =
        new_request(ExchangeCommand::RecoverDispatch { profile, request }).expect("recovery request is encodable");
    let mut service = WorkflowExchangeService::new(AlternateHandler);
    let response = service.exchange(&exchange).expect("observation rejection is encodable");

    assert!(matches!(
        response.result,
        ExchangeResult::Rejected(Rejection {
            code: RejectionCode::InvalidOperationResult,
            ..
        })
    ));
}

#[test]
fn stale_recovery_observation_is_replaced_with_a_safe_rejection() {
    struct ForeignHandler;

    impl WorkflowExchangeHandler for ForeignHandler {
        fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
            let ExchangeCommand::RecoverDispatch { request, .. } = command else {
                return Err(HandlerError::Rejected);
            };
            let accepted = accepted(request);
            let mut stale = observation(request, &accepted);
            stale.execution_identity = identity('e');
            stale.identity = observation_identity(&stale);
            Ok(ExchangeSuccess::Observation(stale))
        }
    }

    let profile = contract_profile();
    let request = workflow_request(&profile);
    let exchange =
        new_request(ExchangeCommand::RecoverDispatch { profile, request }).expect("recovery request is encodable");
    let mut service = WorkflowExchangeService::new(ForeignHandler);
    let response = service.exchange(&exchange).expect("safe rejection is encodable");
    assert!(matches!(
        response.result,
        ExchangeResult::Rejected(Rejection {
            code: RejectionCode::InvalidOperationResult,
            ..
        })
    ));
}

// r[verify lattice.workflow_interchange.transport_dispatch_recovery.scenario.changed]
#[test]
fn changed_recovery_request_fails_before_handler_invocation() {
    let profile = contract_profile();
    let request = workflow_request(&profile);
    let mut exchange =
        new_request(ExchangeCommand::RecoverDispatch { profile, request }).expect("recovery request is encodable");
    if let ExchangeCommand::RecoverDispatch { request, .. } = &mut exchange.command {
        request.idempotency_key = identity('e');
    }
    let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
    let response = service.exchange(&exchange).expect("rejection response is encodable");

    assert!(matches!(response.result, ExchangeResult::Rejected(_)));
    assert_eq!(service.handler().calls, 0);
}

// r[verify lattice.workflow_interchange.transport_dispatch_recovery_result.scenario.not-found]
#[test]
fn missing_recovery_request_returns_a_closed_not_found_rejection() {
    struct AbsentHandler;

    impl WorkflowExchangeHandler for AbsentHandler {
        fn handle(&mut self, _command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
            Err(HandlerError::NotFound)
        }
    }

    let profile = contract_profile();
    let request = workflow_request(&profile);
    let exchange =
        new_request(ExchangeCommand::RecoverDispatch { profile, request }).expect("recovery request is encodable");
    let mut service = WorkflowExchangeService::new(AbsentHandler);
    let response = service.exchange(&exchange).expect("not-found response is encodable");

    assert!(matches!(
        response.result,
        ExchangeResult::Rejected(Rejection {
            code: RejectionCode::NotFound,
            ..
        })
    ));
}

// r[verify lattice.workflow_interchange.transport_response.scenario.log]
#[test]
fn linked_log_bytes_are_returned_with_the_admitted_observation() {
    let profile = contract_profile();
    let request = workflow_request(&profile);
    let accepted = accepted(&request);
    let node_id = request.graph.nodes.first().expect("test node exists").id.clone();
    let log_read = LogReadRequest {
        schema: LOG_READ_SCHEMA.to_string(),
        request_identity: accepted.request_identity.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        node_id,
        stream: LogStream::Stdout,
        cursor: 0,
        maximum_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
    };
    let exchange = new_request(ExchangeCommand::ReadLog {
        profile,
        request,
        accepted,
        log_read,
    })
    .expect("log request is encodable");
    let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
    let response = service.exchange(&exchange).expect("log response is encodable");
    let endpoint_profile = transport_profile(TEST_PROFILE_USER_ID);
    assert_eq!(validate_response_limits(&endpoint_profile, &response), Ok(()));
    let mut narrow_profile = endpoint_profile;
    narrow_profile.limits.maximum_log_bytes = 1;
    assert_eq!(validate_response_limits(&narrow_profile, &response), Err(AdmissionError::LogBound));

    assert!(matches!(
        response.result,
        ExchangeResult::Success(success)
            if matches!(success.as_ref(), ExchangeSuccess::Log { bytes, .. } if bytes == TEST_LOG_BYTES)
    ));
}

// r[verify lattice.workflow_interchange.transport_response.scenario.oversized]
#[test]
fn log_bytes_must_match_the_admitted_reference() {
    struct MismatchedHandler;

    impl WorkflowExchangeHandler for MismatchedHandler {
        fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
            let ExchangeCommand::ReadLog { request, accepted, .. } = command else {
                return Err(HandlerError::Rejected);
            };
            let mut bytes = TEST_LOG_BYTES.to_vec();
            bytes.push(0);
            Ok(ExchangeSuccess::Log {
                observation: observation(request, accepted),
                bytes,
            })
        }
    }

    let profile = contract_profile();
    let request = workflow_request(&profile);
    let accepted = accepted(&request);
    let node_id = request.graph.nodes.first().expect("test node exists").id.clone();
    let log_read = LogReadRequest {
        schema: LOG_READ_SCHEMA.to_string(),
        request_identity: accepted.request_identity.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        node_id,
        stream: LogStream::Stdout,
        cursor: 0,
        maximum_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
    };
    let exchange = new_request(ExchangeCommand::ReadLog {
        profile,
        request,
        accepted,
        log_read,
    })
    .expect("log request is encodable");
    let mut service = WorkflowExchangeService::new(MismatchedHandler);
    let response = service.exchange(&exchange).expect("safe rejection is encodable");

    assert!(matches!(
        response.result,
        ExchangeResult::Rejected(Rejection {
            code: RejectionCode::InvalidOperationResult,
            ..
        })
    ));
}
