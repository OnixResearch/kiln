pub(super) fn export_bundle_caveat_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::ExportBundleIssue> {
    let Some(caveats) = object.get("caveats").and_then(serde_json::Value::as_object) else {
        return vec![super::common::bundle_issue(
            "export_bundle.missing_object",
            "caveats must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(3);
    if caveats
        .get("correctness_claim")
        .and_then(serde_json::Value::as_str)
        != Some(super::super::common::NO_CORRECTNESS_CLAIM)
    {
        issues.push(super::common::bundle_issue(
            "export_bundle.correctness_overclaim",
            "export-bundle hash agreement is replay/provenance evidence, not a correctness proof",
        ));
    }
    if caveats
        .get("compiler_ir_status")
        .and_then(serde_json::Value::as_str)
        == Some("stable-semantic-identity")
    {
        issues.push(super::common::bundle_issue(
            "export_bundle.compiler_ir_overclaim",
            "compiler IR evidence must remain toolchain-coupled and cannot claim stable semantic identity",
        ));
    }
    issues
}
