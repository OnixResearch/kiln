//! Animus #17 serving availability snapshots mapped into inventory v2 model
//! service facts.
//!
//! The DTOs mirror `animus.serving-availability-snapshot.v1` field for field
//! and deny unknown fields; Kiln owns them so no Animus code enters Kiln. A
//! snapshot is serving-owner evidence, never a reservation, a routing grant or
//! a verification of weight bytes. Kiln does not route, admit or schedule
//! inference here.
// r[impl kiln.inventory.producers]
// r[impl kiln.inventory.producers.scenario.serving]
use std::collections::{BTreeMap, BTreeSet};

use kiln_core::inventory::{
    FactMeta, FactRef, InventoryFragment, InventoryJoins, InventoryKind, MAX_INVENTORY_ITEMS,
    ModelService, Permission, ServingState, SourceObservation, public_token,
};
use serde::Deserialize;

pub const SNAPSHOT_SCHEMA: &str = "animus.serving-availability-snapshot.v1";
/// Animus caps one snapshot document at 256 KiB (`MAX_SNAPSHOT_BYTES`).
pub const MAX_SNAPSHOT_BYTES: usize = 262_144;
/// Animus `MAX_SERVICES`.
pub const MAX_SERVICES: usize = 64;
/// Animus `MAX_CAPACITY_OBSERVATIONS` per service.
pub const MAX_CAPACITY_OBSERVATIONS: usize = 16;
/// Animus `MAX_ROLLOUT_OPERATIONS`.
pub const MAX_ROLLOUT_OPERATIONS: usize = 8;
/// Animus `MAX_TOKEN_BYTES`.
const MAX_TOKEN_BYTES: usize = 256;
const REFERENCE_PREFIX: &str = "blake3:";
const REFERENCE_HEX_BYTES: usize = 64;
/// Bytes Animus forbids in an opaque token besides non-graphic ones.
const FORBIDDEN_TOKEN_BYTES: &[u8] = b"/\\\"'`<>{}";
/// Queue reason of a ready service whose current request slots are exhausted.
pub const REQUEST_SLOTS_EXHAUSTED: &str = "request-slots-exhausted";

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SnapshotDto {
    schema: String,
    source_id: String,
    source_generation: u64,
    sequence: u64,
    observed_tick: u64,
    valid_until_tick: u64,
    services: Vec<ServiceDto>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ServiceDto {
    service_id: String,
    endpoint_identity: String,
    deployment_id: String,
    deployment_generation: u64,
    locality: Locality,
    model: Option<ModelDto>,
    readiness: Readiness,
    capabilities: CapabilitiesDto,
    envelope: Option<EnvelopeDto>,
    capacity: Vec<CapacityDto>,
    admission: Admission,
    replica: Option<String>,
    resource_consumer_ref: Option<String>,
    rollout: Option<RolloutDto>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Locality {
    Local,
    PrivateRemote,
    HostedRemote,
}

impl Locality {
    const fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::PrivateRemote => "private-remote",
            Self::HostedRemote => "hosted-remote",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Readiness {
    Absent,
    Unknown,
    Loading,
    Ready,
    Draining,
    Unavailable,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Attestation {
    Claimed,
    Observed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Capability {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Admission {
    OwnerAdmission,
    NotOffered,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ModelDto {
    model_id: String,
    configuration_ref: String,
    attestation: Attestation,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CapabilitiesDto {
    text: Capability,
    tools: Capability,
    streaming: Capability,
    structured_output: Capability,
    developer_role: Capability,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EnvelopeDto {
    max_context_tokens: u32,
    max_output_tokens: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CapacityDto {
    kind: CapacityKind,
    unit: CapacityUnit,
    scope: CapacityScope,
    origin: CapacityOrigin,
    value: Option<u64>,
    observed_tick: u64,
    valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CapacityKind {
    RequestSlots,
    QueueDepth,
    KvCacheTokens,
    RequestQuota,
    TokenQuota,
    FreeMemory,
}

impl CapacityKind {
    const fn label(self) -> &'static str {
        match self {
            Self::RequestSlots => "request-slots",
            Self::QueueDepth => "queue-depth",
            Self::KvCacheTokens => "kv-cache-tokens",
            Self::RequestQuota => "request-quota",
            Self::TokenQuota => "token-quota",
            Self::FreeMemory => "free-memory",
        }
    }

    const fn unit(self) -> CapacityUnit {
        match self {
            Self::RequestSlots | Self::QueueDepth | Self::RequestQuota => CapacityUnit::Requests,
            Self::KvCacheTokens | Self::TokenQuota => CapacityUnit::Tokens,
            Self::FreeMemory => CapacityUnit::Bytes,
        }
    }

    /// Request counts only: queue depth, token budgets and free memory never
    /// count as available requests.
    const fn counts_requests(self) -> bool {
        matches!(self, Self::RequestSlots | Self::RequestQuota)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CapacityUnit {
    Requests,
    Tokens,
    Bytes,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CapacityScope {
    Replica,
    Deployment,
    Service,
    Account,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CapacityOrigin {
    ServingOwner,
    ProviderApi,
    HostProbe,
}

impl CapacityOrigin {
    const fn label(self) -> &'static str {
        match self {
            Self::ServingOwner => "serving-owner",
            Self::ProviderApi => "provider-api",
            Self::HostProbe => "host-probe",
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RolloutDto {
    qualification_ref: String,
    qualified_backend_ref: String,
    qualified_configuration_ref: String,
    base_model_ref: String,
    tokenizer_ref: String,
    adapter_ref: Option<String>,
    loaded_policy_generation: u64,
    activation: Attestation,
    probability_semantics: Semantics,
    operations: Vec<Operation>,
    qualified_envelope: EnvelopeDto,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Semantics {
    Absent,
    SampledTokenLogprobs,
    FullDistribution,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    TokenLogprobs,
    PromptLogprobs,
    SeededSampling,
    AdapterRouting,
}

fn require(condition: bool, refusal: &'static str) -> Result<(), &'static str> {
    if condition { Ok(()) } else { Err(refusal) }
}

fn is_token(value: &str) -> bool {
    (1..=MAX_TOKEN_BYTES).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !FORBIDDEN_TOKEN_BYTES.contains(&b))
}

fn is_reference(value: &str) -> bool {
    value.strip_prefix(REFERENCE_PREFIX).is_some_and(|digest| {
        digest.len() == REFERENCE_HEX_BYTES
            && digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn validate_envelope(envelope: &EnvelopeDto) -> Result<(), &'static str> {
    require(
        envelope.max_output_tokens > 0 && envelope.max_output_tokens <= envelope.max_context_tokens,
        "animus_envelope_invalid",
    )
}

fn validate_service(service: &ServiceDto) -> Result<(), &'static str> {
    for value in [
        &service.service_id,
        &service.endpoint_identity,
        &service.deployment_id,
    ] {
        require(is_token(value), "animus_token_invalid")?;
    }
    for value in [&service.replica, &service.resource_consumer_ref] {
        require(
            value.as_deref().is_none_or(is_token),
            "animus_token_invalid",
        )?;
    }
    if let Some(model) = &service.model {
        require(is_token(&model.model_id), "animus_token_invalid")?;
        require(
            is_reference(&model.configuration_ref),
            "animus_reference_invalid",
        )?;
    }
    if let Some(envelope) = &service.envelope {
        validate_envelope(envelope)?;
    }
    require(
        service.capacity.len() <= MAX_CAPACITY_OBSERVATIONS,
        "animus_list_bound",
    )?;
    for capacity in &service.capacity {
        require(
            capacity.kind.unit() == capacity.unit,
            "animus_unit_mismatch",
        )?;
        require(
            capacity.observed_tick < capacity.valid_until_tick,
            "animus_window_inverted",
        )?;
    }
    if let Some(rollout) = &service.rollout {
        for value in [
            &rollout.qualification_ref,
            &rollout.qualified_backend_ref,
            &rollout.qualified_configuration_ref,
            &rollout.base_model_ref,
            &rollout.tokenizer_ref,
        ]
        .into_iter()
        .chain(rollout.adapter_ref.iter())
        {
            require(is_reference(value), "animus_reference_invalid")?;
        }
        require(
            rollout.operations.len() <= MAX_ROLLOUT_OPERATIONS,
            "animus_list_bound",
        )?;
        validate_envelope(&rollout.qualified_envelope)?;
    }
    Ok(())
}

fn decode(document: &[u8]) -> Result<SnapshotDto, &'static str> {
    require(
        document.len() <= MAX_SNAPSHOT_BYTES,
        "animus_snapshot_byte_bound",
    )?;
    let snapshot: SnapshotDto =
        serde_json::from_slice(document).map_err(|_| "animus_snapshot_malformed")?;
    require(
        snapshot.schema == SNAPSHOT_SCHEMA,
        "animus_unsupported_schema",
    )?;
    require(is_token(&snapshot.source_id), "animus_token_invalid")?;
    require(
        snapshot.observed_tick < snapshot.valid_until_tick,
        "animus_window_inverted",
    )?;
    require(snapshot.services.len() <= MAX_SERVICES, "animus_list_bound")?;
    let mut ids = BTreeSet::new();
    for service in &snapshot.services {
        validate_service(service)?;
        require(
            ids.insert(service.service_id.as_str()),
            "animus_service_ambiguous",
        )?;
        require(
            public_token(&service.service_id),
            "animus_identifier_not_public",
        )?;
    }
    Ok(snapshot)
}

/// Keep the newest `(source_generation, sequence)` per producer source; an
/// equal position with different content is a replay conflict.
fn latest(documents: &[&[u8]]) -> Result<Vec<SnapshotDto>, &'static str> {
    let mut latest: BTreeMap<String, SnapshotDto> = BTreeMap::new();
    for document in documents {
        let snapshot = decode(document)?;
        let position = (snapshot.source_generation, snapshot.sequence);
        if let Some(existing) = latest.get(&snapshot.source_id) {
            let existing_position = (existing.source_generation, existing.sequence);
            if existing_position == position {
                require(*existing == snapshot, "animus_snapshot_replayed")?;
                continue;
            }
            if existing_position > position {
                continue;
            }
        }
        latest.insert(snapshot.source_id.clone(), snapshot);
    }
    Ok(latest.into_values().collect())
}

/// Current trusted request-count evidence at the provider clock.
fn request_capacity(service: &ServiceDto, now: Option<u64>) -> Option<(u64, &CapacityDto)> {
    let now = now?;
    let mut current = service.capacity.iter().filter(|capacity| {
        capacity.kind.counts_requests()
            && capacity.origin != CapacityOrigin::HostProbe
            && capacity.observed_tick <= now
            && now < capacity.valid_until_tick
    });
    let first = current.next()?;
    // Two current request counts are ambiguous; none of them is used.
    if current.next().is_some() {
        return None;
    }
    first.value.map(|value| (value, first))
}

fn capability_tokens(capabilities: &CapabilitiesDto, unknown: &mut Vec<String>) -> Vec<String> {
    let mut tokens = Vec::new();
    for (capability, token) in [
        (capabilities.tools, "tool-calling"),
        (capabilities.streaming, "streaming"),
        (capabilities.structured_output, "structured-output"),
        (capabilities.developer_role, "developer-role"),
    ] {
        match capability {
            Capability::Supported => tokens.push(token.to_owned()),
            Capability::Unknown => unknown.push(format!("capabilities.{token}")),
            Capability::Unsupported => {}
        }
    }
    tokens
}

fn model_service(
    snapshot: &SnapshotDto,
    service: &ServiceDto,
    source: &SourceObservation,
    joins: &InventoryJoins,
    now: Option<u64>,
) -> ModelService {
    let mut unknown = vec![
        // Animus owns routing eligibility; the snapshot carries no permission.
        "routing_permission".to_owned(),
        // No serving producer publishes artifact cache state yet.
        "artifact_available".to_owned(),
    ];
    let is_current =
        now.is_some_and(|now| snapshot.observed_tick <= now && now < snapshot.valid_until_tick);
    if !is_current {
        unknown.push("freshness".to_owned());
    }
    // Without an owner admission offer no request can be admitted, so no
    // capacity reading is request capacity.
    let capacity = match service.admission {
        Admission::OwnerAdmission => request_capacity(service, now),
        Admission::NotOffered => {
            unknown.push("request_admission".to_owned());
            None
        }
    };
    let state = match service.readiness {
        Readiness::Ready if capacity.is_some_and(|(value, _)| value == 0) => {
            ServingState::Saturated
        }
        Readiness::Ready => ServingState::Warm,
        Readiness::Loading => ServingState::Loading,
        Readiness::Absent => ServingState::Configured,
        Readiness::Draining | Readiness::Unavailable => ServingState::Unavailable,
        Readiness::Unknown | Readiness::Stale => {
            unknown.push("readiness".to_owned());
            ServingState::Unknown
        }
    };
    let tools = capability_tokens(&service.capabilities, &mut unknown);
    let modalities = match service.capabilities.text {
        Capability::Supported => vec!["text".to_owned()],
        Capability::Unknown => {
            unknown.push("capabilities.text".to_owned());
            Vec::new()
        }
        Capability::Unsupported => Vec::new(),
    };
    let (artifact, configuration) = match &service.model {
        Some(model) => (model.model_id.clone(), model.configuration_ref.clone()),
        None => {
            unknown.push("model".to_owned());
            ("unknown".to_owned(), "unknown".to_owned())
        }
    };
    if service.envelope.is_none() {
        unknown.push("context_tokens".to_owned());
    }
    if capacity.is_none() {
        unknown.push("available_requests".to_owned());
    }
    let compute_devices = service
        .resource_consumer_ref
        .iter()
        .map(|id| FactRef {
            source: joins.compute.clone(),
            id: id.clone(),
        })
        .collect();
    unknown.sort();
    ModelService {
        meta: FactMeta {
            reference: FactRef {
                source: source.source.clone(),
                id: service.service_id.clone(),
            },
            observed_at_ms: source.observed_at_ms,
            valid_until_ms: source.valid_until_ms,
            unknown_fields: unknown,
        },
        artifact: artifact.clone(),
        artifact_available: None,
        // An observed or claimed model identity never verifies weight bytes.
        artifact_verification_evidence: None,
        configuration,
        claimed_model: artifact,
        deployment: format!(
            "{}@{}",
            service.deployment_id, service.deployment_generation
        ),
        // The endpoint identity is withheld from general views.
        endpoint_alias: service.service_id.clone(),
        replicas: service.replica.iter().cloned().collect(),
        state,
        backend: service.locality.label().to_owned(),
        routing_permission: Permission::Unknown,
        routing_policy_ref: None,
        tools,
        modalities,
        context_tokens: service.envelope.map(|e| u64::from(e.max_context_tokens)),
        available_requests: capacity.map(|(value, _)| value),
        queue_reason: capacity
            .filter(|(value, _)| *value == 0)
            .map(|_| REQUEST_SLOTS_EXHAUSTED.to_owned()),
        measurement: capacity.map(|(_, evidence)| {
            format!(
                "animus.{}.{}@{}:{}:{}",
                evidence.origin.label(),
                evidence.kind.label(),
                snapshot.source_id,
                snapshot.source_generation,
                snapshot.sequence
            )
        }),
        accounting_pools: Vec::new(),
        compute_devices,
    }
}

/// Map one serving source's snapshots. `provider_logical_now` is the producer
/// clock the collecting shell recorded; without it, freshness and request
/// capacity stay unknown.
pub fn map_serving_snapshots(
    source: &SourceObservation,
    joins: &InventoryJoins,
    provider_logical_now: Option<u64>,
    documents: &[&[u8]],
) -> Result<InventoryFragment, &'static str> {
    joins.admit()?;
    require(
        documents.len() <= MAX_INVENTORY_ITEMS,
        "animus_document_bound",
    )?;
    let mut fragment = InventoryFragment::new(InventoryKind::Serving, source.clone());
    let mut seen = BTreeSet::new();
    for snapshot in latest(documents)? {
        for service in &snapshot.services {
            require(
                seen.insert(service.service_id.clone()),
                "animus_service_ambiguous",
            )?;
            fragment.models.push(model_service(
                &snapshot,
                service,
                source,
                joins,
                provider_logical_now,
            ));
        }
    }
    fragment
        .models
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    Ok(fragment)
}

#[cfg(test)]
mod tests;
