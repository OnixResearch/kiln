//! Change-review display of untrusted pull-request CI runs. Display only: the
//! pure checklist decides whether an untrusted result counts for required CI,
//! and an approval never touches review approvals, ownership or vetoes.

use kiln_core::change_review::{Change, CiFact, IntegrationFact};
use serde_json::{Value, json};

use super::Failure;

/// Newest untrusted runs shown on the `ci` checklist item.
const MAX_CHECKLIST_UNTRUSTED_RUNS: usize = 8;

// r[impl kiln.untrusted.console]
/// Attach `untrusted_runs` (state, approver, approved commit, counting) to the
/// `ci` checklist item for the exact commit the checklist evaluates.
pub(super) fn annotate_checklist(
    checklist: &mut Value,
    ci: &[CiFact],
    change: &Change,
    integration: Option<&IntegrationFact>,
) -> Result<(), Failure> {
    let commit = integration.map_or(change.current()?.commit.as_str(), |proof| {
        proof.candidate_commit.as_str()
    });
    let mut runs: Vec<_> = ci
        .iter()
        .filter(|fact| {
            fact.untrusted.is_some()
                && fact.repository == change.repository
                && fact.revision == commit
        })
        .collect();
    if runs.is_empty() {
        return Ok(());
    }
    runs.sort_by(|a, b| b.ordinal.cmp(&a.ordinal));
    let runs: Vec<Value> = runs
        .into_iter()
        .take(MAX_CHECKLIST_UNTRUSTED_RUNS)
        .map(|fact| {
            json!({
                "run_id": fact.run_id,
                "project": fact.project,
                "revision": fact.revision,
                "success": fact.success,
                "terminal": fact.terminal,
                "satisfies_required_ci": fact.satisfies_required(),
                "untrusted": fact.untrusted,
            })
        })
        .collect();
    if let Some(item) = checklist
        .get_mut("items")
        .and_then(Value::as_array_mut)
        .and_then(|items| {
            items
                .iter_mut()
                .find(|item| item.get("key") == Some(&json!("ci")))
        })
        .and_then(Value::as_object_mut)
    {
        item.insert("untrusted_runs".into(), Value::Array(runs));
    }
    Ok(())
}
