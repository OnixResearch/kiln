use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use valence_core::formal_proof_chain::aeneas::AeneasChainEdge;
use valence_core::formal_proof_chain::aeneas::AeneasDigestDomain;
use valence_core::formal_proof_chain::aeneas::AeneasEdgeKind;
use valence_core::formal_proof_chain::aeneas::AeneasNodeRole;
use valence_core::formal_proof_chain::aeneas::AeneasTranslationChain;
use valence_core::formal_proof_chain::aeneas::AeneasVerificationRole;
use valence_core::formal_proof_chain::aeneas::LoadedAeneasArtifact;
use valence_core::formal_proof_chain::aeneas::aeneas_translation_chain_artifacts;
use valence_core::formal_proof_chain::aeneas::validate_aeneas_translation_chain;

const ZERO_DIGEST: &str = "b3:0000000000000000000000000000000000000000000000000000000000000000";
const DRIFTED_TARGET: &str = "aarch64-unknown-linux-gnu";
const STALE_THEOREM_ID: &str = "aeneas.fixture.stale";
const MISSING_NODE_ID: &str = "aeneas.missing";
const EXPECTED_NEGATIVE_CASE_COUNT: usize = 37;
const RENUMBERED_DEF_ID: u64 = 7;
const JSON_SUFFIX: &str = ".json";

#[derive(Debug, Deserialize)]
struct NegativeCase {
    case_id: String,
    mutation: String,
    expected_issue: String,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("fixtures/aeneas-translation-chain")
}

fn load_fixture() -> (AeneasTranslationChain, Vec<LoadedAeneasArtifact>) {
    let root = fixture_root();
    let chain_json = fs::read_to_string(root.join("chain.json")).expect("read frozen Aeneas chain");
    let chain = serde_json::from_str::<AeneasTranslationChain>(&chain_json).expect("parse frozen Aeneas chain");
    let loaded = aeneas_translation_chain_artifacts(&chain)
        .into_iter()
        .map(|artifact| {
            let bytes = fs::read(root.join(&artifact)).expect("read frozen Aeneas artifact");
            let measured_blake3 = format!("b3:{}", valence_core::shared_identity::blake3_hex(&bytes));
            let parsed_json = artifact
                .ends_with(JSON_SUFFIX)
                .then(|| serde_json::from_slice(&bytes).expect("parse frozen Aeneas JSON artifact"));
            LoadedAeneasArtifact {
                artifact,
                measured_blake3,
                parsed_json,
            }
        })
        .collect();
    (chain, loaded)
}

fn load_negative_cases() -> Vec<NegativeCase> {
    let json = fs::read_to_string(fixture_root().join("negative-cases.json")).expect("read Aeneas negative matrix");
    serde_json::from_str(&json).expect("parse Aeneas negative matrix")
}

fn node_mut(
    chain: &mut AeneasTranslationChain,
    role: AeneasNodeRole,
) -> &mut valence_core::formal_proof_chain::aeneas::AeneasChainNode {
    chain.nodes.iter_mut().find(|node| node.role == role).expect("fixture role exists")
}

fn artifact_mut<'a>(loaded: &'a mut [LoadedAeneasArtifact], suffix: &str) -> &'a mut LoadedAeneasArtifact {
    loaded
        .iter_mut()
        .find(|artifact| artifact.artifact.ends_with(suffix))
        .expect("fixture artifact exists")
}

fn replace_parsed_json_text(artifact: &mut LoadedAeneasArtifact, from: &str, to: &str) {
    let text = serde_json::to_string(artifact.parsed_json.as_ref().expect("parsed JSON artifact"))
        .expect("serialize parsed JSON artifact");
    artifact.parsed_json = Some(serde_json::from_str(&text.replace(from, to)).expect("parse mutated JSON artifact"));
}

fn apply_mutation(
    case: &NegativeCase,
    chain: &mut AeneasTranslationChain,
    loaded: &mut [LoadedAeneasArtifact],
) -> bool {
    match case.mutation.as_str() {
        "errorful_extraction" => chain.charon_contract.has_errors = true,
        "skip_borrowck" => chain.charon_contract.skip_borrowck = true,
        "target_drift" => chain.charon_contract.target_triple = DRIFTED_TARGET.to_owned(),
        "profile_drift" => chain.charon_contract.extraction_profile_identity.value = ZERO_DIGEST.to_owned(),
        "stale_source" => node_mut(chain, AeneasNodeRole::RustSource).digest.value = ZERO_DIGEST.to_owned(),
        "swapped_llbc" => {
            let generated_digest = node_mut(chain, AeneasNodeRole::GeneratedLean).digest.value.clone();
            node_mut(chain, AeneasNodeRole::Llbc).digest.value = generated_digest;
        }
        "stale_manifest" => node_mut(chain, AeneasNodeRole::TranslationManifest).digest.value = ZERO_DIGEST.to_owned(),
        "cross_run_def_id" => chain.translation_metadata.declarations[0].run_identity = ZERO_DIGEST.to_owned(),
        "durable_def_id" => node_mut(chain, AeneasNodeRole::RustSource).stable_id = "def-id:0".to_owned(),
        "manifest_opacity_drift" => chain.translation_metadata.declarations[0].is_opaque = true,
        "manifest_failure_drift" => chain.translation_metadata.declarations[0].can_fail = false,
        "manifest_divergence_drift" => chain.translation_metadata.declarations[0].can_diverge = true,
        "manifest_recursion_drift" => chain.translation_metadata.declarations[0].is_recursive = true,
        "loop_parent_drift" => chain.translation_metadata.declarations[0].loop_parent_def_id = Some(0),
        "stale_generated_lean" => node_mut(chain, AeneasNodeRole::GeneratedLean).digest.value = ZERO_DIGEST.to_owned(),
        "missing_trust_node" => chain.nodes.retain(|node| node.role != AeneasNodeRole::TrustClosure),
        "theorem_mismatch" => chain.claim.theorem_id = STALE_THEOREM_ID.to_owned(),
        "adequacy_mismatch" => chain.claim.adequacy_decision.value = ZERO_DIGEST.to_owned(),
        "wrong_digest_domain" => {
            node_mut(chain, AeneasNodeRole::RustSource).digest.domain = AeneasDigestDomain::Theorem
        }
        "artifact_role_alias" => {
            let contract_artifact = node_mut(chain, AeneasNodeRole::CharonContract).artifact.clone();
            node_mut(chain, AeneasNodeRole::CharonExtraction).artifact = contract_artifact;
        }
        "unsupported_profile" => chain.profile = "aeneas-lean/v2".to_owned(),
        "unsupported_stage_role" | "missing_node_field" => {
            let mut value = serde_json::to_value(chain).expect("serialize chain mutation");
            if case.mutation == "unsupported_stage_role" {
                value["nodes"][0]["role"] = serde_json::Value::String("unknown_stage".to_owned());
            } else {
                value["nodes"][0].as_object_mut().expect("node object").remove("digest");
            }
            assert!(
                serde_json::from_value::<AeneasTranslationChain>(value).is_err(),
                "{} must fail decoding",
                case.case_id
            );
            return false;
        }
        "duplicate_edge" => chain.edges.push(chain.edges[0].clone()),
        "missing_edge" => {
            chain.edges.remove(0);
        }
        "dangling_edge" => chain.edges[0].to = MISSING_NODE_ID.to_owned(),
        "cycle" => chain.edges.push(AeneasChainEdge {
            from: "aeneas.provenance".to_owned(),
            to: "aeneas.provider-profile".to_owned(),
            kind: AeneasEdgeKind::AttestedBy,
        }),
        "lean_role_promotion" => {
            node_mut(chain, AeneasNodeRole::LeanChecker).verification_role = AeneasVerificationRole::Property;
        }
        "manifest_role_promotion" => {
            node_mut(chain, AeneasNodeRole::TranslationManifest).verification_role = AeneasVerificationRole::Property;
        }
        "missing_octet_verification" => chain.octet_artifact_verified = false,
        "vacuous_approval_drift" => chain.claim.approved_statement_blake3 = ZERO_DIGEST.to_owned(),
        "undeclared_trust_input" => {
            let artifact = artifact_mut(loaded, "trust-closure.json");
            artifact.measured_blake3 = ZERO_DIGEST.to_owned();
            artifact.parsed_json = Some(serde_json::json!({
                "complete": true,
                "declared_external_models": [],
                "declared_axioms": [],
                "declared_opaque_definitions": [],
                "declared_admitted_declarations": [],
                "undeclared_inputs": ["ambient-model"]
            }));
        }
        "weakened_non_claims" => {
            chain.non_claims.pop();
        }
        "unsupported_non_claim" => {
            let mut value = serde_json::to_value(chain).expect("serialize chain mutation");
            value["non_claims"]
                .as_array_mut()
                .expect("non-claims array")
                .push(serde_json::Value::String("behavioral_correctness".to_owned()));
            assert!(
                serde_json::from_value::<AeneasTranslationChain>(value).is_err(),
                "{} must fail decoding",
                case.case_id
            );
            return false;
        }
        "tampered_artifact" => artifact_mut(loaded, "source.rs").measured_blake3 = ZERO_DIGEST.to_owned(),
        "proof_card_mismatch" => {
            let artifact = artifact_mut(loaded, "proof-cards.json");
            artifact.measured_blake3 = ZERO_DIGEST.to_owned();
            replace_parsed_json_text(artifact, "aeneas.fixture.safe_add", STALE_THEOREM_ID);
        }
        "provenance_mismatch" => {
            let artifact = artifact_mut(loaded, "provenance.json");
            artifact.measured_blake3 = ZERO_DIGEST.to_owned();
            replace_parsed_json_text(
                artifact,
                "b0ce7c7d5673e67b0dc5fff7f54d351f4f58f2b70c986023a3933dee15bcf489",
                "0000000000000000000000000000000000000000000000000000000000000000",
            );
        }
        mutation => panic!("unknown negative mutation {mutation}"),
    }
    true
}

#[test]
fn frozen_octet_aeneas_chain_passes_with_bounded_handoff() {
    // r[verify valence.provenance.aeneas_translation_chain.fixture]
    // r[verify valence.provenance.aeneas_translation_chain.handoff]
    let (chain, loaded) = load_fixture();
    let report = validate_aeneas_translation_chain(&chain, &loaded);

    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(report.computed_chain_identity.as_deref(), Some(chain.declared_chain_identity.value.as_str()));
    let handoff = report.handoff.expect("valid chain has generic handoff");
    assert_eq!(handoff.property_count, 1);
    assert_eq!(handoff.theorem_id, chain.claim.theorem_id);
    assert!(report.supported_claim.contains("identity"));
    assert!(!report.supported_claim.contains("correctness"));
}

#[test]
fn declared_negative_matrix_fails_for_expected_reason() {
    // r[verify valence.provenance.aeneas_translation_chain.charon_contract]
    // r[verify valence.provenance.aeneas_translation_chain.theorem_link]
    // r[verify valence.provenance.aeneas_translation_chain.adequacy_link]
    // r[verify valence.provenance.aeneas_translation_chain.non_claims]
    // r[verify valence.provenance.aeneas_translation_chain.final_validation]
    let cases = load_negative_cases();
    assert_eq!(cases.len(), EXPECTED_NEGATIVE_CASE_COUNT);
    for case in cases {
        let (mut chain, mut loaded) = load_fixture();
        if !apply_mutation(&case, &mut chain, &mut loaded) {
            assert_eq!(case.expected_issue, "parse_error");
            continue;
        }
        let report = validate_aeneas_translation_chain(&chain, &loaded);
        assert!(!report.valid, "{} unexpectedly passed", case.case_id);
        assert!(
            report.issues.iter().any(|issue| issue.code == case.expected_issue),
            "{} expected {}, got {:?}",
            case.case_id,
            case.expected_issue,
            report.issues
        );
        assert!(report.handoff.is_none(), "{} emitted a passing handoff", case.case_id);
    }
}

#[test]
fn every_required_stage_edge_domain_role_and_artifact_fails_closed() {
    // r[verify valence.provenance.aeneas_translation_chain.profile]
    // r[verify valence.provenance.aeneas_translation_chain.linkage]
    // r[verify valence.provenance.aeneas_translation_chain.digest_roles]
    // r[verify valence.provenance.aeneas_translation_chain.role_policy]
    let (chain, loaded) = load_fixture();

    for node_index in 0..chain.nodes.len() {
        let mut missing = chain.clone();
        missing.nodes.remove(node_index);
        let report = validate_aeneas_translation_chain(&missing, &loaded);
        assert!(report.issues.iter().any(|issue| issue.code == "required_stage_missing"));
    }
    for edge_index in 0..chain.edges.len() {
        let mut missing = chain.clone();
        missing.edges.remove(edge_index);
        let report = validate_aeneas_translation_chain(&missing, &loaded);
        assert!(report.issues.iter().any(|issue| issue.code == "required_edge_missing"));
    }
    for node_index in 0..chain.nodes.len() {
        let mut substituted = chain.clone();
        substituted.nodes[node_index].digest.domain = AeneasDigestDomain::CompleteChain;
        let report = validate_aeneas_translation_chain(&substituted, &loaded);
        assert!(report.issues.iter().any(|issue| issue.code == "digest_role_mismatch"));
    }
    for node_index in 0..chain.nodes.len() {
        if chain.nodes[node_index].role == AeneasNodeRole::OctetReceipt {
            continue;
        }
        let mut promoted = chain.clone();
        promoted.nodes[node_index].verification_role = AeneasVerificationRole::Property;
        let report = validate_aeneas_translation_chain(&promoted, &loaded);
        assert!(report.issues.iter().any(|issue| issue.code == "role_promotion"));
    }
    for artifact_index in 0..loaded.len() {
        let mut tampered = loaded.clone();
        tampered[artifact_index].measured_blake3 = ZERO_DIGEST.to_owned();
        let report = validate_aeneas_translation_chain(&chain, &tampered);
        assert!(report.issues.iter().any(|issue| issue.code == "artifact_digest_mismatch"));
    }
}

#[test]
fn canonical_identity_ignores_order_labels_paths_and_run_local_ids() {
    // r[verify valence.provenance.aeneas_translation_chain.identity]
    // r[verify valence.provenance.aeneas_translation_chain.translation_manifest]
    let (chain, loaded) = load_fixture();
    let baseline = validate_aeneas_translation_chain(&chain, &loaded);
    let baseline_identity = baseline.computed_chain_identity.expect("baseline identity");

    let mut reordered = chain.clone();
    reordered.nodes.reverse();
    reordered.edges.reverse();
    for node in &mut reordered.nodes {
        node.display_label = format!("review label for {:?}", node.role);
        let old_artifact = node.artifact.clone();
        node.artifact = format!("relocated/{old_artifact}");
    }
    reordered.producer_fixture.artifact = "relocated/artifacts/producer-adapter.json".to_owned();
    let relocated = loaded
        .iter()
        .map(|artifact| LoadedAeneasArtifact {
            artifact: format!("relocated/{}", artifact.artifact),
            measured_blake3: artifact.measured_blake3.clone(),
            parsed_json: artifact.parsed_json.clone(),
        })
        .collect::<Vec<_>>();
    let reordered_report = validate_aeneas_translation_chain(&reordered, &relocated);
    assert!(reordered_report.valid, "{:?}", reordered_report.issues);
    assert_eq!(reordered_report.computed_chain_identity.as_deref(), Some(baseline_identity.as_str()));

    let mut renumbered = chain.clone();
    renumbered.translation_metadata.declarations[0].def_id = RENUMBERED_DEF_ID;
    let renumbered_report = validate_aeneas_translation_chain(&renumbered, &loaded);
    assert!(!renumbered_report.valid);
    assert_eq!(renumbered_report.computed_chain_identity.as_deref(), Some(baseline_identity.as_str()));
    assert!(renumbered_report.issues.iter().any(|issue| issue.code == "observed_manifest_mismatch"));
}

#[test]
fn proof_relevant_material_changes_chain_identity() {
    let (mut chain, loaded) = load_fixture();
    let baseline = validate_aeneas_translation_chain(&chain, &loaded)
        .computed_chain_identity
        .expect("baseline identity");
    node_mut(&mut chain, AeneasNodeRole::MaintainedTheorem).digest.value = ZERO_DIGEST.to_owned();
    let changed = validate_aeneas_translation_chain(&chain, &loaded)
        .computed_chain_identity
        .expect("changed identity");

    assert_ne!(changed, baseline);
}
