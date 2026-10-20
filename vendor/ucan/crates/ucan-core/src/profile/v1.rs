//! Bounded no-std UCAN `1.0.0` wire-profile types and admission.
//!
//! This module owns deterministic protocol meaning for the final-profile wire
//! slice: nominal payload tags, slash commands, nonce, timestamps, bounded
//! IPLD values, proof references, and Delegation/Invocation field-shape
//! admission. It performs no I/O, clock reads, key lookup, or signing. The
//! std shell owns DAG-CBOR, CID, Varsig, and cryptographic adapters.
//!
//! The module is `no_std` with `alloc`. It never formats diagnostics with the
//! global allocator unless a caller requests an error description.

#![allow(clippy::module_name_repetitions)]

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
#[allow(unused_imports)] // required for Vec resolution under this no-std toolchain prelude gating
use alloc::vec::Vec;
use core::fmt;

pub const WIRE_VERSION: &str = "1.0.0";
pub const DELEGATION_TAG: &str = "ucan/dlg@1.0.0";
pub const INVOCATION_TAG: &str = "ucan/inv@1.0.0";

pub const MAX_URI_FIELD_BYTES: usize = 2_048;
pub const MAX_SLASH_COMMAND_BYTES: usize = 256;
pub const MAX_NONCE_BYTES: usize = 64;
pub const MAX_IPLD_VALUE_DEPTH: usize = 32;
pub const MAX_IPLD_LIST_ITEMS: usize = 1_024;
pub const MAX_IPLD_MAP_ITEMS: usize = 1_024;
pub const MAX_PROOF_REFERENCES: usize = 64;
pub const MAX_PAYLOAD_FIELDS: usize = 16;

/// Largest exact safe-integer timestamp: `2^53 - 1`.
pub const MAX_TIMESTAMP_ABS: i64 = (1 << 53) - 1;
/// Smallest exact safe-integer timestamp: `-(2^53 - 1)`.
pub const MIN_TIMESTAMP_ABS: i64 = -(MAX_TIMESTAMP_ABS);

/// Field identifiers of a Delegation or Invocation payload.
pub const FIELD_ISSUER: &str = "iss";
pub const FIELD_AUDIENCE: &str = "aud";
pub const FIELD_SUBJECT: &str = "sub";
pub const FIELD_COMMAND: &str = "cmd";
pub const FIELD_ARGS: &str = "args";
pub const FIELD_POLICY: &str = "pol";
pub const FIELD_EXPIRES: &str = "exp";
pub const FIELD_NOT_BEFORE: &str = "nbf";
pub const FIELD_NONCE: &str = "nonce";
pub const FIELD_FACTS: &str = "fct";
pub const FIELD_PROOFS: &str = "prf";
/// Envelope tag annotation field.
pub const FIELD_TAG_ANNOTATION: &str = "tag";
/// Envelope version annotation field.
pub const FIELD_VERSION: &str = "version";

/// Deterministic wire-profile admission error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireError {
    UnknownField { field: String },
    MissingField { field: String },
    ForbiddenField { field: String },
    WrongKind { expected: SpecKind, actual: SpecKind },
    WrongType { field: String },
    InvalidCommand { message: &'static str },
    TimestampOutOfBounds { field: String },
    InvertedTimeWindow,
    NonceTooLarge { len: usize },
    UriFieldTooLarge { field: String, len: usize },
    TooManyFields { count: usize },
    TooManyProofReferences { count: usize },
    ValueTooDeep { depth: usize },
    ValueTooLarge { items: usize },
    UnsupportedTag { tag: String },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField { field } => write!(formatter, "unknown profile field {field}"),
            Self::MissingField { field } => write!(formatter, "missing profile field {field}"),
            Self::ForbiddenField { field } => write!(formatter, "field {field} is forbidden for this payload kind"),
            Self::WrongKind { expected, actual } => {
                write!(formatter, "expected {expected:?} payload, found {actual:?}")
            }
            Self::WrongType { field } => write!(formatter, "field {field} has the wrong value type"),
            Self::InvalidCommand { message } => write!(formatter, "invalid slash command: {message}"),
            Self::TimestampOutOfBounds { field } => {
                write!(formatter, "timestamp {field} is outside the safe-integer bounds")
            }
            Self::InvertedTimeWindow => write!(formatter, "not-before must not exceed expires"),
            Self::NonceTooLarge { len } => write!(formatter, "nonce is {len} bytes; the limit is {MAX_NONCE_BYTES}"),
            Self::UriFieldTooLarge { field, len } => {
                write!(formatter, "field {field} is {len} bytes; the limit is {MAX_URI_FIELD_BYTES}")
            }
            Self::TooManyFields { count } => {
                write!(formatter, "payload has {count} fields; the limit is {MAX_PAYLOAD_FIELDS}")
            }
            Self::TooManyProofReferences { count } => {
                write!(formatter, "payload has {count} proofs; the limit is {MAX_PROOF_REFERENCES}")
            }
            Self::ValueTooDeep { depth } => {
                write!(formatter, "IPLD value depth {depth} exceeds {MAX_IPLD_VALUE_DEPTH}")
            }
            Self::ValueTooLarge { items } => {
                write!(formatter, "IPLD value has {items} items; the limit is {MAX_IPLD_LIST_ITEMS}")
            }
            Self::UnsupportedTag { tag } => write!(formatter, "unsupported payload tag {tag}"),
        }
    }
}

/// Nominal payload kind. Delegation and Invocation are different nominal
/// types; a Delegation cannot enter an Invocation API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecKind {
    Delegation,
    Invocation,
}

impl SpecKind {
    /// Return the nominal tag string for this kind.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Delegation => DELEGATION_TAG,
            Self::Invocation => INVOCATION_TAG,
        }
    }

    /// Parse a tag string, failing closed on unknown values.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            DELEGATION_TAG => Some(Self::Delegation),
            INVOCATION_TAG => Some(Self::Invocation),
            _ => None,
        }
    }
}

/// A validated final-profile slash command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashCommand(String);

impl SlashCommand {
    /// Build a validated slash command.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the command has an empty segment, a double
    /// or trailing separator, a control character, whitespace inside a
    /// segment, or exceeds the length limit.
    pub fn new(value: &str) -> Result<Self, WireError> {
        let len = value.len();
        if !(2..=MAX_SLASH_COMMAND_BYTES).contains(&len) {
            return Err(WireError::InvalidCommand {
                message: "length outside 2..=256",
            });
        }
        let bytes = value.as_bytes();
        if bytes[0] != b'/' {
            return Err(WireError::InvalidCommand {
                message: "must start with a slash",
            });
        }
        if bytes[bytes.len() - 1] == b'/' {
            return Err(WireError::InvalidCommand {
                message: "must not end with a slash",
            });
        }
        for window in bytes.windows(2) {
            if window[0] == b'/' && window[1] == b'/' {
                return Err(WireError::InvalidCommand {
                    message: "empty segment",
                });
            }
        }
        for &byte in bytes {
            if byte < 0x20 || byte == 0x7F {
                return Err(WireError::InvalidCommand {
                    message: "control character",
                });
            }
            if (byte == b' ' || byte == b'\t') || (byte == b',' || byte == b';') {
                return Err(WireError::InvalidCommand {
                    message: "unsupported separator character",
                });
            }
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A validated nonce. The wire slice treats the nonce as opaque bounded
/// bytes; it performs no nonce-store or replay semantics here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Nonce(Vec<u8>);

impl Nonce {
    /// Build a nonce with the bounded maximum length.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when `bytes` exceeds [`MAX_NONCE_BYTES`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, WireError> {
        if bytes.len() > MAX_NONCE_BYTES {
            return Err(WireError::NonceTooLarge { len: bytes.len() });
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// A proof reference: the CID bytes of a referenced Delegation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRef(Vec<u8>);

impl ProofRef {
    /// Build a proof reference from opaque CID bytes.
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// A bounded IPLD value subset used for args, policy, and facts at the wire
/// layer. Authorization semantics belong to the authorization slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpldValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(i64),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<Self>),
    Map(Vec<(String, Self)>),
}

impl IpldValue {
    /// Build a value with bounded depth and item counts.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the value exceeds the depth, list, or map
    /// limits.
    pub fn new(value: Self) -> Result<Self, WireError> {
        value.check_depth(0)?;
        Ok(value)
    }

    pub(crate) fn check_depth(&self, depth: usize) -> Result<(), WireError> {
        if depth > MAX_IPLD_VALUE_DEPTH {
            return Err(WireError::ValueTooDeep { depth });
        }
        match self {
            Self::List(items) => {
                if items.len() > MAX_IPLD_LIST_ITEMS {
                    return Err(WireError::ValueTooLarge { items: items.len() });
                }
                for item in items {
                    item.check_depth(depth + 1)?;
                }
            }
            Self::Map(items) => {
                if items.len() > MAX_IPLD_MAP_ITEMS {
                    return Err(WireError::ValueTooLarge { items: items.len() });
                }
                for (_, item) in items {
                    item.check_depth(depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Field-shape admission helper. Validates required, optional, and
/// forbidden fields for a payload kind before the shell allocates typed
/// payload storage.
#[derive(Clone, Debug, Default)]
pub struct FieldAdmission {
    kind: Option<SpecKind>,
    present: Vec<String>,
}

impl FieldAdmission {
    /// Begin admission for a payload kind.
    #[must_use]
    pub const fn for_kind(kind: SpecKind) -> Self {
        Self {
            kind: Some(kind),
            present: vec![],
        }
    }

    /// Record a present field with its decoded byte length.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the field is unknown, too large, exceeds
    /// the field count, or is forbidden for the kind.
    pub fn observe(&mut self, field: &str, byte_len: usize) -> Result<(), WireError> {
        let kind = self.kind.ok_or(WireError::UnsupportedTag { tag: String::new() })?;
        if !is_known_field(field) {
            return Err(WireError::UnknownField {
                field: field.to_owned(),
            });
        }
        if self.present.len() >= MAX_PAYLOAD_FIELDS {
            return Err(WireError::TooManyFields {
                count: self.present.len(),
            });
        }
        let allowed = matches!(
            field,
            FIELD_ISSUER
                | FIELD_AUDIENCE
                | FIELD_SUBJECT
                | FIELD_COMMAND
                | FIELD_POLICY
                | FIELD_EXPIRES
                | FIELD_NOT_BEFORE
                | FIELD_NONCE
                | FIELD_FACTS
                | FIELD_PROOFS
                | FIELD_TAG_ANNOTATION
                | FIELD_VERSION
        ) || (kind == SpecKind::Invocation && field == FIELD_ARGS);
        if !allowed {
            return Err(WireError::ForbiddenField {
                field: field.to_owned(),
            });
        }
        if byte_len > MAX_URI_FIELD_BYTES {
            return Err(WireError::UriFieldTooLarge {
                field: field.to_owned(),
                len: byte_len,
            });
        }
        self.present.push(field.to_owned());
        Ok(())
    }

    /// Finish admission: assert required fields.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when a required field is missing.
    pub fn admit(self) -> Result<(), WireError> {
        let kind = self.kind.ok_or(WireError::UnsupportedTag { tag: String::new() })?;
        require(self.present.iter().any(|f| f == FIELD_ISSUER), FIELD_ISSUER)?;
        require(self.present.iter().any(|f| f == FIELD_AUDIENCE), FIELD_AUDIENCE)?;
        match kind {
            SpecKind::Delegation => {}
            SpecKind::Invocation => {
                require(self.present.iter().any(|f| f == FIELD_COMMAND), FIELD_COMMAND)?;
                require(self.present.iter().any(|f| f == FIELD_PROOFS), FIELD_PROOFS)?;
            }
        }
        Ok(())
    }
}

fn require(satisfied: bool, field: &str) -> Result<(), WireError> {
    if satisfied {
        Ok(())
    } else {
        Err(WireError::MissingField {
            field: field.to_owned(),
        })
    }
}

/// A bounded timestamp value with safe-integer enforcement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeSeconds(i64);

impl TimeSeconds {
    /// Build a timestamp inside the safe-integer bounds.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::TimestampOutOfBounds`] outside
    /// `-(2^53 - 1) ..= 2^53 - 1`.
    pub fn new(value: i64) -> Result<Self, WireError> {
        if !(MIN_TIMESTAMP_ABS..=MAX_TIMESTAMP_ABS).contains(&value) {
            return Err(WireError::TimestampOutOfBounds {
                field: "timestamp".to_owned(),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self.0
    }
}

/// Validate a time window: each bound is safe and `nbf <= exp`.
///
/// # Errors
///
/// Returns [`WireError`] for an out-of-bounds timestamp or an inverted
/// window.
pub fn admit_time_window(nbf: Option<i64>, exp: Option<i64>) -> Result<(), WireError> {
    if let Some(nbf_value) = nbf {
        TimeSeconds::new(nbf_value)?;
    }
    if let Some(exp_value) = exp {
        TimeSeconds::new(exp_value)?;
    }
    if let (Some(nbf_value), Some(exp_value)) = (nbf, exp)
        && nbf_value > exp_value
    {
        return Err(WireError::InvertedTimeWindow);
    }
    Ok(())
}

fn is_known_field(field: &str) -> bool {
    matches!(
        field,
        FIELD_ISSUER
            | FIELD_AUDIENCE
            | FIELD_SUBJECT
            | FIELD_COMMAND
            | FIELD_ARGS
            | FIELD_POLICY
            | FIELD_EXPIRES
            | FIELD_NOT_BEFORE
            | FIELD_NONCE
            | FIELD_FACTS
            | FIELD_PROOFS
            | FIELD_TAG_ANNOTATION
            | FIELD_VERSION
    )
}

#[cfg(test)]
mod tests;
