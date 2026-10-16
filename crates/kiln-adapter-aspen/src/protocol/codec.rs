use std::borrow::Cow;

use kiln_core::types::ContentId;
use preserves::IOValue;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::*;
use crate::profile::{ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA, KILN_EXTENSION_PROTOCOL_SCHEMA};

const CALLBACK_RECORD_FIELD_COUNT: usize = 3;
const CALLBACK_PROTOCOL_VERSION: u64 = 2;

// r[impl kiln.aspen_runtime.callback]
pub fn encode_hosted_callback_input(
    input: &HostedCallbackInput,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    encode_record(
        HOSTED_CALLBACK_INPUT_RECORD,
        HOSTED_CALLBACK_INPUT_SCHEMA,
        input,
        maximum_bytes,
    )
}

// r[impl kiln.aspen_runtime.callback]
pub fn decode_hosted_callback_input(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<HostedCallbackInput, ProtocolError> {
    let input: HostedCallbackInput = decode_record(
        HOSTED_CALLBACK_INPUT_RECORD,
        HOSTED_CALLBACK_INPUT_SCHEMA,
        bytes,
        maximum_bytes,
    )?;
    if input.schema != HOSTED_CALLBACK_INPUT_SCHEMA
        || input.protocol_schema != KILN_EXTENSION_PROTOCOL_SCHEMA
    {
        return Err(ProtocolError::Schema);
    }
    Ok(input)
}

// r[impl kiln.aspen_runtime.callback]
pub fn encode_callback_request(
    request: &CallbackRequest,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    encode_record(
        CALLBACK_REQUEST_RECORD,
        CALLBACK_REQUEST_SCHEMA,
        request,
        maximum_bytes,
    )
}

// r[impl kiln.aspen_runtime.callback]
pub fn decode_callback_request(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<CallbackRequest, ProtocolError> {
    let request: CallbackRequest = decode_record(
        CALLBACK_REQUEST_RECORD,
        CALLBACK_REQUEST_SCHEMA,
        bytes,
        maximum_bytes,
    )?;
    if request.schema != CALLBACK_REQUEST_SCHEMA
        || request.protocol_schema != KILN_EXTENSION_PROTOCOL_SCHEMA
    {
        return Err(ProtocolError::Schema);
    }
    Ok(request)
}

// r[impl kiln.aspen_runtime.callback]
pub fn encode_callback_outcome(
    outcome: &CallbackOutcome,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    encode_record(
        CALLBACK_OUTCOME_RECORD,
        CALLBACK_OUTCOME_SCHEMA,
        outcome,
        maximum_bytes,
    )
}

// r[impl kiln.aspen_runtime.callback]
pub fn decode_callback_outcome(
    bytes: &[u8],
    maximum_bytes: usize,
    maximum_effects: usize,
) -> Result<CallbackOutcome, ProtocolError> {
    let outcome: CallbackOutcome = decode_record(
        CALLBACK_OUTCOME_RECORD,
        CALLBACK_OUTCOME_SCHEMA,
        bytes,
        maximum_bytes,
    )?;
    if outcome.schema != CALLBACK_OUTCOME_SCHEMA
        || outcome.protocol_schema != KILN_EXTENSION_PROTOCOL_SCHEMA
        || outcome.effects.len() > maximum_effects
    {
        return Err(ProtocolError::Schema);
    }
    Ok(outcome)
}

// r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.identity]
pub fn project_native_outcome(
    outcome: &CallbackOutcome,
    maximum_bytes: usize,
) -> Result<AspenNativeOutcomeProjection, ProtocolError> {
    let output_bytes = encode_callback_outcome(outcome, maximum_bytes)?;
    let state_bytes = serde_json::to_vec(&outcome.state)
        .map_err(|error| ProtocolError::Json(error.to_string()))?;
    let state_ref = content_identity(&state_bytes)?;
    let output_ref = content_identity(&output_bytes)?;
    let effect_request_refs = outcome
        .effects
        .iter()
        .map(|effect| {
            serde_json::to_vec(effect)
                .map_err(|error| ProtocolError::Json(error.to_string()))
                .and_then(|bytes| content_identity(&bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AspenNativeOutcomeProjection {
        schema: ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA.to_string(),
        state_ref,
        checkpoint_ref: outcome.checkpoint_ref.clone(),
        output_ref,
        effect_request_refs,
    })
}

fn encode_record<T: Serialize>(
    record_name: &'static str,
    schema: &str,
    value: &T,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let payload =
        serde_json::to_vec(value).map_err(|error| ProtocolError::Json(error.to_string()))?;
    let record = IOValue::record(
        IOValue::symbol(record_name),
        vec![
            IOValue::new(schema.to_string()),
            IOValue::new(CALLBACK_PROTOCOL_VERSION),
            IOValue::bytes(payload),
        ],
    );
    let bytes = preserves::write_iovalue_packed(&record, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    Ok(bytes)
}

fn decode_record<T: DeserializeOwned>(
    record_name: &str,
    schema: &str,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<T, ProtocolError> {
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    let value = preserves::read_iovalue_packed(bytes, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    let canonical = preserves::write_iovalue_packed(&value, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    if canonical.as_slice() != bytes {
        return Err(ProtocolError::NonCanonical);
    }
    let fields = value
        .collect_simple_record(record_name, Some(CALLBACK_RECORD_FIELD_COUNT))
        .ok_or(ProtocolError::Shape)?;
    let mut fields = fields.fields_iter();
    let observed_schema = fields
        .next()
        .and_then(|field| field.as_string())
        .ok_or(ProtocolError::Shape)?;
    if observed_schema.as_ref() != schema {
        return Err(ProtocolError::Schema);
    }
    let version = fields
        .next()
        .and_then(|field| field.as_u64())
        .ok_or(ProtocolError::Shape)?
        .map_err(|_| ProtocolError::Shape)?;
    if version != CALLBACK_PROTOCOL_VERSION {
        return Err(ProtocolError::Schema);
    }
    let payload: Cow<'_, [u8]> = fields
        .next()
        .and_then(|field| field.as_bytestring())
        .ok_or(ProtocolError::Shape)?;
    if fields.next().is_some() {
        return Err(ProtocolError::Shape);
    }
    serde_json::from_slice(payload.as_ref()).map_err(|error| ProtocolError::Json(error.to_string()))
}

fn content_identity(bytes: &[u8]) -> Result<ContentId, ProtocolError> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string()).map_err(|_| {
        ProtocolError::Json("canonical content identity construction failed".to_string())
    })
}
