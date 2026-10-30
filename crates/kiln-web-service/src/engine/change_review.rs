//! Canonical execution observations admitted as review CI evidence.

use kiln_core::change_review::CiFact;
use kiln_core::domain::Outcome;

use super::Engine;

impl Engine {
    pub(crate) fn review_ci_outcome(&self, run_id: &str) -> Option<(bool, bool)> {
        self.state.runs.get(run_id).map(|run| {
            (
                run.outcome().is_terminal(),
                !self.storage_failed && run.outcome() == Outcome::Success,
            )
        })
    }

    /// Export retained canonical run facts, not browser assertions or receipt votes.
    ///
    /// Keep failed/non-terminal observations: the newest run for an exact source
    /// supersedes an earlier success. Dirty snapshots and partial-node reruns can
    /// be inspected but never satisfy a full clean revision requirement.
    pub(crate) fn code_review_ci(&self) -> Vec<CiFact> {
        self.state
            .runs
            .values()
            .map(|run| CiFact {
                project: run.project_id.clone(),
                repository: run.source.repository.native_id.as_str().to_owned(),
                revision: run.source.exact_revision.native_id.as_str().to_owned(),
                run_id: run.id.clone(),
                ordinal: run.ordinal,
                success: !self.storage_failed && run.outcome() == Outcome::Success,
                full_run: run.selected_nodes.is_empty(),
                clean: run.dirty_snapshot.is_none(),
                scope_identity: run.scope.as_ref().map(|scope| scope.identity.clone()),
                terminal: run.outcome().is_terminal(),
            })
            .collect()
    }
}
