use kiln_core::domain::Outcome;
use kiln_core::types::{ContentId, EffectId, ExternalRef, ProviderProfileId};
use kiln_core::workflow::WorkflowObservation;
use serde::{Deserialize, Serialize};

use super::super::{CallbackEffectPlan, CallbackEffectTerminal, ProtocolError};

pub const HOSTED_PROVIDER_COMPLETION_SCHEMA: &str = "kiln.aspen-provider-completion.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedProviderCompletion {
    pub schema: String,
    pub effect_id: EffectId,
    pub operation_id: ExternalRef,
    pub provider_profile: ProviderProfileId,
    pub request_identity: ContentId,
    pub observation: WorkflowObservation,
}

// r[impl kiln.aspen_host.completion]
pub fn encode_hosted_provider_completion(
    completion: &HostedProviderCompletion,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    validate_completion_shape(completion)?;
    let bytes =
        serde_json::to_vec(completion).map_err(|error| ProtocolError::Json(error.to_string()))?;
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    Ok(bytes)
}

// r[impl kiln.aspen_host.completion]
pub fn decode_hosted_provider_completion(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<HostedProviderCompletion, ProtocolError> {
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    let completion: HostedProviderCompletion =
        serde_json::from_slice(bytes).map_err(|error| ProtocolError::Json(error.to_string()))?;
    validate_completion_shape(&completion)?;
    Ok(completion)
}

// r[impl kiln.aspen_host.completion]
pub fn admit_hosted_provider_completion(
    effect: &CallbackEffectPlan,
    completion: &HostedProviderCompletion,
) -> Result<CallbackEffectTerminal, ProtocolError> {
    validate_completion_shape(completion)?;
    if completion.effect_id != effect.effect_id
        || completion.operation_id != effect.operation_id
        || completion.provider_profile != effect.provider_profile
        || completion.request_identity != effect.request_identity
        || completion.observation.effect_id != effect.effect_id
        || completion.observation.provider_profile != effect.provider_profile
        || completion.observation.idempotency_key != effect.request_identity
    {
        return Err(ProtocolError::Identity);
    }
    Ok(terminal_for_observation(&completion.observation))
}

fn validate_completion_shape(completion: &HostedProviderCompletion) -> Result<(), ProtocolError> {
    if completion.schema != HOSTED_PROVIDER_COMPLETION_SCHEMA {
        return Err(ProtocolError::Schema);
    }
    completion
        .operation_id
        .validate()
        .map_err(|_| ProtocolError::Identity)?;
    Ok(())
}

const fn terminal_for_observation(observation: &WorkflowObservation) -> CallbackEffectTerminal {
    if !observation.terminal {
        return CallbackEffectTerminal::Unknown;
    }
    match observation.outcome {
        Outcome::Success => CallbackEffectTerminal::Success,
        Outcome::DomainFailure => CallbackEffectTerminal::Failure,
        Outcome::Denied => CallbackEffectTerminal::Denied,
        Outcome::TimedOut => CallbackEffectTerminal::TimedOut,
        Outcome::Cancelled => CallbackEffectTerminal::Cancelled,
        Outcome::Pending | Outcome::ProviderError | Outcome::Unknown => {
            CallbackEffectTerminal::Unknown
        }
    }
}
