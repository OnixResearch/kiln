//! Opt-in finite requirement meaning. No filesystem, execution, or proof admission.
use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FINITE_SEMANTICS_PROFILE_V1: &str = "cairn.requirement-semantics.finite.v1";
pub const FINITE_INTERPRETATION_V1: &str =
    "finite-enumeration.equality.nominal-domains.total-observables.single-step.v1";
pub const TYPED_REQUIREMENT_BOUNDARY: &str = "Well-formedness is not correspondence; correspondence review is not proof; finite example evaluation is not admitted proof evidence, unbounded validity, or source-code correctness. Observable tables are supplied observations, not executed or verified program behavior. EARS keywords confer no review or proof authority.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProfile {
    pub id: String,
    pub interpretation: String,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_domain_values: usize,
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteValue {
    pub domain: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteDomain {
    pub values: Vec<String>,
}

/// Each mapping names one total finite function, including its before/after phase.
/// `source` and `projection` document the exact program observation; they do not execute it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservableMapping {
    pub arguments: Vec<String>,
    pub result: String,
    pub phase: String,
    pub source: String,
    pub projection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FiniteTerm {
    Literal {
        value: FiniteValue,
    },
    Variable {
        name: String,
    },
    Observable {
        mapping: String,
        arguments: Vec<FiniteTerm>,
    },
    Unsupported {
        construct: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FiniteExpr {
    Bool {
        value: bool,
    },
    Equal {
        left: FiniteTerm,
        right: FiniteTerm,
    },
    Member {
        term: FiniteTerm,
        values: Vec<FiniteValue>,
    },
    Not {
        expression: Box<FiniteExpr>,
    },
    And {
        expressions: Vec<FiniteExpr>,
    },
    Or {
        expressions: Vec<FiniteExpr>,
    },
    Implies {
        premise: Box<FiniteExpr>,
        conclusion: Box<FiniteExpr>,
    },
    Forall {
        variable: String,
        domain: String,
        expression: Box<FiniteExpr>,
    },
    Exists {
        variable: String,
        domain: String,
        expression: Box<FiniteExpr>,
    },
    Unsupported {
        construct: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticContract {
    pub profile: String,
    pub proposed_by: String,
    pub requirement: ContentBoundRequirementRow,
    pub domains: BTreeMap<String, FiniteDomain>,
    pub observables: BTreeMap<String, ObservableMapping>,
    pub assumptions: Vec<FiniteExpr>,
    pub activation: FiniteExpr,
    pub guarantee: FiniteExpr,
    pub supported_portion: String,
    pub informal_remainder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedContract {
    pub contract: SemanticContract,
    pub interpretation_blake3: String,
    pub mapping_blake3: String,
    pub contract_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrespondenceProposal {
    pub requirement_id: String,
    pub proposed_by: String,
    pub rationale: String,
}

/// A retained independent review declaration, not a signature or proof receipt.
/// The shell supplies the explicitly trusted reviewer allowlist and retains the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrespondenceReview {
    pub requirement_row_blake3: String,
    pub contract_blake3: String,
    pub interpretation_blake3: String,
    pub mapping_blake3: String,
    pub proposed_by: String,
    pub reviewer: String,
    pub review_artifact_blake3: String,
    pub decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedRequirementInventory {
    pub profile: SemanticProfile,
    pub contracts: Vec<SemanticContract>,
    pub proposed_correspondence: Vec<CorrespondenceProposal>,
    pub authorized_reviewers: Vec<String>,
    pub reviews: Vec<CorrespondenceReview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservableRow {
    pub arguments: Vec<FiniteValue>,
    pub value: FiniteValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticExample {
    pub observables: BTreeMap<String, Vec<ObservableRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleEvaluation {
    pub contract_blake3: String,
    pub example_blake3: String,
    pub assumptions_hold: bool,
    pub activated: bool,
    pub guarantee_holds: bool,
    pub implication_holds: bool,
    pub steps: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum SemanticError {
    IllTyped(String),
    UnboundVariable(String),
    UnknownProfile(String),
    UnsupportedConstruct(String),
    BoundExhaustion(String),
    RequirementRevisionMismatch,
    InvalidReview(String),
    InvalidExample(String),
    Serialization(String),
}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, SemanticError> {
    let value =
        serde_json::to_value(value).map_err(|e| SemanticError::Serialization(e.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(super::evidence::canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn validate_profile(profile: &SemanticProfile) -> Result<(), SemanticError> {
    if profile.id != FINITE_SEMANTICS_PROFILE_V1
        || profile.interpretation != FINITE_INTERPRETATION_V1
    {
        return Err(SemanticError::UnknownProfile(profile.id.clone()));
    }
    // Hard ceilings protect callers even when supplied policy is untrusted.
    if profile.max_nodes == 0
        || profile.max_nodes > 4096
        || profile.max_depth == 0
        || profile.max_depth > 64
        || profile.max_domain_values == 0
        || profile.max_domain_values > 256
        || profile.max_steps == 0
        || profile.max_steps > 1_000_000
    {
        return Err(SemanticError::BoundExhaustion(
            "profile limits outside supported ceilings".into(),
        ));
    }
    Ok(())
}

fn domain<'a>(
    contract: &'a SemanticContract,
    name: &str,
) -> Result<&'a FiniteDomain, SemanticError> {
    contract
        .domains
        .get(name)
        .ok_or_else(|| SemanticError::IllTyped(format!("undeclared domain {name}")))
}

fn value_type<'a>(
    contract: &SemanticContract,
    value: &'a FiniteValue,
) -> Result<&'a str, SemanticError> {
    if !domain(contract, &value.domain)?
        .values
        .contains(&value.value)
    {
        return Err(SemanticError::IllTyped(format!(
            "value outside domain {}",
            value.domain
        )));
    }
    Ok(&value.domain)
}

struct Budget<'a> {
    profile: &'a SemanticProfile,
    used: usize,
}
impl Budget<'_> {
    fn node(&mut self, depth: usize) -> Result<(), SemanticError> {
        self.used += 1;
        if depth > self.profile.max_depth || self.used > self.profile.max_nodes {
            return Err(SemanticError::BoundExhaustion(
                "syntax nodes or depth".into(),
            ));
        }
        Ok(())
    }
}

fn term_type(
    contract: &SemanticContract,
    term: &FiniteTerm,
    variables: &BTreeMap<String, String>,
    budget: &mut Budget<'_>,
    depth: usize,
) -> Result<String, SemanticError> {
    budget.node(depth)?;
    match term {
        FiniteTerm::Literal { value } => Ok(value_type(contract, value)?.into()),
        FiniteTerm::Variable { name } => variables
            .get(name)
            .cloned()
            .ok_or_else(|| SemanticError::UnboundVariable(name.clone())),
        FiniteTerm::Observable { mapping, arguments } => {
            let observable = contract
                .observables
                .get(mapping)
                .ok_or_else(|| SemanticError::IllTyped(format!("unknown observable {mapping}")))?;
            if arguments.len() != observable.arguments.len() {
                return Err(SemanticError::IllTyped(format!("arity of {mapping}")));
            }
            for (argument, expected) in arguments.iter().zip(&observable.arguments) {
                if term_type(contract, argument, variables, budget, depth + 1)? != *expected {
                    return Err(SemanticError::IllTyped(format!(
                        "argument domain of {mapping}"
                    )));
                }
            }
            Ok(observable.result.clone())
        }
        FiniteTerm::Unsupported { construct } => {
            Err(SemanticError::UnsupportedConstruct(construct.clone()))
        }
    }
}

fn check_expr(
    contract: &SemanticContract,
    expr: &FiniteExpr,
    variables: &mut BTreeMap<String, String>,
    budget: &mut Budget<'_>,
    depth: usize,
) -> Result<(), SemanticError> {
    budget.node(depth)?;
    match expr {
        FiniteExpr::Bool { .. } => Ok(()),
        FiniteExpr::Equal { left, right } => {
            if term_type(contract, left, variables, budget, depth + 1)?
                != term_type(contract, right, variables, budget, depth + 1)?
            {
                return Err(SemanticError::IllTyped(
                    "equality crosses nominal domains".into(),
                ));
            }
            Ok(())
        }
        FiniteExpr::Member { term, values } => {
            let expected = term_type(contract, term, variables, budget, depth + 1)?;
            if values.len() > budget.profile.max_domain_values {
                return Err(SemanticError::BoundExhaustion("membership values".into()));
            }
            for value in values {
                if value_type(contract, value)? != expected {
                    return Err(SemanticError::IllTyped(
                        "membership crosses nominal domains".into(),
                    ));
                }
            }
            Ok(())
        }
        FiniteExpr::Not { expression } => {
            check_expr(contract, expression, variables, budget, depth + 1)
        }
        FiniteExpr::And { expressions } | FiniteExpr::Or { expressions } => {
            for expression in expressions {
                check_expr(contract, expression, variables, budget, depth + 1)?;
            }
            Ok(())
        }
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            check_expr(contract, premise, variables, budget, depth + 1)?;
            check_expr(contract, conclusion, variables, budget, depth + 1)
        }
        FiniteExpr::Forall {
            variable,
            domain: name,
            expression,
        }
        | FiniteExpr::Exists {
            variable,
            domain: name,
            expression,
        } => {
            domain(contract, name)?;
            if variable.is_empty() || variables.contains_key(variable) {
                return Err(SemanticError::IllTyped(
                    "empty or shadowed quantified variable".into(),
                ));
            }
            variables.insert(variable.clone(), name.clone());
            let result = check_expr(contract, expression, variables, budget, depth + 1);
            variables.remove(variable);
            result
        }
        FiniteExpr::Unsupported { construct } => {
            Err(SemanticError::UnsupportedConstruct(construct.clone()))
        }
    }
}

fn normalize_expr(expr: &mut FiniteExpr) -> Result<(), SemanticError> {
    match expr {
        FiniteExpr::Member { values, .. } => {
            values.sort();
            values.dedup();
        }
        FiniteExpr::Not { expression }
        | FiniteExpr::Forall { expression, .. }
        | FiniteExpr::Exists { expression, .. } => normalize_expr(expression)?,
        FiniteExpr::Implies {
            premise,
            conclusion,
        } => {
            normalize_expr(premise)?;
            normalize_expr(conclusion)?;
        }
        FiniteExpr::And { expressions } | FiniteExpr::Or { expressions } => {
            normalize_expressions(expressions)?
        }
        _ => {}
    }
    Ok(())
}

fn normalize_expressions(expressions: &mut Vec<FiniteExpr>) -> Result<(), SemanticError> {
    let mut keyed = Vec::with_capacity(expressions.len());
    for mut expression in std::mem::take(expressions) {
        normalize_expr(&mut expression)?;
        let key = serde_json::to_string(&expression)
            .map_err(|e| SemanticError::Serialization(e.to_string()))?;
        keyed.push((key, expression));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.dedup_by(|a, b| a.0 == b.0);
    expressions.extend(keyed.into_iter().map(|(_, expression)| expression));
    Ok(())
}

/// Exact native row linkage, never an alternate registry. The caller admits registry freshness.
pub fn normalize_contract(
    profile: &SemanticProfile,
    native_row: &ContentBoundRequirementRow,
    contract: &SemanticContract,
) -> Result<NormalizedContract, SemanticError> {
    // r[impl cairn.typed_requirements.finite_contract]
    validate_profile(profile)?;
    if contract.profile != profile.id {
        return Err(SemanticError::UnknownProfile(contract.profile.clone()));
    }
    if contract.requirement != *native_row {
        return Err(SemanticError::RequirementRevisionMismatch);
    }
    if contract.domains.is_empty()
        || contract.domains.len() > profile.max_domain_values
        || contract.observables.len() > profile.max_nodes
    {
        return Err(SemanticError::BoundExhaustion(
            "domain or observable count".into(),
        ));
    }
    for (name, finite) in &contract.domains {
        if finite.values.is_empty() || finite.values.len() > profile.max_domain_values {
            return Err(SemanticError::BoundExhaustion(format!("domain {name}")));
        }
        let unique: BTreeSet<_> = finite.values.iter().collect();
        if name.is_empty()
            || finite.values.iter().any(String::is_empty)
            || unique.len() != finite.values.len()
        {
            return Err(SemanticError::IllTyped(format!(
                "empty name/value or duplicate in domain {name}"
            )));
        }
    }
    if contract.proposed_by.trim().is_empty()
        || contract.supported_portion.trim().is_empty()
        || contract.informal_remainder.trim().is_empty()
    {
        return Err(SemanticError::IllTyped(
            "semantic coverage must be explicit".into(),
        ));
    }
    for (name, observable) in &contract.observables {
        if name.is_empty()
            || observable.source.trim().is_empty()
            || observable.projection.trim().is_empty()
        {
            return Err(SemanticError::IllTyped(
                "observable mapping lacks source or projection".into(),
            ));
        }
        if !matches!(observable.phase.as_str(), "before" | "after" | "state") {
            return Err(SemanticError::UnsupportedConstruct(format!(
                "observable phase {}",
                observable.phase
            )));
        }
        if observable.arguments.len() > profile.max_depth {
            return Err(SemanticError::BoundExhaustion("observable arity".into()));
        }
        for name in observable
            .arguments
            .iter()
            .chain(std::iter::once(&observable.result))
        {
            domain(contract, name)?;
        }
    }
    let mut budget = Budget { profile, used: 0 };
    let mut variables = BTreeMap::new();
    for expression in contract
        .assumptions
        .iter()
        .chain([&contract.activation, &contract.guarantee])
    {
        check_expr(contract, expression, &mut variables, &mut budget, 0)?;
    }
    let mut normalized = contract.clone();
    for finite in normalized.domains.values_mut() {
        finite.values.sort();
    }
    normalize_expressions(&mut normalized.assumptions)?;
    normalize_expr(&mut normalized.activation)?;
    normalize_expr(&mut normalized.guarantee)?;
    let interpretation_blake3 = identity("cairn.requirement-interpretation.v1", profile)?;
    let mapping_blake3 = identity("cairn.requirement-observables.v1", &normalized.observables)?;
    let contract_blake3 = identity(
        "cairn.requirement-contract.v1",
        &(&interpretation_blake3, &mapping_blake3, &normalized),
    )?;
    Ok(NormalizedContract {
        contract: normalized,
        interpretation_blake3,
        mapping_blake3,
        contract_blake3,
    })
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn admit_correspondence_review(
    normalized: &NormalizedContract,
    review: &CorrespondenceReview,
    authorized_reviewers: &[String],
) -> Result<String, SemanticError> {
    // r[impl cairn.typed_requirements.correspondence]
    let mapping = identity(
        "cairn.requirement-observables.v1",
        &normalized.contract.observables,
    )?;
    let contract = identity(
        "cairn.requirement-contract.v1",
        &(
            &normalized.interpretation_blake3,
            &mapping,
            &normalized.contract,
        ),
    )?;
    if mapping != normalized.mapping_blake3
        || contract != normalized.contract_blake3
        || !digest(&normalized.interpretation_blake3)
    {
        return Err(SemanticError::InvalidReview(
            "modified normalized contract or identity".into(),
        ));
    }
    if review.requirement_row_blake3 != normalized.contract.requirement.row_blake3
        || review.contract_blake3 != normalized.contract_blake3
        || review.interpretation_blake3 != normalized.interpretation_blake3
        || review.mapping_blake3 != normalized.mapping_blake3
    {
        return Err(SemanticError::InvalidReview(
            "stale revision, interpretation, contract, or mapping".into(),
        ));
    }
    if review.decision != "approved"
        || review.proposed_by != normalized.contract.proposed_by
        || review.reviewer.trim().is_empty()
        || review.reviewer == review.proposed_by
        || !authorized_reviewers.contains(&review.reviewer)
        || !digest(&review.review_artifact_blake3)
    {
        return Err(SemanticError::InvalidReview(
            "requires retained independent approved review by explicitly authorized reviewer"
                .into(),
        ));
    }
    identity("cairn.requirement-correspondence-review.v1", review)
}

struct Evaluator<'a> {
    contract: &'a SemanticContract,
    example: &'a SemanticExample,
    variables: BTreeMap<String, FiniteValue>,
    steps: usize,
    limit: usize,
}
impl Evaluator<'_> {
    fn tick(&mut self) -> Result<(), SemanticError> {
        self.steps += 1;
        if self.steps > self.limit {
            return Err(SemanticError::BoundExhaustion(
                "finite evaluation steps".into(),
            ));
        }
        Ok(())
    }
    fn term(&mut self, term: &FiniteTerm) -> Result<FiniteValue, SemanticError> {
        self.tick()?;
        match term {
            FiniteTerm::Literal { value } => Ok(value.clone()),
            FiniteTerm::Variable { name } => self
                .variables
                .get(name)
                .cloned()
                .ok_or_else(|| SemanticError::UnboundVariable(name.clone())),
            FiniteTerm::Observable { mapping, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(|term| self.term(term))
                    .collect::<Result<Vec<_>, _>>()?;
                let example = self.example;
                if let Some(rows) = example.observables.get(mapping) {
                    for row in rows {
                        self.tick()?;
                        if row.arguments == arguments {
                            return Ok(row.value.clone());
                        }
                    }
                }
                Err(SemanticError::InvalidExample(format!(
                    "missing observation {mapping}"
                )))
            }
            FiniteTerm::Unsupported { construct } => {
                Err(SemanticError::UnsupportedConstruct(construct.clone()))
            }
        }
    }
    fn expr(&mut self, expr: &FiniteExpr) -> Result<bool, SemanticError> {
        self.tick()?;
        match expr {
            FiniteExpr::Bool { value } => Ok(*value),
            FiniteExpr::Equal { left, right } => Ok(self.term(left)? == self.term(right)?),
            FiniteExpr::Member { term, values } => Ok(values.contains(&self.term(term)?)),
            FiniteExpr::Not { expression } => Ok(!self.expr(expression)?),
            FiniteExpr::And { expressions } => {
                for expression in expressions {
                    if !self.expr(expression)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            FiniteExpr::Or { expressions } => {
                for expression in expressions {
                    if self.expr(expression)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            FiniteExpr::Implies {
                premise,
                conclusion,
            } => Ok(!self.expr(premise)? || self.expr(conclusion)?),
            FiniteExpr::Forall {
                variable,
                domain: name,
                expression,
            }
            | FiniteExpr::Exists {
                variable,
                domain: name,
                expression,
            } => {
                let universal = matches!(expr, FiniteExpr::Forall { .. });
                let contract = self.contract;
                for value in &domain(contract, name)?.values {
                    self.variables.insert(
                        variable.clone(),
                        FiniteValue {
                            domain: name.clone(),
                            value: value.clone(),
                        },
                    );
                    let result = self.expr(expression)?;
                    if result != universal {
                        self.variables.remove(variable);
                        return Ok(result);
                    }
                }
                self.variables.remove(variable);
                Ok(universal)
            }
            FiniteExpr::Unsupported { construct } => {
                Err(SemanticError::UnsupportedConstruct(construct.clone()))
            }
        }
    }
}

/// Validates all tables, including unused observations, before evaluating any formula.
/// Totality prevents short-circuiting from hiding absent or uninterpreted behavior.
pub fn evaluate_example(
    profile: &SemanticProfile,
    normalized: &NormalizedContract,
    example: &SemanticExample,
) -> Result<ExampleEvaluation, SemanticError> {
    // r[impl cairn.typed_requirements.examples]
    let admitted = normalize_contract(
        profile,
        &normalized.contract.requirement,
        &normalized.contract,
    )?;
    if admitted != *normalized {
        return Err(SemanticError::InvalidExample(
            "tampered normalized identity".into(),
        ));
    }
    if example.observables.len() != normalized.contract.observables.len() {
        return Err(SemanticError::InvalidExample(
            "observable set must be exact".into(),
        ));
    }
    let mut validation_steps = 0usize;
    for (name, mapping) in &normalized.contract.observables {
        let rows = example
            .observables
            .get(name)
            .ok_or_else(|| SemanticError::InvalidExample(format!("missing table {name}")))?;
        let mut cardinality = 1usize;
        for argument in &mapping.arguments {
            cardinality = cardinality
                .checked_mul(domain(&normalized.contract, argument)?.values.len())
                .filter(|n| *n <= profile.max_steps)
                .ok_or_else(|| {
                    SemanticError::BoundExhaustion("observable table cardinality".into())
                })?;
        }
        validation_steps = validation_steps
            .checked_add(rows.len())
            .filter(|n| *n <= profile.max_steps)
            .ok_or_else(|| SemanticError::BoundExhaustion("observable table validation".into()))?;
        if rows.len() != cardinality {
            return Err(SemanticError::InvalidExample(format!(
                "non-total table {name}"
            )));
        }
        let mut seen = BTreeSet::new();
        for row in rows {
            if row.arguments.len() != mapping.arguments.len()
                || value_type(&normalized.contract, &row.value)? != mapping.result
            {
                return Err(SemanticError::IllTyped(format!("table row {name}")));
            }
            for (value, expected) in row.arguments.iter().zip(&mapping.arguments) {
                if value_type(&normalized.contract, value)? != expected {
                    return Err(SemanticError::IllTyped(format!("table argument {name}")));
                }
            }
            if !seen.insert(&row.arguments) {
                return Err(SemanticError::InvalidExample(format!(
                    "duplicate arguments {name}"
                )));
            }
        }
    }
    let mut evaluator = Evaluator {
        contract: &normalized.contract,
        example,
        variables: BTreeMap::new(),
        steps: validation_steps,
        limit: profile.max_steps,
    };
    let mut assumptions_hold = true;
    for assumption in &normalized.contract.assumptions {
        assumptions_hold &= evaluator.expr(assumption)?;
    }
    let activated = evaluator.expr(&normalized.contract.activation)?;
    let guarantee_holds = evaluator.expr(&normalized.contract.guarantee)?;
    let mut canonical_example = example.clone();
    for rows in canonical_example.observables.values_mut() {
        rows.sort_by(|a, b| a.arguments.cmp(&b.arguments));
    }
    Ok(ExampleEvaluation {
        contract_blake3: normalized.contract_blake3.clone(),
        example_blake3: identity("cairn.requirement-example.v1", &canonical_example)?,
        assumptions_hold,
        activated,
        guarantee_holds,
        implication_holds: !assumptions_hold || !activated || guarantee_holds,
        steps: evaluator.steps,
        boundary: TYPED_REQUIREMENT_BOUNDARY.into(),
    })
}

/// Admit the opt-in wire model without interpreting unknown operators as predicates.
pub fn parse_typed_requirement_inventory_value(
    value: &serde_json::Value,
) -> Result<TypedRequirementInventory, SemanticError> {
    fn constructs(
        value: &serde_json::Value,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), SemanticError> {
        *nodes += 1;
        if depth > 64 || *nodes > 16384 {
            return Err(SemanticError::BoundExhaustion(
                "wire formula nodes or depth".into(),
            ));
        }
        match value {
            serde_json::Value::Object(fields) => {
                if let Some(kind) = fields.get("kind").and_then(serde_json::Value::as_str) {
                    if !matches!(
                        kind,
                        "literal"
                            | "variable"
                            | "observable"
                            | "bool"
                            | "equal"
                            | "member"
                            | "not"
                            | "and"
                            | "or"
                            | "implies"
                            | "forall"
                            | "exists"
                            | "unsupported"
                    ) {
                        return Err(SemanticError::UnsupportedConstruct(kind.into()));
                    }
                }
                for child in fields.values() {
                    constructs(child, depth + 1, nodes)?;
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    constructs(child, depth + 1, nodes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    if let Some(contracts) = value.get("contracts").and_then(serde_json::Value::as_array) {
        if contracts.len() > 256 {
            return Err(SemanticError::BoundExhaustion(
                "contract inventory size".into(),
            ));
        }
        for contract in contracts {
            let mut nodes = 0;
            for key in ["assumptions", "activation", "guarantee"] {
                if let Some(expression) = contract.get(key) {
                    constructs(expression, 0, &mut nodes)?;
                }
            }
        }
    }
    serde_json::from_value(value.clone())
        .map_err(|error| SemanticError::IllTyped(error.to_string()))
}

#[cfg(test)]
mod tests;
