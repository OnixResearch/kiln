//! Pure ordering and selection over admitted readiness observations.
//!
//! Nickel owns the manifest, profile, instruction, and bound declarations.
//! This module admits that checked projection and selects ordered next actions
//! over facts the shell already observed. It performs no reads, discovery,
//! inference from task checkboxes, execution, or mutation.

use super::{ReadinessAction, ReadinessReport, ReadinessState, READINESS_STAGES};
use crate::ChangeDependencyGraph;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::LazyLock;

/// The checked projection is absent or does not match its declared contract.
pub const WORKFLOW_READINESS_POLICY_INVALID: &str = "workflow_readiness.policy.invalid";
/// The checked projection declares a non-positive or inconsistent bound.
pub const WORKFLOW_READINESS_POLICY_BOUNDS: &str = "workflow_readiness.policy.bounds";
/// The admitted observation exceeds a declared collection bound.
pub const WORKFLOW_READINESS_BOUND: &str = "workflow_readiness.bound";

/// Declared collection bounds for the readiness-backed drain view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReadinessBounds {
    pub max_changes: usize,
    pub max_manifest_bytes: u64,
    pub max_manifest_issues: usize,
    pub max_evidence_files: usize,
    pub max_actions: usize,
    pub max_actions_per_change: usize,
    pub max_path_bytes: usize,
    pub max_subject_hash_bytes: usize,
}

/// Declared exact instruction route for one selected task artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReadinessInstruction {
    pub tool: String,
    pub cli: Vec<String>,
    pub task_artifact: String,
    pub expected_observation: String,
}

/// Checked profile binding the manifest, view, ordering, and bounds contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReadinessPolicy {
    pub schema: String,
    pub manifest_schema: String,
    pub view_schema: String,
    pub default_view: String,
    pub readiness_view: String,
    pub views: Vec<String>,
    pub action_order: Vec<String>,
    pub preview_kinds: Vec<String>,
    pub task_prerequisite_stages: Vec<String>,
    pub instruction: WorkflowReadinessInstruction,
    pub bounds: WorkflowReadinessBounds,
    pub non_claims: Vec<String>,
}

static WORKFLOW_READINESS_POLICY: LazyLock<Result<WorkflowReadinessPolicy, &'static str>> =
    LazyLock::new(|| {
        let value: WorkflowReadinessPolicy = serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/workflow-readiness.json"
        ))
        .map_err(|_| WORKFLOW_READINESS_POLICY_INVALID)?;
        admit_workflow_readiness_policy(&value)?;
        Ok(value)
    });

/// Borrow the checked generated projection, failing closed when it is absent.
pub fn workflow_readiness_policy() -> Result<&'static WorkflowReadinessPolicy, &'static str> {
    match &*WORKFLOW_READINESS_POLICY {
        Ok(policy) => Ok(policy),
        Err(code) => Err(*code),
    }
}

/// Admit one typed projection against structural invariants.
///
/// This checks binding consistency only. It does not reinterpret the merged
/// readiness policy or duplicate a runtime literal of the generated values.
pub fn admit_workflow_readiness_policy(
    policy: &WorkflowReadinessPolicy,
) -> Result<(), &'static str> {
    if policy.schema.is_empty()
        || policy.manifest_schema.is_empty()
        || policy.view_schema.is_empty()
        || policy.non_claims.is_empty()
        || policy
            .non_claims
            .iter()
            .any(|claim| claim.trim().is_empty())
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    if policy.views.len() < 2
        || !unique_nonempty(&policy.views)
        || !policy.views.iter().any(|view| view == &policy.default_view)
        || !policy
            .views
            .iter()
            .any(|view| view == &policy.readiness_view)
        || policy.default_view == policy.readiness_view
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    if policy.action_order.is_empty()
        || !unique_nonempty(&policy.action_order)
        || !policy.action_order.iter().any(|kind| kind == "task")
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    if policy.preview_kinds.is_empty()
        || !unique_nonempty(&policy.preview_kinds)
        || !policy
            .preview_kinds
            .iter()
            .all(|kind| policy.action_order.contains(kind))
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    if policy.task_prerequisite_stages.is_empty()
        || !unique_nonempty(&policy.task_prerequisite_stages)
        || !policy
            .task_prerequisite_stages
            .iter()
            .all(|stage| READINESS_STAGES.contains(&stage.as_str()))
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    if policy.instruction.tool.is_empty()
        || policy.instruction.cli.is_empty()
        || policy.instruction.cli.iter().any(|part| part.is_empty())
        || policy.instruction.task_artifact.is_empty()
        || policy.instruction.expected_observation.trim().is_empty()
    {
        return Err(WORKFLOW_READINESS_POLICY_INVALID);
    }
    let bounds = &policy.bounds;
    if bounds.max_changes == 0
        || bounds.max_manifest_bytes == 0
        || bounds.max_manifest_issues == 0
        || bounds.max_evidence_files == 0
        || bounds.max_actions == 0
        || bounds.max_actions_per_change == 0
        || bounds.max_path_bytes == 0
        || bounds.max_subject_hash_bytes == 0
    {
        return Err(WORKFLOW_READINESS_POLICY_BOUNDS);
    }
    Ok(())
}

fn unique_nonempty(values: &[String]) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value.as_str()))
}

/// Declared ready-task order admitted by the existing task-order validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainTaskOrdering<'a> {
    pub available: bool,
    pub active: bool,
    pub valid: bool,
    pub ready_tasks: &'a [String],
}

/// One admitted per-change observation. No value here is discovered by this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainReadinessChange<'a> {
    pub name: &'a str,
    pub report: &'a ReadinessReport,
    pub blocked_by: &'a [String],
    pub dependency_issue_count: usize,
    pub task_ordering: DrainTaskOrdering<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainInstructionRoute {
    pub tool: String,
    pub arguments: Value,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainNextTask {
    pub task: String,
    pub instruction: DrainInstructionRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainSelectedAction {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub causes: Vec<String>,
    pub prerequisites: Vec<String>,
    pub authority: String,
    pub expected_observation: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainChangeSelection {
    pub name: String,
    pub ordering_blocked: bool,
    pub task_eligible: bool,
    pub next_task: Option<DrainNextTask>,
    pub actions: Vec<DrainSelectedAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainReadinessSelection {
    pub change_order: Vec<String>,
    pub changes: Vec<DrainChangeSelection>,
    pub truncated: bool,
    pub ready_change_count: usize,
    pub blocked_change_count: usize,
    pub unavailable_change_count: usize,
}

// r[impl cairn.agent_workflow_readiness.order]
// r[impl cairn.agent_workflow_readiness.shared]
/// Order admitted changes and select preview-scoped next actions.
///
/// Dependency order comes first, then declared ready-task order, then the
/// checked action-kind order. A change whose declared predecessor is not
/// archived stays ordering-blocked regardless of another report's transport or
/// classification, and it can never select implementation or preview work.
pub fn select_drain_readiness(
    changes: &[DrainReadinessChange<'_>],
    graph: &ChangeDependencyGraph,
    policy: &WorkflowReadinessPolicy,
) -> Result<DrainReadinessSelection, &'static str> {
    if changes.len() > policy.bounds.max_changes {
        return Err(WORKFLOW_READINESS_BOUND);
    }
    let change_order = order_change_names(changes, &graph.drain_order);
    let mut selection = DrainReadinessSelection {
        changes: Vec::with_capacity(change_order.len()),
        change_order,
        truncated: false,
        ready_change_count: 0,
        blocked_change_count: 0,
        unavailable_change_count: 0,
    };
    let mut remaining_actions = policy.bounds.max_actions;
    for name in &selection.change_order {
        let Some(change) = changes.iter().find(|change| change.name == name.as_str()) else {
            continue;
        };
        let ordering_blocked = !change.blocked_by.is_empty() || change.dependency_issue_count > 0;
        let task_eligible = !ordering_blocked && task_prerequisites_ready(change, policy);
        let next_task = task_eligible
            .then(|| change.task_ordering.ready_tasks.first())
            .flatten()
            .map(|task| next_task_route(change.name, task, policy));
        let mut actions = change
            .report
            .actions
            .iter()
            .filter(|action| {
                !ordering_blocked || !policy.preview_kinds.iter().any(|kind| kind == &action.kind)
            })
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| {
            action_rank(policy, &left.kind)
                .cmp(&action_rank(policy, &right.kind))
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.id.cmp(&right.id))
        });
        actions.truncate(policy.bounds.max_actions_per_change);

        let mut selected = Vec::with_capacity(actions.len() + usize::from(next_task.is_some()));
        if let Some(task) = &next_task {
            if remaining_actions == 0 {
                selection.truncated = true;
            } else {
                remaining_actions -= 1;
                selected.push(selected_task_action(change, task, policy));
            }
        }
        for action in actions {
            if remaining_actions == 0 {
                selection.truncated = true;
                break;
            }
            remaining_actions -= 1;
            selected.push(selected_readiness_action(action));
        }
        if change.report.classification == "unavailable" {
            selection.unavailable_change_count += 1;
        } else if change.report.classification == "ready" && !ordering_blocked {
            selection.ready_change_count += 1;
        } else {
            selection.blocked_change_count += 1;
        }
        selection.changes.push(DrainChangeSelection {
            name: change.name.to_string(),
            ordering_blocked,
            task_eligible,
            next_task,
            actions: selected,
        });
    }
    Ok(selection)
}

/// Dependency order first, then any admitted change absent from that order.
fn order_change_names(changes: &[DrainReadinessChange<'_>], drain_order: &[String]) -> Vec<String> {
    let present = changes
        .iter()
        .map(|change| change.name)
        .collect::<std::collections::BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(changes.len());
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for name in drain_order {
        if let Some(change) = changes.iter().find(|change| change.name == name.as_str()) {
            if seen.insert(change.name) {
                ordered.push(change.name.to_string());
            }
        }
    }
    ordered.extend(present.difference(&seen).copied().map(str::to_owned));
    ordered
}

/// Declared ready-task order is usable only when every earlier stage passed.
fn task_prerequisites_ready(
    change: &DrainReadinessChange<'_>,
    policy: &WorkflowReadinessPolicy,
) -> bool {
    if !change.task_ordering.available
        || !change.task_ordering.active
        || !change.task_ordering.valid
        || change.task_ordering.ready_tasks.is_empty()
    {
        return false;
    }
    policy.task_prerequisite_stages.iter().all(|stage| {
        change
            .report
            .stages
            .iter()
            .find(|candidate| candidate.stage == *stage)
            .is_some_and(|candidate| candidate.state == ReadinessState::Ready)
    })
}

fn next_task_route(change: &str, task: &str, policy: &WorkflowReadinessPolicy) -> DrainNextTask {
    let instruction = &policy.instruction;
    let mut argv = instruction.cli.clone();
    argv.push(instruction.task_artifact.clone());
    argv.push("--change".to_string());
    argv.push(change.to_string());
    DrainNextTask {
        task: task.to_string(),
        instruction: DrainInstructionRoute {
            tool: instruction.tool.clone(),
            arguments: json!({"change": change, "artifact": instruction.task_artifact}),
            argv,
        },
    }
}

fn selected_task_action(
    change: &DrainReadinessChange<'_>,
    task: &DrainNextTask,
    policy: &WorkflowReadinessPolicy,
) -> DrainSelectedAction {
    DrainSelectedAction {
        id: crate::canonical_json_hash(&json!([change.name, "task", task.task])),
        kind: "task".to_string(),
        target: task.task.clone(),
        causes: Vec::new(),
        prerequisites: vec![
            change.report.scope.policy_hash.clone(),
            change.report.scope.source_hash.clone(),
        ],
        authority: "advisory".to_string(),
        expected_observation: policy.instruction.expected_observation.clone(),
        argv: Vec::new(),
    }
}

fn selected_readiness_action(action: &ReadinessAction) -> DrainSelectedAction {
    DrainSelectedAction {
        id: action.id.clone(),
        kind: action.kind.clone(),
        target: action.target.clone(),
        causes: action.causes.clone(),
        prerequisites: action.prerequisites.clone(),
        authority: action.authority.clone(),
        expected_observation: action.expected_observation.clone(),
        argv: action.argv.clone(),
    }
}

fn action_rank(policy: &WorkflowReadinessPolicy, kind: &str) -> usize {
    policy
        .action_order
        .iter()
        .position(|candidate| candidate == kind)
        .unwrap_or(policy.action_order.len())
}

#[cfg(test)]
mod tests;
