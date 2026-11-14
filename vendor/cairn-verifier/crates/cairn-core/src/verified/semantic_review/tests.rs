use serde_json::json;

use super::*;

fn hash() -> String {
    "a".repeat(64)
}

#[test]
fn approved_review_receipt_can_have_no_findings() {
    // r[verify semantic_review.split_semantic_review_core.behavior_preserved]
    // r[verify semantic_review.split_semantic_review_core.behavior_preserved.focused_checks]
    // r[verify semantic_review.split_semantic_review_core.behavior_preserved.repository_checks]
    let value = json!({
        "schema_version": REVIEW_RECEIPT_SCHEMA_V1,
        "review_kind": "plan",
        "decision": "approved",
        "reviewed_inputs": [{"path": "proposal.md", "hash": hash()}],
        "reviewer": {"source": "manual", "id": "reviewer"},
        "findings": []
    });
    let report = validate_review_receipt(&value, &[("proposal.md".to_string(), hash())]);
    assert!(report.valid, "{:?}", report.issues);
}

#[test]
fn non_approved_review_requires_categorized_findings() {
    let value = json!({
        "schema_version": REVIEW_RECEIPT_SCHEMA_V1,
        "review_kind": "design",
        "decision": "needs_revision",
        "reviewed_inputs": [{"path": "design.md", "hash": hash()}],
        "reviewer": {"source": "agentic", "id": "gemini"},
        "findings": []
    });
    let report = validate_review_receipt(&value, &[]);
    assert!(!report.valid);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "missing_findings"));
}

#[test]
fn completeness_receipt_reports_missing_links() {
    // r[verify cairn.semantic_review.completeness_check_rail.mapping]
    // r[verify cairn.semantic_review.completeness_check_rail.mapping.mapping.active_change]
    // r[verify cairn.semantic_review.completeness_check_rail.mapping.mapping.accepted_spec]
    // r[verify cairn.semantic_review.completeness_check_rail.omissions]
    // r[verify cairn.semantic_review.completeness_check_rail.omissions.omissions.missing_task]
    // r[verify cairn.semantic_review.completeness_check_rail.omissions.omissions.missing_verification]
    let receipt = completeness_receipt(
        vec!["cairn.example.requirement".to_string()],
        vec![],
        vec!["cairn.example.requirement".to_string()],
        vec![],
    );
    assert_eq!(receipt["valid"], false);
    let findings = receipt["findings"].as_array().unwrap();
    assert!(findings
        .iter()
        .any(|finding| finding["code"] == "missing_task_coverage"));
    assert!(findings
        .iter()
        .any(|finding| finding["code"] == "missing_verification_coverage"));
}

#[test]
fn review_metrics_regression_split_preserves_public_api_behavior() {
    // r[verify cairn.semantic_review.metrics_regression_split.boundaries]
    // r[verify cairn.semantic_review.metrics_regression_split.public_api]
    // r[verify cairn.semantic_review.metrics_regression_split.focused_helpers]
    // r[verify cairn.semantic_review.metrics_regression_split.behavior_preserved]
    // r[verify cairn.semantic_review.metrics_regression_split.focused_verification]
    // r[verify cairn.semantic_review.metrics_regression_split.repository_verification]
    let registry = review_metrics_registry_fixture();
    let report = validate_review_metrics_registry(&registry);
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.verdict, "PASS");
    assert!(report.receipt_hash.len() == 64);

    let coverage = json!({
        "schema_version": REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
        "fixtures": [
            {"path": "fixtures/positive-auto-fix", "valid": true, "diagnostics": []},
            {"path": "fixtures/negative-auto-fix", "valid": false, "diagnostics": ["missing-auto-fix-task"]}
        ]
    });
    let fixture_report = validate_review_metrics_fixture_pairs(&registry, &coverage);
    assert!(fixture_report.valid, "{:?}", fixture_report.issues);
}

#[test]
fn review_metrics_registry_accepts_complete_category_binding() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.registry_binds_surfaces]
    let registry = json!({
        "schema_version": REVIEW_METRICS_REGRESSION_SCHEMA_V1,
        "categories": [{
            "code": "tasks-auto-fix",
            "metrics_class": "omission",
            "metrics_stage": "tasks",
            "metrics_source": "agentic",
            "diagnostic_code": "missing-auto-fix-task",
            "requirement_id": "project.review.auto_fix",
            "guidance_anchor": "# auto-fix guidance",
            "positive_fixture": "fixtures/positive-auto-fix",
            "negative_fixture": "fixtures/negative-auto-fix",
            "evidence_ref": "evidence/metrics-snapshot.md"
        }]
    });
    let report = validate_review_metrics_registry(&registry);
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.verdict, "PASS");
}

#[test]
fn review_metrics_registry_rejects_missing_and_duplicate_bindings() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.registry_binds_surfaces]
    let registry = json!({
        "schema_version": REVIEW_METRICS_REGRESSION_SCHEMA_V1,
        "categories": [
            {
                "code": "tasks-auto-fix",
                "metrics_class": "unknown",
                "metrics_stage": "tasks",
                "metrics_source": "agentic",
                "diagnostic_code": "missing-auto-fix-task",
                "requirement_id": "project.review.auto_fix",
                "guidance_anchor": "# auto-fix guidance",
                "positive_fixture": "fixtures/positive-auto-fix",
                "negative_fixture": "fixtures/negative-auto-fix",
                "evidence_ref": "evidence/metrics-snapshot.md"
            },
            {
                "code": "tasks-auto-fix",
                "metrics_class": "omission",
                "metrics_stage": "tasks",
                "metrics_source": "agentic",
                "diagnostic_code": "missing-auto-fix-task",
                "requirement_id": "project.review.auto_fix",
                "guidance_anchor": "",
                "positive_fixture": "fixtures/positive-auto-fix-2",
                "negative_fixture": "fixtures/negative-auto-fix-2",
                "evidence_ref": "evidence/metrics-snapshot.md"
            }
        ]
    });
    let report = validate_review_metrics_registry(&registry);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"invalid_registry_binding"));
    assert!(codes.contains(&"missing_registry_binding"));
    assert!(codes.contains(&"duplicate_registry_binding"));
}

#[test]
fn review_metrics_fixture_pairs_accept_expected_positive_and_negative_results() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.fixture_pair_enforcement]
    let registry = review_metrics_registry_fixture();
    let coverage = json!({
        "schema_version": REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
        "fixtures": [
            {
                "path": "fixtures/positive-auto-fix",
                "valid": true,
                "diagnostics": []
            },
            {
                "path": "fixtures/negative-auto-fix",
                "valid": false,
                "diagnostics": ["missing-auto-fix-task"]
            }
        ]
    });
    let report = validate_review_metrics_fixture_pairs(&registry, &coverage);
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.verdict, "PASS");
}

#[test]
fn review_metrics_fixture_pairs_reject_drifted_fixture_results() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.fixture_pair_enforcement]
    let registry = review_metrics_registry_fixture();
    let coverage = json!({
        "schema_version": REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
        "fixtures": [
            {
                "path": "fixtures/positive-auto-fix",
                "valid": false,
                "diagnostics": ["unexpected-positive-failure"]
            },
            {
                "path": "fixtures/negative-auto-fix",
                "valid": false,
                "diagnostics": ["wrong-diagnostic"]
            }
        ]
    });
    let report = validate_review_metrics_fixture_pairs(&registry, &coverage);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"positive_fixture_failed"));
    assert!(codes.contains(&"negative_fixture_missing_diagnostic"));
}

#[test]
fn review_metrics_fixture_pairs_reject_missing_and_passing_negative_results() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.fixture_pair_enforcement]
    let registry = review_metrics_registry_fixture();
    let coverage = json!({
        "schema_version": REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
        "fixtures": [{
            "path": "fixtures/negative-auto-fix",
            "valid": true,
            "diagnostics": []
        }]
    });
    let report = validate_review_metrics_fixture_pairs(&registry, &coverage);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"missing_fixture_result"));
    assert!(codes.contains(&"negative_fixture_passed"));
}

#[test]
fn review_metrics_guidance_sync_accepts_documented_diagnostic_and_refs() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.guidance_task_sync]
    let registry = review_metrics_registry_fixture();
    let sync = json!({
        "schema_version": REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1,
        "guidance": [{
            "anchor": "# auto-fix guidance",
            "diagnostics": ["missing-auto-fix-task"]
        }],
        "tasks": [{
            "path": ".cairn/changes/example/tasks.md",
            "requirement_ids": ["project.review.auto_fix"]
        }],
        "specs": [{
            "path": ".cairn/changes/example/specs/review/spec.md",
            "requirement_ids": ["project.review.auto_fix"]
        }]
    });
    let report = validate_review_metrics_guidance_sync(&registry, &sync);
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.verdict, "PASS");
}

#[test]
fn review_metrics_guidance_sync_rejects_missing_diagnostic_anchor_and_refs() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.guidance_task_sync]
    let registry = review_metrics_registry_fixture();
    let sync = json!({
        "schema_version": REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1,
        "guidance": [{
            "anchor": "# other guidance",
            "diagnostics": ["other-diagnostic"]
        }],
        "tasks": [{
            "path": ".cairn/changes/example/tasks.md",
            "requirement_ids": ["project.review.other"]
        }],
        "specs": [{
            "path": ".cairn/changes/example/specs/review/spec.md",
            "requirement_ids": ["project.review.other"]
        }]
    });
    let report = validate_review_metrics_guidance_sync(&registry, &sync);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"guidance_missing_diagnostic"));
    assert!(codes.contains(&"guidance_missing_anchor"));
    assert!(codes.contains(&"task_missing_requirement_ref"));
    assert!(codes.contains(&"spec_missing_requirement_ref"));
}

#[test]
fn review_metrics_guidance_sync_rejects_invalid_sync_surfaces() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.guidance_task_sync]
    let registry = review_metrics_registry_fixture();
    let sync = json!({
        "schema_version": REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1,
        "guidance": [{
            "anchor": "",
            "diagnostics": [""]
        }],
        "tasks": [{"path": "tasks.md"}],
        "specs": []
    });
    let report = validate_review_metrics_guidance_sync(&registry, &sync);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"invalid_guidance_sync"));
    assert!(codes.contains(&"missing_spec_sync"));
}

#[test]
fn review_metrics_evidence_boundary_accepts_sanitized_metadata_and_provenance() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.secret_free_evidence]
    let registry = review_metrics_registry_fixture();
    let evidence = json!({
        "schema_version": REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1,
        "evidence": [{
            "ref": "evidence/metrics-snapshot.md",
            "category_code": "tasks-auto-fix",
            "metrics_class": "omission",
            "metrics_stage": "tasks",
            "metrics_source": "agentic",
            "provenance": {
                "source": "review-metrics-summary",
                "generated_by": "cairn-test-fixture",
                "snapshot_hash": hash()
            },
            "summary": {
                "finding_count": 491,
                "sanitization": "aggregated counts only; no transcript bodies"
            }
        }]
    });
    let report = validate_review_metrics_evidence_boundary(&registry, &evidence);
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.verdict, "PASS");
}

#[test]
fn review_metrics_evidence_boundary_rejects_missing_refs_and_metadata_drift() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.secret_free_evidence]
    let registry = review_metrics_registry_fixture();
    let evidence = json!({
        "schema_version": REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1,
        "evidence": [{
            "ref": "evidence/other-snapshot.md",
            "category_code": "other-category",
            "metrics_class": "omission",
            "metrics_stage": "tasks",
            "metrics_source": "agentic",
            "provenance": {"source": "review-metrics-summary"}
        }]
    });
    let report = validate_review_metrics_evidence_boundary(&registry, &evidence);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"missing_evidence_ref"));
}

#[test]
fn review_metrics_evidence_boundary_rejects_secret_bearing_fields_and_text() {
    // r[verify cairn.semantic_review.review_metrics_regression_rail.secret_free_evidence]
    let registry = review_metrics_registry_fixture();
    let evidence = json!({
        "schema_version": REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1,
        "evidence": [{
            "ref": "evidence/metrics-snapshot.md",
            "category_code": "tasks-auto-fix",
            "metrics_class": "omission",
            "metrics_stage": "tasks",
            "metrics_source": "agentic",
            "provenance": {"source": "review-metrics-summary"},
            "account_id": "acct-123",
            "provider_payload": {"request_body": "raw transcript"},
            "summary": "Authorization: Bearer abc and operator@example.com"
        }]
    });
    let report = validate_review_metrics_evidence_boundary(&registry, &evidence);
    assert!(!report.valid);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"unsafe_evidence_field"));
    assert!(codes.contains(&"unsafe_evidence_text"));
}

fn review_metrics_registry_fixture() -> serde_json::Value {
    json!({
        "schema_version": REVIEW_METRICS_REGRESSION_SCHEMA_V1,
        "categories": [{
            "code": "tasks-auto-fix",
            "metrics_class": "omission",
            "metrics_stage": "tasks",
            "metrics_source": "agentic",
            "diagnostic_code": "missing-auto-fix-task",
            "requirement_id": "project.review.auto_fix",
            "guidance_anchor": "# auto-fix guidance",
            "positive_fixture": "fixtures/positive-auto-fix",
            "negative_fixture": "fixtures/negative-auto-fix",
            "evidence_ref": "evidence/metrics-snapshot.md"
        }]
    })
}
