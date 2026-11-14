mod circuit_primitive;
mod common;
mod downstream_pilot;
mod export_bundle;
mod function_object;
mod name_pointer;

pub use circuit_primitive::{
    circuit_primitive_graph_hash, circuit_primitive_hash_material,
    validate_circuit_primitive_receipt, CircuitPrimitiveIssue, CircuitPrimitiveReport,
    CIRCUIT_PRIMITIVE_SCHEMA_V1,
};
pub use downstream_pilot::{
    downstream_pilot_hash, downstream_pilot_hash_material, validate_downstream_pilot_receipt,
    DownstreamPilotIssue, DownstreamPilotReport, DOWNSTREAM_PILOT_SCHEMA_V1,
};
pub use export_bundle::{
    export_bundle_hash, export_bundle_hash_material, validate_export_bundle_receipt,
    ExportBundleIssue, ExportBundleReport, EXPORT_BUNDLE_SCHEMA_V1,
};
pub use function_object::{
    function_object_hash, function_object_hash_material, validate_function_object_receipt,
    FunctionObjectIssue, FunctionObjectReport, FUNCTION_OBJECT_SCHEMA_V1,
};
pub use name_pointer::{
    name_pointer_hash, name_pointer_hash_material, validate_name_pointer_receipt, NamePointerIssue,
    NamePointerReport, NAME_POINTER_SCHEMA_V1,
};

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::common::{NO_CORRECTNESS_CLAIM, SOURCE_SCANNER_DEPENDENCIES};
    use super::*;

    const SOURCE_TOKEN_IDENTITY: &str = "source-token";
    const SOURCE_TOKEN_CONTENT_KIND: &str = "source-token";
    const DRIFT_VALID_STATUS: &str = "valid";
    const CRUNCH_PILOT_KIND: &str = "crunch-mantle";
    const ASPEN_PILOT_KIND: &str = "aspen-ci-job";
    const CRUNCH_BOUNDED_CLAIM: &str = "checker-evidence-linkage";
    const ASPEN_BOUNDED_CLAIM: &str = "ci-job-identity-linkage";

    fn hash_placeholder() -> String {
        "0".repeat(64)
    }

    fn base_receipt(display_root: &str) -> Value {
        json!({
            "schema": FUNCTION_OBJECT_SCHEMA_V1,
            "object_hash": hash_placeholder(),
            "source_token": {
                "crate": "demo",
                "module_path": "demo::math",
                "name": "add_one",
                "definition_span": "src/lib.rs:1:1-3:2"
            },
            "signature_hash": "2d1d6fd7359ba2e5e25737604b14fc6a46fec9c5d7be69f3e3fe021e54ffd0bc",
            "body_hash": "297b70644d4c9a63fb0df22a8f346b7e600a714c471454a7784713b3983aaf74",
            "normalization": {
                "version": "source-token-v1",
                "content_kind": SOURCE_TOKEN_CONTENT_KIND,
                "display_root": display_root
            },
            "toolchain": {
                "rustc": "rustc 1.87.0",
                "octet_extractor": "octet 0.1.0"
            },
            "caveats": {
                "identity": SOURCE_TOKEN_IDENTITY,
                "dependency_status": SOURCE_SCANNER_DEPENDENCIES,
                "macro_status": "source-visible",
                "effect_status": "unknown",
                "correctness_claim": NO_CORRECTNESS_CLAIM
            }
        })
    }

    fn with_current_hash(mut receipt: Value) -> Value {
        let hash = function_object_hash(&receipt);
        receipt["object_hash"] = Value::String(hash);
        receipt
    }

    #[test]
    fn accepts_source_token_function_object_with_caveats() {
        let receipt = with_current_hash(base_receipt("/workspace/one"));
        let report = validate_function_object_receipt(&receipt);
        assert_eq!(report.verdict, "PASS");
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(receipt["object_hash"], report.computed_object_hash);
    }

    #[test]
    fn object_hash_excludes_display_root_for_portability() {
        let left = base_receipt("/workspace/one");
        let right = base_receipt("/workspace/two");
        assert_eq!(function_object_hash(&left), function_object_hash(&right));
    }

    #[test]
    fn rejects_correctness_and_identity_overclaims() {
        let mut receipt = base_receipt("/workspace/one");
        receipt["caveats"]["identity"] = Value::String("semantic-compiler-identity".to_string());
        receipt["caveats"]["correctness_claim"] = Value::String("proves-behavior".to_string());
        let receipt = with_current_hash(receipt);
        let codes = validate_function_object_receipt(&receipt)
            .issues
            .into_iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"function_object.identity_overclaim".to_string()));
        assert!(codes.contains(&"function_object.correctness_overclaim".to_string()));
    }

    #[test]
    fn rejects_ambient_entropy_in_hash_material() {
        let mut receipt = base_receipt("/workspace/one");
        receipt["source_token"]["captured_at_timestamp"] =
            Value::String("2026-05-19T00:00:00Z".to_string());
        receipt["source_token"]["temp_path"] = Value::String("/tmp/cairn-x/src/lib.rs".to_string());
        let receipt = with_current_hash(receipt);
        let report = validate_function_object_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "function_object.ambient_entropy_in_hash_material"));
    }

    fn base_pointer_receipt() -> Value {
        json!({
            "schema": NAME_POINTER_SCHEMA_V1,
            "pointer_hash": hash_placeholder(),
            "name": "demo::math::add_one",
            "old_object_hash": "1111111111111111111111111111111111111111111111111111111111111111",
            "new_object_hash": "2222222222222222222222222222222222222222222222222222222222222222",
            "reason": "source-token body changed",
            "diff_summary": "body_hash changed; signature_hash unchanged",
            "rollback_handle": "pointer://demo::math::add_one@1111111111111111111111111111111111111111111111111111111111111111",
            "mutation_manifest": {
                "manifest_hash": "3333333333333333333333333333333333333333333333333333333333333333",
                "touched_files": [
                    {
                        "path": "octet/pointers/demo.math.add_one.json",
                        "before_hash": "4444444444444444444444444444444444444444444444444444444444444444",
                        "after_hash": "5555555555555555555555555555555555555555555555555555555555555555"
                    }
                ]
            },
            "drift_check": {
                "status": DRIFT_VALID_STATUS,
                "checked_object_hash": "2222222222222222222222222222222222222222222222222222222222222222"
            },
            "caveats": {
                "dependency_status": SOURCE_SCANNER_DEPENDENCIES,
                "macro_status": "source-visible",
                "effect_status": "unknown",
                "pure_cache_eligible": false
            }
        })
    }

    fn with_current_pointer_hash(mut receipt: Value) -> Value {
        let hash = name_pointer_hash(&receipt);
        receipt["pointer_hash"] = Value::String(hash);
        receipt
    }

    #[test]
    fn accepts_reviewable_name_pointer_move() {
        let receipt = with_current_pointer_hash(base_pointer_receipt());
        let report = validate_name_pointer_receipt(&receipt);
        assert_eq!(report.verdict, "PASS");
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(receipt["pointer_hash"], report.computed_pointer_hash);
    }

    #[test]
    fn rejects_pointer_drift_until_update_receipt_exists() {
        let mut receipt = base_pointer_receipt();
        receipt["drift_check"]["status"] = Value::String("drift".to_string());
        let receipt = with_current_pointer_hash(receipt);
        let report = validate_name_pointer_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "name_pointer.drift_detected"));
    }

    #[test]
    fn rejects_unknown_effect_pure_cache_overclaim() {
        let mut receipt = base_pointer_receipt();
        receipt["caveats"]["effect_status"] = Value::String("unknown".to_string());
        receipt["caveats"]["pure_cache_eligible"] = Value::Bool(true);
        let receipt = with_current_pointer_hash(receipt);
        let report = validate_name_pointer_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "name_pointer.pure_cache_overclaim"));
    }

    fn base_export_bundle_receipt() -> Value {
        json!({
            "schema": EXPORT_BUNDLE_SCHEMA_V1,
            "bundle_hash": hash_placeholder(),
            "objects": [
                {
                    "object_hash": "1111111111111111111111111111111111111111111111111111111111111111",
                    "schema": FUNCTION_OBJECT_SCHEMA_V1,
                    "path": "objects/1111111111111111111111111111111111111111111111111111111111111111.json"
                },
                {
                    "object_hash": "2222222222222222222222222222222222222222222222222222222222222222",
                    "schema": FUNCTION_OBJECT_SCHEMA_V1,
                    "path": "objects/2222222222222222222222222222222222222222222222222222222222222222.json"
                }
            ],
            "name_pointers": [
                {
                    "name": "demo::math::add_one",
                    "object_hash": "1111111111111111111111111111111111111111111111111111111111111111",
                    "pointer_hash": "3333333333333333333333333333333333333333333333333333333333333333"
                }
            ],
            "replay": {
                "command": "octet export-bundle",
                "args": ["--crate", "demo", "--format", "json"],
                "toolchain": {
                    "rustc": "rustc 1.87.0",
                    "octet_extractor": "octet 0.1.0"
                }
            },
            "compiler_ir": {
                "hir": {
                    "rustc": "rustc 1.87.0",
                    "extractor": "octet-hir 0.1.0",
                    "dependency_status": "hir-resolved-toolchain-scoped"
                },
                "mir": {
                    "rustc": "rustc 1.87.0",
                    "extractor": "octet-mir 0.1.0",
                    "summary_status": "panic-drop-async-unsafe-summary",
                    "stability_claim": "toolchain-coupled"
                }
            },
            "mutation_manifest": {
                "manifest_hash": "4444444444444444444444444444444444444444444444444444444444444444",
                "touched_files": [
                    {
                        "path": "octet/export-bundle.json",
                        "before_hash": "5555555555555555555555555555555555555555555555555555555555555555",
                        "after_hash": "6666666666666666666666666666666666666666666666666666666666666666"
                    }
                ]
            },
            "caveats": {
                "compiler_ir_status": "toolchain-coupled",
                "correctness_claim": NO_CORRECTNESS_CLAIM
            }
        })
    }

    fn with_current_bundle_hash(mut receipt: Value) -> Value {
        let hash = export_bundle_hash(&receipt);
        receipt["bundle_hash"] = Value::String(hash);
        receipt
    }

    #[test]
    fn accepts_replayable_export_bundle() {
        let receipt = with_current_bundle_hash(base_export_bundle_receipt());
        let report = validate_export_bundle_receipt(&receipt);
        assert_eq!(report.verdict, "PASS");
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(receipt["bundle_hash"], report.computed_bundle_hash);
    }

    #[test]
    fn rejects_unsorted_bundle_membership() {
        let mut receipt = base_export_bundle_receipt();
        receipt["objects"].as_array_mut().unwrap().reverse();
        let receipt = with_current_bundle_hash(receipt);
        let report = validate_export_bundle_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "export_bundle.unsorted_objects"));
    }

    #[test]
    fn rejects_pointer_to_missing_bundle_object() {
        let mut receipt = base_export_bundle_receipt();
        receipt["name_pointers"][0]["object_hash"] = Value::String(
            "9999999999999999999999999999999999999999999999999999999999999999".to_string(),
        );
        let receipt = with_current_bundle_hash(receipt);
        let report = validate_export_bundle_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "export_bundle.unknown_pointer_object"));
    }

    fn base_crunch_pilot_receipt() -> Value {
        json!({
            "schema": DOWNSTREAM_PILOT_SCHEMA_V1,
            "pilot_hash": hash_placeholder(),
            "pilot_kind": CRUNCH_PILOT_KIND,
            "evidence_object_hash": "1111111111111111111111111111111111111111111111111111111111111111",
            "source_object_hash": "2222222222222222222222222222222222222222222222222222222222222222",
            "checker_hash": "3333333333333333333333333333333333333333333333333333333333333333",
            "input_evidence_hash": "4444444444444444444444444444444444444444444444444444444444444444",
            "bounded_claim_class": CRUNCH_BOUNDED_CLAIM,
            "non_claims": {
                "build_correctness": NO_CORRECTNESS_CLAIM,
                "distributed_execution_correctness": NO_CORRECTNESS_CLAIM
            }
        })
    }

    fn base_aspen_pilot_receipt() -> Value {
        json!({
            "schema": DOWNSTREAM_PILOT_SCHEMA_V1,
            "pilot_hash": hash_placeholder(),
            "pilot_kind": ASPEN_PILOT_KIND,
            "evidence_object_hash": "5555555555555555555555555555555555555555555555555555555555555555",
            "source_object_hash": "6666666666666666666666666666666666666666666666666666666666666666",
            "config_hash": "7777777777777777777777777777777777777777777777777777777777777777",
            "source_hash": "8888888888888888888888888888888888888888888888888888888888888888",
            "executor_identity_hash": "9999999999999999999999999999999999999999999999999999999999999999",
            "replay_cache_key_material": {
                "stage": "dogfood",
                "executor": "aspen-vmci",
                "cache_key_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "bounded_claim_class": ASPEN_BOUNDED_CLAIM,
            "non_claims": {
                "build_correctness": NO_CORRECTNESS_CLAIM,
                "distributed_execution_correctness": NO_CORRECTNESS_CLAIM
            }
        })
    }

    fn with_current_pilot_hash(mut receipt: Value) -> Value {
        let hash = downstream_pilot_hash(&receipt);
        receipt["pilot_hash"] = Value::String(hash);
        receipt
    }

    #[test]
    fn accepts_bounded_crunch_pilot_receipt() {
        let receipt = with_current_pilot_hash(base_crunch_pilot_receipt());
        let report = validate_downstream_pilot_receipt(&receipt);
        assert_eq!(report.verdict, "PASS");
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(receipt["pilot_hash"], report.computed_pilot_hash);
    }

    #[test]
    fn accepts_bounded_aspen_pilot_receipt() {
        let receipt = with_current_pilot_hash(base_aspen_pilot_receipt());
        let report = validate_downstream_pilot_receipt(&receipt);
        assert_eq!(report.verdict, "PASS");
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(receipt["pilot_hash"], report.computed_pilot_hash);
    }

    #[test]
    fn rejects_downstream_pilot_correctness_overclaim() {
        let mut receipt = base_crunch_pilot_receipt();
        receipt["non_claims"]["build_correctness"] = Value::String("proven".to_string());
        let receipt = with_current_pilot_hash(receipt);
        let report = validate_downstream_pilot_receipt(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "downstream_pilot.build_correctness_overclaim"));
    }
}
