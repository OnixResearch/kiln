#![allow(
    unknown_lints,
    non_trait_imports,
    reason = "the transport core keeps producer-contract validators visible at each admission boundary"
)]

use lattice_workflow_contract::Identity;
use lattice_workflow_contract::Label;
use lattice_workflow_contract::validate_accepted;
use lattice_workflow_contract::validate_control_request;
use lattice_workflow_contract::validate_log_read_request;
use lattice_workflow_contract::validate_observation;
use lattice_workflow_contract::validate_profile;
use lattice_workflow_contract::validate_request;

use crate::AdmissionError;
use crate::EXCHANGE_REQUEST_SCHEMA;
use crate::EXCHANGE_RESPONSE_SCHEMA;
use crate::ExchangeCommand;
use crate::ExchangeRequest;
use crate::ExchangeResponse;
use crate::ExchangeResult;
use crate::ExchangeSuccess;
use crate::MAXIMUM_ALLOWED_USERS;
use crate::MAXIMUM_CONNECTIONS;
use crate::MAXIMUM_DEADLINE_MILLISECONDS;
use crate::MAXIMUM_FRAME_BYTES;
use crate::MAXIMUM_LOG_BYTES;
use crate::REQUIRED_NON_CLAIMS;
use crate::Rejection;
use crate::TRANSPORT_PROFILE_SCHEMA;
use crate::TransportProfile;

const REQUEST_IDENTITY_DOMAIN: &[u8] = b"lattice.workflow-exchange-request-identity.v1";
const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"lattice.workflow-exchange-response-identity.v1";
const BLAKE3_PREFIX: &str = "b3:";

// r[impl lattice.workflow_interchange.transport_identity]
pub fn request_identity(command: &ExchangeCommand) -> Result<Identity, AdmissionError> {
    identity_for(REQUEST_IDENTITY_DOMAIN, command)
}

// r[impl lattice.workflow_interchange.transport_identity]
pub fn response_identity(request_identity: &Identity, result: &ExchangeResult) -> Result<Identity, AdmissionError> {
    identity_for(RESPONSE_IDENTITY_DOMAIN, &(request_identity, result))
}

pub fn new_request(command: ExchangeCommand) -> Result<ExchangeRequest, AdmissionError> {
    let identity = request_identity(&command)?;
    Ok(ExchangeRequest {
        schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
        identity,
        command,
    })
}

pub fn new_response(request_identity: Identity, result: ExchangeResult) -> Result<ExchangeResponse, AdmissionError> {
    let identity = response_identity(&request_identity, &result)?;
    Ok(ExchangeResponse {
        schema: EXCHANGE_RESPONSE_SCHEMA.to_string(),
        identity,
        request_identity,
        result,
    })
}

// r[impl lattice.workflow_interchange.transport_bounds]
// r[impl lattice.workflow_interchange.transport_peer]
pub fn validate_transport_profile(profile: &TransportProfile) -> Result<(), AdmissionError> {
    if profile.schema != TRANSPORT_PROFILE_SCHEMA {
        return Err(AdmissionError::ProfileSchema);
    }
    let limits = profile.limits;
    let has_positive_limits = limits.maximum_frame_bytes > 0
        && limits.maximum_log_bytes > 0
        && limits.maximum_connections > 0
        && limits.read_deadline_milliseconds > 0
        && limits.write_deadline_milliseconds > 0;
    let is_within_contract_limits = limits.maximum_frame_bytes <= MAXIMUM_FRAME_BYTES
        && limits.maximum_log_bytes <= MAXIMUM_LOG_BYTES
        && limits.maximum_connections <= MAXIMUM_CONNECTIONS
        && limits.read_deadline_milliseconds <= MAXIMUM_DEADLINE_MILLISECONDS
        && limits.write_deadline_milliseconds <= MAXIMUM_DEADLINE_MILLISECONDS;
    if !has_positive_limits || !is_within_contract_limits {
        return Err(AdmissionError::ProfileLimit);
    }
    let user_count = u32::try_from(profile.peers.allowed_user_ids.len()).map_err(|_| AdmissionError::PeerPolicy)?;
    if user_count == 0 || user_count > MAXIMUM_ALLOWED_USERS {
        return Err(AdmissionError::PeerPolicy);
    }
    let is_user_list_ordered = profile.peers.allowed_user_ids.windows(2).all(|pair| match pair {
        [left, right] => left < right,
        _ => false,
    });
    if !is_user_list_ordered {
        return Err(AdmissionError::PeerPolicy);
    }
    let has_non_claims = REQUIRED_NON_CLAIMS
        .iter()
        .all(|required| profile.non_claims.iter().any(|declared| declared == required));
    if !has_non_claims {
        return Err(AdmissionError::ProfileSchema);
    }
    debug_assert_eq!(profile.schema, TRANSPORT_PROFILE_SCHEMA);
    debug_assert!(!profile.peers.allowed_user_ids.is_empty());
    Ok(())
}

// r[impl lattice.workflow_interchange.transport_envelope]
// r[impl lattice.workflow_interchange.transport_handler]
pub fn validate_exchange_request(request: &ExchangeRequest) -> Result<(), AdmissionError> {
    if request.schema != EXCHANGE_REQUEST_SCHEMA {
        return Err(AdmissionError::RequestSchema);
    }
    let computed = request_identity(&request.command)?;
    if computed != request.identity {
        return Err(AdmissionError::RequestIdentity);
    }
    validate_command(&request.command)
}

fn validate_command(command: &ExchangeCommand) -> Result<(), AdmissionError> {
    match command {
        ExchangeCommand::Dispatch { profile, request } | ExchangeCommand::RecoverDispatch { profile, request } => {
            validate_profile(profile).map_err(|_| AdmissionError::Contract)?;
            validate_request(profile, request).map_err(|_| AdmissionError::Contract)
        }
        ExchangeCommand::Observe {
            profile,
            request,
            accepted,
        }
        | ExchangeCommand::Reconcile {
            profile,
            request,
            accepted,
        } => {
            validate_request(profile, request).map_err(|_| AdmissionError::Contract)?;
            validate_accepted(request, accepted).map_err(|_| AdmissionError::Contract)
        }
        ExchangeCommand::Control {
            profile,
            request,
            accepted,
            current_sequence,
            control,
        } => validate_control_request(profile, request, accepted, *current_sequence, control)
            .map_err(|_| AdmissionError::Contract),
        ExchangeCommand::ReadLog {
            profile,
            request,
            accepted,
            log_read,
        } => {
            validate_request(profile, request).map_err(|_| AdmissionError::Contract)?;
            validate_log_read_request(request, accepted, log_read).map_err(|_| AdmissionError::Contract)
        }
    }
}

// r[impl lattice.workflow_interchange.transport_response]
pub fn validate_exchange_response(
    request: &ExchangeRequest,
    response: &ExchangeResponse,
) -> Result<(), AdmissionError> {
    if response.schema != EXCHANGE_RESPONSE_SCHEMA {
        return Err(AdmissionError::ResponseSchema);
    }
    if response.request_identity != request.identity {
        return Err(AdmissionError::ResponseRequest);
    }
    let computed = response_identity(&response.request_identity, &response.result)?;
    if computed != response.identity {
        return Err(AdmissionError::ResponseIdentity);
    }
    match &response.result {
        ExchangeResult::Success(success) => validate_success(&request.command, success.as_ref()),
        ExchangeResult::Rejected(rejection) => validate_rejection(rejection),
    }
}

pub fn validate_response_limits(profile: &TransportProfile, response: &ExchangeResponse) -> Result<(), AdmissionError> {
    validate_transport_profile(profile)?;
    if let ExchangeResult::Success(success) = &response.result
        && let ExchangeSuccess::Log { bytes, .. } = success.as_ref()
    {
        let byte_count_bytes = u32::try_from(bytes.len()).map_err(|_| AdmissionError::LogBound)?;
        if byte_count_bytes > profile.limits.maximum_log_bytes {
            return Err(AdmissionError::LogBound);
        }
    }
    Ok(())
}

// Contract validators are not const, so this pure dispatcher cannot become const.
#[allow(
    unknown_lints,
    missing_const_fn,
    reason = "the dispatcher calls producer contract validators that are intentionally runtime functions"
)]
fn validate_success(command: &ExchangeCommand, success: &ExchangeSuccess) -> Result<(), AdmissionError> {
    match (command, success) {
        (
            ExchangeCommand::Dispatch { request, .. } | ExchangeCommand::RecoverDispatch { request, .. },
            ExchangeSuccess::Accepted(accepted),
        ) => validate_accepted(request, accepted).map_err(|_| AdmissionError::Contract),
        (
            ExchangeCommand::Observe {
                profile,
                request,
                accepted,
            }
            | ExchangeCommand::Reconcile {
                profile,
                request,
                accepted,
            }
            | ExchangeCommand::Control {
                profile,
                request,
                accepted,
                ..
            },
            ExchangeSuccess::Observation(observation),
        ) => validate_observation(profile, request, accepted, observation).map_err(|_| AdmissionError::Contract),
        (
            ExchangeCommand::ReadLog {
                profile,
                request,
                accepted,
                log_read,
            },
            ExchangeSuccess::Log { observation, bytes },
        ) => validate_log_success(LogSuccessInput {
            profile,
            request,
            accepted,
            log_read,
            observation,
            bytes,
        }),
        _ => Err(AdmissionError::OperationResult),
    }
}

struct LogSuccessInput<'a> {
    profile: &'a lattice_workflow_contract::CapabilityProfile,
    request: &'a lattice_workflow_contract::WorkflowRequest,
    accepted: &'a lattice_workflow_contract::WorkflowAccepted,
    log_read: &'a lattice_workflow_contract::LogReadRequest,
    observation: &'a lattice_workflow_contract::ExecutionObservation,
    bytes: &'a [u8],
}

fn validate_log_success(input: LogSuccessInput<'_>) -> Result<(), AdmissionError> {
    validate_observation(input.profile, input.request, input.accepted, input.observation)
        .map_err(|_| AdmissionError::Contract)?;
    let byte_count = u64::try_from(input.bytes.len()).map_err(|_| AdmissionError::LogBound)?;
    if byte_count > input.log_read.maximum_bytes || byte_count > u64::from(MAXIMUM_LOG_BYTES) {
        return Err(AdmissionError::LogBound);
    }
    let is_log_linked = input.observation.logs.iter().any(|reference| {
        reference.node_id == input.log_read.node_id
            && reference.stream == input.log_read.stream
            && reference.cursor == input.log_read.cursor
            && reference.bytes == byte_count
    });
    if !is_log_linked {
        return Err(AdmissionError::LogLinkage);
    }
    Ok(())
}

fn validate_rejection(rejection: &Rejection) -> Result<(), AdmissionError> {
    Label::new(rejection.field.clone()).map(|_label| ()).map_err(|_| AdmissionError::OperationResult)
}

fn identity_for<T: serde::Serialize>(domain: &[u8], value: &T) -> Result<Identity, AdmissionError> {
    let bytes = serde_json::to_vec(value).map_err(|_| AdmissionError::Encoding)?;
    let domain_length_bytes = u64::try_from(domain.len()).map_err(|_| AdmissionError::Encoding)?;
    let value_length_bytes = u64::try_from(bytes.len()).map_err(|_| AdmissionError::Encoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&domain_length_bytes.to_be_bytes());
    hasher.update(domain);
    hasher.update(&value_length_bytes.to_be_bytes());
    hasher.update(&bytes);
    let value = format!("{BLAKE3_PREFIX}{}", hasher.finalize().to_hex());
    Identity::new(value).map_err(|_| AdmissionError::Encoding)
}
