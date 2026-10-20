use std::collections::BTreeSet;

use serde_json::Value;

// r[verify ucan.aeneas_time_bounds.provider_dependencies]
// r[verify ucan.aeneas_time_bounds.exact_source]
// r[verify ucan.aeneas_time_bounds.translation_manifest]
// r[verify ucan.aeneas_time_bounds.feasibility]
// r[verify ucan.aeneas_time_bounds.properties]
// r[verify ucan.aeneas_time_bounds.theorem_adequacy]
// r[verify ucan.aeneas_time_bounds.generated_boundary]
// r[verify ucan.aeneas_time_bounds.evidence_chain]
// r[verify ucan.aeneas_time_bounds.negative_evidence]
// r[verify ucan.aeneas_time_bounds.resource_budget]
// r[verify ucan.aeneas_time_bounds.runtime_boundary]
// r[verify ucan.aeneas_time_bounds.claim_boundary]
// r[verify ucan.aeneas_time_bounds.final_validation]
const MANIFEST_SCHEMA: &str = "ucan-aeneas-time-bounds-feasibility/v1";
const RECEIPT_SCHEMA: &str = "ucan-aeneas-time-bounds-blocked-receipt/v1";
const PILOT_ID: &str = "ucan-core.time-bounds.aeneas-pilot";
const BLOCKED_STATUS: &str = "blocked";
const SOURCE_PATH: &str = "crates/ucan-core/src/token.rs";
const PROVIDER_SOURCE_PATH: &str = "proofs/aeneas-time-bounds/provider-identity.ncl";
const DIAGNOSTIC_PATH: &str = "proofs/aeneas-time-bounds/evidence/exact-item-selection.log";
const SUPPLEMENTARY_PATH: &str = "proofs/aeneas-time-bounds/evidence/broad-public-probe.log";
const OCTET_REVISION: &str = "ef18352bf579bb7c6eb39070d917e3d005385a61";
const OCTET_PROFILE_ID: &str = "octet-aeneas-lean-0.1.0-bc2eb6f";
const VALENCE_REVISION: &str = "8eccac35fcd8aae8ef64b3d1021e7f1a53656180";
const VALENCE_PROFILE: &str = "aeneas-lean/v1";
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const TARGET_ENDIANNESS: &str = "little";
const TARGET_POINTER_WIDTH_BITS: u64 = 64;
const EXPECTED_ITEM_COUNT: usize = 3;
const MINIMUM_NEGATIVE_CASE_COUNT: usize = 26;
const EXACT_DIAGNOSTIC_CLASS: &str = "charon-start-from-inherent-impl-unsupported";
const EXACT_STAGE: &str = "charon-item-selection";
const RUN_LOCAL_DEF_ID_POLICY: &str = "run-local-only-not-durable";
const NOT_PRODUCED: &str = "not-produced";
const NOT_RUN: &str = "not-run";
const NETWORK_ISOLATION: &str = "bubblewrap-unshare-net";

const EXPECTED_ITEMS: [&str; EXPECTED_ITEM_COUNT] = [
    "ucan_core::token::{TimeBounds}::new",
    "ucan_core::token::{TimeBounds}::not_before_seconds",
    "ucan_core::token::{TimeBounds}::expires_at_seconds",
];

const EXPECTED_START_ROOTS: [&str; EXPECTED_ITEM_COUNT] = [
    "{impl ucan_core::token::TimeBounds}::new",
    "{impl ucan_core::token::TimeBounds}::not_before_seconds",
    "{impl ucan_core::token::TimeBounds}::expires_at_seconds",
];

const REQUIRED_NEGATIVE_CASES: [&str; MINIMUM_NEGATIVE_CASE_COUNT] = [
    "wrong-provider-identity",
    "wrong-extraction-identity",
    "wrong-profile-identity",
    "source-mismatch",
    "shadow-source",
    "has-errors",
    "skip-borrowck",
    "target-drift",
    "stale-manifest",
    "cross-run-def-id",
    "tool-failure",
    "budget-exhaustion",
    "unsupported-const-fn",
    "unsupported-result",
    "unsupported-error-representation",
    "stale-llbc",
    "stale-translation-manifest",
    "stale-generated-lean",
    "wrong-theorem-identity",
    "wrong-requirement-or-claim",
    "undeclared-trust-input",
    "swapped-receipt",
    "absent-artifact-verification",
    "false-theorem",
    "vacuous-theorem",
    "overclaim-text",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub source_text: String,
    pub source_blake3: String,
    pub provider_source_blake3: String,
    pub manifest_source_blake3: String,
    pub diagnostic_blake3: String,
    pub supplementary_blake3: String,
    pub drift_source_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issue {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

impl Issue {
    fn new(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[must_use]
pub fn validate(manifest: &Value, receipt: &Value, observation: &Observation) -> Vec<Issue> {
    let mut issues = Vec::new();
    require_string(manifest, "/schema_version", MANIFEST_SCHEMA, "manifest-schema", &mut issues);
    require_string(manifest, "/pilot_id", PILOT_ID, "pilot-identity", &mut issues);
    require_string(manifest, "/status", BLOCKED_STATUS, "outcome-class", &mut issues);
    validate_source(manifest, observation, &mut issues);
    validate_provider(manifest, observation, &mut issues);
    validate_extraction(manifest, &mut issues);
    validate_blocker(manifest, observation, &mut issues);
    validate_negative_cases(manifest, &mut issues);
    validate_claim_boundary(manifest, &mut issues);
    validate_receipt(receipt, observation, &mut issues);
    issues
}

fn validate_source(manifest: &Value, observation: &Observation, issues: &mut Vec<Issue>) {
    require_string(manifest, "/target/source_path", SOURCE_PATH, "source-path", issues);
    require_string(manifest, "/target/source_blake3", &observation.source_blake3, "source-identity", issues);
    require_bool(manifest, "/target/copied_source_allowed", false, "source-path", issues);
    if observation.source_blake3 == observation.drift_source_blake3 {
        issues.push(Issue::new(
            "shadow-source",
            "proofs/aeneas-time-bounds/fixtures/invalid/source-drift.rs",
            "negative source fixture unexpectedly matches production source",
        ));
    }
    let Some(items) = manifest.pointer("/target/items").and_then(Value::as_array) else {
        issues.push(Issue::new("source-items", "/target/items", "selected item array is missing"));
        return;
    };
    if items.len() != EXPECTED_ITEM_COUNT {
        issues.push(Issue::new(
            "source-items",
            "/target/items",
            format!("expected {EXPECTED_ITEM_COUNT} exact items, observed {}", items.len()),
        ));
        return;
    }
    for (index, (item, expected_path)) in items.iter().zip(EXPECTED_ITEMS).enumerate() {
        validate_item(item, expected_path, index, &observation.source_text, issues);
    }
}

fn validate_item(item: &Value, expected_path: &str, index: usize, source: &str, issues: &mut Vec<Issue>) {
    let item_path = format!("/target/items/{index}");
    let observed_path = item.get("rust_path").and_then(Value::as_str);
    if observed_path != Some(expected_path) {
        issues.push(Issue::new(
            "source-items",
            format!("{item_path}/rust_path"),
            format!("expected {expected_path:?}, observed {observed_path:?}"),
        ));
    }
    let begin = item.get("source_begin_line").and_then(Value::as_u64);
    let end = item.get("source_end_line").and_then(Value::as_u64);
    let markers = item.get("required_markers").and_then(Value::as_array);
    let (Some(begin), Some(end), Some(markers)) = (begin, end, markers) else {
        issues.push(Issue::new("source-span", item_path, "source span or markers are missing"));
        return;
    };
    let Some(span) = source_span(source, begin, end) else {
        issues.push(Issue::new("source-span", item_path, "source span is invalid"));
        return;
    };
    for marker in markers {
        let Some(marker) = marker.as_str() else {
            issues.push(Issue::new("source-marker", &item_path, "source marker is not a string"));
            continue;
        };
        if !span.contains(marker) {
            issues.push(Issue::new(
                "source-marker",
                &item_path,
                format!("source span does not contain marker {marker:?}"),
            ));
        }
    }
}

fn source_span(source: &str, begin: u64, end: u64) -> Option<String> {
    if begin == 0 || end < begin {
        return None;
    }
    let begin_index = usize::try_from(begin.checked_sub(1)?).ok()?;
    let line_count = usize::try_from(end.checked_sub(begin)?.checked_add(1)?).ok()?;
    let lines = source.lines().skip(begin_index).take(line_count).collect::<Vec<_>>();
    if lines.len() != line_count {
        return None;
    }
    Some(lines.join("\n"))
}

fn validate_provider(manifest: &Value, observation: &Observation, issues: &mut Vec<Issue>) {
    require_string(manifest, "/provider_identity_source", PROVIDER_SOURCE_PATH, "provider-identity", issues);
    require_string(
        manifest,
        "/provider_identity_source_blake3",
        &observation.provider_source_blake3,
        "provider-identity",
        issues,
    );
    require_string(manifest, "/provider/octet/revision", OCTET_REVISION, "provider-identity", issues);
    require_string(manifest, "/provider/octet/profile_id", OCTET_PROFILE_ID, "profile-identity", issues);
    require_string(manifest, "/provider/valence/revision", VALENCE_REVISION, "provider-identity", issues);
    require_string(manifest, "/provider/valence/profile", VALENCE_PROFILE, "profile-identity", issues);
}

fn validate_extraction(manifest: &Value, issues: &mut Vec<Issue>) {
    require_bool(manifest, "/extraction/has_errors", false, "extraction-errors", issues);
    require_bool(manifest, "/extraction/skip_borrowck", false, "borrow-check", issues);
    require_bool(manifest, "/extraction/borrow_check_preserved", true, "borrow-check", issues);
    require_string(manifest, "/extraction/target_triple", TARGET_TRIPLE, "target-metadata", issues);
    require_u64(manifest, "/extraction/pointer_width_bits", TARGET_POINTER_WIDTH_BITS, "target-metadata", issues);
    require_string(manifest, "/extraction/endianness", TARGET_ENDIANNESS, "target-metadata", issues);
    require_string(manifest, "/extraction/network_isolation", NETWORK_ISOLATION, "extraction-identity", issues);
    let command_values = manifest.pointer("/extraction/exact_command").and_then(Value::as_array);
    let command = command_values.map(|values| values.iter().filter_map(Value::as_str).collect::<BTreeSet<_>>());
    let Some(command) = command else {
        issues.push(Issue::new("extraction-identity", "/extraction/exact_command", "command is missing"));
        return;
    };
    for root in EXPECTED_START_ROOTS {
        if !command.contains(root) {
            issues.push(Issue::new(
                "extraction-identity",
                "/extraction/exact_command",
                format!("exact start root {root:?} is missing"),
            ));
        }
    }
    require_string(manifest, "/translation_manifest/status", NOT_PRODUCED, "translation-manifest", issues);
    require_string(manifest, "/translation_manifest/def_id_policy", RUN_LOCAL_DEF_ID_POLICY, "run-local-id", issues);
    let rows_are_empty =
        manifest.pointer("/translation_manifest/rows").and_then(Value::as_array).is_some_and(Vec::is_empty);
    if !rows_are_empty {
        issues.push(Issue::new(
            "translation-manifest",
            "/translation_manifest/rows",
            "blocked exact selection must not carry fabricated translation rows",
        ));
    }
}

fn validate_blocker(manifest: &Value, observation: &Observation, issues: &mut Vec<Issue>) {
    require_string(manifest, "/feasibility/outcome", BLOCKED_STATUS, "outcome-class", issues);
    require_string(manifest, "/feasibility/stage", EXACT_STAGE, "outcome-class", issues);
    require_string(manifest, "/feasibility/diagnostic_class", EXACT_DIAGNOSTIC_CLASS, "unsupported-boundary", issues);
    require_string(manifest, "/feasibility/diagnostic_artifact", DIAGNOSTIC_PATH, "diagnostic-identity", issues);
    require_string(
        manifest,
        "/feasibility/diagnostic_blake3",
        &observation.diagnostic_blake3,
        "diagnostic-identity",
        issues,
    );
    require_string(
        manifest,
        "/feasibility/supplementary_probe_artifact",
        SUPPLEMENTARY_PATH,
        "diagnostic-identity",
        issues,
    );
    require_string(
        manifest,
        "/feasibility/supplementary_probe_blake3",
        &observation.supplementary_blake3,
        "diagnostic-identity",
        issues,
    );
    require_string(manifest, "/feasibility/theorem_status", NOT_RUN, "claim-boundary", issues);
    require_string(manifest, "/budgets/outcome", "within-budget", "budget-outcome", issues);
    for path in [
        "/budgets/exact_stage_timeout_ms",
        "/budgets/broad_probe_timeout_ms",
        "/budgets/stdout_max_bytes",
        "/budgets/stderr_max_bytes",
        "/budgets/retained_artifact_max_bytes",
    ] {
        let positive = manifest.pointer(path).and_then(Value::as_u64).is_some_and(|value| value > 0);
        if !positive {
            issues.push(Issue::new("budget-outcome", path, "budget must be a positive integer"));
        }
    }
    let boundary = manifest.pointer("/feasibility/unsupported_boundary").and_then(Value::as_str).unwrap_or_default();
    if !boundary.contains("not evidence") {
        issues.push(Issue::new(
            "unsupported-boundary",
            "/feasibility/unsupported_boundary",
            "blocker must distinguish selection support from Rust construct support",
        ));
    }
}

fn validate_negative_cases(manifest: &Value, issues: &mut Vec<Issue>) {
    let Some(cases) = manifest.pointer("/negative_cases").and_then(Value::as_array) else {
        issues.push(Issue::new("negative-coverage", "/negative_cases", "negative cases are missing"));
        return;
    };
    if cases.len() < MINIMUM_NEGATIVE_CASE_COUNT {
        issues.push(Issue::new(
            "negative-coverage",
            "/negative_cases",
            format!("expected at least {MINIMUM_NEGATIVE_CASE_COUNT} cases, observed {}", cases.len()),
        ));
    }
    let ids = cases.iter().filter_map(|case| case.get("id")).filter_map(Value::as_str).collect::<BTreeSet<_>>();
    for required in REQUIRED_NEGATIVE_CASES {
        if !ids.contains(required) {
            issues.push(Issue::new(
                "negative-coverage",
                "/negative_cases",
                format!("required negative case {required:?} is missing"),
            ));
        }
    }
}

fn validate_claim_boundary(manifest: &Value, issues: &mut Vec<Issue>) {
    for path in [
        "/claims/aeneas_property_proved",
        "/claims/theorem_map_emitted",
        "/claims/octet_property_receipt_emitted",
        "/claims/valence_property_handoff_emitted",
        "/runtime_boundary/ucan_core_dependency_added",
        "/runtime_boundary/proof_tool_runtime_discovery",
        "/runtime_boundary/proof_receipt_runtime_loading",
    ] {
        require_bool(manifest, path, false, "claim-boundary", issues);
    }
    let non_claims = manifest.pointer("/claims/non_claims").and_then(Value::as_array);
    let names_no_theorem = non_claims
        .is_some_and(|values| values.iter().filter_map(Value::as_str).any(|value| value.contains("no Aeneas theorem")));
    if !names_no_theorem {
        issues.push(Issue::new(
            "claim-boundary",
            "/claims/non_claims",
            "blocked evidence must state that no Aeneas theorem was checked",
        ));
    }
}

fn validate_receipt(receipt: &Value, observation: &Observation, issues: &mut Vec<Issue>) {
    require_string(receipt, "/schema_version", RECEIPT_SCHEMA, "receipt-schema", issues);
    require_string(receipt, "/pilot_id", PILOT_ID, "pilot-identity", issues);
    require_string(receipt, "/outcome", BLOCKED_STATUS, "outcome-class", issues);
    require_string(receipt, "/source/path", SOURCE_PATH, "source-path", issues);
    require_string(receipt, "/source/blake3", &observation.source_blake3, "source-identity", issues);
    require_string(
        receipt,
        "/manifest_source_blake3",
        &observation.manifest_source_blake3,
        "manifest-freshness",
        issues,
    );
    require_string(
        receipt,
        "/blocker/diagnostic_blake3",
        &observation.diagnostic_blake3,
        "diagnostic-identity",
        issues,
    );
    for path in [
        "/evidence_admission/aeneas_property_proved",
        "/evidence_admission/theorem_map_emitted",
        "/evidence_admission/octet_property_receipt_emitted",
        "/evidence_admission/valence_property_handoff_emitted",
    ] {
        require_bool(receipt, path, false, "claim-boundary", issues);
    }
}

fn require_string(document: &Value, path: &str, expected: &str, code: &'static str, issues: &mut Vec<Issue>) {
    let observed = document.pointer(path).and_then(Value::as_str);
    if observed != Some(expected) {
        issues.push(Issue::new(code, path, format!("expected {expected:?}, observed {observed:?}")));
    }
}

fn require_bool(document: &Value, path: &str, expected: bool, code: &'static str, issues: &mut Vec<Issue>) {
    let observed = document.pointer(path).and_then(Value::as_bool);
    if observed != Some(expected) {
        issues.push(Issue::new(code, path, format!("expected {expected}, observed {observed:?}")));
    }
}

fn require_u64(document: &Value, path: &str, expected: u64, code: &'static str, issues: &mut Vec<Issue>) {
    let observed = document.pointer(path).and_then(Value::as_u64);
    if observed != Some(expected) {
        issues.push(Issue::new(code, path, format!("expected {expected}, observed {observed:?}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_JSON: &str = include_str!("../../proofs/aeneas-time-bounds/generated/manifest.json");
    const RECEIPT_JSON: &str = include_str!("../../proofs/aeneas-time-bounds/generated/blocked-receipt.json");
    const SOURCE: &str = include_str!("../../crates/ucan-core/src/token.rs");
    const PROVIDER_SOURCE: &str = include_str!("../../proofs/aeneas-time-bounds/provider-identity.ncl");
    const MANIFEST_SOURCE: &str = include_str!("../../proofs/aeneas-time-bounds/manifest.ncl");
    const DIAGNOSTIC: &str = include_str!("../../proofs/aeneas-time-bounds/evidence/exact-item-selection.log");
    const SUPPLEMENTARY: &str = include_str!("../../proofs/aeneas-time-bounds/evidence/broad-public-probe.log");
    const DRIFT_SOURCE: &str = include_str!("../../proofs/aeneas-time-bounds/fixtures/invalid/source-drift.rs");
    const FOREIGN_RUN_DEF_ID: u64 = 7;

    fn fixture() -> (Value, Value, Observation) {
        let manifest = serde_json::from_str(MANIFEST_JSON).expect("manifest fixture must parse");
        let receipt = serde_json::from_str(RECEIPT_JSON).expect("receipt fixture must parse");
        let observation = Observation {
            source_text: SOURCE.to_owned(),
            source_blake3: digest(SOURCE.as_bytes()),
            provider_source_blake3: digest(PROVIDER_SOURCE.as_bytes()),
            manifest_source_blake3: digest(MANIFEST_SOURCE.as_bytes()),
            diagnostic_blake3: digest(DIAGNOSTIC.as_bytes()),
            supplementary_blake3: digest(SUPPLEMENTARY.as_bytes()),
            drift_source_blake3: digest(DRIFT_SOURCE.as_bytes()),
        };
        (manifest, receipt, observation)
    }

    fn issue_codes(manifest: &Value, receipt: &Value, observation: &Observation) -> BTreeSet<&'static str> {
        validate(manifest, receipt, observation).into_iter().map(|issue| issue.code).collect()
    }

    #[test]
    fn accepts_exact_blocked_evidence() {
        let (manifest, receipt, observation) = fixture();
        assert_eq!(validate(&manifest, &receipt, &observation), Vec::new());
    }

    #[test]
    fn rejects_source_and_shadow_drift() {
        let (manifest, receipt, mut observation) = fixture();
        observation.source_blake3 = digest(b"changed production source");
        observation.drift_source_blake3 = observation.source_blake3.clone();
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("source-identity"));
        assert!(codes.contains("shadow-source"));
    }

    #[test]
    fn rejects_provider_and_extraction_drift() {
        let (mut manifest, receipt, observation) = fixture();
        manifest["provider"]["octet"]["profile_id"] = Value::String("ambient-profile".to_owned());
        manifest["extraction"]["skip_borrowck"] = Value::Bool(true);
        manifest["extraction"]["target_triple"] = Value::String("wasm32-unknown-unknown".to_owned());
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("profile-identity"));
        assert!(codes.contains("borrow-check"));
        assert!(codes.contains("target-metadata"));
    }

    #[test]
    fn rejects_fabricated_translation_and_durable_def_id() {
        let (mut manifest, receipt, observation) = fixture();
        manifest["translation_manifest"]["status"] = Value::String("produced".to_owned());
        manifest["translation_manifest"]["def_id_policy"] = Value::String("durable".to_owned());
        manifest["translation_manifest"]["rows"] = serde_json::json!([{ "def_id": FOREIGN_RUN_DEF_ID }]);
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("translation-manifest"));
        assert!(codes.contains("run-local-id"));
    }

    #[test]
    fn rejects_budget_and_outcome_reclassification() {
        let (mut manifest, receipt, observation) = fixture();
        manifest["budgets"]["exact_stage_timeout_ms"] = Value::from(0_u64);
        manifest["budgets"]["outcome"] = Value::String("exhausted".to_owned());
        manifest["feasibility"]["diagnostic_class"] = Value::String("const-fn-unsupported".to_owned());
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("budget-outcome"));
        assert!(codes.contains("unsupported-boundary"));
    }

    #[test]
    fn rejects_property_overclaim_and_stale_receipt() {
        let (mut manifest, mut receipt, observation) = fixture();
        manifest["claims"]["aeneas_property_proved"] = Value::Bool(true);
        receipt["manifest_source_blake3"] = Value::String(digest(b"stale manifest"));
        receipt["evidence_admission"]["octet_property_receipt_emitted"] = Value::Bool(true);
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("claim-boundary"));
        assert!(codes.contains("manifest-freshness"));
    }

    #[test]
    fn rejects_missing_negative_coverage() {
        let (mut manifest, receipt, observation) = fixture();
        manifest["negative_cases"] = Value::Array(Vec::new());
        let codes = issue_codes(&manifest, &receipt, &observation);
        assert!(codes.contains("negative-coverage"));
    }
}
