//! Pure admission and projection for Nickel-authored assurance targets.
//!
//! Nickel owns the targets shape, the policy export and the projection input;
//! this module only admits the exported JSON, binds targets to
//! content-bound requirement registry rows and recomputes the Octet clause
//! mapping. It performs no I/O, reads no environment and starts no evaluator.
//!
//! Admission is not acceptance: a target shows only that a declared claim binds
//! one registry row and (for projected targets) one evidence job. Evidence
//! truth, producer honesty, verifier soundness and release eligibility remain
//! outside the claim.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::content_bound_requirement_registry::{
    ContentBoundRequirementRow, ContentBoundSpecificationIdentity,
};
use super::refinement_typed_dependencies::Assurance;

pub const ASSURANCE_TARGETS_SCHEMA_V1: &str = "cairn.assurance-targets.v1";
pub const ASSURANCE_TARGET_SOURCES_SCHEMA_V1: &str = "cairn.assurance-target-sources.v1";
pub const ASSURANCE_TARGETS_POLICY_SCHEMA_V1: &str = "cairn.assurance-targets-policy.v1";
pub const OCTET_CONTRACT_SCHEMA_V1: &str = "octet-requirement-contract/v1";

pub const ISSUE_UNKNOWN_SCHEMA: &str = "assurance_targets.unknown_schema";
pub const ISSUE_UNKNOWN_FIELD: &str = "assurance_targets.unknown_field";
pub const ISSUE_UNKNOWN_ASSURANCE: &str = "assurance_targets.unknown_assurance";
pub const ISSUE_UNASSESSED_TARGET: &str = "assurance_targets.unassessed_target";
pub const ISSUE_AUTHORED_HASH: &str = "assurance_targets.authored_hash";
pub const ISSUE_MISSING_EVIDENCE_JOB: &str = "assurance_targets.missing_evidence_job";
pub const ISSUE_MISSING_PROJECTION_FIELD: &str = "assurance_targets.missing_projection_field";
pub const ISSUE_STALE_ROW: &str = "assurance_targets.stale_row";
pub const ISSUE_DUPLICATE_REQUIREMENT: &str = "assurance_targets.duplicate_requirement";
pub const ISSUE_SPECIFICATION_MISMATCH: &str = "assurance_targets.specification_mismatch";
pub const ISSUE_BOUND_EXCEEDED: &str = "assurance_targets.bound_exceeded";
pub const ISSUE_MISSING_GENERATED_SOURCES: &str = "assurance_targets.missing_generated_sources";
pub const ISSUE_UNEXPECTED_GENERATED_SOURCES: &str =
    "assurance_targets.unexpected_generated_sources";
pub const ISSUE_PROJECTION_MISMATCH: &str = "assurance_targets.projection_mismatch";

/// Target keys a hand author must never supply; source and tool identity come
/// only from the generated source record.
const AUTHORED_TARGET_KEYS: &[&str] = &[
    "sources",
    "source_bindings",
    "source_ids",
    "policy_ids",
    "tool_identity_hashes",
    "artifact_hashes",
    "required_bindings",
];

/// A closed record rejected an input field, a value or a shape it does not
/// admit. `field_path` is a JSON-path-like location inside the input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceTargetIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetsFile {
    pub schema_version: String,
    pub contracts: Vec<TargetContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetContract {
    pub contract_id: String,
    pub specification: TargetSpecification,
    pub targets: Vec<AssuranceTarget>,
    pub assumptions: Vec<serde_json::Value>,
    pub exclusions: Vec<serde_json::Value>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpecification {
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceTarget {
    pub requirement: ContentBoundRequirementRow,
    pub assurance: Assurance,
    pub claim: String,
    pub informal_remainder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_job: Option<EvidenceJob>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_scope: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_classes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_controls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceJob {
    pub job_id: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSources {
    pub schema_version: String,
    pub specifications: Vec<SpecificationDigest>,
    pub entries: Vec<GeneratedSourceEntry>,
}

/// The plain BLAKE3 of one reviewed specification file at the revision. It is
/// not the registry's domain-separated hash: Octet verifies the plain digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecificationDigest {
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSourceEntry {
    pub requirement_id: String,
    pub sources: Vec<SourceBinding>,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub policy_ids: Vec<String>,
    #[serde(default)]
    pub tool_identity_hashes: Vec<String>,
    #[serde(default)]
    pub artifact_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub locator: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceTargetsPolicy {
    pub schema: String,
    pub targets_schema: String,
    pub sources_schema: String,
    pub case_schema: String,
    pub receipt_schema: String,
    pub assurances: Vec<String>,
    pub tiers: PolicyTiers,
    pub schemas: PolicySchemas,
    pub octet: PolicyOctet,
    pub mantle: PolicyMantle,
    pub bounds: PolicyBounds,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyTiers {
    pub bounded_check: String,
    pub checked_proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySchemas {
    pub registry: String,
    pub octet_contract: String,
    pub octet_evidence: String,
    pub octet_decision: String,
    pub octet_proof_cards: String,
    pub octet_formal_proof_envelope: String,
    pub octet_producer_job: String,
    pub mantle_observation: String,
    pub mantle_realization_receipt: String,
    pub mantle_store_info: String,
    pub mantle_build_report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyOctet {
    pub passing_aggregate: String,
    pub clause_states: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyMantle {
    pub realization_complete_status: String,
    pub realized_states: Vec<String>,
    pub proof_acceptance_relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBounds {
    pub max_contracts: usize,
    pub max_targets_per_contract: usize,
    pub max_registry_rows: usize,
    pub max_text_bytes: usize,
    pub max_list_items: usize,
    pub max_input_bytes: usize,
    pub max_document_bytes: usize,
    pub max_trusted_signers: usize,
}

/// Parse the checked policy export. Runtime commands never evaluate Nickel;
/// they pass the exported JSON here.
pub fn parse_assurance_targets_policy(
    value: &serde_json::Value,
) -> Result<AssuranceTargetsPolicy, Vec<AssuranceTargetIssue>> {
    let mut issues = Vec::new();
    let schema = value.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(ASSURANCE_TARGETS_POLICY_SCHEMA_V1) {
        push_issue(
            &mut issues,
            ISSUE_UNKNOWN_SCHEMA,
            "schema",
            "assurance targets policy must declare cairn.assurance-targets-policy.v1",
        );
    }
    match serde_json::from_value::<AssuranceTargetsPolicy>(value.clone()) {
        Ok(policy) => {
            if policy.targets_schema != ASSURANCE_TARGETS_SCHEMA_V1 {
                push_issue(
                    &mut issues,
                    ISSUE_UNKNOWN_SCHEMA,
                    "targets_schema",
                    "policy must name the supported assurance targets schema",
                );
            }
            if policy.sources_schema != ASSURANCE_TARGET_SOURCES_SCHEMA_V1 {
                push_issue(
                    &mut issues,
                    ISSUE_UNKNOWN_SCHEMA,
                    "sources_schema",
                    "policy must name the supported generated source schema",
                );
            }
            if issues.is_empty() {
                Ok(policy)
            } else {
                Err(issues)
            }
        }
        Err(error) => {
            issues.push(closed_record_issue(&error, "policy"));
            Err(issues)
        }
    }
}

/// Admit a targets file against the checked policy export.
///
/// Checks run in a fixed order: schema, authored binding/hash keys, declared
/// assurance vocabulary, closed-record parse, policy bounds, projection
/// prerequisites, duplicate requirements and per-contract specification
/// equality. Every defect is reported; no input falls back to a default.
pub fn admit_targets(
    value: &serde_json::Value,
    policy: &AssuranceTargetsPolicy,
) -> Result<TargetsFile, Vec<AssuranceTargetIssue>> {
    // r[impl cairn.assurance_targets.contract]
    let mut issues = Vec::new();

    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str);
    if schema != Some(policy.targets_schema.as_str()) {
        push_issue(
            &mut issues,
            ISSUE_UNKNOWN_SCHEMA,
            "schema_version",
            "targets file must declare the schema version exported by the policy",
        );
    }

    reject_authored_keys(value, &mut issues);
    reject_unadmitted_assurance_values(value, policy, &mut issues);

    match serde_json::from_value::<TargetsFile>(value.clone()) {
        Ok(targets) => {
            check_targets_bounds(&targets, policy, &mut issues);
            check_projection_prerequisites(&targets, &mut issues);
            check_duplicate_requirements(&targets, &mut issues);
            check_contract_specification_binding(&targets, &mut issues);
            if issues.is_empty() {
                Ok(targets)
            } else {
                Err(issues)
            }
        }
        Err(error) => {
            issues.push(closed_record_issue(&error, ""));
            Err(issues)
        }
    }
}

/// Bind each admitted target to exactly one registry row and each contract to
/// the specification identity of its rows. A mismatch is never repaired.
pub fn bind_targets_to_registry(
    targets: &TargetsFile,
    rows: &[ContentBoundRequirementRow],
    specifications: &[ContentBoundSpecificationIdentity],
) -> Vec<AssuranceTargetIssue> {
    // r[impl cairn.assurance_targets.row_binding]
    let mut issues = Vec::new();
    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        let specification_bound = specifications.iter().any(|identity| {
            identity.path == contract.specification.path
                && identity.content_blake3 == contract.specification.blake3
        });
        if !specification_bound {
            push_issue(
                &mut issues,
                ISSUE_SPECIFICATION_MISMATCH,
                &format!("contracts[{contract_index}].specification"),
                "contract specification is not a specification identity exported by the registry",
            );
        }
        for (target_index, target) in contract.targets.iter().enumerate() {
            let matches = rows.contains(&target.requirement);
            if !matches {
                push_issue(
                    &mut issues,
                    ISSUE_STALE_ROW,
                    &format!("contracts[{contract_index}].targets[{target_index}].requirement"),
                    "target requirement does not equal the registry row with the same requirement_id",
                );
            }
        }
    }
    issues
}

/// Admit the generated source record that supplies source bindings and the
/// optional required-binding lists for every projected target of the file.
pub fn admit_generated_sources(
    value: &serde_json::Value,
    targets: &TargetsFile,
    policy: &AssuranceTargetsPolicy,
) -> Result<GeneratedSources, Vec<AssuranceTargetIssue>> {
    // r[impl cairn.assurance_targets.evidence_job]
    let mut issues = Vec::new();

    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str);
    if schema != Some(policy.sources_schema.as_str()) {
        push_issue(
            &mut issues,
            ISSUE_UNKNOWN_SCHEMA,
            "schema_version",
            "generated source record must declare the schema version exported by the policy",
        );
    }

    match serde_json::from_value::<GeneratedSources>(value.clone()) {
        Ok(generated) => {
            check_generated_sources(&generated, targets, policy, &mut issues);
            if issues.is_empty() {
                Ok(generated)
            } else {
                Err(issues)
            }
        }
        Err(error) => {
            issues.push(closed_record_issue(&error, ""));
            Err(issues)
        }
    }
}

/// Recompute the Octet contract for the non-`reviewed` targets of one contract.
/// `reviewed` targets produce no clause.
pub fn project_contract(
    contract: &TargetContract,
    generated: &GeneratedSources,
    policy: &AssuranceTargetsPolicy,
) -> Result<serde_json::Value, Vec<AssuranceTargetIssue>> {
    // r[impl cairn.assurance_targets.projection]
    let mut issues = Vec::new();
    let mut clauses = Vec::new();

    for (target_index, target) in contract.targets.iter().enumerate() {
        let path = format!("contracts[0].targets[{target_index}]");
        let tier = match target.assurance {
            // `reviewed` targets are declared but not projected.
            Assurance::Reviewed => continue,
            Assurance::Unassessed => {
                push_issue(
                    &mut issues,
                    ISSUE_UNASSESSED_TARGET,
                    &format!("{path}.assurance"),
                    "unassessed is not an admissible assurance target",
                );
                continue;
            }
            Assurance::BoundedCheck => policy.tiers.bounded_check.clone(),
            Assurance::CheckedProof => policy.tiers.checked_proof.clone(),
        };
        let Some(entry) = generated
            .entries
            .iter()
            .find(|entry| entry.requirement_id == target.requirement.requirement_id)
        else {
            push_issue(
                &mut issues,
                ISSUE_MISSING_GENERATED_SOURCES,
                &format!("{path}.requirement.requirement_id"),
                "projected target has no generated source entry",
            );
            continue;
        };
        let Some(job) = target.evidence_job.as_ref() else {
            push_issue(
                &mut issues,
                ISSUE_MISSING_EVIDENCE_JOB,
                &format!("{path}.evidence_job"),
                "projected target must name exactly one evidence job",
            );
            continue;
        };
        if job.job_id.trim().is_empty() || job.definition.trim().is_empty() {
            push_issue(
                &mut issues,
                ISSUE_MISSING_EVIDENCE_JOB,
                &format!("{path}.evidence_job"),
                "evidence job must carry a non-empty job_id and definition",
            );
            continue;
        }
        let (Some(checked_scope), Some(evidence_ids), Some(evidence_classes), Some(negative)) = (
            target.checked_scope.as_ref(),
            target.evidence_ids.as_ref(),
            target.evidence_classes.as_ref(),
            target.negative_controls.as_ref(),
        ) else {
            push_issue(
                &mut issues,
                ISSUE_MISSING_PROJECTION_FIELD,
                &path,
                "projected target must carry checked_scope, evidence_ids, evidence_classes and negative_controls",
            );
            continue;
        };

        let mut clause = serde_json::Map::new();
        clause.insert(
            "clause_id".into(),
            serde_json::Value::String(target.requirement.requirement_id.clone()),
        );
        clause.insert(
            "statement".into(),
            serde_json::Value::String(target.claim.clone()),
        );
        clause.insert("required".into(), serde_json::Value::Bool(true));
        clause.insert(
            "disposition".into(),
            serde_json::Value::String(tier.clone()),
        );
        clause.insert(
            "expected_evidence_tier".into(),
            serde_json::Value::String(tier.clone()),
        );
        clause.insert(
            "checked_scope".into(),
            serde_json::to_value(checked_scope).unwrap_or(serde_json::Value::Array(Vec::new())),
        );
        clause.insert(
            "allowed_evidence_ids".into(),
            serde_json::to_value(evidence_ids).unwrap_or(serde_json::Value::Array(Vec::new())),
        );
        clause.insert(
            "allowed_source_requirement_ids".into(),
            serde_json::Value::Array(vec![serde_json::Value::String(
                target.requirement.requirement_id.clone(),
            )]),
        );
        clause.insert(
            "required_evidence_classes".into(),
            serde_json::to_value(evidence_classes).unwrap_or(serde_json::Value::Array(Vec::new())),
        );
        clause.insert(
            "required_negative_controls".into(),
            serde_json::to_value(negative).unwrap_or(serde_json::Value::Array(Vec::new())),
        );
        if let Some(provider) = target.provider.as_ref() {
            clause.insert(
                "required_provider_id".into(),
                serde_json::Value::String(provider.clone()),
            );
        }
        clause.insert(
            "required_bindings".into(),
            serde_json::json!({
                "source_ids": entry.source_ids,
                "policy_ids": entry.policy_ids,
                "tool_identity_hashes": entry.tool_identity_hashes,
                "artifact_hashes": entry.artifact_hashes,
                "checked_scope": [],
                "evidence_tier": tier,
            }),
        );
        clause.insert(
            "source_bindings".into(),
            serde_json::to_value(&entry.sources).unwrap_or(serde_json::Value::Array(Vec::new())),
        );
        clauses.push(serde_json::Value::Object(clause));
    }

    if !issues.is_empty() {
        return Err(issues);
    }

    // Octet verifies the plain BLAKE3 of the reviewed specification file, so the
    // projection copies the generated digest, never the registry's
    // domain-separated specification identity.
    let specification = {
        let mut matching = generated
            .specifications
            .iter()
            .filter(|specification| specification.path == contract.specification.path);
        match (matching.next(), matching.next()) {
            (Some(specification), None) => specification,
            (None, _) => {
                push_issue(
                    &mut issues,
                    ISSUE_MISSING_GENERATED_SOURCES,
                    "specifications",
                    "generated source record has no plain digest for the contract specification path",
                );
                return Err(issues);
            }
            (Some(_), Some(_)) => {
                push_issue(
                    &mut issues,
                    ISSUE_UNEXPECTED_GENERATED_SOURCES,
                    "specifications",
                    "generated source record has more than one digest for the contract specification path",
                );
                return Err(issues);
            }
        }
    };

    let mut contract_object = serde_json::Map::new();
    contract_object.insert(
        "schema_version".into(),
        serde_json::Value::String(OCTET_CONTRACT_SCHEMA_V1.into()),
    );
    contract_object.insert(
        "contract_id".into(),
        serde_json::Value::String(contract.contract_id.clone()),
    );
    contract_object.insert(
        "reviewed_specification".into(),
        serde_json::json!({
            "path": specification.path,
            "blake3": specification.blake3,
        }),
    );
    contract_object.insert("clauses".into(), serde_json::Value::Array(clauses));
    contract_object.insert(
        "assumptions".into(),
        serde_json::Value::Array(contract.assumptions.clone()),
    );
    contract_object.insert(
        "exclusions".into(),
        serde_json::Value::Array(contract.exclusions.clone()),
    );
    contract_object.insert("non_claims".into(), serde_json::json!(contract.non_claims));
    Ok(serde_json::Value::Object(contract_object))
}

/// Compare a supplied Octet contract with the recomputed projection. Empty
/// means equal; otherwise every differing top-level key, clause and clause
/// field is reported.
#[must_use]
pub fn compare_projection(
    expected: &serde_json::Value,
    supplied: &serde_json::Value,
) -> Vec<AssuranceTargetIssue> {
    // r[impl cairn.assurance_targets.projection]
    if expected == supplied {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let (Some(expected_object), Some(supplied_object)) =
        (expected.as_object(), supplied.as_object())
    else {
        push_issue(
            &mut issues,
            ISSUE_PROJECTION_MISMATCH,
            "$",
            "supplied projection is not a JSON object",
        );
        return issues;
    };

    let keys: BTreeSet<&String> = expected_object
        .keys()
        .chain(supplied_object.keys())
        .collect();
    for key in keys {
        match (expected_object.get(key), supplied_object.get(key)) {
            (Some(left), Some(right)) if left == right => {}
            (Some(serde_json::Value::Array(left)), Some(serde_json::Value::Array(right)))
                if key == "clauses" =>
            {
                compare_clauses(left, right, &mut issues);
            }
            (None, None) => {}
            _ => push_issue(
                &mut issues,
                ISSUE_PROJECTION_MISMATCH,
                key,
                "supplied value differs from the recomputed projection",
            ),
        }
    }
    issues
}

fn compare_clauses(
    expected: &[serde_json::Value],
    supplied: &[serde_json::Value],
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    for index in 0..expected.len().max(supplied.len()) {
        let path = format!("clauses[{index}]");
        match (expected.get(index), supplied.get(index)) {
            (Some(left), Some(right)) if left == right => {}
            (Some(serde_json::Value::Object(left)), Some(serde_json::Value::Object(right))) => {
                let keys: BTreeSet<&String> = left.keys().chain(right.keys()).collect();
                for key in keys {
                    if left.get(key) != right.get(key) {
                        push_issue(
                            issues,
                            ISSUE_PROJECTION_MISMATCH,
                            &format!("{path}.{key}"),
                            "supplied clause field differs from the recomputed projection",
                        );
                    }
                }
            }
            _ => push_issue(
                issues,
                ISSUE_PROJECTION_MISMATCH,
                &path,
                "supplied clause is missing or unexpected",
            ),
        }
    }
}

fn reject_authored_keys(value: &serde_json::Value, issues: &mut Vec<AssuranceTargetIssue>) {
    let Some(contracts) = value.get("contracts").and_then(serde_json::Value::as_array) else {
        return;
    };
    for (contract_index, contract) in contracts.iter().enumerate() {
        let Some(targets) = contract
            .get("targets")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for (target_index, target) in targets.iter().enumerate() {
            let Some(object) = target.as_object() else {
                continue;
            };
            for key in object.keys() {
                if AUTHORED_TARGET_KEYS.contains(&key.as_str()) || authored_hash_key(key) {
                    push_issue(
                        issues,
                        ISSUE_AUTHORED_HASH,
                        &format!("contracts[{contract_index}].targets[{target_index}].{key}"),
                        "targets must not carry authored source, tool, artifact or policy identities",
                    );
                }
            }
            if let Some(job) = object
                .get("evidence_job")
                .and_then(serde_json::Value::as_object)
            {
                for key in job.keys() {
                    if authored_hash_key(key) {
                        push_issue(
                            issues,
                            ISSUE_AUTHORED_HASH,
                            &format!(
                                "contracts[{contract_index}].targets[{target_index}].evidence_job.{key}"
                            ),
                            "evidence jobs must not carry authored hash identities",
                        );
                    }
                }
            }
        }
    }
}

fn authored_hash_key(key: &str) -> bool {
    key.ends_with("_blake3") || key.ends_with("_hash") || key.ends_with("_hashes")
}

fn reject_unadmitted_assurance_values(
    value: &serde_json::Value,
    policy: &AssuranceTargetsPolicy,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    let Some(contracts) = value.get("contracts").and_then(serde_json::Value::as_array) else {
        return;
    };
    for (contract_index, contract) in contracts.iter().enumerate() {
        let Some(targets) = contract
            .get("targets")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for (target_index, target) in targets.iter().enumerate() {
            let path = format!("contracts[{contract_index}].targets[{target_index}].assurance");
            match target.get("assurance").and_then(serde_json::Value::as_str) {
                None => push_issue(
                    issues,
                    ISSUE_UNKNOWN_ASSURANCE,
                    &path,
                    "target must declare one assurance value from the policy",
                ),
                Some("unassessed") => push_issue(
                    issues,
                    ISSUE_UNASSESSED_TARGET,
                    &path,
                    "unassessed is not an admissible assurance target",
                ),
                Some(assurance) if !policy.assurances.iter().any(|value| value == assurance) => {
                    push_issue(
                        issues,
                        ISSUE_UNKNOWN_ASSURANCE,
                        &path,
                        "assurance value is not admitted by the policy",
                    );
                }
                Some(_) => {}
            }
        }
    }
}

fn check_targets_bounds(
    targets: &TargetsFile,
    policy: &AssuranceTargetsPolicy,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    let bounds = &policy.bounds;
    if targets.contracts.len() > bounds.max_contracts {
        push_issue(
            issues,
            ISSUE_BOUND_EXCEEDED,
            "contracts",
            "targets file exceeds the policy contract bound",
        );
    }
    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        let path = format!("contracts[{contract_index}]");
        check_text(
            &contract.contract_id,
            &format!("{path}.contract_id"),
            bounds.max_text_bytes,
            issues,
        );
        check_text(
            &contract.specification.path,
            &format!("{path}.specification.path"),
            bounds.max_text_bytes,
            issues,
        );
        check_text(
            &contract.specification.blake3,
            &format!("{path}.specification.blake3"),
            bounds.max_text_bytes,
            issues,
        );
        check_list_len(
            contract.assumptions.len(),
            &format!("{path}.assumptions"),
            bounds.max_list_items,
            issues,
        );
        check_list_len(
            contract.exclusions.len(),
            &format!("{path}.exclusions"),
            bounds.max_list_items,
            issues,
        );
        check_string_list(
            &contract.non_claims,
            &format!("{path}.non_claims"),
            bounds,
            issues,
        );
        if contract.targets.len() > bounds.max_targets_per_contract {
            push_issue(
                issues,
                ISSUE_BOUND_EXCEEDED,
                &format!("{path}.targets"),
                "contract exceeds the policy targets-per-contract bound",
            );
        }
        for (target_index, target) in contract.targets.iter().enumerate() {
            let target_path = format!("{path}.targets[{target_index}]");
            check_text(
                &target.claim,
                &format!("{target_path}.claim"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.informal_remainder,
                &format!("{target_path}.informal_remainder"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.requirement.requirement_id,
                &format!("{target_path}.requirement.requirement_id"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.requirement.specification_path,
                &format!("{target_path}.requirement.specification_path"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.requirement.specification_blake3,
                &format!("{target_path}.requirement.specification_blake3"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.requirement.requirement_body_blake3,
                &format!("{target_path}.requirement.requirement_body_blake3"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &target.requirement.row_blake3,
                &format!("{target_path}.requirement.row_blake3"),
                bounds.max_text_bytes,
                issues,
            );
            if let Some(job) = target.evidence_job.as_ref() {
                check_text(
                    &job.job_id,
                    &format!("{target_path}.evidence_job.job_id"),
                    bounds.max_text_bytes,
                    issues,
                );
                check_text(
                    &job.definition,
                    &format!("{target_path}.evidence_job.definition"),
                    bounds.max_text_bytes,
                    issues,
                );
            }
            if let Some(provider) = target.provider.as_ref() {
                check_text(
                    provider,
                    &format!("{target_path}.provider"),
                    bounds.max_text_bytes,
                    issues,
                );
            }
            check_optional_string_list(
                target.checked_scope.as_ref(),
                &format!("{target_path}.checked_scope"),
                bounds,
                issues,
            );
            check_optional_string_list(
                target.evidence_ids.as_ref(),
                &format!("{target_path}.evidence_ids"),
                bounds,
                issues,
            );
            check_optional_string_list(
                target.evidence_classes.as_ref(),
                &format!("{target_path}.evidence_classes"),
                bounds,
                issues,
            );
            check_optional_string_list(
                target.negative_controls.as_ref(),
                &format!("{target_path}.negative_controls"),
                bounds,
                issues,
            );
        }
    }
}

fn check_text(
    text: &str,
    field_path: &str,
    max_text_bytes: usize,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    if text.len() > max_text_bytes {
        push_issue(
            issues,
            ISSUE_BOUND_EXCEEDED,
            field_path,
            "text exceeds the policy byte bound",
        );
    }
}

fn check_list_len(
    length: usize,
    field_path: &str,
    max_list_items: usize,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    if length > max_list_items {
        push_issue(
            issues,
            ISSUE_BOUND_EXCEEDED,
            field_path,
            "list exceeds the policy item bound",
        );
    }
}

fn check_string_list(
    values: &[String],
    field_path: &str,
    bounds: &PolicyBounds,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    check_list_len(values.len(), field_path, bounds.max_list_items, issues);
    for (index, value) in values.iter().enumerate() {
        check_text(
            value,
            &format!("{field_path}[{index}]"),
            bounds.max_text_bytes,
            issues,
        );
    }
}

fn check_optional_string_list(
    values: Option<&Vec<String>>,
    field_path: &str,
    bounds: &PolicyBounds,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    if let Some(values) = values {
        check_string_list(values, field_path, bounds, issues);
    }
}

fn check_projection_prerequisites(targets: &TargetsFile, issues: &mut Vec<AssuranceTargetIssue>) {
    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        for (target_index, target) in contract.targets.iter().enumerate() {
            if target.assurance == Assurance::Reviewed {
                continue;
            }
            let path = format!("contracts[{contract_index}].targets[{target_index}]");
            match target.evidence_job.as_ref() {
                None => push_issue(
                    issues,
                    ISSUE_MISSING_EVIDENCE_JOB,
                    &format!("{path}.evidence_job"),
                    "projected target must name exactly one evidence job",
                ),
                Some(job) if job.job_id.trim().is_empty() || job.definition.trim().is_empty() => {
                    push_issue(
                        issues,
                        ISSUE_MISSING_EVIDENCE_JOB,
                        &format!("{path}.evidence_job"),
                        "evidence job must carry a non-empty job_id and definition",
                    );
                }
                Some(_) => {}
            }
            for (field, present) in [
                (
                    "checked_scope",
                    target
                        .checked_scope
                        .as_ref()
                        .is_some_and(|list| !list.is_empty()),
                ),
                (
                    "evidence_ids",
                    target
                        .evidence_ids
                        .as_ref()
                        .is_some_and(|list| !list.is_empty()),
                ),
                (
                    "evidence_classes",
                    target
                        .evidence_classes
                        .as_ref()
                        .is_some_and(|list| !list.is_empty()),
                ),
                ("negative_controls", target.negative_controls.is_some()),
            ] {
                if !present {
                    push_issue(
                        issues,
                        ISSUE_MISSING_PROJECTION_FIELD,
                        &format!("{path}.{field}"),
                        "projected target must carry this field (non-empty except negative_controls)",
                    );
                }
            }
        }
    }
}

fn check_duplicate_requirements(targets: &TargetsFile, issues: &mut Vec<AssuranceTargetIssue>) {
    let mut seen = BTreeSet::new();
    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        for (target_index, target) in contract.targets.iter().enumerate() {
            if !seen.insert(target.requirement.requirement_id.clone()) {
                push_issue(
                    issues,
                    ISSUE_DUPLICATE_REQUIREMENT,
                    &format!("contracts[{contract_index}].targets[{target_index}].requirement"),
                    "requirement_id must identify exactly one target in the file",
                );
            }
        }
    }
}

fn check_contract_specification_binding(
    targets: &TargetsFile,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        for (target_index, target) in contract.targets.iter().enumerate() {
            if target.requirement.specification_path != contract.specification.path
                || target.requirement.specification_blake3 != contract.specification.blake3
            {
                push_issue(
                    issues,
                    ISSUE_SPECIFICATION_MISMATCH,
                    &format!("contracts[{contract_index}].targets[{target_index}].requirement"),
                    "target row specification does not equal its contract specification",
                );
            }
        }
    }
}

fn check_generated_sources(
    generated: &GeneratedSources,
    targets: &TargetsFile,
    policy: &AssuranceTargetsPolicy,
    issues: &mut Vec<AssuranceTargetIssue>,
) {
    let bounds = &policy.bounds;
    check_list_len(
        generated.entries.len(),
        "entries",
        bounds.max_registry_rows,
        issues,
    );
    for (index, entry) in generated.entries.iter().enumerate() {
        let path = format!("entries[{index}]");
        check_text(
            &entry.requirement_id,
            &format!("{path}.requirement_id"),
            bounds.max_text_bytes,
            issues,
        );
        check_list_len(
            entry.sources.len(),
            &format!("{path}.sources"),
            bounds.max_list_items,
            issues,
        );
        for (source_index, source) in entry.sources.iter().enumerate() {
            check_text(
                &source.locator,
                &format!("{path}.sources[{source_index}].locator"),
                bounds.max_text_bytes,
                issues,
            );
            check_text(
                &source.blake3,
                &format!("{path}.sources[{source_index}].blake3"),
                bounds.max_text_bytes,
                issues,
            );
        }
        check_string_list(
            &entry.source_ids,
            &format!("{path}.source_ids"),
            bounds,
            issues,
        );
        check_string_list(
            &entry.policy_ids,
            &format!("{path}.policy_ids"),
            bounds,
            issues,
        );
        check_string_list(
            &entry.tool_identity_hashes,
            &format!("{path}.tool_identity_hashes"),
            bounds,
            issues,
        );
        check_string_list(
            &entry.artifact_hashes,
            &format!("{path}.artifact_hashes"),
            bounds,
            issues,
        );
    }

    check_list_len(
        generated.specifications.len(),
        "specifications",
        bounds.max_list_items,
        issues,
    );
    for (index, specification) in generated.specifications.iter().enumerate() {
        let path = format!("specifications[{index}]");
        check_text(
            &specification.path,
            &format!("{path}.path"),
            bounds.max_text_bytes,
            issues,
        );
        check_text(
            &specification.blake3,
            &format!("{path}.blake3"),
            bounds.max_text_bytes,
            issues,
        );
    }
    let contract_specification_paths: BTreeSet<&str> = targets
        .contracts
        .iter()
        .map(|contract| contract.specification.path.as_str())
        .collect();
    for path in &contract_specification_paths {
        let matching = generated
            .specifications
            .iter()
            .filter(|specification| specification.path == *path)
            .count();
        if matching == 0 {
            push_issue(
                issues,
                ISSUE_MISSING_GENERATED_SOURCES,
                "specifications",
                "generated source record has no plain digest for a contract specification path",
            );
        } else if matching > 1 {
            push_issue(
                issues,
                ISSUE_UNEXPECTED_GENERATED_SOURCES,
                "specifications",
                "generated source record has more than one digest for a contract specification path",
            );
        }
    }
    for (index, specification) in generated.specifications.iter().enumerate() {
        if !contract_specification_paths.contains(specification.path.as_str()) {
            push_issue(
                issues,
                ISSUE_UNEXPECTED_GENERATED_SOURCES,
                &format!("specifications[{index}].path"),
                "generated source record has a plain digest for an unknown specification path",
            );
        }
    }

    for (contract_index, contract) in targets.contracts.iter().enumerate() {
        for (target_index, target) in contract.targets.iter().enumerate() {
            let path = format!(
                "contracts[{contract_index}].targets[{target_index}].requirement.requirement_id"
            );
            let matching: Vec<&GeneratedSourceEntry> = generated
                .entries
                .iter()
                .filter(|entry| entry.requirement_id == target.requirement.requirement_id)
                .collect();
            if target.assurance == Assurance::Reviewed {
                if !matching.is_empty() {
                    push_issue(
                        issues,
                        ISSUE_UNEXPECTED_GENERATED_SOURCES,
                        &path,
                        "reviewed target must not have a generated source entry",
                    );
                }
                continue;
            }
            if matching.is_empty() {
                push_issue(
                    issues,
                    ISSUE_MISSING_GENERATED_SOURCES,
                    &path,
                    "projected target needs exactly one generated source entry",
                );
            } else if matching.len() > 1 {
                push_issue(
                    issues,
                    ISSUE_UNEXPECTED_GENERATED_SOURCES,
                    &path,
                    "projected target has more than one generated source entry",
                );
            } else if matching[0].sources.is_empty() {
                push_issue(
                    issues,
                    ISSUE_MISSING_GENERATED_SOURCES,
                    &path,
                    "generated source entry must carry at least one source binding",
                );
            }
        }
    }

    for (index, entry) in generated.entries.iter().enumerate() {
        let known = targets.contracts.iter().any(|contract| {
            contract.targets.iter().any(|target| {
                target.requirement.requirement_id == entry.requirement_id
                    && target.assurance != Assurance::Reviewed
            })
        });
        if !known {
            push_issue(
                issues,
                ISSUE_UNEXPECTED_GENERATED_SOURCES,
                &format!("entries[{index}].requirement_id"),
                "generated source entry does not name a projected target of this contract",
            );
        }
    }
}

fn closed_record_issue(error: &serde_json::Error, context: &str) -> AssuranceTargetIssue {
    let message = error.to_string();
    let field_path = if let Some(start) = message.find('`') {
        match message[start + 1..].find('`') {
            Some(relative_end) => {
                let field = &message[start + 1..start + 1 + relative_end];
                if context.is_empty() {
                    field.to_string()
                } else {
                    format!("{context}.{field}")
                }
            }
            None => context.to_string(),
        }
    } else {
        context.to_string()
    };
    issue(
        ISSUE_UNKNOWN_FIELD,
        &field_path,
        &format!("closed record rejected the input: {message}"),
    )
}

fn issue(code: &str, field_path: &str, message: &str) -> AssuranceTargetIssue {
    AssuranceTargetIssue {
        code: code.into(),
        field_path: field_path.into(),
        message: message.into(),
    }
}

fn push_issue(issues: &mut Vec<AssuranceTargetIssue>, code: &str, field_path: &str, message: &str) {
    issues.push(issue(code, field_path, message));
}

#[cfg(test)]
mod tests;
