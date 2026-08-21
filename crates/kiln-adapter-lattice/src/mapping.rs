use kiln_codec::identity_from_bytes;
use kiln_core::domain::Outcome;
use kiln_core::types::{Alias, ContentId, Diagnostic};
use kiln_core::workflow::{
    WorkflowEffectClass, WorkflowEffectPlan, WorkflowLogReference, WorkflowLogStream,
    WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation, WorkflowState,
    admit_workflow_observation, validate_workflow_plan,
};
use kiln_provider_api::{PortFailure, PortResult};
use lattice_workflow_contract::{
    ExecutionObservation, ExecutionState, GRAPH_SCHEMA, Identity, KilnLinkage, Label, LogReference,
    LogStream, NodeObservation, NodeState, ObservationPolicy, REQUEST_SCHEMA, RequestedControls,
    SourceContext, SourceValue, WorkflowAccepted, WorkflowEdge, WorkflowGraph, WorkflowNode,
    WorkflowRequest, graph_identity, profile_identity, request_identity, validate_accepted,
    validate_observation, validate_request,
};

use crate::profile::LatticeProfile;

const ZERO_IDENTITY_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// r[impl kiln.lattice.request]
pub fn lower_request(
    profile: &LatticeProfile,
    plan: &WorkflowEffectPlan,
) -> PortResult<WorkflowRequest> {
    crate::profile::admit_profile(profile)?;
    validate_workflow_plan(plan)?;
    if plan.provider_profile != profile.provider.profile_id
        || plan.bounds != profile.limits.workflow
    {
        return Err(domain(
            "lattice_plan_profile",
            "lattice.plan",
            "workflow plan differs from the selected Lattice profile",
        ));
    }
    let mut graph = WorkflowGraph {
        schema: GRAPH_SCHEMA.to_string(),
        identity: zero_identity()?,
        nodes: plan
            .nodes
            .iter()
            .map(|node| {
                Ok(WorkflowNode {
                    id: label(node.id.as_str())?,
                    kind: label(node.kind.as_str())?,
                    effect_class: effect_class(node.effect_class),
                    adapter_kind: label(node.adapter_kind.as_str())?,
                    operation_identity: content_identity(&node.operation_identity)?,
                })
            })
            .collect::<PortResult<Vec<_>>>()?,
        edges: plan
            .edges
            .iter()
            .map(|edge| {
                Ok(WorkflowEdge {
                    source: label(edge.source.as_str())?,
                    target: label(edge.target.as_str())?,
                })
            })
            .collect::<PortResult<Vec<_>>>()?,
    };
    graph.identity = graph_identity(&graph);
    let mut request = WorkflowRequest {
        schema: REQUEST_SCHEMA.to_string(),
        identity: zero_identity()?,
        idempotency_key: content_identity(&plan.idempotency_key)?,
        profile_identity: profile_identity(&profile.contract),
        platform: label(plan.platform.as_str())?,
        kiln: KilnLinkage {
            effect_identity: text_identity(plan.effect_id.as_str())?,
            attempt_identity: text_identity(plan.attempt_id.as_str())?,
            candidate_identity: text_identity(plan.candidate_id.as_str())?,
            pipeline_revision_identity: text_identity(plan.pipeline_revision_id.as_str())?,
            workflow_policy_identity: text_identity(plan.workflow_policy_id.as_str())?,
        },
        source: plan
            .source
            .as_ref()
            .map(|source| -> PortResult<SourceContext> {
                Ok(SourceContext {
                    provider: label(source.provider.as_str())?,
                    repository: source_value(source.repository.native_id.as_str())?,
                    revision: source_value(source.revision.native_id.as_str())?,
                })
            })
            .transpose()?,
        graph,
        controls: RequestedControls {
            cancel: plan.controls.cancel,
            pause: plan.controls.pause,
            resume: plan.controls.resume,
        },
        observation: ObservationPolicy {
            maximum_log_references: to_u32(
                plan.bounds.max_log_references,
                "lattice.observation.maximum_log_references",
            )?,
            maximum_log_read_bytes: to_u64(
                plan.bounds.max_log_read_bytes,
                "lattice.observation.maximum_log_read_bytes",
            )?,
            maximum_total_log_bytes: to_u64(
                plan.bounds.max_total_log_bytes,
                "lattice.observation.maximum_total_log_bytes",
            )?,
        },
    };
    request.identity = request_identity(&request);
    validate_request(&profile.contract, &request).map_err(contract_error)?;
    Ok(request)
}

pub fn body_identity(bytes: &[u8]) -> PortResult<ContentId> {
    identity_from_bytes(bytes).map_err(|_| {
        infrastructure(
            "lattice_body_identity",
            "Lattice exchange body identity could not be computed",
        )
    })
}

pub fn accepted_observation(
    plan: &WorkflowEffectPlan,
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    disconnected_after_acceptance: bool,
) -> PortResult<WorkflowObservation> {
    validate_accepted(request, accepted).map_err(contract_error)?;
    let (state, outcome) = if disconnected_after_acceptance {
        (WorkflowState::Unknown, Outcome::Unknown)
    } else {
        (WorkflowState::Accepted, Outcome::Pending)
    };
    let observation = WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: external("workflow", &accepted.workflow_identity)?,
        execution: external("execution", &accepted.execution_identity)?,
        sequence: 0,
        state,
        terminal: false,
        successful: false,
        outcome,
        nodes: Vec::new(),
        logs: Vec::new(),
        receipt: None,
    };
    let _outcome = admit_workflow_observation(plan, &observation)?;
    Ok(observation)
}

// r[impl kiln.lattice.observation] r[impl kiln.lattice.reconciliation]
pub fn admitted_observation(
    profile: &LatticeProfile,
    plan: &WorkflowEffectPlan,
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    observation: &ExecutionObservation,
) -> PortResult<WorkflowObservation> {
    validate_observation(&profile.contract, request, accepted, observation)
        .map_err(contract_error)?;
    let mapped = WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: external("workflow", &observation.workflow_identity)?,
        execution: external("execution", &observation.execution_identity)?,
        sequence: observation.sequence,
        state: execution_state(observation.state),
        terminal: observation.terminal,
        successful: observation.successful,
        outcome: execution_outcome(observation.state),
        nodes: observation
            .nodes
            .iter()
            .map(node_observation)
            .collect::<PortResult<Vec<_>>>()?,
        logs: observation
            .logs
            .iter()
            .map(log_reference)
            .collect::<PortResult<Vec<_>>>()?,
        receipt: observation
            .receipt_identity
            .as_ref()
            .map(|identity| external("receipt", identity))
            .transpose()?,
    };
    let _outcome = admit_workflow_observation(plan, &mapped)?;
    Ok(mapped)
}

pub fn matching_log_reference(
    observation: &ExecutionObservation,
    node_id: &Alias,
    stream: WorkflowLogStream,
    cursor: u64,
) -> PortResult<WorkflowLogReference> {
    observation
        .logs
        .iter()
        .find(|log| {
            log.node_id.as_str() == node_id.as_str()
                && log_stream(log.stream) == stream
                && log.cursor == cursor
        })
        .map(log_reference)
        .transpose()?
        .ok_or_else(|| {
            domain(
                "lattice_log_reference",
                "lattice.log",
                "Lattice observation omitted the requested log reference",
            )
        })
}

fn node_observation(observation: &NodeObservation) -> PortResult<WorkflowNodeObservation> {
    Ok(WorkflowNodeObservation {
        node_id: Alias::new(observation.node_id.as_str())?,
        state: node_state(observation.state),
        terminal: observation.terminal,
        successful: observation.successful,
        adapter: external("adapter", &observation.adapter_identity)?,
        receipt: observation
            .receipt_identity
            .as_ref()
            .map(|identity| external("receipt", identity))
            .transpose()?,
    })
}

fn log_reference(reference: &LogReference) -> PortResult<WorkflowLogReference> {
    Ok(WorkflowLogReference {
        node_id: Alias::new(reference.node_id.as_str())?,
        stream: log_stream(reference.stream),
        reference: external("log", &reference.reference_identity)?,
        cursor: reference.cursor,
        bytes: usize::try_from(reference.bytes).map_err(|_| {
            domain(
                "lattice_log_bytes",
                "lattice.log.bytes",
                "Lattice log byte count does not fit the active platform",
            )
        })?,
        truncated: reference.truncated,
        gap_before: reference.gap_before,
        retained: reference.retained,
    })
}

fn execution_state(state: ExecutionState) -> WorkflowState {
    match state {
        ExecutionState::Accepted => WorkflowState::Accepted,
        ExecutionState::Queued => WorkflowState::Queued,
        ExecutionState::Running => WorkflowState::Running,
        ExecutionState::Paused => WorkflowState::Paused,
        ExecutionState::Completed => WorkflowState::Completed,
        ExecutionState::Failed => WorkflowState::Failed,
        ExecutionState::Cancelled => WorkflowState::Cancelled,
        ExecutionState::Unknown => WorkflowState::Unknown,
    }
}

fn execution_outcome(state: ExecutionState) -> Outcome {
    match state {
        ExecutionState::Accepted
        | ExecutionState::Queued
        | ExecutionState::Running
        | ExecutionState::Paused => Outcome::Pending,
        ExecutionState::Completed => Outcome::Success,
        ExecutionState::Failed => Outcome::DomainFailure,
        ExecutionState::Cancelled => Outcome::Cancelled,
        ExecutionState::Unknown => Outcome::Unknown,
    }
}

fn node_state(state: NodeState) -> WorkflowNodeState {
    match state {
        NodeState::Pending => WorkflowNodeState::Pending,
        NodeState::Running => WorkflowNodeState::Running,
        NodeState::Paused => WorkflowNodeState::Paused,
        NodeState::Completed => WorkflowNodeState::Completed,
        NodeState::Failed => WorkflowNodeState::Failed,
        NodeState::Cancelled => WorkflowNodeState::Cancelled,
        NodeState::Skipped => WorkflowNodeState::Skipped,
        NodeState::Unknown => WorkflowNodeState::Unknown,
    }
}

pub fn lattice_stream(stream: WorkflowLogStream) -> LogStream {
    match stream {
        WorkflowLogStream::Stdout => LogStream::Stdout,
        WorkflowLogStream::Stderr => LogStream::Stderr,
        WorkflowLogStream::Diagnostic => LogStream::Diagnostic,
    }
}

fn log_stream(stream: LogStream) -> WorkflowLogStream {
    match stream {
        LogStream::Stdout => WorkflowLogStream::Stdout,
        LogStream::Stderr => WorkflowLogStream::Stderr,
        LogStream::Diagnostic => WorkflowLogStream::Diagnostic,
    }
}

fn effect_class(class: WorkflowEffectClass) -> lattice_workflow_contract::EffectClass {
    match class {
        WorkflowEffectClass::Read => lattice_workflow_contract::EffectClass::Read,
        WorkflowEffectClass::Build => lattice_workflow_contract::EffectClass::Build,
        WorkflowEffectClass::Test => lattice_workflow_contract::EffectClass::Test,
        WorkflowEffectClass::Publish => lattice_workflow_contract::EffectClass::Publish,
        WorkflowEffectClass::Notify => lattice_workflow_contract::EffectClass::Notify,
        WorkflowEffectClass::Custom => lattice_workflow_contract::EffectClass::Custom,
    }
}

fn text_identity(value: &str) -> PortResult<Identity> {
    let content = identity_from_bytes(value.as_bytes()).map_err(|_| {
        domain(
            "lattice_identity",
            "lattice.linkage",
            "Kiln linkage identity could not be computed",
        )
    })?;
    content_identity(&content)
}

fn content_identity(value: &ContentId) -> PortResult<Identity> {
    Identity::new(format!("b3:{}", value.as_str())).map_err(|_| {
        domain(
            "lattice_identity",
            "lattice.linkage",
            "Kiln content identity is not a Lattice contract identity",
        )
    })
}

fn zero_identity() -> PortResult<Identity> {
    Identity::new(format!("b3:{ZERO_IDENTITY_HEX}")).map_err(|_| {
        infrastructure(
            "lattice_zero_identity",
            "Lattice placeholder identity is invalid",
        )
    })
}

fn source_value(value: &str) -> PortResult<SourceValue> {
    SourceValue::new(value.to_string()).map_err(|_| {
        domain(
            "lattice_source_value",
            "lattice.request.source",
            "source value exceeds the contract bound or contains control bytes",
        )
    })
}

fn label(value: &str) -> PortResult<Label> {
    Label::new(value.to_string()).map_err(|_| {
        domain(
            "lattice_label",
            "lattice.graph",
            "Kiln graph label is not supported by the Lattice contract",
        )
    })
}

fn external(kind: &str, identity: &Identity) -> PortResult<kiln_core::types::ExternalRef> {
    kiln_core::types::ExternalRef::new("lattice", kind, identity.as_str())
        .map_err(PortFailure::Domain)
}

fn to_u32(value: usize, field: &'static str) -> PortResult<u32> {
    u32::try_from(value).map_err(|_| {
        domain(
            "lattice_limit",
            field,
            "Lattice count does not fit the contract",
        )
    })
}

fn to_u64(value: usize, field: &'static str) -> PortResult<u64> {
    u64::try_from(value).map_err(|_| {
        domain(
            "lattice_limit",
            field,
            "Lattice byte bound does not fit the contract",
        )
    })
}

fn contract_error(error: lattice_workflow_contract::ContractError) -> PortFailure {
    domain(
        "lattice_contract",
        error.path,
        "Lattice value failed contract admission",
    )
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}
