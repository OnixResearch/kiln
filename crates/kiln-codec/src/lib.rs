use preserves::PackedWriter;
use serde::Serialize;
use std::fmt::{Display, Formatter};

use kiln_core::ContentId;

pub const PACKED_PRESERVES_PROFILE: &str = "kiln.packed-preserves.v1";
pub const DEFAULT_MAX_CANONICAL_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecError {
    BoundExceeded,
    Encode(String),
    InvalidIdentity,
}

impl Display for CodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BoundExceeded => formatter
                .write_str("canonical packed Preserves output exceeds its named byte bound"),
            Self::Encode(message) => {
                write!(formatter, "packed Preserves encoding failed: {message}")
            }
            Self::InvalidIdentity => formatter.write_str("BLAKE3 identity construction failed"),
        }
    }
}

impl std::error::Error for CodecError {}

pub fn encode_packed<T: Serialize>(value: &T, max_bytes: usize) -> Result<Vec<u8>, CodecError> {
    let mut bytes = Vec::new();
    {
        let mut writer = PackedWriter::new(&mut bytes);
        value
            .serialize(&mut preserves::serde::ser::Serializer::new(&mut writer))
            .map_err(|error| CodecError::Encode(error.to_string()))?;
    }
    if bytes.len() > max_bytes {
        return Err(CodecError::BoundExceeded);
    }
    Ok(bytes)
}

pub fn identity_from_bytes(bytes: &[u8]) -> Result<ContentId, CodecError> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string())
        .map_err(|_| CodecError::InvalidIdentity)
}

pub fn encode_and_identify<T: Serialize>(
    value: &T,
    max_bytes: usize,
) -> Result<(Vec<u8>, ContentId), CodecError> {
    let bytes = encode_packed(value, max_bytes)?;
    let identity = identity_from_bytes(&bytes)?;
    Ok((bytes, identity))
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    const TEST_BOUND: usize = 1024;
    const TOO_SMALL_BOUND: usize = 1;

    #[derive(Serialize)]
    struct Fixture<'a> {
        kind: &'a str,
        value: u64,
    }

    #[test]
    fn equal_values_have_equal_packed_identity() {
        let fixture = Fixture {
            kind: "run",
            value: 1,
        };
        let first = encode_and_identify(&fixture, TEST_BOUND);
        let second = encode_and_identify(&fixture, TEST_BOUND);
        assert_eq!(first, second);
    }

    #[test]
    fn rejects_output_above_bound() {
        let fixture = Fixture {
            kind: "run",
            value: 1,
        };
        assert_eq!(
            encode_packed(&fixture, TOO_SMALL_BOUND),
            Err(CodecError::BoundExceeded)
        );
    }
}
