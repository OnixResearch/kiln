use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

pub const CONTRACT_VERSION_V1: u16 = 1;
pub const BLAKE3_HEX_BYTES: usize = 64;
pub const DEFAULT_MAX_ID_BYTES: usize = 128;
pub const DEFAULT_MAX_ALIAS_BYTES: usize = 256;
pub const DEFAULT_MAX_DIAGNOSTIC_BYTES: usize = 512;
pub const DEFAULT_MAX_COLLECTION_ITEMS: usize = 256;
pub const DEFAULT_MAX_TRANSITION_ITEMS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub field: String,
    pub message: String,
}

impl Diagnostic {
    #[must_use]
    pub fn new(code: &str, field: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            field: field.to_string(),
            message: message.to_string(),
        }
    }
}

pub type CoreResult<T> = Result<T, Diagnostic>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BoundedText(String);

impl BoundedText {
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
        field: &'static str,
    ) -> CoreResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(Diagnostic::new("empty", field, "value must not be empty"));
        }
        if value.len() > max_bytes {
            return Err(Diagnostic::new(
                "bound_exceeded",
                field,
                "value exceeds its named byte bound",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        pub struct $name(BoundedText);

        impl $name {
            pub fn new(value: impl Into<String>) -> CoreResult<Self> {
                BoundedText::new(value, DEFAULT_MAX_ID_BYTES, stringify!($name)).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

id_type!(ProjectId);
id_type!(PipelineRevisionId);
id_type!(EvaluationId);
id_type!(RunId);
id_type!(TaskId);
id_type!(AttemptId);
id_type!(CommandId);
id_type!(EffectId);
id_type!(ProviderProfileId);
id_type!(CandidateId);
id_type!(StreamId);
id_type!(PolicyId);
id_type!(CobProfileId);
id_type!(CobObjectId);
id_type!(CobSemanticStateId);
id_type!(NativeEventId);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ContentId(BoundedText);

impl ContentId {
    pub fn from_blake3_hex(value: impl Into<String>) -> CoreResult<Self> {
        let text = BoundedText::new(value, BLAKE3_HEX_BYTES, "content_id")?;
        if text.as_str().len() != BLAKE3_HEX_BYTES {
            return Err(Diagnostic::new(
                "invalid_blake3_length",
                "content_id",
                "BLAKE3 identity must have 64 hexadecimal characters",
            ));
        }
        if !text.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Diagnostic::new(
                "invalid_blake3_hex",
                "content_id",
                "BLAKE3 identity must be hexadecimal",
            ));
        }
        Ok(Self(text))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Alias(BoundedText);

impl Alias {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        BoundedText::new(value, DEFAULT_MAX_ALIAS_BYTES, "alias").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ExternalRef {
    pub contract_version: u16,
    pub provider_namespace: BoundedText,
    pub native_kind: BoundedText,
    pub native_id: BoundedText,
}

impl ExternalRef {
    pub fn new(namespace: &str, native_kind: &str, native_id: &str) -> CoreResult<Self> {
        Ok(Self {
            contract_version: CONTRACT_VERSION_V1,
            provider_namespace: BoundedText::new(
                namespace.to_string(),
                DEFAULT_MAX_ID_BYTES,
                "provider_namespace",
            )?,
            native_kind: BoundedText::new(
                native_kind.to_string(),
                DEFAULT_MAX_ID_BYTES,
                "native_kind",
            )?,
            native_id: BoundedText::new(native_id.to_string(), DEFAULT_MAX_ID_BYTES, "native_id")?,
        })
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.contract_version != CONTRACT_VERSION_V1 {
            return Err(Diagnostic::new(
                "unsupported_version",
                "external_ref.contract_version",
                "external reference version is unsupported",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub max_collection_items: usize,
    pub max_transition_items: usize,
    pub max_diagnostic_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_collection_items: DEFAULT_MAX_COLLECTION_ITEMS,
            max_transition_items: DEFAULT_MAX_TRANSITION_ITEMS,
            max_diagnostic_bytes: DEFAULT_MAX_DIAGNOSTIC_BYTES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOO_LONG_ID_BYTES: usize = DEFAULT_MAX_ID_BYTES + 1;

    #[test]
    fn accepts_namespaced_external_identity() {
        let external = ExternalRef::new("provider-a", "run", "same-text");
        assert!(external.is_ok());
    }

    #[test]
    fn rejects_malformed_content_identity() {
        let invalid = ContentId::from_blake3_hex("not-a-hash");
        assert_eq!(
            invalid.map(|_| ()),
            Err(Diagnostic::new(
                "invalid_blake3_length",
                "content_id",
                "BLAKE3 identity must have 64 hexadecimal characters"
            ))
        );
    }

    #[test]
    fn rejects_oversized_identifier() {
        let invalid = ProjectId::new("x".repeat(TOO_LONG_ID_BYTES));
        assert!(invalid.is_err());
    }
}
