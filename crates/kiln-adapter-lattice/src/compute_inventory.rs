//! Compute allocation observations of Aspen/Molten #42 mapped into inventory
//! v2 compute facts and pools.
//!
//! The accepted document is `lattice.compute-allocation-observation.v1`, the
//! published consumer DTO over the producer's `compute-capacity-allocation-v2`,
//! `compute-capacity-readback-v1` and `compute-capacity-offer-v2` records. The
//! DTOs mirror it field for field, deny unknown fields and enforce the same
//! structural rules. Kiln never reserves, activates or probes compute here.
//!
//! Each non-released allocation is one compute fact keyed by its allocation
//! id. Each claimed pool is one pool fact keyed by its pool id and accounted by
//! one producer authority, so a pool shared by execution and serving is one
//! capacity and a serving deployment's claims are counted once however many
//! workers reference it. The observation carries no pool capacity, so
//! capacity, allocatable and reserved amounts stay unknown.
// r[impl kiln.inventory.producers]
// r[impl kiln.inventory.producers.scenario.compute]
use std::collections::{BTreeMap, BTreeSet};

use kiln_core::inventory::{
    ComputeDevice, FactMeta, FactRef, Health, InventoryFragment, InventoryKind,
    MAX_INVENTORY_ITEMS, PoolAllocation, ResourcePool, SourceObservation, public_token,
};
use serde::Deserialize;

pub const COMPUTE_OBSERVATION_SCHEMA: &str = "lattice.compute-allocation-observation.v1";
pub const ALLOCATION_LABEL: &str = "compute-capacity-allocation-v2";
pub const READBACK_LABEL: &str = "compute-capacity-readback-v1";
pub const OFFER_LABEL: &str = "compute-capacity-offer-v2";
pub const RESERVATION_SCHEMA: &str = "molten.compute.reservation.v1";
pub const OFFER_SCHEMA: &str = "molten.compute.offer.v1";
pub const TIME_DOMAIN: &str = "logical";
/// The consumer DTO's serialized bound.
pub const MAX_COMPUTE_OBSERVATION_BYTES: usize = 65_536;
/// Producer list bound (`molten_core::compute_capacity::MAX_ITEMS`).
pub const MAX_COMPUTE_ITEMS: usize = 128;
/// Producer token bound (`molten_core::compute_capacity::MAX_TEXT`).
const MAX_TOKEN_BYTES: usize = 256;
const REFERENCE_PREFIX: &str = "blake3:";
const REFERENCE_HEX_BYTES: usize = 64;
pub const EXECUTION_ALLOCATION_KIND: &str = "execution-allocation";
pub const SERVING_ALLOCATION_KIND: &str = "serving-deployment-allocation";
/// Pool amounts the observation never carries.
const UNPUBLISHED_POOL_FACTS: [&str; 3] = ["allocatable", "capacity", "reserved"];

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ObservationDto {
    schema: String,
    producer: ProducerDto,
    port: PortDto,
    allocation: AllocationDto,
    readback: ReadbackDto,
    offer: Option<OfferDto>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProducerDto {
    allocation_label: String,
    readback_label: String,
    offer_label: String,
    reservation_schema: String,
    offer_schema: String,
    time_domain: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PortDto {
    port_id: String,
    port_profile_ref: String,
    observation_sequence: u64,
    provider_logical_now: Option<u64>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AllocationDto {
    record_ref: String,
    schema: String,
    id: String,
    host_id: String,
    host_incarnation: String,
    device_incarnation: String,
    offer_revision: u64,
    topology_generation: u64,
    charge_cursor: u64,
    charge_at: u64,
    valid_until: u64,
    authority_ids: Vec<String>,
    view_id: String,
    granularity: Granularity,
    consumer_id: String,
    operation_id: String,
    attempt_id: String,
    owner_generation: u64,
    grant_ref: String,
    claims: Vec<ClaimDto>,
    serving_allocation_id: Option<String>,
    kind: AllocationKind,
    status: Status,
    enforcement: Enforcement,
    activation_backend_ref: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ClaimDto {
    pool_id: String,
    quantity: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ReadbackDto {
    record_ref: String,
    host_id: String,
    revision: u64,
    is_fresh: bool,
    health: ProducerHealth,
    qualification_refs: Vec<String>,
    row_count: u64,
    row: RowDto,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RowDto {
    id: String,
    consumer_id: String,
    owner_generation: u64,
    granularity: Granularity,
    status: Status,
    is_logically_fenced: bool,
    enforcement: Enforcement,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OfferDto {
    record_ref: String,
    host_id: String,
    host_incarnation: String,
    device_incarnation: String,
    revision: u64,
    topology_generation: u64,
    health: ProducerHealth,
    label_authority: LabelAuthority,
    compatibility: CompatibilityDto,
    claimed_pools: Vec<ClaimedPoolDto>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CompatibilityDto {
    backend: Option<String>,
    runtime: Option<String>,
    architecture: Option<String>,
    topology: Option<String>,
    qualification_refs: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ClaimedPoolDto {
    pool_id: String,
    kind: PoolKind,
    authority_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Granularity {
    Chip,
    MmioBundle,
    Tray,
    Mesh,
    Partition,
    Alias,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum AllocationKind {
    Execution,
    ServingDeployment,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Intent,
    Active,
    Quarantined,
    Released,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Enforcement {
    LogicalFenceOnly,
    BackendAcknowledged,
    BackendFenced,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ProducerHealth {
    Healthy,
    Degraded,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum LabelAuthority {
    Advisory,
    Measured,
    OperatorDeclared,
    Authoritative,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum PoolKind {
    DedicatedMemoryBytes,
    UnifiedMemoryBytes,
    Chips,
}

impl PoolKind {
    const fn unit(self) -> &'static str {
        match self {
            Self::DedicatedMemoryBytes => "dedicated-memory-bytes",
            Self::UnifiedMemoryBytes => "unified-memory-bytes",
            Self::Chips => "chips",
        }
    }
}

fn require(condition: bool, refusal: &'static str) -> Result<(), &'static str> {
    if condition { Ok(()) } else { Err(refusal) }
}

fn is_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TOKEN_BYTES && !value.chars().any(char::is_control)
}

fn is_reference(value: &str) -> bool {
    value.strip_prefix(REFERENCE_PREFIX).is_some_and(|digest| {
        digest.len() == REFERENCE_HEX_BYTES
            && digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn references(values: &[String]) -> Result<(), &'static str> {
    require(values.len() <= MAX_COMPUTE_ITEMS, "compute_list_bound")?;
    require(
        values.iter().all(|v| is_reference(v)),
        "compute_reference_invalid",
    )?;
    let distinct: BTreeSet<_> = values.iter().collect();
    require(distinct.len() == values.len(), "compute_reference_invalid")
}

fn validate(observation: &ObservationDto) -> Result<(), &'static str> {
    require(
        observation.schema == COMPUTE_OBSERVATION_SCHEMA,
        "compute_unsupported_schema",
    )?;
    let producer = &observation.producer;
    require(
        producer.allocation_label == ALLOCATION_LABEL
            && producer.readback_label == READBACK_LABEL
            && producer.offer_label == OFFER_LABEL
            && producer.reservation_schema == RESERVATION_SCHEMA
            && producer.offer_schema == OFFER_SCHEMA
            && producer.time_domain == TIME_DOMAIN,
        "compute_producer_label_unsupported",
    )?;
    let port = &observation.port;
    require(is_token(&port.port_id), "compute_token_invalid")?;
    require(
        is_reference(&port.port_profile_ref),
        "compute_reference_invalid",
    )?;
    require(port.observation_sequence > 0, "compute_zero_value")?;
    validate_allocation(&observation.allocation)?;
    validate_readback(&observation.readback, &observation.allocation)?;
    if let Some(offer) = &observation.offer {
        validate_offer(offer, &observation.allocation)?;
    }
    Ok(())
}

fn validate_allocation(allocation: &AllocationDto) -> Result<(), &'static str> {
    require(
        is_reference(&allocation.record_ref),
        "compute_reference_invalid",
    )?;
    require(
        is_reference(&allocation.grant_ref),
        "compute_reference_invalid",
    )?;
    require(
        allocation.schema == RESERVATION_SCHEMA,
        "compute_producer_label_unsupported",
    )?;
    for value in [
        &allocation.id,
        &allocation.host_id,
        &allocation.host_incarnation,
        &allocation.device_incarnation,
        &allocation.view_id,
        &allocation.consumer_id,
        &allocation.operation_id,
        &allocation.attempt_id,
    ] {
        require(is_token(value), "compute_token_invalid")?;
    }
    for value in [
        &allocation.serving_allocation_id,
        &allocation.activation_backend_ref,
    ] {
        require(
            value.as_deref().is_none_or(is_token),
            "compute_token_invalid",
        )?;
    }
    for value in [
        allocation.offer_revision,
        allocation.topology_generation,
        allocation.charge_cursor,
        allocation.valid_until,
        allocation.owner_generation,
    ] {
        require(value > 0, "compute_zero_value")?;
    }
    require(
        allocation.id == allocation.operation_id,
        "compute_id_not_operation",
    )?;
    let claims = &allocation.claims;
    require(
        (1..=MAX_COMPUTE_ITEMS).contains(&claims.len()),
        "compute_list_bound",
    )?;
    require(
        allocation.authority_ids.len() == claims.len(),
        "compute_authority_count",
    )?;
    require(
        allocation.authority_ids.iter().all(|id| is_token(id)),
        "compute_token_invalid",
    )?;
    require(
        claims.iter().all(|c| is_token(&c.pool_id)),
        "compute_token_invalid",
    )?;
    let pools: BTreeSet<_> = claims.iter().map(|c| c.pool_id.as_str()).collect();
    require(pools.len() == claims.len(), "compute_repeated_pool")?;
    require(claims.iter().all(|c| c.quantity > 0), "compute_zero_value")?;
    require(
        allocation.kind != AllocationKind::ServingDeployment
            || allocation.serving_allocation_id.is_none(),
        "compute_serving_references_serving",
    )?;
    let has_evidence = match allocation.status {
        Status::Active => allocation.activation_backend_ref.is_some(),
        Status::Intent => allocation.activation_backend_ref.is_none(),
        Status::Quarantined | Status::Released => true,
    };
    require(has_evidence, "compute_activation_evidence")
}

fn validate_readback(
    readback: &ReadbackDto,
    allocation: &AllocationDto,
) -> Result<(), &'static str> {
    require(
        is_reference(&readback.record_ref),
        "compute_reference_invalid",
    )?;
    require(
        readback.host_id == allocation.host_id,
        "compute_readback_mismatch",
    )?;
    require(
        readback.revision > 0 && readback.revision >= allocation.offer_revision,
        "compute_readback_mismatch",
    )?;
    references(&readback.qualification_refs)?;
    let maximum_rows = u64::try_from(MAX_COMPUTE_ITEMS).map_err(|_| "compute_list_bound")?;
    require(
        (1..=maximum_rows).contains(&readback.row_count),
        "compute_list_bound",
    )?;
    let row = &readback.row;
    require(
        row.id == allocation.id
            && row.consumer_id == allocation.consumer_id
            && row.owner_generation == allocation.owner_generation
            && row.granularity == allocation.granularity
            && row.status == allocation.status
            && row.enforcement == allocation.enforcement
            && row.is_logically_fenced == (allocation.status != Status::Released),
        "compute_readback_mismatch",
    )
}

fn validate_offer(offer: &OfferDto, allocation: &AllocationDto) -> Result<(), &'static str> {
    require(is_reference(&offer.record_ref), "compute_reference_invalid")?;
    require(
        offer.host_id == allocation.host_id
            && offer.host_incarnation == allocation.host_incarnation
            && offer.device_incarnation == allocation.device_incarnation
            && offer.revision == allocation.offer_revision
            && offer.topology_generation == allocation.topology_generation,
        "compute_offer_mismatch",
    )?;
    let compatibility = &offer.compatibility;
    for value in [
        &compatibility.backend,
        &compatibility.runtime,
        &compatibility.architecture,
        &compatibility.topology,
    ] {
        require(
            value.as_deref().is_none_or(is_token),
            "compute_token_invalid",
        )?;
    }
    require(
        !compatibility.qualification_refs.is_empty(),
        "compute_list_bound",
    )?;
    references(&compatibility.qualification_refs)?;
    let pools = &offer.claimed_pools;
    let has_each_claim = pools.len() == allocation.claims.len()
        && allocation
            .claims
            .iter()
            .all(|claim| pools.iter().any(|pool| pool.pool_id == claim.pool_id));
    require(has_each_claim, "compute_offer_mismatch")?;
    let distinct: BTreeSet<_> = pools.iter().map(|p| p.pool_id.as_str()).collect();
    require(distinct.len() == pools.len(), "compute_repeated_pool")?;
    let offered: BTreeSet<_> = pools.iter().map(|p| p.authority_id.as_str()).collect();
    let allocated: BTreeSet<_> = allocation
        .authority_ids
        .iter()
        .map(String::as_str)
        .collect();
    require(offered == allocated, "compute_offer_mismatch")
}

fn decode(document: &[u8]) -> Result<ObservationDto, &'static str> {
    require(
        document.len() <= MAX_COMPUTE_OBSERVATION_BYTES,
        "compute_observation_byte_bound",
    )?;
    let observation: ObservationDto =
        serde_json::from_slice(document).map_err(|_| "compute_observation_malformed")?;
    validate(&observation)?;
    for value in [&observation.allocation.id, &observation.allocation.host_id]
        .into_iter()
        .chain(observation.allocation.claims.iter().map(|c| &c.pool_id))
    {
        require(public_token(value), "compute_identifier_not_public")?;
    }
    Ok(observation)
}

/// Keep the greater observation sequence per allocation; an equal sequence
/// with different content is a replay conflict and another port is ambiguous.
fn latest(documents: &[&[u8]]) -> Result<BTreeMap<String, ObservationDto>, &'static str> {
    let mut latest: BTreeMap<String, ObservationDto> = BTreeMap::new();
    for document in documents {
        let observation = decode(document)?;
        let id = observation.allocation.id.clone();
        match latest.get(&id) {
            None => {}
            Some(existing) if existing.port.port_id != observation.port.port_id => {
                return Err("compute_port_ambiguous");
            }
            Some(existing)
                if existing.port.observation_sequence == observation.port.observation_sequence =>
            {
                require(*existing == observation, "compute_observation_replayed")?;
                continue;
            }
            Some(existing)
                if existing.port.observation_sequence > observation.port.observation_sequence =>
            {
                continue;
            }
            Some(_) => {}
        }
        latest.insert(id, observation);
    }
    Ok(latest)
}

fn health(observation: &ObservationDto, provider_logical_now: Option<u64>) -> (Health, bool) {
    let now = observation
        .port
        .provider_logical_now
        .or(provider_logical_now);
    let allocation = &observation.allocation;
    if matches!(allocation.status, Status::Quarantined | Status::Released)
        || now.is_some_and(|now| now >= allocation.valid_until)
        || observation.readback.health == ProducerHealth::Unavailable
    {
        return (Health::Unavailable, now.is_none());
    }
    let offer_healthy = observation
        .offer
        .as_ref()
        .is_none_or(|offer| offer.health == ProducerHealth::Healthy);
    let healthy = allocation.status == Status::Active
        && observation.readback.is_fresh
        && observation.readback.health == ProducerHealth::Healthy
        && offer_healthy
        && now.is_some();
    (
        if healthy {
            Health::Healthy
        } else {
            Health::Unknown
        },
        now.is_none(),
    )
}

#[derive(Default)]
struct PoolAccount {
    kind: Option<PoolKind>,
    authority: Option<String>,
    authority_unknown: bool,
    hosts: BTreeSet<String>,
    allocations: Vec<PoolAllocation>,
}

fn meta(source: &SourceObservation, id: &str, unknown_fields: Vec<String>) -> FactMeta {
    FactMeta {
        reference: FactRef {
            source: source.source.clone(),
            id: id.to_owned(),
        },
        observed_at_ms: source.observed_at_ms,
        valid_until_ms: source.valid_until_ms,
        unknown_fields,
    }
}

fn account(
    pools: &mut BTreeMap<String, PoolAccount>,
    observation: &ObservationDto,
    source: &SourceObservation,
) -> Result<(), &'static str> {
    let allocation = &observation.allocation;
    for claim in &allocation.claims {
        let pool = pools.entry(claim.pool_id.clone()).or_default();
        let offered = observation.offer.as_ref().and_then(|offer| {
            offer
                .claimed_pools
                .iter()
                .find(|p| p.pool_id == claim.pool_id)
        });
        // Without offer facts the authority is exact only for a single claim.
        let authority = match (offered, allocation.authority_ids.as_slice()) {
            (Some(offered), _) => Some(offered.authority_id.clone()),
            (None, [only]) => Some(only.clone()),
            (None, _) => None,
        };
        match (&pool.authority, authority) {
            (Some(known), Some(value)) if *known != value => {
                return Err("compute_pool_authority_conflict");
            }
            (None, Some(value)) => pool.authority = Some(value),
            (_, None) => pool.authority_unknown = true,
            _ => {}
        }
        if let Some(offered) = offered {
            require(
                pool.kind.is_none_or(|kind| kind == offered.kind),
                "compute_pool_kind_conflict",
            )?;
            pool.kind = Some(offered.kind);
        }
        pool.hosts.insert(allocation.host_id.clone());
        pool.allocations.push(PoolAllocation {
            allocation: allocation.id.clone(),
            consumer: FactRef {
                source: source.source.clone(),
                id: allocation.id.clone(),
            },
            amount: claim.quantity,
        });
    }
    Ok(())
}

fn device(
    observation: &ObservationDto,
    source: &SourceObservation,
    provider_logical_now: Option<u64>,
) -> ComputeDevice {
    let allocation = &observation.allocation;
    let (health, clock_unknown) = health(observation, provider_logical_now);
    let mut unknown = vec!["sandbox".to_owned()];
    if clock_unknown {
        unknown.push("allocation_expiry".to_owned());
    }
    let mut backends = Vec::new();
    let mut qualification: BTreeSet<String> = observation
        .readback
        .qualification_refs
        .iter()
        .cloned()
        .collect();
    match &observation.offer {
        Some(offer) => {
            let compatibility = &offer.compatibility;
            backends.extend(compatibility.backend.iter().cloned());
            backends.extend(compatibility.runtime.iter().cloned());
            qualification.extend(compatibility.qualification_refs.iter().cloned());
            if backends.is_empty() {
                unknown.push("backends".to_owned());
            }
        }
        None => unknown.push("backends".to_owned()),
    }
    backends.retain(|value| public_token(value));
    unknown.sort();
    ComputeDevice {
        meta: meta(source, &allocation.id, unknown),
        host: allocation.host_id.clone(),
        device: Some(allocation.device_incarnation.clone()).filter(|v| public_token(v)),
        pool: allocation.claims.first().map(|c| c.pool_id.clone()),
        kind: match allocation.kind {
            AllocationKind::Execution => EXECUTION_ALLOCATION_KIND,
            AllocationKind::ServingDeployment => SERVING_ALLOCATION_KIND,
        }
        .to_owned(),
        health,
        sandbox: Vec::new(),
        backends,
        accounting_pools: allocation
            .claims
            .iter()
            .map(|c| FactRef {
                source: source.source.clone(),
                id: c.pool_id.clone(),
            })
            .collect(),
        qualification: qualification.into_iter().collect(),
        shared_serving: allocation
            .serving_allocation_id
            .iter()
            .map(|id| FactRef {
                source: source.source.clone(),
                id: id.clone(),
            })
            .collect(),
    }
}

/// Map one compute source's observations. `provider_logical_now` is the
/// producer clock the collecting shell recorded, used only when a document's
/// own port reading carries none.
pub fn map_compute_observations(
    source: &SourceObservation,
    provider_logical_now: Option<u64>,
    documents: &[&[u8]],
) -> Result<InventoryFragment, &'static str> {
    require(
        documents.len() <= MAX_INVENTORY_ITEMS,
        "compute_document_bound",
    )?;
    let observations = latest(documents)?;
    let mut fragment = InventoryFragment::new(InventoryKind::Compute, source.clone());
    let mut pools: BTreeMap<String, PoolAccount> = BTreeMap::new();
    let mut rows: BTreeMap<&str, (u64, BTreeSet<&str>)> = BTreeMap::new();
    for observation in observations.values() {
        let allocation = &observation.allocation;
        let host = rows.entry(allocation.host_id.as_str()).or_default();
        host.0 = host.0.max(observation.readback.row_count);
        host.1.insert(allocation.id.as_str());
        // A released allocation no longer consumes and is not inventory.
        if allocation.status == Status::Released {
            continue;
        }
        account(&mut pools, observation, source)?;
        fragment
            .compute
            .push(device(observation, source, provider_logical_now));
    }
    let incomplete_hosts: BTreeSet<&str> = rows
        .iter()
        .filter(|(_, (count, seen))| u64::try_from(seen.len()).is_ok_and(|seen| seen < *count))
        .map(|(host, _)| *host)
        .collect();
    let mut authorities = BTreeSet::new();
    for (pool_id, pool) in pools {
        if let Some(authority) = &pool.authority
            && !authorities.insert(authority.clone())
        {
            return Err("compute_accounting_identity_ambiguous");
        }
        let mut unknown: Vec<String> = UNPUBLISHED_POOL_FACTS
            .iter()
            .map(|v| (*v).to_owned())
            .collect();
        if pool.authority_unknown || pool.authority.is_none() {
            unknown.push("accounting_authority".to_owned());
        }
        if pool
            .hosts
            .iter()
            .any(|host| incomplete_hosts.contains(host.as_str()))
        {
            unknown.push("allocations".to_owned());
        }
        if pool.kind.is_none() {
            unknown.push("unit".to_owned());
        }
        unknown.sort();
        let mut allocations = pool.allocations;
        allocations.sort_by(|a, b| a.allocation.cmp(&b.allocation));
        fragment.pools.push(ResourcePool {
            meta: meta(source, &pool_id, unknown),
            unit: pool.kind.map_or("unknown", PoolKind::unit).to_owned(),
            capacity: None,
            allocatable: None,
            reserved: None,
            allocations,
            accounting_complete: false,
        });
    }
    debug_assert!(fragment.pools.iter().all(|p| p.capacity.is_none()));
    Ok(fragment)
}

#[cfg(test)]
#[path = "compute_inventory_tests.rs"]
mod tests;
