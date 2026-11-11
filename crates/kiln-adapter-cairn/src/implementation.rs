//! Read-only intake for Cairn's implementation-ready projection.
//!
//! The snapshot supplies observed intent and eligibility only. Kiln still
//! admits the resulting work through its own contract, enrollment and ledger.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::implementation::{
    AcceptanceObligation, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, MAX_MINUTES_PER_ATTEMPT,
    SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef};
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAIRN_IMPLEMENTATION_SCHEMA: &str = "cairn.implementation-ready.v1";
const MAXIMUM_SNAPSHOT_BYTES: usize = 4_194_304;
const MAXIMUM_SELECTION: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnImplementationPolicy {
    pub repository: String,
    pub project: String,
    pub actor: String,
    pub requested_outcome: String,
    pub acceptance: Vec<AcceptanceObligation>,
    pub permitted_paths: Vec<String>,
    pub permitted_effects: BTreeSet<String>,
    pub verification_profile: ContentId,
    pub budget: WorkBudget,
    pub terminal_policy: TerminalPolicy,
    pub priority: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CairnSnapshotAdmission {
    pub outcome: String,
    pub works: Vec<ImplementationWork>,
}

/// Admit one Cairn snapshot at an externally supplied monotonic sequence.
///
/// Only `ready` selections become work. `empty`, `blocked`, and
/// `completion_only` produce no work. `incomplete_limit`, a non-complete
/// snapshot, an unknown schema, or inconsistent/forged selection facts fail.
// r[impl kiln.implementation.contract] r[impl kiln.implementation.cairn_intake]
pub fn admit_cairn_snapshot(
    policy: &CairnImplementationPolicy,
    sequence: u64,
    bytes: &[u8],
) -> PortResult<CairnSnapshotAdmission> {
    admit_policy(policy)?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_SNAPSHOT_BYTES {
        return Err(refusal(
            "cairn_snapshot_bound",
            "snapshot",
            "Cairn snapshot is empty or exceeds its byte bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| {
        refusal(
            "cairn_snapshot_json",
            "snapshot",
            "Cairn snapshot is not valid JSON",
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        refusal(
            "cairn_snapshot_shape",
            "snapshot",
            "Cairn snapshot must be an object",
        )
    })?;
    if object.get("schema").and_then(Value::as_str) != Some(CAIRN_IMPLEMENTATION_SCHEMA) {
        return Err(refusal(
            "cairn_snapshot_schema",
            "snapshot.schema",
            "Cairn snapshot schema is unsupported",
        ));
    }
    if object.get("complete").and_then(Value::as_bool) != Some(true) {
        return Err(refusal(
            "cairn_snapshot_incomplete",
            "snapshot.complete",
            "Cairn snapshot is incomplete",
        ));
    }
    let outcome = object
        .get("outcome")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            refusal(
                "cairn_snapshot_outcome",
                "snapshot.outcome",
                "Cairn snapshot outcome is missing",
            )
        })?;
    if outcome == "incomplete_limit" {
        return Err(refusal(
            "cairn_snapshot_incomplete",
            "snapshot.outcome",
            "Cairn snapshot enumeration was bounded before completion",
        ));
    }
    if !matches!(outcome, "ready" | "blocked" | "empty" | "completion_only") {
        return Err(refusal(
            "cairn_snapshot_outcome",
            "snapshot.outcome",
            "Cairn snapshot outcome is unsupported",
        ));
    }
    let changes = object
        .get("changes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            refusal(
                "cairn_snapshot_changes",
                "snapshot.changes",
                "Cairn snapshot changes are missing",
            )
        })?;
    let selection = object
        .get("selection")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            refusal(
                "cairn_snapshot_selection",
                "snapshot.selection",
                "Cairn snapshot selection is missing",
            )
        })?;
    if selection.len() > MAXIMUM_SELECTION {
        return Err(refusal(
            "cairn_snapshot_selection",
            "snapshot.selection",
            "Cairn snapshot selection exceeds its bound",
        ));
    }
    if outcome == "ready" && selection.is_empty() {
        return Err(refusal(
            "cairn_snapshot_selection",
            "snapshot.selection",
            "ready Cairn snapshot has no selected unit",
        ));
    }
    if outcome != "ready" && !selection.is_empty() {
        return Err(refusal(
            "cairn_snapshot_selection",
            "snapshot.selection",
            "non-ready Cairn snapshot selected work",
        ));
    }
    let mut change_index: BTreeMap<&str, &Value> = BTreeMap::new();
    for change in changes {
        let name = change
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_selection("Cairn change name is missing"))?;
        if !identifier(name, DEFAULT_MAX_ID_BYTES) || change_index.insert(name, change).is_some() {
            return Err(invalid_selection(
                "Cairn change name is malformed or duplicated",
            ));
        }
        let mut units = BTreeSet::new();
        for unit in change
            .get("units")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = unit
                .get("unit")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_selection("Cairn unit name is missing"))?;
            if !valid_unit(name) || !units.insert(name) {
                return Err(invalid_selection(
                    "Cairn unit name is malformed or duplicated",
                ));
            }
        }
    }
    let mut works = Vec::with_capacity(selection.len());
    for selected in selection {
        works.push(selected_work(policy, sequence, selected, &change_index)?);
    }
    Ok(CairnSnapshotAdmission {
        outcome: outcome.to_string(),
        works,
    })
}

fn selected_work(
    policy: &CairnImplementationPolicy,
    sequence: u64,
    selected: &Value,
    changes: &BTreeMap<&str, &Value>,
) -> PortResult<ImplementationWork> {
    let change = selected
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_selection("selected change is missing"))?;
    let unit = selected
        .get("unit")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_selection("selected unit is missing"))?;
    if !valid_unit(unit) {
        return Err(invalid_selection("selected unit name is malformed"));
    }
    let digest = selected
        .get("snapshot_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_selection("selected digest is missing"))?;
    if selected.get("archive_ready").and_then(Value::as_bool) != Some(false) {
        return Err(invalid_selection(
            "selected unit is already archive-ready or its readiness is missing",
        ));
    }
    let record = changes
        .get(change)
        .ok_or_else(|| invalid_selection("selected change is absent from changes"))?;
    if record.get("snapshot_digest").and_then(Value::as_str) != Some(digest)
        || record.get("implementation_ready").and_then(Value::as_bool) != Some(true)
        || record
            .get("blockers")
            .and_then(Value::as_array)
            .is_some_and(|blockers| !blockers.is_empty())
    {
        return Err(invalid_selection(
            "selected change facts are inconsistent with the selection",
        ));
    }
    for gate in ["proposal", "design", "tasks"] {
        if record
            .get("gates")
            .and_then(|gates| gates.get(gate))
            .and_then(|gate| gate.get("valid"))
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(invalid_selection("selected change has an invalid gate"));
        }
    }
    let unit_record = record
        .get("units")
        .and_then(Value::as_array)
        .and_then(|units| {
            units
                .iter()
                .find(|candidate| candidate.get("unit").and_then(Value::as_str) == Some(unit))
        })
        .ok_or_else(|| invalid_selection("selected unit is absent from the change"))?;
    if selected.get("requirement_references") != unit_record.get("requirement_references") {
        return Err(invalid_selection(
            "selected requirement references differ from the change",
        ));
    }
    let mut acceptance = policy.acceptance.clone();
    let mut obligation_ids: BTreeSet<String> = acceptance
        .iter()
        .map(|obligation| obligation.id.clone())
        .collect();
    for reference in unit_record
        .get("requirement_references")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let Some(reference) = reference.as_str() else {
            return Err(invalid_selection("requirement reference is not a string"));
        };
        if obligation_ids.insert(reference.to_string()) {
            acceptance.push(AcceptanceObligation {
                id: reference.to_string(),
                statement: format!("Cairn requirement {reference} is satisfied"),
            });
        }
    }
    let source_version = ContentId::from_blake3_hex(digest)
        .map_err(|_| invalid_selection("selected snapshot digest is not a BLAKE3 identity"))?;
    let native_id = format!("{change}/{unit}");
    let alias = SourceRef {
        kind: WorkSourceKind::CairnTask,
        source: ExternalRef::new("cairn", "task", &native_id)
            .map_err(|_| invalid_selection("selected unit identity is invalid"))?,
        native_id: native_id.clone(),
        version: source_version,
    };
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: sequence,
        actor: policy.actor.clone(),
        repository: policy.repository.clone(),
        project: policy.project.clone(),
        base: None,
        title: format!("{change}: {unit}"),
        requested_outcome: policy.requested_outcome.clone(),
        acceptance,
        dependencies: Vec::new(),
        permitted_paths: policy.permitted_paths.clone(),
        permitted_effects: policy.permitted_effects.clone(),
        verification_profile: policy.verification_profile.clone(),
        budget: policy.budget,
        terminal_policy: policy.terminal_policy,
        priority: policy.priority,
        aliases: vec![alias],
    })
}

fn admit_policy(policy: &CairnImplementationPolicy) -> PortResult<()> {
    if !identifier(&policy.repository, 256)
        || !identifier(&policy.project, 100)
        || !identifier(&policy.actor, DEFAULT_MAX_ID_BYTES)
        || policy.requested_outcome.trim().is_empty()
        || policy.requested_outcome.len() > 8_192
        || policy.acceptance.is_empty()
        || policy.acceptance.len() > 64
        || policy.permitted_paths.len() > 64
        || policy.permitted_effects.len() > 16
        || policy.budget.attempts == 0
        || policy.budget.attempts > 10
        || policy.budget.minutes_per_attempt == 0
        || policy.budget.minutes_per_attempt > MAX_MINUTES_PER_ATTEMPT
    {
        return Err(refusal(
            "cairn_implementation_policy",
            "policy",
            "Cairn implementation policy is incomplete or exceeds a bound",
        ));
    }
    let mut ids = BTreeSet::new();
    for obligation in &policy.acceptance {
        if !identifier(&obligation.id, 128)
            || obligation.statement.trim().is_empty()
            || obligation.statement.len() > 4_096
            || !ids.insert(&obligation.id)
        {
            return Err(refusal(
                "cairn_implementation_acceptance",
                "policy.acceptance",
                "Cairn acceptance obligations are malformed or duplicated",
            ));
        }
    }
    for path in &policy.permitted_paths {
        if path.starts_with('/')
            || path.contains('\\')
            || path.chars().any(char::is_control)
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(refusal(
                "cairn_implementation_path",
                "policy.permitted_paths",
                "Cairn permitted path is not repository-relative",
            ));
        }
    }
    Ok(())
}

fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn valid_unit(value: &str) -> bool {
    value
        .strip_prefix("task:")
        .or_else(|| value.strip_prefix("change:"))
        .is_some_and(|name| identifier(name, DEFAULT_MAX_ID_BYTES))
}

fn invalid_selection(message: &str) -> PortFailure {
    refusal("cairn_snapshot_selection", "snapshot.selection", message)
}

fn refusal(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> CairnImplementationPolicy {
        CairnImplementationPolicy {
            repository: "onix/cairn".to_string(),
            project: "cairn".to_string(),
            actor: "operator".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            permitted_paths: vec!["crates/cairn-cli".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: ContentId::from_blake3_hex("b".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 5,
        }
    }

    fn digest() -> String {
        "a".repeat(64)
    }

    fn change(name: &str, digest: &str, unit: &str, reference: &str) -> Value {
        json!({
            "name": name,
            "path": format!(".cairn/changes/{name}"),
            "archive_ready": false,
            "implementation_ready": true,
            "completion_only": false,
            "task_counts": {"total": 1, "done": 0, "in_progress": 0, "todo": 1, "unmarked": 0},
            "blockers": [],
            "units": [{"unit": unit, "requirement_references": [reference]}],
            "gates": {
                "proposal": {"valid": true},
                "design": {"valid": true},
                "tasks": {"valid": true}
            },
            "workflow_profile": {"workflow_profile": "spec-driven"},
            "snapshot_digest": digest
        })
    }

    fn snapshot(outcome: &str, selection: Value, changes: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema": CAIRN_IMPLEMENTATION_SCHEMA,
            "command": "cairn implementation-ready",
            "outcome": outcome,
            "complete": true,
            "selection": selection,
            "changes": changes,
            "receipt_hash": "c".repeat(64)
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.implementation.cairn_intake] r[verify kiln.implementation.cairn_intake.scenario.ready]
    #[test]
    fn ready_selection_becomes_versioned_work() {
        let digest = digest();
        let bytes = snapshot(
            "ready",
            json!([{
                "change": "demo",
                "unit": "task:build",
                "requirement_references": ["demo.behavior"],
                "snapshot_digest": digest,
                "archive_ready": false
            }]),
            json!([change("demo", &digest, "task:build", "demo.behavior")]),
        );
        let admission = admit_cairn_snapshot(&policy(), 7, &bytes)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(admission.outcome, "ready");
        let work = admission.works.first().unwrap_or_else(|| unreachable!());
        assert_eq!(work.revision, 7);
        assert_eq!(work.aliases.len(), 1);
        let alias = work.aliases.first().unwrap_or_else(|| unreachable!());
        assert_eq!(alias.kind, WorkSourceKind::CairnTask);
        assert_eq!(alias.native_id, "demo/task:build");
        assert_eq!(alias.version.as_str(), digest);
        assert_eq!(work.title, "demo: task:build");
        assert!(
            work.acceptance
                .iter()
                .any(|obligation| obligation.id == "demo.behavior")
        );
        assert!(work.identity().is_ok());
    }

    // r[verify kiln.implementation.cairn_intake.scenario.outcome]
    #[test]
    fn non_ready_outcomes_produce_no_work() {
        for outcome in ["blocked", "empty", "completion_only"] {
            let bytes = snapshot(outcome, json!([]), json!([]));
            let admission = admit_cairn_snapshot(&policy(), 1, &bytes)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            assert_eq!(admission.outcome, outcome);
            assert!(admission.works.is_empty());
        }
    }

    // r[verify kiln.implementation.cairn_intake.scenario.refused]
    #[test]
    fn incomplete_and_forged_snapshots_fail() {
        let digest = digest();
        let forged = snapshot(
            "ready",
            json!([{
                "change": "demo",
                "unit": "task:build",
                "requirement_references": ["demo.behavior"],
                "snapshot_digest": "d".repeat(64),
                "archive_ready": false
            }]),
            json!([change("demo", &digest, "task:build", "demo.behavior")]),
        );
        assert!(admit_cairn_snapshot(&policy(), 1, &forged).is_err());

        let archive_ready = snapshot(
            "ready",
            json!([{
                "change": "demo",
                "unit": "task:build",
                "requirement_references": ["demo.behavior"],
                "snapshot_digest": digest,
                "archive_ready": true
            }]),
            json!([change("demo", &digest, "task:build", "demo.behavior")]),
        );
        assert!(admit_cairn_snapshot(&policy(), 1, &archive_ready).is_err());

        let mut incomplete: Value = serde_json::from_slice(&snapshot(
            "ready",
            json!([{
                "change": "demo",
                "unit": "task:build",
                "requirement_references": ["demo.behavior"],
                "snapshot_digest": digest,
                "archive_ready": false
            }]),
            json!([change("demo", &digest, "task:build", "demo.behavior")]),
        ))
        .unwrap_or_else(|_| unreachable!());
        incomplete
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert("outcome".to_string(), json!("incomplete_limit"));
        let bytes = serde_json::to_vec(&incomplete).unwrap_or_else(|_| unreachable!());
        assert!(admit_cairn_snapshot(&policy(), 1, &bytes).is_err());

        let mut not_complete: Value =
            serde_json::from_slice(&snapshot("empty", json!([]), json!([])))
                .unwrap_or_else(|_| unreachable!());
        not_complete
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert("complete".to_string(), json!(false));
        let bytes = serde_json::to_vec(&not_complete).unwrap_or_else(|_| unreachable!());
        assert!(admit_cairn_snapshot(&policy(), 1, &bytes).is_err());
    }

    // r[verify kiln.implementation.cairn_intake.scenario.policy]
    // r[verify kiln.implementation.cairn_intake.scenario.refused]
    #[test]
    fn malformed_changes_units_and_policy_budgets_fail() {
        let digest = digest();
        let selected = json!([{
            "change": "demo",
            "unit": "task:build",
            "requirement_references": ["demo.behavior"],
            "snapshot_digest": digest,
            "archive_ready": false
        }]);
        let duplicate_changes = snapshot(
            "ready",
            selected.clone(),
            json!([
                change("demo", &digest, "task:build", "demo.behavior"),
                change("demo", &digest, "task:build", "demo.behavior")
            ]),
        );
        assert!(admit_cairn_snapshot(&policy(), 1, &duplicate_changes).is_err());

        let unknown_unit = snapshot(
            "ready",
            json!([{
                "change": "demo",
                "unit": "bogus",
                "requirement_references": ["demo.behavior"],
                "snapshot_digest": digest,
                "archive_ready": false
            }]),
            json!([change("demo", &digest, "bogus", "demo.behavior")]),
        );
        assert!(admit_cairn_snapshot(&policy(), 1, &unknown_unit).is_err());

        let mut oversized = policy();
        oversized.budget.minutes_per_attempt = MAX_MINUTES_PER_ATTEMPT + 1;
        assert!(
            admit_cairn_snapshot(&oversized, 1, &snapshot("empty", json!([]), json!([]))).is_err()
        );
    }

    #[test]
    fn invalid_policy_is_refused() {
        let mut policy = policy();
        policy.permitted_paths = vec!["/etc".to_string()];
        let bytes = snapshot("empty", json!([]), json!([]));
        assert!(admit_cairn_snapshot(&policy, 1, &bytes).is_err());
    }
}
