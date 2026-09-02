// Octet: the version-one wire model stays in one reviewable schema module and uses explicit owned
// interchange types.
#![allow(
    unknown_lints,
    non_trait_imports,
    missing_const_fn,
    reason = "one closed schema keeps request and observation compatibility visible; trait methods cannot be const"
)]

use alloc::string::String;
use alloc::vec::Vec;

mod error;
mod observation;
pub use error::*;
pub use observation::*;

pub const PROFILE_SCHEMA: &str = "lattice.workflow-profile.v1";
pub const REQUEST_SCHEMA: &str = "lattice.workflow-request.v2";
pub const GRAPH_SCHEMA: &str = "lattice.workflow-graph.v1";
pub const ACCEPTED_SCHEMA: &str = "lattice.workflow-accepted.v1";
pub const OBSERVATION_SCHEMA: &str = "lattice.workflow-observation.v1";
pub const CONTROL_SCHEMA: &str = "lattice.workflow-control.v1";
pub const LOG_READ_SCHEMA: &str = "lattice.workflow-log-read.v1";
pub const BLAKE3_PREFIX: &str = "b3:";
pub const BLAKE3_HEX_LENGTH: usize = 64;
pub const MAXIMUM_LABEL_BYTES: usize = 128;
pub const MAXIMUM_SOURCE_VALUE_BYTES: usize = 256;
const ASCII_CONTROL_END_EXCLUSIVE: u8 = 32;
const ASCII_DELETE: u8 = 127;
pub const CONTRACT_MAXIMUM_CAPABILITY_ITEMS: u32 = 256;
pub const CONTRACT_MAXIMUM_NODES: u32 = 4_096;
pub const CONTRACT_MAXIMUM_EDGES: u32 = 16_384;
pub const CONTRACT_MAXIMUM_LOG_REFERENCES: u32 = 4_096;
pub const CONTRACT_MAXIMUM_LOG_READ_BYTES: u64 = 16_777_216;
pub const CONTRACT_MAXIMUM_TOTAL_LOG_BYTES: u64 = 268_435_456;

pub const REQUIRED_NON_CLAIMS: [&str; 5] = [
    "workflow admission does not prove workflow correctness",
    "workflow admission does not prove host confinement",
    "workflow observation does not prove an external effect succeeded",
    "workflow observation does not prove credential freshness",
    "workflow observation does not establish release readiness",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity(String);

impl Identity {
    pub fn new(value: String) -> Result<Self, ValueError> {
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
        let mut value = String::from(BLAKE3_PREFIX);
        value.push_str(digest.to_hex().as_str());
        debug_assert!(is_blake3_identity(&value));
        Self(value)
    }
}

impl serde::Serialize for Identity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Identity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label(String);

impl Label {
    pub fn new(value: String) -> Result<Self, ValueError> {
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
    where S: serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Label {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceValue(String);

impl SourceValue {
    pub fn new(value: String) -> Result<Self, ValueError> {
        let has_control = value.bytes().any(|byte| byte < ASCII_CONTROL_END_EXCLUSIVE || byte == ASCII_DELETE);
        if value.is_empty() || value.len() > MAXIMUM_SOURCE_VALUE_BYTES || has_control {
            Err(ValueError::InvalidSourceValue)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for SourceValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SourceValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueError {
    InvalidIdentity,
    InvalidLabel,
    InvalidSourceValue,
}

impl core::fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIdentity => formatter.write_str("value is not a canonical BLAKE3 identity"),
            Self::InvalidLabel => formatter.write_str("value is not a canonical bounded label"),
            Self::InvalidSourceValue => formatter.write_str("value is not a bounded control-free source value"),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize
)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    Read,
    Build,
    Test,
    Publish,
    Notify,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlCapabilities {
    pub cancel: bool,
    pub pause: bool,
    pub resume: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractLimits {
    pub maximum_nodes: u32,
    pub maximum_edges: u32,
    pub maximum_log_references: u32,
    pub maximum_log_read_bytes: u64,
    pub maximum_total_log_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProfile {
    pub schema: String,
    pub graph_schema: String,
    pub platforms: Vec<Label>,
    pub node_kinds: Vec<Label>,
    pub effect_classes: Vec<EffectClass>,
    pub adapter_kinds: Vec<Label>,
    pub controls: ControlCapabilities,
    pub limits: ContractLimits,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KilnLinkage {
    pub effect_identity: Identity,
    pub attempt_identity: Identity,
    pub candidate_identity: Identity,
    pub pipeline_revision_identity: Identity,
    pub workflow_policy_identity: Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceContext {
    pub provider: Label,
    pub repository: SourceValue,
    pub revision: SourceValue,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowNode {
    pub id: Label,
    pub kind: Label,
    pub effect_class: EffectClass,
    pub adapter_kind: Label,
    pub operation_identity: Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEdge {
    pub source: Label,
    pub target: Label,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGraph {
    pub schema: String,
    pub identity: Identity,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedControls {
    pub cancel: bool,
    pub pause: bool,
    pub resume: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationPolicy {
    pub maximum_log_references: u32,
    pub maximum_log_read_bytes: u64,
    pub maximum_total_log_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRequest {
    pub schema: String,
    pub identity: Identity,
    pub idempotency_key: Identity,
    pub profile_identity: Identity,
    pub platform: Label,
    pub kiln: KilnLinkage,
    #[serde(default)]
    pub source: Option<SourceContext>,
    pub graph: WorkflowGraph,
    pub controls: RequestedControls,
    pub observation: ObservationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAccepted {
    pub schema: String,
    pub request_identity: Identity,
    pub idempotency_key: Identity,
    pub workflow_identity: Identity,
    pub execution_identity: Identity,
}

pub(crate) fn is_blake3_identity(value: &str) -> bool {
    let Some(hex) = value.strip_prefix(BLAKE3_PREFIX) else {
        return false;
    };
    hex.len() == BLAKE3_HEX_LENGTH && hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_LABEL_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}
