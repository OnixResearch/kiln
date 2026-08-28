use super::*;

pub(super) const TEST_MAXIMUM_NODES: usize = 8;
pub(super) const TEST_MAXIMUM_EDGES: usize = 16;
pub(super) const TEST_MAXIMUM_LOG_REFERENCES: usize = 8;
pub(super) const TEST_MAXIMUM_LOG_READ_BYTES: usize = 4_096;
pub(super) const TEST_MAXIMUM_TOTAL_LOG_BYTES: usize = 16_384;
pub(super) const TEST_MAXIMUM_FRAME_BYTES: u32 = 1_048_576;
pub(super) const TEST_MAXIMUM_LOG_BYTES: u32 = 65_536;
pub(super) const TEST_MAXIMUM_CONNECTIONS: u32 = 2;
pub(super) const TEST_DEADLINE_MILLISECONDS: u64 = 5_000;
pub(super) const TEST_MAXIMUM_REPLAY_RECORDS: usize = 16;
pub(super) const TEST_MAXIMUM_POLLS: usize = 4;
pub(super) const TEST_MAXIMUM_IN_FLIGHT: usize = 4;
pub(super) const TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS: u64 = 10;
pub(super) const DELAYED_OBSERVATION_COUNT: usize = 1;
const PENDING_SEQUENCE: u64 = 1;
const TERMINAL_SEQUENCE: u64 = 2;
pub(super) const HOST_MAXIMUM_REQUESTS: usize = 3;
pub(super) const HOST_TIMEOUT_MILLISECONDS: u64 = 5_000;
pub(super) const SOCKET_READY_ATTEMPTS: usize = 100;
pub(super) const SOCKET_READY_DELAY_MILLISECONDS: u64 = 10;
const TEST_IDENTITY_DIGITS: usize = 64;
const OPERATION_IDENTITY: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[derive(Debug, Default)]
pub(super) struct CountingObservationPacer {
    waits: AtomicUsize,
}

impl ObservationPacer for CountingObservationPacer {
    fn wait(&self, interval: std::time::Duration) {
        assert!(!interval.is_zero());
        self.waits.fetch_add(1, Ordering::SeqCst);
    }
}

impl CountingObservationPacer {
    pub(super) fn wait_count(&self) -> usize {
        self.waits.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub(super) struct TerminalHandler {
    pub(super) pending_observations_remaining: usize,
}

impl WorkflowExchangeHandler for TerminalHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        match command {
            ExchangeCommand::Dispatch { request, .. }
            | ExchangeCommand::RecoverDispatch { request, .. } => {
                Ok(ExchangeSuccess::Accepted(accepted(request)?))
            }
            ExchangeCommand::Observe {
                request, accepted, ..
            } => {
                if self.pending_observations_remaining > 0 {
                    self.pending_observations_remaining -= 1;
                    Ok(ExchangeSuccess::Observation(pending_observation(
                        request, accepted,
                    )?))
                } else {
                    Ok(ExchangeSuccess::Observation(observation(
                        request, accepted,
                    )?))
                }
            }
            ExchangeCommand::Reconcile {
                request, accepted, ..
            }
            | ExchangeCommand::Control {
                request, accepted, ..
            } => Ok(ExchangeSuccess::Observation(observation(
                request, accepted,
            )?)),
            ExchangeCommand::ReadLog { .. } => Err(HandlerError::Rejected),
        }
    }
}

pub(super) fn radicle_config(
    socket_path: std::path::PathBuf,
    replay_database_path: std::path::PathBuf,
    user_id: u32,
) -> RadicleShellConfig {
    RadicleShellConfig {
        schema: RADICLE_SHELL_SCHEMA.to_string(),
        mode: RuntimeMode::Production,
        radicle_runtime_revision: PINNED_RADICLE_RUNTIME_REVISION.to_string(),
        radicle_broker_revision: PINNED_RADICLE_BROKER_REVISION.to_string(),
        radicle_license: PINNED_RADICLE_LICENSE.to_string(),
        lattice_source: PINNED_LATTICE_SOURCE.to_string(),
        lattice_revision: PINNED_LATTICE_REVISION.to_string(),
        lattice_license: PINNED_LATTICE_CONTRACT_LICENSE.to_string(),
        capabilities: CapabilitySelection {
            source: false,
            trigger: true,
            status: false,
            collaboration: false,
        },
        socket_path,
        replay_database_path,
        maximum_replay_records: TEST_MAXIMUM_REPLAY_RECORDS,
        maximum_polls: TEST_MAXIMUM_POLLS,
        allowed_user_ids: vec![user_id],
        transport_limits: TransportLimits {
            maximum_frame_bytes: TEST_MAXIMUM_FRAME_BYTES,
            maximum_log_bytes: TEST_MAXIMUM_LOG_BYTES,
            maximum_connections: TEST_MAXIMUM_CONNECTIONS,
            read_deadline_milliseconds: TEST_DEADLINE_MILLISECONDS,
            write_deadline_milliseconds: TEST_DEADLINE_MILLISECONDS,
        },
        adapter_limits: AdapterLimits {
            max_request_bytes: usize::try_from(TEST_MAXIMUM_FRAME_BYTES)
                .unwrap_or_else(|_| unreachable!("request bound")),
            max_response_bytes: usize::try_from(TEST_MAXIMUM_FRAME_BYTES)
                .unwrap_or_else(|_| unreachable!("response bound")),
            max_in_flight: TEST_MAXIMUM_IN_FLIGHT,
        },
        workflow: WorkflowTemplate {
            policy_id: "radicle-ci-workflow-v1".to_string(),
            provider_profile_id: "lattice-v1".to_string(),
            platform: "linux-x86_64".to_string(),
            nodes: vec![ConfigNode {
                id: "build".to_string(),
                kind: "build".to_string(),
                effect_class: ConfigEffectClass::Build,
                adapter_kind: "local".to_string(),
                operation_identity: OPERATION_IDENTITY.to_string(),
            }],
            edges: Vec::new(),
            controls: WorkflowControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            bounds: WorkflowBounds {
                max_nodes: TEST_MAXIMUM_NODES,
                max_edges: TEST_MAXIMUM_EDGES,
                max_log_references: TEST_MAXIMUM_LOG_REFERENCES,
                max_log_read_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
                max_total_log_bytes: TEST_MAXIMUM_TOTAL_LOG_BYTES,
            },
        },
        non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
    }
}

pub(super) fn trigger(is_same_revision: bool) -> TriggerRequest {
    let after = reference(
        "radicle",
        "revision",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    TriggerRequest {
        repository: reference("radicle", "repository", "rad:hosted-fixture"),
        actor: reference("radicle", "actor", "did:key:hosted-fixture"),
        before: Some(if is_same_revision {
            after.clone()
        } else {
            reference(
                "radicle",
                "revision",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
        }),
        after,
        branch: None,
        patch: None,
        event: TriggerEventKind::Push,
    }
}

pub(super) fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!("external reference"))
}

pub(super) fn accepted(
    request: &lattice_workflow_contract::WorkflowRequest,
) -> Result<WorkflowAccepted, HandlerError> {
    Ok(WorkflowAccepted {
        schema: ACCEPTED_SCHEMA.to_string(),
        request_identity: request.identity.clone(),
        idempotency_key: request.idempotency_key.clone(),
        workflow_identity: identity('8')?,
        execution_identity: identity('9')?,
    })
}

fn pending_observation(
    request: &lattice_workflow_contract::WorkflowRequest,
    accepted: &WorkflowAccepted,
) -> Result<ExecutionObservation, HandlerError> {
    let node = request.graph.nodes.first().ok_or(HandlerError::Rejected)?;
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0')?,
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: PENDING_SEQUENCE,
        state: ExecutionState::Running,
        terminal: false,
        successful: false,
        nodes: vec![NodeObservation {
            node_id: node.id.clone(),
            state: NodeState::Running,
            terminal: false,
            successful: false,
            adapter_identity: identity('a')?,
            receipt_identity: None,
        }],
        logs: Vec::new(),
        receipt_identity: None,
    };
    observation.identity = observation_identity(&observation);
    Ok(observation)
}

pub(super) fn observation(
    request: &lattice_workflow_contract::WorkflowRequest,
    accepted: &WorkflowAccepted,
) -> Result<ExecutionObservation, HandlerError> {
    let node = request.graph.nodes.first().ok_or(HandlerError::Rejected)?;
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0')?,
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: TERMINAL_SEQUENCE,
        state: ExecutionState::Completed,
        terminal: true,
        successful: true,
        nodes: vec![NodeObservation {
            node_id: node.id.clone(),
            state: NodeState::Completed,
            terminal: true,
            successful: true,
            adapter_identity: identity('a')?,
            receipt_identity: Some(identity('b')?),
        }],
        logs: Vec::new(),
        receipt_identity: Some(identity('c')?),
    };
    observation.identity = observation_identity(&observation);
    Ok(observation)
}

fn identity(digit: char) -> Result<Identity, HandlerError> {
    Identity::new(format!(
        "{BLAKE3_PREFIX}{}",
        digit.to_string().repeat(TEST_IDENTITY_DIGITS),
    ))
    .map_err(|_| HandlerError::Rejected)
}
