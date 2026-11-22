//! Selected-work transport of one admitted workflow obligation definition.
//!
//! The workflow obligation module owns obligation meaning, the predicate,
//! guidance, typed node facts and evaluation. This module only binds them to
//! one selected-work snapshot (its verification subject, context and workflow
//! policy) and independently re-admits the portable result. Missing or
//! unsupported observations stay remaining; they never become an empty
//! satisfied obligation set.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{CohortIdentity, SelectedWorkError, SemanticAvailability};
use crate::verified::workflow_obligations::{
    evaluate_workflow_obligations, find_workflow_obligation, obligation_guidance,
    obligation_node_facts, obligation_predicate, verify_obligation_node_facts, ActionPrecondition,
    AdmittedWorkflowObligationCatalog, AgentClaims, EvidenceObservation, ExceptionSnapshot,
    ExpectedObservation, ObligationBinding, ObligationGuidance, ObligationNodeFact,
    ObligationPredicate, ObligationRequest, ProgressObservation, TemporalObservation,
    TerminalObservation, WorkflowObligationReport, WORKFLOW_OBLIGATION_REPORT_SCHEMA_V1,
};

/// Upper bound on transported obligation facts; the admitted obligation policy
/// bounds definitions far below this.
const MAX_OBLIGATION_NODES: usize = 256;

/// Explicit caller-supplied observations for the selected definition. Every
/// field is optional; an absent field is an absent observation, never a
/// satisfied one. The snapshot supplies the subject, context and binding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationObservationInput {
    #[serde(default)]
    pub selected_alternatives: BTreeMap<String, String>,
    #[serde(default)]
    pub evidence: Vec<EvidenceObservation>,
    #[serde(default)]
    pub progress: Vec<ProgressObservation>,
    #[serde(default)]
    pub agent_claims: Option<AgentClaims>,
    #[serde(default)]
    pub exceptions: Vec<ExceptionSnapshot>,
    #[serde(default)]
    pub temporal: Option<TemporalObservation>,
    #[serde(default)]
    pub terminal: Option<TerminalObservation>,
}

/// One admitted catalog, a definition ID and optional observations.
pub struct ObligationSource<'a> {
    pub catalog: &'a AdmittedWorkflowObligationCatalog,
    pub definition: &'a str,
    pub observations: Option<&'a ObligationObservationInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedObligationExport {
    pub binding: ObligationBinding,
    pub predicate: ObligationPredicate,
    pub guidance: ObligationGuidance,
    pub nodes: Vec<ObligationNodeFact>,
    pub action_preconditions: Vec<ActionPrecondition>,
    pub expected_observations: Vec<ExpectedObservation>,
    pub observations_supplied: bool,
    pub evaluation: WorkflowObligationReport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "export",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SelectedObligations {
    NotSelected,
    Exported(Box<SelectedObligationExport>),
}

// r[impl cairn.selected_work.obligations]
pub(super) fn compose_obligations(
    source: &ObligationSource<'_>,
    policy_hash: &str,
    subject: &str,
    context: &str,
) -> Result<SelectedObligations, SelectedWorkError> {
    let admitted = find_workflow_obligation(source.catalog, source.definition)
        .ok_or(SelectedWorkError::InvalidSelection)?;
    if admitted.binding.workflow_policy_blake3 != policy_hash
        || source.catalog.workflow_policy_blake3 != policy_hash
    {
        return Err(SelectedWorkError::CrossedSubject);
    }
    let empty = ObligationObservationInput::default();
    let observations = source.observations.unwrap_or(&empty);
    let selection = &observations.selected_alternatives;
    let policy = &source.catalog.policy;
    let invalid = |_| SelectedWorkError::InvalidObligations;
    let predicate =
        obligation_predicate(admitted, policy, selection, subject, context).map_err(invalid)?;
    let guidance = obligation_guidance(admitted, policy, selection).map_err(invalid)?;
    let nodes = obligation_node_facts(admitted, &predicate, &guidance).map_err(invalid)?;
    let request = ObligationRequest {
        binding: admitted.binding.clone(),
        subject_blake3: subject.to_string(),
        context_blake3: context.to_string(),
        selected_alternatives: selection.clone(),
        evidence: observations.evidence.clone(),
        progress: observations.progress.clone(),
        agent_claims: observations.agent_claims.clone(),
        exceptions: observations.exceptions.clone(),
        temporal: observations.temporal.clone(),
        terminal: observations.terminal.clone(),
    };
    let evaluation = evaluate_workflow_obligations(admitted, policy, &request).map_err(invalid)?;
    Ok(SelectedObligations::Exported(Box::new(
        SelectedObligationExport {
            binding: admitted.binding.clone(),
            predicate,
            guidance,
            nodes,
            action_preconditions: admitted.definition.action_preconditions.clone(),
            expected_observations: admitted.definition.expected_observations.clone(),
            observations_supplied: source.observations.is_some(),
            evaluation,
        },
    )))
}

fn check_expected_observations(export: &SelectedObligationExport) -> bool {
    let mut ids = BTreeSet::new();
    let unique = export
        .expected_observations
        .iter()
        .all(|observation| ids.insert(observation.id.as_str()))
        && {
            let mut preconditions = BTreeSet::new();
            export
                .action_preconditions
                .iter()
                .all(|precondition| preconditions.insert(precondition.id.as_str()))
        };
    unique
        && export.nodes.iter().all(|node| {
            let predicates = node
                .evidence
                .iter()
                .map(|reference| reference.predicate.as_str())
                .collect::<BTreeSet<_>>();
            let matching = export
                .expected_observations
                .iter()
                .filter(|observation| predicates.contains(observation.predicate.as_str()))
                .collect::<Vec<_>>();
            node.expected_observations
                == matching
                    .iter()
                    .map(|observation| observation.id.clone())
                    .collect::<Vec<_>>()
                && node.command_classes
                    == matching
                        .iter()
                        .map(|observation| observation.command_class.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
        })
}

/// The evaluation must explain this exact predicate, and its outcome must be
/// consistent with its own satisfied and remaining sets.
fn check_evaluation(export: &SelectedObligationExport) -> bool {
    let report = &export.evaluation;
    let predicate = &export.predicate;
    let nodes = export
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let preconditions = export
        .action_preconditions
        .iter()
        .map(|precondition| precondition.id.as_str())
        .collect::<BTreeSet<_>>();
    let satisfied = report
        .satisfied
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let bound = report.schema == WORKFLOW_OBLIGATION_REPORT_SCHEMA_V1
        && report.binding == export.binding
        && report.workflow_id == predicate.workflow_id
        && report.workflow_version == predicate.workflow_version
        && report.definition_id == predicate.definition_id
        && report.predicate_blake3 == predicate.predicate_blake3
        && report.selected_alternatives == predicate.selected_alternatives;
    let explained = !report.completion.is_empty()
        && report
            .completion
            .iter()
            .all(|node| nodes.contains(node.id.as_str()))
        && satisfied.iter().all(|id| nodes.contains(id))
        && report.remaining.iter().all(|remaining| {
            !satisfied.contains(remaining.id.as_str())
                && (nodes.contains(remaining.id.as_str())
                    || preconditions.contains(remaining.id.as_str()))
        });
    let success = matches!(report.outcome.as_str(), "proved" | "exception_accepted");
    let open = report
        .remaining
        .iter()
        .any(|remaining| nodes.contains(remaining.id.as_str()));
    let honest = matches!(
        report.outcome.as_str(),
        "proved"
            | "exception_accepted"
            | "unsatisfied"
            | "blocked"
            | "abandoned"
            | "unknown"
            | "unsupported"
    ) && report.proved == (report.outcome == "proved")
        && report.successful_delivery == success
        && !(success && open)
        && (success || open || matches!(report.outcome.as_str(), "blocked" | "abandoned"))
        && (export.observations_supplied || !success);
    bound && explained && honest
}

/// Re-admit a transported obligation section against the snapshot's own
/// recomputed cohort and workflow policy.
// r[impl cairn.selected_work.obligations]
pub(super) fn admit_obligations(
    section: &SelectedObligations,
    availability: SemanticAvailability,
    policy_hash: &str,
    cohort: &CohortIdentity,
) -> Result<(), SelectedWorkError> {
    let export = match (section, availability) {
        (SelectedObligations::NotSelected, SemanticAvailability::NotSelected) => return Ok(()),
        (SelectedObligations::Exported(export), SemanticAvailability::Supported) => export,
        _ => return Err(SelectedWorkError::UnsupportedSchema),
    };
    if export.binding.workflow_policy_blake3 != policy_hash
        || export.predicate.subject_blake3 != cohort.verification_subject
        || export.predicate.context_blake3 != cohort.context
        || export.evaluation.subject_blake3 != cohort.verification_subject
        || export.evaluation.context_blake3 != cohort.context
    {
        return Err(SelectedWorkError::CrossedSubject);
    }
    if export.predicate.binding != export.binding
        || export.nodes.is_empty()
        || export.nodes.len() > MAX_OBLIGATION_NODES
    {
        return Err(SelectedWorkError::InvalidObligations);
    }
    verify_obligation_node_facts(&export.predicate, &export.guidance, &export.nodes)
        .map_err(|_| SelectedWorkError::InvalidObligations)?;
    if !check_expected_observations(export) || !check_evaluation(export) {
        return Err(SelectedWorkError::InvalidObligations);
    }
    Ok(())
}
