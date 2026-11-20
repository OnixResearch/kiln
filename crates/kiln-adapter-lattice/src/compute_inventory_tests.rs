//! Compute allocation observations: the positive and negative observation
//! fixtures of Lattice main 1bdacb35, plus a derived local serving deployment
//! that shares the execution host's memory pool.
// r[verify kiln.inventory.producers.scenario.compute]
// r[verify kiln.inventory.producers.scenario.refused]
use kiln_core::inventory::{FactRef, Health, InventoryFragment, SourceObservation};
use serde_json::Value;

use super::{EXECUTION_ALLOCATION_KIND, SERVING_ALLOCATION_KIND, map_compute_observations};

type TestResult<T = ()> = Result<T, String>;

const FIRST: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/compute/agent-a-1-active.json");
const SECOND: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/compute/agent-a-2-active.json");
const DEVICE: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/compute/device-active.json");
const SERVING: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/compute/serving-b-deployment.json");
const LOCAL_SERVING: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/compute/serving-a-local-deployment.json");
const NEGATIVE: [(&str, &[u8]); 15] = [
    (
        "compute_activation_evidence",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/active-without-evidence.json"
        ),
    ),
    (
        "compute_producer_label_unsupported",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/allocation-v1-label.json"
        ),
    ),
    (
        "compute_authority_count",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/authority-count.json"
        ),
    ),
    (
        "compute_id_not_operation",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/id-not-operation.json"
        ),
    ),
    (
        "compute_offer_mismatch",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/offer-authority-mismatch.json"
        ),
    ),
    (
        "compute_offer_mismatch",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/offer-revision-mismatch.json"
        ),
    ),
    (
        "compute_list_bound",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/oversized-claims.json"
        ),
    ),
    (
        "compute_producer_label_unsupported",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/readback-v2-label.json"
        ),
    ),
    (
        "compute_repeated_pool",
        include_bytes!("../../../fixtures/inventory/producers/compute/negative/repeated-pool.json"),
    ),
    (
        "compute_readback_mismatch",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/row-fence-mismatch.json"
        ),
    ),
    (
        "compute_readback_mismatch",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/row-owner-mismatch.json"
        ),
    ),
    (
        "compute_serving_references_serving",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/serving-references-serving.json"
        ),
    ),
    (
        "compute_reference_invalid",
        include_bytes!("../../../fixtures/inventory/producers/compute/negative/uppercase-ref.json"),
    ),
    (
        "compute_producer_label_unsupported",
        include_bytes!(
            "../../../fixtures/inventory/producers/compute/negative/wrong-reservation-schema.json"
        ),
    ),
    (
        "compute_zero_value",
        include_bytes!("../../../fixtures/inventory/producers/compute/negative/zero-quantity.json"),
    ),
];

fn source() -> SourceObservation {
    SourceObservation {
        source: "aspen-compute".into(),
        producer: "aspen".into(),
        scope: "tenant-pilot".into(),
        generation: 1,
        observed_at_ms: 1_795_200_620_000,
        valid_until_ms: 1_795_200_680_000,
        complete: true,
        next_page: None,
        unavailable_reason: None,
    }
}

fn reference(id: &str) -> FactRef {
    FactRef {
        source: "aspen-compute".into(),
        id: id.into(),
    }
}

fn map(documents: &[&[u8]]) -> Result<InventoryFragment, &'static str> {
    map_compute_observations(&source(), None, documents)
}

fn edit(bytes: &[u8], edits: &[(&str, Value)]) -> TestResult<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    for (pointer, replacement) in edits {
        let slot = value
            .pointer_mut(pointer)
            .ok_or_else(|| format!("missing {pointer}"))?;
        *slot = replacement.clone();
    }
    serde_json::to_vec(&value).map_err(|e| e.to_string())
}

fn pool_allocations(fragment: &InventoryFragment, pool: &str) -> Vec<(String, u64)> {
    fragment
        .pools
        .iter()
        .filter(|p| p.meta.reference.id == pool)
        .flat_map(|p| p.allocations.iter())
        .map(|a| (a.allocation.clone(), a.amount))
        .collect()
}

#[test]
fn shared_serving_deployment_is_referenced_and_counted_once() -> TestResult {
    let fragment = map(&[FIRST, SECOND, DEVICE, SERVING])?;
    assert_eq!(fragment.compute.len(), 4);
    for id in ["agent-a-1", "agent-a-2"] {
        let device = fragment
            .compute
            .iter()
            .find(|c| c.meta.reference.id == id)
            .ok_or("missing execution allocation")?;
        assert_eq!(device.kind, EXECUTION_ALLOCATION_KIND);
        assert_eq!(device.host, "host-a");
        assert_eq!(device.shared_serving, vec![reference("serving-b")]);
        assert_eq!(device.accounting_pools, vec![reference("host-a-memory")]);
        assert_eq!(device.health, Health::Healthy);
    }
    let serving = fragment
        .compute
        .iter()
        .find(|c| c.meta.reference.id == "serving-b")
        .ok_or("missing serving allocation")?;
    assert_eq!(serving.kind, SERVING_ALLOCATION_KIND);
    assert!(serving.shared_serving.is_empty());
    assert_eq!(
        pool_allocations(&fragment, "host-b-shared-memory"),
        vec![("serving-b".to_owned(), 80)],
        "the deployment's memory is charged once, never to the workers"
    );
    assert_eq!(
        pool_allocations(&fragment, "host-b-chip"),
        vec![("serving-b".to_owned(), 1)]
    );
    assert_eq!(
        pool_allocations(&fragment, "host-a-memory"),
        vec![("agent-a-1".to_owned(), 40), ("agent-a-2".to_owned(), 40)]
    );
    for pool in &fragment.pools {
        assert_eq!(
            (pool.capacity, pool.allocatable, pool.reserved),
            (None, None, None)
        );
        assert!(!pool.accounting_complete);
        for unknown in ["allocatable", "capacity", "reserved"] {
            assert!(pool.meta.unknown_fields.iter().any(|f| f == unknown));
        }
        assert!(!pool.meta.unknown_fields.iter().any(|f| f == "allocations"));
    }
    let second = fragment
        .compute
        .iter()
        .find(|c| c.meta.reference.id == "agent-a-2")
        .ok_or("missing second allocation")?;
    assert!(
        second.meta.unknown_fields.iter().any(|f| f == "backends"),
        "without offer facts the backend is unknown"
    );
    assert!(second.meta.unknown_fields.iter().any(|f| f == "sandbox"));
    Ok(())
}

#[test]
fn pool_shared_by_execution_and_serving_is_one_capacity() -> TestResult {
    let fragment = map(&[FIRST, SECOND, DEVICE, SERVING, LOCAL_SERVING])?;
    let shared: Vec<_> = fragment
        .pools
        .iter()
        .filter(|p| p.meta.reference.id == "host-a-memory")
        .collect();
    assert_eq!(shared.len(), 1);
    assert_eq!(
        pool_allocations(&fragment, "host-a-memory"),
        vec![
            ("agent-a-1".to_owned(), 40),
            ("agent-a-2".to_owned(), 40),
            ("serving-a".to_owned(), 20)
        ]
    );
    assert_eq!(
        shared.first().map(|p| p.unit.as_str()),
        Some("unified-memory-bytes")
    );
    let partial = map(&[FIRST, SECOND, LOCAL_SERVING])?;
    let pool = partial
        .pools
        .iter()
        .find(|p| p.meta.reference.id == "host-a-memory")
        .ok_or("missing pool")?;
    assert!(
        pool.meta.unknown_fields.iter().any(|f| f == "allocations"),
        "fewer observed allocations than the readback's rows is an incomplete page"
    );
    let alias = edit(
        LOCAL_SERVING,
        &[
            ("/allocation/claims/0/pool_id", "host-a-memory-alias".into()),
            (
                "/offer/claimed_pools/0/pool_id",
                "host-a-memory-alias".into(),
            ),
        ],
    )?;
    assert_eq!(
        map(&[FIRST, &alias]).err(),
        Some("compute_accounting_identity_ambiguous"),
        "one accounting authority under two pool identities would double count"
    );
    let other = edit(
        SECOND,
        &[("/allocation/authority_ids/0", "other-authority".into())],
    )?;
    assert_eq!(
        map(&[FIRST, &other]).err(),
        Some("compute_pool_authority_conflict")
    );
    Ok(())
}

#[test]
fn sequences_order_observations_and_replays_refuse() -> TestResult {
    assert_eq!(
        map(&[FIRST, FIRST])?,
        map(&[FIRST])?,
        "an identical replay collapses"
    );
    let replay = edit(FIRST, &[("/allocation/charge_at", 21.into())])?;
    assert_eq!(
        map(&[FIRST, &replay]).err(),
        Some("compute_observation_replayed")
    );
    let released = edit(
        FIRST,
        &[
            ("/port/observation_sequence", 2.into()),
            ("/allocation/status", "released".into()),
            ("/allocation/activation_backend_ref", Value::Null),
            ("/readback/row/status", "released".into()),
            ("/readback/row/is_logically_fenced", false.into()),
        ],
    )?;
    for order in [[FIRST, released.as_slice()], [released.as_slice(), FIRST]] {
        let fragment = map(&order)?;
        assert!(
            fragment.compute.is_empty(),
            "the newer released reading wins in any order"
        );
        assert!(fragment.pools.is_empty());
    }
    let other_port = edit(FIRST, &[("/port/port_id", "other-port".into())])?;
    assert_eq!(
        map(&[FIRST, &other_port]).err(),
        Some("compute_port_ambiguous")
    );
    Ok(())
}

#[test]
fn producer_clock_decides_expiry_and_health() -> TestResult {
    let unclocked = edit(FIRST, &[("/port/provider_logical_now", Value::Null)])?;
    let fragment = map(&[&unclocked])?;
    let device = fragment.compute.first().ok_or("missing allocation")?;
    assert_eq!(device.health, Health::Unknown);
    assert!(
        device
            .meta
            .unknown_fields
            .iter()
            .any(|f| f == "allocation_expiry")
    );
    let clocked = map_compute_observations(&source(), Some(30), &[&unclocked])?;
    assert_eq!(
        clocked.compute.first().map(|d| d.health),
        Some(Health::Healthy)
    );
    let expired = map_compute_observations(&source(), Some(80), &[&unclocked])?;
    assert_eq!(
        expired.compute.first().map(|d| d.health),
        Some(Health::Unavailable)
    );
    let stale = edit(FIRST, &[("/readback/is_fresh", false.into())])?;
    assert_eq!(
        map(&[&stale])?.compute.first().map(|d| d.health),
        Some(Health::Unknown)
    );
    let degraded = edit(FIRST, &[("/readback/health", "degraded".into())])?;
    assert_eq!(
        map(&[&degraded])?.compute.first().map(|d| d.health),
        Some(Health::Unknown)
    );
    Ok(())
}

#[test]
fn producer_negative_fixtures_and_shapes_refuse() -> TestResult {
    for (expected, document) in NEGATIVE {
        assert_eq!(map(&[document]).err(), Some(expected));
    }
    let schema = edit(
        FIRST,
        &[(
            "/schema",
            "lattice.compute-allocation-observation.v2".into(),
        )],
    )?;
    assert_eq!(map(&[&schema]).err(), Some("compute_unsupported_schema"));
    let mut extra: Value = serde_json::from_slice(FIRST).map_err(|e| e.to_string())?;
    extra
        .pointer_mut("/allocation")
        .and_then(Value::as_object_mut)
        .ok_or("missing allocation")?
        .insert("free_bytes".into(), 1.into());
    let extra = serde_json::to_vec(&extra).map_err(|e| e.to_string())?;
    assert_eq!(map(&[&extra]).err(), Some("compute_observation_malformed"));
    let private = edit(
        FIRST,
        &[
            ("/allocation/host_id", "host a".into()),
            ("/readback/host_id", "host a".into()),
            ("/offer/host_id", "host a".into()),
        ],
    )?;
    assert_eq!(
        map(&[&private]).err(),
        Some("compute_identifier_not_public")
    );
    Ok(())
}
