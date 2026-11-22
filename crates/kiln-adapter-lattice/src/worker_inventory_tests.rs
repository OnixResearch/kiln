//! Lattice `lattice.worker-inventory.v3` documents captured from `lattice
//! implementation-attempt inspect` on the final Lattice #25 integration tree
//! (deterministic fixture ports only, an Animus #18 attachment of `fixture`
//! provenance), copied verbatim; `provenance.json` records their hashes.
// r[verify kiln.inventory.producers.scenario.lattice]
// r[verify kiln.inventory.producers.scenario.capability]
// r[verify kiln.inventory.producers.scenario.revisions]
// r[verify kiln.inventory.producers.scenario.wait]
// r[verify kiln.inventory.producers.scenario.refused]
use kiln_core::inventory::{
    AgentDefinition, EvidenceProvenance, FactRef, InventoryFragment, InventoryJoins, InventoryKind,
    Permission, PinStatus, RevisionManagement, RevisionPin, SourceObservation,
    TaskFamilyQualification, WaitReason, WaitSubject, WorkerInstance, WorkerLifecycle,
};
use serde_json::{Value, json};

use super::map_worker_inventory;

type TestResult<T = ()> = Result<T, String>;

macro_rules! capture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../fixtures/inventory/producers/lattice/",
            $name
        ))
    };
}

const ADMITTED: &[u8] = capture!("agent-a-1-admitted-not-dispatched.json");
const DECLARED: &[u8] = capture!("agent-a-1-declared-capability-dispatched.json");
const FIRST: &[u8] = capture!("agent-a-1-dispatched-outcome-unknown.json");
const TERMINAL: &[u8] = capture!("agent-a-1-terminal-failed.json");
const SECOND: &[u8] = capture!("agent-a-2-dispatched-outcome-unknown.json");
const SERVED: &[u8] = capture!("host-local-served-ready.json");
const SERVED_UNAVAILABLE: &[u8] = capture!("host-local-served-unavailable.json");
const PROFILE_ONLY: &[u8] = capture!("profile-without-instance.json");
const MANAGED_DESIRED: &[u8] = capture!("managed-1-desired.json");
const MANAGED_PREPARED: &[u8] = capture!("managed-2-prepared.json");
const MANAGED_PROMOTED: &[u8] = capture!("managed-3-promoted.json");
const MANAGED_LIVE: &[u8] = capture!("managed-4-admitted-session-live.json");
const MANAGED_RELEASED: &[u8] = capture!("managed-5-terminal-pin-released.json");
const MANAGED_REVOKED_LIVE: &[u8] = capture!("managed-6-revoked-live-session.json");
const MANAGED_REVOKED_TERMINAL: &[u8] = capture!("managed-6-revoked-terminal.json");
const MANAGED_REFUSED: &[u8] = capture!("managed-7-refused-after-revocation.json");
const PROVENANCE: &str =
    include_str!("../../../fixtures/inventory/producers/lattice/provenance.json");
/// The captured instances were observed before this collection time.
const COLLECTED_AT_MS: u64 = 1_795_237_890_000;
const VALID_UNTIL_MS: u64 = 1_795_237_950_000;
const DEFINITION: &str = "blake3:c2edbc9f0aa03386b1e8d4eb900e2b10db1083ed568ed934c53e923a2e23a8b6";
/// The declared `rust-implementer` definition, unmanaged and managed alike.
const DECLARED_DEFINITION: &str =
    "blake3:367783379a41fd2e834f26af4e7504906c1f3967bd9f21bba979441a1f0789c1";
const PROFILE: &str = "7a2e02cbb387e5e5744d4a63084238c2b6cb58d8572ad048fadcd38887ec1258";
const HOST_LOCAL_PROFILE: &str = "d35dafdc3606dbe61288366cf10f9d2a96ca8a186af5ee40386fae45ed3fbe88";
const DECLARED_PROFILE: &str = "facae6f62ce3c6ffae5db21ae9c23281bcd5e1c884ac8145ed1ec6e9e13da8bd";
const MANAGED_PROFILE: &str = "96737fddb3dd1ef86fb1916f1731aa65ca70455506f4a08c984ce04380349f46";
/// The Animus #18 attachment identity the managed promotion recorded.
const QUALIFICATION_REF: &str =
    "blake3:f87cdbc0556abfd2bcf7bdd9d26e89b722a57bf5b6b1a81d0ea73fc5c1c41fcc";
const LIVE_PIN: &str = "blake3:736315c3d03cc5cc9aa67ac4567d008a537b455177749f2c0dbf21d39fcace56";
const RELEASED_PIN: &str =
    "blake3:32be75ce1baa21850b62b00d3cfa4d9f082403a3e7cbba8b8c4cf55be8e7c0b6";
/// A later revision of the managed definition, promoted in derived fixtures.
const NEXT_REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NEXT_REF: &str = "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FAMILY: &str = "rust-implementation";

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

fn reason(code: &str, field: &str, owner: &str) -> WaitReason {
    WaitReason {
        code: code.into(),
        subject: WaitSubject::Fact {
            field: field.into(),
        },
        owner: owner.into(),
    }
}

fn reason_json(code: &str, field: &str, owner: &str) -> Value {
    json!({ "code": code, "subject": { "kind": "fact", "field": field }, "owner": owner })
}

fn edits(bytes: &[u8], changes: &[(&str, Value)]) -> TestResult<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    for (pointer, replacement) in changes {
        let slot = value
            .pointer_mut(pointer)
            .ok_or_else(|| format!("missing {pointer}"))?;
        *slot = replacement.clone();
    }
    serde_json::to_vec(&value).map_err(|e| e.to_string())
}

fn edit(bytes: &[u8], pointer: &str, replacement: Value) -> TestResult<Vec<u8>> {
    edits(bytes, &[(pointer, replacement)])
}

fn field(bytes: &[u8], pointer: &str) -> TestResult<Value> {
    serde_json::from_slice::<Value>(bytes)
        .map_err(|e| e.to_string())?
        .pointer(pointer)
        .cloned()
        .ok_or_else(|| format!("missing {pointer}"))
}

fn agents(documents: &[&[u8]]) -> Result<InventoryFragment, &'static str> {
    map_worker_inventory(
        InventoryKind::Agents,
        &source("lattice-agents"),
        &joins(),
        documents,
    )
}

fn workers(documents: &[&[u8]]) -> Result<InventoryFragment, &'static str> {
    map_worker_inventory(
        InventoryKind::Workers,
        &source("lattice-workers"),
        &joins(),
        documents,
    )
}

fn definition<'a>(fragment: &'a InventoryFragment, id: &str) -> TestResult<&'a AgentDefinition> {
    fragment
        .agents
        .iter()
        .find(|a| a.meta.reference.id == id)
        .ok_or_else(|| format!("missing definition {id}"))
}

fn only_definition(documents: &[&[u8]]) -> TestResult<AgentDefinition> {
    let fragment = agents(documents).map_err(str::to_owned)?;
    match fragment.agents.as_slice() {
        [agent] => Ok(agent.clone()),
        other => Err(format!("expected one definition, got {}", other.len())),
    }
}

fn only_worker(fragment: &InventoryFragment) -> TestResult<&WorkerInstance> {
    match fragment.workers.as_slice() {
        [worker] => Ok(worker),
        other => Err(format!("expected one worker, got {}", other.len())),
    }
}

fn unknown(fields: &[String], name: &str) -> bool {
    fields.iter().any(|f| f == name)
}

fn fixture_qualification(reference: &str) -> TaskFamilyQualification {
    TaskFamilyQualification {
        task_family: FAMILY.into(),
        reference: reference.into(),
        evidence_class: "task_family_evaluation".into(),
        provenance: EvidenceProvenance::Fixture,
    }
}

fn qualified_entry(revision: &str, reference: &str, revoked: bool) -> Value {
    json!({
        "evidence_class": "task_family_evaluation",
        "is_revoked": revoked,
        "provenance": "fixture",
        "qualification_ref": reference,
        "revision": revision,
        "task_family": FAMILY,
    })
}

/// The managed definition after a later revision was desired, prepared,
/// qualified and promoted: the four slots every derived document shares.
fn next_lifecycle() -> [(&'static str, Value); 4] {
    [
        (
            "/profile/revisions/desired/value",
            json!({ "generation": 2, "revision": NEXT_REVISION }),
        ),
        (
            "/profile/revisions/prepared/value",
            json!([MANAGED_PROFILE, NEXT_REVISION]),
        ),
        (
            "/profile/revisions/qualified/value",
            json!([
                qualified_entry(MANAGED_PROFILE, QUALIFICATION_REF, false),
                qualified_entry(NEXT_REVISION, NEXT_REF, false),
            ]),
        ),
        (
            "/profile/revisions/selected_for_new_sessions/value",
            json!({ "kind": "promotion", "qualification_ref": NEXT_REF, "revision": NEXT_REVISION, "rollout_generation": 2 }),
        ),
    ]
}

/// The live session admitted under the managed revision, inspected after the
/// later revision was promoted: its profile is no longer selected, and its
/// pin keeps the older revision.
fn pinned_on_older_revision() -> TestResult<Vec<u8>> {
    let mut changes = next_lifecycle().to_vec();
    changes.push((
        "/profile/wait_reasons",
        json!([
            reason_json("revision_not_admitted", "revisions", "lattice#25"),
            reason_json("serving_port_absent", "serving_readiness", "kiln#49"),
        ]),
    ));
    edits(MANAGED_LIVE, &changes)
}

/// The later revision's own profile, selected for new sessions.
fn selected_next_profile() -> TestResult<Vec<u8>> {
    let mut changes = next_lifecycle().to_vec();
    changes.push(("/profile/revision", json!(NEXT_REVISION)));
    changes.push((
        "/profile/capability_qualifications",
        json!([{ "evidence_class": "task_family_evaluation", "provenance": "fixture", "qualification_ref": NEXT_REF, "task_family": FAMILY }]),
    ));
    edits(MANAGED_PROMOTED, &changes)
}

/// The promoted revision after its qualification was revoked: no current
/// qualification, the family back to declared, and new sessions refused.
fn revoked_qualification(base: &[u8]) -> TestResult<Vec<u8>> {
    edits(
        base,
        &[
            (
                "/profile/revisions/qualified/value/0/is_revoked",
                json!(true),
            ),
            ("/profile/capability_qualifications", json!([])),
            ("/profile/task_families/support", json!("declared")),
            (
                "/profile/wait_reasons",
                field(MANAGED_REFUSED, "/profile/wait_reasons")?,
            ),
        ],
    )
}

#[test]
fn captures_match_their_provenance() -> TestResult {
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|e| e.to_string())?;
    assert_eq!(
        provenance.get("contract").and_then(Value::as_str),
        Some(super::WORKER_INVENTORY_SCHEMA)
    );
    assert_eq!(provenance.get("copied"), Some(&json!("verbatim")));
    assert_eq!(provenance.get("synthetic"), Some(&json!(true)));
    assert!(
        provenance
            .get("lattice_commit")
            .and_then(Value::as_str)
            .is_some_and(|commit| !commit.is_empty()),
        "the record names the producer commit; the hashes bind the bytes"
    );
    let hashes = provenance
        .get("blake3")
        .and_then(Value::as_object)
        .ok_or("missing hashes")?;
    let captures: [(&str, &[u8]); 16] = [
        ("agent-a-1-admitted-not-dispatched.json", ADMITTED),
        ("agent-a-1-declared-capability-dispatched.json", DECLARED),
        ("agent-a-1-dispatched-outcome-unknown.json", FIRST),
        ("agent-a-1-terminal-failed.json", TERMINAL),
        ("agent-a-2-dispatched-outcome-unknown.json", SECOND),
        ("host-local-served-ready.json", SERVED),
        ("host-local-served-unavailable.json", SERVED_UNAVAILABLE),
        ("managed-1-desired.json", MANAGED_DESIRED),
        ("managed-2-prepared.json", MANAGED_PREPARED),
        ("managed-3-promoted.json", MANAGED_PROMOTED),
        ("managed-4-admitted-session-live.json", MANAGED_LIVE),
        ("managed-5-terminal-pin-released.json", MANAGED_RELEASED),
        ("managed-6-revoked-live-session.json", MANAGED_REVOKED_LIVE),
        ("managed-6-revoked-terminal.json", MANAGED_REVOKED_TERMINAL),
        ("managed-7-refused-after-revocation.json", MANAGED_REFUSED),
        ("profile-without-instance.json", PROFILE_ONLY),
    ];
    assert_eq!(hashes.len(), captures.len());
    for (name, bytes) in captures {
        assert_eq!(
            hashes.get(name).and_then(Value::as_str),
            Some(blake3::hash(bytes).to_hex().as_str()),
            "{name} is not the recorded capture"
        );
    }
    let edited = edit(FIRST, "/instance/fence", json!(8))?;
    assert_ne!(
        blake3::hash(&edited).to_hex().as_str(),
        hashes
            .get("agent-a-1-dispatched-outcome-unknown.json")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        "an edited capture no longer matches its provenance"
    );
    Ok(())
}

#[test]
fn definitions_are_keyed_by_identity_apart_from_revision() -> TestResult {
    let fragment = agents(&[FIRST, SECOND, PROFILE_ONLY, DECLARED])?;
    assert!(
        fragment.workers.is_empty(),
        "an agents source never yields workers"
    );
    let ids: Vec<_> = fragment
        .agents
        .iter()
        .map(|a| a.meta.reference.id.as_str())
        .collect();
    assert_eq!(ids, vec![DECLARED_DEFINITION, DEFINITION]);
    let undeclared = definition(&fragment, DEFINITION)?;
    assert_eq!(
        undeclared.meta.reference,
        reference("lattice-agents", DEFINITION)
    );
    assert_eq!(undeclared.profile_revision, PROFILE);
    assert_eq!(undeclared.harnesses, vec!["omp-jsonl".to_owned()]);
    assert_eq!(
        undeclared.output_contracts,
        vec!["candidate-receipt".to_owned()]
    );
    assert_eq!(undeclared.authority, Permission::Unknown);
    assert!(undeclared.task_families.is_empty() && undeclared.tools.is_empty());
    assert!(undeclared.qualifications.is_empty());
    assert!(undeclared.task_family_qualifications.is_empty());
    assert_eq!(
        undeclared.meta.unknown_fields,
        vec![
            "permitted_models",
            "qualifications",
            "task_families",
            "tools"
        ]
    );
    assert_eq!(
        undeclared.wait_reasons,
        vec![
            reason("capability_undeclared", "task_families", "configuration"),
            reason("capability_undeclared", "tools", "configuration"),
            reason(
                "qualification_unbound",
                "capability_qualifications",
                "animus#18"
            ),
        ],
        "serving reasons of one inspection never describe the definition"
    );
    let declared = definition(&fragment, DECLARED_DEFINITION)?;
    assert_eq!(declared.profile_revision, DECLARED_PROFILE);
    assert_eq!(
        declared.task_families,
        vec!["cairn-task".to_owned(), FAMILY.to_owned()]
    );
    assert_eq!(declared.tools, vec!["edit".to_owned(), "read".to_owned()]);
    assert_eq!(
        declared.meta.unknown_fields,
        vec!["permitted_models", "qualifications"]
    );
    assert_eq!(
        declared.wait_reasons,
        vec![reason(
            "qualification_unbound",
            "capability_qualifications",
            "animus#18"
        )]
    );
    // Profiles of one revision whose serving readings differ are one definition.
    let served = agents(&[SERVED, SERVED_UNAVAILABLE])?;
    assert_eq!(
        definition(&served, DEFINITION)?.profile_revision,
        HOST_LOCAL_PROFILE
    );
    assert_eq!(served.agents.len(), 1);
    Ok(())
}

#[test]
fn unmanaged_unknown_and_recorded_revisions_stay_distinct() -> TestResult {
    // Unmanaged: no lifecycle governs the definition; nothing is missing.
    let unmanaged = only_definition(&[TERMINAL])?;
    let revisions = &unmanaged.revisions;
    assert_eq!(revisions.management, Some(RevisionManagement::Unmanaged));
    for slot in [
        &revisions.desired,
        &revisions.prepared,
        &revisions.qualified,
        &revisions.selected_for_new_sessions,
    ] {
        assert_eq!(slot, &None, "an unmanaged slot is never a revision");
    }
    assert_eq!(revisions.generation, 0);
    assert!(revisions.evidence.is_empty());
    assert!(
        !unmanaged
            .meta
            .unknown_fields
            .iter()
            .any(|f| f.starts_with("revisions")),
        "unmanaged is not unknown"
    );
    assert!(
        !unmanaged
            .wait_reasons
            .iter()
            .any(|r| r.owner == "lattice#25"),
        "an unmanaged profile waits on no revision owner"
    );
    let unmanaged_pin = workers(&[TERMINAL])?;
    let unmanaged_pin = only_worker(&unmanaged_pin)?;
    assert_eq!(unmanaged_pin.pin, Some(RevisionPin::Unmanaged {}));
    assert!(!unknown(&unmanaged_pin.meta.unknown_fields, "pinned"));

    // Unknown: managed, but the lifecycle holds no fact yet; each keeps its owner.
    let desired = only_definition(&[MANAGED_DESIRED])?;
    let revisions = &desired.revisions;
    assert_eq!(revisions.management, Some(RevisionManagement::Managed));
    assert_eq!(revisions.desired.as_deref(), Some(MANAGED_PROFILE));
    assert_eq!(revisions.prepared, None);
    assert_eq!(revisions.qualified, None);
    assert_eq!(revisions.selected_for_new_sessions, None);
    assert_eq!(
        desired.meta.unknown_fields,
        vec![
            "permitted_models",
            "qualifications",
            "revisions.prepared",
            "revisions.qualified",
            "revisions.selected_for_new_sessions",
        ]
    );
    assert_eq!(
        desired.wait_reasons,
        vec![
            reason(
                "qualification_unbound",
                "capability_qualifications",
                "animus#18"
            ),
            reason("revision_not_admitted", "revisions", "lattice#25"),
            reason("revision_fact_unknown", "revisions.prepared", "kiln#49"),
            reason("revision_fact_unknown", "revisions.qualified", "animus#18"),
            reason(
                "revision_fact_unknown",
                "revisions.selected_for_new_sessions",
                "lattice#25"
            ),
        ]
    );
    let unknown_pin = edit(
        MANAGED_LIVE,
        "/instance/pinned",
        json!({ "support": "unknown", "owner": "lattice#25" }),
    )?;
    let unknown_pin = workers(&[&unknown_pin])?;
    let unknown_pin = only_worker(&unknown_pin)?;
    assert_eq!(unknown_pin.pin, None);
    assert!(unknown(&unknown_pin.meta.unknown_fields, "pinned"));
    assert!(unknown_pin.wait_reasons.contains(&reason(
        "revision_fact_unknown",
        "pinned",
        "lattice#25"
    )));

    // Recorded: every slot is the lifecycle's fact.
    let promoted = only_definition(&[MANAGED_PROMOTED])?;
    let revisions = &promoted.revisions;
    assert_eq!(revisions.management, Some(RevisionManagement::Managed));
    for slot in [
        &revisions.desired,
        &revisions.prepared,
        &revisions.qualified,
        &revisions.selected_for_new_sessions,
    ] {
        assert_eq!(slot.as_deref(), Some(MANAGED_PROFILE));
    }
    assert_eq!(revisions.generation, 1, "the rollout generation");
    assert_eq!(revisions.evidence, vec![QUALIFICATION_REF.to_owned()]);
    assert_eq!(revisions.preparation_failure, None);
    assert_eq!(
        promoted.meta.unknown_fields,
        vec!["permitted_models", "qualifications"]
    );
    assert!(
        promoted.wait_reasons.is_empty(),
        "a selected, qualified definition has no definition-scoped reason"
    );
    Ok(())
}

#[test]
fn managed_lifecycle_captures_map_in_order() -> TestResult {
    let prepared = only_definition(&[MANAGED_PREPARED])?;
    assert_eq!(
        prepared.revisions.prepared.as_deref(),
        Some(MANAGED_PROFILE)
    );
    assert_eq!(prepared.revisions.qualified, None);
    assert!(!unknown(
        &prepared.meta.unknown_fields,
        "revisions.prepared"
    ));
    assert!(unknown(
        &prepared.meta.unknown_fields,
        "revisions.qualified"
    ));
    assert!(
        !prepared.wait_reasons.iter().any(|r| r.owner == "kiln#49"),
        "the preparation owner clears once preparation is recorded"
    );

    let promoted = only_definition(&[MANAGED_PROMOTED])?;
    assert_eq!(promoted.task_families, vec![FAMILY.to_owned()]);
    assert_eq!(
        promoted.task_family_qualifications,
        vec![fixture_qualification(QUALIFICATION_REF)]
    );

    let live = workers(&[MANAGED_LIVE])?;
    let live = only_worker(&live)?;
    assert_eq!(live.lifecycle, WorkerLifecycle::Reserved);
    assert_eq!(live.pinned_revision, MANAGED_PROFILE);
    assert_eq!(live.attempt.as_deref(), Some("attempt-managed-admitted"));
    assert_eq!(live.fence, Some(8));
    assert_eq!(
        live.definition,
        reference("lattice-agents", DECLARED_DEFINITION)
    );
    assert_eq!(
        live.pin,
        Some(RevisionPin::Recorded {
            qualification: QUALIFICATION_REF.into(),
            rollout_generation: 1,
            identity: LIVE_PIN.into(),
            status: PinStatus::Live,
            blocked: false,
        })
    );
    assert!(
        live.session.is_none(),
        "single-shot workers keep no session"
    );
    assert!(!unknown(&live.meta.unknown_fields, "pinned"));

    let released = workers(&[MANAGED_RELEASED])?;
    let released = only_worker(&released)?;
    assert_eq!(released.lifecycle, WorkerLifecycle::Stopped);
    assert!(released.wait_reasons.is_empty());
    assert_eq!(
        released.pin,
        Some(RevisionPin::Recorded {
            qualification: QUALIFICATION_REF.into(),
            rollout_generation: 1,
            identity: RELEASED_PIN.into(),
            status: PinStatus::Released,
            blocked: false,
        })
    );

    // After revocation of the revision: no current qualification, new
    // sessions refused, and both pins blocked.
    for document in [
        MANAGED_REVOKED_LIVE,
        MANAGED_REVOKED_TERMINAL,
        MANAGED_REFUSED,
    ] {
        let revoked = only_definition(&[document])?;
        assert_eq!(revoked.profile_revision, MANAGED_PROFILE);
        assert_eq!(revoked.task_families, vec![FAMILY.to_owned()]);
        assert!(revoked.task_family_qualifications.is_empty());
        assert_eq!(revoked.revisions.qualified, None);
        assert!(revoked.revisions.evidence.is_empty());
        assert_eq!(
            revoked.revisions.selected_for_new_sessions.as_deref(),
            Some(MANAGED_PROFILE),
            "revocation is not a slot; the selection stays recorded"
        );
        assert_eq!(
            revoked.wait_reasons,
            vec![
                reason(
                    "qualification_unbound",
                    "capability_qualifications",
                    "animus#18"
                ),
                reason("revision_not_admitted", "revisions", "lattice#25"),
            ]
        );
    }
    for (document, status) in [
        (MANAGED_REVOKED_LIVE, PinStatus::Live),
        (MANAGED_REVOKED_TERMINAL, PinStatus::Released),
    ] {
        let fragment = workers(&[document])?;
        let worker = only_worker(&fragment)?;
        assert!(
            matches!(&worker.pin, Some(RevisionPin::Recorded { status: s, blocked: true, .. }) if *s == status),
            "a pin on a revoked revision is blocked"
        );
    }
    assert!(
        workers(&[MANAGED_REFUSED])?.workers.is_empty(),
        "a refused attempt wrote nothing and is no worker"
    );
    let fleet = workers(&[MANAGED_LIVE, MANAGED_RELEASED])?;
    assert_eq!(fleet.workers.len(), 2);
    Ok(())
}

#[test]
fn selected_revision_coexists_with_older_pinned_sessions() -> TestResult {
    let older = pinned_on_older_revision()?;
    let selected = selected_next_profile()?;
    for order in [[&selected[..], &older[..]], [&older[..], &selected[..]]] {
        let agent = only_definition(&order)?;
        assert_eq!(
            agent.profile_revision, NEXT_REVISION,
            "the definition is projected at the selected revision"
        );
        assert_eq!(
            agent.revisions.selected_for_new_sessions.as_deref(),
            Some(NEXT_REVISION)
        );
        assert_eq!(agent.revisions.generation, 2);
        assert_eq!(agent.revisions.prepared.as_deref(), Some(NEXT_REVISION));
        assert_eq!(agent.revisions.qualified.as_deref(), Some(NEXT_REVISION));
        assert_eq!(agent.revisions.evidence, vec![NEXT_REF.to_owned()]);
        assert_eq!(
            agent.task_family_qualifications,
            vec![fixture_qualification(NEXT_REF)]
        );
        assert!(agent.wait_reasons.is_empty());
    }
    // Alone, the older profile is the definition at its own revision, and the
    // selection names the other one.
    let alone = only_definition(&[&older])?;
    assert_eq!(alone.profile_revision, MANAGED_PROFILE);
    assert_eq!(
        alone.revisions.selected_for_new_sessions.as_deref(),
        Some(NEXT_REVISION)
    );
    assert_eq!(alone.revisions.desired.as_deref(), Some(NEXT_REVISION));
    assert_eq!(
        alone.task_family_qualifications,
        vec![fixture_qualification(QUALIFICATION_REF)],
        "the older revision's own qualification is still current"
    );
    assert!(alone.wait_reasons.contains(&reason(
        "revision_not_admitted",
        "revisions",
        "lattice#25"
    )));
    // The session keeps its pin on the non-selected revision.
    let fleet = workers(&[&selected, &older])?;
    let worker = only_worker(&fleet)?;
    assert_eq!(worker.pinned_revision, MANAGED_PROFILE);
    assert_eq!(
        worker.pin,
        Some(RevisionPin::Recorded {
            qualification: QUALIFICATION_REF.into(),
            rollout_generation: 1,
            identity: LIVE_PIN.into(),
            status: PinStatus::Live,
            blocked: false,
        })
    );
    Ok(())
}

fn refuses(documents: &[&[u8]], expected: &str) {
    assert_eq!(agents(documents).err(), Some(expected), "agents");
    assert_eq!(workers(documents).err(), Some(expected), "workers");
}

#[test]
fn contradictory_lifecycles_refuse() -> TestResult {
    let older = pinned_on_older_revision()?;
    // Two selected revisions for one definition.
    refuses(
        &[MANAGED_PROMOTED, &older],
        "lattice_definition_selection_conflict",
    );
    refuses(
        &[&older, MANAGED_LIVE],
        "lattice_definition_selection_conflict",
    );
    refuses(
        &[MANAGED_DESIRED, MANAGED_PREPARED],
        "lattice_definition_lifecycle_conflict",
    );
    refuses(
        // One definition, unmanaged in one document and managed in another.
        &[MANAGED_PROMOTED, DECLARED],
        "lattice_definition_lifecycle_conflict",
    );
    // Several revisions without a selection to choose by.
    let other_desired = edit(MANAGED_DESIRED, "/profile/revision", json!(NEXT_REVISION))?;
    assert_eq!(
        agents(&[MANAGED_DESIRED, &other_desired]).err(),
        Some("lattice_definition_revision_ambiguous")
    );
    assert_eq!(
        agents(&[FIRST, SERVED]).err(),
        Some("lattice_definition_revision_ambiguous"),
        "unmanaged revisions have no selection"
    );
    assert_eq!(
        workers(&[FIRST, SERVED])?.workers.len(),
        2,
        "workers pinned to two revisions of one definition stay visible"
    );
    // Revocation changes one incarnation's observation at one generation.
    assert_eq!(
        workers(&[MANAGED_LIVE, MANAGED_REVOKED_LIVE]).err(),
        Some("lattice_instance_conflict")
    );
    assert_eq!(
        agents(&[MANAGED_PROMOTED, MANAGED_REVOKED_LIVE]).err(),
        Some("lattice_profile_conflict")
    );
    // A selection must rest on a recorded qualification of its revision.
    let unqualified = edit(
        MANAGED_PROMOTED,
        "/profile/revisions/selected_for_new_sessions/value/qualification_ref",
        json!(NEXT_REF),
    )?;
    refuses(&[&unqualified], "lattice_revision_selection_unqualified");
    let no_attachment = edit(
        MANAGED_PROMOTED,
        "/profile/revisions/qualified",
        json!({ "support": "unknown", "owner": "animus#18" }),
    )?;
    refuses(&[&no_attachment], "lattice_revision_selection_unqualified");
    Ok(())
}

#[test]
fn revoked_qualification_is_never_current() -> TestResult {
    let revoked = revoked_qualification(MANAGED_PROMOTED)?;
    let agent = only_definition(&[&revoked])?;
    assert!(agent.task_family_qualifications.is_empty());
    assert_eq!(agent.revisions.qualified, None);
    assert!(agent.revisions.evidence.is_empty());
    assert!(agent.wait_reasons.contains(&reason(
        "revision_not_admitted",
        "revisions",
        "lattice#25"
    )));
    // A revoked attachment listed as current, or refused sessions unreported.
    let still_listed = edit(
        MANAGED_PROMOTED,
        "/profile/revisions/qualified/value/0/is_revoked",
        json!(true),
    )?;
    refuses(
        &[&still_listed],
        "lattice_capability_qualification_inconsistent",
    );
    let unreported = edit(
        &revoked,
        "/profile/wait_reasons",
        json!([
            reason_json(
                "qualification_unbound",
                "capability_qualifications",
                "animus#18"
            ),
            reason_json("serving_port_absent", "serving_readiness", "kiln#49"),
        ]),
    )?;
    refuses(&[&unreported], "lattice_wait_reason_inconsistent");
    // A live pin on a revoked qualification must be blocked.
    let revoked_live = revoked_qualification(MANAGED_LIVE)?;
    refuses(&[&revoked_live], "lattice_pinned_inconsistent");
    let blocked = edit(
        &revoked_live,
        "/instance/pinned/value/is_blocked",
        json!(true),
    )?;
    let fleet = workers(&[&blocked])?;
    assert!(matches!(
        only_worker(&fleet)?.pin,
        Some(RevisionPin::Recorded { blocked: true, .. })
    ));
    Ok(())
}

#[test]
fn qualification_provenance_is_kept_and_never_upgraded() -> TestResult {
    let promoted = only_definition(&[MANAGED_PROMOTED])?;
    assert_eq!(
        promoted
            .task_family_qualifications
            .first()
            .map(|q| q.provenance),
        Some(EvidenceProvenance::Fixture),
        "fixture evidence stays fixture"
    );
    let live = edits(
        MANAGED_PROMOTED,
        &[
            (
                "/profile/capability_qualifications/0/provenance",
                json!("live"),
            ),
            (
                "/profile/revisions/qualified/value/0/provenance",
                json!("live"),
            ),
        ],
    )?;
    let live = only_definition(&[&live])?;
    assert_eq!(
        live.task_family_qualifications
            .first()
            .map(|q| q.provenance),
        Some(EvidenceProvenance::Live)
    );
    let relabelled = edit(
        MANAGED_PROMOTED,
        "/profile/capability_qualifications/0/provenance",
        json!("live"),
    )?;
    refuses(
        &[&relabelled],
        "lattice_capability_qualification_inconsistent",
    );
    for value in [json!("simulated"), json!(null)] {
        let document = edit(
            MANAGED_PROMOTED,
            "/profile/capability_qualifications/0/provenance",
            value,
        )?;
        refuses(&[&document], "lattice_inventory_malformed");
    }
    Ok(())
}

#[test]
fn unavailable_capability_is_a_known_negative() -> TestResult {
    let unavailable = edit(
        DECLARED,
        "/profile/tools",
        json!({ "support": "unavailable", "value": null }),
    )?;
    let fragment = agents(&[&unavailable])?;
    let declared = definition(&fragment, DECLARED_DEFINITION)?;
    assert!(declared.tools.is_empty());
    assert!(
        !unknown(&declared.meta.unknown_fields, "tools"),
        "unavailable is known, not unknown"
    );
    assert_eq!(declared.task_families.len(), 2);
    // A family is qualified only with a current qualification for every name.
    let unqualified = edit(
        DECLARED,
        "/profile/task_families/support",
        "qualified".into(),
    )?;
    refuses(
        &[&unqualified],
        "lattice_capability_qualification_inconsistent",
    );
    let qualified = edits(
        MANAGED_PROMOTED,
        &[
            ("/profile/task_families/support", json!("declared")),
            (
                "/profile/wait_reasons",
                json!([
                    reason_json(
                        "qualification_unbound",
                        "capability_qualifications",
                        "animus#18"
                    ),
                    reason_json("serving_port_absent", "serving_readiness", "kiln#49"),
                ]),
            ),
        ],
    )?;
    refuses(
        &[&qualified],
        "lattice_capability_qualification_inconsistent",
    );
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
fn instances_join_definition_identity_and_never_report_idle() -> TestResult {
    let fragment = workers(&[FIRST, SECOND, SERVED, PROFILE_ONLY])?;
    assert!(fragment.agents.is_empty());
    assert_eq!(fragment.workers.len(), 3);
    for worker in &fragment.workers {
        assert!(!matches!(
            worker.lifecycle,
            WorkerLifecycle::Idle | WorkerLifecycle::Healthy | WorkerLifecycle::Executing
        ));
        assert_eq!(
            worker.definition,
            reference("lattice-agents", DEFINITION),
            "workers join the definition identity, not the revision"
        );
        assert_eq!(worker.fence, Some(7));
        assert_eq!(
            worker.execution_grant, None,
            "grant references are never projected"
        );
        assert_eq!(worker.workspace, None, "workspace paths are restricted");
        assert_eq!(worker.session, None);
        assert_eq!(worker.pin, Some(RevisionPin::Unmanaged {}));
        for name in ["allocation_liveness", "host_reachability"] {
            assert!(unknown(&worker.meta.unknown_fields, name));
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
    assert!(unknown(&host_local.meta.unknown_fields, "execution"));
    assert!(!unknown(&host_local.meta.unknown_fields, "liveness"));
    assert_eq!(host_local.lifecycle, WorkerLifecycle::Reserved);
    assert_eq!(host_local.attempt.as_deref(), Some("attempt-host-local"));
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
    assert_eq!(first.pinned_revision, PROFILE);
    assert_eq!(first.attempt.as_deref(), Some("attempt-agent-a-1"));
    assert_eq!(first.lifecycle, WorkerLifecycle::Unknown);
    assert!(unknown(&first.meta.unknown_fields, "liveness"));
    assert!(first.model_services.is_empty());
    assert!(
        unknown(&first.meta.unknown_fields, "model_services"),
        "a route without a serving reading is an unknown binding, not none"
    );
    assert_eq!(
        first.wait_reasons,
        vec![
            reason("serving_port_absent", "serving_readiness", "kiln#49"),
            reason("dispatch_outcome_unknown", "lifecycle", "lattice#6"),
            reason("liveness_unobserved", "liveness", "kiln#49"),
            reason("host_reachability_unknown", "host_reachability", "kiln#49"),
            reason(
                "allocation_liveness_unknown",
                "allocation_liveness",
                "kiln#49"
            ),
        ]
    );
    let unavailable = workers(&[SERVED_UNAVAILABLE])?;
    assert_eq!(
        only_worker(&unavailable)?.wait_reasons.first(),
        Some(&WaitReason {
            code: "serving_snapshot_unavailable".into(),
            subject: WaitSubject::Route {
                name: "primary".into()
            },
            owner: "animus#17".into(),
        })
    );
    let declared = workers(&[DECLARED])?;
    assert_eq!(
        only_worker(&declared)?.definition,
        reference("lattice-agents", DECLARED_DEFINITION)
    );
    Ok(())
}

#[test]
fn lifecycles_and_liveness_map_conservatively() -> TestResult {
    let admitted = workers(&[ADMITTED])?;
    let admitted = only_worker(&admitted)?;
    assert_eq!(admitted.lifecycle, WorkerLifecycle::Reserved);
    assert!(!unknown(&admitted.meta.unknown_fields, "liveness"));
    assert!(
        admitted
            .wait_reasons
            .contains(&reason("awaiting_dispatch", "lifecycle", "kiln#45"))
    );
    let terminal = workers(&[TERMINAL])?;
    let terminal = only_worker(&terminal)?;
    assert_eq!(terminal.lifecycle, WorkerLifecycle::Stopped);
    assert!(terminal.wait_reasons.is_empty());
    let stale = edits(
        FIRST,
        &[
            ("/instance/liveness/unknown_cause", "stale".into()),
            ("/instance/wait_reasons/2/code", "liveness_stale".into()),
        ],
    )?;
    let stale = workers(&[&stale])?;
    let stale = only_worker(&stale)?;
    assert_eq!(stale.lifecycle, WorkerLifecycle::Unknown);
    assert!(unknown(&stale.meta.unknown_fields, "liveness"));
    assert!(
        stale
            .wait_reasons
            .contains(&reason("liveness_stale", "liveness", "kiln#49"))
    );
    for state in [
        "binding_rejected_before_dispatch",
        "revision_blocked_before_dispatch",
    ] {
        let settled = edits(
            MANAGED_REVOKED_LIVE,
            &[
                ("/instance/lifecycle", json!({ "state": state })),
                ("/instance/wait_reasons", json!([])),
            ],
        )?;
        assert_eq!(
            only_worker(&workers(&[&settled])?)?.lifecycle,
            WorkerLifecycle::Stopped
        );
        let waiting = edit(
            MANAGED_REVOKED_LIVE,
            "/instance/lifecycle",
            json!({ "state": state }),
        )?;
        refuses(&[&waiting], "lattice_wait_reason_inconsistent");
    }
    let cancelled = edit(
        FIRST,
        "/instance/cancellation",
        "requested_unobserved".into(),
    )?;
    let fragment = workers(&[&cancelled])?;
    assert!(unknown(
        &only_worker(&fragment)?.meta.unknown_fields,
        "cancellation"
    ));
    Ok(())
}

#[test]
fn retained_generation_orders_one_incarnation() -> TestResult {
    for order in [
        [ADMITTED, FIRST, TERMINAL],
        [TERMINAL, FIRST, ADMITTED],
        [FIRST, TERMINAL, ADMITTED],
    ] {
        let fragment = workers(&order)?;
        assert_eq!(
            only_worker(&fragment)?.lifecycle,
            WorkerLifecycle::Stopped,
            "the greater retained generation wins in any order"
        );
    }
    let cancelled = edit(
        FIRST,
        "/instance/cancellation",
        "requested_unobserved".into(),
    )?;
    assert_eq!(
        workers(&[FIRST, &cancelled]).err(),
        Some("lattice_instance_conflict"),
        "equal generations must agree"
    );
    assert_eq!(workers(&[FIRST, FIRST])?.workers.len(), 1);
    Ok(())
}

#[test]
fn v2_and_unsupported_schemas_refuse_as_unsupported() -> TestResult {
    // A v2 document: v2 labels, the top-level `qualification_ref`, five owned
    // unknown slots and `revision_selection_unpublished`, no `pinned`.
    let owned = json!({ "support": "unknown", "owner": "lattice#25" });
    let mut v2 = edits(
        FIRST,
        &[
            ("/schema", "lattice.worker-inventory.v2".into()),
            ("/profile/schema", "lattice.worker-profile-facts.v2".into()),
            (
                "/instance/schema",
                "lattice.worker-instance-observation.v2".into(),
            ),
            (
                "/profile/revisions",
                json!({ "desired": owned, "prepared": owned, "qualified": owned, "selected_for_new_sessions": owned, "pinned": owned }),
            ),
            (
                "/profile/wait_reasons/2",
                reason_json("revision_selection_unpublished", "revisions", "lattice#25"),
            ),
        ],
    )
    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string()))?;
    let profile = v2
        .pointer_mut("/profile")
        .and_then(Value::as_object_mut)
        .ok_or("not an object")?;
    profile.remove("capability_qualifications");
    profile.insert("qualification_ref".into(), Value::Null);
    v2.pointer_mut("/instance")
        .and_then(Value::as_object_mut)
        .ok_or("not an object")?
        .remove("pinned");
    let v2 = serde_json::to_vec(&v2).map_err(|e| e.to_string())?;
    refuses(&[&v2], "lattice_inventory_unsupported_schema");
    for (pointer, schema) in [
        ("/schema", "lattice.worker-inventory.v2"),
        ("/profile/schema", "lattice.worker-profile-facts.v2"),
        ("/instance/schema", "lattice.worker-instance-observation.v2"),
        ("/schema", "lattice.worker-inventory.v1"),
        ("/schema", "lattice.worker-inventory.v4"),
    ] {
        let document = edit(FIRST, pointer, schema.into())?;
        refuses(&[&document], "lattice_inventory_unsupported_schema");
    }
    let wrong_source = edit(FIRST, "/instance/source", "other-state".into())?;
    refuses(&[&wrong_source], "lattice_inventory_unsupported_schema");
    Ok(())
}

#[test]
fn malformed_revision_slots_refuse() -> TestResult {
    let pin = field(MANAGED_RELEASED, "/instance/pinned")?;
    for (base, pointer, value, expected) in [
        (
            MANAGED_PROMOTED,
            "/profile/revisions/desired",
            json!({ "support": "unmanaged" }),
            "lattice_revision_slots_inconsistent",
        ),
        (
            TERMINAL,
            "/profile/revisions/prepared",
            json!({ "support": "unknown", "owner": "kiln#49" }),
            "lattice_revision_slots_inconsistent",
        ),
        (
            MANAGED_DESIRED,
            "/profile/revisions/prepared/owner",
            json!("lattice#25"),
            "lattice_revision_owner_invalid",
        ),
        (
            MANAGED_DESIRED,
            "/profile/revisions/qualified/owner",
            json!("kiln#49"),
            "lattice_revision_owner_invalid",
        ),
        (
            MANAGED_DESIRED,
            "/profile/revisions/selected_for_new_sessions/owner",
            json!("animus#18"),
            "lattice_revision_owner_invalid",
        ),
        (
            MANAGED_LIVE,
            "/instance/pinned",
            json!({ "support": "unknown", "owner": "kiln#49" }),
            "lattice_revision_owner_invalid",
        ),
        (
            MANAGED_DESIRED,
            "/profile/revisions/desired/value/revision",
            json!("ABC"),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PREPARED,
            "/profile/revisions/prepared/value",
            json!([]),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PREPARED,
            "/profile/revisions/prepared/value",
            json!([NEXT_REVISION, MANAGED_PROFILE]),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/qualified/value/0/qualification_ref",
            json!("sha256:abc"),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/qualified/value",
            json!([]),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/qualified/value/0/task_family",
            json!("Rust Implementation"),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/capability_qualifications/0/qualification_ref",
            json!("blake3:abc"),
            "lattice_capability_qualification_invalid",
        ),
        (
            MANAGED_LIVE,
            "/instance/pinned/value/pin_identity",
            json!("blake3:xyz"),
            "lattice_revision_invalid",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/qualified/value/0/evidence_class",
            json!("vibes"),
            "lattice_inventory_malformed",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/selected_for_new_sessions/value/kind",
            json!("forced"),
            "lattice_inventory_malformed",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/desired",
            json!({ "support": "assumed" }),
            "lattice_inventory_malformed",
        ),
        (
            TERMINAL,
            "/profile/revisions/desired",
            json!({ "support": "unmanaged", "owner": "lattice#25" }),
            "lattice_inventory_malformed",
        ),
        (
            MANAGED_PROMOTED,
            "/profile/revisions/desired",
            json!({ "support": "recorded" }),
            "lattice_inventory_malformed",
        ),
        (
            MANAGED_LIVE,
            "/instance/pinned/value/status",
            json!("open"),
            "lattice_inventory_malformed",
        ),
        (
            TERMINAL,
            "/instance/pinned",
            pin.clone(),
            "lattice_pinned_inconsistent",
        ),
        (
            MANAGED_RELEASED,
            "/instance/pinned/value/revision",
            json!(NEXT_REVISION),
            "lattice_pinned_inconsistent",
        ),
        (
            MANAGED_LIVE,
            "/instance/pinned/value/qualification_ref",
            json!(NEXT_REF),
            "lattice_pinned_inconsistent",
        ),
    ] {
        let document = edit(base, pointer, value)?;
        refuses(&[&document], expected);
    }
    // A released pin outlives its revision's recorded qualification.
    let forgotten = edit(
        MANAGED_RELEASED,
        "/instance/pinned/value/qualification_ref",
        json!(NEXT_REF),
    )?;
    assert_eq!(workers(&[&forgotten])?.workers.len(), 1);
    Ok(())
}

#[test]
fn revision_wait_reasons_follow_the_lifecycle() -> TestResult {
    let not_admitted = reason_json("revision_not_admitted", "revisions", "lattice#25");
    let unbound = reason_json(
        "qualification_unbound",
        "capability_qualifications",
        "animus#18",
    );
    let serving = reason_json("serving_port_absent", "serving_readiness", "kiln#49");
    for (base, reasons) in [
        // An unmanaged profile is never refused by a lifecycle.
        (
            TERMINAL,
            json!([
                reason_json("capability_undeclared", "task_families", "configuration"),
                reason_json("capability_undeclared", "tools", "configuration"),
                unbound.clone(),
                not_admitted.clone(),
                serving.clone(),
            ]),
        ),
        // Nothing is selected, so a new session would be refused.
        (MANAGED_DESIRED, json!([unbound.clone(), serving.clone()])),
        // A selected revision with current qualifications is admitted.
        (
            MANAGED_PROMOTED,
            json!([not_admitted.clone(), serving.clone()]),
        ),
        // `qualification_unbound` only while the family is not qualified.
        (MANAGED_PROMOTED, json!([unbound.clone(), serving.clone()])),
        (
            MANAGED_PREPARED,
            json!([not_admitted.clone(), serving.clone()]),
        ),
    ] {
        let document = edit(base, "/profile/wait_reasons", reasons)?;
        refuses(&[&document], "lattice_wait_reason_inconsistent");
    }
    let retired = edit(
        MANAGED_PROMOTED,
        "/profile/wait_reasons/0",
        reason_json("revision_selection_unpublished", "revisions", "lattice#25"),
    )?;
    refuses(&[&retired], "lattice_inventory_malformed");
    let old_subject = edit(
        TERMINAL,
        "/profile/wait_reasons/2/subject/field",
        json!("qualification_ref"),
    )?;
    refuses(&[&old_subject], "lattice_inventory_malformed");
    Ok(())
}

#[test]
fn malformed_v3_documents_refuse() -> TestResult {
    let mut extra: Value = serde_json::from_slice(FIRST).map_err(|e| e.to_string())?;
    extra
        .pointer_mut("/instance")
        .and_then(Value::as_object_mut)
        .ok_or("not an object")?
        .insert("idle".into(), Value::Bool(true));
    let extra = serde_json::to_vec(&extra).map_err(|e| e.to_string())?;
    refuses(&[&extra], "lattice_inventory_malformed");
    refuses(&[b"{"], "lattice_inventory_malformed");
    let long = "a".repeat(super::MAX_NAME_BYTES + 1);
    let many: Vec<String> = (0..=super::MAX_CAPABILITY_NAMES)
        .map(|i| format!("tool-{i:02}"))
        .collect();
    for (pointer, value, expected) in [
        (
            "/instance/liveness/state",
            json!("idle"),
            "lattice_inventory_malformed",
        ),
        (
            "/instance/liveness/state",
            json!("live"),
            "lattice_inventory_malformed",
        ),
        (
            "/instance/liveness/runtime",
            json!("reusable"),
            "lattice_inventory_malformed",
        ),
        (
            "/instance/liveness/unknown_cause",
            json!("expired"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/wait_reasons/0/code",
            json!("waiting"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/wait_reasons/0/subject/field",
            json!("prompt"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/wait_reasons/0/subject/kind",
            json!("text"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/task_families/support",
            json!("assumed"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/definition/basis",
            json!("inferred"),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/capability_qualifications",
            json!(null),
            "lattice_inventory_malformed",
        ),
        (
            "/profile/tools/value",
            json!(["edit"]),
            "lattice_fact_support_inconsistent",
        ),
        (
            "/profile/adapter/support",
            json!("qualified"),
            "lattice_fact_support_inconsistent",
        ),
        (
            "/profile/definition/identity",
            json!("blake3:abc"),
            "lattice_definition_identity_invalid",
        ),
        (
            "/profile/definition/identity",
            json!(PROFILE),
            "lattice_definition_identity_invalid",
        ),
        (
            "/profile/definition/name",
            json!(long),
            "lattice_definition_name_invalid",
        ),
        (
            "/profile/definition/name",
            json!("Rust Implementer"),
            "lattice_definition_name_invalid",
        ),
        (
            "/instance/host_reachability/support",
            json!("declared"),
            "lattice_liveness_unsupported",
        ),
        (
            "/instance/allocation_liveness/owner",
            json!("lattice#6"),
            "lattice_liveness_unsupported",
        ),
        (
            "/profile/wait_reasons/0/owner",
            json!("kiln#49"),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/profile/wait_reasons/2/subject/field",
            json!("tools"),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/profile/wait_reasons/2",
            json!({ "code": "awaiting_dispatch", "subject": { "kind": "fact", "field": "lifecycle" }, "owner": "kiln#45" }),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/instance/wait_reasons/1",
            json!({ "code": "capability_undeclared", "subject": { "kind": "fact", "field": "tools" }, "owner": "configuration" }),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/instance/wait_reasons/1",
            reason_json("revision_not_admitted", "revisions", "lattice#25"),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/profile/wait_reasons/1",
            json!({ "code": "capability_undeclared", "subject": { "kind": "fact", "field": "task_families" }, "owner": "configuration" }),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/instance/liveness/state",
            json!("not_started"),
            "lattice_liveness_inconsistent",
        ),
        (
            "/instance/liveness/unknown_cause",
            json!(null),
            "lattice_liveness_inconsistent",
        ),
        (
            "/instance/liveness/unknown_cause",
            json!("stale"),
            "lattice_wait_reason_inconsistent",
        ),
        (
            "/instance/definition_identity",
            json!(DECLARED_DEFINITION),
            "lattice_instance_definition_mismatch",
        ),
        (
            "/instance/profile_revision",
            json!("other-profile"),
            "lattice_instance_profile_mismatch",
        ),
        (
            "/profile/outputs/value",
            json!(null),
            "lattice_fact_support_inconsistent",
        ),
    ] {
        let document = edit(FIRST, pointer, value)?;
        refuses(&[&document], expected);
    }
    for (value, expected) in [
        (json!([long]), "lattice_capability_invalid"),
        (json!([]), "lattice_capability_invalid"),
        (json!(["read", "edit"]), "lattice_capability_invalid"),
        (json!(["edit", "edit"]), "lattice_capability_invalid"),
        (json!(["Edit"]), "lattice_capability_invalid"),
        (json!(["bin/sh"]), "lattice_capability_invalid"),
        (json!(["run tests"]), "lattice_capability_invalid"),
        (json!(["a..b"]), "lattice_capability_invalid"),
        (json!(many), "lattice_capability_invalid"),
    ] {
        let document = edit(DECLARED, "/profile/tools/value", value)?;
        refuses(&[&document], expected);
    }
    let qualified_tools = edit(
        MANAGED_PROMOTED,
        "/profile/tools/support",
        json!("qualified"),
    )?;
    refuses(&[&qualified_tools], "lattice_fact_support_inconsistent");
    // An unknown set must carry `capability_undeclared`, and only then.
    let silent = edit(
        DECLARED,
        "/profile/tools",
        json!({ "support": "unknown", "value": null }),
    )?;
    refuses(&[&silent], "lattice_wait_reason_inconsistent");
    let unknown_route = edit(
        SERVED_UNAVAILABLE,
        "/instance/wait_reasons/0/subject/name",
        "secondary".into(),
    )?;
    refuses(&[&unknown_route], "lattice_wait_reason_inconsistent");
    let reasons = field(TERMINAL, "/profile/wait_reasons")?;
    let settled = edit(
        TERMINAL,
        "/instance/wait_reasons",
        json!([{ "code": "host_reachability_unknown", "subject": { "kind": "fact", "field": "host_reachability" }, "owner": "kiln#49" }]),
    )?;
    refuses(&[&settled], "lattice_wait_reason_inconsistent");
    let mut repeated = reasons.as_array().cloned().ok_or("not a list")?;
    repeated.extend(repeated.clone());
    let duplicate = edit(TERMINAL, "/profile/wait_reasons", Value::Array(repeated))?;
    refuses(&[&duplicate], "lattice_wait_reason_inconsistent");
    let bounded: Vec<Value> = (0..=super::wait::MAX_LATTICE_WAIT_REASONS)
        .map(|i| json!({ "code": "serving_not_ready", "subject": { "kind": "route", "name": format!("route-{i}") }, "owner": "animus#17" }))
        .collect();
    let bounded = edit(TERMINAL, "/profile/wait_reasons", Value::Array(bounded))?;
    refuses(&[&bounded], "lattice_wait_reason_bound");
    Ok(())
}

#[test]
fn out_of_window_and_unsupported_inputs_refuse() -> TestResult {
    let conflicting = edit(SECOND, "/profile/outputs/value", "other-contract".into())?;
    assert_eq!(
        agents(&[FIRST, &conflicting]).err(),
        Some("lattice_profile_conflict")
    );
    let mut early = source("lattice-workers");
    early.observed_at_ms = 1_000;
    assert_eq!(
        map_worker_inventory(InventoryKind::Workers, &early, &joins(), &[FIRST]).err(),
        Some("lattice_instance_after_collection")
    );
    let late_liveness = edit(
        FIRST,
        "/instance/liveness/observed_at_epoch_ms",
        json!(COLLECTED_AT_MS + 1),
    )?;
    assert_eq!(
        workers(&[&late_liveness]).err(),
        Some("lattice_instance_after_collection")
    );
    let negative_time = edit(FIRST, "/instance/observed_at_epoch_ms", json!(-1))?;
    assert_eq!(
        workers(&[&negative_time]).err(),
        Some("lattice_instance_time_invalid")
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
