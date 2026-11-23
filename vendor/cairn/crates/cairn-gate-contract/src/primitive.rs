//! Bounded scalar values for the gate interchange contract.

pub const BLAKE3_PREFIX: &str = "b3:";
pub const BLAKE3_HEX_LENGTH: usize = 64;
pub const GIT_REVISION_HEX_LENGTH: usize = 40;
pub const MAXIMUM_LABEL_BYTES: usize = 128;
pub const MAXIMUM_TEXT_BYTES: usize = 4_096;
const BLAKE3_IDENTITY_BYTES: usize = BLAKE3_PREFIX.len().strict_add(BLAKE3_HEX_LENGTH);
const ASCII_CONTROL_END_EXCLUSIVE: u8 = 32;
const ASCII_DELETE: u8 = 127;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity(alloc::string::String);

impl Identity {
    pub fn new(value: alloc::string::String) -> Result<Self, ValueError> {
        if is_blake3_identity(&value) {
            Ok(Self(value))
        } else {
            Err(ValueError::InvalidIdentity)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn from_digest(digest: blake3::Hash) -> Self {
        let mut value = alloc::string::String::from(BLAKE3_PREFIX);
        value.push_str(digest.to_hex().as_str());
        debug_assert!(is_blake3_identity(&value));
        Self(value)
    }
}

impl serde::Serialize for Identity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Identity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = alloc::string::String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label(alloc::string::String);

impl Label {
    pub fn new(value: alloc::string::String) -> Result<Self, ValueError> {
        if is_label(&value) {
            Ok(Self(value))
        } else {
            Err(ValueError::InvalidLabel)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for Label {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Label {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = alloc::string::String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedText(alloc::string::String);

impl BoundedText {
    pub fn new(value: alloc::string::String) -> Result<Self, ValueError> {
        if is_bounded_text(&value) {
            Ok(Self(value))
        } else {
            Err(ValueError::InvalidText)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for BoundedText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for BoundedText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = alloc::string::String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitRevision(alloc::string::String);

impl GitRevision {
    pub fn new(value: alloc::string::String) -> Result<Self, ValueError> {
        if value.len() == GIT_REVISION_HEX_LENGTH && value.bytes().all(is_lower_hex) {
            Ok(Self(value))
        } else {
            Err(ValueError::InvalidGitRevision)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for GitRevision {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for GitRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = alloc::string::String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueError {
    InvalidIdentity,
    InvalidLabel,
    InvalidText,
    InvalidGitRevision,
}

impl core::fmt::Display for ValueError {
    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            missing_const_fn,
            reason = "Cairn gate-contract owner: core::fmt::Display requires a non-const trait method."
        )
    )]
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIdentity => {
                formatter.write_str("value is not a canonical BLAKE3 identity")
            }
            Self::InvalidLabel => formatter.write_str("value is not a canonical bounded label"),
            Self::InvalidText => formatter.write_str("value is not bounded control-free text"),
            Self::InvalidGitRevision => {
                formatter.write_str("value is not a lowercase full Git revision")
            }
        }
    }
}

#[cfg_attr(
    dylint_lib = "octet",
    allow(
        missing_const_fn,
        reason = "Cairn gate-contract owner: str::starts_with and Iterator::all are not const on the pinned toolchain."
    )
)]
fn is_blake3_identity(value: &str) -> bool {
    value.len() == BLAKE3_IDENTITY_BYTES
        && value.starts_with(BLAKE3_PREFIX)
        && value[BLAKE3_PREFIX.len()..].bytes().all(is_lower_hex)
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn is_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_LABEL_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'-')
        })
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

#[cfg_attr(
    dylint_lib = "octet",
    allow(
        missing_const_fn,
        reason = "Cairn gate-contract owner: Iterator::any cannot run in const evaluation on the pinned toolchain."
    )
)]
fn is_bounded_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_TEXT_BYTES
        && !value
            .bytes()
            .any(|byte| byte < ASCII_CONTROL_END_EXCLUSIVE || byte == ASCII_DELETE)
}
