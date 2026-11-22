//! The Cairn workspace's workflow revision slot over the Lattice `7fb6dc47`
//! `lattice.workflow-agent-revision-slots.v1` captures, collected by Kiln's
//! own reader and admitted by the inventory projection, as the detail route
//! reads them. Deterministic fixture evidence only.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::inventory::{
    InventoryKind, InventoryProfile, InventorySnapshot, PinStatus, SelectionKind, project_inventory,
};

use super::{AgentRevisions, InventoryObservation, WorkflowDefinitionView, project};
use crate::inventory_collection::CollectionSet;

type TestResult<T = ()> = Result<T, String>;

const NOW_MS: u64 = 1_795_237_895_000;
const PILOT: &str = "pilot.collection.json";
const SOURCE: &str = "lattice-workflows";
const DEFINITION: &str = "blake3:176820ae10f42401cfc32aa5e9cdd755cd55a5e9c1255bee5acd5d75b0aaa842";
const FIRST: &str = "555665f511e3244805be21e30e782770151ca46182e74c6c5260bcdff45817ca";
const SECOND: &str = "02dad1b763859fca0fc25d43b03cbbc3fd4059e72215c1661411178fbe9dca4b";
const WORKER_DEFINITION: &str =
    "blake3:367783379a41fd2e834f26af4e7504906c1f3967bd9f21bba979441a1f0789c1";
/// The managed `rust-implementer` profile revision, which the first workflow
/// revision pairs with.
const FIRST_WORKER: &str = "96737fddb3dd1ef86fb1916f1731aa65ca70455506f4a08c984ce04380349f46";
const SECOND_WORKER: &str = "361ecc1502a86dbbe7ad1fd45df493c1e00f3c008750ef448a118a5231294e10";
const SECOND_QUALIFICATION: &str =
    "blake3:b2dea48d667b2821a9bcf69eded39a2458a1d8165ca7fcac64c3557b69a7ea08";
const SECOND_PLAN: &str = "b3:03fd9ac7a53f8013f14693619725203eb9dab4ab4f4fc16ba4c140ea07d1f40b";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/producers")
}

fn profile() -> TestResult<InventoryProfile> {
    let path = fixtures().join("profile.json");
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// The pilot manifest with the named documents replacing a source's, and the
/// workflow source dropped for `None`.
fn snapshot(agents: &[&str], workflows: Option<&[&str]>) -> TestResult<InventorySnapshot> {
    snapshot_from(&fixtures(), agents, workflows)
}

fn snapshot_from(
    base: &Path,
    agents: &[&str],
    workflows: Option<&[&str]>,
) -> TestResult<InventorySnapshot> {
    let mut set = CollectionSet::load(&base.join(PILOT))?;
    set.collections
        .retain(|collection| collection.source != SOURCE || workflows.is_some());
    for collection in &mut set.collections {
        let documents = match (collection.source.as_str(), workflows) {
            ("lattice-agents", _) if !agents.is_empty() => agents,
            (SOURCE, Some(workflows)) => workflows,
            _ => continue,
        };
        collection.documents = documents.iter().map(|name| (*name).to_owned()).collect();
    }
    set.collect(base)
}

fn section_with(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
) -> TestResult<AgentRevisions> {
    let projection = project_inventory(profile, snapshot, None, NOW_MS).map_err(str::to_owned)?;
    Ok(project(
        &InventoryObservation::Admitted(Box::new(projection)),
        &BTreeSet::new(),
    ))
}

fn section(snapshot: &InventorySnapshot) -> TestResult<AgentRevisions> {
    section_with(&profile()?, snapshot)
}

fn only(section: &AgentRevisions) -> TestResult<&WorkflowDefinitionView> {
    match section.workflow_revisions.definitions.as_slice() {
        [definition] => Ok(definition),
        other => Err(format!("{} workflow definitions", other.len())),
    }
}

fn capture(name: &str) -> String {
    format!("lattice-workflows/{name}")
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.workflow]
fn workflow_revisions_render_from_the_captured_slots() -> TestResult {
    let pilot = section(&snapshot(
        &[],
        Some(&[&capture("workflow-4-second-revision.json")]),
    )?)?;
    assert_eq!(pilot.workflow_revisions.source, "admitted");
    assert_eq!(pilot.workflow_revisions.reason, None);
    let sources: Vec<_> = pilot
        .workflow_revisions
        .sources
        .iter()
        .map(|s| (s.source.as_str(), s.kind))
        .collect();
    assert_eq!(sources, [(SOURCE, InventoryKind::WorkflowRevisions)]);
    assert!(
        pilot.sources.iter().all(|s| s.source != SOURCE),
        "worker sources and the workflow source stay apart"
    );
    let workflow = only(&pilot)?;
    assert_eq!(workflow.reference.id, DEFINITION);
    for slot in [
        &workflow.desired,
        &workflow.last_usable,
        &workflow.selected_for_new_sessions,
    ] {
        assert_eq!(slot.state, "recorded");
        let revision = slot.revision.as_ref().ok_or("recorded revision")?;
        assert_eq!(revision.revision, SECOND);
        assert_eq!(revision.workflow.plan, SECOND_PLAN);
        assert_eq!(revision.worker.definition.source, "lattice-agents");
        assert_eq!(revision.worker.definition.id, WORKER_DEFINITION);
        assert_eq!(revision.worker.revision, SECOND_WORKER);
        assert_eq!(
            revision.worker.observed, "other_revision",
            "the pilot projects the declared rust-implementer revision"
        );
    }
    assert_eq!(workflow.rollout_generation, Some(2));
    assert_eq!(workflow.selection_kind, Some(SelectionKind::Promotion));
    assert_eq!(
        workflow.qualification.as_deref(),
        Some(SECOND_QUALIFICATION)
    );
    assert_eq!(workflow.pinned_state, "recorded");
    let pins: Vec<_> = workflow
        .pinned
        .iter()
        .map(|pin| {
            (
                pin.revision.revision.as_str(),
                pin.status,
                pin.blocked,
                pin.sessions,
                pin.selection,
            )
        })
        .collect();
    assert_eq!(
        pins,
        [
            (SECOND, PinStatus::Live, false, 1, "matches_selected"),
            (FIRST, PinStatus::Closed, true, 1, "differs_from_selected"),
        ]
    );
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.workflow]
fn paired_worker_revisions_are_compared_with_the_projected_definition() -> TestResult {
    let workflows = [capture("workflow-4-second-revision.json")];
    let workflows: Vec<&str> = workflows.iter().map(String::as_str).collect();
    let managed = section(&snapshot(
        &["lattice/managed-3-promoted.json"],
        Some(&workflows),
    )?)?;
    let workflow = only(&managed)?;
    let observed: Vec<_> = workflow
        .pinned
        .iter()
        .map(|pin| {
            (
                pin.revision.worker.revision.as_str(),
                pin.revision.worker.observed,
            )
        })
        .collect();
    assert_eq!(
        observed,
        [
            (SECOND_WORKER, "other_revision"),
            (FIRST_WORKER, "profile_revision"),
        ]
    );
    let undeclared = section(&snapshot(
        &["lattice/agent-a-1-dispatched-outcome-unknown.json"],
        Some(&workflows),
    )?)?;
    assert!(
        only(&undeclared)?
            .pinned
            .iter()
            .all(|pin| pin.revision.worker.observed == "definition_not_observed")
    );
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.workflow]
fn empty_and_unknown_workflow_slots_keep_their_words() -> TestResult {
    let desired = section(&snapshot(
        &[],
        Some(&[&capture("workflow-1-desired.json")]),
    )?)?;
    let workflow = only(&desired)?;
    assert_eq!(workflow.desired.state, "recorded");
    assert_eq!(
        workflow
            .desired
            .revision
            .as_ref()
            .map(|r| r.revision.as_str()),
        Some(FIRST)
    );
    for slot in [&workflow.last_usable, &workflow.selected_for_new_sessions] {
        assert_eq!(
            (slot.state, slot.revision.is_none()),
            ("not_recorded", true)
        );
    }
    assert_eq!(
        (
            workflow.rollout_generation,
            workflow.selection_kind,
            workflow.qualification.as_deref()
        ),
        (None, None, None)
    );
    assert_eq!(workflow.pinned_state, "not_recorded");
    assert!(workflow.pinned.is_empty());

    let mut unknown = snapshot(&[], Some(&[&capture("workflow-4-second-revision.json")]))?;
    for slots in &mut unknown.workflow_revisions {
        slots.meta.unknown_fields = vec!["desired".into(), "pinned".into()];
    }
    let unknown = section(&unknown)?;
    let workflow = only(&unknown)?;
    assert_eq!(
        (workflow.desired.state, workflow.desired.revision.is_none()),
        ("unknown", true)
    );
    assert_eq!(workflow.last_usable.state, "recorded");
    assert_eq!(workflow.pinned_state, "unknown");
    assert!(workflow.pinned.is_empty());
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.workflow_source]
fn missing_or_refused_workflow_slots_leave_the_rest_of_the_section() -> TestResult {
    let good = section(&snapshot(
        &[],
        Some(&[&capture("workflow-4-second-revision.json")]),
    )?)?;

    // No workflow source configured: not_configured, everything else unchanged.
    let mut unconfigured_profile = profile()?;
    unconfigured_profile
        .sources
        .retain(|source| source.kind != InventoryKind::WorkflowRevisions);
    let unconfigured = section_with(&unconfigured_profile, &snapshot(&[], None)?)?;
    assert_eq!(
        (
            unconfigured.workflow_revisions.source,
            unconfigured.workflow_revisions.reason.as_deref()
        ),
        ("not_configured", None)
    );
    assert!(unconfigured.workflow_revisions.definitions.is_empty());
    assert_eq!(unconfigured.definitions, good.definitions);
    assert_eq!(unconfigured.workers, good.workers);

    // Configured but not collected: unavailable as missing.
    let missing = section(&snapshot(&[], None)?)?;
    assert_eq!(
        (
            missing.workflow_revisions.source,
            missing.workflow_revisions.reason.as_deref()
        ),
        ("unavailable", Some("workflow_revisions_source_missing"))
    );
    assert_eq!(missing.definitions, good.definitions);

    // A refused document: unavailable with the reader's refusal code.
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_tree(&fixtures(), scratch.path())?;
    let path = scratch
        .path()
        .join(capture("workflow-4-second-revision.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let wrong = scratch.path().join("lattice-workflows/wrong.json");
    std::fs::write(
        &wrong,
        text.replace(
            "lattice.workflow-agent-revision-slots.v1",
            "lattice.workflow-agent-revision-slots.v0",
        ),
    )
    .map_err(|e| e.to_string())?;
    let refused = section(&snapshot_from(
        scratch.path(),
        &[],
        Some(&["lattice-workflows/wrong.json"]),
    )?)?;
    assert_eq!(refused.source, "admitted");
    assert_eq!(
        (
            refused.workflow_revisions.source,
            refused.workflow_revisions.reason.as_deref()
        ),
        (
            "unavailable",
            Some("lattice_workflow_slots_unsupported_schema")
        )
    );
    assert!(refused.workflow_revisions.definitions.is_empty());
    assert_eq!(refused.definitions, good.definitions);
    assert_eq!(refused.workers, good.workers);
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.redaction]
fn workflow_sessions_names_and_restricted_identities_never_appear() -> TestResult {
    let pilot = section(&snapshot(
        &[],
        Some(&[&capture("workflow-4-second-revision.json")]),
    )?)?;
    let text = serde_json::to_string(&pilot.workflow_revisions).map_err(|e| e.to_string())?;
    for withheld in [
        "session-a",
        "session-b",
        "\"session\"",
        "verify-change",
        "rust-implementer",
        "definition_name",
        "guidance",
        "context",
        "source_identity",
        "grant",
    ] {
        assert!(!text.contains(withheld), "{withheld} leaked");
    }
    Ok(())
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
