//! Wire records and schema constants for the gate interchange contract.

pub const REQUEST_SCHEMA: &str = "cairn.gate-request.v1";
pub const RESULT_SCHEMA: &str = "cairn.gate-result.v1";
pub const NATIVE_OBSERVATION_SCHEMA: &str = "cairn.native-gate-observation.v1";
pub const MACHINE_SCHEMA: &str = "cairn.cli-machine.v1";
pub const MACHINE_DIAGNOSTIC_SCHEMA: &str = "cairn.cli-machine-diagnostic.v1";
pub const POLICY_SCHEMA: &str = "cairn.policy-schema.v1";
pub const GATE_RECEIPT_SCHEMA: &str = "cairn.gate-receipt.v1";
pub const MAXIMUM_EVIDENCE_IDENTITIES: usize = 256;
pub const MAXIMUM_FINDINGS: usize = 256;

const REQUIRED_NON_CLAIM_COUNT: usize = 8;
pub const REQUIRED_NON_CLAIMS: [&str; REQUIRED_NON_CLAIM_COUNT] = [
    "gate binding does not prove candidate facts describe the evaluated filesystem",
    "gate binding does not prove source correctness",
    "gate binding does not prove test completeness",
    "gate binding does not prove process reliability",
    "gate binding does not authorize merge or publication",
    "gate binding does not execute a release",
    "gate binding does not prove deployment success",
    "gate binding does not establish future applicability",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStage {
    Proposal,
    Design,
    Tasks,
}

impl GateStage {
    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            fragile_exhaustive_enum_match,
            reason = "Cairn gate-contract owner: every closed-schema stage requires an explicit wire spelling."
        )
    )]
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
    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            fragile_exhaustive_enum_match,
            reason = "Cairn gate-contract owner: every closed-schema verdict requires an explicit wire spelling."
        )
    )]
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
    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            fragile_exhaustive_enum_match,
            reason = "Cairn gate-contract owner: every closed-schema severity requires an explicit wire spelling."
        )
    )]
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
    pub source_revision: crate::GitRevision,
    pub executable_identity: crate::Identity,
    pub machine_schema: alloc::string::String,
    pub diagnostic_schema: alloc::string::String,
    pub policy_schema: alloc::string::String,
    pub gate_receipt_schema: alloc::string::String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateApplicability {
    pub candidate_identity: crate::Identity,
    pub candidate_root_identity: crate::Identity,
    pub pipeline_revision_identity: crate::Identity,
    pub policy_identity: crate::Identity,
    pub evidence_identities: alloc::vec::Vec<crate::Identity>,
    pub provider_profile_identity: crate::Identity,
    pub effect_identity: crate::Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeGateTarget {
    pub change: crate::Label,
    pub stage: GateStage,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRequest {
    pub schema: alloc::string::String,
    pub identity: crate::Identity,
    pub idempotency_key: crate::Identity,
    pub applicability: GateApplicability,
    pub target: ChangeGateTarget,
    pub tool: ToolCohort,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateFinding {
    pub code: crate::Label,
    pub message: crate::BoundedText,
    pub scope: crate::Label,
    pub promoted: crate::Label,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGateObservation {
    pub schema: alloc::string::String,
    pub change: crate::Label,
    pub stage: GateStage,
    pub verdict: GateVerdict,
    pub valid: bool,
    pub input_identity: crate::Identity,
    pub policy_identity: crate::Identity,
    pub receipt_identity: crate::Identity,
    pub findings: alloc::vec::Vec<GateFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineDiagnostic {
    pub schema: alloc::string::String,
    pub code: crate::Label,
    pub severity: DiagnosticSeverity,
    pub message: crate::BoundedText,
    pub target: crate::BoundedText,
    pub fix: Option<crate::BoundedText>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub schema: alloc::string::String,
    pub identity: crate::Identity,
    pub request_identity: crate::Identity,
    pub applicability: GateApplicability,
    pub tool: ToolCohort,
    pub observation: NativeGateObservation,
    pub non_claims: alloc::vec::Vec<crate::BoundedText>,
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
    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            missing_const_fn,
            reason = "Cairn gate-contract owner: core::fmt::Display requires a non-const trait method."
        )
    )]
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
