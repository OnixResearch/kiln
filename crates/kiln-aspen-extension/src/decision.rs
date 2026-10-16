use kiln_adapter_aspen::effects::{EffectMappingInput, ProviderEffectClass, map_provider_effect};
use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, LOCAL_CLAIM_LEVEL, REQUIRED_NON_CLAIMS,
};
use kiln_adapter_aspen::protocol::{
    CALLBACK_CHECKPOINT_SCHEMA, CALLBACK_COMPLETION_SCHEMA, CALLBACK_REQUEST_SCHEMA,
    CALLBACK_STATE_SCHEMA, CALLBACK_STATUS_SCHEMA, CallbackEffectCompletion, CallbackEffectPlan,
    CallbackEffectTerminal, CallbackRequest, CallbackState, CallbackStatus, HostedRunPhase,
    HostedRunState, hosted_run_id,
};
use kiln_core::domain::Outcome;
use kiln_core::types::{ContentId, Diagnostic, EffectId, ExternalRef, ProviderProfileId};
const EFFECT_ID_PREFIX: &str = "kiln-aspen-effect-";

pub(crate) fn validate_request(
    profile: &AdmittedAspenRuntimeProfile,
    request: &CallbackRequest,
) -> Result<(), String> {
    if request.schema != CALLBACK_REQUEST_SCHEMA
        || request.protocol_schema != profile.profile.extension_protocol_schema
        || request.profile_identity != profile.profile_identity
        || request.generation != profile.profile.generation
        || request.prior_state.schema != CALLBACK_STATE_SCHEMA
        || request.prior_state.generation != request.generation
        || request.sequence != request.prior_state.revision.saturating_add(1)
        || request.deadline_tick == 0
        || request.authority_ref != profile.profile.authority_ref
        || request.policy_ref != profile.profile.policy_ref
        || request.resource_ref != profile.profile.resource_ref
    {
        return Err("callback request does not match the active exact Aspen profile".to_string());
    }
    request.operation_id.validate().map_err(diagnostic_string)?;
    Ok(())
}

pub(crate) fn workflow_effect(
    profile: &AdmittedAspenRuntimeProfile,
    request: &CallbackRequest,
    trigger_identity: &ContentId,
    idempotency_key: &ExternalRef,
) -> Result<CallbackEffectPlan, String> {
    let material = format!(
        "{}\0{}\0{}",
        request.operation_id.native_id.as_str(),
        request.generation,
        trigger_identity.as_str()
    );
    let digest = blake3::hash(material.as_bytes()).to_hex().to_string();
    let effect_id =
        EffectId::new(format!("{EFFECT_ID_PREFIX}{digest}")).map_err(diagnostic_string)?;
    let operation_id = ExternalRef::new("molten", "transport-operation-blake3", &digest)
        .map_err(diagnostic_string)?;
    let provider_profile =
        ProviderProfileId::new(profile.profile.workflow_provider_profile.clone())
            .map_err(diagnostic_string)?;
    map_provider_effect(
        profile,
        &EffectMappingInput {
            approved: true,
            effect_id,
            class: ProviderEffectClass::TransportSession,
            provider_profile,
            operation_id,
            idempotency_key: idempotency_key.clone(),
            request_identity: trigger_identity.clone(),
            generation: request.generation,
            authority_ref: request.authority_ref.clone(),
            resource_ref: request.resource_ref.clone(),
        },
    )
    .map_err(diagnostic_string)
}

pub(crate) fn hosted_run(
    trigger_identity: &ContentId,
    phase: HostedRunPhase,
    provider_effect: Option<EffectId>,
) -> Result<HostedRunState, String> {
    let run_id = hosted_run_id(trigger_identity).map_err(diagnostic_string)?;
    Ok(HostedRunState {
        run_id,
        trigger_identity: trigger_identity.clone(),
        phase,
        provider_effect,
        provider_observation: None,
    })
}

pub(crate) fn apply_effect_terminal(
    state: &mut CallbackState,
    effect_id: &EffectId,
    observation_ref: &ExternalRef,
    phase: HostedRunPhase,
) -> Result<(), String> {
    if state
        .unresolved_effects
        .remove(effect_id.as_str())
        .is_none()
    {
        return Err("effect completion is stale, duplicated, or unknown".to_string());
    }
    let Some(run) = state
        .runs
        .values_mut()
        .find(|run| run.provider_effect.as_ref() == Some(effect_id))
    else {
        return Err("effect completion has no linked Kiln run".to_string());
    };
    run.phase = phase;
    run.provider_observation = Some(observation_ref.clone());
    Ok(())
}

pub(crate) const fn phase_for_outcome(outcome: Outcome) -> HostedRunPhase {
    match outcome {
        Outcome::Pending => HostedRunPhase::Running,
        Outcome::Success => HostedRunPhase::Completed,
        Outcome::DomainFailure => HostedRunPhase::Failed,
        Outcome::Denied => HostedRunPhase::Denied,
        Outcome::Cancelled => HostedRunPhase::Cancelled,
        Outcome::TimedOut => HostedRunPhase::TimedOut,
        Outcome::ProviderError | Outcome::Unknown => HostedRunPhase::Unknown,
    }
}

pub(crate) const fn phase_for_terminal(terminal: CallbackEffectTerminal) -> HostedRunPhase {
    match terminal {
        CallbackEffectTerminal::Success => HostedRunPhase::Completed,
        CallbackEffectTerminal::Failure => HostedRunPhase::Failed,
        CallbackEffectTerminal::Denied => HostedRunPhase::Denied,
        CallbackEffectTerminal::TimedOut => HostedRunPhase::TimedOut,
        CallbackEffectTerminal::Cancelled => HostedRunPhase::Cancelled,
        CallbackEffectTerminal::Unknown => HostedRunPhase::Unknown,
    }
}

pub(crate) fn validate_completion(
    generation: u64,
    completion: &CallbackEffectCompletion,
) -> Result<(), String> {
    if completion.schema != CALLBACK_COMPLETION_SCHEMA || completion.generation != generation {
        return Err("effect completion schema or generation is stale".to_string());
    }
    completion
        .operation_id
        .validate()
        .map_err(diagnostic_string)?;
    completion
        .observation_ref
        .validate()
        .map_err(diagnostic_string)
}

pub(crate) fn checkpoint_ref(state: &CallbackState) -> Result<ExternalRef, String> {
    let bytes = serde_json::to_vec(&(CALLBACK_CHECKPOINT_SCHEMA, state))
        .map_err(|error| error.to_string())?;
    let digest = blake3::hash(&bytes).to_hex().to_string();
    ExternalRef::new("kiln", "aspen-checkpoint-blake3", &digest).map_err(diagnostic_string)
}

pub(crate) fn status(state: &CallbackState) -> CallbackStatus {
    CallbackStatus {
        schema: CALLBACK_STATUS_SCHEMA.to_string(),
        generation: state.generation,
        revision: state.revision,
        run_count: state.runs.len(),
        unresolved_effect_count: state.unresolved_effects.len(),
        draining: state.draining,
        stopped: state.stopped,
        claim_level: LOCAL_CLAIM_LEVEL.to_string(),
        non_claims: REQUIRED_NON_CLAIMS
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}

fn diagnostic_string(diagnostic: Diagnostic) -> String {
    format!("{}: {}", diagnostic.code, diagnostic.message)
}
