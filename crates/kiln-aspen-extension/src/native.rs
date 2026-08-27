mod effects;

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, KILN_EXTENSION_ID, KILN_EXTENSION_SERVICE_ID,
};
use kiln_adapter_aspen::protocol::{
    AspenMaterializedValue, AspenNativeCallbackEnvelope, AspenNativeCallbackOutcome,
    CallbackCommand, CallbackRequest, CallbackState, HOSTED_CALLBACK_INPUT_SCHEMA,
    HostedCallbackInput, ProtocolError, decode_aspen_native_envelope, decode_hosted_callback_input,
    encode_aspen_native_outcome, encode_callback_outcome,
};
use kiln_core::types::ExternalRef;

use crate::KilnAspenCallback;
use crate::decision::checkpoint_ref;
use effects::{completion_command, native_effects};

const HEALTHY: &str = "healthy";
const STOPPED: &str = "stopped";

impl KilnAspenCallback {
    // r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.conformance]
    pub fn invoke_aspen_native(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let bounds = &self.profile.profile.bounds;
        let envelope = decode_aspen_native_envelope(
            bytes,
            bounds.max_callback_input_bytes,
            bounds.max_materialized_value_bytes,
            bounds.max_materialized_values,
        )
        .map_err(protocol_string)?;
        validate_envelope(&self.profile, &envelope)?;
        let prior_state = decode_prior_state(&envelope, bounds.max_materialized_value_bytes)?;
        let request = callback_request(&self.profile, &envelope, prior_state)?;
        let outcome = self.handle(&request)?;
        let output_bytes = encode_callback_outcome(&outcome, bounds.max_materialized_value_bytes)
            .map_err(protocol_string)?;
        let state_bytes = serde_json::to_vec(&outcome.state).map_err(|error| error.to_string())?;
        if state_bytes.len() > bounds.max_materialized_value_bytes {
            return Err("callback state exceeds the admitted materialized-value bound".to_string());
        }
        let state = AspenMaterializedValue::exact(state_bytes);
        let effects = native_effects(&self.profile, &outcome.effects)?;
        let checkpoint = (envelope.callback == "checkpoint").then(|| state.clone());
        let health = if outcome.state.stopped {
            STOPPED
        } else {
            HEALTHY
        };
        encode_aspen_native_outcome(
            &AspenNativeCallbackOutcome {
                outputs: vec![AspenMaterializedValue::exact(output_bytes)],
                effects,
                state: Some(state),
                checkpoint,
                health: health.to_string(),
            },
            bounds.max_callback_output_bytes,
            bounds.max_materialized_value_bytes,
            bounds.max_materialized_values,
        )
        .map_err(protocol_string)
    }
}

fn validate_envelope(
    profile: &AdmittedAspenRuntimeProfile,
    envelope: &AspenNativeCallbackEnvelope,
) -> Result<(), String> {
    if envelope.extension_id != KILN_EXTENSION_ID
        || envelope.service_id != KILN_EXTENSION_SERVICE_ID
        || envelope.generation != profile.profile.generation
        || envelope.deadline_tick < envelope.logical_tick
        || envelope.manifest_ref.is_empty()
        || envelope.executable_ref.is_empty()
        || envelope.instance_id.is_empty()
        || envelope.resource_ref.is_empty()
    {
        return Err("Aspen native envelope does not match the admitted Kiln extension".to_string());
    }
    Ok(())
}

fn decode_prior_state(
    envelope: &AspenNativeCallbackEnvelope,
    maximum_bytes: usize,
) -> Result<CallbackState, String> {
    let Some(state) = &envelope.state else {
        return Ok(CallbackState::initial(envelope.generation));
    };
    if state.bytes.len() > maximum_bytes {
        return Err("Aspen prior state exceeds the admitted value bound".to_string());
    }
    let decoded: CallbackState =
        serde_json::from_slice(&state.bytes).map_err(|error| error.to_string())?;
    let canonical = serde_json::to_vec(&decoded).map_err(|error| error.to_string())?;
    if canonical != state.bytes {
        return Err("Aspen prior state is not canonical Kiln state".to_string());
    }
    Ok(decoded)
}

fn callback_request(
    profile: &AdmittedAspenRuntimeProfile,
    envelope: &AspenNativeCallbackEnvelope,
    prior_state: CallbackState,
) -> Result<CallbackRequest, String> {
    let (operation_id, authority_ref, policy_ref, resource_ref, command) = match envelope
        .callback
        .as_str()
    {
        "request" => {
            let payload = envelope
                .payload
                .as_ref()
                .ok_or_else(|| "request callback requires materialized input".to_string())?;
            let input = decode_hosted_callback_input(
                &payload.bytes,
                profile.profile.bounds.max_materialized_value_bytes,
            )
            .map_err(protocol_string)?;
            validate_hosted_input(profile, &input)?;
            (
                input.operation_id,
                input.authority_ref,
                input.policy_ref,
                input.resource_ref,
                input.command,
            )
        }
        "message" => {
            let command = completion_command(profile, envelope, &prior_state)?;
            (
                lifecycle_operation(&envelope.event_ref)?,
                profile.profile.authority_ref.clone(),
                profile.profile.policy_ref.clone(),
                profile.profile.resource_ref.clone(),
                command,
            )
        }
        "checkpoint" => lifecycle_request(profile, envelope, CallbackCommand::Checkpoint)?,
        "recover" => lifecycle_request(
            profile,
            envelope,
            CallbackCommand::Recover {
                checkpoint_ref: checkpoint_ref(&prior_state)?,
            },
        )?,
        "drain" => lifecycle_request(profile, envelope, CallbackCommand::Drain)?,
        "shutdown" => lifecycle_request(profile, envelope, CallbackCommand::Shutdown)?,
        "initialize" | "start" | "health" => {
            lifecycle_request(profile, envelope, CallbackCommand::Status)?
        }
        _ => return Err("Aspen callback kind is not admitted by the Kiln extension".to_string()),
    };
    Ok(CallbackRequest {
        schema: kiln_adapter_aspen::protocol::CALLBACK_REQUEST_SCHEMA.to_string(),
        protocol_schema: profile.profile.extension_protocol_schema.clone(),
        profile_identity: profile.profile_identity.clone(),
        generation: envelope.generation,
        sequence: envelope.sequence,
        operation_id,
        authority_ref,
        policy_ref,
        resource_ref,
        deadline_tick: envelope.deadline_tick,
        prior_state,
        command,
    })
}

fn lifecycle_request(
    profile: &AdmittedAspenRuntimeProfile,
    envelope: &AspenNativeCallbackEnvelope,
    command: CallbackCommand,
) -> Result<(ExternalRef, String, String, String, CallbackCommand), String> {
    Ok((
        lifecycle_operation(&envelope.event_ref)?,
        profile.profile.authority_ref.clone(),
        profile.profile.policy_ref.clone(),
        profile.profile.resource_ref.clone(),
        command,
    ))
}

fn validate_hosted_input(
    profile: &AdmittedAspenRuntimeProfile,
    input: &HostedCallbackInput,
) -> Result<(), String> {
    if input.schema != HOSTED_CALLBACK_INPUT_SCHEMA
        || input.protocol_schema != profile.profile.extension_protocol_schema
        || input.profile_identity != profile.profile_identity
        || input.authority_ref != profile.profile.authority_ref
        || input.policy_ref != profile.profile.policy_ref
        || input.resource_ref != profile.profile.resource_ref
    {
        return Err("hosted callback input does not match the admitted Kiln profile".to_string());
    }
    input
        .operation_id
        .validate()
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn lifecycle_operation(event_ref: &str) -> Result<ExternalRef, String> {
    let hex = event_ref
        .strip_prefix("blake3:")
        .ok_or_else(|| "Aspen callback event identity is malformed".to_string())?;
    ExternalRef::new("molten", "callback-event-blake3", hex)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn protocol_string(error: ProtocolError) -> String {
    error.to_string()
}
