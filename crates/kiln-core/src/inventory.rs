//! Read-only resource observation profile v2. No reservations or scheduling effects.
// r[impl kiln.inventory.separation]
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const INVENTORY_PROFILE: &str = "kiln.observe.inventory.v2";
pub const MAX_INVENTORY_ITEMS: usize = 4096;
pub const MAX_INVENTORY_SOURCES: usize = 64;
pub const MAX_CANDIDATES: usize = 128;
pub const MAX_PUBLIC_TEXT: usize = 256;
pub const MAX_REQUIREMENTS: usize = 64;
/// JSON consumers must retain exact generation/fence/capacity values, not rounded floats.
pub const MAX_EXACT_NUMBER: u64 = 9_007_199_254_740_991;
/// Producer wait reasons one definition or worker fact may carry.
pub const MAX_WAIT_REASONS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactRef {
    pub source: String,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryKind {
    Agents,
    Workers,
    Compute,
    Serving,
    Policy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicy {
    pub source: String,
    pub producer: String,
    pub scope: String,
    pub kind: InventoryKind,
    pub minimum_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryProfile {
    pub schema: String,
    pub principal: String,
    pub sources: Vec<SourcePolicy>,
    pub max_items: usize,
    pub max_candidates: usize,
    pub max_age_ms: u64,
    pub include_evidence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObservation {
    pub source: String,
    pub producer: String,
    pub scope: String,
    pub generation: u64,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub complete: bool,
    pub next_page: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactMeta {
    pub reference: FactRef,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    /// Explicitly unknown field names. An empty list is not permission to infer omitted values.
    pub unknown_fields: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Allowed,
    Denied,
    Revoked,
    Unknown,
}

/// Whether a producer's revision lifecycle governs a definition. `unmanaged`
/// is neither unknown nor unqualified: there is no lifecycle fact to publish.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionManagement {
    Unmanaged,
    Managed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionState {
    pub desired: Option<String>,
    pub prepared: Option<String>,
    pub qualified: Option<String>,
    pub selected_for_new_sessions: Option<String>,
    pub preparation_failure: Option<String>,
    pub generation: u64,
    pub evidence: Vec<String>,
    /// Omitted when the producer does not say, so earlier snapshots keep
    /// their identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management: Option<RevisionManagement>,
}

/// How the producer labelled a qualification's evidence. Fixture evidence is
/// never live evidence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProvenance {
    Fixture,
    Live,
}

/// A producer-recorded, unrevoked reference that qualifies one task family
/// of the definition's profile revision. It is evidence, never authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFamilyQualification {
    pub task_family: String,
    pub reference: String,
    pub evidence_class: String,
    pub provenance: EvidenceProvenance,
}

/// The state of a worker session's pin in a producer's revision lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinStatus {
    Live,
    Closed,
    Released,
}

/// A worker's revision pin. `unmanaged`: the session was admitted outside a
/// revision lifecycle. `recorded`: the lifecycle's pin; `blocked` means a
/// further effect or export would be refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RevisionPin {
    Unmanaged {},
    Recorded {
        qualification: String,
        rollout_generation: u64,
        identity: String,
        status: PinStatus,
        blocked: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub profile_revision: String,
    pub task_family: String,
    pub subject: String,
    pub corpus: String,
    pub protocol: String,
    pub evidence: String,
    pub verified_contracts: Vec<String>,
    pub accepted_outcomes: Option<u64>,
    pub attempted_outcomes: Option<u64>,
    pub measured_cost_microunits: Option<u64>,
    pub unknown_use: bool,
    pub limitations: Vec<String>,
    pub valid_until_ms: u64,
    pub authority: Permission,
}

/// What a producer wait reason is about: a field of the fact that carries it,
/// or one of that fact's model routes by the name the producer published.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WaitSubject {
    Fact { field: String },
    Route { name: String },
}

impl WaitSubject {
    /// The explanation constraint naming this subject: `fact:<field>` or `route:<name>`.
    #[must_use]
    pub fn constraint(&self) -> String {
        match self {
            Self::Fact { field } => alloc::format!("fact:{field}"),
            Self::Route { name } => alloc::format!("route:{name}"),
        }
    }
}

/// A producer's exact reason a fact is not ready: a code from the producer's
/// closed vocabulary, its subject, and the owner that can resolve it (`name`
/// or `name#issue`). It carries no free text and is never a reservation,
/// claim or dispatch decision.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitReason {
    pub code: String,
    pub subject: WaitSubject,
    pub owner: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub meta: FactMeta,
    pub profile_revision: String,
    pub task_families: Vec<String>,
    pub output_contracts: Vec<String>,
    pub harnesses: Vec<String>,
    pub tools: Vec<String>,
    pub permitted_models: Vec<String>,
    pub authority: Permission,
    pub revisions: RevisionState,
    pub qualifications: Vec<Qualification>,
    /// Producer reasons this definition is not ready. Omitted when empty, so
    /// earlier snapshots keep their identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wait_reasons: Vec<WaitReason>,
    /// Current task-family qualification references of `profile_revision`,
    /// sorted by task family, at most one per family. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_family_qualifications: Vec<TaskFamilyQualification>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycle {
    Configured,
    Starting,
    Healthy,
    Idle,
    Reserved,
    Executing,
    Stopped,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerInstance {
    pub meta: FactMeta,
    pub incarnation: String,
    pub session: Option<String>,
    pub definition: FactRef,
    pub pinned_revision: String,
    pub attempt: Option<String>,
    pub fence: Option<u64>,
    pub execution: FactRef,
    pub execution_grant: Option<FactRef>,
    pub workspace: Option<String>,
    pub model_services: Vec<FactRef>,
    pub lifecycle: WorkerLifecycle,
    /// Producer reasons this worker is not ready. Omitted when empty, so
    /// earlier snapshots keep their identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wait_reasons: Vec<WaitReason>,
    /// The session's revision pin. Omitted when the producer publishes none;
    /// an unknown pin is listed in `meta.unknown_fields` as `pinned`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<RevisionPin>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    Draining,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeDevice {
    pub meta: FactMeta,
    pub host: String,
    pub device: Option<String>,
    pub pool: Option<String>,
    pub kind: String,
    pub health: Health,
    pub sandbox: Vec<String>,
    pub backends: Vec<String>,
    /// Authoritative pool references, never independent capacity copies.
    pub accounting_pools: Vec<FactRef>,
    pub qualification: Vec<String>,
    /// Serving deployment allocations an execution allocation references. The
    /// compute producer never charges them to this allocation; the deployment's
    /// own claims are accounted once in its pools. Omitted when empty, so
    /// earlier snapshots keep their identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_serving: Vec<FactRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolAllocation {
    pub allocation: String,
    pub consumer: FactRef,
    pub amount: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePool {
    pub meta: FactMeta,
    pub unit: String,
    pub capacity: Option<u64>,
    pub allocatable: Option<u64>,
    pub reserved: Option<u64>,
    pub allocations: Vec<PoolAllocation>,
    pub accounting_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingState {
    Configured,
    ArtifactCached,
    Loading,
    Warm,
    Saturated,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelService {
    pub meta: FactMeta,
    pub artifact: String,
    pub artifact_available: Option<bool>,
    pub artifact_verification_evidence: Option<String>,
    pub configuration: String,
    /// A claim by the serving producer, never a verification of weight bytes.
    pub claimed_model: String,
    pub deployment: String,
    pub endpoint_alias: String,
    pub replicas: Vec<String>,
    pub state: ServingState,
    pub backend: String,
    pub routing_permission: Permission,
    pub routing_policy_ref: Option<String>,
    pub tools: Vec<String>,
    pub modalities: Vec<String>,
    pub context_tokens: Option<u64>,
    pub available_requests: Option<u64>,
    pub queue_reason: Option<String>,
    pub measurement: Option<String>,
    /// Empty is valid for a hosted API; no fictitious GPU is constructed.
    pub accounting_pools: Vec<FactRef>,
    pub compute_devices: Vec<FactRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionState {
    Waiting,
    Reserved,
    Admitted,
    Rejected,
    Expired,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionFact {
    pub meta: FactMeta,
    pub work_version: String,
    pub attempt: String,
    pub fence: u64,
    pub resource: FactRef,
    pub state: AdmissionState,
    pub owner_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFact {
    pub meta: FactMeta,
    pub work_version: String,
    pub policy_ref: String,
    pub permission: Permission,
    pub owner_reason: Option<String>,
    pub waiting_until_ms: Option<u64>,
    pub queue_position: Option<u64>,
    pub tenant_limit_remaining: Option<u64>,
    pub campaign_limit_remaining: Option<u64>,
    pub parent_budget_remaining: Option<u64>,
    pub admission_explanation: Option<crate::fair_admission::AdmissionExplanation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptBinding {
    pub work_version: String,
    pub attempt: String,
    pub fence: u64,
    pub worker: FactRef,
    pub execution_grant: FactRef,
    pub model_services: Vec<FactRef>,
    pub request_admissions: Vec<FactRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventorySnapshot {
    pub schema: String,
    pub principal: String,
    pub synthetic: bool,
    pub sources: Vec<SourceObservation>,
    pub agents: Vec<AgentDefinition>,
    pub workers: Vec<WorkerInstance>,
    pub compute: Vec<ComputeDevice>,
    pub pools: Vec<ResourcePool>,
    pub models: Vec<ModelService>,
    pub admissions: Vec<AdmissionFact>,
    pub policies: Vec<PolicyFact>,
    pub attempts: Vec<AttemptBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRequirement {
    pub unit: String,
    pub amount: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkNeeds {
    pub work_version: String,
    pub task_family: String,
    pub output_contract: String,
    pub definition: FactRef,
    pub profile_revision: String,
    pub policy_ref: String,
    pub harness: String,
    pub tools: Vec<String>,
    pub sandbox: String,
    pub execution_backend: String,
    pub model_backend: String,
    pub context_tokens: u64,
    pub execution_resources: Vec<ResourceRequirement>,
    pub modalities: Vec<String>,
    pub model_artifact: Option<String>,
    pub qualification_subject: String,
    pub qualification_corpus: String,
    pub qualification_protocol: String,
}

/// Operator-admitted requirements bound to one exact canonical work revision/policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkNeedsAttachment {
    pub work_identity: crate::types::ContentId,
    pub work_revision: u64,
    pub policy_identity: crate::types::ContentId,
    pub requirements_identity: crate::types::ContentId,
    pub needs: WorkNeeds,
}

impl WorkNeedsAttachment {
    pub fn new(
        work_identity: crate::types::ContentId,
        work_revision: u64,
        policy_identity: crate::types::ContentId,
        needs: WorkNeeds,
    ) -> Result<Self, &'static str> {
        admit_work_needs(&needs)?;
        let requirements_identity = crate::types::ContentId::from_blake3_hex(seal(&needs)?)
            .map_err(|_| "inventory_requirements_identity_invalid")?;
        let attachment = Self {
            work_identity,
            work_revision,
            policy_identity,
            requirements_identity,
            needs,
        };
        attachment.validate_for(
            &attachment.work_identity,
            work_revision,
            &attachment.policy_identity,
        )?;
        Ok(attachment)
    }

    pub fn validate_for(
        &self,
        work_identity: &crate::types::ContentId,
        work_revision: u64,
        policy_identity: &crate::types::ContentId,
    ) -> Result<(), &'static str> {
        self.work_identity
            .validate()
            .map_err(|_| "inventory_work_identity_invalid")?;
        self.policy_identity
            .validate()
            .map_err(|_| "inventory_policy_identity_invalid")?;
        admit_work_needs(&self.needs)?;
        if &self.work_identity != work_identity
            || self.work_revision != work_revision
            || &self.policy_identity != policy_identity
            || self.needs.work_version != work_identity.as_str()
            || self.needs.policy_ref != policy_identity.as_str()
        {
            return Err("inventory_requirements_binding_mismatch");
        }
        if self.requirements_identity.as_str() != seal(&self.needs)? {
            return Err("inventory_requirements_identity_mismatch");
        }
        Ok(())
    }

    pub fn identity(&self) -> Result<crate::types::ContentId, &'static str> {
        self.validate_for(
            &self.work_identity,
            self.work_revision,
            &self.policy_identity,
        )?;
        crate::types::ContentId::from_blake3_hex(seal(self)?)
            .map_err(|_| "inventory_attachment_identity_invalid")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Current,
    Missing,
    Stale,
    Future,
    Incomplete,
    OutOfOrder,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceStatus {
    pub source: String,
    pub producer: String,
    pub scope: String,
    pub kind: InventoryKind,
    pub state: ObservationState,
    pub generation: Option<u64>,
    pub observed_at_ms: Option<u64>,
    pub valid_until_ms: Option<u64>,
    pub reason: Option<String>,
    pub next_page: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FactStatus {
    pub reference: FactRef,
    pub state: ObservationState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Blocker {
    pub code: String,
    pub subject: Option<FactRef>,
    pub constraint: String,
    /// Who can resolve a producer wait reason. Omitted for Kiln's own blockers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub worker: FactRef,
    pub execution: FactRef,
    pub model: FactRef,
    pub blockers: Vec<Blocker>,
    /// This can only mean eligible at observation time, never admitted/reserved.
    pub observed_eligible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadinessExplanation {
    pub work_version: String,
    pub definition: FactRef,
    pub profile_revision: String,
    pub policy_ref: String,
    pub blockers: Vec<Blocker>,
    pub candidates: Vec<Candidate>,
    pub combinations_considered: usize,
    pub truncated: bool,
    pub dispatch_authorized: bool,
    /// The needed task family's current qualification reference on the exact
    /// revision, with its provenance. Present only for a definition at the
    /// needed revision whose fact is not missing, stale or in the future;
    /// omitted otherwise, so earlier explanations keep their encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_family_qualification: Option<TaskFamilyQualification>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptExplanation {
    pub binding: AttemptBinding,
    pub blockers: Vec<Blocker>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InventoryProjection {
    pub schema: String,
    pub identity: String,
    pub profile_identity: String,
    pub evaluated_at_ms: u64,
    pub sources: Vec<SourceStatus>,
    pub facts: Vec<FactStatus>,
    pub snapshot: InventorySnapshot,
    pub explanation: Option<ReadinessExplanation>,
    pub attempt_explanations: Vec<AttemptExplanation>,
    pub work_requirements_unknown: bool,
    pub non_claims: Vec<String>,
}

pub fn public_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PUBLIC_TEXT
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:@+".contains(&b))
        && !value.contains("..")
}
fn token(value: &str) -> Result<(), &'static str> {
    if public_token(value) {
        Ok(())
    } else {
        Err("inventory_public_identifier_invalid")
    }
}
fn reference(value: &FactRef) -> Result<(), &'static str> {
    token(&value.source)?;
    token(&value.id)
}
fn tokens(values: &[String]) -> Result<(), &'static str> {
    if values.len() > MAX_REQUIREMENTS {
        return Err("inventory_field_bound");
    }
    for value in values {
        token(value)?;
    }
    Ok(())
}
fn optional(value: &Option<String>) -> Result<(), &'static str> {
    if let Some(value) = value {
        token(value)?;
    }
    Ok(())
}
fn references(values: &[FactRef]) -> Result<(), &'static str> {
    if values.len() > MAX_REQUIREMENTS {
        return Err("inventory_field_bound");
    }
    let mut seen = BTreeSet::new();
    for value in values {
        reference(value)?;
        if !seen.insert(value) {
            return Err("inventory_duplicate_reference");
        }
    }
    Ok(())
}
/// Decimal digits of the largest issue number an owner may name.
const MAX_OWNER_ISSUE_DIGITS: usize = 20;
/// An owner is a public name, optionally followed by `#` and an issue number.
fn owner(value: &str) -> Result<(), &'static str> {
    let (name, issue) = match value.split_once('#') {
        Some((name, issue)) => (name, Some(issue)),
        None => (value, None),
    };
    let issue_valid = issue.is_none_or(|issue| {
        !issue.is_empty()
            && issue.len() <= MAX_OWNER_ISSUE_DIGITS
            && issue.bytes().all(|b| b.is_ascii_digit())
    });
    if public_token(name) && issue_valid {
        Ok(())
    } else {
        Err("inventory_public_identifier_invalid")
    }
}
fn wait_reasons(values: &[WaitReason]) -> Result<(), &'static str> {
    if values.len() > MAX_WAIT_REASONS {
        return Err("inventory_field_bound");
    }
    let mut seen = BTreeSet::new();
    for value in values {
        token(&value.code)?;
        match &value.subject {
            WaitSubject::Fact { field } => token(field)?,
            WaitSubject::Route { name } => token(name)?,
        }
        // The explanation names the subject by this constraint.
        token(&value.subject.constraint())?;
        owner(&value.owner)?;
        if !seen.insert(value) {
            return Err("inventory_wait_reason_duplicate");
        }
    }
    Ok(())
}
/// Public tokens, bounded, strictly ascending by task family: one reference
/// per family.
fn task_family_qualifications(values: &[TaskFamilyQualification]) -> Result<(), &'static str> {
    if values.len() > MAX_REQUIREMENTS {
        return Err("inventory_field_bound");
    }
    for value in values {
        token(&value.task_family)?;
        token(&value.reference)?;
        token(&value.evidence_class)?;
    }
    let ascending = values.windows(2).all(|pair| match pair {
        [first, second] => first.task_family < second.task_family,
        _ => false,
    });
    if ascending {
        Ok(())
    } else {
        Err("inventory_task_family_qualification_unordered")
    }
}

fn exact(values: &[u64]) -> Result<(), &'static str> {
    if values.iter().any(|v| *v > MAX_EXACT_NUMBER) {
        Err("inventory_number_not_exact")
    } else {
        Ok(())
    }
}
fn exact_options<const N: usize>(values: [Option<u64>; N]) -> Result<(), &'static str> {
    for value in values.into_iter().flatten() {
        exact(&[value])?;
    }
    Ok(())
}

impl InventoryProfile {
    pub fn admit(&self) -> Result<(), &'static str> {
        if self.schema != INVENTORY_PROFILE {
            return Err("inventory_profile_unsupported");
        }
        token(&self.principal)?;
        exact(&[self.max_age_ms])?;
        if self.max_items == 0
            || self.max_items > MAX_INVENTORY_ITEMS
            || self.max_candidates == 0
            || self.max_candidates > MAX_CANDIDATES
            || self.max_age_ms == 0
            || self.sources.len() > MAX_INVENTORY_SOURCES
            || self.sources.is_empty()
        {
            return Err("inventory_profile_bound");
        }
        let mut seen = BTreeSet::new();
        for source in &self.sources {
            token(&source.source)?;
            token(&source.producer)?;
            token(&source.scope)?;
            exact(&[source.minimum_generation])?;
            if !seen.insert(&source.source) {
                return Err("inventory_source_ambiguous");
            }
        }
        Ok(())
    }
}

fn metas(snapshot: &InventorySnapshot) -> impl Iterator<Item = (&FactMeta, InventoryKind)> {
    snapshot
        .agents
        .iter()
        .map(|v| (&v.meta, InventoryKind::Agents))
        .chain(
            snapshot
                .workers
                .iter()
                .map(|v| (&v.meta, InventoryKind::Workers)),
        )
        .chain(
            snapshot
                .compute
                .iter()
                .map(|v| (&v.meta, InventoryKind::Compute)),
        )
        .chain(
            snapshot
                .pools
                .iter()
                .map(|v| (&v.meta, InventoryKind::Compute)),
        )
        .chain(
            snapshot
                .models
                .iter()
                .map(|v| (&v.meta, InventoryKind::Serving)),
        )
        .chain(
            snapshot
                .admissions
                .iter()
                .map(|v| (&v.meta, InventoryKind::Policy)),
        )
        .chain(
            snapshot
                .policies
                .iter()
                .map(|v| (&v.meta, InventoryKind::Policy)),
        )
}

pub fn admit_snapshot(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
) -> Result<(), &'static str> {
    profile.admit()?;
    if snapshot.schema != INVENTORY_PROFILE {
        return Err("inventory_snapshot_unsupported");
    }
    if profile.principal != snapshot.principal {
        return Err("inventory_principal_denied");
    }
    if snapshot.sources.len() > profile.sources.len() {
        return Err("inventory_source_bound");
    }
    let mut source_ids = BTreeSet::new();
    for source in &snapshot.sources {
        exact(&[
            source.generation,
            source.observed_at_ms,
            source.valid_until_ms,
        ])?;
        let policy = profile
            .sources
            .iter()
            .find(|p| p.source == source.source)
            .ok_or("inventory_source_denied")?;
        if source.producer != policy.producer || source.scope != policy.scope {
            return Err("inventory_provenance_mismatch");
        }
        if !source_ids.insert(&source.source) {
            return Err("inventory_source_ambiguous");
        }
        if source.observed_at_ms > source.valid_until_ms {
            return Err("inventory_time_invalid");
        }
        optional(&source.next_page)?;
        optional(&source.unavailable_reason)?;
        if source.complete && source.next_page.is_some() {
            return Err("inventory_completeness_invalid");
        }
    }
    if metas(snapshot)
        .count()
        .saturating_add(snapshot.attempts.len())
        > profile.max_items
    {
        return Err("inventory_item_bound");
    }
    let mut ids = BTreeSet::new();
    for (meta, kind) in metas(snapshot) {
        reference(&meta.reference)?;
        tokens(&meta.unknown_fields)?;
        exact(&[meta.observed_at_ms, meta.valid_until_ms])?;
        let policy = profile
            .sources
            .iter()
            .find(|p| p.source == meta.reference.source)
            .ok_or("inventory_source_denied")?;
        let source = snapshot
            .sources
            .iter()
            .find(|s| s.source == meta.reference.source)
            .ok_or("inventory_fact_without_source")?;
        if policy.kind != kind {
            return Err("inventory_source_kind_mismatch");
        }
        if !ids.insert(&meta.reference) {
            return Err("inventory_fact_ambiguous");
        }
        if meta.observed_at_ms > meta.valid_until_ms
            || meta.observed_at_ms > source.observed_at_ms
            || meta.valid_until_ms > source.valid_until_ms
        {
            return Err("inventory_time_invalid");
        }
    }
    for agent in &snapshot.agents {
        token(&agent.profile_revision)?;
        tokens(&agent.task_families)?;
        tokens(&agent.output_contracts)?;
        tokens(&agent.harnesses)?;
        tokens(&agent.tools)?;
        tokens(&agent.permitted_models)?;
        wait_reasons(&agent.wait_reasons)?;
        let revision = &agent.revisions;
        for value in [
            &revision.desired,
            &revision.prepared,
            &revision.qualified,
            &revision.selected_for_new_sessions,
            &revision.preparation_failure,
        ] {
            optional(value)?;
        }
        tokens(&revision.evidence)?;
        exact(&[revision.generation])?;
        if agent.qualifications.len() > MAX_REQUIREMENTS {
            return Err("inventory_field_bound");
        }
        for q in &agent.qualifications {
            for value in [
                &q.profile_revision,
                &q.task_family,
                &q.subject,
                &q.corpus,
                &q.protocol,
                &q.evidence,
            ] {
                token(value)?;
            }
            tokens(&q.verified_contracts)?;
            tokens(&q.limitations)?;
            exact(&[q.valid_until_ms])?;
            exact_options([
                q.accepted_outcomes,
                q.attempted_outcomes,
                q.measured_cost_microunits,
            ])?;
            if q.accepted_outcomes
                .zip(q.attempted_outcomes)
                .is_some_and(|(a, b)| a > b)
            {
                return Err("inventory_outcome_denominator_invalid");
            }
            if q.accepted_outcomes.is_some() && q.attempted_outcomes.is_none() {
                return Err("inventory_outcome_denominator_missing");
            }
        }
        task_family_qualifications(&agent.task_family_qualifications)?;
    }
    for worker in &snapshot.workers {
        token(&worker.incarnation)?;
        optional(&worker.session)?;
        reference(&worker.definition)?;
        exact_options([worker.fence])?;
        token(&worker.pinned_revision)?;
        optional(&worker.attempt)?;
        reference(&worker.execution)?;
        if let Some(grant) = &worker.execution_grant {
            reference(grant)?;
        }
        optional(&worker.workspace)?;
        references(&worker.model_services)?;
        wait_reasons(&worker.wait_reasons)?;
        if let Some(RevisionPin::Recorded {
            qualification,
            rollout_generation,
            identity,
            ..
        }) = &worker.pin
        {
            token(qualification)?;
            token(identity)?;
            exact(&[*rollout_generation])?;
        }
        if worker.attempt.is_some() != worker.fence.is_some() {
            return Err("inventory_attempt_fence_incomplete");
        }
    }
    for compute in &snapshot.compute {
        token(&compute.host)?;
        optional(&compute.device)?;
        optional(&compute.pool)?;
        token(&compute.kind)?;
        tokens(&compute.sandbox)?;
        tokens(&compute.backends)?;
        references(&compute.accounting_pools)?;
        references(&compute.shared_serving)?;
        tokens(&compute.qualification)?;
    }
    for pool in &snapshot.pools {
        token(&pool.unit)?;
        exact_options([pool.capacity, pool.allocatable, pool.reserved])?;
        if pool.allocations.len() > profile.max_items {
            return Err("inventory_field_bound");
        }
        let mut allocations = BTreeSet::new();
        let mut total = 0u64;
        for allocation in &pool.allocations {
            token(&allocation.allocation)?;
            reference(&allocation.consumer)?;
            exact(&[allocation.amount])?;
            if !allocations.insert(&allocation.allocation) {
                return Err("inventory_allocation_duplicate");
            }
            total = total
                .checked_add(allocation.amount)
                .ok_or("inventory_accounting_overflow")?;
        }
        if let Some(capacity) = pool.capacity {
            if pool.allocatable.is_some_and(|v| v > capacity)
                || pool.reserved.is_some_and(|v| v > capacity)
                || total > capacity
            {
                return Err("inventory_accounting_invalid");
            }
            if let (Some(free), Some(reserved)) = (pool.allocatable, pool.reserved) {
                if free
                    .checked_add(reserved)
                    .and_then(|v| v.checked_add(total))
                    .is_none_or(|v| v > capacity)
                {
                    return Err("inventory_accounting_invalid");
                }
            }
        }
    }
    for model in &snapshot.models {
        for value in [
            &model.artifact,
            &model.configuration,
            &model.claimed_model,
            &model.deployment,
            &model.endpoint_alias,
            &model.backend,
        ] {
            token(value)?;
        }
        tokens(&model.replicas)?;
        tokens(&model.tools)?;
        tokens(&model.modalities)?;
        exact_options([model.context_tokens, model.available_requests])?;
        optional(&model.queue_reason)?;
        optional(&model.measurement)?;
        references(&model.accounting_pools)?;
        optional(&model.artifact_verification_evidence)?;
        references(&model.compute_devices)?;
        optional(&model.routing_policy_ref)?;
    }
    for grant in &snapshot.admissions {
        token(&grant.work_version)?;
        token(&grant.attempt)?;
        reference(&grant.resource)?;
        optional(&grant.owner_reason)?;
        exact(&[grant.fence])?;
    }
    for policy in &snapshot.policies {
        token(&policy.work_version)?;
        token(&policy.policy_ref)?;
        optional(&policy.owner_reason)?;
        exact_options([
            policy.waiting_until_ms,
            policy.queue_position,
            policy.tenant_limit_remaining,
            policy.campaign_limit_remaining,
            policy.parent_budget_remaining,
        ])?;
        if let Some(admission) = &policy.admission_explanation {
            if admission.work_version.as_str() != policy.work_version
                || admission.policy_revision.as_str() != policy.policy_ref
            {
                return Err("inventory_admission_binding_mismatch");
            }
            token(admission.campaign.as_str())?;
            exact_options([admission.eligible_at_ms, admission.observation_at_ms])?;
            if admission
                .observation_at_ms
                .is_some_and(|at| at > policy.meta.observed_at_ms)
            {
                return Err("inventory_time_invalid");
            }
            for (key, amount) in admission.held.iter().chain(admission.measured.iter()) {
                token(key.as_str())?;
                exact(&[*amount])?;
            }
        }
    }
    let mut attempts = BTreeSet::new();
    for attempt in &snapshot.attempts {
        admit_attempt_binding(attempt)?;
        if !attempts.insert((&attempt.work_version, &attempt.attempt, attempt.fence)) {
            return Err("inventory_attempt_ambiguous");
        }
    }
    Ok(())
}

fn source_status(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    now: u64,
) -> Vec<SourceStatus> {
    let mut result: Vec<_> = profile
        .sources
        .iter()
        .map(|p| {
            let observed = snapshot.sources.iter().find(|s| s.source == p.source);
            let state = match observed {
                None => ObservationState::Missing,
                Some(s) if s.generation < p.minimum_generation => ObservationState::OutOfOrder,
                Some(s) if s.unavailable_reason.is_some() => ObservationState::Unavailable,
                Some(s) if s.observed_at_ms > now => ObservationState::Future,
                Some(s)
                    if now >= s.valid_until_ms
                        || now.saturating_sub(s.observed_at_ms) > profile.max_age_ms =>
                {
                    ObservationState::Stale
                }
                Some(s) if !s.complete || s.next_page.is_some() => ObservationState::Incomplete,
                Some(_) => ObservationState::Current,
            };
            SourceStatus {
                source: p.source.clone(),
                producer: p.producer.clone(),
                scope: p.scope.clone(),
                kind: p.kind,
                state,
                generation: observed.map(|s| s.generation),
                observed_at_ms: observed.map(|s| s.observed_at_ms),
                valid_until_ms: observed.map(|s| s.valid_until_ms),
                reason: observed.and_then(|s| s.unavailable_reason.clone()),
                next_page: observed.and_then(|s| s.next_page.clone()),
            }
        })
        .collect();
    result.sort_by(|a, b| a.source.cmp(&b.source));
    result
}

fn blocker(code: &str, subject: Option<&FactRef>, constraint: &str) -> Blocker {
    Blocker {
        code: code.into(),
        subject: subject.cloned(),
        constraint: constraint.into(),
        owner: None,
    }
}
/// One blocker per producer wait reason, with its exact code and owner.
fn wait_blockers(reasons: &[WaitReason], subject: &FactRef, result: &mut Vec<Blocker>) {
    result.extend(reasons.iter().map(|reason| Blocker {
        code: reason.code.clone(),
        subject: Some(subject.clone()),
        constraint: reason.subject.constraint(),
        owner: Some(reason.owner.clone()),
    }));
}
fn fact_state(
    meta: &FactMeta,
    statuses: &[SourceStatus],
    profile: &InventoryProfile,
    now: u64,
) -> ObservationState {
    let source = statuses
        .iter()
        .find(|s| s.source == meta.reference.source)
        .map(|s| s.state)
        .unwrap_or(ObservationState::Missing);
    if source != ObservationState::Current {
        return source;
    }
    if meta.observed_at_ms > now {
        return ObservationState::Future;
    }
    if now >= meta.valid_until_ms || now.saturating_sub(meta.observed_at_ms) > profile.max_age_ms {
        return ObservationState::Stale;
    }
    if !meta.unknown_fields.is_empty() {
        return ObservationState::Unknown;
    }
    ObservationState::Current
}
fn current(
    meta: &FactMeta,
    statuses: &[SourceStatus],
    profile: &InventoryProfile,
    now: u64,
) -> bool {
    fact_state(meta, statuses, profile, now) == ObservationState::Current
}
fn require(
    values: &[String],
    wanted: &str,
    code: &str,
    subject: &FactRef,
    result: &mut Vec<Blocker>,
) {
    if !values.iter().any(|v| v == wanted) {
        result.push(blocker(code, Some(subject), wanted));
    }
}
/// A capability Kiln holds unknown is never reported as unsupported.
fn require_capability(
    agent: &AgentDefinition,
    field: &str,
    values: &[String],
    wanted: &str,
    codes: (&str, &str),
    result: &mut Vec<Blocker>,
) {
    let (unsupported, unknown) = codes;
    if agent.meta.unknown_fields.iter().any(|f| f == field) {
        result.push(blocker(unknown, Some(&agent.meta.reference), wanted));
    } else {
        require(values, wanted, unsupported, &agent.meta.reference, result);
    }
}
/// The needed family's reference counts only for a definition at the needed
/// revision whose fact is current or merely has other unknown fields; a
/// missing, stale or future fact shows no qualification.
// r[impl kiln.inventory.producers.scenario.qualification]
fn task_family_qualification(
    agent: &AgentDefinition,
    needs: &WorkNeeds,
    statuses: &[SourceStatus],
    profile: &InventoryProfile,
    now: u64,
) -> Option<TaskFamilyQualification> {
    let fresh = matches!(
        fact_state(&agent.meta, statuses, profile, now),
        ObservationState::Current | ObservationState::Unknown
    );
    if !fresh || agent.profile_revision != needs.profile_revision {
        return None;
    }
    agent
        .task_family_qualifications
        .iter()
        .find(|q| q.task_family == needs.task_family)
        .cloned()
}
fn check_pools(
    refs: &[FactRef],
    snapshot: &InventorySnapshot,
    statuses: &[SourceStatus],
    profile: &InventoryProfile,
    now: u64,
    result: &mut Vec<Blocker>,
) {
    for reference in refs {
        match snapshot
            .pools
            .iter()
            .find(|p| &p.meta.reference == reference)
        {
            None => result.push(blocker(
                "accounting_pool_unknown",
                Some(reference),
                "authoritative_shared_pool",
            )),
            Some(pool) => {
                if !current(&pool.meta, statuses, profile, now)
                    || !pool.accounting_complete
                    || pool.capacity.is_none()
                    || pool.allocatable.is_none()
                    || pool.reserved.is_none()
                {
                    result.push(blocker(
                        "accounting_pool_unknown",
                        Some(reference),
                        "fresh_complete_accounting",
                    ));
                } else if pool.allocatable == Some(0) {
                    result.push(blocker(
                        "execution_capacity_unavailable",
                        Some(reference),
                        &pool.unit,
                    ));
                }
            }
        }
    }
}

fn admission_reason_code(reason: &crate::fair_admission::AdmissionReason) -> &'static str {
    use crate::fair_admission::AdmissionReason;
    match reason {
        AdmissionReason::Waiting => "waiting",
        AdmissionReason::QueueFull => "queue_full",
        AdmissionReason::Expired => "expired",
        AdmissionReason::Denied => "denied",
        AdmissionReason::Incomplete => "incomplete",
        AdmissionReason::ProviderRejected => "provider_rejected",
        AdmissionReason::ProviderFailure => "provider_failure",
        AdmissionReason::Unknown => "unknown",
        AdmissionReason::CancellationRequested => "cancellation_requested",
        AdmissionReason::Cancelled => "cancelled",
        AdmissionReason::Eligible => "eligible",
        AdmissionReason::BudgetExhausted => "budget_exhausted",
        AdmissionReason::Incompatible => "incompatible",
        AdmissionReason::Stale => "stale",
        AdmissionReason::ConcurrencyFull => "concurrency_full",
        AdmissionReason::Overloaded => "overloaded",
    }
}

pub fn admit_work_needs(needs: &WorkNeeds) -> Result<(), &'static str> {
    for value in [
        &needs.work_version,
        &needs.task_family,
        &needs.output_contract,
        &needs.policy_ref,
        &needs.harness,
        &needs.sandbox,
        &needs.execution_backend,
        &needs.model_backend,
        &needs.profile_revision,
        &needs.qualification_subject,
        &needs.qualification_corpus,
        &needs.qualification_protocol,
    ] {
        token(value)?;
    }
    reference(&needs.definition)?;
    tokens(&needs.tools)?;
    tokens(&needs.modalities)?;
    optional(&needs.model_artifact)?;
    exact(&[needs.context_tokens])?;
    if needs.context_tokens == 0 {
        return Err("inventory_context_invalid");
    }
    if needs.execution_resources.is_empty() || needs.execution_resources.len() > MAX_REQUIREMENTS {
        return Err("inventory_resource_requirements_missing_or_bounded");
    }
    let mut units = BTreeSet::new();
    for resource in &needs.execution_resources {
        token(&resource.unit)?;
        exact(&[resource.amount])?;
        if resource.amount == 0 || !units.insert(&resource.unit) {
            return Err("inventory_resource_requirements_invalid");
        }
    }
    Ok(())
}

/// Eligibility explanation only. Owning services must revalidate at reservation/dispatch.
pub fn explain_readiness(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    needs: &WorkNeeds,
    now: u64,
) -> Result<ReadinessExplanation, &'static str> {
    admit_snapshot(profile, snapshot)?;
    admit_work_needs(needs)?;
    exact(&[now])?;
    let statuses = source_status(profile, snapshot, now);
    Ok(explain_admitted_readiness(
        profile, snapshot, needs, now, &statuses,
    ))
}

fn explain_admitted_readiness(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    needs: &WorkNeeds,
    now: u64,
    statuses: &[SourceStatus],
) -> ReadinessExplanation {
    let mut result = ReadinessExplanation {
        work_version: needs.work_version.clone(),
        definition: needs.definition.clone(),
        policy_ref: needs.policy_ref.clone(),
        profile_revision: needs.profile_revision.clone(),
        blockers: Vec::new(),
        candidates: Vec::new(),
        combinations_considered: 0,
        truncated: false,
        dispatch_authorized: false,
        task_family_qualification: None,
    };
    for status in statuses {
        if status.state != ObservationState::Current {
            result.blockers.push(blocker(
                "provider_observation_unknown",
                None,
                &status.source,
            ));
        }
    }
    match snapshot
        .agents
        .iter()
        .find(|a| a.meta.reference == needs.definition)
    {
        None => result.blockers.push(blocker(
            "agent_definition_unknown",
            Some(&needs.definition),
            "exact_definition",
        )),
        Some(agent) => {
            if !current(&agent.meta, statuses, profile, now) {
                result.blockers.push(blocker(
                    "stale_or_unknown_agent",
                    Some(&agent.meta.reference),
                    "fresh_definition",
                ));
            }
            if agent.authority != Permission::Allowed {
                result.blockers.push(blocker(
                    "agent_authority_denied",
                    Some(&agent.meta.reference),
                    "current_permission",
                ));
            }
            if agent.profile_revision != needs.profile_revision {
                result.blockers.push(blocker(
                    "agent_revision_mismatch",
                    Some(&agent.meta.reference),
                    &needs.profile_revision,
                ));
            }
            require_capability(
                agent,
                "task_families",
                &agent.task_families,
                &needs.task_family,
                ("unsupported_task_family", "agent_task_family_unknown"),
                &mut result.blockers,
            );
            require(
                &agent.output_contracts,
                &needs.output_contract,
                "unsupported_output_contract",
                &agent.meta.reference,
                &mut result.blockers,
            );
            require(
                &agent.harnesses,
                &needs.harness,
                "unsupported_harness",
                &agent.meta.reference,
                &mut result.blockers,
            );
            for tool in &needs.tools {
                require_capability(
                    agent,
                    "tools",
                    &agent.tools,
                    tool,
                    ("unsupported_agent_tool", "agent_tool_unknown"),
                    &mut result.blockers,
                );
            }
            if agent.revisions.selected_for_new_sessions.as_ref() != Some(&agent.profile_revision) {
                result.blockers.push(blocker(
                    "revision_not_selected_for_new_sessions",
                    Some(&agent.meta.reference),
                    &agent.profile_revision,
                ));
            }
            result.task_family_qualification =
                task_family_qualification(agent, needs, statuses, profile, now);
            if let Some(q) = &result.task_family_qualification
                && q.provenance != EvidenceProvenance::Live
            {
                result.blockers.push(blocker(
                    "task_family_qualification_not_live",
                    Some(&agent.meta.reference),
                    &q.reference,
                ));
            }
            let applicable = agent.qualifications.iter().any(|q| {
                agent.profile_revision == needs.profile_revision
                    && q.profile_revision == needs.profile_revision
                    && q.task_family == needs.task_family
                    && q.subject == needs.qualification_subject
                    && q.corpus == needs.qualification_corpus
                    && q.protocol == needs.qualification_protocol
                    && q.authority == Permission::Allowed
                    && now < q.valid_until_ms
                    && q.verified_contracts.contains(&needs.output_contract)
            });
            if !applicable {
                result.blockers.push(blocker(
                    "qualification_missing_stale_or_mismatched",
                    Some(&agent.meta.reference),
                    &needs.task_family,
                ));
            }
            wait_blockers(
                &agent.wait_reasons,
                &agent.meta.reference,
                &mut result.blockers,
            );
        }
    }
    let mut policies = snapshot
        .policies
        .iter()
        .filter(|p| p.work_version == needs.work_version && p.policy_ref == needs.policy_ref);
    let policy = policies.next();
    if policies.next().is_some() || policy.is_none() {
        result.blockers.push(blocker(
            "policy_unknown_or_ambiguous",
            None,
            &needs.policy_ref,
        ));
    } else if let Some(policy) = policy {
        if !current(&policy.meta, statuses, profile, now)
            || policy.permission != Permission::Allowed
        {
            result.blockers.push(blocker(
                "policy_denied_or_unknown",
                Some(&policy.meta.reference),
                policy
                    .owner_reason
                    .as_deref()
                    .unwrap_or("policy_permission"),
            ));
        }
        for (code, amount) in [
            ("tenant_limit", policy.tenant_limit_remaining),
            ("campaign_limit", policy.campaign_limit_remaining),
            ("parent_budget_exhausted", policy.parent_budget_remaining),
        ] {
            if amount.is_none_or(|v| v == 0) {
                result.blockers.push(blocker(
                    code,
                    Some(&policy.meta.reference),
                    policy
                        .owner_reason
                        .as_deref()
                        .unwrap_or("remaining_unknown_or_zero"),
                ));
            }
        }
        if policy.waiting_until_ms.is_some_and(|v| now < v) {
            result.blockers.push(blocker(
                "bounded_wait",
                Some(&policy.meta.reference),
                policy.owner_reason.as_deref().unwrap_or("producer_wait"),
            ));
        }
        if let Some(admission) = &policy.admission_explanation {
            let reason = admission_reason_code(&admission.reason);
            if reason != "eligible" {
                result.blockers.push(blocker(
                    "fair_admission_blocked",
                    Some(&policy.meta.reference),
                    reason,
                ));
            }
            if admission
                .observation_at_ms
                .is_none_or(|at| at > now || now.saturating_sub(at) > profile.max_age_ms)
            {
                result.blockers.push(blocker(
                    "admission_observation_unknown",
                    Some(&policy.meta.reference),
                    "current_owning_producer_observation",
                ));
            }
            if admission.eligible_at_ms.is_some_and(|at| now < at) {
                result.blockers.push(blocker(
                    "bounded_wait",
                    Some(&policy.meta.reference),
                    reason,
                ));
            }
        }
    }
    let mut workers: Vec<_> = snapshot
        .workers
        .iter()
        .filter(|w| w.definition == needs.definition)
        .collect();
    workers.sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    let mut models: Vec<_> = snapshot.models.iter().collect();
    models.sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    if workers.is_empty() {
        result.blockers.push(blocker(
            "no_observed_worker",
            Some(&needs.definition),
            "definition_is_not_an_instance",
        ));
    }
    if models.is_empty() {
        result
            .blockers
            .push(blocker("model_service_unknown", None, "serving_inventory"));
    }
    result.combinations_considered = workers.len().saturating_mul(models.len());
    'candidates: for worker in workers {
        for model in &models {
            if result.candidates.len() == profile.max_candidates {
                result.truncated = true;
                break 'candidates;
            }
            let mut reasons = Vec::new();
            if !current(&worker.meta, statuses, profile, now) {
                reasons.push(blocker(
                    "stale_or_unknown_worker",
                    Some(&worker.meta.reference),
                    "fresh_worker",
                ));
            }
            if worker.lifecycle != WorkerLifecycle::Idle {
                reasons.push(blocker(
                    "worker_not_idle",
                    Some(&worker.meta.reference),
                    "observed_idle",
                ));
            }
            wait_blockers(&worker.wait_reasons, &worker.meta.reference, &mut reasons);
            if let Some(RevisionPin::Recorded {
                identity,
                blocked: true,
                ..
            }) = &worker.pin
            {
                reasons.push(blocker(
                    "worker_pin_blocked",
                    Some(&worker.meta.reference),
                    identity,
                ));
            }
            if let Some(agent) = snapshot
                .agents
                .iter()
                .find(|a| a.meta.reference == needs.definition)
            {
                if worker.pinned_revision != needs.profile_revision {
                    reasons.push(blocker(
                        "worker_pinned_other_revision",
                        Some(&worker.meta.reference),
                        &worker.pinned_revision,
                    ));
                }
                require(
                    &agent.permitted_models,
                    &model.artifact,
                    "model_not_permitted",
                    &agent.meta.reference,
                    &mut reasons,
                );
            }
            if !worker.model_services.contains(&model.meta.reference) {
                reasons.push(blocker(
                    "model_service_not_bound",
                    Some(&worker.meta.reference),
                    &model.meta.reference.id,
                ));
            }
            match snapshot
                .compute
                .iter()
                .find(|c| c.meta.reference == worker.execution)
            {
                None => reasons.push(blocker(
                    "execution_offer_unknown",
                    Some(&worker.execution),
                    "exact_execution_binding",
                )),
                Some(compute) => {
                    if !current(&compute.meta, statuses, profile, now)
                        || compute.health != Health::Healthy
                    {
                        reasons.push(blocker(
                            "execution_capacity_unavailable",
                            Some(&compute.meta.reference),
                            "fresh_healthy_offer",
                        ));
                    }
                    require(
                        &compute.sandbox,
                        &needs.sandbox,
                        "unsupported_sandbox",
                        &compute.meta.reference,
                        &mut reasons,
                    );
                    require(
                        &compute.backends,
                        &needs.execution_backend,
                        "incompatible_execution_backend",
                        &compute.meta.reference,
                        &mut reasons,
                    );
                    if compute.accounting_pools.is_empty() {
                        reasons.push(blocker(
                            "execution_capacity_unknown",
                            Some(&compute.meta.reference),
                            "accounting_pool_missing",
                        ));
                    }
                    check_pools(
                        &compute.accounting_pools,
                        snapshot,
                        statuses,
                        profile,
                        now,
                        &mut reasons,
                    );
                    if compute.qualification.is_empty() {
                        reasons.push(blocker(
                            "execution_qualification_unknown",
                            Some(&compute.meta.reference),
                            "compatibility_evidence",
                        ));
                    }
                    for demand in &needs.execution_resources {
                        let mut matching = snapshot.pools.iter().filter(|pool| {
                            compute.accounting_pools.contains(&pool.meta.reference)
                                && pool.unit == demand.unit
                        });
                        let pool = matching.next();
                        if matching.next().is_some() {
                            reasons.push(blocker(
                                "execution_resource_ambiguous",
                                Some(&compute.meta.reference),
                                &demand.unit,
                            ));
                        } else if pool
                            .is_none_or(|p| p.allocatable.is_none_or(|v| v < demand.amount))
                        {
                            reasons.push(blocker(
                                "execution_capacity_insufficient_or_unknown",
                                Some(&compute.meta.reference),
                                &demand.unit,
                            ));
                        }
                    }
                }
            }
            if !current(&model.meta, statuses, profile, now) {
                reasons.push(blocker(
                    "stale_or_unknown_serving",
                    Some(&model.meta.reference),
                    "fresh_serving_observation",
                ));
            }
            if model.routing_permission != Permission::Allowed
                || model.routing_policy_ref.as_ref() != Some(&needs.policy_ref)
            {
                reasons.push(blocker(
                    "model_policy_denied_or_unknown",
                    Some(&model.meta.reference),
                    &needs.policy_ref,
                ));
            }
            if model.state != ServingState::Warm {
                let code = match model.state {
                    ServingState::Loading => "model_loading",
                    ServingState::Saturated => "inference_saturated",
                    _ => "model_not_ready",
                };
                reasons.push(blocker(
                    code,
                    Some(&model.meta.reference),
                    model.queue_reason.as_deref().unwrap_or("serving_state"),
                ));
            }
            if model.backend != needs.model_backend {
                reasons.push(blocker(
                    "incompatible_backend",
                    Some(&model.meta.reference),
                    &needs.model_backend,
                ));
            }
            if needs
                .model_artifact
                .as_ref()
                .is_some_and(|v| v != &model.artifact)
            {
                reasons.push(blocker(
                    "model_artifact_mismatch",
                    Some(&model.meta.reference),
                    needs.model_artifact.as_deref().unwrap_or("model"),
                ));
            }
            if model
                .context_tokens
                .is_none_or(|v| v < needs.context_tokens)
            {
                reasons.push(blocker(
                    "unsupported_context",
                    Some(&model.meta.reference),
                    &needs.context_tokens.to_string(),
                ));
            }
            for tool in &needs.tools {
                require(
                    &model.tools,
                    tool,
                    "unsupported_model_tool",
                    &model.meta.reference,
                    &mut reasons,
                );
            }
            for modality in &needs.modalities {
                require(
                    &model.modalities,
                    modality,
                    "unsupported_modality",
                    &model.meta.reference,
                    &mut reasons,
                );
            }
            if model.available_requests.is_none_or(|v| v == 0) || model.measurement.is_none() {
                reasons.push(blocker(
                    "inference_capacity_unknown_or_saturated",
                    Some(&model.meta.reference),
                    "measured_request_capacity",
                ));
            }
            check_pools(
                &model.accounting_pools,
                snapshot,
                statuses,
                profile,
                now,
                &mut reasons,
            );
            for device in &model.compute_devices {
                if snapshot
                    .compute
                    .iter()
                    .find(|c| &c.meta.reference == device)
                    .is_none_or(|c| {
                        !current(&c.meta, statuses, profile, now) || c.health != Health::Healthy
                    })
                {
                    reasons.push(blocker(
                        "serving_compute_unknown_or_unavailable",
                        Some(device),
                        "exact_serving_placement",
                    ));
                }
            }
            result.candidates.push(Candidate {
                worker: worker.meta.reference.clone(),
                execution: worker.execution.clone(),
                model: model.meta.reference.clone(),
                observed_eligible: reasons.is_empty()
                    && result.blockers.is_empty()
                    && !snapshot.synthetic,
                blockers: reasons,
            });
        }
    }
    if snapshot.synthetic {
        result
            .blockers
            .push(blocker("synthetic_disconnected", None, "not_live_capacity"));
    }
    result
}

fn seal<T: Serialize>(value: &T) -> Result<String, &'static str> {
    let bytes = serde_json::to_vec(value).map_err(|_| "inventory_encoding_invalid")?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// Canonical ordering/redaction is shared by live, CLI and offline HTML consumers.
pub fn project_inventory(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    needs: Option<&WorkNeeds>,
    now: u64,
) -> Result<InventoryProjection, &'static str> {
    admit_snapshot(profile, snapshot)?;
    exact(&[now])?;
    let sources = source_status(profile, snapshot, now);
    let explanation = if let Some(needs) = needs {
        admit_work_needs(needs)?;
        Some(explain_admitted_readiness(
            profile, snapshot, needs, now, &sources,
        ))
    } else {
        None
    };
    let mut attempt_explanations: Vec<_> = snapshot
        .attempts
        .iter()
        .map(|binding| AttemptExplanation {
            binding: binding.clone(),
            blockers: explain_admitted_attempt(profile, snapshot, binding, now, &sources),
        })
        .collect();
    attempt_explanations.sort_by(|a, b| {
        (&a.binding.work_version, &a.binding.attempt, a.binding.fence).cmp(&(
            &b.binding.work_version,
            &b.binding.attempt,
            b.binding.fence,
        ))
    });
    let mut snapshot = snapshot.clone();
    snapshot.sources.sort_by(|a, b| a.source.cmp(&b.source));
    snapshot
        .agents
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .workers
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .compute
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .pools
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .models
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .admissions
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .policies
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot.attempts.sort_by(|a, b| {
        (&a.work_version, &a.attempt, a.fence).cmp(&(&b.work_version, &b.attempt, b.fence))
    });
    if !profile.include_evidence {
        for agent in &mut snapshot.agents {
            agent.revisions.evidence.clear();
            for q in &mut agent.qualifications {
                q.evidence = "redacted".into();
                q.subject = "redacted".into();
                q.corpus = "redacted".into();
                q.protocol = "redacted".into();
            }
        }
        for worker in &mut snapshot.workers {
            worker.workspace = None;
            worker.session = None;
        }
        for compute in &mut snapshot.compute {
            compute.qualification.clear();
        }
        for model in &mut snapshot.models {
            model.measurement = model.measurement.as_ref().map(|_| "redacted".into());
            model.artifact_verification_evidence = model
                .artifact_verification_evidence
                .as_ref()
                .map(|_| "redacted".into());
        }
    }
    let mut facts: Vec<_> = metas(&snapshot)
        .map(|(meta, _)| FactStatus {
            reference: meta.reference.clone(),
            state: fact_state(meta, &sources, profile, now),
        })
        .collect();
    facts.sort_by(|a, b| a.reference.cmp(&b.reference));
    let mut projection = InventoryProjection {
        schema: INVENTORY_PROFILE.into(),
        identity: String::new(),
        profile_identity: seal(profile)?,
        evaluated_at_ms: now,
        sources,
        facts,
        snapshot,
        explanation,
        attempt_explanations,
        work_requirements_unknown: needs.is_none(),
        non_claims: [
            "read_only_no_dispatch_authority",
            "provider_revalidation_required",
            "claimed_model_not_weight_verification",
            "configured_slots_not_physical_capacity",
            "qualification_not_availability",
            "unknown_use_not_zero",
        ]
        .iter()
        .map(|v| (*v).into())
        .collect(),
    };
    projection.identity = seal(&projection)?;
    Ok(projection)
}

fn admit_attempt_binding(binding: &AttemptBinding) -> Result<(), &'static str> {
    token(&binding.work_version)?;
    token(&binding.attempt)?;
    reference(&binding.worker)?;
    exact(&[binding.fence])?;
    reference(&binding.execution_grant)?;
    references(&binding.model_services)?;
    references(&binding.request_admissions)?;
    if binding
        .request_admissions
        .contains(&binding.execution_grant)
    {
        return Err("inventory_grant_binding_ambiguous");
    }
    Ok(())
}

/// Validates the exact observed join; rejection remains visible and cannot become a grant.
pub fn explain_attempt_binding(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    binding: &AttemptBinding,
    now: u64,
) -> Result<Vec<Blocker>, &'static str> {
    admit_snapshot(profile, snapshot)?;
    exact(&[now, binding.fence])?;
    let statuses = source_status(profile, snapshot, now);
    admit_attempt_binding(binding)?;
    Ok(explain_admitted_attempt(
        profile, snapshot, binding, now, &statuses,
    ))
}

fn explain_admitted_attempt(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
    binding: &AttemptBinding,
    now: u64,
    statuses: &[SourceStatus],
) -> Vec<Blocker> {
    let mut result = Vec::new();
    let worker = snapshot
        .workers
        .iter()
        .find(|w| w.meta.reference == binding.worker);
    match worker {
        None => result.push(blocker(
            "attempt_worker_unknown",
            Some(&binding.worker),
            &binding.attempt,
        )),
        Some(worker) => {
            if !current(&worker.meta, statuses, profile, now)
                || worker.attempt.as_ref() != Some(&binding.attempt)
                || worker.fence != Some(binding.fence)
                || worker.execution_grant.as_ref() != Some(&binding.execution_grant)
                || binding.model_services != worker.model_services
            {
                result.push(blocker(
                    "attempt_binding_stale_or_mismatched",
                    Some(&binding.worker),
                    &binding.attempt,
                ));
            }
        }
    }
    if binding.model_services.len() != binding.request_admissions.len() {
        result.push(blocker("request_admission_missing", None, &binding.attempt));
    }
    let expected = core::iter::once((&binding.execution_grant, worker.map(|w| &w.execution)))
        .chain(
            binding
                .request_admissions
                .iter()
                .enumerate()
                .map(|(index, grant)| (grant, binding.model_services.get(index))),
        );
    for (grant_ref, resource) in expected {
        match snapshot
            .admissions
            .iter()
            .find(|g| &g.meta.reference == grant_ref)
        {
            None => result.push(blocker(
                "reservation_unknown",
                Some(grant_ref),
                &binding.attempt,
            )),
            Some(grant) => {
                if !current(&grant.meta, statuses, profile, now)
                    || grant.work_version != binding.work_version
                    || grant.attempt != binding.attempt
                    || grant.fence != binding.fence
                    || resource.is_none_or(|r| r != &grant.resource)
                {
                    result.push(blocker(
                        "reservation_stale_or_mismatched",
                        Some(grant_ref),
                        &binding.attempt,
                    ));
                }
                if grant.state != AdmissionState::Admitted {
                    result.push(blocker(
                        "reservation_not_admitted",
                        Some(grant_ref),
                        grant
                            .owner_reason
                            .as_deref()
                            .unwrap_or("owning_provider_admission_required"),
                    ));
                }
            }
        }
    }
    result
}

#[path = "inventory_compose.rs"]
mod compose;
pub use compose::{InventoryFragment, InventoryJoins, compose_snapshot};

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
