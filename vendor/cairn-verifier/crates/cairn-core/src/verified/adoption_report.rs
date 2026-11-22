//! Pure adoption-stage composition over explicitly supplied snapshots.
//!
//! <!-- r[impl cairn.adoption_report.boundary] -->
//! No sibling scan, transport, pin update, check execution, or mutation
//! happens here: this module only joins supplied facts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::evidence::canonical_json_hash;

pub const ADOPTION_SNAPSHOT_SCHEMA_V1: &str = "cairn.adoption.snapshot.v1";
pub const ADOPTION_REPORT_SCHEMA_V1: &str = "cairn.adoption.report.v1";
pub const ADOPTION_STAGE_UPSTREAM_LIFECYCLE: &str = "upstream_lifecycle";
pub const ADOPTION_STAGE_CONSUMER_PIN: &str = "consumer_pin";
pub const ADOPTION_STAGE_CONSUMER_COMPATIBILITY: &str = "consumer_compatibility";
pub const ADOPTION_STAGE_INTEGRATION_EVIDENCE: &str = "integration_evidence";
pub const ADOPTION_ACTION_AUTHORITY: &str = "advisory";
pub const ADOPTION_ACTION_KIND_EVIDENCE_REFRESH: &str = "evidence_refresh";
pub const ADOPTION_ACTION_KIND_DIAGNOSTIC_INSPECTION: &str = "diagnostic_inspection";
pub const ADOPTION_ACTION_KIND_UNRESOLVED: &str = "unresolved";
pub const ADOPTION_BOUND_MAX_RELATIONSHIPS: usize = 512;
pub const ADOPTION_BOUND_MAX_OBSERVATIONS: usize = 4_096;
pub const ADOPTION_BOUND_MAX_PROVENANCE: usize = 128;
pub const ADOPTION_STACK_WORKSPACE_TOOLS_REVISION: &str =
    "22515fb7bc01d378312053d389d4bbdd55637a50";
pub const STACK_DEPS_CATALOG_SCHEMA_V2: &str = "onixresearch.stack-dependency-catalog.v2";
pub const STACK_DEPS_RECEIPT_SCHEMA_V1: &str = "onixresearch.stack-dependency-receipt.v1";
pub const STACK_DEPS_RECONCILIATION_SCHEMA_V2: &str =
    "onixresearch.stack-dependency-reconciliation.v2";
pub const STACK_DOCTOR_HEALTH_SCHEMA_V1: &str = "onixresearch.stack-health-receipt.v1";
pub const CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1: &str = "cairn.cross-repo-receipt-summary.v1";
pub const CAIRN_GLOBAL_REGISTRY_SCHEMA_V1: &str = "cairn.global-registry-snapshot.v1";
pub const VALENCE_EVIDENCE_ADMISSION_SCHEMA_V1: &str = "valence.evidence-admission.v1";
const RELATIONSHIP_BOUND_EXHAUSTED: &str = "relationships";
const OBSERVATION_BOUND_EXHAUSTED: &str = "observations";
const PROVENANCE_BOUND_EXHAUSTED: &str = "provenance";
const STACK_DEPS_PRODUCER_KIND: &str = "stack_deps_audit";
const STACK_DOCTOR_PRODUCER_KIND: &str = "stack_doctor_health";
const CAIRN_EVIDENCE_PRODUCER_KIND: &str = "cairn_adoption_evidence";
const STACK_DEPS_OPERATION_AUDIT: &str = "audit";
const STACK_DEPS_CLAIM_OBSERVED: &str = "observed-dependency-state";
const STACK_DOCTOR_CLAIM_OBSERVED: &str = "observed-stack-reference-state";
const STACK_DOCTOR_CLAIM_CONFORMANCE: &str = "ran-declared-conformance-checks";
const STACK_DOCTOR_CHECKED_SCOPE_COUNT: usize = 5;
const HASH_HEX_LENGTH: usize = 64;
const GIT_SHA1_HEX_LENGTH: usize = 40;
const GIT_SHA256_HEX_LENGTH: usize = 64;
const STACK_DEPS_NON_CLAIMS: [&str; 4] = [
    "does-not-prove-upstream-correctness",
    "does-not-prove-semantic-compatibility",
    "does-not-prove-release-readiness",
    "does-not-prove-whole-stack-correctness",
];
const STACK_DOCTOR_NON_CLAIMS: [&str; 4] = [
    "Stack health does not prove child repository correctness.",
    "Revision equality does not prove semantic compatibility.",
    "A complete cohort does not create a shared release unit.",
    "A clean report does not prove release readiness, deployment success, or whole-stack correctness.",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionStageKind {
    UpstreamLifecycle,
    ConsumerPin,
    ConsumerCompatibility,
    IntegrationEvidence,
}

// r[impl cairn.adoption_report.unknowns]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionStageState {
    Supported,
    Gap,
    Missing,
    Stale,
    Unavailable,
    Blocked,
    Diverged,
    Conflict,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionRelationshipKey {
    pub relationship_id: String,
    pub upstream_repository: String,
    pub upstream_change: String,
    pub consumer_repository: String,
    pub required_revision: String,
    pub subject_id: String,
    pub policy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionQueryBounds {
    pub max_relationships: usize,
    pub max_observations: usize,
    pub max_provenance: usize,
}

impl Default for AdoptionQueryBounds {
    fn default() -> Self {
        Self {
            max_relationships: ADOPTION_BOUND_MAX_RELATIONSHIPS,
            max_observations: ADOPTION_BOUND_MAX_OBSERVATIONS,
            max_provenance: ADOPTION_BOUND_MAX_PROVENANCE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionQuery {
    pub scope_id: String,
    pub relationships: Vec<AdoptionRelationshipKey>,
    pub bounds: AdoptionQueryBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionProducerProvenance {
    pub producer_id: String,
    pub schema: String,
    pub source_revision: String,
    pub tool_identity: String,
    pub policy_identity: String,
    pub observation_scope: String,
    pub receipt_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionStageObservation {
    pub relationship: AdoptionRelationshipKey,
    pub stage: AdoptionStageKind,
    pub state: AdoptionStageState,
    pub producer_id: String,
    pub producer_status: String,
    pub evidence_hashes: Vec<String>,
    pub observed_revision: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionCoverage {
    pub partial: bool,
    pub selected_relationships: usize,
    pub observed_relationships: usize,
    pub omitted_relationships: Vec<String>,
    pub bound_exhausted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionSnapshot {
    pub schema: String,
    pub query: AdoptionQuery,
    pub provenance: Vec<AdoptionProducerProvenance>,
    pub observations: Vec<AdoptionStageObservation>,
    pub coverage: AdoptionCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionStageReport {
    pub stage: AdoptionStageKind,
    pub state: AdoptionStageState,
    pub producer_statuses: Vec<String>,
    pub evidence_hashes: Vec<String>,
    pub observed_revisions: Vec<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionRelationshipReport {
    pub relationship: AdoptionRelationshipKey,
    pub complete: bool,
    pub stages: Vec<AdoptionStageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionAction {
    pub id: String,
    pub kind: String,
    pub authority: String,
    pub owner_repository: String,
    pub relationship_id: String,
    pub stage: AdoptionStageKind,
    pub prerequisites: Vec<String>,
    pub expected_observation: String,
    pub effect_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionReport {
    pub schema: String,
    pub valid: bool,
    pub complete: bool,
    pub scope_id: String,
    pub input_hash: String,
    pub report_hash: String,
    pub coverage: AdoptionCoverage,
    pub provenance: Vec<AdoptionProducerProvenance>,
    pub relationships: Vec<AdoptionRelationshipReport>,
    pub actions: Vec<AdoptionAction>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionProducerCohort {
    pub producer_id: String,
    pub kind: String,
    pub supported_schemas: Vec<String>,
    pub source_revision: String,
    pub validation_evidence: String,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionProducerAdmission {
    pub valid: bool,
    pub provenance: AdoptionProducerProvenance,
    pub observations: Vec<AdoptionStageObservation>,
    pub diagnostics: Vec<String>,
}

pub struct StackDepsContractInput<'a> {
    pub catalog: &'a Value,
    pub audit_receipt: &'a Value,
    pub reconciliation: &'a Value,
}

pub struct CairnAdoptionEvidenceInput<'a> {
    pub lifecycle_receipts: &'a [Value],
    pub completion_receipts: &'a [Value],
    pub registry_snapshot: Option<&'a Value>,
    pub valence_admission: Option<&'a Value>,
}

#[must_use]
pub fn adoption_stage_kinds() -> [AdoptionStageKind; 4] {
    [
        AdoptionStageKind::UpstreamLifecycle,
        AdoptionStageKind::ConsumerPin,
        AdoptionStageKind::ConsumerCompatibility,
        AdoptionStageKind::IntegrationEvidence,
    ]
}

#[must_use]
// r[impl cairn.adoption_report.inputs]
pub fn admit_stack_deps_contracts(
    query: &AdoptionQuery,
    input: &StackDepsContractInput<'_>,
    cohort: &AdoptionProducerCohort,
) -> AdoptionProducerAdmission {
    // Pure saved-input producer admission; no producer execution or sibling scan.
    let mut diagnostics = validate_stack_deps_cohort(cohort);
    let catalog_relationships = catalog_relationships(input.catalog, &mut diagnostics);
    validate_stack_deps_audit(
        input.audit_receipt,
        &catalog_relationships,
        &mut diagnostics,
    );
    validate_stack_deps_reconciliation(
        input.reconciliation,
        input.catalog,
        input.audit_receipt,
        &mut diagnostics,
    );

    let receipt_hash = string_field(input.audit_receipt, "receipt_blake3").unwrap_or_default();
    let observations = if diagnostics.is_empty() {
        stack_deps_observations(query, input.audit_receipt, &catalog_relationships, cohort)
    } else {
        Vec::new()
    };
    producer_admission(
        cohort,
        STACK_DEPS_RECEIPT_SCHEMA_V1,
        "stack-deps",
        &receipt_hash,
        observations,
        diagnostics,
    )
}

#[must_use]
pub fn admit_stack_doctor_health(
    query: &AdoptionQuery,
    receipt: &Value,
    cohort: &AdoptionProducerCohort,
) -> AdoptionProducerAdmission {
    // Pure saved-input producer admission; no producer execution or sibling scan.
    let mut diagnostics = validate_stack_doctor_cohort(cohort);
    validate_stack_doctor_receipt(receipt, &mut diagnostics);
    let receipt_hash = string_field(receipt, "receipt_blake3").unwrap_or_default();
    let observations = if diagnostics.is_empty() {
        stack_doctor_observations(query, receipt, cohort)
    } else {
        Vec::new()
    };
    producer_admission(
        cohort,
        STACK_DOCTOR_HEALTH_SCHEMA_V1,
        "stack-doctor",
        &receipt_hash,
        observations,
        diagnostics,
    )
}

#[must_use]
pub fn admit_cairn_adoption_evidence(
    query: &AdoptionQuery,
    input: &CairnAdoptionEvidenceInput<'_>,
    cohort: &AdoptionProducerCohort,
) -> AdoptionProducerAdmission {
    // Pure saved-input evidence composition; registry membership is provenance only.
    let mut diagnostics = validate_cairn_evidence_cohort(cohort);
    let mut observations = Vec::new();
    for receipt in input.lifecycle_receipts {
        validate_cross_repo_summary(receipt, &mut diagnostics);
        observations.extend(cairn_lifecycle_observations(query, receipt, cohort));
    }
    for receipt in input.completion_receipts {
        validate_cross_repo_summary(receipt, &mut diagnostics);
        observations.extend(cairn_completion_observations(query, receipt, cohort));
    }
    if let Some(registry) = input.registry_snapshot {
        validate_adoption_registry(registry, &mut diagnostics);
    }
    if let Some(valence) = input.valence_admission {
        validate_valence_admission(valence, &mut diagnostics);
        observations.extend(valence_admission_observations(query, valence, cohort));
    }
    if !diagnostics.is_empty() {
        observations.clear();
    }
    producer_admission(
        cohort,
        CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1,
        "cairn-adoption-evidence",
        &canonical_json_hash(&json!({
            "lifecycle": input.lifecycle_receipts,
            "completion": input.completion_receipts,
            "registry": input.registry_snapshot,
            "valence": input.valence_admission,
        })),
        observations,
        diagnostics,
    )
}

#[must_use]
// r[impl cairn.adoption_report.stages]
pub fn derive_adoption_report(snapshot: &AdoptionSnapshot) -> AdoptionReport {
    let mut diagnostics = validate_snapshot(snapshot);
    let mut relationships = snapshot.query.relationships.clone();
    relationships.sort();
    relationships.dedup();
    let original_relationship_count = relationships.len();
    let max_relationships = nonzero_bound(snapshot.query.bounds.max_relationships);
    if relationships.len() > max_relationships {
        relationships.truncate(max_relationships);
        diagnostics.push("adoption.bound.relationships_exhausted".to_string());
    }

    let relationship_set = relationships.iter().cloned().collect::<BTreeSet<_>>();
    let mut observations = snapshot
        .observations
        .iter()
        .filter(|observation| relationship_set.contains(&observation.relationship))
        .cloned()
        .collect::<Vec<_>>();
    observations.sort_by_key(observation_sort_key);
    observations.dedup();
    let max_observations = nonzero_bound(snapshot.query.bounds.max_observations);
    if observations.len() > max_observations {
        observations.truncate(max_observations);
        diagnostics.push("adoption.bound.observations_exhausted".to_string());
    }

    let mut grouped: BTreeMap<AdoptionRelationshipKey, Vec<AdoptionStageObservation>> =
        BTreeMap::new();
    for observation in observations {
        grouped
            .entry(observation.relationship.clone())
            .or_default()
            .push(observation);
    }

    let reports = relationships
        .into_iter()
        .map(|relationship| {
            let group = grouped.get(&relationship).cloned().unwrap_or_default();
            relationship_report(relationship, &group)
        })
        .collect::<Vec<_>>();

    let mut coverage = snapshot.coverage.clone();
    coverage.selected_relationships = reports.len();
    coverage.observed_relationships = grouped.len();
    if original_relationship_count > reports.len() {
        coverage.partial = true;
        coverage
            .bound_exhausted
            .push(RELATIONSHIP_BOUND_EXHAUSTED.to_string());
    }
    if snapshot.observations.len() > max_observations {
        coverage.partial = true;
        coverage
            .bound_exhausted
            .push(OBSERVATION_BOUND_EXHAUSTED.to_string());
    }
    if snapshot.provenance.len() > snapshot.query.bounds.max_provenance {
        coverage.partial = true;
        coverage
            .bound_exhausted
            .push(PROVENANCE_BOUND_EXHAUSTED.to_string());
    }
    coverage.bound_exhausted.sort();
    coverage.bound_exhausted.dedup();
    coverage.omitted_relationships.sort();
    coverage.omitted_relationships.dedup();

    let mut provenance = snapshot.provenance.clone();
    provenance.sort_by_key(provenance_sort_key);
    provenance.dedup();
    let max_provenance = nonzero_bound(snapshot.query.bounds.max_provenance);
    if provenance.len() > max_provenance {
        provenance.truncate(max_provenance);
    }

    let mut actions = reports
        .iter()
        .flat_map(actions_for_relationship)
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| left.id.cmp(&right.id));
    actions.dedup_by(|left, right| left.id == right.id);

    let valid = diagnostics.is_empty();
    let complete = valid && reports.iter().all(|row| row.complete) && !coverage.partial;
    let input_hash = canonical_json_hash(&snapshot_fingerprint_value(snapshot));
    let mut report = AdoptionReport {
        schema: ADOPTION_REPORT_SCHEMA_V1.to_string(),
        valid,
        complete,
        scope_id: snapshot.query.scope_id.clone(),
        input_hash,
        report_hash: String::new(),
        coverage,
        provenance,
        relationships: reports,
        actions,
        diagnostics,
    };
    report.report_hash = report_fingerprint(&report);
    report
}

fn validate_stack_deps_cohort(cohort: &AdoptionProducerCohort) -> Vec<String> {
    let mut diagnostics = validate_common_cohort(cohort, STACK_DEPS_PRODUCER_KIND);
    require_schema(cohort, STACK_DEPS_CATALOG_SCHEMA_V2, &mut diagnostics);
    require_schema(cohort, STACK_DEPS_RECEIPT_SCHEMA_V1, &mut diagnostics);
    require_schema(
        cohort,
        STACK_DEPS_RECONCILIATION_SCHEMA_V2,
        &mut diagnostics,
    );
    require_non_claims(cohort, &STACK_DEPS_NON_CLAIMS, &mut diagnostics);
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn validate_stack_doctor_cohort(cohort: &AdoptionProducerCohort) -> Vec<String> {
    let mut diagnostics = validate_common_cohort(cohort, STACK_DOCTOR_PRODUCER_KIND);
    require_schema(cohort, STACK_DOCTOR_HEALTH_SCHEMA_V1, &mut diagnostics);
    require_non_claims(cohort, &STACK_DOCTOR_NON_CLAIMS, &mut diagnostics);
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn validate_cairn_evidence_cohort(cohort: &AdoptionProducerCohort) -> Vec<String> {
    let mut diagnostics = validate_common_cohort(cohort, CAIRN_EVIDENCE_PRODUCER_KIND);
    require_schema(cohort, CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1, &mut diagnostics);
    require_schema(cohort, CAIRN_GLOBAL_REGISTRY_SCHEMA_V1, &mut diagnostics);
    require_schema(
        cohort,
        VALENCE_EVIDENCE_ADMISSION_SCHEMA_V1,
        &mut diagnostics,
    );
    if cohort.required_non_claims.iter().any(|claim| {
        claim.contains("authority")
            || claim.contains("release eligibility")
            || claim.contains("catalog")
    }) {
        diagnostics.push("adoption.cairn_evidence.non_claims_overclaim".to_string());
    }
    if cohort.required_non_claims.is_empty() {
        diagnostics.push("adoption.cairn_evidence.non_claims_missing".to_string());
    }
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn validate_common_cohort(cohort: &AdoptionProducerCohort, expected_kind: &str) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if cohort.producer_id.is_empty() {
        diagnostics.push("adoption.producer.id_missing".to_string());
    }
    if cohort.kind != expected_kind {
        diagnostics.push("adoption.producer.kind_unsupported".to_string());
    }
    if cohort.source_revision != ADOPTION_STACK_WORKSPACE_TOOLS_REVISION {
        diagnostics.push("adoption.producer.source_revision_mismatch".to_string());
    }
    if !is_blake3_hex(&cohort.validation_evidence) {
        diagnostics.push("adoption.producer.validation_evidence_invalid".to_string());
    }
    diagnostics
}

fn require_schema(cohort: &AdoptionProducerCohort, schema: &str, diagnostics: &mut Vec<String>) {
    if !cohort.supported_schemas.iter().any(|item| item == schema) {
        diagnostics.push(format!("adoption.producer.schema_missing:{schema}"));
    }
}

fn require_non_claims(
    cohort: &AdoptionProducerCohort,
    expected: &[&str],
    diagnostics: &mut Vec<String>,
) {
    let actual = cohort
        .required_non_claims
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if actual != expected {
        diagnostics.push("adoption.producer.non_claims_mismatch".to_string());
    }
}

fn catalog_relationships(
    catalog: &Value,
    diagnostics: &mut Vec<String>,
) -> BTreeMap<String, Value> {
    let mut relationships = BTreeMap::new();
    if string_field(catalog, "schema").as_deref() != Some(STACK_DEPS_CATALOG_SCHEMA_V2) {
        diagnostics.push("adoption.stack_deps.catalog_schema_unsupported".to_string());
    }
    let Some(items) = array_field(catalog, "relationships") else {
        diagnostics.push("adoption.stack_deps.catalog_relationships_missing".to_string());
        return relationships;
    };
    for item in items {
        let Some(id) = string_field(item, "id") else {
            diagnostics.push("adoption.stack_deps.catalog_relationship_id_missing".to_string());
            continue;
        };
        if relationships.insert(id.clone(), item.clone()).is_some() {
            diagnostics.push(format!(
                "adoption.stack_deps.catalog_relationship_duplicate:{id}"
            ));
        }
    }
    relationships
}

fn validate_stack_deps_audit(
    receipt: &Value,
    catalog_relationships: &BTreeMap<String, Value>,
    diagnostics: &mut Vec<String>,
) {
    if string_field(receipt, "schema").as_deref() != Some(STACK_DEPS_RECEIPT_SCHEMA_V1) {
        diagnostics.push("adoption.stack_deps.audit_schema_unsupported".to_string());
    }
    if string_field(receipt, "operation").as_deref() != Some(STACK_DEPS_OPERATION_AUDIT) {
        diagnostics.push("adoption.stack_deps.audit_operation_unsupported".to_string());
    }
    if !receipt.get("relationship").is_some_and(Value::is_null) {
        diagnostics.push("adoption.stack_deps.audit_relationship_must_be_null".to_string());
    }
    if !hash_field_is_blake3(receipt, "receipt_blake3") {
        diagnostics.push("adoption.stack_deps.audit_receipt_hash_invalid".to_string());
    }
    require_string_array_value(
        receipt,
        "claims",
        &[STACK_DEPS_CLAIM_OBSERVED],
        "adoption.stack_deps.audit_claims_mismatch",
        diagnostics,
    );
    require_string_array_value(
        receipt,
        "non_claims",
        &STACK_DEPS_NON_CLAIMS,
        "adoption.stack_deps.audit_non_claims_mismatch",
        diagnostics,
    );
    for empty_field in ["adapter_results", "check_results", "changed_paths"] {
        if !array_field(receipt, empty_field).is_some_and(Vec::is_empty) {
            diagnostics.push(format!("adoption.stack_deps.audit_{empty_field}_not_empty"));
        }
    }
    if !receipt.get("plan_blake3").is_some_and(Value::is_null) {
        diagnostics.push("adoption.stack_deps.audit_plan_hash_must_be_null".to_string());
    }
    let Some(observations) = array_field(receipt, "observations") else {
        diagnostics.push("adoption.stack_deps.audit_observations_missing".to_string());
        return;
    };
    if observations.len() != catalog_relationships.len() {
        diagnostics.push("adoption.stack_deps.audit_coverage_mismatch".to_string());
    }
    let mut observed = BTreeSet::new();
    for observation in observations {
        validate_stack_deps_observation(
            observation,
            catalog_relationships,
            &mut observed,
            diagnostics,
        );
    }
}

fn validate_stack_deps_observation(
    observation: &Value,
    catalog_relationships: &BTreeMap<String, Value>,
    observed: &mut BTreeSet<String>,
    diagnostics: &mut Vec<String>,
) {
    let Some(relationship_id) = string_field(observation, "relationship") else {
        diagnostics.push("adoption.stack_deps.observation_relationship_missing".to_string());
        return;
    };
    if !observed.insert(relationship_id.clone()) {
        diagnostics.push(format!(
            "adoption.stack_deps.observation_duplicate:{relationship_id}"
        ));
    }
    let Some(relationship) = catalog_relationships.get(&relationship_id) else {
        diagnostics.push(format!(
            "adoption.stack_deps.observation_unknown:{relationship_id}"
        ));
        return;
    };
    let expected_source = relationship
        .get("source")
        .and_then(|source| string_field(source, "source_identity"));
    if string_field(observation, "source_identity") != expected_source {
        diagnostics.push(format!(
            "adoption.stack_deps.observation_source_mismatch:{relationship_id}"
        ));
    }
    if !string_field(observation, "state")
        .as_deref()
        .is_some_and(is_stack_deps_state)
    {
        diagnostics.push(format!(
            "adoption.stack_deps.observation_state_unsupported:{relationship_id}"
        ));
    }
    for field in ["consumer_revision", "candidate_revision"] {
        if let Some(revision) = optional_string_field(observation, field) {
            if !is_exact_revision(&revision) {
                diagnostics.push(format!(
                    "adoption.stack_deps.observation_revision_invalid:{relationship_id}"
                ));
            }
        }
    }
}

fn validate_stack_deps_reconciliation(
    reconciliation: &Value,
    catalog: &Value,
    audit_receipt: &Value,
    diagnostics: &mut Vec<String>,
) {
    if string_field(reconciliation, "schema").as_deref()
        != Some(STACK_DEPS_RECONCILIATION_SCHEMA_V2)
    {
        diagnostics.push("adoption.stack_deps.reconciliation_schema_unsupported".to_string());
    }
    if !hash_field_is_blake3(reconciliation, "reconciliation_blake3") {
        diagnostics.push("adoption.stack_deps.reconciliation_hash_invalid".to_string());
    }
    let catalog_hash = canonical_json_hash(catalog);
    if string_field(reconciliation, "catalog_blake3").as_deref() != Some(catalog_hash.as_str()) {
        diagnostics.push("adoption.stack_deps.reconciliation_catalog_hash_mismatch".to_string());
    }
    if string_field(reconciliation, "audit_receipt_blake3")
        != string_field(audit_receipt, "receipt_blake3")
    {
        diagnostics.push("adoption.stack_deps.reconciliation_audit_hash_mismatch".to_string());
    }
    require_string_array_value(
        reconciliation,
        "non_claims",
        &STACK_DEPS_NON_CLAIMS,
        "adoption.stack_deps.reconciliation_non_claims_mismatch",
        diagnostics,
    );
}

fn validate_stack_doctor_receipt(receipt: &Value, diagnostics: &mut Vec<String>) {
    if string_field(receipt, "schema").as_deref() != Some(STACK_DOCTOR_HEALTH_SCHEMA_V1) {
        diagnostics.push("adoption.stack_doctor.schema_unsupported".to_string());
    }
    if !matches!(
        string_field(receipt, "verdict").as_deref(),
        Some("pass" | "fail")
    ) {
        diagnostics.push("adoption.stack_doctor.verdict_unsupported".to_string());
    }
    for field in ["profile_blake3", "observations_blake3", "receipt_blake3"] {
        if !hash_field_is_blake3(receipt, field) {
            diagnostics.push(format!("adoption.stack_doctor.{field}_invalid"));
        }
    }
    if array_field(receipt, "checked_scopes").map_or(0, Vec::len)
        != STACK_DOCTOR_CHECKED_SCOPE_COUNT
    {
        diagnostics.push("adoption.stack_doctor.checked_scopes_incomplete".to_string());
    }
    require_string_array_value(
        receipt,
        "claims",
        &[STACK_DOCTOR_CLAIM_OBSERVED, STACK_DOCTOR_CLAIM_CONFORMANCE],
        "adoption.stack_doctor.claims_mismatch",
        diagnostics,
    );
    require_string_array_value(
        receipt,
        "non_claims",
        &STACK_DOCTOR_NON_CLAIMS,
        "adoption.stack_doctor.non_claims_mismatch",
        diagnostics,
    );
    if let Some(results) = array_field(receipt, "conformance_results") {
        for result in results {
            validate_stack_doctor_conformance(result, diagnostics);
        }
    } else {
        diagnostics.push("adoption.stack_doctor.conformance_results_missing".to_string());
    }
}

fn validate_cross_repo_summary(receipt: &Value, diagnostics: &mut Vec<String>) {
    if string_field(receipt, "schema").as_deref() != Some(CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1) {
        diagnostics.push("adoption.cairn_evidence.summary_schema_unsupported".to_string());
    }
    for field in ["repo", "change", "receipt_kind", "status"] {
        if string_field(receipt, field).is_none() {
            diagnostics.push(format!("adoption.cairn_evidence.summary_{field}_missing"));
        }
    }
    if let Some(status) = string_field(receipt, "status") {
        if !matches!(
            status.as_str(),
            "pass" | "failed" | "blocked" | "stale" | "missing" | "unsupported"
        ) {
            diagnostics.push("adoption.cairn_evidence.summary_status_unsupported".to_string());
        }
    }
    for field in ["receipt_hash", "policy_hash", "subject_hash"] {
        if let Some(hash) = optional_string_field(receipt, field) {
            if !is_blake3_hex(&hash) {
                diagnostics.push(format!("adoption.cairn_evidence.summary_{field}_invalid"));
            }
        }
    }
}

fn validate_adoption_registry(registry: &Value, diagnostics: &mut Vec<String>) {
    if string_field(registry, "schema").as_deref() != Some(CAIRN_GLOBAL_REGISTRY_SCHEMA_V1) {
        diagnostics.push("adoption.registry.schema_unsupported".to_string());
    }
    if !hash_field_is_blake3(registry, "snapshot_hash") {
        diagnostics.push("adoption.registry.snapshot_hash_invalid".to_string());
    }
    if let Some(non_claims) = array_field(registry, "non_claims") {
        if non_claims.is_empty() {
            diagnostics.push("adoption.registry.non_claims_missing".to_string());
        }
    } else {
        diagnostics.push("adoption.registry.non_claims_missing".to_string());
    }
}

fn validate_valence_admission(admission: &Value, diagnostics: &mut Vec<String>) {
    if string_field(admission, "schema").as_deref() != Some(VALENCE_EVIDENCE_ADMISSION_SCHEMA_V1) {
        diagnostics.push("adoption.valence.schema_unsupported".to_string());
    }
    if admission.get("valid").and_then(Value::as_bool) != Some(true) {
        diagnostics.push("adoption.valence.not_valid".to_string());
    }
    if !hash_field_is_blake3(admission, "receipt_hash") {
        diagnostics.push("adoption.valence.receipt_hash_invalid".to_string());
    }
    if !array_field(admission, "roles").is_some_and(|roles| {
        roles
            .iter()
            .any(|role| role.as_str() == Some("adoption-evidence-admission"))
    }) {
        diagnostics.push("adoption.valence.role_missing".to_string());
    }
    if array_field(admission, "non_claims").is_none_or(Vec::is_empty) {
        diagnostics.push("adoption.valence.non_claims_missing".to_string());
    }
}

fn validate_stack_doctor_conformance(result: &Value, diagnostics: &mut Vec<String>) {
    if string_field(result, "profile").is_none() || string_field(result, "check_id").is_none() {
        diagnostics.push("adoption.stack_doctor.conformance_identity_missing".to_string());
    }
    if !matches!(
        string_field(result, "status").as_deref(),
        Some("pass" | "fail" | "timeout" | "unavailable")
    ) {
        diagnostics.push("adoption.stack_doctor.conformance_status_unsupported".to_string());
    }
    for field in ["producer_revision", "consumer_revision"] {
        if !string_field(result, field)
            .as_deref()
            .is_some_and(is_exact_revision)
        {
            diagnostics.push(format!("adoption.stack_doctor.{field}_invalid"));
        }
    }
    if let Some(hash) = optional_string_field(result, "result_identity_blake3") {
        if !is_blake3_hex(&hash) {
            diagnostics.push("adoption.stack_doctor.result_identity_invalid".to_string());
        }
    }
}

fn stack_deps_observations(
    query: &AdoptionQuery,
    receipt: &Value,
    catalog_relationships: &BTreeMap<String, Value>,
    cohort: &AdoptionProducerCohort,
) -> Vec<AdoptionStageObservation> {
    let receipt_hash = string_field(receipt, "receipt_blake3").unwrap_or_default();
    let observations = array_field(receipt, "observations")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| string_field(item, "relationship").map(|id| (id, item)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    query
        .relationships
        .iter()
        .filter_map(|relationship| {
            let observation = observations.get(&relationship.relationship_id)?;
            let state = stack_deps_adoption_state(string_field(observation, "state").as_deref()?);
            let producer_status = string_field(observation, "state").unwrap_or_default();
            let observed_revision = optional_string_field(observation, "consumer_revision")
                .or_else(|| optional_string_field(observation, "candidate_revision"));
            let message = stack_deps_message(&producer_status);
            let evidence_hashes = vec![receipt_hash.clone()];
            let _catalog_relationship = catalog_relationships.get(&relationship.relationship_id)?;
            Some(AdoptionStageObservation {
                relationship: relationship.clone(),
                stage: AdoptionStageKind::ConsumerPin,
                state,
                producer_id: cohort.producer_id.clone(),
                producer_status,
                evidence_hashes,
                observed_revision,
                message,
            })
        })
        .collect()
}

fn cairn_lifecycle_observations(
    query: &AdoptionQuery,
    receipt: &Value,
    cohort: &AdoptionProducerCohort,
) -> Vec<AdoptionStageObservation> {
    query
        .relationships
        .iter()
        .filter(|relationship| {
            string_field(receipt, "repo").as_deref()
                == Some(relationship.upstream_repository.as_str())
                && string_field(receipt, "change").as_deref()
                    == Some(relationship.upstream_change.as_str())
        })
        .map(|relationship| {
            summary_observation(
                relationship,
                AdoptionStageKind::UpstreamLifecycle,
                receipt,
                cohort,
                "upstream lifecycle evidence",
            )
        })
        .collect()
}

fn cairn_completion_observations(
    query: &AdoptionQuery,
    receipt: &Value,
    cohort: &AdoptionProducerCohort,
) -> Vec<AdoptionStageObservation> {
    query
        .relationships
        .iter()
        .filter(|relationship| {
            string_field(receipt, "repo").as_deref()
                == Some(relationship.consumer_repository.as_str())
                && string_field(receipt, "change").as_deref()
                    == Some(relationship.relationship_id.as_str())
        })
        .map(|relationship| {
            summary_observation(
                relationship,
                AdoptionStageKind::IntegrationEvidence,
                receipt,
                cohort,
                "consumer completion evidence",
            )
        })
        .collect()
}

fn valence_admission_observations(
    query: &AdoptionQuery,
    admission: &Value,
    cohort: &AdoptionProducerCohort,
) -> Vec<AdoptionStageObservation> {
    query
        .relationships
        .iter()
        .filter(|relationship| {
            string_field(admission, "relationship_id").as_deref() == Some(relationship.relationship_id.as_str())
                && string_field(admission, "subject_hash").as_deref() == Some(relationship.subject_id.as_str())
        })
        .map(|relationship| AdoptionStageObservation {
            relationship: relationship.clone(),
            stage: AdoptionStageKind::ConsumerCompatibility,
            state: AdoptionStageState::Supported,
            producer_id: cohort.producer_id.clone(),
            producer_status: "valid".to_string(),
            evidence_hashes: vec![string_field(admission, "receipt_hash").unwrap_or_default()],
            observed_revision: string_field(admission, "consumer_revision"),
            message: "Valence admitted evidence identity, roles, and non-claims without granting catalog or release authority".to_string(),
        })
        .collect()
}

fn summary_observation(
    relationship: &AdoptionRelationshipKey,
    stage: AdoptionStageKind,
    receipt: &Value,
    cohort: &AdoptionProducerCohort,
    message_prefix: &str,
) -> AdoptionStageObservation {
    let status = string_field(receipt, "status").unwrap_or_else(|| "missing".to_string());
    AdoptionStageObservation {
        relationship: relationship.clone(),
        stage,
        state: cairn_summary_state(&status),
        producer_id: cohort.producer_id.clone(),
        producer_status: status.clone(),
        evidence_hashes: vec![string_field(receipt, "receipt_hash").unwrap_or_default()],
        observed_revision: string_field(receipt, "source_revision"),
        message: format!("{message_prefix}: {status}"),
    }
}

fn stack_doctor_observations(
    query: &AdoptionQuery,
    receipt: &Value,
    cohort: &AdoptionProducerCohort,
) -> Vec<AdoptionStageObservation> {
    let receipt_hash = string_field(receipt, "receipt_blake3").unwrap_or_default();
    let results = array_field(receipt, "conformance_results")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| string_field(item, "profile").map(|id| (id, item)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    query
        .relationships
        .iter()
        .filter_map(|relationship| {
            let result = results.get(&relationship.relationship_id)?;
            let status = string_field(result, "status")?;
            let state = stack_doctor_adoption_state(&status);
            let evidence_hashes = optional_string_field(result, "result_identity_blake3")
                .map_or_else(|| vec![receipt_hash.clone()], |hash| vec![hash]);
            Some(AdoptionStageObservation {
                relationship: relationship.clone(),
                stage: AdoptionStageKind::IntegrationEvidence,
                state,
                producer_id: cohort.producer_id.clone(),
                producer_status: status.clone(),
                evidence_hashes,
                observed_revision: optional_string_field(result, "consumer_revision"),
                message: stack_doctor_message(&status),
            })
        })
        .collect()
}

fn producer_admission(
    cohort: &AdoptionProducerCohort,
    schema: &str,
    tool_name: &str,
    receipt_hash: &str,
    observations: Vec<AdoptionStageObservation>,
    diagnostics: Vec<String>,
) -> AdoptionProducerAdmission {
    let mut diagnostics = diagnostics;
    sort_diagnostics(&mut diagnostics);
    AdoptionProducerAdmission {
        valid: diagnostics.is_empty(),
        provenance: AdoptionProducerProvenance {
            producer_id: cohort.producer_id.clone(),
            schema: schema.to_string(),
            source_revision: cohort.source_revision.clone(),
            tool_identity: format!("{tool_name}@{}", cohort.source_revision),
            policy_identity: format!("workspace-dependency-policy@{}", cohort.source_revision),
            observation_scope: "explicit-saved-input".to_string(),
            receipt_hash: receipt_hash.to_string(),
        },
        observations,
        diagnostics,
    }
}

fn stack_deps_adoption_state(state: &str) -> AdoptionStageState {
    match state {
        "current" => AdoptionStageState::Supported,
        "behind" => AdoptionStageState::Gap,
        "ahead" => AdoptionStageState::Diverged,
        "diverged" => AdoptionStageState::Diverged,
        "missing" => AdoptionStageState::Missing,
        "unavailable" => AdoptionStageState::Unavailable,
        "blocked" => AdoptionStageState::Blocked,
        _ => AdoptionStageState::Unknown,
    }
}

fn cairn_summary_state(status: &str) -> AdoptionStageState {
    match status {
        "pass" => AdoptionStageState::Supported,
        "failed" | "blocked" => AdoptionStageState::Blocked,
        "stale" => AdoptionStageState::Stale,
        "missing" => AdoptionStageState::Missing,
        "unsupported" => AdoptionStageState::Unavailable,
        _ => AdoptionStageState::Unknown,
    }
}

fn stack_doctor_adoption_state(status: &str) -> AdoptionStageState {
    match status {
        "pass" => AdoptionStageState::Supported,
        "fail" => AdoptionStageState::Blocked,
        "timeout" => AdoptionStageState::Stale,
        "unavailable" => AdoptionStageState::Unavailable,
        _ => AdoptionStageState::Unknown,
    }
}

fn stack_deps_message(state: &str) -> String {
    match state {
        "current" => "consumer pin matches the required source revision".to_string(),
        "behind" => "consumer pin is behind the required source revision".to_string(),
        "ahead" => "consumer pin is ahead of the required source revision".to_string(),
        "diverged" => "consumer pin diverges from the required source revision".to_string(),
        "missing" => "consumer pin observation is missing".to_string(),
        "unavailable" => "producer source was unavailable".to_string(),
        "blocked" => "producer reported a blocked dependency observation".to_string(),
        _ => "producer reported an unknown dependency observation".to_string(),
    }
}

fn stack_doctor_message(status: &str) -> String {
    match status {
        "pass" => "consumer conformance check passed".to_string(),
        "fail" => "consumer conformance check failed".to_string(),
        "timeout" => "consumer conformance check timed out and must be refreshed".to_string(),
        "unavailable" => "consumer conformance check was unavailable".to_string(),
        _ => "consumer conformance check state is unknown".to_string(),
    }
}

fn is_stack_deps_state(state: &str) -> bool {
    matches!(
        state,
        "current" | "behind" | "ahead" | "diverged" | "missing" | "unavailable" | "blocked"
    )
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    object(value)?.get(field)?.as_str().map(ToOwned::to_owned)
}

fn optional_string_field(value: &Value, field: &str) -> Option<String> {
    match object(value)?.get(field)? {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn array_field<'a>(value: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    object(value)?.get(field)?.as_array()
}

fn object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

fn require_string_array_value(
    value: &Value,
    field: &str,
    expected: &[&str],
    code: &str,
    diagnostics: &mut Vec<String>,
) {
    let actual = array_field(value, field)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if actual != expected {
        diagnostics.push(code.to_string());
    }
}

fn hash_field_is_blake3(value: &Value, field: &str) -> bool {
    string_field(value, field).is_some_and(|text| is_blake3_hex(&text))
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == HASH_HEX_LENGTH && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_exact_revision(value: &str) -> bool {
    let length = value.len();
    (length == GIT_SHA1_HEX_LENGTH || length == GIT_SHA256_HEX_LENGTH)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sort_diagnostics(diagnostics: &mut Vec<String>) {
    diagnostics.sort();
    diagnostics.dedup();
}

fn validate_snapshot(snapshot: &AdoptionSnapshot) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if snapshot.schema != ADOPTION_SNAPSHOT_SCHEMA_V1 {
        diagnostics.push("adoption.schema.unsupported".to_string());
    }
    if snapshot.query.scope_id.is_empty() {
        diagnostics.push("adoption.query.scope_missing".to_string());
    }
    if snapshot.query.relationships.is_empty() {
        diagnostics.push("adoption.query.relationships_missing".to_string());
    }
    if snapshot.query.bounds.max_relationships == 0
        || snapshot.query.bounds.max_observations == 0
        || snapshot.query.bounds.max_provenance == 0
    {
        diagnostics.push("adoption.bounds.zero".to_string());
    }
    diagnostics.sort();
    diagnostics.dedup();
    diagnostics
}

fn relationship_report(
    relationship: AdoptionRelationshipKey,
    observations: &[AdoptionStageObservation],
) -> AdoptionRelationshipReport {
    let stages = adoption_stage_kinds()
        .into_iter()
        .map(|stage| stage_report(stage, observations))
        .collect::<Vec<_>>();
    let complete = stages
        .iter()
        .all(|stage| stage.state == AdoptionStageState::Supported);
    AdoptionRelationshipReport {
        relationship,
        complete,
        stages,
    }
}

fn stage_report(
    stage: AdoptionStageKind,
    observations: &[AdoptionStageObservation],
) -> AdoptionStageReport {
    let mut matches = observations
        .iter()
        .filter(|observation| observation.stage == stage)
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by_key(observation_sort_key);
    matches.dedup();
    if matches.is_empty() {
        return AdoptionStageReport {
            stage,
            state: AdoptionStageState::Missing,
            producer_statuses: Vec::new(),
            evidence_hashes: Vec::new(),
            observed_revisions: Vec::new(),
            messages: vec!["no admitted observation for this stage".to_string()],
        };
    }

    let states = matches
        .iter()
        .map(|observation| observation.state.clone())
        .collect::<BTreeSet<_>>();
    let state = if states.len() == 1 {
        states.iter().next().expect("state present").clone()
    } else {
        AdoptionStageState::Conflict
    };
    let mut producer_statuses = matches
        .iter()
        .map(|observation| {
            format!(
                "{}:{}",
                observation.producer_id, observation.producer_status
            )
        })
        .collect::<Vec<_>>();
    producer_statuses.sort();
    producer_statuses.dedup();
    let mut evidence_hashes = matches
        .iter()
        .flat_map(|observation| observation.evidence_hashes.clone())
        .collect::<Vec<_>>();
    evidence_hashes.sort();
    evidence_hashes.dedup();
    let mut observed_revisions = matches
        .iter()
        .filter_map(|observation| observation.observed_revision.clone())
        .collect::<Vec<_>>();
    observed_revisions.sort();
    observed_revisions.dedup();
    let mut messages = matches
        .iter()
        .map(|observation| observation.message.clone())
        .collect::<Vec<_>>();
    if state == AdoptionStageState::Conflict {
        messages.push("conflicting producer observations for this stage".to_string());
    }
    messages.sort();
    messages.dedup();
    AdoptionStageReport {
        stage,
        state,
        producer_statuses,
        evidence_hashes,
        observed_revisions,
        messages,
    }
}

fn actions_for_relationship(row: &AdoptionRelationshipReport) -> Vec<AdoptionAction> {
    row.stages
        .iter()
        .filter(|stage| stage.state != AdoptionStageState::Supported)
        .map(|stage| action_for_stage(row, stage))
        .collect()
}

fn action_for_stage(
    row: &AdoptionRelationshipReport,
    stage: &AdoptionStageReport,
) -> AdoptionAction {
    let (kind, owner, expected_observation) = match stage.stage {
        AdoptionStageKind::UpstreamLifecycle => (
            ADOPTION_ACTION_KIND_EVIDENCE_REFRESH,
            row.relationship.upstream_repository.as_str(),
            "fresh lifecycle completion evidence",
        ),
        AdoptionStageKind::ConsumerPin => (
            ADOPTION_ACTION_KIND_DIAGNOSTIC_INSPECTION,
            row.relationship.consumer_repository.as_str(),
            "consumer pin observation for the required upstream revision",
        ),
        AdoptionStageKind::ConsumerCompatibility => (
            ADOPTION_ACTION_KIND_EVIDENCE_REFRESH,
            row.relationship.consumer_repository.as_str(),
            "consumer compatibility evidence for the selected subject",
        ),
        AdoptionStageKind::IntegrationEvidence => (
            ADOPTION_ACTION_KIND_EVIDENCE_REFRESH,
            row.relationship.consumer_repository.as_str(),
            "current consumer integration check evidence",
        ),
    };
    let kind = if stage.state == AdoptionStageState::Conflict {
        ADOPTION_ACTION_KIND_UNRESOLVED
    } else {
        kind
    };
    let prerequisites = match stage.state {
        AdoptionStageState::Missing => vec!["supply saved producer receipt".to_string()],
        AdoptionStageState::Stale => vec!["refresh saved producer receipt".to_string()],
        AdoptionStageState::Unavailable => vec!["inspect producer availability".to_string()],
        AdoptionStageState::Blocked => vec!["resolve producer-owned blocker".to_string()],
        AdoptionStageState::Diverged => vec!["review exact revision divergence".to_string()],
        AdoptionStageState::Conflict => {
            vec!["resolve contradictory saved observations".to_string()]
        }
        AdoptionStageState::Partial => vec!["raise or narrow the explicit query bound".to_string()],
        AdoptionStageState::Gap => vec!["perform consumer-owned adoption step".to_string()],
        AdoptionStageState::Unknown => vec!["classify producer-owned unknown state".to_string()],
        AdoptionStageState::Supported => Vec::new(),
    };
    let id = canonical_json_hash(&json!({
        "relationship_id": row.relationship.relationship_id,
        "stage": stage.stage,
        "state": stage.state,
        "kind": kind,
        "owner": owner,
        "expected_observation": expected_observation,
    }));
    AdoptionAction {
        id,
        kind: kind.to_string(),
        authority: ADOPTION_ACTION_AUTHORITY.to_string(),
        owner_repository: owner.to_string(),
        relationship_id: row.relationship.relationship_id.clone(),
        stage: stage.stage.clone(),
        prerequisites,
        expected_observation: expected_observation.to_string(),
        effect_authorized: false,
    }
}

fn nonzero_bound(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value
    }
}

fn observation_sort_key(observation: &AdoptionStageObservation) -> String {
    canonical_json_hash(&serde_json::to_value(observation).expect("observation serializes"))
}

fn provenance_sort_key(provenance: &AdoptionProducerProvenance) -> String {
    canonical_json_hash(&serde_json::to_value(provenance).expect("provenance serializes"))
}

fn snapshot_fingerprint_value(snapshot: &AdoptionSnapshot) -> Value {
    let mut relationships = snapshot.query.relationships.clone();
    relationships.sort();
    relationships.dedup();
    let mut provenance = snapshot.provenance.clone();
    provenance.sort_by_key(provenance_sort_key);
    provenance.dedup();
    let mut observations = snapshot.observations.clone();
    observations.sort_by_key(observation_sort_key);
    observations.dedup();
    json!({
        "schema": snapshot.schema,
        "query": {
            "scope_id": snapshot.query.scope_id,
            "relationships": relationships,
            "bounds": snapshot.query.bounds,
        },
        "provenance": provenance,
        "observations": observations,
        "coverage": snapshot.coverage,
    })
}

fn report_fingerprint(report: &AdoptionReport) -> String {
    let mut value = serde_json::to_value(report).expect("report serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("report_hash");
    }
    canonical_json_hash(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn key(id: &str, consumer: &str) -> AdoptionRelationshipKey {
        AdoptionRelationshipKey {
            relationship_id: id.to_string(),
            upstream_repository: "onixresearch/cairn".to_string(),
            upstream_change: "change-1".to_string(),
            consumer_repository: consumer.to_string(),
            required_revision: "rev-new".to_string(),
            subject_id: "adoption-subject".to_string(),
            policy_id: "policy-a".to_string(),
        }
    }

    fn provenance(id: &str) -> AdoptionProducerProvenance {
        AdoptionProducerProvenance {
            producer_id: id.to_string(),
            schema: "producer.schema.v1".to_string(),
            source_revision: "source-rev".to_string(),
            tool_identity: "tool@source-rev".to_string(),
            policy_identity: "policy@source-rev".to_string(),
            observation_scope: "fixture-scope".to_string(),
            receipt_hash: HASH_A.to_string(),
        }
    }

    fn observation(
        relationship: &AdoptionRelationshipKey,
        stage: AdoptionStageKind,
        state: AdoptionStageState,
        producer: &str,
        message: &str,
    ) -> AdoptionStageObservation {
        AdoptionStageObservation {
            relationship: relationship.clone(),
            stage,
            state,
            producer_id: producer.to_string(),
            producer_status: "recorded".to_string(),
            evidence_hashes: vec![HASH_B.to_string()],
            observed_revision: Some("rev-new".to_string()),
            message: message.to_string(),
        }
    }

    fn complete_snapshot() -> AdoptionSnapshot {
        let relationship = key("cairn-to-octet", "onixresearch/octet");
        AdoptionSnapshot {
            schema: ADOPTION_SNAPSHOT_SCHEMA_V1.to_string(),
            query: AdoptionQuery {
                scope_id: "fixture".to_string(),
                relationships: vec![relationship.clone()],
                bounds: AdoptionQueryBounds::default(),
            },
            provenance: vec![provenance("stack-deps")],
            observations: adoption_stage_kinds()
                .into_iter()
                .map(|stage| {
                    observation(
                        &relationship,
                        stage,
                        AdoptionStageState::Supported,
                        "stack-deps",
                        "supported",
                    )
                })
                .collect(),
            coverage: AdoptionCoverage::default(),
        }
    }

    #[test]
    fn complete_saved_chain_reports_supported_stages() {
        let report = derive_adoption_report(&complete_snapshot());
        assert!(report.valid, "{:?}", report.diagnostics);
        assert!(report.complete);
        assert!(report.actions.is_empty());
        assert_eq!(report.relationships.len(), 1);
        assert!(report.relationships[0].complete);
        assert!(report.relationships[0]
            .stages
            .iter()
            .all(|stage| stage.state == AdoptionStageState::Supported));
    }

    #[test]
    fn equivalent_receipts_in_different_orders_keep_semantic_identity() {
        let mut first = complete_snapshot();
        let mut second = first.clone();
        second.query.relationships.reverse();
        second.provenance.reverse();
        second.observations.reverse();
        first.observations.push(first.observations[0].clone());
        let first_report = derive_adoption_report(&first);
        let second_report = derive_adoption_report(&second);
        assert_eq!(first_report.relationships, second_report.relationships);
        assert_eq!(first_report.actions, second_report.actions);
        assert_eq!(first_report.report_hash, second_report.report_hash);
    }

    #[test]
    fn upstream_archive_with_old_consumer_pin_remains_a_gap() {
        let relationship = key("cairn-to-octet", "onixresearch/octet");
        let snapshot = AdoptionSnapshot {
            schema: ADOPTION_SNAPSHOT_SCHEMA_V1.to_string(),
            query: AdoptionQuery {
                scope_id: "old-pin".to_string(),
                relationships: vec![relationship.clone()],
                bounds: AdoptionQueryBounds::default(),
            },
            provenance: vec![provenance("stack-deps")],
            observations: vec![
                observation(
                    &relationship,
                    AdoptionStageKind::UpstreamLifecycle,
                    AdoptionStageState::Supported,
                    "cairn",
                    "archive present",
                ),
                observation(
                    &relationship,
                    AdoptionStageKind::ConsumerPin,
                    AdoptionStageState::Gap,
                    "stack-deps",
                    "consumer pins old revision",
                ),
            ],
            coverage: AdoptionCoverage::default(),
        };
        let report = derive_adoption_report(&snapshot);
        assert!(report.valid);
        assert!(!report.complete);
        let pin = report.relationships[0]
            .stages
            .iter()
            .find(|stage| stage.stage == AdoptionStageKind::ConsumerPin)
            .expect("pin stage");
        assert_eq!(pin.state, AdoptionStageState::Gap);
        assert!(report
            .actions
            .iter()
            .any(|action| action.owner_repository == "onixresearch/octet"
                && action.kind == ADOPTION_ACTION_KIND_DIAGNOSTIC_INSPECTION
                && !action.effect_authorized));
    }

    #[test]
    fn matching_pin_without_current_checks_remains_incomplete() {
        let mut snapshot = complete_snapshot();
        snapshot
            .observations
            .retain(|observation| observation.stage != AdoptionStageKind::IntegrationEvidence);
        let report = derive_adoption_report(&snapshot);
        assert!(report.valid);
        assert!(!report.complete);
        let integration = report.relationships[0]
            .stages
            .iter()
            .find(|stage| stage.stage == AdoptionStageKind::IntegrationEvidence)
            .expect("integration stage");
        assert_eq!(integration.state, AdoptionStageState::Missing);
    }

    #[test]
    fn unknown_conflict_and_partial_states_are_visible_not_successful() {
        let relationship = key("octet-to-lattice", "onixresearch/lattice");
        let coverage = AdoptionCoverage {
            partial: true,
            omitted_relationships: vec!["onixresearch/extra".to_string()],
            ..AdoptionCoverage::default()
        };
        let snapshot = AdoptionSnapshot {
            schema: ADOPTION_SNAPSHOT_SCHEMA_V1.to_string(),
            query: AdoptionQuery {
                scope_id: "unknowns".to_string(),
                relationships: vec![relationship.clone()],
                bounds: AdoptionQueryBounds::default(),
            },
            provenance: vec![provenance("stack-deps")],
            observations: vec![
                observation(
                    &relationship,
                    AdoptionStageKind::ConsumerPin,
                    AdoptionStageState::Unknown,
                    "stack-deps",
                    "producer reported unknown",
                ),
                observation(
                    &relationship,
                    AdoptionStageKind::ConsumerPin,
                    AdoptionStageState::Diverged,
                    "stack-deps-reconcile",
                    "producer reported diverged",
                ),
                observation(
                    &relationship,
                    AdoptionStageKind::IntegrationEvidence,
                    AdoptionStageState::Partial,
                    "stack-doctor",
                    "bounded query omitted checks",
                ),
            ],
            coverage,
        };
        let report = derive_adoption_report(&snapshot);
        assert!(report.valid);
        assert!(report.coverage.partial);
        assert!(!report.complete);
        let pin = report.relationships[0]
            .stages
            .iter()
            .find(|stage| stage.stage == AdoptionStageKind::ConsumerPin)
            .expect("pin stage");
        assert_eq!(pin.state, AdoptionStageState::Conflict);
        assert!(report
            .actions
            .iter()
            .any(|action| action.kind == ADOPTION_ACTION_KIND_UNRESOLVED));
    }

    fn stack_deps_cohort() -> AdoptionProducerCohort {
        AdoptionProducerCohort {
            producer_id: "workspace-stack-deps-v2".to_string(),
            kind: STACK_DEPS_PRODUCER_KIND.to_string(),
            supported_schemas: vec![
                STACK_DEPS_CATALOG_SCHEMA_V2.to_string(),
                STACK_DEPS_RECEIPT_SCHEMA_V1.to_string(),
                STACK_DEPS_RECONCILIATION_SCHEMA_V2.to_string(),
            ],
            source_revision: ADOPTION_STACK_WORKSPACE_TOOLS_REVISION.to_string(),
            validation_evidence: HASH_A.to_string(),
            required_non_claims: STACK_DEPS_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }

    fn stack_doctor_cohort() -> AdoptionProducerCohort {
        AdoptionProducerCohort {
            producer_id: "workspace-stack-doctor-v1".to_string(),
            kind: STACK_DOCTOR_PRODUCER_KIND.to_string(),
            supported_schemas: vec![STACK_DOCTOR_HEALTH_SCHEMA_V1.to_string()],
            source_revision: ADOPTION_STACK_WORKSPACE_TOOLS_REVISION.to_string(),
            validation_evidence: HASH_B.to_string(),
            required_non_claims: STACK_DOCTOR_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }

    fn cairn_evidence_cohort() -> AdoptionProducerCohort {
        AdoptionProducerCohort {
            producer_id: "cairn-adoption-evidence-v1".to_string(),
            kind: CAIRN_EVIDENCE_PRODUCER_KIND.to_string(),
            supported_schemas: vec![
                CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1.to_string(),
                CAIRN_GLOBAL_REGISTRY_SCHEMA_V1.to_string(),
                VALENCE_EVIDENCE_ADMISSION_SCHEMA_V1.to_string(),
            ],
            source_revision: ADOPTION_STACK_WORKSPACE_TOOLS_REVISION.to_string(),
            validation_evidence: HASH_C.to_string(),
            required_non_claims: vec![
                "registry membership is a derived index, not evidence satisfaction".to_string(),
                "Valence admission preserves identity and roles only".to_string(),
            ],
        }
    }

    fn stack_catalog(relationship: &AdoptionRelationshipKey) -> Value {
        serde_json::json!({
            "schema": STACK_DEPS_CATALOG_SCHEMA_V2,
            "relationships": [{
                "id": relationship.relationship_id,
                "source": {
                    "source_identity": "git:https://github.com/OnixResearch/cairn"
                }
            }]
        })
    }

    fn stack_audit(relationship: &AdoptionRelationshipKey, state: &str) -> Value {
        serde_json::json!({
            "schema": STACK_DEPS_RECEIPT_SCHEMA_V1,
            "operation": "audit",
            "relationship": null,
            "verdict": if state == "current" { "pass" } else { "fail" },
            "observations": [{
                "relationship": relationship.relationship_id,
                "consumer_revision": "1111111111111111111111111111111111111111",
                "candidate_revision": "2222222222222222222222222222222222222222",
                "state": state,
                "blocker": null,
                "source_identity": "git:https://github.com/OnixResearch/cairn"
            }],
            "plan_blake3": null,
            "adapter_results": [],
            "check_results": [],
            "changed_paths": [],
            "claims": [STACK_DEPS_CLAIM_OBSERVED],
            "non_claims": STACK_DEPS_NON_CLAIMS,
            "receipt_blake3": HASH_A
        })
    }

    fn stack_reconciliation(catalog: &Value, audit: &Value) -> Value {
        serde_json::json!({
            "schema": STACK_DEPS_RECONCILIATION_SCHEMA_V2,
            "catalog_blake3": canonical_json_hash(catalog),
            "audit_receipt_blake3": string_field(audit, "receipt_blake3").expect("audit hash"),
            "current_relationships": [],
            "ready_plans": [],
            "deferred_relationships": [],
            "blockers": [],
            "non_claims": STACK_DEPS_NON_CLAIMS,
            "reconciliation_blake3": HASH_C
        })
    }

    fn cairn_summary(
        relationship: &AdoptionRelationshipKey,
        repo: &str,
        change: &str,
        kind: &str,
        status: &str,
    ) -> Value {
        serde_json::json!({
            "schema": CAIRN_CROSS_REPO_SUMMARY_SCHEMA_V1,
            "repo": repo,
            "change": change,
            "receipt_kind": kind,
            "status": status,
            "policy_hash": HASH_A,
            "subject_hash": relationship.subject_id,
            "receipt_hash": HASH_B,
            "source_revision": relationship.required_revision,
            "diagnostic_summary": "fixture"
        })
    }

    fn registry_snapshot() -> Value {
        serde_json::json!({
            "schema": CAIRN_GLOBAL_REGISTRY_SCHEMA_V1,
            "scope_id": "adoption-fixture",
            "nodes": [],
            "edges": [],
            "sources": [],
            "tags": [],
            "adapters": [],
            "non_claims": ["registry membership does not satisfy adoption evidence"],
            "snapshot_hash": HASH_C
        })
    }

    fn valence_admission(relationship: &AdoptionRelationshipKey) -> Value {
        serde_json::json!({
            "schema": VALENCE_EVIDENCE_ADMISSION_SCHEMA_V1,
            "valid": true,
            "relationship_id": relationship.relationship_id,
            "subject_hash": relationship.subject_id,
            "consumer_revision": relationship.required_revision,
            "roles": ["adoption-evidence-admission"],
            "non_claims": ["Valence identity admission does not prove catalog authority or release readiness"],
            "receipt_hash": HASH_A
        })
    }

    fn stack_doctor_receipt(relationship: &AdoptionRelationshipKey, status: &str) -> Value {
        serde_json::json!({
            "schema": STACK_DOCTOR_HEALTH_SCHEMA_V1,
            "verdict": if status == "pass" { "pass" } else { "fail" },
            "profile_blake3": HASH_A,
            "observations_blake3": HASH_B,
            "checked_scopes": ["repositories", "lifecycle", "documents", "external-components", "conformance"],
            "repository_count": 2,
            "discovered_repository_count": 2,
            "findings": [],
            "conformance_results": [{
                "profile": relationship.relationship_id,
                "check_id": "cairn-to-octet-integration",
                "status": status,
                "producer_revision": "1111111111111111111111111111111111111111",
                "consumer_revision": "2222222222222222222222222222222222222222",
                "result_identity_blake3": HASH_C,
                "diagnostic_class": "fixture"
            }],
            "claims": [STACK_DOCTOR_CLAIM_OBSERVED, STACK_DOCTOR_CLAIM_CONFORMANCE],
            "non_claims": STACK_DOCTOR_NON_CLAIMS,
            "receipt_blake3": HASH_C
        })
    }

    #[test]
    fn stack_deps_contracts_admit_exact_producer_and_preserve_old_pin_gap() {
        let relationship = key("cairn-to-octet", "onixresearch/octet");
        let query = AdoptionQuery {
            scope_id: "producer-stack-deps".to_string(),
            relationships: vec![relationship.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let catalog = stack_catalog(&relationship);
        let audit = stack_audit(&relationship, "behind");
        let reconciliation = stack_reconciliation(&catalog, &audit);
        let admission = admit_stack_deps_contracts(
            &query,
            &StackDepsContractInput {
                catalog: &catalog,
                audit_receipt: &audit,
                reconciliation: &reconciliation,
            },
            &stack_deps_cohort(),
        );
        assert!(admission.valid, "{:?}", admission.diagnostics);
        assert_eq!(
            admission.provenance.source_revision,
            ADOPTION_STACK_WORKSPACE_TOOLS_REVISION
        );
        assert_eq!(admission.observations.len(), 1);
        assert_eq!(
            admission.observations[0].stage,
            AdoptionStageKind::ConsumerPin
        );
        assert_eq!(admission.observations[0].state, AdoptionStageState::Gap);
    }

    #[test]
    fn stack_deps_contracts_reject_unsupported_schema_and_stale_reconciliation() {
        let relationship = key("cairn-to-octet", "onixresearch/octet");
        let query = AdoptionQuery {
            scope_id: "producer-stack-deps-negative".to_string(),
            relationships: vec![relationship.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let mut catalog = stack_catalog(&relationship);
        catalog["schema"] = serde_json::json!("onix.stack-deps.audit.v2");
        let audit = stack_audit(&relationship, "current");
        let mut reconciliation = stack_reconciliation(&catalog, &audit);
        reconciliation["audit_receipt_blake3"] = serde_json::json!(HASH_B);
        let admission = admit_stack_deps_contracts(
            &query,
            &StackDepsContractInput {
                catalog: &catalog,
                audit_receipt: &audit,
                reconciliation: &reconciliation,
            },
            &stack_deps_cohort(),
        );
        assert!(!admission.valid);
        assert!(admission
            .diagnostics
            .contains(&"adoption.stack_deps.catalog_schema_unsupported".to_string()));
        assert!(admission
            .diagnostics
            .contains(&"adoption.stack_deps.reconciliation_audit_hash_mismatch".to_string()));
    }

    #[test]
    fn stack_doctor_health_admits_pass_and_failure_without_release_claims() {
        let supported = key("cairn-to-octet", "onixresearch/octet");
        let blocked = key("octet-to-lattice", "onixresearch/lattice");
        let query = AdoptionQuery {
            scope_id: "producer-stack-doctor".to_string(),
            relationships: vec![supported.clone(), blocked.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let mut receipt = stack_doctor_receipt(&supported, "pass");
        receipt["conformance_results"]
            .as_array_mut()
            .expect("conformance array")
            .push(serde_json::json!({
                "profile": blocked.relationship_id,
                "check_id": "octet-to-lattice-integration",
                "status": "fail",
                "producer_revision": "1111111111111111111111111111111111111111",
                "consumer_revision": "2222222222222222222222222222222222222222",
                "result_identity_blake3": HASH_B,
                "diagnostic_class": "fixture-failure"
            }));
        let admission = admit_stack_doctor_health(&query, &receipt, &stack_doctor_cohort());
        assert!(admission.valid, "{:?}", admission.diagnostics);
        assert_eq!(admission.observations.len(), 2);
        assert!(admission
            .observations
            .iter()
            .any(
                |item| item.relationship.relationship_id == supported.relationship_id
                    && item.state == AdoptionStageState::Supported
            ));
        assert!(admission
            .observations
            .iter()
            .any(
                |item| item.relationship.relationship_id == blocked.relationship_id
                    && item.state == AdoptionStageState::Blocked
            ));
    }

    #[test]
    fn stack_doctor_health_rejects_claim_drift_and_unsupported_status() {
        let relationship = key("cairn-to-octet", "onixresearch/octet");
        let query = AdoptionQuery {
            scope_id: "producer-stack-doctor-negative".to_string(),
            relationships: vec![relationship.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let mut receipt = stack_doctor_receipt(&relationship, "pass");
        receipt["claims"] =
            serde_json::json!(["observed-stack-reference-state", "proves-release-readiness"]);
        receipt["conformance_results"][0]["status"] = serde_json::json!("flaky");
        let admission = admit_stack_doctor_health(&query, &receipt, &stack_doctor_cohort());
        assert!(!admission.valid);
        assert!(admission
            .diagnostics
            .contains(&"adoption.stack_doctor.claims_mismatch".to_string()));
        assert!(admission
            .diagnostics
            .contains(&"adoption.stack_doctor.conformance_status_unsupported".to_string()));
    }

    #[test]
    fn cairn_evidence_composes_lifecycle_registry_valence_and_completion() {
        let mut relationship = key("cairn-to-octet", "onixresearch/octet");
        relationship.subject_id = HASH_C.to_string();
        relationship.required_revision = "2222222222222222222222222222222222222222".to_string();
        let query = AdoptionQuery {
            scope_id: "cairn-evidence-positive".to_string(),
            relationships: vec![relationship.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let admission = admit_cairn_adoption_evidence(
            &query,
            &CairnAdoptionEvidenceInput {
                lifecycle_receipts: &[cairn_summary(
                    &relationship,
                    "onixresearch/cairn",
                    "change-1",
                    "archive_receipt",
                    "pass",
                )],
                completion_receipts: &[cairn_summary(
                    &relationship,
                    "onixresearch/octet",
                    "cairn-to-octet",
                    "integration_receipt",
                    "pass",
                )],
                registry_snapshot: Some(&registry_snapshot()),
                valence_admission: Some(&valence_admission(&relationship)),
            },
            &cairn_evidence_cohort(),
        );
        assert!(admission.valid, "{:?}", admission.diagnostics);
        assert_eq!(admission.observations.len(), 3);
        let snapshot = AdoptionSnapshot {
            schema: ADOPTION_SNAPSHOT_SCHEMA_V1.to_string(),
            query,
            provenance: vec![admission.provenance],
            observations: admission.observations,
            coverage: AdoptionCoverage::default(),
        };
        let report = derive_adoption_report(&snapshot);
        assert!(!report.complete, "consumer pin remains separately owned");
        assert_eq!(
            report.relationships[0]
                .stages
                .iter()
                .find(|stage| stage.stage == AdoptionStageKind::ConsumerCompatibility)
                .expect("compatibility")
                .state,
            AdoptionStageState::Supported
        );
    }

    #[test]
    fn cairn_evidence_rejects_bad_registry_and_valence_overclaim() {
        let mut relationship = key("cairn-to-octet", "onixresearch/octet");
        relationship.subject_id = HASH_C.to_string();
        let query = AdoptionQuery {
            scope_id: "cairn-evidence-negative".to_string(),
            relationships: vec![relationship.clone()],
            bounds: AdoptionQueryBounds::default(),
        };
        let mut registry = registry_snapshot();
        registry["snapshot_hash"] = serde_json::json!("not-a-hash");
        let mut valence = valence_admission(&relationship);
        valence["roles"] = serde_json::json!(["catalog-authority"]);
        let admission = admit_cairn_adoption_evidence(
            &query,
            &CairnAdoptionEvidenceInput {
                lifecycle_receipts: &[cairn_summary(
                    &relationship,
                    "onixresearch/cairn",
                    "change-1",
                    "archive_receipt",
                    "pass",
                )],
                completion_receipts: &[],
                registry_snapshot: Some(&registry),
                valence_admission: Some(&valence),
            },
            &cairn_evidence_cohort(),
        );
        assert!(!admission.valid);
        assert!(admission
            .diagnostics
            .contains(&"adoption.registry.snapshot_hash_invalid".to_string()));
        assert!(admission
            .diagnostics
            .contains(&"adoption.valence.role_missing".to_string()));
        assert!(admission.observations.is_empty());
    }

    #[test]
    fn unsupported_snapshot_schema_is_rejected_without_hidden_repair() {
        let mut snapshot = complete_snapshot();
        snapshot.schema = "cairn.adoption.snapshot.v0".to_string();
        snapshot.provenance[0].receipt_hash = HASH_C.to_string();
        let report = derive_adoption_report(&snapshot);
        assert!(!report.valid);
        assert!(report
            .diagnostics
            .contains(&"adoption.schema.unsupported".to_string()));
        assert!(!report.complete);
    }
}
