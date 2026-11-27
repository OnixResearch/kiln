//! Pure projected-manifest persistence, tier placement, CAS admission, and
//! capability-scoped brokered download ticket model (`#80`).
//!
//! This module is `no_std + alloc` and performs zero I/O. Shells supply exact
//! bytes, storage observations, revocation state, and current unix timestamps.
//! Kiln content and record identities never include backend locators, bucket
//! names, credentials, or S3 ETags.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::authority::{PathScope, SourceViewOperation};
use super::disclosure::ViewCeiling;
use super::{
    EntryOrigin, ExpectedProjection, MAX_MANIFEST_ENTRIES, ResolvedProjectionManifest,
    update_external, verify_manifest,
};
use crate::monorepo::{finish, update_field, update_usize};
use crate::paths;
use crate::types::{ContentId, CoreResult, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef};

pub const MANIFEST_ENVELOPE_SCHEMA: &str = "kiln.projected-manifest.envelope.v1";
pub const MANIFEST_RECORD_IDENTITY_SCHEMA: &str = "kiln.projected-manifest.record-identity.v1";
pub const BROKER_TICKET_SCHEMA: &str = "kiln.projected-manifest.broker-ticket.v1";
pub const BROKER_TICKET_IDENTITY_SCHEMA: &str = "kiln.projected-manifest.broker-ticket-identity.v1";

/// Maximum serialized manifest envelope payload bytes (16 MiB).
pub const MAX_MANIFEST_ENVELOPE_BYTES: usize = 16_777_216;
/// Maximum object closure bindings carried by one manifest envelope.
pub const MAX_OBJECT_CLOSURE_ENTRIES: usize = MAX_MANIFEST_ENTRIES;
/// Maximum byte ceiling for one brokered download ticket (16 MiB).
pub const MAX_BROKER_DOWNLOAD_BYTES: usize = 16_777_216;
/// Maximum lifetime of one brokered download ticket (1 hour).
pub const MAX_BROKER_TICKET_TTL_SECS: u64 = 3_600;
/// Maximum retention roots attached to one manifest record.
pub const MAX_RETENTION_ROOTS: usize = 256;
/// Maximum candidate entries considered in one eviction batch.
pub const MAX_EVICTION_BATCH: usize = 1_024;
/// Maximum path scopes attached to one broker download ticket.
pub const MAX_BROKER_SCOPE_PATHS: usize = 256;
/// Maximum explicit ticket/audience/view revocations tracked in one state snapshot.
pub const MAX_REVOKED_ENTRIES: usize = 4_096;
/// Maximum bytes for a provider concurrency tag (e.g. ETag) retained as transport evidence.
pub const MAX_PROVIDER_TAG_BYTES: usize = 256;
/// Maximum bytes for an opaque retention root identifier.
pub const MAX_RETENTION_ROOT_ID_BYTES: usize = 256;

fn valid_bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn validate_object_digest(digest: &ContentId, bytes: &[u8]) -> CoreResult<()> {
    digest.validate()?;
    let observed = blake3::hash(bytes).to_hex().to_string();
    if digest.as_str() != observed {
        return Err(Diagnostic::new(
            "manifest_payload_digest_mismatch",
            "envelope.payload_digest",
            "manifest bytes do not match the declared BLAKE3 payload digest",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Storage tiers, lifecycle, and retention roots

/// Persistence tiers for projected source manifests.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageTier {
    MoltenPrimary,
    S3Secondary,
}

impl StorageTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MoltenPrimary => "molten_primary",
            Self::S3Secondary => "s3_secondary",
        }
    }
}

/// Observed state of one manifest record on a storage tier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierPresence {
    Absent,
    Staged,
    Verified,
    Evicted,
    Unknown,
}

/// Publication lifecycle stage for a projected manifest root.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestLifecycleStage {
    Staged,
    Verified,
    Published,
    Adopted,
}

/// Why a manifest and its object closure are protected from collection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionRootKind {
    ActiveWorkspace,
    PendingPublication,
    SealedCandidate,
    RetainedSnapshot,
    RequiredEvidence,
}

impl RetentionRootKind {
    /// Whether this root pins hot-tier `MoltenPrimary` against local eviction.
    #[must_use]
    pub const fn blocks_primary_eviction(self) -> bool {
        matches!(self, Self::ActiveWorkspace | Self::PendingPublication)
    }
}

/// A named retention root protecting a manifest record and its required objects.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionRoot {
    pub kind: RetentionRootKind,
    pub root_id: String,
}

impl RetentionRoot {
    pub fn validate(&self) -> CoreResult<()> {
        if !valid_bounded_text(&self.root_id, MAX_RETENTION_ROOT_ID_BYTES) {
            return Err(Diagnostic::new(
                "invalid_retention_root",
                "retention_root.root_id",
                "retention root identifier must be non-empty bounded text without control chars",
            ));
        }
        Ok(())
    }
}

/// Placement summary across `MoltenPrimary` and optional `S3Secondary`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierPlacement {
    pub molten_primary: TierPresence,
    pub s3_secondary: TierPresence,
    pub s3_enabled: bool,
}

// ---------------------------------------------------------------------------
// Canonical BLAKE3 manifest envelope and record identity

/// Binding between one native source object reference in a manifest and its
/// exact BLAKE3 byte digest and size.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestObjectBinding {
    pub object: ExternalRef,
    pub digest: ContentId,
    pub byte_len: usize,
}

impl ManifestObjectBinding {
    pub fn validate(&self) -> CoreResult<()> {
        self.object.validate()?;
        self.digest.validate()?;
        Ok(())
    }
}

/// Versioned persistence envelope for a [`ResolvedProjectionManifest`].
///
/// The canonical `record_identity` binds the manifest identity, content
/// identity, repository, exact revision, definition, canonical manifest
/// `payload_digest`, and required object closure. Backend locators, bucket
/// names, and S3 ETags never enter `record_identity`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEnvelope {
    pub schema: String,
    pub repository: ContentId,
    pub exact_revision: ExternalRef,
    pub definition: ContentId,
    pub manifest_identity: ContentId,
    pub content_identity: ContentId,
    pub payload_digest: ContentId,
    pub payload_bytes: usize,
    pub stage: ManifestLifecycleStage,
    /// Sorted uniquely by `object`.
    pub object_closure: Vec<ManifestObjectBinding>,
    pub record_identity: ContentId,
}

impl ManifestEnvelope {
    /// Construct a canonical verified envelope from a resolved manifest, its
    /// serialized canonical bytes, and the exact object-closure bindings.
    pub fn from_manifest(
        manifest: &ResolvedProjectionManifest,
        canonical_manifest_bytes: &[u8],
        object_closure: Vec<ManifestObjectBinding>,
        stage: ManifestLifecycleStage,
    ) -> CoreResult<Self> {
        if canonical_manifest_bytes.is_empty()
            || canonical_manifest_bytes.len() > MAX_MANIFEST_ENVELOPE_BYTES
        {
            return Err(Diagnostic::new(
                "manifest_envelope_size_bound",
                "envelope.payload_bytes",
                "manifest payload must be non-empty and within MAX_MANIFEST_ENVELOPE_BYTES",
            ));
        }
        if object_closure.len() > MAX_OBJECT_CLOSURE_ENTRIES {
            return Err(Diagnostic::new(
                "manifest_closure_bound",
                "envelope.object_closure",
                "manifest object closure exceeds MAX_OBJECT_CLOSURE_ENTRIES",
            ));
        }
        let mut sorted_closure = object_closure;
        sorted_closure.sort();
        for pair in sorted_closure.windows(2) {
            if let [first, second] = pair
                && first.object == second.object
            {
                return Err(Diagnostic::new(
                    "duplicate_object_closure_binding",
                    "envelope.object_closure",
                    "object closure must contain at most one binding per native object",
                ));
            }
        }
        for binding in &sorted_closure {
            binding.validate()?;
        }
        let payload_digest = ContentId::from_blake3_hex(
            blake3::hash(canonical_manifest_bytes).to_hex().to_string(),
        )?;
        let mut envelope = Self {
            schema: MANIFEST_ENVELOPE_SCHEMA.to_string(),
            repository: manifest.repository.clone(),
            exact_revision: manifest.exact_revision.clone(),
            definition: manifest.definition.clone(),
            manifest_identity: manifest.identity.clone(),
            content_identity: manifest.content_identity.clone(),
            payload_digest,
            payload_bytes: canonical_manifest_bytes.len(),
            stage,
            object_closure: sorted_closure,
            record_identity: manifest.identity.clone(),
        };
        envelope.record_identity = compute_record_identity(&envelope);
        verify_envelope(&envelope, manifest, canonical_manifest_bytes, None)?;
        Ok(envelope)
    }
}

fn compute_record_identity(envelope: &ManifestEnvelope) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(
        &mut hasher,
        "schema",
        MANIFEST_RECORD_IDENTITY_SCHEMA.as_bytes(),
    );
    update_field(
        &mut hasher,
        "envelope_schema",
        envelope.schema.as_bytes(),
    );
    update_field(
        &mut hasher,
        "repository",
        envelope.repository.as_str().as_bytes(),
    );
    update_external(&mut hasher, "exact_revision", &envelope.exact_revision);
    update_field(
        &mut hasher,
        "definition",
        envelope.definition.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "manifest_identity",
        envelope.manifest_identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "content_identity",
        envelope.content_identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "payload_digest",
        envelope.payload_digest.as_str().as_bytes(),
    );
    update_usize(&mut hasher, "payload_bytes", envelope.payload_bytes);
    update_usize(&mut hasher, "object_closure", envelope.object_closure.len());
    for binding in &envelope.object_closure {
        update_external(&mut hasher, "object", &binding.object);
        update_field(&mut hasher, "digest", binding.digest.as_str().as_bytes());
        update_usize(&mut hasher, "byte_len", binding.byte_len);
    }
    finish(hasher)
}

/// Verify an untrusted or reloaded [`ManifestEnvelope`] against the decoded
/// [`ResolvedProjectionManifest`] and raw canonical payload bytes.
pub fn verify_envelope(
    envelope: &ManifestEnvelope,
    manifest: &ResolvedProjectionManifest,
    canonical_manifest_bytes: &[u8],
    expected: Option<ExpectedProjection<'_>>,
) -> CoreResult<()> {
    if envelope.schema != MANIFEST_ENVELOPE_SCHEMA {
        return Err(Diagnostic::new(
            "unsupported_envelope_schema",
            "envelope.schema",
            "projected manifest envelope schema is unsupported",
        ));
    }
    if envelope.payload_bytes == 0
        || envelope.payload_bytes > MAX_MANIFEST_ENVELOPE_BYTES
        || canonical_manifest_bytes.len() != envelope.payload_bytes
    {
        return Err(Diagnostic::new(
            "manifest_envelope_size_mismatch",
            "envelope.payload_bytes",
            "manifest payload length does not match envelope bounds",
        ));
    }
    if envelope.object_closure.len() > MAX_OBJECT_CLOSURE_ENTRIES {
        return Err(Diagnostic::new(
            "manifest_closure_bound",
            "envelope.object_closure",
            "manifest object closure exceeds MAX_OBJECT_CLOSURE_ENTRIES",
        ));
    }
    envelope.repository.validate()?;
    envelope.exact_revision.validate()?;
    envelope.definition.validate()?;
    envelope.manifest_identity.validate()?;
    envelope.content_identity.validate()?;
    envelope.record_identity.validate()?;
    validate_object_digest(&envelope.payload_digest, canonical_manifest_bytes)?;

    let self_expected = ExpectedProjection {
        repository: &envelope.repository,
        exact_revision: &envelope.exact_revision,
        definition: &envelope.definition,
        evaluator: &manifest.evaluator,
        adapter: &manifest.adapter,
    };
    verify_manifest(manifest, expected.unwrap_or(self_expected)).map_err(|mut diagnostics| {
        diagnostics
            .drain(..)
            .next()
            .unwrap_or_else(|| Diagnostic::new("invalid_manifest", "manifest", "manifest invalid"))
    })?;

    if manifest.identity != envelope.manifest_identity
        || manifest.content_identity != envelope.content_identity
        || manifest.repository != envelope.repository
        || manifest.exact_revision != envelope.exact_revision
        || manifest.definition != envelope.definition
    {
        return Err(Diagnostic::new(
            "envelope_manifest_binding_mismatch",
            "envelope.manifest_identity",
            "envelope bindings do not match the manifest",
        ));
    }

    if !envelope
        .object_closure
        .windows(2)
        .all(|pair| matches!(pair, [a, b] if a.object < b.object))
    {
        return Err(Diagnostic::new(
            "envelope_closure_order",
            "envelope.object_closure",
            "object closure must be strictly sorted and deduplicated by object",
        ));
    }

    let mut closure_objects = BTreeSet::new();
    for binding in &envelope.object_closure {
        binding.validate()?;
        closure_objects.insert(&binding.object);
    }
    for entry in &manifest.entries {
        if let EntryOrigin::Native { object, .. } = &entry.origin
            && !closure_objects.contains(object)
        {
            return Err(Diagnostic::new(
                "incomplete_object_closure",
                "envelope.object_closure",
                "manifest native entry is missing from the envelope object closure",
            ));
        }
    }

    if compute_record_identity(envelope) != envelope.record_identity {
        return Err(Diagnostic::new(
            "record_identity_mismatch",
            "envelope.record_identity",
            "envelope record_identity does not match canonical BLAKE3 digest",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conditional-write CAS admission and outcome state machine

/// Conditional-write precondition for manifest persistence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case", deny_unknown_fields)]
pub enum CasCondition {
    /// Succeed only if the target tier has no active record (`If-None-Match: *`).
    PutIfAbsent,
    /// Succeed only if the target tier currently holds this verified `record_identity`.
    ExpectedDigest { expected_record_identity: ContentId },
}

/// Request to admit a conditional write of a manifest envelope to a tier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CasWriteRequest {
    pub tier: StorageTier,
    pub condition: CasCondition,
    pub envelope: ManifestEnvelope,
    pub retention_roots: BTreeSet<RetentionRoot>,
}

/// Current state of a manifest slot on one storage tier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierRecordState {
    pub record_identity: Option<ContentId>,
    pub presence: TierPresence,
    /// Transport/concurrency tag (e.g. S3 ETag). Never replaces Kiln identity.
    pub provider_etag: Option<String>,
}

impl Default for TierRecordState {
    fn default() -> Self {
        Self {
            record_identity: None,
            presence: TierPresence::Absent,
            provider_etag: None,
        }
    }
}

/// Pure admitted plan for executing a conditional write in a shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasWritePlan {
    pub tier: StorageTier,
    pub condition: CasCondition,
    pub manifest_identity: ContentId,
    pub record_identity: ContentId,
    pub payload_digest: ContentId,
    pub payload_bytes: usize,
    pub expected_provider_etag: Option<String>,
}

/// Pure admission of a conditional-write CAS request.
pub fn admit_cas_write(
    request: &CasWriteRequest,
    current: &TierRecordState,
    s3_enabled: bool,
    max_envelope_bytes: usize,
) -> CoreResult<CasWritePlan> {
    if request.tier == StorageTier::S3Secondary && !s3_enabled {
        return Err(Diagnostic::new(
            "s3_tier_disabled",
            "cas.tier",
            "S3 secondary tier is disabled or unconfigured",
        ));
    }
    if max_envelope_bytes == 0 || max_envelope_bytes > MAX_MANIFEST_ENVELOPE_BYTES {
        return Err(Diagnostic::new(
            "invalid_envelope_bound",
            "cas.max_envelope_bytes",
            "envelope byte limit must be nonzero and within MAX_MANIFEST_ENVELOPE_BYTES",
        ));
    }
    if request.envelope.payload_bytes == 0 || request.envelope.payload_bytes > max_envelope_bytes {
        return Err(Diagnostic::new(
            "manifest_envelope_size_bound",
            "cas.envelope.payload_bytes",
            "manifest envelope exceeds admitted payload size bound",
        ));
    }
    if request.retention_roots.len() > MAX_RETENTION_ROOTS {
        return Err(Diagnostic::new(
            "retention_roots_bound",
            "cas.retention_roots",
            "retention root count exceeds MAX_RETENTION_ROOTS",
        ));
    }
    for root in &request.retention_roots {
        root.validate()?;
    }
    request.envelope.manifest_identity.validate()?;
    request.envelope.record_identity.validate()?;
    request.envelope.payload_digest.validate()?;
    if compute_record_identity(&request.envelope) != request.envelope.record_identity {
        return Err(Diagnostic::new(
            "record_identity_mismatch",
            "cas.envelope.record_identity",
            "envelope record_identity does not match canonical BLAKE3 digest",
        ));
    }

    if current.presence == TierPresence::Unknown {
        return Err(Diagnostic::new(
            "cas_unknown_prior_state",
            "cas.current.presence",
            "cannot perform conditional write while tier state is unknown; reconcile first",
        ));
    }

    match &request.condition {
        CasCondition::PutIfAbsent => {
            if !matches!(
                current.presence,
                TierPresence::Absent | TierPresence::Evicted
            ) {
                return Err(Diagnostic::new(
                    "cas_conflict",
                    "cas.condition",
                    "put_if_absent refused because tier already holds a manifest record",
                ));
            }
        }
        CasCondition::ExpectedDigest {
            expected_record_identity,
        } => {
            expected_record_identity.validate()?;
            if current.presence != TierPresence::Verified
                || current.record_identity.as_ref() != Some(expected_record_identity)
            {
                return Err(Diagnostic::new(
                    "cas_conflict",
                    "cas.condition",
                    "expected_digest refused because current tier record does not match",
                ));
            }
        }
    }

    Ok(CasWritePlan {
        tier: request.tier,
        condition: request.condition.clone(),
        manifest_identity: request.envelope.manifest_identity.clone(),
        record_identity: request.envelope.record_identity.clone(),
        payload_digest: request.envelope.payload_digest.clone(),
        payload_bytes: request.envelope.payload_bytes,
        expected_provider_etag: current.provider_etag.clone(),
    })
}

/// Result reported by a storage shell after executing a [`CasWritePlan`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BackendWriteOutcome {
    Committed { provider_etag: Option<String> },
    CasConflict { observed_etag: Option<String> },
    Unknown { reason: String },
    Failed { code: String, message: String },
}

/// Pure transition resulting from applying a [`BackendWriteOutcome`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceWriteDecision {
    Committed {
        state: TierRecordState,
    },
    Conflict {
        state: TierRecordState,
        diagnostic: Diagnostic,
    },
    Unknown {
        state: TierRecordState,
        diagnostic: Diagnostic,
    },
    Refused {
        state: TierRecordState,
        diagnostic: Diagnostic,
    },
}

fn sanitize_provider_tag(tag: Option<String>) -> Option<String> {
    tag.filter(|value| valid_bounded_text(value, MAX_PROVIDER_TAG_BYTES))
}

/// Pure transition for a conditional write outcome.
///
/// Ambiguous remote outcomes produce `PersistenceWriteDecision::Unknown` and
/// mark the tier `TierPresence::Unknown` so subsequent operations cannot assume
/// the write succeeded or failed.
#[must_use]
pub fn apply_cas_write_outcome(
    plan: &CasWritePlan,
    prior: &TierRecordState,
    outcome: BackendWriteOutcome,
) -> PersistenceWriteDecision {
    match outcome {
        BackendWriteOutcome::Committed { provider_etag } => PersistenceWriteDecision::Committed {
            state: TierRecordState {
                record_identity: Some(plan.record_identity.clone()),
                presence: TierPresence::Verified,
                provider_etag: sanitize_provider_tag(provider_etag),
            },
        },
        BackendWriteOutcome::CasConflict { observed_etag } => {
            let mut next = prior.clone();
            if let Some(etag) = sanitize_provider_tag(observed_etag) {
                next.provider_etag = Some(etag);
            }
            PersistenceWriteDecision::Conflict {
                state: next,
                diagnostic: Diagnostic::new(
                    "cas_conflict",
                    "cas.outcome",
                    "backend conditional write failed precondition",
                ),
            }
        }
        BackendWriteOutcome::Unknown { reason } => {
            let mut next = prior.clone();
            next.presence = TierPresence::Unknown;
            PersistenceWriteDecision::Unknown {
                state: next,
                diagnostic: Diagnostic::new(
                    "remote_write_unknown",
                    "cas.outcome",
                    if reason.trim().is_empty() {
                        "remote manifest write outcome is unknown until reconciliation"
                    } else {
                        &reason
                    },
                ),
            }
        }
        BackendWriteOutcome::Failed { code, message } => PersistenceWriteDecision::Refused {
            state: prior.clone(),
            diagnostic: Diagnostic::new(
                if code.trim().is_empty() {
                    "backend_write_failed"
                } else {
                    &code
                },
                "cas.outcome",
                if message.trim().is_empty() {
                    "backend manifest write failed before commit"
                } else {
                    &message
                },
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Tier placement, readback planning, and bounded eviction

/// Watermarks and secondary safety rules governing tier eviction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvictionPolicy {
    pub high_watermark_entries: usize,
    pub low_watermark_entries: usize,
    pub high_watermark_bytes: usize,
    pub low_watermark_bytes: usize,
    pub require_secondary_before_primary_eviction: bool,
}

impl EvictionPolicy {
    pub fn validate(&self) -> CoreResult<()> {
        if self.high_watermark_entries == 0
            || self.high_watermark_bytes == 0
            || self.low_watermark_entries > self.high_watermark_entries
            || self.low_watermark_bytes > self.high_watermark_bytes
        {
            return Err(Diagnostic::new(
                "invalid_eviction_watermarks",
                "eviction.policy",
                "eviction watermarks must satisfy 0 <= low <= high and high > 0",
            ));
        }
        Ok(())
    }
}

/// Candidate record evaluated for tier eviction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvictionCandidate {
    pub manifest_identity: ContentId,
    pub record_identity: ContentId,
    pub payload_bytes: usize,
    pub placement: TierPlacement,
    pub retention_roots: BTreeSet<RetentionRoot>,
    /// Monotonic age rank (smaller = older / preferred eviction target).
    pub age_rank: u64,
}

/// Request to evict a single manifest record from one tier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvictionRequest {
    pub target_tier: StorageTier,
    pub current_entries: usize,
    pub current_bytes: usize,
    pub candidate: EvictionCandidate,
}

/// Pure admitted eviction plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvictionPlan {
    pub target_tier: StorageTier,
    pub manifest_identity: ContentId,
    pub record_identity: ContentId,
    pub payload_bytes: usize,
}

/// Pure admission of a single-record tier eviction.
pub fn admit_eviction(policy: &EvictionPolicy, request: &EvictionRequest) -> CoreResult<EvictionPlan> {
    policy.validate()?;
    let candidate = &request.candidate;
    candidate.manifest_identity.validate()?;
    candidate.record_identity.validate()?;
    if candidate.retention_roots.len() > MAX_RETENTION_ROOTS {
        return Err(Diagnostic::new(
            "retention_roots_bound",
            "eviction.retention_roots",
            "retention root count exceeds MAX_RETENTION_ROOTS",
        ));
    }
    for root in &candidate.retention_roots {
        root.validate()?;
    }

    let over_watermark = request.current_entries > policy.high_watermark_entries
        || request.current_bytes > policy.high_watermark_bytes;
    if !over_watermark {
        return Err(Diagnostic::new(
            "below_eviction_watermark",
            "eviction.watermark",
            "tier usage does not exceed high watermark",
        ));
    }

    match request.target_tier {
        StorageTier::MoltenPrimary => {
            if candidate.placement.molten_primary != TierPresence::Verified {
                return Err(Diagnostic::new(
                    "tier_not_evictable",
                    "eviction.placement.molten_primary",
                    "only a verified MoltenPrimary record can be evicted",
                ));
            }
            if candidate
                .retention_roots
                .iter()
                .any(|root| root.kind.blocks_primary_eviction())
            {
                return Err(Diagnostic::new(
                    "retention_root_protected",
                    "eviction.retention_roots",
                    "active workspace or pending publication root blocks MoltenPrimary eviction",
                ));
            }
            let has_durable_root = !candidate.retention_roots.is_empty();
            if (policy.require_secondary_before_primary_eviction || has_durable_root)
                && (!candidate.placement.s3_enabled
                    || candidate.placement.s3_secondary != TierPresence::Verified)
            {
                return Err(Diagnostic::new(
                    "secondary_not_verified",
                    "eviction.placement.s3_secondary",
                    "cannot evict from MoltenPrimary without a verified S3Secondary copy",
                ));
            }
        }
        StorageTier::S3Secondary => {
            if !candidate.placement.s3_enabled {
                return Err(Diagnostic::new(
                    "s3_tier_disabled",
                    "eviction.placement.s3_enabled",
                    "S3 secondary tier is disabled or unconfigured",
                ));
            }
            if candidate.placement.s3_secondary != TierPresence::Verified {
                return Err(Diagnostic::new(
                    "tier_not_evictable",
                    "eviction.placement.s3_secondary",
                    "only a verified S3Secondary record can be evicted",
                ));
            }
            if !candidate.retention_roots.is_empty() {
                return Err(Diagnostic::new(
                    "durable_retention_protected",
                    "eviction.retention_roots",
                    "retention roots forbid deleting a manifest from durable S3Secondary",
                ));
            }
        }
    }

    Ok(EvictionPlan {
        target_tier: request.target_tier,
        manifest_identity: candidate.manifest_identity.clone(),
        record_identity: candidate.record_identity.clone(),
        payload_bytes: candidate.payload_bytes,
    })
}

/// Plan a bounded batch of evictions down to the low watermark, oldest first.
pub fn select_evictions(
    policy: &EvictionPolicy,
    target_tier: StorageTier,
    current_entries: usize,
    current_bytes: usize,
    candidates: &[EvictionCandidate],
) -> CoreResult<Vec<EvictionPlan>> {
    policy.validate()?;
    if candidates.len() > MAX_EVICTION_BATCH {
        return Err(Diagnostic::new(
            "eviction_batch_bound",
            "eviction.candidates",
            "candidate slice exceeds MAX_EVICTION_BATCH",
        ));
    }
    let mut ordered: Vec<&EvictionCandidate> = candidates.iter().collect();
    ordered.sort_by(|a, b| {
        a.age_rank
            .cmp(&b.age_rank)
            .then_with(|| a.manifest_identity.cmp(&b.manifest_identity))
    });

    let mut entries = current_entries;
    let mut bytes = current_bytes;
    let mut plans = Vec::new();

    for candidate in ordered {
        if entries <= policy.low_watermark_entries && bytes <= policy.low_watermark_bytes {
            break;
        }
        let request = EvictionRequest {
            target_tier,
            current_entries: entries.max(policy.high_watermark_entries.saturating_add(1)),
            current_bytes: bytes.max(policy.high_watermark_bytes.saturating_add(1)),
            candidate: candidate.clone(),
        };
        if (entries > policy.high_watermark_entries
            || bytes > policy.high_watermark_bytes
            || !plans.is_empty())
            && let Ok(plan) = admit_eviction(policy, &request)
        {
            entries = entries.saturating_sub(1);
            bytes = bytes.saturating_sub(plan.payload_bytes);
            plans.push(plan);
        }
    }

    Ok(plans)
}

/// Outcome reported by a shell after attempting tier eviction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BackendEvictOutcome {
    Evicted,
    Unknown { reason: String },
    Failed { code: String, message: String },
}

/// Pure transition resulting from applying a [`BackendEvictOutcome`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvictionDecision {
    Evicted {
        placement: TierPlacement,
    },
    Unknown {
        placement: TierPlacement,
        diagnostic: Diagnostic,
    },
    Refused {
        placement: TierPlacement,
        diagnostic: Diagnostic,
    },
}

/// Apply an eviction outcome, preserving `TierPresence::Unknown` when remote
/// eviction status is ambiguous.
#[must_use]
pub fn apply_eviction_outcome(
    plan: &EvictionPlan,
    prior: TierPlacement,
    outcome: BackendEvictOutcome,
) -> EvictionDecision {
    let set_tier = |mut placement: TierPlacement, presence: TierPresence| {
        match plan.target_tier {
            StorageTier::MoltenPrimary => placement.molten_primary = presence,
            StorageTier::S3Secondary => placement.s3_secondary = presence,
        }
        placement
    };
    match outcome {
        BackendEvictOutcome::Evicted => EvictionDecision::Evicted {
            placement: set_tier(prior, TierPresence::Evicted),
        },
        BackendEvictOutcome::Unknown { reason } => EvictionDecision::Unknown {
            placement: set_tier(prior, TierPresence::Unknown),
            diagnostic: Diagnostic::new(
                "remote_evict_unknown",
                "eviction.outcome",
                if reason.trim().is_empty() {
                    "remote manifest eviction outcome is unknown until reconciliation"
                } else {
                    &reason
                },
            ),
        },
        BackendEvictOutcome::Failed { code, message } => EvictionDecision::Refused {
            placement: prior,
            diagnostic: Diagnostic::new(
                if code.trim().is_empty() {
                    "backend_evict_failed"
                } else {
                    &code
                },
                "eviction.outcome",
                if message.trim().is_empty() {
                    "backend eviction failed"
                } else {
                    &message
                },
            ),
        },
    }
}

/// Which tier a read should target given current placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadTierPlan {
    MoltenPrimary,
    S3SecondaryReadback,
}

/// Decide which tier serves a manifest readback, failing closed on missing,
/// evicted-without-secondary, or unknown tier states.
pub fn plan_read_tier(placement: &TierPlacement) -> CoreResult<ReadTierPlan> {
    if placement.molten_primary == TierPresence::Verified {
        return Ok(ReadTierPlan::MoltenPrimary);
    }
    if placement.molten_primary == TierPresence::Unknown {
        return Err(Diagnostic::new(
            "manifest_tier_unknown",
            "placement.molten_primary",
            "MoltenPrimary manifest state is unknown",
        ));
    }
    if placement.s3_enabled {
        match placement.s3_secondary {
            TierPresence::Verified => return Ok(ReadTierPlan::S3SecondaryReadback),
            TierPresence::Unknown => {
                return Err(Diagnostic::new(
                    "manifest_tier_unknown",
                    "placement.s3_secondary",
                    "S3Secondary manifest state is unknown",
                ));
            }
            TierPresence::Absent | TierPresence::Staged | TierPresence::Evicted => {}
        }
    }
    if placement.molten_primary == TierPresence::Evicted {
        return Err(Diagnostic::new(
            "manifest_evicted_no_secondary",
            "placement",
            "manifest was evicted from MoltenPrimary and no verified S3Secondary copy exists",
        ));
    }
    Err(Diagnostic::new(
        "manifest_unavailable",
        "placement",
        "manifest is not available on any verified storage tier",
    ))
}

// ---------------------------------------------------------------------------
// Capability-restricted brokered download ticket admission

/// What a broker download ticket authorizes reading.
///
/// Notice that raw S3 buckets, credentials, presigned bearer URLs, and
/// storage object keys never appear in this model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrokerTargetKind {
    /// Read one exact blob within the view's disclosure ceiling.
    Blob {
        destination: String,
        object_digest: ContentId,
        max_bytes: usize,
    },
    /// Read only the view-authorized subset of a projected manifest.
    ManifestView,
    /// Read the complete projected manifest; admitted only when the view
    /// ceiling covers the entire repository root.
    FullManifest,
}

/// Strict view ceiling and authority bindings carried by a download ticket.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrokerTicketScope {
    pub repository: ContentId,
    pub exact_revision: ExternalRef,
    pub view_binding: ContentId,
    pub audience: String,
    pub ceiling: ViewCeiling,
    pub generation: u64,
    pub revocation_generation: u64,
    pub allowed_paths: BTreeSet<PathScope>,
    pub max_bytes: usize,
}

/// Capability-restricted brokered download ticket.
///
/// Contains only Kiln identities, view ceilings, expiry bounds, and an opaque
/// BLAKE3 `ticket_id`. Never contains bucket credentials or object keys.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrokerDownloadTicket {
    pub schema: String,
    pub ticket_id: ContentId,
    pub manifest_identity: ContentId,
    pub target: BrokerTargetKind,
    pub scope: BrokerTicketScope,
    pub issued_at_unix_s: u64,
    pub expires_at_unix_s: u64,
}

/// Request presented to exercise a [`BrokerDownloadTicket`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerTicketRequest {
    pub ticket_id: ContentId,
    pub audience: String,
    pub view_binding: ContentId,
    pub expected_digest: ContentId,
    pub path: Option<String>,
    pub requested_bytes: usize,
}

/// Current revocation and generation snapshot owned by the broker shell.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerRevocationState {
    pub current_generation: u64,
    pub revocation_generation: u64,
    pub revoked_tickets: BTreeSet<ContentId>,
    pub revoked_audiences: BTreeSet<String>,
    pub revoked_views: BTreeSet<ContentId>,
}

/// Why a brokered download ticket was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerDenial {
    TicketInvalid,
    Expired,
    Revoked,
    StaleGeneration,
    OutOfScope,
    DigestMismatch,
    ByteCeilingExceeded,
}

impl BrokerDenial {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TicketInvalid => "ticket_invalid",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
            Self::StaleGeneration => "stale_generation",
            Self::OutOfScope => "out_of_scope",
            Self::DigestMismatch => "digest_mismatch",
            Self::ByteCeilingExceeded => "byte_ceiling_exceeded",
        }
    }

    #[must_use]
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::new(
            self.as_str(),
            "broker.ticket",
            match self {
                Self::TicketInvalid => "broker download ticket is malformed or forged",
                Self::Expired => "broker download ticket is expired or not yet valid",
                Self::Revoked => "broker download ticket or holder authority has been revoked",
                Self::StaleGeneration => "broker download ticket generation is stale",
                Self::OutOfScope => "requested path, audience, or view lies outside ticket ceiling",
                Self::DigestMismatch => "requested digest does not match ticket digest",
                Self::ByteCeilingExceeded => "requested read size exceeds ticket byte ceiling",
            },
        )
    }
}

/// Unforgeable witness produced by [`admit_broker_download`] authorizing a
/// shell to perform one bounded read.
///
/// Private fields prevent callers from constructing or deserializing an
/// admitted read directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedBrokerRead {
    ticket_id: ContentId,
    manifest_identity: ContentId,
    expected_digest: ContentId,
    max_bytes: usize,
    target: BrokerTargetKind,
    ceiling: ViewCeiling,
    allowed_paths: BTreeSet<PathScope>,
}

impl AdmittedBrokerRead {
    #[must_use]
    pub fn ticket_id(&self) -> &ContentId {
        &self.ticket_id
    }

    #[must_use]
    pub fn manifest_identity(&self) -> &ContentId {
        &self.manifest_identity
    }

    #[must_use]
    pub fn expected_digest(&self) -> &ContentId {
        &self.expected_digest
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    #[must_use]
    pub fn target(&self) -> &BrokerTargetKind {
        &self.target
    }

    #[must_use]
    pub fn ceiling(&self) -> &ViewCeiling {
        &self.ceiling
    }

    #[must_use]
    pub fn allowed_paths(&self) -> &BTreeSet<PathScope> {
        &self.allowed_paths
    }

    /// Whether a manifest entry path is authorized by both the view ceiling
    /// and the ticket's allowed path scopes.
    #[must_use]
    pub fn permits_path(&self, path: &str) -> bool {
        self.ceiling.allows(SourceViewOperation::ReadContent, path)
            && self
                .allowed_paths
                .iter()
                .any(|scope| scope.covers_path(path))
    }
}

fn compute_ticket_id(
    manifest_identity: &ContentId,
    target: &BrokerTargetKind,
    scope: &BrokerTicketScope,
    issued_at_unix_s: u64,
    expires_at_unix_s: u64,
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(
        &mut hasher,
        "schema",
        BROKER_TICKET_IDENTITY_SCHEMA.as_bytes(),
    );
    update_field(
        &mut hasher,
        "manifest_identity",
        manifest_identity.as_str().as_bytes(),
    );
    match target {
        BrokerTargetKind::Blob {
            destination,
            object_digest,
            max_bytes,
        } => {
            update_field(&mut hasher, "target", b"blob");
            update_field(&mut hasher, "destination", destination.as_bytes());
            update_field(
                &mut hasher,
                "object_digest",
                object_digest.as_str().as_bytes(),
            );
            update_usize(&mut hasher, "target_max_bytes", *max_bytes);
        }
        BrokerTargetKind::ManifestView => {
            update_field(&mut hasher, "target", b"manifest_view");
        }
        BrokerTargetKind::FullManifest => {
            update_field(&mut hasher, "target", b"full_manifest");
        }
    }
    update_field(
        &mut hasher,
        "repository",
        scope.repository.as_str().as_bytes(),
    );
    update_external(&mut hasher, "exact_revision", &scope.exact_revision);
    update_field(
        &mut hasher,
        "view_binding",
        scope.view_binding.as_str().as_bytes(),
    );
    update_field(&mut hasher, "audience", scope.audience.as_bytes());
    update_field(
        &mut hasher,
        "ceiling",
        scope.ceiling.identity().as_str().as_bytes(),
    );
    hasher.update(&scope.generation.to_le_bytes());
    hasher.update(&scope.revocation_generation.to_le_bytes());
    update_usize(&mut hasher, "allowed_paths", scope.allowed_paths.len());
    for path in &scope.allowed_paths {
        path.update(&mut hasher);
    }
    update_usize(&mut hasher, "max_bytes", scope.max_bytes);
    hasher.update(&issued_at_unix_s.to_le_bytes());
    hasher.update(&expires_at_unix_s.to_le_bytes());
    finish(hasher)
}

fn validate_scope(scope: &BrokerTicketScope) -> CoreResult<()> {
    scope.repository.validate()?;
    scope.exact_revision.validate()?;
    scope.view_binding.validate()?;
    if !valid_bounded_text(&scope.audience, DEFAULT_MAX_ID_BYTES) {
        return Err(Diagnostic::new(
            "invalid_ticket_audience",
            "ticket.scope.audience",
            "ticket audience must be non-empty bounded text",
        ));
    }
    if scope.max_bytes == 0 || scope.max_bytes > MAX_BROKER_DOWNLOAD_BYTES {
        return Err(Diagnostic::new(
            "invalid_ticket_byte_bound",
            "ticket.scope.max_bytes",
            "ticket max_bytes must be nonzero and within MAX_BROKER_DOWNLOAD_BYTES",
        ));
    }
    if !scope
        .ceiling
        .operations()
        .contains(&SourceViewOperation::ReadContent)
    {
        return Err(Diagnostic::new(
            "ceiling_forbids_read",
            "ticket.scope.ceiling",
            "view ceiling does not permit read_content",
        ));
    }
    if scope.allowed_paths.is_empty() || scope.allowed_paths.len() > MAX_BROKER_SCOPE_PATHS {
        return Err(Diagnostic::new(
            "invalid_ticket_paths",
            "ticket.scope.allowed_paths",
            "ticket allowed_paths must be non-empty and within MAX_BROKER_SCOPE_PATHS",
        ));
    }
    for path in &scope.allowed_paths {
        path.validate()?;
        if !scope.ceiling.contains(path) {
            return Err(Diagnostic::new(
                "scope_exceeds_view_ceiling",
                "ticket.scope.allowed_paths",
                "ticket path scope exceeds the frozen view ceiling",
            ));
        }
    }
    Ok(())
}

/// Issue a capability-restricted brokered download ticket after validating
/// view ceiling, path scope, TTL, and byte bounds.
pub fn admit_broker_ticket_issue(
    manifest_identity: ContentId,
    target: BrokerTargetKind,
    scope: BrokerTicketScope,
    issued_at_unix_s: u64,
    expires_at_unix_s: u64,
) -> CoreResult<BrokerDownloadTicket> {
    manifest_identity.validate()?;
    validate_scope(&scope)?;
    if expires_at_unix_s <= issued_at_unix_s
        || expires_at_unix_s - issued_at_unix_s > MAX_BROKER_TICKET_TTL_SECS
    {
        return Err(Diagnostic::new(
            "invalid_ticket_ttl",
            "ticket.expires_at_unix_s",
            "ticket TTL must be positive and within MAX_BROKER_TICKET_TTL_SECS",
        ));
    }

    match &target {
        BrokerTargetKind::Blob {
            destination,
            object_digest,
            max_bytes,
        } => {
            if !paths::canonical(destination) {
                return Err(Diagnostic::new(
                    "invalid_ticket_destination",
                    "ticket.target.destination",
                    "blob destination must be a canonical relative path",
                ));
            }
            object_digest.validate()?;
            if *max_bytes == 0 || *max_bytes > scope.max_bytes {
                return Err(Diagnostic::new(
                    "invalid_ticket_byte_bound",
                    "ticket.target.max_bytes",
                    "blob byte bound must fit within ticket scope max_bytes",
                ));
            }
            if !scope
                .ceiling
                .allows(SourceViewOperation::ReadContent, destination)
                || !scope
                    .allowed_paths
                    .iter()
                    .any(|path| path.covers_path(destination))
            {
                return Err(Diagnostic::new(
                    "blob_outside_view_ceiling",
                    "ticket.target.destination",
                    "blob destination lies outside the authorized view ceiling or allowed paths",
                ));
            }
        }
        BrokerTargetKind::ManifestView => {}
        BrokerTargetKind::FullManifest => {
            let root = PathScope::root();
            if !scope.ceiling.contains(&root)
                || !scope.allowed_paths.iter().any(|p| p.contains(&root))
            {
                return Err(Diagnostic::new(
                    "full_manifest_outside_ceiling",
                    "ticket.target",
                    "full-manifest download requires root coverage in both ceiling and allowed_paths",
                ));
            }
        }
    }

    let ticket_id = compute_ticket_id(
        &manifest_identity,
        &target,
        &scope,
        issued_at_unix_s,
        expires_at_unix_s,
    );
    Ok(BrokerDownloadTicket {
        schema: BROKER_TICKET_SCHEMA.to_string(),
        ticket_id,
        manifest_identity,
        target,
        scope,
        issued_at_unix_s,
        expires_at_unix_s,
    })
}

/// Pre-read validation of a broker download ticket against current revocations,
/// time, view ceiling, exact digest, and byte limits.
pub fn admit_broker_download(
    ticket: &BrokerDownloadTicket,
    request: &BrokerTicketRequest,
    revocations: &BrokerRevocationState,
    now_unix_s: u64,
) -> Result<AdmittedBrokerRead, BrokerDenial> {
    if ticket.schema != BROKER_TICKET_SCHEMA
        || ticket.manifest_identity.validate().is_err()
        || ticket.ticket_id.validate().is_err()
        || validate_scope(&ticket.scope).is_err()
    {
        return Err(BrokerDenial::TicketInvalid);
    }
    if compute_ticket_id(
        &ticket.manifest_identity,
        &ticket.target,
        &ticket.scope,
        ticket.issued_at_unix_s,
        ticket.expires_at_unix_s,
    ) != ticket.ticket_id
    {
        return Err(BrokerDenial::TicketInvalid);
    }
    if ticket.ticket_id != request.ticket_id {
        return Err(BrokerDenial::TicketInvalid);
    }
    if ticket.expires_at_unix_s <= ticket.issued_at_unix_s
        || ticket.expires_at_unix_s - ticket.issued_at_unix_s > MAX_BROKER_TICKET_TTL_SECS
        || now_unix_s < ticket.issued_at_unix_s
        || now_unix_s >= ticket.expires_at_unix_s
    {
        return Err(BrokerDenial::Expired);
    }
    if revocations.revoked_tickets.len() > MAX_REVOKED_ENTRIES
        || revocations.revoked_audiences.len() > MAX_REVOKED_ENTRIES
        || revocations.revoked_views.len() > MAX_REVOKED_ENTRIES
    {
        return Err(BrokerDenial::TicketInvalid);
    }
    if revocations.revoked_tickets.contains(&ticket.ticket_id)
        || revocations.revoked_audiences.contains(&ticket.scope.audience)
        || revocations.revoked_views.contains(&ticket.scope.view_binding)
        || ticket.scope.revocation_generation != revocations.revocation_generation
    {
        return Err(BrokerDenial::Revoked);
    }
    if ticket.scope.generation != revocations.current_generation {
        return Err(BrokerDenial::StaleGeneration);
    }
    if request.audience != ticket.scope.audience || request.view_binding != ticket.scope.view_binding
    {
        return Err(BrokerDenial::OutOfScope);
    }
    if request.requested_bytes == 0 || request.requested_bytes > ticket.scope.max_bytes {
        return Err(BrokerDenial::ByteCeilingExceeded);
    }

    match &ticket.target {
        BrokerTargetKind::Blob {
            destination,
            object_digest,
            max_bytes,
        } => {
            if request.expected_digest != *object_digest {
                return Err(BrokerDenial::DigestMismatch);
            }
            if request.path.as_deref() != Some(destination.as_str()) {
                return Err(BrokerDenial::OutOfScope);
            }
            if !ticket
                .scope
                .ceiling
                .allows(SourceViewOperation::ReadContent, destination)
                || !ticket
                    .scope
                    .allowed_paths
                    .iter()
                    .any(|path| path.covers_path(destination))
            {
                return Err(BrokerDenial::OutOfScope);
            }
            if request.requested_bytes > *max_bytes {
                return Err(BrokerDenial::ByteCeilingExceeded);
            }
        }
        BrokerTargetKind::ManifestView => {
            if request.expected_digest != ticket.manifest_identity {
                return Err(BrokerDenial::DigestMismatch);
            }
            if let Some(path) = &request.path
                && (!ticket
                    .scope
                    .ceiling
                    .allows(SourceViewOperation::ReadContent, path)
                    || !ticket
                        .scope
                        .allowed_paths
                        .iter()
                        .any(|scope| scope.covers_path(path)))
            {
                return Err(BrokerDenial::OutOfScope);
            }
        }
        BrokerTargetKind::FullManifest => {
            if request.expected_digest != ticket.manifest_identity {
                return Err(BrokerDenial::DigestMismatch);
            }
            let root = PathScope::root();
            if !ticket.scope.ceiling.contains(&root)
                || !ticket
                    .scope
                    .allowed_paths
                    .iter()
                    .any(|scope| scope.contains(&root))
            {
                return Err(BrokerDenial::OutOfScope);
            }
        }
    }

    Ok(AdmittedBrokerRead {
        ticket_id: ticket.ticket_id.clone(),
        manifest_identity: ticket.manifest_identity.clone(),
        expected_digest: request.expected_digest.clone(),
        max_bytes: request.requested_bytes,
        target: ticket.target.clone(),
        ceiling: ticket.scope.ceiling.clone(),
        allowed_paths: ticket.scope.allowed_paths.clone(),
    })
}

#[cfg(test)]
mod tests {
    use alloc::{format, vec};

    use super::*;
    use crate::projection::{
        CompatibilityProfile, ComponentVersion, Coverage, DEFINITION_SCHEMA, DependencyDeclaration,
        GeneratedInputs, Mount, ProjectionDefinition, ProjectionFacts, ProjectionProfile,
        RepositoryEnrollment, RepositoryLocator, Selection, SourceCapability, SourceEntry,
        TargetProfile, admit_definition, admit_repositories, resolve,
    };
    use crate::types::ProviderProfileId;

    fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|err| unreachable!("unexpected error: {err:?}"))
    }

    fn digest(label: &str) -> ContentId {
        ok(ContentId::from_blake3_hex(
            blake3::hash(label.as_bytes()).to_hex().to_string(),
        ))
    }

    fn ext(ns: &str, kind: &str, id: &str) -> ExternalRef {
        ok(ExternalRef::new(ns, kind, id))
    }

    fn sample_definition(root: &str, include: Vec<&str>) -> crate::projection::AdmittedDefinition {
        let def = ProjectionDefinition {
            schema: DEFINITION_SCHEMA.into(),
            id: format!("proj-{}", if root.is_empty() { "root" } else { root }),
            profile: ProjectionProfile::Runnable,
            selection: Selection {
                root: root.into(),
                include: include.into_iter().map(String::from).collect(),
                exclude: Vec::new(),
            },
            mounts: vec![Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            }],
            dependencies: Vec::<DependencyDeclaration>::new(),
            creation_destinations: Vec::new(),
            required_capabilities: vec![SourceCapability::ExactTree],
            generated_inputs: GeneratedInputs::Refused,
            target: TargetProfile::POSIX,
            compatibility: Option::<crate::projection::Compatibility>::None,
        };
        let _ = CompatibilityProfile::MonorepoProjectScopeV1;
        ok(admit_definition(&def))
    }

    fn sample_manifest() -> (ResolvedProjectionManifest, Vec<u8>, Vec<ManifestObjectBinding>) {
        let repo = ok(admit_repositories(
            &[RepositoryEnrollment {
                repository_id: "kiln".into(),
                binding: crate::projection::ProviderBinding {
                    source_provider: ok(ProviderProfileId::new("git-source-v1")),
                    native_repository: ext("github", "repository", "OnixResearch/kiln"),
                },
                locators: vec![RepositoryLocator::LocalPath {
                    path: "/srv/kiln".into(),
                }],
            }],
            &[],
        ))
        .pop()
        .unwrap_or_else(|| unreachable!("repo"));

        let def = sample_definition("", vec!["crates/a/**", "crates/b/**"]);
        let obj_a = ext("git", "blob", "blob-a");
        let obj_b = ext("git", "blob", "blob-b");
        let facts = ProjectionFacts {
            repository: repo.identity().clone(),
            exact_revision: ext("git", "commit", "1111111111111111111111111111111111111111"),
            canonical_tree: digest("tree"),
            evaluator: ComponentVersion {
                name: "kiln-eval".into(),
                version: "1.0.0".into(),
            },
            adapter: ComponentVersion {
                name: "git-adapter".into(),
                version: "1.0.0".into(),
            },
            source_capabilities: vec![SourceCapability::ExactTree],
            coverage: Coverage::Complete,
            entries: vec![
                SourceEntry {
                    path: "crates/a/lib.rs".into(),
                    object: obj_a.clone(),
                    kind: crate::projection::EntryKind::File,
                },
                SourceEntry {
                    path: "crates/b/secret.rs".into(),
                    object: obj_b.clone(),
                    kind: crate::projection::EntryKind::File,
                },
            ],
            dependencies: Vec::new(),
            generated: Vec::new(),
        };
        let manifest = ok(resolve(&def, &repo, &facts))
            .require_complete(crate::projection::ProjectionOperation::Build)
            .unwrap_or_else(|e| unreachable!("{:?}", e.diagnostics))
            .manifest()
            .clone();
        let bytes = ok(serde_json::to_vec(&manifest));
        let closure = alloc::vec![
            ManifestObjectBinding {
                object: obj_a,
                digest: digest("shared-bytes"),
                byte_len: 12,
            },
            ManifestObjectBinding {
                object: obj_b,
                digest: digest("shared-bytes"),
                byte_len: 12,
            },
        ];
        (manifest, bytes, closure)
    }

    #[test]
    fn cas_admission_handles_put_if_absent_conflict_and_unknown_state() {
        let (manifest, bytes, closure) = sample_manifest();
        let envelope = ok(ManifestEnvelope::from_manifest(
            &manifest,
            &bytes,
            closure,
            ManifestLifecycleStage::Published,
        ));
        let req = CasWriteRequest {
            tier: StorageTier::S3Secondary,
            condition: CasCondition::PutIfAbsent,
            envelope: envelope.clone(),
            retention_roots: BTreeSet::from([RetentionRoot {
                kind: RetentionRootKind::SealedCandidate,
                root_id: "cand-1".into(),
            }]),
        };

        // Disabled S3 fails closed.
        assert!(admit_cas_write(&req, &TierRecordState::default(), false, 65_536).is_err());

        // Enabled S3 admits PutIfAbsent when Absent.
        let plan = ok(admit_cas_write(&req, &TierRecordState::default(), true, 65_536));
        let decision = apply_cas_write_outcome(
            &plan,
            &TierRecordState::default(),
            BackendWriteOutcome::Unknown {
                reason: "s3 connection reset".into(),
            },
        );
        let PersistenceWriteDecision::Unknown { state, .. } = decision else {
            unreachable!("expected unknown decision");
        };
        assert_eq!(state.presence, TierPresence::Unknown);

        // Cannot perform further CAS writes while state is Unknown.
        assert!(admit_cas_write(&req, &state, true, 65_536).is_err());

        // Verified state refuses PutIfAbsent, accepts matching ExpectedDigest.
        let verified = TierRecordState {
            record_identity: Some(envelope.record_identity.clone()),
            presence: TierPresence::Verified,
            provider_etag: Some("\"etag-1\"".into()),
        };
        assert!(admit_cas_write(&req, &verified, true, 65_536).is_err());
        let cas_update = CasWriteRequest {
            condition: CasCondition::ExpectedDigest {
                expected_record_identity: envelope.record_identity.clone(),
            },
            ..req
        };
        assert!(admit_cas_write(&cas_update, &verified, true, 65_536).is_ok());
    }

    #[test]
    fn eviction_respects_retention_roots_and_secondary_verification() {
        let (manifest, bytes, closure) = sample_manifest();
        let envelope = ok(ManifestEnvelope::from_manifest(
            &manifest,
            &bytes,
            closure,
            ManifestLifecycleStage::Adopted,
        ));
        let policy = EvictionPolicy {
            high_watermark_entries: 1,
            low_watermark_entries: 0,
            high_watermark_bytes: 100,
            low_watermark_bytes: 50,
            require_secondary_before_primary_eviction: true,
        };
        let candidate = EvictionCandidate {
            manifest_identity: envelope.manifest_identity.clone(),
            record_identity: envelope.record_identity.clone(),
            payload_bytes: envelope.payload_bytes,
            placement: TierPlacement {
                molten_primary: TierPresence::Verified,
                s3_secondary: TierPresence::Absent,
                s3_enabled: true,
            },
            retention_roots: BTreeSet::from([RetentionRoot {
                kind: RetentionRootKind::RetainedSnapshot,
                root_id: "snap-1".into(),
            }]),
            age_rank: 1,
        };
        // Refused without verified S3Secondary.
        let req = EvictionRequest {
            target_tier: StorageTier::MoltenPrimary,
            current_entries: 2,
            current_bytes: 200,
            candidate: candidate.clone(),
        };
        assert!(admit_eviction(&policy, &req).is_err());

        // Admitted from MoltenPrimary once S3Secondary is Verified, still refused on S3Secondary.
        let mut with_s3 = req;
        with_s3.candidate.placement.s3_secondary = TierPresence::Verified;
        let plan = ok(admit_eviction(&policy, &with_s3));
        let EvictionDecision::Evicted { placement } = apply_eviction_outcome(
            &plan,
            with_s3.candidate.placement,
            BackendEvictOutcome::Evicted,
        ) else {
            unreachable!("expected evicted");
        };
        assert_eq!(plan_read_tier(&placement), Ok(ReadTierPlan::S3SecondaryReadback));

        with_s3.target_tier = StorageTier::S3Secondary;
        assert!(admit_eviction(&policy, &with_s3).is_err());
    }

    #[test]
    fn broker_ticket_enforces_view_ceiling_digest_expiry_and_revocation() {
        let (manifest, _, _) = sample_manifest();
        let def_a = sample_definition("crates/a", vec!["**"]);
        let ceiling_a = ViewCeiling::from_definition(&def_a);
        let scope_a = BrokerTicketScope {
            repository: manifest.repository.clone(),
            exact_revision: manifest.exact_revision.clone(),
            view_binding: digest("view-a"),
            audience: "did:key:worker-a".into(),
            ceiling: ceiling_a,
            generation: 1,
            revocation_generation: 1,
            allowed_paths: BTreeSet::from([PathScope {
                components: alloc::vec!["crates".into(), "a".into()],
                subtree: true,
            }]),
            max_bytes: 4096,
        };
        // Cannot issue ticket for crates/b/secret.rs under view-a ceiling.
        assert!(
            admit_broker_ticket_issue(
                manifest.identity.clone(),
                BrokerTargetKind::Blob {
                    destination: "crates/b/secret.rs".into(),
                    object_digest: digest("shared-bytes"),
                    max_bytes: 12,
                },
                scope_a.clone(),
                1000,
                1600,
            )
            .is_err()
        );
        // Cannot issue FullManifest ticket under narrowed ceiling.
        assert!(
            admit_broker_ticket_issue(
                manifest.identity.clone(),
                BrokerTargetKind::FullManifest,
                scope_a.clone(),
                1000,
                1600,
            )
            .is_err()
        );

        let ticket = ok(admit_broker_ticket_issue(
            manifest.identity.clone(),
            BrokerTargetKind::Blob {
                destination: "crates/a/lib.rs".into(),
                object_digest: digest("shared-bytes"),
                max_bytes: 12,
            },
            scope_a,
            1000,
            1600,
        ));
        let req = BrokerTicketRequest {
            ticket_id: ticket.ticket_id.clone(),
            audience: "did:key:worker-a".into(),
            view_binding: digest("view-a"),
            expected_digest: digest("shared-bytes"),
            path: Some("crates/a/lib.rs".into()),
            requested_bytes: 12,
        };
        let rev = BrokerRevocationState {
            current_generation: 1,
            revocation_generation: 1,
            ..BrokerRevocationState::default()
        };
        assert!(admit_broker_download(&ticket, &req, &rev, 1200).is_ok());
        assert_eq!(
            admit_broker_download(&ticket, &req, &rev, 1600),
            Err(BrokerDenial::Expired)
        );
        let mut revoked = rev.clone();
        revoked.revoked_tickets.insert(ticket.ticket_id.clone());
        assert_eq!(
            admit_broker_download(&ticket, &req, &revoked, 1200),
            Err(BrokerDenial::Revoked)
        );
    }
}
