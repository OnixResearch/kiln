//! Workflow revision slots through explicit collection: the Lattice
//! `7fb6dc47` capture in the pilot manifest, and every refused slots document
//! making only its own source unavailable.
// r[verify kiln.inventory.workflow.scenario.isolated]
// r[verify kiln.inventory.workflow.scenario.mapped]
use std::path::{Path, PathBuf};

use kiln_core::inventory::{
    InventoryProfile, InventorySnapshot, ObservationState, project_inventory,
};
use serde_json::{Value, json};

use super::{CollectionSet, collect_input, collect_snapshot};

type TestResult<T = ()> = Result<T, String>;

const NOW_MS: u64 = 1_795_237_895_000;
const PILOT: &str = "pilot.collection.json";
const SOURCE: &str = "lattice-workflows";
const CAPTURE: &str = "lattice-workflows/workflow-4-second-revision.json";
const DEFINITION: &str = "blake3:176820ae10f42401cfc32aa5e9cdd755cd55a5e9c1255bee5acd5d75b0aaa842";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/producers")
}

fn profile() -> TestResult<InventoryProfile> {
    let path = fixtures().join("profile.json");
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
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

/// The pilot manifest collected from `base` with the workflow source naming `document`.
fn collect_naming(base: &Path, document: &str) -> TestResult<InventorySnapshot> {
    let mut set = CollectionSet::load(&base.join(PILOT))?;
    for collection in set.collections.iter_mut().filter(|c| c.source == SOURCE) {
        collection.documents = vec![document.to_owned()];
    }
    set.collect(base)
}

fn workflow_reason(snapshot: &InventorySnapshot) -> Option<&str> {
    snapshot
        .sources
        .iter()
        .find(|source| source.source == SOURCE)
        .and_then(|source| source.unavailable_reason.as_deref())
}

/// Everything but the workflow source and its facts.
fn without_workflows(snapshot: &InventorySnapshot) -> InventorySnapshot {
    let mut rest = snapshot.clone();
    rest.sources.retain(|source| source.source != SOURCE);
    rest.workflow_revisions.clear();
    rest
}

/// `value` with the JSON pointer replaced or, when absent, its last key inserted.
fn edited(value: &Value, pointer: &str, replacement: Value) -> TestResult<Value> {
    let mut value = value.clone();
    let (parent, key) = pointer.rsplit_once('/').ok_or("pointer")?;
    match value.pointer_mut(pointer) {
        Some(slot) => *slot = replacement,
        None => {
            value
                .pointer_mut(parent)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("missing {parent}"))?
                .insert(key.to_owned(), replacement);
        }
    }
    Ok(value)
}

fn write_json(path: &Path, value: &Value) -> TestResult {
    std::fs::write(path, serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[test]
fn the_pilot_collects_the_captured_workflow_slots() -> TestResult {
    let snapshot = collect_snapshot(&fixtures().join(PILOT))?;
    let [slots] = snapshot.workflow_revisions.as_slice() else {
        return Err("one workflow fact".into());
    };
    assert_eq!(slots.meta.reference.source, SOURCE);
    assert_eq!(slots.meta.reference.id, DEFINITION);
    assert_eq!(workflow_reason(&snapshot), None);
    let projection = project_inventory(&profile()?, &snapshot, None, NOW_MS)?;
    let source = projection
        .sources
        .iter()
        .find(|source| source.source == SOURCE)
        .ok_or("missing workflow source")?;
    assert_eq!(source.state, ObservationState::Current);
    let input = collect_input(&fixtures().join(PILOT), &fixtures().join("profile.json"))?;
    assert_eq!(input.snapshot.workflow_revisions.len(), 1);
    Ok(())
}

#[test]
fn a_refused_slots_document_makes_only_its_source_unavailable() -> TestResult {
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_tree(&fixtures(), scratch.path())?;
    let base = scratch.path();
    let good = collect_naming(base, CAPTURE)?;
    let capture: Value =
        serde_json::from_slice(&std::fs::read(base.join(CAPTURE)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let wrong_schema = edited(
        &capture,
        "/schema",
        json!("lattice.workflow-agent-revision-slots.v2"),
    )?;
    write_json(
        &base.join("lattice-workflows/wrong-schema.json"),
        &wrong_schema,
    )?;
    let extra = edited(
        &capture,
        "/desired/workflow/guidance_identity",
        json!("b3:guidance"),
    )?;
    write_json(&base.join("lattice-workflows/extra-field.json"), &extra)?;
    let mismatched = edited(
        &capture,
        "/pinned/0/bundle/worker/definition",
        json!(format!("blake3:{}", "0".repeat(64))),
    )?;
    write_json(
        &base.join("lattice-workflows/mismatched-worker.json"),
        &mismatched,
    )?;
    std::fs::write(
        base.join("lattice-workflows/oversized.json"),
        vec![b' '; super::MAX_PRODUCER_DOCUMENT_BYTES as usize + 1],
    )
    .map_err(|e| e.to_string())?;
    for (document, reason) in [
        (
            "lattice-workflows/wrong-schema.json",
            "lattice_workflow_slots_unsupported_schema",
        ),
        (
            "lattice-workflows/extra-field.json",
            "lattice_workflow_slots_malformed",
        ),
        (
            "lattice-workflows/mismatched-worker.json",
            "lattice_workflow_worker_binding_mismatch",
        ),
        (
            "lattice-workflows/oversized.json",
            "inventory_collection_byte_bound",
        ),
        (
            "lattice-workflows/missing.json",
            "inventory_collection_document_unavailable",
        ),
    ] {
        let snapshot = collect_naming(base, document)?;
        assert_eq!(workflow_reason(&snapshot), Some(reason), "{document}");
        assert!(snapshot.workflow_revisions.is_empty(), "{document}");
        assert_eq!(
            without_workflows(&snapshot),
            without_workflows(&good),
            "{document}: every other source keeps its facts"
        );
        let projection = project_inventory(&profile()?, &snapshot, None, NOW_MS)?;
        let source = projection
            .sources
            .iter()
            .find(|source| source.source == SOURCE)
            .ok_or("missing workflow source")?;
        assert_eq!(source.state, ObservationState::Unavailable);
        assert_eq!(source.reason.as_deref(), Some(reason));
    }
    Ok(())
}

#[test]
fn manifest_defects_of_a_workflow_source_still_refuse_the_collection() -> TestResult {
    assert_eq!(
        collect_naming(&fixtures(), "../profile.json")
            .err()
            .as_deref(),
        Some("inventory_collection_path_invalid")
    );
    let mut set = CollectionSet::load(&fixtures().join(PILOT))?;
    for collection in set.collections.iter_mut().filter(|c| c.source == SOURCE) {
        collection.contract = "lattice.worker-inventory.v3".into();
    }
    assert_eq!(
        set.collect(&fixtures()).err().as_deref(),
        Some("inventory_collection_contract_unsupported")
    );
    let mut agents = CollectionSet::load(&fixtures().join(PILOT))?;
    for collection in agents
        .collections
        .iter_mut()
        .filter(|c| c.source == "lattice-agents")
    {
        collection.documents = vec![CAPTURE.to_owned()];
    }
    assert_eq!(
        agents.collect(&fixtures()).err().as_deref(),
        Some("lattice_inventory_malformed"),
        "only the display-only workflow source is isolated"
    );
    Ok(())
}
