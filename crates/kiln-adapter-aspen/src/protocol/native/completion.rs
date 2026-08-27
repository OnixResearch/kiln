use preserves::IOValue;
use preserves::Value;

use super::model::AspenEffectCompletion;
use super::value::*;
use crate::protocol::ProtocolError;

const EFFECT_COMPLETION_RECORD: &str = "system-extension-effect-completion-v1";
const EFFECT_COMPLETION_SCHEMA: &str = "molten.system-extension.effect-completion.v1";
const EFFECT_COMPLETION_FIELD_COUNT: usize = 10;

pub fn decode_aspen_effect_completion(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<AspenEffectCompletion, ProtocolError> {
    let value = decode_canonical(bytes, maximum_bytes)?;
    let fields = record_fields(
        &value,
        EFFECT_COMPLETION_RECORD,
        EFFECT_COMPLETION_FIELD_COUNT,
    )?;
    let [
        schema,
        _callback_receipt,
        _binding,
        _port_id,
        _port_version,
        request_ref,
        generation,
        _output_schema,
        output_ref,
        _checks,
    ] = fields.as_slice()
    else {
        return Err(ProtocolError::Shape);
    };
    require_string(schema, EFFECT_COMPLETION_SCHEMA)?;
    Ok(AspenEffectCompletion {
        request_ref: completion_content_ref(request_ref, "request-ref")?,
        generation: completion_u64(generation, "generation")?,
        output_ref: completion_content_ref(output_ref, "output-ref")?,
    })
}

fn completion_content_ref(
    value: &Value<IOValue>,
    field_name: &str,
) -> Result<String, ProtocolError> {
    let value = value_to_io(value);
    let fields = record_fields(&value, field_name, 1)?;
    content_ref_field(fields.first().ok_or(ProtocolError::Shape)?)
}

fn completion_u64(value: &Value<IOValue>, field_name: &str) -> Result<u64, ProtocolError> {
    let value = value_to_io(value);
    let fields = record_fields(&value, field_name, 1)?;
    u64_field(fields.first().ok_or(ProtocolError::Shape)?)
}
