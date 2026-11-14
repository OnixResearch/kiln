use serde_json::{Map, Value};

use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_compiler_ir_issues(
    object: &Map<String, Value>,
) -> Vec<ExportBundleIssue> {
    let Some(compiler_ir) = object.get("compiler_ir").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "compiler_ir must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(6);
    issues.extend(export_bundle_hir_issues(compiler_ir));
    issues.extend(export_bundle_mir_issues(compiler_ir));
    issues
}

fn export_bundle_hir_issues(compiler_ir: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(hir) = compiler_ir.get("hir").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "compiler_ir.hir must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(3);
    for field in ["rustc", "extractor", "dependency_status"] {
        if hir
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            issues.push(bundle_issue(
                "export_bundle.missing_compiler_ir_field",
                format!("compiler_ir.hir.{field} must be a non-empty string"),
            ));
        }
    }
    if hir.get("dependency_status").and_then(Value::as_str) == Some("source-scanner-token-set") {
        issues.push(bundle_issue(
            "export_bundle.hir_dependency_overclaim_missing",
            "HIR evidence must report toolchain-scoped resolved dependency status, not source-token scanner status",
        ));
    }
    issues
}

fn export_bundle_mir_issues(compiler_ir: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(mir) = compiler_ir.get("mir").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "compiler_ir.mir must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(4);
    for field in ["rustc", "extractor", "summary_status"] {
        if mir
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            issues.push(bundle_issue(
                "export_bundle.missing_compiler_ir_field",
                format!("compiler_ir.mir.{field} must be a non-empty string"),
            ));
        }
    }
    if mir.get("stability_claim").and_then(Value::as_str) != Some("toolchain-coupled") {
        issues.push(bundle_issue(
            "export_bundle.mir_stability_overclaim",
            "MIR summaries must declare toolchain-coupled stability",
        ));
    }
    issues
}
