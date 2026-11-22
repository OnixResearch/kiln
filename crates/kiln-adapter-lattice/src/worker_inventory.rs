//! Lattice `lattice.worker-inventory.v3` documents (the output of
//! `lattice implementation-attempt inspect`, `docs/worker-inventory.md` and
//! `docs/agent-revisions.md` in Lattice) mapped into inventory v2 agent
//! definitions and worker instances.
//!
//! The DTOs mirror Lattice's wire record field for field and deny unknown
//! fields; Kiln owns them so no Lattice code enters Kiln. v3 is the only
//! accepted version: a v1 or v2 document is refused as unsupported. A profile
//! is a definition keyed by its definition identity, never a worker. Its
//! revision lifecycle is `unmanaged`, `unknown` with an owner, or `recorded`,
//! and the three stay distinct. Several revisions of one definition coexist
//! when the lifecycle records which one is selected for new sessions; workers
//! keep the revision they pin. Lattice's single-shot liveness is never idle or
//! live, so no lifecycle maps to idle, healthy or executing. Wait reasons keep
//! their exact code, subject and owner.
// r[impl kiln.inventory.producers]
// r[impl kiln.inventory.producers.scenario.lattice]
// r[impl kiln.inventory.producers.scenario.capability]
// r[impl kiln.inventory.producers.scenario.revisions]
// r[impl kiln.inventory.producers.scenario.refused]
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use kiln_core::inventory::{
    AgentDefinition, EvidenceProvenance, FactMeta, FactRef, InventoryFragment, InventoryJoins,
    InventoryKind, MAX_INVENTORY_ITEMS, Permission, PinStatus, RevisionManagement, RevisionPin,
    RevisionState, SourceObservation, TaskFamilyQualification, WaitReason, WorkerInstance,
    WorkerLifecycle, public_token,
};
use serde::Deserialize;
use serde_json::Value;

#[path = "worker_inventory_wait.rs"]
mod wait;

use wait::{Code, Field, ReasonDto, Section};

pub const WORKER_INVENTORY_SCHEMA: &str = "lattice.worker-inventory.v3";
pub const PROFILE_FACTS_SCHEMA: &str = "lattice.worker-profile-facts.v3";
pub const INSTANCE_SCHEMA: &str = "lattice.worker-instance-observation.v3";
pub const INSTANCE_STATE_SOURCE: &str = "lattice.implementation-attempt-state.v1";
/// Lattice bounds one inspected artifact to 256 KiB (`MAX_SMALL_ARTIFACT_BYTES`).
pub const MAX_WORKER_INVENTORY_BYTES: usize = 262_144;
/// Execution reference of a host-local worker: there is no compute fact to join.
pub const HOST_LOCAL_EXECUTION: &str = "host-local";
/// Lattice's `blake3:` identities (definitions, qualification references and
/// session pins) and their lowercase hex digest; a revision is the bare digest.
const IDENTITY_PREFIX: &str = "blake3:";
const DIGEST_HEX: usize = 64;
/// Lattice bounds a declared name set (`MAX_CAPABILITY_NAMES`).
const MAX_CAPABILITY_NAMES: usize = 32;
/// Lattice bounds one definition, task family or tool name (`MAX_NAME_BYTES`).
const MAX_NAME_BYTES: usize = 64;
/// Lattice keeps at most 64 referenced revisions per definition lifecycle.
const MAX_REVISIONS: usize = 64;
/// Lattice keeps at most 256 qualification scopes per definition lifecycle.
const MAX_QUALIFICATION_SCOPES: usize = 256;
/// The rollout generation of a lifecycle that has selected nothing yet.
const NO_ROLLOUT_GENERATION: u64 = 0;

/// Profile facts Lattice does not publish; a definition always lists them as
/// unknown. A qualification reference names an attachment; the subject,
/// corpus, protocol and contracts of a full qualification stay unpublished.
const UNPUBLISHED_PROFILE_FACTS: [&str; 1] = ["qualifications"];
/// Instance facts Lattice reports as owned unknowns.
const UNPUBLISHED_INSTANCE_FACTS: [&str; 2] = ["host_reachability", "allocation_liveness"];
/// Kiln's unknown-field names for the revision slots.
const DESIRED_FIELD: &str = "revisions.desired";
const PREPARED_FIELD: &str = "revisions.prepared";
const QUALIFIED_FIELD: &str = "revisions.qualified";
const SELECTED_FIELD: &str = "revisions.selected_for_new_sessions";
const PINNED_FIELD: &str = "pinned";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Support {
    Declared,
    Qualified,
    Unavailable,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactDto<T> {
    support: Support,
    value: Option<T>,
}

impl<T> FactDto<T> {
    /// The published value, `None` when unavailable or unknown.
    fn published(&self) -> Result<Option<&T>, &'static str> {
        match (self.support, &self.value) {
            (Support::Declared | Support::Qualified, Some(value)) => Ok(Some(value)),
            (Support::Unavailable | Support::Unknown, None) => Ok(None),
            _ => Err("lattice_fact_support_inconsistent"),
        }
    }

    /// Only `task_families` can be qualified; every other fact is declared,
    /// unavailable or unknown.
    fn declared(&self) -> Result<Option<&T>, &'static str> {
        if self.support == Support::Qualified {
            return Err("lattice_fact_support_inconsistent");
        }
        self.published()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProfileOwner {
    ConfiguredRuntime,
    ImplementationAttempt,
}

/// Decoded for exact shape: inventory v2 has no definition name or basis field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum DefinitionBasis {
    Declared,
    OwnerDefault,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionDto {
    identity: String,
    name: String,
    #[allow(dead_code)]
    basis: DefinitionBasis,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RoutesDto {
    NoModel {},
    Routes { routes: Vec<RouteDto> },
}

impl RoutesDto {
    fn names(&self) -> Vec<&str> {
        match self {
            Self::NoModel {} => Vec::new(),
            Self::Routes { routes } => routes.iter().map(|r| r.name.as_str()).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RouteDto {
    name: String,
    route_identity: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ReadinessDto {
    NoServingPort {},
    NoServedRoute {},
    Observed { routes: Vec<RouteReadinessDto> },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteReadinessDto {
    route_name: String,
    service_id: String,
    // Read by the decoder only: Lattice's reading of the serving owner is not
    // a Kiln serving fact. Serving facts come from the serving producer.
    #[allow(dead_code)]
    snapshot: SnapshotReadingDto,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "reading", rename_all = "snake_case", deny_unknown_fields)]
#[allow(dead_code)]
enum SnapshotReadingDto {
    Observed {
        snapshot_identity: String,
        source_generation: u64,
        sequence: u64,
        readiness: String,
        freshness: String,
    },
    Unavailable {},
    PortIssue {},
}

/// One of Lattice's owned unknowns: published as unknown, with the owner
/// that can resolve it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedUnknownDto {
    support: Support,
    owner: String,
}

impl OwnedUnknownDto {
    fn is(&self, owner: &str) -> bool {
        self.support == Support::Unknown && self.owner == owner
    }
}

/// One revision slot: outside the lifecycle, managed but unknown with the
/// owner that can supply it, or the lifecycle's recorded fact.
#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "support", rename_all = "snake_case", deny_unknown_fields)]
enum SlotDto<T> {
    Unmanaged {},
    Unknown { owner: String },
    Recorded { value: T },
}

impl<T> SlotDto<T> {
    const fn recorded(&self) -> Option<&T> {
        match self {
            Self::Recorded { value } => Some(value),
            Self::Unmanaged {} | Self::Unknown { .. } => None,
        }
    }

    fn unknown_owner(&self) -> Option<&str> {
        match self {
            Self::Unknown { owner } => Some(owner),
            Self::Unmanaged {} | Self::Recorded { .. } => None,
        }
    }

    const fn is_unmanaged(&self) -> bool {
        matches!(self, Self::Unmanaged {})
    }

    /// Lattice fixes the owner of each unknown slot.
    fn check_owner(&self, owner: &str) -> Result<(), &'static str> {
        match self.unknown_owner() {
            Some(published) if published != owner => Err("lattice_revision_owner_invalid"),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct DesiredDto {
    revision: String,
    generation: u64,
}

/// The promotion policy's required class; independent classes, not a ladder.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum EvidenceClassDto {
    DeclaredSupport,
    ContractConformance,
    TaskFamilyEvaluation,
    DeploymentAdmission,
}

impl EvidenceClassDto {
    const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredSupport => "declared_support",
            Self::ContractConformance => "contract_conformance",
            Self::TaskFamilyEvaluation => "task_family_evaluation",
            Self::DeploymentAdmission => "deployment_admission",
        }
    }
}

/// The producer's label on the attachment; it never gates anything.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum ProvenanceDto {
    Fixture,
    Live,
}

impl ProvenanceDto {
    const fn map(self) -> EvidenceProvenance {
        match self {
            Self::Fixture => EvidenceProvenance::Fixture,
            Self::Live => EvidenceProvenance::Live,
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct QualifiedDto {
    revision: String,
    task_family: String,
    qualification_ref: String,
    evidence_class: EvidenceClassDto,
    provenance: ProvenanceDto,
    is_revoked: bool,
}

/// Decoded for exact shape; promotion and rollback select alike.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum SelectionKindDto {
    Promotion,
    Rollback,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct SelectedDto {
    revision: String,
    rollout_generation: u64,
    #[allow(dead_code)]
    kind: SelectionKindDto,
    qualification_ref: String,
}

/// The definition's lifecycle: the four profile slots.
#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RevisionSlotsDto {
    desired: SlotDto<DesiredDto>,
    prepared: SlotDto<Vec<String>>,
    qualified: SlotDto<Vec<QualifiedDto>>,
    selected_for_new_sessions: SlotDto<SelectedDto>,
}

impl RevisionSlotsDto {
    /// Every slot is unmanaged, or none is.
    fn management(&self) -> Result<RevisionManagement, &'static str> {
        let unmanaged = [
            self.desired.is_unmanaged(),
            self.prepared.is_unmanaged(),
            self.qualified.is_unmanaged(),
            self.selected_for_new_sessions.is_unmanaged(),
        ];
        if unmanaged.iter().all(|slot| *slot) {
            Ok(RevisionManagement::Unmanaged)
        } else if unmanaged.iter().any(|slot| *slot) {
            Err("lattice_revision_slots_inconsistent")
        } else {
            Ok(RevisionManagement::Managed)
        }
    }

    const fn selection(&self) -> Option<&SelectedDto> {
        self.selected_for_new_sessions.recorded()
    }

    fn qualifications(&self) -> &[QualifiedDto] {
        self.qualified.recorded().map_or(&[], Vec::as_slice)
    }

    /// The recorded qualification of one revision by its reference.
    fn qualification(&self, revision: &str, reference: &str) -> Option<&QualifiedDto> {
        self.qualifications()
            .iter()
            .find(|q| q.revision == revision && q.qualification_ref == reference)
    }

    /// Kiln's field name and the published owner of each unknown slot.
    fn unknown_slots(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            (DESIRED_FIELD, self.desired.unknown_owner()),
            (PREPARED_FIELD, self.prepared.unknown_owner()),
            (QUALIFIED_FIELD, self.qualified.unknown_owner()),
            (
                SELECTED_FIELD,
                self.selected_for_new_sessions.unknown_owner(),
            ),
        ]
        .into_iter()
        .filter_map(|(field, owner)| owner.map(|owner| (field, owner)))
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct CapabilityQualificationDto {
    task_family: String,
    qualification_ref: String,
    evidence_class: EvidenceClassDto,
    provenance: ProvenanceDto,
}

/// `owner` and `serving_readiness` are decoded for exact shape only: the
/// serving owner's readiness is a serving fact, read from the serving producer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct ProfileDto {
    schema: String,
    owner: ProfileOwner,
    definition: DefinitionDto,
    revision: String,
    adapter: FactDto<String>,
    protocol_revision: FactDto<String>,
    executable_identity: FactDto<String>,
    inputs: FactDto<String>,
    outputs: FactDto<String>,
    sessions: FactDto<String>,
    checkpoint: FactDto<String>,
    workspace: FactDto<String>,
    network: FactDto<String>,
    isolation: FactDto<String>,
    deadline_secs: FactDto<u64>,
    max_output_bytes: FactDto<u64>,
    cpu_millis: FactDto<u64>,
    memory_bytes: FactDto<u64>,
    storage_bytes: FactDto<u64>,
    model_routes: FactDto<RoutesDto>,
    compute: FactDto<Value>,
    serving_readiness: ReadinessDto,
    task_families: FactDto<Vec<String>>,
    tools: FactDto<Vec<String>>,
    capability_qualifications: Vec<CapabilityQualificationDto>,
    revisions: RevisionSlotsDto,
    wait_reasons: Vec<ReasonDto>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "compute_kind", rename_all = "snake_case")]
enum ComputeDto {
    HostLocal {},
    ProviderAllocation(Box<AllocationDto>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct AllocationDto {
    allocation_id: String,
    consumer_id: String,
    owner_generation: u64,
    placement: PlacementDto,
    charge_cursor: u64,
    charge_at: u64,
    kind: String,
    view_id: String,
    granularity: String,
    claims: Vec<ClaimDto>,
    shared_serving_ref: Option<String>,
    port_id: String,
    enforcement_at_bind: String,
    admission_sequence: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct PlacementDto {
    host_id: String,
    host_incarnation: String,
    device_incarnation: String,
    topology_generation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct ClaimDto {
    pool_id: String,
    quantity: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum LifecycleDto {
    AdmittedNotDispatched,
    BindingRejectedBeforeDispatch,
    /// The managed session's effect check was blocked right before spawn.
    RevisionBlockedBeforeDispatch,
    DispatchRecordedOutcomeUnknown,
    TerminalObserved {
        #[allow(dead_code)]
        disposition: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CancellationDto {
    NotRequested,
    RequestedUnobserved,
    ObservedTerminated,
}

/// Lattice owns only single-shot attempt workers; none is reused or idles.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RuntimeDto {
    SingleShot,
}

/// `idle` and `live` are not in Lattice's vocabulary and refuse as malformed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum LivenessStateDto {
    NotStarted,
    Exited,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum UnknownCauseDto {
    Unobserved,
    Stale,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LivenessDto {
    // Decoded for exact shape: the only runtime is single-shot.
    #[allow(dead_code)]
    runtime: RuntimeDto,
    state: LivenessStateDto,
    unknown_cause: Option<UnknownCauseDto>,
    /// Retained write-once artifacts plus the terminal result; never decreases
    /// for one attempt, so it orders observations of one incarnation.
    retained_generation: u64,
    observed_at_epoch_ms: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PinStatusDto {
    Live,
    Closed,
    Released,
}

impl PinStatusDto {
    const fn map(self) -> PinStatus {
        match self {
            Self::Live => PinStatus::Live,
            Self::Closed => PinStatus::Closed,
            Self::Released => PinStatus::Released,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PinDto {
    revision: String,
    qualification_ref: String,
    rollout_generation: u64,
    pin_identity: String,
    status: PinStatusDto,
    is_blocked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct InstanceDto {
    schema: String,
    source: String,
    definition_identity: String,
    profile_revision: String,
    pinned: SlotDto<PinDto>,
    work_id: String,
    attempt_id: String,
    incarnation: String,
    fence: u64,
    binding_identity: String,
    compute: ComputeDto,
    model_routes: RoutesDto,
    lifecycle: LifecycleDto,
    cancellation: CancellationDto,
    liveness: LivenessDto,
    host_reachability: OwnedUnknownDto,
    model_readiness: ReadinessDto,
    allocation_liveness: OwnedUnknownDto,
    wait_reasons: Vec<ReasonDto>,
    observed_at_epoch_ms: i64,
    state_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryDto {
    /// Checked by [`SchemaProbe`] before this strict decode.
    #[allow(dead_code)]
    schema: String,
    profile: ProfileDto,
    instance: Option<InstanceDto>,
    /// Per-request usage belongs to Lattice #13 accounting, not the inventory.
    #[allow(dead_code)]
    model_requests: Value,
}

/// Only the schema labels, read before the strict decode so an earlier or
/// later version is refused as unsupported rather than as malformed.
#[derive(Deserialize)]
struct SchemaProbe {
    schema: String,
    profile: SectionProbe,
    instance: Option<SectionProbe>,
}

#[derive(Deserialize)]
struct SectionProbe {
    schema: String,
}

/// 1 to 64 bytes of `[a-z0-9]` followed by `[a-z0-9._:-]`, and a Kiln public token.
fn is_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let lead = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    lead && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._:-".contains(&b))
        && value.len() <= MAX_NAME_BYTES
        && public_token(value)
}

/// A bare revision: 64 lowercase hex characters.
fn is_revision(value: &str) -> bool {
    value.len() == DIGEST_HEX
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A definition identity, qualification reference or pin identity.
fn is_identity(value: &str) -> bool {
    value.strip_prefix(IDENTITY_PREFIX).is_some_and(is_revision)
}

fn ascending<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| match pair {
        [first, second] => first < second,
        _ => false,
    })
}

/// A declared or qualified set holds 1 to 32 sorted, distinct names.
fn validate_capability(fact: &FactDto<Vec<String>>) -> Result<(), &'static str> {
    if let Some(names) = fact.published()? {
        if names.is_empty()
            || names.len() > MAX_CAPABILITY_NAMES
            || !ascending(names)
            || !names.iter().all(|name| is_name(name))
        {
            return Err("lattice_capability_invalid");
        }
    }
    Ok(())
}

/// The slots' shape, owners and bounds, and that a selection rests on a
/// recorded qualification of its revision.
fn validate_revisions(slots: &RevisionSlotsDto) -> Result<RevisionManagement, &'static str> {
    let management = slots.management()?;
    slots.desired.check_owner(wait::REVISION_OWNER)?;
    slots.prepared.check_owner(wait::PREPARATION_OWNER)?;
    slots.qualified.check_owner(wait::QUALIFICATION_OWNER)?;
    slots
        .selected_for_new_sessions
        .check_owner(wait::REVISION_OWNER)?;
    let desired_valid = slots
        .desired
        .recorded()
        .is_none_or(|desired| is_revision(&desired.revision));
    let prepared_valid = slots.prepared.recorded().is_none_or(|prepared| {
        !prepared.is_empty()
            && prepared.len() <= MAX_REVISIONS
            && ascending(prepared)
            && prepared.iter().all(|revision| is_revision(revision))
    });
    let qualified_valid = slots.qualified.recorded().is_none_or(|qualified| {
        let keys: Vec<_> = qualified
            .iter()
            .map(|q| (&q.revision, &q.task_family, &q.qualification_ref))
            .collect();
        !qualified.is_empty()
            && qualified.len() <= MAX_QUALIFICATION_SCOPES
            && ascending(&keys)
            && qualified.iter().all(|q| {
                is_revision(&q.revision)
                    && is_name(&q.task_family)
                    && is_identity(&q.qualification_ref)
            })
    });
    let selected_valid = slots.selection().is_none_or(|selected| {
        is_revision(&selected.revision) && is_identity(&selected.qualification_ref)
    });
    if !(desired_valid && prepared_valid && qualified_valid && selected_valid) {
        return Err("lattice_revision_invalid");
    }
    if let Some(selected) = slots.selection()
        && slots
            .qualification(&selected.revision, &selected.qualification_ref)
            .is_none()
    {
        return Err("lattice_revision_selection_unqualified");
    }
    Ok(management)
}

/// `capability_qualifications` are exactly the unrevoked recorded
/// qualifications of the profile's revision (or empty, when the revision is
/// revoked), and `task_families` is `qualified` exactly when every declared
/// family has one.
fn validate_capability_qualifications(profile: &ProfileDto) -> Result<(), &'static str> {
    let listed = &profile.capability_qualifications;
    let families: Vec<_> = listed.iter().map(|q| &q.task_family).collect();
    if listed.len() > MAX_QUALIFICATION_SCOPES
        || !ascending(&families)
        || !listed
            .iter()
            .all(|q| is_name(&q.task_family) && is_identity(&q.qualification_ref))
    {
        return Err("lattice_capability_qualification_invalid");
    }
    let current: Vec<_> = profile
        .revisions
        .qualifications()
        .iter()
        .filter(|q| q.revision == profile.revision && !q.is_revoked)
        .map(|q| {
            (
                &q.task_family,
                &q.qualification_ref,
                q.evidence_class,
                q.provenance,
            )
        })
        .collect();
    let published: Vec<_> = listed
        .iter()
        .map(|q| {
            (
                &q.task_family,
                &q.qualification_ref,
                q.evidence_class,
                q.provenance,
            )
        })
        .collect();
    if !published.is_empty() && published != current {
        return Err("lattice_capability_qualification_inconsistent");
    }
    let fact = &profile.task_families;
    let all_qualified = fact
        .published()?
        .is_some_and(|names| names.iter().all(|name| families.contains(&name)));
    let consistent = match fact.support {
        Support::Qualified => all_qualified,
        Support::Declared => !all_qualified,
        Support::Unavailable | Support::Unknown => true,
    };
    if consistent {
        Ok(())
    } else {
        Err("lattice_capability_qualification_inconsistent")
    }
}

/// `qualification_unbound` appears exactly when `task_families` is not
/// qualified. `revision_not_admitted` never appears on an unmanaged profile;
/// on a managed one it must appear when another or no revision is selected or
/// the selection's qualification is revoked, and otherwise only for a revoked
/// revision, which has no current qualification.
fn validate_revision_reasons(
    profile: &ProfileDto,
    management: RevisionManagement,
) -> Result<(), &'static str> {
    let reasons = &profile.wait_reasons;
    let unbound = reasons
        .iter()
        .any(|r| r.names(Code::QualificationUnbound, Field::CapabilityQualifications));
    if unbound != (profile.task_families.support != Support::Qualified) {
        return Err("lattice_wait_reason_inconsistent");
    }
    let not_admitted = reasons.iter().any(|r| r.is(Code::RevisionNotAdmitted));
    let slots = &profile.revisions;
    let consistent = match management {
        RevisionManagement::Unmanaged => !not_admitted,
        RevisionManagement::Managed => {
            let selected_here = slots
                .selection()
                .is_some_and(|selected| selected.revision == profile.revision);
            let selection_revoked = slots
                .selection()
                .and_then(|s| slots.qualification(&s.revision, &s.qualification_ref))
                .is_some_and(|q| q.is_revoked);
            let refused = !selected_here || selection_revoked;
            if refused {
                not_admitted
            } else {
                !not_admitted || profile.capability_qualifications.is_empty()
            }
        }
    };
    if consistent {
        Ok(())
    } else {
        Err("lattice_wait_reason_inconsistent")
    }
}

fn validate_profile(profile: &ProfileDto) -> Result<RevisionManagement, &'static str> {
    if !is_identity(&profile.definition.identity) {
        return Err("lattice_definition_identity_invalid");
    }
    if !is_name(&profile.definition.name) {
        return Err("lattice_definition_name_invalid");
    }
    for fact in [
        &profile.adapter,
        &profile.outputs,
        &profile.protocol_revision,
        &profile.executable_identity,
        &profile.inputs,
        &profile.sessions,
        &profile.checkpoint,
        &profile.workspace,
        &profile.network,
        &profile.isolation,
    ] {
        fact.declared()?;
    }
    for fact in [
        &profile.deadline_secs,
        &profile.max_output_bytes,
        &profile.cpu_millis,
        &profile.memory_bytes,
        &profile.storage_bytes,
    ] {
        fact.declared()?;
    }
    profile.compute.declared()?;
    validate_capability(&profile.task_families)?;
    validate_capability(&profile.tools)?;
    if profile.tools.support == Support::Qualified {
        return Err("lattice_fact_support_inconsistent");
    }
    let management = validate_revisions(&profile.revisions)?;
    validate_capability_qualifications(profile)?;
    let routes = profile
        .model_routes
        .declared()?
        .map(RoutesDto::names)
        .unwrap_or_default();
    wait::validate(&profile.wait_reasons, Section::Profile, &routes)?;
    // Lattice reports an undeclared set, and only that, as `capability_undeclared`.
    for (fact, field) in [
        (&profile.task_families, Field::TaskFamilies),
        (&profile.tools, Field::Tools),
    ] {
        let reported = profile
            .wait_reasons
            .iter()
            .any(|r| r.names(Code::CapabilityUndeclared, field));
        if reported != (fact.support == Support::Unknown) {
            return Err("lattice_wait_reason_inconsistent");
        }
    }
    validate_revision_reasons(profile, management)?;
    Ok(management)
}

/// Liveness must be the single-shot state Lattice derives from the lifecycle.
fn lifecycle(instance: &InstanceDto) -> Result<WorkerLifecycle, &'static str> {
    let (expected, mapped) = match instance.lifecycle {
        // A binding is admitted for exactly one attempt; nothing ran yet.
        LifecycleDto::AdmittedNotDispatched => {
            (LivenessStateDto::NotStarted, WorkerLifecycle::Reserved)
        }
        LifecycleDto::BindingRejectedBeforeDispatch
        | LifecycleDto::RevisionBlockedBeforeDispatch => {
            (LivenessStateDto::NotStarted, WorkerLifecycle::Stopped)
        }
        LifecycleDto::TerminalObserved { .. } => {
            (LivenessStateDto::Exited, WorkerLifecycle::Stopped)
        }
        // Dispatch is recorded; whether the worker still runs is unobserved.
        LifecycleDto::DispatchRecordedOutcomeUnknown => {
            (LivenessStateDto::Unknown, WorkerLifecycle::Unknown)
        }
    };
    let liveness = &instance.liveness;
    let cause_consistent =
        (liveness.state == LivenessStateDto::Unknown) == liveness.unknown_cause.is_some();
    if liveness.state != expected || !cause_consistent {
        return Err("lattice_liveness_inconsistent");
    }
    Ok(mapped)
}

/// A recorded pin belongs to a managed definition and to the instance's own
/// revision. A live pin's qualification is recorded (its revision is
/// retained), and a pin whose recorded qualification is revoked is blocked.
fn validate_pin(
    instance: &InstanceDto,
    profile: &ProfileDto,
    management: RevisionManagement,
) -> Result<(), &'static str> {
    instance.pinned.check_owner(wait::REVISION_OWNER)?;
    let Some(pin) = instance.pinned.recorded() else {
        return Ok(());
    };
    if !is_revision(&pin.revision)
        || !is_identity(&pin.qualification_ref)
        || !is_identity(&pin.pin_identity)
    {
        return Err("lattice_revision_invalid");
    }
    let recorded = profile
        .revisions
        .qualification(&pin.revision, &pin.qualification_ref);
    if management != RevisionManagement::Managed
        || pin.revision != instance.profile_revision
        || (pin.status == PinStatusDto::Live && recorded.is_none())
        || (recorded.is_some_and(|q| q.is_revoked) && !pin.is_blocked)
    {
        return Err("lattice_pinned_inconsistent");
    }
    Ok(())
}

fn validate_instance(
    instance: &InstanceDto,
    profile: &ProfileDto,
    management: RevisionManagement,
) -> Result<(), &'static str> {
    if instance.schema != INSTANCE_SCHEMA || instance.source != INSTANCE_STATE_SOURCE {
        return Err("lattice_inventory_unsupported_schema");
    }
    if instance.profile_revision != profile.revision {
        return Err("lattice_instance_profile_mismatch");
    }
    // Lattice states that every instance carries its profile's definition identity.
    if instance.definition_identity != profile.definition.identity {
        return Err("lattice_instance_definition_mismatch");
    }
    if !instance.host_reachability.is(wait::LIVE_HOST_OWNER)
        || !instance.allocation_liveness.is(wait::LIVE_HOST_OWNER)
    {
        return Err("lattice_liveness_unsupported");
    }
    validate_pin(instance, profile, management)?;
    lifecycle(instance)?;
    let reasons = &instance.wait_reasons;
    wait::validate(reasons, Section::Instance, &instance.model_routes.names())?;
    let settled = matches!(
        instance.lifecycle,
        LifecycleDto::TerminalObserved { .. }
            | LifecycleDto::BindingRejectedBeforeDispatch
            | LifecycleDto::RevisionBlockedBeforeDispatch
    );
    let cause = instance.liveness.unknown_cause;
    let reports = |code| reasons.iter().any(|r| r.names(code, Field::Liveness));
    if (settled && !reasons.is_empty())
        || reports(Code::LivenessUnobserved) != (cause == Some(UnknownCauseDto::Unobserved))
        || reports(Code::LivenessStale) != (cause == Some(UnknownCauseDto::Stale))
    {
        return Err("lattice_wait_reason_inconsistent");
    }
    Ok(())
}

fn decode(document: &[u8]) -> Result<(InventoryDto, RevisionManagement), &'static str> {
    if document.len() > MAX_WORKER_INVENTORY_BYTES {
        return Err("lattice_inventory_byte_bound");
    }
    let probe: SchemaProbe =
        serde_json::from_slice(document).map_err(|_| "lattice_inventory_malformed")?;
    if probe.schema != WORKER_INVENTORY_SCHEMA
        || probe.profile.schema != PROFILE_FACTS_SCHEMA
        || probe
            .instance
            .is_some_and(|instance| instance.schema != INSTANCE_SCHEMA)
    {
        return Err("lattice_inventory_unsupported_schema");
    }
    let inventory: InventoryDto =
        serde_json::from_slice(document).map_err(|_| "lattice_inventory_malformed")?;
    let management = validate_profile(&inventory.profile)?;
    if let Some(instance) = &inventory.instance {
        validate_instance(instance, &inventory.profile, management)?;
    }
    Ok((inventory, management))
}

fn meta(source: &SourceObservation, id: &str, observed_at_ms: u64) -> FactMeta {
    FactMeta {
        reference: FactRef {
            source: source.source.clone(),
            id: id.to_owned(),
        },
        observed_at_ms,
        valid_until_ms: source.valid_until_ms,
        unknown_fields: Vec::new(),
    }
}

fn published_list(
    fact: &FactDto<String>,
    name: &str,
    unknown: &mut Vec<String>,
) -> Result<Vec<String>, &'static str> {
    Ok(match fact.published()? {
        Some(value) => vec![value.clone()],
        None => {
            unknown.push(name.to_owned());
            Vec::new()
        }
    })
}

/// Declared or qualified names are known; `unavailable` is a known negative
/// (empty and not unknown); `unknown` is listed as unknown, never as empty.
fn capability(
    fact: &FactDto<Vec<String>>,
    name: &str,
    unknown: &mut Vec<String>,
) -> Result<Vec<String>, &'static str> {
    Ok(match fact.published()? {
        Some(names) => names.clone(),
        None => {
            if fact.support == Support::Unknown {
                unknown.push(name.to_owned());
            }
            Vec::new()
        }
    })
}

/// The lifecycle onto Kiln's single-revision slots. `prepared` and
/// `qualified` answer for the profile's own revision; a revoked revision has
/// no current qualification, so `capability_qualifications` decides
/// `qualified`. Unmanaged slots stay empty and known.
fn revision_state(profile: &ProfileDto, management: RevisionManagement) -> RevisionState {
    let slots = &profile.revisions;
    let selection = slots.selection();
    let current = &profile.capability_qualifications;
    RevisionState {
        desired: slots.desired.recorded().map(|d| d.revision.clone()),
        prepared: slots
            .prepared
            .recorded()
            .filter(|prepared| prepared.contains(&profile.revision))
            .map(|_| profile.revision.clone()),
        qualified: (!current.is_empty()).then(|| profile.revision.clone()),
        selected_for_new_sessions: selection.map(|s| s.revision.clone()),
        // Lattice publishes no failed preparation in the inventory.
        preparation_failure: None,
        generation: selection.map_or(NO_ROLLOUT_GENERATION, |s| s.rollout_generation),
        evidence: current
            .iter()
            .map(|q| q.qualification_ref.clone())
            .collect(),
        management: Some(management),
    }
}

fn definition(
    profile: &ProfileDto,
    management: RevisionManagement,
    source: &SourceObservation,
) -> Result<AgentDefinition, &'static str> {
    let mut meta = meta(source, &profile.definition.identity, source.observed_at_ms);
    let unknown = &mut meta.unknown_fields;
    unknown.extend(
        UNPUBLISHED_PROFILE_FACTS
            .iter()
            .map(|name| (*name).to_owned()),
    );
    let output_contracts = published_list(&profile.outputs, "output_contracts", unknown)?;
    let harnesses = published_list(&profile.adapter, "harnesses", unknown)?;
    let task_families = capability(&profile.task_families, "task_families", unknown)?;
    let tools = capability(&profile.tools, "tools", unknown)?;
    // A route identity names a broker route, not a permitted model; only a
    // no-model profile states its permitted models exactly (none).
    let permitted_models = match profile.model_routes.published()? {
        Some(RoutesDto::NoModel {}) => Vec::new(),
        Some(RoutesDto::Routes { .. }) | None => {
            unknown.push("permitted_models".to_owned());
            Vec::new()
        }
    };
    // A profile's serving reasons describe one inspection of the serving
    // owner, not the definition; serving facts come from the serving producer.
    let mut wait_reasons: Vec<WaitReason> = profile
        .wait_reasons
        .iter()
        .filter(|reason| reason.code.is_definition_scoped())
        .map(wait::map)
        .collect();
    // A managed fact still unknown keeps the owner Lattice published for it.
    for (field, owner) in profile.revisions.unknown_slots() {
        unknown.push(field.to_owned());
        wait_reasons.push(wait::revision_fact_unknown(field, owner));
    }
    unknown.sort();
    Ok(AgentDefinition {
        meta,
        profile_revision: profile.revision.clone(),
        task_families,
        output_contracts,
        harnesses,
        tools,
        permitted_models,
        // Lattice publishes no admission authority for a profile.
        authority: Permission::Unknown,
        revisions: revision_state(profile, management),
        qualifications: Vec::new(),
        wait_reasons,
        task_family_qualifications: profile
            .capability_qualifications
            .iter()
            .map(|q| TaskFamilyQualification {
                task_family: q.task_family.clone(),
                reference: q.qualification_ref.clone(),
                evidence_class: q.evidence_class.as_str().to_owned(),
                provenance: q.provenance.map(),
            })
            .collect(),
    })
}

fn worker(
    instance: &InstanceDto,
    source: &SourceObservation,
    joins: &InventoryJoins,
) -> Result<WorkerInstance, &'static str> {
    let observed_at_ms = u64::try_from(instance.observed_at_epoch_ms)
        .map_err(|_| "lattice_instance_time_invalid")?;
    let liveness_at_ms = u64::try_from(instance.liveness.observed_at_epoch_ms)
        .map_err(|_| "lattice_instance_time_invalid")?;
    if observed_at_ms > source.observed_at_ms || liveness_at_ms > source.observed_at_ms {
        return Err("lattice_instance_after_collection");
    }
    let lifecycle = lifecycle(instance)?;
    let mut meta = meta(source, &instance.incarnation, observed_at_ms);
    let unknown = &mut meta.unknown_fields;
    unknown.extend(
        UNPUBLISHED_INSTANCE_FACTS
            .iter()
            .map(|name| (*name).to_owned()),
    );
    // Unobserved or stale liveness is neither failure nor idle.
    if instance.liveness.state == LivenessStateDto::Unknown {
        unknown.push("liveness".to_owned());
    }
    let execution = match &instance.compute {
        ComputeDto::ProviderAllocation(allocation) => FactRef {
            source: joins.compute.clone(),
            id: allocation.allocation_id.clone(),
        },
        ComputeDto::HostLocal {} => {
            unknown.push("execution".to_owned());
            FactRef {
                source: source.source.clone(),
                id: HOST_LOCAL_EXECUTION.to_owned(),
            }
        }
    };
    let mut model_services = Vec::new();
    match (&instance.model_routes, &instance.model_readiness) {
        (RoutesDto::NoModel {}, _) => {}
        (RoutesDto::Routes { .. }, ReadinessDto::Observed { routes }) => {
            for route in routes {
                let reference = FactRef {
                    source: joins.serving.clone(),
                    id: route.service_id.clone(),
                };
                if !model_services.contains(&reference) {
                    model_services.push(reference);
                }
                debug_assert!(!route.route_name.is_empty());
            }
        }
        (RoutesDto::Routes { .. }, _) => unknown.push("model_services".to_owned()),
    }
    model_services.sort();
    if matches!(instance.cancellation, CancellationDto::RequestedUnobserved) {
        unknown.push("cancellation".to_owned());
    }
    let mut wait_reasons: Vec<WaitReason> = instance.wait_reasons.iter().map(wait::map).collect();
    let pin = match &instance.pinned {
        SlotDto::Unmanaged {} => Some(RevisionPin::Unmanaged {}),
        SlotDto::Unknown { owner } => {
            unknown.push(PINNED_FIELD.to_owned());
            wait_reasons.push(wait::revision_fact_unknown(PINNED_FIELD, owner));
            None
        }
        SlotDto::Recorded { value } => Some(RevisionPin::Recorded {
            qualification: value.qualification_ref.clone(),
            rollout_generation: value.rollout_generation,
            identity: value.pin_identity.clone(),
            status: value.status.map(),
            blocked: value.is_blocked,
        }),
    };
    unknown.sort();
    Ok(WorkerInstance {
        meta,
        incarnation: instance.incarnation.clone(),
        // Lattice attempt workers are single-shot and keep no session.
        session: None,
        definition: FactRef {
            source: joins.agents.clone(),
            id: instance.definition_identity.clone(),
        },
        // A recorded pin's revision is this profile revision.
        pinned_revision: instance.profile_revision.clone(),
        attempt: Some(instance.attempt_id.clone()),
        fence: Some(instance.fence),
        execution,
        // Lattice never projects grant references.
        execution_grant: None,
        // Workspace paths are restricted and never projected.
        workspace: None,
        model_services,
        lifecycle,
        wait_reasons,
        pin,
    })
}

/// Every document of one definition in one source describes one lifecycle.
/// Two different recorded selections are two selected revisions.
fn agree(
    lifecycles: &mut BTreeMap<String, RevisionSlotsDto>,
    definition: String,
    slots: RevisionSlotsDto,
) -> Result<(), &'static str> {
    match lifecycles.entry(definition) {
        Entry::Vacant(entry) => {
            entry.insert(slots);
            Ok(())
        }
        Entry::Occupied(entry) => {
            let existing = entry.get();
            if *existing == slots {
                Ok(())
            } else if existing
                .selection()
                .zip(slots.selection())
                .is_some_and(|(first, second)| first.revision != second.revision)
            {
                Err("lattice_definition_selection_conflict")
            } else {
                Err("lattice_definition_lifecycle_conflict")
            }
        }
    }
}

/// The definition at its only revision, or at the selected one when several
/// revisions appear. Without a selection among them the choice is ambiguous.
fn project(
    mut revisions: BTreeMap<String, AgentDefinition>,
    lifecycle: Option<&RevisionSlotsDto>,
) -> Result<AgentDefinition, &'static str> {
    if revisions.len() == 1 {
        return revisions
            .into_values()
            .next()
            .ok_or("lattice_definition_revision_ambiguous");
    }
    lifecycle
        .and_then(RevisionSlotsDto::selection)
        .and_then(|selected| revisions.remove(&selected.revision))
        .ok_or("lattice_definition_revision_ambiguous")
}

/// Map one source's Lattice inventory documents. A `kind: agents` source yields
/// one definition per definition identity, at its only revision or at the
/// revision its lifecycle selects for new sessions; a `kind: workers` source
/// yields one worker per retained incarnation, whatever revision it pins.
/// Repeated identical facts collapse, and all documents of one definition
/// must carry one lifecycle. Observations of one incarnation are ordered by
/// retained generation; equal generations must agree.
pub fn map_worker_inventory(
    kind: InventoryKind,
    source: &SourceObservation,
    joins: &InventoryJoins,
    documents: &[&[u8]],
) -> Result<InventoryFragment, &'static str> {
    joins.admit()?;
    if documents.len() > MAX_INVENTORY_ITEMS {
        return Err("lattice_inventory_document_bound");
    }
    let mut fragment = InventoryFragment::new(kind, source.clone());
    let mut lifecycles: BTreeMap<String, RevisionSlotsDto> = BTreeMap::new();
    let mut definitions: BTreeMap<String, BTreeMap<String, AgentDefinition>> = BTreeMap::new();
    let mut workers: BTreeMap<String, (u64, WorkerInstance)> = BTreeMap::new();
    for document in documents {
        let (inventory, management) = decode(document)?;
        let InventoryDto {
            profile, instance, ..
        } = inventory;
        let mapped = match kind {
            InventoryKind::Agents => Some(Mapped::Definition(definition(
                &profile, management, source,
            )?)),
            InventoryKind::Workers => match &instance {
                Some(instance) => Some(Mapped::Worker(
                    instance.liveness.retained_generation,
                    worker(instance, source, joins)?,
                )),
                None => None,
            },
            InventoryKind::Compute
            | InventoryKind::Serving
            | InventoryKind::Policy
            | InventoryKind::WorkflowRevisions => {
                return Err("lattice_inventory_kind_unsupported");
            }
        };
        // A contradictory lifecycle refuses before any fact of this document
        // is compared with another.
        agree(
            &mut lifecycles,
            profile.definition.identity,
            profile.revisions,
        )?;
        match mapped {
            Some(Mapped::Definition(value)) => insert_definition(&mut definitions, value)?,
            Some(Mapped::Worker(generation, value)) => {
                insert_worker(&mut workers, generation, value)?;
            }
            None => {}
        }
    }
    fragment.agents = definitions
        .into_iter()
        .map(|(identity, revisions)| project(revisions, lifecycles.get(&identity)))
        .collect::<Result<_, _>>()?;
    fragment.workers = workers.into_values().map(|(_, worker)| worker).collect();
    Ok(fragment)
}

/// One document's fact for the source's kind.
enum Mapped {
    Definition(AgentDefinition),
    Worker(u64, WorkerInstance),
}

/// Profiles of one definition at one revision must agree.
fn insert_definition(
    definitions: &mut BTreeMap<String, BTreeMap<String, AgentDefinition>>,
    value: AgentDefinition,
) -> Result<(), &'static str> {
    let revisions = definitions
        .entry(value.meta.reference.id.clone())
        .or_default();
    match revisions.entry(value.profile_revision.clone()) {
        Entry::Vacant(slot) => {
            slot.insert(value);
            Ok(())
        }
        Entry::Occupied(slot) if *slot.get() == value => Ok(()),
        Entry::Occupied(_) => Err("lattice_profile_conflict"),
    }
}

/// The greater retained generation of one incarnation wins in any order;
/// equal generations must agree.
fn insert_worker(
    workers: &mut BTreeMap<String, (u64, WorkerInstance)>,
    generation: u64,
    value: WorkerInstance,
) -> Result<(), &'static str> {
    match workers.entry(value.incarnation.clone()) {
        Entry::Vacant(slot) => {
            slot.insert((generation, value));
        }
        Entry::Occupied(mut slot) => {
            let (existing_generation, existing) = slot.get();
            if generation > *existing_generation {
                slot.insert((generation, value));
            } else if generation == *existing_generation && *existing != value {
                return Err("lattice_instance_conflict");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "worker_inventory_tests.rs"]
mod tests;
