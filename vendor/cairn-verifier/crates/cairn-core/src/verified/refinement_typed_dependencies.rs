//! Opt-in refinement arguments and typed assume-guarantee dependencies.
//!
//! A decomposition binds exact parent and child requirement revisions, explicit
//! environment assumptions, selected alternatives, integration obligations and a
//! separately justified feasibility disposition. Typed dependencies keep ordering,
//! verified-specification, verified-candidate, candidate-input, integrated-revision,
//! lifecycle and semantic relations distinct.
//!
//! This module is deliberately not an authority: a supported parent argument is
//! not behavioral requirement satisfaction, a reviewed argument is a retained
//! declaration rather than proof, an integration observation establishes linkage
//! rather than correctness, and no result authorizes a claim, readiness state, gate
//! or lifecycle action. Native requirement rows, task ordering, archive
//! dependencies and cross-repository v1 semantics retain their meanings.

use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use super::scoped_claim_derivations::{
    evaluate_derivations, ClaimContext, ClaimPolicy, ClaimState,
    EvaluationRequest as ScopedEvaluationRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const REFINEMENT_SCHEMA_V1: &str = "cairn.refinement-decomposition.v1";
pub const DEPENDENCY_SCHEMA_V1: &str = "cairn.typed-dependency.v1";
pub const REFINEMENT_POLICY_SCHEMA_V1: &str = "cairn.refinement-policy.v1";
pub const REFINEMENT_REPORT_SCHEMA_V1: &str = "cairn.refinement-argument-report.v1";
// r[impl cairn.refinement.compatibility]
pub const REFINEMENT_BOUNDARY: &str = "Advisory composition and dependency explanation only: a supported parent argument is not behavioral requirement satisfaction, a reviewed argument or retained witness is a declaration rather than proof, an integration observation establishes presented linkage rather than integration correctness, and no result authorizes a claim, readiness state, verification role, gate verdict or lifecycle action. Native requirement rows, task ordering, archive dependencies, cross-repository v1 semantics and the Pass | Block gate retain authority.";

const MAX_DECOMPOSITIONS_CEILING: usize = 256;
const MAX_REQUIREMENTS_CEILING: usize = 1024;
const MAX_ASSUMPTIONS_CEILING: usize = 512;
const MAX_OBLIGATIONS_CEILING: usize = 2048;
const MAX_ALTERNATIVES_CEILING: usize = 128;
const MAX_DEPENDENCIES_CEILING: usize = 1024;
const MAX_EVIDENCE_CEILING: usize = 4096;
const MAX_GUARANTEE_CASES_CEILING: usize = 256;
const MAX_DEPTH_CEILING: usize = 64;

/// Four establishing distinctions. `Ord` is weakest-to-strongest, so the
/// weakest applicable assurance is the composition's assurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assurance {
    Unassessed,
    Reviewed,
    BoundedCheck,
    CheckedProof,
}

/// Deterministic obligation outcome vocabulary shared by decompositions,
/// premises and dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationState {
    Pending,
    Supported,
    Unsupported,
    Infeasible,
    Contradicted,
}

impl ObligationState {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Infeasible => "infeasible",
            Self::Contradicted => "contradicted",
        }
    }

    fn severity(&self) -> u8 {
        match self {
            Self::Supported => 0,
            Self::Pending => 1,
            Self::Unsupported => 2,
            Self::Infeasible => 3,
            Self::Contradicted => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feasibility {
    Feasible,
    Infeasible,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRevision {
    pub requirement: ContentBoundRequirementRow,
    pub interpretation_blake3: String,
    pub contract_blake3: String,
}

/// Exact composition scope. Every component is mandatory and all child
/// guarantee evidence must carry the identical scope; v1 admits no widening.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgumentScope {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
    pub bounds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuaranteeCase {
    pub id: String,
    pub policy: ClaimPolicy,
    pub case: ScopedEvaluationRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildGuarantee {
    pub id: String,
    pub requirement: RequirementRevision,
    pub scope: ArgumentScope,
    pub guarantee_case: String,
    pub claim_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssumptionAtom {
    pub predicate: String,
    pub asserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentAssumption {
    pub id: String,
    pub statement: String,
    pub atom: AssumptionAtom,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationObligation {
    pub id: String,
    pub statement: String,
    pub required: bool,
    pub scope: ArgumentScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alternative {
    pub id: String,
    pub obligations: Vec<Composition>,
}

/// Finite composition vocabulary. `AnyOf` names one selected complete
/// alternative; evidence from unselected alternatives is never combined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Composition {
    Child {
        child: String,
    },
    AllOf {
        obligations: Vec<Composition>,
    },
    AnyOf {
        alternatives: Vec<Alternative>,
        selected: String,
    },
    Conditional {
        condition: String,
        when_satisfied: Vec<Composition>,
        when_unsatisfied: Vec<Composition>,
    },
}

/// The retained argument relation. A witness is a declaration artifact, not a
/// proof; `CheckedProof` additionally requires a policy-admitted checker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplicationRelation {
    pub assurance: Assurance,
    pub witness_blake3: String,
    pub statement: String,
    pub proof_checker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeasibilityDisposition {
    pub disposition: Feasibility,
    pub method: String,
    pub assurance: Assurance,
    pub justification_blake3: String,
    pub covered: BTreeSet<String>,
    pub proof_checker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decomposition {
    pub schema: String,
    pub id: String,
    pub parent: RequirementRevision,
    pub scope: ArgumentScope,
    pub children: Vec<ChildGuarantee>,
    pub environment: Vec<EnvironmentAssumption>,
    pub integration: Vec<IntegrationObligation>,
    pub composition: Composition,
    pub relation: ImplicationRelation,
    pub feasibility: FeasibilityDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootAssumption {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyRelation {
    OrderingArchive,
    VerifiedSpecification,
    VerifiedCandidate,
    CandidateInput,
    IntegratedRevision,
    Lifecycle,
    Semantic,
}

impl DependencyRelation {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::OrderingArchive => "ordering_archive",
            Self::VerifiedSpecification => "verified_specification",
            Self::VerifiedCandidate => "verified_candidate",
            Self::CandidateInput => "candidate_input",
            Self::IntegratedRevision => "integrated_revision",
            Self::Lifecycle => "lifecycle",
            Self::Semantic => "semantic",
        }
    }
}

/// Whole-change versus selected-task coverage. A first selected task is not a
/// substitute for a whole-change prerequisite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Coverage {
    WholeChange,
    SelectedTasks { tasks: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceCoverage {
    WholeChange,
    Partial { tasks: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyEndpoint {
    pub requirement: RequirementRevision,
    pub work: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyContext {
    pub producer_repository: String,
    pub consumer_repository: String,
    pub intended_target: String,
    pub environment: String,
    pub policy_blake3: String,
    pub bounds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssumptionRef {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuaranteeRef {
    pub guarantee_case: String,
    pub claim_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticWitness {
    pub assurance: Assurance,
    pub artifact_blake3: String,
    pub rationale: String,
    pub guarantee_statement_blake3: String,
    pub assumption_statement_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticBinding {
    pub producer_guarantee: GuaranteeRef,
    pub consumer_assumption: AssumptionRef,
    pub producer_scope: ArgumentScope,
    pub consumer_scope: ArgumentScope,
    pub witness: SemanticWitness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedDependency {
    pub schema: String,
    pub id: String,
    pub relation: DependencyRelation,
    pub producer: DependencyEndpoint,
    pub consumer: DependencyEndpoint,
    pub context: DependencyContext,
    pub coverage: Coverage,
    pub required_gates: Vec<String>,
    pub required_input_blake3: Option<String>,
    pub required_lifecycle_stage: Option<String>,
    pub semantic: Option<SemanticBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence", rename_all = "snake_case", deny_unknown_fields)]
pub enum DependencyEvidence {
    Archive {
        dependency_id: String,
        change: String,
        repository: String,
        revision: String,
        archive_receipt_blake3: String,
        coverage: EvidenceCoverage,
    },
    Specification {
        dependency_id: String,
        repository: String,
        revision: String,
        gates: Vec<String>,
        receipt_blake3: String,
    },
    CandidateVerified {
        dependency_id: String,
        repository: String,
        candidate: String,
        revision: String,
        verification_receipt_blake3: String,
    },
    CandidateInput {
        dependency_id: String,
        repository: String,
        input_blake3: String,
        revision: String,
    },
    Integration {
        dependency_id: String,
        repository: String,
        target: String,
        change: String,
        candidate: String,
        revision: String,
        observation_blake3: String,
    },
    Lifecycle {
        dependency_id: String,
        repository: String,
        stage: String,
        revision: String,
        receipt_blake3: String,
    },
}

impl DependencyEvidence {
    fn dependency_id(&self) -> &str {
        match self {
            Self::Archive { dependency_id, .. }
            | Self::Specification { dependency_id, .. }
            | Self::CandidateVerified { dependency_id, .. }
            | Self::CandidateInput { dependency_id, .. }
            | Self::Integration { dependency_id, .. }
            | Self::Lifecycle { dependency_id, .. } => dependency_id,
        }
    }

    fn kind_tag(&self) -> &'static str {
        match self {
            Self::Archive { .. } => "archive",
            Self::Specification { .. } => "specification",
            Self::CandidateVerified { .. } => "candidate_verified",
            Self::CandidateInput { .. } => "candidate_input",
            Self::Integration { .. } => "integration",
            Self::Lifecycle { .. } => "lifecycle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationEvidence {
    pub obligation_id: String,
    pub statement: String,
    pub scope: ArgumentScope,
    pub observation_blake3: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicabilityObservation {
    pub condition: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefinementBounds {
    pub max_decompositions: usize,
    pub max_requirements: usize,
    pub max_assumptions: usize,
    pub max_obligations: usize,
    pub max_alternatives: usize,
    pub max_dependencies: usize,
    pub max_evidence: usize,
    pub max_guarantee_cases: usize,
    pub max_depth: usize,
}

/// Nickel owns this vocabulary and every bound. Core keeps only implementation
/// ceilings and rejects anything outside them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefinementPolicy {
    pub schema: String,
    pub relations: Vec<DependencyRelation>,
    pub assurances: Vec<Assurance>,
    pub feasibility_methods: Vec<String>,
    pub root_assumptions: Vec<RootAssumption>,
    pub proof_checkers: Vec<String>,
    pub bounds: RefinementBounds,
}

/// Stable identity material for reevaluation. Requirement, policy, assumption
/// and context changes are visible without replaying the whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBasis {
    pub policy_blake3: String,
    pub requirement_rows: BTreeSet<String>,
    pub assumption_statements: BTreeSet<String>,
    pub contexts: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefinementRequest {
    pub decompositions: Vec<Decomposition>,
    pub guarantee_cases: Vec<GuaranteeCase>,
    pub dependencies: Vec<TypedDependency>,
    pub dependency_evidence: Vec<DependencyEvidence>,
    pub integration_evidence: Vec<IntegrationEvidence>,
    pub applicability: Vec<ApplicabilityObservation>,
    pub previous_basis: Option<EvaluationBasis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationKind {
    Child,
    EnvironmentAssumption,
    IntegrationObligation,
}

impl ObligationKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Child => "child",
            Self::EnvironmentAssumption => "environment_assumption",
            Self::IntegrationObligation => "integration_obligation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionStatus {
    Discharged,
    PolicyAccepted,
    Residual,
    Pending,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationExplanation {
    pub id: String,
    pub kind: ObligationKind,
    pub state: ObligationState,
    pub assurance: Assurance,
    pub statement_blake3: String,
    pub source: String,
    pub reason: String,
    pub discharge_paths: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeasibilityExplanation {
    pub declared: Feasibility,
    pub state: ObligationState,
    pub assurance: Assurance,
    pub method: String,
    pub covered: BTreeSet<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecompositionExplanation {
    pub id: String,
    pub parent: RequirementRevision,
    pub state: ObligationState,
    pub assurance: Assurance,
    pub conditional: bool,
    pub reason: String,
    pub obligations: Vec<ObligationExplanation>,
    pub feasibility: FeasibilityExplanation,
    pub selected_alternatives: BTreeMap<String, String>,
    pub residual_assumption_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyExplanation {
    pub id: String,
    pub relation: DependencyRelation,
    pub state: ObligationState,
    pub assurance: Assurance,
    pub reason: String,
    pub evidence: Option<String>,
    pub producer_row_blake3: String,
    pub consumer_row_blake3: String,
    pub residual_assumption_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidualAssumptionExplanation {
    pub id: String,
    pub statement_blake3: String,
    pub source: String,
    pub status: AssumptionStatus,
    pub discharge_paths: Vec<Vec<String>>,
    pub referenced_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefinementReport {
    pub schema: String,
    pub advisory: bool,
    pub decompositions: Vec<DecompositionExplanation>,
    pub dependencies: Vec<DependencyExplanation>,
    pub residual_assumptions: Vec<ResidualAssumptionExplanation>,
    pub unresolved: BTreeSet<String>,
    pub reevaluation_triggers: BTreeSet<String>,
    pub basis: EvaluationBasis,
    pub trust_dependencies: BTreeSet<String>,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum RefinementError {
    Unsupported(String),
    ScopeMismatch(String),
    InvalidInput(String),
    BoundExhaustion(String),
    Cycle(String),
}

impl RefinementError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::ScopeMismatch(_) => "scope_mismatch",
            Self::InvalidInput(_) => "invalid_input",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::Cycle(_) => "cycle",
        }
    }
}

impl std::fmt::Display for RefinementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for RefinementError {}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, RefinementError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RefinementError::InvalidInput(error.to_string()))?;
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

fn require_digest(value: &str, field: &str) -> Result<(), RefinementError> {
    if digest(value) {
        Ok(())
    } else {
        Err(RefinementError::InvalidInput(format!(
            "{field} must be a lowercase 64-character BLAKE3 identity"
        )))
    }
}

fn require_text(value: &str, field: &str) -> Result<(), RefinementError> {
    if value.trim().is_empty() {
        Err(RefinementError::InvalidInput(format!(
            "{field} must be non-empty"
        )))
    } else {
        Ok(())
    }
}

/// Statement identity a semantic witness must bind for an admitted child or
/// producer guarantee. The proposition and exact requirement row are the whole
/// guarantee; descriptive prose confers no authority.
pub fn guarantee_statement_identity(
    requirement_row_blake3: &str,
    proposition: &str,
) -> Result<String, RefinementError> {
    require_digest(requirement_row_blake3, "requirement row")?;
    require_text(proposition, "guarantee proposition")?;
    identity(
        "cairn.refinement-guarantee-statement.v1",
        &(requirement_row_blake3, proposition),
    )
}

/// Statement identity for a declared environment assumption.
pub fn assumption_statement_identity(id: &str, statement: &str) -> Result<String, RefinementError> {
    require_text(id, "assumption id")?;
    require_text(statement, "assumption statement")?;
    identity("cairn.refinement-assumption-statement.v1", &(id, statement))
}

/// Meaning and authoritative evidence source for every supported relation.
/// This is a documentation projection only; it admits nothing.
#[must_use]
pub fn dependency_relation_contract() -> serde_json::Value {
    serde_json::json!({
        "schema": DEPENDENCY_SCHEMA_V1,
        "relations": {
            "ordering_archive": {
                "meaning": "the producer whole change or selected tasks are ordered before the consumer through an admitted archive receipt",
                "evidence": "archive receipt binding producer change, repository, exact revision and whole-change or selected-task coverage",
                "non_claims": ["an archive receipt proves ordering and retained coverage only; it does not prove producer correctness or consumer integration"]
            },
            "verified_specification": {
                "meaning": "the producer specification revision is verified for the consumer without requiring archival",
                "evidence": "specification gate receipt binding repository, exact revision and every required gate",
                "non_claims": ["verified specification evidence is not source implementation, archive ordering or integration evidence"]
            },
            "verified_candidate": {
                "meaning": "the exact producer candidate revision was verified",
                "evidence": "candidate verification receipt binding repository, candidate, exact revision",
                "non_claims": ["candidate verification is not integration, worker success or checked task status"]
            },
            "candidate_input": {
                "meaning": "the consumer requires the exact producer candidate input identity",
                "evidence": "candidate input receipt binding repository, required input identity and exact revision",
                "non_claims": ["candidate input identity is not verification, integration or completeness of the producer change"]
            },
            "integrated_revision": {
                "meaning": "the exact producer candidate revision was observed integrated into the consumer's intended target",
                "evidence": "admitted integration observation binding producer change, candidate, revision, repository and intended target",
                "non_claims": ["integration linkage is not integration correctness, release eligibility or task completion"]
            },
            "lifecycle": {
                "meaning": "one named producer lifecycle stage receipt precedes the consumer stage",
                "evidence": "lifecycle receipt binding repository, exact revision and required stage",
                "non_claims": ["a lifecycle receipt proves the named stage linkage only"]
            },
            "semantic": {
                "meaning": "an admitted producer guarantee discharges one consumer assumption under a compatible context and retained implication/correspondence witness",
                "evidence": "supported admitted guarantee plus witness binding both exact statement identities; names or receipt identities alone never discharge",
                "non_claims": ["a semantic witness is a declaration; it does not prove behavioral correctness, refinement faithfulness or proof"]
            }
        }
    })
}

fn validate_requirement(revision: &RequirementRevision) -> Result<(), RefinementError> {
    let row = &revision.requirement;
    if row.requirement_id.trim().is_empty()
        || row.specification_path.trim().is_empty()
        || row.source_span.start_byte > row.source_span.end_byte
        || row.source_span.start_line == 0
        || row.source_span.end_line < row.source_span.start_line
    {
        return Err(RefinementError::InvalidInput(
            "exact native requirement row identity is required".into(),
        ));
    }
    require_digest(&row.specification_blake3, "specification_blake3")?;
    require_digest(&row.requirement_body_blake3, "requirement_body_blake3")?;
    require_digest(&row.row_blake3, "row_blake3")?;
    require_digest(
        &revision.interpretation_blake3,
        "requirement interpretation_blake3",
    )?;
    require_digest(&revision.contract_blake3, "requirement contract_blake3")
}

fn validate_scope(scope: &ArgumentScope) -> Result<(), RefinementError> {
    require_digest(&scope.subject_blake3, "scope subject_blake3")?;
    require_digest(&scope.policy_blake3, "scope policy_blake3")?;
    require_text(&scope.target, "scope target")?;
    require_text(&scope.environment, "scope environment")?;
    if scope.bounds.is_empty()
        || scope
            .bounds
            .iter()
            .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
    {
        return Err(RefinementError::InvalidInput(
            "scope requires non-empty explicit bounds".into(),
        ));
    }
    Ok(())
}

fn validate_dependency_context(context: &DependencyContext) -> Result<(), RefinementError> {
    require_text(&context.producer_repository, "producer repository")?;
    require_text(&context.consumer_repository, "consumer repository")?;
    require_text(&context.intended_target, "intended integration target")?;
    require_text(&context.environment, "dependency environment")?;
    require_digest(&context.policy_blake3, "dependency policy_blake3")?;
    if context.bounds.is_empty()
        || context
            .bounds
            .iter()
            .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
    {
        return Err(RefinementError::InvalidInput(
            "dependency requires non-empty explicit bounds".into(),
        ));
    }
    Ok(())
}

fn validate_tasks(tasks: &[String], field: &str) -> Result<(), RefinementError> {
    if tasks.iter().any(|task| task.trim().is_empty()) {
        return Err(RefinementError::InvalidInput(format!(
            "{field} must contain non-empty task identities"
        )));
    }
    let unique = tasks.iter().collect::<BTreeSet<_>>();
    if unique.len() != tasks.len() {
        return Err(RefinementError::InvalidInput(format!(
            "{field} repeats a task identity"
        )));
    }
    Ok(())
}

pub fn admit_refinement_policy(policy: &RefinementPolicy) -> Result<(), RefinementError> {
    // r[impl cairn.refinement.dependencies]
    if policy.schema != REFINEMENT_POLICY_SCHEMA_V1 {
        return Err(RefinementError::Unsupported(format!(
            "unsupported refinement policy schema {}",
            policy.schema
        )));
    }
    let bounds = &policy.bounds;
    if bounds.max_decompositions == 0
        || bounds.max_decompositions > MAX_DECOMPOSITIONS_CEILING
        || bounds.max_requirements == 0
        || bounds.max_requirements > MAX_REQUIREMENTS_CEILING
        || bounds.max_assumptions == 0
        || bounds.max_assumptions > MAX_ASSUMPTIONS_CEILING
        || bounds.max_obligations == 0
        || bounds.max_obligations > MAX_OBLIGATIONS_CEILING
        || bounds.max_alternatives == 0
        || bounds.max_alternatives > MAX_ALTERNATIVES_CEILING
        || bounds.max_dependencies == 0
        || bounds.max_dependencies > MAX_DEPENDENCIES_CEILING
        || bounds.max_evidence == 0
        || bounds.max_evidence > MAX_EVIDENCE_CEILING
        || bounds.max_guarantee_cases == 0
        || bounds.max_guarantee_cases > MAX_GUARANTEE_CASES_CEILING
        || bounds.max_depth == 0
        || bounds.max_depth > MAX_DEPTH_CEILING
    {
        return Err(RefinementError::BoundExhaustion(
            "admitted policy bounds are outside supported implementation ceilings".into(),
        ));
    }
    if policy.relations.is_empty() {
        return Err(RefinementError::Unsupported(
            "policy admits no dependency relations".into(),
        ));
    }
    let mut relations = BTreeSet::new();
    for relation in &policy.relations {
        if !relations.insert(*relation) {
            return Err(RefinementError::Unsupported(format!(
                "duplicate admitted relation {}",
                relation.tag()
            )));
        }
    }
    if policy.assurances.is_empty() {
        return Err(RefinementError::Unsupported(
            "policy admits no establishing assurance".into(),
        ));
    }
    let mut assurances = BTreeSet::new();
    for assurance in &policy.assurances {
        if *assurance == Assurance::Unassessed {
            return Err(RefinementError::Unsupported(
                "unassessed cannot be admitted as an establishing assurance".into(),
            ));
        }
        if !assurances.insert(*assurance) {
            return Err(RefinementError::Unsupported(format!(
                "duplicate admitted assurance {assurance:?}"
            )));
        }
    }
    if policy.feasibility_methods.is_empty() {
        return Err(RefinementError::Unsupported(
            "policy admits no feasibility method".into(),
        ));
    }
    let mut methods = BTreeSet::new();
    for method in &policy.feasibility_methods {
        require_text(method, "feasibility method")?;
        if !methods.insert(method.as_str()) {
            return Err(RefinementError::Unsupported(format!(
                "duplicate feasibility method {method}"
            )));
        }
    }
    let mut roots = BTreeSet::new();
    for root in &policy.root_assumptions {
        require_text(&root.id, "root assumption id")?;
        require_text(&root.statement, "root assumption statement")?;
        if !roots.insert((root.id.as_str(), root.statement.as_str())) {
            return Err(RefinementError::Unsupported(format!(
                "duplicate root assumption {}",
                root.id
            )));
        }
    }
    let mut checkers = BTreeSet::new();
    for checker in &policy.proof_checkers {
        require_text(checker, "proof checker")?;
        if !checkers.insert(checker.as_str()) {
            return Err(RefinementError::Unsupported(format!(
                "duplicate proof checker {checker}"
            )));
        }
    }
    Ok(())
}

fn composition_nodes(
    composition: &Composition,
    depth: usize,
    nodes: &mut usize,
    alternatives: &mut usize,
    max_depth: usize,
) -> Result<(), RefinementError> {
    *nodes += 1;
    if depth > max_depth {
        return Err(RefinementError::BoundExhaustion(
            "composition depth exceeds the admitted bound".into(),
        ));
    }
    match composition {
        Composition::Child { .. } => Ok(()),
        Composition::AllOf { obligations } => {
            if obligations.is_empty() {
                return Err(RefinementError::InvalidInput(
                    "all_of requires at least one obligation".into(),
                ));
            }
            for obligation in obligations {
                composition_nodes(obligation, depth + 1, nodes, alternatives, max_depth)?;
            }
            Ok(())
        }
        Composition::AnyOf {
            alternatives: options,
            selected,
        } => {
            if options.is_empty() {
                return Err(RefinementError::InvalidInput(
                    "any_of requires at least one alternative".into(),
                ));
            }
            if !options.iter().any(|option| option.id == *selected) {
                return Err(RefinementError::InvalidInput(format!(
                    "any_of selects unknown alternative {selected}"
                )));
            }
            let mut ids = BTreeSet::new();
            for option in options {
                require_text(&option.id, "alternative id")?;
                if !ids.insert(option.id.as_str()) {
                    return Err(RefinementError::InvalidInput(
                        "any_of repeats an alternative id".into(),
                    ));
                }
                if option.obligations.is_empty() {
                    return Err(RefinementError::InvalidInput(
                        "alternative requires at least one obligation".into(),
                    ));
                }
                *alternatives += 1;
                for obligation in &option.obligations {
                    composition_nodes(obligation, depth + 1, nodes, alternatives, max_depth)?;
                }
            }
            Ok(())
        }
        Composition::Conditional {
            condition,
            when_satisfied,
            when_unsatisfied,
        } => {
            require_text(condition, "conditional applicability")?;
            if when_satisfied.is_empty() || when_unsatisfied.is_empty() {
                return Err(RefinementError::InvalidInput(
                    "conditional branches must both contain an obligation".into(),
                ));
            }
            for obligation in when_satisfied.iter().chain(when_unsatisfied) {
                composition_nodes(obligation, depth + 1, nodes, alternatives, max_depth)?;
            }
            Ok(())
        }
    }
}

fn validate_relation(
    relation: &ImplicationRelation,
    policy: &RefinementPolicy,
) -> Result<(), RefinementError> {
    require_text(&relation.statement, "implication statement")?;
    if relation.assurance != Assurance::Unassessed {
        require_digest(&relation.witness_blake3, "implication witness_blake3")?;
    }
    if relation.assurance == Assurance::CheckedProof {
        let Some(checker) = relation.proof_checker.as_deref() else {
            return Err(RefinementError::Unsupported(
                "checked proof requires a named proof checker".into(),
            ));
        };
        require_text(checker, "proof checker")?;
        if !policy.proof_checkers.iter().any(|known| known == checker) {
            return Err(RefinementError::Unsupported(format!(
                "proof checker {checker} is not admitted by policy"
            )));
        }
    }
    Ok(())
}

fn validate_feasibility(
    feasibility: &FeasibilityDisposition,
    policy: &RefinementPolicy,
    known: &BTreeSet<String>,
) -> Result<(), RefinementError> {
    require_text(&feasibility.method, "feasibility method")?;
    require_digest(
        &feasibility.justification_blake3,
        "feasibility justification_blake3",
    )?;
    if !feasibility.covered.is_subset(known) {
        return Err(RefinementError::InvalidInput(
            "feasibility coverage names an undeclared assumption or child".into(),
        ));
    }
    if feasibility.assurance == Assurance::CheckedProof {
        let Some(checker) = feasibility.proof_checker.as_deref() else {
            return Err(RefinementError::Unsupported(
                "checked feasibility requires a named proof checker".into(),
            ));
        };
        if !policy.proof_checkers.iter().any(|known| known == checker) {
            return Err(RefinementError::Unsupported(format!(
                "proof checker {checker} is not admitted by policy"
            )));
        }
    }
    Ok(())
}

fn validate_decomposition(
    decomposition: &Decomposition,
    policy: &RefinementPolicy,
    bounds: &RefinementBounds,
) -> Result<(), RefinementError> {
    // r[impl cairn.refinement.decomposition]
    if decomposition.schema != REFINEMENT_SCHEMA_V1 {
        return Err(RefinementError::Unsupported(format!(
            "unsupported decomposition schema {}",
            decomposition.schema
        )));
    }
    require_text(&decomposition.id, "decomposition id")?;
    validate_requirement(&decomposition.parent)?;
    validate_scope(&decomposition.scope)?;
    let mut child_ids = BTreeSet::new();
    for child in &decomposition.children {
        require_text(&child.id, "child id")?;
        if !child_ids.insert(child.id.as_str()) {
            return Err(RefinementError::InvalidInput(format!(
                "decomposition {} repeats child {}",
                decomposition.id, child.id
            )));
        }
        validate_requirement(&child.requirement)?;
        validate_scope(&child.scope)?;
        if child.scope != decomposition.scope {
            return Err(RefinementError::ScopeMismatch(format!(
                "child {} scope differs from its decomposition scope",
                child.id
            )));
        }
        require_text(&child.guarantee_case, "child guarantee case")?;
        require_text(&child.claim_id, "child guarantee claim")?;
    }
    let mut assumption_ids = BTreeSet::new();
    for assumption in &decomposition.environment {
        require_text(&assumption.id, "assumption id")?;
        require_text(&assumption.statement, "assumption statement")?;
        require_text(&assumption.atom.predicate, "assumption predicate")?;
        require_text(&assumption.source, "assumption source")?;
        if !assumption_ids.insert(assumption.id.as_str()) {
            return Err(RefinementError::InvalidInput(format!(
                "decomposition {} repeats assumption {}",
                decomposition.id, assumption.id
            )));
        }
    }
    let mut obligation_ids = BTreeSet::new();
    for obligation in &decomposition.integration {
        require_text(&obligation.id, "integration obligation id")?;
        require_text(&obligation.statement, "integration obligation statement")?;
        if obligation.scope != decomposition.scope {
            return Err(RefinementError::ScopeMismatch(format!(
                "integration obligation {} scope differs from its decomposition scope",
                obligation.id
            )));
        }
        if !obligation_ids.insert(obligation.id.as_str()) {
            return Err(RefinementError::InvalidInput(format!(
                "decomposition {} repeats integration obligation {}",
                decomposition.id, obligation.id
            )));
        }
    }
    let mut nodes = 0;
    let mut alternatives = 0;
    composition_nodes(
        &decomposition.composition,
        0,
        &mut nodes,
        &mut alternatives,
        bounds.max_depth,
    )?;
    if nodes > bounds.max_obligations {
        return Err(RefinementError::BoundExhaustion(format!(
            "decomposition {} exceeds the admitted obligation bound",
            decomposition.id
        )));
    }
    if alternatives > bounds.max_alternatives {
        return Err(RefinementError::BoundExhaustion(format!(
            "decomposition {} exceeds the admitted alternative bound",
            decomposition.id
        )));
    }
    assert_children_declared(&decomposition.composition, &child_ids, &decomposition.id)?;
    validate_relation(&decomposition.relation, policy)?;
    let mut known: BTreeSet<String> = child_ids.iter().map(|id| (*id).to_string()).collect();
    known.extend(assumption_ids.iter().map(|id| (*id).to_string()));
    validate_feasibility(&decomposition.feasibility, policy, &known)
}

fn assert_children_declared(
    composition: &Composition,
    declared: &BTreeSet<&str>,
    decomposition_id: &str,
) -> Result<(), RefinementError> {
    match composition {
        Composition::Child { child } => {
            if !declared.contains(child.as_str()) {
                return Err(RefinementError::InvalidInput(format!(
                    "decomposition {decomposition_id} composes undeclared child {child}"
                )));
            }
            Ok(())
        }
        Composition::AllOf { obligations } => {
            for obligation in obligations {
                assert_children_declared(obligation, declared, decomposition_id)?;
            }
            Ok(())
        }
        Composition::AnyOf { alternatives, .. } => {
            for alternative in alternatives {
                for obligation in &alternative.obligations {
                    assert_children_declared(obligation, declared, decomposition_id)?;
                }
            }
            Ok(())
        }
        Composition::Conditional {
            when_satisfied,
            when_unsatisfied,
            ..
        } => {
            for obligation in when_satisfied.iter().chain(when_unsatisfied) {
                assert_children_declared(obligation, declared, decomposition_id)?;
            }
            Ok(())
        }
    }
}

fn validate_dependency(dependency: &TypedDependency) -> Result<(), RefinementError> {
    // r[impl cairn.refinement.dependencies]
    if dependency.schema != DEPENDENCY_SCHEMA_V1 {
        return Err(RefinementError::Unsupported(format!(
            "unsupported dependency schema {}",
            dependency.schema
        )));
    }
    require_text(&dependency.id, "dependency id")?;
    validate_requirement(&dependency.producer.requirement)?;
    validate_requirement(&dependency.consumer.requirement)?;
    require_text(&dependency.producer.work, "producer work identity")?;
    require_text(&dependency.consumer.work, "consumer work identity")?;
    require_text(&dependency.producer.revision, "producer revision")?;
    require_text(&dependency.consumer.revision, "consumer revision")?;
    validate_dependency_context(&dependency.context)?;
    match &dependency.coverage {
        Coverage::WholeChange => {}
        Coverage::SelectedTasks { tasks } => {
            validate_tasks(tasks, "selected dependency tasks")?;
        }
    }
    if dependency.relation == DependencyRelation::VerifiedSpecification {
        validate_tasks(&dependency.required_gates, "required specification gates")?;
        if dependency.required_gates.is_empty() {
            return Err(RefinementError::InvalidInput(
                "verified_specification requires at least one gate".into(),
            ));
        }
    }
    if dependency.relation == DependencyRelation::CandidateInput {
        let Some(input) = dependency.required_input_blake3.as_deref() else {
            return Err(RefinementError::InvalidInput(
                "candidate_input requires the exact input identity".into(),
            ));
        };
        require_digest(input, "required candidate input")?;
    }
    if dependency.relation == DependencyRelation::Lifecycle {
        let Some(stage) = dependency.required_lifecycle_stage.as_deref() else {
            return Err(RefinementError::InvalidInput(
                "lifecycle dependency requires a stage".into(),
            ));
        };
        require_text(stage, "required lifecycle stage")?;
    }
    match (&dependency.relation, &dependency.semantic) {
        (DependencyRelation::Semantic, Some(binding)) => {
            require_text(&binding.producer_guarantee.guarantee_case, "guarantee case")?;
            require_text(&binding.producer_guarantee.claim_id, "guarantee claim")?;
            require_text(&binding.consumer_assumption.id, "consumer assumption id")?;
            require_text(
                &binding.consumer_assumption.statement,
                "consumer assumption statement",
            )?;
            validate_scope(&binding.producer_scope)?;
            validate_scope(&binding.consumer_scope)?;
            require_text(&binding.witness.rationale, "semantic witness rationale")?;
            require_digest(
                &binding.witness.artifact_blake3,
                "semantic witness artifact",
            )?;
            require_digest(
                &binding.witness.guarantee_statement_blake3,
                "witness guarantee statement",
            )?;
            require_digest(
                &binding.witness.assumption_statement_blake3,
                "witness assumption statement",
            )?;
        }
        (DependencyRelation::Semantic, None) => {}
        (_, Some(_)) => {
            return Err(RefinementError::InvalidInput(
                "only semantic dependencies may declare a semantic binding".into(),
            ));
        }
        (_, None) => {}
    }
    Ok(())
}

fn validate_dependency_evidence(evidence: &DependencyEvidence) -> Result<(), RefinementError> {
    match evidence {
        DependencyEvidence::Archive {
            dependency_id,
            change,
            repository,
            revision,
            archive_receipt_blake3,
            coverage,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(change, "archive change")?;
            require_text(repository, "archive repository")?;
            require_text(revision, "archive revision")?;
            require_digest(archive_receipt_blake3, "archive receipt")?;
            if let EvidenceCoverage::Partial { tasks } = coverage {
                validate_tasks(tasks, "archive coverage tasks")?;
                if tasks.is_empty() {
                    return Err(RefinementError::InvalidInput(
                        "partial archive coverage requires at least one task".into(),
                    ));
                }
            }
        }
        DependencyEvidence::Specification {
            dependency_id,
            repository,
            revision,
            gates,
            receipt_blake3,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(repository, "specification repository")?;
            require_text(revision, "specification revision")?;
            validate_tasks(gates, "specification gates")?;
            require_digest(receipt_blake3, "specification receipt")?;
        }
        DependencyEvidence::CandidateVerified {
            dependency_id,
            repository,
            candidate,
            revision,
            verification_receipt_blake3,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(repository, "candidate repository")?;
            require_text(candidate, "candidate identity")?;
            require_text(revision, "candidate revision")?;
            require_digest(verification_receipt_blake3, "candidate receipt")?;
        }
        DependencyEvidence::CandidateInput {
            dependency_id,
            repository,
            input_blake3,
            revision,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(repository, "input repository")?;
            require_digest(input_blake3, "candidate input")?;
            require_text(revision, "input revision")?;
        }
        DependencyEvidence::Integration {
            dependency_id,
            repository,
            target,
            change,
            candidate,
            revision,
            observation_blake3,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(repository, "integration repository")?;
            require_text(target, "integration target")?;
            require_text(change, "integration change")?;
            require_text(candidate, "integration candidate")?;
            require_text(revision, "integration revision")?;
            require_digest(observation_blake3, "integration observation")?;
        }
        DependencyEvidence::Lifecycle {
            dependency_id,
            repository,
            stage,
            revision,
            receipt_blake3,
        } => {
            require_text(dependency_id, "evidence dependency id")?;
            require_text(repository, "lifecycle repository")?;
            require_text(stage, "lifecycle stage")?;
            require_text(revision, "lifecycle revision")?;
            require_digest(receipt_blake3, "lifecycle receipt")?;
        }
    }
    Ok(())
}

fn coverage_satisfied(required: &Coverage, provided: &EvidenceCoverage) -> bool {
    match required {
        Coverage::WholeChange => matches!(provided, EvidenceCoverage::WholeChange),
        Coverage::SelectedTasks { tasks } => match provided {
            EvidenceCoverage::WholeChange => true,
            EvidenceCoverage::Partial { tasks: covered } => {
                tasks.iter().all(|task| covered.contains(task))
            }
        },
    }
}

fn scope_identity(scope: &ArgumentScope) -> Result<String, RefinementError> {
    identity("cairn.refinement-scope.v1", scope)
}

fn context_identity(context: &DependencyContext) -> Result<String, RefinementError> {
    identity("cairn.refinement-dependency-context.v1", context)
}

fn assess_acyclic(
    rows: &BTreeSet<String>,
    edges: &BTreeSet<(String, String)>,
    labels: &BTreeMap<String, String>,
    max_depth: usize,
) -> Result<(), RefinementError> {
    // r[impl cairn.refinement.acyclic]
    let mut adjacent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (producer, consumer) in edges {
        adjacent
            .entry(producer.clone())
            .or_default()
            .push(consumer.clone());
    }
    for values in adjacent.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    let mut color: BTreeMap<String, u8> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    fn visit(
        node: &str,
        adjacent: &BTreeMap<String, Vec<String>>,
        color: &mut BTreeMap<String, u8>,
        stack: &mut Vec<String>,
        labels: &BTreeMap<String, String>,
        max_depth: usize,
    ) -> Result<(), RefinementError> {
        match color.get(node) {
            Some(1) => {
                let start = stack.iter().position(|entry| entry == node).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(node.to_string());
                let names = cycle
                    .iter()
                    .map(|entry| labels.get(entry).cloned().unwrap_or_else(|| entry.clone()))
                    .collect::<Vec<_>>();
                return Err(RefinementError::Cycle(format!(
                    "requirement justification cycle: {}",
                    names.join(" -> ")
                )));
            }
            Some(2) => return Ok(()),
            _ => {}
        }
        color.insert(node.to_string(), 1);
        stack.push(node.to_string());
        if stack.len() > max_depth {
            return Err(RefinementError::BoundExhaustion(
                "justification graph depth exceeds the admitted bound".into(),
            ));
        }
        if let Some(next) = adjacent.get(node) {
            for target in next {
                visit(target, adjacent, color, stack, labels, max_depth)?;
            }
        }
        stack.pop();
        color.insert(node.to_string(), 2);
        Ok(())
    }
    for row in rows {
        if !color.contains_key(row) {
            visit(row, &adjacent, &mut color, &mut stack, labels, max_depth)?;
        }
    }
    Ok(())
}

struct AdmittedGuarantee {
    requirement: ContentBoundRequirementRow,
    interpretation_blake3: String,
    contract_blake3: String,
    context: ClaimContext,
    proposition: String,
    state: ObligationState,
    assurance: Assurance,
    residual: BTreeSet<String>,
}

fn assurance_for_proposition(tag: &str) -> Assurance {
    match tag {
        "check_passed" | "receipt_structurally_valid" | "native_completion_eligible" => {
            Assurance::BoundedCheck
        }
        "reviewed_correspondence" | "reviewed_completion_decision" => Assurance::Reviewed,
        _ => Assurance::Unassessed,
    }
}

fn guarantee_statement_for(guarantee: &AdmittedGuarantee) -> Result<String, RefinementError> {
    guarantee_statement_identity(&guarantee.requirement.row_blake3, &guarantee.proposition)
}

struct GuaranteeLookup<'a> {
    guarantees: &'a BTreeMap<(String, String), AdmittedGuarantee>,
    failures: &'a BTreeMap<String, String>,
}

struct GuaranteeUse {
    state: ObligationState,
    assurance: Assurance,
    residual: BTreeSet<String>,
    reason: Option<String>,
}

fn resolve_guarantee(
    lookup: &GuaranteeLookup<'_>,
    reference: &GuaranteeRef,
    requirement: &RequirementRevision,
    scope: &ArgumentScope,
) -> GuaranteeUse {
    if let Some(failure) = lookup.failures.get(&reference.guarantee_case) {
        return GuaranteeUse {
            state: ObligationState::Unsupported,
            assurance: Assurance::Unassessed,
            residual: BTreeSet::new(),
            reason: Some(format!("guarantee_case_rejected:{failure}")),
        };
    }
    let Some(guarantee) = lookup
        .guarantees
        .get(&(reference.guarantee_case.clone(), reference.claim_id.clone()))
    else {
        return GuaranteeUse {
            state: ObligationState::Unsupported,
            assurance: Assurance::Unassessed,
            residual: BTreeSet::new(),
            reason: Some("guarantee_claim_not_admitted".into()),
        };
    };
    let expected = ClaimContext {
        subject_blake3: scope.subject_blake3.clone(),
        policy_blake3: scope.policy_blake3.clone(),
        target: scope.target.clone(),
        environment: scope.environment.clone(),
        bounds: scope.bounds.clone(),
    };
    if guarantee.requirement != requirement.requirement
        || guarantee.interpretation_blake3 != requirement.interpretation_blake3
        || guarantee.contract_blake3 != requirement.contract_blake3
    {
        return GuaranteeUse {
            state: ObligationState::Unsupported,
            assurance: guarantee.assurance,
            residual: guarantee.residual.clone(),
            reason: Some("guarantee_revision_mismatch".into()),
        };
    }
    if guarantee.context != expected {
        return GuaranteeUse {
            state: ObligationState::Unsupported,
            assurance: guarantee.assurance,
            residual: guarantee.residual.clone(),
            reason: Some("guarantee_scope_mismatch".into()),
        };
    }
    let reason = match guarantee.state {
        ObligationState::Supported => None,
        ObligationState::Pending => Some("guarantee_pending".into()),
        ObligationState::Contradicted => Some("guarantee_contradicted".into()),
        ObligationState::Unsupported => Some("guarantee_unsupported".into()),
        ObligationState::Infeasible => Some("guarantee_infeasible".into()),
    };
    GuaranteeUse {
        state: guarantee.state,
        assurance: guarantee.assurance,
        residual: guarantee.residual.clone(),
        reason,
    }
}

fn merge_state(current: ObligationState, next: ObligationState) -> ObligationState {
    if next.severity() > current.severity() {
        next
    } else {
        current
    }
}

fn state_reason(state: ObligationState, blockers: &BTreeSet<String>) -> String {
    if blockers.is_empty() {
        return state.tag().into();
    }
    format!(
        "{}: {}",
        state.tag(),
        blockers.iter().cloned().collect::<Vec<_>>().join("; ")
    )
}

struct CompositionOutcome {
    state: ObligationState,
    assurance: Assurance,
    required_children: BTreeSet<String>,
    selected_alternatives: BTreeMap<String, String>,
    blockers: BTreeSet<String>,
}

fn child_outcome(
    child: &ChildGuarantee,
    lookup: &GuaranteeLookup<'_>,
) -> (CompositionOutcome, ObligationExplanation, BTreeSet<String>) {
    let reference = GuaranteeRef {
        guarantee_case: child.guarantee_case.clone(),
        claim_id: child.claim_id.clone(),
    };
    let resolved = resolve_guarantee(lookup, &reference, &child.requirement, &child.scope);
    let statement = lookup
        .guarantees
        .get(&(child.guarantee_case.clone(), child.claim_id.clone()))
        .and_then(|guarantee| guarantee_statement_for(guarantee).ok())
        .unwrap_or_default();
    let mut blockers = BTreeSet::new();
    if resolved.state != ObligationState::Supported {
        blockers.insert(format!(
            "{}:{}",
            resolved.reason.as_deref().unwrap_or("guarantee_unresolved"),
            child.id
        ));
    }
    let mut required_children = BTreeSet::new();
    required_children.insert(child.id.clone());
    let outcome = CompositionOutcome {
        state: resolved.state,
        assurance: resolved.assurance,
        required_children,
        selected_alternatives: BTreeMap::new(),
        blockers,
    };
    let explanation = ObligationExplanation {
        id: child.id.clone(),
        kind: ObligationKind::Child,
        state: resolved.state,
        assurance: resolved.assurance,
        statement_blake3: statement,
        source: format!("guarantee:{}:{}", child.guarantee_case, child.claim_id),
        reason: state_reason(resolved.state, &outcome.blockers),
        discharge_paths: Vec::new(),
    };
    (outcome, explanation, resolved.residual)
}

fn composition_outcome(
    composition: &Composition,
    path: &str,
    children: &BTreeMap<String, ChildGuarantee>,
    lookup: &GuaranteeLookup<'_>,
    conditions: &BTreeMap<String, Option<bool>>,
) -> (
    CompositionOutcome,
    Vec<ObligationExplanation>,
    BTreeSet<String>,
) {
    match composition {
        Composition::Child { child } => {
            let Some(declared) = children.get(child) else {
                return (
                    CompositionOutcome {
                        state: ObligationState::Unsupported,
                        assurance: Assurance::Unassessed,
                        required_children: BTreeSet::new(),
                        selected_alternatives: BTreeMap::new(),
                        blockers: BTreeSet::from([format!("undeclared_child:{child}")]),
                    },
                    Vec::new(),
                    BTreeSet::new(),
                );
            };
            let (outcome, explanation, residual) = child_outcome(declared, lookup);
            (outcome, vec![explanation], residual)
        }
        Composition::AllOf { obligations } => {
            let mut state = ObligationState::Supported;
            let mut assurance = Assurance::CheckedProof;
            let mut required_children = BTreeSet::new();
            let mut selected_alternatives = BTreeMap::new();
            let mut blockers = BTreeSet::new();
            let mut explanations = Vec::with_capacity(obligations.len());
            let mut residual = BTreeSet::new();
            for (index, obligation) in obligations.iter().enumerate() {
                let (outcome, child_explanations, child_residual) = composition_outcome(
                    obligation,
                    &format!("{path}.{index}"),
                    children,
                    lookup,
                    conditions,
                );
                state = merge_state(state, outcome.state);
                assurance = assurance.min(outcome.assurance);
                required_children.extend(outcome.required_children);
                selected_alternatives.extend(outcome.selected_alternatives);
                blockers.extend(outcome.blockers);
                residual.extend(child_residual);
                explanations.extend(child_explanations);
            }
            (
                CompositionOutcome {
                    state,
                    assurance,
                    required_children,
                    selected_alternatives,
                    blockers,
                },
                explanations,
                residual,
            )
        }
        Composition::AnyOf {
            alternatives,
            selected,
        } => {
            let Some(alternative) = alternatives.iter().find(|option| option.id == *selected)
            else {
                return (
                    CompositionOutcome {
                        state: ObligationState::Unsupported,
                        assurance: Assurance::Unassessed,
                        required_children: BTreeSet::new(),
                        selected_alternatives: BTreeMap::new(),
                        blockers: BTreeSet::from([format!("alternative_not_selected:{path}")]),
                    },
                    Vec::new(),
                    BTreeSet::new(),
                );
            };
            let mut state = ObligationState::Supported;
            let mut assurance = Assurance::CheckedProof;
            let mut required_children = BTreeSet::new();
            let mut selected_alternatives = BTreeMap::new();
            selected_alternatives.insert(path.to_string(), alternative.id.clone());
            let mut blockers = BTreeSet::new();
            let mut explanations = Vec::new();
            let mut residual = BTreeSet::new();
            for (index, obligation) in alternative.obligations.iter().enumerate() {
                let (outcome, child_explanations, child_residual) = composition_outcome(
                    obligation,
                    &format!("{path}.{index}"),
                    children,
                    lookup,
                    conditions,
                );
                state = merge_state(state, outcome.state);
                assurance = assurance.min(outcome.assurance);
                required_children.extend(outcome.required_children);
                selected_alternatives.extend(outcome.selected_alternatives);
                blockers.extend(outcome.blockers);
                residual.extend(child_residual);
                explanations.extend(child_explanations);
            }
            (
                CompositionOutcome {
                    state,
                    assurance,
                    required_children,
                    selected_alternatives,
                    blockers,
                },
                explanations,
                residual,
            )
        }
        Composition::Conditional {
            condition,
            when_satisfied,
            when_unsatisfied,
        } => match conditions.get(condition) {
            Some(Some(satisfied)) => {
                let branch = if *satisfied {
                    when_satisfied
                } else {
                    when_unsatisfied
                };
                let mut state = ObligationState::Supported;
                let mut assurance = Assurance::CheckedProof;
                let mut required_children = BTreeSet::new();
                let mut selected_alternatives = BTreeMap::new();
                selected_alternatives.insert(path.to_string(), condition.clone());
                let mut blockers = BTreeSet::new();
                let mut explanations = Vec::new();
                let mut residual = BTreeSet::new();
                for (index, obligation) in branch.iter().enumerate() {
                    let (outcome, child_explanations, child_residual) = composition_outcome(
                        obligation,
                        &format!("{path}.{index}"),
                        children,
                        lookup,
                        conditions,
                    );
                    state = merge_state(state, outcome.state);
                    assurance = assurance.min(outcome.assurance);
                    required_children.extend(outcome.required_children);
                    selected_alternatives.extend(outcome.selected_alternatives);
                    blockers.extend(outcome.blockers);
                    residual.extend(child_residual);
                    explanations.extend(child_explanations);
                }
                (
                    CompositionOutcome {
                        state,
                        assurance,
                        required_children,
                        selected_alternatives,
                        blockers,
                    },
                    explanations,
                    residual,
                )
            }
            Some(None) => (
                CompositionOutcome {
                    state: ObligationState::Pending,
                    assurance: Assurance::Unassessed,
                    required_children: BTreeSet::new(),
                    selected_alternatives: BTreeMap::new(),
                    blockers: BTreeSet::from([format!("condition_ambiguous:{condition}")]),
                },
                Vec::new(),
                BTreeSet::new(),
            ),
            None => (
                CompositionOutcome {
                    state: ObligationState::Pending,
                    assurance: Assurance::Unassessed,
                    required_children: BTreeSet::new(),
                    selected_alternatives: BTreeMap::new(),
                    blockers: BTreeSet::from([format!("condition_unobserved:{condition}")]),
                },
                Vec::new(),
                BTreeSet::new(),
            ),
        },
    }
}

fn admission_reason(
    assurance: Assurance,
    policy: &RefinementPolicy,
    proof_checker: Option<&str>,
) -> Option<String> {
    if assurance == Assurance::Unassessed {
        return Some("unassessed".into());
    }
    if !policy.assurances.contains(&assurance) {
        return Some("assurance_not_admitted".into());
    }
    if assurance == Assurance::CheckedProof {
        match proof_checker {
            Some(checker) if policy.proof_checkers.iter().any(|known| known == checker) => {}
            Some(checker) => {
                return Some(format!("proof_checker_not_admitted:{checker}"));
            }
            None => return Some("proof_checker_missing".into()),
        }
    }
    None
}

fn assumption_status(
    key: &(String, String),
    contradiction: bool,
    discharge: &BTreeMap<(String, String), Vec<Vec<String>>>,
    uses: &BTreeMap<(String, String), Vec<ObligationState>>,
    roots: &BTreeSet<(String, String)>,
) -> (AssumptionStatus, Vec<Vec<String>>) {
    let mut paths = discharge.get(key).cloned().unwrap_or_default();
    paths.sort();
    paths.dedup();
    if contradiction {
        return (AssumptionStatus::Contradicted, paths);
    }
    let contradicted = uses
        .get(key)
        .is_some_and(|states| states.contains(&ObligationState::Contradicted));
    if contradicted {
        return (AssumptionStatus::Contradicted, paths);
    }
    if !paths.is_empty() {
        return (AssumptionStatus::Discharged, paths);
    }
    if roots.contains(key) {
        return (AssumptionStatus::PolicyAccepted, paths);
    }
    let pending = uses.get(key).is_some_and(|states| {
        states.contains(&ObligationState::Pending) || states.contains(&ObligationState::Unsupported)
    });
    if pending {
        (AssumptionStatus::Pending, paths)
    } else {
        (AssumptionStatus::Residual, paths)
    }
}

struct AssumptionRecord {
    id: String,
    key: (String, String),
    statement_blake3: String,
    source: String,
    referenced_by: BTreeSet<String>,
    atom: Option<AssumptionAtom>,
}

/// A decomposition premise and a semantic dependency premise may name the same
/// assumption; identical identities merge into one closure node, while the same
/// id with different content is an explicit conflict.
fn add_assumption(
    records: &mut BTreeMap<(String, String), AssumptionRecord>,
    ids: &mut BTreeMap<String, String>,
    record: AssumptionRecord,
) -> Result<(), RefinementError> {
    if let Some(existing) = ids.get(&record.id) {
        if existing != &record.statement_blake3 {
            return Err(RefinementError::InvalidInput(format!(
                "assumption {} has conflicting statements",
                record.id
            )));
        }
    }
    ids.insert(record.id.clone(), record.statement_blake3.clone());
    let references = record.referenced_by.clone();
    records
        .entry(record.key.clone())
        .and_modify(|existing| {
            existing.referenced_by.extend(references.iter().cloned());
        })
        .or_insert(record);
    Ok(())
}

/// Re-admits native guarantee cases through the scoped-claim evaluator, checks
/// the whole justification graph, and returns deterministic explanations.
// r[impl cairn.refinement.feasibility]
// r[impl cairn.refinement.semantic_dependency]
// r[impl cairn.refinement.discharge]
// r[impl cairn.refinement.explanations]
pub fn evaluate_refinement(
    policy: &RefinementPolicy,
    request: &RefinementRequest,
) -> Result<RefinementReport, RefinementError> {
    admit_refinement_policy(policy)?;
    let bounds = &policy.bounds;
    if request.decompositions.len() > bounds.max_decompositions {
        return Err(RefinementError::BoundExhaustion(
            "decompositions exceed the admitted bound".into(),
        ));
    }
    if request.dependencies.len() > bounds.max_dependencies {
        return Err(RefinementError::BoundExhaustion(
            "dependencies exceed the admitted bound".into(),
        ));
    }
    if request.guarantee_cases.len() > bounds.max_guarantee_cases {
        return Err(RefinementError::BoundExhaustion(
            "guarantee cases exceed the admitted bound".into(),
        ));
    }
    if request.dependency_evidence.len()
        + request.integration_evidence.len()
        + request.applicability.len()
        > bounds.max_evidence
    {
        return Err(RefinementError::BoundExhaustion(
            "evidence exceeds the admitted bound".into(),
        ));
    }

    // Guarantee cases are re-evaluated, never trusted from serialized state.
    let mut cases: BTreeMap<String, &GuaranteeCase> = BTreeMap::new();
    for case in &request.guarantee_cases {
        require_text(&case.id, "guarantee case id")?;
        if cases.insert(case.id.clone(), case).is_some() {
            return Err(RefinementError::InvalidInput(format!(
                "duplicate guarantee case {}",
                case.id
            )));
        }
    }
    let mut guarantees: BTreeMap<(String, String), AdmittedGuarantee> = BTreeMap::new();
    let mut failures: BTreeMap<String, String> = BTreeMap::new();
    for (id, case) in &cases {
        match evaluate_derivations(&case.policy, &case.case) {
            Ok(report) => {
                for explanation in &report.claims {
                    let claim = &explanation.claim;
                    let state = match explanation.state {
                        ClaimState::Supported => ObligationState::Supported,
                        ClaimState::Refuted | ClaimState::Disputed => ObligationState::Contradicted,
                        ClaimState::Unresolved => ObligationState::Pending,
                    };
                    guarantees.insert(
                        (id.clone(), explanation.claim_id.clone()),
                        AdmittedGuarantee {
                            requirement: claim.requirement.clone(),
                            interpretation_blake3: claim.interpretation_blake3.clone(),
                            contract_blake3: claim.contract_blake3.clone(),
                            context: claim.context.clone(),
                            proposition: claim.proposition.tag().into(),
                            state,
                            assurance: assurance_for_proposition(claim.proposition.tag()),
                            residual: explanation.residual_assumptions.clone(),
                        },
                    );
                }
            }
            Err(error) => {
                failures.insert(id.clone(), error.to_string());
            }
        }
    }
    let lookup = GuaranteeLookup {
        guarantees: &guarantees,
        failures: &failures,
    };

    // Validate and index decompositions.
    let mut decomposition_ids = BTreeSet::new();
    let mut decompositions: Vec<&Decomposition> = Vec::with_capacity(request.decompositions.len());
    let mut edge_rows = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let mut requirement_count = 0usize;
    let mut all_assumptions: BTreeMap<(String, String), AssumptionRecord> = BTreeMap::new();
    let mut assumption_ids: BTreeMap<String, String> = BTreeMap::new();
    for decomposition in &request.decompositions {
        if !decomposition_ids.insert(decomposition.id.as_str()) {
            return Err(RefinementError::InvalidInput(format!(
                "duplicate decomposition id {}",
                decomposition.id
            )));
        }
        validate_decomposition(decomposition, policy, bounds)?;
        requirement_count += 1 + decomposition.children.len();
        edge_rows.insert(decomposition.parent.requirement.row_blake3.clone());
        labels
            .entry(decomposition.parent.requirement.row_blake3.clone())
            .or_insert_with(|| decomposition.parent.requirement.requirement_id.clone());
        for child in &decomposition.children {
            edge_rows.insert(child.requirement.requirement.row_blake3.clone());
            labels
                .entry(child.requirement.requirement.row_blake3.clone())
                .or_insert_with(|| child.requirement.requirement.requirement_id.clone());
            edges.insert((
                decomposition.parent.requirement.row_blake3.clone(),
                child.requirement.requirement.row_blake3.clone(),
            ));
        }
        for assumption in &decomposition.environment {
            let key = (
                assumption.id.clone(),
                assumption_statement_identity(&assumption.id, &assumption.statement)?,
            );
            add_assumption(
                &mut all_assumptions,
                &mut assumption_ids,
                AssumptionRecord {
                    id: assumption.id.clone(),
                    key,
                    statement_blake3: assumption_statement_identity(
                        &assumption.id,
                        &assumption.statement,
                    )?,
                    source: assumption.source.clone(),
                    referenced_by: BTreeSet::from([format!("decomposition:{}", decomposition.id)]),
                    atom: Some(assumption.atom.clone()),
                },
            )?;
        }
        decompositions.push(decomposition);
    }

    // Validate and index dependencies.
    let mut dependency_ids = BTreeSet::new();
    let mut dependencies: Vec<&TypedDependency> = Vec::with_capacity(request.dependencies.len());
    for dependency in &request.dependencies {
        if !dependency_ids.insert(dependency.id.as_str()) {
            return Err(RefinementError::InvalidInput(format!(
                "duplicate dependency id {}",
                dependency.id
            )));
        }
        validate_dependency(dependency)?;
        requirement_count += 2;
        for endpoint in [&dependency.producer, &dependency.consumer] {
            edge_rows.insert(endpoint.requirement.requirement.row_blake3.clone());
            labels
                .entry(endpoint.requirement.requirement.row_blake3.clone())
                .or_insert_with(|| endpoint.requirement.requirement.requirement_id.clone());
        }
        edges.insert((
            dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
        ));
        if let Some(binding) = &dependency.semantic {
            let key = (
                binding.consumer_assumption.id.clone(),
                assumption_statement_identity(
                    &binding.consumer_assumption.id,
                    &binding.consumer_assumption.statement,
                )?,
            );
            add_assumption(
                &mut all_assumptions,
                &mut assumption_ids,
                AssumptionRecord {
                    id: binding.consumer_assumption.id.clone(),
                    key,
                    statement_blake3: assumption_statement_identity(
                        &binding.consumer_assumption.id,
                        &binding.consumer_assumption.statement,
                    )?,
                    source: format!("dependency:{}", dependency.id),
                    referenced_by: BTreeSet::from([format!("dependency:{}", dependency.id)]),
                    atom: None,
                },
            )?;
        }
        dependencies.push(dependency);
    }
    decompositions.sort_by(|left, right| left.id.cmp(&right.id));
    dependencies.sort_by(|left, right| left.id.cmp(&right.id));
    if requirement_count > bounds.max_requirements {
        return Err(RefinementError::BoundExhaustion(
            "requirement revisions exceed the admitted bound".into(),
        ));
    }
    if all_assumptions.len() > bounds.max_assumptions {
        return Err(RefinementError::BoundExhaustion(
            "assumptions exceed the admitted bound".into(),
        ));
    }
    let mut evidence_count = 0usize;
    for evidence in &request.dependency_evidence {
        validate_dependency_evidence(evidence)?;
        evidence_count += 1;
    }
    for evidence in &request.integration_evidence {
        require_text(&evidence.obligation_id, "integration evidence id")?;
        require_text(&evidence.statement, "integration evidence statement")?;
        validate_scope(&evidence.scope)?;
        require_digest(&evidence.observation_blake3, "integration observation")?;
        require_text(&evidence.source, "integration evidence source")?;
        evidence_count += 1;
    }
    let mut condition_values: BTreeMap<String, Option<bool>> = BTreeMap::new();
    for observation in &request.applicability {
        require_text(&observation.condition, "applicability condition")?;
        match condition_values.get(&observation.condition) {
            Some(Some(existing)) if *existing != observation.satisfied => {
                condition_values.insert(observation.condition.clone(), None);
            }
            Some(None) => {}
            _ => {
                condition_values.insert(observation.condition.clone(), Some(observation.satisfied));
            }
        }
        evidence_count += 1;
    }
    if evidence_count > bounds.max_evidence {
        return Err(RefinementError::BoundExhaustion(
            "evidence exceeds the admitted bound".into(),
        ));
    }
    assess_acyclic(&edge_rows, &edges, &labels, bounds.max_depth)?;

    let mut triggers = BTreeSet::new();
    let mut trust_dependencies = BTreeSet::new();
    let mut unresolved = BTreeSet::new();

    // Semantic dependencies first: their discharge feeds decomposition closure.
    let mut discharge_paths: BTreeMap<(String, String), Vec<Vec<String>>> = BTreeMap::new();
    let mut assumption_uses: BTreeMap<(String, String), Vec<ObligationState>> = BTreeMap::new();
    let mut dependency_explanations = Vec::with_capacity(dependencies.len());
    for dependency in &dependencies {
        let explanation = evaluate_dependency(
            dependency,
            &request.dependency_evidence,
            &lookup,
            policy,
            &mut discharge_paths,
            &mut assumption_uses,
            &mut triggers,
            &mut trust_dependencies,
        )?;
        if explanation.state != ObligationState::Supported {
            unresolved.insert(format!("dependency:{}", explanation.id));
        }
        dependency_explanations.push(explanation);
    }

    let mut contradiction_predicates: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    for assumption in all_assumptions.values() {
        if let Some(atom) = &assumption.atom {
            let entry = contradiction_predicates
                .entry(atom.predicate.as_str())
                .or_insert((false, false));
            if atom.asserted {
                entry.0 = true;
            } else {
                entry.1 = true;
            }
        }
    }
    let roots: BTreeSet<(String, String)> = policy
        .root_assumptions
        .iter()
        .map(|root| {
            Ok((
                root.id.clone(),
                assumption_statement_identity(&root.id, &root.statement)?,
            ))
        })
        .collect::<Result<_, RefinementError>>()?;

    let mut residual_explanations = Vec::with_capacity(all_assumptions.len());
    let mut status_by_key: BTreeMap<(String, String), AssumptionStatus> = BTreeMap::new();
    for record in all_assumptions.values() {
        let contradiction = record
            .atom
            .as_ref()
            .and_then(|atom| contradiction_predicates.get(atom.predicate.as_str()))
            .is_some_and(|(asserted, negated)| *asserted && *negated);
        let (status, paths) = assumption_status(
            &record.key,
            contradiction,
            &discharge_paths,
            &assumption_uses,
            &roots,
        );
        status_by_key.insert(record.key.clone(), status);
        if !matches!(
            status,
            AssumptionStatus::Discharged | AssumptionStatus::PolicyAccepted
        ) {
            unresolved.insert(format!("assumption:{}", record.id));
        }
        if status == AssumptionStatus::PolicyAccepted {
            trust_dependencies.insert(format!("policy-root-assumption:{}", record.id));
        }
        residual_explanations.push(ResidualAssumptionExplanation {
            id: record.id.clone(),
            statement_blake3: record.statement_blake3.clone(),
            source: record.source.clone(),
            status,
            discharge_paths: paths,
            referenced_by: record.referenced_by.iter().cloned().collect(),
        });
    }

    // Decomposition arguments use established discharge and policy roots.
    let mut decomposition_explanations = Vec::with_capacity(decompositions.len());
    for decomposition in &decompositions {
        let explanation = evaluate_decomposition(
            decomposition,
            &lookup,
            policy,
            &condition_values,
            &request.integration_evidence,
            &discharge_paths,
            &status_by_key,
            &mut trust_dependencies,
        )?;
        if explanation.state != ObligationState::Supported {
            unresolved.insert(format!("decomposition:{}", explanation.id));
        }
        if explanation.conditional {
            trust_dependencies.insert(format!("conditional-parent:{}", explanation.id));
        }
        decomposition_explanations.push(explanation);
    }
    for dependency in &dependency_explanations {
        if dependency.state == ObligationState::Supported {
            trust_dependencies.insert(format!(
                "dependency-evidence:{}:retained-observation",
                dependency.relation.tag()
            ));
        }
    }

    let policy_blake3 = identity("cairn.refinement-policy.v1", policy)?;
    let mut basis_requirement_rows = BTreeSet::new();
    let mut basis_assumptions = BTreeSet::new();
    let mut basis_contexts = BTreeSet::new();
    for decomposition in &decompositions {
        basis_requirement_rows.insert(decomposition.parent.requirement.row_blake3.clone());
        basis_contexts.insert(scope_identity(&decomposition.scope)?);
        for child in &decomposition.children {
            basis_requirement_rows.insert(child.requirement.requirement.row_blake3.clone());
        }
        for assumption in &decomposition.environment {
            basis_assumptions.insert(format!(
                "{}:{}",
                assumption.id,
                assumption_statement_identity(&assumption.id, &assumption.statement)?
            ));
        }
    }
    for dependency in &dependencies {
        basis_requirement_rows.insert(
            dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
        );
        basis_requirement_rows.insert(
            dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
        );
        basis_contexts.insert(context_identity(&dependency.context)?);
        if let Some(binding) = &dependency.semantic {
            basis_assumptions.insert(format!(
                "{}:{}",
                binding.consumer_assumption.id,
                assumption_statement_identity(
                    &binding.consumer_assumption.id,
                    &binding.consumer_assumption.statement
                )?
            ));
            basis_contexts.insert(scope_identity(&binding.producer_scope)?);
            basis_contexts.insert(scope_identity(&binding.consumer_scope)?);
        }
    }
    let basis = EvaluationBasis {
        policy_blake3,
        requirement_rows: basis_requirement_rows,
        assumption_statements: basis_assumptions,
        contexts: basis_contexts,
    };
    if let Some(previous) = &request.previous_basis {
        triggers.extend(reevaluation_triggers(previous, &basis));
    }
    let input_blake3 = identity("cairn.refinement-input.v1", &(policy, request))?;
    let mut report = RefinementReport {
        schema: REFINEMENT_REPORT_SCHEMA_V1.into(),
        advisory: true,
        decompositions: decomposition_explanations,
        dependencies: dependency_explanations,
        residual_assumptions: residual_explanations,
        unresolved,
        reevaluation_triggers: triggers,
        basis,
        trust_dependencies,
        input_blake3,
        report_blake3: String::new(),
        boundary: REFINEMENT_BOUNDARY.into(),
    };
    // r[impl cairn.refinement.explanations]
    let report_blake3 = identity("cairn.refinement-report.v1", &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

fn reevaluation_triggers(
    previous: &EvaluationBasis,
    current: &EvaluationBasis,
) -> BTreeSet<String> {
    let mut triggers = BTreeSet::new();
    if previous.policy_blake3 != current.policy_blake3 {
        triggers.insert("policy_changed".into());
    }
    for row in previous
        .requirement_rows
        .symmetric_difference(&current.requirement_rows)
    {
        triggers.insert(format!("requirement_changed:{row}"));
    }
    for statement in previous
        .assumption_statements
        .symmetric_difference(&current.assumption_statements)
    {
        triggers.insert(format!("assumption_changed:{statement}"));
    }
    for context in previous.contexts.symmetric_difference(&current.contexts) {
        triggers.insert(format!("context_changed:{context}"));
    }
    triggers
}

#[allow(clippy::too_many_arguments)]
fn evaluate_decomposition(
    decomposition: &Decomposition,
    lookup: &GuaranteeLookup<'_>,
    policy: &RefinementPolicy,
    conditions: &BTreeMap<String, Option<bool>>,
    integration_evidence: &[IntegrationEvidence],
    discharge_paths: &BTreeMap<(String, String), Vec<Vec<String>>>,
    status_by_key: &BTreeMap<(String, String), AssumptionStatus>,
    trust_dependencies: &mut BTreeSet<String>,
) -> Result<DecompositionExplanation, RefinementError> {
    let children: BTreeMap<String, ChildGuarantee> = decomposition
        .children
        .iter()
        .map(|child| (child.id.clone(), child.clone()))
        .collect();
    let (composition, mut obligations, guarantee_residuals) = composition_outcome(
        &decomposition.composition,
        "composition",
        &children,
        lookup,
        conditions,
    );
    for residual in &guarantee_residuals {
        trust_dependencies.insert(residual.clone());
    }

    // Environment premise states.
    let mut assumption_residuals = BTreeSet::new();
    let mut assumption_contradicted = false;
    let mut assumption_unresolved = false;
    for assumption in &decomposition.environment {
        let key = (
            assumption.id.clone(),
            assumption_statement_identity(&assumption.id, &assumption.statement)?,
        );
        let status = status_by_key
            .get(&key)
            .copied()
            .unwrap_or(AssumptionStatus::Residual);
        let state = match status {
            AssumptionStatus::Discharged | AssumptionStatus::PolicyAccepted => {
                ObligationState::Supported
            }
            AssumptionStatus::Pending => ObligationState::Pending,
            AssumptionStatus::Residual => ObligationState::Pending,
            AssumptionStatus::Contradicted => ObligationState::Infeasible,
        };
        if status == AssumptionStatus::Contradicted {
            assumption_contradicted = true;
        }
        if matches!(
            status,
            AssumptionStatus::Pending | AssumptionStatus::Residual | AssumptionStatus::Contradicted
        ) {
            assumption_unresolved = true;
        }
        if !matches!(status, AssumptionStatus::Discharged) {
            assumption_residuals.insert(assumption.id.clone());
        }
        let mut paths = discharge_paths.get(&key).cloned().unwrap_or_default();
        paths.sort();
        paths.dedup();
        obligations.push(ObligationExplanation {
            id: assumption.id.clone(),
            kind: ObligationKind::EnvironmentAssumption,
            state,
            assurance: if status == AssumptionStatus::Discharged {
                Assurance::BoundedCheck
            } else {
                Assurance::Unassessed
            },
            statement_blake3: assumption_statement_identity(&assumption.id, &assumption.statement)?,
            source: assumption.source.clone(),
            reason: format!(
                "assumption_{}:{}",
                assumption_status_tag(status),
                assumption.id
            ),
            discharge_paths: paths,
        });
    }

    // Integration obligations are premises, never implied by child completion.
    let mut integration_state = ObligationState::Supported;
    let mut integration_assurance = if decomposition.integration.is_empty() {
        Assurance::CheckedProof
    } else {
        Assurance::BoundedCheck
    };
    let mut integration_blockers = BTreeSet::new();
    for obligation in &decomposition.integration {
        let statement = assumption_statement_identity(&obligation.id, &obligation.statement)?;
        let mut exact = BTreeSet::new();
        let mut blockers = BTreeSet::new();
        for evidence in integration_evidence {
            if evidence.obligation_id != obligation.id {
                continue;
            }
            if evidence.scope != obligation.scope {
                blockers.insert("out_of_scope".to_string());
                continue;
            }
            if assumption_statement_identity(&evidence.obligation_id, &evidence.statement)?
                != statement
            {
                blockers.insert("statement_mismatch".to_string());
                continue;
            }
            exact.insert(
                serde_json::to_string(evidence)
                    .map_err(|error| RefinementError::InvalidInput(error.to_string()))?,
            );
        }
        let (state, reason) = if exact.len() > 1 {
            (ObligationState::Pending, "integration_evidence_ambiguous")
        } else if exact.len() == 1 {
            (
                ObligationState::Supported,
                "integration_obligation_established",
            )
        } else if blockers.contains("out_of_scope") {
            (
                ObligationState::Pending,
                "integration_evidence_out_of_scope",
            )
        } else if blockers.contains("statement_mismatch") {
            (ObligationState::Pending, "integration_statement_mismatch")
        } else {
            (ObligationState::Pending, "integration_obligation_pending")
        };
        if state == ObligationState::Supported {
            trust_dependencies.insert(format!(
                "integration-observation:{}:declared-not-executed",
                obligation.id
            ));
        }
        if obligation.required && state != ObligationState::Supported {
            integration_state = merge_state(integration_state, state);
            integration_blockers.insert(format!("{reason}:{}", obligation.id));
        }
        obligations.push(ObligationExplanation {
            id: obligation.id.clone(),
            kind: ObligationKind::IntegrationObligation,
            state,
            assurance: if state == ObligationState::Supported {
                Assurance::BoundedCheck
            } else {
                Assurance::Unassessed
            },
            statement_blake3: statement,
            source: "declared-integration-obligation".into(),
            reason: format!("{reason}:{}", obligation.id),
            discharge_paths: Vec::new(),
        });
    }
    if integration_state != ObligationState::Supported {
        integration_assurance = assurance_min(integration_assurance, Assurance::Unassessed);
    }

    // Feasibility is evaluated separately and cannot be inferred from children.
    let mut feasibility_blockers = BTreeSet::new();
    let feasibility_state;
    let mut feasibility_assurance = decomposition.feasibility.assurance;
    if assumption_contradicted {
        feasibility_state = ObligationState::Infeasible;
        feasibility_blockers.insert("contradictory_assumptions".into());
    } else if composition.state == ObligationState::Contradicted {
        feasibility_state = ObligationState::Infeasible;
        feasibility_blockers.insert("contradictory_children".into());
    } else {
        match decomposition.feasibility.disposition {
            Feasibility::Infeasible => {
                feasibility_state = ObligationState::Infeasible;
                feasibility_blockers.insert("declared_infeasible".into());
            }
            Feasibility::Unresolved => {
                feasibility_state = ObligationState::Pending;
                feasibility_blockers.insert("feasibility_unresolved".into());
            }
            Feasibility::Feasible => {
                let mut blockers = BTreeSet::new();
                if let Some(reason) = admission_reason(
                    decomposition.feasibility.assurance,
                    policy,
                    decomposition.feasibility.proof_checker.as_deref(),
                ) {
                    blockers.insert(format!("feasibility_{reason}"));
                }
                if !policy
                    .feasibility_methods
                    .iter()
                    .any(|method| method == &decomposition.feasibility.method)
                {
                    blockers.insert("feasibility_method_unadmitted".into());
                }
                let mut required: BTreeSet<&str> = composition
                    .required_children
                    .iter()
                    .map(String::as_str)
                    .collect();
                required.extend(
                    decomposition
                        .environment
                        .iter()
                        .map(|assumption| assumption.id.as_str()),
                );
                if !required
                    .iter()
                    .all(|id| decomposition.feasibility.covered.contains(*id))
                {
                    blockers.insert("feasibility_coverage_incomplete".into());
                }
                if assumption_unresolved {
                    blockers.insert("feasibility_assumption_unresolved".into());
                }
                if composition.state == ObligationState::Pending {
                    blockers.insert("feasibility_composition_pending".into());
                }
                if composition.state == ObligationState::Unsupported {
                    blockers.insert("feasibility_composition_unsupported".into());
                }
                if blockers.is_empty() {
                    feasibility_state = ObligationState::Supported;
                } else if blockers.iter().any(|blocker| {
                    blocker.starts_with("feasibility_unassessed")
                        || blocker.starts_with("feasibility_assurance_not_admitted")
                        || blocker.starts_with("feasibility_proof_checker")
                }) || blockers.contains("feasibility_coverage_incomplete")
                    || blockers.contains("feasibility_method_unadmitted")
                {
                    feasibility_state = ObligationState::Unsupported;
                } else {
                    feasibility_state = ObligationState::Pending;
                }
                feasibility_blockers.extend(blockers);
            }
        }
    }
    if feasibility_state != ObligationState::Supported {
        feasibility_assurance = assurance_min(feasibility_assurance, Assurance::Unassessed);
    }

    // The parent argument merges relation, feasibility, composition,
    // integration and assumption outcomes. Weakest assurance is retained.
    let mut blockers = feasibility_blockers.clone();
    blockers.extend(integration_blockers.iter().cloned());
    if assumption_contradicted {
        blockers.insert("assumption_contradicted".into());
    }
    if assumption_unresolved && !assumption_contradicted {
        blockers.insert("assumption_unresolved".into());
    }
    blockers.extend(composition.blockers.iter().cloned());
    let mut state = ObligationState::Supported;
    state = merge_state(state, composition.state);
    state = merge_state(state, feasibility_state);
    state = merge_state(state, integration_state);
    if assumption_contradicted {
        state = merge_state(state, ObligationState::Infeasible);
    } else if assumption_unresolved {
        state = merge_state(state, ObligationState::Pending);
    }
    if let Some(reason) = admission_reason(
        decomposition.relation.assurance,
        policy,
        decomposition.relation.proof_checker.as_deref(),
    ) {
        blockers.insert(format!("relation_{reason}"));
        state = merge_state(state, ObligationState::Unsupported);
    }
    if decomposition.relation.assurance == Assurance::CheckedProof
        && admission_reason(
            decomposition.relation.assurance,
            policy,
            decomposition.relation.proof_checker.as_deref(),
        )
        .is_none()
    {
        if let Some(checker) = decomposition.relation.proof_checker.as_deref() {
            trust_dependencies.insert(format!("proof-checker:{checker}:trusted-declaration"));
        }
    }
    if state == ObligationState::Supported {
        trust_dependencies.insert(format!(
            "implication-witness:{}:retained-declaration",
            decomposition.relation.witness_blake3
        ));
        trust_dependencies.insert(format!(
            "feasibility-method:{}:declared",
            decomposition.feasibility.method
        ));
    }
    let assurance = assurance_min(
        assurance_min(decomposition.relation.assurance, composition.assurance),
        assurance_min(feasibility_assurance, integration_assurance),
    );
    obligations.sort_by(|left, right| (left.kind, &left.id).cmp(&(right.kind, &right.id)));
    let conditional = state == ObligationState::Supported && !assumption_residuals.is_empty();
    let reason = if state == ObligationState::Supported {
        "supported: selected composition, established integration obligations, and admitted feasibility"
            .into()
    } else {
        state_reason(state, &blockers)
    };
    Ok(DecompositionExplanation {
        id: decomposition.id.clone(),
        parent: decomposition.parent.clone(),
        state,
        assurance: if state == ObligationState::Supported {
            assurance
        } else {
            assurance_min(assurance, Assurance::Unassessed)
        },
        conditional,
        reason,
        obligations,
        feasibility: FeasibilityExplanation {
            declared: decomposition.feasibility.disposition,
            state: feasibility_state,
            assurance: feasibility_assurance,
            method: decomposition.feasibility.method.clone(),
            covered: decomposition.feasibility.covered.clone(),
            blockers: feasibility_blockers.iter().cloned().collect(),
        },
        selected_alternatives: composition.selected_alternatives.clone(),
        residual_assumption_ids: assumption_residuals,
    })
}

fn assurance_min(left: Assurance, right: Assurance) -> Assurance {
    if left <= right {
        left
    } else {
        right
    }
}

fn assumption_status_tag(status: AssumptionStatus) -> &'static str {
    match status {
        AssumptionStatus::Discharged => "discharged",
        AssumptionStatus::PolicyAccepted => "policy_accepted",
        AssumptionStatus::Residual => "residual",
        AssumptionStatus::Pending => "pending",
        AssumptionStatus::Contradicted => "contradicted",
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_dependency(
    dependency: &TypedDependency,
    evidence: &[DependencyEvidence],
    lookup: &GuaranteeLookup<'_>,
    policy: &RefinementPolicy,
    discharge_paths: &mut BTreeMap<(String, String), Vec<Vec<String>>>,
    assumption_uses: &mut BTreeMap<(String, String), Vec<ObligationState>>,
    triggers: &mut BTreeSet<String>,
    trust_dependencies: &mut BTreeSet<String>,
) -> Result<DependencyExplanation, RefinementError> {
    if !policy.relations.contains(&dependency.relation) {
        return Ok(DependencyExplanation {
            id: dependency.id.clone(),
            relation: dependency.relation,
            state: ObligationState::Unsupported,
            assurance: Assurance::Unassessed,
            reason: format!(
                "unsupported: relation_not_admitted:{}",
                dependency.relation.tag()
            ),
            evidence: None,
            producer_row_blake3: dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            consumer_row_blake3: dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            residual_assumption_ids: BTreeSet::new(),
        });
    }
    if dependency.relation == DependencyRelation::Semantic {
        return evaluate_semantic_dependency(
            dependency,
            lookup,
            policy,
            discharge_paths,
            assumption_uses,
            triggers,
            trust_dependencies,
        );
    }
    let mut exact: BTreeSet<String> = BTreeSet::new();
    let mut blockers: BTreeSet<String> = BTreeSet::new();
    for candidate in evidence
        .iter()
        .filter(|candidate| candidate.dependency_id() == dependency.id)
    {
        let expected = match dependency.relation {
            DependencyRelation::OrderingArchive => "archive",
            DependencyRelation::VerifiedSpecification => "specification",
            DependencyRelation::VerifiedCandidate => "candidate_verified",
            DependencyRelation::CandidateInput => "candidate_input",
            DependencyRelation::IntegratedRevision => "integration",
            DependencyRelation::Lifecycle => "lifecycle",
            DependencyRelation::Semantic => continue,
        };
        if candidate.kind_tag() != expected {
            blockers.insert(format!("evidence_not_applicable:{}", candidate.kind_tag()));
            continue;
        }
        match candidate {
            DependencyEvidence::Archive {
                change,
                repository,
                revision,
                archive_receipt_blake3,
                coverage,
                ..
            } => {
                if change != &dependency.producer.work
                    || repository != &dependency.context.producer_repository
                {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else if !coverage_satisfied(&dependency.coverage, coverage) {
                    blockers.insert("coverage_incomplete".into());
                } else {
                    exact.insert(format!(
                        "archive:{}:{}:{}",
                        change, revision, archive_receipt_blake3
                    ));
                }
            }
            DependencyEvidence::Specification {
                repository,
                revision,
                gates,
                receipt_blake3,
                ..
            } => {
                if repository != &dependency.context.producer_repository {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else if !dependency
                    .required_gates
                    .iter()
                    .all(|gate| gates.contains(gate))
                {
                    blockers.insert("missing_specification_gate".into());
                } else {
                    exact.insert(format!("specification:{}:{}", revision, receipt_blake3));
                }
            }
            DependencyEvidence::CandidateVerified {
                repository,
                candidate,
                revision,
                verification_receipt_blake3,
                ..
            } => {
                if candidate != &dependency.producer.work
                    || repository != &dependency.context.producer_repository
                {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else {
                    exact.insert(format!(
                        "candidate_verified:{}:{}:{}",
                        candidate, revision, verification_receipt_blake3
                    ));
                }
            }
            DependencyEvidence::CandidateInput {
                repository,
                input_blake3,
                revision,
                ..
            } => {
                if repository != &dependency.context.producer_repository
                    || dependency.required_input_blake3.as_deref() != Some(input_blake3.as_str())
                {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else {
                    exact.insert(format!("candidate_input:{}:{}", input_blake3, revision));
                }
            }
            DependencyEvidence::Integration {
                repository,
                target,
                change,
                candidate,
                revision,
                observation_blake3,
                ..
            } => {
                if change != &dependency.producer.work
                    || candidate != &dependency.producer.work
                    || repository != &dependency.context.producer_repository
                    || target != &dependency.context.intended_target
                {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else {
                    exact.insert(format!(
                        "integration:{}:{}:{}:{}",
                        change, candidate, revision, observation_blake3
                    ));
                }
            }
            DependencyEvidence::Lifecycle {
                repository,
                stage,
                revision,
                receipt_blake3,
                ..
            } => {
                if repository != &dependency.context.producer_repository
                    || dependency.required_lifecycle_stage.as_deref() != Some(stage.as_str())
                {
                    blockers.insert("out_of_scope".into());
                } else if revision != &dependency.producer.revision {
                    blockers.insert("stale_revision".into());
                } else {
                    exact.insert(format!(
                        "lifecycle:{}:{}:{}",
                        stage, revision, receipt_blake3
                    ));
                }
            }
        }
    }
    if blockers.contains("stale_revision") || blockers.contains("out_of_scope") {
        triggers.insert(format!("applicability_reevaluation:{}", dependency.id));
    }
    let (state, reason, label) = if exact.len() > 1 {
        (
            ObligationState::Pending,
            "pending: ambiguous_evidence".to_string(),
            None,
        )
    } else if let Some(label) = exact.into_iter().next() {
        (
            ObligationState::Supported,
            "supported: exact admitted evidence".to_string(),
            Some(label),
        )
    } else {
        let code = if blockers.contains("stale_revision") {
            "stale_revision"
        } else if blockers.contains("out_of_scope") {
            "out_of_scope"
        } else if blockers.contains("coverage_incomplete") {
            "coverage_incomplete"
        } else if blockers.contains("missing_specification_gate") {
            "missing_specification_gate"
        } else if blockers.contains("evidence_not_applicable") {
            "evidence_not_applicable"
        } else {
            "missing_evidence"
        };
        (ObligationState::Pending, format!("pending: {code}"), None)
    };
    Ok(DependencyExplanation {
        id: dependency.id.clone(),
        relation: dependency.relation,
        state,
        assurance: if state == ObligationState::Supported {
            Assurance::BoundedCheck
        } else {
            Assurance::Unassessed
        },
        reason,
        evidence: label,
        producer_row_blake3: dependency
            .producer
            .requirement
            .requirement
            .row_blake3
            .clone(),
        consumer_row_blake3: dependency
            .consumer
            .requirement
            .requirement
            .row_blake3
            .clone(),
        residual_assumption_ids: BTreeSet::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_semantic_dependency(
    dependency: &TypedDependency,
    lookup: &GuaranteeLookup<'_>,
    policy: &RefinementPolicy,
    discharge_paths: &mut BTreeMap<(String, String), Vec<Vec<String>>>,
    assumption_uses: &mut BTreeMap<(String, String), Vec<ObligationState>>,
    triggers: &mut BTreeSet<String>,
    trust_dependencies: &mut BTreeSet<String>,
) -> Result<DependencyExplanation, RefinementError> {
    let Some(binding) = &dependency.semantic else {
        return Ok(DependencyExplanation {
            id: dependency.id.clone(),
            relation: dependency.relation,
            state: ObligationState::Unsupported,
            assurance: Assurance::Unassessed,
            reason: "unsupported: semantic_binding_missing".into(),
            evidence: None,
            producer_row_blake3: dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            consumer_row_blake3: dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            residual_assumption_ids: BTreeSet::new(),
        });
    };
    let mut residual = BTreeSet::new();
    let resolved = resolve_guarantee(
        lookup,
        &binding.producer_guarantee,
        &dependency.producer.requirement,
        &binding.producer_scope,
    );
    residual.extend(resolved.residual.iter().cloned());
    for value in &resolved.residual {
        trust_dependencies.insert(value.clone());
    }
    let compatible = binding.producer_scope.policy_blake3 == dependency.context.policy_blake3
        && binding.producer_scope.environment == dependency.context.environment
        && binding.producer_scope.bounds == dependency.context.bounds
        && binding.consumer_scope.policy_blake3 == dependency.context.policy_blake3
        && binding.consumer_scope.environment == dependency.context.environment
        && binding.consumer_scope.bounds == dependency.context.bounds
        && binding.producer_scope.target == dependency.context.producer_repository
        && binding.consumer_scope.target == dependency.context.intended_target;
    let key = (
        binding.consumer_assumption.id.clone(),
        assumption_statement_identity(
            &binding.consumer_assumption.id,
            &binding.consumer_assumption.statement,
        )?,
    );
    let admitted = admission_reason(binding.witness.assurance, policy, None);
    if let Some(reason) = admitted.clone() {
        assumption_uses
            .entry(key)
            .or_default()
            .push(ObligationState::Unsupported);
        return Ok(DependencyExplanation {
            id: dependency.id.clone(),
            relation: dependency.relation,
            state: ObligationState::Unsupported,
            assurance: binding.witness.assurance,
            reason: format!("unsupported: witness_{reason}"),
            evidence: None,
            producer_row_blake3: dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            consumer_row_blake3: dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            residual_assumption_ids: residual,
        });
    }
    if !compatible {
        triggers.insert(format!("applicability_reevaluation:{}", dependency.id));
        assumption_uses
            .entry(key)
            .or_default()
            .push(ObligationState::Unsupported);
        return Ok(DependencyExplanation {
            id: dependency.id.clone(),
            relation: dependency.relation,
            state: ObligationState::Unsupported,
            assurance: binding.witness.assurance,
            reason: "unsupported: context_incompatible".into(),
            evidence: None,
            producer_row_blake3: dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            consumer_row_blake3: dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            residual_assumption_ids: residual,
        });
    }
    let guarantee = lookup.guarantees.get(&(
        binding.producer_guarantee.guarantee_case.clone(),
        binding.producer_guarantee.claim_id.clone(),
    ));
    let witness_matches = guarantee.is_some_and(|guarantee| {
        guarantee_statement_for(guarantee)
            .is_ok_and(|statement| statement == binding.witness.guarantee_statement_blake3)
    }) && assumption_statement_identity(
        &binding.consumer_assumption.id,
        &binding.consumer_assumption.statement,
    )? == binding.witness.assumption_statement_blake3;
    if !witness_matches {
        assumption_uses
            .entry(key)
            .or_default()
            .push(ObligationState::Unsupported);
        return Ok(DependencyExplanation {
            id: dependency.id.clone(),
            relation: dependency.relation,
            state: ObligationState::Unsupported,
            assurance: binding.witness.assurance,
            reason: "unsupported: witness_mismatch".into(),
            evidence: None,
            producer_row_blake3: dependency
                .producer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            consumer_row_blake3: dependency
                .consumer
                .requirement
                .requirement
                .row_blake3
                .clone(),
            residual_assumption_ids: residual,
        });
    }
    let state = resolved.state;
    if state == ObligationState::Supported {
        let path = vec![
            dependency.id.clone(),
            binding.producer_guarantee.guarantee_case.clone(),
            binding.producer_guarantee.claim_id.clone(),
        ];
        discharge_paths.entry(key.clone()).or_default().push(path);
        trust_dependencies.insert(format!(
            "semantic-witness:{}:retained-declaration",
            binding.witness.artifact_blake3
        ));
    }
    assumption_uses.entry(key).or_default().push(state);
    Ok(DependencyExplanation {
        id: dependency.id.clone(),
        relation: dependency.relation,
        state,
        assurance: assurance_min(resolved.assurance, binding.witness.assurance),
        reason: if state == ObligationState::Supported {
            "supported: admitted guarantee discharges the consumer assumption under a compatible context".into()
        } else {
            state_reason(
                state,
                &BTreeSet::from([resolved
                    .reason
                    .unwrap_or_else(|| "guarantee_unresolved".into())]),
            )
        },
        evidence: Some(format!(
            "guarantee:{}:{}",
            binding.producer_guarantee.guarantee_case, binding.producer_guarantee.claim_id
        )),
        producer_row_blake3: dependency
            .producer
            .requirement
            .requirement
            .row_blake3
            .clone(),
        consumer_row_blake3: dependency
            .consumer
            .requirement
            .requirement
            .row_blake3
            .clone(),
        residual_assumption_ids: residual,
    })
}

/// Reject a cyclic justification graph and bound its depth.
///
/// Nodes are requirement row identities; edges point parent → child. A node that is
/// revisited while still on the active path closes a cycle. `labels` maps row
/// identities to requirement ids so the error names the offending requirement path.
#[cfg(test)]
mod tests;
