//! Lattice `lattice.worker-inventory.v1` documents (the output of
//! `lattice implementation-attempt inspect`) mapped into inventory v2 agent
//! definitions and worker instances.
//!
//! The DTOs mirror Lattice's wire record field for field and deny unknown
//! fields; Kiln owns them so no Lattice code enters Kiln. A profile is a
//! definition, never a worker. Lattice never reports idle, running, healthy or
//! available, so no lifecycle maps to those states, and every v2 fact Lattice
//! does not publish is listed as unknown.
// r[impl kiln.inventory.producers]
// r[impl kiln.inventory.producers.scenario.lattice]
use std::collections::BTreeMap;

use kiln_core::inventory::{
    AgentDefinition, FactMeta, FactRef, InventoryFragment, InventoryJoins, InventoryKind,
    MAX_INVENTORY_ITEMS, Permission, RevisionState, SourceObservation, WorkerInstance,
    WorkerLifecycle,
};
use serde::Deserialize;
use serde_json::Value;

pub const WORKER_INVENTORY_SCHEMA: &str = "lattice.worker-inventory.v1";
pub const PROFILE_FACTS_SCHEMA: &str = "lattice.worker-profile-facts.v1";
pub const INSTANCE_SCHEMA: &str = "lattice.worker-instance-observation.v1";
pub const INSTANCE_STATE_SOURCE: &str = "lattice.implementation-attempt-state.v1";
/// Lattice bounds one inspected artifact to 256 KiB (`MAX_SMALL_ARTIFACT_BYTES`).
pub const MAX_WORKER_INVENTORY_BYTES: usize = 262_144;
/// Execution reference of a host-local worker: there is no compute fact to join.
pub const HOST_LOCAL_EXECUTION: &str = "host-local";

/// Profile facts Lattice never publishes; a definition always lists them as unknown.
const UNPUBLISHED_PROFILE_FACTS: [&str; 4] =
    ["task_families", "tools", "revisions", "qualifications"];
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

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RoutesDto {
    NoModel {},
    Routes { routes: Vec<RouteDto> },
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

/// `owner` and `serving_readiness` are decoded for exact shape only: the
/// serving owner's readiness is a serving fact, read from the serving producer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct ProfileDto {
    schema: String,
    owner: ProfileOwner,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedUnknownDto {
    support: Support,
    #[allow(dead_code)]
    owner: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct InstanceDto {
    schema: String,
    source: String,
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
    host_reachability: OwnedUnknownDto,
    model_readiness: ReadinessDto,
    allocation_liveness: OwnedUnknownDto,
    observed_at_epoch_ms: i64,
    state_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryDto {
    schema: String,
    profile: ProfileDto,
    instance: Option<InstanceDto>,
    /// Per-request usage belongs to Lattice #13 accounting, not the inventory.
    #[allow(dead_code)]
    model_requests: Value,
}

fn decode(document: &[u8]) -> Result<InventoryDto, &'static str> {
    if document.len() > MAX_WORKER_INVENTORY_BYTES {
        return Err("lattice_inventory_byte_bound");
    }
    let inventory: InventoryDto =
        serde_json::from_slice(document).map_err(|_| "lattice_inventory_malformed")?;
    if inventory.schema != WORKER_INVENTORY_SCHEMA
        || inventory.profile.schema != PROFILE_FACTS_SCHEMA
    {
        return Err("lattice_inventory_unsupported_schema");
    }
    if let Some(instance) = &inventory.instance {
        if instance.schema != INSTANCE_SCHEMA || instance.source != INSTANCE_STATE_SOURCE {
            return Err("lattice_inventory_unsupported_schema");
        }
        if instance.profile_revision != inventory.profile.revision {
            return Err("lattice_instance_profile_mismatch");
        }
        if instance.host_reachability.support != Support::Unknown
            || instance.allocation_liveness.support != Support::Unknown
        {
            return Err("lattice_liveness_unsupported");
        }
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

fn definition(
    profile: &ProfileDto,
    source: &SourceObservation,
) -> Result<AgentDefinition, &'static str> {
    for fact in [
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
    let mut meta = meta(source, &profile.revision, source.observed_at_ms);
    let unknown = &mut meta.unknown_fields;
    unknown.extend(
        UNPUBLISHED_PROFILE_FACTS
            .iter()
            .map(|name| (*name).to_owned()),
    );
    let output_contracts = published_list(&profile.outputs, "output_contracts", unknown)?;
    let harnesses = published_list(&profile.adapter, "harnesses", unknown)?;
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
        task_families: Vec::new(),
        output_contracts,
        harnesses,
        tools: Vec::new(),
        permitted_models,
        // Lattice publishes no admission authority for a profile.
        authority: Permission::Unknown,
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
    })
}

fn lifecycle(value: &LifecycleDto) -> WorkerLifecycle {
    match value {
        // A binding is admitted for exactly one attempt; nothing ran yet.
        LifecycleDto::AdmittedNotDispatched => WorkerLifecycle::Reserved,
        LifecycleDto::BindingRejectedBeforeDispatch | LifecycleDto::TerminalObserved { .. } => {
            WorkerLifecycle::Stopped
        }
        // Dispatch is recorded; whether the worker still runs is unobserved.
        LifecycleDto::DispatchRecordedOutcomeUnknown => WorkerLifecycle::Unknown,
    }
}

fn worker(
    instance: &InstanceDto,
    source: &SourceObservation,
    joins: &InventoryJoins,
) -> Result<WorkerInstance, &'static str> {
    let observed_at_ms = u64::try_from(instance.observed_at_epoch_ms)
        .map_err(|_| "lattice_instance_time_invalid")?;
    if observed_at_ms > source.observed_at_ms {
        return Err("lattice_instance_after_collection");
    }
    let mut meta = meta(source, &instance.incarnation, observed_at_ms);
    let unknown = &mut meta.unknown_fields;
    unknown.extend(
        UNPUBLISHED_INSTANCE_FACTS
            .iter()
            .map(|name| (*name).to_owned()),
    );
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
            id: instance.profile_revision.clone(),
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
        lifecycle: lifecycle(&instance.lifecycle),
    })
}

/// Map one source's Lattice inventory documents. A `kind: agents` source yields
/// one definition per distinct profile revision; a `kind: workers` source
/// yields one worker per retained instance. Repeated identical facts collapse;
/// conflicting facts for one identity refuse.
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
    let mut definitions = BTreeMap::new();
    let mut workers = BTreeMap::new();
    for document in documents {
        let inventory = decode(document)?;
        match kind {
            InventoryKind::Agents => {
                let value = definition(&inventory.profile, source)?;
                if let Some(existing) =
                    definitions.insert(value.profile_revision.clone(), value.clone())
                    && existing != value
                {
                    return Err("lattice_profile_conflict");
                }
            }
            InventoryKind::Workers => {
                if let Some(instance) = &inventory.instance {
                    let value = worker(instance, source, joins)?;
                    if let Some(existing) = workers.insert(value.incarnation.clone(), value.clone())
                        && existing != value
                    {
                        return Err("lattice_instance_conflict");
                    }
                }
            }
            InventoryKind::Compute | InventoryKind::Serving | InventoryKind::Policy => {
                return Err("lattice_inventory_kind_unsupported");
            }
        }
    }
    fragment.agents = definitions.into_values().collect();
    fragment.workers = workers.into_values().collect();
    Ok(fragment)
}

#[cfg(test)]
#[path = "worker_inventory_tests.rs"]
mod tests;
