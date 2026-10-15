//! Pure deterministic forge metadata migration and exact-revision CI admission.
//!
//! Repository, Radicle, signature, process, clock, network, and canonical-ref
//! effects remain in shells. This module validates already-loaded observations
//! and emits deterministic plans or bounded receipts.

// r[impl valence.radicle_forge_ops.migration_core]
// r[impl valence.radicle_forge_ops.attribution]
// r[impl valence.radicle_forge_ops.binding]
// r[impl valence.radicle_forge_ops.ci_admission]
// r[impl valence.radicle_forge_ops.boundaries]

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

pub const PROFILE_SCHEMA: &str = "valence.radicle-forge-operations.profile.v1";
pub const SOURCE_SCHEMA: &str = "valence.radicle-forge-operations.source-batch.v1";
pub const PLAN_SCHEMA: &str = "valence.radicle-forge-operations.plan.v1";
pub const BINDING_SCHEMA: &str = "valence.radicle-forge-operations.binding.v1";
pub const ADMISSION_SCHEMA: &str = "valence.radicle-forge-operations.ci-admission.v1";
pub const ISSUE_COB_TYPE: &str = "xyz.radicle.issue";
pub const PATCH_COB_TYPE: &str = "xyz.radicle.patch";
pub const REQUIRED_SIGNED_REFS_FEATURE: &str = "parent";
pub const REQUIRED_REVIEW_HANDLING: &str = "attributed-comment-only";
pub const CANONICAL_TARGET_REF: &str = "refs/heads/main";
pub const GIT_OID_HEX_LENGTH: usize = 40;
pub const BLAKE3_HEX_LENGTH: usize = 64;
pub const MAX_IDENTITY_BYTES: usize = 1_024;
pub const ATTRIBUTION_HEADER: &str = "<!-- valence-radicle-forge-import:v1 -->";
pub const MIGRATION_NON_CLAIM: &str = "migration records normalized source metadata and observed native COB state; it does not prove source export completeness, source actor authenticity, original signature or timestamp preservation, or semantic equivalence";
pub const REVIEW_NON_CLAIM: &str = "imported review metadata is recorded attribution only and does not create Radicle approval, merge eligibility, delegate authority, or canonical-ref authority";
pub const ADMISSION_NON_CLAIM: &str = "CI admission is bounded input for an external guarded compare-and-swap; it is not protocol-enforced mandatory CI, bypass-proof delegate behavior, canonical mutation, CI correctness, or release readiness";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeOperationsProfile {
    pub schema: String,
    pub source_schema: String,
    pub plan_schema: String,
    pub binding_schema: String,
    pub admission_schema: String,
    pub source_kinds: Vec<String>,
    pub issue_states: Vec<String>,
    pub pull_request_states: Vec<String>,
    pub review_states: Vec<String>,
    pub review_handling: String,
    pub cob_types: CobTypes,
    pub bounds: ForgeBounds,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobTypes {
    pub issue: String,
    pub pull_request: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeBounds {
    pub max_records: usize,
    pub max_events_per_record: usize,
    pub max_labels_per_record: usize,
    pub max_text_bytes: usize,
    pub max_title_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBatch {
    pub schema: String,
    pub source_system: String,
    pub repository_url: String,
    pub repository_rid: String,
    pub importer_did: String,
    pub records: Vec<SourceRecord>,
    #[serde(default)]
    pub existing_mappings: Vec<ExistingMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "record", rename_all = "snake_case")]
pub enum SourceRecord {
    Issue(SourceIssue),
    PullRequest(SourcePullRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIssue {
    pub source_key: String,
    pub number: u64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: IssueSourceState,
    pub author: SourceActor,
    pub created_at: String,
    pub updated_at: String,
    pub labels: Vec<String>,
    pub events: Vec<SourceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePullRequest {
    pub source_key: String,
    pub number: u64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: PullRequestSourceState,
    pub author: SourceActor,
    pub created_at: String,
    pub updated_at: String,
    pub labels: Vec<String>,
    pub base_oid: String,
    pub head_oid: String,
    pub target_ref: String,
    pub merged_commit_oid: Option<String>,
    pub events: Vec<SourceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceActor {
    pub source_id: String,
    pub login: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvent {
    pub event_id: String,
    pub kind: SourceEventKind,
    pub actor: SourceActor,
    pub timestamp: String,
    pub url: String,
    pub body: String,
    pub review_state: Option<ReviewSourceState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceEventKind {
    Comment,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSourceState {
    Open,
    Closed,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestSourceState {
    Open,
    Closed,
    Merged,
    Draft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSourceState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExistingMapping {
    pub source_key: String,
    pub source_record_blake3: String,
    pub target_type: String,
    pub target_object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlanningReport {
    pub schema: String,
    pub valid: bool,
    pub issues: Vec<ForgeIssue>,
    pub plan: Option<MigrationPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlan {
    pub schema: String,
    pub profile_blake3: String,
    pub source_batch_blake3: String,
    pub plan_blake3: String,
    pub source_system: String,
    pub repository_url: String,
    pub repository_rid: String,
    pub importer_did: String,
    pub records: Vec<PlannedRecord>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedRecord {
    pub source_key: String,
    pub source_record_blake3: String,
    pub target_type: String,
    pub decision: MigrationDecision,
    pub operations: Vec<PlannedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDecision {
    Create,
    VerifyExisting { target_object_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PlannedOperation {
    CreateIssue {
        title: String,
        description: String,
        labels: Vec<String>,
        state: IssueSourceState,
    },
    CreatePatch {
        title: String,
        description: String,
        labels: Vec<String>,
        lifecycle: PatchLifecycle,
        base_oid: String,
        head_oid: String,
        target_ref: String,
    },
    AppendComment {
        event_id: String,
        body: String,
    },
    VerifyExisting {
        target_object_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchLifecycle {
    Open,
    Draft,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeIssue {
    pub code: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedRecordBinding {
    pub source_key: String,
    pub source_record_blake3: String,
    pub target_type: String,
    pub target_object_id: String,
    pub entry_oids: Vec<String>,
    pub observed_state_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationBindingReceipt {
    pub schema: String,
    pub receipt_blake3: String,
    pub profile_blake3: String,
    pub source_batch_blake3: String,
    pub plan_blake3: String,
    pub repository_rid: String,
    pub importer_did: String,
    pub records: Vec<ObservedRecordBinding>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiAdmissionPolicy {
    pub schema: String,
    pub policy_blake3: String,
    pub rid: String,
    pub bot_did: String,
    pub delegates: Vec<String>,
    pub threshold: usize,
    pub required_check: String,
    pub signed_refs_feature: String,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchAdmissionFacts {
    pub patch_id: String,
    pub revision_id: String,
    pub base_oid: String,
    pub head_oid: String,
    pub target_ref: String,
    pub current_canonical_oid: String,
    pub candidate_is_descendant: bool,
    pub canonical_observation_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiEventEvidence {
    pub schema: String,
    pub event_blake3: String,
    pub job_id: String,
    pub policy_blake3: String,
    pub rid: String,
    pub trigger: String,
    pub object_oid: String,
    pub patch_id: String,
    pub revision_id: String,
    pub source_archive_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiResultEvidence {
    pub schema: String,
    pub result_blake3: String,
    pub job_id: String,
    pub rid: String,
    pub trigger: String,
    pub object_oid: String,
    pub patch_id: String,
    pub revision_id: String,
    pub disposition: String,
    pub exit_code: Option<i32>,
    pub artifact_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedBotStatusObservation {
    pub signer_did: String,
    pub signature_verified: bool,
    pub patch_id: String,
    pub revision_id: String,
    pub check_name: String,
    pub job_id: String,
    pub object_oid: String,
    pub disposition: String,
    pub artifact_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegateApprovalObservation {
    pub delegate_did: String,
    pub signature_verified: bool,
    pub revision_id: String,
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiAdmissionInput {
    pub policy: CiAdmissionPolicy,
    pub patch: PatchAdmissionFacts,
    pub event: CiEventEvidence,
    pub result: CiResultEvidence,
    pub bot_status: SignedBotStatusObservation,
    pub approvals: Vec<DelegateApprovalObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiAdmissionReport {
    pub schema: String,
    pub admitted: bool,
    pub issues: Vec<ForgeIssue>,
    pub receipt: Option<CiAdmissionReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiAdmissionReceipt {
    pub schema: String,
    pub receipt_blake3: String,
    pub policy_blake3: String,
    pub rid: String,
    pub patch_id: String,
    pub revision_id: String,
    pub current_canonical_oid: String,
    pub candidate_oid: String,
    pub job_id: String,
    pub event_blake3: String,
    pub result_blake3: String,
    pub bot_did: String,
    pub approving_delegates: Vec<String>,
    pub threshold: usize,
    pub disposition: String,
    pub claim_scope: String,
    pub required_non_claims: Vec<String>,
}

/// Parse a closed profile from JSON bytes.
///
/// # Errors
/// Returns the JSON decoder error for malformed or unknown-field input.
pub fn parse_profile(bytes: &[u8]) -> Result<ForgeOperationsProfile, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// Parse a closed normalized source batch from JSON bytes.
///
/// # Errors
/// Returns the JSON decoder error for malformed or unknown-field input.
pub fn parse_source_batch(bytes: &[u8]) -> Result<SourceBatch, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[must_use]
pub fn validate_profile(profile: &ForgeOperationsProfile) -> Vec<ForgeIssue> {
    let mut issues = Vec::new();
    if profile.schema != PROFILE_SCHEMA
        || profile.source_schema != SOURCE_SCHEMA
        || profile.plan_schema != PLAN_SCHEMA
        || profile.binding_schema != BINDING_SCHEMA
        || profile.admission_schema != ADMISSION_SCHEMA
    {
        push_issue(&mut issues, "forge-profile-schema", "profile");
    }
    if profile.source_kinds != ["issue", "pull_request"]
        || profile.issue_states != ["open", "closed", "solved"]
        || profile.pull_request_states != ["open", "closed", "merged", "draft"]
        || profile.review_states != ["approved", "changes_requested", "commented", "dismissed", "pending"]
        || profile.review_handling != REQUIRED_REVIEW_HANDLING
    {
        push_issue(&mut issues, "forge-profile-vocabulary", "profile");
    }
    if profile.cob_types.issue != ISSUE_COB_TYPE || profile.cob_types.pull_request != PATCH_COB_TYPE {
        push_issue(&mut issues, "forge-profile-cob-types", "profile.cob_types");
    }
    if profile.bounds.max_records == 0
        || profile.bounds.max_events_per_record == 0
        || profile.bounds.max_labels_per_record == 0
        || profile.bounds.max_text_bytes == 0
        || profile.bounds.max_title_bytes == 0
        || profile.bounds.max_title_bytes > profile.bounds.max_text_bytes
    {
        push_issue(&mut issues, "forge-profile-bounds", "profile.bounds");
    }
    if !has_exact_non_claims(&profile.required_non_claims) {
        push_issue(&mut issues, "forge-profile-non-claims", "profile.required_non_claims");
    }
    issues
}

/// Build a deterministic migration plan over already-loaded normalized records.
#[must_use]
pub fn plan_migration(profile: &ForgeOperationsProfile, batch: &SourceBatch) -> MigrationPlanningReport {
    let mut issues = validate_profile(profile);
    validate_batch(profile, batch, &mut issues);
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return MigrationPlanningReport {
            schema: PLAN_SCHEMA.to_owned(),
            valid: false,
            issues,
            plan: None,
        };
    }

    let profile_blake3 = canonical_blake3(profile);
    let mut normalized = batch.clone();
    normalized.records.sort_by(|left, right| source_key(left).cmp(source_key(right)));
    normalized.existing_mappings.sort_by(|left, right| left.source_key.cmp(&right.source_key));
    for record in &mut normalized.records {
        normalize_record(record);
    }
    let source_batch_blake3 = canonical_blake3(&normalized);
    let mappings = normalized
        .existing_mappings
        .iter()
        .map(|mapping| (mapping.source_key.as_str(), mapping))
        .collect::<BTreeMap<_, _>>();
    let records = normalized
        .records
        .iter()
        .map(|record| planned_record(profile, &normalized, record, &mappings))
        .collect::<Vec<_>>();
    let plan_blake3 = canonical_blake3(&(
        PLAN_SCHEMA,
        &profile_blake3,
        &source_batch_blake3,
        &normalized.source_system,
        &normalized.repository_url,
        &normalized.repository_rid,
        &normalized.importer_did,
        &records,
        &profile.required_non_claims,
    ));
    MigrationPlanningReport {
        schema: PLAN_SCHEMA.to_owned(),
        valid: true,
        issues: Vec::new(),
        plan: Some(MigrationPlan {
            schema: PLAN_SCHEMA.to_owned(),
            profile_blake3,
            source_batch_blake3,
            plan_blake3,
            source_system: normalized.source_system,
            repository_url: normalized.repository_url,
            repository_rid: normalized.repository_rid,
            importer_did: normalized.importer_did,
            records,
            required_non_claims: profile.required_non_claims.clone(),
        }),
    }
}

/// Validate that a migration plan is closed, ordered, and bound by its BLAKE3 identity.
#[must_use]
pub fn validate_migration_plan(plan: &MigrationPlan) -> Vec<ForgeIssue> {
    let mut issues = Vec::new();
    if plan.schema != PLAN_SCHEMA
        || !blake3_digest(&plan.profile_blake3)
        || !blake3_digest(&plan.source_batch_blake3)
        || !blake3_digest(&plan.plan_blake3)
        || plan.source_system.trim().is_empty()
        || !https_url(&plan.repository_url)
        || !radicle_id(&plan.repository_rid)
        || !did(&plan.importer_did)
        || !has_exact_non_claims(&plan.required_non_claims)
    {
        push_issue(&mut issues, "forge-plan-header", "plan");
    }
    let mut records = plan.records.clone();
    records.sort_by(|left, right| left.source_key.cmp(&right.source_key));
    if records.is_empty() || records != plan.records || !unique_by(&records, |record| record.source_key.clone()) {
        push_issue(&mut issues, "forge-plan-order", "plan.records");
    }
    for record in &records {
        if record.source_key.trim().is_empty()
            || !blake3_digest(&record.source_record_blake3)
            || !valid_planned_record(record)
        {
            push_issue(&mut issues, "forge-plan-record", &format!("plan.records.{}", record.source_key));
        }
    }
    if plan.plan_blake3 != expected_plan_blake3(plan) {
        push_issue(&mut issues, "forge-plan-identity", "plan.plan_blake3");
    }
    issues.sort();
    issues.dedup();
    issues
}

/// Build a deterministic source-to-target binding receipt.
///
/// # Errors
/// Returns stable issues when observations do not match the planned source and target roles.
pub fn build_binding_receipt(
    profile: &ForgeOperationsProfile,
    plan: &MigrationPlan,
    observed: Vec<ObservedRecordBinding>,
) -> Result<MigrationBindingReceipt, Vec<ForgeIssue>> {
    let mut issues = validate_profile(profile);
    issues.extend(validate_migration_plan(plan));
    if plan.profile_blake3 != canonical_blake3(profile) {
        push_issue(&mut issues, "forge-binding-plan", "plan.profile_blake3");
    }
    let planned = plan.records.iter().map(|record| (record.source_key.as_str(), record)).collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for record in &observed {
        let path = format!("records.{}", record.source_key);
        if !seen.insert(record.source_key.as_str()) {
            push_issue(&mut issues, "forge-binding-duplicate", &path);
            continue;
        }
        if !targets.insert((record.target_type.as_str(), record.target_object_id.as_str())) {
            push_issue(&mut issues, "forge-binding-duplicate-target", &path);
        }
        let Some(expected) = planned.get(record.source_key.as_str()) else {
            push_issue(&mut issues, "forge-binding-unplanned", &path);
            continue;
        };
        if record.source_record_blake3 != expected.source_record_blake3
            || record.target_type != expected.target_type
            || !git_oid(&record.target_object_id)
            || record.entry_oids.is_empty()
            || record.entry_oids.iter().any(|oid| !git_oid(oid))
            || !blake3_digest(&record.observed_state_blake3)
        {
            push_issue(&mut issues, "forge-binding-observation", &path);
        }
        if let MigrationDecision::VerifyExisting { target_object_id } = &expected.decision
            && &record.target_object_id != target_object_id
        {
            push_issue(&mut issues, "forge-binding-existing-target", &path);
        }
    }
    if seen.len() != plan.records.len() {
        push_issue(&mut issues, "forge-binding-incomplete", "records");
    }
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut records = observed;
    records.sort_by(|left, right| left.source_key.cmp(&right.source_key));
    let receipt_blake3 = canonical_blake3(&(
        BINDING_SCHEMA,
        &plan.profile_blake3,
        &plan.source_batch_blake3,
        &plan.plan_blake3,
        &plan.repository_rid,
        &plan.importer_did,
        &records,
        &plan.required_non_claims,
    ));
    Ok(MigrationBindingReceipt {
        schema: BINDING_SCHEMA.to_owned(),
        receipt_blake3,
        profile_blake3: plan.profile_blake3.clone(),
        source_batch_blake3: plan.source_batch_blake3.clone(),
        plan_blake3: plan.plan_blake3.clone(),
        repository_rid: plan.repository_rid.clone(),
        importer_did: plan.importer_did.clone(),
        records,
        required_non_claims: plan.required_non_claims.clone(),
    })
}

#[must_use]
pub fn verify_binding_receipt(profile: &ForgeOperationsProfile, receipt: &MigrationBindingReceipt) -> Vec<ForgeIssue> {
    let mut issues = validate_profile(profile);
    if receipt.schema != BINDING_SCHEMA
        || receipt.profile_blake3 != canonical_blake3(profile)
        || !blake3_digest(&receipt.source_batch_blake3)
        || !blake3_digest(&receipt.plan_blake3)
        || !radicle_id(&receipt.repository_rid)
        || !did(&receipt.importer_did)
        || !has_exact_non_claims(&receipt.required_non_claims)
    {
        push_issue(&mut issues, "forge-binding-header", "receipt");
    }
    let mut records = receipt.records.clone();
    records.sort_by(|left, right| left.source_key.cmp(&right.source_key));
    if records != receipt.records || !unique_by(&records, |record| record.source_key.clone()) {
        push_issue(&mut issues, "forge-binding-order", "receipt.records");
    }
    if !unique_by(&records, |record| format!("{}:{}", record.target_type, record.target_object_id)) {
        push_issue(&mut issues, "forge-binding-duplicate-target", "receipt.records");
    }
    for record in &records {
        if !blake3_digest(&record.source_record_blake3)
            || ![ISSUE_COB_TYPE, PATCH_COB_TYPE].contains(&record.target_type.as_str())
            || !git_oid(&record.target_object_id)
            || record.entry_oids.is_empty()
            || record.entry_oids.iter().any(|oid| !git_oid(oid))
            || !blake3_digest(&record.observed_state_blake3)
        {
            push_issue(&mut issues, "forge-binding-record", &format!("receipt.records.{}", record.source_key));
        }
    }
    let expected = canonical_blake3(&(
        BINDING_SCHEMA,
        &receipt.profile_blake3,
        &receipt.source_batch_blake3,
        &receipt.plan_blake3,
        &receipt.repository_rid,
        &receipt.importer_did,
        &records,
        &receipt.required_non_claims,
    ));
    if receipt.receipt_blake3 != expected {
        push_issue(&mut issues, "forge-binding-identity", "receipt.receipt_blake3");
    }
    issues.sort();
    issues.dedup();
    issues
}

/// Materialize exact-revision CI admission from already-loaded observations.
#[must_use]
pub fn admit_ci_revision(input: &CiAdmissionInput) -> CiAdmissionReport {
    let mut issues = Vec::new();
    validate_admission_policy(&input.policy, &mut issues);
    let policy = &input.policy;
    let patch = &input.patch;
    let event = &input.event;
    let result = &input.result;
    let status = &input.bot_status;

    if !git_oid(&patch.patch_id)
        || !git_oid(&patch.revision_id)
        || !git_oid(&patch.base_oid)
        || !git_oid(&patch.head_oid)
        || !git_oid(&patch.current_canonical_oid)
        || patch.base_oid == patch.head_oid
        || patch.target_ref != CANONICAL_TARGET_REF
    {
        push_issue(&mut issues, "forge-admission-patch", "patch");
    }
    if !patch.candidate_is_descendant || !patch.canonical_observation_current {
        push_issue(&mut issues, "forge-admission-canonical", "patch.current_canonical_oid");
    }
    if event.schema != "onix.radicle-ci-event.v1"
        || !blake3_digest(&event.event_blake3)
        || !blake3_digest(&event.job_id)
        || event.policy_blake3 != policy.policy_blake3
        || event.rid != policy.rid
        || event.trigger != "patch"
        || event.object_oid != patch.head_oid
        || event.patch_id != patch.patch_id
        || event.revision_id != patch.revision_id
        || !blake3_digest(&event.source_archive_blake3)
    {
        push_issue(&mut issues, "forge-admission-event", "event");
    }
    if result.schema != "onix.radicle-ci-result.v1"
        || !blake3_digest(&result.result_blake3)
        || result.job_id != event.job_id
        || result.rid != event.rid
        || result.trigger != event.trigger
        || result.object_oid != event.object_oid
        || result.patch_id != event.patch_id
        || result.revision_id != event.revision_id
        || result.disposition != "succeeded"
        || result.exit_code != Some(0)
        || !blake3_digest(&result.artifact_blake3)
    {
        push_issue(&mut issues, "forge-admission-result", "result");
    }
    if status.signer_did != policy.bot_did
        || !status.signature_verified
        || status.patch_id != patch.patch_id
        || status.revision_id != patch.revision_id
        || status.check_name != policy.required_check
        || status.job_id != result.job_id
        || status.object_oid != result.object_oid
        || status.disposition != result.disposition
        || status.artifact_blake3 != result.artifact_blake3
    {
        push_issue(&mut issues, "forge-admission-bot-status", "bot_status");
    }

    let mut approvals = BTreeSet::new();
    for approval in &input.approvals {
        if !approval.signature_verified
            || approval.revision_id != patch.revision_id
            || approval.verdict != "accept"
            || !policy.delegates.contains(&approval.delegate_did)
        {
            push_issue(&mut issues, "forge-admission-approval", &format!("approvals.{}", approval.delegate_did));
            continue;
        }
        if !approvals.insert(approval.delegate_did.clone()) {
            push_issue(
                &mut issues,
                "forge-admission-duplicate-approval",
                &format!("approvals.{}", approval.delegate_did),
            );
        }
    }
    if approvals.len() < policy.threshold {
        push_issue(&mut issues, "forge-admission-threshold", "approvals");
    }

    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return CiAdmissionReport {
            schema: ADMISSION_SCHEMA.to_owned(),
            admitted: false,
            issues,
            receipt: None,
        };
    }
    let approving_delegates = approvals.into_iter().collect::<Vec<_>>();
    let claim_scope = "external-guarded-compare-and-swap-input".to_owned();
    let receipt_blake3 = canonical_blake3(&(
        ADMISSION_SCHEMA,
        &policy.policy_blake3,
        &policy.rid,
        &patch.patch_id,
        &patch.revision_id,
        &patch.current_canonical_oid,
        &patch.head_oid,
        &event.job_id,
        &event.event_blake3,
        &result.result_blake3,
        &policy.bot_did,
        &approving_delegates,
        policy.threshold,
        &result.disposition,
        &claim_scope,
        &policy.required_non_claims,
    ));
    CiAdmissionReport {
        schema: ADMISSION_SCHEMA.to_owned(),
        admitted: true,
        issues: Vec::new(),
        receipt: Some(CiAdmissionReceipt {
            schema: ADMISSION_SCHEMA.to_owned(),
            receipt_blake3,
            policy_blake3: policy.policy_blake3.clone(),
            rid: policy.rid.clone(),
            patch_id: patch.patch_id.clone(),
            revision_id: patch.revision_id.clone(),
            current_canonical_oid: patch.current_canonical_oid.clone(),
            candidate_oid: patch.head_oid.clone(),
            job_id: event.job_id.clone(),
            event_blake3: event.event_blake3.clone(),
            result_blake3: result.result_blake3.clone(),
            bot_did: policy.bot_did.clone(),
            approving_delegates,
            threshold: policy.threshold,
            disposition: result.disposition.clone(),
            claim_scope,
            required_non_claims: policy.required_non_claims.clone(),
        }),
    }
}

#[must_use]
pub fn verify_ci_admission_receipt(receipt: &CiAdmissionReceipt) -> Vec<ForgeIssue> {
    let mut issues = Vec::new();
    if receipt.schema != ADMISSION_SCHEMA
        || !blake3_digest(&receipt.policy_blake3)
        || !radicle_id(&receipt.rid)
        || !git_oid(&receipt.patch_id)
        || !git_oid(&receipt.revision_id)
        || !git_oid(&receipt.current_canonical_oid)
        || !git_oid(&receipt.candidate_oid)
        || receipt.current_canonical_oid == receipt.candidate_oid
        || !blake3_digest(&receipt.job_id)
        || !blake3_digest(&receipt.event_blake3)
        || !blake3_digest(&receipt.result_blake3)
        || !did(&receipt.bot_did)
        || receipt.approving_delegates.len() < receipt.threshold
        || receipt.threshold == 0
        || receipt.disposition != "succeeded"
        || receipt.claim_scope != "external-guarded-compare-and-swap-input"
        || !has_exact_non_claims(&receipt.required_non_claims)
        || !unique_sorted(&receipt.approving_delegates)
        || receipt.approving_delegates.iter().any(|delegate| !did(delegate) || delegate == &receipt.bot_did)
    {
        push_issue(&mut issues, "forge-admission-receipt", "receipt");
    }
    let expected = canonical_blake3(&(
        ADMISSION_SCHEMA,
        &receipt.policy_blake3,
        &receipt.rid,
        &receipt.patch_id,
        &receipt.revision_id,
        &receipt.current_canonical_oid,
        &receipt.candidate_oid,
        &receipt.job_id,
        &receipt.event_blake3,
        &receipt.result_blake3,
        &receipt.bot_did,
        &receipt.approving_delegates,
        receipt.threshold,
        &receipt.disposition,
        &receipt.claim_scope,
        &receipt.required_non_claims,
    ));
    if receipt.receipt_blake3 != expected {
        push_issue(&mut issues, "forge-admission-receipt-identity", "receipt.receipt_blake3");
    }
    issues.sort();
    issues.dedup();
    issues
}

fn validate_batch(profile: &ForgeOperationsProfile, batch: &SourceBatch, issues: &mut Vec<ForgeIssue>) {
    if batch.schema != SOURCE_SCHEMA
        || !one_line(&batch.source_system)
        || !https_url(&batch.repository_url)
        || !radicle_id(&batch.repository_rid)
        || !did(&batch.importer_did)
    {
        push_issue(issues, "forge-source-header", "source");
    }
    if batch.records.is_empty() || batch.records.len() > profile.bounds.max_records {
        push_issue(issues, "forge-source-record-bound", "source.records");
    }
    if !unique_by(&batch.records, |record| source_key(record).to_owned()) {
        push_issue(issues, "forge-source-duplicate-key", "source.records");
    }
    if !unique_by(&batch.existing_mappings, |mapping| mapping.source_key.clone()) {
        push_issue(issues, "forge-source-duplicate-mapping", "source.existing_mappings");
    }
    for mapping in &batch.existing_mappings {
        if !one_line(&mapping.source_key)
            || !blake3_digest(&mapping.source_record_blake3)
            || ![ISSUE_COB_TYPE, PATCH_COB_TYPE].contains(&mapping.target_type.as_str())
            || !git_oid(&mapping.target_object_id)
        {
            push_issue(issues, "forge-source-mapping", &format!("source.existing_mappings.{}", mapping.source_key));
        }
    }
    let mappings = batch
        .existing_mappings
        .iter()
        .map(|mapping| (mapping.source_key.as_str(), mapping))
        .collect::<BTreeMap<_, _>>();
    for record in &batch.records {
        validate_record(profile, record, issues);
        if let Some(mapping) = mappings.get(source_key(record)) {
            let mut normalized = record.clone();
            normalize_record(&mut normalized);
            let record_blake3 = canonical_blake3(&normalized);
            let expected_type = match record {
                SourceRecord::Issue(_) => ISSUE_COB_TYPE,
                SourceRecord::PullRequest(_) => PATCH_COB_TYPE,
            };
            if mapping.source_record_blake3 != record_blake3 || mapping.target_type != expected_type {
                push_issue(
                    issues,
                    "forge-source-conflicting-mapping",
                    &format!("source.records.{}", source_key(record)),
                );
            }
        }
    }
}

fn validate_record(profile: &ForgeOperationsProfile, record: &SourceRecord, issues: &mut Vec<ForgeIssue>) {
    match record {
        SourceRecord::Issue(issue) => {
            validate_common_record(
                profile,
                &issue.source_key,
                issue.number,
                &issue.url,
                &issue.title,
                &issue.body,
                &issue.author,
                &issue.created_at,
                &issue.updated_at,
                &issue.labels,
                &issue.events,
                issues,
            );
        }
        SourceRecord::PullRequest(patch) => {
            validate_common_record(
                profile,
                &patch.source_key,
                patch.number,
                &patch.url,
                &patch.title,
                &patch.body,
                &patch.author,
                &patch.created_at,
                &patch.updated_at,
                &patch.labels,
                &patch.events,
                issues,
            );
            let path = format!("source.records.{}", patch.source_key);
            if !git_oid(&patch.base_oid)
                || !git_oid(&patch.head_oid)
                || patch.base_oid == patch.head_oid
                || patch.target_ref != CANONICAL_TARGET_REF
                || patch.merged_commit_oid.as_ref().is_some_and(|oid| !git_oid(oid))
                || (patch.state == PullRequestSourceState::Merged && patch.merged_commit_oid.is_none())
                || (patch.state != PullRequestSourceState::Merged && patch.merged_commit_oid.is_some())
            {
                push_issue(issues, "forge-source-patch", &path);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_common_record(
    profile: &ForgeOperationsProfile,
    source_key_value: &str,
    number: u64,
    url: &str,
    title: &str,
    body: &str,
    author: &SourceActor,
    created_at: &str,
    updated_at: &str,
    labels: &[String],
    events: &[SourceEvent],
    issues: &mut Vec<ForgeIssue>,
) {
    let path = format!("source.records.{source_key_value}");
    if !one_line(source_key_value) || number == 0 || !https_url(url) {
        push_issue(issues, "forge-source-record-identity", &path);
    }
    if !bounded_text(title, profile.bounds.max_title_bytes) || !bounded_text(body, profile.bounds.max_text_bytes) {
        push_issue(issues, "forge-source-record-text", &path);
    }
    if !valid_actor(author) || !timestamp(created_at) || !timestamp(updated_at) || created_at > updated_at {
        push_issue(issues, "forge-source-record-attribution", &path);
    }
    if labels.len() > profile.bounds.max_labels_per_record
        || labels.iter().any(|label| !one_line(label) || label.len() > profile.bounds.max_title_bytes)
        || !unique_strings(labels)
    {
        push_issue(issues, "forge-source-record-labels", &path);
    }
    if events.len() > profile.bounds.max_events_per_record || !unique_by(events, |event| event.event_id.clone()) {
        push_issue(issues, "forge-source-record-events", &path);
    }
    for event in events {
        if !one_line(&event.event_id)
            || !valid_actor(&event.actor)
            || !timestamp(&event.timestamp)
            || !https_url(&event.url)
            || !bounded_text(&event.body, profile.bounds.max_text_bytes)
            || (event.kind == SourceEventKind::Review) != event.review_state.is_some()
        {
            push_issue(issues, "forge-source-event", &format!("{path}.events.{}", event.event_id));
        }
    }
}

fn planned_record(
    profile: &ForgeOperationsProfile,
    batch: &SourceBatch,
    record: &SourceRecord,
    mappings: &BTreeMap<&str, &ExistingMapping>,
) -> PlannedRecord {
    let mut normalized = record.clone();
    normalize_record(&mut normalized);
    let source_record_blake3 = canonical_blake3(&normalized);
    let source_key_value = source_key(&normalized).to_owned();
    let target_type = match &normalized {
        SourceRecord::Issue(_) => profile.cob_types.issue.clone(),
        SourceRecord::PullRequest(_) => profile.cob_types.pull_request.clone(),
    };
    if let Some(mapping) = mappings.get(source_key_value.as_str()) {
        return PlannedRecord {
            source_key: source_key_value,
            source_record_blake3,
            target_type,
            decision: MigrationDecision::VerifyExisting {
                target_object_id: mapping.target_object_id.clone(),
            },
            operations: vec![PlannedOperation::VerifyExisting {
                target_object_id: mapping.target_object_id.clone(),
            }],
        };
    }
    let operations = match &normalized {
        SourceRecord::Issue(issue) => {
            let mut operations = vec![PlannedOperation::CreateIssue {
                title: issue.title.clone(),
                description: root_attribution(batch, &normalized),
                labels: mapped_labels(&issue.labels),
                state: issue.state,
            }];
            operations.extend(issue.events.iter().map(|event| PlannedOperation::AppendComment {
                event_id: event.event_id.clone(),
                body: event_attribution(batch, &source_key_value, event),
            }));
            operations
        }
        SourceRecord::PullRequest(patch) => {
            let lifecycle = match patch.state {
                PullRequestSourceState::Open => PatchLifecycle::Open,
                PullRequestSourceState::Draft => PatchLifecycle::Draft,
                PullRequestSourceState::Closed | PullRequestSourceState::Merged => PatchLifecycle::Archived,
            };
            let mut operations = vec![PlannedOperation::CreatePatch {
                title: patch.title.clone(),
                description: root_attribution(batch, &normalized),
                labels: mapped_labels(&patch.labels),
                lifecycle,
                base_oid: patch.base_oid.clone(),
                head_oid: patch.head_oid.clone(),
                target_ref: patch.target_ref.clone(),
            }];
            operations.extend(patch.events.iter().map(|event| PlannedOperation::AppendComment {
                event_id: event.event_id.clone(),
                body: event_attribution(batch, &source_key_value, event),
            }));
            operations
        }
    };
    PlannedRecord {
        source_key: source_key_value,
        source_record_blake3,
        target_type,
        decision: MigrationDecision::Create,
        operations,
    }
}

fn normalize_record(record: &mut SourceRecord) {
    let (labels, events) = match record {
        SourceRecord::Issue(issue) => (&mut issue.labels, &mut issue.events),
        SourceRecord::PullRequest(patch) => (&mut patch.labels, &mut patch.events),
    };
    labels.sort();
    events.sort_by(|left, right| (&left.timestamp, &left.event_id).cmp(&(&right.timestamp, &right.event_id)));
}

fn root_attribution(batch: &SourceBatch, record: &SourceRecord) -> String {
    let (url, actor, created_at, updated_at, body, labels, state) = match record {
        SourceRecord::Issue(issue) => (
            issue.url.as_str(),
            &issue.author,
            issue.created_at.as_str(),
            issue.updated_at.as_str(),
            issue.body.as_str(),
            &issue.labels,
            format!("{:?}", issue.state).to_ascii_lowercase(),
        ),
        SourceRecord::PullRequest(patch) => (
            patch.url.as_str(),
            &patch.author,
            patch.created_at.as_str(),
            patch.updated_at.as_str(),
            patch.body.as_str(),
            &patch.labels,
            format!("{:?}", patch.state).to_ascii_lowercase(),
        ),
    };
    format!(
        "{ATTRIBUTION_HEADER}\nsource-system: {}\nsource-key: {}\nsource-url: {url}\nsource-actor: {}\nsource-actor-id: {}\nsource-created-at: {created_at}\nsource-updated-at: {updated_at}\nsource-state: {state}\nsource-labels: {}\nimporter-did: {}\n\n---\n\n{body}",
        batch.source_system,
        source_key(record),
        actor_login(actor),
        actor.source_id,
        labels.join(","),
        batch.importer_did,
    )
}

fn event_attribution(batch: &SourceBatch, record_key: &str, event: &SourceEvent) -> String {
    let review = event
        .review_state
        .map(|state| format!("\nsource-review-state: {}", format!("{state:?}").to_ascii_lowercase()))
        .unwrap_or_default();
    format!(
        "{ATTRIBUTION_HEADER}\nsource-system: {}\nsource-key: {record_key}\nsource-event-id: {}\nsource-event-kind: {}\nsource-url: {}\nsource-actor: {}\nsource-actor-id: {}\nsource-timestamp: {}{review}\nimporter-did: {}\n\n---\n\n{}",
        batch.source_system,
        event.event_id,
        format!("{:?}", event.kind).to_ascii_lowercase(),
        event.url,
        actor_login(&event.actor),
        event.actor.source_id,
        event.timestamp,
        batch.importer_did,
        event.body,
    )
}

fn mapped_labels(labels: &[String]) -> Vec<String> {
    let mut mapped = labels.iter().map(|label| format!("source-{}", label_slug(label))).collect::<Vec<_>>();
    mapped.sort();
    mapped.dedup();
    mapped
}

fn label_slug(label: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in label.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !slug.is_empty() {
            slug.push('-');
            separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() { "label".to_owned() } else { slug }
}

fn validate_admission_policy(policy: &CiAdmissionPolicy, issues: &mut Vec<ForgeIssue>) {
    if policy.schema != ADMISSION_SCHEMA
        || !blake3_digest(&policy.policy_blake3)
        || !radicle_id(&policy.rid)
        || !did(&policy.bot_did)
        || !one_line(&policy.required_check)
        || policy.signed_refs_feature != REQUIRED_SIGNED_REFS_FEATURE
        || policy.threshold == 0
        || policy.threshold > policy.delegates.len()
        || !unique_sorted(&policy.delegates)
        || policy.delegates.iter().any(|delegate| !did(delegate) || delegate == &policy.bot_did)
        || !has_exact_non_claims(&policy.required_non_claims)
    {
        push_issue(issues, "forge-admission-policy", "policy");
    }
}

fn source_key(record: &SourceRecord) -> &str {
    match record {
        SourceRecord::Issue(issue) => &issue.source_key,
        SourceRecord::PullRequest(patch) => &patch.source_key,
    }
}

fn actor_login(actor: &SourceActor) -> &str {
    actor.login.as_deref().unwrap_or("<deleted>")
}

fn valid_actor(actor: &SourceActor) -> bool {
    one_line(&actor.source_id) && actor.login.as_ref().is_none_or(|login| one_line(login) && login != "<deleted>")
}

fn timestamp(value: &str) -> bool {
    value.is_ascii()
        && value.len() <= MAX_IDENTITY_BYTES
        && value.ends_with('Z')
        && value.contains('T')
        && !value.chars().any(char::is_whitespace)
}

fn https_url(value: &str) -> bool {
    value.starts_with("https://")
        && value.len() <= MAX_IDENTITY_BYTES
        && !value.chars().any(char::is_whitespace)
        && !value.contains('\0')
}

fn one_line(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && !value.contains('\0')
        && !value.chars().any(|character| character.is_control())
}

fn bounded_text(value: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
        && !value.contains('\0')
        && !value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\r' && character != '\t')
}

fn git_oid(value: &str) -> bool {
    lower_hex(value, GIT_OID_HEX_LENGTH)
}

fn blake3_digest(value: &str) -> bool {
    lower_hex(value, BLAKE3_HEX_LENGTH)
}

fn lower_hex(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn radicle_id(value: &str) -> bool {
    value.starts_with("rad:z") && value.len() <= MAX_IDENTITY_BYTES && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn did(value: &str) -> bool {
    value.starts_with("did:key:z")
        && value.len() <= MAX_IDENTITY_BYTES
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b':')
}

fn unique_strings(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().all(|value| seen.insert(value))
}

fn unique_sorted(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn unique_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().all(|value| seen.insert(key(value)))
}

fn has_exact_non_claims(non_claims: &[String]) -> bool {
    non_claims
        == [
            MIGRATION_NON_CLAIM.to_owned(),
            REVIEW_NON_CLAIM.to_owned(),
            ADMISSION_NON_CLAIM.to_owned(),
        ]
}

fn expected_plan_blake3(plan: &MigrationPlan) -> String {
    canonical_blake3(&(
        PLAN_SCHEMA,
        &plan.profile_blake3,
        &plan.source_batch_blake3,
        &plan.source_system,
        &plan.repository_url,
        &plan.repository_rid,
        &plan.importer_did,
        &plan.records,
        &plan.required_non_claims,
    ))
}

fn valid_planned_record(record: &PlannedRecord) -> bool {
    match (&record.decision, record.operations.as_slice()) {
        (MigrationDecision::Create, [PlannedOperation::CreateIssue { .. }, comments @ ..]) => {
            record.target_type == ISSUE_COB_TYPE && valid_planned_comments(comments)
        }
        (
            MigrationDecision::Create,
            [
                PlannedOperation::CreatePatch {
                    base_oid,
                    head_oid,
                    target_ref,
                    ..
                },
                comments @ ..,
            ],
        ) => {
            record.target_type == PATCH_COB_TYPE
                && git_oid(base_oid)
                && git_oid(head_oid)
                && target_ref == CANONICAL_TARGET_REF
                && valid_planned_comments(comments)
        }
        (
            MigrationDecision::VerifyExisting { target_object_id },
            [
                PlannedOperation::VerifyExisting {
                    target_object_id: operation_target,
                },
            ],
        ) => {
            (record.target_type == ISSUE_COB_TYPE || record.target_type == PATCH_COB_TYPE)
                && target_object_id == operation_target
                && git_oid(target_object_id)
        }
        _ => false,
    }
}

fn valid_planned_comments(operations: &[PlannedOperation]) -> bool {
    let mut event_ids = BTreeSet::new();
    operations.iter().all(|operation| {
        if let PlannedOperation::AppendComment { event_id, body } = operation {
            !event_id.trim().is_empty() && !body.trim().is_empty() && event_ids.insert(event_id.as_str())
        } else {
            false
        }
    })
}

fn canonical_blake3(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("serializing typed forge facts cannot fail");
    blake3::hash(&bytes).to_hex().to_string()
}

fn push_issue(issues: &mut Vec<ForgeIssue>, code: &str, path: &str) {
    issues.push(ForgeIssue {
        code: code.to_owned(),
        path: path.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    // r[verify valence.radicle_forge_ops.migration_core]
    // r[verify valence.radicle_forge_ops.attribution]
    // r[verify valence.radicle_forge_ops.binding]
    // r[verify valence.radicle_forge_ops.ci_admission]
    // r[verify valence.radicle_forge_ops.validation]

    use super::*;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const OID_A: &str = "1111111111111111111111111111111111111111";
    const OID_B: &str = "2222222222222222222222222222222222222222";
    const OID_C: &str = "3333333333333333333333333333333333333333";
    const AUTHOR_DID: &str = "did:key:z6MksnXbFoE8zkCkGWhHc8zuxpnEUhrJHv2KECRV4GSv9gkx";
    const BOT_DID: &str = "did:key:z6Mbotbotbotbotbotbotbotbotbotbotbotbotbotbotbotbot";
    const DELEGATE_A: &str = "did:key:z6MdelegateAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const DELEGATE_B: &str = "did:key:z6MdelegateBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    const MAX_RECORDS: usize = 128;
    const MAX_EVENTS: usize = 512;
    const MAX_LABELS: usize = 64;
    const MAX_TEXT: usize = 262_144;
    const MAX_TITLE: usize = 512;
    const FORBIDDEN_EFFECT_MARKERS: [&str; 12] = [
        "std::fs::",
        "std::env::",
        "std::process::",
        "std::net::",
        "std::time::SystemTime",
        "std::time::Instant",
        "git2::",
        "radicle::",
        "radicle_cob::",
        "OpenOptions",
        "Command::",
        "TcpStream",
    ];

    fn forbidden_effect_markers(source: &str) -> Vec<&'static str> {
        FORBIDDEN_EFFECT_MARKERS.into_iter().filter(|marker| source.contains(marker)).collect()
    }

    fn profile() -> ForgeOperationsProfile {
        ForgeOperationsProfile {
            schema: PROFILE_SCHEMA.to_owned(),
            source_schema: SOURCE_SCHEMA.to_owned(),
            plan_schema: PLAN_SCHEMA.to_owned(),
            binding_schema: BINDING_SCHEMA.to_owned(),
            admission_schema: ADMISSION_SCHEMA.to_owned(),
            source_kinds: vec!["issue".to_owned(), "pull_request".to_owned()],
            issue_states: vec!["open".to_owned(), "closed".to_owned(), "solved".to_owned()],
            pull_request_states: vec![
                "open".to_owned(),
                "closed".to_owned(),
                "merged".to_owned(),
                "draft".to_owned(),
            ],
            review_states: vec![
                "approved".to_owned(),
                "changes_requested".to_owned(),
                "commented".to_owned(),
                "dismissed".to_owned(),
                "pending".to_owned(),
            ],
            review_handling: REQUIRED_REVIEW_HANDLING.to_owned(),
            cob_types: CobTypes {
                issue: ISSUE_COB_TYPE.to_owned(),
                pull_request: PATCH_COB_TYPE.to_owned(),
            },
            bounds: ForgeBounds {
                max_records: MAX_RECORDS,
                max_events_per_record: MAX_EVENTS,
                max_labels_per_record: MAX_LABELS,
                max_text_bytes: MAX_TEXT,
                max_title_bytes: MAX_TITLE,
            },
            required_non_claims: vec![
                MIGRATION_NON_CLAIM.to_owned(),
                REVIEW_NON_CLAIM.to_owned(),
                ADMISSION_NON_CLAIM.to_owned(),
            ],
        }
    }

    fn actor() -> SourceActor {
        SourceActor {
            source_id: "github:42".to_owned(),
            login: Some("alice".to_owned()),
        }
    }

    fn comment(event_id: &str) -> SourceEvent {
        SourceEvent {
            event_id: event_id.to_owned(),
            kind: SourceEventKind::Comment,
            actor: actor(),
            timestamp: "2026-07-26T12:00:00Z".to_owned(),
            url: format!("https://github.com/onix/repo/issues/1#issuecomment-{event_id}"),
            body: "A source comment".to_owned(),
            review_state: None,
        }
    }

    fn issue() -> SourceRecord {
        SourceRecord::Issue(SourceIssue {
            source_key: "github:onix/repo:issue:1".to_owned(),
            number: 1,
            url: "https://github.com/onix/repo/issues/1".to_owned(),
            title: "Issue title".to_owned(),
            body: "Issue body".to_owned(),
            state: IssueSourceState::Solved,
            author: actor(),
            created_at: "2026-07-26T10:00:00Z".to_owned(),
            updated_at: "2026-07-26T13:00:00Z".to_owned(),
            labels: vec!["Help Wanted".to_owned(), "bug".to_owned()],
            events: vec![comment("event-2"), comment("event-1")],
        })
    }

    fn pull_request() -> SourceRecord {
        let mut review = comment("review-1");
        review.kind = SourceEventKind::Review;
        review.review_state = Some(ReviewSourceState::Approved);
        SourceRecord::PullRequest(SourcePullRequest {
            source_key: "github:onix/repo:pull_request:2".to_owned(),
            number: 2,
            url: "https://github.com/onix/repo/pull/2".to_owned(),
            title: "Patch title".to_owned(),
            body: "Patch body".to_owned(),
            state: PullRequestSourceState::Merged,
            author: actor(),
            created_at: "2026-07-26T10:00:00Z".to_owned(),
            updated_at: "2026-07-26T13:00:00Z".to_owned(),
            labels: vec!["enhancement".to_owned()],
            base_oid: OID_A.to_owned(),
            head_oid: OID_B.to_owned(),
            target_ref: "refs/heads/main".to_owned(),
            merged_commit_oid: Some(OID_C.to_owned()),
            events: vec![review],
        })
    }

    fn batch() -> SourceBatch {
        SourceBatch {
            schema: SOURCE_SCHEMA.to_owned(),
            source_system: "github".to_owned(),
            repository_url: "https://github.com/onix/repo".to_owned(),
            repository_rid: "rad:z2CpqLFpdP36fZXYUK5ZNWxMibpCo".to_owned(),
            importer_did: AUTHOR_DID.to_owned(),
            records: vec![pull_request(), issue()],
            existing_mappings: Vec::new(),
        }
    }

    #[test]
    fn pure_core_has_no_effect_markers_and_detector_rejects_effects() {
        let source = include_str!("radicle_forge_ops.rs");
        let production = source.split("#[cfg(test)]").next().expect("production source");
        assert!(forbidden_effect_markers(production).is_empty());
        assert_eq!(forbidden_effect_markers("std::fs::read(path); Command::new(tool);"), vec![
            "std::fs::",
            "Command::"
        ]);
    }

    #[test]
    fn plans_native_issue_and_patch_deterministically_without_promoting_review() {
        let first = plan_migration(&profile(), &batch());
        let second = plan_migration(&profile(), &batch());
        assert_eq!(first, second);
        assert!(first.valid);
        let plan = first.plan.expect("valid plan");
        assert_eq!(plan.records.len(), 2);
        assert_eq!(plan.records[0].target_type, ISSUE_COB_TYPE);
        assert_eq!(plan.records[1].target_type, PATCH_COB_TYPE);
        let patch_json = serde_json::to_string(&plan.records[1]).expect("serialize patch plan");
        assert!(patch_json.contains("source-review-state: approved"));
        assert!(!patch_json.contains("\"operation\":\"review\""));
        assert!(!patch_json.contains("merge_approval"));
    }

    #[test]
    fn rejects_duplicate_keys_malformed_patch_and_review_shape() {
        let mut duplicate = batch();
        duplicate.records.push(issue());
        let report = plan_migration(&profile(), &duplicate);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "forge-source-duplicate-key"));

        let mut malformed = batch();
        let SourceRecord::PullRequest(patch) = &mut malformed.records[0] else {
            panic!("fixture patch")
        };
        patch.head_oid = "bad".to_owned();
        patch.events[0].review_state = None;
        let report = plan_migration(&profile(), &malformed);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "forge-source-patch"));
        assert!(report.issues.iter().any(|issue| issue.code == "forge-source-event"));
    }

    #[test]
    fn exact_existing_mapping_is_verify_only_and_conflict_fails() {
        let initial = plan_migration(&profile(), &batch());
        let plan = initial.plan.expect("valid initial plan");
        let source = &plan.records[0];
        let mut replay = batch();
        replay.existing_mappings.push(ExistingMapping {
            source_key: source.source_key.clone(),
            source_record_blake3: source.source_record_blake3.clone(),
            target_type: source.target_type.clone(),
            target_object_id: OID_C.to_owned(),
        });
        let replay_plan = plan_migration(&profile(), &replay).plan.expect("replay plan");
        assert!(matches!(replay_plan.records[0].decision, MigrationDecision::VerifyExisting { .. }));

        replay.existing_mappings[0].source_record_blake3 = DIGEST_A.to_owned();
        let report = plan_migration(&profile(), &replay);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "forge-source-conflicting-mapping"));
    }

    #[test]
    fn binding_receipt_replays_and_tampering_fails() {
        let plan = plan_migration(&profile(), &batch()).plan.expect("plan");
        assert!(validate_migration_plan(&plan).is_empty());
        let mut mutated_plan = plan.clone();
        mutated_plan.records[0].source_key.push_str(":mutated");
        assert!(validate_migration_plan(&mutated_plan).iter().any(|issue| issue.code == "forge-plan-identity"));
        assert!(build_binding_receipt(&profile(), &mutated_plan, Vec::new()).is_err());

        let observed = plan
            .records
            .iter()
            .map(|record| ObservedRecordBinding {
                source_key: record.source_key.clone(),
                source_record_blake3: record.source_record_blake3.clone(),
                target_type: record.target_type.clone(),
                target_object_id: OID_C.to_owned(),
                entry_oids: vec![OID_C.to_owned()],
                observed_state_blake3: DIGEST_B.to_owned(),
            })
            .collect();
        let receipt = build_binding_receipt(&profile(), &plan, observed).expect("binding");
        assert!(verify_binding_receipt(&profile(), &receipt).is_empty());
        let mut tampered = receipt;
        tampered.records[0].observed_state_blake3 = DIGEST_A.to_owned();
        assert!(
            verify_binding_receipt(&profile(), &tampered)
                .iter()
                .any(|issue| issue.code == "forge-binding-identity")
        );
    }

    fn admission_input() -> CiAdmissionInput {
        let delegates = vec![DELEGATE_A.to_owned(), DELEGATE_B.to_owned()];
        CiAdmissionInput {
            policy: CiAdmissionPolicy {
                schema: ADMISSION_SCHEMA.to_owned(),
                policy_blake3: DIGEST_A.to_owned(),
                rid: "rad:z2CpqLFpdP36fZXYUK5ZNWxMibpCo".to_owned(),
                bot_did: BOT_DID.to_owned(),
                delegates: delegates.clone(),
                threshold: 2,
                required_check: "onix/bounded-exec".to_owned(),
                signed_refs_feature: REQUIRED_SIGNED_REFS_FEATURE.to_owned(),
                required_non_claims: vec![
                    MIGRATION_NON_CLAIM.to_owned(),
                    REVIEW_NON_CLAIM.to_owned(),
                    ADMISSION_NON_CLAIM.to_owned(),
                ],
            },
            patch: PatchAdmissionFacts {
                patch_id: OID_A.to_owned(),
                revision_id: OID_B.to_owned(),
                base_oid: OID_A.to_owned(),
                head_oid: OID_C.to_owned(),
                target_ref: "refs/heads/main".to_owned(),
                current_canonical_oid: OID_B.to_owned(),
                candidate_is_descendant: true,
                canonical_observation_current: true,
            },
            event: CiEventEvidence {
                schema: "onix.radicle-ci-event.v1".to_owned(),
                event_blake3: DIGEST_A.to_owned(),
                job_id: DIGEST_B.to_owned(),
                policy_blake3: DIGEST_A.to_owned(),
                rid: "rad:z2CpqLFpdP36fZXYUK5ZNWxMibpCo".to_owned(),
                trigger: "patch".to_owned(),
                object_oid: OID_C.to_owned(),
                patch_id: OID_A.to_owned(),
                revision_id: OID_B.to_owned(),
                source_archive_blake3: DIGEST_A.to_owned(),
            },
            result: CiResultEvidence {
                schema: "onix.radicle-ci-result.v1".to_owned(),
                result_blake3: DIGEST_B.to_owned(),
                job_id: DIGEST_B.to_owned(),
                rid: "rad:z2CpqLFpdP36fZXYUK5ZNWxMibpCo".to_owned(),
                trigger: "patch".to_owned(),
                object_oid: OID_C.to_owned(),
                patch_id: OID_A.to_owned(),
                revision_id: OID_B.to_owned(),
                disposition: "succeeded".to_owned(),
                exit_code: Some(0),
                artifact_blake3: DIGEST_A.to_owned(),
            },
            bot_status: SignedBotStatusObservation {
                signer_did: BOT_DID.to_owned(),
                signature_verified: true,
                patch_id: OID_A.to_owned(),
                revision_id: OID_B.to_owned(),
                check_name: "onix/bounded-exec".to_owned(),
                job_id: DIGEST_B.to_owned(),
                object_oid: OID_C.to_owned(),
                disposition: "succeeded".to_owned(),
                artifact_blake3: DIGEST_A.to_owned(),
            },
            approvals: delegates
                .into_iter()
                .map(|delegate_did| DelegateApprovalObservation {
                    delegate_did,
                    signature_verified: true,
                    revision_id: OID_B.to_owned(),
                    verdict: "accept".to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn admits_exact_successful_revision_and_verifies_receipt() {
        let report = admit_ci_revision(&admission_input());
        assert!(report.admitted, "{:?}", report.issues);
        let receipt = report.receipt.expect("admission receipt");
        assert!(verify_ci_admission_receipt(&receipt).is_empty());
        assert_eq!(receipt.approving_delegates, vec![DELEGATE_A, DELEGATE_B]);
    }

    #[test]
    fn rejects_failed_unsigned_duplicate_wrong_revision_and_non_descendant_admission() {
        let mut failed = admission_input();
        failed.result.disposition = "timed_out".to_owned();
        assert!(admit_ci_revision(&failed).issues.iter().any(|issue| issue.code == "forge-admission-result"));

        let mut unsigned = admission_input();
        unsigned.bot_status.signature_verified = false;
        assert!(admit_ci_revision(&unsigned).issues.iter().any(|issue| issue.code == "forge-admission-bot-status"));

        let mut duplicate = admission_input();
        duplicate.approvals[1] = duplicate.approvals[0].clone();
        let issues = admit_ci_revision(&duplicate).issues;
        assert!(issues.iter().any(|issue| issue.code == "forge-admission-duplicate-approval"));
        assert!(issues.iter().any(|issue| issue.code == "forge-admission-threshold"));

        let mut wrong_revision = admission_input();
        wrong_revision.approvals[0].revision_id = OID_A.to_owned();
        assert!(
            admit_ci_revision(&wrong_revision)
                .issues
                .iter()
                .any(|issue| issue.code == "forge-admission-approval")
        );

        let mut non_descendant = admission_input();
        non_descendant.patch.candidate_is_descendant = false;
        assert!(
            admit_ci_revision(&non_descendant)
                .issues
                .iter()
                .any(|issue| issue.code == "forge-admission-canonical")
        );
    }

    #[test]
    fn rejects_bot_delegate_and_weakened_admission_non_claims() {
        let mut bot_delegate = admission_input();
        bot_delegate.policy.delegates[0] = BOT_DID.to_owned();
        assert!(admit_ci_revision(&bot_delegate).issues.iter().any(|issue| issue.code == "forge-admission-policy"));

        let mut weakened = admission_input();
        weakened.policy.required_non_claims.pop();
        assert!(admit_ci_revision(&weakened).issues.iter().any(|issue| issue.code == "forge-admission-policy"));
    }
}
