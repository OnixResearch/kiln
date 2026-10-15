//! Build-service evidence linkage profile.
//!
//! The validator consumes parsed in-memory profile records and checks typed
//! linkage, canonical identity, verification roles, privacy exclusions, and
//! non-claim boundaries. It does not schedule builds, execute store or cache
//! operations, verify OIDC or UCAN tokens, read logs, decide Cairn release
//! readiness, or prove behavioral correctness.
//!
//! r[impl valence.evidence_ir.build_service_profile]
//! r[impl valence.evidence_ir.build_service_identity]

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;
use crate::shared_identity::contains_overclaim;

#[cfg(test)]
mod tests;

const FIELD_ATTEMPT_ID: &str = "attempt_id";
const FIELD_DECISION: &str = "decision";
const FIELD_IDENTITY: &str = "identity";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_PROFILE_ID: &str = "profile_id";
const FIELD_PROFILE_VERSION: &str = "profile_version";
const FIELD_REASONS: &str = "reasons";
const FIELD_ROLE: &str = "role";
const FIELD_SELECTED_CLASS: &str = "selected_class";
const FIELD_SEQUENCE: &str = "sequence";
const FIELD_TRIGGER_EVIDENCE: &str = "trigger_evidence";
const FIELD_UNITS: &str = "units";

/// Profile identity for the versioned build-service evidence profile.
pub const BUILD_SERVICE_PROFILE_ID: &str = "valence.build-service-evidence.v1";
/// Canonicalization contract identifier for build-service identity material.
pub const BUILD_SERVICE_CANONICALIZATION: &str = "valence-canonical-json-v1";
/// Every build-service profile must carry this exact non-claim boundary.
pub const BUILD_SERVICE_REQUIRED_NON_CLAIM: &str = "build-service linkage proves canonical identity and bounded observation linkage only; it does not prove build correctness, hermeticity, sandboxing, host isolation, identity beyond checked claims, authorization freshness, future resource sufficiency, accounting fairness, external status correctness, or release eligibility";
const BUILD_SERVICE_SUPPORTED_CLAIM: &str = "build-service profile proves canonical identity, typed linkage, role separation, privacy exclusion, and non-claim conformance over supplied records only; it does not prove build correctness, hermeticity, isolation, authorization freshness, resource sufficiency, billing fairness, external status truth, or release eligibility";

const DOMAIN_PREFIX: &str = "valence.build-service";
const DOMAIN_VERSION: &str = "v1";

const ROLE_VERIFIED: &str = "verified";
const ROLE_LINKED: &str = "linked";
const ROLE_SIGNED: &str = "signed";
const ROLE_RECORDED_ONLY: &str = "recorded_only";
const ROLE_EXTERNAL: &str = "external";

const SUPPORTED_ROLES: &[&str] = &[
    ROLE_VERIFIED,
    ROLE_LINKED,
    ROLE_SIGNED,
    ROLE_RECORDED_ONLY,
    ROLE_EXTERNAL,
];

const EXTERNAL_STATUS_ROLES: &[&str] = &[ROLE_RECORDED_ONLY, ROLE_EXTERNAL];

const OBSERVATION_CLASS_BUILD: &str = "build";
const OBSERVATION_CLASS_RETRY: &str = "retry";
const OBSERVATION_CLASS_REUSE: &str = "reuse";
const OBSERVATION_CLASS_USAGE: &str = "usage";
const OBSERVATION_CLASS_EXTERNAL_STATUS: &str = "external-status";

const SUPPORTED_OBSERVATION_CLASSES: &[&str] = &[
    OBSERVATION_CLASS_BUILD,
    OBSERVATION_CLASS_RETRY,
    OBSERVATION_CLASS_REUSE,
    OBSERVATION_CLASS_USAGE,
    OBSERVATION_CLASS_EXTERNAL_STATUS,
];

const TRANSITION_STATES: &[&str] = &["queued", "scheduled", "running", "succeeded", "failed", "cancelled"];

const RETRY_TRIGGER_OOM: &str = "oom_escalation";

const USAGE_KIND_RESERVATION: &str = "reservation";
const USAGE_KIND_RECONCILIATION: &str = "reconciliation";

const USABILITY_USABLE: &str = "usable";
const USABILITY_REJECTED: &str = "rejected";

const MAX_RECORDS_PER_KIND: usize = 64;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_LIST_ITEMS: usize = 32;
const MAX_ISSUES: usize = 256;

/// Field values carrying any of these markers are forbidden in canonical
/// build-service artifacts.
const FORBIDDEN_VALUE_MARKERS: &[&str] = &[
    "bearer ",
    "begin private key",
    "begin openssh private key",
    "oidc_token",
    "ucan_token",
    "compatibility_ticket",
    "private_claim",
    "replay_key",
    "provider_credential",
    "secret=",
    "raw_log",
    "value_derived_hash",
];

/// Verification role assigned to one field or edge, never to a whole build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildServiceRole {
    /// A declared validator checked the bounded fact under an identified policy.
    Verified,
    /// Canonical identities and required references are consistent.
    Linked,
    /// A configured producer signature verified.
    Signed,
    /// The producer reported the observation without independent validation.
    RecordedOnly,
    /// An external system reported the state under named trust assumptions.
    External,
}

impl BuildServiceRole {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => ROLE_VERIFIED,
            Self::Linked => ROLE_LINKED,
            Self::Signed => ROLE_SIGNED,
            Self::RecordedOnly => ROLE_RECORDED_ONLY,
            Self::External => ROLE_EXTERNAL,
        }
    }
}

/// Profile header binding the profile version, canonicalization, and producer policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildServiceEvidenceProfile {
    pub profile_id: String,
    pub profile_version: u32,
    pub canonicalization: String,
    pub producer_policy: String,
    pub claimed_observation_classes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Canonical concrete request and idempotency identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceRequestRecord {
    pub request_id: String,
    pub concrete_request_hash: String,
    pub idempotency_key: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Durable attempt, generation, parent or retry relation, and service policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt_id: String,
    pub request_identity: String,
    pub generation: u32,
    pub parent_attempt_identity: Option<String>,
    pub service_policy: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Prior and next state, sequence, observed time, producer, and reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptTransitionRecord {
    pub attempt_identity: String,
    pub prior_state: String,
    pub next_state: String,
    pub sequence: u32,
    pub observed_at: String,
    pub producer: String,
    pub reason: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Public subject projection, policy identity, operation, decision, and verifier link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityDecisionRecord {
    pub decision_id: String,
    pub attempt_identity: String,
    pub subject_projection: String,
    pub policy_identity: String,
    pub operation: String,
    pub decision: String,
    pub verifier_evidence_ref: Option<String>,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Declared platform and resource class identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineClassRecord {
    pub class_id: String,
    pub platform: String,
    pub resource_class: String,
    pub ordinal: u32,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Declared minima, eligible-set identity, selected class, policy, and ordered reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDecisionRecord {
    pub decision_id: String,
    pub attempt_identity: String,
    pub declared_minima: Vec<String>,
    pub eligible_class_identities: Vec<String>,
    pub selected_class_identity: String,
    pub policy: String,
    pub compatible_observation_identities: Vec<String>,
    pub reasons: Vec<String>,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// One bounded measurement with an explicit unit and integer width.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMeasurement {
    pub name: String,
    pub value: u64,
    pub unit: String,
}

/// Bounded measurements, collector, compatibility scope, and terminal outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceObservationRecord {
    pub observation_id: String,
    pub attempt_identity: String,
    pub measurements: Vec<ResourceMeasurement>,
    pub collector: String,
    pub compatibility_scope: String,
    pub terminal_outcome: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Predecessor, successor, trigger evidence, limit state, and policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryEdgeRecord {
    pub predecessor_attempt_identity: String,
    pub successor_attempt_identity: String,
    pub trigger: String,
    pub trigger_evidence: String,
    pub prior_class_identity: String,
    pub next_class_identity: String,
    pub limit_state: String,
    pub policy: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Reservation or reconciliation identity, public units, window, scope, and outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub usage_id: String,
    pub attempt_identity: String,
    pub usage_kind: String,
    pub unit_schedule: String,
    pub window: String,
    pub scope: String,
    pub outcome: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// One produced output reference with content identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildOutputRef {
    pub path: String,
    pub content_hash: String,
}

/// Request, attempt, platform, policy, outputs, signature reference, and CAS observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildResultRecord {
    pub result_id: String,
    pub request_identity: String,
    pub attempt_identity: String,
    pub platform: String,
    pub policy: String,
    pub outputs: Vec<BuildOutputRef>,
    pub producer_signature_ref: String,
    pub cas_observation: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Producer-to-consumer reuse facts kept as separate, individually roled fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReuseEdgeRecord {
    pub producer_result_identity: String,
    pub consumer_request_identity: String,
    pub sharing_policy: String,
    pub authorization_decision_identity: Option<String>,
    pub request_compatible: bool,
    pub platform_compatible: bool,
    pub output_check_ref: Option<String>,
    pub signature_ref: Option<String>,
    pub cas_observation: Option<String>,
    pub usability_outcome: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Lattice workflow and matrix-job identities linked to one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationRecord {
    pub job_id: String,
    pub workflow_identity: String,
    pub matrix_job_identity: String,
    pub attempt_identity: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// GitHub Check Run or another external status identity and observed state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalStatusRecord {
    pub status_id: String,
    pub provider: String,
    pub job_identity: String,
    pub attempt_identity: String,
    pub observed_state: String,
    pub role: BuildServiceRole,
    pub non_claims: Vec<String>,
}

/// Complete in-memory build-service evidence bundle under one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildServiceBundle {
    pub profile: BuildServiceEvidenceProfile,
    pub service_requests: Vec<ServiceRequestRecord>,
    pub attempts: Vec<AttemptRecord>,
    pub transitions: Vec<AttemptTransitionRecord>,
    pub authority_decisions: Vec<AuthorityDecisionRecord>,
    pub machine_classes: Vec<MachineClassRecord>,
    pub resource_decisions: Vec<ResourceDecisionRecord>,
    pub resource_observations: Vec<ResourceObservationRecord>,
    pub retry_edges: Vec<RetryEdgeRecord>,
    pub usage_records: Vec<UsageRecord>,
    pub build_results: Vec<BuildResultRecord>,
    pub reuse_edges: Vec<ReuseEdgeRecord>,
    pub orchestration_jobs: Vec<OrchestrationRecord>,
    pub external_statuses: Vec<ExternalStatusRecord>,
}

/// Field-scoped deterministic build-service issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct BuildServiceIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl BuildServiceIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Build-service validation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildServiceReport {
    pub valid: bool,
    pub record_count: u32,
    pub issues: Vec<BuildServiceIssue>,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
}

fn domain(kind: &str) -> String {
    format!("{DOMAIN_PREFIX}.{kind}.{DOMAIN_VERSION}")
}

fn identity_material(kind: &str, record: &Value) -> Value {
    json!({
        "domain": domain(kind),
        "record": record,
    })
}

fn record_identity(kind: &str, record: &Value) -> String {
    let material = identity_material(kind, record);
    let bytes = canonical_json_bytes(&material).expect("record material is serializable");
    blake3_hex(&bytes)
}

fn sorted_strings(values: &[String]) -> Vec<String> {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted
}

/// Canonical identity for a resource decision; set-like fields sort before hashing.
///
/// r[impl valence.evidence_ir.build_service_identity]
#[must_use]
pub fn resource_decision_identity(record: &ResourceDecisionRecord) -> String {
    let mut value = serde_json::to_value(record).expect("record serializes");
    value["declared_minima"] = json!(sorted_strings(&record.declared_minima));
    value["eligible_class_identities"] = json!(sorted_strings(&record.eligible_class_identities));
    value["compatible_observation_identities"] = json!(sorted_strings(&record.compatible_observation_identities));
    record_identity("resource-decision", &value)
}

/// Canonical identity for a build result; outputs sort by path then hash before hashing.
///
/// r[impl valence.evidence_ir.build_service_identity]
#[must_use]
pub fn build_result_identity(record: &BuildResultRecord) -> String {
    let mut outputs = record.outputs.clone();
    outputs.sort_by(|left, right| left.path.cmp(&right.path).then(left.content_hash.cmp(&right.content_hash)));
    let mut value = serde_json::to_value(record).expect("record serializes");
    value["outputs"] = json!(outputs);
    record_identity("build-result", &value)
}

macro_rules! identity_fn {
    ($name:ident, $record:ty, $kind:literal) => {
        /// Compute the canonical domain-separated BLAKE3 identity for one record.
        #[must_use]
        pub fn $name(record: &$record) -> String {
            let value = serde_json::to_value(record).expect("record serializes");
            record_identity($kind, &value)
        }
    };
}

identity_fn!(service_request_identity, ServiceRequestRecord, "service-request");
identity_fn!(attempt_identity, AttemptRecord, "attempt");
identity_fn!(transition_identity, AttemptTransitionRecord, "attempt-transition");
identity_fn!(authority_decision_identity, AuthorityDecisionRecord, "authority-decision");
identity_fn!(machine_class_identity, MachineClassRecord, "machine-class");
identity_fn!(resource_observation_identity, ResourceObservationRecord, "resource-observation");
identity_fn!(retry_edge_identity, RetryEdgeRecord, "retry-edge");
identity_fn!(usage_identity, UsageRecord, "usage");
identity_fn!(reuse_edge_identity, ReuseEdgeRecord, "reuse-edge");
identity_fn!(orchestration_identity, OrchestrationRecord, "orchestration-job");
identity_fn!(external_status_identity, ExternalStatusRecord, "external-status");

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn text_bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES
}

fn is_rfc3339_utc(value: &str) -> bool {
    // Strict YYYY-MM-DDTHH:MM:SSZ shape; calendar semantics stay producer-owned.
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digit = |index: usize| bytes[index].is_ascii_digit();
    (0..=3).all(digit)
        && bytes[4] == b'-'
        && (5..=6).all(digit)
        && bytes[7] == b'-'
        && (8..=9).all(digit)
        && bytes[10] == b'T'
        && (11..=12).all(digit)
        && bytes[13] == b':'
        && (14..=15).all(digit)
        && bytes[16] == b':'
        && (17..=18).all(digit)
        && bytes[19] == b'Z'
}

fn scan_forbidden(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    FORBIDDEN_VALUE_MARKERS.iter().any(|marker| lower.contains(marker))
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<BuildServiceIssue>) {
    if non_claims.is_empty() {
        issues.push(BuildServiceIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return;
    }
    if !non_claims.iter().any(|claim| claim.contains(BUILD_SERVICE_REQUIRED_NON_CLAIM)) {
        issues.push(BuildServiceIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(BuildServiceIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
}

fn validate_role(role: BuildServiceRole, issues: &mut Vec<BuildServiceIssue>) {
    if !SUPPORTED_ROLES.contains(&role.as_str()) {
        issues.push(BuildServiceIssue::new(FIELD_ROLE, "role_unsupported"));
    }
}

fn validate_privacy(value: &Value, issues: &mut Vec<BuildServiceIssue>) {
    match value {
        Value::String(text) if scan_forbidden(text) => {
            issues.push(BuildServiceIssue::new("record_field", "forbidden_material"));
        }
        Value::String(_) => {}
        Value::Array(items) => {
            for item in items {
                validate_privacy(item, issues);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                if scan_forbidden(key) {
                    issues.push(BuildServiceIssue::new("record_field", "forbidden_material"));
                }
                validate_privacy(item, issues);
            }
        }
        _ => {}
    }
}

fn validate_profile(profile: &BuildServiceEvidenceProfile, issues: &mut Vec<BuildServiceIssue>) {
    if profile.profile_id != BUILD_SERVICE_PROFILE_ID {
        issues.push(BuildServiceIssue::new(FIELD_PROFILE_ID, "profile_id_unsupported"));
    }
    if profile.profile_version != 1 {
        issues.push(BuildServiceIssue::new(FIELD_PROFILE_VERSION, "profile_version_unsupported"));
    }
    if profile.canonicalization != BUILD_SERVICE_CANONICALIZATION {
        issues.push(BuildServiceIssue::new("canonicalization", "canonicalization_unsupported"));
    }
    if !is_hex_digest(&profile.producer_policy) {
        issues.push(BuildServiceIssue::new("producer_policy", "policy_identity_malformed"));
    }
    if profile.claimed_observation_classes.is_empty() {
        issues.push(BuildServiceIssue::new("claimed_observation_classes", "observation_class_missing"));
    }
    for class in &profile.claimed_observation_classes {
        if !SUPPORTED_OBSERVATION_CLASSES.contains(&class.as_str()) {
            issues.push(BuildServiceIssue::new("claimed_observation_classes", "observation_class_unsupported"));
        }
    }
    validate_non_claims(&profile.non_claims, issues);
}

struct IdentityIndex {
    requests: Vec<String>,
    attempts: Vec<String>,
    classes: Vec<String>,
    decisions: Vec<String>,
    observations: Vec<String>,
    results: Vec<String>,
    jobs: Vec<String>,
}

fn reject_duplicates(identities: &[String], issues: &mut Vec<BuildServiceIssue>) {
    let mut sorted = identities.to_vec();
    sorted.sort();
    for pair in sorted.windows(2) {
        if pair[0] == pair[1] {
            issues.push(BuildServiceIssue::new(FIELD_IDENTITY, "duplicate_identity"));
            return;
        }
    }
}

fn require_reference(
    reference: &str,
    known: &[String],
    field: &'static str,
    code: &'static str,
    issues: &mut Vec<BuildServiceIssue>,
) {
    if !known.iter().any(|candidate| candidate == reference) {
        issues.push(BuildServiceIssue::new(field, code));
    }
}

fn validate_attempts(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for attempt in &bundle.attempts {
        validate_role(attempt.role, issues);
        validate_non_claims(&attempt.non_claims, issues);
        if !text_bounded(&attempt.attempt_id) || !text_bounded(&attempt.service_policy) {
            issues.push(BuildServiceIssue::new(FIELD_ATTEMPT_ID, "text_unbounded"));
        }
        require_reference(&attempt.request_identity, &index.requests, "request_identity", "dangling_link", issues);
        if let Some(parent) = &attempt.parent_attempt_identity {
            require_reference(parent, &index.attempts, "parent_attempt_identity", "dangling_link", issues);
            if parent == &attempt_identity(attempt) {
                issues.push(BuildServiceIssue::new("parent_attempt_identity", "self_dependency"));
            }
        }
        if attempt.generation == 0 {
            issues.push(BuildServiceIssue::new("generation", "generation_zero"));
        }
    }
}

fn validate_transitions(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    let mut per_attempt: Vec<(&String, u32)> = Vec::new();
    for transition in &bundle.transitions {
        validate_role(transition.role, issues);
        validate_non_claims(&transition.non_claims, issues);
        require_reference(&transition.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if !TRANSITION_STATES.contains(&transition.prior_state.as_str())
            || !TRANSITION_STATES.contains(&transition.next_state.as_str())
        {
            issues.push(BuildServiceIssue::new("next_state", "state_unsupported"));
        }
        if !is_rfc3339_utc(&transition.observed_at) {
            issues.push(BuildServiceIssue::new("observed_at", "timestamp_ambiguous"));
        }
        if !text_bounded(&transition.producer) || !text_bounded(&transition.reason) {
            issues.push(BuildServiceIssue::new("producer", "text_unbounded"));
        }
        per_attempt.push((&transition.attempt_identity, transition.sequence));
    }
    per_attempt.sort();
    for pair in per_attempt.windows(2) {
        if pair[0] == pair[1] {
            issues.push(BuildServiceIssue::new(FIELD_SEQUENCE, "duplicate_sequence"));
            return;
        }
    }
}

fn validate_authority(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for decision in &bundle.authority_decisions {
        validate_role(decision.role, issues);
        validate_non_claims(&decision.non_claims, issues);
        require_reference(&decision.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if !matches!(decision.decision.as_str(), "allow" | "deny") {
            issues.push(BuildServiceIssue::new(FIELD_DECISION, "decision_unsupported"));
        }
        if !text_bounded(&decision.subject_projection) || !text_bounded(&decision.operation) {
            issues.push(BuildServiceIssue::new("subject_projection", "text_unbounded"));
        }
        if !is_hex_digest(&decision.policy_identity) {
            issues.push(BuildServiceIssue::new("policy_identity", "policy_identity_malformed"));
        }
        if let Some(reference) = &decision.verifier_evidence_ref
            && !text_bounded(reference)
        {
            issues.push(BuildServiceIssue::new("verifier_evidence_ref", "text_unbounded"));
        }
    }
}

fn validate_classes(bundle: &BuildServiceBundle, issues: &mut Vec<BuildServiceIssue>) {
    let mut ordinals: Vec<u32> = Vec::new();
    for class in &bundle.machine_classes {
        validate_role(class.role, issues);
        validate_non_claims(&class.non_claims, issues);
        if !text_bounded(&class.class_id) || !text_bounded(&class.platform) || !text_bounded(&class.resource_class) {
            issues.push(BuildServiceIssue::new("class_id", "text_unbounded"));
        }
        ordinals.push(class.ordinal);
    }
    ordinals.sort_unstable();
    for pair in ordinals.windows(2) {
        if pair[0] == pair[1] {
            issues.push(BuildServiceIssue::new("ordinal", "duplicate_class_ordinal"));
            return;
        }
    }
}

fn class_ordinal(bundle: &BuildServiceBundle, identity: &str) -> Option<u32> {
    bundle
        .machine_classes
        .iter()
        .find(|class| machine_class_identity(class) == identity)
        .map(|class| class.ordinal)
}

fn validate_resource_decisions(
    bundle: &BuildServiceBundle,
    index: &IdentityIndex,
    issues: &mut Vec<BuildServiceIssue>,
) {
    for decision in &bundle.resource_decisions {
        validate_role(decision.role, issues);
        validate_non_claims(&decision.non_claims, issues);
        require_reference(&decision.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if decision.eligible_class_identities.is_empty() {
            issues.push(BuildServiceIssue::new("eligible_class_identities", "eligible_set_empty"));
        }
        for class_reference in &decision.eligible_class_identities {
            require_reference(class_reference, &index.classes, "eligible_class_identities", "dangling_link", issues);
        }
        require_reference(
            &decision.selected_class_identity,
            &decision.eligible_class_identities,
            FIELD_SELECTED_CLASS,
            "selected_class_not_eligible",
            issues,
        );
        for observation_reference in &decision.compatible_observation_identities {
            require_reference(
                observation_reference,
                &index.observations,
                "compatible_observation_identities",
                "dangling_link",
                issues,
            );
        }
        if decision.reasons.is_empty() || decision.reasons.len() > MAX_LIST_ITEMS {
            issues.push(BuildServiceIssue::new(FIELD_REASONS, "reasons_unbounded"));
        }
        if decision.reasons.iter().any(|reason| !text_bounded(reason)) {
            issues.push(BuildServiceIssue::new(FIELD_REASONS, "text_unbounded"));
        }
    }
}

fn validate_observations(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for observation in &bundle.resource_observations {
        validate_role(observation.role, issues);
        validate_non_claims(&observation.non_claims, issues);
        require_reference(&observation.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if observation.measurements.is_empty() || observation.measurements.len() > MAX_LIST_ITEMS {
            issues.push(BuildServiceIssue::new("measurements", "measurements_unbounded"));
        }
        for measurement in &observation.measurements {
            if !text_bounded(&measurement.name) || measurement.unit.is_empty() {
                issues.push(BuildServiceIssue::new(FIELD_UNITS, "unit_missing"));
            }
        }
    }
}

fn validate_retry_edges(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for edge in &bundle.retry_edges {
        validate_role(edge.role, issues);
        validate_non_claims(&edge.non_claims, issues);
        require_reference(
            &edge.predecessor_attempt_identity,
            &index.attempts,
            "predecessor_attempt_identity",
            "dangling_link",
            issues,
        );
        require_reference(
            &edge.successor_attempt_identity,
            &index.attempts,
            "successor_attempt_identity",
            "dangling_link",
            issues,
        );
        if edge.predecessor_attempt_identity == edge.successor_attempt_identity {
            issues.push(BuildServiceIssue::new("successor_attempt_identity", "self_dependency"));
        }
        require_reference(&edge.prior_class_identity, &index.classes, "prior_class_identity", "dangling_link", issues);
        require_reference(&edge.next_class_identity, &index.classes, "next_class_identity", "dangling_link", issues);
        if edge.trigger == RETRY_TRIGGER_OOM {
            // r[impl valence.evidence_ir.resource_retry_links]
            if edge.trigger_evidence.is_empty() || !text_bounded(&edge.trigger_evidence) {
                issues.push(BuildServiceIssue::new(FIELD_TRIGGER_EVIDENCE, "oom_evidence_missing"));
            }
            match (class_ordinal(bundle, &edge.prior_class_identity), class_ordinal(bundle, &edge.next_class_identity))
            {
                (Some(prior), Some(next)) if next > prior => {}
                (Some(_), Some(_)) => {
                    issues.push(BuildServiceIssue::new("next_class_identity", "class_not_increasing"));
                }
                _ => {}
            }
        } else {
            issues.push(BuildServiceIssue::new("trigger", "trigger_unsupported"));
        }
        if !text_bounded(&edge.limit_state) || !text_bounded(&edge.policy) {
            issues.push(BuildServiceIssue::new("limit_state", "text_unbounded"));
        }
    }
}

fn validate_usage(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    let mut reconciliation_identities: Vec<String> = Vec::new();
    for usage in &bundle.usage_records {
        validate_role(usage.role, issues);
        validate_non_claims(&usage.non_claims, issues);
        require_reference(&usage.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if !matches!(usage.usage_kind.as_str(), USAGE_KIND_RESERVATION | USAGE_KIND_RECONCILIATION) {
            issues.push(BuildServiceIssue::new("usage_kind", "usage_kind_unsupported"));
        }
        if !text_bounded(&usage.unit_schedule) || !text_bounded(&usage.window) || !text_bounded(&usage.scope) {
            issues.push(BuildServiceIssue::new("unit_schedule", "text_unbounded"));
        }
        if usage.usage_kind == USAGE_KIND_RECONCILIATION {
            // r[impl valence.evidence_ir.usage_links]
            reconciliation_identities.push(usage_identity(usage));
        }
    }
    reject_duplicates(&reconciliation_identities, issues);
}

fn validate_results(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for result in &bundle.build_results {
        validate_role(result.role, issues);
        validate_non_claims(&result.non_claims, issues);
        require_reference(&result.request_identity, &index.requests, "request_identity", "dangling_link", issues);
        require_reference(&result.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if result.outputs.is_empty() || result.outputs.len() > MAX_LIST_ITEMS {
            issues.push(BuildServiceIssue::new("outputs", "outputs_unbounded"));
        }
        let mut paths: Vec<&str> = result.outputs.iter().map(|output| output.path.as_str()).collect();
        paths.sort_unstable();
        for pair in paths.windows(2) {
            if pair[0] == pair[1] {
                issues.push(BuildServiceIssue::new("outputs", "duplicate_output_path"));
            }
        }
        for output in &result.outputs {
            if !text_bounded(&output.path) || !is_hex_digest(&output.content_hash) {
                issues.push(BuildServiceIssue::new("outputs", "output_identity_malformed"));
            }
        }
        if !text_bounded(&result.producer_signature_ref) || !text_bounded(&result.cas_observation) {
            issues.push(BuildServiceIssue::new("producer_signature_ref", "text_unbounded"));
        }
    }
}

fn validate_reuse(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for edge in &bundle.reuse_edges {
        validate_role(edge.role, issues);
        validate_non_claims(&edge.non_claims, issues);
        // r[impl valence.evidence_ir.result_reuse_edges]
        require_reference(
            &edge.producer_result_identity,
            &index.results,
            "producer_result_identity",
            "dangling_link",
            issues,
        );
        require_reference(
            &edge.consumer_request_identity,
            &index.requests,
            "consumer_request_identity",
            "dangling_link",
            issues,
        );
        let producer_request = bundle
            .build_results
            .iter()
            .find(|result| build_result_identity(result) == edge.producer_result_identity)
            .map(|result| result.request_identity.clone());
        if producer_request.as_ref() == Some(&edge.consumer_request_identity) {
            issues.push(BuildServiceIssue::new("consumer_request_identity", "producer_consumer_confusion"));
        }
        if !matches!(edge.usability_outcome.as_str(), USABILITY_USABLE | USABILITY_REJECTED) {
            issues.push(BuildServiceIssue::new("usability_outcome", "usability_unsupported"));
        }
        if edge.usability_outcome == USABILITY_USABLE {
            if edge.authorization_decision_identity.is_none() {
                issues.push(BuildServiceIssue::new("authorization_decision_identity", "reuse_authority_missing"));
            }
            if edge.signature_ref.is_none() {
                issues.push(BuildServiceIssue::new("signature_ref", "reuse_signature_missing"));
            }
            if edge.cas_observation.is_none() {
                issues.push(BuildServiceIssue::new("cas_observation", "reuse_cas_missing"));
            }
            if !edge.request_compatible || !edge.platform_compatible {
                issues.push(BuildServiceIssue::new("platform_compatible", "reuse_compatibility_missing"));
            }
            if edge.output_check_ref.is_none() {
                issues.push(BuildServiceIssue::new("output_check_ref", "reuse_output_check_missing"));
            }
        }
        if let Some(authority) = &edge.authorization_decision_identity {
            require_reference(authority, &index.decisions, "authorization_decision_identity", "dangling_link", issues);
        }
    }
}

fn validate_orchestration(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for job in &bundle.orchestration_jobs {
        validate_role(job.role, issues);
        validate_non_claims(&job.non_claims, issues);
        require_reference(&job.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if !text_bounded(&job.job_id)
            || !text_bounded(&job.workflow_identity)
            || !text_bounded(&job.matrix_job_identity)
        {
            issues.push(BuildServiceIssue::new("job_id", "text_unbounded"));
        }
    }
}

fn validate_external_status(bundle: &BuildServiceBundle, index: &IdentityIndex, issues: &mut Vec<BuildServiceIssue>) {
    for status in &bundle.external_statuses {
        // r[impl valence.evidence_ir.external_status_links]
        validate_role(status.role, issues);
        if !EXTERNAL_STATUS_ROLES.contains(&status.role.as_str()) {
            issues.push(BuildServiceIssue::new(FIELD_ROLE, "external_status_role_overclaim"));
        }
        validate_non_claims(&status.non_claims, issues);
        require_reference(&status.job_identity, &index.jobs, "job_identity", "dangling_link", issues);
        require_reference(&status.attempt_identity, &index.attempts, "attempt_identity", "dangling_link", issues);
        if !text_bounded(&status.status_id) || !text_bounded(&status.provider) || !text_bounded(&status.observed_state)
        {
            issues.push(BuildServiceIssue::new("status_id", "text_unbounded"));
        }
    }
}

fn validate_claimed_classes(bundle: &BuildServiceBundle, issues: &mut Vec<BuildServiceIssue>) {
    let claimed = &bundle.profile.claimed_observation_classes;
    if claimed.iter().any(|class| class == OBSERVATION_CLASS_BUILD) && bundle.build_results.is_empty() {
        issues.push(BuildServiceIssue::new("build_results", "claimed_class_unsupported_by_records"));
    }
    if claimed.iter().any(|class| class == OBSERVATION_CLASS_RETRY) && bundle.retry_edges.is_empty() {
        issues.push(BuildServiceIssue::new("retry_edges", "claimed_class_unsupported_by_records"));
    }
    if claimed.iter().any(|class| class == OBSERVATION_CLASS_REUSE) && bundle.reuse_edges.is_empty() {
        issues.push(BuildServiceIssue::new("reuse_edges", "claimed_class_unsupported_by_records"));
    }
    if claimed.iter().any(|class| class == OBSERVATION_CLASS_USAGE) && bundle.usage_records.is_empty() {
        issues.push(BuildServiceIssue::new("usage_records", "claimed_class_unsupported_by_records"));
    }
    if claimed.iter().any(|class| class == OBSERVATION_CLASS_EXTERNAL_STATUS) && bundle.external_statuses.is_empty() {
        issues.push(BuildServiceIssue::new("external_statuses", "claimed_class_unsupported_by_records"));
    }
}

/// Validate a build-service bundle through pure in-memory checks.
///
/// r[impl valence.evidence_ir.build_service_roles]
/// r[impl valence.evidence_ir.build_service_privacy]
/// r[impl valence.evidence_ir.build_service_non_claims]
#[must_use]
pub fn validate_build_service_bundle(bundle: &BuildServiceBundle) -> BuildServiceReport {
    let mut issues = Vec::new();
    validate_profile(&bundle.profile, &mut issues);

    let record_count = bundle.service_requests.len()
        + bundle.attempts.len()
        + bundle.transitions.len()
        + bundle.authority_decisions.len()
        + bundle.machine_classes.len()
        + bundle.resource_decisions.len()
        + bundle.resource_observations.len()
        + bundle.retry_edges.len()
        + bundle.usage_records.len()
        + bundle.build_results.len()
        + bundle.reuse_edges.len()
        + bundle.orchestration_jobs.len()
        + bundle.external_statuses.len();
    if record_count > MAX_RECORDS_PER_KIND * 13 {
        issues.push(BuildServiceIssue::new("records", "record_count_unbounded"));
    }

    let index = IdentityIndex {
        requests: bundle.service_requests.iter().map(service_request_identity).collect(),
        attempts: bundle.attempts.iter().map(attempt_identity).collect(),
        classes: bundle.machine_classes.iter().map(machine_class_identity).collect(),
        decisions: bundle.authority_decisions.iter().map(authority_decision_identity).collect(),
        observations: bundle.resource_observations.iter().map(resource_observation_identity).collect(),
        results: bundle.build_results.iter().map(build_result_identity).collect(),
        jobs: bundle.orchestration_jobs.iter().map(orchestration_identity).collect(),
    };

    reject_duplicates(&index.requests, &mut issues);
    reject_duplicates(&index.attempts, &mut issues);
    reject_duplicates(&index.classes, &mut issues);
    reject_duplicates(&index.decisions, &mut issues);
    reject_duplicates(&index.observations, &mut issues);
    reject_duplicates(&index.results, &mut issues);
    reject_duplicates(&index.jobs, &mut issues);

    for request in &bundle.service_requests {
        validate_role(request.role, &mut issues);
        validate_non_claims(&request.non_claims, &mut issues);
        if !text_bounded(&request.request_id) || !text_bounded(&request.idempotency_key) {
            issues.push(BuildServiceIssue::new("request_id", "text_unbounded"));
        }
        if !is_hex_digest(&request.concrete_request_hash) {
            issues.push(BuildServiceIssue::new("concrete_request_hash", "request_identity_malformed"));
        }
    }

    validate_attempts(bundle, &index, &mut issues);
    validate_transitions(bundle, &index, &mut issues);
    validate_authority(bundle, &index, &mut issues);
    validate_classes(bundle, &mut issues);
    validate_resource_decisions(bundle, &index, &mut issues);
    validate_observations(bundle, &index, &mut issues);
    validate_retry_edges(bundle, &index, &mut issues);
    validate_usage(bundle, &index, &mut issues);
    validate_results(bundle, &index, &mut issues);
    validate_reuse(bundle, &index, &mut issues);
    validate_orchestration(bundle, &index, &mut issues);
    validate_external_status(bundle, &index, &mut issues);
    validate_claimed_classes(bundle, &mut issues);

    // r[impl valence.evidence_ir.build_service_privacy]
    if let Ok(value) = serde_json::to_value(bundle) {
        validate_privacy(&value, &mut issues);
    }

    issues.sort();
    issues.dedup();
    issues.truncate(MAX_ISSUES);
    let valid = issues.is_empty();
    BuildServiceReport {
        valid,
        record_count: record_count as u32,
        issues,
        supported_claim: BUILD_SERVICE_SUPPORTED_CLAIM.to_owned(),
        non_claims: bundle.profile.non_claims.clone(),
    }
}
