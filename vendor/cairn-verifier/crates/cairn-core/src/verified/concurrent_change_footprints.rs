//! Advisory concurrent-change compatibility obligations from semantic footprints.
//!
//! A change footprint names exact source and intent revisions, the typed
//! requirement revisions it changes or consumes, the guarantees it changes, the
//! assumptions it consumes, the invariants it commits to, the obligations it
//! invalidates, the interfaces and configurations it touches, and the scope it
//! explicitly does not know. The analyzer derives interactions between two
//! declared footprints and emits one compatibility obligation for the exact
//! combined candidate.
//!
//! This module is deliberately not an authority. A derived obligation is a
//! review surface: it never resolves a native requirement, refreshes evidence,
//! transfers a receipt, authorizes a merge or release, or changes any gate,
//! readiness, verification-role, collaboration-resolution or
//! verification-impact decision. Individual receipts remain individually
//! scoped, and a finite implication result covers only the declared finite
//! universe and admitted bounds.
// r[impl cairn.concurrent_footprints.footprint]
#![allow(clippy::collapsible_if)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use super::evidence::canonical_json;
use super::typed_requirement_contracts::{
    normalize_contract, FiniteDomain, FiniteExpr, FiniteTerm, FiniteValue, ObservableMapping,
    ObservableRow, SemanticContract, SemanticProfile,
};

pub const FOOTPRINT_SCHEMA_V1: &str = "cairn.change-footprint.v1";
pub const CASE_SCHEMA_V1: &str = "cairn.concurrent-change-case.v1";
pub const POLICY_SCHEMA_V1: &str = "cairn.concurrent-change-footprints-policy.v1";
pub const REPORT_SCHEMA_V1: &str = "cairn.concurrent-change-footprints-report.v1";
pub const FOOTPRINTS_PRODUCER: &str = "cairn.concurrent-footprints.v1";
// r[impl cairn.concurrent_footprints.independence]
pub const CONCURRENT_FOOTPRINTS_BOUNDARY: &str = "Advisory concurrent-change compatibility analysis only: a derived interaction is a declared-footprint inference, not behavioral independence, implementation correctness or merge permission. A finite implication result covers only one declared finite universe, admitted observable tables and policy bounds. A satisfiability witness is one model; it establishes no preservation over all admitted compositions. Individual receipts stay individually scoped and never discharge a combined-candidate obligation. Invariant preservation never implies commutativity. Textual collaboration conflict resolution, verification-impact, native applicability validators, readiness states, gates and lifecycle decisions retain their own authority and remain independently usable.";

const MAX_CHANGES_CEILING: usize = 64;
const MAX_REVISIONS_CEILING: usize = 256;
const MAX_ASSUMPTIONS_CEILING: usize = 512;
const MAX_GUARANTEES_CEILING: usize = 512;
const MAX_INVARIANTS_CEILING: usize = 512;
const MAX_INVALIDATIONS_CEILING: usize = 512;
const MAX_INTERFACES_CEILING: usize = 512;
const MAX_UNKNOWN_CEILING: usize = 64;
const MAX_COMPOSITIONS_CEILING: usize = 64;
const MAX_EVIDENCE_CEILING: usize = 1024;
const MAX_CONDITIONS_CEILING: usize = 256;
const MAX_DOMAIN_VALUES_CEILING: usize = 256;
const MAX_WORLDS_CEILING: u64 = 1_048_576;
const MAX_STEPS_CEILING: u64 = 10_000_000;
const MAX_DEPTH_CEILING: usize = 64;

const GUARANTEE_IDENTITY_DOMAIN_V1: &str = "cairn.concurrent-footprint-guarantee.v1";
const INVARIANT_IDENTITY_DOMAIN_V1: &str = "cairn.concurrent-footprint-invariant.v1";
const INTERACTION_IDENTITY_DOMAIN_V1: &str = "cairn.compatibility-interaction.v1";
const CANDIDATE_IDENTITY_DOMAIN_V1: &str = "cairn.combined-change-candidate.v1";
const OBLIGATION_IDENTITY_DOMAIN_V1: &str = "cairn.compatibility-obligation.v1";
const INPUT_IDENTITY_DOMAIN_V1: &str = "cairn.concurrent-change-input.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum FootprintError {
    Unsupported(String),
    InvalidInput(String),
    RevisionMismatch(String),
    BoundExhaustion(String),
    Serialization(String),
}

impl FootprintError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::InvalidInput(_) => "invalid_input",
            Self::RevisionMismatch(_) => "revision_mismatch",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::Serialization(_) => "serialization",
        }
    }
}

impl std::fmt::Display for FootprintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for FootprintError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintPolicy {
    pub schema: String,
    pub bounds: FootprintBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintBounds {
    pub max_changes: usize,
    pub max_revisions: usize,
    pub max_assumptions: usize,
    pub max_guarantees: usize,
    pub max_invariants: usize,
    pub max_invalidations: usize,
    pub max_interfaces: usize,
    pub max_unknown: usize,
    pub max_compositions: usize,
    pub max_evidence: usize,
    pub max_conditions: usize,
    pub max_domain_values: usize,
    pub max_worlds: u64,
    pub max_steps: u64,
    pub max_depth: usize,
}

impl FootprintPolicy {
    // r[impl cairn.concurrent_footprints.bounds]
    pub fn admit(&self) -> Result<(), FootprintError> {
        if self.schema != POLICY_SCHEMA_V1 {
            return Err(FootprintError::Unsupported(format!(
                "unsupported concurrent change footprints policy schema {}",
                self.schema
            )));
        }
        let bounds = &self.bounds;
        let within = bounds.max_changes > 0
            && bounds.max_changes <= MAX_CHANGES_CEILING
            && bounds.max_revisions > 0
            && bounds.max_revisions <= MAX_REVISIONS_CEILING
            && bounds.max_assumptions > 0
            && bounds.max_assumptions <= MAX_ASSUMPTIONS_CEILING
            && bounds.max_guarantees > 0
            && bounds.max_guarantees <= MAX_GUARANTEES_CEILING
            && bounds.max_invariants > 0
            && bounds.max_invariants <= MAX_INVARIANTS_CEILING
            && bounds.max_invalidations > 0
            && bounds.max_invalidations <= MAX_INVALIDATIONS_CEILING
            && bounds.max_interfaces > 0
            && bounds.max_interfaces <= MAX_INTERFACES_CEILING
            && bounds.max_unknown > 0
            && bounds.max_unknown <= MAX_UNKNOWN_CEILING
            && bounds.max_compositions > 0
            && bounds.max_compositions <= MAX_COMPOSITIONS_CEILING
            && bounds.max_evidence > 0
            && bounds.max_evidence <= MAX_EVIDENCE_CEILING
            && bounds.max_conditions > 0
            && bounds.max_conditions <= MAX_CONDITIONS_CEILING
            && bounds.max_domain_values > 0
            && bounds.max_domain_values <= MAX_DOMAIN_VALUES_CEILING
            && bounds.max_worlds > 0
            && bounds.max_worlds <= MAX_WORLDS_CEILING
            && bounds.max_steps > 0
            && bounds.max_steps <= MAX_STEPS_CEILING
            && bounds.max_depth > 0
            && bounds.max_depth <= MAX_DEPTH_CEILING;
        if !within {
            return Err(FootprintError::BoundExhaustion(
                "concurrent change footprints policy bounds are empty or exceed the implementation ceilings"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeIdentity {
    pub change_id: String,
    pub source_revision: String,
    pub intent_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum StatementForm {
    Finite { expression: FiniteExpr },
    Informal { statement: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuaranteeStatement {
    pub requirement_id: String,
    pub form: StatementForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuaranteeChange {
    Strengthened,
    Weakened,
    Replaced,
    Removed,
    Preserved,
    Unknown,
}

impl GuaranteeChange {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Strengthened => "strengthened",
            Self::Weakened => "weakened",
            Self::Replaced => "replaced",
            Self::Removed => "removed",
            Self::Preserved => "preserved",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedGuarantee {
    pub id: String,
    pub previous: Option<GuaranteeStatement>,
    pub current: Option<GuaranteeStatement>,
    pub change: GuaranteeChange,
    pub affected_claims: Vec<String>,
    pub interfaces: Vec<String>,
    pub configurations: Vec<String>,
    pub unknown: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionDependency {
    Strict,
    Monotone,
}

impl AssumptionDependency {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Monotone => "monotone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumedAssumption {
    pub id: String,
    pub requirement_id: String,
    pub consumed_change: String,
    pub consumed_guarantee: String,
    pub consumed_identity: String,
    pub dependency: AssumptionDependency,
    pub meaning: String,
    pub finite_predicate: Option<FiniteExpr>,
    pub evidence_basis: Vec<String>,
    pub configurations: Vec<String>,
    pub unknown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationCondition {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedInvariant {
    pub id: String,
    pub requirement_id: String,
    pub commitment: StatementForm,
    pub interfaces: Vec<String>,
    pub configurations: Vec<String>,
    pub under_assumptions: Vec<String>,
    pub required_conditions: Vec<IntegrationCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidatedObligation {
    pub id: String,
    pub requirement_id: String,
    pub obligation: String,
    pub interfaces: Vec<String>,
    pub configurations: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBasis {
    pub id: String,
    pub requirement_id: String,
    pub obligations: Vec<String>,
    pub claims: Vec<String>,
    pub assumptions: Vec<String>,
    pub configurations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceChange {
    Add,
    Modify,
    Remove,
    Unknown,
}

impl InterfaceChange {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Modify => "modify",
            Self::Remove => "remove",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffectedInterface {
    pub id: String,
    pub configuration_blake3: String,
    pub change_kind: InterfaceChange,
    pub operation: String,
    pub order_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownKind {
    IncompleteFootprint,
    UnsupportedInterpretation,
    InformalBehavior,
    BoundedExploration,
    Other,
}

impl UnknownKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::IncompleteFootprint => "incomplete_footprint",
            Self::UnsupportedInterpretation => "unsupported_interpretation",
            Self::InformalBehavior => "informal_behavior",
            Self::BoundedExploration => "bounded_exploration",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownScope {
    pub scope: String,
    pub kind: UnknownKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintRevision {
    pub requirement: ContentBoundRequirementRow,
    pub contract: SemanticContract,
    pub interpretation_blake3: String,
    pub mapping_blake3: String,
    pub contract_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeFootprint {
    pub schema: String,
    pub change: ChangeIdentity,
    pub revisions: Vec<FootprintRevision>,
    pub assumptions: Vec<ConsumedAssumption>,
    pub guarantees: Vec<ChangedGuarantee>,
    pub invariants: Vec<PreservedInvariant>,
    pub invalidated: Vec<InvalidatedObligation>,
    pub interfaces: Vec<AffectedInterface>,
    pub evidence_basis: Vec<EvidenceBasis>,
    pub provided_conditions: Vec<String>,
    pub unknown: Vec<UnknownScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseContext {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
    pub configurations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionCandidate {
    pub id: String,
    pub participants: Vec<String>,
    pub order: Vec<String>,
    pub fixed: bool,
    pub admitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionRef {
    pub id: String,
    pub order: Vec<String>,
    pub fixed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceScope {
    Reviewed,
    BoundedCheck,
    CheckedProof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceScope {
    Witness,
    DeclaredComposition,
    AllAdmittedCompositions,
}

impl EvidenceScope {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Witness => "witness",
            Self::DeclaredComposition => "declared_composition",
            Self::AllAdmittedCompositions => "all_admitted_compositions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropositionKind {
    Coexistence,
    InvariantPreservation,
    Commutativity,
}

impl PropositionKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Coexistence => "coexistence",
            Self::InvariantPreservation => "invariant_preservation",
            Self::Commutativity => "commutativity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropositionState {
    Supported,
    NotSupported,
    NotEstablished,
    NotApplicable,
    Unknown,
}

impl PropositionState {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::NotSupported => "not_supported",
            Self::NotEstablished => "not_established",
            Self::NotApplicable => "not_applicable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropositionEvidence {
    pub proposition: PropositionKind,
    pub state: PropositionState,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointEvidence {
    pub id: String,
    pub obligation: String,
    pub combined_candidate_blake3: String,
    pub participants: Vec<String>,
    pub compositions: Vec<CompositionRef>,
    pub context: CaseContext,
    pub affected_claims: Vec<String>,
    pub propositions: Vec<PropositionEvidence>,
    pub assurance: AssuranceScope,
    pub scope: EvidenceScope,
    pub artifact_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndividualReceipt {
    pub id: String,
    pub change: String,
    pub claim: String,
    pub requirement_id: String,
    pub evidence_blake3: String,
    pub receipt_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationFingerprint {
    pub obligation_id: String,
    pub participants: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, String>,
    pub configurations: BTreeMap<String, String>,
    pub composition: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConcurrentChangeCase {
    pub schema: String,
    pub profile: SemanticProfile,
    pub context: CaseContext,
    pub changes: Vec<ChangeFootprint>,
    pub compositions: Vec<CompositionCandidate>,
    pub joint_evidence: Vec<JointEvidence>,
    pub individual_receipts: Vec<IndividualReceipt>,
    pub prior_obligations: Vec<ObligationFingerprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationOutcome {
    Conflict,
    NeedsJointEvidence,
    SupportedWithinDeclaredScope,
    Unknown,
}

impl ObligationOutcome {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::NeedsJointEvidence => "needs_joint_evidence",
            Self::SupportedWithinDeclaredScope => "supported_within_declared_scope",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    ChangedGuaranteeAffectsAssumption,
    InvalidationAffectsEvidenceBasis,
    InvalidationUnderminesInvariant,
    OverlappingInvariantCommitment,
    InterfaceConfigurationMismatch,
    InterfaceOperationConflict,
}

impl InteractionKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::ChangedGuaranteeAffectsAssumption => "changed_guarantee_affects_assumption",
            Self::InvalidationAffectsEvidenceBasis => "invalidation_affects_evidence_basis",
            Self::InvalidationUnderminesInvariant => "invalidation_undermines_invariant",
            Self::OverlappingInvariantCommitment => "overlapping_invariant_commitment",
            Self::InterfaceConfigurationMismatch => "interface_configuration_mismatch",
            Self::InterfaceOperationConflict => "interface_operation_conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteWitness {
    pub observables: BTreeMap<String, Vec<ObservableRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteCheck {
    pub requirement_id: String,
    pub worlds: u64,
    pub worlds_examined: u64,
    pub evaluations: u64,
    pub complete: bool,
    pub stopped_early: bool,
    pub previous_implies_assumption: Option<bool>,
    pub current_implies_assumption: Option<bool>,
    pub witness: Option<FiniteWitness>,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionAssessment {
    pub outcome: ObligationOutcome,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedInteraction {
    pub id: String,
    pub kind: InteractionKind,
    pub producer_change: String,
    pub consumer_change: String,
    pub guarantee: Option<String>,
    pub assumption: Option<String>,
    pub invalidation: Option<String>,
    pub evidence_basis: Option<String>,
    pub invariant: Option<String>,
    pub interfaces: Vec<String>,
    pub configurations: Vec<String>,
    pub affected_claims: Vec<String>,
    pub assessment: InteractionAssessment,
    pub finite: Option<FiniteCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclarationGap {
    pub kind: String,
    pub change: String,
    pub reference: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedUnknown {
    pub change: String,
    pub scope: String,
    pub kind: UnknownKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundRequirementSummary {
    pub requirement_id: String,
    pub requirement_row_blake3: String,
    pub interpretation_blake3: String,
    pub mapping_blake3: String,
    pub contract_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintSummary {
    pub change: String,
    pub source_revision: String,
    pub intent_revision: String,
    pub requirements: Vec<BoundRequirementSummary>,
    pub assumptions: usize,
    pub guarantees: usize,
    pub invariants: usize,
    pub invalidated: usize,
    pub interfaces: usize,
    pub unknown: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateParticipant {
    pub source_revision: String,
    pub intent_revision: String,
    pub requirements: BTreeMap<String, BoundRequirementSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombinedCandidate {
    pub participants: Vec<String>,
    pub binding: BTreeMap<String, CandidateParticipant>,
    pub compositions: Vec<CompositionRef>,
    pub context: CaseContext,
    pub combined_candidate_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationConditionStatus {
    pub id: String,
    pub statement: String,
    pub state: String,
    pub provided_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredEvidence {
    pub proposition: PropositionKind,
    pub requirement: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationPropositions {
    pub coexistence: PropositionState,
    pub invariant_preservation: PropositionState,
    pub commutativity: PropositionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRejection {
    pub id: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDisposition {
    pub admitted: Vec<String>,
    pub rejected: Vec<EvidenceRejection>,
    pub insufficient: Vec<EvidenceRejection>,
    pub individual_receipts_considered: Vec<String>,
    pub individual_receipts_do_not_discharge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplanationStep {
    pub step: usize,
    pub kind: String,
    pub change: Option<String>,
    pub reference: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityObligation {
    pub id: String,
    pub participants: Vec<String>,
    pub candidate: CombinedCandidate,
    pub interaction_ids: Vec<String>,
    pub context: CaseContext,
    pub affected_claims: Vec<String>,
    pub affected_interfaces: Vec<String>,
    pub configurations: BTreeMap<String, String>,
    pub affected_invariants: Vec<String>,
    pub integration_conditions: Vec<IntegrationConditionStatus>,
    pub required_evidence: Vec<RequiredEvidence>,
    pub propositions: ObligationPropositions,
    pub outcome: ObligationOutcome,
    pub outcome_basis: String,
    pub order_sensitive: bool,
    pub commutativity_required: bool,
    pub evidence: EvidenceDisposition,
    pub path: Vec<ExplanationStep>,
    pub fingerprint: ObligationFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndividualReceiptStatus {
    pub id: String,
    pub change: String,
    pub claim: String,
    pub change_known: bool,
    pub requirement_known: bool,
    pub discharges_combined_obligation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionSummary {
    pub id: String,
    pub participants: Vec<String>,
    pub order: Vec<String>,
    pub fixed: bool,
    pub admitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorApplicability {
    pub obligation_id: String,
    pub applicable: bool,
    pub reasons: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ExplorationUsage {
    pub worlds: u64,
    pub evaluations: u64,
    pub complete: bool,
    pub bound_exhausted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeCounts {
    pub conflict: usize,
    pub needs_joint_evidence: usize,
    pub supported_within_declared_scope: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityReport {
    pub schema: String,
    pub advisory: bool,
    pub producer: String,
    pub policy_blake3: String,
    pub input_blake3: String,
    pub report_blake3: String,
    pub context: CaseContext,
    pub footprints: Vec<FootprintSummary>,
    pub declaration_complete: bool,
    pub gaps: Vec<DeclarationGap>,
    pub interactions: Vec<DerivedInteraction>,
    pub obligations: Vec<CompatibilityObligation>,
    pub outcomes: OutcomeCounts,
    pub individual_receipts: Vec<IndividualReceiptStatus>,
    pub individual_receipts_do_not_discharge: bool,
    pub unmatched_joint_evidence: Vec<EvidenceRejection>,
    pub compositions: Vec<CompositionSummary>,
    pub prior_applicability: Vec<PriorApplicability>,
    pub exploration: ExplorationUsage,
    pub scoped_unknown: Vec<ScopedUnknown>,
    pub independent_reports: Vec<String>,
    pub boundary: String,
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BoundRequirement {
    requirement: ContentBoundRequirementRow,
    contract: SemanticContract,
    interpretation_blake3: String,
    mapping_blake3: String,
    contract_blake3: String,
}

impl BoundRequirement {
    fn summary(&self, requirement_id: &str) -> BoundRequirementSummary {
        BoundRequirementSummary {
            requirement_id: requirement_id.to_owned(),
            requirement_row_blake3: self.requirement.row_blake3.clone(),
            interpretation_blake3: self.interpretation_blake3.clone(),
            mapping_blake3: self.mapping_blake3.clone(),
            contract_blake3: self.contract_blake3.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct AdmittedGuarantee {
    guarantee: ChangedGuarantee,
    previous_identity: Option<String>,
    current_identity: Option<String>,
    previous_requirement: Option<String>,
    current_requirement: Option<String>,
    previous_finite: Option<FiniteExpr>,
    current_finite: Option<FiniteExpr>,
}

#[derive(Debug, Clone)]
struct AdmittedInvariant {
    invariant: PreservedInvariant,
    commitment_identity: String,
}

#[derive(Debug, Clone)]
struct AdmittedFootprint {
    change: ChangeIdentity,
    requirements: BTreeMap<String, BoundRequirement>,
    guarantees: BTreeMap<String, AdmittedGuarantee>,
    assumptions: Vec<ConsumedAssumption>,
    invariants: BTreeMap<String, AdmittedInvariant>,
    invalidated: Vec<InvalidatedObligation>,
    evidence_basis: Vec<EvidenceBasis>,
    interfaces: BTreeMap<String, AffectedInterface>,
    provided_conditions: BTreeSet<String>,
    unknown: Vec<UnknownScope>,
}

#[derive(Debug, Clone)]
struct ResolvedAssumption {
    consumer: String,
    producer: String,
    assumption: ConsumedAssumption,
    guarantee: AdmittedGuarantee,
    consumed_previous: bool,
}

#[derive(Debug, Clone)]
struct AdmittedCase {
    policy_blake3: String,
    input_blake3: String,
    context: CaseContext,
    footprints: BTreeMap<String, AdmittedFootprint>,
    compositions: Vec<CompositionCandidate>,
    joint_evidence: Vec<JointEvidence>,
    individual_receipts: Vec<IndividualReceipt>,
    prior_obligations: Vec<ObligationFingerprint>,
    resolved_assumptions: Vec<ResolvedAssumption>,
    gaps: Vec<DeclarationGap>,
}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, FootprintError> {
    let value = serde_json::to_value(value)
        .map_err(|error| FootprintError::Serialization(error.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn require_text(field: &str, value: &str) -> Result<(), FootprintError> {
    if value.trim().is_empty() {
        Err(FootprintError::InvalidInput(format!(
            "{field} must be non-empty"
        )))
    } else {
        Ok(())
    }
}

fn require_digest(field: &str, value: &str) -> Result<(), FootprintError> {
    if digest(value) {
        Ok(())
    } else {
        Err(FootprintError::InvalidInput(format!(
            "{field} must be a lowercase BLAKE3 digest"
        )))
    }
}

fn typed_error(error: super::typed_requirement_contracts::SemanticError) -> FootprintError {
    use super::typed_requirement_contracts::SemanticError;
    match error {
        SemanticError::RequirementRevisionMismatch => FootprintError::RevisionMismatch(
            "contract requirement row differs from the named native revision".into(),
        ),
        SemanticError::BoundExhaustion(detail) => FootprintError::BoundExhaustion(detail),
        other => FootprintError::InvalidInput(format!("{other:?}")),
    }
}

fn statement_identity(
    domain: &str,
    row_blake3: &str,
    declaration: &str,
    form: &StatementForm,
) -> Result<String, FootprintError> {
    match form {
        StatementForm::Finite { expression } => {
            identity(domain, &(row_blake3, declaration, "finite", expression))
        }
        StatementForm::Informal { statement } => {
            require_text("informal statement", statement)?;
            // Informal guarantees and invariants reuse the refinement statement
            // identities from #7 so a consumer can recompute them without a
            // second registry.
            let identity = if domain == GUARANTEE_IDENTITY_DOMAIN_V1 {
                super::refinement_typed_dependencies::guarantee_statement_identity(
                    row_blake3, statement,
                )
            } else {
                super::refinement_typed_dependencies::assumption_statement_identity(
                    declaration,
                    statement,
                )
            };
            identity.map_err(|error| {
                FootprintError::InvalidInput(format!("refinement statement identity: {error:?}"))
            })
        }
    }
}

/// Exact identity a consumer must name when it consumes a guarantee
/// declaration. A finite declaration binds the native requirement row, the
/// declaration id and the expression. An informal declaration reuses the
/// refinement guarantee statement identity over the requirement row and
/// proposition.
// r[impl cairn.concurrent_footprints.footprint]
pub fn footprint_guarantee_identity(
    requirement_row_blake3: &str,
    guarantee_id: &str,
    form: &StatementForm,
) -> Result<String, FootprintError> {
    require_digest("guarantee requirement row", requirement_row_blake3)?;
    require_text("guarantee id", guarantee_id)?;
    statement_identity(
        GUARANTEE_IDENTITY_DOMAIN_V1,
        requirement_row_blake3,
        guarantee_id,
        form,
    )
}

/// Exact identity of a declared invariant commitment.
// r[impl cairn.concurrent_footprints.footprint]
pub fn invariant_statement_identity(
    requirement_row_blake3: &str,
    invariant_id: &str,
    form: &StatementForm,
) -> Result<String, FootprintError> {
    require_digest("invariant requirement row", requirement_row_blake3)?;
    require_text("invariant id", invariant_id)?;
    statement_identity(
        INVARIANT_IDENTITY_DOMAIN_V1,
        requirement_row_blake3,
        invariant_id,
        form,
    )
}

fn validate_context(context: &CaseContext) -> Result<(), FootprintError> {
    require_digest("context subject", &context.subject_blake3)?;
    require_digest("context policy", &context.policy_blake3)?;
    require_text("context target", &context.target)?;
    require_text("context environment", &context.environment)?;
    for (interface, configuration) in &context.configurations {
        require_text("context configuration interface", interface)?;
        require_digest("context configuration identity", configuration)?;
    }
    Ok(())
}

fn admit_revision(
    profile: &SemanticProfile,
    revision: &FootprintRevision,
) -> Result<BoundRequirement, FootprintError> {
    let row = &revision.requirement;
    if row.requirement_id.trim().is_empty()
        || row.specification_path.trim().is_empty()
        || !digest(&row.specification_blake3)
        || !digest(&row.requirement_body_blake3)
        || !digest(&row.row_blake3)
        || row.source_span.start_byte > row.source_span.end_byte
        || row.source_span.start_line == 0
        || row.source_span.end_line < row.source_span.start_line
    {
        return Err(FootprintError::InvalidInput(
            "each footprint revision requires an exact native requirement row identity".into(),
        ));
    }
    let normalized = normalize_contract(profile, row, &revision.contract).map_err(typed_error)?;
    if normalized.interpretation_blake3 != revision.interpretation_blake3
        || normalized.mapping_blake3 != revision.mapping_blake3
        || normalized.contract_blake3 != revision.contract_blake3
    {
        return Err(FootprintError::RevisionMismatch(format!(
            "footprint revision {} does not match its declared interpretation, mapping or contract identity",
            row.requirement_id
        )));
    }
    Ok(BoundRequirement {
        requirement: row.clone(),
        contract: revision.contract.clone(),
        interpretation_blake3: normalized.interpretation_blake3,
        mapping_blake3: normalized.mapping_blake3,
        contract_blake3: normalized.contract_blake3,
    })
}

fn admit_guarantee(
    footprint: &AdmittedFootprint,
    guarantee: ChangedGuarantee,
) -> Result<AdmittedGuarantee, FootprintError> {
    require_text("guarantee id", &guarantee.id)?;
    let mut previous_identity = None;
    let mut current_identity = None;
    let mut previous_requirement = None;
    let mut current_requirement = None;
    let mut previous_finite = None;
    let mut current_finite = None;
    if let Some(statement) = &guarantee.previous {
        let bound = footprint
            .requirements
            .get(&statement.requirement_id)
            .ok_or_else(|| {
                FootprintError::InvalidInput(format!(
                    "guarantee {} names an unknown requirement {}",
                    guarantee.id, statement.requirement_id
                ))
            })?;
        previous_identity = Some(statement_identity(
            GUARANTEE_IDENTITY_DOMAIN_V1,
            &bound.requirement.row_blake3,
            &guarantee.id,
            &statement.form,
        )?);
        previous_requirement = Some(statement.requirement_id.clone());
        if let StatementForm::Finite { expression } = &statement.form {
            previous_finite = Some(expression.clone());
        }
    }
    if let Some(statement) = &guarantee.current {
        let bound = footprint
            .requirements
            .get(&statement.requirement_id)
            .ok_or_else(|| {
                FootprintError::InvalidInput(format!(
                    "guarantee {} names an unknown requirement {}",
                    guarantee.id, statement.requirement_id
                ))
            })?;
        current_identity = Some(statement_identity(
            GUARANTEE_IDENTITY_DOMAIN_V1,
            &bound.requirement.row_blake3,
            &guarantee.id,
            &statement.form,
        )?);
        current_requirement = Some(statement.requirement_id.clone());
        if let StatementForm::Finite { expression } = &statement.form {
            current_finite = Some(expression.clone());
        }
    }
    match guarantee.change {
        GuaranteeChange::Removed => {
            if previous_identity.is_none() || current_identity.is_some() {
                return Err(FootprintError::InvalidInput(format!(
                    "removed guarantee {} must name a previous statement and no current statement",
                    guarantee.id
                )));
            }
        }
        GuaranteeChange::Preserved => {
            if previous_identity.is_none() || previous_identity != current_identity {
                return Err(FootprintError::InvalidInput(format!(
                    "preserved guarantee {} must retain one exact statement identity",
                    guarantee.id
                )));
            }
        }
        GuaranteeChange::Strengthened
        | GuaranteeChange::Weakened
        | GuaranteeChange::Replaced
        | GuaranteeChange::Unknown => {
            if previous_identity.is_none() || current_identity.is_none() {
                return Err(FootprintError::InvalidInput(format!(
                    "guarantee {} change {} requires previous and current statements",
                    guarantee.id,
                    guarantee.change.tag()
                )));
            }
            if guarantee.change != GuaranteeChange::Unknown && previous_identity == current_identity
            {
                return Err(FootprintError::InvalidInput(format!(
                    "guarantee {} declares {} but keeps one statement identity",
                    guarantee.id,
                    guarantee.change.tag()
                )));
            }
        }
    }
    for claim in &guarantee.affected_claims {
        require_text("guarantee affected claim", claim)?;
    }
    for interface in &guarantee.interfaces {
        require_text("guarantee interface", interface)?;
    }
    for configuration in &guarantee.configurations {
        require_digest("guarantee configuration identity", configuration)?;
    }
    Ok(AdmittedGuarantee {
        guarantee,
        previous_identity,
        current_identity,
        previous_requirement,
        current_requirement,
        previous_finite,
        current_finite,
    })
}

fn admit_invariant(
    footprint: &AdmittedFootprint,
    invariant: PreservedInvariant,
) -> Result<AdmittedInvariant, FootprintError> {
    require_text("invariant id", &invariant.id)?;
    let bound = footprint
        .requirements
        .get(&invariant.requirement_id)
        .ok_or_else(|| {
            FootprintError::InvalidInput(format!(
                "invariant {} names an unknown requirement {}",
                invariant.id, invariant.requirement_id
            ))
        })?;
    let commitment_identity = statement_identity(
        INVARIANT_IDENTITY_DOMAIN_V1,
        &bound.requirement.row_blake3,
        &invariant.id,
        &invariant.commitment,
    )?;
    for assumption in &invariant.under_assumptions {
        require_text("invariant assumption reference", assumption)?;
    }
    for condition in &invariant.required_conditions {
        require_text("invariant condition id", &condition.id)?;
        require_text("invariant condition statement", &condition.statement)?;
    }
    for interface in &invariant.interfaces {
        require_text("invariant interface", interface)?;
    }
    for configuration in &invariant.configurations {
        require_digest("invariant configuration identity", configuration)?;
    }
    Ok(AdmittedInvariant {
        invariant,
        commitment_identity,
    })
}

fn admit_footprint(
    policy: &FootprintPolicy,
    profile: &SemanticProfile,
    footprint: &ChangeFootprint,
) -> Result<AdmittedFootprint, FootprintError> {
    if footprint.schema != FOOTPRINT_SCHEMA_V1 {
        return Err(FootprintError::Unsupported(format!(
            "unsupported change footprint schema {}",
            footprint.schema
        )));
    }
    require_text("change id", &footprint.change.change_id)?;
    require_text("change source revision", &footprint.change.source_revision)?;
    require_text("change intent revision", &footprint.change.intent_revision)?;
    let bounds = &policy.bounds;
    if footprint.revisions.len() > bounds.max_revisions
        || footprint.assumptions.len() > bounds.max_assumptions
        || footprint.guarantees.len() > bounds.max_guarantees
        || footprint.invariants.len() > bounds.max_invariants
        || footprint.invalidated.len() > bounds.max_invalidations
        || footprint.interfaces.len() > bounds.max_interfaces
        || footprint.unknown.len() > bounds.max_unknown
    {
        return Err(FootprintError::BoundExhaustion(format!(
            "change footprint {} exceeds an admitted declaration bound",
            footprint.change.change_id
        )));
    }
    let mut requirements = BTreeMap::new();
    for revision in &footprint.revisions {
        let bound = admit_revision(profile, revision)?;
        let requirement_id = revision.requirement.requirement_id.clone();
        if requirements.insert(requirement_id.clone(), bound).is_some() {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares requirement {requirement_id} twice",
                footprint.change.change_id
            )));
        }
    }
    let mut admitted = AdmittedFootprint {
        change: footprint.change.clone(),
        requirements,
        guarantees: BTreeMap::new(),
        assumptions: Vec::new(),
        invariants: BTreeMap::new(),
        invalidated: Vec::new(),
        evidence_basis: Vec::new(),
        interfaces: BTreeMap::new(),
        provided_conditions: BTreeSet::new(),
        unknown: footprint.unknown.clone(),
    };
    for guarantee in &footprint.guarantees {
        let admitted_guarantee = admit_guarantee(&admitted, guarantee.clone())?;
        if admitted
            .guarantees
            .insert(guarantee.id.clone(), admitted_guarantee)
            .is_some()
        {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares guarantee {} twice",
                footprint.change.change_id, guarantee.id
            )));
        }
    }
    let mut assumption_ids = BTreeSet::new();
    for assumption in &footprint.assumptions {
        require_text("assumption id", &assumption.id)?;
        require_text("assumption meaning", &assumption.meaning)?;
        require_digest(
            "assumption consumed identity",
            &assumption.consumed_identity,
        )?;
        if !assumption_ids.insert(assumption.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares assumption {} twice",
                footprint.change.change_id, assumption.id
            )));
        }
        if !admitted
            .requirements
            .contains_key(&assumption.requirement_id)
        {
            return Err(FootprintError::InvalidInput(format!(
                "assumption {} names an unknown requirement {}",
                assumption.id, assumption.requirement_id
            )));
        }
        for basis in &assumption.evidence_basis {
            require_text("assumption evidence basis reference", basis)?;
        }
        for configuration in &assumption.configurations {
            require_digest("assumption configuration identity", configuration)?;
        }
        admitted.assumptions.push(assumption.clone());
    }
    for invariant in &footprint.invariants {
        let admitted_invariant = admit_invariant(&admitted, invariant.clone())?;
        if admitted
            .invariants
            .insert(invariant.id.clone(), admitted_invariant)
            .is_some()
        {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares invariant {} twice",
                footprint.change.change_id, invariant.id
            )));
        }
    }
    for invariant in admitted.invariants.values() {
        for assumption in &invariant.invariant.under_assumptions {
            if !assumption_ids.contains(assumption) {
                return Err(FootprintError::InvalidInput(format!(
                    "invariant {} names an unknown local assumption {}",
                    invariant.invariant.id, assumption
                )));
            }
        }
    }
    let mut invalidation_ids = BTreeSet::new();
    for invalidation in &footprint.invalidated {
        require_text("invalidation id", &invalidation.id)?;
        require_text("invalidated obligation", &invalidation.obligation)?;
        require_text("invalidation reason", &invalidation.reason)?;
        if !admitted
            .requirements
            .contains_key(&invalidation.requirement_id)
        {
            return Err(FootprintError::InvalidInput(format!(
                "invalidation {} names an unknown requirement {}",
                invalidation.id, invalidation.requirement_id
            )));
        }
        if !invalidation_ids.insert(invalidation.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares invalidation {} twice",
                footprint.change.change_id, invalidation.id
            )));
        }
        for configuration in &invalidation.configurations {
            require_digest("invalidation configuration identity", configuration)?;
        }
        admitted.invalidated.push(invalidation.clone());
    }
    for basis in &footprint.evidence_basis {
        require_text("evidence basis id", &basis.id)?;
        if !admitted.requirements.contains_key(&basis.requirement_id) {
            return Err(FootprintError::InvalidInput(format!(
                "evidence basis {} names an unknown requirement {}",
                basis.id, basis.requirement_id
            )));
        }
        for obligation in &basis.obligations {
            require_text("evidence basis obligation", obligation)?;
        }
        for claim in &basis.claims {
            require_text("evidence basis claim", claim)?;
        }
        for configuration in &basis.configurations {
            require_digest("evidence basis configuration identity", configuration)?;
        }
        if admitted
            .evidence_basis
            .iter()
            .any(|existing| existing.id == basis.id)
        {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares evidence basis {} twice",
                footprint.change.change_id, basis.id
            )));
        }
        admitted.evidence_basis.push(basis.clone());
    }
    for interface in &footprint.interfaces {
        require_text("interface id", &interface.id)?;
        require_digest(
            "interface configuration identity",
            &interface.configuration_blake3,
        )?;
        require_text("interface operation", &interface.operation)?;
        if admitted
            .interfaces
            .insert(interface.id.clone(), interface.clone())
            .is_some()
        {
            return Err(FootprintError::InvalidInput(format!(
                "change footprint {} declares interface {} twice",
                footprint.change.change_id, interface.id
            )));
        }
    }
    for condition in &footprint.provided_conditions {
        require_text("provided integration condition", condition)?;
        admitted.provided_conditions.insert(condition.clone());
    }
    for unknown in &footprint.unknown {
        require_text("unknown scope", &unknown.scope)?;
        require_text("unknown detail", &unknown.detail)?;
    }
    Ok(admitted)
}

fn admit_case(
    policy: &FootprintPolicy,
    case: &ConcurrentChangeCase,
) -> Result<AdmittedCase, FootprintError> {
    if case.schema != CASE_SCHEMA_V1 {
        return Err(FootprintError::Unsupported(format!(
            "unsupported concurrent change case schema {}",
            case.schema
        )));
    }
    validate_context(&case.context)?;
    if case.changes.len() > policy.bounds.max_changes {
        return Err(FootprintError::BoundExhaustion(format!(
            "change count {} exceeds the admitted bound {}",
            case.changes.len(),
            policy.bounds.max_changes
        )));
    }
    if case.compositions.len() > policy.bounds.max_compositions
        || case.joint_evidence.len() > policy.bounds.max_evidence
        || case.individual_receipts.len() > policy.bounds.max_evidence
        || case.prior_obligations.len() > policy.bounds.max_evidence
    {
        return Err(FootprintError::BoundExhaustion(
            "case composition, evidence or prior-obligation count exceeds an admitted bound".into(),
        ));
    }
    let mut footprints = BTreeMap::new();
    for footprint in &case.changes {
        let admitted = admit_footprint(policy, &case.profile, footprint)?;
        let change_id = admitted.change.change_id.clone();
        if footprints.insert(change_id.clone(), admitted).is_some() {
            return Err(FootprintError::InvalidInput(format!(
                "concurrent change case declares change {change_id} twice"
            )));
        }
    }
    for (change_id, footprint) in &footprints {
        for (interface_id, interface) in &footprint.interfaces {
            if let Some(declared) = case.context.configurations.get(interface_id) {
                if declared != &interface.configuration_blake3 {
                    return Err(FootprintError::InvalidInput(format!(
                        "change {change_id} interface {interface_id} configuration identity differs from the case context"
                    )));
                }
            }
        }
    }
    let mut composition_ids = BTreeSet::new();
    for composition in &case.compositions {
        require_text("composition id", &composition.id)?;
        if !composition_ids.insert(composition.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "concurrent change case declares composition {} twice",
                composition.id
            )));
        }
        if composition.participants.is_empty()
            || composition.participants.len() != composition.order.len()
        {
            return Err(FootprintError::InvalidInput(format!(
                "composition {} must name participants and an order of equal non-zero length",
                composition.id
            )));
        }
        let participants: BTreeSet<&String> = composition.participants.iter().collect();
        let order: BTreeSet<&String> = composition.order.iter().collect();
        if participants.len() != composition.participants.len()
            || order.len() != composition.order.len()
            || participants != order
        {
            return Err(FootprintError::InvalidInput(format!(
                "composition {} order is not a permutation of its participants",
                composition.id
            )));
        }
        for participant in &composition.participants {
            if !footprints.contains_key(participant) {
                return Err(FootprintError::InvalidInput(format!(
                    "composition {} names unknown change {}",
                    composition.id, participant
                )));
            }
        }
    }
    let mut evidence_ids = BTreeSet::new();
    for evidence in &case.joint_evidence {
        require_text("joint evidence id", &evidence.id)?;
        require_text("joint evidence obligation", &evidence.obligation)?;
        require_digest(
            "joint evidence candidate identity",
            &evidence.combined_candidate_blake3,
        )?;
        require_digest("joint evidence artifact", &evidence.artifact_blake3)?;
        if !evidence_ids.insert(evidence.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "concurrent change case declares joint evidence {} twice",
                evidence.id
            )));
        }
        validate_context(&evidence.context)?;
        for participant in &evidence.participants {
            if !footprints.contains_key(participant) {
                return Err(FootprintError::InvalidInput(format!(
                    "joint evidence {} names unknown change {}",
                    evidence.id, participant
                )));
            }
        }
        for composition in &evidence.compositions {
            require_text("joint evidence composition id", &composition.id)?;
            if composition.order.is_empty() {
                return Err(FootprintError::InvalidInput(format!(
                    "joint evidence {} names an empty composition order",
                    evidence.id
                )));
            }
        }
    }
    let mut receipt_ids = BTreeSet::new();
    for receipt in &case.individual_receipts {
        require_text("individual receipt id", &receipt.id)?;
        require_text("individual receipt change", &receipt.change)?;
        require_text("individual receipt claim", &receipt.claim)?;
        require_digest("individual receipt evidence", &receipt.evidence_blake3)?;
        require_digest("individual receipt identity", &receipt.receipt_blake3)?;
        if !receipt_ids.insert(receipt.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "concurrent change case declares individual receipt {} twice",
                receipt.id
            )));
        }
    }
    let mut prior_ids = BTreeSet::new();
    for prior in &case.prior_obligations {
        require_text("prior obligation id", &prior.obligation_id)?;
        if !prior_ids.insert(prior.obligation_id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "concurrent change case declares prior obligation {} twice",
                prior.obligation_id
            )));
        }
        for revision in prior.participants.values() {
            require_text("prior participant revision", revision)?;
        }
        for identity in prior.dependencies.values() {
            require_text("prior dependency identity", identity)?;
        }
        for identity in prior.configurations.values() {
            require_digest("prior configuration identity", identity)?;
        }
        for composition in &prior.composition {
            require_text("prior composition reference", composition)?;
        }
    }

    let mut gaps = Vec::new();
    let mut resolved_assumptions = Vec::new();
    let mut change_ids: Vec<String> = footprints.keys().cloned().collect();
    change_ids.sort();
    for consumer in &change_ids {
        let footprint = &footprints[consumer];
        for assumption in &footprint.assumptions {
            if assumption.consumed_change == *consumer {
                return Err(FootprintError::InvalidInput(format!(
                    "assumption {} must consume another change, not {consumer}",
                    assumption.id
                )));
            }
            let Some(producer_footprint) = footprints.get(&assumption.consumed_change) else {
                gaps.push(DeclarationGap {
                    kind: "unresolved_change".into(),
                    change: consumer.clone(),
                    reference: assumption.consumed_change.clone(),
                    detail: format!(
                        "assumption {} consumes a change that is not declared in this case",
                        assumption.id
                    ),
                });
                continue;
            };
            let Some(guarantee) = producer_footprint
                .guarantees
                .get(&assumption.consumed_guarantee)
            else {
                gaps.push(DeclarationGap {
                    kind: "unresolved_guarantee".into(),
                    change: consumer.clone(),
                    reference: format!(
                        "{}.{}",
                        assumption.consumed_change, assumption.consumed_guarantee
                    ),
                    detail: format!(
                        "assumption {} consumes a guarantee that change {} does not declare",
                        assumption.id, assumption.consumed_change
                    ),
                });
                continue;
            };
            let consumed_previous = if guarantee.current_identity.as_deref()
                == Some(assumption.consumed_identity.as_str())
            {
                false
            } else if guarantee.previous_identity.as_deref()
                == Some(assumption.consumed_identity.as_str())
            {
                true
            } else {
                gaps.push(DeclarationGap {
                    kind: "unresolved_consumed_identity".into(),
                    change: consumer.clone(),
                    reference: assumption.consumed_identity.clone(),
                    detail: format!(
                        "assumption {} names an identity that guarantee {} never declared",
                        assumption.id, assumption.consumed_guarantee
                    ),
                });
                continue;
            };
            resolved_assumptions.push(ResolvedAssumption {
                consumer: consumer.clone(),
                producer: assumption.consumed_change.clone(),
                assumption: assumption.clone(),
                guarantee: guarantee.clone(),
                consumed_previous,
            });
        }
    }
    let policy_blake3 = identity(POLICY_SCHEMA_V1, policy)?;
    let input_blake3 = identity(INPUT_IDENTITY_DOMAIN_V1, case)?;
    Ok(AdmittedCase {
        policy_blake3,
        input_blake3,
        context: case.context.clone(),
        footprints,
        compositions: case.compositions.clone(),
        joint_evidence: case.joint_evidence.clone(),
        individual_receipts: case.individual_receipts.clone(),
        prior_obligations: case.prior_obligations.clone(),
        resolved_assumptions,
        gaps,
    })
}

// ---------------------------------------------------------------------------
// Finite universe evaluation
// ---------------------------------------------------------------------------

type Tables = BTreeMap<String, BTreeMap<Vec<FiniteValue>, FiniteValue>>;

fn cartesian(
    arguments: &[(String, Vec<String>)],
    max_worlds: u64,
) -> Result<Vec<Vec<FiniteValue>>, String> {
    let mut out: Vec<Vec<FiniteValue>> = vec![Vec::new()];
    for (domain, values) in arguments {
        let next_len = (out.len() as u64)
            .checked_mul(values.len() as u64)
            .ok_or_else(|| "bound: observable argument tuple count overflow".to_string())?;
        if next_len > max_worlds {
            return Err("bound: observable argument tuple count".into());
        }
        let mut next = Vec::with_capacity(next_len as usize);
        for prefix in &out {
            for value in values {
                let mut tuple = prefix.clone();
                tuple.push(FiniteValue {
                    domain: domain.clone(),
                    value: value.clone(),
                });
                next.push(tuple);
            }
        }
        out = next;
    }
    Ok(out)
}

struct Universe {
    names: Vec<String>,
    tuples: BTreeMap<String, Vec<Vec<FiniteValue>>>,
    results: BTreeMap<String, Vec<String>>,
    result_domains: BTreeMap<String, String>,
    worlds: u64,
}

fn build_universe(
    domains: &BTreeMap<String, FiniteDomain>,
    observables: &BTreeMap<String, ObservableMapping>,
    max_domain_values: usize,
    max_worlds: u64,
) -> Result<Universe, String> {
    let mut names: Vec<String> = observables.keys().cloned().collect();
    names.sort();
    let mut tuples = BTreeMap::new();
    let mut results = BTreeMap::new();
    let mut result_domains = BTreeMap::new();
    let mut worlds: u64 = 1;
    for name in &names {
        let mapping = &observables[name];
        let mut arguments = Vec::new();
        for argument in &mapping.arguments {
            let domain = domains
                .get(argument)
                .ok_or_else(|| format!("unsupported: unknown argument domain {argument}"))?;
            if domain.values.is_empty() || domain.values.len() > max_domain_values {
                return Err("bound: observable argument domain".into());
            }
            let mut values = domain.values.clone();
            values.sort();
            values.dedup();
            arguments.push((argument.clone(), values));
        }
        let result_domain = domains
            .get(&mapping.result)
            .ok_or_else(|| format!("unsupported: unknown result domain {}", mapping.result))?;
        if result_domain.values.is_empty() || result_domain.values.len() > max_domain_values {
            return Err("bound: observable result domain".into());
        }
        let mut result_values = result_domain.values.clone();
        result_values.sort();
        result_values.dedup();
        let tuple_list = cartesian(&arguments, max_worlds)?;
        let tuple_count = tuple_list.len() as u64;
        let result_count = result_values.len() as u64;
        let table_worlds = if tuple_count == 0 || result_count == 1 {
            1
        } else {
            let mut product = 1u64;
            let mut exponent = tuple_count;
            while exponent > 0 {
                product = product
                    .checked_mul(result_count)
                    .ok_or_else(|| "bound: observable world count overflow".to_string())?;
                if product > max_worlds {
                    return Err("bound: observable world count".into());
                }
                exponent -= 1;
            }
            product
        };
        worlds = worlds
            .checked_mul(table_worlds)
            .ok_or_else(|| "bound: world count overflow".to_string())?;
        if worlds > max_worlds {
            return Err("bound: world count".into());
        }
        tuples.insert(name.clone(), tuple_list);
        results.insert(name.clone(), result_values);
        result_domains.insert(name.clone(), mapping.result.clone());
    }
    Ok(Universe {
        names,
        tuples,
        results,
        result_domains,
        worlds,
    })
}

fn type_check_term(
    term: &FiniteTerm,
    domains: &BTreeMap<String, FiniteDomain>,
    observables: &BTreeMap<String, ObservableMapping>,
    scope: &BTreeMap<String, String>,
    profile: &SemanticProfile,
    max_depth: usize,
    depth: usize,
    nodes: &mut usize,
) -> Result<String, String> {
    *nodes += 1;
    if *nodes > profile.max_nodes {
        return Err("bound: finite term node count".into());
    }
    if depth > max_depth {
        return Err("bound: finite term depth".into());
    }
    match term {
        FiniteTerm::Literal { value } => {
            let domain = domains
                .get(&value.domain)
                .ok_or_else(|| format!("unsupported: unknown literal domain {}", value.domain))?;
            if !domain.values.contains(&value.value) {
                return Err(format!(
                    "ill_typed: literal {} is absent from domain {}",
                    value.value, value.domain
                ));
            }
            Ok(value.domain.clone())
        }
        FiniteTerm::Variable { name } => scope
            .get(name)
            .cloned()
            .ok_or_else(|| format!("ill_typed: unbound variable {name}")),
        FiniteTerm::Observable { mapping, arguments } => {
            let declared = observables
                .get(mapping)
                .ok_or_else(|| format!("unsupported: unknown observable {mapping}"))?;
            if declared.arguments.len() != arguments.len() {
                return Err(format!("ill_typed: observable {mapping} arity mismatch"));
            }
            for (position, argument) in arguments.iter().enumerate() {
                let argument_domain = type_check_term(
                    argument,
                    domains,
                    observables,
                    scope,
                    profile,
                    max_depth,
                    depth + 1,
                    nodes,
                )?;
                if argument_domain != declared.arguments[position] {
                    return Err(format!(
                        "ill_typed: observable {mapping} argument {position} domain mismatch"
                    ));
                }
            }
            if !domains.contains_key(&declared.result) {
                return Err(format!(
                    "unsupported: observable {mapping} result domain {}",
                    declared.result
                ));
            }
            Ok(declared.result.clone())
        }
        FiniteTerm::Unsupported { construct } => {
            Err(format!("unsupported: term construct {construct}"))
        }
    }
}

fn type_check_expr(
    expression: &FiniteExpr,
    domains: &BTreeMap<String, FiniteDomain>,
    observables: &BTreeMap<String, ObservableMapping>,
    scope: &mut BTreeMap<String, String>,
    profile: &SemanticProfile,
    max_depth: usize,
    depth: usize,
    nodes: &mut usize,
) -> Result<String, String> {
    *nodes += 1;
    if *nodes > profile.max_nodes {
        return Err("bound: finite expression node count".into());
    }
    if depth > max_depth {
        return Err("bound: finite expression depth".into());
    }
    match expression {
        FiniteExpr::Bool { .. } => Ok("bool".into()),
        FiniteExpr::Equal { left, right } => {
            let left_domain = type_check_term(
                left,
                domains,
                observables,
                scope,
                profile,
                max_depth,
                depth + 1,
                nodes,
            )?;
            let right_domain = type_check_term(
                right,
                domains,
                observables,
                scope,
                profile,
                max_depth,
                depth + 1,
                nodes,
            )?;
            if left_domain != right_domain {
                return Err(format!(
                    "ill_typed: equality compares {left_domain} with {right_domain}"
                ));
            }
            Ok("bool".into())
        }
        FiniteExpr::Member { term, values } => {
            let term_domain = type_check_term(
                term,
                domains,
                observables,
                scope,
                profile,
                max_depth,
                depth + 1,
                nodes,
            )?;
            for value in values {
                if value.domain != term_domain {
                    return Err(format!(
                        "ill_typed: membership value {} uses domain {} not {term_domain}",
                        value.value, value.domain
                    ));
                }
                let domain = domains
                    .get(&value.domain)
                    .ok_or_else(|| format!("unsupported: membership domain {}", value.domain))?;
                if !domain.values.contains(&value.value) {
                    return Err(format!(
                        "ill_typed: membership value {} is absent from domain {}",
                        value.value, value.domain
                    ));
                }
            }
            Ok("bool".into())
        }
        FiniteExpr::Not { expression } => {
            let domain = type_check_expr(
                expression,
                domains,
                observables,
                scope,
                profile,
                max_depth,
                depth + 1,
                nodes,
            )?;
            if domain != "bool" {
                return Err("ill_typed: negation of a non-boolean expression".into());
            }
            Ok("bool".into())
        }
        FiniteExpr::And { expressions } | FiniteExpr::Or { expressions } => {
            for expression in expressions {
                let domain = type_check_expr(
                    expression,
                    domains,
                    observables,
                    scope,
                    profile,
                    max_depth,
                    depth + 1,
                    nodes,
                )?;
                if domain != "bool" {
                    return Err(
                        "ill_typed: conjunction or disjunction of a non-boolean expression".into(),
                    );
                }
            }
            Ok("bool".into())
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            for expression in [premise, conclusion] {
                let domain = type_check_expr(
                    expression,
                    domains,
                    observables,
                    scope,
                    profile,
                    max_depth,
                    depth + 1,
                    nodes,
                )?;
                if domain != "bool" {
                    return Err("ill_typed: implication of a non-boolean expression".into());
                }
            }
            Ok("bool".into())
        }
        FiniteExpr::Forall {
            variable,
            domain,
            expression,
        }
        | FiniteExpr::Exists {
            variable,
            domain,
            expression,
        } => {
            if !domains.contains_key(domain) {
                return Err(format!("unsupported: unknown quantifier domain {domain}"));
            }
            let previous = scope.insert(variable.clone(), domain.clone());
            let result = type_check_expr(
                expression,
                domains,
                observables,
                scope,
                profile,
                max_depth,
                depth + 1,
                nodes,
            );
            match previous {
                Some(value) => {
                    scope.insert(variable.clone(), value);
                }
                None => {
                    scope.remove(variable);
                }
            }
            let child = result?;
            if child != "bool" {
                return Err("ill_typed: quantified body is not boolean".into());
            }
            Ok("bool".into())
        }
        FiniteExpr::Unsupported { construct } => {
            Err(format!("unsupported: expression construct {construct}"))
        }
    }
}

fn type_check_finite(
    expression: &FiniteExpr,
    domains: &BTreeMap<String, FiniteDomain>,
    observables: &BTreeMap<String, ObservableMapping>,
    profile: &SemanticProfile,
    max_depth: usize,
) -> Result<(), String> {
    let mut scope = BTreeMap::new();
    let mut nodes = 0usize;
    let domain = type_check_expr(
        expression,
        domains,
        observables,
        &mut scope,
        profile,
        max_depth,
        0,
        &mut nodes,
    )?;
    if domain != "bool" {
        return Err("ill_typed: a finite obligation must be boolean".into());
    }
    Ok(())
}

fn eval_term(
    term: &FiniteTerm,
    tables: &Tables,
    scope: &BTreeMap<String, FiniteValue>,
) -> Result<FiniteValue, String> {
    match term {
        FiniteTerm::Literal { value } => Ok(value.clone()),
        FiniteTerm::Variable { name } => scope
            .get(name)
            .cloned()
            .ok_or_else(|| format!("ill_typed: unbound variable {name}")),
        FiniteTerm::Observable { mapping, arguments } => {
            let mut values = Vec::with_capacity(arguments.len());
            for argument in arguments {
                values.push(eval_term(argument, tables, scope)?);
            }
            let table = tables
                .get(mapping)
                .ok_or_else(|| format!("missing_observation: {mapping}"))?;
            table
                .get(&values)
                .cloned()
                .ok_or_else(|| format!("missing_observation: {mapping} argument tuple"))
        }
        FiniteTerm::Unsupported { construct } => {
            Err(format!("unsupported: term construct {construct}"))
        }
    }
}

fn eval_expr(
    expression: &FiniteExpr,
    tables: &Tables,
    domains: &BTreeMap<String, FiniteDomain>,
    scope: &mut BTreeMap<String, FiniteValue>,
) -> Result<bool, String> {
    match expression {
        FiniteExpr::Bool { value } => Ok(*value),
        FiniteExpr::Equal { left, right } => {
            let left = eval_term(left, tables, scope)?;
            let right = eval_term(right, tables, scope)?;
            Ok(left == right)
        }
        FiniteExpr::Member { term, values } => {
            let value = eval_term(term, tables, scope)?;
            Ok(values.contains(&value))
        }
        FiniteExpr::Not { expression } => Ok(!eval_expr(expression, tables, domains, scope)?),
        FiniteExpr::And { expressions } => {
            for expression in expressions {
                if !eval_expr(expression, tables, domains, scope)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FiniteExpr::Or { expressions } => {
            for expression in expressions {
                if eval_expr(expression, tables, domains, scope)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            if !eval_expr(premise, tables, domains, scope)? {
                return Ok(true);
            }
            eval_expr(conclusion, tables, domains, scope)
        }
        FiniteExpr::Forall {
            variable,
            domain,
            expression,
        } => {
            let values = domains
                .get(domain)
                .ok_or_else(|| format!("unsupported: unknown quantifier domain {domain}"))?
                .values
                .clone();
            let mut result = true;
            for value in values {
                let previous = scope.insert(
                    variable.clone(),
                    FiniteValue {
                        domain: domain.clone(),
                        value,
                    },
                );
                let child = eval_expr(expression, tables, domains, scope);
                match previous {
                    Some(value) => {
                        scope.insert(variable.clone(), value);
                    }
                    None => {
                        scope.remove(variable);
                    }
                }
                if !child? {
                    result = false;
                    break;
                }
            }
            Ok(result)
        }
        FiniteExpr::Exists {
            variable,
            domain,
            expression,
        } => {
            let values = domains
                .get(domain)
                .ok_or_else(|| format!("unsupported: unknown quantifier domain {domain}"))?
                .values
                .clone();
            let mut result = false;
            for value in values {
                let previous = scope.insert(
                    variable.clone(),
                    FiniteValue {
                        domain: domain.clone(),
                        value,
                    },
                );
                let child = eval_expr(expression, tables, domains, scope);
                match previous {
                    Some(value) => {
                        scope.insert(variable.clone(), value);
                    }
                    None => {
                        scope.remove(variable);
                    }
                }
                if child? {
                    result = true;
                    break;
                }
            }
            Ok(result)
        }
        FiniteExpr::Unsupported { construct } => {
            Err(format!("unsupported: expression construct {construct}"))
        }
    }
}

fn witness_from(tables: &Tables) -> FiniteWitness {
    let mut observables = BTreeMap::new();
    for (name, rows) in tables {
        let mut list: Vec<ObservableRow> = rows
            .iter()
            .map(|(arguments, value)| ObservableRow {
                arguments: arguments.clone(),
                value: value.clone(),
            })
            .collect();
        list.sort_by(|left, right| left.arguments.cmp(&right.arguments));
        observables.insert(name.clone(), list);
    }
    FiniteWitness { observables }
}

struct Exploration<'a> {
    universe: &'a Universe,
    domains: &'a BTreeMap<String, FiniteDomain>,
    previous: &'a FiniteExpr,
    current: &'a FiniteExpr,
    predicate: &'a FiniteExpr,
    max_steps: u64,
    evaluations: u64,
    worlds_examined: u64,
    previous_ok: bool,
    current_ok: bool,
    witness: Option<FiniteWitness>,
    stopped: bool,
    problem: Option<String>,
}

impl Exploration<'_> {
    fn run(&mut self) -> Result<(), String> {
        let mut tables = BTreeMap::new();
        self.assign_observable(0, &mut tables)
    }

    fn assign_observable(&mut self, index: usize, tables: &mut Tables) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        if index == self.universe.names.len() {
            return self.observe(tables);
        }
        let name = self.universe.names[index].clone();
        self.assign_tuple(&name, 0, tables, index)
    }

    fn assign_tuple(
        &mut self,
        name: &str,
        tuple_index: usize,
        tables: &mut Tables,
        observable_index: usize,
    ) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        let tuples = self.universe.tuples.get(name).cloned().unwrap_or_default();
        let results = self.universe.results.get(name).cloned().unwrap_or_default();
        if tuple_index == tuples.len() {
            return self.assign_observable(observable_index + 1, tables);
        }
        let tuple = tuples[tuple_index].clone();
        let result_domain = self.result_domain(name);
        for result in &results {
            tables.entry(name.to_owned()).or_default().insert(
                tuple.clone(),
                FiniteValue {
                    domain: result_domain.clone(),
                    value: result.clone(),
                },
            );
            self.assign_tuple(name, tuple_index + 1, tables, observable_index)?;
            if self.stopped {
                return Ok(());
            }
        }
        Ok(())
    }

    fn result_domain(&self, name: &str) -> String {
        self.universe
            .result_domains
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    fn observe(&mut self, tables: &Tables) -> Result<(), String> {
        self.evaluations = self.evaluations.saturating_add(3);
        if self.evaluations > self.max_steps {
            return Err("bound: finite exploration evaluation steps".into());
        }
        self.worlds_examined = self.worlds_examined.saturating_add(1);
        let mut predicate_scope = BTreeMap::new();
        let predicate_holds =
            match eval_expr(self.predicate, tables, self.domains, &mut predicate_scope) {
                Ok(value) => value,
                Err(error) => {
                    self.problem = Some(error);
                    self.stopped = true;
                    return Ok(());
                }
            };
        if predicate_holds {
            return Ok(());
        }
        let mut previous_scope = BTreeMap::new();
        let previous_holds =
            match eval_expr(self.previous, tables, self.domains, &mut previous_scope) {
                Ok(value) => value,
                Err(error) => {
                    self.problem = Some(error);
                    self.stopped = true;
                    return Ok(());
                }
            };
        if previous_holds {
            self.previous_ok = false;
        }
        let mut current_scope = BTreeMap::new();
        let current_holds = match eval_expr(self.current, tables, self.domains, &mut current_scope)
        {
            Ok(value) => value,
            Err(error) => {
                self.problem = Some(error);
                self.stopped = true;
                return Ok(());
            }
        };
        if current_holds {
            self.current_ok = false;
            if self.witness.is_none() {
                self.witness = Some(witness_from(tables));
            }
        }
        if !self.previous_ok && !self.current_ok {
            self.stopped = true;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Interaction derivation
// ---------------------------------------------------------------------------

enum CheckOutcome {
    Check(FiniteCheck),
    Unavailable(String),
    Unsupported(String),
    Bound(String),
}

// r[impl cairn.concurrent_footprints.bounds]
fn finite_guarantee_check(
    policy: &FootprintPolicy,
    profile: &SemanticProfile,
    admitted: &AdmittedCase,
    resolved: &ResolvedAssumption,
    usage: &mut ExplorationUsage,
) -> CheckOutcome {
    let guarantee = &resolved.guarantee;
    let (Some(previous_finite), Some(current_finite), Some(predicate)) = (
        guarantee.previous_finite.as_ref(),
        guarantee.current_finite.as_ref(),
        resolved.assumption.finite_predicate.as_ref(),
    ) else {
        return CheckOutcome::Unavailable(
            "declaration does not supply finite previous, current and assumption expressions"
                .into(),
        );
    };
    let (Some(previous_requirement), Some(current_requirement)) = (
        guarantee.previous_requirement.as_ref(),
        guarantee.current_requirement.as_ref(),
    ) else {
        return CheckOutcome::Unavailable(
            "guarantee statements do not name requirement revisions".into(),
        );
    };
    if previous_requirement != current_requirement {
        return CheckOutcome::Unavailable(
            "changed guarantee statements span different requirement revisions".into(),
        );
    }
    let Some(bound) = admitted
        .footprints
        .get(&resolved.producer)
        .and_then(|footprint| footprint.requirements.get(previous_requirement))
    else {
        return CheckOutcome::Unavailable(
            "changed guarantee names a requirement outside the producer footprint".into(),
        );
    };
    let domains = &bound.contract.domains;
    let observables = &bound.contract.observables;
    let max_depth = profile.max_depth.min(policy.bounds.max_depth);
    for (label, expression) in [
        ("previous guarantee", previous_finite),
        ("current guarantee", current_finite),
        ("consumer assumption", predicate),
    ] {
        if let Err(error) = type_check_finite(expression, domains, observables, profile, max_depth)
        {
            if error.starts_with("unsupported:") || error.starts_with("ill_typed:") {
                return CheckOutcome::Unsupported(format!("{label}: {error}"));
            }
            let detail = format!("{label}: {error}");
            usage.bound_exhausted.push(detail.clone());
            return CheckOutcome::Bound(detail);
        }
    }
    let universe = match build_universe(
        domains,
        observables,
        policy.bounds.max_domain_values,
        policy.bounds.max_worlds,
    ) {
        Ok(universe) => universe,
        Err(error) => {
            if error.starts_with("unsupported:") {
                return CheckOutcome::Unsupported(error);
            }
            let detail = format!("requirement {}: {error}", bound.requirement.requirement_id);
            usage.bound_exhausted.push(detail.clone());
            return CheckOutcome::Bound(detail);
        }
    };
    usage.worlds = usage.worlds.saturating_add(universe.worlds);
    let mut exploration = Exploration {
        universe: &universe,
        domains,
        previous: previous_finite,
        current: current_finite,
        predicate,
        max_steps: policy.bounds.max_steps,
        evaluations: 0,
        worlds_examined: 0,
        previous_ok: true,
        current_ok: true,
        witness: None,
        stopped: false,
        problem: None,
    };
    let result = exploration.run();
    if let Err(error) = result {
        usage.bound_exhausted.push(format!(
            "requirement {}: {error}",
            bound.requirement.requirement_id
        ));
        return CheckOutcome::Bound(error);
    }
    if let Some(problem) = exploration.problem {
        return CheckOutcome::Unsupported(problem);
    }
    usage.evaluations = usage.evaluations.saturating_add(exploration.evaluations);
    let complete = exploration.worlds_examined == universe.worlds;
    CheckOutcome::Check(FiniteCheck {
        requirement_id: bound.requirement.requirement_id.clone(),
        worlds: universe.worlds,
        worlds_examined: exploration.worlds_examined,
        evaluations: exploration.evaluations,
        complete,
        stopped_early: exploration.stopped,
        previous_implies_assumption: Some(exploration.previous_ok),
        current_implies_assumption: Some(exploration.current_ok),
        witness: exploration.witness,
        basis: if complete {
            "all declared finite observation worlds were examined".into()
        } else {
            "exploration stopped after both implication directions were decided by a witness".into()
        },
    })
}

fn interaction_id(
    kind: InteractionKind,
    producer: &str,
    consumer: &str,
    guarantee: Option<&str>,
    assumption: Option<&str>,
    invalidation: Option<&str>,
    evidence_basis: Option<&str>,
    invariant: Option<&str>,
) -> Result<String, FootprintError> {
    identity(
        INTERACTION_IDENTITY_DOMAIN_V1,
        &(
            kind.tag(),
            producer,
            consumer,
            guarantee,
            assumption,
            invalidation,
            evidence_basis,
            invariant,
        ),
    )
}

fn guarantee_assessment(
    policy: &FootprintPolicy,
    profile: &SemanticProfile,
    admitted: &AdmittedCase,
    resolved: &ResolvedAssumption,
    usage: &mut ExplorationUsage,
) -> (InteractionAssessment, Option<FiniteCheck>) {
    let guarantee = &resolved.guarantee;
    if guarantee.guarantee.change == GuaranteeChange::Removed {
        return (
            InteractionAssessment {
                outcome: ObligationOutcome::Conflict,
                basis: format!(
                    "consumed guarantee {} was removed by change {}",
                    guarantee.guarantee.id, resolved.producer
                ),
            },
            None,
        );
    }
    if resolved.assumption.dependency == AssumptionDependency::Strict
        && matches!(
            guarantee.guarantee.change,
            GuaranteeChange::Weakened | GuaranteeChange::Replaced
        )
    {
        return (
            InteractionAssessment {
                outcome: ObligationOutcome::Conflict,
                basis: format!(
                    "strict assumption {} names the previous guarantee identity and change {} declared it {}",
                    resolved.assumption.id,
                    resolved.producer,
                    guarantee.guarantee.change.tag()
                ),
            },
            None,
        );
    }
    if guarantee.guarantee.change == GuaranteeChange::Unknown
        || guarantee
            .guarantee
            .unknown
            .as_deref()
            .is_some_and(|unknown| !unknown.trim().is_empty())
    {
        return (
            InteractionAssessment {
                outcome: ObligationOutcome::Unknown,
                basis: format!(
                    "change {} declares unknown guarantee behaviour for {}",
                    resolved.producer, guarantee.guarantee.id
                ),
            },
            None,
        );
    }
    match finite_guarantee_check(policy, profile, admitted, resolved, usage) {
        CheckOutcome::Check(check) => {
            let assessment = match (
                check.previous_implies_assumption,
                check.current_implies_assumption,
            ) {
                (Some(true), Some(false)) => InteractionAssessment {
                    outcome: ObligationOutcome::Conflict,
                    basis: format!(
                        "the previous guarantee established assumption {} over the declared finite universe and the current guarantee does not",
                        resolved.assumption.id
                    ),
                },
                (Some(true), Some(true)) => InteractionAssessment {
                    outcome: ObligationOutcome::NeedsJointEvidence,
                    basis: format!(
                        "the current guarantee still establishes assumption {} over the declared finite universe; joint evidence is still required for the exact combined candidate",
                        resolved.assumption.id
                    ),
                },
                (Some(false), _) => InteractionAssessment {
                    outcome: ObligationOutcome::Unknown,
                    basis: format!(
                        "assumption {} did not follow from the previous guarantee either, so the change cannot be credited or blamed for the gap",
                        resolved.assumption.id
                    ),
                },
                _ => InteractionAssessment {
                    outcome: ObligationOutcome::Unknown,
                    basis: format!(
                        "bounded finite exploration did not establish a definite relation for assumption {}",
                        resolved.assumption.id
                    ),
                },
            };
            (assessment, Some(check))
        }
        CheckOutcome::Unavailable(reason) => {
            let assessment = match guarantee.guarantee.change {
                GuaranteeChange::Strengthened => InteractionAssessment {
                    outcome: ObligationOutcome::NeedsJointEvidence,
                    basis: format!(
                        "strengthening of guarantee {} is declared but unverified; {reason}",
                        guarantee.guarantee.id
                    ),
                },
                GuaranteeChange::Weakened | GuaranteeChange::Replaced => InteractionAssessment {
                    outcome: ObligationOutcome::NeedsJointEvidence,
                    basis: format!(
                        "monotone assumption {} may survive a declared {} guarantee change; {reason}",
                        resolved.assumption.id,
                        guarantee.guarantee.change.tag()
                    ),
                },
                _ => InteractionAssessment {
                    outcome: ObligationOutcome::Unknown,
                    basis: format!(
                        "no finite relation and no admitted interpretation for guarantee {}; {reason}",
                        guarantee.guarantee.id
                    ),
                },
            };
            (assessment, None)
        }
        CheckOutcome::Unsupported(reason) => (
            InteractionAssessment {
                outcome: ObligationOutcome::Unknown,
                basis: format!(
                    "unsupported finite interpretation for guarantee {}: {reason}",
                    guarantee.guarantee.id
                ),
            },
            None,
        ),
        CheckOutcome::Bound(reason) => (
            InteractionAssessment {
                outcome: ObligationOutcome::Unknown,
                basis: format!(
                    "bounded finite exploration could not complete for guarantee {}: {reason}",
                    guarantee.guarantee.id
                ),
            },
            None,
        ),
    }
}

fn affected_invariants_for_assumption(
    admitted: &AdmittedCase,
    consumer: &str,
    producer: &str,
    assumption_id: &str,
    interfaces: &[String],
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for change in [consumer, producer] {
        if let Some(footprint) = admitted.footprints.get(change) {
            for invariant in footprint.invariants.values() {
                let declared = &invariant.invariant;
                let named = declared
                    .under_assumptions
                    .iter()
                    .any(|assumption| assumption == assumption_id);
                let shared = declared.under_assumptions.is_empty()
                    && declared
                        .interfaces
                        .iter()
                        .any(|interface| interfaces.contains(interface));
                if named || shared {
                    ids.insert(declared.id.clone());
                }
            }
        }
    }
    ids.into_iter().collect()
}

// r[impl cairn.concurrent_footprints.derivation]
fn derive_interactions(
    policy: &FootprintPolicy,
    profile: &SemanticProfile,
    admitted: &AdmittedCase,
    usage: &mut ExplorationUsage,
) -> BTreeMap<(String, String), Vec<DerivedInteraction>> {
    let mut pairs: BTreeMap<(String, String), Vec<DerivedInteraction>> = BTreeMap::new();
    let mut push = |interaction: DerivedInteraction| {
        let mut participants = [
            interaction.producer_change.clone(),
            interaction.consumer_change.clone(),
        ];
        participants.sort();
        let key = (participants[0].clone(), participants[1].clone());
        pairs.entry(key).or_default().push(interaction);
    };

    for resolved in &admitted.resolved_assumptions {
        if !resolved.consumed_previous {
            continue;
        }
        let mut interfaces: Vec<String> = resolved.guarantee.guarantee.interfaces.clone();
        interfaces.sort();
        interfaces.dedup();
        let (assessment, finite) = guarantee_assessment(policy, profile, admitted, resolved, usage);
        let mut affected_claims = resolved.guarantee.guarantee.affected_claims.clone();
        affected_claims.extend(resolved.assumption.evidence_basis.iter().cloned());
        affected_claims.sort();
        affected_claims.dedup();
        let mut configurations = resolved.guarantee.guarantee.configurations.clone();
        configurations.extend(resolved.assumption.configurations.iter().cloned());
        configurations.sort();
        configurations.dedup();
        let id = interaction_id(
            InteractionKind::ChangedGuaranteeAffectsAssumption,
            &resolved.producer,
            &resolved.consumer,
            Some(resolved.guarantee.guarantee.id.as_str()),
            Some(resolved.assumption.id.as_str()),
            None,
            None,
            None,
        )
        .expect("static interaction identity is serializable");
        push(DerivedInteraction {
            id,
            kind: InteractionKind::ChangedGuaranteeAffectsAssumption,
            producer_change: resolved.producer.clone(),
            consumer_change: resolved.consumer.clone(),
            guarantee: Some(resolved.guarantee.guarantee.id.clone()),
            assumption: Some(resolved.assumption.id.clone()),
            invalidation: None,
            evidence_basis: None,
            invariant: None,
            interfaces,
            configurations,
            affected_claims,
            assessment,
            finite,
        });
    }

    let mut change_ids: Vec<String> = admitted.footprints.keys().cloned().collect();
    change_ids.sort();
    let mut invalidation_pairs = BTreeSet::new();
    for producer in &change_ids {
        let Some(producer_footprint) = admitted.footprints.get(producer) else {
            continue;
        };
        for invalidation in &producer_footprint.invalidated {
            for consumer in &change_ids {
                if consumer == producer {
                    continue;
                }
                let Some(consumer_footprint) = admitted.footprints.get(consumer) else {
                    continue;
                };
                for basis in &consumer_footprint.evidence_basis {
                    if !basis
                        .obligations
                        .iter()
                        .any(|obligation| obligation == &invalidation.obligation)
                    {
                        continue;
                    }
                    invalidation_pairs.insert((
                        producer.clone(),
                        consumer.clone(),
                        invalidation.id.clone(),
                        basis.id.clone(),
                    ));
                }
            }
        }
    }
    for (producer, consumer, invalidation_id, basis_id) in invalidation_pairs {
        let producer_footprint = &admitted.footprints[&producer];
        let consumer_footprint = &admitted.footprints[&consumer];
        let invalidation = producer_footprint
            .invalidated
            .iter()
            .find(|candidate| candidate.id == invalidation_id)
            .expect("invalidation was indexed from this footprint");
        let basis = consumer_footprint
            .evidence_basis
            .iter()
            .find(|candidate| candidate.id == basis_id)
            .expect("evidence basis was indexed from this footprint");
        let mut interfaces = invalidation.interfaces.clone();
        interfaces.sort();
        interfaces.dedup();
        let mut affected_claims = basis.claims.clone();
        affected_claims.sort();
        affected_claims.dedup();
        let mut configurations = invalidation.configurations.clone();
        configurations.extend(basis.configurations.iter().cloned());
        configurations.sort();
        configurations.dedup();
        let id = interaction_id(
            InteractionKind::InvalidationAffectsEvidenceBasis,
            &producer,
            &consumer,
            None,
            None,
            Some(invalidation.id.as_str()),
            Some(basis.id.as_str()),
            None,
        )
        .expect("static interaction identity is serializable");
        push(DerivedInteraction {
            id,
            kind: InteractionKind::InvalidationAffectsEvidenceBasis,
            producer_change: producer.clone(),
            consumer_change: consumer.clone(),
            guarantee: None,
            assumption: None,
            invalidation: Some(invalidation.id.clone()),
            evidence_basis: Some(basis.id.clone()),
            invariant: None,
            interfaces,
            configurations,
            affected_claims,
            assessment: InteractionAssessment {
                outcome: ObligationOutcome::NeedsJointEvidence,
                basis: format!(
                    "change {} invalidates obligation {}, which is part of evidence basis {} for change {}; the affected claims need joint evidence for the exact combined candidate",
                    producer, invalidation.obligation, basis.id, consumer
                ),
            },
            finite: None,
        });
    }

    for (index, producer) in change_ids.iter().enumerate() {
        for consumer in change_ids.iter().skip(index + 1) {
            let producer_footprint = &admitted.footprints[producer];
            let consumer_footprint = &admitted.footprints[consumer];
            for producer_invariant in producer_footprint.invariants.values() {
                for consumer_invariant in consumer_footprint.invariants.values() {
                    let same_id =
                        producer_invariant.invariant.id == consumer_invariant.invariant.id;
                    let same_commitment = producer_invariant.commitment_identity
                        == consumer_invariant.commitment_identity;
                    if !same_id && !same_commitment {
                        continue;
                    }
                    let mut interfaces = producer_invariant.invariant.interfaces.clone();
                    interfaces.extend(consumer_invariant.invariant.interfaces.iter().cloned());
                    interfaces.sort();
                    interfaces.dedup();
                    let mut configurations = producer_invariant.invariant.configurations.clone();
                    configurations
                        .extend(consumer_invariant.invariant.configurations.iter().cloned());
                    configurations.sort();
                    configurations.dedup();
                    let id = interaction_id(
                        InteractionKind::OverlappingInvariantCommitment,
                        producer,
                        consumer,
                        None,
                        None,
                        None,
                        None,
                        Some(producer_invariant.invariant.id.as_str()),
                    )
                    .expect("static interaction identity is serializable");
                    push(DerivedInteraction {
                        id,
                        kind: InteractionKind::OverlappingInvariantCommitment,
                        producer_change: producer.clone(),
                        consumer_change: consumer.clone(),
                        guarantee: None,
                        assumption: None,
                        invalidation: None,
                        evidence_basis: None,
                        invariant: Some(producer_invariant.invariant.id.clone()),
                        interfaces,
                        configurations,
                        affected_claims: Vec::new(),
                        assessment: InteractionAssessment {
                            outcome: ObligationOutcome::NeedsJointEvidence,
                            basis: if producer_invariant.commitment_identity
                                == consumer_invariant.commitment_identity
                            {
                                format!(
                                    "both changes commit to invariant {} with the same declared identity; joint assessment is still required because an identical commitment is a declaration, not proof",
                                    producer_invariant.invariant.id
                                )
                            } else {
                                format!(
                                    "both changes commit to invariant {} with different declared commitments; joint assessment is required",
                                    producer_invariant.invariant.id
                                )
                            },
                        },
                        finite: None,
                    });
                }
            }
            for invalidation in &producer_footprint.invalidated {
                for invariant in consumer_footprint.invariants.values() {
                    if invariant.invariant.id != invalidation.obligation {
                        continue;
                    }
                    let mut interfaces = invalidation.interfaces.clone();
                    interfaces.extend(invariant.invariant.interfaces.iter().cloned());
                    interfaces.sort();
                    interfaces.dedup();
                    let id = interaction_id(
                        InteractionKind::InvalidationUnderminesInvariant,
                        producer,
                        consumer,
                        None,
                        None,
                        Some(invalidation.id.as_str()),
                        None,
                        Some(invariant.invariant.id.as_str()),
                    )
                    .expect("static interaction identity is serializable");
                    push(DerivedInteraction {
                        id,
                        kind: InteractionKind::InvalidationUnderminesInvariant,
                        producer_change: producer.clone(),
                        consumer_change: consumer.clone(),
                        guarantee: None,
                        assumption: None,
                        invalidation: Some(invalidation.id.clone()),
                        evidence_basis: None,
                        invariant: Some(invariant.invariant.id.clone()),
                        interfaces,
                        configurations: invalidation.configurations.clone(),
                        affected_claims: Vec::new(),
                        assessment: InteractionAssessment {
                            outcome: ObligationOutcome::Conflict,
                            basis: format!(
                                "change {} invalidates obligation {}, which change {} declares as a preserved invariant",
                                producer, invalidation.obligation, consumer
                            ),
                        },
                        finite: None,
                    });
                }
            }
            for invalidation in &consumer_footprint.invalidated {
                for invariant in producer_footprint.invariants.values() {
                    if invariant.invariant.id != invalidation.obligation {
                        continue;
                    }
                    let mut interfaces = invalidation.interfaces.clone();
                    interfaces.extend(invariant.invariant.interfaces.iter().cloned());
                    interfaces.sort();
                    interfaces.dedup();
                    let id = interaction_id(
                        InteractionKind::InvalidationUnderminesInvariant,
                        consumer,
                        producer,
                        None,
                        None,
                        Some(invalidation.id.as_str()),
                        None,
                        Some(invariant.invariant.id.as_str()),
                    )
                    .expect("static interaction identity is serializable");
                    push(DerivedInteraction {
                        id,
                        kind: InteractionKind::InvalidationUnderminesInvariant,
                        producer_change: consumer.clone(),
                        consumer_change: producer.clone(),
                        guarantee: None,
                        assumption: None,
                        invalidation: Some(invalidation.id.clone()),
                        evidence_basis: None,
                        invariant: Some(invariant.invariant.id.clone()),
                        interfaces,
                        configurations: invalidation.configurations.clone(),
                        affected_claims: Vec::new(),
                        assessment: InteractionAssessment {
                            outcome: ObligationOutcome::Conflict,
                            basis: format!(
                                "change {} invalidates obligation {}, which change {} declares as a preserved invariant",
                                consumer, invalidation.obligation, producer
                            ),
                        },
                        finite: None,
                    });
                }
            }
            let producer_interfaces = &producer_footprint.interfaces;
            let consumer_interfaces = &consumer_footprint.interfaces;
            for (interface_id, producer_interface) in producer_interfaces {
                let Some(consumer_interface) = consumer_interfaces.get(interface_id) else {
                    continue;
                };
                if producer_interface.configuration_blake3
                    != consumer_interface.configuration_blake3
                {
                    let id = interaction_id(
                        InteractionKind::InterfaceConfigurationMismatch,
                        producer,
                        consumer,
                        None,
                        None,
                        None,
                        None,
                        Some(interface_id.as_str()),
                    )
                    .expect("static interaction identity is serializable");
                    push(DerivedInteraction {
                        id,
                        kind: InteractionKind::InterfaceConfigurationMismatch,
                        producer_change: producer.clone(),
                        consumer_change: consumer.clone(),
                        guarantee: None,
                        assumption: None,
                        invalidation: None,
                        evidence_basis: None,
                        invariant: None,
                        interfaces: vec![interface_id.clone()],
                        configurations: vec![
                            producer_interface.configuration_blake3.clone(),
                            consumer_interface.configuration_blake3.clone(),
                        ],
                        affected_claims: Vec::new(),
                        assessment: InteractionAssessment {
                            outcome: ObligationOutcome::Conflict,
                            basis: format!(
                                "both changes declare interface {interface_id} with different configuration identities"
                            ),
                        },
                        finite: None,
                    });
                }
                let operation_conflict = matches!(
                    (
                        producer_interface.change_kind,
                        consumer_interface.change_kind
                    ),
                    (InterfaceChange::Remove, InterfaceChange::Add)
                        | (InterfaceChange::Remove, InterfaceChange::Modify)
                        | (InterfaceChange::Add, InterfaceChange::Remove)
                        | (InterfaceChange::Modify, InterfaceChange::Remove)
                );
                if operation_conflict {
                    let id = interaction_id(
                        InteractionKind::InterfaceOperationConflict,
                        producer,
                        consumer,
                        None,
                        None,
                        None,
                        None,
                        Some(interface_id.as_str()),
                    )
                    .expect("static interaction identity is serializable");
                    push(DerivedInteraction {
                        id,
                        kind: InteractionKind::InterfaceOperationConflict,
                        producer_change: producer.clone(),
                        consumer_change: consumer.clone(),
                        guarantee: None,
                        assumption: None,
                        invalidation: None,
                        evidence_basis: None,
                        invariant: None,
                        interfaces: vec![interface_id.clone()],
                        configurations: Vec::new(),
                        affected_claims: Vec::new(),
                        assessment: InteractionAssessment {
                            outcome: ObligationOutcome::Conflict,
                            basis: format!(
                                "one change removes interface {interface_id} while the other keeps or adds it"
                            ),
                        },
                        finite: None,
                    });
                }
            }
        }
    }

    pairs
}

// ---------------------------------------------------------------------------
// Obligation assembly
// ---------------------------------------------------------------------------

fn composition_ref(composition: &CompositionCandidate) -> CompositionRef {
    CompositionRef {
        id: composition.id.clone(),
        order: composition.order.clone(),
        fixed: composition.fixed,
    }
}

fn normalize_composition_refs(refs: &[CompositionRef]) -> Vec<CompositionRef> {
    let mut sorted = refs.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
}

#[derive(Serialize)]
struct CandidateBasis<'a> {
    participants: &'a [String],
    binding: &'a BTreeMap<String, CandidateParticipant>,
    compositions: &'a [CompositionRef],
    context: &'a CaseContext,
}

fn participant_bound(footprint: &AdmittedFootprint) -> CandidateParticipant {
    let mut requirements = BTreeMap::new();
    for (requirement_id, bound) in &footprint.requirements {
        requirements.insert(requirement_id.clone(), bound.summary(requirement_id));
    }
    CandidateParticipant {
        source_revision: footprint.change.source_revision.clone(),
        intent_revision: footprint.change.intent_revision.clone(),
        requirements,
    }
}

fn outcome_counts(obligations: &[CompatibilityObligation]) -> OutcomeCounts {
    let mut counts = OutcomeCounts {
        conflict: 0,
        needs_joint_evidence: 0,
        supported_within_declared_scope: 0,
        unknown: 0,
    };
    for obligation in obligations {
        match obligation.outcome {
            ObligationOutcome::Conflict => counts.conflict += 1,
            ObligationOutcome::NeedsJointEvidence => counts.needs_joint_evidence += 1,
            ObligationOutcome::SupportedWithinDeclaredScope => {
                counts.supported_within_declared_scope += 1;
            }
            ObligationOutcome::Unknown => counts.unknown += 1,
        }
    }
    counts
}

struct ObligationDraft {
    id: String,
    participants: Vec<String>,
    candidate: CombinedCandidate,
    interactions: Vec<DerivedInteraction>,
    affected_claims: Vec<String>,
    affected_interfaces: Vec<String>,
    configurations: BTreeMap<String, String>,
    affected_invariants: Vec<String>,
    integration_conditions: Vec<IntegrationConditionStatus>,
    required_evidence: Vec<RequiredEvidence>,
    order_sensitive: bool,
    commutativity_required: bool,
    propositions: ObligationPropositions,
    outcome: ObligationOutcome,
    outcome_basis: String,
    evidence: EvidenceDisposition,
    path: Vec<ExplanationStep>,
    fingerprint: ObligationFingerprint,
}

fn map_diff(
    prior: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut missing = Vec::new();
    let mut changed = Vec::new();
    let mut added = Vec::new();
    for (key, value) in prior {
        match current.get(key) {
            None => missing.push(key.clone()),
            Some(other) if other != value => changed.push(key.clone()),
            Some(_) => {}
        }
    }
    for key in current.keys() {
        if !prior.contains_key(key) {
            added.push(key.clone());
        }
    }
    missing.sort();
    changed.sort();
    added.sort();
    (missing, changed, added)
}

// r[impl cairn.concurrent_footprints.invalidation]
fn prior_applicability(
    admitted: &AdmittedCase,
    obligations: &[CompatibilityObligation],
) -> Vec<PriorApplicability> {
    let mut prior: Vec<ObligationFingerprint> = admitted.prior_obligations.clone();
    prior.sort_by(|left, right| left.obligation_id.cmp(&right.obligation_id));
    let mut out = Vec::new();
    for prior_obligation in prior {
        let Some(current) = obligations
            .iter()
            .find(|obligation| obligation.id == prior_obligation.obligation_id)
        else {
            out.push(PriorApplicability {
                obligation_id: prior_obligation.obligation_id,
                applicable: false,
                reasons: vec!["obligation_absent".into()],
                detail: "no current obligation carries this identity".into(),
            });
            continue;
        };
        let mut reasons = Vec::new();
        let mut details = Vec::new();
        let (missing, changed, added) = map_diff(
            &prior_obligation.participants,
            &current.fingerprint.participants,
        );
        if !missing.is_empty() || !changed.is_empty() || !added.is_empty() {
            reasons.push("participants_changed".into());
            details.push(format!(
                "participants missing {missing:?}, changed {changed:?}, added {added:?}"
            ));
        }
        let (missing, changed, added) = map_diff(
            &prior_obligation.dependencies,
            &current.fingerprint.dependencies,
        );
        if !missing.is_empty() || !changed.is_empty() || !added.is_empty() {
            reasons.push("dependencies_changed".into());
            details.push(format!(
                "dependencies missing {missing:?}, changed {changed:?}, added {added:?}"
            ));
        }
        let (missing, changed, added) = map_diff(
            &prior_obligation.configurations,
            &current.fingerprint.configurations,
        );
        if !missing.is_empty() || !changed.is_empty() || !added.is_empty() {
            reasons.push("configuration_changed".into());
            details.push(format!(
                "configurations missing {missing:?}, changed {changed:?}, added {added:?}"
            ));
        }
        if prior_obligation.composition != current.fingerprint.composition {
            reasons.push("composition_changed".into());
            details.push(format!(
                "composition {:?} became {:?}",
                prior_obligation.composition, current.fingerprint.composition
            ));
        }
        out.push(PriorApplicability {
            obligation_id: prior_obligation.obligation_id,
            applicable: reasons.is_empty(),
            reasons,
            detail: if details.is_empty() {
                "participants, dependencies, configurations and composition are unchanged".into()
            } else {
                details.join("; ")
            },
        });
    }
    out
}

// r[impl cairn.concurrent_footprints.obligation]
// r[impl cairn.concurrent_footprints.explanation]
fn build_obligation(
    admitted: &AdmittedCase,
    pair: (&str, &str),
    interactions: &[DerivedInteraction],
    gaps: &mut Vec<DeclarationGap>,
) -> Result<ObligationDraft, FootprintError> {
    let mut ordered = interactions.to_vec();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let interactions: &[DerivedInteraction] = &ordered;
    let pair_set: BTreeSet<String> = [pair.0.to_owned(), pair.1.to_owned()].into_iter().collect();
    let candidates: Vec<&CompositionCandidate> = admitted
        .compositions
        .iter()
        .filter(|composition| {
            pair_set
                .iter()
                .all(|participant| composition.participants.contains(participant))
        })
        .collect();
    let minimum = candidates
        .iter()
        .map(|composition| composition.participants.len())
        .min();
    let selected: Vec<&CompositionCandidate> = candidates
        .into_iter()
        .filter(|composition| Some(composition.participants.len()) == minimum)
        .collect();
    let admitted_compositions: Vec<CompositionRef> = selected
        .iter()
        .filter(|composition| composition.admitted)
        .map(|composition| composition_ref(composition))
        .collect();
    let admitted_compositions = normalize_composition_refs(&admitted_compositions);
    let participants: Vec<String> = if let Some(first) = selected.first() {
        let mut participants = first.participants.clone();
        participants.sort();
        participants
    } else {
        let mut participants = vec![pair.0.to_owned(), pair.1.to_owned()];
        participants.sort();
        participants
    };
    if admitted_compositions.is_empty() {
        gaps.push(DeclarationGap {
            kind: if selected.is_empty() {
                "composition_undeclared".into()
            } else {
                "no_admitted_composition".into()
            },
            change: pair.0.to_owned(),
            reference: pair.1.to_owned(),
            detail: format!(
                "no admitted composition covers changes {} and {}",
                pair.0, pair.1
            ),
        });
    }
    let mut binding = BTreeMap::new();
    for participant in &participants {
        let footprint = admitted.footprints.get(participant).ok_or_else(|| {
            FootprintError::InvalidInput(format!("composition names unknown change {participant}"))
        })?;
        let participant_bound = participant_bound(footprint);
        binding.insert(participant.clone(), participant_bound);
    }
    let candidate_blake3 = identity(
        CANDIDATE_IDENTITY_DOMAIN_V1,
        &CandidateBasis {
            participants: &participants,
            binding: &binding,
            compositions: &admitted_compositions,
            context: &admitted.context,
        },
    )?;
    let candidate = CombinedCandidate {
        participants: participants.clone(),
        binding,
        compositions: admitted_compositions.clone(),
        context: admitted.context.clone(),
        combined_candidate_blake3: candidate_blake3,
    };

    let mut affected_claims = BTreeSet::new();
    let mut interfaces = BTreeSet::new();
    let mut configurations: BTreeMap<String, String> = BTreeMap::new();
    let mut affected_invariants = BTreeSet::new();
    for interaction in interactions {
        affected_claims.extend(interaction.affected_claims.iter().cloned());
        for interface in &interaction.interfaces {
            interfaces.insert(interface.clone());
        }
        if let Some(interface) = interaction
            .interfaces
            .first()
            .filter(|_| interaction.kind == InteractionKind::InterfaceConfigurationMismatch)
        {
            let mut identities: Vec<String> = interaction.configurations.clone();
            identities.sort();
            if let Some(first) = identities.first() {
                configurations.insert(interface.clone(), first.clone());
            }
        }
        if let Some(invariant) = &interaction.invariant {
            affected_invariants.insert(invariant.clone());
        }
        if interaction.kind == InteractionKind::ChangedGuaranteeAffectsAssumption {
            if let (Some(assumption_id), Some(_)) =
                (&interaction.assumption, &interaction.guarantee)
            {
                for invariant in affected_invariants_for_assumption(
                    admitted,
                    &interaction.consumer_change,
                    &interaction.producer_change,
                    assumption_id,
                    &interaction.interfaces,
                ) {
                    affected_invariants.insert(invariant);
                }
            }
        }
    }
    for participant in &participants {
        let Some(footprint) = admitted.footprints.get(participant) else {
            continue;
        };
        for (interface_id, interface) in &footprint.interfaces {
            if interfaces.contains(interface_id) {
                configurations
                    .entry(interface_id.clone())
                    .or_insert_with(|| interface.configuration_blake3.clone());
            }
        }
    }
    let mut order_sensitive = false;
    for shared in interfaces.iter() {
        let mut flags = Vec::new();
        for participant in &participants {
            if let Some(footprint) = admitted.footprints.get(participant) {
                if let Some(interface) = footprint.interfaces.get(shared) {
                    flags.push(interface.order_sensitive);
                }
            }
        }
        if flags.iter().any(|flag| *flag) {
            order_sensitive = true;
        }
    }
    let distinct_orders: BTreeSet<Vec<String>> = admitted_compositions
        .iter()
        .map(|composition| composition.order.clone())
        .collect();
    if distinct_orders.len() > 1 {
        order_sensitive = true;
    }
    let single_fixed = admitted_compositions.len() == 1 && admitted_compositions[0].fixed;
    let commutativity_required = order_sensitive && !single_fixed;

    let mut integration_conditions = Vec::new();
    let mut condition_ids = BTreeSet::new();
    for invariant_id in &affected_invariants {
        for participant in &participants {
            let Some(footprint) = admitted.footprints.get(participant) else {
                continue;
            };
            let Some(invariant) = footprint.invariants.get(invariant_id) else {
                continue;
            };
            for condition in &invariant.invariant.required_conditions {
                if !condition_ids.insert(condition.id.clone()) {
                    continue;
                }
                let provided_by: Vec<String> = participants
                    .iter()
                    .filter(|candidate| {
                        admitted
                            .footprints
                            .get(*candidate)
                            .is_some_and(|footprint| {
                                footprint.provided_conditions.contains(&condition.id)
                            })
                    })
                    .cloned()
                    .collect();
                integration_conditions.push(IntegrationConditionStatus {
                    id: condition.id.clone(),
                    statement: condition.statement.clone(),
                    state: if provided_by.is_empty() {
                        "missing".into()
                    } else {
                        "provided".into()
                    },
                    provided_by,
                });
            }
        }
    }
    integration_conditions.sort_by(|left, right| left.id.cmp(&right.id));

    let required_evidence = vec![
        RequiredEvidence {
            proposition: PropositionKind::Coexistence,
            requirement: "joint-evidence.proposition.coexistence".into(),
            scope: if admitted_compositions.len() > 1 {
                "all_admitted_compositions".into()
            } else {
                "declared_composition".into()
            },
        },
        RequiredEvidence {
            proposition: PropositionKind::InvariantPreservation,
            requirement: "joint-evidence.proposition.invariant-preservation".into(),
            scope: if admitted_compositions.len() > 1 {
                "all_admitted_compositions".into()
            } else {
                "declared_composition".into()
            },
        },
        RequiredEvidence {
            proposition: PropositionKind::Commutativity,
            requirement: "joint-evidence.proposition.commutativity".into(),
            scope: if commutativity_required {
                "all_admitted_compositions".into()
            } else if order_sensitive {
                "not_required_fixed_composition".into()
            } else {
                "not_applicable".into()
            },
        },
    ];

    let interaction_ids: Vec<String> = interactions
        .iter()
        .map(|interaction| interaction.id.clone())
        .collect();
    let obligation_basis = (
        candidate.combined_candidate_blake3.clone(),
        interaction_ids.clone(),
        affected_claims.iter().cloned().collect::<Vec<String>>(),
        vec![
            PropositionKind::Coexistence,
            PropositionKind::InvariantPreservation,
            PropositionKind::Commutativity,
        ],
        affected_invariants.iter().cloned().collect::<Vec<String>>(),
        integration_conditions
            .iter()
            .map(|condition| condition.id.clone())
            .collect::<Vec<String>>(),
    );
    let obligation_id = identity(OBLIGATION_IDENTITY_DOMAIN_V1, &obligation_basis)?;

    let mut fingerprint_dependencies: BTreeMap<String, String> = BTreeMap::new();
    for interaction in interactions {
        match interaction.kind {
            InteractionKind::ChangedGuaranteeAffectsAssumption => {
                let (Some(guarantee_id), Some(assumption_id)) =
                    (&interaction.guarantee, &interaction.assumption)
                else {
                    continue;
                };
                let Some(producer) = admitted.footprints.get(&interaction.producer_change) else {
                    continue;
                };
                let Some(guarantee) = producer.guarantees.get(guarantee_id) else {
                    continue;
                };
                let current_identity = guarantee
                    .current_identity
                    .clone()
                    .unwrap_or_else(|| "removed".into());
                fingerprint_dependencies.insert(
                    format!("guarantee.{}.{}", interaction.producer_change, guarantee_id),
                    current_identity,
                );
                let Some(consumer) = admitted.footprints.get(&interaction.consumer_change) else {
                    continue;
                };
                if let Some(assumption) = consumer
                    .assumptions
                    .iter()
                    .find(|candidate| candidate.id == *assumption_id)
                {
                    fingerprint_dependencies.insert(
                        format!(
                            "assumption.{}.{}",
                            interaction.consumer_change, assumption_id
                        ),
                        assumption.consumed_identity.clone(),
                    );
                }
            }
            InteractionKind::InvalidationAffectsEvidenceBasis
            | InteractionKind::InvalidationUnderminesInvariant => {
                if let Some(invalidation_id) = &interaction.invalidation {
                    if let Some(producer) = admitted.footprints.get(&interaction.producer_change) {
                        if let Some(invalidation) = producer
                            .invalidated
                            .iter()
                            .find(|candidate| candidate.id == *invalidation_id)
                        {
                            fingerprint_dependencies.insert(
                                format!(
                                    "invalidation.{}.{}",
                                    interaction.producer_change, invalidation_id
                                ),
                                invalidation.obligation.clone(),
                            );
                        }
                    }
                }
                if let Some(basis_id) = &interaction.evidence_basis {
                    if let Some(consumer) = admitted.footprints.get(&interaction.consumer_change) {
                        if let Some(basis) = consumer
                            .evidence_basis
                            .iter()
                            .find(|candidate| candidate.id == *basis_id)
                        {
                            fingerprint_dependencies.insert(
                                format!(
                                    "evidence-basis.{}.{}",
                                    interaction.consumer_change, basis_id
                                ),
                                basis.obligations.join(","),
                            );
                        }
                    }
                }
            }
            InteractionKind::OverlappingInvariantCommitment => {
                if let Some(invariant_id) = &interaction.invariant {
                    for change in [&interaction.producer_change, &interaction.consumer_change] {
                        if let Some(invariant) = admitted
                            .footprints
                            .get(change)
                            .and_then(|footprint| footprint.invariants.get(invariant_id))
                        {
                            fingerprint_dependencies.insert(
                                format!("invariant.{change}.{invariant_id}"),
                                invariant.commitment_identity.clone(),
                            );
                        }
                    }
                }
            }
            InteractionKind::InterfaceConfigurationMismatch
            | InteractionKind::InterfaceOperationConflict => {}
        }
    }
    let mut fingerprint_configurations: BTreeMap<String, String> = BTreeMap::new();
    for interface in &interfaces {
        for participant in &participants {
            if let Some(declared) = admitted
                .footprints
                .get(participant)
                .and_then(|footprint| footprint.interfaces.get(interface))
            {
                fingerprint_configurations
                    .insert(interface.clone(), declared.configuration_blake3.clone());
                break;
            }
        }
    }
    let mut fingerprint_participants = BTreeMap::new();
    for participant in &participants {
        if let Some(footprint) = admitted.footprints.get(participant) {
            fingerprint_participants.insert(
                participant.clone(),
                footprint.change.intent_revision.clone(),
            );
        }
    }
    let mut composition_fingerprint = Vec::new();
    for composition in &admitted_compositions {
        composition_fingerprint.push(format!(
            "{}:{}:{}",
            composition.id,
            composition.order.join(">"),
            composition.fixed
        ));
    }
    let fingerprint = ObligationFingerprint {
        obligation_id: obligation_id.clone(),
        participants: fingerprint_participants,
        dependencies: fingerprint_dependencies,
        configurations: fingerprint_configurations,
        composition: composition_fingerprint,
    };

    // Evidence admission.
    let mut evidence = EvidenceDisposition {
        admitted: Vec::new(),
        rejected: Vec::new(),
        insufficient: Vec::new(),
        individual_receipts_considered: admitted
            .individual_receipts
            .iter()
            .filter(|receipt| participants.contains(&receipt.change))
            .map(|receipt| receipt.id.clone())
            .collect(),
        individual_receipts_do_not_discharge: true,
    };
    let mut coexistence = PropositionState::NotEstablished;
    let mut preservation = PropositionState::NotEstablished;
    let mut commutativity = if order_sensitive {
        PropositionState::NotEstablished
    } else {
        PropositionState::NotApplicable
    };
    let mut applicable_evidence: Vec<&JointEvidence> = admitted
        .joint_evidence
        .iter()
        .filter(|candidate_evidence| candidate_evidence.obligation == obligation_id)
        .collect();
    applicable_evidence.sort_by(|left, right| left.id.cmp(&right.id));
    for candidate_evidence in applicable_evidence {
        let mut reasons = Vec::new();
        if candidate_evidence.combined_candidate_blake3 != candidate.combined_candidate_blake3 {
            reasons.push("candidate_mismatch".to_string());
        }
        let mut evidence_participants = candidate_evidence.participants.clone();
        evidence_participants.sort();
        evidence_participants.dedup();
        if evidence_participants != participants {
            reasons.push("participants_mismatch".to_string());
        }
        if candidate_evidence.context != admitted.context {
            reasons.push("context_mismatch".to_string());
        }
        for claim in &affected_claims {
            if !candidate_evidence.affected_claims.contains(claim) {
                reasons.push(format!("affected_claim_missing:{claim}"));
            }
        }
        let declared = normalize_composition_refs(&candidate_evidence.compositions);
        let composition_matches = match candidate_evidence.scope {
            EvidenceScope::AllAdmittedCompositions => {
                !declared.is_empty() && declared == admitted_compositions
            }
            EvidenceScope::DeclaredComposition | EvidenceScope::Witness => {
                declared.len() == 1 && admitted_compositions.contains(&declared[0])
            }
        };
        if !composition_matches {
            reasons.push("composition_mismatch".to_string());
        }
        let mut seen = BTreeSet::new();
        for proposition in &candidate_evidence.propositions {
            if !seen.insert(proposition.proposition) {
                reasons.push(format!(
                    "duplicate_proposition:{}",
                    proposition.proposition.tag()
                ));
            }
        }
        if !reasons.is_empty() {
            reasons.sort();
            evidence.rejected.push(EvidenceRejection {
                id: candidate_evidence.id.clone(),
                reasons,
            });
            continue;
        }
        if candidate_evidence.scope == EvidenceScope::Witness {
            // A satisfiability witness is one model. It establishes neither
            // coexistence nor preservation over the admitted composition set.
            evidence.insufficient.push(EvidenceRejection {
                id: candidate_evidence.id.clone(),
                reasons: vec![
                    "no_required_proposition_supported".to_string(),
                    "witness_scope_insufficient".to_string(),
                ],
            });
            continue;
        }
        let covers_all = candidate_evidence.scope == EvidenceScope::AllAdmittedCompositions
            || admitted_compositions.len() == 1;
        let mut contributed = false;
        for proposition in &candidate_evidence.propositions {
            if proposition.state != PropositionState::Supported {
                continue;
            }
            match proposition.proposition {
                PropositionKind::Coexistence if covers_all => {
                    coexistence = PropositionState::Supported;
                    contributed = true;
                }
                PropositionKind::InvariantPreservation
                    if candidate_evidence.scope == EvidenceScope::AllAdmittedCompositions =>
                {
                    if admitted_compositions.len() > 1 || covers_all {
                        preservation = PropositionState::Supported;
                        contributed = true;
                    }
                }
                PropositionKind::InvariantPreservation
                    if candidate_evidence.scope == EvidenceScope::DeclaredComposition
                        && admitted_compositions.len() == 1 =>
                {
                    preservation = PropositionState::Supported;
                    contributed = true;
                }
                PropositionKind::Commutativity
                    if candidate_evidence.scope == EvidenceScope::AllAdmittedCompositions
                        && commutativity_required =>
                {
                    commutativity = PropositionState::Supported;
                    contributed = true;
                }
                _ => {}
            }
        }
        if contributed {
            evidence.admitted.push(candidate_evidence.id.clone());
        } else {
            let mut reasons = vec!["no_required_proposition_supported".to_string()];
            if candidate_evidence.scope == EvidenceScope::Witness {
                reasons.push("witness_scope_insufficient".to_string());
            }
            evidence.insufficient.push(EvidenceRejection {
                id: candidate_evidence.id.clone(),
                reasons,
            });
        }
    }

    let conflicts: Vec<&DerivedInteraction> = interactions
        .iter()
        .filter(|interaction| interaction.assessment.outcome == ObligationOutcome::Conflict)
        .collect();
    let unknowns: Vec<&DerivedInteraction> = interactions
        .iter()
        .filter(|interaction| interaction.assessment.outcome == ObligationOutcome::Unknown)
        .collect();
    let participant_unknown = participants.iter().any(|participant| {
        admitted
            .footprints
            .get(participant)
            .is_some_and(|footprint| !footprint.unknown.is_empty())
    });
    let participant_gap = gaps.iter().any(|gap| participants.contains(&gap.change));
    let discharged = coexistence == PropositionState::Supported
        && preservation == PropositionState::Supported
        && (!commutativity_required || commutativity == PropositionState::Supported);

    let (outcome, outcome_basis) = if !conflicts.is_empty() {
        (
            ObligationOutcome::Conflict,
            conflicts
                .iter()
                .map(|interaction| interaction.assessment.basis.clone())
                .collect::<Vec<String>>()
                .join("; "),
        )
    } else if participant_unknown || participant_gap || admitted_compositions.is_empty() {
        let mut reasons = Vec::new();
        if participant_unknown {
            reasons.push("a participant declares unknown scope".to_string());
        }
        if participant_gap {
            reasons.push("a participant has an unresolved declaration gap".to_string());
        }
        if admitted_compositions.is_empty() {
            reasons.push("no admitted composition names the participants".to_string());
        }
        (ObligationOutcome::Unknown, reasons.join("; "))
    } else if !unknowns.is_empty() {
        (
            ObligationOutcome::Unknown,
            unknowns
                .iter()
                .map(|interaction| interaction.assessment.basis.clone())
                .collect::<Vec<String>>()
                .join("; "),
        )
    } else if discharged {
        (
            ObligationOutcome::SupportedWithinDeclaredScope,
            format!(
                "joint evidence {} covers coexistence and invariant preservation for compositions [{}]{}",
                evidence.admitted.join(","),
                admitted_compositions
                    .iter()
                    .map(|composition| composition.id.clone())
                    .collect::<Vec<String>>()
                    .join(","),
                if commutativity_required {
                    format!(
                        " and commutativity ({})",
                        commutativity.tag()
                    )
                } else if order_sensitive {
                    " under the declared fixed composition; commutativity remains not established".into()
                } else {
                    "; commutativity is not applicable".into()
                }
            ),
        )
    } else {
        (
            ObligationOutcome::NeedsJointEvidence,
            format!(
                "coexistence={}, invariant_preservation={}, commutativity={}",
                coexistence.tag(),
                preservation.tag(),
                commutativity.tag()
            ),
        )
    };

    let mut path = Vec::new();
    let mut step = 0usize;
    for interaction in interactions {
        match interaction.kind {
            InteractionKind::ChangedGuaranteeAffectsAssumption => {
                if let (Some(guarantee), Some(assumption)) =
                    (&interaction.guarantee, &interaction.assumption)
                {
                    if let Some(producer) = admitted.footprints.get(&interaction.producer_change) {
                        if let Some(changed) = producer.guarantees.get(guarantee) {
                            step += 1;
                            path.push(ExplanationStep {
                                step,
                                kind: "guarantee_change".into(),
                                change: Some(interaction.producer_change.clone()),
                                reference: Some(guarantee.clone()),
                                detail: format!(
                                    "guarantee {} in requirement {} changed {} to {}",
                                    guarantee,
                                    changed
                                        .current_requirement
                                        .as_deref()
                                        .or(changed.previous_requirement.as_deref())
                                        .unwrap_or("unknown"),
                                    changed.previous_identity.as_deref().unwrap_or("none"),
                                    changed.current_identity.as_deref().unwrap_or("none")
                                ),
                            });
                        }
                    }
                    if let Some(consumer) = admitted.footprints.get(&interaction.consumer_change) {
                        if let Some(consumed) = consumer
                            .assumptions
                            .iter()
                            .find(|candidate| candidate.id == *assumption)
                        {
                            step += 1;
                            path.push(ExplanationStep {
                                step,
                                kind: "assumption_consumption".into(),
                                change: Some(interaction.consumer_change.clone()),
                                reference: Some(assumption.clone()),
                                detail: format!(
                                    "assumption {} in requirement {} consumes identity {} with {} dependency and meaning \"{}\"",
                                    assumption,
                                    consumed.requirement_id,
                                    consumed.consumed_identity,
                                    consumed.dependency.tag(),
                                    consumed.meaning
                                ),
                            });
                        }
                    }
                }
                if let Some(check) = &interaction.finite {
                    step += 1;
                    path.push(ExplanationStep {
                        step,
                        kind: "finite_check".into(),
                        change: Some(interaction.producer_change.clone()),
                        reference: Some(check.requirement_id.clone()),
                        detail: format!(
                            "{} worlds ({} examined, {} evaluations): previous_implies_assumption={:?}, current_implies_assumption={:?}{}",
                            check.worlds,
                            check.worlds_examined,
                            check.evaluations,
                            check.previous_implies_assumption,
                            check.current_implies_assumption,
                            if check.witness.is_some() {
                                "; a distinguishing observation table is retained"
                            } else {
                                ""
                            }
                        ),
                    });
                }
            }
            InteractionKind::InvalidationAffectsEvidenceBasis => {
                step += 1;
                path.push(ExplanationStep {
                    step,
                    kind: "invalidation".into(),
                    change: Some(interaction.producer_change.clone()),
                    reference: interaction.invalidation.clone(),
                    detail: format!(
                        "invalidation affects evidence basis {} for claims {:?}",
                        interaction.evidence_basis.as_deref().unwrap_or("none"),
                        interaction.affected_claims
                    ),
                });
            }
            InteractionKind::InvalidationUnderminesInvariant => {
                step += 1;
                path.push(ExplanationStep {
                    step,
                    kind: "invalidation".into(),
                    change: Some(interaction.producer_change.clone()),
                    reference: interaction.invariant.clone(),
                    detail: "the invalidation undermines an invariant the other change preserves"
                        .into(),
                });
            }
            InteractionKind::OverlappingInvariantCommitment => {
                step += 1;
                path.push(ExplanationStep {
                    step,
                    kind: "invariant_overlap".into(),
                    change: Some(interaction.producer_change.clone()),
                    reference: interaction.invariant.clone(),
                    detail: format!(
                        "invariant commitments overlap for interfaces {:?}",
                        interaction.interfaces
                    ),
                });
            }
            InteractionKind::InterfaceConfigurationMismatch
            | InteractionKind::InterfaceOperationConflict => {
                step += 1;
                path.push(ExplanationStep {
                    step,
                    kind: "interface_conflict".into(),
                    change: Some(interaction.producer_change.clone()),
                    reference: interaction.interfaces.first().cloned(),
                    detail: interaction.assessment.basis.clone(),
                });
            }
        }
    }
    if !admitted_compositions.is_empty() {
        step += 1;
        path.push(ExplanationStep {
            step,
            kind: "composition".into(),
            change: None,
            reference: None,
            detail: format!(
                "admitted compositions: [{}]",
                admitted_compositions
                    .iter()
                    .map(|composition| format!(
                        "{}:{}:fixed={}",
                        composition.id,
                        composition.order.join(">"),
                        composition.fixed
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        });
    }
    let admitted_evidence = evidence.admitted.clone();
    let rejected_evidence = evidence.rejected.len() + evidence.insufficient.len();
    step += 1;
    path.push(ExplanationStep {
        step,
        kind: "evidence".into(),
        change: None,
        reference: None,
        detail: format!(
            "joint evidence admitted {:?}; rejected or insufficient {} record(s); individual receipts considered {:?} cannot discharge the combined candidate",
            admitted_evidence, rejected_evidence, evidence.individual_receipts_considered
        ),
    });
    step += 1;
    path.push(ExplanationStep {
        step,
        kind: "outcome".into(),
        change: None,
        reference: Some(obligation_id.clone()),
        detail: format!("{}: {}", outcome.tag(), outcome_basis),
    });

    Ok(ObligationDraft {
        id: obligation_id,
        participants,
        candidate,
        interactions: interactions.to_vec(),
        affected_claims: affected_claims.into_iter().collect(),
        affected_interfaces: interfaces.into_iter().collect(),
        configurations,
        affected_invariants: affected_invariants.into_iter().collect(),
        integration_conditions,
        required_evidence,
        order_sensitive,
        commutativity_required,
        propositions: ObligationPropositions {
            coexistence,
            invariant_preservation: preservation,
            commutativity,
        },
        outcome,
        outcome_basis,
        evidence,
        path,
        fingerprint,
    })
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Analyze declared footprints and derive compatibility obligations.
///
/// The result is advisory. It never refreshes receipts, transfers evidence,
/// resolves a native obligation or changes a lifecycle decision.
// r[impl cairn.concurrent_footprints.footprint]
// r[impl cairn.concurrent_footprints.derivation]
// r[impl cairn.concurrent_footprints.obligation]
// r[impl cairn.concurrent_footprints.outcomes]
// r[impl cairn.concurrent_footprints.explanation]
// r[impl cairn.concurrent_footprints.invalidation]
// r[impl cairn.concurrent_footprints.bounds]
pub fn analyze_concurrent_changes(
    policy: &FootprintPolicy,
    case: &ConcurrentChangeCase,
) -> Result<CompatibilityReport, FootprintError> {
    policy.admit()?;
    let admitted = admit_case(policy, case)?;
    let mut gaps = admitted.gaps.clone();
    let mut usage = ExplorationUsage {
        worlds: 0,
        evaluations: 0,
        complete: true,
        bound_exhausted: Vec::new(),
    };
    let pair_interactions = derive_interactions(policy, &case.profile, &admitted, &mut usage);

    let mut obligation_ids = BTreeSet::new();
    let mut obligations = Vec::new();
    for (pair, interactions) in &pair_interactions {
        let draft = build_obligation(
            &admitted,
            (pair.0.as_str(), pair.1.as_str()),
            interactions,
            &mut gaps,
        )?;
        if !obligation_ids.insert(draft.id.clone()) {
            return Err(FootprintError::InvalidInput(format!(
                "derived two obligations with the same identity {}",
                draft.id
            )));
        }
        obligations.push(CompatibilityObligation {
            id: draft.id.clone(),
            participants: draft.participants.clone(),
            candidate: draft.candidate.clone(),
            interaction_ids: draft
                .interactions
                .iter()
                .map(|interaction| interaction.id.clone())
                .collect(),
            context: admitted.context.clone(),
            affected_claims: draft.affected_claims.clone(),
            affected_interfaces: draft.affected_interfaces.clone(),
            configurations: draft.configurations.clone(),
            affected_invariants: draft.affected_invariants.clone(),
            integration_conditions: draft.integration_conditions.clone(),
            required_evidence: draft.required_evidence.clone(),
            propositions: draft.propositions.clone(),
            outcome: draft.outcome,
            outcome_basis: draft.outcome_basis.clone(),
            order_sensitive: draft.order_sensitive,
            commutativity_required: draft.commutativity_required,
            evidence: draft.evidence.clone(),
            path: draft.path.clone(),
            fingerprint: draft.fingerprint.clone(),
        });
    }
    obligations.sort_by(|left, right| left.id.cmp(&right.id));

    let interactions: Vec<DerivedInteraction> = pair_interactions
        .values()
        .flat_map(|interactions| interactions.iter().cloned())
        .collect();
    let mut interactions = interactions;
    interactions.sort_by(|left, right| left.id.cmp(&right.id));

    let mut unmatched_joint_evidence = Vec::new();
    let mutual_obligation_ids: BTreeSet<String> = obligations
        .iter()
        .map(|obligation| obligation.id.clone())
        .collect();
    for evidence in &admitted.joint_evidence {
        if !mutual_obligation_ids.contains(&evidence.obligation) {
            unmatched_joint_evidence.push(EvidenceRejection {
                id: evidence.id.clone(),
                reasons: vec!["unknown_obligation".into()],
            });
        }
    }
    unmatched_joint_evidence.sort_by(|left, right| left.id.cmp(&right.id));

    let mut compositions: Vec<CompositionSummary> = admitted
        .compositions
        .iter()
        .map(|composition| CompositionSummary {
            id: composition.id.clone(),
            participants: composition.participants.clone(),
            order: composition.order.clone(),
            fixed: composition.fixed,
            admitted: composition.admitted,
        })
        .collect();
    compositions.sort_by(|left, right| left.id.cmp(&right.id));

    let mut individual_receipts: Vec<IndividualReceiptStatus> = admitted
        .individual_receipts
        .iter()
        .map(|receipt| {
            let change_known = admitted.footprints.contains_key(&receipt.change);
            let requirement_known =
                admitted
                    .footprints
                    .get(&receipt.change)
                    .is_some_and(|footprint| {
                        footprint.requirements.contains_key(&receipt.requirement_id)
                    });
            IndividualReceiptStatus {
                id: receipt.id.clone(),
                change: receipt.change.clone(),
                claim: receipt.claim.clone(),
                change_known,
                requirement_known,
                discharges_combined_obligation: false,
            }
        })
        .collect();
    individual_receipts.sort_by(|left, right| left.id.cmp(&right.id));

    let mut footprints: Vec<FootprintSummary> = admitted
        .footprints
        .values()
        .map(|footprint| {
            let mut requirements: Vec<BoundRequirementSummary> = footprint
                .requirements
                .iter()
                .map(|(requirement_id, bound)| bound.summary(requirement_id))
                .collect();
            requirements.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
            FootprintSummary {
                change: footprint.change.change_id.clone(),
                source_revision: footprint.change.source_revision.clone(),
                intent_revision: footprint.change.intent_revision.clone(),
                requirements,
                assumptions: footprint.assumptions.len(),
                guarantees: footprint.guarantees.len(),
                invariants: footprint.invariants.len(),
                invalidated: footprint.invalidated.len(),
                interfaces: footprint.interfaces.len(),
                unknown: footprint.unknown.len(),
                complete: footprint.unknown.is_empty()
                    && !gaps
                        .iter()
                        .any(|gap| gap.change == footprint.change.change_id),
            }
        })
        .collect();
    footprints.sort_by(|left, right| left.change.cmp(&right.change));

    let mut scoped_unknown = Vec::new();
    for footprint in admitted.footprints.values() {
        for unknown in &footprint.unknown {
            scoped_unknown.push(ScopedUnknown {
                change: footprint.change.change_id.clone(),
                scope: unknown.scope.clone(),
                kind: unknown.kind,
                detail: unknown.detail.clone(),
            });
        }
    }
    scoped_unknown.sort_by(|left, right| {
        (left.change.clone(), left.scope.clone()).cmp(&(right.change.clone(), right.scope.clone()))
    });
    gaps.sort_by(|left, right| {
        (
            left.change.clone(),
            left.kind.clone(),
            left.reference.clone(),
        )
            .cmp(&(
                right.change.clone(),
                right.kind.clone(),
                right.reference.clone(),
            ))
    });
    usage.complete = usage.bound_exhausted.is_empty();
    let prior_applicability = prior_applicability(&admitted, &obligations);
    let outcomes = outcome_counts(&obligations);
    let declaration_complete = gaps.is_empty() && scoped_unknown.is_empty();
    let mut report = CompatibilityReport {
        schema: REPORT_SCHEMA_V1.into(),
        advisory: true,
        producer: FOOTPRINTS_PRODUCER.into(),
        policy_blake3: admitted.policy_blake3.clone(),
        input_blake3: admitted.input_blake3.clone(),
        report_blake3: String::new(),
        context: admitted.context.clone(),
        footprints,
        declaration_complete,
        gaps,
        interactions,
        obligations,
        outcomes,
        individual_receipts,
        individual_receipts_do_not_discharge: true,
        unmatched_joint_evidence,
        compositions,
        prior_applicability,
        exploration: usage,
        scoped_unknown,
        independent_reports: vec![
            "textual_collaboration_conflict_resolution".into(),
            "verification_impact".into(),
        ],
        boundary: CONCURRENT_FOOTPRINTS_BOUNDARY.into(),
    };
    report.report_blake3.clear();
    let report_blake3 = identity(REPORT_SCHEMA_V1, &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

// r[impl cairn.concurrent_footprints.conformance]
#[cfg(test)]
mod tests;
