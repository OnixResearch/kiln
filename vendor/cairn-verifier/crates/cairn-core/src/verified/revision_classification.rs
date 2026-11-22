//! Advisory semantic revision classification.
//!
//! This module compares two exact native requirement revisions, or two declared
//! acceptance-policy fragments, under an explicit finite correspondence. It
//! reports whether the new obligation is editorial, clarifying, stronger,
//! weaker, incompatible, or undetermined. It never transfers evidence, never
//! marks an old receipt current, and never mutates lifecycle state.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use super::scoped_claim_derivations::{claim_identity, ScopedClaim};
use super::typed_requirement_contracts::{
    admit_correspondence_review, normalize_contract, CorrespondenceReview, FiniteExpr, FiniteTerm,
    FiniteValue, NormalizedContract, SemanticContract, SemanticError, SemanticProfile,
};

pub const REVISION_CLASSIFICATION_POLICY_SCHEMA_V1: &str =
    "cairn.revision-classification-policy.v1";
pub const REQUIREMENT_REVISION_REQUEST_SCHEMA_V1: &str = "cairn.requirement-revision-comparison.v1";
pub const POLICY_REVISION_REQUEST_SCHEMA_V1: &str = "cairn.policy-revision-comparison.v1";
pub const REQUIREMENT_REVISION_REPORT_SCHEMA_V1: &str = "cairn.requirement-revision-report.v1";
pub const POLICY_REVISION_REPORT_SCHEMA_V1: &str = "cairn.policy-revision-report.v1";
pub const ACCEPTANCE_FRAGMENT_SCHEMA_V1: &str = "cairn.acceptance-fragment.v1";
pub const EVIDENCE_TRANSFER_SCHEMA_V1: &str = "cairn.evidence-transfer-request.v1";
pub const SUPPORTED_POLICY_FRAGMENT_V1: &str =
    "named-required-checks.evidence-roles.assumption-sets.v1";
pub const REVISION_CLASSIFIER_PRODUCER: &str = "cairn.revision-classifier.v1";
pub const TRANSFER_DISABLED_REASON: &str =
    "automated evidence transfer is disabled until its admission rules and consumers are separately qualified";
// r[impl cairn.revision_classification.inspection]
pub const REVISION_CLASSIFICATION_BOUNDARY: &str = "Advisory semantic comparison only: classification does not discharge any obligation, omit required checks, authorize release, transfer or refresh evidence, or change any native completion, readiness, verification-role or gate decision. A finite implication result covers only the declared common domains, observable correspondence, assumptions and bounds. Original evidence applicability validators remain authoritative, and an old receipt stays stale after every requirement revision, including a semantically equivalent edit.";

const MAX_DOMAINS_CEILING: usize = 64;
const MAX_OBSERVABLES_CEILING: usize = 64;
const MAX_DOMAIN_VALUES_CEILING: usize = 256;
const MAX_WORLDS_CEILING: u64 = 1_048_576;
const MAX_STEPS_CEILING: u64 = 10_000_000;
const MAX_EVIDENCE_CEILING: usize = 1_024;
const MAX_POLICY_CHECKS_CEILING: usize = 128;
const MAX_POLICY_ROLES_CEILING: usize = 64;
const MAX_POLICY_ASSUMPTIONS_CEILING: usize = 64;
const MAX_POLICY_ASSUMPTION_SETS_CEILING: usize = 4_096;
const MAX_POLICY_ACCEPTED_CEILING: u64 = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum RevisionClassificationError {
    Unsupported(String),
    InvalidInput(String),
    RevisionMismatch(String),
    InvalidCorrespondence(String),
    InvalidPolicy(String),
    BoundExhaustion(String),
    Serialization(String),
}

impl RevisionClassificationError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::InvalidInput(_) => "invalid_input",
            Self::RevisionMismatch(_) => "revision_mismatch",
            Self::InvalidCorrespondence(_) => "invalid_correspondence",
            Self::InvalidPolicy(_) => "invalid_policy",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::Serialization(_) => "serialization",
        }
    }
}

impl std::fmt::Display for RevisionClassificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for RevisionClassificationError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionClassificationPolicy {
    pub schema: String,
    pub comparison: ComparisonBounds,
    pub policy_fragment: PolicyFragmentBounds,
    pub transfer: TransferPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonBounds {
    pub max_domains: usize,
    pub max_observables: usize,
    pub max_domain_values: usize,
    pub max_worlds: u64,
    pub max_steps: u64,
    pub max_evidence: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFragmentBounds {
    pub max_checks: usize,
    pub max_roles: usize,
    pub max_assumptions: usize,
    pub max_assumption_sets: usize,
    pub max_accepted: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferPolicy {
    pub enabled: bool,
    pub admission_rules: Vec<String>,
}

impl RevisionClassificationPolicy {
    // r[impl cairn.revision_classification.bounds]
    pub fn admit(&self) -> Result<(), RevisionClassificationError> {
        if self.schema != REVISION_CLASSIFICATION_POLICY_SCHEMA_V1 {
            return Err(RevisionClassificationError::Unsupported(format!(
                "unsupported revision classification policy schema {}",
                self.schema
            )));
        }
        let comparison = &self.comparison;
        if comparison.max_domains == 0
            || comparison.max_domains > MAX_DOMAINS_CEILING
            || comparison.max_observables == 0
            || comparison.max_observables > MAX_OBSERVABLES_CEILING
            || comparison.max_domain_values == 0
            || comparison.max_domain_values > MAX_DOMAIN_VALUES_CEILING
            || comparison.max_worlds == 0
            || comparison.max_worlds > MAX_WORLDS_CEILING
            || comparison.max_steps == 0
            || comparison.max_steps > MAX_STEPS_CEILING
            || comparison.max_evidence == 0
            || comparison.max_evidence > MAX_EVIDENCE_CEILING
        {
            return Err(RevisionClassificationError::BoundExhaustion(
                "comparison bounds are outside supported implementation ceilings".into(),
            ));
        }
        let fragment = &self.policy_fragment;
        if fragment.max_checks == 0
            || fragment.max_checks > MAX_POLICY_CHECKS_CEILING
            || fragment.max_roles == 0
            || fragment.max_roles > MAX_POLICY_ROLES_CEILING
            || fragment.max_assumptions == 0
            || fragment.max_assumptions > MAX_POLICY_ASSUMPTIONS_CEILING
            || fragment.max_assumption_sets == 0
            || fragment.max_assumption_sets > MAX_POLICY_ASSUMPTION_SETS_CEILING
            || fragment.max_accepted == 0
            || fragment.max_accepted > MAX_POLICY_ACCEPTED_CEILING
        {
            return Err(RevisionClassificationError::BoundExhaustion(
                "policy fragment bounds are outside supported implementation ceilings".into(),
            ));
        }
        // r[impl cairn.revision_classification.transfer_disabled]
        if self.transfer.enabled {
            return Err(RevisionClassificationError::Unsupported(
                TRANSFER_DISABLED_REASON.into(),
            ));
        }
        let mut rules = BTreeSet::new();
        for rule in &self.transfer.admission_rules {
            if rule.trim().is_empty() {
                return Err(RevisionClassificationError::InvalidPolicy(
                    "transfer admission rule identifiers must be non-empty".into(),
                ));
            }
            if !rules.insert(rule.as_str()) {
                return Err(RevisionClassificationError::InvalidPolicy(format!(
                    "duplicate transfer admission rule {rule}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionBinding {
    pub old_requirement_row_blake3: String,
    pub new_requirement_row_blake3: String,
    pub old_interpretation_blake3: String,
    pub new_interpretation_blake3: String,
    pub old_mapping_blake3: String,
    pub new_mapping_blake3: String,
    pub old_contract_blake3: String,
    pub new_contract_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionContext {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueCorrespondence {
    pub common: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainCorrespondence {
    pub old_domain: Option<String>,
    pub new_domain: Option<String>,
    pub values: Vec<ValueCorrespondence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservableCorrespondence {
    pub old: Option<String>,
    pub new: Option<String>,
    pub arguments: Vec<String>,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionMapping {
    pub domains: BTreeMap<String, DomainCorrespondence>,
    pub observables: BTreeMap<String, ObservableCorrespondence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrespondenceInputs {
    pub old: Option<CorrespondenceReview>,
    pub new: Option<CorrespondenceReview>,
    pub authorized_reviewers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedEvidence {
    pub evidence_blake3: String,
    pub receipt_blake3: String,
    pub claim: ScopedClaim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRevisionRequest {
    pub schema: String,
    pub profile: SemanticProfile,
    pub old_native_requirement: ContentBoundRequirementRow,
    pub new_native_requirement: ContentBoundRequirementRow,
    pub old_contract: SemanticContract,
    pub new_contract: SemanticContract,
    pub binding: RevisionBinding,
    pub context: RevisionContext,
    pub mapping: RevisionMapping,
    pub correspondence: CorrespondenceInputs,
    pub retained_evidence: Vec<RetainedEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionClass {
    Editorial,
    Clarifying,
    Strengthening,
    Weakening,
    Incompatible,
    Undetermined,
}

impl RevisionClass {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Editorial => "editorial",
            Self::Clarifying => "clarifying",
            Self::Strengthening => "strengthening",
            Self::Weakening => "weakening",
            Self::Incompatible => "incompatible",
            Self::Undetermined => "undetermined",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionRelation {
    Equivalent,
    Stronger,
    Weaker,
    Incomparable,
    Unknown,
}

impl RevisionRelation {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Equivalent => "equivalent",
            Self::Stronger => "stronger",
            Self::Weaker => "weaker",
            Self::Incomparable => "incomparable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormulaRelation {
    Equivalent,
    Strengthened,
    Weakened,
    Incomparable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetRelation {
    Equivalent,
    Tightened,
    Widened,
    Incomparable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseDecision {
    Reusable,
    NotReusable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsupportedDifference {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InconclusiveEvidence {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundContract {
    pub normalized: NormalizedContract,
    pub profile: SemanticProfile,
    pub correspondence_blake3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralDelta {
    pub determined: bool,
    pub statement_changed: bool,
    pub assumptions_changed: bool,
    pub activation_changed: bool,
    pub guarantee_changed: bool,
    pub domains_changed: bool,
    pub observables_changed: bool,
    pub profile_bounds_changed: bool,
    pub editorial_eligible: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessValue {
    pub domain: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessRow {
    pub arguments: Vec<WitnessValue>,
    pub value: WitnessValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessWorld {
    pub observables: BTreeMap<String, Vec<WitnessRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideTruths {
    pub assumption_values: Vec<bool>,
    pub assumptions_hold: bool,
    pub activated: bool,
    pub guarantee_holds: bool,
    pub obligation_holds: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplicationWitness {
    pub premise_side: String,
    pub conclusion_side: String,
    pub world: WitnessWorld,
    pub premise: SideTruths,
    pub conclusion: SideTruths,
    pub world_blake3: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplicationResult {
    pub direction: String,
    pub premise: String,
    pub conclusion: String,
    pub holds: Option<bool>,
    pub complete: bool,
    pub witness: Option<ImplicationWitness>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonObservableReport {
    pub arguments: Vec<String>,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionSearchBasis {
    pub common_domains: BTreeMap<String, Vec<String>>,
    pub common_observables: BTreeMap<String, CommonObservableReport>,
    pub world_count: u64,
    pub worlds_checked: u64,
    pub steps: u64,
    pub max_worlds: u64,
    pub max_steps: u64,
    pub complete: bool,
    pub conclusive: bool,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssumptionGuaranteeDelta {
    pub assumptions: FormulaRelation,
    pub activation: FormulaRelation,
    pub guarantee: FormulaRelation,
    pub obligation: RevisionRelation,
    pub guarantee_structurally_equal: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReuseDecision {
    pub evidence_blake3: String,
    pub receipt_blake3: String,
    pub claim_id: String,
    pub claim_kind: String,
    pub proposition: String,
    pub new_obligation: String,
    pub decision: ReuseDecision,
    pub old_receipt_current: bool,
    pub applicability_current: bool,
    pub requires_new_derivation: bool,
    pub transfer_allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrespondenceStatus {
    pub old: String,
    pub new: String,
    pub old_review_blake3: Option<String>,
    pub new_review_blake3: Option<String>,
    pub authorized_reviewers: Vec<String>,
    pub authenticity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferPolicyStatus {
    pub enabled: bool,
    pub admission_rules: Vec<String>,
    pub automated_transfer: String,
}

// r[impl cairn.revision_classification.conformance]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementComparisonReport {
    pub schema: String,
    pub advisory: bool,
    pub producer: String,
    pub classification_policy_blake3: String,
    pub classification: RevisionClass,
    pub relation: RevisionRelation,
    pub reason: String,
    pub old: BoundContract,
    pub new: BoundContract,
    pub binding: RevisionBinding,
    pub context: RevisionContext,
    pub mapping: RevisionMapping,
    pub structural: Option<StructuralDelta>,
    pub assumption_guarantee: Option<AssumptionGuaranteeDelta>,
    pub implications: Vec<ImplicationResult>,
    pub search: Option<RevisionSearchBasis>,
    pub evidence_reuse: Vec<EvidenceReuseDecision>,
    pub correspondence: CorrespondenceStatus,
    pub transfer: TransferPolicyStatus,
    pub unsupported: Option<UnsupportedDifference>,
    pub inconclusive: Option<InconclusiveEvidence>,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceUniverse {
    pub checks: BTreeSet<String>,
    pub evidence_roles: BTreeSet<String>,
    pub assumptions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceFragment {
    pub schema: String,
    pub fragment: String,
    pub required_checks: BTreeSet<String>,
    pub evidence_roles: BTreeSet<String>,
    pub assumption_sets: Vec<BTreeSet<String>>,
    pub universe: EvidenceUniverse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRevisionBinding {
    pub old_policy_blake3: String,
    pub new_policy_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRevisionRequest {
    pub schema: String,
    pub binding: PolicyRevisionBinding,
    pub old: AcceptanceFragment,
    pub new: AcceptanceFragment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCandidate {
    pub checks: Vec<String>,
    pub evidence_role: String,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetInclusion {
    pub direction: String,
    pub holds: bool,
    pub vacuous: bool,
    pub witness: Option<EvidenceCandidate>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFragmentReport {
    pub fragment_blake3: String,
    pub required_checks: Vec<String>,
    pub evidence_roles: Vec<String>,
    pub assumption_sets: Vec<Vec<String>>,
    pub accepted_count: Option<u64>,
    pub feasible: bool,
    pub accepted_empty: bool,
    pub count_within_bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceUniverseReport {
    pub checks: Vec<String>,
    pub evidence_roles: Vec<String>,
    pub assumptions: Vec<String>,
    pub candidate_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFeasibility {
    pub old_feasible: bool,
    pub new_feasible: bool,
    pub old_accepted_empty: bool,
    pub new_accepted_empty: bool,
    pub vacuous_inclusion: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyStructuralDelta {
    pub determined: bool,
    pub required_checks_changed: bool,
    pub evidence_roles_changed: bool,
    pub assumption_sets_changed: bool,
    pub universe_changed: bool,
    pub editorial_eligible: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyComparisonReport {
    pub schema: String,
    pub advisory: bool,
    pub producer: String,
    pub classification_policy_blake3: String,
    pub classification: RevisionClass,
    pub relation: SetRelation,
    pub reason: String,
    pub supported_fragment: String,
    pub binding: PolicyRevisionBinding,
    pub universe: EvidenceUniverseReport,
    pub old: PolicyFragmentReport,
    pub new: PolicyFragmentReport,
    pub inclusion: Vec<SetInclusion>,
    pub feasibility: PolicyFeasibility,
    pub structural: PolicyStructuralDelta,
    pub unsupported: Option<UnsupportedDifference>,
    pub inconclusive: Option<InconclusiveEvidence>,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceTransferRequest {
    pub schema: String,
    pub evidence_blake3: String,
    pub old_claim_blake3: String,
    pub new_claim_blake3: String,
    pub comparison_report_blake3: String,
    pub subject_blake3: String,
    pub environment: String,
    pub assumptions: BTreeSet<String>,
}

/// Future transfer record shape. It is intentionally not produced by this
/// implementation: transfer requires a new derivation and separate qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceTransferRecord {
    pub schema: String,
    pub evidence_blake3: String,
    pub old_claim_blake3: String,
    pub new_claim_blake3: String,
    pub comparison_report_blake3: String,
    pub subject_blake3: String,
    pub environment: String,
    pub assumptions: BTreeSet<String>,
    pub record_blake3: String,
}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, RevisionClassificationError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RevisionClassificationError::Serialization(error.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(super::evidence::canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_native_row(
    row: &ContentBoundRequirementRow,
) -> Result<(), RevisionClassificationError> {
    if row.requirement_id.trim().is_empty()
        || row.specification_path.trim().is_empty()
        || !digest(&row.specification_blake3)
        || !digest(&row.requirement_body_blake3)
        || !digest(&row.row_blake3)
        || row.source_span.start_byte > row.source_span.end_byte
        || row.source_span.start_line == 0
        || row.source_span.end_line < row.source_span.start_line
    {
        return Err(RevisionClassificationError::InvalidInput(
            "each side requires an exact native requirement row identity".into(),
        ));
    }
    Ok(())
}

fn validate_context(context: &RevisionContext) -> Result<(), RevisionClassificationError> {
    if !digest(&context.subject_blake3)
        || !digest(&context.policy_blake3)
        || context.target.trim().is_empty()
        || context.environment.trim().is_empty()
    {
        return Err(RevisionClassificationError::InvalidInput(
            "explicit subject, policy, target and environment identities are required".into(),
        ));
    }
    Ok(())
}

fn typed_error(error: SemanticError) -> RevisionClassificationError {
    match error {
        SemanticError::UnsupportedConstruct(construct) => RevisionClassificationError::Unsupported(
            format!("unsupported typed requirement construct {construct}"),
        ),
        SemanticError::UnknownProfile(profile) => RevisionClassificationError::Unsupported(
            format!("unknown typed requirement profile {profile}"),
        ),
        SemanticError::RequirementRevisionMismatch => {
            RevisionClassificationError::RevisionMismatch(
                "typed contract does not bind the supplied native requirement row".into(),
            )
        }
        SemanticError::BoundExhaustion(detail) => RevisionClassificationError::BoundExhaustion(
            format!("typed requirement contract: {detail}"),
        ),
        other => RevisionClassificationError::InvalidInput(format!(
            "typed requirement contract: {other:?}"
        )),
    }
}

fn validate_binding(
    binding: &RevisionBinding,
    old: &NormalizedContract,
    new: &NormalizedContract,
) -> Result<(), RevisionClassificationError> {
    if binding.old_requirement_row_blake3 != old.contract.requirement.row_blake3
        || binding.new_requirement_row_blake3 != new.contract.requirement.row_blake3
        || binding.old_interpretation_blake3 != old.interpretation_blake3
        || binding.new_interpretation_blake3 != new.interpretation_blake3
        || binding.old_mapping_blake3 != old.mapping_blake3
        || binding.new_mapping_blake3 != new.mapping_blake3
        || binding.old_contract_blake3 != old.contract_blake3
        || binding.new_contract_blake3 != new.contract_blake3
    {
        return Err(RevisionClassificationError::RevisionMismatch(
            "comparison binding does not match both exact normalized requirement revisions".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Old,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MappedObservableMeta {
    arguments: Vec<String>,
    result: String,
    phase: String,
}

#[derive(Debug, Clone, Default)]
struct SideModel {
    domain_to_common: BTreeMap<String, String>,
    value_to_common: BTreeMap<(String, String), String>,
    observable_to_common: BTreeMap<String, String>,
    mapped_domain_values: BTreeMap<String, Vec<String>>,
    observable_meta: BTreeMap<String, MappedObservableMeta>,
}

#[derive(Debug, Clone)]
struct CommonDomain {
    values: Vec<String>,
    index: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct CommonObservable {
    arguments: Vec<String>,
    argument_indices: Vec<BTreeMap<String, usize>>,
    result: String,
    result_values: Vec<String>,
    row_count: usize,
    row_strides: Vec<usize>,
    offset: usize,
}

struct CommonModel {
    domains: BTreeMap<String, CommonDomain>,
    observables: BTreeMap<String, CommonObservable>,
    slot_counts: Vec<usize>,
    world_count: u64,
    old: SideModel,
    new: SideModel,
}

#[allow(clippy::large_enum_variant)]
enum ModelBuild {
    Supported(CommonModel),
    Unsupported(UnsupportedDifference),
    Inconclusive(InconclusiveEvidence),
}

fn unsupported(code: &str, detail: impl Into<String>) -> ModelBuild {
    ModelBuild::Unsupported(UnsupportedDifference {
        code: code.into(),
        detail: detail.into(),
    })
}

fn build_models(
    mapping: &RevisionMapping,
    old_contract: &SemanticContract,
    new_contract: &SemanticContract,
    bounds: &ComparisonBounds,
) -> ModelBuild {
    if mapping.domains.len() > bounds.max_domains {
        return ModelBuild::Inconclusive(InconclusiveEvidence {
            code: "domain_bound".into(),
            detail: "declared common domains exceed the admitted bound".into(),
        });
    }
    if mapping.observables.len() > bounds.max_observables {
        return ModelBuild::Inconclusive(InconclusiveEvidence {
            code: "observable_bound".into(),
            detail: "declared common observables exceed the admitted bound".into(),
        });
    }

    let mut common_values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut old_domains: BTreeMap<String, String> = BTreeMap::new();
    let mut new_domains: BTreeMap<String, String> = BTreeMap::new();
    for (common, correspondence) in &mapping.domains {
        if common.trim().is_empty() {
            return unsupported("empty_domain", "common domain names must be non-empty");
        }
        if correspondence.values.is_empty() {
            return unsupported(
                "empty_domain",
                format!("common domain {common} declares no corresponding values"),
            );
        }
        if correspondence.values.len() > bounds.max_domain_values {
            return ModelBuild::Inconclusive(InconclusiveEvidence {
                code: "domain_value_bound".into(),
                detail: format!("common domain {common} exceeds the admitted value bound"),
            });
        }
        let mut values = Vec::with_capacity(correspondence.values.len());
        let mut seen = BTreeSet::new();
        for value in &correspondence.values {
            if value.common.trim().is_empty() {
                return unsupported(
                    "empty_value",
                    format!("common domain {common} declares an empty value"),
                );
            }
            if !seen.insert(value.common.clone()) {
                return unsupported(
                    "duplicate_common_value",
                    format!("common domain {common} repeats value {}", value.common),
                );
            }
            values.push(value.common.clone());
        }
        if correspondence.old_domain.is_none() && correspondence.new_domain.is_none() {
            return unsupported(
                "unused_domain",
                format!("common domain {common} names neither a prior nor a current domain"),
            );
        }
        if let Some(domain) = &correspondence.old_domain {
            let duplicate = old_domains.insert(domain.clone(), common.clone()).is_some();
            if duplicate {
                return unsupported(
                    "duplicate_old_domain",
                    format!("prior domain {domain} is mapped more than once"),
                );
            }
        }
        if let Some(domain) = &correspondence.new_domain {
            let duplicate = new_domains.insert(domain.clone(), common.clone()).is_some();
            if duplicate {
                return unsupported(
                    "duplicate_new_domain",
                    format!("current domain {domain} is mapped more than once"),
                );
            }
        }
        values.sort();
        common_values.insert(common.clone(), values);
    }

    for domain in old_contract.domains.keys() {
        if !old_domains.contains_key(domain) {
            return unsupported(
                "unmapped_domain",
                format!("prior domain {domain} has no explicit common correspondence"),
            );
        }
    }
    for domain in new_contract.domains.keys() {
        if !new_domains.contains_key(domain) {
            return unsupported(
                "unmapped_domain",
                format!("current domain {domain} has no explicit common correspondence"),
            );
        }
    }

    for domain in old_domains.keys() {
        if !old_contract.domains.contains_key(domain) {
            return unsupported(
                "unknown_domain",
                format!("prior correspondence names absent contract domain {domain}"),
            );
        }
    }
    for domain in new_domains.keys() {
        if !new_contract.domains.contains_key(domain) {
            return unsupported(
                "unknown_domain",
                format!("current correspondence names absent contract domain {domain}"),
            );
        }
    }

    let mut old_model = SideModel {
        domain_to_common: old_domains,
        ..SideModel::default()
    };
    let mut new_model = SideModel {
        domain_to_common: new_domains,
        ..SideModel::default()
    };

    for correspondence in mapping.domains.values() {
        for value in &correspondence.values {
            if value.old.is_none() && value.new.is_none() {
                return unsupported(
                    "unused_value",
                    format!(
                        "common value {} names neither a prior nor a current value",
                        value.common
                    ),
                );
            }
            for (side, named) in [
                (Side::Old, value.old.as_ref()),
                (Side::New, value.new.as_ref()),
            ] {
                let Some(local) = named else {
                    continue;
                };
                let (contract, model) = match side {
                    Side::Old => (old_contract, &mut old_model),
                    Side::New => (new_contract, &mut new_model),
                };
                let domain = match side {
                    Side::Old => correspondence.old_domain.as_ref(),
                    Side::New => correspondence.new_domain.as_ref(),
                };
                let Some(domain) = domain else {
                    return unsupported(
                        "unmapped_value",
                        format!("common value {} names an absent side domain", value.common),
                    );
                };
                let Some(local_domain) = contract.domains.get(domain) else {
                    return unsupported(
                        "unknown_domain",
                        format!("declared domain {domain} is absent from the typed contract"),
                    );
                };
                if !local_domain.values.contains(local) {
                    return unsupported(
                        "unknown_value",
                        format!("value {local} is absent from declared domain {domain}"),
                    );
                }
                if model
                    .value_to_common
                    .insert((domain.clone(), local.clone()), value.common.clone())
                    .is_some()
                {
                    return unsupported(
                        "duplicate_value_mapping",
                        format!("value {local} in domain {domain} is mapped more than once"),
                    );
                }
                if model
                    .value_to_common
                    .iter()
                    .any(|((mapped_domain, mapped_value), common)| {
                        mapped_domain == domain && mapped_value != local && common == &value.common
                    })
                {
                    return unsupported(
                        "non_injective_value_mapping",
                        format!(
                            "domain {domain} maps distinct values onto common value {}",
                            value.common
                        ),
                    );
                }
                let entry = model
                    .mapped_domain_values
                    .entry(value.common.clone())
                    .or_default();
                if !entry.iter().any(|existing| existing == &value.common) {
                    entry.push(value.common.clone());
                }
            }
        }
    }

    for (side, contract, model) in [
        (Side::Old, old_contract, &mut old_model),
        (Side::New, new_contract, &mut new_model),
    ] {
        for (domain, finite) in &contract.domains {
            let local_mapped: BTreeMap<&str, &str> = model
                .value_to_common
                .iter()
                .filter(|((mapped_domain, _), _)| mapped_domain == domain)
                .map(|((_, value), common)| (value.as_str(), common.as_str()))
                .collect();
            if local_mapped.len() != finite.values.len() {
                return unsupported(
                    "incomplete_value_mapping",
                    format!(
                        "{side:?} domain {domain} does not map every declared value exactly once"
                    ),
                );
            }
            for value in &finite.values {
                if !local_mapped.contains_key(value.as_str()) {
                    return unsupported(
                        "incomplete_value_mapping",
                        format!("{side:?} domain {domain} leaves value {value} unmapped"),
                    );
                }
            }
            let common = model.domain_to_common[domain].clone();
            model.mapped_domain_values.entry(common).or_default().sort();
        }
    }

    for (common, correspondence) in &mapping.observables {
        if common.trim().is_empty() {
            return unsupported(
                "empty_observable",
                "common observable names must be non-empty",
            );
        }
        if correspondence.arguments.len() > bounds.max_domains {
            return ModelBuild::Inconclusive(InconclusiveEvidence {
                code: "observable_arity_bound".into(),
                detail: format!("common observable {common} exceeds the admitted arity bound"),
            });
        }
        for argument in &correspondence.arguments {
            if !common_values.contains_key(argument) {
                return unsupported(
                    "unknown_common_domain",
                    format!(
                        "common observable {common} names undeclared argument domain {argument}"
                    ),
                );
            }
        }
        if !common_values.contains_key(&correspondence.result) {
            return unsupported(
                "unknown_common_domain",
                format!(
                    "common observable {common} names undeclared result domain {}",
                    correspondence.result
                ),
            );
        }
        if correspondence.old.is_none() && correspondence.new.is_none() {
            return unsupported(
                "unused_observable",
                format!("common observable {common} names neither a prior nor a current mapping"),
            );
        }
        for (side, contract, model) in [
            (Side::Old, old_contract, &mut old_model),
            (Side::New, new_contract, &mut new_model),
        ] {
            let named = match side {
                Side::Old => correspondence.old.as_ref(),
                Side::New => correspondence.new.as_ref(),
            };
            let Some(local) = named else {
                continue;
            };
            let Some(observable) = contract.observables.get(local) else {
                return unsupported(
                    "unknown_observable",
                    format!("{side:?} observable {local} is absent from the typed contract"),
                );
            };
            if observable.arguments.len() != correspondence.arguments.len() {
                return unsupported(
                    "observable_arity",
                    format!("common observable {common} changes arity for {side:?}"),
                );
            }
            for (index, argument) in observable.arguments.iter().enumerate() {
                if model.domain_to_common.get(argument) != Some(&correspondence.arguments[index]) {
                    return unsupported(
                        "observable_argument_mismatch",
                        format!(
                            "common observable {common} argument {index} has no matching domain"
                        ),
                    );
                }
            }
            if model.domain_to_common.get(&observable.result) != Some(&correspondence.result) {
                return unsupported(
                    "observable_result_mismatch",
                    format!("common observable {common} has a mismatched result domain"),
                );
            }
            if model
                .observable_to_common
                .insert(local.clone(), common.clone())
                .is_some()
            {
                return unsupported(
                    "duplicate_observable_mapping",
                    format!("{side:?} observable {local} is mapped more than once"),
                );
            }
            model.observable_meta.insert(
                common.clone(),
                MappedObservableMeta {
                    arguments: correspondence.arguments.clone(),
                    result: correspondence.result.clone(),
                    phase: observable.phase.clone(),
                },
            );
        }
    }

    for (side, contract, model) in [
        (Side::Old, old_contract, &mut old_model),
        (Side::New, new_contract, &mut new_model),
    ] {
        for observable in contract.observables.keys() {
            if !model.observable_to_common.contains_key(observable) {
                return unsupported(
                    "unmapped_observable",
                    format!("{side:?} observable {observable} has no common correspondence"),
                );
            }
        }
    }

    let mut domains = BTreeMap::new();
    for (name, values) in common_values {
        let index = values
            .iter()
            .enumerate()
            .map(|(position, value)| (value.clone(), position))
            .collect();
        domains.insert(name, CommonDomain { values, index });
    }

    let mut observables = BTreeMap::new();
    let mut slot_counts = Vec::new();
    let mut offset = 0usize;
    let mut total_worlds: u64 = 1;
    for (name, correspondence) in &mapping.observables {
        let result_domain = &domains[&correspondence.result];
        let mut argument_indices = Vec::with_capacity(correspondence.arguments.len());
        let mut row_count = 1usize;
        let mut argument_lengths = Vec::with_capacity(correspondence.arguments.len());
        for argument in &correspondence.arguments {
            let domain = &domains[argument];
            row_count = match row_count.checked_mul(domain.values.len()) {
                Some(value) => value,
                None => {
                    return ModelBuild::Inconclusive(InconclusiveEvidence {
                        code: "observable_cardinality".into(),
                        detail: format!("common observable {name} overflows the row count"),
                    })
                }
            };
            argument_lengths.push(domain.values.len());
            argument_indices.push(domain.index.clone());
        }
        let mut strides = vec![1usize; argument_lengths.len()];
        let mut stride = 1usize;
        for (position, length) in argument_lengths.iter().enumerate().rev() {
            strides[position] = stride;
            stride = match stride.checked_mul(*length) {
                Some(value) => value,
                None => {
                    return ModelBuild::Inconclusive(InconclusiveEvidence {
                        code: "observable_cardinality".into(),
                        detail: format!("common observable {name} overflows row strides"),
                    })
                }
            };
        }
        let exponent = match u32::try_from(row_count) {
            Ok(exponent) => exponent,
            Err(_) => {
                return ModelBuild::Inconclusive(InconclusiveEvidence {
                    code: "observable_cardinality".into(),
                    detail: format!("common observable {name} row count is unsupported"),
                })
            }
        };
        let choices = match (result_domain.values.len() as u64).checked_pow(exponent) {
            Some(choices) => choices,
            None => {
                return ModelBuild::Inconclusive(InconclusiveEvidence {
                    code: "world_bound".into(),
                    detail: format!("common observable {name} overflows the world count"),
                })
            }
        };
        total_worlds = match total_worlds.checked_mul(choices) {
            Some(value) => value,
            None => {
                return ModelBuild::Inconclusive(InconclusiveEvidence {
                    code: "world_bound".into(),
                    detail: "world enumeration overflows the supported count".into(),
                })
            }
        };
        if total_worlds > bounds.max_worlds {
            return ModelBuild::Inconclusive(InconclusiveEvidence {
                code: "world_bound".into(),
                detail: format!(
                    "finite world count {total_worlds} exceeds the admitted bound {}",
                    bounds.max_worlds
                ),
            });
        }
        let slots = row_count;
        let next_offset = match offset.checked_add(slots) {
            Some(next) => next,
            None => {
                return ModelBuild::Inconclusive(InconclusiveEvidence {
                    code: "world_bound".into(),
                    detail: "world slots overflow".into(),
                })
            }
        };
        if next_offset as u64 > bounds.max_steps {
            return ModelBuild::Inconclusive(InconclusiveEvidence {
                code: "world_bound".into(),
                detail: "world slot count exceeds the admitted step bound".into(),
            });
        }
        slot_counts.resize(next_offset, result_domain.values.len());
        observables.insert(
            name.clone(),
            CommonObservable {
                arguments: correspondence.arguments.clone(),
                argument_indices,
                result: correspondence.result.clone(),
                result_values: result_domain.values.clone(),
                row_count,
                row_strides: strides,
                offset,
            },
        );
        offset = next_offset;
    }

    if total_worlds == 0 {
        return ModelBuild::Inconclusive(InconclusiveEvidence {
            code: "world_bound".into(),
            detail: "world enumeration produced no assignments".into(),
        });
    }

    ModelBuild::Supported(CommonModel {
        domains,
        observables,
        slot_counts,
        world_count: total_worlds,
        old: old_model,
        new: new_model,
    })
}

fn translate_term(
    term: &FiniteTerm,
    side: &SideModel,
) -> Result<FiniteTerm, UnsupportedDifference> {
    match term {
        FiniteTerm::Literal { value } => {
            let common_domain =
                side.domain_to_common
                    .get(&value.domain)
                    .ok_or_else(|| UnsupportedDifference {
                        code: "unmapped_domain".into(),
                        detail: format!("domain {} has no common correspondence", value.domain),
                    })?;
            let common_value = side
                .value_to_common
                .get(&(value.domain.clone(), value.value.clone()))
                .ok_or_else(|| UnsupportedDifference {
                    code: "unmapped_value".into(),
                    detail: format!(
                        "value {} in domain {} is unmapped",
                        value.value, value.domain
                    ),
                })?;
            Ok(FiniteTerm::Literal {
                value: FiniteValue {
                    domain: common_domain.clone(),
                    value: common_value.clone(),
                },
            })
        }
        FiniteTerm::Variable { name } => Ok(FiniteTerm::Variable { name: name.clone() }),
        FiniteTerm::Observable { mapping, arguments } => {
            let common =
                side.observable_to_common
                    .get(mapping)
                    .ok_or_else(|| UnsupportedDifference {
                        code: "unmapped_observable".into(),
                        detail: format!("observable {mapping} has no common correspondence"),
                    })?;
            let arguments = arguments
                .iter()
                .map(|argument| translate_term(argument, side))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(FiniteTerm::Observable {
                mapping: common.clone(),
                arguments,
            })
        }
        FiniteTerm::Unsupported { construct } => Err(UnsupportedDifference {
            code: "unsupported_construct".into(),
            detail: format!("unsupported typed construct {construct}"),
        }),
    }
}

fn translate_expr(
    expr: &FiniteExpr,
    side: &SideModel,
) -> Result<FiniteExpr, UnsupportedDifference> {
    match expr {
        FiniteExpr::Bool { value } => Ok(FiniteExpr::Bool { value: *value }),
        FiniteExpr::Equal { left, right } => Ok(FiniteExpr::Equal {
            left: translate_term(left, side)?,
            right: translate_term(right, side)?,
        }),
        FiniteExpr::Member { term, values } => {
            let mut translated = Vec::with_capacity(values.len());
            for value in values {
                let common_domain = side.domain_to_common.get(&value.domain).ok_or_else(|| {
                    UnsupportedDifference {
                        code: "unmapped_domain".into(),
                        detail: format!("domain {} has no common correspondence", value.domain),
                    }
                })?;
                let common_value = side
                    .value_to_common
                    .get(&(value.domain.clone(), value.value.clone()))
                    .ok_or_else(|| UnsupportedDifference {
                        code: "unmapped_value".into(),
                        detail: format!(
                            "value {} in domain {} is unmapped",
                            value.value, value.domain
                        ),
                    })?;
                translated.push(FiniteValue {
                    domain: common_domain.clone(),
                    value: common_value.clone(),
                });
            }
            Ok(FiniteExpr::Member {
                term: translate_term(term, side)?,
                values: translated,
            })
        }
        FiniteExpr::Not { expression } => Ok(FiniteExpr::Not {
            expression: Box::new(translate_expr(expression, side)?),
        }),
        FiniteExpr::And { expressions } => Ok(FiniteExpr::And {
            expressions: expressions
                .iter()
                .map(|expression| translate_expr(expression, side))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        FiniteExpr::Or { expressions } => Ok(FiniteExpr::Or {
            expressions: expressions
                .iter()
                .map(|expression| translate_expr(expression, side))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => Ok(FiniteExpr::Implies {
            premise: Box::new(translate_expr(premise, side)?),
            conclusion: Box::new(translate_expr(conclusion, side)?),
        }),
        FiniteExpr::Forall {
            variable,
            domain,
            expression,
        } => {
            let common =
                side.domain_to_common
                    .get(domain)
                    .ok_or_else(|| UnsupportedDifference {
                        code: "unmapped_domain".into(),
                        detail: format!("domain {domain} has no common correspondence"),
                    })?;
            Ok(FiniteExpr::Forall {
                variable: variable.clone(),
                domain: common.clone(),
                expression: Box::new(translate_expr(expression, side)?),
            })
        }
        FiniteExpr::Exists {
            variable,
            domain,
            expression,
        } => {
            let common =
                side.domain_to_common
                    .get(domain)
                    .ok_or_else(|| UnsupportedDifference {
                        code: "unmapped_domain".into(),
                        detail: format!("domain {domain} has no common correspondence"),
                    })?;
            Ok(FiniteExpr::Exists {
                variable: variable.clone(),
                domain: common.clone(),
                expression: Box::new(translate_expr(expression, side)?),
            })
        }
        FiniteExpr::Unsupported { construct } => Err(UnsupportedDifference {
            code: "unsupported_construct".into(),
            detail: format!("unsupported typed construct {construct}"),
        }),
    }
}

#[derive(Debug)]
enum EvalFailure {
    Budget,
    Internal(String),
}

#[derive(Debug, Clone, Copy)]
struct EvaluatedSide {
    assumptions_hold: bool,
    activated: bool,
    guarantee_holds: bool,
    obligation_holds: bool,
}

struct EvalBudget {
    used: u64,
    maximum: u64,
}

impl EvalBudget {
    fn tick(&mut self) -> Result<(), EvalFailure> {
        if self.used >= self.maximum {
            return Err(EvalFailure::Budget);
        }
        self.used += 1;
        Ok(())
    }
}

fn eval_term(
    model: &CommonModel,
    side: &SideModel,
    term: &FiniteTerm,
    variables: &BTreeMap<String, (String, String)>,
    world: &[usize],
    budget: &mut EvalBudget,
) -> Result<(String, String), EvalFailure> {
    budget.tick()?;
    match term {
        FiniteTerm::Literal { value } => {
            let domain = side
                .domain_to_common
                .get(&value.domain)
                .ok_or_else(|| EvalFailure::Internal("literal domain is unmapped".into()))?;
            let common = side
                .value_to_common
                .get(&(value.domain.clone(), value.value.clone()))
                .ok_or_else(|| EvalFailure::Internal("literal value is unmapped".into()))?;
            Ok((domain.clone(), common.clone()))
        }
        FiniteTerm::Variable { name } => variables
            .get(name)
            .cloned()
            .ok_or_else(|| EvalFailure::Internal(format!("unbound variable {name}"))),
        FiniteTerm::Observable { mapping, arguments } => {
            let common = side
                .observable_to_common
                .get(mapping)
                .ok_or_else(|| EvalFailure::Internal("observable is unmapped".into()))?;
            let observable = model
                .observables
                .get(common)
                .ok_or_else(|| EvalFailure::Internal("common observable is absent".into()))?;
            let mut row = 0usize;
            for (index, argument) in arguments.iter().enumerate() {
                let value = eval_term(model, side, argument, variables, world, budget)?;
                if observable.arguments.get(index) != Some(&value.0) {
                    return Err(EvalFailure::Internal(
                        "observable argument domain mismatch".into(),
                    ));
                }
                let position = observable
                    .argument_indices
                    .get(index)
                    .and_then(|index| index.get(&value.1))
                    .ok_or_else(|| {
                        EvalFailure::Internal("observable argument value is out of domain".into())
                    })?;
                row = row
                    .checked_add(*position * observable.row_strides[index])
                    .ok_or_else(|| EvalFailure::Internal("observable row overflows".into()))?;
            }
            let slot = observable
                .offset
                .checked_add(row)
                .ok_or_else(|| EvalFailure::Internal("observable slot overflows".into()))?;
            let result_index = *world
                .get(slot)
                .ok_or_else(|| EvalFailure::Internal("world slot is absent".into()))?;
            let value = observable
                .result_values
                .get(result_index)
                .ok_or_else(|| EvalFailure::Internal("world result is out of domain".into()))?;
            Ok((observable.result.clone(), value.clone()))
        }
        FiniteTerm::Unsupported { construct } => Err(EvalFailure::Internal(format!(
            "unsupported typed construct {construct}"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_quantifier(
    model: &CommonModel,
    side: &SideModel,
    variable: &str,
    domain: &str,
    expression: &FiniteExpr,
    universal: bool,
    variables: &mut BTreeMap<String, (String, String)>,
    world: &[usize],
    budget: &mut EvalBudget,
) -> Result<bool, EvalFailure> {
    let common_domain = side
        .domain_to_common
        .get(domain)
        .ok_or_else(|| EvalFailure::Internal("quantifier domain is unmapped".into()))?;
    let values = model
        .domains
        .get(common_domain)
        .ok_or_else(|| EvalFailure::Internal("common quantifier domain is absent".into()))?;
    let previous = variables.insert(variable.to_string(), (common_domain.clone(), String::new()));
    let mut outcome = universal;
    for value in &values.values {
        variables.insert(variable.to_string(), (common_domain.clone(), value.clone()));
        let result = eval_expr(model, side, expression, variables, world, budget)?;
        if result != universal {
            outcome = result;
            break;
        }
    }
    match previous {
        Some(previous) => {
            variables.insert(variable.to_string(), previous);
        }
        None => {
            variables.remove(variable);
        }
    }
    Ok(outcome)
}

fn eval_expr(
    model: &CommonModel,
    side: &SideModel,
    expression: &FiniteExpr,
    variables: &mut BTreeMap<String, (String, String)>,
    world: &[usize],
    budget: &mut EvalBudget,
) -> Result<bool, EvalFailure> {
    budget.tick()?;
    match expression {
        FiniteExpr::Bool { value } => Ok(*value),
        FiniteExpr::Equal { left, right } => {
            let left = eval_term(model, side, left, variables, world, budget)?;
            let right = eval_term(model, side, right, variables, world, budget)?;
            if left.0 != right.0 {
                return Err(EvalFailure::Internal(
                    "equality crosses nominal domains".into(),
                ));
            }
            Ok(left.1 == right.1)
        }
        FiniteExpr::Member { term, values } => {
            let term = eval_term(model, side, term, variables, world, budget)?;
            for value in values {
                let domain = side
                    .domain_to_common
                    .get(&value.domain)
                    .ok_or_else(|| EvalFailure::Internal("member domain is unmapped".into()))?;
                let common = side
                    .value_to_common
                    .get(&(value.domain.clone(), value.value.clone()))
                    .ok_or_else(|| EvalFailure::Internal("member value is unmapped".into()))?;
                if domain == &term.0 && common == &term.1 {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FiniteExpr::Not { expression } => Ok(!eval_expr(
            model, side, expression, variables, world, budget,
        )?),
        FiniteExpr::And { expressions } => {
            for expression in expressions {
                if !eval_expr(model, side, expression, variables, world, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FiniteExpr::Or { expressions } => {
            for expression in expressions {
                if eval_expr(model, side, expression, variables, world, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => Ok(!eval_expr(model, side, premise, variables, world, budget)?
            || eval_expr(model, side, conclusion, variables, world, budget)?),
        FiniteExpr::Forall {
            variable,
            domain,
            expression,
        } => eval_quantifier(
            model, side, variable, domain, expression, true, variables, world, budget,
        ),
        FiniteExpr::Exists {
            variable,
            domain,
            expression,
        } => eval_quantifier(
            model, side, variable, domain, expression, false, variables, world, budget,
        ),
        FiniteExpr::Unsupported { construct } => Err(EvalFailure::Internal(format!(
            "unsupported typed construct {construct}"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_side(
    model: &CommonModel,
    side: &SideModel,
    contract: &SemanticContract,
    world: &[usize],
    variables: &mut BTreeMap<String, (String, String)>,
    budget: &mut EvalBudget,
    assumption_values: &mut Vec<bool>,
) -> Result<EvaluatedSide, EvalFailure> {
    variables.clear();
    assumption_values.clear();
    for assumption in &contract.assumptions {
        assumption_values.push(eval_expr(
            model, side, assumption, variables, world, budget,
        )?);
    }
    let assumptions_hold = assumption_values.iter().all(|value| *value);
    let activated = eval_expr(model, side, &contract.activation, variables, world, budget)?;
    let guarantee_holds = eval_expr(model, side, &contract.guarantee, variables, world, budget)?;
    Ok(EvaluatedSide {
        assumptions_hold,
        activated,
        guarantee_holds,
        obligation_holds: !assumptions_hold || !activated || guarantee_holds,
    })
}

fn materialize_world(model: &CommonModel, world: &[usize]) -> WitnessWorld {
    let mut observables = BTreeMap::new();
    for (name, observable) in &model.observables {
        let mut rows = Vec::with_capacity(observable.row_count);
        for row_index in 0..observable.row_count {
            let mut remaining = row_index;
            let mut arguments = Vec::with_capacity(observable.arguments.len());
            for (position, domain) in observable.arguments.iter().enumerate() {
                let stride = observable.row_strides[position];
                let value_index = if stride == 0 { 0 } else { remaining / stride };
                remaining %= stride.max(1);
                let values = &model.domains[domain].values;
                arguments.push(WitnessValue {
                    domain: domain.clone(),
                    value: values[value_index].clone(),
                });
            }
            let result_index = world[observable.offset + row_index];
            rows.push(WitnessRow {
                arguments,
                value: WitnessValue {
                    domain: observable.result.clone(),
                    value: observable.result_values[result_index].clone(),
                },
            });
        }
        observables.insert(name.clone(), rows);
    }
    WitnessWorld { observables }
}

fn side_truths(side: EvaluatedSide, assumption_values: &[bool]) -> SideTruths {
    SideTruths {
        assumption_values: assumption_values.to_vec(),
        assumptions_hold: side.assumptions_hold,
        activated: side.activated,
        guarantee_holds: side.guarantee_holds,
        obligation_holds: side.obligation_holds,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_witness(
    model: &CommonModel,
    world: &[usize],
    premise_side: &str,
    premise: EvaluatedSide,
    premise_assumptions: &[bool],
    conclusion_side: &str,
    conclusion: EvaluatedSide,
    conclusion_assumptions: &[bool],
) -> Result<ImplicationWitness, RevisionClassificationError> {
    let world = materialize_world(model, world);
    let world_blake3 = identity("cairn.revision-witness-world.v1", &world)?;
    let explanation = format!(
        "{premise_side} obligation holds while {conclusion_side} obligation is false in this complete finite observation world; assumptions, activation and guarantee values are reported separately"
    );
    Ok(ImplicationWitness {
        premise_side: premise_side.into(),
        conclusion_side: conclusion_side.into(),
        world,
        premise: side_truths(premise, premise_assumptions),
        conclusion: side_truths(conclusion, conclusion_assumptions),
        world_blake3,
        explanation,
    })
}

struct SearchOutcome {
    complete: bool,
    worlds_checked: u64,
    steps: u64,
    obligation_new_to_old_violated: bool,
    obligation_old_to_new_violated: bool,
    assumptions_new_to_old_violated: bool,
    assumptions_old_to_new_violated: bool,
    activation_new_to_old_violated: bool,
    activation_old_to_new_violated: bool,
    guarantee_new_to_old_violated: bool,
    guarantee_old_to_new_violated: bool,
    new_to_old_witness: Option<ImplicationWitness>,
    old_to_new_witness: Option<ImplicationWitness>,
}

fn search_worlds(
    model: &CommonModel,
    old_contract: &SemanticContract,
    new_contract: &SemanticContract,
    bounds: &ComparisonBounds,
) -> Result<SearchOutcome, RevisionClassificationError> {
    let mut outcome = SearchOutcome {
        complete: false,
        worlds_checked: 0,
        steps: 0,
        obligation_new_to_old_violated: false,
        obligation_old_to_new_violated: false,
        assumptions_new_to_old_violated: false,
        assumptions_old_to_new_violated: false,
        activation_new_to_old_violated: false,
        activation_old_to_new_violated: false,
        guarantee_new_to_old_violated: false,
        guarantee_old_to_new_violated: false,
        new_to_old_witness: None,
        old_to_new_witness: None,
    };
    let mut world = vec![0usize; model.slot_counts.len()];
    let mut budget = EvalBudget {
        used: 0,
        maximum: bounds.max_steps,
    };
    let mut variables = BTreeMap::new();
    let mut old_assumptions = Vec::new();
    let mut new_assumptions = Vec::new();
    let mut completed = false;
    'worlds: loop {
        if budget.tick().is_err() {
            break;
        }
        let old = match eval_side(
            model,
            &model.old,
            old_contract,
            &world,
            &mut variables,
            &mut budget,
            &mut old_assumptions,
        ) {
            Ok(value) => value,
            Err(EvalFailure::Budget) => break,
            Err(EvalFailure::Internal(detail)) => {
                return Err(RevisionClassificationError::InvalidInput(detail))
            }
        };
        let new = match eval_side(
            model,
            &model.new,
            new_contract,
            &world,
            &mut variables,
            &mut budget,
            &mut new_assumptions,
        ) {
            Ok(value) => value,
            Err(EvalFailure::Budget) => break,
            Err(EvalFailure::Internal(detail)) => {
                return Err(RevisionClassificationError::InvalidInput(detail))
            }
        };
        outcome.worlds_checked += 1;
        if new.obligation_holds && !old.obligation_holds {
            outcome.obligation_new_to_old_violated = true;
            if outcome.new_to_old_witness.is_none() {
                outcome.new_to_old_witness = Some(make_witness(
                    model,
                    &world,
                    "new",
                    new,
                    &new_assumptions,
                    "old",
                    old,
                    &old_assumptions,
                )?);
            }
        }
        if old.obligation_holds && !new.obligation_holds {
            outcome.obligation_old_to_new_violated = true;
            if outcome.old_to_new_witness.is_none() {
                outcome.old_to_new_witness = Some(make_witness(
                    model,
                    &world,
                    "old",
                    old,
                    &old_assumptions,
                    "new",
                    new,
                    &new_assumptions,
                )?);
            }
        }
        if new.assumptions_hold && !old.assumptions_hold {
            outcome.assumptions_new_to_old_violated = true;
        }
        if old.assumptions_hold && !new.assumptions_hold {
            outcome.assumptions_old_to_new_violated = true;
        }
        if new.activated && !old.activated {
            outcome.activation_new_to_old_violated = true;
        }
        if old.activated && !new.activated {
            outcome.activation_old_to_new_violated = true;
        }
        if new.guarantee_holds && !old.guarantee_holds {
            outcome.guarantee_new_to_old_violated = true;
        }
        if old.guarantee_holds && !new.guarantee_holds {
            outcome.guarantee_old_to_new_violated = true;
        }

        let mut index = world.len();
        loop {
            if index == 0 {
                completed = true;
                break 'worlds;
            }
            index -= 1;
            if budget.tick().is_err() {
                break 'worlds;
            }
            world[index] += 1;
            if world[index] < model.slot_counts[index] {
                break;
            }
            world[index] = 0;
        }
    }
    outcome.complete = completed;
    outcome.steps = budget.used;
    Ok(outcome)
}

fn revision_relation(
    complete: bool,
    new_to_old_violated: bool,
    old_to_new_violated: bool,
) -> RevisionRelation {
    if new_to_old_violated && old_to_new_violated {
        RevisionRelation::Incomparable
    } else if !complete {
        RevisionRelation::Unknown
    } else if !new_to_old_violated && !old_to_new_violated {
        RevisionRelation::Equivalent
    } else if !new_to_old_violated {
        RevisionRelation::Stronger
    } else {
        RevisionRelation::Weaker
    }
}

fn formula_relation(
    complete: bool,
    new_to_old_violated: bool,
    old_to_new_violated: bool,
) -> FormulaRelation {
    if new_to_old_violated && old_to_new_violated {
        FormulaRelation::Incomparable
    } else if !complete {
        FormulaRelation::Unknown
    } else if !new_to_old_violated && !old_to_new_violated {
        FormulaRelation::Equivalent
    } else if !new_to_old_violated {
        FormulaRelation::Strengthened
    } else {
        FormulaRelation::Weakened
    }
}

fn classify_requirement(relation: RevisionRelation, structural: &StructuralDelta) -> RevisionClass {
    match relation {
        RevisionRelation::Equivalent if structural.editorial_eligible => RevisionClass::Editorial,
        RevisionRelation::Equivalent => RevisionClass::Clarifying,
        RevisionRelation::Stronger => RevisionClass::Strengthening,
        RevisionRelation::Weaker => RevisionClass::Weakening,
        RevisionRelation::Incomparable => RevisionClass::Incompatible,
        RevisionRelation::Unknown => RevisionClass::Undetermined,
    }
}

fn structural_delta(
    old_contract: &SemanticContract,
    new_contract: &SemanticContract,
    model: &CommonModel,
) -> Result<StructuralDelta, UnsupportedDifference> {
    let old_assumptions = old_contract
        .assumptions
        .iter()
        .map(|expression| translate_expr(expression, &model.old))
        .collect::<Result<Vec<_>, _>>()?;
    let new_assumptions = new_contract
        .assumptions
        .iter()
        .map(|expression| translate_expr(expression, &model.new))
        .collect::<Result<Vec<_>, _>>()?;
    let old_activation = translate_expr(&old_contract.activation, &model.old)?;
    let new_activation = translate_expr(&new_contract.activation, &model.new)?;
    let old_guarantee = translate_expr(&old_contract.guarantee, &model.old)?;
    let new_guarantee = translate_expr(&new_contract.guarantee, &model.new)?;

    let assumptions_changed = old_assumptions != new_assumptions;
    let activation_changed = old_activation != new_activation;
    let guarantee_changed = old_guarantee != new_guarantee;
    let domains_changed = model.old.mapped_domain_values != model.new.mapped_domain_values;
    let observables_changed = model.old.observable_meta != model.new.observable_meta;
    let profile_bounds_changed = false;
    let statement_changed = activation_changed || guarantee_changed;
    let mut details = Vec::new();
    if assumptions_changed {
        details.push("assumptions changed".to_string());
    }
    if activation_changed {
        details.push("activation changed".to_string());
    }
    if guarantee_changed {
        details.push("guarantee changed".to_string());
    }
    if domains_changed {
        details.push("finite domains changed".to_string());
    }
    if observables_changed {
        details.push("observable structure changed".to_string());
    }
    if profile_bounds_changed {
        details.push("semantics profile bounds changed".to_string());
    }
    Ok(StructuralDelta {
        determined: true,
        statement_changed,
        assumptions_changed,
        activation_changed,
        guarantee_changed,
        domains_changed,
        observables_changed,
        profile_bounds_changed,
        editorial_eligible: details.is_empty(),
        details,
    })
}

fn unsatisfied_structural_delta() -> StructuralDelta {
    StructuralDelta {
        determined: false,
        statement_changed: false,
        assumptions_changed: false,
        activation_changed: false,
        guarantee_changed: false,
        domains_changed: false,
        observables_changed: false,
        profile_bounds_changed: false,
        editorial_eligible: false,
        details: vec![
            "the declared correspondence is not supported; no structural comparison ran".into(),
        ],
    }
}

fn assumption_guarantee_delta(
    outcome: &SearchOutcome,
    relation: RevisionRelation,
    guarantee_structurally_equal: bool,
) -> AssumptionGuaranteeDelta {
    let assumptions = formula_relation(
        outcome.complete,
        outcome.assumptions_new_to_old_violated,
        outcome.assumptions_old_to_new_violated,
    );
    let activation = formula_relation(
        outcome.complete,
        outcome.activation_new_to_old_violated,
        outcome.activation_old_to_new_violated,
    );
    let guarantee = formula_relation(
        outcome.complete,
        outcome.guarantee_new_to_old_violated,
        outcome.guarantee_old_to_new_violated,
    );
    let note = if assumptions == FormulaRelation::Strengthened && guarantee_structurally_equal {
        "stronger environmental assumptions reduce the implementation obligation even though the guarantee is unchanged"
    } else if assumptions == FormulaRelation::Weakened && guarantee_structurally_equal {
        "weaker environmental assumptions enlarge the implementation obligation even though the guarantee is unchanged"
    } else {
        "assumptions, activation and guarantee changes are reported separately from the overall obligation relation"
    };
    AssumptionGuaranteeDelta {
        assumptions,
        activation,
        guarantee,
        obligation: relation,
        guarantee_structurally_equal,
        note: note.into(),
    }
}

fn implication_result(
    direction: &str,
    premise: &str,
    conclusion: &str,
    outcome: &SearchOutcome,
    witness: &Option<ImplicationWitness>,
) -> ImplicationResult {
    let (holds, complete, reason) = if witness.is_some() {
        (
            Some(false),
            true,
            format!(
                "{premise} does not imply {conclusion}; the retained witness replays this counterexample"
            ),
        )
    } else if outcome.complete {
        (
            Some(true),
            true,
            format!("{premise} implies {conclusion} at every assignment in the declared finite universe"),
        )
    } else {
        (
            None,
            false,
            format!(
                "the finite search stopped before proving or refuting {premise} implies {conclusion}"
            ),
        )
    };
    ImplicationResult {
        direction: direction.into(),
        premise: premise.into(),
        conclusion: conclusion.into(),
        holds,
        complete,
        witness: witness.as_ref().cloned(),
        reason,
    }
}

fn search_basis(
    model: &CommonModel,
    outcome: &SearchOutcome,
    relation: RevisionRelation,
    bounds: &ComparisonBounds,
) -> RevisionSearchBasis {
    let common_domains = model
        .domains
        .iter()
        .map(|(name, domain)| (name.clone(), domain.values.clone()))
        .collect();
    let common_observables = model
        .observables
        .iter()
        .map(|(name, observable)| {
            (
                name.clone(),
                CommonObservableReport {
                    arguments: observable.arguments.clone(),
                    result: observable.result.clone(),
                },
            )
        })
        .collect();
    let conclusive = relation != RevisionRelation::Unknown;
    let basis = if outcome.complete {
        "all finite observation worlds were checked under the declared correspondence".to_string()
    } else if conclusive {
        "both failed implication directions have replayable witnesses; the bound stopped further enumeration".to_string()
    } else {
        "the declared world or step bound was exhausted before a complete implication search"
            .to_string()
    };
    RevisionSearchBasis {
        common_domains,
        common_observables,
        world_count: model.world_count,
        worlds_checked: outcome.worlds_checked,
        steps: outcome.steps,
        max_worlds: bounds.max_worlds,
        max_steps: bounds.max_steps,
        complete: outcome.complete,
        conclusive,
        basis,
    }
}

fn correspondence_status(
    inputs: &CorrespondenceInputs,
    old: &NormalizedContract,
    new: &NormalizedContract,
) -> Result<CorrespondenceStatus, RevisionClassificationError> {
    let old_review = match &inputs.old {
        Some(review) => Some(
            admit_correspondence_review(old, review, &inputs.authorized_reviewers).map_err(
                |error| {
                    RevisionClassificationError::InvalidCorrespondence(format!(
                        "prior correspondence review rejected: {error:?}"
                    ))
                },
            )?,
        ),
        None => None,
    };
    let new_review = match &inputs.new {
        Some(review) => Some(
            admit_correspondence_review(new, review, &inputs.authorized_reviewers).map_err(
                |error| {
                    RevisionClassificationError::InvalidCorrespondence(format!(
                        "current correspondence review rejected: {error:?}"
                    ))
                },
            )?,
        ),
        None => None,
    };
    Ok(CorrespondenceStatus {
        old: if old_review.is_some() {
            "admitted_declaration".into()
        } else {
            "not_admitted".into()
        },
        new: if new_review.is_some() {
            "admitted_declaration".into()
        } else {
            "not_admitted".into()
        },
        old_review_blake3: old_review,
        new_review_blake3: new_review,
        authorized_reviewers: inputs.authorized_reviewers.clone(),
        authenticity: "review declarations are not signatures; retained artifact existence, reviewer authenticity and review quality remain external trust".into(),
    })
}

fn evidence_reuse_decisions(
    request: &RequirementRevisionRequest,
    old: &NormalizedContract,
    new: &NormalizedContract,
    classification: RevisionClass,
) -> Result<Vec<EvidenceReuseDecision>, RevisionClassificationError> {
    let mut decisions = Vec::with_capacity(request.retained_evidence.len());
    let new_obligation = format!(
        "{}@{}",
        new.contract.requirement.requirement_id, new.contract_blake3
    );
    for evidence in &request.retained_evidence {
        if !digest(&evidence.evidence_blake3) || !digest(&evidence.receipt_blake3) {
            return Err(RevisionClassificationError::InvalidInput(
                "retained evidence and receipt identities must be BLAKE3 digests".into(),
            ));
        }
        let claim_id = claim_identity(&evidence.claim).map_err(|error| {
            RevisionClassificationError::InvalidInput(format!(
                "retained claim identity rejected: {error:?}"
            ))
        })?;
        let old_binding = evidence.claim.requirement == old.contract.requirement
            && evidence.claim.interpretation_blake3 == old.interpretation_blake3
            && evidence.claim.contract_blake3 == old.contract_blake3;
        let applicability_current = evidence.claim.context.subject_blake3
            == request.context.subject_blake3
            && evidence.claim.context.policy_blake3 == request.context.policy_blake3
            && evidence.claim.context.target == request.context.target
            && evidence.claim.context.environment == request.context.environment;
        let (decision, reason) = if !old_binding {
            (
                ReuseDecision::NotReusable,
                "claim is bound to a different requirement or contract revision".to_string(),
            )
        } else if !applicability_current {
            (
                ReuseDecision::NotReusable,
                "claim subject, policy, target or environment is not current".to_string(),
            )
        } else {
            match classification {
                RevisionClass::Editorial | RevisionClass::Clarifying => (
                    ReuseDecision::Reusable,
                    "the revisions are semantically equivalent, so the retained payload may seed a fresh derivation after re-admission; the old receipt is not current and automated transfer is disabled".to_string(),
                ),
                RevisionClass::Strengthening => (
                    ReuseDecision::NotReusable,
                    "the new obligation strengthens the old one; stale evidence cannot cross a strengthening change".to_string(),
                ),
                RevisionClass::Weakening => (
                    ReuseDecision::NotReusable,
                    "the new obligation weakens the old one; old evidence is not silently promoted and the scope change must be reviewed".to_string(),
                ),
                RevisionClass::Incompatible => (
                    ReuseDecision::NotReusable,
                    "the revisions are incompatible; no retained evidence is reusable".to_string(),
                ),
                RevisionClass::Undetermined => (
                    ReuseDecision::NotReusable,
                    "the revision relation is undetermined; no retained evidence is reusable".to_string(),
                ),
            }
        };
        decisions.push(EvidenceReuseDecision {
            evidence_blake3: evidence.evidence_blake3.clone(),
            receipt_blake3: evidence.receipt_blake3.clone(),
            claim_id,
            claim_kind: evidence.claim.kind.tag().to_string(),
            proposition: evidence.claim.proposition.tag().to_string(),
            new_obligation: new_obligation.clone(),
            decision,
            old_receipt_current: false,
            applicability_current,
            requires_new_derivation: decision == ReuseDecision::Reusable,
            transfer_allowed: false,
            reason,
        });
    }
    Ok(decisions)
}

fn transfer_status(policy: &RevisionClassificationPolicy) -> TransferPolicyStatus {
    TransferPolicyStatus {
        enabled: policy.transfer.enabled,
        admission_rules: policy.transfer.admission_rules.clone(),
        automated_transfer: "disabled_until_separately_qualified".into(),
    }
}

fn finish_requirement_report(
    mut report: RequirementComparisonReport,
) -> Result<RequirementComparisonReport, RevisionClassificationError> {
    report.report_blake3.clear();
    report.report_blake3 = identity("cairn.requirement-revision-report.v1", &report)?;
    Ok(report)
}

/// Compare two exact native requirement contract revisions under a finite
/// correspondence. The result is advisory and never refreshes evidence.
// r[impl cairn.revision_classification.requirement_relation]
// r[impl cairn.revision_classification.assumption_guarantee]
// r[impl cairn.revision_classification.evidence_reuse]
pub fn compare_requirement_revisions(
    policy: &RevisionClassificationPolicy,
    request: &RequirementRevisionRequest,
) -> Result<RequirementComparisonReport, RevisionClassificationError> {
    policy.admit()?;
    if request.schema != REQUIREMENT_REVISION_REQUEST_SCHEMA_V1 {
        return Err(RevisionClassificationError::Unsupported(format!(
            "unsupported requirement revision request schema {}",
            request.schema
        )));
    }
    if request.retained_evidence.len() > policy.comparison.max_evidence {
        return Err(RevisionClassificationError::BoundExhaustion(format!(
            "retained evidence {} exceeds the admitted bound {}",
            request.retained_evidence.len(),
            policy.comparison.max_evidence
        )));
    }
    validate_context(&request.context)?;
    validate_native_row(&request.old_native_requirement)?;
    validate_native_row(&request.new_native_requirement)?;
    let old = normalize_contract(
        &request.profile,
        &request.old_native_requirement,
        &request.old_contract,
    )
    .map_err(typed_error)?;
    let new = normalize_contract(
        &request.profile,
        &request.new_native_requirement,
        &request.new_contract,
    )
    .map_err(typed_error)?;
    validate_binding(&request.binding, &old, &new)?;
    let correspondence = correspondence_status(&request.correspondence, &old, &new)?;
    let input_blake3 = identity("cairn.requirement-revision-input.v1", request)?;
    let policy_blake3 = identity("cairn.revision-classification-policy.v1", policy)?;
    let old_bound = BoundContract {
        normalized: old.clone(),
        profile: request.profile.clone(),
        correspondence_blake3: correspondence.old_review_blake3.clone(),
    };
    let new_bound = BoundContract {
        normalized: new.clone(),
        profile: request.profile.clone(),
        correspondence_blake3: correspondence.new_review_blake3.clone(),
    };

    let build = build_models(
        &request.mapping,
        &old.contract,
        &new.contract,
        &policy.comparison,
    );
    match build {
        ModelBuild::Unsupported(unsupported) => {
            let classification = RevisionClass::Incompatible;
            let relation = RevisionRelation::Unknown;
            let evidence_reuse = evidence_reuse_decisions(request, &old, &new, classification)?;
            let implications = vec![
                ImplicationResult {
                    direction: "new_implies_old".into(),
                    premise: "new".into(),
                    conclusion: "old".into(),
                    holds: None,
                    complete: false,
                    witness: None,
                    reason: unsupported.detail.clone(),
                },
                ImplicationResult {
                    direction: "old_implies_new".into(),
                    premise: "old".into(),
                    conclusion: "new".into(),
                    holds: None,
                    complete: false,
                    witness: None,
                    reason: unsupported.detail.clone(),
                },
            ];
            finish_requirement_report(RequirementComparisonReport {
                schema: REQUIREMENT_REVISION_REPORT_SCHEMA_V1.into(),
                advisory: true,
                producer: REVISION_CLASSIFIER_PRODUCER.into(),
                classification_policy_blake3: policy_blake3.clone(),
                classification,
                relation,
                reason: unsupported.detail.clone(),
                old: old_bound,
                new: new_bound,
                binding: request.binding.clone(),
                context: request.context.clone(),
                mapping: request.mapping.clone(),
                structural: Some(unsatisfied_structural_delta()),
                assumption_guarantee: None,
                implications,
                search: None,
                evidence_reuse,
                correspondence,
                transfer: transfer_status(policy),
                unsupported: Some(unsupported),
                inconclusive: None,
                input_blake3,
                report_blake3: String::new(),
                boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
            })
        }
        ModelBuild::Inconclusive(inconclusive) => {
            let classification = RevisionClass::Undetermined;
            let relation = RevisionRelation::Unknown;
            let evidence_reuse = evidence_reuse_decisions(request, &old, &new, classification)?;
            let search = RevisionSearchBasis {
                common_domains: BTreeMap::new(),
                common_observables: BTreeMap::new(),
                world_count: 0,
                worlds_checked: 0,
                steps: 0,
                max_worlds: policy.comparison.max_worlds,
                max_steps: policy.comparison.max_steps,
                complete: false,
                conclusive: false,
                basis: inconclusive.detail.clone(),
            };
            finish_requirement_report(RequirementComparisonReport {
                schema: REQUIREMENT_REVISION_REPORT_SCHEMA_V1.into(),
                advisory: true,
                producer: REVISION_CLASSIFIER_PRODUCER.into(),
                classification_policy_blake3: policy_blake3.clone(),
                classification,
                relation,
                reason: inconclusive.detail.clone(),
                old: old_bound,
                new: new_bound,
                binding: request.binding.clone(),
                context: request.context.clone(),
                mapping: request.mapping.clone(),
                structural: None,
                assumption_guarantee: None,
                implications: vec![
                    ImplicationResult {
                        direction: "new_implies_old".into(),
                        premise: "new".into(),
                        conclusion: "old".into(),
                        holds: None,
                        complete: false,
                        witness: None,
                        reason: inconclusive.detail.clone(),
                    },
                    ImplicationResult {
                        direction: "old_implies_new".into(),
                        premise: "old".into(),
                        conclusion: "new".into(),
                        holds: None,
                        complete: false,
                        witness: None,
                        reason: inconclusive.detail.clone(),
                    },
                ],
                search: Some(search),
                evidence_reuse,
                correspondence,
                transfer: transfer_status(policy),
                unsupported: None,
                inconclusive: Some(inconclusive),
                input_blake3,
                report_blake3: String::new(),
                boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
            })
        }
        ModelBuild::Supported(model) => {
            let outcome = search_worlds(&model, &old.contract, &new.contract, &policy.comparison)?;
            let relation = revision_relation(
                outcome.complete,
                outcome.obligation_new_to_old_violated,
                outcome.obligation_old_to_new_violated,
            );
            let structural = match structural_delta(&old.contract, &new.contract, &model) {
                Ok(structural) => structural,
                Err(unsupported) => {
                    let classification = RevisionClass::Incompatible;
                    let evidence_reuse =
                        evidence_reuse_decisions(request, &old, &new, classification)?;
                    return finish_requirement_report(RequirementComparisonReport {
                        schema: REQUIREMENT_REVISION_REPORT_SCHEMA_V1.into(),
                        advisory: true,
                        producer: REVISION_CLASSIFIER_PRODUCER.into(),
                        classification_policy_blake3: policy_blake3.clone(),
                        classification,
                        relation: RevisionRelation::Unknown,
                        reason: unsupported.detail.clone(),
                        old: old_bound,
                        new: new_bound,
                        binding: request.binding.clone(),
                        context: request.context.clone(),
                        mapping: request.mapping.clone(),
                        structural: Some(unsatisfied_structural_delta()),
                        assumption_guarantee: None,
                        implications: Vec::new(),
                        search: None,
                        evidence_reuse,
                        correspondence,
                        transfer: transfer_status(policy),
                        unsupported: Some(unsupported),
                        inconclusive: None,
                        input_blake3,
                        report_blake3: String::new(),
                        boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
                    });
                }
            };
            let classification = classify_requirement(relation, &structural);
            let evidence_reuse = evidence_reuse_decisions(request, &old, &new, classification)?;
            let assumptions =
                assumption_guarantee_delta(&outcome, relation, !structural.guarantee_changed);
            let implications = vec![
                implication_result(
                    "new_implies_old",
                    "new",
                    "old",
                    &outcome,
                    &outcome.new_to_old_witness,
                ),
                implication_result(
                    "old_implies_new",
                    "old",
                    "new",
                    &outcome,
                    &outcome.old_to_new_witness,
                ),
            ];
            let search = search_basis(&model, &outcome, relation, &policy.comparison);
            let reason = match relation {
                RevisionRelation::Equivalent if structural.editorial_eligible => {
                    "the exact typed obligation is unchanged; only revision-bound prose or metadata differs"
                }
                RevisionRelation::Equivalent => {
                    "both obligations imply each other, but the finite structure differs"
                }
                RevisionRelation::Stronger => {
                    "the new obligation implies the old obligation and not conversely"
                }
                RevisionRelation::Weaker => {
                    "the old obligation implies the new obligation and not conversely"
                }
                RevisionRelation::Incomparable => {
                    "each direction has a replayable distinguishing witness"
                }
                RevisionRelation::Unknown => {
                    "the declared finite search did not establish either implication direction"
                }
            };
            finish_requirement_report(RequirementComparisonReport {
                schema: REQUIREMENT_REVISION_REPORT_SCHEMA_V1.into(),
                advisory: true,
                producer: REVISION_CLASSIFIER_PRODUCER.into(),
                classification_policy_blake3: policy_blake3.clone(),
                classification,
                relation,
                reason: reason.into(),
                old: old_bound,
                new: new_bound,
                binding: request.binding.clone(),
                context: request.context.clone(),
                mapping: request.mapping.clone(),
                structural: Some(structural),
                assumption_guarantee: Some(assumptions),
                implications,
                search: Some(search),
                evidence_reuse,
                correspondence,
                transfer: transfer_status(policy),
                unsupported: None,
                inconclusive: if relation == RevisionRelation::Unknown {
                    Some(InconclusiveEvidence {
                        code: "search_bound".into(),
                        detail: "the finite implication search stopped before a conclusion".into(),
                    })
                } else {
                    None
                },
                input_blake3,
                report_blake3: String::new(),
                boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionWitnessReplay {
    pub premise: SideTruths,
    pub conclusion: SideTruths,
    pub distinguishing: bool,
}

fn world_from_witness(
    model: &CommonModel,
    witness: &WitnessWorld,
) -> Result<Vec<usize>, RevisionClassificationError> {
    if witness.observables.len() != model.observables.len()
        || witness
            .observables
            .keys()
            .any(|name| !model.observables.contains_key(name))
    {
        return Err(RevisionClassificationError::InvalidInput(
            "witness observable set does not match the declared common correspondence".into(),
        ));
    }
    let mut world = vec![usize::MAX; model.slot_counts.len()];
    for (name, observable) in &model.observables {
        let rows = witness.observables.get(name).ok_or_else(|| {
            RevisionClassificationError::InvalidInput(format!(
                "witness is missing observable {name}"
            ))
        })?;
        if rows.len() != observable.row_count {
            return Err(RevisionClassificationError::InvalidInput(format!(
                "witness table {name} is not total"
            )));
        }
        let mut seen = BTreeSet::new();
        for row in rows {
            if row.arguments.len() != observable.arguments.len() {
                return Err(RevisionClassificationError::InvalidInput(format!(
                    "witness row arity differs for {name}"
                )));
            }
            let mut row_index = 0usize;
            for (position, argument) in row.arguments.iter().enumerate() {
                if argument.domain != observable.arguments[position] {
                    return Err(RevisionClassificationError::InvalidInput(format!(
                        "witness argument domain differs for {name}"
                    )));
                }
                let value_index = model.domains[&argument.domain]
                    .index
                    .get(&argument.value)
                    .ok_or_else(|| {
                        RevisionClassificationError::InvalidInput(format!(
                            "witness argument value {} is outside its domain",
                            argument.value
                        ))
                    })?;
                row_index = row_index
                    .checked_add(*value_index * observable.row_strides[position])
                    .ok_or_else(|| {
                        RevisionClassificationError::InvalidInput(
                            "witness row index overflows".into(),
                        )
                    })?;
            }
            if !seen.insert(row_index) {
                return Err(RevisionClassificationError::InvalidInput(format!(
                    "witness table {name} repeats an argument tuple"
                )));
            }
            if row.value.domain != observable.result {
                return Err(RevisionClassificationError::InvalidInput(format!(
                    "witness result domain differs for {name}"
                )));
            }
            let result_index = *model.domains[&observable.result]
                .index
                .get(&row.value.value)
                .ok_or_else(|| {
                    RevisionClassificationError::InvalidInput(format!(
                        "witness result value {} is outside its domain",
                        row.value.value
                    ))
                })?;
            world[observable.offset + row_index] = result_index;
        }
    }
    if world.contains(&usize::MAX) {
        return Err(RevisionClassificationError::InvalidInput(
            "witness does not cover every observation slot".into(),
        ));
    }
    Ok(world)
}

/// Re-evaluate a retained implication witness against the same exact revisions
/// and declared correspondence. This is the replay check for reported witnesses.
// r[impl cairn.revision_classification.requirement_relation]
#[allow(clippy::too_many_arguments)]
pub fn replay_requirement_witness(
    policy: &RevisionClassificationPolicy,
    profile: &SemanticProfile,
    old_native_requirement: &ContentBoundRequirementRow,
    old_contract: &SemanticContract,
    new_native_requirement: &ContentBoundRequirementRow,
    new_contract: &SemanticContract,
    mapping: &RevisionMapping,
    witness: &ImplicationWitness,
) -> Result<RevisionWitnessReplay, RevisionClassificationError> {
    policy.admit()?;
    validate_native_row(old_native_requirement)?;
    validate_native_row(new_native_requirement)?;
    let old =
        normalize_contract(profile, old_native_requirement, old_contract).map_err(typed_error)?;
    let new =
        normalize_contract(profile, new_native_requirement, new_contract).map_err(typed_error)?;
    let model = match build_models(mapping, &old.contract, &new.contract, &policy.comparison) {
        ModelBuild::Supported(model) => model,
        ModelBuild::Unsupported(unsupported) => {
            return Err(RevisionClassificationError::Unsupported(format!(
                "{}: {}",
                unsupported.code, unsupported.detail
            )))
        }
        ModelBuild::Inconclusive(inconclusive) => {
            return Err(RevisionClassificationError::BoundExhaustion(format!(
                "{}: {}",
                inconclusive.code, inconclusive.detail
            )))
        }
    };
    if identity("cairn.revision-witness-world.v1", &witness.world)? != witness.world_blake3 {
        return Err(RevisionClassificationError::InvalidInput(
            "witness world identity does not match its retained content".into(),
        ));
    }
    let world = world_from_witness(&model, &witness.world)?;
    let mut budget = EvalBudget {
        used: 0,
        maximum: policy.comparison.max_steps,
    };
    let mut variables = BTreeMap::new();
    let mut old_assumptions = Vec::new();
    let mut new_assumptions = Vec::new();
    let old_side = eval_side(
        &model,
        &model.old,
        &old.contract,
        &world,
        &mut variables,
        &mut budget,
        &mut old_assumptions,
    )
    .map_err(|failure| match failure {
        EvalFailure::Budget => RevisionClassificationError::BoundExhaustion(
            "witness replay exhausted the admitted step bound".into(),
        ),
        EvalFailure::Internal(detail) => RevisionClassificationError::InvalidInput(detail),
    })?;
    let new_side = eval_side(
        &model,
        &model.new,
        &new.contract,
        &world,
        &mut variables,
        &mut budget,
        &mut new_assumptions,
    )
    .map_err(|failure| match failure {
        EvalFailure::Budget => RevisionClassificationError::BoundExhaustion(
            "witness replay exhausted the admitted step bound".into(),
        ),
        EvalFailure::Internal(detail) => RevisionClassificationError::InvalidInput(detail),
    })?;
    let (premise, conclusion) = match (
        witness.premise_side.as_str(),
        witness.conclusion_side.as_str(),
    ) {
        ("new", "old") => (
            side_truths(new_side, &new_assumptions),
            side_truths(old_side, &old_assumptions),
        ),
        ("old", "new") => (
            side_truths(old_side, &old_assumptions),
            side_truths(new_side, &new_assumptions),
        ),
        _ => {
            return Err(RevisionClassificationError::InvalidInput(
                "witness names an unknown premise or conclusion side".into(),
            ))
        }
    };
    if premise != witness.premise || conclusion != witness.conclusion {
        return Err(RevisionClassificationError::InvalidInput(
            "witness truth annotations do not replay against the declared revisions".into(),
        ));
    }
    let distinguishing = premise.obligation_holds && !conclusion.obligation_holds;
    Ok(RevisionWitnessReplay {
        premise,
        conclusion,
        distinguishing,
    })
}

fn validate_fragment(fragment: &AcceptanceFragment) -> Result<(), RevisionClassificationError> {
    if fragment.schema != ACCEPTANCE_FRAGMENT_SCHEMA_V1 {
        return Err(RevisionClassificationError::InvalidPolicy(format!(
            "unsupported acceptance fragment schema {}",
            fragment.schema
        )));
    }
    for (label, names) in [
        ("check", &fragment.universe.checks),
        ("evidence role", &fragment.universe.evidence_roles),
        ("assumption", &fragment.universe.assumptions),
    ] {
        for name in names {
            if name.trim().is_empty() {
                return Err(RevisionClassificationError::InvalidPolicy(format!(
                    "{label} names must be non-empty"
                )));
            }
        }
    }
    for check in &fragment.required_checks {
        if !fragment.universe.checks.contains(check) {
            return Err(RevisionClassificationError::InvalidPolicy(format!(
                "required check {check} is outside the declared accepted-evidence universe"
            )));
        }
    }
    for role in &fragment.evidence_roles {
        if !fragment.universe.evidence_roles.contains(role) {
            return Err(RevisionClassificationError::InvalidPolicy(format!(
                "evidence role {role} is outside the declared accepted-evidence universe"
            )));
        }
    }
    for assumptions in &fragment.assumption_sets {
        for assumption in assumptions {
            if !fragment.universe.assumptions.contains(assumption) {
                return Err(RevisionClassificationError::InvalidPolicy(format!(
                    "assumption {assumption} is outside the declared accepted-evidence universe"
                )));
            }
        }
    }
    Ok(())
}

fn normalized_fragment(
    fragment: &AcceptanceFragment,
) -> Result<AcceptanceFragment, RevisionClassificationError> {
    validate_fragment(fragment)?;
    let mut normalized = fragment.clone();
    normalized.assumption_sets.sort();
    if normalized
        .assumption_sets
        .windows(2)
        .any(|window| window[0] == window[1])
    {
        return Err(RevisionClassificationError::InvalidPolicy(
            "acceptance fragment repeats an allowed assumption set".into(),
        ));
    }
    Ok(normalized)
}

fn fragment_feasible(fragment: &AcceptanceFragment) -> bool {
    !fragment.evidence_roles.is_empty() && !fragment.assumption_sets.is_empty()
}

fn accepted_count(fragment: &AcceptanceFragment, maximum: u64) -> Option<u64> {
    if !fragment_feasible(fragment) {
        return Some(0);
    }
    let free_checks = fragment
        .universe
        .checks
        .len()
        .checked_sub(fragment.required_checks.len())?;
    let check_choices = 1u64.checked_shl(u32::try_from(free_checks).ok()?)?;
    let roles = u64::try_from(fragment.evidence_roles.len()).ok()?;
    let sets = u64::try_from(fragment.assumption_sets.len()).ok()?;
    let count = check_choices.checked_mul(roles)?.checked_mul(sets)?;
    (count <= maximum).then_some(count)
}

fn universe_candidate_count(universe: &EvidenceUniverse, maximum: u64) -> Option<u64> {
    let checks = 1u64.checked_shl(u32::try_from(universe.checks.len()).ok()?)?;
    let assumptions = 1u64.checked_shl(u32::try_from(universe.assumptions.len()).ok()?)?;
    let roles = u64::try_from(universe.evidence_roles.len()).ok()?;
    let count = checks.checked_mul(roles)?.checked_mul(assumptions)?;
    (count <= maximum).then_some(count)
}

fn first_candidate(fragment: &AcceptanceFragment) -> Option<EvidenceCandidate> {
    if !fragment_feasible(fragment) {
        return None;
    }
    Some(EvidenceCandidate {
        checks: fragment.required_checks.iter().cloned().collect(),
        evidence_role: fragment.evidence_roles.iter().next()?.clone(),
        assumptions: fragment.assumption_sets.first()?.iter().cloned().collect(),
    })
}

/// Replays one policy-set witness against the declared fragment.
// r[impl cairn.revision_classification.policy_fragment]
pub fn candidate_accepted(fragment: &AcceptanceFragment, candidate: &EvidenceCandidate) -> bool {
    let unique_checks: BTreeSet<&String> = candidate.checks.iter().collect();
    let unique_assumptions: BTreeSet<&String> = candidate.assumptions.iter().collect();
    if unique_checks.len() != candidate.checks.len()
        || unique_assumptions.len() != candidate.assumptions.len()
    {
        return false;
    }
    if candidate
        .checks
        .iter()
        .any(|check| !fragment.universe.checks.contains(check))
        || candidate
            .assumptions
            .iter()
            .any(|assumption| !fragment.universe.assumptions.contains(assumption))
    {
        return false;
    }
    if !fragment
        .required_checks
        .iter()
        .all(|check| candidate.checks.contains(check))
    {
        return false;
    }
    if !fragment.evidence_roles.contains(&candidate.evidence_role) {
        return false;
    }
    fragment.assumption_sets.iter().any(|allowed| {
        allowed.len() == candidate.assumptions.len()
            && allowed
                .iter()
                .all(|assumption| candidate.assumptions.contains(assumption))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyWitnessReplay {
    pub premise_accepted: bool,
    pub conclusion_accepted: bool,
    pub distinguishing: bool,
}

/// Replays one containment witness against both declared acceptance fragments.
// r[impl cairn.revision_classification.policy_fragment]
pub fn replay_policy_witness(
    premise: &AcceptanceFragment,
    conclusion: &AcceptanceFragment,
    witness: &EvidenceCandidate,
) -> Result<PolicyWitnessReplay, RevisionClassificationError> {
    for fragment in [premise, conclusion] {
        if fragment.fragment != SUPPORTED_POLICY_FRAGMENT_V1 {
            return Err(RevisionClassificationError::Unsupported(format!(
                "unsupported acceptance fragment {}",
                fragment.fragment
            )));
        }
    }
    validate_fragment(premise)?;
    validate_fragment(conclusion)?;
    let premise_accepted = candidate_accepted(premise, witness);
    let conclusion_accepted = candidate_accepted(conclusion, witness);
    Ok(PolicyWitnessReplay {
        premise_accepted,
        conclusion_accepted,
        distinguishing: premise_accepted && !conclusion_accepted,
    })
}

fn set_inclusion(
    premise: &AcceptanceFragment,
    conclusion: &AcceptanceFragment,
    direction: &str,
) -> SetInclusion {
    let premise_feasible = fragment_feasible(premise);
    let conclusion_feasible = fragment_feasible(conclusion);
    if !premise_feasible {
        return SetInclusion {
            direction: direction.into(),
            holds: true,
            vacuous: true,
            witness: None,
            reason: "the premise acceptance set is empty, so containment is vacuous".into(),
        };
    }
    if !conclusion_feasible {
        return SetInclusion {
            direction: direction.into(),
            holds: false,
            vacuous: false,
            witness: first_candidate(premise),
            reason: "the conclusion acceptance set is empty and the premise set is not".into(),
        };
    }
    if !premise.evidence_roles.is_subset(&conclusion.evidence_roles) {
        let role = premise
            .evidence_roles
            .difference(&conclusion.evidence_roles)
            .next()
            .cloned()
            .unwrap_or_default();
        return SetInclusion {
            direction: direction.into(),
            holds: false,
            vacuous: false,
            witness: Some(EvidenceCandidate {
                checks: premise.required_checks.iter().cloned().collect(),
                evidence_role: role.clone(),
                assumptions: premise
                    .assumption_sets
                    .first()
                    .map(|set| set.iter().cloned().collect())
                    .unwrap_or_default(),
            }),
            reason: format!(
                "evidence role {role} is accepted by the premise and rejected by the conclusion"
            ),
        };
    }
    if let Some(assumptions) = premise
        .assumption_sets
        .iter()
        .find(|assumptions| !conclusion.assumption_sets.contains(*assumptions))
    {
        return SetInclusion {
            direction: direction.into(),
            holds: false,
            vacuous: false,
            witness: Some(EvidenceCandidate {
                checks: premise.required_checks.iter().cloned().collect(),
                evidence_role: premise
                    .evidence_roles
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_default(),
                assumptions: assumptions.iter().cloned().collect(),
            }),
            reason: "an allowed assumption set is accepted by the premise and rejected by the conclusion".into(),
        };
    }
    if let Some(check) = conclusion
        .required_checks
        .difference(&premise.required_checks)
        .next()
    {
        return SetInclusion {
            direction: direction.into(),
            holds: false,
            vacuous: false,
            witness: Some(EvidenceCandidate {
                checks: premise.required_checks.iter().cloned().collect(),
                evidence_role: premise
                    .evidence_roles
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_default(),
                assumptions: premise
                    .assumption_sets
                    .first()
                    .map(|set| set.iter().cloned().collect())
                    .unwrap_or_default(),
            }),
            reason: format!("required check {check} narrows the conclusion but not the premise"),
        };
    }
    SetInclusion {
        direction: direction.into(),
        holds: true,
        vacuous: false,
        witness: None,
        reason: "every accepted evidence candidate in the premise is accepted by the conclusion"
            .into(),
    }
}

fn policy_fragment_report(
    fragment: &AcceptanceFragment,
    maximum: u64,
) -> Result<PolicyFragmentReport, RevisionClassificationError> {
    let feasible = fragment_feasible(fragment);
    let accepted_count = accepted_count(fragment, maximum);
    Ok(PolicyFragmentReport {
        fragment_blake3: identity("cairn.acceptance-fragment.v1", fragment)?,
        required_checks: fragment.required_checks.iter().cloned().collect(),
        evidence_roles: fragment.evidence_roles.iter().cloned().collect(),
        assumption_sets: fragment
            .assumption_sets
            .iter()
            .map(|set| set.iter().cloned().collect())
            .collect(),
        accepted_count,
        feasible,
        accepted_empty: !feasible,
        count_within_bound: accepted_count.is_some(),
    })
}

fn policy_universe_report(universe: &EvidenceUniverse, maximum: u64) -> EvidenceUniverseReport {
    EvidenceUniverseReport {
        checks: universe.checks.iter().cloned().collect(),
        evidence_roles: universe.evidence_roles.iter().cloned().collect(),
        assumptions: universe.assumptions.iter().cloned().collect(),
        candidate_count: universe_candidate_count(universe, maximum),
    }
}

fn policy_structural_delta(
    old: &AcceptanceFragment,
    new: &AcceptanceFragment,
) -> PolicyStructuralDelta {
    let required_checks_changed = old.required_checks != new.required_checks;
    let evidence_roles_changed = old.evidence_roles != new.evidence_roles;
    let assumption_sets_changed = old.assumption_sets != new.assumption_sets;
    let universe_changed = old.universe != new.universe;
    let mut details = Vec::new();
    if required_checks_changed {
        details.push("required check set changed".to_string());
    }
    if evidence_roles_changed {
        details.push("admissible evidence roles changed".to_string());
    }
    if assumption_sets_changed {
        details.push("allowed assumption sets changed".to_string());
    }
    if universe_changed {
        details.push("accepted-evidence universe changed".to_string());
    }
    PolicyStructuralDelta {
        determined: true,
        required_checks_changed,
        evidence_roles_changed,
        assumption_sets_changed,
        universe_changed,
        editorial_eligible: details.is_empty(),
        details,
    }
}

fn unsatisfied_policy_structural_delta(detail: &str) -> PolicyStructuralDelta {
    PolicyStructuralDelta {
        determined: false,
        required_checks_changed: false,
        evidence_roles_changed: false,
        assumption_sets_changed: false,
        universe_changed: false,
        editorial_eligible: false,
        details: vec![detail.to_string()],
    }
}

fn classify_policy(relation: SetRelation, structural: &PolicyStructuralDelta) -> RevisionClass {
    match relation {
        SetRelation::Equivalent if structural.editorial_eligible => RevisionClass::Editorial,
        SetRelation::Equivalent => RevisionClass::Clarifying,
        SetRelation::Tightened => RevisionClass::Strengthening,
        SetRelation::Widened => RevisionClass::Weakening,
        SetRelation::Incomparable => RevisionClass::Incompatible,
        SetRelation::Unknown => RevisionClass::Undetermined,
    }
}

fn policy_feasibility(
    inclusion: &[SetInclusion],
    old: &AcceptanceFragment,
    new: &AcceptanceFragment,
) -> PolicyFeasibility {
    let vacuous_inclusion = inclusion.iter().any(|result| result.vacuous);
    let note = if !fragment_feasible(old) || !fragment_feasible(new) {
        "an empty acceptance set is visible independently of the containment ordering; containment involving it is vacuous"
    } else {
        "feasibility and accepted-set cardinality are reported separately from the containment ordering"
    };
    PolicyFeasibility {
        old_feasible: fragment_feasible(old),
        new_feasible: fragment_feasible(new),
        old_accepted_empty: !fragment_feasible(old),
        new_accepted_empty: !fragment_feasible(new),
        vacuous_inclusion,
        note: note.into(),
    }
}

fn finish_policy_report(
    mut report: PolicyComparisonReport,
) -> Result<PolicyComparisonReport, RevisionClassificationError> {
    report.report_blake3.clear();
    report.report_blake3 = identity("cairn.policy-revision-report.v1", &report)?;
    Ok(report)
}

/// Compare two declared acceptance-policy fragments. Only the supported named
/// required-check, evidence-role and assumption-set fragment is interpreted.
// r[impl cairn.revision_classification.policy_fragment]
pub fn compare_policy_revisions(
    policy: &RevisionClassificationPolicy,
    request: &PolicyRevisionRequest,
) -> Result<PolicyComparisonReport, RevisionClassificationError> {
    policy.admit()?;
    if request.schema != POLICY_REVISION_REQUEST_SCHEMA_V1 {
        return Err(RevisionClassificationError::Unsupported(format!(
            "unsupported policy revision request schema {}",
            request.schema
        )));
    }
    if !digest(&request.binding.old_policy_blake3) || !digest(&request.binding.new_policy_blake3) {
        return Err(RevisionClassificationError::InvalidInput(
            "policy revision identities must be BLAKE3 digests".into(),
        ));
    }
    let old = normalized_fragment(&request.old)?;
    let new = normalized_fragment(&request.new)?;
    let input_blake3 = identity("cairn.policy-revision-input.v1", request)?;
    let policy_blake3 = identity("cairn.revision-classification-policy.v1", policy)?;
    let old_report = policy_fragment_report(&old, policy.policy_fragment.max_accepted)?;
    let new_report = policy_fragment_report(&new, policy.policy_fragment.max_accepted)?;
    let universe = policy_universe_report(&old.universe, policy.policy_fragment.max_accepted);

    if old.fragment != SUPPORTED_POLICY_FRAGMENT_V1 || new.fragment != SUPPORTED_POLICY_FRAGMENT_V1
    {
        let unsupported = UnsupportedDifference {
            code: "unsupported_fragment".into(),
            detail: "only the named required-check, evidence-role and assumption-set acceptance fragment is supported".into(),
        };
        return finish_policy_report(PolicyComparisonReport {
            schema: POLICY_REVISION_REPORT_SCHEMA_V1.into(),
            advisory: true,
            producer: REVISION_CLASSIFIER_PRODUCER.into(),
            classification_policy_blake3: policy_blake3.clone(),
            classification: RevisionClass::Incompatible,
            relation: SetRelation::Unknown,
            reason: unsupported.detail.clone(),
            supported_fragment: SUPPORTED_POLICY_FRAGMENT_V1.into(),
            binding: request.binding.clone(),
            universe,
            old: old_report,
            new: new_report,
            inclusion: Vec::new(),
            feasibility: policy_feasibility(&[], &old, &new),
            structural: unsatisfied_policy_structural_delta(&unsupported.detail),
            unsupported: Some(unsupported),
            inconclusive: None,
            input_blake3,
            report_blake3: String::new(),
            boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
        });
    }
    if old.universe != new.universe {
        let unsupported = UnsupportedDifference {
            code: "universe_mismatch".into(),
            detail: "the two fragments declare different accepted-evidence universes, so containment is not comparable".into(),
        };
        return finish_policy_report(PolicyComparisonReport {
            schema: POLICY_REVISION_REPORT_SCHEMA_V1.into(),
            advisory: true,
            producer: REVISION_CLASSIFIER_PRODUCER.into(),
            classification_policy_blake3: policy_blake3.clone(),
            classification: RevisionClass::Incompatible,
            relation: SetRelation::Unknown,
            reason: unsupported.detail.clone(),
            supported_fragment: SUPPORTED_POLICY_FRAGMENT_V1.into(),
            binding: request.binding.clone(),
            universe,
            old: old_report,
            new: new_report,
            inclusion: Vec::new(),
            feasibility: policy_feasibility(&[], &old, &new),
            structural: unsatisfied_policy_structural_delta(&unsupported.detail),
            unsupported: Some(unsupported),
            inconclusive: None,
            input_blake3,
            report_blake3: String::new(),
            boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
        });
    }
    if old.universe.checks.len() > policy.policy_fragment.max_checks
        || old.universe.evidence_roles.len() > policy.policy_fragment.max_roles
        || old.universe.assumptions.len() > policy.policy_fragment.max_assumptions
        || old.assumption_sets.len() > policy.policy_fragment.max_assumption_sets
        || new.assumption_sets.len() > policy.policy_fragment.max_assumption_sets
    {
        let inconclusive = InconclusiveEvidence {
            code: "fragment_bound".into(),
            detail: "the declared accepted-evidence universe or assumption-set list exceeds the admitted bound".into(),
        };
        return finish_policy_report(PolicyComparisonReport {
            schema: POLICY_REVISION_REPORT_SCHEMA_V1.into(),
            advisory: true,
            producer: REVISION_CLASSIFIER_PRODUCER.into(),
            classification_policy_blake3: policy_blake3.clone(),
            classification: RevisionClass::Undetermined,
            relation: SetRelation::Unknown,
            reason: inconclusive.detail.clone(),
            supported_fragment: SUPPORTED_POLICY_FRAGMENT_V1.into(),
            binding: request.binding.clone(),
            universe,
            old: old_report,
            new: new_report,
            inclusion: Vec::new(),
            feasibility: policy_feasibility(&[], &old, &new),
            structural: policy_structural_delta(&old, &new),
            unsupported: None,
            inconclusive: Some(inconclusive),
            input_blake3,
            report_blake3: String::new(),
            boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
        });
    }

    let inclusion = vec![
        set_inclusion(&new, &old, "new_accepted_subset_of_old"),
        set_inclusion(&old, &new, "old_accepted_subset_of_new"),
    ];
    let new_to_old = inclusion[0].holds;
    let old_to_new = inclusion[1].holds;
    let relation = match (new_to_old, old_to_new) {
        (true, true) => SetRelation::Equivalent,
        (true, false) => SetRelation::Tightened,
        (false, true) => SetRelation::Widened,
        (false, false) => SetRelation::Incomparable,
    };
    let structural = policy_structural_delta(&old, &new);
    let classification = classify_policy(relation, &structural);
    let feasibility = policy_feasibility(&inclusion, &old, &new);
    let reason = match relation {
        SetRelation::Equivalent if structural.editorial_eligible => {
            "the declared acceptance structure is unchanged"
        }
        SetRelation::Equivalent => {
            "both acceptance sets contain each other, but their declared structure differs"
        }
        SetRelation::Tightened => {
            "the new acceptance set is a strict subset of the old acceptance set; the gate is strengthened"
        }
        SetRelation::Widened => {
            "the old acceptance set is a strict subset of the new acceptance set; the gate is weakened"
        }
        SetRelation::Incomparable => {
            "neither acceptance set contains the other; a replayable candidate distinguishes each direction"
        }
        SetRelation::Unknown => "the declared fragment bound prevented a complete comparison",
    };
    finish_policy_report(PolicyComparisonReport {
        schema: POLICY_REVISION_REPORT_SCHEMA_V1.into(),
        advisory: true,
        producer: REVISION_CLASSIFIER_PRODUCER.into(),
        classification_policy_blake3: policy_blake3.clone(),
        classification,
        relation,
        reason: reason.into(),
        supported_fragment: SUPPORTED_POLICY_FRAGMENT_V1.into(),
        binding: request.binding.clone(),
        universe,
        old: old_report,
        new: new_report,
        inclusion,
        feasibility,
        structural,
        unsupported: None,
        inconclusive: None,
        input_blake3,
        report_blake3: String::new(),
        boundary: REVISION_CLASSIFICATION_BOUNDARY.into(),
    })
}

/// Future transfer admission entry point. It validates the declared record but
/// always refuses while automated transfer is unqualified.
// r[impl cairn.revision_classification.transfer_disabled]
pub fn plan_evidence_transfer(
    policy: &RevisionClassificationPolicy,
    comparison: &RequirementComparisonReport,
    request: &EvidenceTransferRequest,
) -> Result<EvidenceTransferRecord, RevisionClassificationError> {
    policy.admit()?;
    if request.schema != EVIDENCE_TRANSFER_SCHEMA_V1 {
        return Err(RevisionClassificationError::Unsupported(format!(
            "unsupported evidence transfer request schema {}",
            request.schema
        )));
    }
    if !digest(&request.evidence_blake3)
        || !digest(&request.old_claim_blake3)
        || !digest(&request.new_claim_blake3)
        || !digest(&request.comparison_report_blake3)
        || !digest(&request.subject_blake3)
        || request.environment.trim().is_empty()
    {
        return Err(RevisionClassificationError::InvalidInput(
            "transfer request requires exact evidence, claim, comparison, subject and environment identities".into(),
        ));
    }
    if request.comparison_report_blake3 != comparison.report_blake3 {
        return Err(RevisionClassificationError::InvalidInput(
            "transfer request names a different comparison report".into(),
        ));
    }
    if request.subject_blake3 != comparison.context.subject_blake3
        || request.environment != comparison.context.environment
    {
        return Err(RevisionClassificationError::InvalidInput(
            "transfer request subject or environment is not current for this comparison".into(),
        ));
    }
    Err(RevisionClassificationError::Unsupported(
        TRANSFER_DISABLED_REASON.into(),
    ))
}

#[cfg(test)]
mod tests;
