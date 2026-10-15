#![allow(clippy::module_name_repetitions)]

mod boundary;
mod html;
mod ingest;
mod model;
mod query;

pub use boundary::output_has_required_boundary;
pub use html::render_evidence_index_html;
pub use ingest::build_evidence_codebase_index;
pub use model::EvidenceArtifactInput;
pub use model::EvidenceCodebaseIndex;
pub use model::EvidenceIndexBuildMode;
pub use model::EvidenceIndexIssue;
pub use model::EvidenceIndexIssueSeverity;
pub use model::EvidenceIndexQueryReport;
pub use model::EvidenceIndexRecord;
pub use model::INDEX_BOUNDARY;
pub use model::INDEX_SCHEMA_VERSION;
pub use model::NORMALIZATION_VERSION;
pub use query::query_evidence_index;

#[cfg(test)]
mod tests {
    use super::model::MALFORMED_JSON_CODE;
    use super::model::UNSUPPORTED_FAMILY_CODE;
    use super::model::blake3_hex;
    use super::*;

    const FINDING_LOCATOR: &str = "artifacts/octet/findings.jsonl";
    const PROVENANCE_LOCATOR: &str = "artifacts/octet/provenance.jsonl";
    const PROOF_CARDS_LOCATOR: &str = "artifacts/octet/proof-cards.json";
    const CAPSULE_LOCATOR: &str = "artifacts/octet/check-capsule.json";
    const TRANSCRIPT_LOCATOR: &str = "docs/examples/release-operator-smoke/transcript.md";
    const FINGERPRINT_LOCATOR: &str = "artifacts/octet/fingerprint-review.json";
    const EVIDENCE_LOG_LOCATOR: &str = "artifacts/octet/evidence-log-manifest.json";
    const RAIL_LOCATOR: &str = "artifacts/octet/evidence-rail-receipt.json";
    const REQUIREMENT_ID: &str = "r[octet.evidence_codebase_index.query.lookup]";
    const LINT_ID: &str = "ambient_clock";
    const POLICY_ID: &str = "octet.policy.capability-boundary";
    const FINDING_STABLE_ID: &str = "finding-stable-id";
    const FINGERPRINT_STABLE_ID: &str = "fingerprint-stable-id";
    const CAPSULE_HASH: &str = "capsule-hash-fixture";
    const EVIDENCE_IDENTITY_LAW: &str = "evidence index equivalent inputs have byte-stable identity";
    const EVIDENCE_ORDERING_LAW: &str = "evidence index records and diagnostics are deterministically ordered";
    const EVIDENCE_NEGATIVE_DUPLICATE_LAW: &str = "evidence index conflicting duplicate locators fail closed";
    const EVIDENCE_INDEX_LAW_REPLAY_COMMAND: &str =
        "nix develop -c bash -c 'cargo test -p valence-core evidence_index_law -- --nocapture'";
    const EVIDENCE_EQUIVALENCE_CASE_BUDGET: usize = 3;
    const ORDERED_PAIR_WINDOW: usize = 2;

    #[test]
    fn deterministic_build_has_stable_records_and_hash() {
        let inputs = fixture_inputs();
        let first = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Strict);
        let second = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Strict);

        assert_eq!(first, second);
        assert!(first.issues.iter().all(|issue| issue.severity == EvidenceIndexIssueSeverity::Caveat));
        assert!(first.records.iter().any(|record| record.artifact_family == "octet-finding"));
        assert!(first.records.iter().any(|record| record.artifact_family == "octet-proof-card"));
        assert!(first.records.iter().any(|record| record.artifact_family == "octet-replay-capsule"));
        assert!(first.records.iter().any(|record| record.artifact_family == "octet-transcript-markdown"));
        assert!(first.index_hash.len() > INDEX_SCHEMA_VERSION.len());
    }

    #[test]
    fn evidence_index_law_equivalent_inputs_have_byte_stable_identity() {
        assert!(EVIDENCE_INDEX_LAW_REPLAY_COMMAND.contains("evidence_index_law"));

        let baseline = build_evidence_codebase_index(&fixture_inputs(), EvidenceIndexBuildMode::Strict);
        let baseline_bytes = serde_json::to_vec(&baseline).expect("serialize baseline evidence index");
        let variants = vec![
            ("reversed", reversed_inputs(fixture_inputs())),
            ("absolute-root", absolute_root_inputs(fixture_inputs(), "/tmp/octet-review-a")),
            (
                "absolute-root-reversed",
                reversed_inputs(absolute_root_inputs(fixture_inputs(), "/var/tmp/octet-review-b")),
            ),
        ];
        let mut case_count = 0;

        for (case_name, inputs) in variants {
            let index = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Strict);
            let rendered = serde_json::to_vec(&index).expect("serialize generated evidence index case");

            assert_eq!(index.index_hash, baseline.index_hash, "{EVIDENCE_IDENTITY_LAW} failed for {case_name}");
            assert_eq!(rendered, baseline_bytes, "{EVIDENCE_IDENTITY_LAW} bytes changed for {case_name}");
            assert_index_ordered(&index);
            case_count += 1;
        }

        assert_eq!(case_count, EVIDENCE_EQUIVALENCE_CASE_BUDGET, "{EVIDENCE_IDENTITY_LAW}");
    }

    #[test]
    fn evidence_index_law_duplicate_advisory_inputs_preserve_identity() {
        let baseline = build_evidence_codebase_index(&fixture_inputs(), EvidenceIndexBuildMode::Advisory);
        let mut duplicate_inputs = fixture_inputs();
        duplicate_inputs.push(duplicate_inputs[0].clone());

        let index = build_evidence_codebase_index(&duplicate_inputs, EvidenceIndexBuildMode::Advisory);

        assert_eq!(index.source_artifact_count, baseline.source_artifact_count, "{EVIDENCE_IDENTITY_LAW}");
        assert_eq!(index.record_count, baseline.record_count, "{EVIDENCE_IDENTITY_LAW}");
        assert_eq!(index.index_hash, baseline.index_hash, "{EVIDENCE_IDENTITY_LAW}");
        assert_eq!(index.records, baseline.records, "{EVIDENCE_IDENTITY_LAW}");
        assert_eq!(index.issues, baseline.issues, "{EVIDENCE_IDENTITY_LAW}");
        assert_index_ordered(&index);
    }

    #[test]
    fn evidence_index_law_conflicting_duplicate_locator_fails_closed_stably() {
        let mut inputs = fixture_inputs();
        inputs.push(EvidenceArtifactInput {
            locator: FINDING_LOCATOR.to_owned(),
            bytes: b"{".to_vec(),
        });

        let first = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Strict);
        let second = build_evidence_codebase_index(&reversed_inputs(inputs), EvidenceIndexBuildMode::Strict);

        assert_eq!(first, second, "{EVIDENCE_NEGATIVE_DUPLICATE_LAW} should be order-stable");
        assert!(
            first.issues.iter().any(|issue| issue.code == MALFORMED_JSON_CODE),
            "{EVIDENCE_NEGATIVE_DUPLICATE_LAW} should report malformed duplicate input"
        );
        assert!(
            first.issues.iter().any(|issue| issue.code == "source_locator.duplicate_conflict"),
            "{EVIDENCE_NEGATIVE_DUPLICATE_LAW} should report duplicate source identity conflicts"
        );
        assert!(
            first.issues.iter().any(|issue| issue.severity == EvidenceIndexIssueSeverity::Error),
            "{EVIDENCE_NEGATIVE_DUPLICATE_LAW} should keep strict-mode errors"
        );
        assert_index_ordered(&first);
    }

    #[test]
    fn iddqd_source_index_reports_advisory_conflicting_duplicate_locators() {
        // r[verify valence.evidence_ir.iddqd_evidence_source_index.fixtures.negative]
        let original = fixture_inputs()
            .into_iter()
            .find(|input| input.locator == FINDING_LOCATOR)
            .expect("fixture should contain finding input");
        let inputs = vec![original, EvidenceArtifactInput {
            locator: FINDING_LOCATOR.to_owned(),
            bytes: br#"{"version":2,"stable_id":"other","lint":"ambient_clock"}"#.to_vec(),
        }];

        let index = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Advisory);

        assert!(
            index.issues.iter().any(|issue| issue.code == "source_locator.duplicate_conflict"
                && issue.severity == EvidenceIndexIssueSeverity::Caveat),
            "{EVIDENCE_NEGATIVE_DUPLICATE_LAW} should surface advisory duplicate conflicts"
        );
    }

    #[test]
    fn absolute_root_prefixes_do_not_change_index_hash() {
        let mut left = fixture_inputs();
        let mut right = fixture_inputs();
        for input in &mut left {
            input.locator = format!("/tmp/review-a/{}", input.locator);
        }
        for input in &mut right {
            input.locator = format!("/home/review-b/work/{}", input.locator);
        }

        let left_index = build_evidence_codebase_index(&left, EvidenceIndexBuildMode::Strict);
        let right_index = build_evidence_codebase_index(&right, EvidenceIndexBuildMode::Strict);

        assert_eq!(left_index.index_hash, right_index.index_hash);
        assert_eq!(left_index.records, right_index.records);
    }

    #[test]
    fn malformed_and_missing_identity_artifacts_fail_closed_in_strict_mode() {
        let inputs = vec![
            EvidenceArtifactInput {
                locator: "artifacts/octet/malformed.json".to_owned(),
                bytes: b"{".to_vec(),
            },
            EvidenceArtifactInput {
                locator: FINDING_LOCATOR.to_owned(),
                bytes: br#"{"version":2,"lint":"ambient_clock","evidence_tier":"static_lint"}"#.to_vec(),
            },
            EvidenceArtifactInput {
                locator: "artifacts/octet/unsupported.json".to_owned(),
                bytes: br#"{"hello":"world"}"#.to_vec(),
            },
        ];

        let index = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Strict);

        assert!(index.issues.iter().any(|issue| issue.code == MALFORMED_JSON_CODE));
        assert!(index.issues.iter().any(|issue| issue.code == "finding.missing_stable_id"));
        assert!(index.issues.iter().any(|issue| issue.code == UNSUPPORTED_FAMILY_CODE));
        assert!(index.issues.iter().any(|issue| issue.severity == EvidenceIndexIssueSeverity::Error));
    }

    #[test]
    fn advisory_mode_keeps_partial_caveats_without_panic() {
        let inputs = vec![EvidenceArtifactInput {
            locator: "artifacts/octet/unsupported.json".to_owned(),
            bytes: br#"{"hello":"world"}"#.to_vec(),
        }];

        let index = build_evidence_codebase_index(&inputs, EvidenceIndexBuildMode::Advisory);

        assert_eq!(index.record_count, 0);
        assert!(index.issues.iter().all(|issue| issue.severity == EvidenceIndexIssueSeverity::Caveat));
    }

    #[test]
    fn stale_provenance_hash_is_reported() {
        let artifact = EvidenceArtifactInput {
            locator: "artifacts/octet/status.json".to_owned(),
            bytes: br#"{"status":"clean"}"#.to_vec(),
        };
        let provenance = EvidenceArtifactInput {
            locator: PROVENANCE_LOCATOR.to_owned(),
            bytes: br#"{"schema_version":"octet-provenance-record/v1","source_id":"octet.runner.artifact-bundle","checker_id":"cargo-octet","evidence_tier":"manual_review_required","artifact_locator":"artifacts/octet/status.json","blake3":"stale","checked_scope":"fixture","human_action":"review"}
"#.to_vec(),
        };

        let index = build_evidence_codebase_index(&[artifact, provenance], EvidenceIndexBuildMode::Strict);

        assert!(index.issues.iter().any(|issue| issue.code == "provenance.blake3_mismatch"));
    }

    #[test]
    fn query_returns_direct_and_linked_records() {
        let index = build_evidence_codebase_index(&fixture_inputs(), EvidenceIndexBuildMode::Strict);
        let report = query_evidence_index(&index, REQUIREMENT_ID);

        assert!(report.records.iter().any(|record| record.stable_id == FINDING_STABLE_ID));
        assert!(report.records.iter().any(|record| record.policy_ids.contains(&POLICY_ID.to_owned())));
        assert!(report.records.iter().any(|record| record.lint_ids.contains(&LINT_ID.to_owned())));
        assert!(report.records.iter().any(|record| record.stable_id == CAPSULE_HASH));
        assert!(report.boundary.contains("identity and linkage only"));
    }

    #[test]
    fn html_surface_has_links_and_non_claim_boundary() {
        let index = build_evidence_codebase_index(&fixture_inputs(), EvidenceIndexBuildMode::Strict);
        let html = render_evidence_index_html(&index);

        assert!(html.contains("Octet evidence codebase index"));
        assert!(html.contains("href=\"#octet-finding-"));
        assert!(html.contains("identity and linkage only"));
        assert!(output_has_required_boundary(&html));
    }

    #[test]
    fn boundary_rejects_ownership_and_non_proof_overclaims() {
        let clean = format!("{INDEX_BOUNDARY}\nOctet retains release ownership.");
        assert!(output_has_required_boundary(&clean));

        let overclaims = [
            "proves whole-program correctness",
            "proves semantic equivalence",
            "complete dependency closure proof",
            "distributed execution correctness guarantee",
            "remote code mobility guarantee",
            "formal proof for non-proof tiers",
            "claims standalone Valence release ownership",
            "Valence owns the evidence codebase index release",
        ];
        for overclaim in overclaims {
            let tampered = format!("{INDEX_BOUNDARY}\n{overclaim}");
            assert!(!output_has_required_boundary(&tampered), "accepted overclaim: {overclaim}");
        }
    }

    fn fixture_inputs() -> Vec<EvidenceArtifactInput> {
        let finding = format!(
            "{{\"version\":2,\"index\":\"F1\",\"stable_id\":\"{FINDING_STABLE_ID}\",\"semantic_id\":\"semantic-fixture\",\"identity_status\":\"computed\",\"normalized_snippet_hash\":\"snippet-fixture\",\"config_hash\":\"config-fixture\",\"cache_status\":\"disabled\",\"lint\":\"{LINT_ID}\",\"group\":\"tigerstyle::purity\",\"severity\":\"warning\",\"file\":\"src/lib.rs\",\"line\":1,\"column\":1,\"message\":\"{REQUIREMENT_ID}\",\"principle\":\"Functional Core\",\"effect_family\":\"time\",\"evidence_tier\":\"static_lint\",\"crate_name\":\"fixture\"}}\n"
        );
        let proof_cards = format!(
            "{{\"schema_version\":\"octet-proof-cards/v1\",\"cards\":[{{\"proof_or_property_id\":\"{POLICY_ID}#rail-0\",\"source_requirement_or_policy_id\":\"{REQUIREMENT_ID}\",\"policy_id\":\"{POLICY_ID}\",\"rail_index\":0,\"checked_rust_source_scope\":\"src/lib.rs\",\"source_paths\":[\"src/lib.rs\"],\"proof_target_paths\":[\"proof.rs\"],\"verifier_mode\":\"fixture\",\"verifier_identity_summary\":\"fixture\",\"solver_identity_summary\":\"fixture\",\"toolchain_identity_summary\":\"fixture\",\"receipt_path\":\"artifacts/octet/proofs/policy.receipt\",\"artifact_blake3_identities\":[{{\"artifact\":\"artifacts/octet/proofs/policy.receipt\",\"blake3\":\"proof-hash\"}}],\"evidence_tier\":\"formal_proof\",\"evidence_origin\":\"octet-rerun\",\"status\":\"passed\",\"trust_boundaries\":[],\"non_claims\":[\"not whole-program correctness\"],\"artifact_verifier_command\":\"cargo octet artifact verify --artifact-dir artifacts/octet\",\"provenance_artifact\":\"artifacts/octet/provenance.jsonl\",\"diagnostics\":[]}}]}}"
        );
        let capsule = format!(
            "{{\"schema_version\":1,\"capsule_hash\":\"{CAPSULE_HASH}\",\"capsule_kind\":\"octet-replay-capsule\",\"manifest\":{{\"command_argv\":[\"cargo\",\"octet\",\"check\"],\"run_kind\":\"check\",\"octet_binary_identity\":\"cargo-octet\",\"config_hash\":\"config-fixture\",\"input_hashes\":[\"input-hash\"],\"output_hashes\":[\"{FINDING_STABLE_ID}\"],\"expected_exit_code\":0}},\"replay_contract\":{{}},\"redaction_status\":\"normalized\"}}"
        );
        let transcript = "$ cargo octet check\n=> pass\n".to_owned();
        let fingerprint = format!(
            "{{\"schema_version\":1,\"tool_name\":\"cargo-octet\",\"tool_version\":\"0.1.0\",\"mode\":\"check\",\"input_paths\":[\"src/lib.rs\"],\"manifest_hash\":\"manifest-fixture\",\"records\":[{{\"stable_id\":\"{FINGERPRINT_STABLE_ID}\",\"path\":\"src/lib.rs\",\"name\":\"checked\",\"status\":\"match\",\"computed_hash\":\"fingerprint-hash\",\"normalized_body_hash\":\"body-hash\",\"semantic_id\":\"semantic-fixture\",\"dependency_set_hash\":\"dependency-hash\",\"dependency_identity_status\":\"source-scanner-token-set\",\"tool_version\":\"0.1.0\",\"review\":\"{REQUIREMENT_ID}\",\"evidence\":\"{POLICY_ID}\",\"requires_review\":false,\"macro_warning\":false}}]}}"
        );
        let evidence_log = "{\"schema_version\":\"octet-evidence-log-manifest/v1\",\"logs\":[{\"artifact_locator\":\"run.txt.gz\",\"blake3\":\"log-hash\",\"bytes\":10,\"replay_command\":\"cargo octet check\",\"checked_scope\":\"fixture\",\"capture_source\":\"test\",\"retention\":\"compressed\"}]}".to_owned();
        let rail = format!(
            "{{\"schema_version\":\"octet-evidence-rail-receipt/v1\",\"rail_id\":\"{POLICY_ID}\",\"checker_id\":\"cargo-deny\",\"mode\":\"import\",\"command_argv\":[\"cargo\",\"deny\",\"check\"],\"workspace_scope\":\".\",\"checked_rule_families\":[\"licenses\"],\"status\":\"passed\",\"finding_counts\":{{}},\"bounded_log_identity\":\"rail-log-hash\",\"imported_input_identity\":\"rail-input-hash\",\"non_claims\":[\"not correctness\"],\"diagnostics\":[],\"findings\":[{{\"family\":\"{LINT_ID}\",\"severity\":\"warning\",\"message\":\"fixture\",\"file\":\"src/lib.rs\"}}],\"parent_artifacts\":[\"{CAPSULE_HASH}\"]}}"
        );
        let provenance = format!(
            "{{\"schema_version\":\"octet-provenance-record/v1\",\"source_id\":\"octet.transcript.assertion\",\"checker_id\":\"octet-transcript\",\"evidence_tier\":\"transcript_assertion\",\"artifact_locator\":\"{TRANSCRIPT_LOCATOR}\",\"blake3\":\"{}\",\"checked_scope\":\"transcript\",\"parent_artifacts\":[\"{CAPSULE_HASH}\"]}}\n",
            blake3_hex(transcript.as_bytes())
        );

        vec![
            input(FINDING_LOCATOR, finding),
            input(PROVENANCE_LOCATOR, provenance),
            input(PROOF_CARDS_LOCATOR, proof_cards),
            input(CAPSULE_LOCATOR, capsule),
            input(TRANSCRIPT_LOCATOR, transcript),
            input(FINGERPRINT_LOCATOR, fingerprint),
            input(EVIDENCE_LOG_LOCATOR, evidence_log),
            input(RAIL_LOCATOR, rail),
        ]
    }

    fn input(locator: &str, text: String) -> EvidenceArtifactInput {
        EvidenceArtifactInput {
            locator: locator.to_owned(),
            bytes: text.into_bytes(),
        }
    }

    fn reversed_inputs(mut inputs: Vec<EvidenceArtifactInput>) -> Vec<EvidenceArtifactInput> {
        inputs.reverse();
        inputs
    }

    fn absolute_root_inputs(mut inputs: Vec<EvidenceArtifactInput>, root: &str) -> Vec<EvidenceArtifactInput> {
        for input in &mut inputs {
            input.locator = format!("{root}/{}", input.locator);
        }
        inputs
    }

    fn assert_index_ordered(index: &EvidenceCodebaseIndex) {
        for pair in index.records.windows(ORDERED_PAIR_WINDOW) {
            let left = &pair[0];
            let right = &pair[1];
            let ordered = left
                .artifact_family
                .cmp(&right.artifact_family)
                .then(left.stable_id.cmp(&right.stable_id))
                .then(left.artifact_locator.cmp(&right.artifact_locator))
                .then(left.record_id.cmp(&right.record_id));
            assert!(ordered.is_le(), "{EVIDENCE_ORDERING_LAW}: records are not sorted");
        }
        for pair in index.issues.windows(ORDERED_PAIR_WINDOW) {
            let left = &pair[0];
            let right = &pair[1];
            let ordered = left
                .severity
                .cmp(&right.severity)
                .then(left.code.cmp(&right.code))
                .then(left.artifact_locator.cmp(&right.artifact_locator))
                .then(left.detail.cmp(&right.detail));
            assert!(ordered.is_le(), "{EVIDENCE_ORDERING_LAW}: issues are not sorted");
        }
    }
}
