//! Pure Valence identity, authorization, reduction, and receipt model for Radicle COB evidence
//! sidecars.
//!
//! r[impl valence.radicle_cob_evidence.sidecar_profile]
//! r[impl valence.radicle_cob_evidence.pure_core]
//! r[impl valence.radicle_cob_evidence.attachment_identity]
//! r[impl valence.radicle_cob_evidence.digest_domains]
//! r[impl valence.radicle_cob_evidence.payload_modes]
//! r[impl valence.radicle_cob_evidence.binding_receipt]
//! r[impl valence.radicle_cob_evidence.role_policy]
//! r[impl valence.radicle_cob_evidence.authority]
//! r[impl valence.radicle_cob_evidence.convergence]
//! r[impl valence.radicle_cob_evidence.snapshot_handoff]
//! r[impl valence.radicle_cob_evidence.privacy_retention]
//! r[impl valence.radicle_cob_evidence.final_validation]

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::EvidenceArtifact;
use crate::evidence_ir::REQUIRED_NON_CLAIM;
use crate::evidence_ir::VerificationRole;
use crate::evidence_ir::build_evidence_artifact;
use crate::evidence_ir::check_evidence_artifact;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const COB_EVIDENCE_TYPE: &str = "org.onixresearch.valence.evidence";
pub const PROFILE_SCHEMA: &str = "valence.radicle-cob-evidence.profile.v1";
pub const ROOT_SCHEMA: &str = "valence.radicle-cob-evidence.root.v1";
pub const EVENT_SCHEMA: &str = "valence.radicle-cob-evidence.event.v1";
pub const BINDING_SCHEMA: &str = "valence.radicle-cob-evidence.binding.v1";
pub const SNAPSHOT_SCHEMA: &str = "valence.radicle-cob-evidence.snapshot.v1";
pub const AUTHORIZATION_SCHEMA: &str = "valence.radicle-cob-evidence.authorization.v1";
pub const SNAPSHOT_ARTIFACT_KIND: &str = "radicle-cob-evidence-snapshot-v1";
pub const MAX_EMBEDDED_ARTIFACT_BYTES: u64 = 65_536;
pub const EMBEDDED_ARTIFACT_MEDIA_TYPE: &str = "application/json";
pub const EVIDENCE_NON_CLAIM: &str = "attaching evidence records identity and linkage only; it does not prove target correctness, authorize a waiver or disposition, satisfy a requirement, or establish lifecycle or release readiness";
pub const RETENTION_NON_CLAIM: &str = "supersession and revocation change accepted state but do not promise deletion of replicated Git or Radicle history";
pub const CONFIDENTIALITY_NON_CLAIM: &str =
    "Radicle selective replication is not an at-rest encryption or secret-storage guarantee";
pub const SUPPORTED_CLAIM: &str = "the saved artifact records deterministic identity, linkage, signature, authorization-input, and reduction facts for the supplied local COB history";
const BLAKE3_PREFIX: &str = "b3:";
const BLAKE3_HEX_LENGTH: usize = 64;
const GIT_OID_HEX_LENGTH: usize = 40;
const RADICLE_RID_PREFIX: &str = "rad:";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CobIssue {
    pub field: String,
    pub code: String,
    pub entry_oid: Option<String>,
}

impl CobIssue {
    fn new(field: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            entry_oid: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentRelationship {
    ContextFor,
    DispositionFor,
    EvidenceAgainst,
    EvidenceFor,
}

impl AttachmentRelationship {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContextFor => "context_for",
            Self::DispositionFor => "disposition_for",
            Self::EvidenceAgainst => "evidence_against",
            Self::EvidenceFor => "evidence_for",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CobOperation {
    Attach,
    Initialize,
    Revoke,
    Supersede,
}

impl CobOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attach => "attach",
            Self::Initialize => "initialize",
            Self::Revoke => "revoke",
            Self::Supersede => "supersede",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidentialityClass {
    Public,
    Restricted,
    Secret,
    Regulated,
    DeletionDependent,
}

impl ConfidentialityClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Restricted => "restricted",
            Self::Secret => "secret",
            Self::Regulated => "regulated",
            Self::DeletionDependent => "deletion_dependent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorRoleProfile {
    pub role: String,
    pub operations: Vec<CobOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedPolicy {
    pub max_bytes: u64,
    pub media_types: Vec<String>,
    pub allowed_reference_confidentiality: Vec<ConfidentialityClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobEvidenceProfile {
    pub schema: String,
    pub cob_type: String,
    pub root_schema: String,
    pub event_schema: String,
    pub binding_schema: String,
    pub snapshot_schema: String,
    pub authorization_schema: String,
    pub relationships: Vec<AttachmentRelationship>,
    pub actor_roles: Vec<ActorRoleProfile>,
    pub embed_policy: EmbedPolicy,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobTargetRef {
    pub repository_rid: String,
    pub type_name: String,
    pub object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedTargetState {
    pub tips: Vec<String>,
    pub state_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    pub artifact_kind: String,
    pub stable_id: String,
    pub canonical_hash: String,
    pub verification_role: VerificationRole,
}

impl EvidenceReference {
    #[must_use]
    pub fn from_artifact(artifact: &EvidenceArtifact) -> Self {
        Self {
            artifact_kind: artifact.artifact_kind.clone(),
            stable_id: artifact.stable_id.clone(),
            canonical_hash: artifact.canonical_hash.clone(),
            verification_role: artifact.verification_role,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AttachmentPayload {
    ReferenceOnly {
        locator: String,
        media_type: String,
        size_bytes: u64,
        raw_blake3: String,
    },
    Embedded {
        name: String,
        git_oid: String,
        media_type: String,
        size_bytes: u64,
        raw_blake3: String,
    },
}

impl AttachmentPayload {
    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        match self {
            Self::ReferenceOnly { size_bytes, .. } | Self::Embedded { size_bytes, .. } => *size_bytes,
        }
    }

    #[must_use]
    pub fn raw_blake3(&self) -> &str {
        match self {
            Self::ReferenceOnly { raw_blake3, .. } | Self::Embedded { raw_blake3, .. } => raw_blake3,
        }
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        match self {
            Self::ReferenceOnly { media_type, .. } | Self::Embedded { media_type, .. } => media_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobEvidenceAttachment {
    pub schema: String,
    pub profile_hash: String,
    pub target_key: String,
    pub attachment_id: String,
    pub relationship: AttachmentRelationship,
    pub artifact: EvidenceReference,
    pub observed_target_state: ObservedTargetState,
    pub payload: AttachmentPayload,
    pub confidentiality: ConfidentialityClass,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarRoot {
    pub schema: String,
    pub target: CobTargetRef,
    pub target_key: String,
    pub profile_hash: String,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupersedeAction {
    pub profile_hash: String,
    pub attachment_id: String,
    pub replacement_attachment_id: String,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeAction {
    pub profile_hash: String,
    pub attachment_id: String,
    pub reason: String,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CobEventAction {
    Initialize(SidecarRoot),
    Attach(CobEvidenceAttachment),
    Supersede(SupersedeAction),
    Revoke(RevokeAction),
}

impl CobEventAction {
    #[must_use]
    pub const fn operation(&self) -> CobOperation {
        match self {
            Self::Initialize(_) => CobOperation::Initialize,
            Self::Attach(_) => CobOperation::Attach,
            Self::Supersede(_) => CobOperation::Supersede,
            Self::Revoke(_) => CobOperation::Revoke,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobEvent {
    pub schema: String,
    pub action: CobEventAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBinding {
    pub signer: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobAuthorizationProfile {
    pub schema: String,
    pub sidecar_profile_hash: String,
    pub bindings: Vec<ActorBinding>,
    pub authorization_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarObservation {
    pub object_id: String,
    pub tips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedCobEvent {
    pub sidecar_object_id: String,
    pub entry_oid: String,
    pub operation_index: u32,
    pub parent_oids: Vec<String>,
    pub related_oids: Vec<String>,
    pub signer: String,
    pub signature_valid: bool,
    pub event: Option<CobEvent>,
    pub decode_issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedArtifact {
    pub attachment_id: String,
    pub artifact: EvidenceArtifact,
    pub size_bytes: u64,
    pub raw_blake3: String,
    pub git_oid: Option<String>,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobReductionInput {
    pub profile: CobEvidenceProfile,
    pub authorization: CobAuthorizationProfile,
    pub target: CobTargetRef,
    pub current_target_state: ObservedTargetState,
    pub sidecars: Vec<SidecarObservation>,
    pub events: Vec<ObservedCobEvent>,
    pub resolutions: Vec<ResolvedArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentStatus {
    Active,
    Superseded { replacement_attachment_id: String },
    Revoked { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedAttachment {
    pub attachment: CobEvidenceAttachment,
    pub source_entries: Vec<String>,
    pub signers: Vec<String>,
    pub status: AttachmentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectedEvent {
    pub sidecar_object_id: String,
    pub entry_oid: String,
    pub operation_index: u32,
    pub signer: String,
    pub issue_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobConflict {
    pub attachment_id: String,
    pub event_entries: Vec<String>,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CobSnapshot {
    pub schema: String,
    pub target: CobTargetRef,
    pub target_key: String,
    pub current_target_state: ObservedTargetState,
    pub sidecars: Vec<SidecarObservation>,
    pub accepted_attachments: Vec<AcceptedAttachment>,
    pub rejected_events: Vec<RejectedEvent>,
    pub conflicts: Vec<CobConflict>,
    pub profile_hash: String,
    pub authorization_hash: String,
    pub verification_role: VerificationRole,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
    pub materialized_state_hash: String,
    pub receipt_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CobReductionReport {
    pub valid: bool,
    pub issues: Vec<CobIssue>,
    pub snapshot: Option<CobSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingReceipt {
    pub schema: String,
    pub target_key: String,
    pub attachment_id: String,
    pub artifact_stable_id: String,
    pub artifact_canonical_hash: String,
    pub entry_oid: String,
    pub embed_oid: Option<String>,
    pub profile_hash: String,
    pub verification_role: VerificationRole,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
    pub receipt_hash: String,
}

#[derive(Serialize)]
struct AttachmentIdentityMaterial<'a> {
    schema: &'a str,
    profile_hash: &'a str,
    target_key: &'a str,
    relationship: AttachmentRelationship,
    artifact: &'a EvidenceReference,
    observed_target_state: &'a ObservedTargetState,
    payload: &'a AttachmentPayload,
    confidentiality: ConfidentialityClass,
    verification_role: &'a str,
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct AuthorizationIdentityMaterial<'a> {
    schema: &'a str,
    sidecar_profile_hash: &'a str,
    bindings: &'a [ActorBinding],
}

#[derive(Serialize)]
struct BindingIdentityMaterial<'a> {
    schema: &'a str,
    target_key: &'a str,
    attachment_id: &'a str,
    artifact_stable_id: &'a str,
    artifact_canonical_hash: &'a str,
    entry_oid: &'a str,
    embed_oid: &'a Option<String>,
    profile_hash: &'a str,
    verification_role: &'a str,
    supported_claim: &'a str,
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct SnapshotStateMaterial<'a> {
    target: &'a CobTargetRef,
    target_key: &'a str,
    current_target_state: &'a ObservedTargetState,
    sidecars: &'a [SidecarObservation],
    accepted_attachments: &'a [AcceptedAttachment],
    rejected_events: &'a [RejectedEvent],
    conflicts: &'a [CobConflict],
    profile_hash: &'a str,
    authorization_hash: &'a str,
}

#[derive(Serialize)]
struct SnapshotReceiptMaterial<'a> {
    schema: &'a str,
    state: SnapshotStateMaterial<'a>,
    materialized_state_hash: &'a str,
    verification_role: &'a str,
    supported_claim: &'a str,
    non_claims: &'a [String],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EventKey {
    entry_oid: String,
    operation_index: u32,
}

#[derive(Debug, Clone)]
struct CandidateAttachment {
    attachment: CobEvidenceAttachment,
    source_keys: Vec<EventKey>,
    signers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum DispositionKind {
    Supersede(String),
    Revoke(String),
}

#[derive(Debug, Clone)]
struct Disposition {
    key: EventKey,
    target_attachment_id: String,
    kind: DispositionKind,
}

#[must_use]
pub fn profile_identity(profile: &CobEvidenceProfile) -> String {
    typed_blake3(profile)
}

#[must_use]
pub fn validate_profile(profile: &CobEvidenceProfile) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    for (field, actual, expected) in [
        ("schema", profile.schema.as_str(), PROFILE_SCHEMA),
        ("cob_type", profile.cob_type.as_str(), COB_EVIDENCE_TYPE),
        ("root_schema", profile.root_schema.as_str(), ROOT_SCHEMA),
        ("event_schema", profile.event_schema.as_str(), EVENT_SCHEMA),
        ("binding_schema", profile.binding_schema.as_str(), BINDING_SCHEMA),
        ("snapshot_schema", profile.snapshot_schema.as_str(), SNAPSHOT_SCHEMA),
        ("authorization_schema", profile.authorization_schema.as_str(), AUTHORIZATION_SCHEMA),
    ] {
        if actual != expected {
            issues.push(CobIssue::new(field, "unsupported_profile_value"));
        }
    }
    require_unique_nonempty(&profile.relationships, "relationships", &mut issues);
    let expected_relationships = vec![
        AttachmentRelationship::ContextFor,
        AttachmentRelationship::DispositionFor,
        AttachmentRelationship::EvidenceAgainst,
        AttachmentRelationship::EvidenceFor,
    ];
    if profile.relationships != expected_relationships {
        issues.push(CobIssue::new("relationships", "unsupported_relationship_vocabulary"));
    }
    require_unique_nonempty(&profile.actor_roles, "actor_roles", &mut issues);
    require_unique_nonempty(&profile.embed_policy.media_types, "embed_policy.media_types", &mut issues);
    require_unique_nonempty(
        &profile.embed_policy.allowed_reference_confidentiality,
        "embed_policy.allowed_reference_confidentiality",
        &mut issues,
    );
    if profile.embed_policy.max_bytes != MAX_EMBEDDED_ARTIFACT_BYTES {
        issues.push(CobIssue::new("embed_policy.max_bytes", "unsupported_embed_limit"));
    }
    if profile.embed_policy.media_types != [EMBEDDED_ARTIFACT_MEDIA_TYPE] {
        issues.push(CobIssue::new("embed_policy.media_types", "unsupported_embed_media_types"));
    }
    if profile.embed_policy.allowed_reference_confidentiality
        != [ConfidentialityClass::Public, ConfidentialityClass::Restricted]
    {
        issues.push(CobIssue::new(
            "embed_policy.allowed_reference_confidentiality",
            "unsupported_confidentiality_vocabulary",
        ));
    }
    let mut role_names = BTreeSet::new();
    let expected_roles = vec![
        ActorRoleProfile {
            role: "attacher".to_owned(),
            operations: vec![CobOperation::Attach, CobOperation::Initialize],
        },
        ActorRoleProfile {
            role: "maintainer".to_owned(),
            operations: vec![
                CobOperation::Attach,
                CobOperation::Initialize,
                CobOperation::Revoke,
                CobOperation::Supersede,
            ],
        },
    ];
    if profile.actor_roles != expected_roles {
        issues.push(CobIssue::new("actor_roles", "unsupported_actor_role_vocabulary"));
    }
    for role in &profile.actor_roles {
        if !valid_text(&role.role) {
            issues.push(CobIssue::new("actor_roles.role", "invalid_role"));
        }
        if !role_names.insert(role.role.as_str()) {
            issues.push(CobIssue::new("actor_roles.role", "duplicate_role"));
        }
        require_unique_nonempty(&role.operations, "actor_roles.operations", &mut issues);
    }
    validate_non_claims(&profile.required_non_claims, &profile.required_non_claims, &mut issues);
    let expected_non_claims = [EVIDENCE_NON_CLAIM, RETENTION_NON_CLAIM, CONFIDENTIALITY_NON_CLAIM];
    for required in expected_non_claims {
        if !profile.required_non_claims.iter().any(|claim| claim == required) {
            issues.push(CobIssue::new("required_non_claims", "profile_non_claim_missing"));
        }
    }
    if profile.required_non_claims.len() != expected_non_claims.len() {
        issues.push(CobIssue::new("required_non_claims", "unsupported_non_claim_vocabulary"));
    }
    sort_issues(&mut issues);
    issues
}

pub fn target_key(target: &CobTargetRef) -> Result<String, Vec<CobIssue>> {
    let mut issues = validate_target(target);
    if !issues.is_empty() {
        sort_issues(&mut issues);
        return Err(issues);
    }
    Ok(typed_blake3(target))
}

pub fn build_observed_target_state(
    target: &CobTargetRef,
    tips: &[String],
) -> Result<ObservedTargetState, Vec<CobIssue>> {
    let mut issues = validate_target(target);
    let mut canonical_tips = tips.to_vec();
    canonical_tips.sort();
    canonical_tips.dedup();
    if canonical_tips.is_empty() {
        issues.push(CobIssue::new("current_target_state.tips", "target_tips_empty"));
    }
    for tip in &canonical_tips {
        validate_git_oid(tip, "current_target_state.tips", &mut issues);
    }
    if !issues.is_empty() {
        sort_issues(&mut issues);
        return Err(issues);
    }
    let target_key = target_key(target).expect("validated target has an identity");
    let material = serde_json::json!({"target_key": target_key, "tips": &canonical_tips});
    Ok(ObservedTargetState {
        tips: canonical_tips,
        state_hash: typed_blake3(&material),
    })
}

#[must_use]
pub fn build_authorization_profile(
    sidecar_profile_hash: &str,
    mut bindings: Vec<ActorBinding>,
) -> CobAuthorizationProfile {
    bindings.sort_by(|left, right| (&left.signer, &left.role).cmp(&(&right.signer, &right.role)));
    let material = AuthorizationIdentityMaterial {
        schema: AUTHORIZATION_SCHEMA,
        sidecar_profile_hash,
        bindings: &bindings,
    };
    CobAuthorizationProfile {
        schema: AUTHORIZATION_SCHEMA.to_owned(),
        sidecar_profile_hash: sidecar_profile_hash.to_owned(),
        authorization_hash: typed_blake3(&material),
        bindings,
    }
}

#[must_use]
pub fn validate_authorization_profile(
    profile: &CobEvidenceProfile,
    authorization: &CobAuthorizationProfile,
) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    if authorization.schema != profile.authorization_schema {
        issues.push(CobIssue::new("authorization.schema", "unsupported_authorization_schema"));
    }
    let expected_profile_hash = profile_identity(profile);
    if authorization.sidecar_profile_hash != expected_profile_hash {
        issues.push(CobIssue::new("authorization.sidecar_profile_hash", "stale_sidecar_profile_hash"));
    }
    let mut canonical_bindings = authorization.bindings.clone();
    canonical_bindings.sort_by(|left, right| (&left.signer, &left.role).cmp(&(&right.signer, &right.role)));
    let material = AuthorizationIdentityMaterial {
        schema: &authorization.schema,
        sidecar_profile_hash: &authorization.sidecar_profile_hash,
        bindings: &canonical_bindings,
    };
    if authorization.authorization_hash != typed_blake3(&material) {
        issues.push(CobIssue::new("authorization.authorization_hash", "stale_authorization_hash"));
    }
    let known_roles = profile.actor_roles.iter().map(|role| role.role.as_str()).collect::<BTreeSet<_>>();
    let mut signer_roles = BTreeMap::new();
    for binding in &authorization.bindings {
        if !valid_signer(&binding.signer) {
            issues.push(CobIssue::new("authorization.bindings.signer", "invalid_signer"));
        }
        if !known_roles.contains(binding.role.as_str()) {
            issues.push(CobIssue::new("authorization.bindings.role", "unknown_actor_role"));
        }
        if signer_roles.insert(binding.signer.as_str(), binding.role.as_str()).is_some() {
            issues.push(CobIssue::new("authorization.bindings.signer", "duplicate_actor_binding"));
        }
    }
    sort_issues(&mut issues);
    issues
}

#[allow(clippy::too_many_arguments)]
pub fn build_attachment(
    profile: &CobEvidenceProfile,
    target: &CobTargetRef,
    relationship: AttachmentRelationship,
    artifact: EvidenceReference,
    observed_target_state: ObservedTargetState,
    payload: AttachmentPayload,
    confidentiality: ConfidentialityClass,
    non_claims: Vec<String>,
) -> Result<CobEvidenceAttachment, Vec<CobIssue>> {
    let mut issues = validate_profile(profile);
    let target_key = match target_key(target) {
        Ok(key) => key,
        Err(mut target_issues) => {
            issues.append(&mut target_issues);
            String::new()
        }
    };
    let mut attachment = CobEvidenceAttachment {
        schema: EVENT_SCHEMA.to_owned(),
        profile_hash: profile_identity(profile),
        target_key,
        attachment_id: String::new(),
        relationship,
        artifact,
        observed_target_state,
        payload,
        confidentiality,
        verification_role: VerificationRole::RecordedOnly,
        non_claims,
    };
    attachment.attachment_id = attachment_identity(&attachment);
    issues.extend(validate_attachment_structure(profile, &attachment));
    sort_issues(&mut issues);
    if issues.is_empty() { Ok(attachment) } else { Err(issues) }
}

#[must_use]
pub fn attachment_identity(attachment: &CobEvidenceAttachment) -> String {
    let material = AttachmentIdentityMaterial {
        schema: &attachment.schema,
        profile_hash: &attachment.profile_hash,
        target_key: &attachment.target_key,
        relationship: attachment.relationship,
        artifact: &attachment.artifact,
        observed_target_state: &attachment.observed_target_state,
        payload: &attachment.payload,
        confidentiality: attachment.confidentiality,
        verification_role: attachment.verification_role.as_str(),
        non_claims: &attachment.non_claims,
    };
    typed_blake3(&material)
}

#[must_use]
pub fn validate_attachment_structure(
    profile: &CobEvidenceProfile,
    attachment: &CobEvidenceAttachment,
) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    if attachment.schema != profile.event_schema {
        issues.push(CobIssue::new("attachment.schema", "unsupported_event_schema"));
    }
    if attachment.profile_hash != profile_identity(profile) {
        issues.push(CobIssue::new("attachment.profile_hash", "stale_profile_hash"));
    }
    validate_blake3(&attachment.target_key, "attachment.target_key", &mut issues);
    validate_blake3(&attachment.attachment_id, "attachment.attachment_id", &mut issues);
    validate_blake3(&attachment.artifact.canonical_hash, "attachment.artifact.canonical_hash", &mut issues);
    if !valid_text(&attachment.artifact.artifact_kind) {
        issues.push(CobIssue::new("attachment.artifact.artifact_kind", "invalid_artifact_kind"));
    }
    if !valid_text(&attachment.artifact.stable_id) {
        issues.push(CobIssue::new("attachment.artifact.stable_id", "invalid_artifact_stable_id"));
    }
    if !profile.relationships.contains(&attachment.relationship) {
        issues.push(CobIssue::new("attachment.relationship", "unsupported_relationship"));
    }
    if attachment.verification_role != VerificationRole::RecordedOnly {
        issues.push(CobIssue::new("attachment.verification_role", "unsupported_role_promotion"));
    }
    validate_non_claims(&attachment.non_claims, &profile.required_non_claims, &mut issues);
    validate_observed_target_state(&attachment.observed_target_state, &attachment.target_key, &mut issues);
    validate_payload(profile, attachment, &mut issues);
    if attachment.attachment_id != attachment_identity(attachment) {
        issues.push(CobIssue::new("attachment.attachment_id", "stale_attachment_identity"));
    }
    sort_issues(&mut issues);
    issues
}

#[must_use]
pub fn validate_event(profile: &CobEvidenceProfile, event: &CobEvent) -> Vec<CobIssue> {
    let mut issues = validate_profile(profile);
    if event.schema != profile.event_schema {
        issues.push(CobIssue::new("event.schema", "unsupported_event_schema"));
    }
    match &event.action {
        CobEventAction::Initialize(root) => {
            if root.schema != profile.root_schema {
                issues.push(CobIssue::new("root.schema", "unsupported_root_schema"));
            }
            issues.extend(validate_target(&root.target));
            match target_key(&root.target) {
                Ok(expected) if expected == root.target_key => {}
                Ok(_) => issues.push(CobIssue::new("root.target_key", "stale_target_key")),
                Err(mut target_issues) => issues.append(&mut target_issues),
            }
            if root.profile_hash != profile_identity(profile) {
                issues.push(CobIssue::new("root.profile_hash", "stale_profile_hash"));
            }
            if root.verification_role != VerificationRole::RecordedOnly {
                issues.push(CobIssue::new("root.verification_role", "unsupported_role_promotion"));
            }
            validate_non_claims(&root.non_claims, &profile.required_non_claims, &mut issues);
        }
        CobEventAction::Attach(attachment) => {
            issues.extend(validate_attachment_structure(profile, attachment));
        }
        CobEventAction::Supersede(action) => {
            issues.extend(validate_action_boundary(
                profile,
                &action.profile_hash,
                action.verification_role,
                &action.non_claims,
            ));
            validate_blake3(&action.attachment_id, "supersede.attachment_id", &mut issues);
            validate_blake3(&action.replacement_attachment_id, "supersede.replacement_attachment_id", &mut issues);
            if action.attachment_id == action.replacement_attachment_id {
                issues.push(CobIssue::new("supersede.replacement_attachment_id", "self_supersession"));
            }
        }
        CobEventAction::Revoke(action) => {
            issues.extend(validate_action_boundary(
                profile,
                &action.profile_hash,
                action.verification_role,
                &action.non_claims,
            ));
            validate_blake3(&action.attachment_id, "revoke.attachment_id", &mut issues);
            if !valid_text(&action.reason) {
                issues.push(CobIssue::new("revoke.reason", "invalid_revoke_reason"));
            }
            if contains_overclaim(&action.reason) {
                issues.push(CobIssue::new("revoke.reason", "unsupported_overclaim"));
            }
        }
    }
    sort_issues(&mut issues);
    issues
}

#[must_use]
pub fn reduce_logical_sidecar(input: &CobReductionInput) -> CobReductionReport {
    let mut issues = validate_profile(&input.profile);
    issues.extend(validate_authorization_profile(&input.profile, &input.authorization));
    issues.extend(validate_target(&input.target));
    let expected_target_key = target_key(&input.target).unwrap_or_default();
    validate_observed_target_state(&input.current_target_state, &expected_target_key, &mut issues);
    let expected_profile_hash = profile_identity(&input.profile);
    let sidecars = canonical_sidecars(&input.sidecars, &mut issues);
    let sidecar_ids = sidecars.iter().map(|sidecar| sidecar.object_id.as_str()).collect::<BTreeSet<_>>();
    let mut events = input.events.clone();
    events.sort_by(event_order);
    let parent_map = build_parent_map(&events);
    let resolutions = resolution_map(&input.resolutions, &mut issues);
    let valid_roots = validate_roots(
        &input.profile,
        &input.authorization,
        &input.target,
        &expected_target_key,
        &expected_profile_hash,
        &sidecar_ids,
        &events,
    );
    let mut rejected = valid_roots.rejected;
    let mut candidates = BTreeMap::<String, CandidateAttachment>::new();
    let mut dispositions = Vec::new();

    for observed in &events {
        let Some(event) = &observed.event else {
            if observed.decode_issue.is_some() && !is_root_slot(observed) {
                rejected.push(rejected_event(observed, vec!["event_decode_failed".to_owned()]));
            }
            continue;
        };
        if matches!(event.action, CobEventAction::Initialize(_)) {
            continue;
        }
        let mut event_issues = validate_observed_transport(observed, &sidecar_ids, &input.target.object_id);
        if !valid_roots.objects.contains(observed.sidecar_object_id.as_str()) {
            event_issues.push(CobIssue::new("sidecar_object_id", "invalid_sidecar_root"));
        }
        if event.schema != input.profile.event_schema {
            event_issues.push(CobIssue::new("event.schema", "unsupported_event_schema"));
        }
        if !actor_is_authorized(&input.profile, &input.authorization, &observed.signer, event.action.operation()) {
            event_issues.push(CobIssue::new("event.signer", "operation_unauthorized"));
        }
        match &event.action {
            CobEventAction::Attach(attachment) => {
                event_issues.extend(validate_attachment_structure(&input.profile, attachment));
                if attachment.target_key != expected_target_key {
                    event_issues.push(CobIssue::new("attachment.target_key", "foreign_target_key"));
                }
                match resolutions.get(attachment.attachment_id.as_str()) {
                    Some(resolution) => {
                        event_issues.extend(validate_resolution(&input.profile, attachment, resolution));
                    }
                    None => event_issues.push(CobIssue::new("attachment.payload", "artifact_resolution_missing")),
                }
                if event_issues.is_empty() {
                    let key = EventKey {
                        entry_oid: observed.entry_oid.clone(),
                        operation_index: observed.operation_index,
                    };
                    let candidate =
                        candidates.entry(attachment.attachment_id.clone()).or_insert_with(|| CandidateAttachment {
                            attachment: attachment.clone(),
                            source_keys: Vec::new(),
                            signers: BTreeSet::new(),
                        });
                    if candidate.attachment == *attachment {
                        candidate.source_keys.push(key);
                        candidate.signers.insert(observed.signer.clone());
                    } else {
                        event_issues.push(CobIssue::new("attachment.attachment_id", "conflicting_attachment_identity"));
                    }
                }
            }
            CobEventAction::Supersede(action) => {
                event_issues.extend(validate_action_boundary(
                    &input.profile,
                    &action.profile_hash,
                    action.verification_role,
                    &action.non_claims,
                ));
                validate_blake3(&action.attachment_id, "supersede.attachment_id", &mut event_issues);
                validate_blake3(
                    &action.replacement_attachment_id,
                    "supersede.replacement_attachment_id",
                    &mut event_issues,
                );
                if action.attachment_id == action.replacement_attachment_id {
                    event_issues.push(CobIssue::new("supersede.replacement_attachment_id", "self_supersession"));
                }
                if event_issues.is_empty() {
                    dispositions.push(Disposition {
                        key: event_key(observed),
                        target_attachment_id: action.attachment_id.clone(),
                        kind: DispositionKind::Supersede(action.replacement_attachment_id.clone()),
                    });
                }
            }
            CobEventAction::Revoke(action) => {
                event_issues.extend(validate_action_boundary(
                    &input.profile,
                    &action.profile_hash,
                    action.verification_role,
                    &action.non_claims,
                ));
                validate_blake3(&action.attachment_id, "revoke.attachment_id", &mut event_issues);
                if !valid_text(&action.reason) {
                    event_issues.push(CobIssue::new("revoke.reason", "invalid_revoke_reason"));
                }
                if contains_overclaim(&action.reason) {
                    event_issues.push(CobIssue::new("revoke.reason", "unsupported_overclaim"));
                }
                if event_issues.is_empty() {
                    dispositions.push(Disposition {
                        key: event_key(observed),
                        target_attachment_id: action.attachment_id.clone(),
                        kind: DispositionKind::Revoke(action.reason.clone()),
                    });
                }
            }
            CobEventAction::Initialize(_) => unreachable!("initialize handled before action match"),
        }
        if !event_issues.is_empty() {
            rejected.push(rejected_event(observed, event_issues.into_iter().map(|issue| issue.code).collect()));
        }
    }

    validate_disposition_predecessors(&candidates, &parent_map, &mut dispositions, &events, &mut rejected);
    let (statuses, conflicts) = reduce_dispositions(&candidates, &parent_map, &dispositions);
    let mut accepted_attachments = candidates
        .into_iter()
        .map(|(attachment_id, mut candidate)| {
            candidate.source_keys.sort();
            candidate.source_keys.dedup();
            let mut source_entries = candidate.source_keys.into_iter().map(|key| key.entry_oid).collect::<Vec<_>>();
            source_entries.sort();
            source_entries.dedup();
            AcceptedAttachment {
                attachment: candidate.attachment,
                source_entries,
                signers: candidate.signers.into_iter().collect(),
                status: statuses.get(&attachment_id).cloned().unwrap_or(AttachmentStatus::Active),
            }
        })
        .collect::<Vec<_>>();
    accepted_attachments.sort_by(|left, right| left.attachment.attachment_id.cmp(&right.attachment.attachment_id));
    rejected.sort_by(rejected_order);
    rejected.dedup();
    let mut conflicts = conflicts;
    conflicts.sort_by(|left, right| (&left.attachment_id, &left.code).cmp(&(&right.attachment_id, &right.code)));
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return CobReductionReport {
            valid: false,
            issues,
            snapshot: None,
        };
    }
    let mut snapshot = CobSnapshot {
        schema: input.profile.snapshot_schema.clone(),
        target: input.target.clone(),
        target_key: expected_target_key,
        current_target_state: input.current_target_state.clone(),
        sidecars,
        accepted_attachments,
        rejected_events: rejected,
        conflicts,
        profile_hash: expected_profile_hash,
        authorization_hash: input.authorization.authorization_hash.clone(),
        verification_role: VerificationRole::RecordedOnly,
        supported_claim: SUPPORTED_CLAIM.to_owned(),
        non_claims: receipt_non_claims(&input.profile),
        materialized_state_hash: String::new(),
        receipt_hash: String::new(),
    };
    snapshot.materialized_state_hash = snapshot_state_hash(&snapshot);
    snapshot.receipt_hash = snapshot_receipt_hash(&snapshot);
    CobReductionReport {
        valid: true,
        issues,
        snapshot: Some(snapshot),
    }
}

pub fn build_binding_receipt(
    profile: &CobEvidenceProfile,
    attachment: &CobEvidenceAttachment,
    entry_oid: &str,
    embed_oid: Option<String>,
) -> Result<BindingReceipt, Vec<CobIssue>> {
    let mut issues = validate_profile(profile);
    issues.extend(validate_attachment_structure(profile, attachment));
    validate_git_oid(entry_oid, "binding.entry_oid", &mut issues);
    if let Some(oid) = &embed_oid {
        validate_git_oid(oid, "binding.embed_oid", &mut issues);
    }
    match (&attachment.payload, &embed_oid) {
        (AttachmentPayload::Embedded { git_oid, .. }, Some(binding_oid)) if git_oid == binding_oid => {}
        (AttachmentPayload::Embedded { .. }, _) => {
            issues.push(CobIssue::new("binding.embed_oid", "embed_binding_mismatch"));
        }
        (AttachmentPayload::ReferenceOnly { .. }, Some(_)) => {
            issues.push(CobIssue::new("binding.embed_oid", "unexpected_embed_binding"));
        }
        (AttachmentPayload::ReferenceOnly { .. }, None) => {}
    }
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut receipt = BindingReceipt {
        schema: profile.binding_schema.clone(),
        target_key: attachment.target_key.clone(),
        attachment_id: attachment.attachment_id.clone(),
        artifact_stable_id: attachment.artifact.stable_id.clone(),
        artifact_canonical_hash: attachment.artifact.canonical_hash.clone(),
        entry_oid: entry_oid.to_owned(),
        embed_oid,
        profile_hash: profile_identity(profile),
        verification_role: VerificationRole::RecordedOnly,
        supported_claim: SUPPORTED_CLAIM.to_owned(),
        non_claims: receipt_non_claims(profile),
        receipt_hash: String::new(),
    };
    receipt.receipt_hash = binding_receipt_hash(&receipt);
    Ok(receipt)
}

#[must_use]
pub fn verify_binding_receipt(profile: &CobEvidenceProfile, receipt: &BindingReceipt) -> Vec<CobIssue> {
    let mut issues = validate_profile(profile);
    if receipt.schema != profile.binding_schema {
        issues.push(CobIssue::new("binding.schema", "unsupported_binding_schema"));
    }
    for (field, value) in [
        ("binding.target_key", receipt.target_key.as_str()),
        ("binding.attachment_id", receipt.attachment_id.as_str()),
        ("binding.artifact_canonical_hash", receipt.artifact_canonical_hash.as_str()),
        ("binding.profile_hash", receipt.profile_hash.as_str()),
        ("binding.receipt_hash", receipt.receipt_hash.as_str()),
    ] {
        validate_blake3(value, field, &mut issues);
    }
    if !valid_text(&receipt.artifact_stable_id) {
        issues.push(CobIssue::new("binding.artifact_stable_id", "invalid_artifact_stable_id"));
    }
    validate_git_oid(&receipt.entry_oid, "binding.entry_oid", &mut issues);
    if let Some(embed_oid) = &receipt.embed_oid {
        validate_git_oid(embed_oid, "binding.embed_oid", &mut issues);
    }
    if receipt.profile_hash != profile_identity(profile) {
        issues.push(CobIssue::new("binding.profile_hash", "stale_profile_hash"));
    }
    if receipt.verification_role != VerificationRole::RecordedOnly {
        issues.push(CobIssue::new("binding.verification_role", "unsupported_role_promotion"));
    }
    if receipt.supported_claim != SUPPORTED_CLAIM {
        issues.push(CobIssue::new("binding.supported_claim", "unsupported_claim"));
    }
    validate_non_claims(&receipt.non_claims, &receipt_non_claims(profile), &mut issues);
    if receipt.receipt_hash != binding_receipt_hash(receipt) {
        issues.push(CobIssue::new("binding.receipt_hash", "stale_binding_receipt_hash"));
    }
    sort_issues(&mut issues);
    issues
}

pub fn snapshot_evidence_artifact(
    profile: &CobEvidenceProfile,
    authorization: &CobAuthorizationProfile,
    snapshot: &CobSnapshot,
) -> Result<EvidenceArtifact, Vec<CobIssue>> {
    let issues = verify_snapshot(profile, authorization, snapshot);
    if !issues.is_empty() {
        return Err(issues);
    }
    let payload = serde_json::to_value(snapshot).expect("typed snapshot serialization is infallible");
    build_evidence_artifact(
        SNAPSHOT_ARTIFACT_KIND,
        &format!("radicle-cob-snapshot:{}", snapshot.target_key),
        None,
        VerificationRole::RecordedOnly,
        payload,
        snapshot.non_claims.clone(),
    )
    .map_err(|_| vec![CobIssue::new("snapshot", "snapshot_artifact_build_failed")])
}

#[must_use]
pub fn verify_snapshot(
    profile: &CobEvidenceProfile,
    authorization: &CobAuthorizationProfile,
    snapshot: &CobSnapshot,
) -> Vec<CobIssue> {
    let mut issues = validate_profile(profile);
    issues.extend(validate_authorization_profile(profile, authorization));
    if snapshot.schema != profile.snapshot_schema {
        issues.push(CobIssue::new("snapshot.schema", "unsupported_snapshot_schema"));
    }
    issues.extend(validate_target(&snapshot.target));
    let expected_target_key = target_key(&snapshot.target).unwrap_or_default();
    if snapshot.target_key != expected_target_key {
        issues.push(CobIssue::new("snapshot.target_key", "stale_target_key"));
    }
    validate_observed_target_state(&snapshot.current_target_state, &expected_target_key, &mut issues);
    validate_blake3(&snapshot.profile_hash, "snapshot.profile_hash", &mut issues);
    validate_blake3(&snapshot.authorization_hash, "snapshot.authorization_hash", &mut issues);
    validate_blake3(&snapshot.materialized_state_hash, "snapshot.materialized_state_hash", &mut issues);
    validate_blake3(&snapshot.receipt_hash, "snapshot.receipt_hash", &mut issues);
    if snapshot.profile_hash != profile_identity(profile) {
        issues.push(CobIssue::new("snapshot.profile_hash", "stale_profile_hash"));
    }
    if snapshot.authorization_hash != authorization.authorization_hash {
        issues.push(CobIssue::new("snapshot.authorization_hash", "stale_authorization_hash"));
    }
    if snapshot.verification_role != VerificationRole::RecordedOnly {
        issues.push(CobIssue::new("snapshot.verification_role", "unsupported_role_promotion"));
    }
    if snapshot.supported_claim != SUPPORTED_CLAIM {
        issues.push(CobIssue::new("snapshot.supported_claim", "unsupported_claim"));
    }
    validate_non_claims(&snapshot.non_claims, &receipt_non_claims(profile), &mut issues);
    validate_snapshot_collections(profile, snapshot, &mut issues);
    if snapshot.materialized_state_hash != snapshot_state_hash(snapshot) {
        issues.push(CobIssue::new("snapshot.materialized_state_hash", "stale_materialized_state_hash"));
    }
    if snapshot.receipt_hash != snapshot_receipt_hash(snapshot) {
        issues.push(CobIssue::new("snapshot.receipt_hash", "stale_snapshot_receipt_hash"));
    }
    sort_issues(&mut issues);
    issues
}

fn validate_snapshot_collections(profile: &CobEvidenceProfile, snapshot: &CobSnapshot, issues: &mut Vec<CobIssue>) {
    let canonical_sidecar_rows = canonical_sidecars(&snapshot.sidecars, issues);
    if canonical_sidecar_rows != snapshot.sidecars {
        issues.push(CobIssue::new("snapshot.sidecars", "sidecars_not_canonical"));
    }
    let mut attachment_ids = Vec::new();
    for accepted in &snapshot.accepted_attachments {
        issues.extend(validate_attachment_structure(profile, &accepted.attachment));
        if accepted.attachment.target_key != snapshot.target_key {
            issues.push(CobIssue::new("snapshot.accepted_attachments", "foreign_target_key"));
        }
        let mut source_entries = accepted.source_entries.clone();
        source_entries.sort();
        source_entries.dedup();
        if source_entries != accepted.source_entries || source_entries.is_empty() {
            issues.push(CobIssue::new("snapshot.accepted_attachments.source_entries", "source_entries_not_canonical"));
        }
        for entry in &accepted.source_entries {
            validate_git_oid(entry, "snapshot.accepted_attachments.source_entries", issues);
        }
        let mut signers = accepted.signers.clone();
        signers.sort();
        signers.dedup();
        if signers != accepted.signers || signers.is_empty() {
            issues.push(CobIssue::new("snapshot.accepted_attachments.signers", "signers_not_canonical"));
        }
        for signer in &accepted.signers {
            if !valid_signer(signer) {
                issues.push(CobIssue::new("snapshot.accepted_attachments.signers", "invalid_signer"));
            }
        }
        if let AttachmentStatus::Superseded {
            replacement_attachment_id,
        } = &accepted.status
        {
            validate_blake3(
                replacement_attachment_id,
                "snapshot.accepted_attachments.replacement_attachment_id",
                issues,
            );
        }
        attachment_ids.push(accepted.attachment.attachment_id.clone());
    }
    let mut canonical_attachment_ids = attachment_ids.clone();
    canonical_attachment_ids.sort();
    canonical_attachment_ids.dedup();
    if canonical_attachment_ids != attachment_ids {
        issues.push(CobIssue::new("snapshot.accepted_attachments", "attachments_not_canonical"));
    }
    let mut rejected = snapshot.rejected_events.clone();
    rejected.sort_by(rejected_order);
    rejected.dedup();
    if rejected != snapshot.rejected_events {
        issues.push(CobIssue::new("snapshot.rejected_events", "rejected_events_not_canonical"));
    }
    for event in &snapshot.rejected_events {
        validate_git_oid(&event.sidecar_object_id, "snapshot.rejected_events.sidecar_object_id", issues);
        validate_git_oid(&event.entry_oid, "snapshot.rejected_events.entry_oid", issues);
        if event.issue_codes.is_empty() {
            issues.push(CobIssue::new("snapshot.rejected_events.issue_codes", "issue_codes_empty"));
        }
    }
    let mut conflicts = snapshot.conflicts.clone();
    conflicts.sort_by(|left, right| (&left.attachment_id, &left.code).cmp(&(&right.attachment_id, &right.code)));
    conflicts.dedup();
    if conflicts != snapshot.conflicts {
        issues.push(CobIssue::new("snapshot.conflicts", "conflicts_not_canonical"));
    }
    for conflict in &snapshot.conflicts {
        validate_blake3(&conflict.attachment_id, "snapshot.conflicts.attachment_id", issues);
        if !valid_text(&conflict.code) || conflict.event_entries.is_empty() {
            issues.push(CobIssue::new("snapshot.conflicts", "invalid_conflict"));
        }
        let mut event_entries = conflict.event_entries.clone();
        event_entries.sort();
        event_entries.dedup();
        if event_entries != conflict.event_entries {
            issues.push(CobIssue::new("snapshot.conflicts.event_entries", "conflict_entries_not_canonical"));
        }
        for entry in &conflict.event_entries {
            validate_git_oid(entry, "snapshot.conflicts.event_entries", issues);
        }
    }
}

fn validate_target(target: &CobTargetRef) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    if !valid_text(&target.repository_rid)
        || !target.repository_rid.starts_with(RADICLE_RID_PREFIX)
        || target.repository_rid.len() <= RADICLE_RID_PREFIX.len()
    {
        issues.push(CobIssue::new("target.repository_rid", "invalid_repository_rid"));
    }
    if !valid_type_name(&target.type_name) {
        issues.push(CobIssue::new("target.type_name", "invalid_cob_type_name"));
    }
    validate_git_oid(&target.object_id, "target.object_id", &mut issues);
    issues
}

fn validate_observed_target_state(state: &ObservedTargetState, expected_target_key: &str, issues: &mut Vec<CobIssue>) {
    if state.tips.is_empty() {
        issues.push(CobIssue::new("observed_target_state.tips", "target_tips_empty"));
    }
    let mut canonical = state.tips.clone();
    canonical.sort();
    canonical.dedup();
    if canonical != state.tips {
        issues.push(CobIssue::new("observed_target_state.tips", "target_tips_not_canonical"));
    }
    for tip in &state.tips {
        validate_git_oid(tip, "observed_target_state.tips", issues);
    }
    validate_blake3(&state.state_hash, "observed_target_state.state_hash", issues);
    let material = serde_json::json!({"target_key": expected_target_key, "tips": &state.tips});
    if state.state_hash != typed_blake3(&material) {
        issues.push(CobIssue::new("observed_target_state.state_hash", "stale_target_state_hash"));
    }
}

fn validate_payload(profile: &CobEvidenceProfile, attachment: &CobEvidenceAttachment, issues: &mut Vec<CobIssue>) {
    if attachment.payload.size_bytes() == 0 {
        issues.push(CobIssue::new("attachment.payload.size_bytes", "payload_empty"));
    }
    validate_blake3(attachment.payload.raw_blake3(), "attachment.payload.raw_blake3", issues);
    if !valid_text(attachment.payload.media_type()) {
        issues.push(CobIssue::new("attachment.payload.media_type", "invalid_media_type"));
    }
    match &attachment.payload {
        AttachmentPayload::ReferenceOnly { locator, .. } => {
            if !safe_relative_locator(locator) {
                issues.push(CobIssue::new("attachment.payload.locator", "unsafe_artifact_locator"));
            }
            if !profile.embed_policy.allowed_reference_confidentiality.contains(&attachment.confidentiality) {
                issues.push(CobIssue::new("attachment.confidentiality", "confidentiality_not_allowed"));
            }
        }
        AttachmentPayload::Embedded {
            name,
            git_oid,
            media_type,
            size_bytes,
            ..
        } => {
            if !safe_embed_name(name) {
                issues.push(CobIssue::new("attachment.payload.name", "unsafe_embed_name"));
            }
            validate_git_oid(git_oid, "attachment.payload.git_oid", issues);
            if *size_bytes > profile.embed_policy.max_bytes {
                issues.push(CobIssue::new("attachment.payload.size_bytes", "embed_limit_exceeded"));
            }
            if !profile.embed_policy.media_types.contains(media_type) {
                issues.push(CobIssue::new("attachment.payload.media_type", "embed_media_type_unsupported"));
            }
            if attachment.confidentiality != ConfidentialityClass::Public {
                issues.push(CobIssue::new("attachment.confidentiality", "embedded_confidentiality_not_public"));
            }
        }
    }
}

fn validate_resolution(
    profile: &CobEvidenceProfile,
    attachment: &CobEvidenceAttachment,
    resolution: &ResolvedArtifact,
) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    if resolution.attachment_id != attachment.attachment_id {
        issues.push(CobIssue::new("resolution.attachment_id", "resolution_attachment_mismatch"));
    }
    let artifact_json =
        serde_json::to_string(&resolution.artifact).expect("typed artifact serialization is infallible");
    let report = check_evidence_artifact(&artifact_json);
    if !report.valid {
        issues.push(CobIssue::new("resolution.artifact", "evidence_artifact_invalid"));
    }
    let expected_ref = EvidenceReference::from_artifact(&resolution.artifact);
    if expected_ref != attachment.artifact {
        issues.push(CobIssue::new("resolution.artifact", "evidence_reference_mismatch"));
    }
    if resolution.size_bytes != attachment.payload.size_bytes() {
        issues.push(CobIssue::new("resolution.size_bytes", "payload_size_mismatch"));
    }
    if resolution.raw_blake3 != attachment.payload.raw_blake3() {
        issues.push(CobIssue::new("resolution.raw_blake3", "payload_hash_mismatch"));
    }
    match (&attachment.payload, &resolution.git_oid, &resolution.locator) {
        (AttachmentPayload::Embedded { git_oid, .. }, Some(measured_oid), None) if git_oid == measured_oid => {}
        (AttachmentPayload::Embedded { .. }, _, _) => {
            issues.push(CobIssue::new("resolution.git_oid", "embed_resolution_mismatch"));
        }
        (AttachmentPayload::ReferenceOnly { locator, .. }, None, Some(measured_locator))
            if locator == measured_locator => {}
        (AttachmentPayload::ReferenceOnly { .. }, _, _) => {
            issues.push(CobIssue::new("resolution.locator", "reference_resolution_mismatch"));
        }
    }
    issues.extend(validate_attachment_structure(profile, attachment));
    sort_issues(&mut issues);
    issues
}

fn validate_action_boundary(
    profile: &CobEvidenceProfile,
    profile_hash: &str,
    role: VerificationRole,
    non_claims: &[String],
) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    if profile_hash != profile_identity(profile) {
        issues.push(CobIssue::new("action.profile_hash", "stale_profile_hash"));
    }
    if role != VerificationRole::RecordedOnly {
        issues.push(CobIssue::new("action.verification_role", "unsupported_role_promotion"));
    }
    validate_non_claims(non_claims, &profile.required_non_claims, &mut issues);
    issues
}

fn validate_non_claims(actual: &[String], required: &[String], issues: &mut Vec<CobIssue>) {
    if actual.is_empty() {
        issues.push(CobIssue::new("non_claims", "non_claims_empty"));
    }
    let mut seen = BTreeSet::new();
    for claim in actual {
        if !valid_text(claim) {
            issues.push(CobIssue::new("non_claims", "invalid_non_claim"));
        }
        if !seen.insert(claim.as_str()) {
            issues.push(CobIssue::new("non_claims", "duplicate_non_claim"));
        }
        if contains_overclaim(claim) {
            issues.push(CobIssue::new("non_claims", "unsupported_overclaim"));
        }
    }
    for claim in required {
        if !actual.iter().any(|candidate| candidate == claim) {
            issues.push(CobIssue::new("non_claims", "required_non_claim_missing"));
        }
    }
}

fn validate_roots<'a>(
    profile: &CobEvidenceProfile,
    authorization: &CobAuthorizationProfile,
    target: &CobTargetRef,
    target_key: &str,
    profile_hash: &str,
    sidecar_ids: &BTreeSet<&'a str>,
    events: &[ObservedCobEvent],
) -> RootValidation<'a> {
    let mut objects = BTreeSet::new();
    let mut rejected = Vec::new();
    for sidecar_id in sidecar_ids {
        let roots = events
            .iter()
            .filter(|event| event.sidecar_object_id == *sidecar_id && event.entry_oid == *sidecar_id)
            .collect::<Vec<_>>();
        let mut accepted = false;
        for observed in roots {
            let mut root_issues = validate_observed_transport(observed, sidecar_ids, &target.object_id);
            match &observed.event {
                Some(CobEvent {
                    schema,
                    action: CobEventAction::Initialize(root),
                }) => {
                    if schema != &profile.event_schema {
                        root_issues.push(CobIssue::new("event.schema", "unsupported_event_schema"));
                    }
                    if root.schema != profile.root_schema {
                        root_issues.push(CobIssue::new("root.schema", "unsupported_root_schema"));
                    }
                    if root.target != *target {
                        root_issues.push(CobIssue::new("root.target", "foreign_sidecar_target"));
                    }
                    if root.target_key != target_key {
                        root_issues.push(CobIssue::new("root.target_key", "stale_target_key"));
                    }
                    if root.profile_hash != profile_hash {
                        root_issues.push(CobIssue::new("root.profile_hash", "stale_profile_hash"));
                    }
                    if root.verification_role != VerificationRole::RecordedOnly {
                        root_issues.push(CobIssue::new("root.verification_role", "unsupported_role_promotion"));
                    }
                    validate_non_claims(&root.non_claims, &profile.required_non_claims, &mut root_issues);
                    if !actor_is_authorized(profile, authorization, &observed.signer, CobOperation::Initialize) {
                        root_issues.push(CobIssue::new("root.signer", "operation_unauthorized"));
                    }
                }
                Some(_) => root_issues.push(CobIssue::new("root.action", "initialize_event_missing")),
                None => root_issues.push(CobIssue::new("root.action", "event_decode_failed")),
            }
            if root_issues.is_empty() {
                accepted = true;
            } else {
                rejected.push(rejected_event(observed, root_issues.into_iter().map(|issue| issue.code).collect()));
            }
        }
        if accepted {
            objects.insert(*sidecar_id);
        } else if !events.iter().any(|event| event.sidecar_object_id == *sidecar_id && event.entry_oid == *sidecar_id) {
            rejected.push(RejectedEvent {
                sidecar_object_id: (*sidecar_id).to_owned(),
                entry_oid: (*sidecar_id).to_owned(),
                operation_index: 0,
                signer: String::new(),
                issue_codes: vec!["initialize_event_missing".to_owned()],
            });
        }
    }
    RootValidation { objects, rejected }
}

struct RootValidation<'a> {
    objects: BTreeSet<&'a str>,
    rejected: Vec<RejectedEvent>,
}

fn validate_observed_transport(
    observed: &ObservedCobEvent,
    sidecar_ids: &BTreeSet<&str>,
    target_object_id: &str,
) -> Vec<CobIssue> {
    let mut issues = Vec::new();
    validate_git_oid(&observed.sidecar_object_id, "event.sidecar_object_id", &mut issues);
    validate_git_oid(&observed.entry_oid, "event.entry_oid", &mut issues);
    for parent in &observed.parent_oids {
        validate_git_oid(parent, "event.parent_oids", &mut issues);
    }
    for related in &observed.related_oids {
        validate_git_oid(related, "event.related_oids", &mut issues);
    }
    if !sidecar_ids.contains(observed.sidecar_object_id.as_str()) {
        issues.push(CobIssue::new("event.sidecar_object_id", "unknown_sidecar_object"));
    }
    if !observed.related_oids.iter().any(|oid| oid == target_object_id) {
        issues.push(CobIssue::new("event.related_oids", "target_relation_missing"));
    }
    if !valid_signer(&observed.signer) {
        issues.push(CobIssue::new("event.signer", "invalid_signer"));
    }
    if !observed.signature_valid {
        issues.push(CobIssue::new("event.signature", "signature_invalid"));
    }
    if observed.event.is_some() == observed.decode_issue.is_some() {
        issues.push(CobIssue::new("event.decode", "event_decode_state_invalid"));
    }
    issues
}

#[must_use]
pub fn actor_is_authorized(
    profile: &CobEvidenceProfile,
    authorization: &CobAuthorizationProfile,
    signer: &str,
    operation: CobOperation,
) -> bool {
    let matching = authorization.bindings.iter().filter(|binding| binding.signer == signer).collect::<Vec<_>>();
    if matching.len() != 1 {
        return false;
    }
    profile
        .actor_roles
        .iter()
        .find(|role| role.role == matching[0].role)
        .is_some_and(|role| role.operations.contains(&operation))
}

fn canonical_sidecars(sidecars: &[SidecarObservation], issues: &mut Vec<CobIssue>) -> Vec<SidecarObservation> {
    let mut canonical = sidecars.to_vec();
    for sidecar in &mut canonical {
        validate_git_oid(&sidecar.object_id, "sidecars.object_id", issues);
        sidecar.tips.sort();
        sidecar.tips.dedup();
        if sidecar.tips.is_empty() {
            issues.push(CobIssue::new("sidecars.tips", "sidecar_tips_empty"));
        }
        for tip in &sidecar.tips {
            validate_git_oid(tip, "sidecars.tips", issues);
        }
    }
    canonical.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let mut seen = BTreeSet::new();
    for sidecar in &canonical {
        if !seen.insert(sidecar.object_id.as_str()) {
            issues.push(CobIssue::new("sidecars.object_id", "duplicate_sidecar_observation"));
        }
    }
    canonical
}

fn resolution_map<'a>(
    resolutions: &'a [ResolvedArtifact],
    issues: &mut Vec<CobIssue>,
) -> BTreeMap<&'a str, &'a ResolvedArtifact> {
    let mut map = BTreeMap::new();
    for resolution in resolutions {
        if map.insert(resolution.attachment_id.as_str(), resolution).is_some() {
            issues.push(CobIssue::new("resolutions.attachment_id", "duplicate_artifact_resolution"));
        }
    }
    map
}

fn build_parent_map(events: &[ObservedCobEvent]) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    for event in events {
        map.entry(event.entry_oid.clone()).or_insert_with(|| {
            let mut parents = event.parent_oids.clone();
            parents.sort();
            parents.dedup();
            parents
        });
    }
    map
}

fn validate_disposition_predecessors(
    candidates: &BTreeMap<String, CandidateAttachment>,
    parents: &BTreeMap<String, Vec<String>>,
    dispositions: &mut Vec<Disposition>,
    events: &[ObservedCobEvent],
    rejected: &mut Vec<RejectedEvent>,
) {
    dispositions.retain(|disposition| {
        let source = candidates.get(&disposition.target_attachment_id);
        let replacement = match &disposition.kind {
            DispositionKind::Supersede(id) => candidates.get(id),
            DispositionKind::Revoke(_) => source,
        };
        let causal_source = source.is_some_and(|candidate| {
            candidate.source_keys.iter().any(|source_key| precedes(source_key, &disposition.key, parents))
        });
        let causal_replacement = replacement.is_some_and(|candidate| {
            candidate.source_keys.iter().any(|source_key| precedes(source_key, &disposition.key, parents))
        });
        let valid = causal_source && causal_replacement;
        if !valid && let Some(observed) = events.iter().find(|event| event_key(event) == disposition.key) {
            rejected.push(rejected_event(observed, vec!["unknown_or_noncausal_predecessor".to_owned()]));
        }
        valid
    });
}

fn reduce_dispositions(
    candidates: &BTreeMap<String, CandidateAttachment>,
    parents: &BTreeMap<String, Vec<String>>,
    dispositions: &[Disposition],
) -> (BTreeMap<String, AttachmentStatus>, Vec<CobConflict>) {
    let mut statuses = candidates.keys().map(|id| (id.clone(), AttachmentStatus::Active)).collect::<BTreeMap<_, _>>();
    let mut conflicts = Vec::new();
    for attachment_id in candidates.keys() {
        let relevant = dispositions
            .iter()
            .filter(|disposition| &disposition.target_attachment_id == attachment_id)
            .collect::<Vec<_>>();
        let maximal = relevant
            .iter()
            .copied()
            .filter(|candidate| {
                !relevant
                    .iter()
                    .any(|other| candidate.key != other.key && precedes(&candidate.key, &other.key, parents))
            })
            .collect::<Vec<_>>();
        if maximal.is_empty() {
            continue;
        }
        let effects = maximal.iter().map(|disposition| disposition.kind.clone()).collect::<BTreeSet<_>>();
        if effects.len() == 1 {
            let effect = effects.into_iter().next().expect("one disposition effect exists");
            let status = match effect {
                DispositionKind::Supersede(replacement_attachment_id) => AttachmentStatus::Superseded {
                    replacement_attachment_id,
                },
                DispositionKind::Revoke(reason) => AttachmentStatus::Revoked { reason },
            };
            statuses.insert(attachment_id.clone(), status);
        } else {
            let mut event_entries =
                maximal.iter().map(|disposition| disposition.key.entry_oid.clone()).collect::<Vec<_>>();
            event_entries.sort();
            event_entries.dedup();
            conflicts.push(CobConflict {
                attachment_id: attachment_id.clone(),
                event_entries,
                code: "concurrent_disposition_conflict".to_owned(),
            });
        }
    }
    (statuses, conflicts)
}

fn precedes(left: &EventKey, right: &EventKey, parents: &BTreeMap<String, Vec<String>>) -> bool {
    if left.entry_oid == right.entry_oid {
        return left.operation_index < right.operation_index;
    }
    let mut pending = parents.get(&right.entry_oid).cloned().unwrap_or_default();
    let mut visited = BTreeSet::new();
    while let Some(candidate) = pending.pop() {
        if candidate == left.entry_oid {
            return true;
        }
        if visited.insert(candidate.clone()) {
            pending.extend(parents.get(&candidate).cloned().unwrap_or_default());
        }
    }
    false
}

fn snapshot_state_hash(snapshot: &CobSnapshot) -> String {
    let material = SnapshotStateMaterial {
        target: &snapshot.target,
        target_key: &snapshot.target_key,
        current_target_state: &snapshot.current_target_state,
        sidecars: &snapshot.sidecars,
        accepted_attachments: &snapshot.accepted_attachments,
        rejected_events: &snapshot.rejected_events,
        conflicts: &snapshot.conflicts,
        profile_hash: &snapshot.profile_hash,
        authorization_hash: &snapshot.authorization_hash,
    };
    typed_blake3(&material)
}

fn snapshot_receipt_hash(snapshot: &CobSnapshot) -> String {
    let state = SnapshotStateMaterial {
        target: &snapshot.target,
        target_key: &snapshot.target_key,
        current_target_state: &snapshot.current_target_state,
        sidecars: &snapshot.sidecars,
        accepted_attachments: &snapshot.accepted_attachments,
        rejected_events: &snapshot.rejected_events,
        conflicts: &snapshot.conflicts,
        profile_hash: &snapshot.profile_hash,
        authorization_hash: &snapshot.authorization_hash,
    };
    let material = SnapshotReceiptMaterial {
        schema: &snapshot.schema,
        state,
        materialized_state_hash: &snapshot.materialized_state_hash,
        verification_role: snapshot.verification_role.as_str(),
        supported_claim: &snapshot.supported_claim,
        non_claims: &snapshot.non_claims,
    };
    typed_blake3(&material)
}

fn binding_receipt_hash(receipt: &BindingReceipt) -> String {
    let material = BindingIdentityMaterial {
        schema: &receipt.schema,
        target_key: &receipt.target_key,
        attachment_id: &receipt.attachment_id,
        artifact_stable_id: &receipt.artifact_stable_id,
        artifact_canonical_hash: &receipt.artifact_canonical_hash,
        entry_oid: &receipt.entry_oid,
        embed_oid: &receipt.embed_oid,
        profile_hash: &receipt.profile_hash,
        verification_role: receipt.verification_role.as_str(),
        supported_claim: &receipt.supported_claim,
        non_claims: &receipt.non_claims,
    };
    typed_blake3(&material)
}

fn receipt_non_claims(profile: &CobEvidenceProfile) -> Vec<String> {
    let mut claims = profile.required_non_claims.clone();
    claims.push(REQUIRED_NON_CLAIM.to_owned());
    claims.sort();
    claims.dedup();
    claims
}

fn event_order(left: &ObservedCobEvent, right: &ObservedCobEvent) -> std::cmp::Ordering {
    (&left.entry_oid, left.operation_index, &left.sidecar_object_id).cmp(&(
        &right.entry_oid,
        right.operation_index,
        &right.sidecar_object_id,
    ))
}

fn rejected_order(left: &RejectedEvent, right: &RejectedEvent) -> std::cmp::Ordering {
    (&left.entry_oid, left.operation_index, &left.sidecar_object_id, &left.issue_codes).cmp(&(
        &right.entry_oid,
        right.operation_index,
        &right.sidecar_object_id,
        &right.issue_codes,
    ))
}

fn rejected_event(observed: &ObservedCobEvent, mut issue_codes: Vec<String>) -> RejectedEvent {
    issue_codes.sort();
    issue_codes.dedup();
    RejectedEvent {
        sidecar_object_id: observed.sidecar_object_id.clone(),
        entry_oid: observed.entry_oid.clone(),
        operation_index: observed.operation_index,
        signer: observed.signer.clone(),
        issue_codes,
    }
}

fn event_key(event: &ObservedCobEvent) -> EventKey {
    EventKey {
        entry_oid: event.entry_oid.clone(),
        operation_index: event.operation_index,
    }
}

fn is_root_slot(event: &ObservedCobEvent) -> bool {
    event.entry_oid == event.sidecar_object_id && event.operation_index == 0
}

fn typed_blake3<T: Serialize>(material: &T) -> String {
    let value = serde_json::to_value(material).expect("typed identity serialization is infallible");
    let bytes = canonical_json_bytes(&value).expect("typed canonical JSON serialization is infallible");
    format!("{BLAKE3_PREFIX}{}", blake3_hex(&bytes))
}

fn validate_blake3(value: &str, field: &str, issues: &mut Vec<CobIssue>) {
    let Some(hex) = value.strip_prefix(BLAKE3_PREFIX) else {
        issues.push(CobIssue::new(field, "blake3_prefix_missing"));
        return;
    };
    if !is_lower_hex(hex, BLAKE3_HEX_LENGTH) {
        issues.push(CobIssue::new(field, "invalid_blake3_digest"));
    }
}

fn validate_git_oid(value: &str, field: &str, issues: &mut Vec<CobIssue>) {
    if value.starts_with(BLAKE3_PREFIX) {
        issues.push(CobIssue::new(field, "digest_domain_mismatch"));
    } else if !is_lower_hex(value, GIT_OID_HEX_LENGTH) {
        issues.push(CobIssue::new(field, "invalid_git_oid"));
    }
}

fn is_lower_hex(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_signer(value: &str) -> bool {
    valid_text(value) && value.starts_with('z')
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn valid_type_name(value: &str) -> bool {
    if !valid_text(value) {
        return false;
    }
    value.split('.').all(|component| {
        !component.is_empty()
            && component.chars().all(|character| character.is_ascii_alphanumeric() || character == '-')
            && component.chars().next().is_some_and(|character| character.is_ascii_alphanumeric())
            && component.chars().last().is_some_and(|character| character.is_ascii_alphanumeric())
    })
}

fn safe_relative_locator(value: &str) -> bool {
    valid_text(value)
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|component| !component.is_empty() && component != "." && component != "..")
}

fn safe_embed_name(value: &str) -> bool {
    valid_text(value) && !value.contains('/') && !value.contains('\\') && value != "." && value != ".."
}

fn contains_overclaim(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "proves target correctness",
        "authorizes a waiver",
        "authorizes disposition",
        "satisfies a requirement",
        "establishes lifecycle readiness",
        "establishes release readiness",
        "release eligible",
        "certified correct",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn require_unique_nonempty<T: Ord>(values: &[T], field: &str, issues: &mut Vec<CobIssue>) {
    if values.is_empty() {
        issues.push(CobIssue::new(field, "collection_empty"));
        return;
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        issues.push(CobIssue::new(field, "duplicate_value"));
    }
}

fn sort_issues(issues: &mut Vec<CobIssue>) {
    issues.sort();
    issues.dedup();
}

#[cfg(test)]
mod tests {
    // r[verify valence.radicle_cob_evidence.sidecar_profile]
    // r[verify valence.radicle_cob_evidence.pure_core]
    // r[verify valence.radicle_cob_evidence.attachment_identity]
    // r[verify valence.radicle_cob_evidence.digest_domains]
    // r[verify valence.radicle_cob_evidence.payload_modes]
    // r[verify valence.radicle_cob_evidence.binding_receipt]
    // r[verify valence.radicle_cob_evidence.role_policy]
    // r[verify valence.radicle_cob_evidence.authority]
    // r[verify valence.radicle_cob_evidence.convergence]
    // r[verify valence.radicle_cob_evidence.snapshot_handoff]
    // r[verify valence.radicle_cob_evidence.privacy_retention]
    // r[verify valence.radicle_cob_evidence.final_validation]
    // r[verify valence.radicle_cob_evidence.attachment_identity.change]
    // r[verify valence.radicle_cob_evidence.attachment_identity.stable]
    // r[verify valence.radicle_cob_evidence.digest_domains.valid]
    // r[verify valence.radicle_cob_evidence.final_validation.negative]
    // r[verify valence.radicle_cob_evidence.final_validation.positive]
    // r[verify valence.radicle_cob_evidence.pure_core.boundary]
    // r[verify valence.radicle_cob_evidence.pure_core.valid]
    // r[verify valence.radicle_cob_evidence.role_policy.valid]
    use super::*;
    use crate::evidence_ir::build_evidence_artifact;

    const ACTOR: &str = "z6MkrLMMsiPWUcNPHcRajuMi9mDfYckSoJyPwwnknocNYPm7";
    const OTHER_ACTOR: &str = "z6MkvUJtYD9dHDJfpevWRT98mzDDpdAtmUjwyDSkyqksUr7C";

    fn oid(digit: char) -> String {
        digit.to_string().repeat(GIT_OID_HEX_LENGTH)
    }

    fn profile() -> CobEvidenceProfile {
        CobEvidenceProfile {
            schema: PROFILE_SCHEMA.to_owned(),
            cob_type: COB_EVIDENCE_TYPE.to_owned(),
            root_schema: ROOT_SCHEMA.to_owned(),
            event_schema: EVENT_SCHEMA.to_owned(),
            binding_schema: BINDING_SCHEMA.to_owned(),
            snapshot_schema: SNAPSHOT_SCHEMA.to_owned(),
            authorization_schema: AUTHORIZATION_SCHEMA.to_owned(),
            relationships: vec![
                AttachmentRelationship::ContextFor,
                AttachmentRelationship::DispositionFor,
                AttachmentRelationship::EvidenceAgainst,
                AttachmentRelationship::EvidenceFor,
            ],
            actor_roles: vec![
                ActorRoleProfile {
                    role: "attacher".to_owned(),
                    operations: vec![CobOperation::Attach, CobOperation::Initialize],
                },
                ActorRoleProfile {
                    role: "maintainer".to_owned(),
                    operations: vec![
                        CobOperation::Attach,
                        CobOperation::Initialize,
                        CobOperation::Revoke,
                        CobOperation::Supersede,
                    ],
                },
            ],
            embed_policy: EmbedPolicy {
                max_bytes: MAX_EMBEDDED_ARTIFACT_BYTES,
                media_types: vec![EMBEDDED_ARTIFACT_MEDIA_TYPE.to_owned()],
                allowed_reference_confidentiality: vec![ConfidentialityClass::Public, ConfidentialityClass::Restricted],
            },
            required_non_claims: vec![
                EVIDENCE_NON_CLAIM.to_owned(),
                RETENTION_NON_CLAIM.to_owned(),
                CONFIDENTIALITY_NON_CLAIM.to_owned(),
            ],
        }
    }

    fn target() -> CobTargetRef {
        CobTargetRef {
            repository_rid: "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5".to_owned(),
            type_name: "xyz.radicle.issue".to_owned(),
            object_id: oid('1'),
        }
    }

    fn state() -> ObservedTargetState {
        build_observed_target_state(&target(), &[oid('2')]).expect("valid target state")
    }

    fn artifact(stable_id: &str) -> EvidenceArtifact {
        build_evidence_artifact(
            "lint-receipt-v1",
            stable_id,
            None,
            VerificationRole::Property,
            serde_json::json!({"status": "failed", "finding": "durable"}),
            vec![REQUIRED_NON_CLAIM.to_owned()],
        )
        .expect("valid artifact")
    }

    fn attachment(stable_id: &str) -> (CobEvidenceAttachment, ResolvedArtifact) {
        let artifact = artifact(stable_id);
        let bytes = serde_json::to_vec(&artifact).expect("serialize artifact");
        let raw_blake3 = format!("{BLAKE3_PREFIX}{}", blake3_hex(&bytes));
        let payload = AttachmentPayload::ReferenceOnly {
            locator: format!("artifacts/{stable_id}.json"),
            media_type: "application/json".to_owned(),
            size_bytes: u64::try_from(bytes.len()).expect("fixture size fits u64"),
            raw_blake3: raw_blake3.clone(),
        };
        let attachment = build_attachment(
            &profile(),
            &target(),
            AttachmentRelationship::EvidenceFor,
            EvidenceReference::from_artifact(&artifact),
            state(),
            payload,
            ConfidentialityClass::Public,
            profile().required_non_claims,
        )
        .expect("valid attachment");
        let resolution = ResolvedArtifact {
            attachment_id: attachment.attachment_id.clone(),
            artifact,
            size_bytes: u64::try_from(bytes.len()).expect("fixture size fits u64"),
            raw_blake3,
            git_oid: None,
            locator: Some(format!("artifacts/{stable_id}.json")),
        };
        (attachment, resolution)
    }

    fn authorization() -> CobAuthorizationProfile {
        build_authorization_profile(&profile_identity(&profile()), vec![
            ActorBinding {
                signer: ACTOR.to_owned(),
                role: "maintainer".to_owned(),
            },
            ActorBinding {
                signer: OTHER_ACTOR.to_owned(),
                role: "attacher".to_owned(),
            },
        ])
    }

    fn observed(
        sidecar: &str,
        entry: &str,
        parents: Vec<String>,
        signer: &str,
        index: u32,
        action: CobEventAction,
    ) -> ObservedCobEvent {
        ObservedCobEvent {
            sidecar_object_id: sidecar.to_owned(),
            entry_oid: entry.to_owned(),
            operation_index: index,
            parent_oids: parents,
            related_oids: vec![target().object_id],
            signer: signer.to_owned(),
            signature_valid: true,
            event: Some(CobEvent {
                schema: EVENT_SCHEMA.to_owned(),
                action,
            }),
            decode_issue: None,
        }
    }

    fn root(sidecar: &str, signer: &str) -> ObservedCobEvent {
        observed(
            sidecar,
            sidecar,
            Vec::new(),
            signer,
            0,
            CobEventAction::Initialize(SidecarRoot {
                schema: ROOT_SCHEMA.to_owned(),
                target: target(),
                target_key: target_key(&target()).expect("valid target"),
                profile_hash: profile_identity(&profile()),
                verification_role: VerificationRole::RecordedOnly,
                non_claims: profile().required_non_claims,
            }),
        )
    }

    fn reduction_input(events: Vec<ObservedCobEvent>, resolutions: Vec<ResolvedArtifact>) -> CobReductionInput {
        let sidecar_ids = events.iter().map(|event| event.sidecar_object_id.clone()).collect::<BTreeSet<_>>();
        CobReductionInput {
            profile: profile(),
            authorization: authorization(),
            target: target(),
            current_target_state: state(),
            sidecars: sidecar_ids
                .into_iter()
                .map(|object_id| SidecarObservation {
                    tips: vec![object_id.clone()],
                    object_id,
                })
                .collect(),
            events,
            resolutions,
        }
    }

    #[test]
    fn profile_attachment_and_binding_round_trip() {
        // r[verify valence.radicle_cob_evidence.sidecar_profile.valid]
        // r[verify valence.radicle_cob_evidence.binding_receipt.valid]
        let profile = profile();
        assert!(validate_profile(&profile).is_empty());
        let (attachment, _) = attachment("lint-1");
        assert_eq!(attachment.verification_role, VerificationRole::RecordedOnly);
        assert_eq!(attachment.attachment_id, attachment_identity(&attachment));
        let receipt = build_binding_receipt(&profile, &attachment, &oid('9'), None).expect("valid receipt");
        assert!(verify_binding_receipt(&profile, &receipt).is_empty());
    }

    #[test]
    fn malformed_domains_role_payload_and_binding_fail_closed() {
        // r[verify valence.radicle_cob_evidence.digest_domains.invalid]
        // r[verify valence.radicle_cob_evidence.payload_modes.invalid]
        // r[verify valence.radicle_cob_evidence.role_policy.invalid]
        // r[verify valence.radicle_cob_evidence.binding_receipt.invalid]
        let (mut candidate, _) = attachment("lint-2");
        candidate.target_key = oid('a');
        candidate.verification_role = VerificationRole::Property;
        candidate.payload = AttachmentPayload::ReferenceOnly {
            locator: "../escape.json".to_owned(),
            media_type: "application/json".to_owned(),
            size_bytes: 1,
            raw_blake3: format!("{BLAKE3_PREFIX}{}", "a".repeat(BLAKE3_HEX_LENGTH)),
        };
        candidate.attachment_id = attachment_identity(&candidate);
        let issues = validate_attachment_structure(&profile(), &candidate);
        assert!(issues.iter().any(|issue| issue.code == "blake3_prefix_missing"));
        assert!(issues.iter().any(|issue| issue.code == "stale_target_state_hash"));
        assert!(issues.iter().any(|issue| issue.code == "unsupported_role_promotion"));
        assert!(issues.iter().any(|issue| issue.code == "unsafe_artifact_locator"));
        assert!(build_binding_receipt(&profile(), &candidate, &format!("{BLAKE3_PREFIX}bad"), None).is_err());
    }

    #[test]
    fn multiple_physical_roots_converge_independent_of_input_order() {
        // r[verify valence.radicle_cob_evidence.convergence.valid]
        let sidecar_a = oid('3');
        let sidecar_b = oid('4');
        let entry_a = oid('5');
        let entry_b = oid('6');
        let (attachment_a, resolution_a) = attachment("lint-a");
        let (attachment_b, resolution_b) = attachment("lint-b");
        let events = vec![
            root(&sidecar_a, ACTOR),
            observed(&sidecar_a, &entry_a, vec![sidecar_a.clone()], ACTOR, 0, CobEventAction::Attach(attachment_a)),
            root(&sidecar_b, OTHER_ACTOR),
            observed(
                &sidecar_b,
                &entry_b,
                vec![sidecar_b.clone()],
                OTHER_ACTOR,
                0,
                CobEventAction::Attach(attachment_b),
            ),
        ];
        let first =
            reduce_logical_sidecar(&reduction_input(events.clone(), vec![resolution_a.clone(), resolution_b.clone()]));
        let second = reduce_logical_sidecar(&reduction_input(events.into_iter().rev().collect(), vec![
            resolution_b,
            resolution_a,
        ]));
        assert!(first.valid, "{:?}", first.issues);
        assert_eq!(first.snapshot, second.snapshot);
        assert_eq!(first.snapshot.expect("snapshot").accepted_attachments.len(), 2);
    }

    #[test]
    fn unauthorized_actor_and_tampered_resolution_are_rejected_not_promoted() {
        // r[verify valence.radicle_cob_evidence.authority.invalid]
        // r[verify valence.radicle_cob_evidence.payload_modes.invalid]
        let sidecar = oid('3');
        let entry = oid('5');
        let (attachment, mut resolution) = attachment("lint-auth");
        resolution.raw_blake3 = format!("{BLAKE3_PREFIX}{}", "f".repeat(BLAKE3_HEX_LENGTH));
        let unknown_actor = "z6MkUnknownActor";
        let events = vec![
            root(&sidecar, ACTOR),
            observed(&sidecar, &entry, vec![sidecar.clone()], unknown_actor, 0, CobEventAction::Attach(attachment)),
        ];
        let report = reduce_logical_sidecar(&reduction_input(events, vec![resolution]));
        let snapshot = report.snapshot.expect("structurally valid snapshot");
        assert!(snapshot.accepted_attachments.is_empty());
        assert_eq!(snapshot.verification_role, VerificationRole::RecordedOnly);
        assert!(snapshot.rejected_events.iter().any(|event| {
            event.issue_codes.contains(&"operation_unauthorized".to_owned())
                && event.issue_codes.contains(&"payload_hash_mismatch".to_owned())
        }));
    }

    #[test]
    fn concurrent_incompatible_dispositions_are_explicit_conflicts() {
        // r[verify valence.radicle_cob_evidence.convergence.invalid]
        let sidecar = oid('3');
        let attach_entry = oid('5');
        let replacement_entry = oid('6');
        let revoke_entry = oid('7');
        let supersede_entry = oid('8');
        let (original, original_resolution) = attachment("lint-original");
        let (replacement, replacement_resolution) = attachment("lint-replacement");
        let original_id = original.attachment_id.clone();
        let replacement_id = replacement.attachment_id.clone();
        let non_claims = profile().required_non_claims;
        let events = vec![
            root(&sidecar, ACTOR),
            observed(&sidecar, &attach_entry, vec![sidecar.clone()], ACTOR, 0, CobEventAction::Attach(original)),
            observed(
                &sidecar,
                &replacement_entry,
                vec![attach_entry.clone()],
                ACTOR,
                0,
                CobEventAction::Attach(replacement),
            ),
            observed(
                &sidecar,
                &revoke_entry,
                vec![replacement_entry.clone()],
                ACTOR,
                0,
                CobEventAction::Revoke(RevokeAction {
                    profile_hash: profile_identity(&profile()),
                    attachment_id: original_id.clone(),
                    reason: "producer withdrew the receipt".to_owned(),
                    verification_role: VerificationRole::RecordedOnly,
                    non_claims: non_claims.clone(),
                }),
            ),
            observed(
                &sidecar,
                &supersede_entry,
                vec![replacement_entry],
                ACTOR,
                0,
                CobEventAction::Supersede(SupersedeAction {
                    profile_hash: profile_identity(&profile()),
                    attachment_id: original_id.clone(),
                    replacement_attachment_id: replacement_id,
                    verification_role: VerificationRole::RecordedOnly,
                    non_claims,
                }),
            ),
        ];
        let report =
            reduce_logical_sidecar(&reduction_input(events, vec![original_resolution, replacement_resolution]));
        let snapshot = report.snapshot.expect("snapshot");
        assert_eq!(snapshot.conflicts.len(), 1);
        let original = snapshot
            .accepted_attachments
            .iter()
            .find(|accepted| accepted.attachment.attachment_id == original_id)
            .expect("original attachment");
        assert_eq!(original.status, AttachmentStatus::Active);
    }

    #[test]
    fn snapshot_is_recorded_only_and_detects_tamper() {
        // r[verify valence.radicle_cob_evidence.snapshot_handoff.valid]
        // r[verify valence.radicle_cob_evidence.snapshot_handoff.invalid]
        let sidecar = oid('3');
        let entry = oid('5');
        let (attachment, resolution) = attachment("lint-snapshot");
        let report = reduce_logical_sidecar(&reduction_input(
            vec![
                root(&sidecar, ACTOR),
                observed(&sidecar, &entry, vec![sidecar.clone()], ACTOR, 0, CobEventAction::Attach(attachment)),
            ],
            vec![resolution],
        ));
        let mut snapshot = report.snapshot.expect("snapshot");
        let artifact = snapshot_evidence_artifact(&profile(), &authorization(), &snapshot).expect("snapshot artifact");
        assert_eq!(artifact.verification_role, VerificationRole::RecordedOnly);
        snapshot.authorization_hash = format!("{BLAKE3_PREFIX}{}", "e".repeat(BLAKE3_HEX_LENGTH));
        let issues = verify_snapshot(&profile(), &authorization(), &snapshot);
        assert!(issues.iter().any(|issue| issue.code == "stale_materialized_state_hash"));
        assert!(issues.iter().any(|issue| issue.code == "stale_snapshot_receipt_hash"));
    }

    #[test]
    fn secret_and_deletion_dependent_evidence_fail_policy() {
        // r[verify valence.radicle_cob_evidence.privacy_retention.invalid]
        let artifact = artifact("secret");
        let bytes = serde_json::to_vec(&artifact).expect("serialize artifact");
        for confidentiality in [ConfidentialityClass::Secret, ConfidentialityClass::DeletionDependent] {
            let result = build_attachment(
                &profile(),
                &target(),
                AttachmentRelationship::ContextFor,
                EvidenceReference::from_artifact(&artifact),
                state(),
                AttachmentPayload::ReferenceOnly {
                    locator: "vault/secret.json".to_owned(),
                    media_type: "application/json".to_owned(),
                    size_bytes: u64::try_from(bytes.len()).expect("fixture size"),
                    raw_blake3: format!("{BLAKE3_PREFIX}{}", blake3_hex(&bytes)),
                },
                confidentiality,
                profile().required_non_claims,
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn malformed_profile_target_and_duplicate_fields_fail_closed() {
        // r[verify valence.radicle_cob_evidence.sidecar_profile.invalid]
        let mut unsupported_profile = profile();
        unsupported_profile.relationships.pop();
        unsupported_profile.embed_policy.max_bytes = 1;
        unsupported_profile.required_non_claims.push("unreviewed boundary".to_owned());
        let issues = validate_profile(&unsupported_profile);
        assert!(issues.iter().any(|issue| issue.code == "unsupported_relationship_vocabulary"));
        assert!(issues.iter().any(|issue| issue.code == "unsupported_embed_limit"));
        assert!(issues.iter().any(|issue| issue.code == "unsupported_non_claim_vocabulary"));

        let invalid_target = CobTargetRef {
            repository_rid: "not-a-rid".to_owned(),
            type_name: "invalid..type".to_owned(),
            object_id: format!("{BLAKE3_PREFIX}{}", "a".repeat(BLAKE3_HEX_LENGTH)),
        };
        let target_issues = target_key(&invalid_target).expect_err("invalid target must fail");
        assert!(target_issues.iter().any(|issue| issue.code == "invalid_repository_rid"));
        assert!(target_issues.iter().any(|issue| issue.code == "invalid_cob_type_name"));
        assert!(target_issues.iter().any(|issue| issue.code == "digest_domain_mismatch"));

        let duplicate = format!(
            "{{\"repository_rid\":\"rad:one\",\"repository_rid\":\"rad:two\",\"type_name\":\"x.y\",\"object_id\":\"{}\"}}",
            oid('a')
        );
        let error = serde_json::from_str::<CobTargetRef>(&duplicate).expect_err("duplicate field must fail");
        assert!(error.to_string().contains("duplicate field"));
    }

    #[test]
    fn oversized_tampered_embed_and_weakened_boundary_fail_closed() {
        // r[verify valence.radicle_cob_evidence.payload_modes.invalid]
        // r[verify valence.radicle_cob_evidence.role_policy.invalid]
        let (mut candidate, mut resolution) = attachment("embed-negative");
        candidate.payload = AttachmentPayload::Embedded {
            name: "evidence.json".to_owned(),
            git_oid: oid('b'),
            media_type: EMBEDDED_ARTIFACT_MEDIA_TYPE.to_owned(),
            size_bytes: MAX_EMBEDDED_ARTIFACT_BYTES + 1,
            raw_blake3: candidate.payload.raw_blake3().to_owned(),
        };
        candidate.attachment_id = attachment_identity(&candidate);
        resolution.attachment_id = candidate.attachment_id.clone();
        resolution.size_bytes = candidate.payload.size_bytes();
        resolution.git_oid = Some(oid('c'));
        resolution.locator = None;
        let issues = validate_resolution(&profile(), &candidate, &resolution);
        assert!(issues.iter().any(|issue| issue.code == "embed_limit_exceeded"));
        assert!(issues.iter().any(|issue| issue.code == "embed_resolution_mismatch"));

        candidate.non_claims.clear();
        candidate.attachment_id = attachment_identity(&candidate);
        let boundary_issues = validate_attachment_structure(&profile(), &candidate);
        assert!(boundary_issues.iter().any(|issue| issue.code == "non_claims_empty"));
        assert!(boundary_issues.iter().any(|issue| issue.code == "required_non_claim_missing"));
    }

    #[test]
    fn duplicate_replay_is_idempotent_and_invalid_signature_or_predecessor_is_rejected() {
        // r[verify valence.radicle_cob_evidence.convergence.valid]
        // r[verify valence.radicle_cob_evidence.authority.invalid]
        let sidecar_a = oid('3');
        let sidecar_b = oid('4');
        let entry_a = oid('5');
        let entry_b = oid('6');
        let revoke_entry = oid('7');
        let (attachment, resolution) = attachment("duplicate-replay");
        let attachment_id = attachment.attachment_id.clone();
        let mut invalid_signature = observed(
            &sidecar_b,
            &entry_b,
            vec![sidecar_b.clone()],
            OTHER_ACTOR,
            0,
            CobEventAction::Attach(attachment.clone()),
        );
        invalid_signature.signature_valid = false;
        let events = vec![
            root(&sidecar_a, ACTOR),
            observed(
                &sidecar_a,
                &entry_a,
                vec![sidecar_a.clone()],
                ACTOR,
                0,
                CobEventAction::Attach(attachment.clone()),
            ),
            root(&sidecar_b, OTHER_ACTOR),
            invalid_signature,
            observed(
                &sidecar_a,
                &revoke_entry,
                vec![entry_a.clone()],
                ACTOR,
                0,
                CobEventAction::Revoke(RevokeAction {
                    profile_hash: profile_identity(&profile()),
                    attachment_id: format!("{BLAKE3_PREFIX}{}", "d".repeat(BLAKE3_HEX_LENGTH)),
                    reason: "unknown producer evidence".to_owned(),
                    verification_role: VerificationRole::RecordedOnly,
                    non_claims: profile().required_non_claims,
                }),
            ),
            observed(&sidecar_a, &oid('8'), vec![entry_a], ACTOR, 0, CobEventAction::Attach(attachment)),
        ];
        let report = reduce_logical_sidecar(&reduction_input(events, vec![resolution]));
        let snapshot = report.snapshot.expect("snapshot");
        let accepted = snapshot
            .accepted_attachments
            .iter()
            .find(|accepted| accepted.attachment.attachment_id == attachment_id)
            .expect("deduplicated attachment");
        assert_eq!(snapshot.accepted_attachments.len(), 1);
        assert_eq!(accepted.source_entries.len(), 2);
        assert!(
            snapshot
                .rejected_events
                .iter()
                .any(|event| { event.issue_codes.contains(&"signature_invalid".to_owned()) })
        );
        assert!(
            snapshot
                .rejected_events
                .iter()
                .any(|event| { event.issue_codes.contains(&"unknown_or_noncausal_predecessor".to_owned()) })
        );
    }

    #[test]
    fn unknown_attachment_control_field_is_rejected_by_serde() {
        // r[verify valence.radicle_cob_evidence.sidecar_profile.invalid]
        let (attachment, _) = attachment("closed-shape");
        let mut value = serde_json::to_value(attachment).expect("serialize attachment");
        value["entry_oid"] = serde_json::Value::String(oid('9'));
        let error = serde_json::from_value::<CobEvidenceAttachment>(value).expect_err("unknown field must fail");
        assert!(error.to_string().contains("unknown field"));
    }
}
