//! Host normalization of an admitted selected-work snapshot into Lattice's
//! `lattice.workflow-intent.v1` input, and admission of the compiled plan.
//!
//! The DTOs mirror Lattice's wire record field for field; Kiln owns them so no
//! Lattice planning code enters Kiln. Lattice compiles an inert plan; Kiln
//! binds that plan identity to the selected obligations and the attempt
//! contract. A worker-proposed intent or plan is data: it never becomes
//! executable until the owner re-admits it with new exact identities.
use std::collections::{BTreeMap, BTreeSet};

use kiln_core::implementation::selected_work::{
    AdmittedSelectedWork, AttemptSnapshotContract, EvidenceKind, NodeSelection, ObligationNode,
    guidance_projection,
};
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INTENT_SCHEMA: &str = "lattice.workflow-intent.v1";
pub const PLAN_SCHEMA: &str = "lattice.workflow-intent-plan.v1";
pub const PLANNING_DISPOSITION: &str = "planning_only_requires_separate_execution_admission";
/// Kiln's host-normalized obligation for authoritative completion.
pub const COMPLETION_OBLIGATION: &str = "kiln.authoritative-completion";
pub const COMPLETION_OBSERVATION_SCHEMA: &str = "cairn.authoritative-completion.v1";
pub const EVIDENCE_OBSERVATION_SCHEMA: &str = "cairn.workflow-obligation-evidence-reference.v1";
/// Lattice's admitted bounds (`MAX_OBLIGATIONS`, `MAX_CONTRACTS_PER_OBLIGATION`,
/// `MAX_IDENTIFIER_BYTES`).
pub const MAX_INTENT_OBLIGATIONS: usize = 128;
pub const MAX_OBSERVATIONS_PER_OBLIGATION: usize = 16;
pub const MAX_INTENT_IDENTIFIER_BYTES: usize = 256;
const NAMED_CHECK: &str = "named_check";
const COMPLETION_ONLY: &str = "completion_only";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentCohort {
    pub producer_identity: String,
    pub selection_identity: String,
    pub obligations_identity: String,
    pub profile_identity: String,
    pub context_identity: String,
    pub source_identity: String,
    pub policy_identity: String,
    pub dependency_identity: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentCoverage {
    Complete,
    Restricted,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentObservationContract {
    pub id: String,
    pub schema: String,
    pub predicate_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentObligation {
    pub id: String,
    pub obligation_identity: String,
    pub guidance_identity: String,
    pub remaining_condition_identity: String,
    pub operation_class: String,
    pub action_identity: String,
    pub prerequisites: Vec<String>,
    pub required_observations: Vec<IntentObservationContract>,
    pub precondition_identity: String,
    pub required_capabilities: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAlternative {
    pub id: String,
    pub implementation_identity: String,
    pub obligations: BTreeSet<String>,
}

/// Lattice `Definition` as a Kiln-owned DTO.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowIntent {
    pub schema: String,
    pub cohort: IntentCohort,
    pub coverage: IntentCoverage,
    pub unsupported_fields: Vec<String>,
    pub obligations: Vec<IntentObligation>,
    pub alternatives: Vec<IntentAlternative>,
    pub selected_alternative: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentCompilerFacts {
    pub expected_cohort: IntentCohort,
    pub compiler_artifact_identity: String,
    pub runtime_profile_identity: String,
    pub available_capabilities: BTreeSet<String>,
    pub available_implementations: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentCompileRequest {
    pub intent: WorkflowIntent,
    pub facts: IntentCompilerFacts,
}

/// Lattice's serialized compiled plan. Graph fields stay opaque values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledPlanObservation {
    pub schema: String,
    pub compiler_revision: String,
    pub identity: String,
    pub graph_identity: Value,
    pub graph: Value,
    pub intent: WorkflowIntent,
    pub facts: IntentCompilerFacts,
    pub nodes: Vec<String>,
    pub disposition: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentRefusal {
    /// The snapshot cannot supply a complete intent (no exported obligations,
    /// a missing alternative selection, or an incomplete enumeration).
    IncompleteCoverage,
    InvalidIdentity,
    LimitExceeded,
    /// The plan echoes another intent, facts, cohort or schema.
    BindingMismatch,
    InvalidPlan,
}

impl std::fmt::Display for IntentRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workflow intent refused: {self:?}")
    }
}

impl std::error::Error for IntentRefusal {}

fn b3(domain: &str, value: &impl Serialize) -> Result<String, IntentRefusal> {
    let bytes = serde_json::to_vec(&(domain, value)).map_err(|_| IntentRefusal::InvalidIdentity)?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

fn prefixed(id: &ContentId) -> String {
    format!("b3:{}", id.as_str())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_INTENT_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-:/".contains(&byte))
}

/// Normalize the admitted snapshot's contributing obligations. Coverage is
/// `complete` only when the producer exported a fully selected plan over a
/// complete enumeration; anything else is `incomplete`, which Lattice refuses.
// r[impl kiln.selected_work.lattice]
pub fn normalize_intent(snapshot: &AdmittedSelectedWork) -> Result<WorkflowIntent, IntentRefusal> {
    snapshot
        .validate()
        .map_err(|_| IntentRefusal::IncompleteCoverage)?;
    let cohort = IntentCohort {
        producer_identity: b3(
            "kiln.lattice-intent.producer.v1",
            &(&snapshot.producer_schema, "cairn"),
        )?,
        selection_identity: prefixed(&snapshot.cohort.selection),
        obligations_identity: prefixed(&snapshot.cohort.obligations),
        profile_identity: prefixed(&snapshot.profile_identity),
        context_identity: prefixed(&snapshot.cohort.context),
        source_identity: prefixed(&snapshot.cohort.source),
        policy_identity: prefixed(&snapshot.policy),
        dependency_identity: prefixed(&snapshot.cohort.dependencies),
    };
    let guidance = guidance_projection(snapshot).map_err(|_| IntentRefusal::InvalidIdentity)?;
    let mut obligations = Vec::new();
    let mut complete = snapshot.enumeration.complete;
    let completion_predicate = match snapshot.plan() {
        None => {
            complete = false;
            prefixed(&snapshot.cohort.obligations)
        }
        Some(plan) => {
            complete &= !plan
                .nodes
                .iter()
                .any(|node| node.selection == NodeSelection::SelectionMissing);
            let by_id: BTreeMap<&str, &ObligationNode> = plan
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node))
                .collect();
            for node in plan.nodes.iter().filter(|node| node.contributing_leaf()) {
                obligations.push(leaf_obligation(snapshot, node, &by_id)?);
            }
            complete &= !obligations.is_empty();
            prefixed(&plan.predicate)
        }
    };
    obligations.sort_by(|left, right| left.id.cmp(&right.id));
    let prerequisites: Vec<String> = obligations.iter().map(|item| item.id.clone()).collect();
    obligations.push(IntentObligation {
        id: COMPLETION_OBLIGATION.into(),
        obligation_identity: b3(
            "kiln.lattice-intent.completion.v1",
            &(&completion_predicate, &snapshot.cohort.verification_subject),
        )?,
        guidance_identity: prefixed(&guidance.identity),
        remaining_condition_identity: b3(
            "kiln.lattice-intent.completion-condition.v1",
            &(&snapshot.cohort.verification_subject, &snapshot.policy),
        )?,
        operation_class: COMPLETION_ONLY.into(),
        action_identity: b3(
            "kiln.lattice-intent.action.v1",
            &(
                COMPLETION_ONLY,
                COMPLETION_OBLIGATION,
                &completion_predicate,
            ),
        )?,
        prerequisites,
        required_observations: vec![IntentObservationContract {
            id: COMPLETION_OBLIGATION.into(),
            schema: COMPLETION_OBSERVATION_SCHEMA.into(),
            predicate_identity: completion_predicate,
        }],
        precondition_identity: b3(
            "kiln.lattice-intent.precondition.v1",
            &(Vec::<&str>::new(), &snapshot.cohort.dependencies),
        )?,
        required_capabilities: BTreeSet::new(),
    });
    obligations.sort_by(|left, right| left.id.cmp(&right.id));
    if obligations.len() > MAX_INTENT_OBLIGATIONS {
        return Err(IntentRefusal::LimitExceeded);
    }
    Ok(WorkflowIntent {
        schema: INTENT_SCHEMA.into(),
        cohort,
        coverage: if complete {
            IntentCoverage::Complete
        } else {
            IntentCoverage::Incomplete
        },
        unsupported_fields: Vec::new(),
        obligations,
        alternatives: Vec::new(),
        selected_alternative: None,
    })
}

fn leaf_obligation(
    snapshot: &AdmittedSelectedWork,
    node: &ObligationNode,
    by_id: &BTreeMap<&str, &ObligationNode>,
) -> Result<IntentObligation, IntentRefusal> {
    if !identifier(&node.id) {
        return Err(IntentRefusal::InvalidIdentity);
    }
    // The leaf's own evidence plus the applicability evidence that gates it.
    let mut contracts: BTreeMap<String, IntentObservationContract> = BTreeMap::new();
    let gating = node
        .conditions
        .iter()
        .filter_map(|condition| by_id.get(condition.as_str()))
        .flat_map(|condition| condition.evidence.iter())
        .filter(|item| item.kind == EvidenceKind::Applicability);
    for item in node.evidence.iter().chain(gating) {
        if !identifier(&item.predicate) {
            return Err(IntentRefusal::InvalidIdentity);
        }
        contracts.insert(
            item.predicate.clone(),
            IntentObservationContract {
                id: item.predicate.clone(),
                schema: EVIDENCE_OBSERVATION_SCHEMA.into(),
                predicate_identity: prefixed(&item.reference),
            },
        );
    }
    if contracts.len() > MAX_OBSERVATIONS_PER_OBLIGATION {
        return Err(IntentRefusal::LimitExceeded);
    }
    Ok(IntentObligation {
        id: node.id.clone(),
        obligation_identity: prefixed(&node.obligation),
        guidance_identity: prefixed(&node.guidance),
        remaining_condition_identity: prefixed(&node.condition),
        operation_class: NAMED_CHECK.into(),
        action_identity: b3(
            "kiln.lattice-intent.action.v1",
            &(NAMED_CHECK, &node.id, &node.obligation),
        )?,
        prerequisites: Vec::new(),
        required_observations: contracts.into_values().collect(),
        precondition_identity: b3(
            "kiln.lattice-intent.precondition.v1",
            &(&node.conditions, &snapshot.cohort.dependencies),
        )?,
        required_capabilities: BTreeSet::new(),
    })
}

/// A complete compile request for this host's frozen compiler and runtime.
pub fn compile_request(
    snapshot: &AdmittedSelectedWork,
    compiler_artifact_identity: &str,
    runtime_profile_identity: &str,
) -> Result<IntentCompileRequest, IntentRefusal> {
    let intent = normalize_intent(snapshot)?;
    if intent.coverage != IntentCoverage::Complete {
        return Err(IntentRefusal::IncompleteCoverage);
    }
    for identity in [compiler_artifact_identity, runtime_profile_identity] {
        if !identity.strip_prefix("b3:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(IntentRefusal::InvalidIdentity);
        }
    }
    Ok(IntentCompileRequest {
        facts: IntentCompilerFacts {
            expected_cohort: intent.cohort.clone(),
            compiler_artifact_identity: compiler_artifact_identity.into(),
            runtime_profile_identity: runtime_profile_identity.into(),
            available_capabilities: BTreeSet::new(),
            available_implementations: BTreeSet::new(),
        },
        intent,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedObligation {
    pub id: String,
    pub obligation_identity: String,
}

/// A compiled plan bound to the attempt contract and its selected obligations.
/// It is still not an execution grant; dispatch admission stays separate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedWorkflowPlan {
    pub plan_identity: String,
    pub intent_identity: String,
    pub compiler_artifact_identity: String,
    pub contract: ContentId,
    pub attempt: ContentId,
    pub work: ContentId,
    pub snapshot: ContentId,
    pub cohort: IntentCohort,
    pub obligations: Vec<AdmittedObligation>,
}

/// Admit Lattice's compiled plan only if it echoes exactly the intent Kiln
/// normalized from the attempt contract's snapshot.
pub fn admit_compiled_plan(
    contract: &AttemptSnapshotContract,
    request: &IntentCompileRequest,
    plan: &CompiledPlanObservation,
) -> Result<AdmittedWorkflowPlan, IntentRefusal> {
    contract
        .verify()
        .map_err(|_| IntentRefusal::BindingMismatch)?;
    let expected = normalize_intent(&contract.snapshot)?;
    if request.intent != expected || request.facts.expected_cohort != expected.cohort {
        return Err(IntentRefusal::BindingMismatch);
    }
    if plan.schema != PLAN_SCHEMA
        || plan.disposition != PLANNING_DISPOSITION
        || !plan.identity.starts_with("b3:")
    {
        return Err(IntentRefusal::InvalidPlan);
    }
    if plan.intent != request.intent || plan.facts != request.facts {
        return Err(IntentRefusal::BindingMismatch);
    }
    let ids: Vec<String> = expected
        .obligations
        .iter()
        .map(|item| item.id.clone())
        .collect();
    if plan.nodes != ids {
        return Err(IntentRefusal::BindingMismatch);
    }
    Ok(AdmittedWorkflowPlan {
        plan_identity: plan.identity.clone(),
        intent_identity: b3("kiln.lattice-intent.v1", &expected)?,
        compiler_artifact_identity: request.facts.compiler_artifact_identity.clone(),
        contract: contract.identity.clone(),
        attempt: contract.attempt.clone(),
        work: contract.work.clone(),
        snapshot: contract.snapshot.snapshot.clone(),
        cohort: expected.cohort,
        obligations: expected
            .obligations
            .into_iter()
            .map(|item| AdmittedObligation {
                id: item.id,
                obligation_identity: item.obligation_identity,
            })
            .collect(),
    })
}

/// What a proposal changes relative to the admitted plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalDifference {
    pub admitted_plan: String,
    pub proposed_plan: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

/// Review outcome for a worker-proposed plan. Neither variant grants
/// execution: an identical plan is the already admitted plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum ProposalReview {
    IdenticalToAdmitted,
    OwnerReadmissionRequired(ProposalDifference),
}

/// A worker-proposed intent or plan is never executable. An identical plan is
/// the admitted plan itself; anything else returns its difference and requires
/// owner re-admission with new exact identities.
// r[impl kiln.selected_work.lattice.scenario.proposal]
#[must_use]
pub fn review_proposal(
    admitted: &AdmittedWorkflowPlan,
    proposed: &CompiledPlanObservation,
) -> ProposalReview {
    let before: BTreeMap<&str, &str> = admitted
        .obligations
        .iter()
        .map(|item| (item.id.as_str(), item.obligation_identity.as_str()))
        .collect();
    let after: BTreeMap<&str, &IntentObligation> = proposed
        .intent
        .obligations
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let difference = ProposalDifference {
        admitted_plan: admitted.plan_identity.clone(),
        proposed_plan: proposed.identity.clone(),
        added: after
            .keys()
            .filter(|id| !before.contains_key(**id))
            .map(|id| (*id).to_string())
            .collect(),
        removed: before
            .keys()
            .filter(|id| !after.contains_key(**id))
            .map(|id| (*id).to_string())
            .collect(),
        changed: before
            .iter()
            .filter(|(id, identity)| {
                after
                    .get(**id)
                    .is_some_and(|item| item.obligation_identity != **identity)
            })
            .map(|(id, _)| (*id).to_string())
            .collect(),
    };
    let same_intent = b3("kiln.lattice-intent.v1", &proposed.intent)
        .is_ok_and(|identity| identity == admitted.intent_identity);
    if proposed.identity == admitted.plan_identity
        && same_intent
        && proposed.facts.expected_cohort == admitted.cohort
        && proposed.facts.compiler_artifact_identity == admitted.compiler_artifact_identity
    {
        return ProposalReview::IdenticalToAdmitted;
    }
    ProposalReview::OwnerReadmissionRequired(difference)
}

#[cfg(test)]
#[path = "workflow_intent_tests.rs"]
mod tests;
