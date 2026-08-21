use std::collections::BTreeMap;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::Outcome;
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId, PolicyId,
    ProviderProfileId,
};
use kiln_core::workflow::{
    WorkflowBounds, WorkflowControlAction, WorkflowControlRequest, WorkflowControls, WorkflowEdge,
    WorkflowEffectClass, WorkflowEffectPlan, WorkflowLogReadRequest, WorkflowLogStream,
    WorkflowNode, WorkflowSourceContext, WorkflowState,
};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, WorkflowExchangeOperation,
    WorkflowExchangePort, WorkflowExchangeRequest, WorkflowExchangeResponse, WorkflowPort,
};
use lattice_workflow_contract::{
    ACCEPTED_SCHEMA, ExecutionObservation, ExecutionState, Identity, LogReference, LogStream,
    NodeObservation, NodeState, OBSERVATION_SCHEMA, WorkflowAccepted, WorkflowRequest,
    observation_identity,
};

use super::*;

const IDENTITY_HEX_BYTES: usize = 64;
const MAX_NODES: usize = 8;
const MAX_EDGES: usize = 16;
const MAX_LOG_REFERENCES: usize = 8;
const MAX_LOG_READ_BYTES: usize = 4096;
const MAX_TOTAL_LOG_BYTES: usize = 16384;
const MAX_REQUEST_BYTES: usize = 65536;
const MAX_RESPONSE_BYTES: usize = 65536;
const MAX_IN_FLIGHT: usize = 4;
const FIRST_OBSERVATION_SEQUENCE: u64 = 1;
const LOG_BYTES: &[u8] = b"log";

#[derive(Debug, Default)]
struct FixtureTransport {
    exchanges: Vec<WorkflowExchangeOperation>,
    requests: BTreeMap<String, (WorkflowRequest, WorkflowAccepted)>,
    unavailable: bool,
    disconnect_dispatch: bool,
    stale_observation: bool,
    sequence: u64,
}

impl WorkflowExchangePort for FixtureTransport {
    fn exchange(
        &mut self,
        request: WorkflowExchangeRequest,
    ) -> PortResult<WorkflowExchangeResponse> {
        if self.unavailable {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "fixture_unavailable".to_string(),
                message: "fixture transport is unavailable".to_string(),
            }));
        }
        assert_eq!(
            identity_from_bytes(&request.body),
            Ok(request.body_identity.clone())
        );
        self.exchanges.push(request.operation);
        match request.operation {
            WorkflowExchangeOperation::Dispatch => {
                let workflow_request: WorkflowRequest =
                    serde_json::from_slice(&request.body).unwrap_or_else(|_| unreachable!());
                let accepted = accepted(&workflow_request);
                self.requests.insert(
                    workflow_request.identity.as_str().to_string(),
                    (workflow_request, accepted.clone()),
                );
                Ok(WorkflowExchangeResponse {
                    body: serde_json::to_vec(&accepted).unwrap_or_else(|_| unreachable!()),
                    log_bytes: Vec::new(),
                    disconnected_after_acceptance: self.disconnect_dispatch,
                })
            }
            WorkflowExchangeOperation::RecoverDispatch => {
                let workflow_request: WorkflowRequest =
                    serde_json::from_slice(&request.body).unwrap_or_else(|_| unreachable!());
                let accepted = accepted(&workflow_request);
                self.requests.insert(
                    workflow_request.identity.as_str().to_string(),
                    (workflow_request, accepted.clone()),
                );
                Ok(WorkflowExchangeResponse {
                    body: serde_json::to_vec(&accepted).unwrap_or_else(|_| unreachable!()),
                    log_bytes: Vec::new(),
                    disconnected_after_acceptance: false,
                })
            }
            WorkflowExchangeOperation::Observe
            | WorkflowExchangeOperation::Reconcile
            | WorkflowExchangeOperation::Cancel
            | WorkflowExchangeOperation::Pause
            | WorkflowExchangeOperation::Resume
            | WorkflowExchangeOperation::ReadLog => {
                let accepted: WorkflowAccepted = serde_json::from_slice(&request.body)
                    .unwrap_or_else(|_| {
                        let control = serde_json::from_slice::<
                            lattice_workflow_contract::ControlRequest,
                        >(&request.body)
                        .ok();
                        let log =
                            serde_json::from_slice::<lattice_workflow_contract::LogReadRequest>(
                                &request.body,
                            )
                            .ok();
                        let request_identity = control
                            .as_ref()
                            .map(|value| value.request_identity.as_str())
                            .or_else(|| log.as_ref().map(|value| value.request_identity.as_str()))
                            .unwrap_or_else(|| unreachable!());
                        self.requests
                            .get(request_identity)
                            .map(|(_workflow_request, accepted)| accepted.clone())
                            .unwrap_or_else(|| unreachable!())
                    });
                let (workflow_request, _) = self
                    .requests
                    .get(accepted.request_identity.as_str())
                    .cloned()
                    .unwrap_or_else(|| unreachable!());
                let state = match request.operation {
                    WorkflowExchangeOperation::Cancel => ExecutionState::Cancelled,
                    WorkflowExchangeOperation::Pause => ExecutionState::Paused,
                    WorkflowExchangeOperation::Resume => ExecutionState::Running,
                    WorkflowExchangeOperation::Observe
                    | WorkflowExchangeOperation::Reconcile
                    | WorkflowExchangeOperation::ReadLog => ExecutionState::Completed,
                    WorkflowExchangeOperation::Dispatch
                    | WorkflowExchangeOperation::RecoverDispatch => unreachable!(),
                };
                let include_log = request.operation == WorkflowExchangeOperation::ReadLog;
                self.sequence = self.sequence.saturating_add(1);
                let mut observation = observation(
                    &workflow_request,
                    &accepted,
                    state,
                    include_log,
                    self.sequence,
                );
                if self.stale_observation {
                    observation.execution_identity = identity('2');
                    observation.identity = observation_identity(&observation);
                }
                Ok(WorkflowExchangeResponse {
                    body: serde_json::to_vec(&observation).unwrap_or_else(|_| unreachable!()),
                    log_bytes: if include_log {
                        LOG_BYTES.to_vec()
                    } else {
                        Vec::new()
                    },
                    disconnected_after_acceptance: false,
                })
            }
        }
    }
}

fn identity(character: char) -> Identity {
    Identity::new(format!(
        "b3:{}",
        character.to_string().repeat(IDENTITY_HEX_BYTES)
    ))
    .unwrap_or_else(|_| unreachable!())
}

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn limits() -> LatticeAdapterLimits {
    LatticeAdapterLimits {
        workflow: WorkflowBounds {
            max_nodes: MAX_NODES,
            max_edges: MAX_EDGES,
            max_log_references: MAX_LOG_REFERENCES,
            max_log_read_bytes: MAX_LOG_READ_BYTES,
            max_total_log_bytes: MAX_TOTAL_LOG_BYTES,
        },
        max_request_bytes: MAX_REQUEST_BYTES,
        max_response_bytes: MAX_RESPONSE_BYTES,
        max_in_flight: MAX_IN_FLIGHT,
    }
}

fn profile() -> LatticeProfile {
    LatticeProfile::pinned(
        ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!()),
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn plan() -> WorkflowEffectPlan {
    WorkflowEffectPlan {
        effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
        candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new("pipeline")
            .unwrap_or_else(|_| unreachable!()),
        workflow_policy_id: PolicyId::new("workflow-policy").unwrap_or_else(|_| unreachable!()),
        provider_profile: ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('0'),
        platform: alias(PINNED_LATTICE_PLATFORM),
        source: None,
        nodes: vec![
            WorkflowNode {
                id: alias("build"),
                kind: alias("build"),
                effect_class: WorkflowEffectClass::Build,
                adapter_kind: alias("local"),
                operation_identity: content('1'),
            },
            WorkflowNode {
                id: alias("test"),
                kind: alias("test"),
                effect_class: WorkflowEffectClass::Test,
                adapter_kind: alias("remote"),
                operation_identity: content('2'),
            },
        ],
        edges: vec![WorkflowEdge {
            source: alias("build"),
            target: alias("test"),
        }],
        controls: WorkflowControls {
            cancel: true,
            pause: true,
            resume: true,
        },
        bounds: limits().workflow,
    }
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

fn observation(
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    state: ExecutionState,
    include_log: bool,
    sequence: u64,
) -> ExecutionObservation {
    let terminal = matches!(state, ExecutionState::Completed | ExecutionState::Cancelled);
    let successful = state == ExecutionState::Completed;
    let node_state = match state {
        ExecutionState::Completed => NodeState::Completed,
        ExecutionState::Cancelled => NodeState::Cancelled,
        ExecutionState::Paused => NodeState::Paused,
        ExecutionState::Running => NodeState::Running,
        _ => NodeState::Unknown,
    };
    let log_node = request
        .graph
        .nodes
        .first()
        .map(|node| node.id.clone())
        .unwrap_or_else(|| unreachable!());
    let logs = if include_log {
        vec![LogReference {
            node_id: log_node,
            stream: LogStream::Stdout,
            reference_identity: identity('c'),
            cursor: 0,
            bytes: u64::try_from(LOG_BYTES.len()).unwrap_or_else(|_| unreachable!()),
            truncated: false,
            gap_before: false,
            retained: true,
        }]
    } else {
        Vec::new()
    };
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0'),
        request_identity: request.identity.clone(),
        idempotency_key: request.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence,
        state,
        terminal,
        successful,
        nodes: request
            .graph
            .nodes
            .iter()
            .map(|node| NodeObservation {
                node_id: node.id.clone(),
                state: node_state,
                terminal,
                successful,
                adapter_identity: identity('a'),
                receipt_identity: terminal.then(|| identity('b')),
            })
            .collect(),
        logs,
        receipt_identity: terminal.then(|| identity('d')),
    };
    observation.identity = observation_identity(&observation);
    observation
}

// r[verify kiln.lattice.profile.scenario.match] r[verify kiln.lattice.nonclaims.scenario.scope]
#[test]
fn pinned_profile_is_exact_and_admitted() {
    let profile = profile();
    assert_eq!(profile.source, PINNED_LATTICE_SOURCE);
    assert_eq!(profile.revision, PINNED_LATTICE_REVISION);
    assert_eq!(profile.license, PINNED_LATTICE_CONTRACT_LICENSE);
    assert!(admit_profile(&profile).is_ok());
}

#[test]
fn profile_rejects_missing_operation_and_capability_drift() {
    let mut invalid = profile();
    invalid
        .provider
        .operations
        .remove(&kiln_core::provider::Operation::Observe);
    assert!(admit_profile(&invalid).is_err());
    invalid = profile();
    invalid.contract.controls.pause = false;
    assert!(admit_profile(&invalid).is_err());
}

// r[verify kiln.lattice.request.scenario.valid]
#[test]
fn request_lowering_is_deterministic() {
    let profile = profile();
    let plan = plan();
    assert_eq!(
        lower_request(&profile, &plan),
        lower_request(&profile, &plan)
    );
}

// r[verify kiln.lattice.profile.scenario.node]
#[test]
fn source_context_preserves_mixed_case_provider_identity() {
    let profile = profile();
    let mut plan = plan();
    plan.source = Some(WorkflowSourceContext {
        provider: alias("radicle"),
        repository: ExternalRef::new("radicle", "repository", "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE")
            .unwrap_or_else(|_| unreachable!()),
        revision: ExternalRef::new(
            "radicle",
            "revision",
            "70496e67c7fd4a8b05914161a8e09de2759bebc8",
        )
        .unwrap_or_else(|_| unreachable!()),
    });

    let request = lower_request(&profile, &plan).unwrap_or_else(|_| unreachable!());
    let source = request.source.unwrap_or_else(|| unreachable!());
    assert_eq!(
        source.repository.as_str(),
        "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE"
    );
}

#[test]
fn unsupported_node_fails_before_transport() {
    let mut plan = plan();
    let Some(node) = plan.nodes.first_mut() else {
        unreachable!();
    };
    node.adapter_kind = alias("undeclared");
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    assert!(adapter.dispatch(&plan).is_err());
    assert!(adapter.transport().exchanges.is_empty());
}

// r[verify kiln.lattice.observation.scenario.complete]
#[test]
fn dispatch_and_observe_preserve_linkage_and_complete() {
    let plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    let accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(accepted.state, WorkflowState::Accepted);
    assert_eq!(accepted.outcome, Outcome::Pending);
    let completed = adapter
        .observe(&plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(completed.state, WorkflowState::Completed);
    assert_eq!(completed.outcome, Outcome::Success);
    assert_eq!(completed.attempt_id, plan.attempt_id);
}

// r[verify kiln.lattice.reconciliation.scenario.disconnect]
// r[verify kiln.lattice.transport_recovery]
#[test]
fn recovery_only_dispatch_never_sends_a_new_dispatch() {
    let plan = plan();
    let profile = profile();
    let request = lower_request(&profile, &plan).unwrap_or_else(|_| unreachable!());
    let accepted = accepted(&request);
    let mut transport = FixtureTransport::default();
    transport
        .requests
        .insert(request.identity.as_str().to_string(), (request, accepted));
    let mut adapter = LatticeAdapter::new(profile, transport).unwrap_or_else(|_| unreachable!());

    let recovered = adapter.recover_dispatch(&plan);

    assert!(recovered.is_ok());
    assert_eq!(
        adapter.transport().exchanges,
        vec![WorkflowExchangeOperation::RecoverDispatch]
    );
}

#[test]
fn disconnect_after_acceptance_stays_unknown_with_execution_identity() {
    let plan = plan();
    let transport = FixtureTransport {
        disconnect_dispatch: true,
        ..FixtureTransport::default()
    };
    let mut adapter = LatticeAdapter::new(profile(), transport).unwrap_or_else(|_| unreachable!());
    let accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(accepted.state, WorkflowState::Unknown);
    assert_eq!(accepted.outcome, Outcome::Unknown);
    assert_eq!(accepted.execution.native_kind.as_str(), "execution");
}

// r[verify kiln.lattice.reconciliation.scenario.stale]
#[test]
fn stale_execution_is_rejected_without_state_change() {
    let plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    let _accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    adapter.transport_mut().stale_observation = true;
    assert!(adapter.reconcile(&plan.effect_id).is_err());
    adapter.transport_mut().stale_observation = false;
    assert!(
        adapter
            .reconcile(&plan.effect_id)
            .is_ok_and(|observation| observation.state == WorkflowState::Completed)
    );
}

#[test]
fn pause_and_log_operations_are_bounded() {
    let plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    let accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let paused = adapter
        .control(&WorkflowControlRequest {
            effect_id: plan.effect_id.clone(),
            execution: accepted.execution.clone(),
            expected_sequence: 0,
            action: WorkflowControlAction::Pause,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(paused.state, WorkflowState::Paused);
    let resumed = adapter
        .control(&WorkflowControlRequest {
            effect_id: plan.effect_id.clone(),
            execution: accepted.execution.clone(),
            expected_sequence: FIRST_OBSERVATION_SEQUENCE,
            action: WorkflowControlAction::Resume,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(resumed.state, WorkflowState::Running);
    let chunk = adapter
        .read_log(&WorkflowLogReadRequest {
            effect_id: plan.effect_id.clone(),
            execution: accepted.execution,
            node_id: alias("build"),
            stream: WorkflowLogStream::Stdout,
            cursor: 0,
            max_bytes: MAX_LOG_READ_BYTES,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(chunk.bytes, LOG_BYTES);
    assert_eq!(chunk.reference.bytes, LOG_BYTES.len());
}

#[test]
fn unrequested_control_fails_before_transport() {
    let mut plan = plan();
    plan.controls.pause = false;
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    let accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert!(
        adapter
            .control(&WorkflowControlRequest {
                effect_id: plan.effect_id,
                execution: accepted.execution,
                expected_sequence: 0,
                action: WorkflowControlAction::Pause,
            })
            .is_err()
    );
    assert_eq!(adapter.transport().exchanges.len(), 1);
}

// r[verify kiln.lattice.conformance.scenario.cancel]
#[test]
fn cancellation_returns_a_linked_terminal_observation() {
    let plan = plan();
    let mut adapter = LatticeAdapter::new(profile(), FixtureTransport::default())
        .unwrap_or_else(|_| unreachable!());
    let accepted = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let cancelled = adapter
        .control(&WorkflowControlRequest {
            effect_id: plan.effect_id,
            execution: accepted.execution,
            expected_sequence: 0,
            action: WorkflowControlAction::Cancel,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(cancelled.state, WorkflowState::Cancelled);
    assert_eq!(cancelled.outcome, Outcome::Cancelled);
    assert!(cancelled.terminal);
}

#[derive(serde::Deserialize)]
struct ProducerFixtures {
    profile: lattice_workflow_contract::CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
    observation: ExecutionObservation,
    rejected: RejectedProducerFixtures,
}

#[derive(serde::Deserialize)]
struct RejectedProducerFixtures {
    cyclic_request: WorkflowRequest,
    false_paused_success: ExecutionObservation,
    stale_execution: ExecutionObservation,
}

#[test]
fn pinned_producer_fixtures_pass_independent_consumer_admission() {
    let fixtures: ProducerFixtures = serde_json::from_str(include_str!(
        "../../../fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/producer-fixtures-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        lattice_workflow_contract::validate_profile(&fixtures.profile),
        Ok(())
    );
    assert_eq!(
        lattice_workflow_contract::validate_request(&fixtures.profile, &fixtures.request),
        Ok(())
    );
    assert_eq!(
        lattice_workflow_contract::validate_observation(
            &fixtures.profile,
            &fixtures.request,
            &fixtures.accepted,
            &fixtures.observation,
        ),
        Ok(())
    );
    assert!(
        lattice_workflow_contract::validate_request(
            &fixtures.profile,
            &fixtures.rejected.cyclic_request,
        )
        .is_err()
    );
    assert!(
        lattice_workflow_contract::validate_observation(
            &fixtures.profile,
            &fixtures.request,
            &fixtures.accepted,
            &fixtures.rejected.false_paused_success,
        )
        .is_err()
    );
    assert!(
        lattice_workflow_contract::validate_observation(
            &fixtures.profile,
            &fixtures.request,
            &fixtures.accepted,
            &fixtures.rejected.stale_execution,
        )
        .is_err()
    );
}

#[test]
fn unavailable_provider_fails_without_fallback() {
    let transport = FixtureTransport {
        unavailable: true,
        ..FixtureTransport::default()
    };
    let mut adapter = LatticeAdapter::new(profile(), transport).unwrap_or_else(|_| unreachable!());
    assert!(adapter.dispatch(&plan()).is_err());
    assert_eq!(adapter.transport().exchanges.len(), 0);
}

#[cfg(target_os = "linux")]
mod unix;
