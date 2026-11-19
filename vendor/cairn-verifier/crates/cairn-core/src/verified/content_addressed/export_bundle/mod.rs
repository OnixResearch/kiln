// r[impl cairn.nominal_evidence_identities.receipt_chains]

mod caveats;
mod common;
mod compiler_ir;
mod constants;
mod entropy;
mod hash;
mod manifest;
mod objects;
mod pointers;
mod replay;
mod schema;
mod types;

pub use constants::EXPORT_BUNDLE_SCHEMA_V1;
pub use hash::{export_bundle_hash, export_bundle_hash_material};
pub use types::{ExportBundleIssue, ExportBundleReport};

use caveats::export_bundle_caveat_issues;
use compiler_ir::export_bundle_compiler_ir_issues;
use constants::EXPORT_BUNDLE_ISSUE_CAPACITY;
use entropy::export_bundle_entropy_issues;
use hash::export_bundle_hash_issues;
use manifest::export_bundle_manifest_issues;
use objects::export_bundle_object_issues;
use pointers::export_bundle_pointer_issues;
use replay::export_bundle_replay_issues;
use schema::export_bundle_required_issues;

// Active Cairn refs: content_addressed_rust_evidence.export_bundle,
// content_addressed_rust_evidence.export_bundle.replay,
// content_addressed_rust_evidence.export_bundle.mutation_manifest,
// content_addressed_rust_evidence.compiler_ir.
// r[impl cli_maintainability.export_bundle_core_modules.focused_modules]
// r[impl cli_maintainability.export_bundle_core_modules.focused_modules.visible_boundaries]
// r[impl cli_maintainability.export_bundle_core_modules.focused_modules.narrow_bridges]
// r[impl cli_maintainability.export_bundle_core_modules.behavior_preserved]
pub fn validate_export_bundle_receipt(receipt: &serde_json::Value) -> ExportBundleReport {
    let computed_bundle_hash = export_bundle_hash(receipt);
    let issues = export_bundle_issues(receipt, &computed_bundle_hash);
    let is_valid = issues.is_empty();
    ExportBundleReport {
        valid: is_valid,
        verdict: if is_valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        computed_bundle_hash,
    }
}

fn export_bundle_issues(
    receipt: &serde_json::Value,
    computed_bundle_hash: &str,
) -> Vec<ExportBundleIssue> {
    let Some(object) = receipt.as_object() else {
        return vec![common::bundle_issue(
            "export_bundle.not_object",
            "export bundle receipt must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(EXPORT_BUNDLE_ISSUE_CAPACITY);
    issues.extend(export_bundle_required_issues(object));
    issues.extend(export_bundle_hash_issues(object, computed_bundle_hash));
    let (object_hashes, object_issues) = export_bundle_object_issues(object);
    issues.extend(object_issues);
    issues.extend(export_bundle_pointer_issues(object, &object_hashes));
    issues.extend(export_bundle_replay_issues(object));
    issues.extend(export_bundle_compiler_ir_issues(object));
    issues.extend(export_bundle_manifest_issues(object));
    issues.extend(export_bundle_caveat_issues(object));
    issues.extend(export_bundle_entropy_issues(
        "$",
        &export_bundle_hash_material(receipt),
    ));
    issues
}
