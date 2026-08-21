use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::thread;

use lattice_workflow_transport::ExchangeCommand;
use lattice_workflow_transport::ExchangeRequest;
use lattice_workflow_transport::ExchangeResult;
use lattice_workflow_transport::ExchangeSuccess;
use lattice_workflow_transport::HandlerError;
use lattice_workflow_transport::PeerPolicy;
use lattice_workflow_transport::Rejection;
use lattice_workflow_transport::RejectionCode;
use lattice_workflow_transport::TransportLimits;
use lattice_workflow_transport::TransportProfile;
use lattice_workflow_transport::WorkflowExchangeHandler;
use lattice_workflow_transport::WorkflowExchangeService;
use lattice_workflow_transport::encode_frame;
use lattice_workflow_transport::new_response;
use lattice_workflow_transport::read_frame;
use lattice_workflow_transport::serve_listener;
use lattice_workflow_transport::write_frame;

use super::*;

const SOCKET_FRAME_BYTES: u32 = 1_048_576;
const SOCKET_LOG_BYTES: u32 = 65_536;
const SOCKET_CONNECTIONS: u32 = 7;
const RECOVERY_CONNECTIONS: u32 = 2;
const SOCKET_DEADLINE_MILLISECONDS: u64 = 5_000;
const MAXIMUM_CLIENT_RECORDS: usize = 8;

#[derive(Debug, Default)]
struct SocketHandler {
    workflow: Option<WorkflowRequest>,
    accepted: Option<WorkflowAccepted>,
    sequence: u64,
}

impl WorkflowExchangeHandler for SocketHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        self.sequence = self.sequence.saturating_add(1);
        match command {
            ExchangeCommand::Dispatch { request, .. }
            | ExchangeCommand::RecoverDispatch { request, .. } => {
                let linked = accepted(request);
                self.workflow = Some(request.clone());
                self.accepted = Some(linked.clone());
                Ok(ExchangeSuccess::Accepted(linked))
            }
            ExchangeCommand::Observe {
                request, accepted, ..
            } => Ok(ExchangeSuccess::Observation(observation(
                request,
                accepted,
                ExecutionState::Running,
                false,
                self.sequence,
            ))),
            ExchangeCommand::Control {
                request,
                accepted,
                control,
                ..
            } => {
                let state = match control.action {
                    lattice_workflow_contract::ControlAction::Cancel => ExecutionState::Cancelled,
                    lattice_workflow_contract::ControlAction::Pause => ExecutionState::Paused,
                    lattice_workflow_contract::ControlAction::Resume => ExecutionState::Running,
                };
                Ok(ExchangeSuccess::Observation(observation(
                    request,
                    accepted,
                    state,
                    false,
                    self.sequence,
                )))
            }
            ExchangeCommand::ReadLog {
                request, accepted, ..
            } => Ok(ExchangeSuccess::Log {
                observation: observation(
                    request,
                    accepted,
                    ExecutionState::Completed,
                    true,
                    self.sequence,
                ),
                bytes: LOG_BYTES.to_vec(),
            }),
            ExchangeCommand::Reconcile {
                request, accepted, ..
            } => Ok(ExchangeSuccess::Observation(observation(
                request,
                accepted,
                ExecutionState::Completed,
                false,
                self.sequence,
            ))),
        }
    }
}

fn transport_profile(user_id: u32, maximum_connections: u32) -> TransportProfile {
    TransportProfile {
        schema: lattice_workflow_transport::TRANSPORT_PROFILE_SCHEMA.to_string(),
        limits: TransportLimits {
            maximum_frame_bytes: SOCKET_FRAME_BYTES,
            maximum_log_bytes: SOCKET_LOG_BYTES,
            maximum_connections,
            read_deadline_milliseconds: SOCKET_DEADLINE_MILLISECONDS,
            write_deadline_milliseconds: SOCKET_DEADLINE_MILLISECONDS,
        },
        peers: PeerPolicy {
            allowed_user_ids: vec![user_id],
        },
        non_claims: lattice_workflow_transport::REQUIRED_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}

fn user_id(directory: &std::path::Path) -> u32 {
    std::fs::metadata(directory)
        .map(|metadata| metadata.uid())
        .unwrap_or_else(|_| unreachable!())
}

fn client(socket_path: std::path::PathBuf, transport: TransportProfile) -> UnixWorkflowExchange {
    let profile = profile();
    UnixWorkflowExchange::new(
        socket_path,
        transport,
        profile.contract,
        MAXIMUM_CLIENT_RECORDS,
    )
    .unwrap_or_else(|_| unreachable!())
}

#[derive(serde::Deserialize)]
struct RecoveryFixtures {
    request: ExchangeRequest,
    accepted_response: lattice_workflow_transport::ExchangeResponse,
    not_found_response: lattice_workflow_transport::ExchangeResponse,
}

#[test]
fn pinned_recovery_fixtures_pass_producer_admission() {
    let fixtures: RecoveryFixtures = serde_json::from_str(include_str!(
        "../../../../fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/recovery-fixtures-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        lattice_workflow_transport::validate_exchange_request(&fixtures.request),
        Ok(())
    );
    assert_eq!(
        lattice_workflow_transport::validate_exchange_response(
            &fixtures.request,
            &fixtures.accepted_response,
        ),
        Ok(())
    );
    assert_eq!(
        lattice_workflow_transport::validate_exchange_response(
            &fixtures.request,
            &fixtures.not_found_response,
        ),
        Ok(())
    );
}

// r[verify kiln.lattice.transport_client.scenario.valid]
// r[verify kiln.lattice.transport_conformance]
// r[verify kiln.lattice.transport_conformance.scenario.fixture-only]
#[test]
fn concrete_socket_client_covers_every_exchange_operation() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let socket_path = directory.path().join("lattice.sock");
    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|_| unreachable!());
    let transport = transport_profile(user_id(directory.path()), SOCKET_CONNECTIONS);
    let server_profile = transport.clone();
    let server = thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(SocketHandler::default());
        serve_listener(&listener, &server_profile, &mut service)
    });
    let workflow_plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), client(socket_path, transport))
        .unwrap_or_else(|_| unreachable!());

    let accepted = adapter
        .dispatch(&workflow_plan)
        .unwrap_or_else(|_| unreachable!());
    let running = adapter
        .observe(&workflow_plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(running.state, WorkflowState::Running);
    let paused = adapter
        .control(&WorkflowControlRequest {
            effect_id: workflow_plan.effect_id.clone(),
            execution: accepted.execution.clone(),
            expected_sequence: running.sequence,
            action: WorkflowControlAction::Pause,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(paused.state, WorkflowState::Paused);
    let resumed = adapter
        .control(&WorkflowControlRequest {
            effect_id: workflow_plan.effect_id.clone(),
            execution: accepted.execution.clone(),
            expected_sequence: paused.sequence,
            action: WorkflowControlAction::Resume,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(resumed.state, WorkflowState::Running);
    let log = adapter
        .read_log(&WorkflowLogReadRequest {
            effect_id: workflow_plan.effect_id.clone(),
            execution: accepted.execution.clone(),
            node_id: alias("build"),
            stream: WorkflowLogStream::Stdout,
            cursor: 0,
            max_bytes: MAX_LOG_READ_BYTES,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(log.bytes, LOG_BYTES);
    let reconciled = adapter
        .reconcile(&workflow_plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(reconciled.state, WorkflowState::Completed);
    let cancelled = adapter
        .control(&WorkflowControlRequest {
            effect_id: workflow_plan.effect_id,
            execution: accepted.execution,
            expected_sequence: reconciled.sequence,
            action: WorkflowControlAction::Cancel,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(cancelled.state, WorkflowState::Cancelled);

    let report = server
        .join()
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(report.completed_exchanges, SOCKET_CONNECTIONS);
}

// r[verify kiln.lattice.transport_client.scenario.unavailable]
#[test]
fn unavailable_socket_fails_without_fallback() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let socket_path = directory.path().join("missing.sock");
    let transport = transport_profile(user_id(directory.path()), 1);
    let mut adapter = LatticeAdapter::new(profile(), client(socket_path, transport))
        .unwrap_or_else(|_| unreachable!());
    let Err(failure) = adapter.dispatch(&plan()) else {
        unreachable!();
    };
    assert!(matches!(failure, PortFailure::Infrastructure(_)));
}

// r[verify kiln.lattice.transport_recovery.scenario.accepted]
#[test]
fn unknown_dispatch_recovers_acceptance_without_redispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let socket_path = directory.path().join("recovery.sock");
    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|_| unreachable!());
    let transport = transport_profile(user_id(directory.path()), RECOVERY_CONNECTIONS);
    let maximum_frame_bytes = transport.limits.maximum_frame_bytes;
    let server = thread::spawn(move || {
        let (mut first, _address) = listener.accept().unwrap_or_else(|_| unreachable!());
        let dispatch_body =
            read_frame(&mut first, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        let dispatch: ExchangeRequest =
            serde_json::from_slice(&dispatch_body).unwrap_or_else(|_| unreachable!());
        let workflow = match dispatch.command {
            ExchangeCommand::Dispatch { request, .. } => request,
            _ => unreachable!(),
        };
        drop(first);

        let (mut second, _address) = listener.accept().unwrap_or_else(|_| unreachable!());
        let recovery_body =
            read_frame(&mut second, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        let recovery: ExchangeRequest =
            serde_json::from_slice(&recovery_body).unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            recovery.command,
            ExchangeCommand::RecoverDispatch { .. }
        ));
        let response = new_response(
            recovery.identity,
            ExchangeResult::Success(Box::new(ExchangeSuccess::Accepted(accepted(&workflow)))),
        )
        .unwrap_or_else(|_| unreachable!());
        let frame = encode_frame(&response, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        write_frame(&mut second, &frame).unwrap_or_else(|_| unreachable!());
    });
    let workflow_plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), client(socket_path, transport))
        .unwrap_or_else(|_| unreachable!());
    let recovered = adapter
        .dispatch(&workflow_plan)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(recovered.state, WorkflowState::Accepted);
    server.join().unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.lattice.transport_recovery.scenario.not-found]
#[test]
fn not_found_recovery_remains_unknown_without_retry_authority() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let socket_path = directory.path().join("not-found.sock");
    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|_| unreachable!());
    let transport = transport_profile(user_id(directory.path()), RECOVERY_CONNECTIONS);
    let maximum_frame_bytes = transport.limits.maximum_frame_bytes;
    let server = thread::spawn(move || {
        let (mut first, _address) = listener.accept().unwrap_or_else(|_| unreachable!());
        let _dispatch =
            read_frame(&mut first, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        drop(first);
        let (mut second, _address) = listener.accept().unwrap_or_else(|_| unreachable!());
        let recovery_body =
            read_frame(&mut second, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        let recovery: ExchangeRequest =
            serde_json::from_slice(&recovery_body).unwrap_or_else(|_| unreachable!());
        let response = new_response(
            recovery.identity,
            ExchangeResult::Rejected(Rejection {
                code: RejectionCode::NotFound,
                field: "exchange.handler".to_string(),
            }),
        )
        .unwrap_or_else(|_| unreachable!());
        let frame = encode_frame(&response, maximum_frame_bytes).unwrap_or_else(|_| unreachable!());
        write_frame(&mut second, &frame).unwrap_or_else(|_| unreachable!());
    });
    let mut adapter = LatticeAdapter::new(profile(), client(socket_path, transport))
        .unwrap_or_else(|_| unreachable!());
    let Err(failure) = adapter.dispatch(&plan()) else {
        unreachable!();
    };
    assert!(matches!(failure, PortFailure::Unknown(_)));
    server.join().unwrap_or_else(|_| unreachable!());
}
