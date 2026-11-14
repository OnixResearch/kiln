use serde_json::{Map, Value};

use super::super::common::NO_CORRECTNESS_CLAIM;
use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_caveat_issues(object: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(caveats) = object.get("caveats").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "caveats must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(3);
    if caveats.get("correctness_claim").and_then(Value::as_str) != Some(NO_CORRECTNESS_CLAIM) {
        issues.push(bundle_issue(
            "export_bundle.correctness_overclaim",
            "export-bundle hash agreement is replay/provenance evidence, not a correctness proof",
        ));
    }
    if caveats.get("compiler_ir_status").and_then(Value::as_str) == Some("stable-semantic-identity")
    {
        issues.push(bundle_issue(
            "export_bundle.compiler_ir_overclaim",
            "compiler IR evidence must remain toolchain-coupled and cannot claim stable semantic identity",
        ));
    }
    issues
}
