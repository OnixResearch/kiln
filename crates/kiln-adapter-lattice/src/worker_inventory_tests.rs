//! Lattice inventory documents captured from `lattice implementation-attempt
//! inspect` at Lattice main 1bdacb35 (deterministic fixture ports only).
// r[verify kiln.inventory.producers.scenario.lattice]
// r[verify kiln.inventory.producers.scenario.refused]
use kiln_core::inventory::{
    FactRef, InventoryJoins, InventoryKind, Permission, SourceObservation, WorkerLifecycle,
};
use serde_json::Value;

use super::map_worker_inventory;

type TestResult<T = ()> = Result<T, String>;

const ADMITTED: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/agent-a-1-admitted-not-dispatched.json"
);
const FIRST: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/agent-a-1-dispatched-outcome-unknown.json"
);
const TERMINAL: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/lattice/agent-a-1-terminal-failed.json");
const SECOND: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/agent-a-2-dispatched-outcome-unknown.json"
);
const SERVED: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/lattice/host-local-served-ready.json");
const SERVED_UNAVAILABLE: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/host-local-served-unavailable.json"
);
const PROFILE_ONLY: &[u8] =
    include_bytes!("../../../fixtures/inventory/producers/lattice/profile-without-instance.json");
/// The captured instances were observed before this collection time.
const COLLECTED_AT_MS: u64 = 1_795_200_620_000;
const VALID_UNTIL_MS: u64 = 1_795_200_680_000;
const PROFILE: &str = "8a836baf7f4924334a2d50ced5590fd4f5a70e91bc67ee04c667c00722b17b72";
const HOST_LOCAL_PROFILE: &str = "2fb8023506390884d4d5974a91d807e8a8ff5af11533ecc67b34f39f79e4fa41";

fn source(id: &str) -> SourceObservation {
    SourceObservation {
        source: id.into(),
        producer: "lattice".into(),
        scope: "tenant-pilot".into(),
        generation: 1,
        observed_at_ms: COLLECTED_AT_MS,
        valid_until_ms: VALID_UNTIL_MS,
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

fn reference(source: &str, id: &str) -> FactRef {
    FactRef {
        source: source.into(),
        id: id.into(),
    }
}

fn edit(bytes: &[u8], pointer: &str, replacement: Value) -> TestResult<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let slot = value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("missing {pointer}"))?;
    *slot = replacement;
    serde_json::to_vec(&value).map_err(|e| e.to_string())
}

fn agents(documents: &[&[u8]]) -> Result<kiln_core::inventory::InventoryFragment, &'static str> {
    map_worker_inventory(
        InventoryKind::Agents,
        &source("lattice-agents"),
        &joins(),
        documents,
    )
}

fn workers(documents: &[&[u8]]) -> Result<kiln_core::inventory::InventoryFragment, &'static str> {
    map_worker_inventory(
        InventoryKind::Workers,
        &source("lattice-workers"),
        &joins(),
        documents,
    )
}

#[test]
fn profiles_are_definitions_with_explicit_unknowns() -> TestResult {
    let fragment = agents(&[FIRST, SECOND, PROFILE_ONLY, SERVED, SERVED_UNAVAILABLE])?;
    assert!(
        fragment.workers.is_empty(),
        "an agents source never yields workers"
    );
    let revisions: Vec<_> = fragment
        .agents
        .iter()
        .map(|a| a.profile_revision.as_str())
        .collect();
    assert_eq!(revisions, vec![HOST_LOCAL_PROFILE, PROFILE]);
    let definition = fragment
        .agents
        .iter()
        .find(|a| a.profile_revision == PROFILE)
        .ok_or("missing definition")?;
    assert_eq!(
        definition.meta.reference,
        reference("lattice-agents", PROFILE)
    );
    assert_eq!(definition.harnesses, vec!["omp-jsonl".to_owned()]);
    assert_eq!(
        definition.output_contracts,
        vec!["candidate-receipt".to_owned()]
    );
    assert_eq!(definition.authority, Permission::Unknown);
    assert!(definition.task_families.is_empty() && definition.tools.is_empty());
    assert!(definition.qualifications.is_empty());
    assert_eq!(definition.revisions.selected_for_new_sessions, None);
    for unknown in [
        "permitted_models",
        "qualifications",
        "revisions",
        "task_families",
        "tools",
    ] {
        assert!(
            definition.meta.unknown_fields.iter().any(|f| f == unknown),
            "{unknown} must stay unknown"
        );
    }
    Ok(())
}

#[test]
fn configured_profile_without_instance_yields_no_worker() -> TestResult {
    let definitions = agents(&[PROFILE_ONLY])?;
    assert_eq!(definitions.agents.len(), 1);
    let fleet = workers(&[PROFILE_ONLY])?;
    assert!(
        fleet.workers.is_empty(),
        "a definition is never an instance"
    );
    Ok(())
}

#[test]
fn instances_join_exact_allocations_and_never_report_idle() -> TestResult {
    let fragment = workers(&[FIRST, SECOND, SERVED, PROFILE_ONLY])?;
    assert!(fragment.agents.is_empty());
    assert_eq!(fragment.workers.len(), 3);
    for worker in &fragment.workers {
        assert!(!matches!(
            worker.lifecycle,
            WorkerLifecycle::Idle | WorkerLifecycle::Healthy | WorkerLifecycle::Executing
        ));
        assert_eq!(
            worker.execution_grant, None,
            "grant references are never projected"
        );
        assert_eq!(worker.workspace, None, "workspace paths are restricted");
        assert_eq!(worker.session, None);
        for unknown in ["allocation_liveness", "host_reachability"] {
            assert!(worker.meta.unknown_fields.iter().any(|f| f == unknown));
        }
    }
    let executions: Vec<_> = fragment
        .workers
        .iter()
        .map(|w| w.execution.clone())
        .collect();
    assert!(executions.contains(&reference("aspen-compute", "agent-a-1")));
    assert!(executions.contains(&reference("aspen-compute", "agent-a-2")));
    let host_local = fragment
        .workers
        .iter()
        .find(|w| w.pinned_revision == HOST_LOCAL_PROFILE)
        .ok_or("missing host-local worker")?;
    assert_eq!(
        host_local.execution,
        reference("lattice-workers", super::HOST_LOCAL_EXECUTION),
        "a host-local worker never joins a compute fact"
    );
    assert!(
        host_local
            .meta
            .unknown_fields
            .iter()
            .any(|f| f == "execution")
    );
    assert_eq!(host_local.lifecycle, WorkerLifecycle::Reserved);
    assert_eq!(
        host_local.model_services,
        vec![reference("animus-serving", "svc-shared")],
        "served routes bind exact serving service ids"
    );
    let first = fragment
        .workers
        .iter()
        .find(|w| w.execution == reference("aspen-compute", "agent-a-1"))
        .ok_or("missing first worker")?;
    assert_eq!(first.definition, reference("lattice-agents", PROFILE));
    assert_eq!(first.attempt.as_deref(), Some("attempt-agent-a-1"));
    assert_eq!(first.fence, Some(7));
    assert_eq!(first.lifecycle, WorkerLifecycle::Unknown);
    assert!(first.model_services.is_empty());
    assert!(
        first
            .meta
            .unknown_fields
            .iter()
            .any(|f| f == "model_services"),
        "a route without a serving reading is an unknown binding, not none"
    );
    Ok(())
}

#[test]
fn lifecycles_map_conservatively() -> TestResult {
    let admitted = workers(&[ADMITTED])?;
    let terminal = workers(&[TERMINAL])?;
    assert_eq!(
        admitted.workers.first().map(|w| w.lifecycle),
        Some(WorkerLifecycle::Reserved)
    );
    assert_eq!(
        terminal.workers.first().map(|w| w.lifecycle),
        Some(WorkerLifecycle::Stopped)
    );
    let cancelled = edit(
        FIRST,
        "/instance/cancellation",
        "requested_unobserved".into(),
    )?;
    let fragment = workers(&[&cancelled])?;
    assert!(
        fragment.workers.first().is_some_and(|w| w
            .meta
            .unknown_fields
            .iter()
            .any(|f| f == "cancellation"))
    );
    Ok(())
}

#[test]
fn invalid_documents_refuse_with_named_reasons() -> TestResult {
    let unsupported = edit(FIRST, "/schema", "lattice.worker-inventory.v2".into())?;
    assert_eq!(
        workers(&[&unsupported]).err(),
        Some("lattice_inventory_unsupported_schema")
    );
    let mut extra: Value = serde_json::from_slice(FIRST).map_err(|e| e.to_string())?;
    extra
        .as_object_mut()
        .ok_or("not an object")?
        .insert("unexpected".into(), Value::Bool(true));
    let extra = serde_json::to_vec(&extra).map_err(|e| e.to_string())?;
    assert_eq!(
        workers(&[&extra]).err(),
        Some("lattice_inventory_malformed")
    );
    assert_eq!(workers(&[b"{"]).err(), Some("lattice_inventory_malformed"));
    let inconsistent = edit(FIRST, "/profile/outputs/value", Value::Null)?;
    assert_eq!(
        agents(&[&inconsistent]).err(),
        Some("lattice_fact_support_inconsistent")
    );
    let conflicting = edit(SECOND, "/profile/outputs/value", "other-contract".into())?;
    assert_eq!(
        agents(&[FIRST, &conflicting]).err(),
        Some("lattice_profile_conflict")
    );
    assert_eq!(
        workers(&[FIRST, TERMINAL]).err(),
        Some("lattice_instance_conflict"),
        "two states for one incarnation are not ordered by Lattice"
    );
    let mismatched = edit(FIRST, "/instance/profile_revision", "other-profile".into())?;
    assert_eq!(
        workers(&[&mismatched]).err(),
        Some("lattice_instance_profile_mismatch")
    );
    let liveness = edit(
        FIRST,
        "/instance/allocation_liveness/support",
        "declared".into(),
    )?;
    assert_eq!(
        workers(&[&liveness]).err(),
        Some("lattice_liveness_unsupported")
    );
    let mut early = source("lattice-workers");
    early.observed_at_ms = 1_000;
    assert_eq!(
        map_worker_inventory(InventoryKind::Workers, &early, &joins(), &[FIRST]).err(),
        Some("lattice_instance_after_collection")
    );
    assert_eq!(
        map_worker_inventory(InventoryKind::Compute, &source("x"), &joins(), &[FIRST]).err(),
        Some("lattice_inventory_kind_unsupported")
    );
    let oversized = vec![b' '; super::MAX_WORKER_INVENTORY_BYTES + 1];
    assert_eq!(
        workers(&[&oversized]).err(),
        Some("lattice_inventory_byte_bound")
    );
    let mut bad_joins = joins();
    bad_joins.compute = "../compute".into();
    assert_eq!(
        map_worker_inventory(InventoryKind::Workers, &source("w"), &bad_joins, &[FIRST]).err(),
        Some("inventory_public_identifier_invalid")
    );
    Ok(())
}
