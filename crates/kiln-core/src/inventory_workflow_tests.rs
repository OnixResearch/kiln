// r[verify kiln.inventory.workflow]
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

use super::*;
use crate::inventory::{
    InventoryKind, InventoryProfile, InventorySnapshot, ObservationState, SourceObservation,
    SourcePolicy, admit_snapshot, project_inventory,
};

#[derive(Deserialize)]
struct Fixture {
    profile: InventoryProfile,
    snapshot: InventorySnapshot,
}
type TestResult<T = ()> = Result<T, String>;

const SOURCE: &str = "lattice-workflows";
const FIRST: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const SECOND: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn revision(revision: &str) -> WorkflowRevision {
    WorkflowRevision {
        revision: revision.into(),
        workflow: CompiledWorkflow {
            compiler_revision: "compiler.v1".into(),
            compiler_artifact: format!("b3:{revision}"),
            plan: format!("b3:{revision}"),
            graph: format!("blake3:{revision}"),
            selection: "b3:selection".into(),
            obligations: "b3:obligations".into(),
            policy: "b3:policy".into(),
        },
        worker: WorkerPairing {
            definition: FactRef {
                source: "lattice-agents".into(),
                id: "blake3:worker".into(),
            },
            revision: "worker-revision".into(),
            bundle: "blake3:bundle".into(),
        },
    }
}

fn slots() -> WorkflowRevisionSlots {
    WorkflowRevisionSlots {
        meta: FactMeta {
            reference: FactRef {
                source: SOURCE.into(),
                id: "blake3:workflow".into(),
            },
            observed_at_ms: 1000,
            valid_until_ms: 3000,
            unknown_fields: vec![],
        },
        revisions: vec![revision(FIRST), revision(SECOND)],
        desired: Some(SECOND.into()),
        last_usable: Some(SECOND.into()),
        selected_for_new_sessions: Some(WorkflowSelection {
            revision: SECOND.into(),
            rollout_generation: 2,
            kind: SelectionKind::Promotion,
            qualification: "blake3:qualification".into(),
        }),
        pinned: vec![
            WorkflowPin {
                revision: FIRST.into(),
                status: PinStatus::Live,
                blocked: false,
                sessions: 2,
            },
            WorkflowPin {
                revision: FIRST.into(),
                status: PinStatus::Closed,
                blocked: true,
                sessions: 1,
            },
        ],
    }
}

/// The synthetic v2 cohort with a workflow source and `value`.
fn cohort(value: WorkflowRevisionSlots) -> TestResult<(InventoryProfile, InventorySnapshot)> {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../fixtures/inventory/synthetic-v2.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    let (mut profile, mut snapshot) = (fixture.profile, fixture.snapshot);
    profile.sources.push(SourcePolicy {
        source: SOURCE.into(),
        producer: "lattice".into(),
        scope: "tenant".into(),
        kind: InventoryKind::WorkflowRevisions,
        minimum_generation: 1,
    });
    snapshot.sources.push(SourceObservation {
        source: SOURCE.into(),
        producer: "lattice".into(),
        scope: "tenant".into(),
        generation: 1,
        observed_at_ms: 1000,
        valid_until_ms: 3000,
        complete: true,
        next_page: None,
        unavailable_reason: None,
    });
    snapshot.workflow_revisions.push(value);
    Ok((profile, snapshot))
}

fn admitted(value: WorkflowRevisionSlots) -> TestResult<Result<(), &'static str>> {
    let (profile, snapshot) = cohort(value)?;
    Ok(admit_snapshot(&profile, &snapshot))
}

#[test]
fn workflow_slots_are_admitted_projected_and_keep_earlier_identities() -> TestResult {
    let (profile, snapshot) = cohort(slots())?;
    admit_snapshot(&profile, &snapshot)?;
    let projection = project_inventory(&profile, &snapshot, None, 2000)?;
    assert_eq!(projection.snapshot.workflow_revisions, vec![slots()]);
    let fact = projection
        .facts
        .iter()
        .find(|fact| fact.reference == slots().meta.reference)
        .ok_or("missing workflow fact status")?;
    assert_eq!(fact.state, ObservationState::Current);
    assert_eq!(
        slots().revision(FIRST).map(|r| r.revision.as_str()),
        Some(FIRST)
    );
    assert!(slots().revision("absent").is_none());

    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../fixtures/inventory/synthetic-v2.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    let encoded = serde_json::to_string(&fixture.snapshot).map_err(|e| e.to_string())?;
    assert!(
        !encoded.contains("workflow_revisions"),
        "an empty list is omitted, so earlier snapshots keep their identity"
    );
    Ok(())
}

#[test]
fn workflow_slots_in_another_kind_of_source_are_refused() -> TestResult {
    let (mut profile, snapshot) = cohort(slots())?;
    for policy in &mut profile.sources {
        if policy.source == SOURCE {
            policy.kind = InventoryKind::Agents;
        }
    }
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_source_kind_mismatch")
    );
    let mut isolated = snapshot;
    isolate_workflow_revisions(&profile, &mut isolated);
    admit_snapshot(&profile, &isolated)?;
    assert!(isolated.workflow_revisions.is_empty());
    let projection = project_inventory(&profile, &isolated, None, 2000)?;
    let source = projection
        .sources
        .iter()
        .find(|source| source.source == SOURCE)
        .ok_or("missing workflow source")?;
    assert_eq!(source.state, ObservationState::Unavailable);
    assert_eq!(
        source.reason.as_deref(),
        Some("inventory_source_kind_mismatch")
    );
    Ok(())
}

#[test]
fn unlisted_unreferenced_unordered_and_unbounded_slots_are_refused() -> TestResult {
    let mut unlisted = slots();
    unlisted.desired = Some("3333".into());
    assert_eq!(
        admitted(unlisted)?,
        Err("inventory_workflow_revision_unlisted")
    );

    let mut unreferenced = slots();
    unreferenced.pinned.clear();
    assert_eq!(
        admitted(unreferenced)?,
        Err("inventory_workflow_revision_unreferenced")
    );

    let mut unordered = slots();
    unordered.revisions.reverse();
    assert_eq!(
        admitted(unordered)?,
        Err("inventory_workflow_revision_unordered")
    );

    let mut duplicated = slots();
    duplicated.revisions.push(revision(SECOND));
    assert_eq!(
        admitted(duplicated)?,
        Err("inventory_workflow_revision_unordered")
    );

    let mut pins = slots();
    pins.pinned.reverse();
    assert_eq!(admitted(pins)?, Err("inventory_workflow_pin_unordered"));

    let mut empty = slots();
    if let Some(pin) = empty.pinned.first_mut() {
        pin.sessions = 0;
    }
    assert_eq!(admitted(empty)?, Err("inventory_workflow_pin_invalid"));

    let mut crowded = slots();
    if let Some(pin) = crowded.pinned.first_mut() {
        pin.sessions = MAX_WORKFLOW_PINNED_SESSIONS;
    }
    assert_eq!(admitted(crowded)?, Err("inventory_field_bound"));

    let mut many = slots();
    many.revisions = (0..=MAX_WORKFLOW_REVISIONS)
        .map(|index| revision(&format!("{index:064}")))
        .collect();
    assert_eq!(admitted(many)?, Err("inventory_field_bound"));

    let mut inexact = slots();
    if let Some(selection) = inexact.selected_for_new_sessions.as_mut() {
        selection.rollout_generation = crate::inventory::MAX_EXACT_NUMBER + 1;
    }
    assert_eq!(admitted(inexact)?, Err("inventory_number_not_exact"));
    Ok(())
}

#[test]
fn private_or_malformed_identities_are_refused() -> TestResult {
    let mut path = slots();
    if let Some(revision) = path.revisions.first_mut() {
        revision.workflow.plan = "/nix/store/plan".to_string();
    }
    assert_eq!(admitted(path)?, Err("inventory_public_identifier_invalid"));

    let mut worker = slots();
    if let Some(revision) = worker.revisions.first_mut() {
        revision.worker.definition.source = String::new();
    }
    assert_eq!(
        admitted(worker)?,
        Err("inventory_public_identifier_invalid")
    );

    let mut qualification = slots();
    if let Some(selection) = qualification.selected_for_new_sessions.as_mut() {
        selection.qualification = "has space".into();
    }
    assert_eq!(
        admitted(qualification)?,
        Err("inventory_public_identifier_invalid")
    );
    Ok(())
}

#[test]
fn mixed_kind_non_workflow_source_still_counts_toward_workflow_item_bound() -> TestResult {
    let (mut profile, mut snapshot) = cohort(slots())?;
    let mut stray = slots();
    stray.meta.reference.source = "agents".into();
    stray.meta.reference.id = "blake3:stray-workflow".into();
    snapshot.workflow_revisions.push(stray);

    let non_workflow_count = crate::inventory::metas(&snapshot)
        .filter(|(_, kind)| *kind != InventoryKind::WorkflowRevisions)
        .count()
        .saturating_add(snapshot.attempts.len());
    profile.max_items = non_workflow_count;

    let mut isolated = snapshot;
    isolate_workflow_revisions(&profile, &mut isolated);
    admit_snapshot(&profile, &isolated)?;
    assert!(isolated.workflow_revisions.is_empty());
    assert!(!isolated.agents.is_empty());

    let agents_source = isolated
        .sources
        .iter()
        .find(|source| source.source == "agents")
        .ok_or("missing agents source")?;
    assert_eq!(
        agents_source.unavailable_reason.as_deref(),
        Some("inventory_source_kind_mismatch")
    );
    let workflow_source = isolated
        .sources
        .iter()
        .find(|source| source.source == SOURCE)
        .ok_or("missing workflow source")?;
    assert_eq!(
        workflow_source.unavailable_reason.as_deref(),
        Some("inventory_item_bound")
    );
    Ok(())
}
