use super::super::{
    ComputeDevice, FactMeta, FactRef, Health, InventoryKind, ModelService, Permission,
    ResourcePool, ServingState, SourceObservation,
};
use super::{InventoryFragment, compose_snapshot};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

fn reference(source: &str, id: &str) -> FactRef {
    FactRef {
        source: source.into(),
        id: id.into(),
    }
}

fn meta(source: &str, id: &str) -> FactMeta {
    FactMeta {
        reference: reference(source, id),
        observed_at_ms: 10,
        valid_until_ms: 20,
        unknown_fields: Vec::new(),
    }
}

fn source(id: &str) -> SourceObservation {
    SourceObservation {
        source: id.into(),
        producer: "producer".into(),
        scope: "scope".into(),
        generation: 1,
        observed_at_ms: 10,
        valid_until_ms: 20,
        complete: true,
        next_page: None,
        unavailable_reason: None,
    }
}

fn device(id: &str, pools: &[&str]) -> ComputeDevice {
    ComputeDevice {
        meta: meta("compute", id),
        host: "host-a".into(),
        device: None,
        pool: None,
        kind: "serving-deployment-allocation".into(),
        health: Health::Unknown,
        sandbox: Vec::new(),
        backends: Vec::new(),
        accounting_pools: pools.iter().map(|p| reference("compute", p)).collect(),
        qualification: Vec::new(),
        shared_serving: Vec::new(),
    }
}

fn pool(id: &str) -> ResourcePool {
    ResourcePool {
        meta: meta("compute", id),
        unit: "unified-memory-bytes".into(),
        capacity: None,
        allocatable: None,
        reserved: None,
        allocations: Vec::new(),
        accounting_complete: false,
    }
}

fn model(id: &str, devices: &[&str]) -> ModelService {
    ModelService {
        meta: meta("serving", id),
        artifact: "model-a".into(),
        artifact_available: None,
        artifact_verification_evidence: None,
        configuration: "configuration".into(),
        claimed_model: "model-a".into(),
        deployment: "deployment@1".into(),
        endpoint_alias: id.into(),
        replicas: Vec::new(),
        state: ServingState::Warm,
        backend: "private-remote".into(),
        routing_permission: Permission::Unknown,
        routing_policy_ref: None,
        tools: Vec::new(),
        modalities: Vec::new(),
        context_tokens: None,
        available_requests: None,
        queue_reason: None,
        measurement: None,
        accounting_pools: Vec::new(),
        compute_devices: devices.iter().map(|d| reference("compute", d)).collect(),
    }
}

fn fragments() -> Vec<InventoryFragment> {
    let mut compute = InventoryFragment::new(InventoryKind::Compute, source("compute"));
    compute
        .compute
        .push(device("serving-b", &["shared-memory", "chip"]));
    compute.pools.push(pool("shared-memory"));
    compute.pools.push(pool("chip"));
    let mut serving = InventoryFragment::new(InventoryKind::Serving, source("serving"));
    serving.models.push(model("svc-b", &["serving-b"]));
    serving.models.push(model("svc-hosted", &[]));
    serving
        .models
        .push(model("svc-unplaced", &["missing-allocation"]));
    vec![serving, compute]
}

// r[verify kiln.inventory.collection]
#[test]
fn composition_joins_exact_allocations_and_sorts_deterministically() -> Result<(), String> {
    let snapshot = compose_snapshot("operator", true, fragments()).map_err(String::from)?;
    let reversed = {
        let mut values = fragments();
        values.reverse();
        compose_snapshot("operator", true, values).map_err(String::from)?
    };
    assert_eq!(
        snapshot, reversed,
        "fragment order never changes the snapshot"
    );
    assert_eq!(snapshot.sources.len(), 2);
    assert_eq!(
        snapshot.sources.first().map(|s| s.source.as_str()),
        Some("compute")
    );
    let served = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-b")
        .ok_or("missing served model")?;
    assert_eq!(
        served.accounting_pools,
        vec![
            reference("compute", "chip"),
            reference("compute", "shared-memory")
        ],
        "a serving fact accounts only through its exact allocation's pools"
    );
    let hosted = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-hosted")
        .ok_or("missing hosted model")?;
    assert!(hosted.accounting_pools.is_empty() && hosted.compute_devices.is_empty());
    let unplaced = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-unplaced")
        .ok_or("missing unplaced model")?;
    assert!(
        unplaced.accounting_pools.is_empty(),
        "an unknown allocation contributes no pool"
    );
    assert_eq!(
        snapshot.pools.len(),
        2,
        "pools are never duplicated per consumer"
    );
    Ok(())
}

fn with_serving(change: impl FnOnce(&mut InventoryFragment)) -> Vec<InventoryFragment> {
    let mut values = fragments();
    if let Some(serving) = values.iter_mut().find(|f| f.kind == InventoryKind::Serving) {
        change(serving);
    }
    values
}

#[test]
fn composition_refuses_ambiguous_and_mismatched_fragments() {
    let mut duplicate_source = fragments();
    duplicate_source.push(InventoryFragment::new(
        InventoryKind::Serving,
        source("serving"),
    ));
    assert_eq!(
        compose_snapshot("operator", true, duplicate_source),
        Err("inventory_source_ambiguous")
    );
    let wrong_kind = with_serving(|serving| serving.pools.push(pool("serving-pool")));
    assert_eq!(
        compose_snapshot("operator", true, wrong_kind),
        Err("inventory_fragment_kind_mismatch")
    );
    let foreign = with_serving(|serving| {
        let mut value = model("svc-c", &[]);
        value.meta.reference.source = "compute".into();
        serving.models.push(value);
    });
    assert_eq!(
        compose_snapshot("operator", true, foreign),
        Err("inventory_fragment_source_mismatch")
    );
    let repeated = with_serving(|serving| serving.models.push(model("svc-b", &[])));
    assert_eq!(
        compose_snapshot("operator", true, repeated),
        Err("inventory_fact_ambiguous")
    );
    let policy = vec![InventoryFragment::new(
        InventoryKind::Policy,
        source("policy"),
    )];
    assert_eq!(
        compose_snapshot("operator", true, policy),
        Err("inventory_fragment_kind_mismatch")
    );
    assert_eq!(
        compose_snapshot("operator/../x", true, fragments()),
        Err("inventory_public_identifier_invalid")
    );
}
