//! Opt-in bounded consistency, nonvacuity and counterexample analysis.
//!
//! Every analysis is a set of separate finite queries over contracts already
//! admitted by [`super::typed_requirement_contracts`]: can the shared
//! assumptions hold, can a state activate each requirement, can the selected
//! guarantees hold together, and does withholding one guarantee admit a
//! previously excluded behavior.
//!
//! A model is one complete total observable table for every selected contract,
//! which is exactly the artifact the typed-contract evaluator accepts. Search is
//! a deterministic depth-first enumeration of observation cells, so a negative
//! answer is only reported when the declared finite space was explored
//! completely. Found models are returned with their exact contract identities and
//! replay through [`replay_witness`], which re-admits the case and the tables.
//!
//! This module is advisory. A finite model is not unbounded validity, reactive
//! realizability, prose correspondence, implementation correctness or proof
//! evidence, and a finding never resolves a native obligation.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use super::typed_requirement_contracts::{
    admit_correspondence_review, evaluate_example, normalize_contract, CorrespondenceReview,
    FiniteExpr, FiniteTerm, FiniteValue, NormalizedContract, ObservableRow, SemanticContract,
    SemanticError, SemanticExample, SemanticProfile, FINITE_SEMANTICS_PROFILE_V1,
};

pub const CONSISTENCY_CASE_SCHEMA_V1: &str = "cairn.requirement-consistency-case.v1";
pub const CONSISTENCY_POLICY_SCHEMA_V1: &str = "cairn.requirement-consistency-policy.v1";
pub const CONSISTENCY_REPORT_SCHEMA_V1: &str = "cairn.requirement-consistency-report.v1";
pub const FRAGMENT_IDENTITY_DOMAIN_V1: &str = "cairn.requirement-fragment.v1";
pub const WITNESS_IDENTITY_DOMAIN_V1: &str = "cairn.requirement-consistency-witness.v1";
pub const INPUT_IDENTITY_DOMAIN_V1: &str = "cairn.requirement-consistency-input.v1";
pub const QUERY_IDENTITY_DOMAIN_V1: &str = "cairn.requirement-consistency-query.v1";
pub const EXAMPLE_IDENTITY_DOMAIN_V1: &str = "cairn.requirement-example.v1";
pub const UNRESOLVED_DISPOSITION: &str = "unresolved";
// r[impl cairn.requirement_consistency.statuses]
pub const CONSISTENCY_BOUNDARY: &str = "Advisory bounded finite analysis only: a found model is one possible model over the declared finite domains and admitted total observable tables, not unbounded validity, reactive realizability, prose correspondence or implementation correctness. Cross-contract observable identity is only what an explicit correspondence declaration establishes; equal observable names in different contracts stay independent. A negative result holds only when the declared finite space was explored completely with no unsupported construct; exhausted bounds, disabled queries and unsupported input are never verdicts. Findings name exact revisions and reproducing assumptions but never resolve native obligations, alter requirement meaning, or change any gate verdict.";

const MAX_CONTRACTS_CEILING: usize = 16;
const MAX_GUARANTEES_CEILING: usize = 16;
const MAX_CELLS_CEILING: usize = 65_536;
const MAX_STATES_CEILING: usize = 1_000_000;
const MAX_EVALUATIONS_CEILING: usize = 5_000_000;
const MAX_DEPTH_CEILING: usize = 64;

/// The five admitted analysis statuses. They describe the queried formula, not
/// the requirement's prose and not any gate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// A model of the exact query was found and retained as a replayable witness.
    Consistent,
    /// The query has no model and the declared finite space was explored completely.
    Inconsistent,
    /// The query's antecedent has no model, so the implication is vacuously true.
    Vacuous,
    /// Exploration was incomplete: a bound was exhausted before a verdict.
    Undetermined,
    /// The query is outside the supported or policy-enabled analysis.
    Unsupported,
}

impl AnalysisStatus {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Consistent => "consistent",
            Self::Inconsistent => "inconsistent",
            Self::Vacuous => "vacuous",
            Self::Undetermined => "undetermined",
            Self::Unsupported => "unsupported",
        }
    }
}

/// How a status was reached. Only `Exhaustive` supports a negative verdict and
/// only `Witness` supports a positive one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchBasisKind {
    Witness,
    Exhaustive,
    Exhausted,
    Unsupported,
}

/// Retained exploration basis: the complete declared domain plus its identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchBasis {
    pub kind: SearchBasisKind,
    pub detail: String,
    pub states_explored: usize,
    pub evaluations: usize,
    pub model_cells: usize,
    pub domains: BTreeMap<String, usize>,
}

/// The admitted query vocabulary. Nickel policy owns which queries are enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisQueryKind {
    AssumptionsSatisfiable,
    ActivationReachable,
    GuaranteesCompatible,
    GuaranteeConstraining,
}

impl AnalysisQueryKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::AssumptionsSatisfiable => "assumptions_satisfiable",
            Self::ActivationReachable => "activation_reachable",
            Self::GuaranteesCompatible => "guarantees_compatible",
            Self::GuaranteeConstraining => "guarantee_constraining",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsistencyBounds {
    pub max_contracts: usize,
    pub max_guarantees: usize,
    pub max_cells: usize,
    pub max_states: usize,
    pub max_evaluations: usize,
    pub max_depth: usize,
}

/// Nickel-authored policy: only the enabled vocabulary and the bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsistencyPolicy {
    pub schema: String,
    pub queries: Vec<AnalysisQueryKind>,
    pub bounds: ConsistencyBounds,
}

/// Caller-declared exact subject scope. Every component is mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisSubject {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
}

/// One contract offered for analysis, with its exact native revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseContract {
    pub native_requirement: ContentBoundRequirementRow,
    pub contract: SemanticContract,
    pub correspondence_review: Option<CorrespondenceReview>,
}

/// Exact native obligation a finding may be attached to. Attachment never
/// changes the obligation's state or disposition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeObligationBinding {
    pub requirement_id: String,
    pub obligation_id: String,
    pub finding_id: String,
}

/// Adapter identity retained with the result. The core does not execute it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerIdentity {
    pub producer: String,
    pub adapter: String,
    pub version: String,
}

/// One contract-local observable named by an explicit correspondence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservableMember {
    pub requirement_id: String,
    pub mapping: String,
}

/// An explicit reviewed claim that two or more contract-local observables denote
/// one shared observation. Without such a claim the analysis assumes no
/// cross-contract identity at all.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservablesCorrespondence {
    pub identity: String,
    pub members: Vec<ObservableMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsistencyCase {
    pub schema: String,
    pub profile: SemanticProfile,
    pub subject: AnalysisSubject,
    pub contracts: Vec<CaseContract>,
    /// Selected requirement ids, in the caller's analysis order.
    pub selected: Vec<String>,
    /// Explicit observable correspondence. Equal observable names in different
    /// contracts are independent unless a declaration unifies them here.
    pub correspondence: Vec<ObservablesCorrespondence>,
    pub authorized_reviewers: Vec<String>,
    pub native_obligations: Vec<NativeObligationBinding>,
    pub producer: ProducerIdentity,
}

/// One exact formula fragment: the requirement revision and the syntactic
/// location inside its normalized contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormulaFragment {
    pub requirement_id: String,
    pub contract_blake3: String,
    pub location: String,
    pub fragment_blake3: String,
    pub expression: FiniteExpr,
}

impl FormulaFragment {
    /// Stable reference used by conflicting subsets and query labels.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}::{}", self.requirement_id, self.location)
    }
}

/// Exact contract scope recorded with the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeContract {
    pub requirement_id: String,
    pub requirement_row_blake3: String,
    pub contract_blake3: String,
    pub interpretation_blake3: String,
    pub mapping_blake3: String,
    pub correspondence: String,
    pub domains: BTreeMap<String, Vec<String>>,
    /// Quantifier variables with the declared domain that binds them. Values are
    /// contract-local: equal names in different contracts are not the same variable.
    pub quantified_variables: BTreeMap<String, String>,
    /// Observable name to the shared observation identity it is bound to. A
    /// contract-local identity means no correspondence was declared.
    pub observable_states: BTreeMap<String, String>,
    pub observable_cells: BTreeMap<String, usize>,
}

/// Everything a query is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisScope {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
    pub semantics_profile: String,
    pub interpretation: String,
    pub selected: Vec<String>,
    pub contracts: Vec<ScopeContract>,
    /// The shared assumptions conjoined across the selected contracts.
    pub environmental_assumptions: Vec<FormulaFragment>,
    pub guarantees: Vec<FormulaFragment>,
    /// Explicit observable correspondence used to build the model.
    pub correspondence: Vec<ObservablesCorrespondence>,
    pub bounds: ConsistencyBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsistencyQuery {
    AssumptionsSatisfiable { contracts: Vec<String> },
    ActivationReachable { requirement_id: String },
    GuaranteesCompatible { guarantees: Vec<String> },
    GuaranteeConstraining { guarantee: String },
}

impl ConsistencyQuery {
    /// Deterministic replay label unique inside one report.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::AssumptionsSatisfiable { contracts } => {
                format!("assumptions_satisfiable:{}", contracts.join(","))
            }
            Self::ActivationReachable { requirement_id } => {
                format!("activation_reachable:{requirement_id}")
            }
            Self::GuaranteesCompatible { guarantees } => {
                format!("guarantees_compatible:{}", guarantees.join(","))
            }
            Self::GuaranteeConstraining { guarantee } => {
                format!("guarantee_constraining:{guarantee}")
            }
        }
    }

    #[must_use]
    pub fn kind(&self) -> AnalysisQueryKind {
        match self {
            Self::AssumptionsSatisfiable { .. } => AnalysisQueryKind::AssumptionsSatisfiable,
            Self::ActivationReachable { .. } => AnalysisQueryKind::ActivationReachable,
            Self::GuaranteesCompatible { .. } => AnalysisQueryKind::GuaranteesCompatible,
            Self::GuaranteeConstraining { .. } => AnalysisQueryKind::GuaranteeConstraining,
        }
    }
}

/// One complete total observable table for one exact contract revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessTable {
    pub requirement_id: String,
    pub contract_blake3: String,
    pub example_blake3: String,
    pub observables: BTreeMap<String, Vec<ObservableRow>>,
}

/// A replayable model: total tables plus their exact contract identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisWitness {
    pub query: String,
    pub tables: Vec<WitnessTable>,
    pub model_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryOutcome {
    pub query: ConsistencyQuery,
    pub query_identity: String,
    pub status: AnalysisStatus,
    pub basis: SearchBasis,
    pub witness: Option<AnalysisWitness>,
    pub detail: String,
}

/// The specific proposition a finding reports. Statuses stay the five query
/// statuses; the finding says which claim of the requirement set it concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    ContradictoryAssumptions,
    UnreachableActivation,
    ActivationReachable,
    ConflictingGuarantees,
    GuaranteesCompatible,
    RedundantGuarantee,
    ConstrainingGuarantee,
    VacuousRequirementSet,
    TrivialQuantification,
    IndistinguishableStates,
    IncompleteAnalysis,
}

impl FindingKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::ContradictoryAssumptions => "contradictory_assumptions",
            Self::UnreachableActivation => "unreachable_activation",
            Self::ActivationReachable => "activation_reachable",
            Self::ConflictingGuarantees => "conflicting_guarantees",
            Self::GuaranteesCompatible => "guarantees_compatible",
            Self::RedundantGuarantee => "redundant_guarantee",
            Self::ConstrainingGuarantee => "constraining_guarantee",
            Self::VacuousRequirementSet => "vacuous_requirement_set",
            Self::TrivialQuantification => "trivial_quantification",
            Self::IndistinguishableStates => "indistinguishable_states",
            Self::IncompleteAnalysis => "incomplete_analysis",
        }
    }
}

/// Retained conflicting subset. Minimality is only claimed when every smaller
/// subset was separately shown satisfiable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictingSubset {
    pub members: Vec<String>,
    pub minimal: bool,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisFinding {
    pub kind: FindingKind,
    pub status: Option<AnalysisStatus>,
    pub contracts: Vec<ScopeContract>,
    pub fragments: Vec<FormulaFragment>,
    pub domains: BTreeMap<String, Vec<String>>,
    pub conflicting: Vec<ConflictingSubset>,
    pub conflicting_exploration_complete: bool,
    pub witness: Option<AnalysisWitness>,
    pub detail: String,
    pub native_obligations: Vec<NativeObligationBinding>,
    pub disposition: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsistencyReport {
    pub schema: String,
    pub advisory: bool,
    pub scope: AnalysisScope,
    pub queries: Vec<QueryOutcome>,
    pub findings: Vec<AnalysisFinding>,
    pub producer: ProducerIdentity,
    pub trust_dependencies: BTreeSet<String>,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

/// One independent check that a retained witness satisfies the exact query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractEvaluation {
    pub requirement_id: String,
    pub contract_blake3: String,
    pub assumptions_hold: bool,
    pub activated: bool,
    pub guarantee_holds: bool,
    pub implication_holds: bool,
    pub steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessReplay {
    pub query: String,
    pub replays: bool,
    pub detail: String,
    pub contract_evaluations: Vec<ContractEvaluation>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum ConsistencyError {
    Unsupported(String),
    InvalidInput(String),
    ScopeMismatch(String),
    BoundExhaustion(String),
    Serialization(String),
}

impl ConsistencyError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::InvalidInput(_) => "invalid_input",
            Self::ScopeMismatch(_) => "scope_mismatch",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::Serialization(_) => "serialization",
        }
    }
}

impl std::fmt::Display for ConsistencyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for ConsistencyError {}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, ConsistencyError> {
    let value = serde_json::to_value(value)
        .map_err(|error| ConsistencyError::Serialization(error.to_string()))?;
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

fn typed_error(error: SemanticError) -> ConsistencyError {
    match error {
        SemanticError::UnsupportedConstruct(detail) => ConsistencyError::Unsupported(detail),
        SemanticError::UnknownProfile(detail) => ConsistencyError::Unsupported(detail),
        SemanticError::BoundExhaustion(detail) => ConsistencyError::BoundExhaustion(detail),
        SemanticError::RequirementRevisionMismatch => {
            ConsistencyError::ScopeMismatch("native requirement revision".into())
        }
        SemanticError::IllTyped(detail)
        | SemanticError::UnboundVariable(detail)
        | SemanticError::InvalidReview(detail)
        | SemanticError::InvalidExample(detail) => ConsistencyError::InvalidInput(detail),
        SemanticError::Serialization(detail) => ConsistencyError::Serialization(detail),
    }
}

/// Admits the policy vocabulary and bounds. Core keeps only implementation
/// ceilings; every admitted number comes from the checked Nickel export.
// r[impl cairn.requirement_consistency.policy]
pub fn admit_consistency_policy(policy: &ConsistencyPolicy) -> Result<(), ConsistencyError> {
    if policy.schema != CONSISTENCY_POLICY_SCHEMA_V1 {
        return Err(ConsistencyError::Unsupported(format!(
            "unsupported consistency policy schema {}",
            policy.schema
        )));
    }
    let bounds = &policy.bounds;
    if bounds.max_contracts == 0
        || bounds.max_contracts > MAX_CONTRACTS_CEILING
        || bounds.max_guarantees == 0
        || bounds.max_guarantees > MAX_GUARANTEES_CEILING
        || bounds.max_cells == 0
        || bounds.max_cells > MAX_CELLS_CEILING
        || bounds.max_states == 0
        || bounds.max_states > MAX_STATES_CEILING
        || bounds.max_evaluations == 0
        || bounds.max_evaluations > MAX_EVALUATIONS_CEILING
        || bounds.max_depth == 0
        || bounds.max_depth > MAX_DEPTH_CEILING
    {
        return Err(ConsistencyError::BoundExhaustion(
            "admitted policy bounds are outside supported implementation ceilings".into(),
        ));
    }
    if policy.queries.is_empty() {
        return Err(ConsistencyError::Unsupported(
            "policy enables no analysis queries".into(),
        ));
    }
    let mut queries = BTreeSet::new();
    for query in &policy.queries {
        if !queries.insert(*query) {
            return Err(ConsistencyError::Unsupported(format!(
                "duplicate admitted query {}",
                query.tag()
            )));
        }
    }
    Ok(())
}

struct AdmittedContract {
    requirement_id: String,
    normalized: NormalizedContract,
    correspondence: &'static str,
    observable_cells: BTreeMap<String, usize>,
    observable_states: BTreeMap<String, String>,
}

/// The shared observation identity of one contract-local observable.
fn state_of_contract<'a>(contract: &'a AdmittedContract, mapping: &str) -> Result<&'a str, Stop> {
    contract
        .observable_states
        .get(mapping)
        .map(String::as_str)
        .ok_or_else(|| {
            Stop::Invalid(format!(
                "observable {mapping} has no admitted model cell identity"
            ))
        })
}

fn state_key(requirement_id: &str, mapping: &str) -> String {
    format!("{requirement_id}#{mapping}")
}

/// Declared domain names and values a shared observation must agree on.
#[derive(PartialEq, Eq)]
struct ObservableShape {
    arguments: Vec<(String, Vec<String>)>,
    result: (String, Vec<String>),
}

/// Admits the explicit cross-contract observable correspondence. Two observables
/// are the same observation only when the declaration names both and their
/// declared argument and result domains agree positionally.
///
/// Without a declaration, equal observable names in different contracts remain
/// independent: the analysis claims no cross-contract identity it was not given.
fn admit_correspondence(
    declared: &[ObservablesCorrespondence],
    contracts: &mut BTreeMap<String, AdmittedContract>,
    selected: &BTreeSet<String>,
) -> Result<Vec<ObservablesCorrespondence>, ConsistencyError> {
    let mut admitted = declared.to_vec();
    admitted.sort();
    admitted.dedup();
    let mut assigned: BTreeMap<String, String> = BTreeMap::new();
    for declaration in &admitted {
        if declaration.identity.trim().is_empty() {
            return Err(ConsistencyError::InvalidInput(
                "observable correspondence requires an explicit identity".into(),
            ));
        }
        if declaration.members.len() < 2 {
            return Err(ConsistencyError::InvalidInput(format!(
                "correspondence {} must unify at least two observables",
                declaration.identity
            )));
        }
        let mut shape: Option<ObservableShape> = None;
        for member in &declaration.members {
            if !selected.contains(&member.requirement_id) {
                return Err(ConsistencyError::ScopeMismatch(format!(
                    "correspondence {} names an unselected requirement {}",
                    declaration.identity, member.requirement_id
                )));
            }
            let contract = contracts.get(&member.requirement_id).ok_or_else(|| {
                ConsistencyError::ScopeMismatch(format!(
                    "correspondence {} names an absent contract {}",
                    declaration.identity, member.requirement_id
                ))
            })?;
            let observable = contract
                .normalized
                .contract
                .observables
                .get(&member.mapping)
                .ok_or_else(|| {
                    ConsistencyError::InvalidInput(format!(
                        "correspondence {} names an undeclared observable {}",
                        declaration.identity, member.mapping
                    ))
                })?;
            let mut argument_shape = Vec::with_capacity(observable.arguments.len());
            for argument in &observable.arguments {
                let domain = contract
                    .normalized
                    .contract
                    .domains
                    .get(argument)
                    .ok_or_else(|| {
                        ConsistencyError::InvalidInput(format!(
                            "correspondence {} names an undeclared domain {argument}",
                            declaration.identity
                        ))
                    })?;
                argument_shape.push((argument.clone(), domain.values.clone()));
            }
            let result_domain = contract
                .normalized
                .contract
                .domains
                .get(&observable.result)
                .ok_or_else(|| {
                    ConsistencyError::InvalidInput(format!(
                        "correspondence {} names an undeclared result domain",
                        declaration.identity
                    ))
                })?;
            let result_shape = (observable.result.clone(), result_domain.values.clone());
            let candidate = ObservableShape {
                arguments: argument_shape,
                result: result_shape,
            };
            if let Some(known) = &shape {
                if known != &candidate {
                    return Err(ConsistencyError::InvalidInput(format!(
                        "correspondence {} unifies observables with different declared domains",
                        declaration.identity
                    )));
                }
            } else {
                shape = Some(candidate);
            }
            let key = state_key(&member.requirement_id, &member.mapping);
            let previous = assigned.insert(key, declaration.identity.clone());
            if previous.is_some_and(|previous| previous != declaration.identity) {
                return Err(ConsistencyError::InvalidInput(format!(
                    "observable {} of {} is bound to two different shared identities",
                    member.mapping, member.requirement_id
                )));
            }
        }
    }
    for contract in contracts.values_mut() {
        let requirement_id = contract.requirement_id.clone();
        for (mapping, state) in contract.observable_states.iter_mut() {
            if let Some(identity) = assigned.get(&state_key(&requirement_id, mapping)) {
                state.clone_from(identity);
            }
        }
    }
    Ok(admitted)
}

struct Admitted {
    contracts: BTreeMap<String, AdmittedContract>,
    selected: Vec<String>,
    scope: AnalysisScope,
    obligations: Vec<NativeObligationBinding>,
    total_cells: usize,
}

impl Admitted {
    fn contract(&self, requirement_id: &str) -> Result<&AdmittedContract, ConsistencyError> {
        self.contracts.get(requirement_id).ok_or_else(|| {
            ConsistencyError::InvalidInput(format!("contract {requirement_id} is not admitted"))
        })
    }

    fn domains(&self) -> BTreeMap<String, usize> {
        self.scope
            .contracts
            .iter()
            .flat_map(|contract| {
                contract.domains.iter().map(move |(name, values)| {
                    (format!("{}#{name}", contract.requirement_id), values.len())
                })
            })
            .collect()
    }

    fn scope_contracts(&self, requirement_ids: &[String]) -> Vec<ScopeContract> {
        self.scope
            .contracts
            .iter()
            .filter(|contract| requirement_ids.contains(&contract.requirement_id))
            .cloned()
            .collect()
    }

    fn domains_of(&self, requirement_ids: &[String]) -> BTreeMap<String, Vec<String>> {
        self.scope
            .contracts
            .iter()
            .filter(|contract| requirement_ids.contains(&contract.requirement_id))
            .flat_map(|contract| {
                contract.domains.iter().map(move |(name, values)| {
                    (
                        format!("{}#{name}", contract.requirement_id),
                        values.clone(),
                    )
                })
            })
            .collect()
    }

    fn obligations_for(&self, requirement_ids: &[String]) -> Vec<NativeObligationBinding> {
        self.obligations
            .iter()
            .filter(|obligation| requirement_ids.contains(&obligation.requirement_id))
            .cloned()
            .collect()
    }
}

/// One quantifier occurrence with its declaring fragment and instance count.
struct QuantifierUse {
    variable: String,
    domain: String,
    location: String,
    instances: usize,
}

fn observable_cells(
    contract: &SemanticContract,
    max_cells: usize,
) -> Result<BTreeMap<String, usize>, ConsistencyError> {
    let mut cells = BTreeMap::new();
    for (name, mapping) in &contract.observables {
        let mut count = 1usize;
        for argument in &mapping.arguments {
            let domain = contract.domains.get(argument).ok_or_else(|| {
                ConsistencyError::InvalidInput(format!("undeclared domain {argument}"))
            })?;
            count = count
                .checked_mul(domain.values.len())
                .filter(|count| *count <= max_cells)
                .ok_or_else(|| {
                    ConsistencyError::BoundExhaustion(format!("observable table {name}"))
                })?;
        }
        cells.insert(name.clone(), count);
    }
    Ok(cells)
}

fn collect_quantifiers(
    contract: &SemanticContract,
    expression: &FiniteExpr,
    location: &str,
    into: &mut Vec<QuantifierUse>,
) -> Result<(), ConsistencyError> {
    match expression {
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
            let instances = contract
                .domains
                .get(domain)
                .ok_or_else(|| {
                    ConsistencyError::InvalidInput(format!("undeclared domain {domain}"))
                })?
                .values
                .len();
            if instances == 0 {
                return Err(ConsistencyError::Unsupported(format!(
                    "quantified domain {domain} has no admitted values"
                )));
            }
            into.push(QuantifierUse {
                variable: variable.clone(),
                domain: domain.clone(),
                location: location.to_string(),
                instances,
            });
            collect_quantifiers(contract, expression, location, into)
        }
        FiniteExpr::Not { expression } => collect_quantifiers(contract, expression, location, into),
        FiniteExpr::And { expressions } | FiniteExpr::Or { expressions } => {
            for expression in expressions {
                collect_quantifiers(contract, expression, location, into)?;
            }
            Ok(())
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            collect_quantifiers(contract, premise, location, into)?;
            collect_quantifiers(contract, conclusion, location, into)
        }
        FiniteExpr::Bool { .. } | FiniteExpr::Equal { .. } | FiniteExpr::Member { .. } => Ok(()),
        FiniteExpr::Unsupported { construct } => {
            Err(ConsistencyError::Unsupported(construct.clone()))
        }
    }
}

fn guarantee_expressions(contract: &SemanticContract) -> Vec<&FiniteExpr> {
    match &contract.guarantee {
        FiniteExpr::And { expressions } if !expressions.is_empty() => expressions.iter().collect(),
        expression => vec![expression],
    }
}

fn fragment(
    contract: &AdmittedContract,
    location: String,
    expression: &FiniteExpr,
) -> Result<FormulaFragment, ConsistencyError> {
    Ok(FormulaFragment {
        requirement_id: contract.requirement_id.clone(),
        contract_blake3: contract.normalized.contract_blake3.clone(),
        location,
        fragment_blake3: identity(FRAGMENT_IDENTITY_DOMAIN_V1, expression)?,
        expression: expression.clone(),
    })
}

fn assumption_fragments(
    contract: &AdmittedContract,
) -> Result<Vec<FormulaFragment>, ConsistencyError> {
    contract
        .normalized
        .contract
        .assumptions
        .iter()
        .enumerate()
        .map(|(index, expression)| fragment(contract, format!("assumptions[{index}]"), expression))
        .collect()
}

fn activation_fragment(contract: &AdmittedContract) -> Result<FormulaFragment, ConsistencyError> {
    fragment(
        contract,
        "activation".into(),
        &contract.normalized.contract.activation,
    )
}

fn guarantee_fragments(
    contract: &AdmittedContract,
) -> Result<Vec<FormulaFragment>, ConsistencyError> {
    guarantee_expressions(&contract.normalized.contract)
        .into_iter()
        .enumerate()
        .map(|(index, expression)| fragment(contract, format!("guarantee[{index}]"), expression))
        .collect()
}

/// The constraint fragments that reproduce a finding: assumptions and
/// activations of the named contracts.
fn base_fragments(
    admitted: &Admitted,
    requirement_ids: &[String],
) -> Result<Vec<FormulaFragment>, ConsistencyError> {
    let mut fragments = Vec::new();
    for requirement_id in requirement_ids {
        let contract = admitted.contract(requirement_id)?;
        fragments.extend(assumption_fragments(contract)?);
        fragments.push(activation_fragment(contract)?);
    }
    Ok(fragments)
}

fn scope_contract(contract: &AdmittedContract, uses: &[QuantifierUse]) -> ScopeContract {
    let mut variables = BTreeMap::new();
    for use_ in uses {
        variables.insert(use_.variable.clone(), use_.domain.clone());
    }
    ScopeContract {
        requirement_id: contract.requirement_id.clone(),
        requirement_row_blake3: contract.normalized.contract.requirement.row_blake3.clone(),
        contract_blake3: contract.normalized.contract_blake3.clone(),
        interpretation_blake3: contract.normalized.interpretation_blake3.clone(),
        mapping_blake3: contract.normalized.mapping_blake3.clone(),
        correspondence: contract.correspondence.into(),
        domains: contract
            .normalized
            .contract
            .domains
            .iter()
            .map(|(name, finite)| (name.clone(), finite.values.clone()))
            .collect(),
        quantified_variables: variables,
        observable_states: contract.observable_states.clone(),
        observable_cells: contract.observable_cells.clone(),
    }
}

/// Collects every quantifier of the analyzed formulas for one contract.
fn quantifier_uses(contract: &AdmittedContract) -> Result<Vec<QuantifierUse>, ConsistencyError> {
    let semantic = &contract.normalized.contract;
    let mut uses = Vec::new();
    for (index, assumption) in semantic.assumptions.iter().enumerate() {
        collect_quantifiers(
            semantic,
            assumption,
            &format!("assumptions[{index}]"),
            &mut uses,
        )?;
    }
    collect_quantifiers(semantic, &semantic.activation, "activation", &mut uses)?;
    for (index, expression) in guarantee_expressions(semantic).into_iter().enumerate() {
        collect_quantifiers(
            semantic,
            expression,
            &format!("guarantee[{index}]"),
            &mut uses,
        )?;
    }
    Ok(uses)
}

// r[impl cairn.requirement_consistency.scope]
fn admit_case(
    policy: &ConsistencyPolicy,
    case: &ConsistencyCase,
) -> Result<Admitted, ConsistencyError> {
    if case.schema != CONSISTENCY_CASE_SCHEMA_V1 {
        return Err(ConsistencyError::Unsupported(format!(
            "unsupported consistency case schema {}",
            case.schema
        )));
    }
    if case.profile.id != FINITE_SEMANTICS_PROFILE_V1 {
        return Err(ConsistencyError::Unsupported(format!(
            "unsupported analysis profile {}",
            case.profile.id
        )));
    }
    if !digest(&case.subject.subject_blake3)
        || !digest(&case.subject.policy_blake3)
        || case.subject.target.trim().is_empty()
        || case.subject.environment.trim().is_empty()
    {
        return Err(ConsistencyError::InvalidInput(
            "explicit subject, policy, target and environment identities are required".into(),
        ));
    }
    if case.producer.producer.trim().is_empty()
        || case.producer.adapter.trim().is_empty()
        || case.producer.version.trim().is_empty()
    {
        return Err(ConsistencyError::InvalidInput(
            "producer, adapter and version identity are required".into(),
        ));
    }
    if case.contracts.is_empty() || case.contracts.len() > policy.bounds.max_contracts {
        return Err(ConsistencyError::BoundExhaustion(
            "contract count is outside the admitted bound".into(),
        ));
    }
    if case.selected.is_empty() || case.selected.len() > policy.bounds.max_contracts {
        return Err(ConsistencyError::BoundExhaustion(
            "selected requirement count is outside the admitted bound".into(),
        ));
    }
    let mut contracts = BTreeMap::new();
    let mut offered = BTreeSet::new();
    for offered_contract in &case.contracts {
        let requirement_id = offered_contract.native_requirement.requirement_id.clone();
        if !offered.insert(requirement_id.clone()) {
            return Err(ConsistencyError::InvalidInput(format!(
                "duplicate contract for {requirement_id}"
            )));
        }
        let normalized = normalize_contract(
            &case.profile,
            &offered_contract.native_requirement,
            &offered_contract.contract,
        )
        .map_err(typed_error)?;
        let correspondence = match &offered_contract.correspondence_review {
            Some(review) => {
                admit_correspondence_review(&normalized, review, &case.authorized_reviewers)
                    .map_err(typed_error)?;
                "reviewed_declaration"
            }
            None => "not_reviewed",
        };
        let cells = observable_cells(&normalized.contract, policy.bounds.max_cells)?;
        let states = normalized
            .contract
            .observables
            .keys()
            .map(|mapping| (mapping.clone(), state_key(&requirement_id, mapping)))
            .collect();
        contracts.insert(
            requirement_id.clone(),
            AdmittedContract {
                requirement_id,
                normalized,
                correspondence,
                observable_cells: cells,
                observable_states: states,
            },
        );
    }
    let mut selected_seen = BTreeSet::new();
    for requirement_id in &case.selected {
        if !contracts.contains_key(requirement_id) {
            return Err(ConsistencyError::ScopeMismatch(format!(
                "selected requirement {requirement_id} has no offered contract"
            )));
        }
        if !selected_seen.insert(requirement_id.clone()) {
            return Err(ConsistencyError::InvalidInput(
                "duplicate selected requirement".into(),
            ));
        }
    }
    let correspondence =
        admit_correspondence(&case.correspondence, &mut contracts, &selected_seen)?;
    let mut total_cells = 0usize;
    let mut guarantees = Vec::new();
    let mut selected = Vec::with_capacity(case.selected.len());
    let mut scope_contracts = Vec::with_capacity(case.selected.len());
    let mut environmental_assumptions = Vec::new();
    for requirement_id in &case.selected {
        let contract = contracts.get(requirement_id).ok_or_else(|| {
            ConsistencyError::InvalidInput("selected contract is unavailable".into())
        })?;
        total_cells = total_cells
            .checked_add(contract.observable_cells.values().sum::<usize>())
            .filter(|total| *total <= policy.bounds.max_cells)
            .ok_or_else(|| {
                ConsistencyError::BoundExhaustion("selected observable model cells".into())
            })?;
        guarantees.extend(guarantee_fragments(contract)?);
        environmental_assumptions.extend(assumption_fragments(contract)?);
        selected.push(requirement_id.clone());
        scope_contracts.push(scope_contract(contract, &quantifier_uses(contract)?));
    }
    if guarantees.len() > policy.bounds.max_guarantees {
        return Err(ConsistencyError::BoundExhaustion(
            "selected guarantee count is outside the admitted bound".into(),
        ));
    }
    let mut obligations = case.native_obligations.clone();
    for obligation in &obligations {
        if !selected_seen.contains(&obligation.requirement_id)
            || obligation.obligation_id.trim().is_empty()
            || obligation.finding_id.trim().is_empty()
        {
            return Err(ConsistencyError::InvalidInput(
                "native obligation binding must name a selected requirement and exact ids".into(),
            ));
        }
    }
    obligations.sort();
    obligations.dedup();
    let scope = AnalysisScope {
        subject_blake3: case.subject.subject_blake3.clone(),
        policy_blake3: case.subject.policy_blake3.clone(),
        target: case.subject.target.clone(),
        environment: case.subject.environment.clone(),
        semantics_profile: case.profile.id.clone(),
        interpretation: case.profile.interpretation.clone(),
        selected: selected.clone(),
        contracts: scope_contracts,
        environmental_assumptions,
        guarantees,
        correspondence,
        bounds: policy.bounds.clone(),
    };
    Ok(Admitted {
        contracts,
        selected,
        scope,
        obligations,
        total_cells,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

impl Truth {
    fn negate(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

/// One free observation cell: a shared observation applied to ground values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CellKey {
    state: String,
    arguments: Vec<FiniteValue>,
}

/// The first unassigned cell a goal depends on, with the exact result domain
/// values its referencing contract declares.
struct PendingCell {
    cell: CellKey,
    domain: String,
    values: Vec<String>,
}

#[derive(Debug)]
enum Stop {
    Exhausted(&'static str),
    Unsupported(String),
    Invalid(String),
}

struct Work {
    evaluations: usize,
    max_evaluations: usize,
    states: usize,
    max_states: usize,
    unknown: Option<PendingCell>,
    depth: usize,
    max_depth: usize,
}

impl Work {
    fn new(policy: &ConsistencyPolicy) -> Self {
        Self {
            evaluations: 0,
            max_evaluations: policy.bounds.max_evaluations,
            states: 0,
            max_states: policy.bounds.max_states,
            unknown: None,
            depth: 0,
            max_depth: policy.bounds.max_depth,
        }
    }

    fn charge(&mut self) -> Result<(), Stop> {
        self.evaluations += 1;
        if self.evaluations > self.max_evaluations {
            return Err(Stop::Exhausted("finite analysis evaluations"));
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), Stop> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(Stop::Exhausted("analysis expression nesting"));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn record(&mut self, cell: CellKey, domain: String, values: Vec<String>) {
        if self.unknown.is_none() {
            self.unknown = Some(PendingCell {
                cell,
                domain,
                values,
            });
        }
    }
}

fn quantified_values<'a>(
    contract: &'a AdmittedContract,
    domain: &str,
) -> Result<&'a [String], Stop> {
    let finite = contract
        .normalized
        .contract
        .domains
        .get(domain)
        .ok_or_else(|| Stop::Invalid(format!("undeclared quantified domain {domain}")))?;
    if finite.values.is_empty() {
        return Err(Stop::Unsupported(format!(
            "quantified domain {domain} has no admitted values"
        )));
    }
    Ok(&finite.values)
}

fn term_value(
    contract: &AdmittedContract,
    term: &FiniteTerm,
    environment: &BTreeMap<String, FiniteValue>,
    cells: &BTreeMap<CellKey, FiniteValue>,
    work: &mut Work,
) -> Result<Option<FiniteValue>, Stop> {
    work.charge()?;
    work.enter()?;
    let result = term_value_inner(contract, term, environment, cells, work);
    work.leave();
    result
}

fn term_value_inner(
    contract: &AdmittedContract,
    term: &FiniteTerm,
    environment: &BTreeMap<String, FiniteValue>,
    cells: &BTreeMap<CellKey, FiniteValue>,
    work: &mut Work,
) -> Result<Option<FiniteValue>, Stop> {
    match term {
        FiniteTerm::Literal { value } => Ok(Some(value.clone())),
        FiniteTerm::Variable { name } => environment
            .get(name)
            .cloned()
            .map(Some)
            .ok_or_else(|| Stop::Invalid(format!("unbound quantified variable {name}"))),
        FiniteTerm::Observable { mapping, arguments } => {
            let observable = contract
                .normalized
                .contract
                .observables
                .get(mapping)
                .ok_or_else(|| Stop::Invalid(format!("undeclared observable {mapping}")))?;
            let mut values = Vec::with_capacity(arguments.len());
            for argument in arguments {
                match term_value(contract, argument, environment, cells, work)? {
                    Some(value) => values.push(value),
                    None => return Ok(None),
                }
            }
            let state = state_of_contract(contract, mapping)?;
            let cell = CellKey {
                state: state.to_string(),
                arguments: values,
            };
            match cells.get(&cell) {
                Some(value) => Ok(Some(value.clone())),
                None => {
                    let domain = observable.result.clone();
                    let declared = quantified_values(contract, &domain)?.to_vec();
                    work.record(cell, domain, declared);
                    Ok(None)
                }
            }
        }
        FiniteTerm::Unsupported { construct } => Err(Stop::Unsupported(construct.clone())),
    }
}

fn expression_truth(
    contract: &AdmittedContract,
    expression: &FiniteExpr,
    environment: &mut BTreeMap<String, FiniteValue>,
    cells: &BTreeMap<CellKey, FiniteValue>,
    work: &mut Work,
) -> Result<Truth, Stop> {
    work.charge()?;
    work.enter()?;
    let result = expression_truth_inner(contract, expression, environment, cells, work);
    work.leave();
    result
}

fn expression_truth_inner(
    contract: &AdmittedContract,
    expression: &FiniteExpr,
    environment: &mut BTreeMap<String, FiniteValue>,
    cells: &BTreeMap<CellKey, FiniteValue>,
    work: &mut Work,
) -> Result<Truth, Stop> {
    match expression {
        FiniteExpr::Bool { value } => Ok(if *value { Truth::True } else { Truth::False }),
        FiniteExpr::Equal { left, right } => {
            let left = term_value(contract, left, environment, cells, work)?;
            let right = term_value(contract, right, environment, cells, work)?;
            match (left, right) {
                (Some(left), Some(right)) => Ok(if left == right {
                    Truth::True
                } else {
                    Truth::False
                }),
                _ => Ok(Truth::Unknown),
            }
        }
        FiniteExpr::Member { term, values } => {
            match term_value(contract, term, environment, cells, work)? {
                Some(value) => Ok(if values.contains(&value) {
                    Truth::True
                } else {
                    Truth::False
                }),
                None => Ok(Truth::Unknown),
            }
        }
        FiniteExpr::Not { expression } => {
            Ok(expression_truth(contract, expression, environment, cells, work)?.negate())
        }
        FiniteExpr::And { expressions } => {
            let mut truth = Truth::True;
            for expression in expressions {
                match expression_truth(contract, expression, environment, cells, work)? {
                    Truth::False => return Ok(Truth::False),
                    Truth::Unknown => truth = Truth::Unknown,
                    Truth::True => {}
                }
            }
            Ok(truth)
        }
        FiniteExpr::Or { expressions } => {
            let mut truth = Truth::False;
            for expression in expressions {
                match expression_truth(contract, expression, environment, cells, work)? {
                    Truth::True => return Ok(Truth::True),
                    Truth::Unknown => truth = Truth::Unknown,
                    Truth::False => {}
                }
            }
            Ok(truth)
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            let premise = expression_truth(contract, premise, environment, cells, work)?;
            if premise == Truth::False {
                return Ok(Truth::True);
            }
            let conclusion = expression_truth(contract, conclusion, environment, cells, work)?;
            Ok(match (premise, conclusion) {
                (_, Truth::True) => Truth::True,
                (Truth::True, Truth::False) => Truth::False,
                _ => Truth::Unknown,
            })
        }
        FiniteExpr::Forall {
            variable,
            domain,
            expression,
        } => {
            let values = quantified_values(contract, domain)?;
            let mut truth = Truth::True;
            for value in values {
                environment.insert(
                    variable.clone(),
                    FiniteValue {
                        domain: domain.clone(),
                        value: value.clone(),
                    },
                );
                let inner = expression_truth(contract, expression, environment, cells, work);
                environment.remove(variable);
                match inner? {
                    Truth::False => return Ok(Truth::False),
                    Truth::Unknown => truth = Truth::Unknown,
                    Truth::True => {}
                }
            }
            Ok(truth)
        }
        FiniteExpr::Exists {
            variable,
            domain,
            expression,
        } => {
            let values = quantified_values(contract, domain)?;
            let mut truth = Truth::False;
            for value in values {
                environment.insert(
                    variable.clone(),
                    FiniteValue {
                        domain: domain.clone(),
                        value: value.clone(),
                    },
                );
                let inner = expression_truth(contract, expression, environment, cells, work);
                environment.remove(variable);
                match inner? {
                    Truth::True => return Ok(Truth::True),
                    Truth::Unknown => truth = Truth::Unknown,
                    Truth::False => {}
                }
            }
            Ok(truth)
        }
        FiniteExpr::Unsupported { construct } => Err(Stop::Unsupported(construct.clone())),
    }
}

#[derive(Debug, Clone, Copy)]
enum GoalTerm<'a> {
    Holds {
        requirement_id: &'a str,
        expression: &'a FiniteExpr,
    },
    Fails {
        requirement_id: &'a str,
        expression: &'a FiniteExpr,
    },
}

struct Goal<'a> {
    terms: Vec<GoalTerm<'a>>,
}

impl Goal<'_> {
    fn evaluate(
        &self,
        contracts: &BTreeMap<String, AdmittedContract>,
        cells: &BTreeMap<CellKey, FiniteValue>,
        work: &mut Work,
    ) -> Result<Truth, Stop> {
        let mut truth = Truth::True;
        for term in &self.terms {
            let (requirement_id, expression, expected) = match term {
                GoalTerm::Holds {
                    requirement_id,
                    expression,
                } => (*requirement_id, *expression, Truth::True),
                GoalTerm::Fails {
                    requirement_id,
                    expression,
                } => (*requirement_id, *expression, Truth::False),
            };
            let contract = contracts.get(requirement_id).ok_or_else(|| {
                Stop::Invalid(format!("contract {requirement_id} is not admitted"))
            })?;
            let mut environment = BTreeMap::new();
            let value = expression_truth(contract, expression, &mut environment, cells, work)?;
            if value == Truth::Unknown {
                truth = Truth::Unknown;
            } else if value != expected {
                return Ok(Truth::False);
            }
        }
        Ok(truth)
    }
}

/// Explores the declared finite observation space depth-first. Every cell is an
/// independent total-table entry, so branching over all values of the first
/// unassigned cell that affects the goal is complete.
fn solve(
    goal: &Goal<'_>,
    contracts: &BTreeMap<String, AdmittedContract>,
    work: &mut Work,
) -> Result<Option<BTreeMap<CellKey, FiniteValue>>, Stop> {
    let mut cells: BTreeMap<CellKey, FiniteValue> = BTreeMap::new();
    let mut frames: Vec<(CellKey, String, std::vec::IntoIter<String>)> = Vec::new();
    loop {
        work.unknown = None;
        match goal.evaluate(contracts, &cells, work)? {
            Truth::True => return Ok(Some(cells)),
            Truth::False => {}
            Truth::Unknown => {
                let pending = work.unknown.take().ok_or_else(|| {
                    Stop::Invalid("unknown query truth without a pending observation".into())
                })?;
                cells.remove(&pending.cell);
                frames.push((pending.cell, pending.domain, pending.values.into_iter()));
            }
        }
        loop {
            let next = match frames.last_mut() {
                Some((cell, domain, values)) => {
                    cells.remove(cell);
                    values
                        .next()
                        .map(|value| (cell.clone(), domain.clone(), value))
                }
                None => return Ok(None),
            };
            let Some((cell, domain, value)) = next else {
                frames.pop();
                continue;
            };
            if work.states >= work.max_states {
                return Err(Stop::Exhausted("explored finite analysis states"));
            }
            work.states += 1;
            cells.insert(cell, FiniteValue { domain, value });
            break;
        }
    }
}

/// Canonical total tables: every observable cell of every selected contract,
/// with unconstrained cells completed by the first declared value.
fn total_tables(
    admitted: &Admitted,
    cells: &BTreeMap<CellKey, FiniteValue>,
) -> Result<Vec<WitnessTable>, ConsistencyError> {
    let mut tables = Vec::with_capacity(admitted.selected.len());
    for requirement_id in &admitted.selected {
        let contract = admitted.contract(requirement_id)?;
        let semantic = &contract.normalized.contract;
        let mut observables = BTreeMap::new();
        for (name, mapping) in &semantic.observables {
            let state = contract.observable_states.get(name).ok_or_else(|| {
                ConsistencyError::InvalidInput(format!(
                    "observable {name} has no admitted model cell identity"
                ))
            })?;
            let mut rows = Vec::new();
            for arguments in argument_tuples(semantic, &mapping.arguments)? {
                let cell = CellKey {
                    state: state.clone(),
                    arguments: arguments.clone(),
                };
                let value = match cells.get(&cell) {
                    Some(value) => value.clone(),
                    None => {
                        let domain = semantic.domains.get(&mapping.result).ok_or_else(|| {
                            ConsistencyError::InvalidInput("undeclared result domain".into())
                        })?;
                        FiniteValue {
                            domain: mapping.result.clone(),
                            value: domain.values.first().cloned().ok_or_else(|| {
                                ConsistencyError::Unsupported(format!(
                                    "result domain {} has no admitted values",
                                    mapping.result
                                ))
                            })?,
                        }
                    }
                };
                rows.push(ObservableRow { arguments, value });
            }
            rows.sort_by(|left, right| left.arguments.cmp(&right.arguments));
            observables.insert(name.clone(), rows);
        }
        let example = SemanticExample {
            observables: observables.clone(),
        };
        tables.push(WitnessTable {
            requirement_id: requirement_id.clone(),
            contract_blake3: contract.normalized.contract_blake3.clone(),
            example_blake3: identity(EXAMPLE_IDENTITY_DOMAIN_V1, &example)?,
            observables,
        });
    }
    Ok(tables)
}

fn argument_tuples(
    contract: &SemanticContract,
    domains: &[String],
) -> Result<Vec<Vec<FiniteValue>>, ConsistencyError> {
    let Some((first, rest)) = domains.split_first() else {
        return Ok(vec![Vec::new()]);
    };
    let domain = contract
        .domains
        .get(first)
        .ok_or_else(|| ConsistencyError::InvalidInput(format!("undeclared domain {first}")))?;
    let suffixes = argument_tuples(contract, rest)?;
    let mut tuples = Vec::new();
    for value in &domain.values {
        for suffix in &suffixes {
            let mut tuple = Vec::with_capacity(domains.len());
            tuple.push(FiniteValue {
                domain: first.clone(),
                value: value.clone(),
            });
            tuple.extend(suffix.iter().cloned());
            tuples.push(tuple);
        }
    }
    Ok(tuples)
}

fn witness_for(
    admitted: &Admitted,
    query: &ConsistencyQuery,
    cells: &BTreeMap<CellKey, FiniteValue>,
) -> Result<AnalysisWitness, ConsistencyError> {
    let tables = total_tables(admitted, cells)?;
    let query = query.label();
    let model_blake3 = identity(WITNESS_IDENTITY_DOMAIN_V1, &(&query, &tables))?;
    Ok(AnalysisWitness {
        query,
        tables,
        model_blake3,
    })
}

/// What an unsatisfiable goal means for the claim under test.
///
/// `Terms` exists because vacuity is only sound when the premise set it rests
/// on has a model: `guarantees compatible` is unsatisfiable either because the
/// guarantees conflict or because the shared assumptions and activations have
/// no model, and only the second is vacuity. When no planned query isolates
/// those premise terms, they are searched directly.
enum VacuityBasis<'a> {
    /// Unsatisfiability of the goal is itself the vacuous verdict.
    Unconditional,
    /// Vacuity relative to a premise set searched here.
    Terms(Goal<'a>),
}

/// One planned query: its exact goal and what an unsatisfiable goal means.
struct PlannedQuery<'a> {
    query: ConsistencyQuery,
    goal: Goal<'a>,
    /// `None` means an unsatisfiable goal is inconsistent, never vacuous.
    vacuity: Option<VacuityBasis<'a>>,
}

fn assumption_terms(admitted: &Admitted) -> Vec<GoalTerm<'_>> {
    let mut terms = Vec::new();
    for requirement_id in &admitted.selected {
        if let Ok(contract) = admitted.contract(requirement_id) {
            for expression in &contract.normalized.contract.assumptions {
                terms.push(GoalTerm::Holds {
                    requirement_id: contract.requirement_id.as_str(),
                    expression,
                });
            }
        }
    }
    terms
}

fn activation_terms(admitted: &Admitted) -> Vec<GoalTerm<'_>> {
    let mut terms = Vec::new();
    for requirement_id in &admitted.selected {
        if let Ok(contract) = admitted.contract(requirement_id) {
            terms.push(GoalTerm::Holds {
                requirement_id: contract.requirement_id.as_str(),
                expression: &contract.normalized.contract.activation,
            });
        }
    }
    terms
}

/// Guarantee fragments with their stable per-contract reference.
fn guarantee_terms(admitted: &Admitted) -> Vec<(&str, &FiniteExpr, String)> {
    let mut terms = Vec::new();
    for requirement_id in &admitted.selected {
        if let Ok(contract) = admitted.contract(requirement_id) {
            for (index, expression) in guarantee_expressions(&contract.normalized.contract)
                .into_iter()
                .enumerate()
            {
                terms.push((
                    contract.requirement_id.as_str(),
                    expression,
                    format!("{}::guarantee[{index}]", contract.requirement_id),
                ));
            }
        }
    }
    terms
}

fn planned_queries(admitted: &Admitted) -> Vec<PlannedQuery<'_>> {
    let assumptions = assumption_terms(admitted);
    let activations = activation_terms(admitted);
    let guarantees = guarantee_terms(admitted);
    let mut plans = Vec::new();
    plans.push(PlannedQuery {
        query: ConsistencyQuery::AssumptionsSatisfiable {
            contracts: admitted.selected.clone(),
        },
        goal: Goal {
            terms: assumptions.clone(),
        },
        vacuity: None,
    });
    for requirement_id in &admitted.selected {
        let Ok(contract) = admitted.contract(requirement_id) else {
            continue;
        };
        let mut terms: Vec<GoalTerm<'_>> = contract
            .normalized
            .contract
            .assumptions
            .iter()
            .map(|expression| GoalTerm::Holds {
                requirement_id: contract.requirement_id.as_str(),
                expression,
            })
            .collect();
        terms.push(GoalTerm::Holds {
            requirement_id: contract.requirement_id.as_str(),
            expression: &contract.normalized.contract.activation,
        });
        plans.push(PlannedQuery {
            query: ConsistencyQuery::ActivationReachable {
                requirement_id: requirement_id.clone(),
            },
            goal: Goal { terms },
            vacuity: Some(VacuityBasis::Unconditional),
        });
    }
    let mut shared = assumptions.clone();
    shared.extend(activations.iter().copied());
    let shared_premise = Goal {
        terms: shared.clone(),
    };
    let mut compatible = shared;
    compatible.extend(
        guarantees
            .iter()
            .map(|(requirement_id, expression, _)| GoalTerm::Holds {
                requirement_id,
                expression,
            }),
    );
    let guarantee_references: Vec<String> = guarantees
        .iter()
        .map(|(_, _, reference)| reference.clone())
        .collect();
    plans.push(PlannedQuery {
        query: ConsistencyQuery::GuaranteesCompatible {
            guarantees: guarantee_references.clone(),
        },
        goal: Goal { terms: compatible },
        vacuity: Some(VacuityBasis::Terms(shared_premise)),
    });
    for (index, (requirement_id, expression, reference)) in guarantees.iter().enumerate() {
        let mut terms = assumptions.clone();
        terms.extend(activations.iter().copied());
        for (other_index, (other_requirement_id, other_expression, _)) in
            guarantees.iter().enumerate()
        {
            if other_index != index {
                terms.push(GoalTerm::Holds {
                    requirement_id: other_requirement_id,
                    expression: other_expression,
                });
            }
        }
        terms.push(GoalTerm::Fails {
            requirement_id,
            expression,
        });
        plans.push(PlannedQuery {
            query: ConsistencyQuery::GuaranteeConstraining {
                guarantee: reference.clone(),
            },
            goal: Goal { terms },
            vacuity: None,
        });
    }
    plans
}

fn status_detail(status: AnalysisStatus) -> &'static str {
    match status {
        AnalysisStatus::Consistent => {
            "a model of the exact query was found and retained as a replayable witness"
        }
        AnalysisStatus::Inconsistent => {
            "no model exists in the completely explored declared finite space"
        }
        AnalysisStatus::Vacuous => {
            "the query's antecedent has no model, so the implication is vacuously true within the declared scope"
        }
        AnalysisStatus::Undetermined => {
            "a declared bound was exhausted before the query could be decided"
        }
        AnalysisStatus::Unsupported => {
            "the query is outside the supported or policy-enabled analysis"
        }
    }
}

// r[impl cairn.requirement_consistency.queries]
fn run_query(
    admitted: &Admitted,
    policy: &ConsistencyPolicy,
    work: &mut Work,
    plan: &PlannedQuery<'_>,
) -> Result<QueryOutcome, ConsistencyError> {
    let started_states = work.states;
    let started_evaluations = work.evaluations;
    let mut basis = SearchBasis {
        kind: SearchBasisKind::Exhaustive,
        detail: String::new(),
        states_explored: 0,
        evaluations: 0,
        model_cells: admitted.total_cells,
        domains: admitted.domains(),
    };
    let (status, witness, detail) = if !policy.queries.contains(&plan.query.kind()) {
        basis.kind = SearchBasisKind::Unsupported;
        basis.detail = format!(
            "query kind {} is not enabled by the admitted policy",
            plan.query.kind().tag()
        );
        (
            AnalysisStatus::Unsupported,
            None,
            status_detail(AnalysisStatus::Unsupported),
        )
    } else {
        match solve(&plan.goal, &admitted.contracts, work) {
            Ok(Some(cells)) => {
                basis.kind = SearchBasisKind::Witness;
                basis.detail =
                    "goal-directed search found a model; total tables are retained for replay"
                        .into();
                (
                    AnalysisStatus::Consistent,
                    Some(witness_for(admitted, &plan.query, &cells)?),
                    status_detail(AnalysisStatus::Consistent),
                )
            }
            Ok(None) => {
                let mut premise_detail: Option<(SearchBasisKind, String)> = None;
                let status = match &plan.vacuity {
                    None => AnalysisStatus::Inconsistent,
                    Some(VacuityBasis::Unconditional) => AnalysisStatus::Vacuous,
                    Some(VacuityBasis::Terms(goal)) => {
                        match solve(goal, &admitted.contracts, work) {
                            // A model of the shared premise means the guarantees
                            // themselves conflict, which is an inconsistency.
                            Ok(Some(_)) => AnalysisStatus::Inconsistent,
                            // No model of the shared premise means every guarantee
                            // is vacuously implied.
                            Ok(None) => AnalysisStatus::Vacuous,
                            Err(Stop::Exhausted(detail)) => {
                                premise_detail = Some((
                                    SearchBasisKind::Exhausted,
                                    format!(
                                        "{detail} bound exhausted while checking the premise the vacuity claim rests on"
                                    ),
                                ));
                                AnalysisStatus::Undetermined
                            }
                            Err(Stop::Unsupported(detail)) => {
                                premise_detail = Some((SearchBasisKind::Unsupported, detail));
                                AnalysisStatus::Unsupported
                            }
                            Err(Stop::Invalid(detail)) => {
                                return Err(ConsistencyError::InvalidInput(detail))
                            }
                        }
                    }
                };
                if let Some((kind, detail)) = premise_detail {
                    basis.kind = kind;
                    basis.detail = detail;
                } else if matches!(
                    status,
                    AnalysisStatus::Inconsistent | AnalysisStatus::Vacuous
                ) {
                    basis.kind = SearchBasisKind::Exhaustive;
                    basis.detail = format!(
                        "complete exploration of {} independent observation cells under the declared finite domains",
                        admitted.total_cells
                    );
                }
                (status, None, status_detail(status))
            }
            Err(Stop::Exhausted(detail)) => {
                basis.kind = SearchBasisKind::Exhausted;
                basis.detail = format!("{detail} bound exhausted");
                (
                    AnalysisStatus::Undetermined,
                    None,
                    status_detail(AnalysisStatus::Undetermined),
                )
            }
            Err(Stop::Unsupported(detail)) => {
                basis.kind = SearchBasisKind::Unsupported;
                basis.detail = detail;
                (
                    AnalysisStatus::Unsupported,
                    None,
                    status_detail(AnalysisStatus::Unsupported),
                )
            }
            Err(Stop::Invalid(detail)) => return Err(ConsistencyError::InvalidInput(detail)),
        }
    };
    basis.states_explored = work.states - started_states;
    basis.evaluations = work.evaluations - started_evaluations;
    Ok(QueryOutcome {
        query_identity: identity(
            QUERY_IDENTITY_DOMAIN_V1,
            &(&plan.query, &admitted.scope, &basis.domains),
        )?,
        query: plan.query.clone(),
        status,
        basis,
        witness,
        detail: detail.into(),
    })
}

fn outcome_for<'a>(
    queries: &'a [QueryOutcome],
    query: &ConsistencyQuery,
) -> Result<&'a QueryOutcome, ConsistencyError> {
    let label = query.label();
    queries
        .iter()
        .find(|outcome| outcome.query.label() == label)
        .ok_or_else(|| {
            ConsistencyError::InvalidInput(format!("planned query {label} has no outcome"))
        })
}

/// Pairwise conflicting subsets. A pair is minimal only when both singletons
/// were separately shown satisfiable with the same base.
fn conflicting_subsets(
    admitted: &Admitted,
    work: &mut Work,
    base: &[GoalTerm<'_>],
    members: &[(&str, &FiniteExpr, String)],
) -> (Vec<ConflictingSubset>, bool) {
    let mut conflicts = Vec::new();
    let mut satisfiable = vec![false; members.len()];
    for (index, (requirement_id, expression, reference)) in members.iter().enumerate() {
        let mut terms = base.to_vec();
        terms.push(GoalTerm::Holds {
            requirement_id,
            expression,
        });
        match solve(&Goal { terms }, &admitted.contracts, work) {
            Ok(Some(_)) => satisfiable[index] = true,
            Ok(None) => conflicts.push(ConflictingSubset {
                members: vec![reference.clone()],
                minimal: true,
                basis: "this member alone has no model under the shared base".into(),
            }),
            Err(_) => return (conflicts, false),
        }
    }
    for (left, (left_requirement, left_expression, left_reference)) in members.iter().enumerate() {
        if !satisfiable[left] {
            continue;
        }
        for (right, (right_requirement, right_expression, right_reference)) in
            members.iter().enumerate().skip(left + 1)
        {
            if !satisfiable[right] {
                continue;
            }
            let mut terms = base.to_vec();
            terms.push(GoalTerm::Holds {
                requirement_id: left_requirement,
                expression: left_expression,
            });
            terms.push(GoalTerm::Holds {
                requirement_id: right_requirement,
                expression: right_expression,
            });
            match solve(&Goal { terms }, &admitted.contracts, work) {
                Ok(None) => conflicts.push(ConflictingSubset {
                    members: vec![left_reference.clone(), right_reference.clone()],
                    minimal: true,
                    basis: "each member is satisfiable with the shared base; the pair is not"
                        .into(),
                }),
                Ok(Some(_)) => {}
                Err(_) => return (conflicts, false),
            }
        }
    }
    (conflicts, true)
}

#[allow(clippy::too_many_arguments)]
fn finding(
    admitted: &Admitted,
    kind: FindingKind,
    status: Option<AnalysisStatus>,
    requirement_ids: &[String],
    fragments: Vec<FormulaFragment>,
    conflicts: (Vec<ConflictingSubset>, bool),
    witness: Option<AnalysisWitness>,
    detail: String,
) -> AnalysisFinding {
    let mut fragments = fragments;
    fragments.sort_by_key(|fragment| fragment.reference());
    fragments.dedup_by(|left, right| left.reference() == right.reference());
    AnalysisFinding {
        kind,
        status,
        contracts: admitted.scope_contracts(requirement_ids),
        fragments,
        domains: admitted.domains_of(requirement_ids),
        conflicting: conflicts.0,
        conflicting_exploration_complete: conflicts.1,
        witness,
        detail,
        native_obligations: admitted.obligations_for(requirement_ids),
        disposition: UNRESOLVED_DISPOSITION.into(),
        boundary: CONSISTENCY_BOUNDARY.into(),
    }
}

fn fragment_at(contract: &AdmittedContract, location: &str) -> Option<FormulaFragment> {
    let semantic = &contract.normalized.contract;
    let expression = if location == "activation" {
        Some(&semantic.activation)
    } else if let Some(index) = location
        .strip_prefix("assumptions[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|index| index.parse::<usize>().ok())
    {
        semantic.assumptions.get(index)
    } else {
        let index = location
            .strip_prefix("guarantee[")
            .and_then(|rest| rest.strip_suffix(']'))
            .and_then(|index| index.parse::<usize>().ok())?;
        guarantee_expressions(semantic).get(index).copied()
    };
    expression.and_then(|expression| fragment(contract, location.to_string(), expression).ok())
}

/// Static diagnostics that need no search: single-instance quantifiers and
/// domains that no observable of the contract accepts as an argument.
// r[impl cairn.requirement_consistency.nonvacuity]
fn structural_findings(admitted: &Admitted) -> Result<Vec<AnalysisFinding>, ConsistencyError> {
    let mut findings = Vec::new();
    for requirement_id in &admitted.selected {
        let contract = admitted.contract(requirement_id)?;
        let uses = quantifier_uses(contract)?;
        let observed: BTreeSet<&str> = contract
            .normalized
            .contract
            .observables
            .values()
            .flat_map(|observable| observable.arguments.iter().map(String::as_str))
            .collect();
        let mut trivial_seen = BTreeSet::new();
        let mut unobserved = BTreeSet::new();
        for use_ in &uses {
            if use_.instances == 1
                && trivial_seen.insert((
                    use_.location.clone(),
                    use_.variable.clone(),
                    use_.domain.clone(),
                ))
            {
                findings.push(finding(
                    admitted,
                    FindingKind::TrivialQuantification,
                    None,
                    std::slice::from_ref(requirement_id),
                    fragment_at(contract, &use_.location).into_iter().collect(),
                    (Vec::new(), true),
                    None,
                    format!(
                        "quantifier {}.{} ranges over the single-valued domain {}; the quantified claim has one instance, so it cannot distinguish any state inside the declared scope",
                        use_.location, use_.variable, use_.domain
                    ),
                ));
            }
            if !observed.contains(use_.domain.as_str()) && unobserved.insert(use_.domain.clone()) {
                let fragments = uses
                    .iter()
                    .filter(|other| other.domain == use_.domain)
                    .filter_map(|other| fragment_at(contract, &other.location))
                    .collect();
                findings.push(finding(
                    admitted,
                    FindingKind::IndistinguishableStates,
                    None,
                    std::slice::from_ref(requirement_id),
                    fragments,
                    (Vec::new(), true),
                    None,
                    format!(
                        "no observable of {requirement_id} accepts domain {} as an argument, so observable projections cannot distinguish states that differ only in {}",
                        use_.domain, use_.domain
                    ),
                ));
            }
        }
    }
    Ok(findings)
}

/// Bounded consistency and nonvacuity analysis over admitted typed contracts.
// r[impl cairn.requirement_consistency.queries]
// r[impl cairn.requirement_consistency.nonvacuity]
pub fn analyze_consistency(
    policy: &ConsistencyPolicy,
    case: &ConsistencyCase,
) -> Result<ConsistencyReport, ConsistencyError> {
    admit_consistency_policy(policy)?;
    let admitted = admit_case(policy, case)?;
    let mut work = Work::new(policy);
    let plans = planned_queries(&admitted);
    let mut queries = Vec::with_capacity(plans.len());
    for plan in &plans {
        queries.push(run_query(&admitted, policy, &mut work, plan)?);
    }
    let mut findings = structural_findings(&admitted)?;
    let assumptions = outcome_for(
        &queries,
        &ConsistencyQuery::AssumptionsSatisfiable {
            contracts: admitted.selected.clone(),
        },
    )?;
    match assumptions.status {
        AnalysisStatus::Inconsistent => {
            let members: Vec<(&str, &FiniteExpr, String)> = admitted
                .selected
                .iter()
                .filter_map(|requirement_id| admitted.contract(requirement_id).ok())
                .flat_map(|contract| {
                    contract
                        .normalized
                        .contract
                        .assumptions
                        .iter()
                        .enumerate()
                        .map(|(index, expression)| {
                            (
                                contract.requirement_id.as_str(),
                                expression,
                                format!("{}::assumptions[{index}]", contract.requirement_id),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let conflicts = conflicting_subsets(&admitted, &mut work, &[], &members);
            findings.push(finding(
                &admitted,
                FindingKind::ContradictoryAssumptions,
                Some(AnalysisStatus::Inconsistent),
                &admitted.selected,
                admitted.scope.environmental_assumptions.clone(),
                conflicts.clone(),
                None,
                format!(
                    "the shared assumptions have no model in the completely explored finite space; {} conflicting subset(s) retained, pairwise exploration {}",
                    conflicts.0.len(),
                    if conflicts.1 { "complete" } else { "incomplete" }
                ),
            ));
        }
        AnalysisStatus::Undetermined | AnalysisStatus::Unsupported => {
            findings.push(finding(
                &admitted,
                FindingKind::IncompleteAnalysis,
                Some(assumptions.status),
                &admitted.selected,
                admitted.scope.environmental_assumptions.clone(),
                (Vec::new(), true),
                None,
                format!(
                    "shared-assumption satisfiability was not established: {}",
                    assumptions.basis.detail
                ),
            ));
        }
        AnalysisStatus::Consistent | AnalysisStatus::Vacuous => {}
    }
    for requirement_id in &admitted.selected {
        let contract = admitted.contract(requirement_id)?;
        let outcome = outcome_for(
            &queries,
            &ConsistencyQuery::ActivationReachable {
                requirement_id: requirement_id.clone(),
            },
        )?;
        let fragments = base_fragments(&admitted, std::slice::from_ref(requirement_id))?;
        match outcome.status {
            AnalysisStatus::Consistent => findings.push(finding(
                &admitted,
                FindingKind::ActivationReachable,
                Some(AnalysisStatus::Consistent),
                std::slice::from_ref(requirement_id),
                fragments,
                (Vec::new(), true),
                outcome.witness.clone(),
                "a reproducing state activates the requirement under its own assumptions".into(),
            )),
            AnalysisStatus::Vacuous => {
                let alone = solve(
                    &Goal {
                        terms: contract
                            .normalized
                            .contract
                            .assumptions
                            .iter()
                            .map(|expression| GoalTerm::Holds {
                                requirement_id: contract.requirement_id.as_str(),
                                expression,
                            })
                            .collect(),
                    },
                    &admitted.contracts,
                    &mut work,
                );
                match alone {
                    Ok(None) => {
                        let members: Vec<(&str, &FiniteExpr, String)> = contract
                            .normalized
                            .contract
                            .assumptions
                            .iter()
                            .enumerate()
                            .map(|(index, expression)| {
                                (
                                    contract.requirement_id.as_str(),
                                    expression,
                                    format!("{}::assumptions[{index}]", contract.requirement_id),
                                )
                            })
                            .collect();
                        let conflicts = conflicting_subsets(&admitted, &mut work, &[], &members);
                        findings.push(finding(
                            &admitted,
                            FindingKind::ContradictoryAssumptions,
                            Some(AnalysisStatus::Inconsistent),
                            std::slice::from_ref(requirement_id),
                            assumption_fragments(contract)?,
                            conflicts.clone(),
                            None,
                            format!(
                                "this contract's assumptions have no model, so its guarantee is vacuously implied; {} conflicting subset(s) retained, pairwise exploration {}",
                                conflicts.0.len(),
                                if conflicts.1 { "complete" } else { "incomplete" }
                            ),
                        ));
                    }
                    Ok(Some(_)) => findings.push(finding(
                        &admitted,
                        FindingKind::UnreachableActivation,
                        Some(AnalysisStatus::Vacuous),
                        std::slice::from_ref(requirement_id),
                        fragments,
                        (Vec::new(), true),
                        None,
                        "the assumptions have a model but no model activates the requirement, so its implication is vacuous within the declared scope".into(),
                    )),
                    Err(_) => findings.push(finding(
                        &admitted,
                        FindingKind::IncompleteAnalysis,
                        Some(AnalysisStatus::Undetermined),
                        std::slice::from_ref(requirement_id),
                        fragments,
                        (Vec::new(), true),
                        None,
                        "activation has no model within the explored states, and the cause was not established within the declared bounds".into(),
                    )),
                }
            }
            AnalysisStatus::Inconsistent
            | AnalysisStatus::Undetermined
            | AnalysisStatus::Unsupported => findings.push(finding(
                &admitted,
                FindingKind::IncompleteAnalysis,
                Some(outcome.status),
                std::slice::from_ref(requirement_id),
                fragments,
                (Vec::new(), true),
                None,
                format!("activation was not established: {}", outcome.basis.detail),
            )),
        }
    }
    let guarantee_term_list = guarantee_terms(&admitted);
    let guarantee_references: Vec<String> = guarantee_term_list
        .iter()
        .map(|(_, _, reference)| reference.clone())
        .collect();
    let compatible = outcome_for(
        &queries,
        &ConsistencyQuery::GuaranteesCompatible {
            guarantees: guarantee_references.clone(),
        },
    )?;
    let all_guarantees = admitted.scope.guarantees.clone();
    let all_base = base_fragments(&admitted, &admitted.selected)?;
    match compatible.status {
        AnalysisStatus::Consistent => findings.push(finding(
            &admitted,
            FindingKind::GuaranteesCompatible,
            Some(AnalysisStatus::Consistent),
            &admitted.selected,
            all_guarantees.clone(),
            (Vec::new(), true),
            compatible.witness.clone(),
            "all selected guarantees hold in one model under the shared assumptions".into(),
        )),
        AnalysisStatus::Inconsistent => {
            let members: Vec<(&str, &FiniteExpr, String)> = admitted
                .selected
                .iter()
                .filter_map(|requirement_id| admitted.contract(requirement_id).ok())
                .flat_map(|contract| {
                    guarantee_expressions(&contract.normalized.contract)
                        .into_iter()
                        .enumerate()
                        .map(|(index, expression)| {
                            (
                                contract.requirement_id.as_str(),
                                expression,
                                format!("{}::guarantee[{index}]", contract.requirement_id),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let base = {
                let mut terms = assumption_terms(&admitted);
                terms.extend(activation_terms(&admitted));
                terms
            };
            let conflicts = conflicting_subsets(&admitted, &mut work, &base, &members);
            let mut fragments = all_base.clone();
            fragments.extend(all_guarantees.clone());
            findings.push(finding(
                &admitted,
                FindingKind::ConflictingGuarantees,
                Some(AnalysisStatus::Inconsistent),
                &admitted.selected,
                fragments,
                conflicts.clone(),
                None,
                format!(
                    "no model satisfies all selected guarantees under the shared assumptions; {} conflicting subset(s) retained, pairwise exploration {}",
                    conflicts.0.len(),
                    if conflicts.1 { "complete" } else { "incomplete" }
                ),
            ));
        }
        AnalysisStatus::Vacuous => {
            let mut fragments = all_base.clone();
            fragments.extend(all_guarantees.clone());
            findings.push(finding(
                &admitted,
                FindingKind::VacuousRequirementSet,
                Some(AnalysisStatus::Vacuous),
                &admitted.selected,
                fragments,
                (Vec::new(), true),
                None,
                "the shared assumptions and activations have no model, so every selected guarantee is vacuously implied".into(),
            ));
        }
        AnalysisStatus::Undetermined | AnalysisStatus::Unsupported => findings.push(finding(
            &admitted,
            FindingKind::IncompleteAnalysis,
            Some(compatible.status),
            &admitted.selected,
            all_guarantees.clone(),
            (Vec::new(), true),
            None,
            format!(
                "joint guarantee compatibility was not established: {}",
                compatible.basis.detail
            ),
        )),
    }
    for reference in &guarantee_references {
        let outcome = outcome_for(
            &queries,
            &ConsistencyQuery::GuaranteeConstraining {
                guarantee: reference.clone(),
            },
        )?;
        let Some(guarantee) = admitted
            .scope
            .guarantees
            .iter()
            .find(|fragment| fragment.reference() == *reference)
        else {
            return Err(ConsistencyError::InvalidInput(format!(
                "guarantee reference {reference} is not in scope"
            )));
        };
        let requirement_ids = admitted.selected.as_slice();
        let mut fragments = base_fragments(&admitted, requirement_ids)?;
        fragments.push(guarantee.clone());
        match outcome.status {
            AnalysisStatus::Consistent => findings.push(finding(
                &admitted,
                FindingKind::ConstrainingGuarantee,
                Some(AnalysisStatus::Consistent),
                requirement_ids,
                fragments,
                (Vec::new(), true),
                outcome.witness.clone(),
                "withholding this guarantee admits a behavior excluded by it while the remaining constraints still hold; weakening it to true is the same test".into(),
            )),
            AnalysisStatus::Inconsistent => findings.push(finding(
                &admitted,
                FindingKind::RedundantGuarantee,
                Some(AnalysisStatus::Inconsistent),
                requirement_ids,
                fragments,
                (Vec::new(), true),
                None,
                "no model of the remaining constraints violates this guarantee within the declared scope, so it is redundant inside this model; redundancy does not make the requirement useless or removable from its prose".into(),
            )),
            AnalysisStatus::Undetermined | AnalysisStatus::Unsupported => {
                findings.push(finding(
                    &admitted,
                    FindingKind::IncompleteAnalysis,
                    Some(outcome.status),
                    requirement_ids,
                    fragments,
                    (Vec::new(), true),
                    None,
                    format!(
                        "guarantee withholding was not decided: {}",
                        outcome.basis.detail
                    ),
                ))
            }
            AnalysisStatus::Vacuous => {}
        }
    }
    let mut trust_dependencies = BTreeSet::from([
        "search:complete-finite-observation-cell-enumeration".to_string(),
        "observable-extraction:supplied-total-mapping-projection".to_string(),
        format!("interpretation:{}", admitted.scope.interpretation),
    ]);
    for contract in &admitted.scope.contracts {
        trust_dependencies.insert(format!(
            "requirement-revision:{}/{}",
            contract.requirement_id, contract.requirement_row_blake3
        ));
        trust_dependencies.insert(format!(
            "correspondence:{}:{}",
            contract.requirement_id, contract.correspondence
        ));
    }
    let input_blake3 = identity(INPUT_IDENTITY_DOMAIN_V1, &(policy, case))?;
    let mut report = ConsistencyReport {
        schema: CONSISTENCY_REPORT_SCHEMA_V1.into(),
        advisory: true,
        scope: admitted.scope.clone(),
        queries,
        findings,
        producer: case.producer.clone(),
        trust_dependencies,
        input_blake3,
        report_blake3: String::new(),
        boundary: CONSISTENCY_BOUNDARY.into(),
    };
    let report_blake3 = identity(CONSISTENCY_REPORT_SCHEMA_V1, &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

fn cells_from_tables(
    admitted: &Admitted,
    witness: &AnalysisWitness,
) -> Result<BTreeMap<CellKey, FiniteValue>, ConsistencyError> {
    let mut tables = BTreeMap::new();
    for table in &witness.tables {
        if tables.insert(table.requirement_id.clone(), table).is_some() {
            return Err(ConsistencyError::InvalidInput(format!(
                "duplicate witness table for {}",
                table.requirement_id
            )));
        }
    }
    if tables.len() != admitted.selected.len()
        || admitted
            .selected
            .iter()
            .any(|requirement_id| !tables.contains_key(requirement_id))
    {
        return Err(ConsistencyError::ScopeMismatch(
            "witness tables are not the exact selected contract set".into(),
        ));
    }
    let mut cells = BTreeMap::new();
    for requirement_id in &admitted.selected {
        let contract = admitted.contract(requirement_id)?;
        let Some(table) = tables.get(requirement_id) else {
            return Err(ConsistencyError::ScopeMismatch(
                "missing witness table".into(),
            ));
        };
        if table.contract_blake3 != contract.normalized.contract_blake3 {
            return Err(ConsistencyError::ScopeMismatch(format!(
                "witness table for {requirement_id} names a different contract revision"
            )));
        }
        if table.observables.len() != contract.normalized.contract.observables.len() {
            return Err(ConsistencyError::InvalidInput(format!(
                "witness observable set is not exact for {requirement_id}"
            )));
        }
        for (mapping, rows) in &table.observables {
            if !contract
                .normalized
                .contract
                .observables
                .contains_key(mapping)
            {
                return Err(ConsistencyError::InvalidInput(format!(
                    "witness names undeclared observable {mapping}"
                )));
            }
            let state = contract.observable_states.get(mapping).ok_or_else(|| {
                ConsistencyError::InvalidInput(format!(
                    "observable {mapping} has no admitted model cell identity"
                ))
            })?;
            for row in rows {
                let cell = CellKey {
                    state: state.clone(),
                    arguments: row.arguments.clone(),
                };
                match cells.get(&cell) {
                    Some(existing) if existing != &row.value => {
                        return Err(ConsistencyError::ScopeMismatch(format!(
                            "witness tables disagree on the shared observation {state} for {mapping}"
                        )))
                    }
                    Some(_) => {}
                    None => {
                        cells.insert(cell, row.value.clone());
                    }
                }
            }
        }
    }
    Ok(cells)
}

fn whole_contract_holds(
    contract: &AdmittedContract,
    expression: &FiniteExpr,
    cells: &BTreeMap<CellKey, FiniteValue>,
    work: &mut Work,
) -> Result<bool, Stop> {
    let mut environment = BTreeMap::new();
    expression_truth(contract, expression, &mut environment, cells, work)
        .map(|truth| truth == Truth::True)
}

/// Replays a retained witness against the exact admitted case. The tables are
/// re-admitted through the typed-contract evaluator, the whole-contract booleans
/// must agree with this module's evaluator, and the named query must hold.
// r[impl cairn.requirement_consistency.replay]
pub fn replay_witness(
    policy: &ConsistencyPolicy,
    case: &ConsistencyCase,
    witness: &AnalysisWitness,
) -> Result<WitnessReplay, ConsistencyError> {
    admit_consistency_policy(policy)?;
    let admitted = admit_case(policy, case)?;
    let plans = planned_queries(&admitted);
    let mut evaluations = Vec::new();
    let mut work = Work::new(policy);
    let reject = |detail: String, evaluations: Vec<ContractEvaluation>| WitnessReplay {
        query: witness.query.clone(),
        replays: false,
        detail,
        contract_evaluations: evaluations,
        boundary: CONSISTENCY_BOUNDARY.into(),
    };
    let Some(plan) = plans
        .iter()
        .find(|plan| plan.query.label() == witness.query)
    else {
        return Ok(reject(
            "the witness names a query outside this exact case scope".into(),
            evaluations,
        ));
    };
    if !policy.queries.contains(&plan.query.kind()) {
        return Ok(reject(
            format!(
                "the witness names query kind {} which the admitted policy does not enable",
                plan.query.kind().tag()
            ),
            evaluations,
        ));
    }
    let cells = match cells_from_tables(&admitted, witness) {
        Ok(cells) => cells,
        Err(error) => return Ok(reject(format!("{error:?}"), evaluations)),
    };
    for requirement_id in &admitted.selected {
        let contract = admitted.contract(requirement_id)?;
        let Some(table) = witness
            .tables
            .iter()
            .find(|table| table.requirement_id == *requirement_id)
        else {
            return Ok(reject("missing witness table".into(), evaluations));
        };
        let example = SemanticExample {
            observables: table.observables.clone(),
        };
        let evaluation = match evaluate_example(&case.profile, &contract.normalized, &example) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                return Ok(reject(
                    format!("typed-contract evaluation rejected the witness: {error:?}"),
                    evaluations,
                ))
            }
        };
        let mut assumptions_hold = true;
        for assumption in &contract.normalized.contract.assumptions {
            match whole_contract_holds(contract, assumption, &cells, &mut work) {
                Ok(value) => assumptions_hold &= value,
                Err(error) => {
                    return Ok(reject(
                        format!("witness replay could not evaluate assumptions: {error:?}"),
                        evaluations,
                    ))
                }
            }
        }
        let activated = match whole_contract_holds(
            contract,
            &contract.normalized.contract.activation,
            &cells,
            &mut work,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Ok(reject(
                    format!("witness replay could not evaluate activation: {error:?}"),
                    evaluations,
                ))
            }
        };
        let guarantee_holds = match whole_contract_holds(
            contract,
            &contract.normalized.contract.guarantee,
            &cells,
            &mut work,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Ok(reject(
                    format!("witness replay could not evaluate the guarantee: {error:?}"),
                    evaluations,
                ))
            }
        };
        evaluations.push(ContractEvaluation {
            requirement_id: requirement_id.clone(),
            contract_blake3: contract.normalized.contract_blake3.clone(),
            assumptions_hold,
            activated,
            guarantee_holds,
            implication_holds: !assumptions_hold || !activated || guarantee_holds,
            steps: evaluation.steps,
        });
        if assumptions_hold != evaluation.assumptions_hold
            || activated != evaluation.activated
            || guarantee_holds != evaluation.guarantee_holds
        {
            return Ok(reject(
                format!(
                    "the analysis evaluator and the typed-contract evaluator disagree for {requirement_id}"
                ),
                evaluations,
            ));
        }
    }
    work.unknown = None;
    match plan.goal.evaluate(&admitted.contracts, &cells, &mut work) {
        Ok(Truth::True) => {}
        Ok(_) => {
            return Ok(reject(
                "the retained model does not satisfy the named query".into(),
                evaluations,
            ))
        }
        Err(Stop::Exhausted(detail)) => {
            return Ok(reject(
                format!("replay exhausted {detail} before deciding the query"),
                evaluations,
            ))
        }
        Err(Stop::Unsupported(detail)) | Err(Stop::Invalid(detail)) => {
            return Ok(reject(detail, evaluations))
        }
    }
    let tables = total_tables(&admitted, &cells)?;
    let model_blake3 = identity(WITNESS_IDENTITY_DOMAIN_V1, &(&witness.query, &tables))?;
    if model_blake3 != witness.model_blake3 {
        return Ok(reject(
            "the witness model identity does not cover the replayed total tables".into(),
            evaluations,
        ));
    }
    Ok(WitnessReplay {
        query: witness.query.clone(),
        replays: true,
        detail: "the retained total tables re-admit and satisfy the exact query under the exact contract revisions".into(),
        contract_evaluations: evaluations,
        boundary: CONSISTENCY_BOUNDARY.into(),
    })
}

#[cfg(test)]
mod tests;
