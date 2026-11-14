use serde_json::{json, Value};

use super::common::sorted_unique;
use super::REVIEW_COMPLETENESS_SCHEMA_V1;
use crate::verified::evidence::canonical_json_hash;

pub fn completeness_receipt(
    requirements: Vec<String>,
    tasks: Vec<String>,
    implementation_refs: Vec<String>,
    verification_refs: Vec<String>,
) -> Value {
    // r[impl cairn.semantic_review.completeness_check_rail.mapping]
    // r[impl cairn.semantic_review.completeness_check_rail.mapping.mapping.active_change]
    // r[impl cairn.semantic_review.completeness_check_rail.mapping.mapping.accepted_spec]
    // r[impl cairn.semantic_review.completeness_check_rail.omissions]
    let mut requirements = sorted_unique(requirements);
    let tasks = sorted_unique(tasks);
    let implementation_refs = sorted_unique(implementation_refs);
    let verification_refs = sorted_unique(verification_refs);
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    for requirement in requirements.drain(..) {
        let has_task = tasks.iter().any(|task| task.contains(&requirement));
        let has_impl = implementation_refs
            .iter()
            .any(|reference| reference == &requirement);
        let has_verify = verification_refs
            .iter()
            .any(|reference| reference == &requirement);
        if !has_task {
            // r[impl cairn.semantic_review.completeness_check_rail.omissions.omissions.missing_task]
            findings.push(json!({"id": format!("missing-task-{requirement}"), "category": "omission", "code": "missing_task_coverage", "requirement": requirement}));
        }
        if has_impl && !has_verify {
            // r[impl cairn.semantic_review.completeness_check_rail.omissions.omissions.missing_verification]
            findings.push(json!({"id": format!("missing-verification-{requirement}"), "category": "omission", "code": "missing_verification_coverage", "requirement": requirement}));
        }
        entries.push(json!({
            "requirement": requirement,
            "has_task": has_task,
            "has_implementation_ref": has_impl,
            "has_verification_ref": has_verify,
        }));
    }
    let mut receipt = json!({
        "schema_version": REVIEW_COMPLETENESS_SCHEMA_V1,
        "valid": findings.is_empty(),
        "verdict": if findings.is_empty() {"PASS"} else {"FAIL"},
        "requirements": entries,
        "tasks": tasks,
        "implementation_refs": implementation_refs,
        "verification_refs": verification_refs,
        "findings": findings,
    });
    let hash = canonical_json_hash(&receipt);
    receipt["receipt_hash"] = json!(hash);
    receipt
}
