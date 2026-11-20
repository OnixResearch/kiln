//! Lattice `lattice.worker-inventory.v2` documents (the output of
//! `lattice implementation-attempt inspect`, `docs/worker-inventory.md` in
//! Lattice) mapped into inventory v2 agent definitions and worker instances.
//!
//! The DTOs mirror Lattice's wire record field for field and deny unknown
//! fields; Kiln owns them so no Lattice code enters Kiln. v2 is the only
//! accepted version: a v1 document is refused as unsupported. A profile is a
//! definition keyed by its definition identity, never a worker. Lattice's
//! single-shot liveness is never idle or live, so no lifecycle maps to idle,
//! healthy or executing. Declared capability is known, unavailable capability
//! is a known negative, and every v2 fact Lattice does not publish is listed
//! as unknown. Wait reasons keep their exact code, subject and owner.
// r[impl kiln.inventory.producers]
// r[impl kiln.inventory.producers.scenario.lattice]
// r[impl kiln.inventory.producers.scenario.capability]
// r[impl kiln.inventory.producers.scenario.refused]
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use kiln_core::inventory::{
    AgentDefinition, FactMeta, FactRef, InventoryFragment, InventoryJoins, InventoryKind,
    MAX_INVENTORY_ITEMS, Permission, RevisionState, SourceObservation, WorkerInstance,
    WorkerLifecycle, public_token,
};
use serde::Deserialize;
use serde_json::Value;

#[path = "worker_inventory_wait.rs"]
mod wait;

use wait::{Code, Field, ReasonDto, Section};

pub const WORKER_INVENTORY_SCHEMA: &str = "lattice.worker-inventory.v2";
pub const PROFILE_FACTS_SCHEMA: &str = "lattice.worker-profile-facts.v2";
pub const INSTANCE_SCHEMA: &str = "lattice.worker-instance-observation.v2";
pub const INSTANCE_STATE_SOURCE: &str = "lattice.implementation-attempt-state.v1";
/// Lattice bounds one inspected artifact to 256 KiB (`MAX_SMALL_ARTIFACT_BYTES`).
pub const MAX_WORKER_INVENTORY_BYTES: usize = 262_144;
/// Execution reference of a host-local worker: there is no compute fact to join.
pub const HOST_LOCAL_EXECUTION: &str = "host-local";
/// Lattice's definition identity: `blake3:` and a lowercase hex digest.
const DEFINITION_IDENTITY_PREFIX: &str = "blake3:";
const DEFINITION_DIGEST_HEX: usize = 64;
/// Lattice bounds a declared name set (`MAX_CAPABILITY_NAMES`).
const MAX_CAPABILITY_NAMES: usize = 32;
/// Lattice bounds one definition, task family or tool name (`MAX_NAME_BYTES`).
const MAX_NAME_BYTES: usize = 64;

/// Profile facts Lattice does not publish; a definition always lists them as unknown.
/// Revisions wait on Lattice #25; per-task-family qualifications on Animus #18.
const UNPUBLISHED_PROFILE_FACTS: [&str; 2] = ["revisions", "qualifications"];
/// Instance facts Lattice reports as owned unknowns.
const UNPUBLISHED_INSTANCE_FACTS: [&str; 2] = ["host_reachability", "allocation_liveness"];

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

/// The five Lattice #25 revision slots, all unpublished.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionSlotsDto {
    desired: OwnedUnknownDto,
    prepared: OwnedUnknownDto,
    qualified: OwnedUnknownDto,
    selected_for_new_sessions: OwnedUnknownDto,
    pinned: OwnedUnknownDto,
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
    qualification_ref: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct InstanceDto {
    schema: String,
    source: String,
    definition_identity: String,
    profile_revision: String,
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

fn is_definition_identity(value: &str) -> bool {
    value
        .strip_prefix(DEFINITION_IDENTITY_PREFIX)
        .is_some_and(|digest| {
            digest.len() == DEFINITION_DIGEST_HEX
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// A declared or qualified set holds 1 to 32 sorted, distinct names.
fn validate_capability(fact: &FactDto<Vec<String>>) -> Result<(), &'static str> {
    if let Some(names) = fact.published()? {
        let sorted = names.windows(2).all(|pair| match pair {
            [first, second] => first < second,
            _ => false,
        });
        if names.is_empty()
            || names.len() > MAX_CAPABILITY_NAMES
            || !sorted
            || !names.iter().all(|name| is_name(name))
        {
            return Err("lattice_capability_invalid");
        }
    }
    Ok(())
}

fn validate_profile(profile: &ProfileDto) -> Result<(), &'static str> {
    if !is_definition_identity(&profile.definition.identity) {
        return Err("lattice_definition_identity_invalid");
    }
    if !is_name(&profile.definition.name) {
        return Err("lattice_definition_name_invalid");
    }
    let slots = &profile.revisions;
    if ![
        &slots.desired,
        &slots.prepared,
        &slots.qualified,
        &slots.selected_for_new_sessions,
        &slots.pinned,
    ]
    .iter()
    .all(|slot| slot.is(wait::REVISION_OWNER))
    {
        return Err("lattice_revision_unsupported");
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
        fact.published()?;
    }
    for fact in [
        &profile.deadline_secs,
        &profile.max_output_bytes,
        &profile.cpu_millis,
        &profile.memory_bytes,
        &profile.storage_bytes,
    ] {
        fact.published()?;
    }
    profile.compute.published()?;
    validate_capability(&profile.task_families)?;
    validate_capability(&profile.tools)?;
    let routes = profile
        .model_routes
        .published()?
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
    Ok(())
}

/// Liveness must be the single-shot state Lattice derives from the lifecycle.
fn lifecycle(instance: &InstanceDto) -> Result<WorkerLifecycle, &'static str> {
    let (expected, mapped) = match instance.lifecycle {
        // A binding is admitted for exactly one attempt; nothing ran yet.
        LifecycleDto::AdmittedNotDispatched => {
            (LivenessStateDto::NotStarted, WorkerLifecycle::Reserved)
        }
        LifecycleDto::BindingRejectedBeforeDispatch => {
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

fn validate_instance(instance: &InstanceDto, profile: &ProfileDto) -> Result<(), &'static str> {
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
    lifecycle(instance)?;
    let reasons = &instance.wait_reasons;
    wait::validate(reasons, Section::Instance, &instance.model_routes.names())?;
    let settled = matches!(
        instance.lifecycle,
        LifecycleDto::TerminalObserved { .. } | LifecycleDto::BindingRejectedBeforeDispatch
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

fn decode(document: &[u8]) -> Result<InventoryDto, &'static str> {
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
    validate_profile(&inventory.profile)?;
    if let Some(instance) = &inventory.instance {
        validate_instance(instance, &inventory.profile)?;
    }
    Ok(inventory)
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

fn definition(
    profile: &ProfileDto,
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
    // Lattice publishes `qualification_ref` only with bound evidence; whether
    // it applies to a task family stays unknown.
    let qualified = profile.qualification_ref.as_ref();
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
        // The #25 slots are unpublished owned unknowns: never empty, never the
        // current revision. Their owner is the `revision_selection_unpublished` reason.
        revisions: RevisionState {
            desired: None,
            prepared: None,
            qualified: qualified.map(|_| profile.revision.clone()),
            selected_for_new_sessions: None,
            preparation_failure: None,
            generation: source.generation,
            evidence: qualified.into_iter().cloned().collect(),
        },
        qualifications: Vec::new(),
        // A profile's serving reasons describe one inspection of the serving
        // owner, not the definition; serving facts come from the serving producer.
        wait_reasons: profile
            .wait_reasons
            .iter()
            .filter(|reason| reason.code.is_definition_scoped())
            .map(wait::map)
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
        wait_reasons: instance.wait_reasons.iter().map(wait::map).collect(),
    })
}

/// Map one source's Lattice inventory documents. A `kind: agents` source yields
/// one definition per definition identity; a `kind: workers` source yields one
/// worker per retained incarnation. Repeated identical facts collapse. One
/// definition at two revisions refuses, because the revision selected for new
/// sessions is unpublished (Lattice #25). Observations of one incarnation are
/// ordered by retained generation; equal generations must agree.
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
    let mut definitions: BTreeMap<String, AgentDefinition> = BTreeMap::new();
    let mut workers: BTreeMap<String, (u64, WorkerInstance)> = BTreeMap::new();
    for document in documents {
        let inventory = decode(document)?;
        match kind {
            InventoryKind::Agents => {
                let value = definition(&inventory.profile, source)?;
                match definitions.entry(value.meta.reference.id.clone()) {
                    Entry::Vacant(slot) => {
                        slot.insert(value);
                    }
                    Entry::Occupied(slot) => {
                        let existing = slot.get();
                        if existing.profile_revision != value.profile_revision {
                            return Err("lattice_definition_revision_ambiguous");
                        }
                        if *existing != value {
                            return Err("lattice_profile_conflict");
                        }
                    }
                }
            }
            InventoryKind::Workers => {
                if let Some(instance) = &inventory.instance {
                    let generation = instance.liveness.retained_generation;
                    let value = worker(instance, source, joins)?;
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
                }
            }
            InventoryKind::Compute | InventoryKind::Serving | InventoryKind::Policy => {
                return Err("lattice_inventory_kind_unsupported");
            }
        }
    }
    fragment.agents = definitions.into_values().collect();
    fragment.workers = workers.into_values().map(|(_, worker)| worker).collect();
    Ok(fragment)
}

#[cfg(test)]
#[path = "worker_inventory_tests.rs"]
mod tests;
