use serde::{Deserialize, Serialize};

const DIGEST_HEX_BYTES: usize = 64;
const MAX_LABEL_BYTES: usize = 128;
const ADJACENT_PAIR_LENGTH: usize = 2;

pub const LOCAL_EXECUTOR_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PROFILE_SCHEMA: &str = "cairn.check-profile.v1";
pub const PLAN_SCHEMA: &str = "cairn.check-evidence-plan.v1";
pub const MAX_CHECKS: usize = 64;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_FEATURES: usize = 64;
pub const MAX_ENVIRONMENT: usize = 32;
pub const MAX_STREAM_BYTES: u64 = 1_048_576;
pub const MAX_DEADLINE_MS: u64 = 3_600_000;
pub const MAX_TEARDOWN_MS: u64 = 60_000;
pub const NON_CLAIMS: [&str; 4] = [
    "a plan does not prove execution or grant authority",
    "supplied observations do not prove check-oracle soundness",
    "process observations do not prove sandboxing or source correctness",
    "producer normalization does not prove release eligibility",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    CargoTest,
    NixCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBounds {
    pub id: String,
    pub deadline_ms: u64,
    pub teardown_ms: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckProfile {
    pub schema: String,
    pub check_id: String,
    pub kind: CheckKind,
    pub executable: String,
    pub executable_hash: String,
    pub tool_cohort_hash: String,
    pub target: String,
    pub features: Vec<String>,
    pub environment: std::collections::BTreeMap<String, String>,
    pub working_root: String,
    pub bounds: ExecutionBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Applicability {
    pub subject_hash: String,
    pub policy_hash: String,
    pub dependency_hashes: Vec<String>,
    /// Identity of the consumer-owned source projection, including exclusions.
    pub source_projection_hash: String,
}

/// Private fields prevent construction or deserialization without admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckEvidencePlan {
    pub(super) schema: String,
    pub(super) contract: crate::verified::agent_receipt::AgentVerificationContract,
    pub(super) applicability: Applicability,
    pub(super) profiles: Vec<CheckProfile>,
    pub(super) plan_hash: String,
    pub(super) non_claims: Vec<String>,
}

impl CheckEvidencePlan {
    pub fn identity(&self) -> &str {
        &self.plan_hash
    }
    pub fn profiles(&self) -> &[CheckProfile] {
        &self.profiles
    }
    pub fn applicability(&self) -> &Applicability {
        &self.applicability
    }
}

/// Safe, finite diagnostic vocabulary. No producer output enters diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProducerIssue {
    Bounds,
    Contract,
    Profile,
    Selection,
    Command,
    Applicability,
    Denied,
    PlanChanged,
    ExecutableChanged,
    Prerequisite,
    Observation,
    PublicationIncomplete,
}

impl std::fmt::Display for ProducerIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "check evidence rejected: {self:?}")
    }
}
impl std::error::Error for ProducerIssue {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessOutcome {
    ExitedZero,
    ExitedNonzero,
    Denied,
    TimedOut,
    Cancelled,
    OutputOverflow,
    TeardownFailed,
    InfrastructureError,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    Test,
    Build,
    StrictLint,
}

/// Normalized facts from an application-owned execution/admission adapter.
/// Deliberately not Deserialize: this is not an invented upstream wire schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttemptObservation {
    pub check_id: String,
    pub attempt_hash: String,
    pub plan_hash: String,
    pub applicability: Applicability,
    pub source_after_hash: String,
    pub executable_hash: String,
    pub tool_cohort_hash: String,
    pub command_hash: String,
    pub producer_identity: String,
    pub native_status: String,
    pub native_non_claims: Vec<String>,
    pub role: EvidenceRole,
    pub outcome: ProcessOutcome,
    pub payload_passed: bool,
    pub executed_tests: u64,
    pub execution_observed: bool,
    pub cache_claimed: bool,
    pub teardown_complete: bool,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedAttempt {
    pub(super) observation: AttemptObservation,
    pub(super) result: crate::verified::agent_receipt::AgentExecutionResult,
}
impl NormalizedAttempt {
    pub fn result(&self) -> &crate::verified::agent_receipt::AgentExecutionResult {
        &self.result
    }
    pub fn observation(&self) -> &AttemptObservation {
        &self.observation
    }
}

/// Publication facts are supplied after the shell writes and reads back artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedArtifact {
    pub expected_hash: String,
    pub observed_hash: String,
    pub durable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationDecision {
    PublishFinalManifest,
    Withhold,
}

pub(super) fn digest(value: &str) -> bool {
    value.len() == DIGEST_HEX_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LABEL_BYTES
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}
pub(super) fn canonical<T: Ord>(values: &[T]) -> bool {
    values
        .windows(ADJACENT_PAIR_LENGTH)
        .all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
mod tests {
    #[test]
    fn digest_accepts_exact_lowercase_hex() {
        let valid = "a".repeat(super::DIGEST_HEX_BYTES);
        assert!(super::digest(&valid));
        assert!(super::digest(&"0".repeat(super::DIGEST_HEX_BYTES)));
    }

    #[test]
    fn digest_rejects_wrong_length_and_alphabet() {
        assert!(!super::digest(""));
        let short_bytes = super::DIGEST_HEX_BYTES
            .checked_sub(1)
            .expect("nonzero width");
        let long_bytes = super::DIGEST_HEX_BYTES
            .checked_add(1)
            .expect("bounded width");
        assert!(!super::digest(&"a".repeat(short_bytes)));
        assert!(!super::digest(&"a".repeat(long_bytes)));
        assert!(!super::digest(&"A".repeat(super::DIGEST_HEX_BYTES)));
        assert!(!super::digest(&"g".repeat(super::DIGEST_HEX_BYTES)));
    }

    #[test]
    fn labels_accept_inclusive_limit_and_supported_separators() {
        assert!(super::label("a"));
        assert!(super::label("a0._-"));
        assert!(super::label(&"a".repeat(super::MAX_LABEL_BYTES)));
    }

    #[test]
    fn labels_reject_empty_overlong_and_invalid_bytes() {
        assert!(!super::label(""));
        let long_bytes = super::MAX_LABEL_BYTES
            .checked_add(1)
            .expect("bounded width");
        assert!(!super::label(&"a".repeat(long_bytes)));
        for invalid in ["0a", "Ab", "a/b", "a b", "aé", "éa"] {
            assert!(!super::label(invalid));
        }
    }

    #[test]
    fn order_accepts_empty_singleton_and_strictly_increasing_values() {
        assert!(super::canonical::<u8>(&[]));
        assert!(super::canonical(&[0]));
        assert!(super::canonical(&[0, 1]));
    }

    #[test]
    fn order_rejects_duplicates_and_descending_values() {
        assert!(!super::canonical(&[0, 0]));
        assert!(!super::canonical(&[1, 0]));
        assert!(!super::canonical(&[0, 1, 0]));
    }
}
