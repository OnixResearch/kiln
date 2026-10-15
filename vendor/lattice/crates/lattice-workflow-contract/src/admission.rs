// Octet: this bounded validator keeps concrete contract names visible and returns typed errors for
// every rejected invariant.
#![allow(
    unknown_lints,
    non_trait_imports,
    assertion_density,
    compound_condition,
    missing_const_fn,
    bool_naming,
    fragile_exhaustive_enum_match,
    numeric_units,
    reason = "profile limits bound every collection before allocation; exhaustive wire enums and compound checks fail closed"
)]

mod graph;

use crate::model::ACCEPTED_SCHEMA;
use crate::model::CONTRACT_MAXIMUM_CAPABILITY_ITEMS;
use crate::model::CONTRACT_MAXIMUM_EDGES;
use crate::model::CONTRACT_MAXIMUM_LOG_READ_BYTES;
use crate::model::CONTRACT_MAXIMUM_LOG_REFERENCES;
use crate::model::CONTRACT_MAXIMUM_NODES;
use crate::model::CONTRACT_MAXIMUM_TOTAL_LOG_BYTES;
use crate::model::CONTROL_SCHEMA;
use crate::model::CapabilityProfile;
use crate::model::ContractError;
use crate::model::ControlAction;
use crate::model::ControlRequest;
use crate::model::ErrorCode;
use crate::model::GRAPH_SCHEMA;
use crate::model::LOG_READ_SCHEMA;
use crate::model::LogReadRequest;
use crate::model::REQUEST_SCHEMA;
use crate::model::REQUIRED_NON_CLAIMS;
use crate::model::WorkflowAccepted;
use crate::model::WorkflowRequest;

pub fn validate_profile(profile: &CapabilityProfile) -> Result<(), ContractError> {
    if profile.schema != crate::model::PROFILE_SCHEMA || profile.graph_schema != GRAPH_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "profile.schema"));
    }
    if profile.platforms.is_empty()
        || profile.node_kinds.is_empty()
        || profile.effect_classes.is_empty()
        || profile.adapter_kinds.is_empty()
    {
        return Err(error(ErrorCode::EmptyCapability, "profile.capabilities"));
    }
    if exceeds_u32(profile.platforms.len(), CONTRACT_MAXIMUM_CAPABILITY_ITEMS)
        || exceeds_u32(profile.node_kinds.len(), CONTRACT_MAXIMUM_CAPABILITY_ITEMS)
        || exceeds_u32(profile.effect_classes.len(), CONTRACT_MAXIMUM_CAPABILITY_ITEMS)
        || exceeds_u32(profile.adapter_kinds.len(), CONTRACT_MAXIMUM_CAPABILITY_ITEMS)
    {
        return Err(error(ErrorCode::LimitInvalid, "profile.capabilities"));
    }
    if !strictly_ordered(&profile.platforms)
        || !strictly_ordered(&profile.node_kinds)
        || !strictly_ordered(&profile.effect_classes)
        || !strictly_ordered(&profile.adapter_kinds)
    {
        return Err(error(ErrorCode::CapabilityOrder, "profile.capabilities"));
    }
    let limits = profile.limits;
    if limits.maximum_nodes == 0
        || limits.maximum_nodes > CONTRACT_MAXIMUM_NODES
        || limits.maximum_edges == 0
        || limits.maximum_edges > CONTRACT_MAXIMUM_EDGES
        || limits.maximum_log_references == 0
        || limits.maximum_log_references > CONTRACT_MAXIMUM_LOG_REFERENCES
        || limits.maximum_log_read_bytes == 0
        || limits.maximum_log_read_bytes > CONTRACT_MAXIMUM_LOG_READ_BYTES
        || limits.maximum_total_log_bytes == 0
        || limits.maximum_total_log_bytes > CONTRACT_MAXIMUM_TOTAL_LOG_BYTES
        || limits.maximum_log_read_bytes > limits.maximum_total_log_bytes
    {
        return Err(error(ErrorCode::LimitInvalid, "profile.limits"));
    }
    if profile.non_claims.len() != REQUIRED_NON_CLAIMS.len()
        || profile.non_claims.iter().zip(REQUIRED_NON_CLAIMS).any(|(observed, required)| observed != required)
    {
        return Err(error(ErrorCode::ProfileMismatch, "profile.non_claims"));
    }
    Ok(())
}

// r[impl lattice.workflow_interchange.request]
pub fn validate_request(profile: &CapabilityProfile, request: &WorkflowRequest) -> Result<(), ContractError> {
    validate_profile(profile)?;
    if request.schema != REQUEST_SCHEMA || request.graph.schema != GRAPH_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "request.schema"));
    }
    if request.profile_identity != crate::profile_identity(profile) {
        return Err(error(ErrorCode::ProfileMismatch, "request.profile_identity"));
    }
    if !profile.platforms.contains(&request.platform) {
        return Err(error(ErrorCode::UnsupportedPlatform, "request.platform"));
    }
    validate_controls(profile, request)?;
    validate_observation_policy(profile, request)?;
    graph::validate(profile, &request.graph)?;
    if request.graph.identity != crate::graph_identity(&request.graph) {
        return Err(error(ErrorCode::GraphIdentityMismatch, "request.graph.identity"));
    }
    if request.identity != crate::request_identity(request) {
        return Err(error(ErrorCode::RequestIdentityMismatch, "request.identity"));
    }
    Ok(())
}

pub fn validate_accepted(request: &WorkflowRequest, accepted: &WorkflowAccepted) -> Result<(), ContractError> {
    if accepted.schema != ACCEPTED_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "accepted.schema"));
    }
    if accepted.request_identity != request.identity || accepted.idempotency_key != request.idempotency_key {
        return Err(error(ErrorCode::AcceptedLinkageMismatch, "accepted.request_linkage"));
    }
    Ok(())
}

fn validate_controls(profile: &CapabilityProfile, request: &WorkflowRequest) -> Result<(), ContractError> {
    let requested = request.controls;
    let supported = profile.controls;
    if (requested.cancel && !supported.cancel)
        || (requested.pause && !supported.pause)
        || (requested.resume && !supported.resume)
    {
        return Err(error(ErrorCode::UnsupportedControl, "request.controls"));
    }
    Ok(())
}

fn validate_observation_policy(profile: &CapabilityProfile, request: &WorkflowRequest) -> Result<(), ContractError> {
    let policy = request.observation;
    let limits = profile.limits;
    if policy.maximum_log_references == 0
        || policy.maximum_log_references > limits.maximum_log_references
        || policy.maximum_log_read_bytes == 0
        || policy.maximum_log_read_bytes > limits.maximum_log_read_bytes
        || policy.maximum_total_log_bytes == 0
        || policy.maximum_total_log_bytes > limits.maximum_total_log_bytes
        || policy.maximum_log_read_bytes > policy.maximum_total_log_bytes
    {
        return Err(error(ErrorCode::LimitInvalid, "request.observation"));
    }
    Ok(())
}

// r[impl lattice.workflow_interchange.controls_logs]
pub fn validate_control_request(
    profile: &CapabilityProfile,
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    latest_sequence: u64,
    control: &ControlRequest,
) -> Result<(), ContractError> {
    validate_request(profile, request)?;
    validate_accepted(request, accepted)?;
    if control.schema != CONTROL_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "control.schema"));
    }
    if control.request_identity != accepted.request_identity
        || control.workflow_identity != accepted.workflow_identity
        || control.execution_identity != accepted.execution_identity
        || control.expected_sequence != latest_sequence
    {
        return Err(error(ErrorCode::ControlLinkageMismatch, "control.linkage"));
    }
    let supported = match control.action {
        ControlAction::Cancel => profile.controls.cancel,
        ControlAction::Pause => profile.controls.pause,
        ControlAction::Resume => profile.controls.resume,
    };
    if !supported {
        return Err(error(ErrorCode::UnsupportedControl, "control.action"));
    }
    Ok(())
}

pub fn validate_log_read_request(
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    log_read: &LogReadRequest,
) -> Result<(), ContractError> {
    validate_accepted(request, accepted)?;
    if log_read.schema != LOG_READ_SCHEMA {
        return Err(error(ErrorCode::SchemaMismatch, "log_read.schema"));
    }
    let node_exists = request.graph.nodes.iter().any(|node| node.id == log_read.node_id);
    if log_read.request_identity != accepted.request_identity
        || log_read.workflow_identity != accepted.workflow_identity
        || log_read.execution_identity != accepted.execution_identity
        || !node_exists
    {
        return Err(error(ErrorCode::LogReadLinkageMismatch, "log_read.linkage"));
    }
    if log_read.maximum_bytes == 0 || log_read.maximum_bytes > request.observation.maximum_log_read_bytes {
        return Err(error(ErrorCode::ObservationLimit, "log_read.maximum_bytes"));
    }
    Ok(())
}

fn strictly_ordered<T: Ord>(values: &[T]) -> bool {
    ordered_with(values, |previous, current| previous < current)
}

fn ordered_with<T>(values: &[T], compare: impl Fn(&T, &T) -> bool) -> bool {
    let mut values = values.iter();
    let Some(mut previous) = values.next() else {
        return true;
    };
    for current in values {
        if !compare(previous, current) {
            return false;
        }
        previous = current;
    }
    true
}

pub(super) fn exceeds_u32(length: usize, maximum: u32) -> bool {
    match u32::try_from(length) {
        Ok(length) => length > maximum,
        Err(_) => true,
    }
}

const fn error(code: ErrorCode, path: &'static str) -> ContractError {
    ContractError { code, path }
}
