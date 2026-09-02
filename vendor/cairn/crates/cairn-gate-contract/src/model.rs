use alloc::{string::String, vec::Vec};

pub const REQUEST_SCHEMA: &str = "cairn.gate-request.v1";
pub const RESULT_SCHEMA: &str = "cairn.gate-result.v1";
pub const NATIVE_OBSERVATION_SCHEMA: &str = "cairn.native-gate-observation.v1";
pub const MACHINE_SCHEMA: &str = "cairn.cli-machine.v1";
pub const MACHINE_DIAGNOSTIC_SCHEMA: &str = "cairn.cli-machine-diagnostic.v1";
pub const POLICY_SCHEMA: &str = "cairn.policy-schema.v1";
pub const GATE_RECEIPT_SCHEMA: &str = "cairn.gate-receipt.v1";
pub const BLAKE3_PREFIX: &str = "b3:";
pub const BLAKE3_HEX_LENGTH: usize = 64;
pub const GIT_REVISION_HEX_LENGTH: usize = 40;
pub const MAXIMUM_LABEL_BYTES: usize = 128;
pub const MAXIMUM_TEXT_BYTES: usize = 4_096;
pub const MAXIMUM_EVIDENCE_IDENTITIES: usize = 256;
pub const MAXIMUM_FINDINGS: usize = 256;
const ASCII_CONTROL_END_EXCLUSIVE: u8 = 32;
const ASCII_DELETE: u8 = 127;

pub const REQUIRED_NON_CLAIMS: [&str; 8] = [
    "gate binding does not prove candidate facts describe the evaluated filesystem",
    "gate binding does not prove source correctness",
    "gate binding does not prove test completeness",
    "gate binding does not prove process reliability",
    "gate binding does not authorize merge or publication",
    "gate binding does not execute a release",
    "gate binding does not prove deployment success",
    "gate binding does not establish future applicability",
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
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedText(String);

impl BoundedText {
    pub fn new(value: String) -> Result<Self, ValueError> {
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
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitRevision(String);

impl GitRevision {
    pub fn new(value: String) -> Result<Self, ValueError> {
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
        let value = String::deserialize(deserializer)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStage {
    Proposal,
    Design,
    Tasks,
}

impl GateStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Design => "design",
            Self::Tasks => "tasks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Pass,
    Block,
}

impl GateVerdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCohort {
    pub source_revision: GitRevision,
    pub executable_identity: Identity,
    pub machine_schema: String,
    pub diagnostic_schema: String,
    pub policy_schema: String,
    pub gate_receipt_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateApplicability {
    pub candidate_identity: Identity,
    pub candidate_root_identity: Identity,
    pub pipeline_revision_identity: Identity,
    pub policy_identity: Identity,
    pub evidence_identities: Vec<Identity>,
    pub provider_profile_identity: Identity,
    pub effect_identity: Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeGateTarget {
    pub change: Label,
    pub stage: GateStage,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRequest {
    pub schema: String,
    pub identity: Identity,
    pub idempotency_key: Identity,
    pub applicability: GateApplicability,
    pub target: ChangeGateTarget,
    pub tool: ToolCohort,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateFinding {
    pub code: Label,
    pub message: BoundedText,
    pub scope: Label,
    pub promoted: Label,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGateObservation {
    pub schema: String,
    pub change: Label,
    pub stage: GateStage,
    pub verdict: GateVerdict,
    pub valid: bool,
    pub input_identity: Identity,
    pub policy_identity: Identity,
    pub receipt_identity: Identity,
    pub findings: Vec<GateFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineDiagnostic {
    pub schema: String,
    pub code: Label,
    pub severity: DiagnosticSeverity,
    pub message: BoundedText,
    pub target: BoundedText,
    pub fix: Option<BoundedText>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub schema: String,
    pub identity: Identity,
    pub request_identity: Identity,
    pub applicability: GateApplicability,
    pub tool: ToolCohort,
    pub observation: NativeGateObservation,
    pub non_claims: Vec<BoundedText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    UnsupportedSchema,
    RequestIdentityMismatch,
    ResultIdentityMismatch,
    EvidenceLimitExceeded,
    EvidenceNotCanonical,
    FindingLimitExceeded,
    InvalidToolCohort,
    ApplicabilityMismatch,
    ToolCohortMismatch,
    TargetMismatch,
    PolicyMismatch,
    ContradictoryVerdict,
    InvalidNonClaims,
    InvalidDiagnostic,
}

impl core::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedSchema => "unsupported gate interchange schema",
            Self::RequestIdentityMismatch => "gate request identity does not match its content",
            Self::ResultIdentityMismatch => "gate result identity does not match its content",
            Self::EvidenceLimitExceeded => "gate request evidence limit exceeded",
            Self::EvidenceNotCanonical => {
                "gate request evidence identities are not sorted and unique"
            }
            Self::FindingLimitExceeded => "native gate finding limit exceeded",
            Self::InvalidToolCohort => "gate tool cohort is not admitted",
            Self::ApplicabilityMismatch => "gate result applicability does not match the request",
            Self::ToolCohortMismatch => "gate result tool cohort does not match the request",
            Self::TargetMismatch => "native gate target does not match the request",
            Self::PolicyMismatch => "native gate policy does not match the request",
            Self::ContradictoryVerdict => {
                "native gate verdict contradicts its validity or findings"
            }
            Self::InvalidNonClaims => "gate result non-claims do not match the contract",
            Self::InvalidDiagnostic => "machine diagnostic is not admitted",
        })
    }
}

fn is_blake3_identity(value: &str) -> bool {
    value.len() == BLAKE3_PREFIX.len() + BLAKE3_HEX_LENGTH
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

fn is_bounded_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_TEXT_BYTES
        && !value
            .bytes()
            .any(|byte| byte < ASCII_CONTROL_END_EXCLUSIVE || byte == ASCII_DELETE)
}
