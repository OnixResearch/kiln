//! The Cairn workspace's agents section over the Lattice v3 producer fixtures
//! (`fixtures/inventory/producers/`), mapped by Kiln's own reader and admitted
//! by the inventory projection, as the detail route reads them. Deterministic
//! fixture evidence only.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::inventory::{
    EvidenceProvenance, FactRef, InventoryProfile, InventorySnapshot, ObservationState, PinStatus,
    Qualification, TaskFamilyQualification, project_inventory,
};

use super::{AgentRevisions, InventoryObservation, Slot, project};
use crate::inventory_collection::CollectionSet;

type TestResult<T = ()> = Result<T, String>;

/// Five seconds after the fixtures' collection time.
const NOW_MS: u64 = 1_795_237_895_000;
/// The declared `rust-implementer` definition, unmanaged and managed alike.
const DECLARED_DEFINITION: &str =
    "blake3:367783379a41fd2e834f26af4e7504906c1f3967bd9f21bba979441a1f0789c1";
/// The definition of every undeclared OMP attempt-worker capture.
const UNDECLARED_DEFINITION: &str =
    "blake3:c2edbc9f0aa03386b1e8d4eb900e2b10db1083ed568ed934c53e923a2e23a8b6";
const MANAGED_PROFILE: &str = "96737fddb3dd1ef86fb1916f1731aa65ca70455506f4a08c984ce04380349f46";
/// The Animus #18 attachment the managed promotion recorded, with `fixture` provenance.
const QUALIFICATION_REF: &str =
    "blake3:f87cdbc0556abfd2bcf7bdd9d26e89b722a57bf5b6b1a81d0ea73fc5c1c41fcc";
const PILOT: &str = "pilot.collection.json";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/producers")
}

fn profile() -> TestResult<InventoryProfile> {
    let path = fixtures().join("profile.json");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The pilot manifest, with its Lattice sources replaced when documents are named.
fn snapshot(agents: &[&str], workers: &[&str]) -> TestResult<InventorySnapshot> {
    let mut set = CollectionSet::load(&fixtures().join(PILOT))?;
    for collection in &mut set.collections {
        let documents = match collection.source.as_str() {
            "lattice-agents" if !agents.is_empty() => agents,
            "lattice-workers" if !workers.is_empty() => workers,
            _ => continue,
        };
        collection.documents = documents
            .iter()
            .map(|name| format!("lattice/{name}"))
            .collect();
    }
    set.collect(&fixtures())
}

fn admitted_with(
    profile: &InventoryProfile,
    snapshot: &InventorySnapshot,
) -> TestResult<InventoryObservation> {
    project_inventory(profile, snapshot, None, NOW_MS)
        .map(|projection| InventoryObservation::Admitted(Box::new(projection)))
        .map_err(str::to_owned)
}

fn section(snapshot: &InventorySnapshot) -> TestResult<AgentRevisions> {
    Ok(project(
        &admitted_with(&profile()?, snapshot)?,
        &BTreeSet::new(),
    ))
}

fn definition<'a>(section: &'a AgentRevisions, id: &str) -> TestResult<&'a super::DefinitionView> {
    section
        .definitions
        .iter()
        .find(|d| d.reference.id == id)
        .ok_or_else(|| format!("missing definition {id}"))
}

fn slots(definition: &super::DefinitionView) -> [(&'static str, Option<&str>); 4] {
    [
        &definition.desired,
        &definition.prepared,
        &definition.qualified,
        &definition.selected_for_new_sessions,
    ]
    .map(|slot| (slot.state, slot.owner.as_deref()))
}

fn has_wait(waits: &[super::WaitView], code: &str, subject: &str, owner: &str) -> bool {
    waits
        .iter()
        .any(|w| w.code == code && w.subject == subject && w.owner == owner)
}

#[test]
// r[verify kiln.cairn.workspace.agents]
// r[verify kiln.cairn.workspace.agents.scenario.slots]
fn unmanaged_unknown_recorded_and_unpublished_slots_stay_distinct() -> TestResult {
    // Unmanaged: no lifecycle governs the definition; neither unknown nor empty.
    let unmanaged = section(&snapshot(&[], &[])?)?;
    let declared = definition(&unmanaged, DECLARED_DEFINITION)?;
    assert_eq!(declared.management, "unmanaged");
    assert_eq!(slots(declared), [("unmanaged", None); 4]);
    assert!(
        declared
            .wait_reasons
            .iter()
            .all(|w| w.code != "revision_fact_unknown")
    );
    assert!(
        unmanaged
            .workers
            .iter()
            .all(|w| w.pin.state == "unmanaged" && w.pin.owner.is_none())
    );

    // Managed with unknown slots: each keeps the owner Lattice published.
    let desired = section(&snapshot(&["managed-1-desired.json"], &[])?)?;
    let managed = definition(&desired, DECLARED_DEFINITION)?;
    assert_eq!(managed.management, "managed");
    assert_eq!(
        managed.desired,
        Slot::new("recorded", Some(MANAGED_PROFILE.into()), None)
    );
    assert_eq!(
        slots(managed)[1..],
        [
            ("unknown", Some("kiln#49")),
            ("unknown", Some("animus#18")),
            ("unknown", Some("lattice#25")),
        ]
    );
    assert_eq!(managed.rollout_generation, None);
    for (field, owner) in [
        ("fact:revisions.prepared", "kiln#49"),
        ("fact:revisions.qualified", "animus#18"),
        ("fact:revisions.selected_for_new_sessions", "lattice#25"),
    ] {
        assert!(
            has_wait(&managed.wait_reasons, "revision_fact_unknown", field, owner),
            "{field}"
        );
    }

    // Managed and recorded: promoted for new sessions.
    let promoted = section(&snapshot(&["managed-3-promoted.json"], &[])?)?;
    let selected = definition(&promoted, DECLARED_DEFINITION)?;
    assert_eq!(
        slots(selected),
        [("recorded", None); 4],
        "no recorded slot names an owner"
    );
    assert_eq!(
        selected.selected_for_new_sessions.value.as_deref(),
        Some(MANAGED_PROFILE)
    );
    assert!(selected.rollout_generation.is_some());

    // Managed after revocation: no current qualification is not unknown.
    let revoked = section(&snapshot(
        &["managed-7-refused-after-revocation.json"],
        &[],
    )?)?;
    let refused = definition(&revoked, DECLARED_DEFINITION)?;
    assert_eq!(refused.qualified, Slot::new("not_recorded", None, None));
    assert_eq!(refused.selected_for_new_sessions.state, "recorded");

    // A producer that does not say whether a lifecycle exists.
    let mut silent = snapshot(&["managed-1-desired.json"], &[])?;
    for agent in &mut silent.agents {
        agent.revisions.management = None;
        agent.revisions.desired = None;
    }
    let silent = section(&silent)?;
    let silent = definition(&silent, DECLARED_DEFINITION)?;
    assert_eq!(silent.management, "unpublished");
    assert_eq!(silent.desired, Slot::new("unpublished", None, None));
    assert_eq!(
        silent.prepared.state, "unknown",
        "an unknown fact stays unknown"
    );

    let states: BTreeSet<&str> = [
        declared.desired.state,
        managed.prepared.state,
        selected.desired.state,
        refused.qualified.state,
        silent.desired.state,
    ]
    .into();
    assert_eq!(states.len(), 5, "five distinct states: {states:?}");
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.selection]
fn selected_and_pinned_revisions_stay_separate_with_blocked_pins() -> TestResult {
    let live = snapshot(
        &["managed-3-promoted.json"],
        &[
            "managed-4-admitted-session-live.json",
            "managed-5-terminal-pin-released.json",
        ],
    )?;
    let promoted = section(&live)?;
    assert_eq!(promoted.workers.len(), 2);
    for worker in &promoted.workers {
        assert_eq!(worker.pinned_revision, MANAGED_PROFILE);
        assert_eq!(worker.selection, "matches_selected");
        assert_eq!(worker.pin.state, "recorded");
        assert_eq!(worker.pin.qualification.as_deref(), Some(QUALIFICATION_REF));
        assert_eq!(worker.pin.blocked, Some(false));
    }
    let statuses: BTreeSet<_> = promoted
        .workers
        .iter()
        .filter_map(|w| w.pin.status.map(|s| format!("{s:?}")))
        .collect();
    assert_eq!(
        statuses,
        [PinStatus::Live, PinStatus::Released]
            .map(|s| format!("{s:?}"))
            .into()
    );
    assert_eq!(definition(&promoted, DECLARED_DEFINITION)?.workers, 2);

    // A newer selection leaves the pinned workers on their revision.
    let mut moved = live.clone();
    for agent in &mut moved.agents {
        agent.revisions.selected_for_new_sessions = Some("f".repeat(64));
    }
    let moved = section(&moved)?;
    assert!(
        moved
            .workers
            .iter()
            .all(|w| w.selection == "differs_from_selected" && w.pinned_revision == MANAGED_PROFILE)
    );

    // Revoked: new sessions are refused by Lattice and every pin is blocked.
    let revoked = section(&snapshot(
        &["managed-7-refused-after-revocation.json"],
        &[
            "managed-6-revoked-live-session.json",
            "managed-6-revoked-terminal.json",
        ],
    )?)?;
    assert!(!revoked.workers.is_empty());
    assert!(
        revoked
            .workers
            .iter()
            .all(|w| w.pin.state == "recorded" && w.pin.blocked == Some(true))
    );
    let refused = definition(&revoked, DECLARED_DEFINITION)?;
    assert!(has_wait(
        &refused.wait_reasons,
        "revision_not_admitted",
        "fact:revisions",
        "lattice#25"
    ));
    assert!(has_wait(
        &refused.wait_reasons,
        "qualification_unbound",
        "fact:capability_qualifications",
        "animus#18"
    ));

    // No recorded selection, and a worker whose definition is not observed.
    let unselected = section(&snapshot(
        &["managed-1-desired.json"],
        &["managed-4-admitted-session-live.json"],
    )?)?;
    let [pinned] = unselected.workers.as_slice() else {
        return Err("one pinned worker expected".into());
    };
    assert_eq!(pinned.selection, "no_selection");
    let orphaned = section(&snapshot(&["managed-1-desired.json"], &[])?)?;
    assert!(
        orphaned
            .workers
            .iter()
            .filter(|w| w.definition.id == UNDECLARED_DEFINITION)
            .all(|w| w.selection == "definition_not_observed")
    );
    assert!(
        orphaned
            .workers
            .iter()
            .any(|w| w.definition.id == UNDECLARED_DEFINITION)
    );

    // Workflow revisions are their own slot that no producer publishes.
    for revisions in [&promoted, &revoked, &unselected] {
        let owner = Some("lattice#25".to_owned());
        assert_eq!(
            revisions.workflow_revisions.selected_for_new_sessions,
            Slot::new("unpublished", None, owner.clone())
        );
        assert_eq!(
            revisions.workflow_revisions.pinned,
            Slot::new("unpublished", None, owner)
        );
    }
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.provenance]
fn fixture_provenance_never_counts_as_live() -> TestResult {
    let promoted = snapshot(&["managed-3-promoted.json"], &[])?;
    let fixture = section(&promoted)?;
    let families = &definition(&fixture, DECLARED_DEFINITION)?.task_families;
    let family = families
        .iter()
        .find(|f| f.task_family == "rust-implementation")
        .ok_or("missing family")?;
    assert!(family.declared);
    assert_eq!(family.state, "fixture");
    let reference = family.qualification.as_ref().ok_or("missing reference")?;
    assert_eq!(reference.reference, QUALIFICATION_REF);
    assert_eq!(reference.evidence_class, "task_family_evaluation");
    assert_eq!(reference.provenance, EvidenceProvenance::Fixture);
    assert!(!reference.counts_as_live);
    assert!(
        fixture
            .definitions
            .iter()
            .flat_map(|d| &d.task_families)
            .all(|f| f.state != "live")
    );

    // Only live provenance counts as live; an undeclared qualified family stays listed.
    let mut live = promoted.clone();
    for agent in &mut live.agents {
        for qualification in &mut agent.task_family_qualifications {
            qualification.provenance = EvidenceProvenance::Live;
        }
        agent.task_family_qualifications.insert(
            0,
            TaskFamilyQualification {
                task_family: "a-undeclared-family".into(),
                reference: QUALIFICATION_REF.into(),
                evidence_class: "task_family_evaluation".into(),
                provenance: EvidenceProvenance::Fixture,
            },
        );
    }
    let live = section(&live)?;
    let families = &definition(&live, DECLARED_DEFINITION)?.task_families;
    let declared = families
        .iter()
        .find(|f| f.task_family == "rust-implementation")
        .ok_or("missing family")?;
    assert_eq!(declared.state, "live");
    assert!(
        declared
            .qualification
            .as_ref()
            .is_some_and(|q| q.counts_as_live)
    );
    let undeclared = families
        .iter()
        .find(|f| f.task_family == "a-undeclared-family")
        .ok_or("missing undeclared family")?;
    assert!(!undeclared.declared);
    assert_eq!(undeclared.state, "fixture");

    // An unqualified declared family reads unqualified, never qualified.
    let unmanaged = section(&snapshot(&[], &[])?)?;
    assert!(
        definition(&unmanaged, DECLARED_DEFINITION)?
            .task_families
            .iter()
            .all(|f| f.state == "unqualified" && f.qualification.is_none())
    );
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.redaction]
fn restricted_context_and_other_work_never_appear() -> TestResult {
    let mut profile = profile()?;
    // Disclose everything the inventory may hold: the allow-list alone must withhold it.
    profile.include_evidence = true;
    let mut snapshot = snapshot(
        &["managed-3-promoted.json"],
        &[
            "managed-4-admitted-session-live.json",
            "managed-5-terminal-pin-released.json",
        ],
    )?;
    let restricted = [
        "restricted-session",
        "restricted-workspace",
        "restricted-grant",
        "restricted-model-binding",
        "restricted-subject",
        "restricted-corpus",
        "restricted-preparation-failure",
    ];
    for worker in &mut snapshot.workers {
        worker.session = Some(restricted[0].into());
        worker.workspace = Some(restricted[1].into());
        worker.execution_grant = Some(FactRef {
            source: "aspen-compute".into(),
            id: restricted[2].into(),
        });
        worker.model_services.push(FactRef {
            source: "animus-serving".into(),
            id: restricted[3].into(),
        });
    }
    for agent in &mut snapshot.agents {
        agent.revisions.preparation_failure = Some(restricted[6].into());
        agent.qualifications.push(Qualification {
            profile_revision: agent.profile_revision.clone(),
            task_family: "rust-implementation".into(),
            subject: restricted[4].into(),
            corpus: restricted[5].into(),
            protocol: "protocol".into(),
            evidence: QUALIFICATION_REF.into(),
            verified_contracts: Vec::new(),
            accepted_outcomes: None,
            attempted_outcomes: None,
            measured_cost_microunits: None,
            unknown_use: false,
            limitations: Vec::new(),
            valid_until_ms: NOW_MS,
            authority: kiln_core::inventory::Permission::Unknown,
        });
    }
    let observation = admitted_with(&profile, &snapshot)?;
    let InventoryObservation::Admitted(projection) = &observation else {
        return Err("inventory was not admitted".into());
    };
    let disclosed = serde_json::to_string(projection).map_err(|e| e.to_string())?;
    assert!(
        restricted.iter().all(|value| disclosed.contains(value)),
        "the inventory itself discloses the injected values"
    );
    let executions: Vec<String> = snapshot
        .workers
        .iter()
        .map(|w| w.execution.id.clone())
        .collect();
    let bound: Vec<(String, u64)> = snapshot
        .workers
        .iter()
        .filter_map(|w| Some((w.attempt.clone()?, w.fence?)))
        .collect();
    assert_eq!(bound.len(), 2, "both captures bind an attempt");

    // Other work: no attempt, fence or restricted value reaches the section.
    let other = project(&observation, &BTreeSet::new());
    let text = serde_json::to_string(&other).map_err(|e| e.to_string())?;
    for value in restricted
        .iter()
        .copied()
        .chain(executions.iter().map(String::as_str))
        .chain(bound.iter().map(|(attempt, _)| attempt.as_str()))
    {
        assert!(!text.contains(value), "{value} leaked");
    }
    assert!(
        other
            .workers
            .iter()
            .all(|w| w.work == "other_work" && w.attempt.is_none() && w.fence.is_none())
    );

    // This change's attempt: only that worker shows it, and it comes first.
    let [(other_attempt, _), (attempt, fence)] = bound.as_slice() else {
        return Err("two bound workers expected".into());
    };
    let mine = project(&observation, &BTreeSet::from([(attempt.as_str(), *fence)]));
    let [first, rest @ ..] = mine.workers.as_slice() else {
        return Err("workers expected".into());
    };
    assert_eq!(first.work, "this_change");
    assert_eq!(first.attempt.as_deref(), Some(attempt.as_str()));
    assert_eq!(first.fence, Some(*fence));
    assert!(
        rest.iter()
            .all(|w| w.work != "this_change" && w.attempt.is_none())
    );
    let text = serde_json::to_string(&mine).map_err(|e| e.to_string())?;
    assert!(!text.contains(other_attempt.as_str()));
    // A matching attempt at another fence is not this change's.
    let crossed = project(
        &observation,
        &BTreeSet::from([(attempt.as_str(), fence + 1)]),
    );
    assert!(crossed.workers.iter().all(|w| w.work == "other_work"));
    Ok(())
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.source]
fn unconfigured_refused_and_stale_inventory_keep_their_meaning() -> TestResult {
    let unconfigured = project(&InventoryObservation::NotConfigured, &BTreeSet::new());
    assert_eq!(
        (unconfigured.source, unconfigured.reason.as_deref()),
        ("not_configured", None)
    );
    let refused = project(
        &InventoryObservation::Unavailable("inventory_principal_denied".into()),
        &BTreeSet::new(),
    );
    assert_eq!(
        (refused.source, refused.reason.as_deref()),
        ("unavailable", Some("inventory_principal_denied"))
    );
    for section in [&unconfigured, &refused] {
        assert!(section.definitions.is_empty() && section.workers.is_empty());
        assert!(section.sources.is_empty() && section.inventory_identity.is_none());
        assert_eq!(
            section.workflow_revisions.pinned.owner.as_deref(),
            Some("lattice#25")
        );
    }

    // The admitted section names its inventory and only agents and workers sources.
    let snapshot = snapshot(&["managed-3-promoted.json"], &[])?;
    let admitted = section(&snapshot)?;
    assert_eq!(admitted.source, "admitted");
    assert!(admitted.inventory_identity.is_some());
    assert_eq!(admitted.evaluated_at_ms, Some(NOW_MS));
    assert_eq!(admitted.synthetic, Some(true));
    let sources: BTreeSet<&str> = admitted.sources.iter().map(|s| s.source.as_str()).collect();
    assert_eq!(
        sources,
        BTreeSet::from(["lattice-agents", "lattice-workers"])
    );
    // Lattice leaves full qualification records unpublished, so a definition
    // with unknown fields is never a current fact.
    assert_eq!(
        definition(&admitted, DECLARED_DEFINITION)?.observation,
        ObservationState::Unknown
    );

    // A later clock leaves recorded facts recorded but not current.
    let stale = project_inventory(&profile()?, &snapshot, None, NOW_MS + 3_600_000)
        .map_err(str::to_owned)?;
    let stale = project(
        &InventoryObservation::Admitted(Box::new(stale)),
        &BTreeSet::new(),
    );
    let definition = definition(&stale, DECLARED_DEFINITION)?;
    assert_eq!(definition.observation, ObservationState::Stale);
    assert_eq!(definition.selected_for_new_sessions.state, "recorded");
    assert_eq!(definition.management, "managed");
    Ok(())
}
