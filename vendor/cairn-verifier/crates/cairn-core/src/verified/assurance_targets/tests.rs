use super::*;

use serde_json::{json, Value};

const TARGETS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/assurance-targets/octet-example/targets.json"
));
const GENERATED_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/assurance-targets/octet-example/generated.json"
));
const OCTET_CLAUSE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/assurance-targets/octet-example/octet-clause-0.json"
));
const OCTET_META_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/assurance-targets/octet-example/octet-contract-meta.json"
));
const POLICY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../cairn-policy/generated/assurance-targets.json"
));

fn value(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture JSON must parse")
}

fn policy() -> AssuranceTargetsPolicy {
    parse_assurance_targets_policy(&value(POLICY_JSON)).expect("generated policy must parse")
}

fn admitted_targets() -> TargetsFile {
    admit_targets(&value(TARGETS_JSON), &policy()).expect("golden targets must be admitted")
}

fn expect_rejected<T>(result: Result<T, Vec<AssuranceTargetIssue>>) -> Vec<AssuranceTargetIssue> {
    match result {
        Ok(_) => panic!("input was admitted but rejection was required"),
        Err(issues) => issues,
    }
}

fn assert_code(issues: &[AssuranceTargetIssue], code: &str) {
    assert!(
        issues.iter().any(|issue| issue.code == code),
        "expected issue code {code}, got {issues:?}"
    );
}

fn registry_rows(targets: &TargetsFile) -> Vec<ContentBoundRequirementRow> {
    targets
        .contracts
        .iter()
        .flat_map(|contract| contract.targets.iter())
        .map(|target| target.requirement.clone())
        .collect()
}

fn registry_specifications(targets: &TargetsFile) -> Vec<ContentBoundSpecificationIdentity> {
    targets
        .contracts
        .iter()
        .map(|contract| ContentBoundSpecificationIdentity {
            path: contract.specification.path.clone(),
            content_blake3: contract.specification.blake3.clone(),
        })
        .collect()
}

fn multi_contract_targets() -> TargetsFile {
    let mut multi = value(TARGETS_JSON);
    let mut second = multi["contracts"][0].clone();
    second["contract_id"] = json!("octet.requirements.second");
    second["targets"][0]["requirement"]["requirement_id"] = json!("octet.other.theorem_adequacy");
    second["targets"][1]["requirement"]["requirement_id"] = json!("octet.other.reviewed_scope");
    multi["contracts"]
        .as_array_mut()
        .expect("contracts must be an array")
        .push(second);
    admit_targets(&multi, &policy()).expect("two-contract targets must be admitted")
}

fn multi_contract_generated() -> Value {
    let mut generated = value(GENERATED_JSON);
    generated["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(json!({
            "requirement_id": "octet.other.theorem_adequacy",
            "sources": [{ "locator": "other/src/lib.rs", "blake3": "b".repeat(64) }],
        }));
    generated
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn golden_projection_reproduces_octet_clause() {
    let targets = admitted_targets();
    let policy = policy();
    let contract = &targets.contracts[0];
    let generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");

    let projected =
        project_contract(contract, &generated, &policy).expect("golden targets must project");

    let expected_clause = value(OCTET_CLAUSE_JSON);
    let clauses = projected["clauses"]
        .as_array()
        .expect("projection must carry clauses");
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0], expected_clause);
    assert_eq!(projected["schema_version"], json!(OCTET_CONTRACT_SCHEMA_V1));

    let meta = value(OCTET_META_JSON);
    assert_eq!(projected["contract_id"], meta["contract_id"]);
    assert_eq!(
        projected["reviewed_specification"],
        meta["reviewed_specification"]
    );
    assert_eq!(
        projected["reviewed_specification"]["blake3"],
        generated.specifications[0].blake3
    );

    assert_eq!(projected["assumptions"], meta["assumptions"]);
    assert_eq!(projected["exclusions"], meta["exclusions"]);
    assert_eq!(projected["non_claims"], meta["non_claims"]);

    assert!(compare_projection(&projected, &projected).is_empty());

    let rows = registry_rows(&targets);
    let specifications = registry_specifications(&targets);
    assert!(bind_targets_to_registry(&targets, &rows, &specifications).is_empty());
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn projection_uses_the_plain_specification_digest() {
    let domain = "7".repeat(64);
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["specification"]["blake3"] = json!(domain);
    for target in targets_value["contracts"][0]["targets"]
        .as_array_mut()
        .expect("targets must be an array")
    {
        target["requirement"]["specification_blake3"] = json!(domain);
    }
    let policy = policy();
    let targets = admit_targets(&targets_value, &policy).expect("targets must be admitted");
    let contract = &targets.contracts[0];
    let generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");

    let projected =
        project_contract(contract, &generated, &policy).expect("golden targets must project");
    assert_eq!(
        projected["reviewed_specification"]["path"],
        json!(contract.specification.path)
    );
    assert_eq!(
        projected["reviewed_specification"]["blake3"],
        json!(generated.specifications[0].blake3)
    );
    assert_ne!(
        projected["reviewed_specification"]["blake3"],
        json!(domain),
        "the registry's domain-separated identity must never reach Octet"
    );
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn reviewed_target_yields_no_clause() {
    let targets = admitted_targets();
    let policy = policy();
    let contract = &targets.contracts[0];
    let generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");
    let projected =
        project_contract(contract, &generated, &policy).expect("golden targets must project");

    let clauses = projected["clauses"]
        .as_array()
        .expect("projection must carry clauses");
    assert_eq!(clauses.len(), 1);
    let reviewed_id = contract.targets[1].requirement.requirement_id.as_str();
    assert!(contract.targets[1].assurance == Assurance::Reviewed);
    assert!(
        clauses
            .iter()
            .all(|clause| clause["clause_id"] != json!(reviewed_id)),
        "reviewed target must not produce a clause"
    );
}

// r[verify cairn.assurance_targets.row_binding]
#[test]
fn stale_row_is_rejected() {
    let targets = admitted_targets();
    let specifications = registry_specifications(&targets);

    let rows = registry_rows(&targets);
    assert!(bind_targets_to_registry(&targets, &rows, &specifications).is_empty());

    let mut stale = registry_rows(&targets);
    stale[0].row_blake3 = "0".repeat(64);
    let issues = bind_targets_to_registry(&targets, &stale, &specifications);
    assert_code(&issues, ISSUE_STALE_ROW);

    let mut missing = registry_rows(&targets);
    missing.remove(0);
    let issues = bind_targets_to_registry(&targets, &missing, &specifications);
    assert_code(&issues, ISSUE_STALE_ROW);
}

// r[verify cairn.assurance_targets.row_binding]
#[test]
fn specification_mismatch_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["requirement"]["specification_blake3"] =
        json!("1".repeat(64));
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_SPECIFICATION_MISMATCH);

    let targets = admitted_targets();
    let rows = registry_rows(&targets);
    let mut specifications = registry_specifications(&targets);
    specifications[0].content_blake3 = "2".repeat(64);
    let issues = bind_targets_to_registry(&targets, &rows, &specifications);
    assert_code(&issues, ISSUE_SPECIFICATION_MISMATCH);

    assert!(
        bind_targets_to_registry(&targets, &rows, &registry_specifications(&targets)).is_empty()
    );
}

// r[verify cairn.assurance_targets.row_binding]
#[test]
fn duplicate_requirement_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    let first = targets_value["contracts"][0]["targets"][0].clone();
    targets_value["contracts"][0]["targets"][1] = first;
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_DUPLICATE_REQUIREMENT);
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn unknown_assurance_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["assurance"] = json!("vibes");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_UNKNOWN_ASSURANCE);
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn unassessed_target_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["assurance"] = json!("unassessed");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_UNASSESSED_TARGET);
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn bound_exceeded_is_rejected() {
    let mut policy = policy();
    policy.bounds.max_targets_per_contract = 1;
    let issues = expect_rejected(admit_targets(&value(TARGETS_JSON), &policy));
    assert_code(&issues, ISSUE_BOUND_EXCEEDED);
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn unknown_field_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["extra"] = json!(1);
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_UNKNOWN_FIELD);
    assert!(
        issues
            .iter()
            .any(|issue| issue.field_path.ends_with("extra")),
        "unknown field path must name the rejected key: {issues:?}"
    );
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn producer_record_bounds_are_required() {
    for key in ["max_tool_graph_bytes", "max_realization_receipt_bytes"] {
        let mut policy_value = value(POLICY_JSON);
        policy_value["bounds"]
            .as_object_mut()
            .expect("bounds must be an object")
            .remove(key);
        let issues = expect_rejected(parse_assurance_targets_policy(&policy_value));
        assert_code(&issues, ISSUE_UNKNOWN_FIELD);
        assert!(
            issues.iter().any(|issue| issue.field_path.ends_with(key)),
            "missing {key} must be named: {issues:?}"
        );
    }
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn unknown_schema_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["schema_version"] = json!("cairn.assurance-targets.v2");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_UNKNOWN_SCHEMA);

    let mut policy_value = value(POLICY_JSON);
    policy_value["schema"] = json!("cairn.assurance-targets-policy.v2");
    let issues = expect_rejected(parse_assurance_targets_policy(&policy_value));
    assert_code(&issues, ISSUE_UNKNOWN_SCHEMA);

    let mut policy_value = value(POLICY_JSON);
    policy_value["targets_schema"] = json!("cairn.assurance-targets.v2");
    let issues = expect_rejected(parse_assurance_targets_policy(&policy_value));
    assert_code(&issues, ISSUE_UNKNOWN_SCHEMA);

    let mut policy_value = value(POLICY_JSON);
    policy_value["sources_schema"] = json!("cairn.assurance-target-sources.v2");
    let issues = expect_rejected(parse_assurance_targets_policy(&policy_value));
    assert_code(&issues, ISSUE_UNKNOWN_SCHEMA);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn missing_evidence_job_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    let target = targets_value["contracts"][0]["targets"][0]
        .as_object_mut()
        .expect("target must be an object");
    target.remove("evidence_job");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_MISSING_EVIDENCE_JOB);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn missing_projection_field_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    let target = targets_value["contracts"][0]["targets"][0]
        .as_object_mut()
        .expect("target must be an object");
    target.remove("checked_scope");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_MISSING_PROJECTION_FIELD);

    let mut targets_value = value(TARGETS_JSON);
    let target = targets_value["contracts"][0]["targets"][0]
        .as_object_mut()
        .expect("target must be an object");
    target.remove("negative_controls");
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_MISSING_PROJECTION_FIELD);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn authored_hash_is_rejected() {
    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["sources"] = json!([]);
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_AUTHORED_HASH);

    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["artifact_hashes"] = json!([]);
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_AUTHORED_HASH);

    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["author_blake3"] = json!("0".repeat(64));
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_AUTHORED_HASH);
    assert!(
        issues
            .iter()
            .any(|issue| issue.field_path == "contracts[0].targets[0].author_blake3"),
        "authored key issue must name its target field: {issues:?}"
    );

    let mut targets_value = value(TARGETS_JSON);
    targets_value["contracts"][0]["targets"][0]["evidence_job"]["definition_hash"] =
        json!("0".repeat(64));
    let issues = expect_rejected(admit_targets(&targets_value, &policy()));
    assert_code(&issues, ISSUE_AUTHORED_HASH);
    assert!(
        issues.iter().any(|issue| {
            issue.field_path == "contracts[0].targets[0].evidence_job.definition_hash"
        }),
        "nested evidence job hash must be rejected at its field path: {issues:?}"
    );
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn missing_generated_sources_is_rejected() {
    let targets = admitted_targets();
    let mut generated = value(GENERATED_JSON);
    generated["entries"] = json!([]);
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);

    let mut generated = value(GENERATED_JSON);
    generated["entries"][0]["sources"] = json!([]);
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn missing_specification_digest_is_rejected() {
    let targets = admitted_targets();

    let mut generated = value(GENERATED_JSON);
    generated["specifications"] = json!([]);
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);

    let mut generated = value(GENERATED_JSON);
    generated["specifications"][0]["path"] = json!("standards/requirements/other.md");
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn duplicate_specification_digest_is_rejected() {
    let targets = admitted_targets();
    let mut generated = value(GENERATED_JSON);
    let first = generated["specifications"][0].clone();
    generated["specifications"]
        .as_array_mut()
        .expect("specifications must be an array")
        .push(first);
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn missing_reviewed_specification_projection_is_rejected() {
    let targets = admitted_targets();
    let policy = policy();
    let contract = &targets.contracts[0];
    let mut generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");
    generated.specifications.clear();
    let issues = expect_rejected(project_contract(contract, &generated, &policy));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);

    let mut generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");
    let first = generated.specifications[0].clone();
    generated.specifications.push(first);
    let issues = expect_rejected(project_contract(contract, &generated, &policy));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn unexpected_generated_sources_is_rejected() {
    let targets = admitted_targets();
    let contract = &targets.contracts[0];
    let reviewed_id = contract.targets[1].requirement.requirement_id.clone();

    let mut generated = value(GENERATED_JSON);
    generated["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(json!({
            "requirement_id": reviewed_id,
            "sources": [{ "locator": "src/lib.rs", "blake3": "a".repeat(64) }],
        }));
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);

    let mut generated = value(GENERATED_JSON);
    let first = generated["entries"][0].clone();
    generated["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(first);
    let issues = expect_rejected(admit_generated_sources(&generated, &targets, &policy()));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.evidence_job]
#[test]
fn generated_sources_admit_every_contract() {
    let targets = multi_contract_targets();
    let generated_value = multi_contract_generated();
    let generated = admit_generated_sources(&generated_value, &targets, &policy())
        .expect("one entry per contract must be admitted");
    assert_eq!(generated.entries.len(), 2);

    let mut missing = generated_value.clone();
    missing["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .pop();
    let issues = expect_rejected(admit_generated_sources(&missing, &targets, &policy()));
    assert_code(&issues, ISSUE_MISSING_GENERATED_SOURCES);

    let mut unknown = multi_contract_generated();
    unknown["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(json!({
            "requirement_id": "octet.other.unlisted",
            "sources": [{ "locator": "x", "blake3": "c".repeat(64) }],
        }));
    let issues = expect_rejected(admit_generated_sources(&unknown, &targets, &policy()));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);

    let mut reviewed = multi_contract_generated();
    reviewed["entries"]
        .as_array_mut()
        .expect("entries must be an array")
        .push(json!({
            "requirement_id": "octet.other.reviewed_scope",
            "sources": [{ "locator": "x", "blake3": "d".repeat(64) }],
        }));
    let issues = expect_rejected(admit_generated_sources(&reviewed, &targets, &policy()));
    assert_code(&issues, ISSUE_UNEXPECTED_GENERATED_SOURCES);
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn unassessed_target_is_not_projected() {
    let targets = admitted_targets();
    let policy = policy();
    let generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");
    let mut contract = targets.contracts[0].clone();
    contract.targets[0].assurance = Assurance::Unassessed;
    let issues = expect_rejected(project_contract(&contract, &generated, &policy));
    assert_code(&issues, ISSUE_UNASSESSED_TARGET);
}

// r[verify cairn.assurance_targets.projection]
#[test]
fn projection_mismatch_is_detected() {
    let targets = admitted_targets();
    let policy = policy();
    let contract = &targets.contracts[0];
    let generated = admit_generated_sources(&value(GENERATED_JSON), &targets, &policy)
        .expect("golden generated record must be admitted");
    let projected =
        project_contract(contract, &generated, &policy).expect("golden targets must project");

    let mut weakened = projected.clone();
    weakened["clauses"][0]["disposition"] = json!("empirical_check");
    let issues = compare_projection(&projected, &weakened);
    assert_code(&issues, ISSUE_PROJECTION_MISMATCH);
    assert!(
        issues
            .iter()
            .any(|issue| issue.field_path == "clauses[0].disposition"),
        "mismatch must name the weakened clause field: {issues:?}"
    );

    let mut weakened = projected.clone();
    weakened["clauses"][0]["allowed_evidence_ids"] = json!([]);
    let issues = compare_projection(&projected, &weakened);
    assert_code(&issues, ISSUE_PROJECTION_MISMATCH);

    let mut weakened = projected.clone();
    weakened["contract_id"] = json!("octet.requirements.other");
    let issues = compare_projection(&projected, &weakened);
    assert_code(&issues, ISSUE_PROJECTION_MISMATCH);
    assert!(issues.iter().any(|issue| issue.field_path == "contract_id"));
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn producer_job_schemas_are_owner_scoped() {
    let policy = policy();
    assert_eq!(
        admitted_producer_job_schema(&policy, "Octet"),
        Some("octet-mantle-producer-job/v1")
    );
    assert_eq!(
        admitted_producer_job_schema(&policy, "Trellis"),
        Some("trellis-mantle-producer-job/v1")
    );
    assert_eq!(admitted_producer_job_schema(&policy, "unknown"), None);
}

// r[verify cairn.assurance_targets.contract]
#[test]
fn invalid_producer_job_maps_are_rejected() {
    let mut empty = value(POLICY_JSON);
    empty["schemas"]["producer_jobs"] = json!({});
    assert_code(
        &expect_rejected(parse_assurance_targets_policy(&empty)),
        ISSUE_INVALID_PRODUCER_JOBS,
    );

    let mut blank = value(POLICY_JSON);
    blank["schemas"]["producer_jobs"]["octet"] = json!("");
    assert_code(
        &expect_rejected(parse_assurance_targets_policy(&blank)),
        ISSUE_INVALID_PRODUCER_JOBS,
    );

    let mut duplicate = value(POLICY_JSON);
    duplicate["schemas"]["producer_jobs"]["OCTET"] = json!("octet-mantle-producer-job/v2");
    assert_code(
        &expect_rejected(parse_assurance_targets_policy(&duplicate)),
        ISSUE_INVALID_PRODUCER_JOBS,
    );
}
