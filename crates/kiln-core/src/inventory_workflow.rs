//! Workflow revision slots of inventory v2. One fact per workflow definition:
//! its desired, last usable, selected and pinned workflow revisions, each
//! binding one compiled workflow and the worker revision it pairs with.
//!
//! Only identities have a field here. No guidance, context, source, session,
//! grant or name reaches this model, and pins are counted per revision and
//! state rather than listed by session.
// r[impl kiln.inventory.workflow]
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::{FactMeta, FactRef, MAX_EXACT_NUMBER, PinStatus, reference, token};

/// Workflow revisions one definition fact may list.
pub const MAX_WORKFLOW_REVISIONS: usize = 64;
/// Pinned sessions one definition fact may count, in all of its pin groups.
pub const MAX_WORKFLOW_PINNED_SESSIONS: u64 = 1024;

/// How a producer selected a revision for new sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKind {
    Promotion,
    Rollback,
}

/// The compiled workflow a workflow revision runs: its compiler, plan, graph
/// and the Cairn selection, obligations and policy identities it was compiled
/// against.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledWorkflow {
    pub compiler_revision: String,
    pub compiler_artifact: String,
    pub plan: String,
    pub graph: String,
    pub selection: String,
    pub obligations: String,
    pub policy: String,
}

/// The worker revision a workflow revision pairs with. `definition` names the
/// worker's definition fact in the agents join source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerPairing {
    pub definition: FactRef,
    pub revision: String,
    pub bundle: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRevision {
    pub revision: String,
    pub workflow: CompiledWorkflow,
    pub worker: WorkerPairing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSelection {
    pub revision: String,
    pub rollout_generation: u64,
    pub kind: SelectionKind,
    /// The qualification reference the selection names; evidence, never authority.
    pub qualification: String,
}

/// Sessions pinned to one revision in one pin state. `blocked` means a
/// further effect under the pin would be refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPin {
    pub revision: String,
    pub status: PinStatus,
    pub blocked: bool,
    pub sessions: u64,
}

/// One workflow definition's revision slots. `revisions` lists exactly the
/// revisions the slots name, strictly ascending; `pinned` is strictly
/// ascending by revision, status and blocked state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRevisionSlots {
    pub meta: FactMeta,
    pub revisions: Vec<WorkflowRevision>,
    pub desired: Option<String>,
    pub last_usable: Option<String>,
    pub selected_for_new_sessions: Option<WorkflowSelection>,
    pub pinned: Vec<WorkflowPin>,
}

impl WorkflowRevisionSlots {
    /// The listed revision a slot names.
    #[must_use]
    pub fn revision(&self, revision: &str) -> Option<&WorkflowRevision> {
        self.revisions
            .binary_search_by(|listed| listed.revision.as_str().cmp(revision))
            .ok()
            .and_then(|index| self.revisions.get(index))
    }

    fn named(&self) -> impl Iterator<Item = &str> {
        self.desired
            .iter()
            .chain(self.last_usable.iter())
            .map(String::as_str)
            .chain(
                self.selected_for_new_sessions
                    .iter()
                    .map(|selection| selection.revision.as_str()),
            )
            .chain(self.pinned.iter().map(|pin| pin.revision.as_str()))
    }
}

fn pin_order(pin: &WorkflowPin) -> (&str, u8, bool) {
    let status = match pin.status {
        PinStatus::Live => 0,
        PinStatus::Closed => 1,
        PinStatus::Released => 2,
    };
    (pin.revision.as_str(), status, pin.blocked)
}

fn admit_revision(value: &WorkflowRevision) -> Result<(), &'static str> {
    let workflow = &value.workflow;
    for identity in [
        &value.revision,
        &workflow.compiler_revision,
        &workflow.compiler_artifact,
        &workflow.plan,
        &workflow.graph,
        &workflow.selection,
        &workflow.obligations,
        &workflow.policy,
        &value.worker.revision,
        &value.worker.bundle,
    ] {
        token(identity)?;
    }
    reference(&value.worker.definition)
}

fn admit_pins(pins: &[WorkflowPin]) -> Result<(), &'static str> {
    let mut sessions = 0u64;
    for pin in pins {
        if pin.sessions == 0 {
            return Err("inventory_workflow_pin_invalid");
        }
        sessions = sessions
            .checked_add(pin.sessions)
            .filter(|total| *total <= MAX_WORKFLOW_PINNED_SESSIONS)
            .ok_or("inventory_field_bound")?;
    }
    let ascending = pins.windows(2).all(|pair| match pair {
        [first, second] => pin_order(first) < pin_order(second),
        _ => false,
    });
    if ascending {
        Ok(())
    } else {
        Err("inventory_workflow_pin_unordered")
    }
}

/// Admit one fact's slots: bounded public tokens, revisions strictly
/// ascending, every named revision listed and every listed revision named.
pub(super) fn admit_workflow_revisions(value: &WorkflowRevisionSlots) -> Result<(), &'static str> {
    if value.revisions.len() > MAX_WORKFLOW_REVISIONS {
        return Err("inventory_field_bound");
    }
    for revision in &value.revisions {
        admit_revision(revision)?;
    }
    let ascending = value.revisions.windows(2).all(|pair| match pair {
        [first, second] => first.revision < second.revision,
        _ => false,
    });
    if !ascending {
        return Err("inventory_workflow_revision_unordered");
    }
    if let Some(selection) = &value.selected_for_new_sessions {
        token(&selection.qualification)?;
        if selection.rollout_generation > MAX_EXACT_NUMBER {
            return Err("inventory_number_not_exact");
        }
    }
    admit_pins(&value.pinned)?;
    let named: BTreeSet<&str> = value.named().collect();
    if named
        .iter()
        .any(|revision| value.revision(revision).is_none())
    {
        return Err("inventory_workflow_revision_unlisted");
    }
    if named.len() != value.revisions.len() {
        return Err("inventory_workflow_revision_unreferenced");
    }
    Ok(())
}

#[cfg(test)]
#[path = "inventory_workflow_tests.rs"]
mod tests;
