use preserves::IOValue;
use preserves::Value;

use super::model::{AspenNativeCallbackOutcome, AspenNativeEffect};
use super::value::*;
use crate::profile::ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA;
use crate::protocol::ProtocolError;

const OUTCOME_RECORD: &str = "native-callback-outcome-v2";
const EFFECT_RECORD: &str = "native-callback-effect-v2";
const PORT_TARGET_RECORD: &str = "native-callback-port-target-v2";
const OUTCOME_FIELD_COUNT: usize = 6;
const EFFECT_FIELD_COUNT: usize = 8;
const PORT_TARGET_FIELD_COUNT: usize = 2;

// r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.conformance]
pub fn encode_aspen_native_outcome(
    outcome: &AspenNativeCallbackOutcome,
    maximum_frame_bytes: usize,
    maximum_value_bytes: usize,
    maximum_values: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let value_count = outcome
        .outputs
        .len()
        .checked_add(outcome.effects.len())
        .and_then(|count| count.checked_add(outcome.state.iter().count()))
        .and_then(|count| count.checked_add(outcome.checkpoint.iter().count()))
        .ok_or(ProtocolError::BoundExceeded)?;
    if value_count > maximum_values {
        return Err(ProtocolError::BoundExceeded);
    }
    for value in outcome
        .outputs
        .iter()
        .chain(outcome.effects.iter().map(|effect| &effect.request))
        .chain(outcome.state.iter())
        .chain(outcome.checkpoint.iter())
    {
        admit_value(value, maximum_value_bytes)?;
    }
    let record = IOValue::record(
        IOValue::symbol(OUTCOME_RECORD),
        vec![
            IOValue::new(ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA.to_string()),
            sequence(outcome.outputs.iter().map(value_value).collect()),
            sequence(outcome.effects.iter().map(effect_value).collect()),
            optional_value(outcome.state.as_ref()),
            optional_value(outcome.checkpoint.as_ref()),
            IOValue::new(outcome.health.clone()),
        ],
    );
    encode_canonical(&record, maximum_frame_bytes)
}

pub fn decode_aspen_native_outcome(
    bytes: &[u8],
    maximum_frame_bytes: usize,
    maximum_value_bytes: usize,
    maximum_values: usize,
) -> Result<AspenNativeCallbackOutcome, ProtocolError> {
    let value = decode_canonical(bytes, maximum_frame_bytes)?;
    let fields = record_fields(&value, OUTCOME_RECORD, OUTCOME_FIELD_COUNT)?;
    let [
        schema,
        output_field,
        effect_field,
        state_field,
        checkpoint_field,
        health_field,
    ] = fields.as_slice()
    else {
        return Err(ProtocolError::Shape);
    };
    require_string(schema, ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA)?;
    let output_values = output_field
        .collect_sequence()
        .ok_or(ProtocolError::Shape)?;
    let effect_values = effect_field
        .collect_sequence()
        .ok_or(ProtocolError::Shape)?;
    let state = parse_optional_value(state_field, maximum_value_bytes)?;
    let checkpoint = parse_optional_value(checkpoint_field, maximum_value_bytes)?;
    let value_count = output_values
        .len()
        .checked_add(effect_values.len())
        .and_then(|count| count.checked_add(state.iter().count()))
        .and_then(|count| count.checked_add(checkpoint.iter().count()))
        .ok_or(ProtocolError::BoundExceeded)?;
    if value_count > maximum_values {
        return Err(ProtocolError::BoundExceeded);
    }
    let outputs = output_values
        .iter()
        .map(|value| parse_value(value, maximum_value_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let effects = effect_values
        .iter()
        .map(|value| parse_effect(value, maximum_value_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AspenNativeCallbackOutcome {
        outputs,
        effects,
        state,
        checkpoint,
        health: string_field(health_field)?,
    })
}

fn effect_value(effect: &AspenNativeEffect) -> IOValue {
    IOValue::record(
        IOValue::symbol(EFFECT_RECORD),
        vec![
            IOValue::record(
                IOValue::symbol(PORT_TARGET_RECORD),
                vec![
                    IOValue::new(effect.port_id.clone()),
                    IOValue::new(effect.port_version.clone()),
                ],
            ),
            IOValue::new(effect.operation.clone()),
            IOValue::new(effect.input_schema.clone()),
            IOValue::new(effect.output_schema.clone()),
            value_value(&effect.request),
            IOValue::new(effect.generation),
            IOValue::new(effect.accounted_bytes),
            IOValue::new(ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA.to_string()),
        ],
    )
}

fn parse_effect(
    value: &Value<IOValue>,
    maximum_value_bytes: usize,
) -> Result<AspenNativeEffect, ProtocolError> {
    let value = value_to_io(value);
    let fields = record_fields(&value, EFFECT_RECORD, EFFECT_FIELD_COUNT)?;
    let [
        target_field,
        operation,
        input_schema,
        output_schema,
        request,
        generation,
        accounted_bytes,
        schema,
    ] = fields.as_slice()
    else {
        return Err(ProtocolError::Shape);
    };
    let target_value = value_to_io(target_field);
    let target = record_fields(&target_value, PORT_TARGET_RECORD, PORT_TARGET_FIELD_COUNT)?;
    let [port_id, port_version] = target.as_slice() else {
        return Err(ProtocolError::Shape);
    };
    require_string(schema, ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA)?;
    Ok(AspenNativeEffect {
        port_id: string_field(port_id)?,
        port_version: string_field(port_version)?,
        operation: string_field(operation)?,
        input_schema: string_field(input_schema)?,
        output_schema: string_field(output_schema)?,
        request: parse_value(request, maximum_value_bytes)?,
        generation: u64_field(generation)?,
        accounted_bytes: u64_field(accounted_bytes)?,
    })
}
