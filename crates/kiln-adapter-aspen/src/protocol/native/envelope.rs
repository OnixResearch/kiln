use preserves::IOValue;

use super::model::AspenNativeCallbackEnvelope;
use super::value::*;
use crate::profile::{ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA, ASPEN_NATIVE_FRAMING};
use crate::protocol::ProtocolError;

const ENVELOPE_RECORD: &str = "native-callback-envelope-v2";
const ENVELOPE_FIELD_COUNT: usize = 18;

// r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.conformance]
pub fn encode_aspen_native_envelope(
    envelope: &AspenNativeCallbackEnvelope,
    maximum_frame_bytes: usize,
    maximum_value_bytes: usize,
    maximum_values: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let value_count = envelope.payload.iter().count() + envelope.state.iter().count();
    if value_count > maximum_values {
        return Err(ProtocolError::BoundExceeded);
    }
    for value in envelope.payload.iter().chain(envelope.state.iter()) {
        admit_value(value, maximum_value_bytes)?;
    }
    for reference in [
        envelope.manifest_ref.as_str(),
        envelope.executable_ref.as_str(),
        envelope.event_ref.as_str(),
        envelope.resource_ref.as_str(),
    ]
    .into_iter()
    .chain(envelope.policy_refs.iter().map(String::as_str))
    .chain(envelope.port_binding_refs.iter().map(String::as_str))
    {
        require_content_ref_text(reference)?;
    }
    let value = IOValue::record(
        IOValue::symbol(ENVELOPE_RECORD),
        vec![
            IOValue::new(ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA.to_string()),
            IOValue::new(envelope.manifest_ref.clone()),
            IOValue::new(envelope.executable_ref.clone()),
            IOValue::new(envelope.instance_id.clone()),
            IOValue::new(envelope.extension_id.clone()),
            IOValue::new(envelope.service_id.clone()),
            IOValue::new(envelope.generation),
            IOValue::new(envelope.callback.clone()),
            IOValue::new(envelope.sequence),
            IOValue::new(envelope.event_ref.clone()),
            optional_value(envelope.payload.as_ref()),
            IOValue::new(envelope.logical_tick),
            IOValue::new(envelope.deadline_tick),
            optional_value(envelope.state.as_ref()),
            string_sequence(&envelope.policy_refs),
            IOValue::new(envelope.resource_ref.clone()),
            string_sequence(&envelope.port_binding_refs),
            IOValue::new(ASPEN_NATIVE_FRAMING.to_string()),
        ],
    );
    encode_canonical(&value, maximum_frame_bytes)
}

// r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.conformance]
pub fn decode_aspen_native_envelope(
    bytes: &[u8],
    maximum_frame_bytes: usize,
    maximum_value_bytes: usize,
    maximum_values: usize,
) -> Result<AspenNativeCallbackEnvelope, ProtocolError> {
    let value = decode_canonical(bytes, maximum_frame_bytes)?;
    let fields = record_fields(&value, ENVELOPE_RECORD, ENVELOPE_FIELD_COUNT)?;
    let [
        schema,
        manifest_ref,
        executable_ref,
        instance_id,
        extension_id,
        service_id,
        generation,
        callback,
        sequence,
        event_ref,
        payload_value,
        logical_tick,
        deadline_tick,
        state_value,
        policy_refs,
        resource_ref,
        port_binding_refs,
        framing,
    ] = fields.as_slice()
    else {
        return Err(ProtocolError::Shape);
    };
    require_string(schema, ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA)?;
    require_string(framing, ASPEN_NATIVE_FRAMING)?;
    let payload = parse_optional_value(payload_value, maximum_value_bytes)?;
    let state = parse_optional_value(state_value, maximum_value_bytes)?;
    let materialized_count = payload.iter().count() + state.iter().count();
    if materialized_count > maximum_values {
        return Err(ProtocolError::BoundExceeded);
    }
    Ok(AspenNativeCallbackEnvelope {
        manifest_ref: string_field(manifest_ref)?,
        executable_ref: string_field(executable_ref)?,
        instance_id: string_field(instance_id)?,
        extension_id: string_field(extension_id)?,
        service_id: string_field(service_id)?,
        generation: u64_field(generation)?,
        callback: string_field(callback)?,
        sequence: u64_field(sequence)?,
        event_ref: content_ref_field(event_ref)?,
        payload,
        logical_tick: u64_field(logical_tick)?,
        deadline_tick: u64_field(deadline_tick)?,
        state,
        policy_refs: ref_sequence(policy_refs, maximum_values)?,
        resource_ref: content_ref_field(resource_ref)?,
        port_binding_refs: ref_sequence(port_binding_refs, maximum_values)?,
    })
}
