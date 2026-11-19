pub fn completeness_receipt(
    requirements: Vec<String>,
    tasks: Vec<String>,
    implementation_refs: Vec<String>,
    verification_refs: Vec<String>,
) -> serde_json::Value {
    // r[impl cairn.semantic_review.completeness_check_rail.mapping]
    // r[impl cairn.semantic_review.completeness_check_rail.mapping.mapping.active_change]
    // r[impl cairn.semantic_review.completeness_check_rail.mapping.mapping.accepted_spec]
    // r[impl cairn.semantic_review.completeness_check_rail.omissions]
    let mut requirements = super::common::sorted_unique(requirements);
    let tasks = super::common::sorted_unique(tasks);
    let implementation_refs = super::common::sorted_unique(implementation_refs);
    let verification_refs = super::common::sorted_unique(verification_refs);
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
            findings.push(serde_json::json!({"id": format!("missing-task-{requirement}"), "category": "omission", "code": "missing_task_coverage", "requirement": requirement}));
        }
        if has_impl && !has_verify {
            // r[impl cairn.semantic_review.completeness_check_rail.omissions.omissions.missing_verification]
            findings.push(serde_json::json!({"id": format!("missing-verification-{requirement}"), "category": "omission", "code": "missing_verification_coverage", "requirement": requirement}));
        }
        entries.push(serde_json::json!({
            "requirement": requirement,
            "has_task": has_task,
            "has_implementation_ref": has_impl,
            "has_verification_ref": has_verify,
        }));
    }
    let mut receipt = serde_json::json!({
        "schema_version": super::REVIEW_COMPLETENESS_SCHEMA_V1,
        "valid": findings.is_empty(),
        "verdict": if findings.is_empty() {"PASS"} else {"FAIL"},
        "requirements": entries,
        "tasks": tasks,
        "implementation_refs": implementation_refs,
        "verification_refs": verification_refs,
        "findings": findings,
    });
    let hash = crate::verified::evidence::canonical_json_hash(&receipt);
    receipt["receipt_hash"] = serde_json::json!(hash);
    receipt
}
