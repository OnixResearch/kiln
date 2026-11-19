//! Typed per-obligation facts for external consumers (selected-work, Kiln,
//! Lattice).
//!
//! Every fact derives from the same admitted definition, predicate and
//! guidance used by evaluation, so a consumer never needs to parse guidance
//! prose to recover an obligation, its native evidence references or its
//! selection state. Facts carry three identities:
//!
//! - `obligation_blake3`: the node's admitted meaning under one binding;
//! - `guidance_blake3`: that meaning as presented by the structured guidance;
//! - `condition_blake3`: the exact remaining discharge condition under one
//!   predicate (subject, context, selection and evidence revisions).
//!
//! Nothing here evaluates evidence, executes actions or grants authority.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    identity, AdmittedWorkflowObligation, EvidencePredicateKind, EvidenceRequirement,
    ObligationExpr, ObligationGuidance, ObligationPredicate, WorkflowObligationError,
    WORKFLOW_OBLIGATION_PREDICATE_SCHEMA_V1,
};

pub const WORKFLOW_OBLIGATION_NODE_SCHEMA_V1: &str = "cairn.workflow-obligation-node.v1";
const NODE_DOMAIN: &str = "cairn.workflow-obligation-node.v1";
const GUIDANCE_DOMAIN: &str = "cairn.workflow-obligation-node-guidance.v1";
const CONDITION_DOMAIN: &str = "cairn.workflow-obligation-node-condition.v1";
const EVIDENCE_DOMAIN: &str = "cairn.workflow-obligation-evidence-reference.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationNodeKind {
    Evidence,
    AllOf,
    AnyOf,
    Applicable,
    Temporal,
    Exception,
}

/// How the current alternative selection and applicability conditions place a
/// node. Ordered by restriction: a node takes the most restrictive state on
/// its path. Only `required`, `selected_alternative` and `conditional` nodes
/// can contribute to completion; `selection_missing` is unresolved and
/// `unselected_alternative` never counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationSelectionState {
    Required,
    SelectedAlternative,
    Conditional,
    SelectionMissing,
    UnselectedAlternative,
}

/// One exact native evidence or applicability predicate reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    pub predicate: String,
    pub kind: EvidencePredicateKind,
    pub requirement_id: String,
    pub owner: String,
    pub accepted_revision_blake3: String,
    pub reference_blake3: String,
}

/// Native policy-exception identity for an exception node. The replaced node
/// keeps its own identity; acceptance never relabels it as proved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionReference {
    pub replaces: String,
    pub exception_id: String,
    pub request_revision: u64,
    pub policy_identity: String,
    pub scope_rule_id: String,
    pub control: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationNodeFact {
    pub schema: String,
    pub id: String,
    pub kind: ObligationNodeKind,
    pub parent: Option<String>,
    pub alternative: Option<String>,
    pub children: Vec<String>,
    pub alternatives: Vec<String>,
    pub selection: ObligationSelectionState,
    pub conditions: Vec<String>,
    pub evidence: Vec<EvidenceReference>,
    pub expected_observations: Vec<String>,
    pub command_classes: Vec<String>,
    pub temporal_check: Option<String>,
    pub exception: Option<ExceptionReference>,
    pub obligation_blake3: String,
    pub guidance_blake3: String,
    pub condition_blake3: String,
}

fn mismatch(message: &str) -> WorkflowObligationError {
    WorkflowObligationError::ScopeMismatch(message.to_string())
}

fn evidence_reference(
    requirement: &EvidenceRequirement,
    kind: EvidencePredicateKind,
) -> Result<EvidenceReference, WorkflowObligationError> {
    let mut reference = EvidenceReference {
        predicate: requirement.predicate.clone(),
        kind,
        requirement_id: requirement.requirement_id.clone(),
        owner: requirement.owner.clone(),
        accepted_revision_blake3: requirement.accepted_revision_blake3.clone(),
        reference_blake3: String::new(),
    };
    reference.reference_blake3 = identity(EVIDENCE_DOMAIN, &reference)?;
    Ok(reference)
}

fn shape(
    id: &str,
    kind: ObligationNodeKind,
    parent: Option<&str>,
    alternative: Option<&str>,
    selection: ObligationSelectionState,
    conditions: &[String],
) -> ObligationNodeFact {
    ObligationNodeFact {
        schema: WORKFLOW_OBLIGATION_NODE_SCHEMA_V1.to_string(),
        id: id.to_string(),
        kind,
        parent: parent.map(str::to_string),
        alternative: alternative.map(str::to_string),
        children: Vec::new(),
        alternatives: Vec::new(),
        selection,
        conditions: conditions.to_vec(),
        evidence: Vec::new(),
        expected_observations: Vec::new(),
        command_classes: Vec::new(),
        temporal_check: None,
        exception: None,
        obligation_blake3: String::new(),
        guidance_blake3: String::new(),
        condition_blake3: String::new(),
    }
}

struct Place<'a> {
    parent: Option<&'a str>,
    alternative: Option<&'a str>,
    selection: ObligationSelectionState,
    conditions: &'a [String],
}

/// Pre-order structural walk. Evidence references come from the predicate's
/// resolved requirements, which `resolve_predicate` derived from the same
/// admitted definition.
fn walk(
    expression: &ObligationExpr,
    place: &Place<'_>,
    selected: &BTreeMap<String, String>,
    requirements: &BTreeMap<&str, &EvidenceRequirement>,
    facts: &mut Vec<ObligationNodeFact>,
) -> Result<(), WorkflowObligationError> {
    let requirement = |key: &str| {
        requirements.get(key).copied().ok_or_else(|| {
            WorkflowObligationError::Unsupported(format!(
                "obligation {key} has no resolved native predicate requirement"
            ))
        })
    };
    let kind = match expression {
        ObligationExpr::Evidence { .. } => ObligationNodeKind::Evidence,
        ObligationExpr::AllOf { .. } => ObligationNodeKind::AllOf,
        ObligationExpr::AnyOf { .. } => ObligationNodeKind::AnyOf,
        ObligationExpr::Applicable { .. } => ObligationNodeKind::Applicable,
        ObligationExpr::Temporal { .. } => ObligationNodeKind::Temporal,
    };
    let mut fact = shape(
        expression.id(),
        kind,
        place.parent,
        place.alternative,
        place.selection,
        place.conditions,
    );
    let index = facts.len();
    match expression {
        ObligationExpr::Evidence { id, .. } => {
            fact.evidence = vec![evidence_reference(
                requirement(id)?,
                EvidencePredicateKind::Evidence,
            )?];
            facts.push(fact);
        }
        ObligationExpr::Temporal { check, .. } => {
            fact.temporal_check = Some(check.clone());
            facts.push(fact);
        }
        ObligationExpr::AllOf { id, obligations } => {
            fact.children = obligations
                .iter()
                .map(|child| child.id().to_string())
                .collect();
            facts.push(fact);
            for child in obligations {
                let child_place = Place {
                    parent: Some(id),
                    alternative: None,
                    selection: place.selection,
                    conditions: place.conditions,
                };
                walk(child, &child_place, selected, requirements, facts)?;
            }
        }
        ObligationExpr::AnyOf { id, alternatives } => {
            fact.alternatives = alternatives
                .iter()
                .map(|alternative| alternative.id.clone())
                .collect();
            fact.children = alternatives
                .iter()
                .flat_map(|alternative| alternative.obligations.iter())
                .map(|child| child.id().to_string())
                .collect();
            facts.push(fact);
            for alternative in alternatives {
                let branch = match selected.get(id) {
                    None => ObligationSelectionState::SelectionMissing,
                    Some(choice) if *choice == alternative.id => {
                        ObligationSelectionState::SelectedAlternative
                    }
                    Some(_) => ObligationSelectionState::UnselectedAlternative,
                };
                for child in &alternative.obligations {
                    let child_place = Place {
                        parent: Some(id),
                        alternative: Some(&alternative.id),
                        selection: place.selection.max(branch),
                        conditions: place.conditions,
                    };
                    walk(child, &child_place, selected, requirements, facts)?;
                }
            }
        }
        ObligationExpr::Applicable {
            id,
            condition,
            then,
        } => {
            fact.evidence = vec![evidence_reference(
                requirement(condition)?,
                EvidencePredicateKind::Applicability,
            )?];
            fact.children = vec![then.id().to_string()];
            facts.push(fact);
            let mut conditions = place.conditions.to_vec();
            conditions.push(id.clone());
            let child_place = Place {
                parent: Some(id),
                alternative: None,
                selection: place.selection.max(ObligationSelectionState::Conditional),
                conditions: &conditions,
            };
            walk(then, &child_place, selected, requirements, facts)?;
        }
    }
    debug_assert_eq!(facts[index].id, expression.id());
    Ok(())
}

/// Recompute the structure (everything except exception and expected
/// observation detail, which come from the admitted definition) from a
/// self-contained predicate.
fn structural_facts(
    predicate: &ObligationPredicate,
) -> Result<Vec<ObligationNodeFact>, WorkflowObligationError> {
    let requirements = predicate
        .evidence_requirements
        .iter()
        .map(|requirement| (requirement.node.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    let mut facts = Vec::new();
    let root = Place {
        parent: None,
        alternative: None,
        selection: ObligationSelectionState::Required,
        conditions: &[],
    };
    walk(
        &predicate.expression,
        &root,
        &predicate.selected_alternatives,
        &requirements,
        &mut facts,
    )?;
    Ok(facts)
}

fn seal(
    fact: &mut ObligationNodeFact,
    binding_blake3: &str,
    predicate: &ObligationPredicate,
    guidance: &ObligationGuidance,
) -> Result<(), WorkflowObligationError> {
    // Meaning excludes the per-request selection state and every identity.
    let meaning = (
        &fact.schema,
        &fact.id,
        fact.kind,
        &fact.parent,
        &fact.alternative,
        &fact.children,
        &fact.alternatives,
        &fact.conditions,
        &fact.evidence,
        &fact.expected_observations,
        &fact.command_classes,
        &fact.temporal_check,
        &fact.exception,
    );
    fact.obligation_blake3 = identity(NODE_DOMAIN, &(binding_blake3, meaning))?;
    fact.guidance_blake3 = identity(
        GUIDANCE_DOMAIN,
        &(
            &guidance.binding_blake3,
            &guidance.requirements_blake3,
            &guidance.selected_alternatives,
            &fact.obligation_blake3,
            fact.selection,
        ),
    )?;
    fact.condition_blake3 = identity(
        CONDITION_DOMAIN,
        &(
            &predicate.predicate_blake3,
            &fact.obligation_blake3,
            fact.selection,
        ),
    )?;
    Ok(())
}

fn check_alignment(
    binding_blake3: &str,
    predicate: &ObligationPredicate,
    guidance: &ObligationGuidance,
) -> Result<(), WorkflowObligationError> {
    if predicate.schema != WORKFLOW_OBLIGATION_PREDICATE_SCHEMA_V1
        || predicate.binding.binding_blake3 != binding_blake3
        || guidance.binding_blake3 != binding_blake3
        || guidance.definition_id != predicate.definition_id
        || guidance.workflow_id != predicate.workflow_id
        || guidance.workflow_version != predicate.workflow_version
        || guidance.selected_alternatives != predicate.selected_alternatives
        || guidance.allowed_command_classes != predicate.allowed_command_classes
    {
        return Err(mismatch(
            "predicate and guidance must derive from the same admitted binding and selection",
        ));
    }
    if !verify_predicate_identity(predicate) {
        return Err(mismatch(
            "predicate identity does not match its bound content",
        ));
    }
    Ok(())
}

/// Recompute a predicate's identity from its own content.
#[must_use]
pub fn verify_predicate_identity(predicate: &ObligationPredicate) -> bool {
    let mut unsealed = predicate.clone();
    unsealed.predicate_blake3 = String::new();
    identity("cairn.workflow-obligation-predicate.v1", &unsealed)
        .is_ok_and(|computed| computed == predicate.predicate_blake3)
}

/// Derive typed per-obligation facts for one admitted definition. The
/// predicate and guidance must be the ones derived from `admitted` for the
/// same selection; misaligned inputs are refused rather than mixed.
// r[impl cairn.workflow_obligations.consumer_facts]
pub fn obligation_node_facts(
    admitted: &AdmittedWorkflowObligation,
    predicate: &ObligationPredicate,
    guidance: &ObligationGuidance,
) -> Result<Vec<ObligationNodeFact>, WorkflowObligationError> {
    let binding = &admitted.binding.binding_blake3;
    if predicate.binding != admitted.binding
        || predicate.expression != admitted.definition.completion
    {
        return Err(mismatch(
            "predicate does not derive from the admitted definition",
        ));
    }
    check_alignment(binding, predicate, guidance)?;
    let mut facts = structural_facts(predicate)?;
    let requirements = predicate
        .evidence_requirements
        .iter()
        .map(|requirement| (requirement.node.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    for exception in &admitted.definition.exceptions {
        let replaced = facts
            .iter()
            .find(|fact| fact.id == exception.node)
            .ok_or_else(|| {
                WorkflowObligationError::Unsupported(format!(
                    "exception {} replaces unknown node {}",
                    exception.id, exception.node
                ))
            })?;
        let mut fact = shape(
            &exception.id,
            ObligationNodeKind::Exception,
            None,
            None,
            replaced.selection,
            &replaced.conditions,
        );
        let requirement = requirements.get(exception.id.as_str()).ok_or_else(|| {
            WorkflowObligationError::Unsupported(format!(
                "exception {} has no resolved control predicate",
                exception.id
            ))
        })?;
        fact.evidence = vec![evidence_reference(
            requirement,
            EvidencePredicateKind::Evidence,
        )?];
        fact.exception = Some(ExceptionReference {
            replaces: exception.node.clone(),
            exception_id: exception.exception_id.clone(),
            request_revision: exception.request_revision,
            policy_identity: exception.policy_identity.clone(),
            scope_rule_id: exception.scope_rule_id.clone(),
            control: exception.control.id.clone(),
        });
        facts.push(fact);
    }
    for fact in &mut facts {
        let predicates = fact
            .evidence
            .iter()
            .map(|reference| reference.predicate.as_str())
            .collect::<BTreeSet<_>>();
        let expected = admitted
            .definition
            .expected_observations
            .iter()
            .filter(|observation| predicates.contains(observation.predicate.as_str()))
            .collect::<Vec<_>>();
        fact.expected_observations = expected
            .iter()
            .map(|observation| observation.id.clone())
            .collect();
        fact.command_classes = expected
            .iter()
            .map(|observation| observation.command_class.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        seal(fact, binding, predicate, guidance)?;
    }
    debug_assert!(verify_obligation_node_facts(predicate, guidance, &facts).is_ok());
    Ok(facts)
}

/// Independent consumer check: the facts must match a structural
/// recomputation from the predicate, reference exactly its resolved native
/// predicates and carry recomputable identities.
// r[impl cairn.workflow_obligations.consumer_facts]
pub fn verify_obligation_node_facts(
    predicate: &ObligationPredicate,
    guidance: &ObligationGuidance,
    facts: &[ObligationNodeFact],
) -> Result<(), WorkflowObligationError> {
    let binding = &predicate.binding.binding_blake3;
    check_alignment(binding, predicate, guidance)?;
    let structural = structural_facts(predicate)?;
    if facts.len() < structural.len() {
        return Err(mismatch("obligation facts omit expression nodes"));
    }
    let (expression_facts, exception_facts) = facts.split_at(structural.len());
    let mut ids = BTreeSet::new();
    for (fact, expected) in expression_facts.iter().zip(&structural) {
        let mut unsealed = fact.clone();
        unsealed.expected_observations = Vec::new();
        unsealed.command_classes = Vec::new();
        unsealed.obligation_blake3 = String::new();
        unsealed.guidance_blake3 = String::new();
        unsealed.condition_blake3 = String::new();
        if &unsealed != expected || fact.exception.is_some() {
            return Err(mismatch("obligation fact does not match the predicate"));
        }
    }
    let requirements = predicate
        .evidence_requirements
        .iter()
        .map(|requirement| (requirement.node.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    for fact in exception_facts {
        let Some(exception) = &fact.exception else {
            return Err(mismatch("unexpected non-exception obligation fact"));
        };
        let Some(replaced) = structural.iter().find(|node| node.id == exception.replaces) else {
            return Err(mismatch("exception replaces an unknown obligation"));
        };
        let Some(requirement) = requirements.get(fact.id.as_str()) else {
            return Err(mismatch("exception control is not a resolved predicate"));
        };
        if fact.kind != ObligationNodeKind::Exception
            || fact.parent.is_some()
            || fact.alternative.is_some()
            || !fact.children.is_empty()
            || !fact.alternatives.is_empty()
            || fact.temporal_check.is_some()
            || fact.selection != replaced.selection
            || fact.conditions != replaced.conditions
            || fact.evidence
                != vec![evidence_reference(
                    requirement,
                    EvidencePredicateKind::Evidence,
                )?]
        {
            return Err(mismatch("exception fact does not match the predicate"));
        }
    }
    for fact in facts {
        if fact.schema != WORKFLOW_OBLIGATION_NODE_SCHEMA_V1 || !ids.insert(fact.id.as_str()) {
            return Err(mismatch("obligation fact schema or identity is invalid"));
        }
        let mut sealed = fact.clone();
        seal(&mut sealed, binding, predicate, guidance)?;
        if &sealed != fact {
            return Err(mismatch("obligation fact identity is stale"));
        }
    }
    // Every resolved native requirement (evidence node, applicability
    // condition or exception control) must be carried by exactly one fact key.
    let covered = facts
        .iter()
        .map(|fact| match fact.kind {
            ObligationNodeKind::Applicable => fact
                .evidence
                .first()
                .map_or(fact.id.as_str(), |reference| reference.predicate.as_str()),
            _ => fact.id.as_str(),
        })
        .filter(|key| requirements.contains_key(key))
        .collect::<BTreeSet<_>>();
    if covered.len() != requirements.len() {
        return Err(mismatch("obligation facts omit resolved native predicates"));
    }
    Ok(())
}
