use kiln_adapter_aspen::effects::admit_effect_plan;
use kiln_adapter_aspen::profile::{AdmittedAspenRuntimeProfile, AspenPortPurpose};
use kiln_adapter_aspen::protocol::{
    AspenEffectKind, AspenMaterializedValue, AspenNativeCallbackEnvelope, AspenNativeEffect,
    CallbackCommand, CallbackEffectCompletion, CallbackEffectPlan, CallbackEffectTerminal,
    CallbackState, aspen_content_ref, decode_aspen_effect_completion,
};
use kiln_core::types::ExternalRef;

use super::protocol_string;

const KILN_EFFECT_OPERATION: &str = "kiln-provider-effect-v2";

pub(super) fn completion_command(
    profile: &AdmittedAspenRuntimeProfile,
    envelope: &AspenNativeCallbackEnvelope,
    state: &CallbackState,
) -> Result<CallbackCommand, String> {
    let payload = envelope
        .payload
        .as_ref()
        .ok_or_else(|| "message callback requires a materialized completion".to_string())?;
    let completion = decode_aspen_effect_completion(
        &payload.bytes,
        profile.profile.bounds.max_materialized_value_bytes,
    )
    .map_err(protocol_string)?;
    if completion.generation != envelope.generation {
        return Err("Aspen effect completion generation is stale".to_string());
    }
    let mut matched_effect = None;
    for candidate in state.unresolved_effects.values() {
        if effect_request_ref(candidate)? == completion.request_ref {
            if matched_effect.is_some() {
                return Err("Aspen completion ambiguously names multiple Kiln effects".to_string());
            }
            matched_effect = Some(candidate);
        }
    }
    let effect = matched_effect
        .ok_or_else(|| "Aspen completion does not name an unresolved Kiln effect".to_string())?;
    let output_hex = completion
        .output_ref
        .strip_prefix("blake3:")
        .ok_or_else(|| "Aspen completion output identity is malformed".to_string())?;
    let observation_ref = ExternalRef::new("molten", "effect-output-blake3", output_hex)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    Ok(CallbackCommand::CompleteEffect(CallbackEffectCompletion {
        schema: kiln_adapter_aspen::protocol::CALLBACK_COMPLETION_SCHEMA.to_string(),
        effect_id: effect.effect_id.clone(),
        operation_id: effect.operation_id.clone(),
        generation: completion.generation,
        terminal: CallbackEffectTerminal::Unknown,
        observation_ref,
    }))
}

pub(super) fn native_effects(
    profile: &AdmittedAspenRuntimeProfile,
    effects: &[CallbackEffectPlan],
) -> Result<Vec<AspenNativeEffect>, String> {
    if effects.len() > profile.profile.bounds.max_effects {
        return Err("callback effect count exceeds the admitted bound".to_string());
    }
    effects
        .iter()
        .map(|effect| {
            admit_effect_plan(profile, effect)
                .map_err(|error| format!("{}: {}", error.code, error.message))?;
            let purpose = purpose(effect.kind);
            let binding = profile
                .profile
                .ports
                .iter()
                .find(|binding| binding.purpose == purpose)
                .ok_or_else(|| "callback effect has no exact Aspen port binding".to_string())?;
            let bytes = serde_json::to_vec(effect).map_err(|error| error.to_string())?;
            if bytes.len() > profile.profile.bounds.max_materialized_value_bytes {
                return Err("callback effect body exceeds the materialized-value bound".to_string());
            }
            let accounted_bytes = u64::try_from(bytes.len())
                .map_err(|_| "callback effect byte count does not fit u64".to_string())?;
            let request = AspenMaterializedValue::exact(bytes);
            Ok(AspenNativeEffect {
                port_id: binding.port_id.clone(),
                port_version: binding.version.clone(),
                operation: KILN_EFFECT_OPERATION.to_string(),
                input_schema: binding.input_schema.clone(),
                output_schema: binding.output_schema.clone(),
                request,
                generation: effect.generation,
                accounted_bytes,
            })
        })
        .collect()
}

fn effect_request_ref(effect: &CallbackEffectPlan) -> Result<String, String> {
    serde_json::to_vec(effect)
        .map(|bytes| aspen_content_ref(&bytes))
        .map_err(|error| error.to_string())
}

const fn purpose(kind: AspenEffectKind) -> AspenPortPurpose {
    match kind {
        AspenEffectKind::DurableState => AspenPortPurpose::Durability,
        AspenEffectKind::ContentStore => AspenPortPurpose::Content,
        AspenEffectKind::BoundedProcess => AspenPortPurpose::Execution,
        AspenEffectKind::TransportSession => AspenPortPurpose::Transport,
        AspenEffectKind::Timer => AspenPortPurpose::Time,
        AspenEffectKind::Schedule => AspenPortPurpose::Scheduling,
        AspenEffectKind::Evidence => AspenPortPurpose::Evidence,
    }
}
