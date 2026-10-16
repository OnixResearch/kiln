use std::borrow::Cow;

use preserves::IOValue;
use preserves::Value;

use super::AspenMaterializedValue;
use crate::protocol::ProtocolError;

const VALUE_RECORD: &str = "native-callback-value-v2";
const NONE_RECORD: &str = "none";
const SOME_RECORD: &str = "some";
const VALUE_FIELD_COUNT: usize = 2;
const BLAKE3_HEX_MULTIPLIER: usize = 2;

pub fn aspen_content_ref(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

pub(super) fn value_value(value: &AspenMaterializedValue) -> IOValue {
    IOValue::record(
        IOValue::symbol(VALUE_RECORD),
        vec![
            IOValue::new(value.value_ref.clone()),
            sequence(
                value
                    .bytes
                    .iter()
                    .map(|byte| IOValue::new(u64::from(*byte)))
                    .collect(),
            ),
        ],
    )
}

pub(super) fn optional_value(value: Option<&AspenMaterializedValue>) -> IOValue {
    value.map_or_else(
        || IOValue::record(IOValue::symbol(NONE_RECORD), Vec::new()),
        |value| IOValue::record(IOValue::symbol(SOME_RECORD), vec![value_value(value)]),
    )
}

pub(super) fn parse_optional_value(
    value: &Value<IOValue>,
    maximum_bytes: usize,
) -> Result<Option<AspenMaterializedValue>, ProtocolError> {
    let value = value_to_io(value);
    if value.collect_simple_record(NONE_RECORD, Some(0)).is_some() {
        return Ok(None);
    }
    let fields = record_fields(&value, SOME_RECORD, 1)?;
    let field = fields.first().ok_or(ProtocolError::Shape)?;
    parse_value(field, maximum_bytes).map(Some)
}

pub(super) fn parse_value(
    value: &Value<IOValue>,
    maximum_bytes: usize,
) -> Result<AspenMaterializedValue, ProtocolError> {
    let value = value_to_io(value);
    let fields = record_fields(&value, VALUE_RECORD, VALUE_FIELD_COUNT)?;
    let value_ref = content_ref_field(fields.first().ok_or(ProtocolError::Shape)?)?;
    let values = fields
        .get(1)
        .ok_or(ProtocolError::Shape)?
        .collect_sequence()
        .ok_or(ProtocolError::Shape)?;
    if values.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    let bytes = values
        .iter()
        .map(|value| {
            let number = u64_field(value)?;
            u8::try_from(number).map_err(|_| ProtocolError::Shape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = AspenMaterializedValue { value_ref, bytes };
    admit_value(&result, maximum_bytes)?;
    Ok(result)
}

pub(super) fn admit_value(
    value: &AspenMaterializedValue,
    maximum_bytes: usize,
) -> Result<(), ProtocolError> {
    if value.bytes.len() > maximum_bytes || aspen_content_ref(&value.bytes) != value.value_ref {
        return Err(ProtocolError::Identity);
    }
    Ok(())
}

pub(super) fn ref_sequence(
    value: &Value<IOValue>,
    maximum_values: usize,
) -> Result<Vec<String>, ProtocolError> {
    let values = value.collect_sequence().ok_or(ProtocolError::Shape)?;
    if values.len() > maximum_values {
        return Err(ProtocolError::BoundExceeded);
    }
    values.iter().map(content_ref_field).collect()
}

pub(super) fn sequence(values: Vec<IOValue>) -> IOValue {
    IOValue::new(values)
}

pub(super) fn string_sequence(values: &[String]) -> IOValue {
    sequence(values.iter().cloned().map(IOValue::new).collect())
}

pub(super) fn record_fields(
    value: &IOValue,
    name: &str,
    count: usize,
) -> Result<Vec<Value<IOValue>>, ProtocolError> {
    let fields = value
        .collect_simple_record(name, Some(count))
        .ok_or(ProtocolError::Shape)?;
    let fields = fields.fields_iter().cloned().collect::<Vec<_>>();
    if fields.len() != count {
        return Err(ProtocolError::Shape);
    }
    Ok(fields)
}

pub(super) fn decode_canonical(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<IOValue, ProtocolError> {
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    let value = preserves::read_iovalue_packed(bytes, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    let canonical = preserves::write_iovalue_packed(&value, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    if canonical != bytes {
        return Err(ProtocolError::NonCanonical);
    }
    Ok(value)
}

pub(super) fn encode_canonical(
    value: &IOValue,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let bytes = preserves::write_iovalue_packed(value, false)
        .map_err(|error| ProtocolError::Parse(error.to_string()))?;
    if bytes.len() > maximum_bytes {
        return Err(ProtocolError::BoundExceeded);
    }
    Ok(bytes)
}

pub(super) fn require_string(value: &Value<IOValue>, expected: &str) -> Result<(), ProtocolError> {
    if string_field(value)?.as_str() != expected {
        return Err(ProtocolError::Schema);
    }
    Ok(())
}

pub(super) fn string_field(value: &Value<IOValue>) -> Result<String, ProtocolError> {
    value
        .as_string()
        .map(Cow::into_owned)
        .ok_or(ProtocolError::Shape)
}

pub(super) fn content_ref_field(value: &Value<IOValue>) -> Result<String, ProtocolError> {
    let value = string_field(value)?;
    require_content_ref_text(&value)?;
    Ok(value)
}

pub(super) fn require_content_ref_text(value: &str) -> Result<(), ProtocolError> {
    let Some(hex) = value.strip_prefix("blake3:") else {
        return Err(ProtocolError::Identity);
    };
    if hex.len() != blake3::OUT_LEN * BLAKE3_HEX_MULTIPLIER
        || !hex
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ProtocolError::Identity);
    }
    Ok(())
}

pub(super) fn u64_field(value: &Value<IOValue>) -> Result<u64, ProtocolError> {
    value
        .as_u64()
        .ok_or(ProtocolError::Shape)?
        .map_err(|_| ProtocolError::Shape)
}

pub(super) fn value_to_io(value: &Value<IOValue>) -> IOValue {
    IOValue(value.clone())
}
