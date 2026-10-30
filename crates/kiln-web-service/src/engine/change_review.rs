//! Canonical execution observations admitted as review CI evidence.

use kiln_core::change_review::CiFact;
use kiln_core::domain::Outcome;

use super::Engine;

impl Engine {
    pub(crate) fn review_ci_outcome(&self, run_id: &str) -> Option<(bool, bool)> {
        self.state.runs.get(run_id).map(|run| {
            (
                run.outcome().is_terminal(),
                !self.storage_failed
                    && run.outcome() == Outcome::Success
                    && run.untrusted.as_ref().is_none_or(|gate| {
                        gate.satisfies_required_ci(run.source.exact_revision.native_id.as_str())
                    }),
            )
        })
    }

    /// Cancel a superseded speculative review candidate run that has not been
    /// dispatched yet. Dispatched runs are left alone: provider cancellation is
    /// an ambiguous external effect, so the review queue ignores their results.
    /// Returns whether the run was cancelled; repeated calls are no-ops.
    // r[impl kiln.review.queue.speculative.failure]
    pub(crate) fn cancel_review_candidate(&mut self, run_id: &str) -> Result<bool, String> {
        if self.storage_failed {
            return Err("durable_state_unavailable".into());
        }
        let Some(run) = self.state.runs.get_mut(run_id) else {
            return Ok(false);
        };
        if run.phase != "queued" {
            return Ok(false);
        }
        run.complete(Outcome::Cancelled, None)?;
        run.phase = "terminal".into();
        run.completed_at_ms = Some(super::host_time_ms()?);
        self.persist()?;
        Ok(true)
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
                // r[impl kiln.untrusted.review]
                untrusted: run
                    .untrusted
                    .as_ref()
                    .map(kiln_core::untrusted::UntrustedGate::summary),
            })
            .collect()
    }
}
