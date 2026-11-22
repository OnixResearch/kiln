//! Animus serving snapshots: the published `snapshot-cpu-worker-remote.json`
//! fixture of Animus main 4071c65e and a derived snapshot covering loading,
//! saturated, ready and absent services.
// r[verify kiln.inventory.producers.scenario.serving]
// r[verify kiln.inventory.producers.scenario.refused]
use kiln_core::inventory::{
    FactRef, InventoryFragment, InventoryJoins, ModelService, Permission, ServingState,
    SourceObservation,
};
use serde_json::Value;

use super::{REQUEST_SLOTS_EXHAUSTED, map_serving_snapshots};

type TestResult<T = ()> = Result<T, String>;

const REMOTE: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/animus/snapshot-cpu-worker-remote.json");
const STATES: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/animus/snapshot-serving-states.json");
/// A provider clock inside both snapshots' `[100, 160)` validity window.
const CURRENT_TICK: u64 = 120;

fn source() -> SourceObservation {
    SourceObservation {
        source: "animus-serving".into(),
        producer: "animus".into(),
        scope: "tenant-pilot".into(),
        generation: 1,
        observed_at_ms: 1_795_200_620_000,
        valid_until_ms: 1_795_200_680_000,
        complete: true,
        next_page: None,
        unavailable_reason: None,
    }
}

fn joins() -> InventoryJoins {
    InventoryJoins {
        agents: "lattice-agents".into(),
        compute: "aspen-compute".into(),
        serving: "animus-serving".into(),
    }
}

fn map(now: Option<u64>, documents: &[&[u8]]) -> Result<InventoryFragment, &'static str> {
    map_serving_snapshots(&source(), &joins(), now, documents)
}

fn service<'a>(fragment: &'a InventoryFragment, id: &str) -> TestResult<&'a ModelService> {
    fragment
        .models
        .iter()
        .find(|m| m.meta.reference.id == id)
        .ok_or_else(|| format!("missing {id}"))
}

fn unknown(model: &ModelService, field: &str) -> bool {
    model.meta.unknown_fields.iter().any(|f| f == field)
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

#[test]
fn serving_states_capacity_and_placement_stay_distinct() -> TestResult {
    let fragment = map(Some(CURRENT_TICK), &[REMOTE, STATES])?;
    assert_eq!(fragment.models.len(), 6);
    let placed = service(&fragment, "svc-serving-b")?;
    assert_eq!(placed.state, ServingState::Warm);
    assert_eq!(
        placed.available_requests,
        Some(1),
        "only serving-owner request slots count"
    );
    assert_eq!(
        placed.measurement.as_deref(),
        Some("animus.serving-owner.request-slots@host-observer:1:7")
    );
    assert_eq!(
        placed.compute_devices,
        vec![FactRef {
            source: "aspen-compute".into(),
            id: "serving-b".into()
        }],
        "placement joins the exact compute allocation"
    );
    assert_eq!(placed.deployment, "deploy-serving-b@3");
    assert_eq!(
        placed.endpoint_alias, "svc-serving-b",
        "endpoint identity is withheld"
    );
    assert_eq!(placed.claimed_model, "model-a");
    assert_eq!(placed.artifact_verification_evidence, None);
    assert_eq!(placed.artifact_available, None);
    assert_eq!(placed.routing_permission, Permission::Unknown);
    assert_eq!(placed.context_tokens, Some(8192));
    assert!(unknown(placed, "artifact_available") && unknown(placed, "routing_permission"));
    let hosted = service(&fragment, "svc-hosted")?;
    assert!(
        hosted.compute_devices.is_empty(),
        "a hosted API has no fictitious accelerator"
    );
    assert_eq!(hosted.backend, "hosted-remote");
    assert_eq!(
        hosted.available_requests, None,
        "an unreported quota is unknown, not zero"
    );
    assert!(unknown(hosted, "available_requests") && unknown(hosted, "capabilities.streaming"));
    let loading = service(&fragment, "svc-loading")?;
    assert_eq!(loading.state, ServingState::Loading);
    assert_eq!(loading.available_requests, None);
    let saturated = service(&fragment, "svc-saturated")?;
    assert_eq!(saturated.state, ServingState::Saturated);
    assert_eq!(
        saturated.available_requests,
        Some(0),
        "queue depth never counts as capacity"
    );
    assert_eq!(
        saturated.queue_reason.as_deref(),
        Some(REQUEST_SLOTS_EXHAUSTED)
    );
    let ready = service(&fragment, "svc-shared")?;
    assert_eq!(
        (ready.state, ready.available_requests),
        (ServingState::Warm, Some(2))
    );
    assert!(ready.tools.contains(&"tool-calling".to_owned()));
    assert_eq!(ready.modalities, vec!["text".to_owned()]);
    let absent = service(&fragment, "svc-absent")?;
    assert_eq!(absent.state, ServingState::Configured);
    assert!(unknown(absent, "model") && unknown(absent, "request_admission"));
    for model in &fragment.models {
        assert!(!unknown(model, "freshness"));
        assert_eq!(model.artifact_verification_evidence, None);
    }
    Ok(())
}

#[test]
fn stale_or_unclocked_snapshots_never_report_capacity() -> TestResult {
    for now in [None, Some(160), Some(99)] {
        let fragment = map(now, &[REMOTE, STATES])?;
        for model in &fragment.models {
            assert!(unknown(model, "freshness"), "{now:?} must not be current");
        }
        let saturated = service(&fragment, "svc-saturated")?;
        assert_eq!(saturated.available_requests, None);
        assert_ne!(saturated.state, ServingState::Saturated);
    }
    let stale = edit(STATES, &[("/services/2/readiness", "stale".into())])?;
    let fragment = map(Some(CURRENT_TICK), &[&stale])?;
    let model = service(&fragment, "svc-shared")?;
    assert_eq!(model.state, ServingState::Unknown);
    assert!(unknown(model, "readiness"));
    let probe = edit(
        REMOTE,
        &[("/services/0/capacity/0/origin", "host-probe".into())],
    )?;
    let fragment = map(Some(CURRENT_TICK), &[&probe])?;
    assert_eq!(
        service(&fragment, "svc-serving-b")?.available_requests,
        None,
        "a host probe is never request capacity"
    );
    Ok(())
}

#[test]
fn sequences_order_snapshots_and_replays_refuse() -> TestResult {
    let newer = edit(
        STATES,
        &[
            ("/sequence", 5.into()),
            ("/services/1/capacity/0/value", 3.into()),
        ],
    )?;
    for order in [[STATES, newer.as_slice()], [newer.as_slice(), STATES]] {
        let fragment = map(Some(CURRENT_TICK), &order)?;
        let model = service(&fragment, "svc-saturated")?;
        assert_eq!(
            (model.state, model.available_requests),
            (ServingState::Warm, Some(3))
        );
    }
    let replay = edit(STATES, &[("/services/1/capacity/0/value", 3.into())])?;
    assert_eq!(
        map(Some(CURRENT_TICK), &[STATES, &replay]).err(),
        Some("animus_snapshot_replayed")
    );
    let crossed = edit(STATES, &[("/source_id", "other-observer".into())])?;
    let crossed_with = edit(
        &crossed,
        &[("/services/0/service_id", "svc-serving-b".into())],
    )?;
    assert_eq!(
        map(Some(CURRENT_TICK), &[REMOTE, &crossed_with]).err(),
        Some("animus_service_ambiguous")
    );
    Ok(())
}

#[test]
fn invalid_snapshots_refuse_with_named_reasons() -> TestResult {
    let cases: [(&str, &[(&str, Value)]); 6] = [
        (
            "animus_unsupported_schema",
            &[(
                "/schema",
                Value::from("animus.serving-availability-snapshot.v2"),
            )],
        ),
        (
            "animus_window_inverted",
            &[("/valid_until_tick", Value::from(100))],
        ),
        (
            "animus_unit_mismatch",
            &[("/services/0/capacity/0/unit", Value::from("tokens"))],
        ),
        (
            "animus_reference_invalid",
            &[(
                "/services/0/model/configuration_ref",
                Value::from("blake3:AA"),
            )],
        ),
        (
            "animus_token_invalid",
            &[(
                "/services/0/endpoint_identity",
                Value::from("https://endpoint/v1"),
            )],
        ),
        (
            "animus_service_ambiguous",
            &[("/services/1/service_id", Value::from("svc-serving-b"))],
        ),
    ];
    for (expected, edits) in cases {
        let document = edit(REMOTE, edits)?;
        assert_eq!(map(Some(CURRENT_TICK), &[&document]).err(), Some(expected));
    }
    let mut extra: Value = serde_json::from_slice(REMOTE).map_err(|e| e.to_string())?;
    extra
        .pointer_mut("/services/0")
        .and_then(Value::as_object_mut)
        .ok_or("missing service")?
        .insert("gpu_free_bytes".into(), 1.into());
    let extra = serde_json::to_vec(&extra).map_err(|e| e.to_string())?;
    assert_eq!(
        map(Some(CURRENT_TICK), &[&extra]).err(),
        Some("animus_snapshot_malformed")
    );
    let oversized = vec![b' '; super::MAX_SNAPSHOT_BYTES + 1];
    assert_eq!(
        map(None, &[&oversized]).err(),
        Some("animus_snapshot_byte_bound")
    );
    Ok(())
}
