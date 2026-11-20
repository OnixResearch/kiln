//! Kiln #60 acceptance over producer-shaped fixtures: Lattice `inspect`
//! output captured at Lattice main 1bdacb35, compute allocation observations
//! from Lattice's fixtures, and Animus #17 serving snapshots. Each test names
//! the criterion it covers. Everything here is deterministic fixture evidence;
//! no live provider, fleet or capacity is claimed.
// r[verify kiln.inventory.collection]
// r[verify kiln.inventory.collection.scenario.parity]
// r[verify kiln.inventory.collection.scenario.hostname]
// r[verify kiln.inventory.collection.scenario.refused]
// r[verify kiln.inventory.producers]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::inventory::{
    FactRef, Health, InventoryProfile, InventoryProjection, InventorySnapshot, ObservationState,
    ReadinessExplanation, ServingState, WorkNeeds, WorkerLifecycle, explain_readiness,
    project_inventory,
};
use kiln_provider_api::inventory::{InventoryObservationPort, InventoryReadFailure};
use serde_json::Value;

use super::{
    CollectionSet, ExplicitCollectionPort, collect_input, collect_snapshot, run_collect_cli_args,
};
use crate::inventory::{InventoryInput, read_inventory, render_html, run_cli_args};

type TestResult<T = ()> = Result<T, String>;

/// A clock five seconds after the fixtures' collection time.
const NOW_MS: u64 = 1_795_200_625_000;
const PROFILE: &str = "8a836baf7f4924334a2d50ced5590fd4f5a70e91bc67ee04c667c00722b17b72";
const PILOT: &str = "pilot.collection.json";
const SHARED_POOL: &str = "shared-pool.collection.json";
const CONFIGURED_ONLY: &str = "configured-only.collection.json";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/producers")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> TestResult<T> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn profile() -> TestResult<InventoryProfile> {
    read_json(&fixtures().join("profile.json"))
}

fn needs() -> TestResult<WorkNeeds> {
    read_json(&fixtures().join("work-needs.json"))
}

fn collect(name: &str) -> TestResult<InventorySnapshot> {
    collect_snapshot(&fixtures().join(name))
}

/// One edit of a loaded manifest, paired with the source state it must produce.
type ManifestVariant = (ObservationState, fn(&mut CollectionSet));

fn collect_with(
    name: &str,
    change: impl FnOnce(&mut CollectionSet),
) -> TestResult<InventorySnapshot> {
    let mut set = CollectionSet::load(&fixtures().join(name))?;
    change(&mut set);
    set.collect(&fixtures())
}

fn project(
    snapshot: &InventorySnapshot,
    needs: Option<&WorkNeeds>,
) -> TestResult<InventoryProjection> {
    project_inventory(&profile()?, snapshot, needs, NOW_MS).map_err(str::to_owned)
}

fn explain(snapshot: &InventorySnapshot, needs: &WorkNeeds) -> TestResult<ReadinessExplanation> {
    explain_readiness(&profile()?, snapshot, needs, NOW_MS).map_err(str::to_owned)
}

fn reference(source: &str, id: &str) -> FactRef {
    FactRef {
        source: source.into(),
        id: id.into(),
    }
}

fn candidate_codes(
    explanation: &ReadinessExplanation,
    worker: &str,
    model: &str,
) -> TestResult<BTreeSet<String>> {
    let candidate = explanation
        .candidates
        .iter()
        .find(|c| c.execution.id == worker && c.model.id == model)
        .ok_or_else(|| format!("missing candidate {worker} x {model}"))?;
    Ok(candidate.blockers.iter().map(|b| b.code.clone()).collect())
}

fn source_state(projection: &InventoryProjection, source: &str) -> Option<ObservationState> {
    projection
        .sources
        .iter()
        .find(|s| s.source == source)
        .map(|s| s.state)
}

fn copy_tree(from: &Path, to: &Path) -> TestResult {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn edit_file(path: &Path, edits: &[(&str, Value)]) -> TestResult<Vec<u8>> {
    let mut value: Value = read_json(path)?;
    for (pointer, replacement) in edits {
        let slot = value
            .pointer_mut(pointer)
            .ok_or_else(|| format!("missing {pointer}"))?;
        *slot = replacement.clone();
    }
    serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())
}

fn tree_digest(root: &Path) -> TestResult<Vec<(PathBuf, String)>> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                entries.push((path, blake3::hash(&bytes).to_hex().to_string()));
            }
        }
    }
    entries.sort();
    Ok(entries)
}

/// #60: a registered agent with no running instances is visible as a
/// definition and does not inflate the idle-worker count.
#[test]
fn registered_definition_without_instances_is_not_an_idle_worker() -> TestResult {
    let configured = project(&collect(CONFIGURED_ONLY)?, Some(&needs()?))?;
    assert_eq!(configured.snapshot.agents.len(), 1);
    assert!(configured.snapshot.workers.is_empty());
    let explanation = configured
        .explanation
        .as_ref()
        .ok_or("missing explanation")?;
    assert!(
        explanation
            .blockers
            .iter()
            .any(|b| b.code == "no_observed_worker")
    );
    for source in ["aspen-compute", "animus-serving"] {
        assert_eq!(
            source_state(&configured, source),
            Some(ObservationState::Missing),
            "an uncollected provider is missing, never an empty fleet"
        );
    }
    let pilot = project(&collect(PILOT)?, None)?;
    assert_eq!(pilot.snapshot.agents.len(), 2);
    assert_eq!(pilot.snapshot.workers.len(), 3);
    let idle = pilot
        .snapshot
        .workers
        .iter()
        .filter(|w| w.lifecycle == WorkerLifecycle::Idle)
        .count();
    assert_eq!(
        idle, 0,
        "definitions and retained attempts are never idle workers"
    );
    Ok(())
}

/// #60: two workers on one execution host share a separately placed model
/// deployment; all bindings are visible and the deployment's accelerator
/// allocation is counted once.
#[test]
fn shared_deployment_bindings_are_visible_and_counted_once() -> TestResult {
    let projection = project(&collect(PILOT)?, None)?;
    let snapshot = &projection.snapshot;
    let serving_allocation = reference("aspen-compute", "serving-b");
    for id in ["agent-a-1", "agent-a-2"] {
        let worker = snapshot
            .workers
            .iter()
            .find(|w| w.execution == reference("aspen-compute", id))
            .ok_or("missing worker")?;
        assert_eq!(worker.definition, reference("lattice-agents", PROFILE));
        let execution = snapshot
            .compute
            .iter()
            .find(|c| c.meta.reference == worker.execution)
            .ok_or("missing execution allocation")?;
        assert_eq!(execution.host, "host-a");
        assert_eq!(execution.shared_serving, vec![serving_allocation.clone()]);
    }
    let deployment = snapshot
        .compute
        .iter()
        .find(|c| c.meta.reference == serving_allocation)
        .ok_or("missing deployment allocation")?;
    assert_eq!(
        deployment.host, "host-b",
        "the deployment is placed separately"
    );
    let service = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-serving-b")
        .ok_or("missing service")?;
    assert_eq!(service.compute_devices, vec![serving_allocation]);
    assert_eq!(
        service.accounting_pools,
        vec![
            reference("aspen-compute", "host-b-chip"),
            reference("aspen-compute", "host-b-shared-memory")
        ]
    );
    let charges: Vec<_> = snapshot
        .pools
        .iter()
        .flat_map(|p| {
            p.allocations
                .iter()
                .map(move |a| (p.meta.reference.id.as_str(), a))
        })
        .filter(|(_, a)| a.allocation == "serving-b")
        .map(|(pool, a)| (pool, a.amount))
        .collect();
    assert_eq!(
        charges,
        vec![("host-b-chip", 1), ("host-b-shared-memory", 80)],
        "one accelerator and its memory, charged once for both workers"
    );
    for pool in snapshot
        .pools
        .iter()
        .filter(|p| p.meta.reference.id.starts_with("host-b"))
    {
        assert!(pool.allocations.iter().all(|a| a.allocation == "serving-b"));
    }
    Ok(())
}

/// #60: one physical or shared memory pool used by execution and serving does
/// not appear as two independent allocatable capacities.
#[test]
fn shared_execution_and_serving_pool_is_one_capacity() -> TestResult {
    let projection = project(&collect(SHARED_POOL)?, None)?;
    let snapshot = &projection.snapshot;
    let pool_ref = reference("aspen-compute", "host-a-memory");
    let pools: Vec<_> = snapshot
        .pools
        .iter()
        .filter(|p| p.meta.reference == pool_ref)
        .collect();
    assert_eq!(pools.len(), 1);
    let pool = pools.first().ok_or("missing pool")?;
    let allocations: Vec<_> = pool
        .allocations
        .iter()
        .map(|a| a.allocation.as_str())
        .collect();
    assert_eq!(allocations, vec!["agent-a-1", "agent-a-2", "serving-a"]);
    assert_eq!(
        (pool.capacity, pool.allocatable),
        (None, None),
        "unknown, never free"
    );
    let loading = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-loading")
        .ok_or("missing service")?;
    assert!(loading.accounting_pools.contains(&pool_ref));
    let execution = snapshot
        .compute
        .iter()
        .find(|c| c.meta.reference.id == "agent-a-1")
        .ok_or("missing execution")?;
    assert!(execution.accounting_pools.contains(&pool_ref));
    Ok(())
}

/// #60: hardware present, artifact cached, model loading, ready but saturated
/// and request admitted stay distinct observations and explanations.
#[test]
fn hardware_artifact_loading_saturation_and_admission_stay_distinct() -> TestResult {
    let snapshot = collect(PILOT)?;
    let explanation = explain(&snapshot, &needs()?)?;
    let loading = candidate_codes(&explanation, "agent-a-1", "svc-loading")?;
    let saturated = candidate_codes(&explanation, "agent-a-1", "svc-saturated")?;
    let ready = candidate_codes(&explanation, "agent-a-1", "svc-serving-b")?;
    assert!(loading.contains("model_loading") && !loading.contains("inference_saturated"));
    assert!(saturated.contains("inference_saturated") && !saturated.contains("model_loading"));
    assert!(!ready.contains("model_loading") && !ready.contains("inference_saturated"));
    let hardware = snapshot
        .compute
        .iter()
        .find(|c| c.meta.reference.id == "agent-a-device")
        .ok_or("missing device allocation")?;
    assert_eq!(hardware.health, Health::Healthy);
    assert!(
        snapshot
            .models
            .iter()
            .all(|m| !m.compute_devices.contains(&hardware.meta.reference)),
        "present hardware is not a loaded model"
    );
    for model in &snapshot.models {
        assert_eq!(
            model.artifact_available, None,
            "no producer publishes artifact cache state"
        );
        assert!(
            model
                .meta
                .unknown_fields
                .iter()
                .any(|f| f == "artifact_available")
        );
    }
    let states: BTreeSet<_> = snapshot
        .models
        .iter()
        .map(|m| format!("{:?}", m.state))
        .collect();
    for state in [
        ServingState::Loading,
        ServingState::Saturated,
        ServingState::Warm,
    ] {
        assert!(states.contains(&format!("{state:?}")));
    }
    assert!(
        snapshot.admissions.is_empty(),
        "a serving snapshot is never a request admission"
    );
    assert!(!explanation.dispatch_authorized);
    Ok(())
}

/// #60: missing or stale providers, partial pages and out-of-order
/// generations stay unknown or incomplete and never produce ready or free.
#[test]
fn missing_stale_partial_and_out_of_order_inputs_never_ready() -> TestResult {
    let needs = needs()?;
    let variants: [ManifestVariant; 5] = [
        (ObservationState::Missing, |set| {
            set.collections.retain(|c| c.source != "animus-serving");
        }),
        (ObservationState::Stale, |set| {
            for c in set
                .collections
                .iter_mut()
                .filter(|c| c.source == "animus-serving")
            {
                c.observed_at_ms = NOW_MS - 120_000;
                c.valid_until_ms = NOW_MS - 1_000;
            }
        }),
        (ObservationState::Incomplete, |set| {
            for c in set
                .collections
                .iter_mut()
                .filter(|c| c.source == "animus-serving")
            {
                c.complete = false;
                c.next_page = Some("page-2".into());
            }
        }),
        (ObservationState::OutOfOrder, |set| {
            for c in set
                .collections
                .iter_mut()
                .filter(|c| c.source == "animus-serving")
            {
                c.generation = 0;
            }
        }),
        (ObservationState::Unavailable, |set| {
            for c in set
                .collections
                .iter_mut()
                .filter(|c| c.source == "animus-serving")
            {
                c.unavailable_reason = Some("provider_unreachable".into());
                c.documents.clear();
            }
        }),
    ];
    for (expected, change) in variants {
        let snapshot = collect_with(PILOT, change)?;
        let projection = project(&snapshot, Some(&needs))?;
        assert_eq!(source_state(&projection, "animus-serving"), Some(expected));
        let explanation = projection
            .explanation
            .as_ref()
            .ok_or("missing explanation")?;
        assert!(
            explanation
                .blockers
                .iter()
                .any(|b| b.code == "provider_observation_unknown")
        );
        assert!(explanation.candidates.iter().all(|c| !c.observed_eligible));
    }
    let unclocked = collect_with(PILOT, |set| {
        for c in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "animus-serving")
        {
            c.provider_logical_now = Some(200);
        }
    })?;
    let projection = project(&unclocked, None)?;
    for fact in projection
        .facts
        .iter()
        .filter(|f| f.reference.source == "animus-serving")
    {
        assert_ne!(
            fact.state,
            ObservationState::Current,
            "an expired producer clock is not current"
        );
    }
    let partial = collect_with(SHARED_POOL, |set| {
        for c in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "aspen-compute")
        {
            c.documents.retain(|d| !d.ends_with("device-active.json"));
        }
    })?;
    let projection = project(&partial, None)?;
    let pool = projection
        .facts
        .iter()
        .find(|f| f.reference == reference("aspen-compute", "host-a-memory"))
        .ok_or("missing pool state")?;
    assert_eq!(
        pool.state,
        ObservationState::Unknown,
        "a partial readback page is not complete"
    );
    let pilot = project(&collect(PILOT)?, Some(&needs))?;
    assert!(pilot.snapshot.pools.iter().all(|p| p.allocatable.is_none()));
    let explanation = pilot.explanation.as_ref().ok_or("missing explanation")?;
    assert!(explanation.candidates.iter().all(|c| !c.observed_eligible));
    Ok(())
}

/// #60: an unsupported context, tool capability, sandbox or backend reports
/// the exact unmet constraint; a hosted API has no fictitious accelerator.
#[test]
fn unsupported_constraints_are_exact_and_hosted_api_has_no_accelerator() -> TestResult {
    let snapshot = collect(PILOT)?;
    let mut needs = needs()?;
    needs.context_tokens = 200_000;
    needs.execution_backend = "rocm".into();
    let explanation = explain(&snapshot, &needs)?;
    let blockers: Vec<_> = explanation
        .candidates
        .iter()
        .filter(|c| c.execution.id == "agent-a-1" && c.model.id == "svc-serving-b")
        .flat_map(|c| c.blockers.iter())
        .map(|b| (b.code.as_str(), b.subject.clone(), b.constraint.as_str()))
        .collect();
    let service = Some(reference("animus-serving", "svc-serving-b"));
    let execution = Some(reference("aspen-compute", "agent-a-1"));
    for expected in [
        ("unsupported_context", service.clone(), "200000"),
        ("unsupported_model_tool", service.clone(), "edit"),
        ("unsupported_sandbox", execution.clone(), "required-sandbox"),
        ("incompatible_execution_backend", execution, "rocm"),
    ] {
        assert!(blockers.contains(&expected), "missing {expected:?}");
    }
    needs.model_backend = "hosted-remote".into();
    let explanation = explain(&snapshot, &needs)?;
    assert!(
        candidate_codes(&explanation, "agent-a-1", "svc-serving-b")?
            .contains("incompatible_backend")
    );
    assert!(
        !candidate_codes(&explanation, "agent-a-1", "svc-hosted")?.contains("incompatible_backend")
    );
    let hosted = snapshot
        .models
        .iter()
        .find(|m| m.meta.reference.id == "svc-hosted")
        .ok_or("missing hosted service")?;
    assert!(hosted.compute_devices.is_empty() && hosted.accounting_pools.is_empty());
    assert!(
        !candidate_codes(&explanation, "agent-a-1", "svc-hosted")?
            .contains("serving_compute_unknown_or_unavailable")
    );
    assert_eq!(
        snapshot.compute.len(),
        4,
        "no accelerator is invented for the hosted API"
    );
    Ok(())
}

/// #60: a resource changes after explanation; the new observation shows the
/// change and no explanation is dispatch authority.
#[test]
fn resource_change_after_explanation_cannot_dispatch_on_the_old_view() -> TestResult {
    let needs = needs()?;
    let before = project(&collect(PILOT)?, Some(&needs))?;
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_tree(&fixtures(), scratch.path())?;
    let released = edit_file(
        &scratch.path().join("compute/agent-a-1-active.json"),
        &[
            ("/port/observation_sequence", 2.into()),
            ("/allocation/status", "released".into()),
            ("/allocation/activation_backend_ref", Value::Null),
            ("/readback/row/status", "released".into()),
            ("/readback/row/is_logically_fenced", false.into()),
        ],
    )?;
    std::fs::write(
        scratch.path().join("compute/agent-a-1-released.json"),
        released,
    )
    .map_err(|e| e.to_string())?;
    let mut set = CollectionSet::load(&scratch.path().join(PILOT))?;
    for c in set
        .collections
        .iter_mut()
        .filter(|c| c.source == "aspen-compute")
    {
        c.generation = 2;
        c.documents.push("compute/agent-a-1-released.json".into());
    }
    let after = project(&set.collect(scratch.path())?, Some(&needs))?;
    assert_ne!(before.identity, after.identity);
    let old = before.explanation.as_ref().ok_or("missing explanation")?;
    let new = after.explanation.as_ref().ok_or("missing explanation")?;
    assert!(
        !candidate_codes(old, "agent-a-1", "svc-serving-b")?.contains("execution_offer_unknown")
    );
    assert!(
        candidate_codes(new, "agent-a-1", "svc-serving-b")?.contains("execution_offer_unknown")
    );
    assert!(!old.dispatch_authorized && !new.dispatch_authorized);
    assert!(
        after
            .snapshot
            .pools
            .iter()
            .flat_map(|p| p.allocations.iter())
            .all(|a| a.allocation != "agent-a-1"),
        "a released allocation no longer consumes its pool"
    );
    Ok(())
}

/// #60: CLI, live and offline rendering of one admitted snapshot give the same
/// bounded projection, identities and redaction; offline rendering reads only
/// named files and mutates nothing.
#[test]
fn cli_live_and_offline_share_one_projection_without_mutation() -> TestResult {
    let before = tree_digest(&fixtures())?;
    let manifest = fixtures().join(PILOT);
    let profile_path = fixtures().join("profile.json");
    let input = collect_input(&manifest, &profile_path)?;
    let printed: InventoryInput = serde_json::from_str(&run_collect_cli_args(&[
        manifest.display().to_string(),
        profile_path.display().to_string(),
    ])?)
    .map_err(|e| e.to_string())?;
    assert_eq!(printed.snapshot, input.snapshot);
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let input_path = scratch.path().join("inventory-input.json");
    std::fs::write(
        &input_path,
        serde_json::to_vec(&input).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let args = |format: &str| {
        vec![
            input_path.display().to_string(),
            "operator".to_owned(),
            NOW_MS.to_string(),
            format.to_owned(),
        ]
    };
    let cli: InventoryProjection =
        serde_json::from_str(&run_cli_args(&args("json"))?).map_err(|e| e.to_string())?;
    let live = read_inventory(&input_path, "operator", NOW_MS)?;
    let html = run_cli_args(&args("html"))?;
    assert_eq!(cli, live);
    assert_eq!(cli.identity, live.identity);
    assert_eq!(html, render_html(&live)?);
    assert!(html.contains(&live.identity) && html.contains(&live.profile_identity));
    assert!(!html.contains("<script") && !html.contains("src="));
    assert!(!html.contains("http://") && !html.contains("https://"));
    let mut port = ExplicitCollectionPort::new(manifest.clone());
    assert_eq!(
        port.observe_inventory("operator")
            .map_err(|e| format!("{e:?}"))?,
        input.snapshot
    );
    assert_eq!(
        port.observe_inventory("other-principal").err(),
        Some(InventoryReadFailure::PrincipalDenied)
    );
    let text = serde_json::to_string(&live).map_err(|e| e.to_string())?;
    for withheld in [
        "endpoint:",
        "synthetic-backend-agent-a-1",
        "2222222222222222222222222222222222222222222222222222222222222222",
        "/workspace",
        "MODEL_TOKEN",
        "model.invalid",
        "blake3:eeeeeeee",
    ] {
        assert!(
            !text.contains(withheld),
            "{withheld} leaked into the projection"
        );
    }
    assert!(
        live.snapshot
            .compute
            .iter()
            .all(|c| c.qualification.is_empty())
    );
    assert!(
        live.snapshot.synthetic,
        "fixture snapshots stay marked synthetic"
    );
    assert_eq!(
        tree_digest(&fixtures())?,
        before,
        "collection and rendering wrote nothing"
    );
    Ok(())
}

/// #60: existing inventory v2 snapshots keep their encoding and identity.
#[test]
fn existing_inventory_v2_snapshots_keep_their_identity() -> TestResult {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/synthetic-v2.json");
    let original: Value = read_json(&path)?;
    let input: InventoryInput = read_json(&path)?;
    assert_eq!(
        serde_json::to_value(&input.snapshot.compute).map_err(|e| e.to_string())?,
        original
            .pointer("/snapshot/compute")
            .cloned()
            .ok_or("missing compute")?,
        "an empty shared-serving list is omitted, so earlier compute facts encode unchanged"
    );
    let first = input.project("operator", None, 2000)?;
    let second = input.project("operator", None, 2000)?;
    assert_eq!(first.identity, second.identity);
    assert!(
        !serde_json::to_string(&first)
            .map_err(|e| e.to_string())?
            .contains("shared_serving")
    );
    Ok(())
}

/// Host names never join inventories: only exact producer identities do.
#[test]
fn host_names_never_join_independent_inventories() -> TestResult {
    let snapshot = collect(PILOT)?;
    let host_local = snapshot
        .workers
        .iter()
        .find(|w| w.execution.source == "lattice-workers")
        .ok_or("missing host-local worker")?;
    assert!(
        snapshot
            .compute
            .iter()
            .all(|c| c.meta.reference != host_local.execution)
    );
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_tree(&fixtures(), scratch.path())?;
    let renamed = scratch
        .path()
        .join("lattice/agent-a-2-dispatched-outcome-unknown.json");
    let bytes = edit_file(
        &renamed,
        &[("/instance/compute/allocation_id", "agent-a-9".into())],
    )?;
    std::fs::write(&renamed, bytes).map_err(|e| e.to_string())?;
    let snapshot = collect_snapshot(&scratch.path().join(PILOT))?;
    let worker = snapshot
        .workers
        .iter()
        .find(|w| w.execution.id == "agent-a-9")
        .ok_or("missing renamed worker")?;
    assert!(snapshot.compute.iter().any(|c| c.host == "host-a"));
    assert!(
        snapshot
            .compute
            .iter()
            .all(|c| c.meta.reference != worker.execution)
    );
    let explanation = explain(&snapshot, &needs()?)?;
    assert!(
        candidate_codes(&explanation, "agent-a-9", "svc-serving-b")?
            .contains("execution_offer_unknown")
    );
    Ok(())
}

#[test]
fn unsafe_or_unsupported_manifests_refuse_before_projection() -> TestResult {
    for (path, expected) in [
        ("/etc/passwd", "inventory_collection_path_invalid"),
        ("../profile.json", "inventory_collection_path_invalid"),
        (
            "lattice/../profile.json",
            "inventory_collection_path_invalid",
        ),
        (
            "lattice/missing.json",
            "inventory_collection_document_unavailable",
        ),
    ] {
        let result = collect_with(PILOT, |set| {
            for c in set
                .collections
                .iter_mut()
                .filter(|c| c.source == "lattice-agents")
            {
                c.documents = vec![path.to_owned()];
            }
        });
        assert_eq!(result.err().as_deref(), Some(expected), "{path}");
    }
    let wrong_contract = collect_with(PILOT, |set| {
        for c in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "aspen-compute")
        {
            c.contract = "lattice.worker-inventory.v1".into();
        }
    });
    assert_eq!(
        wrong_contract.err().as_deref(),
        Some("inventory_collection_contract_unsupported")
    );
    let unavailable_with_documents = collect_with(PILOT, |set| {
        for c in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "aspen-compute")
        {
            c.unavailable_reason = Some("provider_unreachable".into());
        }
    });
    assert_eq!(
        unavailable_with_documents.err().as_deref(),
        Some("inventory_collection_unavailable_with_documents")
    );
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_tree(&fixtures(), scratch.path())?;
    let manifest = scratch.path().join(PILOT);
    let mut value: Value = read_json(&manifest)?;
    value
        .as_object_mut()
        .ok_or("manifest is not an object")?
        .insert("discover".into(), true.into());
    std::fs::write(
        &manifest,
        serde_json::to_vec(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(
        collect_snapshot(&manifest).err().as_deref(),
        Some("inventory_collection_invalid")
    );
    let bytes = edit_file(
        &scratch.path().join(CONFIGURED_ONLY),
        &[("/schema", "kiln.inventory.collection-set.v2".into())],
    )?;
    std::fs::write(scratch.path().join(CONFIGURED_ONLY), bytes).map_err(|e| e.to_string())?;
    assert_eq!(
        collect_snapshot(&scratch.path().join(CONFIGURED_ONLY))
            .err()
            .as_deref(),
        Some("inventory_collection_unsupported")
    );
    let oversized = scratch.path().join("lattice/oversized.json");
    std::fs::write(&oversized, vec![b' '; 262_145]).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        scratch.path().join("profile.json"),
        scratch.path().join("lattice/link.json"),
    )
    .map_err(|e| e.to_string())?;
    for (document, expected) in [
        ("lattice/oversized.json", "inventory_collection_byte_bound"),
        (
            "lattice/link.json",
            "inventory_collection_document_not_regular",
        ),
    ] {
        let mut set = CollectionSet::load(&scratch.path().join(SHARED_POOL))?;
        for c in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "lattice-agents")
        {
            c.documents = vec![document.to_owned()];
        }
        assert_eq!(set.collect(scratch.path()).err().as_deref(), Some(expected));
    }
    let mut denied = profile()?;
    denied.principal = "other-principal".into();
    let denied_path = scratch.path().join("denied-profile.json");
    std::fs::write(
        &denied_path,
        serde_json::to_vec(&denied).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(
        collect_input(&fixtures().join(PILOT), &denied_path)
            .err()
            .as_deref(),
        Some("inventory_principal_denied")
    );
    Ok(())
}
