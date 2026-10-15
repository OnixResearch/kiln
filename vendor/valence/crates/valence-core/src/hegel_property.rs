//! Bounded Hegel property-test receipt model.
//!
//! This module is a pure functional core for recording Hegel property-test runs
//! as generated-input evidence. It validates in-memory receipt DTOs only; test
//! execution, filesystem discovery, and CI orchestration remain in tests or
//! shells.

use serde::Deserialize;
use serde::Serialize;

/// Schema identifier for Hegel property receipts.
pub const HEGEL_PROPERTY_RECEIPT_SCHEMA: &str = "valence.hegel-property-receipt.v1";
/// Evidence tier used for Hegel property receipts.
pub const HEGEL_GENERATED_TESTING_TIER: &str = "generated_testing";
/// Required non-claim boundary for Hegel property receipts.
pub const HEGEL_GENERATED_TESTING_NON_CLAIM: &str = "Hegel checks bounded generated inputs only and does not prove correctness, semantic equivalence, verifier soundness, requirement satisfaction, release eligibility, or whole-program safety";
/// Stable command for the fast Hegel lane.
pub const HEGEL_FAST_LANE_COMMAND: &str =
    "nix develop --option builders '' -c cargo test -p valence-core --test hegel_property_laws";
/// Stable command for the opt-in deep Hegel lane.
pub const HEGEL_DEEP_LANE_COMMAND: &str = "nix develop --option builders '' -c cargo test -p valence-core --test hegel_property_laws -- --ignored --nocapture";
/// Named case budget for ordinary Hegel property tests.
pub const HEGEL_FAST_CASE_BUDGET: u64 = 16;
/// Named case budget for opt-in deep Hegel property tests.
pub const HEGEL_DEEP_CASE_BUDGET: u64 = 256;

const BLAKE3_IDENTITY_PREFIX: &str = "b3:";
const BLAKE3_HEX_LENGTH: usize = 64;
const MINIMUM_CASE_BUDGET: u64 = 1;
const ISSUE_AMBIGUOUS_SETTINGS: &str = "ambiguous_settings";
const ISSUE_ARTIFACT_HASH_MISSING: &str = "artifact_hash_missing";
const ISSUE_COUNTEREXAMPLE_MISSING: &str = "counterexample_missing";
const ISSUE_COUNTEREXAMPLE_HASH_MALFORMED: &str = "counterexample_hash_malformed";
const ISSUE_EVIDENCE_TIER_UNSUPPORTED: &str = "unsupported_evidence_tier";
const ISSUE_GENERATOR_HASH_MALFORMED: &str = "generator_hash_malformed";
const ISSUE_MISSING_IDENTITY: &str = "missing_identity";
const ISSUE_NON_CLAIMS_MISSING: &str = "non_claims_missing";
const ISSUE_OVERCLAIM_DETECTED: &str = "overclaim_detected";
const ISSUE_RECEIPT_HASH_MALFORMED: &str = "receipt_hash_malformed";
const ISSUE_RECEIPT_HASH_MISMATCH: &str = "receipt_hash_mismatch";
const ISSUE_SCHEMA_MISMATCH: &str = "schema_mismatch";
const ISSUE_SOURCE_HASH_MALFORMED: &str = "source_hash_malformed";
const ISSUE_STATUS_COUNTEREXAMPLE_CONFLICT: &str = "status_counterexample_conflict";

const OVERCLAIM_MARKERS: &[&str] = &[
    "formal proof",
    "exhaustive correctness",
    "proves correctness",
    "proves semantic equivalence",
    "proves verifier soundness",
    "proves requirement satisfaction",
    "proves release eligibility",
    "proves whole-program safety",
    "whole-program correctness",
];

/// Status reported by a Hegel property run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HegelPropertyStatus {
    /// All generated cases and explicit examples passed.
    Passed,
    /// Hegel found at least one counterexample.
    Failed,
}

impl HegelPropertyStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// Hegel runner identity recorded by the test shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelRunnerIdentity {
    pub crate_name: String,
    pub crate_version: String,
    pub backend: String,
}

impl HegelRunnerIdentity {
    #[must_use]
    pub fn new(crate_name: impl Into<String>, crate_version: impl Into<String>, backend: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            crate_version: crate_version.into(),
            backend: backend.into(),
        }
    }
}

/// Deterministic Hegel runner settings relevant to replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelRunnerSettings {
    pub case_budget: u64,
    pub phases: Vec<String>,
    pub seed: Option<u64>,
    pub derandomized: bool,
    pub database: Option<String>,
}

impl HegelRunnerSettings {
    #[must_use]
    pub fn fast() -> Self {
        Self {
            case_budget: HEGEL_FAST_CASE_BUDGET,
            phases: sorted_dedup_strings(vec!["explicit".to_owned(), "generate".to_owned(), "shrink".to_owned()]),
            seed: None,
            derandomized: true,
            database: None,
        }
    }
}

/// Generator identity used by a property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelGeneratorIdentity {
    pub family: String,
    pub content_hash: String,
}

impl HegelGeneratorIdentity {
    #[must_use]
    pub fn new(family: impl Into<String>, content_hash: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            content_hash: content_hash.into(),
        }
    }
}

/// Minimal failing example identity for failed Hegel runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelCounterexample {
    pub summary_hash: String,
    pub issue_codes: Vec<String>,
}

impl HegelCounterexample {
    #[must_use]
    pub fn new(summary_hash: impl Into<String>, issue_codes: Vec<String>) -> Self {
        Self {
            summary_hash: summary_hash.into(),
            issue_codes: sorted_dedup_strings(issue_codes),
        }
    }
}

/// Input used to build a receipt before its receipt hash is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelPropertyReceiptInput {
    pub property_id: String,
    pub requirement_ids: Vec<String>,
    pub checked_scope: String,
    pub runner: HegelRunnerIdentity,
    pub settings: HegelRunnerSettings,
    pub generator: HegelGeneratorIdentity,
    pub status: HegelPropertyStatus,
    pub issue_codes: Vec<String>,
    pub counterexample: Option<HegelCounterexample>,
    pub source_hashes: Vec<String>,
    pub parent_artifacts: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Canonical Hegel property receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelPropertyReceipt {
    pub schema: String,
    pub property_id: String,
    pub requirement_ids: Vec<String>,
    pub checked_scope: String,
    pub runner: HegelRunnerIdentity,
    pub settings: HegelRunnerSettings,
    pub generator: HegelGeneratorIdentity,
    pub evidence_tier: String,
    pub status: HegelPropertyStatus,
    pub issue_codes: Vec<String>,
    pub counterexample: Option<HegelCounterexample>,
    pub source_hashes: Vec<String>,
    pub parent_artifacts: Vec<String>,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
}

/// Receipt validation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelPropertyReceiptReport {
    pub valid: bool,
    pub issue_codes: Vec<String>,
    pub receipt_hash: Option<String>,
    pub evidence_tier: String,
    pub non_claims: Vec<String>,
}

/// Hegel lane kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HegelSuiteLaneKind {
    Fast,
    Deep,
}

/// Hegel suite lane definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HegelSuiteLane {
    pub id: String,
    pub kind: HegelSuiteLaneKind,
    pub command: String,
    pub case_budget: u64,
    pub opt_in: bool,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct HegelPropertyReceiptHashPayload<'a> {
    schema: &'a str,
    property_id: &'a str,
    requirement_ids: &'a [String],
    checked_scope: &'a str,
    runner: &'a HegelRunnerIdentity,
    settings: &'a HegelRunnerSettings,
    generator: &'a HegelGeneratorIdentity,
    evidence_tier: &'a str,
    status: &'a str,
    issue_codes: &'a [String],
    counterexample: &'a Option<HegelCounterexample>,
    source_hashes: &'a [String],
    parent_artifacts: &'a [String],
    non_claims: &'a [String],
}

/// Builds a canonical Hegel property receipt.
///
/// r[impl valence.evidence_ir.hegel_property_receipts]
pub fn build_hegel_property_receipt(
    input: HegelPropertyReceiptInput,
) -> Result<HegelPropertyReceipt, HegelPropertyReceiptError> {
    let mut receipt = HegelPropertyReceipt {
        schema: HEGEL_PROPERTY_RECEIPT_SCHEMA.to_owned(),
        property_id: input.property_id,
        requirement_ids: sorted_dedup_strings(input.requirement_ids),
        checked_scope: input.checked_scope,
        runner: input.runner,
        settings: input.settings,
        generator: input.generator,
        evidence_tier: HEGEL_GENERATED_TESTING_TIER.to_owned(),
        status: input.status,
        issue_codes: sorted_dedup_strings(input.issue_codes),
        counterexample: input.counterexample,
        source_hashes: sorted_dedup_strings(input.source_hashes),
        parent_artifacts: sorted_dedup_strings(input.parent_artifacts),
        receipt_hash: String::new(),
        non_claims: sorted_dedup_strings(input.non_claims),
    };
    receipt.receipt_hash = hegel_property_receipt_hash(&receipt)?;
    Ok(receipt)
}

/// Computes the canonical BLAKE3 identity for a Hegel receipt.
///
/// r[impl valence.evidence_ir.hegel_property_receipts]
pub fn hegel_property_receipt_hash(receipt: &HegelPropertyReceipt) -> Result<String, HegelPropertyReceiptError> {
    let payload = HegelPropertyReceiptHashPayload {
        schema: &receipt.schema,
        property_id: &receipt.property_id,
        requirement_ids: &receipt.requirement_ids,
        checked_scope: &receipt.checked_scope,
        runner: &receipt.runner,
        settings: &receipt.settings,
        generator: &receipt.generator,
        evidence_tier: &receipt.evidence_tier,
        status: receipt.status.as_str(),
        issue_codes: &receipt.issue_codes,
        counterexample: &receipt.counterexample,
        source_hashes: &receipt.source_hashes,
        parent_artifacts: &receipt.parent_artifacts,
        non_claims: &receipt.non_claims,
    };
    let bytes = serde_json::to_vec(&payload).map_err(|error| HegelPropertyReceiptError::CanonicalSerialization {
        detail: error.to_string(),
    })?;
    Ok(render_b3_hash(&bytes))
}

/// Validates a Hegel property receipt.
///
/// r[impl valence.evidence_ir.hegel_property_receipts.fail_closed]
/// r[impl valence.evidence_ir.hegel_property_receipts.non_claims]
#[must_use]
pub fn validate_hegel_property_receipt(receipt: &HegelPropertyReceipt) -> HegelPropertyReceiptReport {
    let mut issue_codes = Vec::new();

    if receipt.schema != HEGEL_PROPERTY_RECEIPT_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH.to_owned());
    }
    if receipt.property_id.trim().is_empty()
        || receipt.requirement_ids.is_empty()
        || receipt.checked_scope.trim().is_empty()
        || receipt.runner.crate_name.trim().is_empty()
        || receipt.runner.crate_version.trim().is_empty()
        || receipt.runner.backend.trim().is_empty()
        || receipt.generator.family.trim().is_empty()
    {
        issue_codes.push(ISSUE_MISSING_IDENTITY.to_owned());
    }
    if receipt.settings.case_budget < MINIMUM_CASE_BUDGET || receipt.settings.phases.is_empty() {
        issue_codes.push(ISSUE_AMBIGUOUS_SETTINGS.to_owned());
    }
    if receipt.evidence_tier != HEGEL_GENERATED_TESTING_TIER {
        issue_codes.push(ISSUE_EVIDENCE_TIER_UNSUPPORTED.to_owned());
    }
    if !is_b3_identity(&receipt.generator.content_hash) {
        issue_codes.push(ISSUE_GENERATOR_HASH_MALFORMED.to_owned());
    }
    if receipt.source_hashes.is_empty() {
        issue_codes.push(ISSUE_ARTIFACT_HASH_MISSING.to_owned());
    }
    if receipt.source_hashes.iter().any(|hash| !is_b3_identity(hash)) {
        issue_codes.push(ISSUE_SOURCE_HASH_MALFORMED.to_owned());
    }
    if !is_b3_identity(&receipt.receipt_hash) {
        issue_codes.push(ISSUE_RECEIPT_HASH_MALFORMED.to_owned());
    }
    match hegel_property_receipt_hash(receipt) {
        Ok(expected) if expected != receipt.receipt_hash => {
            issue_codes.push(ISSUE_RECEIPT_HASH_MISMATCH.to_owned());
        }
        Ok(_) => {}
        Err(_) => issue_codes.push(ISSUE_RECEIPT_HASH_MISMATCH.to_owned()),
    }
    match (receipt.status, receipt.counterexample.as_ref()) {
        (HegelPropertyStatus::Failed, Some(counterexample)) => {
            if !is_b3_identity(&counterexample.summary_hash) {
                issue_codes.push(ISSUE_COUNTEREXAMPLE_HASH_MALFORMED.to_owned());
            }
        }
        (HegelPropertyStatus::Failed, None) => issue_codes.push(ISSUE_COUNTEREXAMPLE_MISSING.to_owned()),
        (HegelPropertyStatus::Passed, Some(_)) => issue_codes.push(ISSUE_STATUS_COUNTEREXAMPLE_CONFLICT.to_owned()),
        (HegelPropertyStatus::Passed, None) => {}
    }
    if !receipt.non_claims.iter().any(|claim| claim == HEGEL_GENERATED_TESTING_NON_CLAIM) {
        issue_codes.push(ISSUE_NON_CLAIMS_MISSING.to_owned());
    }
    let claim_text = receipt.non_claims.join(" ");
    if OVERCLAIM_MARKERS.iter().any(|marker| claim_text.contains(marker)) {
        issue_codes.push(ISSUE_OVERCLAIM_DETECTED.to_owned());
    }

    let issue_codes = sorted_dedup_strings(issue_codes);
    HegelPropertyReceiptReport {
        valid: issue_codes.is_empty(),
        issue_codes,
        receipt_hash: Some(receipt.receipt_hash.clone()).filter(|hash| is_b3_identity(hash)),
        evidence_tier: receipt.evidence_tier.clone(),
        non_claims: receipt.non_claims.clone(),
    }
}

/// Default Valence Hegel suite lanes.
///
/// r[impl valence.evidence_ir.hegel_suite_lanes.fast]
/// r[impl valence.evidence_ir.hegel_suite_lanes.deep]
#[must_use]
pub fn default_hegel_suite_lanes() -> Vec<HegelSuiteLane> {
    vec![
        HegelSuiteLane {
            id: "hegel-fast".to_owned(),
            kind: HegelSuiteLaneKind::Fast,
            command: HEGEL_FAST_LANE_COMMAND.to_owned(),
            case_budget: HEGEL_FAST_CASE_BUDGET,
            opt_in: false,
            non_claims: vec![HEGEL_GENERATED_TESTING_NON_CLAIM.to_owned()],
        },
        HegelSuiteLane {
            id: "hegel-deep".to_owned(),
            kind: HegelSuiteLaneKind::Deep,
            command: HEGEL_DEEP_LANE_COMMAND.to_owned(),
            case_budget: HEGEL_DEEP_CASE_BUDGET,
            opt_in: true,
            non_claims: vec![HEGEL_GENERATED_TESTING_NON_CLAIM.to_owned()],
        },
    ]
}

/// Error returned when canonical receipt hashing fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HegelPropertyReceiptError {
    CanonicalSerialization { detail: String },
}

impl std::fmt::Display for HegelPropertyReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CanonicalSerialization { detail } => write!(f, "canonicalize Hegel property receipt: {detail}"),
        }
    }
}

impl std::error::Error for HegelPropertyReceiptError {}

#[must_use]
pub fn render_b3_hash(bytes: &[u8]) -> String {
    format!("{BLAKE3_IDENTITY_PREFIX}{}", blake3::hash(bytes).to_hex())
}

#[must_use]
fn is_b3_identity(value: &str) -> bool {
    let Some(hex) = value.strip_prefix(BLAKE3_IDENTITY_PREFIX) else {
        return false;
    };
    hex.len() == BLAKE3_HEX_LENGTH && hex.chars().all(|character| character.is_ascii_hexdigit())
}

#[must_use]
fn sorted_dedup_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUIREMENT_ID: &str = "r[valence.evidence_ir.hegel_property_receipts]";
    const PROPERTY_ID: &str = "valence.evidence_ir.hegel_property_receipts.valid";
    const CHECKED_SCOPE: &str = "crates/valence-core/src/evidence_ir.rs";
    const RUNNER_VERSION: &str = "0.14.12";
    const NATIVE_BACKEND: &str = "native";
    const GENERATOR_FAMILY: &str = "valence.hegel.fixture";
    const SOURCE_BYTES: &[u8] = b"source fixture";
    const GENERATOR_BYTES: &[u8] = b"generator fixture";
    const COUNTEREXAMPLE_BYTES: &[u8] = b"counterexample fixture";

    fn passing_receipt() -> HegelPropertyReceipt {
        build_hegel_property_receipt(HegelPropertyReceiptInput {
            property_id: PROPERTY_ID.to_owned(),
            requirement_ids: vec![REQUIREMENT_ID.to_owned()],
            checked_scope: CHECKED_SCOPE.to_owned(),
            runner: HegelRunnerIdentity::new("hegeltest", RUNNER_VERSION, NATIVE_BACKEND),
            settings: HegelRunnerSettings::fast(),
            generator: HegelGeneratorIdentity::new(GENERATOR_FAMILY, render_b3_hash(GENERATOR_BYTES)),
            status: HegelPropertyStatus::Passed,
            issue_codes: Vec::new(),
            counterexample: None,
            source_hashes: vec![render_b3_hash(SOURCE_BYTES)],
            parent_artifacts: Vec::new(),
            non_claims: vec![HEGEL_GENERATED_TESTING_NON_CLAIM.to_owned()],
        })
        .expect("receipt builds")
    }

    #[test]
    fn complete_hegel_receipt_validates() {
        let receipt = passing_receipt();
        let report = validate_hegel_property_receipt(&receipt);
        assert!(report.valid);
        assert_eq!(report.evidence_tier, HEGEL_GENERATED_TESTING_TIER);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn failed_receipt_preserves_counterexample() {
        let receipt = build_hegel_property_receipt(HegelPropertyReceiptInput {
            status: HegelPropertyStatus::Failed,
            issue_codes: vec!["assertion_failed".to_owned()],
            counterexample: Some(HegelCounterexample::new(render_b3_hash(COUNTEREXAMPLE_BYTES), vec![
                "assertion_failed".to_owned(),
            ])),
            ..receipt_input_from(passing_receipt())
        })
        .expect("failed receipt builds");

        let report = validate_hegel_property_receipt(&receipt);
        assert!(report.valid);
    }

    #[test]
    fn receipt_missing_identity_hash_or_non_claim_fails_closed() {
        let mut receipt = passing_receipt();
        receipt.property_id.clear();
        receipt.generator.content_hash = "not-a-b3".to_owned();
        receipt.non_claims.clear();
        receipt.receipt_hash = hegel_property_receipt_hash(&receipt).expect("hash malformed receipt");

        let report = validate_hegel_property_receipt(&receipt);
        assert!(!report.valid);
        assert_eq!(report.issue_codes, vec![
            ISSUE_GENERATOR_HASH_MALFORMED.to_owned(),
            ISSUE_MISSING_IDENTITY.to_owned(),
            ISSUE_NON_CLAIMS_MISSING.to_owned(),
        ]);
    }

    #[test]
    fn stale_generator_identity_and_receipt_hash_fail_closed() {
        let mut receipt = passing_receipt();
        receipt.generator.family = "valence.hegel.changed".to_owned();

        let report = validate_hegel_property_receipt(&receipt);
        assert!(!report.valid);
        assert_eq!(report.issue_codes, vec![ISSUE_RECEIPT_HASH_MISMATCH.to_owned()]);
    }

    #[test]
    fn overclaim_and_tier_promotion_fail_closed() {
        let mut receipt = passing_receipt();
        receipt.evidence_tier = "formal_proof".to_owned();
        receipt.non_claims.push("formal proof of whole-program correctness".to_owned());
        receipt.receipt_hash = hegel_property_receipt_hash(&receipt).expect("hash promoted receipt");

        let report = validate_hegel_property_receipt(&receipt);
        assert!(!report.valid);
        assert_eq!(report.issue_codes, vec![
            ISSUE_OVERCLAIM_DETECTED.to_owned(),
            ISSUE_EVIDENCE_TIER_UNSUPPORTED.to_owned()
        ]);
    }

    #[test]
    fn suite_lane_definitions_are_bounded_and_ordered() {
        let lanes = default_hegel_suite_lanes();
        assert_eq!(lanes.len(), EXPECTED_LANE_COUNT);
        assert_eq!(lanes[FAST_LANE_INDEX].case_budget, HEGEL_FAST_CASE_BUDGET);
        assert_eq!(lanes[FAST_LANE_INDEX].command, HEGEL_FAST_LANE_COMMAND);
        assert_eq!(lanes[DEEP_LANE_INDEX].command, HEGEL_DEEP_LANE_COMMAND);
        assert!(lanes[FAST_LANE_INDEX].command.contains("--test hegel_property_laws"));
        assert!(!lanes[FAST_LANE_INDEX].command.contains("valence-core hegel_property_laws"));
        assert!(!lanes[FAST_LANE_INDEX].opt_in);
        assert!(lanes[DEEP_LANE_INDEX].opt_in);
        assert!(lanes.iter().all(|lane| lane.non_claims == vec![HEGEL_GENERATED_TESTING_NON_CLAIM.to_owned()]));
    }

    const EXPECTED_LANE_COUNT: usize = 2;
    const FAST_LANE_INDEX: usize = 0;
    const DEEP_LANE_INDEX: usize = 1;

    fn receipt_input_from(receipt: HegelPropertyReceipt) -> HegelPropertyReceiptInput {
        HegelPropertyReceiptInput {
            property_id: receipt.property_id,
            requirement_ids: receipt.requirement_ids,
            checked_scope: receipt.checked_scope,
            runner: receipt.runner,
            settings: receipt.settings,
            generator: receipt.generator,
            status: receipt.status,
            issue_codes: receipt.issue_codes,
            counterexample: receipt.counterexample,
            source_hashes: receipt.source_hashes,
            parent_artifacts: receipt.parent_artifacts,
            non_claims: receipt.non_claims,
        }
    }
}
