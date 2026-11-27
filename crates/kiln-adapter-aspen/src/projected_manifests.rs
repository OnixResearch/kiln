//! Imperative shell for persisting and broker-serving projected source
//! manifests through Molten (`AspenContentPort`) and an optional, off-by-default
//! conditional-write S3 secondary tier (`#80`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use kiln_core::projection::authority::{PathScope, SourceViewOperation};
use kiln_core::projection::manifest_store::{
    AdmittedBrokerRead, BackendEvictOutcome, BackendWriteOutcome, BrokerDenial,
    BrokerDownloadTicket, BrokerRevocationState, BrokerTargetKind, BrokerTicketRequest,
    BrokerTicketScope, CasCondition, CasWriteRequest, EvictionCandidate, EvictionDecision,
    EvictionPolicy, EvictionRequest, MAX_EVICTION_BATCH, MAX_MANIFEST_ENVELOPE_BYTES,
    ManifestEnvelope, ManifestLifecycleStage, ManifestObjectBinding, PersistenceWriteDecision,
    ReadTierPlan, RetentionRoot, StorageTier, TierPlacement, TierPresence, TierRecordState,
    admit_broker_download, admit_broker_ticket_issue, admit_cas_write, admit_eviction,
    apply_cas_write_outcome, apply_eviction_outcome, plan_read_tier, verify_envelope,
};
use kiln_core::projection::{
    EntryOrigin, ExpectedProjection, MANIFEST_SCHEMA, ManifestEntry, ResolvedProjectionManifest,
    verify_manifest,
};
use kiln_core::types::{ContentId, Diagnostic, EffectId, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure};
use serde::{Deserialize, Serialize};

use crate::content::{AspenContentAdapter, AspenContentPort, InMemoryAspenContent, MappedContent};
use crate::identity::KilnArtifactRole;
use crate::profile::AdmittedAspenRuntimeProfile;

pub const CONDITIONAL_OBJECT_STORE_PORT_VERSION: u16 = 1;
pub const MANIFEST_ENVELOPE_ARTIFACT_SCHEMA: &str = "kiln.projected-manifest.envelope.v1";
pub const MANIFEST_OBJECT_ARTIFACT_SCHEMA: &str = "kiln.projected-manifest.object.v1";

// ---------------------------------------------------------------------------
// Optional conditional-write S3 secondary tier port

/// Precondition for conditional S3 object writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConditionalPutPrecondition {
    /// `If-None-Match: *` — succeed only when the key does not exist.
    IfNoneMatch,
    /// `If-Match: <etag>` — succeed only when current ETag matches.
    IfMatch { expected_etag: String },
}

/// Request to conditionally write exact bytes to an S3-compatible store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalPutRequest {
    pub contract_version: u16,
    pub object_key: String,
    pub expected_blake3: ContentId,
    pub precondition: ConditionalPutPrecondition,
    pub bytes: Vec<u8>,
}

/// Outcome of a conditional put on the optional S3 tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionalPutOutcome {
    Committed { etag: String },
    PreconditionFailed { observed_etag: Option<String> },
    /// Remote outcome could not be observed (e.g. connection reset after send).
    Unknown { reason: String },
    Failed { failure: PortFailure },
}

/// Outcome of a bounded read from the optional S3 tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionalReadOutcome {
    Found { bytes: Vec<u8>, etag: Option<String> },
    NotFound,
    Unknown { reason: String },
    Failed { failure: PortFailure },
}

/// Request to conditionally evict one object from the optional S3 tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedEvictRequest {
    pub contract_version: u16,
    pub object_key: String,
    pub expected_etag: Option<String>,
    pub max_evict_batch: usize,
}

/// Outcome of a bounded conditional eviction from the optional S3 tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionalEvictOutcome {
    Evicted,
    PreconditionFailed { observed_etag: Option<String> },
    NotFound,
    Unknown { reason: String },
    Failed { failure: PortFailure },
}

/// Versioned port for the optional conditional-write S3 secondary tier.
///
/// Bucket credentials and raw object keys remain inside the storage owner
/// behind this port and are never surfaced to broker clients.
pub trait ConditionalObjectStorePort {
    fn contract_version(&self) -> u16 {
        CONDITIONAL_OBJECT_STORE_PORT_VERSION
    }

    fn put_conditional(&mut self, request: &ConditionalPutRequest) -> ConditionalPutOutcome;

    fn get_bounded(&self, object_key: &str, maximum_bytes: usize) -> ConditionalReadOutcome;

    fn evict_conditional(&mut self, request: &BoundedEvictRequest) -> ConditionalEvictOutcome;
}

/// Configuration for the optional S3 secondary tier (disabled by default).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct S3TierConfig {
    pub enabled: bool,
    pub contract_version: u16,
    pub endpoint_ref: String,
    pub bucket_ref: String,
    pub key_prefix: String,
    pub max_evict_batch: usize,
}

impl Default for S3TierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            contract_version: CONDITIONAL_OBJECT_STORE_PORT_VERSION,
            endpoint_ref: String::new(),
            bucket_ref: String::new(),
            key_prefix: String::new(),
            max_evict_batch: MAX_EVICTION_BATCH,
        }
    }
}

/// Fail-closed unconfigured S3 port (used when S3 tier is off).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnconfiguredS3Store;

impl ConditionalObjectStorePort for UnconfiguredS3Store {
    fn put_conditional(&mut self, _request: &ConditionalPutRequest) -> ConditionalPutOutcome {
        ConditionalPutOutcome::Failed {
            failure: PortFailure::Domain(Diagnostic::new(
                "s3_tier_unconfigured",
                "s3.port",
                "optional S3 secondary tier is unconfigured",
            )),
        }
    }

    fn get_bounded(&self, _object_key: &str, _maximum_bytes: usize) -> ConditionalReadOutcome {
        ConditionalReadOutcome::Failed {
            failure: PortFailure::Domain(Diagnostic::new(
                "s3_tier_unconfigured",
                "s3.port",
                "optional S3 secondary tier is unconfigured",
            )),
        }
    }

    fn evict_conditional(&mut self, _request: &BoundedEvictRequest) -> ConditionalEvictOutcome {
        ConditionalEvictOutcome::Failed {
            failure: PortFailure::Domain(Diagnostic::new(
                "s3_tier_unconfigured",
                "s3.port",
                "optional S3 secondary tier is unconfigured",
            )),
        }
    }
}

/// Hot-tier eviction capability for `AspenContentPort` backends.
pub trait MoltenEvictPort: AspenContentPort {
    fn evict_exact(&mut self, reference: &ExternalRef) -> Result<(), PortFailure>;
}

impl MoltenEvictPort for InMemoryAspenContent {
    fn evict_exact(&mut self, reference: &ExternalRef) -> Result<(), PortFailure> {
        self.remove(reference)
    }
}

/// Fault injection modes for [`InMemoryConditionalObjectStore`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum S3Fault {
    None,
    UnknownAfterPut,
    UnknownAfterEvict,
    FailedBeforeWrite,
    CorruptNextRead,
}

#[derive(Clone, Debug)]
struct InMemoryS3Object {
    bytes: Vec<u8>,
    etag: String,
}

/// In-memory test/fixture implementation of [`ConditionalObjectStorePort`]
/// supporting conditional `If-None-Match` / `If-Match` semantics and fault
/// injection.
#[derive(Clone, Debug)]
pub struct InMemoryConditionalObjectStore {
    objects: Arc<Mutex<BTreeMap<String, InMemoryS3Object>>>,
    next_fault: Arc<Mutex<S3Fault>>,
    etag_seq: Arc<Mutex<u64>>,
}

impl Default for InMemoryConditionalObjectStore {
    fn default() -> Self {
        Self {
            objects: Arc::new(Mutex::new(BTreeMap::new())),
            next_fault: Arc::new(Mutex::new(S3Fault::None)),
            etag_seq: Arc::new(Mutex::new(1)),
        }
    }
}

impl InMemoryConditionalObjectStore {
    pub fn set_next_fault(&self, fault: S3Fault) -> Result<(), PortFailure> {
        let mut slot = self.next_fault.lock().map_err(|_| lock_failure())?;
        *slot = fault;
        Ok(())
    }

    /// Test-only helper to simulate silent backend bitrot/tampering.
    pub fn replace_unchecked(&self, key: &str, bytes: Vec<u8>) -> Result<(), PortFailure> {
        let mut objects = self.objects.lock().map_err(|_| lock_failure())?;
        if let Some(entry) = objects.get_mut(key) {
            entry.bytes = bytes;
        }
        Ok(())
    }

    pub fn contains_key(&self, key: &str) -> Result<bool, PortFailure> {
        let objects = self.objects.lock().map_err(|_| lock_failure())?;
        Ok(objects.contains_key(key))
    }

    fn take_fault(&self) -> Result<S3Fault, PortFailure> {
        let mut slot = self.next_fault.lock().map_err(|_| lock_failure())?;
        let current = *slot;
        *slot = S3Fault::None;
        Ok(current)
    }

    fn next_etag(&self, bytes: &[u8]) -> Result<String, PortFailure> {
        let mut seq = self.etag_seq.lock().map_err(|_| lock_failure())?;
        let current = *seq;
        *seq = current.saturating_add(1);
        let hash = blake3::hash(bytes).to_hex();
        let prefix = hash.get(..16).unwrap_or("0000000000000000");
        Ok(format!("\"{prefix}-{current}\""))
    }
}

impl ConditionalObjectStorePort for InMemoryConditionalObjectStore {
    fn put_conditional(&mut self, request: &ConditionalPutRequest) -> ConditionalPutOutcome {
        if request.contract_version != CONDITIONAL_OBJECT_STORE_PORT_VERSION {
            return ConditionalPutOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "unsupported_s3_port_version",
                    "s3.contract_version",
                    "unsupported ConditionalObjectStorePort contract version",
                )),
            };
        }
        if request.object_key.trim().is_empty() {
            return ConditionalPutOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "invalid_s3_object_key",
                    "s3.object_key",
                    "S3 object key must be non-empty",
                )),
            };
        }
        if let Err(diag) = request.expected_blake3.validate() {
            return ConditionalPutOutcome::Failed {
                failure: PortFailure::Domain(diag),
            };
        }
        let observed = blake3::hash(&request.bytes).to_hex().to_string();
        if observed != request.expected_blake3.as_str() {
            return ConditionalPutOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "s3_put_digest_mismatch",
                    "s3.expected_blake3",
                    "S3 upload bytes do not match expected BLAKE3 digest",
                )),
            };
        }

        let fault = match self.take_fault() {
            Ok(fault) => fault,
            Err(failure) => return ConditionalPutOutcome::Failed { failure },
        };
        if fault == S3Fault::FailedBeforeWrite {
            return ConditionalPutOutcome::Failed {
                failure: PortFailure::Infrastructure(infrastructure(
                    "s3_transport_failed",
                    "S3 put failed before commit",
                )),
            };
        }

        let mut objects = match self.objects.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return ConditionalPutOutcome::Failed {
                    failure: lock_failure(),
                };
            }
        };

        match &request.precondition {
            ConditionalPutPrecondition::IfNoneMatch => {
                if let Some(existing) = objects.get(&request.object_key) {
                    return ConditionalPutOutcome::PreconditionFailed {
                        observed_etag: Some(existing.etag.clone()),
                    };
                }
            }
            ConditionalPutPrecondition::IfMatch { expected_etag } => {
                match objects.get(&request.object_key) {
                    Some(existing) if existing.etag == *expected_etag => {}
                    Some(existing) => {
                        return ConditionalPutOutcome::PreconditionFailed {
                            observed_etag: Some(existing.etag.clone()),
                        };
                    }
                    None => {
                        return ConditionalPutOutcome::PreconditionFailed {
                            observed_etag: None,
                        };
                    }
                }
            }
        }

        let etag = match self.next_etag(&request.bytes) {
            Ok(etag) => etag,
            Err(failure) => return ConditionalPutOutcome::Failed { failure },
        };
        objects.insert(
            request.object_key.clone(),
            InMemoryS3Object {
                bytes: request.bytes.clone(),
                etag: etag.clone(),
            },
        );
        if fault == S3Fault::UnknownAfterPut {
            return ConditionalPutOutcome::Unknown {
                reason: "S3 conditional put response lost after remote transmission".to_string(),
            };
        }
        ConditionalPutOutcome::Committed { etag }
    }

    fn get_bounded(&self, object_key: &str, maximum_bytes: usize) -> ConditionalReadOutcome {
        if maximum_bytes == 0 {
            return ConditionalReadOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "s3_read_zero_bound",
                    "s3.maximum_bytes",
                    "S3 bounded read limit must be nonzero",
                )),
            };
        }
        let fault = match self.take_fault() {
            Ok(fault) => fault,
            Err(failure) => return ConditionalReadOutcome::Failed { failure },
        };
        let objects = match self.objects.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return ConditionalReadOutcome::Failed {
                    failure: lock_failure(),
                };
            }
        };
        let Some(entry) = objects.get(object_key) else {
            return ConditionalReadOutcome::NotFound;
        };
        if entry.bytes.len() > maximum_bytes {
            return ConditionalReadOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "s3_read_bound_exceeded",
                    "s3.maximum_bytes",
                    "S3 object exceeds bounded read limit",
                )),
            };
        }
        let mut bytes = entry.bytes.clone();
        if fault == S3Fault::CorruptNextRead {
            if let Some(first) = bytes.first_mut() {
                *first ^= 0xff;
            } else {
                bytes.push(0xff);
            }
        }
        ConditionalReadOutcome::Found {
            bytes,
            etag: Some(entry.etag.clone()),
        }
    }

    fn evict_conditional(&mut self, request: &BoundedEvictRequest) -> ConditionalEvictOutcome {
        if request.contract_version != CONDITIONAL_OBJECT_STORE_PORT_VERSION {
            return ConditionalEvictOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "unsupported_s3_port_version",
                    "s3.contract_version",
                    "unsupported ConditionalObjectStorePort contract version",
                )),
            };
        }
        if request.max_evict_batch == 0 || request.max_evict_batch > MAX_EVICTION_BATCH {
            return ConditionalEvictOutcome::Failed {
                failure: PortFailure::Domain(Diagnostic::new(
                    "invalid_evict_batch",
                    "s3.max_evict_batch",
                    "evict batch bound must be positive and within MAX_EVICTION_BATCH",
                )),
            };
        }
        let fault = match self.take_fault() {
            Ok(fault) => fault,
            Err(failure) => return ConditionalEvictOutcome::Failed { failure },
        };
        let mut objects = match self.objects.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return ConditionalEvictOutcome::Failed {
                    failure: lock_failure(),
                };
            }
        };
        let Some(existing) = objects.get(&request.object_key) else {
            return ConditionalEvictOutcome::NotFound;
        };
        if let Some(expected_etag) = &request.expected_etag
            && existing.etag != *expected_etag
        {
            return ConditionalEvictOutcome::PreconditionFailed {
                observed_etag: Some(existing.etag.clone()),
            };
        }
        objects.remove(&request.object_key);
        if fault == S3Fault::UnknownAfterEvict {
            return ConditionalEvictOutcome::Unknown {
                reason: "S3 conditional delete acknowledgement lost".to_string(),
            };
        }
        ConditionalEvictOutcome::Evicted
    }
}

// ---------------------------------------------------------------------------
// Projected manifest persistence store (Molten primary + optional S3 tier)

#[derive(Clone, Debug)]
struct StoredBlob {
    digest: ContentId,
    byte_len: usize,
    molten_mapped: Option<MappedContent>,
    s3_key: Option<String>,
}

#[derive(Clone, Debug)]
struct StoredManifestRecord {
    envelope: ManifestEnvelope,
    molten_manifest: Option<MappedContent>,
    molten_envelope: Option<MappedContent>,
    molten_state: TierRecordState,
    s3_state: TierRecordState,
    s3_root_key: Option<String>,
    s3_manifest_key: Option<String>,
    retention_roots: BTreeSet<RetentionRoot>,
    age_rank: u64,
}

impl StoredManifestRecord {
    fn placement(&self, s3_enabled: bool) -> TierPlacement {
        TierPlacement {
            molten_primary: self.molten_state.presence,
            s3_secondary: self.s3_state.presence,
            s3_enabled,
        }
    }
}

/// Outcome of persisting a projected manifest across tiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistManifestOutcome {
    Persisted {
        record_identity: ContentId,
        placement: TierPlacement,
    },
    /// Remote tier write outcome was ambiguous and is preserved explicitly.
    Unknown {
        record_identity: ContentId,
        placement: TierPlacement,
        diagnostic: Diagnostic,
    },
}

/// Result of reloading and verifying a projected manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedProjectedManifest {
    pub manifest: ResolvedProjectionManifest,
    pub envelope: ManifestEnvelope,
    pub served_from: StorageTier,
}

/// Explicit, fail-closed error surface for projected manifest storage and
/// brokered reads. Never exposes raw S3 bucket credentials or object keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestStoreFailure {
    Domain(Diagnostic),
    BrokerDenied(BrokerDenial),
    CasConflict(Diagnostic),
    EvictedWithoutSecondary(Diagnostic),
    UnknownOutcome(Diagnostic),
    IntegrityMismatch(Diagnostic),
    NotFound(Diagnostic),
    Infrastructure(PortFailure),
}

impl From<Diagnostic> for ManifestStoreFailure {
    fn from(diag: Diagnostic) -> Self {
        Self::Domain(diag)
    }
}

impl From<PortFailure> for ManifestStoreFailure {
    fn from(failure: PortFailure) -> Self {
        match failure {
            PortFailure::Unknown(infra) => Self::UnknownOutcome(Diagnostic::new(
                &infra.code,
                "storage.outcome",
                &infra.message,
            )),
            other => Self::Infrastructure(other),
        }
    }
}

/// Imperative shell persisting projected source manifests through
/// [`AspenContentPort`] (Molten primary) and an optional
/// [`ConditionalObjectStorePort`] (off-by-default S3 secondary tier).
#[derive(Clone, Debug)]
pub struct ProjectedManifestStore<P, S = UnconfiguredS3Store> {
    content: AspenContentAdapter<P>,
    s3: Option<S>,
    s3_config: S3TierConfig,
    eviction_policy: EvictionPolicy,
    records: BTreeMap<ContentId, StoredManifestRecord>,
    blobs: BTreeMap<ContentId, StoredBlob>,
    next_age_rank: u64,
}

impl<P: AspenContentPort> ProjectedManifestStore<P, UnconfiguredS3Store> {
    /// Construct a store with Molten primary storage only (S3 secondary off).
    pub fn new_local_only(
        admitted_profile: AdmittedAspenRuntimeProfile,
        port: P,
        eviction_policy: EvictionPolicy,
    ) -> Result<Self, ManifestStoreFailure> {
        eviction_policy.validate()?;
        Ok(Self {
            content: AspenContentAdapter::new(admitted_profile, port),
            s3: None,
            s3_config: S3TierConfig::default(),
            eviction_policy,
            records: BTreeMap::new(),
            blobs: BTreeMap::new(),
            next_age_rank: 1,
        })
    }
}

impl<P: AspenContentPort, S: ConditionalObjectStorePort> ProjectedManifestStore<P, S> {
    /// Construct a store with Molten primary and an explicit optional S3 tier.
    pub fn with_optional_s3(
        admitted_profile: AdmittedAspenRuntimeProfile,
        port: P,
        s3: Option<S>,
        s3_config: S3TierConfig,
        eviction_policy: EvictionPolicy,
    ) -> Result<Self, ManifestStoreFailure> {
        eviction_policy.validate()?;
        if s3_config.enabled && s3.is_none() {
            return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                "s3_tier_missing_port",
                "s3.port",
                "S3 tier is marked enabled but no ConditionalObjectStorePort was provided",
            )));
        }
        if s3_config.max_evict_batch == 0 || s3_config.max_evict_batch > MAX_EVICTION_BATCH {
            return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                "invalid_evict_batch",
                "s3.max_evict_batch",
                "S3 max_evict_batch must be positive and within MAX_EVICTION_BATCH",
            )));
        }
        Ok(Self {
            content: AspenContentAdapter::new(admitted_profile, port),
            s3,
            s3_config,
            eviction_policy,
            records: BTreeMap::new(),
            blobs: BTreeMap::new(),
            next_age_rank: 1,
        })
    }

    #[must_use]
    pub const fn s3_enabled(&self) -> bool {
        self.s3_config.enabled && self.s3.is_some()
    }

    #[must_use]
    pub const fn content_adapter(&self) -> &AspenContentAdapter<P> {
        &self.content
    }

    #[must_use]
    pub fn s3_port(&self) -> Option<&S> {
        self.s3.as_ref()
    }

    #[must_use]
    pub fn placement(&self, manifest_identity: &ContentId) -> Option<TierPlacement> {
        self.records
            .get(manifest_identity)
            .map(|record| record.placement(self.s3_enabled()))
    }

    #[must_use]
    pub fn record_state(
        &self,
        manifest_identity: &ContentId,
        tier: StorageTier,
    ) -> Option<&TierRecordState> {
        self.records.get(manifest_identity).map(|record| match tier {
            StorageTier::MoltenPrimary => &record.molten_state,
            StorageTier::S3Secondary => &record.s3_state,
        })
    }

    /// Persist a resolved projected source manifest and its complete required
    /// native object closure to Molten primary, and optionally to S3 secondary.
    #[allow(clippy::too_many_arguments)]
    pub fn persist_manifest(
        &mut self,
        manifest: &ResolvedProjectionManifest,
        object_bytes: &BTreeMap<ExternalRef, Vec<u8>>,
        stage: ManifestLifecycleStage,
        condition: CasCondition,
        retention_roots: BTreeSet<RetentionRoot>,
        replicate_to_s3: bool,
        producer_effect: Option<EffectId>,
    ) -> Result<PersistManifestOutcome, ManifestStoreFailure> {
        if replicate_to_s3 && !self.s3_enabled() {
            return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                "s3_tier_disabled",
                "s3.enabled",
                "cannot replicate to S3 secondary tier when S3 is disabled or unconfigured",
            )));
        }

        let expected = ExpectedProjection {
            repository: &manifest.repository,
            exact_revision: &manifest.exact_revision,
            definition: &manifest.definition,
            evaluator: &manifest.evaluator,
            adapter: &manifest.adapter,
        };
        verify_manifest(manifest, expected).map_err(|mut diagnostics| {
            ManifestStoreFailure::Domain(diagnostics.drain(..).next().unwrap_or_else(|| {
                Diagnostic::new("invalid_manifest", "manifest", "manifest failed verification")
            }))
        })?;

        // Ensure complete required object closure is supplied before any root publication.
        let mut required_objects: BTreeSet<ExternalRef> = BTreeSet::new();
        for entry in &manifest.entries {
            if let EntryOrigin::Native { object, .. } = &entry.origin {
                if !object_bytes.contains_key(object) {
                    return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                        "incomplete_object_closure",
                        "manifest.entries",
                        "missing required source object bytes for manifest closure",
                    )));
                }
                required_objects.insert(object.clone());
            }
        }

        // Persist required native objects to MoltenPrimary (and S3 if requested) first,
        // so no partial root is ever exposed.
        let mut closure_bindings = Vec::with_capacity(required_objects.len());
        let mut s3_blob_uploads: Vec<(ContentId, String, Vec<u8>)> = Vec::new();
        for object in &required_objects {
            let bytes = object_bytes.get(object).ok_or_else(|| {
                ManifestStoreFailure::Domain(Diagnostic::new(
                    "incomplete_object_closure",
                    "manifest.entries",
                    "missing required source object bytes",
                ))
            })?;
            let mapped = self.content.store(
                KilnArtifactRole::InputRoot,
                MANIFEST_OBJECT_ARTIFACT_SCHEMA,
                bytes,
                producer_effect.clone(),
            )?;
            let digest = ContentId::from_blake3_hex(mapped.mapping.kiln.digest.clone())?;
            let s3_key = if replicate_to_s3 {
                let key = self.private_blob_key(&digest);
                s3_blob_uploads.push((digest.clone(), key.clone(), bytes.clone()));
                Some(key)
            } else {
                None
            };
            self.blobs
                .entry(digest.clone())
                .and_modify(|existing| {
                    existing.molten_mapped = Some(mapped.clone());
                    if s3_key.is_some() {
                        existing.s3_key = s3_key.clone();
                    }
                })
                .or_insert(StoredBlob {
                    digest: digest.clone(),
                    byte_len: bytes.len(),
                    molten_mapped: Some(mapped),
                    s3_key,
                });
            closure_bindings.push(ManifestObjectBinding {
                object: object.clone(),
                digest,
                byte_len: bytes.len(),
            });
        }

        let manifest_bytes = serde_json::to_vec(manifest).map_err(|_| {
            ManifestStoreFailure::Domain(Diagnostic::new(
                "manifest_encode_failed",
                "manifest",
                "failed to serialize projected manifest",
            ))
        })?;
        let envelope =
            ManifestEnvelope::from_manifest(manifest, &manifest_bytes, closure_bindings, stage)?;
        let envelope_bytes = serde_json::to_vec(&envelope).map_err(|_| {
            ManifestStoreFailure::Domain(Diagnostic::new(
                "envelope_encode_failed",
                "envelope",
                "failed to serialize manifest envelope",
            ))
        })?;

        let existing = self.records.get(&manifest.identity).cloned();
        let prior_primary = existing
            .as_ref()
            .map(|r| r.molten_state.clone())
            .unwrap_or_default();
        let prior_s3 = existing
            .as_ref()
            .map(|r| r.s3_state.clone())
            .unwrap_or_default();

        // Admit primary CAS write before mutating Molten storage.
        let primary_plan = admit_cas_write(
            &CasWriteRequest {
                tier: StorageTier::MoltenPrimary,
                condition: condition.clone(),
                envelope: envelope.clone(),
                retention_roots: retention_roots.clone(),
            },
            &prior_primary,
            self.s3_enabled(),
            MAX_MANIFEST_ENVELOPE_BYTES,
        )
        .map_err(|diag| {
            if diag.code == "cas_conflict" {
                ManifestStoreFailure::CasConflict(diag)
            } else {
                ManifestStoreFailure::Domain(diag)
            }
        })?;

        // Admit S3 CAS write upfront too (if replicating) so conflict refuses before side effects.
        let s3_plan = if replicate_to_s3 {
            Some(
                admit_cas_write(
                    &CasWriteRequest {
                        tier: StorageTier::S3Secondary,
                        condition,
                        envelope: envelope.clone(),
                        retention_roots: retention_roots.clone(),
                    },
                    &prior_s3,
                    self.s3_enabled(),
                    MAX_MANIFEST_ENVELOPE_BYTES,
                )
                .map_err(|diag| {
                    if diag.code == "cas_conflict" {
                        ManifestStoreFailure::CasConflict(diag)
                    } else {
                        ManifestStoreFailure::Domain(diag)
                    }
                })?,
            )
        } else {
            None
        };

        let mapped_manifest = self.content.store(
            KilnArtifactRole::SourceManifest,
            MANIFEST_SCHEMA,
            &manifest_bytes,
            producer_effect.clone(),
        )?;
        let mapped_envelope = self.content.store(
            KilnArtifactRole::SourceManifest,
            MANIFEST_ENVELOPE_ARTIFACT_SCHEMA,
            &envelope_bytes,
            producer_effect,
        )?;
        let primary_decision = apply_cas_write_outcome(
            &primary_plan,
            &prior_primary,
            BackendWriteOutcome::Committed {
                provider_etag: Some(mapped_envelope.reference.native_id.as_str().to_string()),
            },
        );
        let molten_state = match primary_decision {
            PersistenceWriteDecision::Committed { state } => state,
            PersistenceWriteDecision::Conflict { diagnostic, .. } => {
                return Err(ManifestStoreFailure::CasConflict(diagnostic));
            }
            PersistenceWriteDecision::Unknown { diagnostic, .. } => {
                return Err(ManifestStoreFailure::UnknownOutcome(diagnostic));
            }
            PersistenceWriteDecision::Refused { diagnostic, .. } => {
                return Err(ManifestStoreFailure::Domain(diagnostic));
            }
        };

        let mut s3_state = prior_s3.clone();
        let mut s3_root_key = existing.as_ref().and_then(|r| r.s3_root_key.clone());
        let mut s3_manifest_key = existing.as_ref().and_then(|r| r.s3_manifest_key.clone());
        let mut unknown_diag: Option<Diagnostic> = None;

        if let Some(plan) = s3_plan {
            let blob_outcome = self.upload_s3_blobs(&s3_blob_uploads)?;
            if let Some(diag) = blob_outcome {
                s3_state.presence = TierPresence::Unknown;
                unknown_diag = Some(diag);
            } else {
                let manifest_key = self.private_manifest_payload_key(&envelope.payload_digest);
                let manifest_put = self.s3_put_object(
                    &manifest_key,
                    &envelope.payload_digest,
                    ConditionalPutPrecondition::IfNoneMatch,
                    &manifest_bytes,
                )?;
                if let Some(diag) = manifest_put {
                    s3_state.presence = TierPresence::Unknown;
                    unknown_diag = Some(diag);
                } else {
                    let root_key = self.private_manifest_root_key(&manifest.identity);
                    let root_digest = ContentId::from_blake3_hex(
                        blake3::hash(&envelope_bytes).to_hex().to_string(),
                    )?;
                    let precondition = match &plan.condition {
                        CasCondition::PutIfAbsent => ConditionalPutPrecondition::IfNoneMatch,
                        CasCondition::ExpectedDigest { .. } => {
                            let expected_etag =
                                plan.expected_provider_etag.clone().ok_or_else(|| {
                                    ManifestStoreFailure::CasConflict(Diagnostic::new(
                                        "s3_missing_etag",
                                        "cas.expected_etag",
                                        "conditional S3 update requires prior ETag",
                                    ))
                                })?;
                            ConditionalPutPrecondition::IfMatch { expected_etag }
                        }
                    };
                    let s3 = self.s3.as_mut().ok_or_else(|| {
                        ManifestStoreFailure::Domain(Diagnostic::new(
                            "s3_tier_unconfigured",
                            "s3.port",
                            "optional S3 secondary tier is unconfigured",
                        ))
                    })?;
                    let raw_outcome = s3.put_conditional(&ConditionalPutRequest {
                        contract_version: self.s3_config.contract_version,
                        object_key: root_key.clone(),
                        expected_blake3: root_digest,
                        precondition,
                        bytes: envelope_bytes,
                    });
                    let backend_outcome = map_s3_put_outcome(raw_outcome)?;
                    match apply_cas_write_outcome(&plan, &prior_s3, backend_outcome) {
                        PersistenceWriteDecision::Committed { state } => {
                            s3_state = state;
                            s3_root_key = Some(root_key);
                            s3_manifest_key = Some(manifest_key);
                        }
                        PersistenceWriteDecision::Conflict { diagnostic, .. } => {
                            return Err(ManifestStoreFailure::CasConflict(diagnostic));
                        }
                        PersistenceWriteDecision::Unknown { state, diagnostic } => {
                            s3_state = state;
                            s3_root_key = Some(root_key);
                            s3_manifest_key = Some(manifest_key);
                            unknown_diag = Some(diagnostic);
                        }
                        PersistenceWriteDecision::Refused { diagnostic, .. } => {
                            return Err(ManifestStoreFailure::Domain(diagnostic));
                        }
                    }
                }
            }
        }

        let age_rank = existing
            .as_ref()
            .map(|r| r.age_rank)
            .unwrap_or_else(|| self.take_age_rank());
        let record = StoredManifestRecord {
            envelope: envelope.clone(),
            molten_manifest: Some(mapped_manifest),
            molten_envelope: Some(mapped_envelope),
            molten_state,
            s3_state,
            s3_root_key,
            s3_manifest_key,
            retention_roots,
            age_rank,
        };
        let placement = record.placement(self.s3_enabled());
        self.records.insert(manifest.identity.clone(), record);

        if let Some(diagnostic) = unknown_diag {
            Ok(PersistManifestOutcome::Unknown {
                record_identity: envelope.record_identity,
                placement,
                diagnostic,
            })
        } else {
            Ok(PersistManifestOutcome::Persisted {
                record_identity: envelope.record_identity,
                placement,
            })
        }
    }

    /// Reload a persisted manifest from Molten primary or S3 secondary readback
    /// with full BLAKE3 envelope and manifest verification.
    pub fn load_manifest(
        &self,
        manifest_identity: &ContentId,
        expected: Option<ExpectedProjection<'_>>,
        maximum_bytes: usize,
    ) -> Result<LoadedProjectedManifest, ManifestStoreFailure> {
        let record = self.records.get(manifest_identity).ok_or_else(|| {
            ManifestStoreFailure::NotFound(Diagnostic::new(
                "manifest_not_found",
                "manifest_identity",
                "projected manifest is not registered in store",
            ))
        })?;
        let tier = plan_read_tier(&record.placement(self.s3_enabled())).map_err(|diag| {
            match diag.code.as_str() {
                "manifest_evicted_no_secondary" => {
                    ManifestStoreFailure::EvictedWithoutSecondary(diag)
                }
                "manifest_tier_unknown" => ManifestStoreFailure::UnknownOutcome(diag),
                _ => ManifestStoreFailure::NotFound(diag),
            }
        })?;

        match tier {
            ReadTierPlan::MoltenPrimary => {
                let mapped_manifest = record.molten_manifest.as_ref().ok_or_else(|| {
                    ManifestStoreFailure::NotFound(Diagnostic::new(
                        "molten_manifest_missing",
                        "molten.manifest",
                        "MoltenPrimary manifest mapping missing",
                    ))
                })?;
                let mapped_envelope = record.molten_envelope.as_ref().ok_or_else(|| {
                    ManifestStoreFailure::NotFound(Diagnostic::new(
                        "molten_envelope_missing",
                        "molten.envelope",
                        "MoltenPrimary envelope mapping missing",
                    ))
                })?;
                let manifest_bytes = self
                    .content
                    .load(mapped_manifest, maximum_bytes)
                    .map_err(map_content_read_failure)?;
                let envelope_bytes = self
                    .content
                    .load(mapped_envelope, maximum_bytes)
                    .map_err(map_content_read_failure)?;
                let envelope: ManifestEnvelope =
                    serde_json::from_slice(&envelope_bytes).map_err(|_| {
                        ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                            "envelope_decode_failed",
                            "envelope",
                            "stored manifest envelope failed decoding",
                        ))
                    })?;
                if Some(&envelope.record_identity) != record.molten_state.record_identity.as_ref() {
                    return Err(ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                        "record_identity_drift",
                        "envelope.record_identity",
                        "reloaded envelope record_identity differs from committed record",
                    )));
                }
                let manifest: ResolvedProjectionManifest =
                    serde_json::from_slice(&manifest_bytes).map_err(|_| {
                        ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                            "manifest_decode_failed",
                            "manifest",
                            "stored manifest failed decoding",
                        ))
                    })?;
                verify_envelope(&envelope, &manifest, &manifest_bytes, expected)
                    .map_err(ManifestStoreFailure::IntegrityMismatch)?;
                Ok(LoadedProjectedManifest {
                    manifest,
                    envelope,
                    served_from: StorageTier::MoltenPrimary,
                })
            }
            ReadTierPlan::S3SecondaryReadback => {
                let s3 = self.s3.as_ref().ok_or_else(|| {
                    ManifestStoreFailure::Domain(Diagnostic::new(
                        "s3_tier_unconfigured",
                        "s3.port",
                        "optional S3 secondary tier is unconfigured",
                    ))
                })?;
                let root_key = record.s3_root_key.as_ref().ok_or_else(|| {
                    ManifestStoreFailure::NotFound(Diagnostic::new(
                        "s3_root_key_missing",
                        "s3.root_key",
                        "S3 root object key missing for manifest",
                    ))
                })?;
                let manifest_key = record.s3_manifest_key.as_ref().ok_or_else(|| {
                    ManifestStoreFailure::NotFound(Diagnostic::new(
                        "s3_manifest_key_missing",
                        "s3.manifest_key",
                        "S3 manifest payload key missing",
                    ))
                })?;
                let envelope_bytes = read_s3_exact(s3, root_key, maximum_bytes)?;
                let envelope: ManifestEnvelope =
                    serde_json::from_slice(&envelope_bytes).map_err(|_| {
                        ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                            "envelope_decode_failed",
                            "envelope",
                            "S3 manifest envelope failed decoding",
                        ))
                    })?;
                if Some(&envelope.record_identity) != record.s3_state.record_identity.as_ref() {
                    return Err(ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                        "record_identity_drift",
                        "envelope.record_identity",
                        "S3 envelope record_identity differs from committed record",
                    )));
                }
                let manifest_bytes = read_s3_exact(s3, manifest_key, maximum_bytes)?;
                let manifest: ResolvedProjectionManifest =
                    serde_json::from_slice(&manifest_bytes).map_err(|_| {
                        ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                            "manifest_decode_failed",
                            "manifest",
                            "S3 manifest payload failed decoding",
                        ))
                    })?;
                verify_envelope(&envelope, &manifest, &manifest_bytes, expected)
                    .map_err(ManifestStoreFailure::IntegrityMismatch)?;
                Ok(LoadedProjectedManifest {
                    manifest,
                    envelope,
                    served_from: StorageTier::S3Secondary,
                })
            }
        }
    }

    /// Read and BLAKE3-verify one exact object blob from Molten primary or S3
    /// secondary readback.
    pub fn load_blob_verified(
        &self,
        expected_digest: &ContentId,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, ManifestStoreFailure> {
        let blob = self.blobs.get(expected_digest).ok_or_else(|| {
            ManifestStoreFailure::NotFound(Diagnostic::new(
                "blob_not_found",
                "blob.digest",
                "object blob is not registered in store",
            ))
        })?;
        let bytes = if let Some(mapped) = &blob.molten_mapped {
            match self.content.load(mapped, maximum_bytes) {
                Ok(bytes) => bytes,
                Err(PortFailure::Infrastructure(infra))
                    if infra.code == "aspen_content_missing"
                        && self.s3_enabled()
                        && blob.s3_key.is_some() =>
                {
                    let s3 = self.s3.as_ref().ok_or_else(|| {
                        ManifestStoreFailure::Domain(Diagnostic::new(
                            "s3_tier_unconfigured",
                            "s3.port",
                            "S3 port unavailable",
                        ))
                    })?;
                    let key = blob.s3_key.as_ref().ok_or_else(|| {
                        ManifestStoreFailure::NotFound(Diagnostic::new(
                            "s3_blob_key_missing",
                            "s3.key",
                            "S3 blob key missing",
                        ))
                    })?;
                    read_s3_exact(s3, key, maximum_bytes)?
                }
                Err(other) => return Err(map_content_read_failure(other)),
            }
        } else if self.s3_enabled()
            && let Some(key) = &blob.s3_key
        {
            let s3 = self.s3.as_ref().ok_or_else(|| {
                ManifestStoreFailure::Domain(Diagnostic::new(
                    "s3_tier_unconfigured",
                    "s3.port",
                    "S3 port unavailable",
                ))
            })?;
            read_s3_exact(s3, key, maximum_bytes)?
        } else {
            return Err(ManifestStoreFailure::EvictedWithoutSecondary(
                Diagnostic::new(
                    "blob_evicted_no_secondary",
                    "blob",
                    "object blob unavailable on primary and no secondary copy exists",
                ),
            ));
        };

        let observed = blake3::hash(&bytes).to_hex().to_string();
        if observed != blob.digest.as_str() || bytes.len() != blob.byte_len {
            return Err(ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                "blob_digest_mismatch",
                "blob.digest",
                "loaded object bytes fail exact BLAKE3 verification",
            )));
        }
        Ok(bytes)
    }

    pub fn update_retention_roots(
        &mut self,
        manifest_identity: &ContentId,
        retention_roots: BTreeSet<RetentionRoot>,
    ) -> Result<(), ManifestStoreFailure> {
        for root in &retention_roots {
            root.validate()?;
        }
        let record = self.records.get_mut(manifest_identity).ok_or_else(|| {
            ManifestStoreFailure::NotFound(Diagnostic::new(
                "manifest_not_found",
                "manifest_identity",
                "manifest not found",
            ))
        })?;
        record.retention_roots = retention_roots;
        Ok(())
    }

    fn take_age_rank(&mut self) -> u64 {
        let current = self.next_age_rank;
        self.next_age_rank = current.saturating_add(1);
        current
    }

    fn private_manifest_root_key(&self, manifest_identity: &ContentId) -> String {
        let prefix = self.s3_config.key_prefix.trim_matches('/');
        if prefix.is_empty() {
            format!("roots/{}.json", manifest_identity.as_str())
        } else {
            format!("{prefix}/roots/{}.json", manifest_identity.as_str())
        }
    }

    fn private_manifest_payload_key(&self, payload_digest: &ContentId) -> String {
        let prefix = self.s3_config.key_prefix.trim_matches('/');
        if prefix.is_empty() {
            format!("manifests/{}.json", payload_digest.as_str())
        } else {
            format!("{prefix}/manifests/{}.json", payload_digest.as_str())
        }
    }

    fn private_blob_key(&self, digest: &ContentId) -> String {
        let prefix = self.s3_config.key_prefix.trim_matches('/');
        if prefix.is_empty() {
            format!("blobs/{}.bin", digest.as_str())
        } else {
            format!("{prefix}/blobs/{}.bin", digest.as_str())
        }
    }

    fn upload_s3_blobs(
        &mut self,
        uploads: &[(ContentId, String, Vec<u8>)],
    ) -> Result<Option<Diagnostic>, ManifestStoreFailure> {
        for (digest, key, bytes) in uploads {
            if let Some(diag) = self.s3_put_object(
                key,
                digest,
                ConditionalPutPrecondition::IfNoneMatch,
                bytes,
            )? {
                return Ok(Some(diag));
            }
        }
        Ok(None)
    }

    fn s3_put_object(
        &mut self,
        object_key: &str,
        expected_blake3: &ContentId,
        precondition: ConditionalPutPrecondition,
        bytes: &[u8],
    ) -> Result<Option<Diagnostic>, ManifestStoreFailure> {
        let contract_version = self.s3_config.contract_version;
        let s3 = self.s3.as_mut().ok_or_else(|| {
            ManifestStoreFailure::Domain(Diagnostic::new(
                "s3_tier_unconfigured",
                "s3.port",
                "optional S3 secondary tier is unconfigured",
            ))
        })?;
        match s3.put_conditional(&ConditionalPutRequest {
            contract_version,
            object_key: object_key.to_string(),
            expected_blake3: expected_blake3.clone(),
            precondition,
            bytes: bytes.to_vec(),
        }) {
            ConditionalPutOutcome::Committed { .. } => Ok(None),
            ConditionalPutOutcome::PreconditionFailed { .. } => {
                // Content-addressed immutable blob/manifest already exists; verify exact bytes.
                match read_s3_exact(s3, object_key, bytes.len().max(1)) {
                    Ok(existing) if blake3::hash(&existing).to_hex().as_str() == expected_blake3.as_str() => {
                        Ok(None)
                    }
                    Ok(_) => Err(ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                        "s3_existing_object_mismatch",
                        "s3.object",
                        "existing S3 object fails expected BLAKE3 digest",
                    ))),
                    Err(err) => Err(err),
                }
            }
            ConditionalPutOutcome::Unknown { reason } => Ok(Some(Diagnostic::new(
                "remote_write_unknown",
                "s3.put",
                &reason,
            ))),
            ConditionalPutOutcome::Failed { failure } => Err(ManifestStoreFailure::from(failure)),
        }
    }
}

impl<P: MoltenEvictPort, S: ConditionalObjectStorePort> ProjectedManifestStore<P, S> {
    /// Evict a manifest record from `MoltenPrimary` or `S3Secondary` after
    /// verifying eviction policy watermarks and retention roots.
    pub fn evict_manifest(
        &mut self,
        manifest_identity: &ContentId,
        target_tier: StorageTier,
        current_entries: usize,
        current_bytes: usize,
    ) -> Result<EvictionDecision, ManifestStoreFailure> {
        let record = self.records.get(manifest_identity).ok_or_else(|| {
            ManifestStoreFailure::NotFound(Diagnostic::new(
                "manifest_not_found",
                "manifest_identity",
                "manifest not found for eviction",
            ))
        })?;
        let prior_placement = record.placement(self.s3_enabled());
        let request = EvictionRequest {
            target_tier,
            current_entries,
            current_bytes,
            candidate: EvictionCandidate {
                manifest_identity: record.envelope.manifest_identity.clone(),
                record_identity: record.envelope.record_identity.clone(),
                payload_bytes: record.envelope.payload_bytes,
                placement: prior_placement,
                retention_roots: record.retention_roots.clone(),
                age_rank: record.age_rank,
            },
        };
        let plan = admit_eviction(&self.eviction_policy, &request)?;

        let outcome = match target_tier {
            StorageTier::MoltenPrimary => {
                if let Some(mapped_manifest) = &record.molten_manifest {
                    self.content
                        .port_mut()
                        .evict_exact(&mapped_manifest.reference)?;
                }
                if let Some(mapped_envelope) = &record.molten_envelope {
                    self.content
                        .port_mut()
                        .evict_exact(&mapped_envelope.reference)?;
                }
                // Evict unshared Molten blobs whose objects belong only to evicted records.
                let referenced_by_other_primary: BTreeSet<ContentId> = self
                    .records
                    .iter()
                    .filter(|(id, other)| {
                        *id != manifest_identity
                            && other.molten_state.presence == TierPresence::Verified
                    })
                    .flat_map(|(_, other)| {
                        other
                            .envelope
                            .object_closure
                            .iter()
                            .map(|b| b.digest.clone())
                    })
                    .collect();
                let mut evicted_blob_digests = Vec::new();
                for binding in &record.envelope.object_closure {
                    if !referenced_by_other_primary.contains(&binding.digest)
                        && let Some(blob) = self.blobs.get(&binding.digest)
                        && let Some(mapped) = &blob.molten_mapped
                    {
                        self.content.port_mut().evict_exact(&mapped.reference)?;
                        evicted_blob_digests.push(binding.digest.clone());
                    }
                }
                for digest in evicted_blob_digests {
                    if let Some(blob) = self.blobs.get_mut(&digest) {
                        blob.molten_mapped = None;
                    }
                }
                BackendEvictOutcome::Evicted
            }
            StorageTier::S3Secondary => {
                let root_key = record.s3_root_key.clone().ok_or_else(|| {
                    ManifestStoreFailure::NotFound(Diagnostic::new(
                        "s3_root_key_missing",
                        "s3.root_key",
                        "S3 root key missing for eviction",
                    ))
                })?;
                let expected_etag = record.s3_state.provider_etag.clone();
                let contract_version = self.s3_config.contract_version;
                let max_evict_batch = self.s3_config.max_evict_batch;
                let s3 = self.s3.as_mut().ok_or_else(|| {
                    ManifestStoreFailure::Domain(Diagnostic::new(
                        "s3_tier_unconfigured",
                        "s3.port",
                        "optional S3 secondary tier is unconfigured",
                    ))
                })?;
                match s3.evict_conditional(&BoundedEvictRequest {
                    contract_version,
                    object_key: root_key,
                    expected_etag,
                    max_evict_batch,
                }) {
                    ConditionalEvictOutcome::Evicted | ConditionalEvictOutcome::NotFound => {
                        BackendEvictOutcome::Evicted
                    }
                    ConditionalEvictOutcome::PreconditionFailed { .. } => {
                        BackendEvictOutcome::Failed {
                            code: "s3_evict_precondition_failed".to_string(),
                            message: "S3 conditional eviction ETag mismatch".to_string(),
                        }
                    }
                    ConditionalEvictOutcome::Unknown { reason } => {
                        BackendEvictOutcome::Unknown { reason }
                    }
                    ConditionalEvictOutcome::Failed { failure } => {
                        return Err(ManifestStoreFailure::from(failure));
                    }
                }
            }
        };

        let decision = apply_eviction_outcome(&plan, prior_placement, outcome);
        if let Some(record_mut) = self.records.get_mut(manifest_identity) {
            let next_presence = match &decision {
                EvictionDecision::Evicted { placement } => match target_tier {
                    StorageTier::MoltenPrimary => placement.molten_primary,
                    StorageTier::S3Secondary => placement.s3_secondary,
                },
                EvictionDecision::Unknown { placement, .. } => match target_tier {
                    StorageTier::MoltenPrimary => placement.molten_primary,
                    StorageTier::S3Secondary => placement.s3_secondary,
                },
                EvictionDecision::Refused { .. } => match target_tier {
                    StorageTier::MoltenPrimary => prior_placement.molten_primary,
                    StorageTier::S3Secondary => prior_placement.s3_secondary,
                },
            };
            match target_tier {
                StorageTier::MoltenPrimary => {
                    record_mut.molten_state.presence = next_presence;
                    if next_presence == TierPresence::Evicted {
                        record_mut.molten_manifest = None;
                        record_mut.molten_envelope = None;
                    }
                }
                StorageTier::S3Secondary => {
                    record_mut.s3_state.presence = next_presence;
                }
            }
        }
        Ok(decision)
    }
}

// ---------------------------------------------------------------------------
// Capability-scoped brokered download server

/// Redacted manifest entry returned for a view-scoped manifest read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedManifestEntry {
    pub destination: String,
    pub kind: kiln_core::projection::EntryKind,
    pub object_digest: Option<ContentId>,
}

/// Payload delivered by [`ProjectedManifestBroker::download`].
///
/// Never exposes S3 buckets, credentials, presigned URLs, or storage keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokeredPayload {
    Blob {
        digest: ContentId,
        bytes: Vec<u8>,
        served_from: StorageTier,
    },
    ManifestView {
        manifest_identity: ContentId,
        entries: Vec<AuthorizedManifestEntry>,
        served_from: StorageTier,
    },
    FullManifest {
        manifest: Box<ResolvedProjectionManifest>,
        served_from: StorageTier,
    },
}

/// Authorizing broker mediating capability-restricted manifest and blob reads.
#[derive(Clone, Debug)]
pub struct ProjectedManifestBroker<P, S = UnconfiguredS3Store> {
    store: ProjectedManifestStore<P, S>,
    revocations: BrokerRevocationState,
}

impl<P: AspenContentPort, S: ConditionalObjectStorePort> ProjectedManifestBroker<P, S> {
    #[must_use]
    pub fn new(store: ProjectedManifestStore<P, S>, revocations: BrokerRevocationState) -> Self {
        Self { store, revocations }
    }

    #[must_use]
    pub const fn store(&self) -> &ProjectedManifestStore<P, S> {
        &self.store
    }

    pub const fn store_mut(&mut self) -> &mut ProjectedManifestStore<P, S> {
        &mut self.store
    }

    #[must_use]
    pub const fn revocations(&self) -> &BrokerRevocationState {
        &self.revocations
    }

    pub fn revoke_ticket(&mut self, ticket_id: ContentId) {
        self.revocations.revoked_tickets.insert(ticket_id);
    }

    pub fn revoke_audience(&mut self, audience: impl Into<String>) {
        self.revocations.revoked_audiences.insert(audience.into());
    }

    pub fn revoke_view(&mut self, view_binding: ContentId) {
        self.revocations.revoked_views.insert(view_binding);
    }

    pub fn bump_revocation_generation(&mut self) {
        self.revocations.revocation_generation =
            self.revocations.revocation_generation.saturating_add(1);
    }

    pub fn bump_authority_generation(&mut self) {
        self.revocations.current_generation =
            self.revocations.current_generation.saturating_add(1);
    }

    /// Issue a capability-restricted download ticket for an existing manifest
    /// or one of its authorized entries.
    pub fn issue_ticket(
        &self,
        manifest_identity: &ContentId,
        target: BrokerTargetKind,
        scope: BrokerTicketScope,
        issued_at_unix_s: u64,
        expires_at_unix_s: u64,
    ) -> Result<BrokerDownloadTicket, ManifestStoreFailure> {
        let record = self.store.records.get(manifest_identity).ok_or_else(|| {
            ManifestStoreFailure::NotFound(Diagnostic::new(
                "manifest_not_found",
                "manifest_identity",
                "cannot issue broker ticket for unknown manifest",
            ))
        })?;
        if record.envelope.repository != scope.repository
            || record.envelope.exact_revision != scope.exact_revision
        {
            return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                "ticket_scope_binding_mismatch",
                "ticket.scope",
                "ticket scope repository or revision does not match persisted manifest",
            )));
        }
        if let BrokerTargetKind::Blob {
            destination,
            object_digest,
            ..
        } = &target
        {
            // Verify destination actually binds to `object_digest` inside this manifest.
            let loaded = self.store.load_manifest(
                manifest_identity,
                None,
                MAX_MANIFEST_ENVELOPE_BYTES,
            )?;
            let matched = loaded.manifest.entries.iter().find(|entry| {
                entry.destination == *destination
                    && matches!(&entry.origin, EntryOrigin::Native { .. })
            });
            let Some(entry) = matched else {
                return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                    "ticket_destination_not_in_manifest",
                    "ticket.target.destination",
                    "requested blob destination is not part of the manifest",
                )));
            };
            let EntryOrigin::Native { object, .. } = &entry.origin else {
                return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                    "ticket_destination_not_native",
                    "ticket.target.destination",
                    "requested destination is not a native blob entry",
                )));
            };
            let binding = record
                .envelope
                .object_closure
                .iter()
                .find(|b| b.object == *object);
            if binding.is_none_or(|b| b.digest != *object_digest) {
                return Err(ManifestStoreFailure::Domain(Diagnostic::new(
                    "ticket_digest_mismatch",
                    "ticket.target.object_digest",
                    "ticket digest does not match manifest entry object digest",
                )));
            }
        }

        Ok(admit_broker_ticket_issue(
            manifest_identity.clone(),
            target,
            scope,
            issued_at_unix_s,
            expires_at_unix_s,
        )?)
    }

    /// Execute a capability-scoped brokered read with pre-read ticket
    /// validation, revocation checks, strict view ceiling filtering, digest
    /// verification, and bounded size limits.
    pub fn download(
        &self,
        ticket: &BrokerDownloadTicket,
        request: &BrokerTicketRequest,
        now_unix_s: u64,
    ) -> Result<BrokeredPayload, ManifestStoreFailure> {
        let admitted = admit_broker_download(ticket, request, &self.revocations, now_unix_s)
            .map_err(ManifestStoreFailure::BrokerDenied)?;

        match admitted.target() {
            BrokerTargetKind::Blob { .. } => {
                self.download_admitted_blob(&admitted)
            }
            BrokerTargetKind::ManifestView => {
                self.download_admitted_manifest_view(&admitted)
            }
            BrokerTargetKind::FullManifest => {
                self.download_admitted_full_manifest(&admitted)
            }
        }
    }

    fn download_admitted_blob(
        &self,
        admitted: &AdmittedBrokerRead,
    ) -> Result<BrokeredPayload, ManifestStoreFailure> {
        let loaded = self.store.load_manifest(
            admitted.manifest_identity(),
            None,
            MAX_MANIFEST_ENVELOPE_BYTES,
        )?;
        let BrokerTargetKind::Blob { destination, .. } = admitted.target() else {
            return Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope));
        };
        if !admitted.permits_path(destination) {
            return Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope));
        }
        let entry = loaded
            .manifest
            .entries
            .iter()
            .find(|e| e.destination == *destination)
            .ok_or_else(|| {
                ManifestStoreFailure::NotFound(Diagnostic::new(
                    "blob_entry_missing",
                    "broker.destination",
                    "manifest entry no longer present",
                ))
            })?;
        let EntryOrigin::Native { object, .. } = &entry.origin else {
            return Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope));
        };
        let binding = loaded
            .envelope
            .object_closure
            .iter()
            .find(|b| b.object == *object)
            .ok_or_else(|| {
                ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                    "missing_closure_binding",
                    "envelope.object_closure",
                    "object closure binding missing for manifest entry",
                ))
            })?;
        if &binding.digest != admitted.expected_digest() {
            return Err(ManifestStoreFailure::BrokerDenied(
                BrokerDenial::DigestMismatch,
            ));
        }
        let bytes = self
            .store
            .load_blob_verified(admitted.expected_digest(), admitted.max_bytes())?;
        Ok(BrokeredPayload::Blob {
            digest: admitted.expected_digest().clone(),
            bytes,
            served_from: loaded.served_from,
        })
    }

    fn download_admitted_manifest_view(
        &self,
        admitted: &AdmittedBrokerRead,
    ) -> Result<BrokeredPayload, ManifestStoreFailure> {
        let loaded = self
            .store
            .load_manifest(admitted.manifest_identity(), None, admitted.max_bytes())?;
        let digest_by_object: BTreeMap<&ExternalRef, &ContentId> = loaded
            .envelope
            .object_closure
            .iter()
            .map(|b| (&b.object, &b.digest))
            .collect();
        let entries = loaded
            .manifest
            .entries
            .iter()
            .filter(|entry| manifest_entry_authorized(admitted, entry))
            .map(|entry| AuthorizedManifestEntry {
                destination: entry.destination.clone(),
                kind: entry.kind,
                object_digest: match &entry.origin {
                    EntryOrigin::Native { object, .. } => {
                        digest_by_object.get(object).map(|d| (*d).clone())
                    }
                    EntryOrigin::Generated { .. } => None,
                },
            })
            .collect();
        Ok(BrokeredPayload::ManifestView {
            manifest_identity: loaded.manifest.identity,
            entries,
            served_from: loaded.served_from,
        })
    }

    fn download_admitted_full_manifest(
        &self,
        admitted: &AdmittedBrokerRead,
    ) -> Result<BrokeredPayload, ManifestStoreFailure> {
        let root = PathScope::root();
        if !admitted.ceiling().contains(&root)
            || !admitted.allowed_paths().iter().any(|p| p.contains(&root))
        {
            return Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope));
        }
        let loaded = self
            .store
            .load_manifest(admitted.manifest_identity(), None, admitted.max_bytes())?;
        Ok(BrokeredPayload::FullManifest {
            manifest: Box::new(loaded.manifest),
            served_from: loaded.served_from,
        })
    }
}

fn manifest_entry_authorized(admitted: &AdmittedBrokerRead, entry: &ManifestEntry) -> bool {
    match &entry.origin {
        EntryOrigin::Native { path, .. } => {
            admitted.ceiling().allows(SourceViewOperation::ReadContent, path)
                && admitted
                    .allowed_paths()
                    .iter()
                    .any(|scope| scope.covers_path(path))
        }
        EntryOrigin::Generated { .. } => {
            let root = PathScope::root();
            admitted.ceiling().contains(&root)
                && admitted.allowed_paths().iter().any(|scope| scope.contains(&root))
        }
    }
}

fn map_s3_put_outcome(outcome: ConditionalPutOutcome) -> Result<BackendWriteOutcome, ManifestStoreFailure> {
    match outcome {
        ConditionalPutOutcome::Committed { etag } => Ok(BackendWriteOutcome::Committed {
            provider_etag: Some(etag),
        }),
        ConditionalPutOutcome::PreconditionFailed { observed_etag } => {
            Ok(BackendWriteOutcome::CasConflict { observed_etag })
        }
        ConditionalPutOutcome::Unknown { reason } => Ok(BackendWriteOutcome::Unknown { reason }),
        ConditionalPutOutcome::Failed { failure } => Err(ManifestStoreFailure::from(failure)),
    }
}

fn read_s3_exact<S: ConditionalObjectStorePort>(
    s3: &S,
    object_key: &str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ManifestStoreFailure> {
    match s3.get_bounded(object_key, maximum_bytes) {
        ConditionalReadOutcome::Found { bytes, .. } => Ok(bytes),
        ConditionalReadOutcome::NotFound => Err(ManifestStoreFailure::NotFound(Diagnostic::new(
            "s3_object_missing",
            "s3.object",
            "required S3 object is missing",
        ))),
        ConditionalReadOutcome::Unknown { reason } => {
            Err(ManifestStoreFailure::UnknownOutcome(Diagnostic::new(
                "s3_read_unknown",
                "s3.read",
                &reason,
            )))
        }
        ConditionalReadOutcome::Failed { failure } => Err(ManifestStoreFailure::from(failure)),
    }
}

fn map_content_read_failure(failure: PortFailure) -> ManifestStoreFailure {
    match failure {
        PortFailure::Infrastructure(ref infra) if infra.code == "aspen_content_corrupt" => {
            ManifestStoreFailure::IntegrityMismatch(Diagnostic::new(
                &infra.code,
                "aspen.content",
                &infra.message,
            ))
        }
        PortFailure::Domain(ref diag)
            if diag.code == "aspen_content_mapping_drift" || diag.code == "aspen_mapping_content_drift" =>
        {
            ManifestStoreFailure::IntegrityMismatch(diag.clone())
        }
        other => ManifestStoreFailure::from(other),
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn lock_failure() -> PortFailure {
    PortFailure::Infrastructure(infrastructure(
        "s3_store_lock",
        "in-memory S3 fixture lock is unavailable",
    ))
}

#[allow(dead_code)]
fn _unused_scope_operation_marker() -> SourceViewOperation {
    SourceViewOperation::ReadContent
}
