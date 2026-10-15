// Octet: observation admission uses closed wire states and typed errors to reject every
// inconsistent lifecycle claim.
#![allow(
    unknown_lints,
    non_trait_imports,
    path_segment_repetition,
    assertion_density,
    compound_condition,
    missing_const_fn,
    fragile_exhaustive_enum_match,
    reason = "exhaustive versioned states and explicit compound invariants prevent false terminal or success observations"
)]

use crate::model::CapabilityProfile;
use crate::model::ContractError;
use crate::model::ErrorCode;
use crate::model::ExecutionObservation;
use crate::model::ExecutionState;
use crate::model::NodeState;
use crate::model::OBSERVATION_SCHEMA;
use crate::model::WorkflowAccepted;
use crate::model::WorkflowRequest;

// r[impl lattice.workflow_interchange.observation]
pub fn validate_observation(
    profile: &CapabilityProfile,
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    observation: &ExecutionObservation,
) -> Result<(), ContractError> {
    crate::validate_request(profile, request)?;
    crate::validate_accepted(request, accepted)?;
    if observation.schema != OBSERVATION_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "observation.schema"));
    }
    if observation.request_identity != accepted.request_identity
        || observation.idempotency_key != accepted.idempotency_key
        || observation.workflow_identity != accepted.workflow_identity
        || observation.execution_identity != accepted.execution_identity
    {
        return Err(error(ErrorCode::ObservationLinkageMismatch, "observation.linkage"));
    }
    validate_node_observations(request, observation)?;
    validate_execution_state(observation)?;
    validate_logs(request, observation)?;
    if observation.identity != crate::observation_identity(observation) {
        return Err(error(ErrorCode::ObservationIdentityMismatch, "observation.identity"));
    }
    Ok(())
}

fn validate_node_observations(
    request: &WorkflowRequest,
    observation: &ExecutionObservation,
) -> Result<(), ContractError> {
    if observation.nodes.len() != request.graph.nodes.len() {
        return Err(error(ErrorCode::ObservationNodeMismatch, "observation.nodes"));
    }
    for (expected, observed) in request.graph.nodes.iter().zip(&observation.nodes) {
        if observed.node_id != expected.id {
            return Err(error(ErrorCode::ObservationOrder, "observation.nodes.node_id"));
        }
        if observed.terminal != observed.state.is_terminal() || observed.successful != observed.state.is_successful() {
            return Err(error(ErrorCode::ObservationStateMismatch, "observation.nodes.state"));
        }
        let is_receipt_required =
            matches!(observed.state, NodeState::Completed | NodeState::Failed | NodeState::Cancelled);
        if is_receipt_required != observed.receipt_identity.is_some() {
            return Err(error(ErrorCode::ObservationStateMismatch, "observation.nodes.receipt_identity"));
        }
    }
    Ok(())
}

fn validate_execution_state(observation: &ExecutionObservation) -> Result<(), ContractError> {
    if observation.terminal != observation.state.is_terminal() {
        return Err(error(ErrorCode::ObservationStateMismatch, "observation.terminal"));
    }
    let is_all_terminal = observation.nodes.iter().all(|node| node.terminal);
    let is_all_successful = observation.nodes.iter().all(|node| node.successful);
    let has_failed = observation.nodes.iter().any(|node| node.state == NodeState::Failed);
    let has_cancelled = observation.nodes.iter().any(|node| node.state == NodeState::Cancelled);
    let is_state_valid = match observation.state {
        ExecutionState::Completed => is_all_terminal && is_all_successful && observation.successful,
        ExecutionState::Failed => is_all_terminal && has_failed && !observation.successful,
        ExecutionState::Cancelled => is_all_terminal && has_cancelled && !observation.successful,
        ExecutionState::Accepted
        | ExecutionState::Queued
        | ExecutionState::Running
        | ExecutionState::Paused
        | ExecutionState::Unknown => !observation.terminal && !observation.successful,
    };
    if !is_state_valid || observation.state.is_terminal() != observation.receipt_identity.is_some() {
        return Err(error(ErrorCode::ObservationStateMismatch, "observation.state"));
    }
    Ok(())
}

fn validate_logs(request: &WorkflowRequest, observation: &ExecutionObservation) -> Result<(), ContractError> {
    let log_count =
        u32::try_from(observation.logs.len()).map_err(|_| error(ErrorCode::ObservationLimit, "observation.logs"))?;
    if log_count > request.observation.maximum_log_references {
        return Err(error(ErrorCode::ObservationLimit, "observation.logs"));
    }
    let mut previous = None;
    let mut total_bytes = 0_u64;
    for log in &observation.logs {
        let is_node_present = request.graph.nodes.iter().any(|node| node.id == log.node_id);
        if !is_node_present {
            return Err(error(ErrorCode::ObservationNodeMismatch, "observation.logs.node_id"));
        }
        if let Some((node_id, stream, cursor)) = previous
            && (node_id, stream, cursor) >= (&log.node_id, log.stream, log.cursor)
        {
            return Err(error(ErrorCode::ObservationOrder, "observation.logs"));
        }
        previous = Some((&log.node_id, log.stream, log.cursor));
        if log.bytes > request.observation.maximum_log_read_bytes {
            return Err(error(ErrorCode::ObservationLimit, "observation.logs.bytes"));
        }
        total_bytes = total_bytes
            .checked_add(log.bytes)
            .ok_or_else(|| error(ErrorCode::ObservationLimit, "observation.logs.bytes"))?;
    }
    if total_bytes > request.observation.maximum_total_log_bytes {
        return Err(error(ErrorCode::ObservationLimit, "observation.logs.total_bytes"));
    }
    Ok(())
}

const fn error(code: ErrorCode, path: &'static str) -> ContractError {
    ContractError { code, path }
}
