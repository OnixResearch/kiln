use super::*;

use serde_json::{json, Value};

use crate::verified::assurance_targets::{
    parse_assurance_targets_policy, EvidenceJob, GeneratedSourceEntry, SourceBinding,
    SpecificationDigest, TargetSpecification,
};
use crate::verified::content_bound_requirement_registry::{
    ContentBoundRequirementRegistryReceipt, ContentBoundRequirementRow, ContentBoundRevision,
    ContentBoundSourceSpan, ContentBoundSpecificationIdentity,
};
use crate::verified::evidence::blake3_hex;
use crate::verified::refinement_typed_dependencies::Assurance;

const POLICY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../cairn-policy/generated/assurance-targets.json"
));

const REVISION: &str = "d5b34c2d64b1227417a46df8ed8427f2d10ed3c8";
const SPECIFICATION_PATH: &str = "specs/pilot/spec.md";
const CONTRACT_ID: &str = "pilot.contract";
const ROW_ONE: &str = "pilot.requirement.one";
const ROW_TWO: &str = "pilot.requirement.two";
const JOB_ID: &str = "pilot.job.v1";
const EVIDENCE_ID: &str = "pilot.evidence.deck";
const DECK_PATH: &str = "artifacts/proof-cards.json";
const RECEIPT_LOCATOR: &str = "external-receipt.json";
const JOB_TEXT: &str =
    r#"{"schema_version":"octet-mantle-producer-job/v1","owner":"Octet","job_id":"pilot.job.v1"}"#;
const HANDOFF_RECEIPT_TEXT: &str = "handoff external receipt bytes";
const HANDOFF_PROFILE_TEXT: &str = "handoff external profile bytes";
const TOOL_GRAPH_TEXT: &str = "tool closure derivation json bytes";
const ROOT_DERIVATION: &str = "/nix/store/00000000000000000000000000000000-pilot-root.drv";
const TRANSLATED_DERIVATION: &str =
    "/nix/store/11111111111111111111111111111111-pilot-translated.drv";
const PRODUCER_DIR: &str = "abc123-pilot-output";
const STORE_PATH: &str = "/nix/store/abc123-pilot-output";
const NAR_SHA256: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn text_facts(text: &str) -> BytesFacts {
    BytesFacts {
        blake3: blake3_hex(text.as_bytes()),
        text: Some(text.to_string()),
    }
}

fn json_facts(value: Value) -> FileFacts {
    FileFacts {
        blake3: blake3_hex(canonical_json(&value).as_bytes()),
        value,
    }
}

fn policy() -> AssuranceTargetsPolicy {
    let value: Value = serde_json::from_str(POLICY_JSON).expect("policy JSON must parse");
    parse_assurance_targets_policy(&value).expect("generated policy must parse")
}

fn row(requirement_id: &str, specification_blake3: &str) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: requirement_id.to_string(),
        specification_path: SPECIFICATION_PATH.to_string(),
        specification_blake3: specification_blake3.to_string(),
        source_span: ContentBoundSourceSpan {
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            end_line: 1,
        },
        requirement_body_blake3: blake3_hex(requirement_id.as_bytes()),
        row_blake3: blake3_hex(format!("row:{requirement_id}").as_bytes()),
    }
}

fn checked_proof_target(row: &ContentBoundRequirementRow) -> AssuranceTarget {
    AssuranceTarget {
        requirement: row.clone(),
        assurance: Assurance::CheckedProof,
        claim: "the pilot theorem states the requirement".to_string(),
        informal_remainder: "fixture packaging outside the theorem".to_string(),
        evidence_job: Some(EvidenceJob {
            job_id: JOB_ID.to_string(),
            definition: "standards/proof-rails/job.ncl".to_string(),
        }),
        checked_scope: Some(vec!["pilot output".to_string()]),
        evidence_ids: Some(vec![EVIDENCE_ID.to_string()]),
        evidence_classes: Some(vec!["formal_proof".to_string()]),
        negative_controls: Some(Vec::new()),
        provider: Some("octet".to_string()),
    }
}

fn generated_entry(requirement_id: &str) -> GeneratedSourceEntry {
    GeneratedSourceEntry {
        requirement_id: requirement_id.to_string(),
        sources: vec![SourceBinding {
            locator: "src/arithmetic.rs".to_string(),
            blake3: blake3_hex(b"pilot source"),
        }],
        source_ids: Vec::new(),
        policy_ids: Vec::new(),
        tool_identity_hashes: Vec::new(),
        artifact_hashes: Vec::new(),
    }
}

/// Build the synthetic positive case. `full` targets both registry rows.
fn build_request(full: bool) -> ConvergenceRequest {
    let policy = policy();
    let specification_blake3 = blake3_hex(b"pilot specification");
    let plain_specification_blake3 = blake3_hex(b"pilot specification markdown");
    let rows = vec![
        row(ROW_ONE, &specification_blake3),
        row(ROW_TWO, &specification_blake3),
    ];
    let target_ids: Vec<&str> = if full {
        vec![ROW_ONE, ROW_TWO]
    } else {
        vec![ROW_ONE]
    };
    let specifications = vec![ContentBoundSpecificationIdentity {
        path: SPECIFICATION_PATH.to_string(),
        content_blake3: specification_blake3,
    }];
    let registry = ContentBoundRequirementRegistry {
        schema_version: "cairn.content-bound-requirement-registry.v1".to_string(),
        boundary: "bounded registry".to_string(),
        repository_id: "onixresearch/pilot".to_string(),
        revision: ContentBoundRevision {
            kind: "git".to_string(),
            value: REVISION.to_string(),
        },
        specification_set_blake3: blake3_hex(b"specification set"),
        specifications,
        policy_blake3: blake3_hex(b"registry policy"),
        rows: rows.clone(),
        non_claims: Vec::new(),
        registry_blake3: blake3_hex(b"registry bytes"),
        receipt: ContentBoundRequirementRegistryReceipt {
            schema_version: "cairn.content-bound-requirement-registry-receipt.v1".to_string(),
            registry_blake3: blake3_hex(b"registry bytes"),
            repository_id: "onixresearch/pilot".to_string(),
            revision_kind: "git".to_string(),
            revision: REVISION.to_string(),
            specification_set_blake3: blake3_hex(b"specification set"),
            policy_blake3: blake3_hex(b"registry policy"),
            row_count: rows.len(),
            non_claims: Vec::new(),
            receipt_blake3: blake3_hex(b"registry receipt"),
        },
    };

    let contract_targets: Vec<AssuranceTarget> = rows
        .iter()
        .filter(|row| target_ids.contains(&row.requirement_id.as_str()))
        .map(checked_proof_target)
        .collect();
    let contract = TargetContract {
        contract_id: CONTRACT_ID.to_string(),
        specification: TargetSpecification {
            path: SPECIFICATION_PATH.to_string(),
            blake3: rows[0].specification_blake3.clone(),
        },
        targets: contract_targets,
        assumptions: Vec::new(),
        exclusions: Vec::new(),
        non_claims: Vec::new(),
    };
    let targets = TargetsFile {
        schema_version: "cairn.assurance-targets.v1".to_string(),
        contracts: vec![contract.clone()],
    };
    let generated = GeneratedSources {
        schema_version: "cairn.assurance-target-sources.v1".to_string(),
        specifications: vec![SpecificationDigest {
            path: SPECIFICATION_PATH.to_string(),
            blake3: plain_specification_blake3.clone(),
        }],
        entries: target_ids.iter().map(|id| generated_entry(id)).collect(),
    };

    let contract_value =
        project_contract(&contract, &generated, &policy).expect("projection must succeed");
    let contract = json_facts(contract_value.clone());
    let contract_digest = contract.blake3.clone();

    let evidence_value = json!({
        "schema_version": "octet-requirement-evidence/v1",
        "contract_id": CONTRACT_ID,
        "contract_blake3": contract_digest,
        "proof_decks": [{"evidence_id": EVIDENCE_ID, "deck": DECK_PATH}],
        "non_claims": [],
    });
    let evidence = json_facts(evidence_value);
    let evidence_digest = evidence.blake3.clone();

    let clauses: Vec<Value> = target_ids
        .iter()
        .map(|id| {
            json!({
                "clause_id": id,
                "required": true,
                "disposition": "formal_proof",
                "status": "passed",
                "evidence_ids": [EVIDENCE_ID],
            })
        })
        .collect();
    let decision_value = json!({
        "schema_version": "octet-requirement-decision/v1",
        "contract_id": CONTRACT_ID,
        "contract_blake3": contract_digest,
        "evidence_blake3": evidence_digest,
        "reviewed_specification": contract_value["reviewed_specification"],
        "aggregate_result": "passed",
        "clauses": clauses,
        "non_claims": [],
    });
    let decision = json_facts(decision_value);

    let handoff_job = FileFacts {
        blake3: blake3_hex(JOB_TEXT.as_bytes()),
        value: serde_json::from_str(JOB_TEXT).expect("job JSON must parse"),
    };
    let observation = json_facts(json!({
        "schema": "mantle-external-evidence-observation-v1",
        "packaging": {"source_revision": REVISION},
        "producer_job_identity": format!("b3:{}", handoff_job.blake3),
        "tool_graph_identity": format!("b3:{}", blake3_hex(TOOL_GRAPH_TEXT.as_bytes())),
        "receipt_identity": format!("b3:{}", blake3_hex(HANDOFF_RECEIPT_TEXT.as_bytes())),
        "proof_acceptance": "not-evaluated-by-mantle",
        "externally_reported_outcome": "supported",
    }));

    let receipt_blake3 = blake3_hex(HANDOFF_RECEIPT_TEXT.as_bytes());
    let card = json!({
        "proof_or_property_id": EVIDENCE_ID,
        "receipt_path": RECEIPT_LOCATOR,
        "artifact_blake3_identities": [
            {"artifact": RECEIPT_LOCATOR, "blake3": receipt_blake3},
        ],
    });
    let mut decks = BTreeMap::new();
    decks.insert(
        DECK_PATH.to_string(),
        json_facts(json!({"schema": "octet-proof-cards/v1", "cards": [card]})),
    );

    let producer_output = ProducerOutputFacts {
        dir_name: PRODUCER_DIR.to_string(),
        receipt: BytesFacts {
            blake3: receipt_blake3.clone(),
            text: Some(HANDOFF_RECEIPT_TEXT.to_string()),
        },
        profile: text_facts(HANDOFF_PROFILE_TEXT),
        decks,
    };

    let realization = json_facts(json!({
        "schema": "mantle-foreign-realization-receipt-v1",
        "status": "complete",
        "strongest_state": "realized",
        "selected_root_paths": [STORE_PATH],
        "units": [{
            "node_id": format!("nix:{}", ROOT_DERIVATION.rsplit('/').next().unwrap_or(ROOT_DERIVATION)),
            "target_derivation": TRANSLATED_DERIVATION,
            "outputs": [{
                "target_path": STORE_PATH,
                "nar_sha256": NAR_SHA256,
                "nar_size": 42,
            }],
        }],
    }));
    let store_info = json_facts(json!({
        "schema": "mantle-store-info-v3",
        "paths": [{
            "store_path": STORE_PATH,
            "nar_sha256": NAR_SHA256,
            "nar_size": 42,
            "signatures": ["pilot-key:AAAA"],
        }],
    }));

    let handoff = HandoffFacts {
        observation,
        job: handoff_job,
        receipt: text_facts(HANDOFF_RECEIPT_TEXT),
        profile: text_facts(HANDOFF_PROFILE_TEXT),
        tool_graph: text_facts(TOOL_GRAPH_TEXT),
        root_derivation: text_facts(&format!("{ROOT_DERIVATION}\n")),
        source_revision: text_facts(&format!("{REVISION}\n")),
    };

    ConvergenceRequest {
        policy,
        policy_blake3: blake3_hex(POLICY_JSON.as_bytes()),
        case_blake3: blake3_hex(b"case bytes"),
        registry_blake3: registry.registry_blake3.clone(),
        registry,
        registry_verification_issues: Vec::new(),
        targets,
        targets_blake3: blake3_hex(b"targets bytes"),
        generated,
        generated_blake3: blake3_hex(b"generated bytes"),
        root_specification_digests: BTreeMap::from([(
            SPECIFICATION_PATH.to_string(),
            plain_specification_blake3,
        )]),
        trusted_signers: vec!["pilot-key".to_string()],
        trusted_builder_keys: Vec::new(),
        contracts: vec![ContractCaseFacts {
            contract_id: CONTRACT_ID.to_string(),
            producer_output_locator: None,
            contract,
            decision,
            evidence,
            handoff,
            producer_output,
            realization: Some(realization),
            original_realization: None,
            store_info: Some(store_info),
        }],
    }
}

fn state_of(receipt: &Value, index: usize) -> String {
    receipt["rows"][index]["state"]
        .as_str()
        .expect("state must be a string")
        .to_string()
}

fn issues_of(receipt: &Value, index: usize) -> Vec<String> {
    receipt["rows"][index]["issues"]
        .as_array()
        .expect("issues must be an array")
        .iter()
        .map(|issue| issue.as_str().unwrap_or_default().to_string())
        .collect()
}

fn assert_row(receipt: &Value, index: usize, expected_state: &str, code: &str) {
    assert_eq!(
        state_of(receipt, index),
        expected_state,
        "issues: {:?}",
        issues_of(receipt, index)
    );
    assert!(
        issues_of(receipt, index).iter().any(|issue| issue == code),
        "expected issue {code}, got {:?}",
        issues_of(receipt, index)
    );
}

fn decision_mut(request: &mut ConvergenceRequest) -> &mut Value {
    &mut request.contracts[0].decision.value
}

fn evidence_mut(request: &mut ConvergenceRequest) -> &mut Value {
    &mut request.contracts[0].evidence.value
}

fn observation_mut(request: &mut ConvergenceRequest) -> &mut Value {
    &mut request.contracts[0].handoff.observation.value
}

// r[verify cairn.convergence.inputs] r[verify cairn.convergence.states]
#[test]
fn positive_case_reports_current_and_untargeted() {
    let request = build_request(false);
    let receipt = evaluate_convergence(&request);

    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert!(issues_of(&receipt, 0).is_empty());
    assert_eq!(receipt["rows"][0]["assurance"], "checked_proof");
    assert_eq!(receipt["rows"][0]["contract_id"], CONTRACT_ID);
    assert_eq!(receipt["rows"][0]["job_id"], JOB_ID);
    assert_eq!(receipt["rows"][0]["bound"]["signer"], "pilot-key");
    assert_eq!(receipt["rows"][0]["bound"]["target_path"], STORE_PATH);
    assert_eq!(receipt["rows"][0]["bound"]["source_revision"], REVISION);
    assert_eq!(
        receipt["rows"][0]["bound"]["root_derivation"],
        ROOT_DERIVATION
    );
    assert_eq!(
        receipt["rows"][0]["bound"]["node_id"],
        format!(
            "nix:{}",
            ROOT_DERIVATION
                .rsplit('/')
                .next()
                .unwrap_or(ROOT_DERIVATION)
        )
    );
    assert_eq!(
        receipt["rows"][0]["bound"]["target_derivation"],
        TRANSLATED_DERIVATION
    );
    assert_eq!(
        receipt["rows"][0]["bound"]["realization_route"],
        Value::Null
    );

    assert_eq!(state_of(&receipt, 1), CONVERGENCE_STATE_UNTARGETED);
    assert!(issues_of(&receipt, 1).is_empty());
    assert_eq!(receipt["rows"][1]["assurance"], Value::Null);

    assert_eq!(receipt["targets_current"], true);
    assert_eq!(receipt["coverage_complete"], false);
    assert_eq!(receipt["complete"], false);
    assert_eq!(receipt["unbound_target_issues"], json!([]));
    assert_eq!(receipt["schema_version"], CONVERGENCE_RECEIPT_SCHEMA_V1);
    assert_eq!(receipt["status"], "evaluated");
    assert_eq!(receipt["authority"], CONVERGENCE_AUTHORITY);
}

// r[verify cairn.convergence.states]
#[test]
fn unverified_registry_is_stale() {
    let mut request = build_request(false);
    request.registry_verification_issues = vec!["registry does not verify".to_string()];
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REGISTRY_UNVERIFIED,
    );
}

#[test]
fn non_git_revision_is_stale() {
    let mut request = build_request(false);
    request.registry.revision.kind = "tag".to_string();
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REVISION_KIND,
    );
}

#[test]
fn stale_row_is_stale() {
    let mut request = build_request(false);
    request.targets.contracts[0].targets[0]
        .requirement
        .row_blake3 = "0".repeat(64);
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_STALE_ROW,
    );
}

#[test]
fn misspecified_contract_is_stale() {
    let mut request = build_request(false);
    request.targets.contracts[0].specification.blake3 = "0".repeat(64);
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_SPECIFICATION_MISMATCH,
    );
}

#[test]
fn missing_contract_inputs_are_incomplete() {
    let mut request = build_request(false);
    request.contracts.clear();
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_INCOMPLETE,
        CONVERGENCE_ISSUE_CONTRACT_INPUTS_MISSING,
    );
}

// r[verify cairn.convergence.completeness]
#[test]
fn a_target_absent_from_the_registry_blocks_completeness() {
    let mut request = build_request(true);
    request
        .registry
        .rows
        .retain(|row| row.requirement_id != ROW_TWO);
    let receipt = evaluate_convergence(&request);

    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert_eq!(receipt["coverage_complete"], true);
    assert_eq!(receipt["targets_current"], false);
    assert_eq!(receipt["complete"], false);
    let unbound = receipt["unbound_target_issues"]
        .as_array()
        .expect("unbound issues must be an array");
    assert!(
        unbound.iter().any(|issue| {
            issue["code"] == "assurance_targets.stale_row"
                && issue["field_path"] == "contracts[0].targets[1].requirement"
        }),
        "unbound issues: {unbound:?}"
    );
}

// r[verify cairn.convergence.completeness]
#[test]
fn zero_targets_is_neither_current_nor_covered() {
    let mut request = build_request(false);
    request.targets.contracts[0].targets.clear();
    request.generated.entries.clear();
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_UNTARGETED);
    assert_eq!(receipt["targets_current"], false);
    assert_eq!(receipt["coverage_complete"], false);
    assert_eq!(receipt["complete"], false);
}

// r[verify cairn.convergence.completeness]
#[test]
fn empty_registry_without_targets_is_not_current() {
    let mut request = build_request(false);
    request.registry.rows.clear();
    request.registry.specifications.clear();
    request.targets.contracts.clear();
    request.generated.entries.clear();
    let receipt = evaluate_convergence(&request);
    assert_eq!(receipt["rows"], json!([]));
    assert_eq!(receipt["targets_current"], false);
    assert_eq!(receipt["coverage_complete"], true);
    assert_eq!(receipt["complete"], false);
}

#[test]
fn projection_mismatch_is_stale() {
    let mut request = build_request(false);
    request.contracts[0].contract.value["non_claims"] = json!(["tampered"]);
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PROJECTION_MISMATCH,
    );
}

#[test]
fn reviewed_specification_plain_digest_matches() {
    let request = build_request(false);
    assert_eq!(
        request.contracts[0].contract.value["reviewed_specification"]["blake3"],
        json!(request.root_specification_digests[SPECIFICATION_PATH])
    );
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
}

#[test]
fn reviewed_specification_changed_digest_is_stale() {
    let mut request = build_request(false);
    request
        .root_specification_digests
        .insert(SPECIFICATION_PATH.to_string(), "0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_STALE,
    );
}

#[test]
fn reviewed_specification_missing_path_is_stale() {
    let mut request = build_request(false);
    request.root_specification_digests.clear();
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_STALE,
    );
}

#[test]
fn decision_contract_digest_mismatch_is_stale() {
    let mut request = build_request(false);
    decision_mut(&mut request)["contract_blake3"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_CONTRACT_BLAKE3_MISMATCH,
    );
}

#[test]
fn decision_reviewed_specification_mismatch_is_stale() {
    let mut request = build_request(false);
    decision_mut(&mut request)["reviewed_specification"]["path"] = json!("specs/other.md");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_MISMATCH,
    );
}

#[test]
fn decision_evidence_digest_mismatch_is_stale() {
    let mut request = build_request(false);
    decision_mut(&mut request)["evidence_blake3"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_EVIDENCE_BLAKE3_MISMATCH,
    );
}

#[test]
fn evidence_contract_mismatch_is_stale() {
    let mut request = build_request(false);
    if let Some(object) = evidence_mut(&mut request).as_object_mut() {
        object.remove("contract_blake3");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_EVIDENCE_CONTRACT_MISMATCH,
    );
}

#[test]
fn missing_decision_clause_is_incomplete() {
    let mut request = build_request(false);
    decision_mut(&mut request)["clauses"] = json!([]);
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_INCOMPLETE,
        CONVERGENCE_ISSUE_DECISION_CLAUSE_MISSING,
    );
}

#[test]
fn failed_aggregate_with_passed_clause_is_failed() {
    let mut request = build_request(false);
    decision_mut(&mut request)["aggregate_result"] = json!("failed");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_FAILED,
        CONVERGENCE_ISSUE_AGGREGATE_FAILED,
    );
}

// r[verify cairn.convergence.states] r[verify cairn.convergence.acceptance]
#[test]
fn clause_status_maps_through_policy() {
    let cases = [
        (
            "failed",
            Some(CONVERGENCE_STATE_FAILED),
            Some(CONVERGENCE_ISSUE_DECISION_FAILED),
        ),
        (
            "stale_binding",
            Some(CONVERGENCE_STATE_STALE),
            Some(CONVERGENCE_ISSUE_DECISION_STALE),
        ),
        (
            "unsupported",
            Some(CONVERGENCE_STATE_UNSUPPORTED),
            Some(CONVERGENCE_ISSUE_DECISION_UNSUPPORTED),
        ),
        (
            "missing",
            Some(CONVERGENCE_STATE_INCOMPLETE),
            Some(CONVERGENCE_ISSUE_DECISION_INCOMPLETE),
        ),
        (
            "incomplete",
            Some(CONVERGENCE_STATE_INCOMPLETE),
            Some(CONVERGENCE_ISSUE_DECISION_INCOMPLETE),
        ),
        (
            "unresolved",
            Some(CONVERGENCE_STATE_INCOMPLETE),
            Some(CONVERGENCE_ISSUE_DECISION_INCOMPLETE),
        ),
        (
            "not-an-admitted-status",
            Some(CONVERGENCE_STATE_UNSUPPORTED),
            Some(CONVERGENCE_ISSUE_UNKNOWN_CLAUSE_STATUS),
        ),
    ];
    for (status, expected_state, expected_code) in cases {
        let mut request = build_request(false);
        decision_mut(&mut request)["clauses"][0]["status"] = json!(status);
        let receipt = evaluate_convergence(&request);
        assert_eq!(
            state_of(&receipt, 0),
            expected_state.unwrap_or_default(),
            "status {status}"
        );
        if let Some(code) = expected_code {
            assert!(
                issues_of(&receipt, 0).iter().any(|issue| issue == code),
                "status {status}: expected {code}, got {:?}",
                issues_of(&receipt, 0)
            );
        }
    }
}

#[test]
fn both_bridge_relay_values_bind() {
    for relay in ["not-evaluated-by-mantle", "externally-reported-by-octet"] {
        let mut request = build_request(false);
        observation_mut(&mut request)["proof_acceptance"] = json!(relay);
        let receipt = evaluate_convergence(&request);
        assert_eq!(
            state_of(&receipt, 0),
            CONVERGENCE_STATE_CURRENT,
            "relay {relay}"
        );
    }
}

#[test]
fn proof_acceptance_relay_outside_the_closed_set_is_unsupported() {
    let mut request = build_request(false);
    observation_mut(&mut request)["proof_acceptance"] = json!("accepted-by-mantle");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNSUPPORTED,
        CONVERGENCE_ISSUE_PROOF_ACCEPTANCE_RELAY,
    );
}

#[test]
fn observation_revision_mismatch_is_stale() {
    let mut request = build_request(false);
    observation_mut(&mut request)["packaging"]["source_revision"] = json!("0000000");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REVISION_MISMATCH,
    );
}

#[test]
fn handoff_job_id_mismatch_is_stale() {
    let mut request = build_request(false);
    request.contracts[0].handoff.job.value["job_id"] = json!("other.job");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_JOB_ID_MISMATCH,
    );
}

// r[verify cairn.convergence.inputs]
#[test]
fn trellis_owner_job_binds_current() {
    let mut request = build_request(false);
    request.contracts[0].handoff.job.value = json!({
        "schema_version": "trellis-mantle-producer-job/v1",
        "owner": "Trellis",
        "job_id": JOB_ID,
    });
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert!(issues_of(&receipt, 0).is_empty());
}

// r[verify cairn.convergence.inputs]
#[test]
fn unknown_producer_owner_is_unsupported() {
    let mut request = build_request(false);
    request.contracts[0].handoff.job.value["owner"] = json!("Unknown");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNSUPPORTED,
        CONVERGENCE_ISSUE_UNKNOWN_PRODUCER_OWNER,
    );
}

// r[verify cairn.convergence.inputs]
#[test]
fn producer_schema_owner_mismatch_is_stale() {
    let mut request = build_request(false);
    request.contracts[0].handoff.job.value =
        json!({"schema_version": "trellis-mantle-producer-job/v1", "owner": "Octet"});
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PRODUCER_SCHEMA_MISMATCH,
    );
}

// r[verify cairn.convergence.inputs]
#[test]
fn missing_producer_owner_is_incomplete() {
    let mut request = build_request(false);
    request.contracts[0].handoff.job.value =
        json!({"schema_version": "octet-mantle-producer-job/v1", "job_id": JOB_ID});
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_INCOMPLETE,
        CONVERGENCE_ISSUE_MISSING_PRODUCER_OWNER,
    );
}

#[test]
fn producer_job_identity_mismatch_is_stale() {
    let mut request = build_request(false);
    observation_mut(&mut request)["producer_job_identity"] =
        json!(format!("b3:{}", "0".repeat(64)));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PRODUCER_JOB_IDENTITY_MISMATCH,
    );
}

#[test]
fn tool_graph_identity_mismatch_is_stale() {
    let mut request = build_request(false);
    observation_mut(&mut request)["tool_graph_identity"] = json!(format!("b3:{}", "0".repeat(64)));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_TOOL_GRAPH_IDENTITY_MISMATCH,
    );
}

#[test]
fn receipt_identity_mismatch_is_stale() {
    let mut request = build_request(false);
    observation_mut(&mut request)["receipt_identity"] = json!(format!("b3:{}", "0".repeat(64)));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_RECEIPT_IDENTITY_MISMATCH,
    );
}

#[test]
fn producer_receipt_and_profile_mismatch_are_stale() {
    let mut receipt_request = build_request(false);
    receipt_request.contracts[0].producer_output.receipt.blake3 = "0".repeat(64);
    let receipt = evaluate_convergence(&receipt_request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PRODUCER_RECEIPT_MISMATCH,
    );

    let mut profile_request = build_request(false);
    profile_request.contracts[0].producer_output.profile.blake3 = "0".repeat(64);
    let receipt = evaluate_convergence(&profile_request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PRODUCER_PROFILE_MISMATCH,
    );
}

#[test]
fn missing_proof_deck_is_incomplete() {
    let mut request = build_request(false);
    request.contracts[0].producer_output.decks.clear();
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_INCOMPLETE,
        CONVERGENCE_ISSUE_PROOF_DECK_MISSING,
    );
}

#[test]
fn proof_card_receipt_mismatch_is_stale() {
    let mut request = build_request(false);
    let deck = request.contracts[0]
        .producer_output
        .decks
        .get_mut(DECK_PATH)
        .expect("deck facts must exist");
    deck.value["cards"][0]["artifact_blake3_identities"][0]["blake3"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH,
    );
}

#[test]
fn proof_card_with_a_foreign_evidence_id_is_stale() {
    let mut request = build_request(false);
    evidence_mut(&mut request)["proof_decks"][0]["evidence_id"] = json!("pilot.evidence.other");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH,
    );
}

#[test]
fn proof_card_with_a_foreign_property_id_is_stale() {
    let mut request = build_request(false);
    let deck = request.contracts[0]
        .producer_output
        .decks
        .get_mut(DECK_PATH)
        .expect("deck facts must exist");
    deck.value["cards"][0]["proof_or_property_id"] = json!("pilot.evidence.other");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH,
    );
}

#[test]
fn proof_card_with_a_foreign_artifact_locator_is_stale() {
    let mut request = build_request(false);
    let deck = request.contracts[0]
        .producer_output
        .decks
        .get_mut(DECK_PATH)
        .expect("deck facts must exist");
    deck.value["cards"][0]["artifact_blake3_identities"][0]["artifact"] =
        json!("other/external-receipt.json");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH,
    );
}

// r[verify cairn.convergence.realization]
#[test]
fn a_preserved_realization_route_is_recorded_in_the_binding() {
    let mut request = build_request(false);
    if let Some(realization) = request.contracts[0].realization.as_mut() {
        realization.value["cache_policy"] = json!({"realization_route": "cache-only-preserve-v1"});
    }
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert_eq!(
        receipt["rows"][0]["bound"]["realization_route"],
        "cache-only-preserve-v1"
    );

    let mut non_string = build_request(false);
    if let Some(realization) = non_string.contracts[0].realization.as_mut() {
        realization.value["cache_policy"] = json!({"realization_route": 7});
    }
    let receipt = evaluate_convergence(&non_string);
    assert_eq!(
        receipt["rows"][0]["bound"]["realization_route"],
        Value::Null
    );
}

// r[verify cairn.convergence.realization]
#[test]
fn missing_realization_is_unrealized() {
    let mut request = build_request(false);
    request.contracts[0].realization = None;
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_REALIZATION_MISSING,
    );
}

#[test]
fn incomplete_realization_is_unrealized() {
    let mut request = build_request(false);
    if let Some(realization) = request.contracts[0].realization.as_mut() {
        realization.value["status"] = json!("partial-failure");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE,
    );
}

#[test]
fn realization_derivation_mismatch_is_stale() {
    let mut request = build_request(false);
    if let Some(realization) = request.contracts[0].realization.as_mut() {
        // The unit is selected by the foreign `node_id`; a matching
        // `target_derivation` alone does not bind it.
        realization.value["units"][0]["node_id"] = json!("nix:other.drv");
        realization.value["units"][0]["target_derivation"] = json!(ROOT_DERIVATION);
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH,
    );
}

#[test]
fn realization_output_mismatch_is_stale() {
    let mut request = build_request(false);
    if let Some(realization) = request.contracts[0].realization.as_mut() {
        realization.value["units"][0]["outputs"][0]["target_path"] = json!("/nix/store/other-name");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REALIZATION_OUTPUT_MISMATCH,
    );
}

#[test]
fn missing_store_info_is_unrealized() {
    let mut request = build_request(false);
    request.contracts[0].store_info = None;
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_STORE_INFO_MISSING,
    );
}

#[test]
fn missing_store_info_path_is_unrealized() {
    let mut request = build_request(false);
    if let Some(store_info) = request.contracts[0].store_info.as_mut() {
        store_info.value["paths"][0]["store_path"] = json!("/nix/store/other-name");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_STORE_INFO_PATH_MISSING,
    );
}

#[test]
fn nar_mismatch_is_stale() {
    let mut request = build_request(false);
    if let Some(store_info) = request.contracts[0].store_info.as_mut() {
        store_info.value["paths"][0]["nar_sha256"] = json!("0".repeat(64));
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_NAR_MISMATCH,
    );
}

#[test]
fn missing_realization_nar_hash_is_stale() {
    let mut request = build_request(false);
    if let Some(realization) = request.contracts[0].realization.as_mut() {
        realization.value["units"][0]["outputs"][0]
            .as_object_mut()
            .expect("output must be an object")
            .remove("nar_sha256");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_NAR_MISMATCH,
    );
}

#[test]
fn missing_store_info_nar_hash_is_stale() {
    let mut request = build_request(false);
    if let Some(store_info) = request.contracts[0].store_info.as_mut() {
        store_info.value["paths"][0]
            .as_object_mut()
            .expect("path entry must be an object")
            .remove("nar_sha256");
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_NAR_MISMATCH,
    );
}

#[test]
fn untrusted_signer_is_unrealized() {
    let mut request = build_request(false);
    if let Some(store_info) = request.contracts[0].store_info.as_mut() {
        store_info.value["paths"][0]["signatures"] = json!(["other-key:AAAA"]);
    }
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_UNTRUSTED_SIGNER,
    );
}

// r[verify cairn.convergence.states]
#[test]
fn unrealized_precedes_stale() {
    let mut request = build_request(false);
    request.contracts[0].realization = None;
    decision_mut(&mut request)["contract_blake3"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_REALIZATION_MISSING,
    );
    assert!(issues_of(&receipt, 0)
        .iter()
        .any(|issue| issue == CONVERGENCE_ISSUE_CONTRACT_BLAKE3_MISMATCH));
}

#[test]
fn reviewed_target_is_not_projected() {
    let mut request = build_request(false);
    request.targets.contracts[0].targets[0].assurance = Assurance::Reviewed;
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_NOT_PROJECTED);
    assert_eq!(receipt["targets_current"], true);
    assert_eq!(receipt["rows"][0]["bound"], Value::Null);
}

// r[verify cairn.convergence.completeness]
#[test]
fn full_coverage_with_current_targets_is_complete() {
    let request = build_request(true);
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert_eq!(state_of(&receipt, 1), CONVERGENCE_STATE_CURRENT);
    assert_eq!(receipt["targets_current"], true);
    assert_eq!(receipt["coverage_complete"], true);
    assert_eq!(receipt["complete"], true);
}

// r[verify cairn.convergence.determinism]
#[test]
fn evaluation_is_deterministic_and_input_sensitive() {
    let request = build_request(false);
    let first = evaluate_convergence(&request);
    let second = evaluate_convergence(&request);
    assert_eq!(first, second);
    assert_eq!(first["receipt_blake3"], second["receipt_blake3"]);

    let mut changed = build_request(false);
    changed.contracts[0].handoff.observation.blake3 = "1".repeat(64);
    let third = evaluate_convergence(&changed);
    assert_ne!(first["receipt_blake3"], third["receipt_blake3"]);
}

// r[verify cairn.convergence.acceptance]
#[test]
fn externally_reported_outcome_is_never_acceptance() {
    let mut request = build_request(false);
    observation_mut(&mut request)["externally_reported_outcome"] = json!("supported");
    decision_mut(&mut request)["clauses"][0]["status"] = json!("failed");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_FAILED,
        CONVERGENCE_ISSUE_DECISION_FAILED,
    );
    assert!(!issues_of(&receipt, 0).is_empty());
}

const BUILDER_FINGERPRINT: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb01";
const ORIGINAL_RECEIPT_BLAKE3: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";
const REUSED_RECEIPT_BLAKE3: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";

/// One version 2 root entry for the fixture's selected root.
fn v2_root(disposition: &str, fingerprint: &str) -> Value {
    let (built, reused) = match disposition {
        ROOT_DISPOSITION_BUILT => (
            json!({
                "mantle_version": "0.2.1",
                "execution_profile_digest_blake3": "3".repeat(64),
                "input_closure_blake3": "4".repeat(64),
            }),
            Value::Null,
        ),
        ROOT_DISPOSITION_REUSED => (
            Value::Null,
            json!({
                "original_receipt_blake3": ORIGINAL_RECEIPT_BLAKE3,
                "source": "local",
                "builds": 0,
                "downloads": 0,
            }),
        ),
        other => panic!("test helper has no shape for disposition {other}"),
    };
    json!({
        "node_id": format!("nix:{}", final_component(ROOT_DERIVATION)),
        "target_derivation": TRANSLATED_DERIVATION,
        "output_name": "out",
        "path": STORE_PATH,
        "nar_sha256": NAR_SHA256,
        "nar_size": 42,
        "references": [STORE_PATH],
        "disposition": disposition,
        "signer": {
            "key_name": "crunch-host-1",
            "public_key_blake3": fingerprint,
            "role": BUILDER_SIGNER_ROLE,
        },
        "trust_policy_blake3": "5".repeat(64),
        "built": built,
        "reused": reused,
    })
}

fn v2_receipt(root: Value, receipt_blake3: &str) -> Value {
    json!({
        "schema": REALIZATION_RECEIPT_SCHEMA_V2,
        "status": "complete",
        "strongest_state": "realized",
        "receipt_blake3": receipt_blake3,
        "cache_policy": {"realization_route": "cache-inputs-build-roots-v2"},
        "roots": [root],
    })
}

/// A request whose realization receipt is version 2. A `reused` request also
/// carries the `built` original the link names.
fn v2_request(disposition: &str) -> ConvergenceRequest {
    let mut request = build_request(false);
    request.trusted_builder_keys = vec![BUILDER_FINGERPRINT.to_string()];
    request.contracts[0].realization = Some(json_facts(v2_receipt(
        v2_root(disposition, BUILDER_FINGERPRINT),
        REUSED_RECEIPT_BLAKE3,
    )));
    if disposition == ROOT_DISPOSITION_REUSED {
        request.contracts[0].original_realization = Some(json_facts(v2_receipt(
            v2_root(ROOT_DISPOSITION_BUILT, BUILDER_FINGERPRINT),
            ORIGINAL_RECEIPT_BLAKE3,
        )));
    }
    request
}

fn realization_mut(request: &mut ConvergenceRequest) -> &mut Value {
    &mut request.contracts[0]
        .realization
        .as_mut()
        .expect("realization must exist")
        .value
}

fn original_realization_mut(request: &mut ConvergenceRequest) -> &mut Value {
    &mut request.contracts[0]
        .original_realization
        .as_mut()
        .expect("original realization must exist")
        .value
}

// r[verify cairn.convergence.realization.root_disposition]
#[test]
fn a_v2_built_root_reports_built() {
    let request = v2_request(ROOT_DISPOSITION_BUILT);
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert_eq!(receipt["rows"][0]["bound"]["root_disposition"], "built");
    assert_eq!(
        receipt["rows"][0]["bound"]["original_receipt_blake3"],
        Value::Null
    );
}

// r[verify cairn.convergence.realization.reuse_link]
#[test]
fn a_v2_reused_root_reports_reused_and_the_original_link() {
    let request = v2_request(ROOT_DISPOSITION_REUSED);
    let receipt = evaluate_convergence(&request);
    assert_eq!(state_of(&receipt, 0), CONVERGENCE_STATE_CURRENT);
    assert_eq!(receipt["rows"][0]["bound"]["root_disposition"], "reused");
    assert_eq!(
        receipt["rows"][0]["bound"]["original_receipt_blake3"],
        ORIGINAL_RECEIPT_BLAKE3
    );
}

#[test]
fn a_v2_reused_root_without_the_original_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    request.contracts[0].original_realization = None;
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
    assert_eq!(receipt["rows"][0]["bound"], Value::Null);
}

#[test]
fn a_v2_reused_root_whose_original_is_not_v2_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    original_realization_mut(&mut request)["schema"] =
        json!("mantle-foreign-realization-receipt-v1");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
}

#[test]
fn a_v2_reused_root_with_a_tampered_original_digest_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    original_realization_mut(&mut request)["receipt_blake3"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
}

#[test]
fn a_v2_reused_root_whose_original_was_not_built_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    original_realization_mut(&mut request)["roots"][0]["disposition"] = json!("reused");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
}

#[test]
fn a_v2_reused_root_with_a_mismatched_original_nar_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    original_realization_mut(&mut request)["roots"][0]["nar_sha256"] = json!("0".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
}

#[test]
fn a_v2_reused_root_with_a_mismatched_original_signer_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    original_realization_mut(&mut request)["roots"][0]["signer"]["public_key_blake3"] =
        json!("c".repeat(64));
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REUSE_LINK_INVALID,
    );
}

#[test]
fn a_v2_untrusted_builder_fingerprint_is_unrealized() {
    for disposition in [ROOT_DISPOSITION_BUILT, ROOT_DISPOSITION_REUSED] {
        let mut request = v2_request(disposition);
        request.trusted_builder_keys = vec!["c".repeat(64)];
        let receipt = evaluate_convergence(&request);
        assert_row(
            &receipt,
            0,
            CONVERGENCE_STATE_UNREALIZED,
            CONVERGENCE_ISSUE_UNTRUSTED_SIGNER,
        );
    }
}

#[test]
fn a_v2_cache_role_signer_is_unrealized() {
    let mut request = v2_request(ROOT_DISPOSITION_BUILT);
    realization_mut(&mut request)["roots"][0]["signer"]["role"] = json!("cache");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_UNTRUSTED_SIGNER,
    );
}

#[test]
fn a_v2_disposition_outside_the_policy_is_unrealized() {
    let mut request = v2_request(ROOT_DISPOSITION_BUILT);
    realization_mut(&mut request)["roots"][0]["disposition"] = json!("preserved");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE,
    );
    assert_eq!(receipt["rows"][0]["bound"], Value::Null);
}

#[test]
fn a_v2_root_whose_built_and_reused_do_not_match_the_disposition_is_unrealized() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    realization_mut(&mut request)["roots"][0]["built"] = json!({"mantle_version": "0.2.1"});
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_UNREALIZED,
        CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE,
    );
}

#[test]
fn a_v2_receipt_without_the_selected_root_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_BUILT);
    realization_mut(&mut request)["roots"] = json!([]);
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH,
    );
}

#[test]
fn a_v2_root_for_another_path_is_stale() {
    let mut request = v2_request(ROOT_DISPOSITION_BUILT);
    realization_mut(&mut request)["roots"][0]["path"] = json!("/nix/store/other-name");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_STALE,
        CONVERGENCE_ISSUE_REALIZATION_OUTPUT_MISMATCH,
    );
}

// r[verify cairn.convergence.realization.root_disposition]
#[test]
fn a_v1_route_names_its_derived_disposition_or_none() {
    let request = build_request(false);
    let receipt = evaluate_convergence(&request);
    assert_eq!(
        receipt["rows"][0]["bound"]["root_disposition"],
        Value::Null,
        "the ordinary route makes no disposition claim"
    );

    let mut preserve = build_request(false);
    if let Some(realization) = preserve.contracts[0].realization.as_mut() {
        realization.value["cache_policy"] = json!({"realization_route": "cache-only-preserve-v1"});
    }
    let receipt = evaluate_convergence(&preserve);
    assert_eq!(receipt["rows"][0]["bound"]["root_disposition"], "preserved");

    let mut build = build_request(false);
    if let Some(realization) = build.contracts[0].realization.as_mut() {
        realization.value["cache_policy"] =
            json!({"realization_route": "cache-inputs-build-roots-v1"});
    }
    let receipt = evaluate_convergence(&build);
    assert_eq!(receipt["rows"][0]["bound"]["root_disposition"], "built");
}

#[test]
fn a_v2_reused_root_with_a_failing_decision_is_not_current() {
    let mut request = v2_request(ROOT_DISPOSITION_REUSED);
    decision_mut(&mut request)["aggregate_result"] = json!("failed");
    decision_mut(&mut request)["clauses"][0]["status"] = json!("failed");
    let receipt = evaluate_convergence(&request);
    assert_row(
        &receipt,
        0,
        CONVERGENCE_STATE_FAILED,
        CONVERGENCE_ISSUE_DECISION_FAILED,
    );
}
