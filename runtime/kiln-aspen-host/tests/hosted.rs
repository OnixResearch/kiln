#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![cfg(feature = "test-fixture")]
#![allow(
    unknown_lints,
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed fixture invariants"
)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "integration tests keep concrete Aspen, Kiln, and Lattice owners visible"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their protocol ownership"
)]

use std::os::unix::net::UnixListener;

use kiln_adapter_aspen::profile::{admit_profile, profile_fixture};
use kiln_adapter_aspen::transport::{
    AspenServiceDecision, AspenServiceTransport, IngressSource, ServiceTransportOutcome,
    UnixAspenServiceTransport, build_ingress,
};
use kiln_adapter_lattice::{
    LatticeAdapter, PINNED_LATTICE_CONTRACT_LICENSE, PINNED_LATTICE_REVISION,
    PINNED_LATTICE_SOURCE, UnixWorkflowExchange,
};
use kiln_adapter_radicle::{
    AdapterLimits, CapabilitySelection, ConfigEffectClass, ConfigNode,
    PINNED_RADICLE_BROKER_REVISION, PINNED_RADICLE_LICENSE, PINNED_RADICLE_RUNTIME_REVISION,
    RADICLE_SHELL_SCHEMA, REQUIRED_NON_CLAIMS, RadicleShellConfig, RuntimeMode, WorkflowTemplate,
    admit_config,
};
use kiln_aspen_host::cohort::{HostCohort, HostPaths};
use kiln_aspen_host::effect::LatticeEffectPort;
use kiln_aspen_host::runtime::KilnAspenHostRuntime;
use kiln_aspen_host::server::{ServerConfig, serve};
use kiln_core::types::ExternalRef;
use kiln_core::workflow::{WorkflowBounds, WorkflowControls};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};
use lattice_workflow_contract::{
    ACCEPTED_SCHEMA, BLAKE3_PREFIX, ExecutionObservation, ExecutionState, Identity,
    NodeObservation, NodeState, OBSERVATION_SCHEMA, WorkflowAccepted, observation_identity,
};
use lattice_workflow_transport::{
    ExchangeCommand, ExchangeRequest, ExchangeSuccess, HandlerError, TransportLimits,
    WorkflowExchangeHandler, WorkflowExchangeService, read_frame, serve_listener,
};

const TEST_MAXIMUM_NODES: usize = 8;
const TEST_MAXIMUM_EDGES: usize = 16;
const TEST_MAXIMUM_LOG_REFERENCES: usize = 8;
const TEST_MAXIMUM_LOG_READ_BYTES: usize = 4_096;
const TEST_MAXIMUM_TOTAL_LOG_BYTES: usize = 16_384;
const TEST_MAXIMUM_FRAME_BYTES: u32 = 1_048_576;
const TEST_MAXIMUM_LOG_BYTES: u32 = 65_536;
const TEST_MAXIMUM_CONNECTIONS: u32 = 2;
const TEST_DEADLINE_MILLISECONDS: u64 = 5_000;
const TEST_MAXIMUM_REPLAY_RECORDS: usize = 16;
const TEST_MAXIMUM_POLLS: usize = 4;
const TEST_MAXIMUM_IN_FLIGHT: usize = 4;
const TEST_SEQUENCE: u64 = 1;
const TEST_IDENTITY_DIGITS: usize = 64;
const HOST_MAXIMUM_REQUESTS: usize = 3;
const HOST_TIMEOUT_MILLISECONDS: u64 = 5_000;
const SOCKET_READY_ATTEMPTS: usize = 100;
const SOCKET_READY_DELAY_MILLISECONDS: u64 = 10;
const OPERATION_IDENTITY: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[derive(Debug)]
struct TerminalHandler;

impl WorkflowExchangeHandler for TerminalHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        match command {
            ExchangeCommand::Dispatch { request, .. }
            | ExchangeCommand::RecoverDispatch { request, .. } => {
                Ok(ExchangeSuccess::Accepted(accepted(request)?))
            }
            ExchangeCommand::Observe {
                request, accepted, ..
            }
            | ExchangeCommand::Reconcile {
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

fn radicle_config(
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

fn trigger(is_same_revision: bool) -> TriggerRequest {
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

fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!("external reference"))
}

fn accepted(
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

fn observation(
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
        sequence: TEST_SEQUENCE,
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

#[path = "hosted/accepted.rs"]
mod accepted_test;
#[path = "hosted/recovery.rs"]
mod recovery_test;
#[path = "hosted/rejected.rs"]
mod rejected_test;
#[path = "hosted/uncertain.rs"]
mod uncertain_test;
