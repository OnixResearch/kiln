//! Lattice `lattice.workflow-agent-revision-slots.v1` documents captured from
//! `lattice agent-revision inspect` at Lattice `7fb6dc47` (deterministic
//! fixture ports only), copied verbatim; `provenance.json` records how and
//! their hashes. Negative cases are derived from the captures.
// r[verify kiln.inventory.workflow]
// r[verify kiln.inventory.workflow.scenario.mapped]
// r[verify kiln.inventory.workflow.scenario.refused]
use kiln_core::inventory::{
    FactRef, InventoryJoins, InventoryKind, PinStatus, SelectionKind, SourceObservation,
    WorkflowPin, WorkflowRevisionSlots,
};
use serde_json::{Value, json};

use super::{MAX_WORKFLOW_REVISION_SLOTS_BYTES, map_workflow_revision_slots};

type TestResult<T = ()> = Result<T, String>;

macro_rules! capture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../fixtures/inventory/producers/lattice-workflows/",
            $name
        ))
    };
}

const DESIRED: &[u8] = capture!("workflow-1-desired.json");
const PROMOTED: &[u8] = capture!("workflow-2-promoted.json");
const PINNED: &[u8] = capture!("workflow-3-session-pinned.json");
const SECOND: &[u8] = capture!("workflow-4-second-revision.json");
const PROVENANCE: &str =
    include_str!("../../../fixtures/inventory/producers/lattice-workflows/provenance.json");
const CAPTURES: [(&str, &[u8]); 4] = [
    ("workflow-1-desired.json", DESIRED),
    ("workflow-2-promoted.json", PROMOTED),
    ("workflow-3-session-pinned.json", PINNED),
    ("workflow-4-second-revision.json", SECOND),
];
const COLLECTED_AT_MS: u64 = 1_795_237_890_000;
const VALID_UNTIL_MS: u64 = 1_795_237_950_000;
/// The `verify-change` workflow-agent definition.
const DEFINITION: &str = "blake3:176820ae10f42401cfc32aa5e9cdd755cd55a5e9c1255bee5acd5d75b0aaa842";
/// The first workflow revision (guidance `guidance`, worker executable `e`).
const FIRST: &str = "555665f511e3244805be21e30e782770151ca46182e74c6c5260bcdff45817ca";
/// The second workflow revision (guidance `guidance-2`, worker executable `9`).
const SECOND_REVISION: &str = "02dad1b763859fca0fc25d43b03cbbc3fd4059e72215c1661411178fbe9dca4b";
/// The `rust-implementer` worker definition both revisions pair with.
const WORKER_DEFINITION: &str =
    "blake3:367783379a41fd2e834f26af4e7504906c1f3967bd9f21bba979441a1f0789c1";
const FIRST_WORKER: &str = "96737fddb3dd1ef86fb1916f1731aa65ca70455506f4a08c984ce04380349f46";
const SECOND_WORKER: &str = "361ecc1502a86dbbe7ad1fd45df493c1e00f3c008750ef448a118a5231294e10";
const SECOND_BUNDLE: &str =
    "blake3:cd14d0c903338e76a265ddfdb3b469016105a057b17512f0acc70faebc3c9cb5";
const FIRST_QUALIFICATION: &str =
    "blake3:f003db3c8fddc3611cd75cdcb05108606923e0bc137e99f53a83bfdd5c8fd6d6";
const SECOND_QUALIFICATION: &str =
    "blake3:b2dea48d667b2821a9bcf69eded39a2458a1d8165ca7fcac64c3557b69a7ea08";
const FIRST_PLAN: &str = "b3:3fe31d36b99ede22dd956048352efd7a730432d28e547bca5e18d430efd15e09";
const SECOND_PLAN: &str = "b3:03fd9ac7a53f8013f14693619725203eb9dab4ab4f4fc16ba4c140ea07d1f40b";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn source() -> SourceObservation {
    SourceObservation {
        source: "lattice-workflows".into(),
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

fn map(documents: &[&[u8]]) -> Result<Vec<WorkflowRevisionSlots>, &'static str> {
    let fragment = map_workflow_revision_slots(&source(), &joins(), documents)?;
    assert_eq!(fragment.kind, InventoryKind::WorkflowRevisions);
    assert!(fragment.agents.is_empty() && fragment.workers.is_empty());
    Ok(fragment.workflow_revisions)
}

fn only(document: &[u8]) -> TestResult<WorkflowRevisionSlots> {
    let mut facts = map(&[document]).map_err(str::to_owned)?;
    if facts.len() != 1 {
        return Err(format!("{} facts", facts.len()));
    }
    facts.pop().ok_or_else(|| "no fact".into())
}

fn json_of(document: &[u8]) -> TestResult<Value> {
    serde_json::from_slice(document).map_err(|e| e.to_string())
}

/// `document` with each JSON pointer replaced, or removed for `None`.
fn edited(document: &[u8], edits: &[(&str, Option<Value>)]) -> TestResult<Vec<u8>> {
    let mut value = json_of(document)?;
    for (pointer, replacement) in edits {
        match replacement {
            Some(replacement) => {
                let slot = value
                    .pointer_mut(pointer)
                    .ok_or_else(|| format!("missing {pointer}"))?;
                *slot = replacement.clone();
            }
            None => {
                let (parent, key) = pointer.rsplit_once('/').ok_or("pointer")?;
                value
                    .pointer_mut(parent)
                    .and_then(Value::as_object_mut)
                    .and_then(|object| object.remove(key))
                    .ok_or_else(|| format!("missing {pointer}"))?;
            }
        }
    }
    serde_json::to_vec(&value).map_err(|e| e.to_string())
}

fn refusal(document: &[u8]) -> Option<&'static str> {
    map(&[document]).err()
}

#[test]
fn captures_match_their_provenance() -> TestResult {
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|e| e.to_string())?;
    assert_eq!(
        provenance.get("contract").and_then(Value::as_str),
        Some(super::WORKFLOW_REVISION_SLOTS_SCHEMA)
    );
    assert_eq!(
        provenance.get("lattice_commit").and_then(Value::as_str),
        Some("7fb6dc473c2cd7ae23282f515c741d398e885fcf")
    );
    assert_eq!(provenance.get("copied"), Some(&json!("verbatim")));
    assert_eq!(provenance.get("synthetic"), Some(&json!(true)));
    let hashes = provenance
        .get("blake3")
        .and_then(Value::as_object)
        .ok_or("missing hashes")?;
    assert_eq!(hashes.len(), CAPTURES.len());
    for (name, bytes) in CAPTURES {
        assert_eq!(
            hashes.get(name).and_then(Value::as_str),
            Some(blake3::hash(bytes).to_hex().as_str()),
            "{name} is not the recorded capture"
        );
    }
    Ok(())
}

#[test]
fn a_desired_revision_maps_with_its_workflow_and_worker() -> TestResult {
    let fact = only(DESIRED)?;
    assert_eq!(
        fact.meta.reference,
        FactRef {
            source: "lattice-workflows".into(),
            id: DEFINITION.into()
        }
    );
    assert_eq!(fact.meta.observed_at_ms, COLLECTED_AT_MS);
    assert_eq!(fact.meta.valid_until_ms, VALID_UNTIL_MS);
    assert!(fact.meta.unknown_fields.is_empty());
    assert_eq!(fact.desired.as_deref(), Some(FIRST));
    assert_eq!(fact.last_usable, None);
    assert_eq!(fact.selected_for_new_sessions, None);
    assert!(fact.pinned.is_empty());
    let [revision] = fact.revisions.as_slice() else {
        return Err("one revision".into());
    };
    assert_eq!(revision.revision, FIRST);
    assert_eq!(revision.workflow.plan, FIRST_PLAN);
    assert_eq!(
        revision.workflow.compiler_revision,
        "lattice.workflow-intent-compiler.v1"
    );
    assert!(revision.workflow.graph.starts_with("blake3:"));
    for cohort in [
        &revision.workflow.selection,
        &revision.workflow.obligations,
        &revision.workflow.policy,
    ] {
        assert!(cohort.starts_with("b3:"));
    }
    assert_eq!(
        revision.worker.definition,
        FactRef {
            source: "lattice-agents".into(),
            id: WORKER_DEFINITION.into()
        },
        "the worker joins the agents source by its exact definition identity"
    );
    assert_eq!(revision.worker.revision, FIRST_WORKER);
    Ok(())
}

#[test]
fn selection_and_pins_map_per_revision_and_state() -> TestResult {
    let promoted = only(PROMOTED)?;
    let selection = promoted
        .selected_for_new_sessions
        .as_ref()
        .ok_or("selection")?;
    assert_eq!(selection.revision, FIRST);
    assert_eq!(selection.rollout_generation, 1);
    assert_eq!(selection.kind, SelectionKind::Promotion);
    assert_eq!(selection.qualification, FIRST_QUALIFICATION);
    assert_eq!(promoted.last_usable.as_deref(), Some(FIRST));
    assert!(promoted.pinned.is_empty());

    let pinned = only(PINNED)?;
    assert_eq!(
        pinned.pinned,
        vec![WorkflowPin {
            revision: FIRST.into(),
            status: PinStatus::Live,
            blocked: false,
            sessions: 1
        }]
    );

    let second = only(SECOND)?;
    let listed: Vec<&str> = second
        .revisions
        .iter()
        .map(|revision| revision.revision.as_str())
        .collect();
    assert_eq!(
        listed,
        [SECOND_REVISION, FIRST],
        "listed once each, ascending"
    );
    assert_eq!(second.desired.as_deref(), Some(SECOND_REVISION));
    let selection = second
        .selected_for_new_sessions
        .as_ref()
        .ok_or("selection")?;
    assert_eq!(
        (
            selection.revision.as_str(),
            selection.rollout_generation,
            selection.qualification.as_str()
        ),
        (SECOND_REVISION, 2, SECOND_QUALIFICATION)
    );
    assert_eq!(
        second.pinned,
        vec![
            WorkflowPin {
                revision: SECOND_REVISION.into(),
                status: PinStatus::Live,
                blocked: false,
                sessions: 1
            },
            WorkflowPin {
                revision: FIRST.into(),
                status: PinStatus::Closed,
                blocked: true,
                sessions: 1
            },
        ]
    );
    let paired = second.revision(SECOND_REVISION).ok_or("second revision")?;
    assert_eq!(paired.worker.revision, SECOND_WORKER);
    assert_eq!(paired.worker.bundle, SECOND_BUNDLE);
    assert_eq!(paired.workflow.plan, SECOND_PLAN);
    assert_eq!(
        second.revision(FIRST).map(|r| r.worker.revision.as_str()),
        Some(FIRST_WORKER),
        "each workflow revision keeps the worker revision it pairs with"
    );
    Ok(())
}

#[test]
fn sessions_names_and_unprojected_identities_never_appear() -> TestResult {
    let encoded = serde_json::to_string(&map(&[SECOND]).map_err(str::to_owned)?)
        .map_err(|e| e.to_string())?;
    for withheld in [
        "session-a",
        "session-b",
        "\"session\"",
        "rust-implementer",
        "definition_name",
        "verify-change",
        "guidance",
        "context",
        "source_identity",
    ] {
        assert!(!encoded.contains(withheld), "{withheld} was copied");
    }
    Ok(())
}

#[test]
fn identical_repeats_collapse_and_definitions_stay_apart() -> TestResult {
    assert_eq!(map(&[SECOND, SECOND]).map_err(str::to_owned)?.len(), 1);
    let other = edited(
        SECOND,
        &[("/definition", Some(json!(format!("blake3:{ZERO_DIGEST}"))))],
    )?;
    assert_eq!(map(&[SECOND, &other]).map_err(str::to_owned)?.len(), 2);
    assert_eq!(
        map(&[PROMOTED, SECOND]).err(),
        Some("lattice_workflow_definition_conflict"),
        "two different lifecycles of one definition in one source"
    );
    Ok(())
}

#[test]
fn wrong_schemas_extra_fields_and_oversized_documents_are_refused() -> TestResult {
    for schema in [
        "lattice.workflow-agent-revision-slots.v2",
        "lattice.worker-inventory.v3",
        "",
    ] {
        let document = edited(SECOND, &[("/schema", Some(json!(schema)))])?;
        assert_eq!(
            refusal(&document),
            Some("lattice_workflow_slots_unsupported_schema"),
            "{schema}"
        );
    }
    for (pointer, field) in [
        ("/desired/workflow/guidance_identity", "guidance"),
        ("/desired/workflow/context_identity", "context"),
        ("/desired/workflow/source_identity", "source"),
        ("/desired/workflow/obligations", "obligations"),
        ("/desired/worker/grant", "grant"),
        ("/pinned/0/workspace", "workspace"),
        ("/selected_for_new_sessions/session", "session"),
        ("/extra", "envelope"),
    ] {
        let mut value = json_of(SECOND)?;
        let (parent, key) = pointer.rsplit_once('/').ok_or("pointer")?;
        value
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or("parent")?
            .insert(key.into(), json!(format!("b3:{ZERO_DIGEST}")));
        let document = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        assert_eq!(
            refusal(&document),
            Some("lattice_workflow_slots_malformed"),
            "an extra {field} field is refused, never copied"
        );
    }
    for (pointer, value) in [
        ("/pinned/0/status", json!("released")),
        ("/selected_for_new_sessions/kind", json!("canary")),
        ("/pinned", json!({})),
    ] {
        let document = edited(SECOND, &[(pointer, Some(value))])?;
        assert_eq!(
            refusal(&document),
            Some("lattice_workflow_slots_malformed"),
            "{pointer}"
        );
    }
    let missing = edited(SECOND, &[("/name", None)])?;
    assert_eq!(refusal(&missing), Some("lattice_workflow_slots_malformed"));
    assert_eq!(
        refusal(b"not json"),
        Some("lattice_workflow_slots_malformed")
    );
    let oversized = json_of(SECOND)?;
    let padding = " ".repeat(MAX_WORKFLOW_REVISION_SLOTS_BYTES);
    let mut bytes = serde_json::to_vec(&oversized).map_err(|e| e.to_string())?;
    bytes.extend_from_slice(padding.as_bytes());
    assert_eq!(refusal(&bytes), Some("lattice_workflow_slots_byte_bound"));
    let mut within = serde_json::to_vec(&oversized).map_err(|e| e.to_string())?;
    let room = MAX_WORKFLOW_REVISION_SLOTS_BYTES.saturating_sub(within.len());
    within.extend(std::iter::repeat_n(b' ', room));
    assert_eq!(within.len(), MAX_WORKFLOW_REVISION_SLOTS_BYTES);
    assert_eq!(
        refusal(&within),
        None,
        "a document at the bound is admitted"
    );
    Ok(())
}

#[test]
fn mismatched_worker_bindings_and_conflicting_revisions_are_refused() -> TestResult {
    let other_worker = json!(format!("blake3:{ZERO_DIGEST}"));
    for pointer in [
        "/pinned/0/bundle/worker/definition",
        "/selected_for_new_sessions/bundle/worker/definition",
    ] {
        let document = edited(SECOND, &[(pointer, Some(other_worker.clone()))])?;
        assert_eq!(
            refusal(&document),
            Some("lattice_workflow_worker_binding_mismatch"),
            "{pointer}: one workflow definition pairs with one worker definition"
        );
    }
    for (pointer, value) in [
        (
            "/selected_for_new_sessions/bundle/worker/revision",
            json!(FIRST_WORKER),
        ),
        ("/last_usable/workflow/plan_identity", json!(FIRST_PLAN)),
        (
            "/pinned/1/bundle/worker/bundle_identity",
            json!(format!("blake3:{ZERO_DIGEST}")),
        ),
    ] {
        let document = edited(SECOND, &[(pointer, Some(value))])?;
        assert_eq!(
            refusal(&document),
            Some("lattice_workflow_revision_conflict"),
            "{pointer}: one revision carries one binding"
        );
    }
    Ok(())
}

#[test]
fn malformed_identities_selections_and_pins_are_refused() -> TestResult {
    let cases: [(&str, Value, &str); 13] = [
        (
            "/definition",
            json!(FIRST),
            "lattice_workflow_definition_invalid",
        ),
        ("/name", json!(""), "lattice_workflow_name_invalid"),
        (
            "/desired/revision",
            json!("ABC"),
            "lattice_workflow_revision_invalid",
        ),
        (
            "/desired/workflow/plan_identity",
            json!(format!("blake3:{ZERO_DIGEST}")),
            "lattice_workflow_identity_invalid",
        ),
        (
            "/desired/workflow/graph_identity",
            json!(format!("b3:{ZERO_DIGEST}")),
            "lattice_workflow_identity_invalid",
        ),
        (
            "/desired/workflow/compiler_revision",
            json!("has space"),
            "lattice_workflow_identity_invalid",
        ),
        (
            "/desired/worker/revision",
            json!(format!("blake3:{ZERO_DIGEST}")),
            "lattice_workflow_worker_invalid",
        ),
        (
            "/desired/worker/bundle_identity",
            json!(ZERO_DIGEST),
            "lattice_workflow_worker_invalid",
        ),
        (
            "/selected_for_new_sessions/rollout_generation",
            json!(0),
            "lattice_workflow_selection_invalid",
        ),
        (
            "/selected_for_new_sessions/qualification_ref",
            json!(ZERO_DIGEST),
            "lattice_workflow_selection_invalid",
        ),
        (
            "/pinned/0/is_blocked",
            json!(false),
            "lattice_workflow_pin_inconsistent",
        ),
        (
            "/pinned/1/session",
            json!("session-0"),
            "lattice_workflow_pin_invalid",
        ),
        (
            "/pinned/0/session",
            json!(""),
            "lattice_workflow_pin_invalid",
        ),
    ];
    for (pointer, value, code) in cases {
        let document = edited(SECOND, &[(pointer, Some(value))])?;
        assert_eq!(refusal(&document), Some(code), "{pointer}");
    }
    Ok(())
}

/// The second capture's live pin entry, pinned by `session` to `revision`.
/// Another revision gets its own plan identity, so its binding is distinct.
fn pin_on(revision: &str, session: &str) -> TestResult<Value> {
    let mut pin = json_of(SECOND)?
        .pointer("/pinned/1")
        .cloned()
        .ok_or("pin template")?;
    *pin.pointer_mut("/session").ok_or("session")? = json!(session);
    if revision != SECOND_REVISION {
        *pin.pointer_mut("/bundle/revision").ok_or("revision")? = json!(revision);
        *pin.pointer_mut("/bundle/workflow/plan_identity")
            .ok_or("plan")? = json!(format!("b3:{revision}"));
    }
    Ok(pin)
}

fn with_pins(pins: Vec<Value>) -> TestResult<Vec<u8>> {
    edited(SECOND, &[("/pinned", Some(Value::Array(pins)))])
}

#[test]
fn a_document_past_the_revision_bound_or_with_an_inexact_generation_is_refused() -> TestResult {
    let distinct = (0..=kiln_core::inventory::MAX_WORKFLOW_REVISIONS)
        .map(|index| pin_on(&format!("{index:064x}"), &format!("session-{index:04}")))
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        refusal(&with_pins(distinct)?),
        Some("lattice_workflow_revision_bound"),
        "more distinct revisions than core admits"
    );
    let within = (0..kiln_core::inventory::MAX_WORKFLOW_REVISIONS - 1)
        .map(|index| pin_on(&format!("{index:064x}"), &format!("session-{index:04}")))
        .collect::<TestResult<Vec<_>>>()?;
    let fact = only(&with_pins(within)?)?;
    assert_eq!(
        fact.revisions.len(),
        kiln_core::inventory::MAX_WORKFLOW_REVISIONS,
        "the selected revision and 63 pinned ones fill the bound"
    );
    let inexact = edited(
        SECOND,
        &[(
            "/selected_for_new_sessions/rollout_generation",
            Some(json!(kiln_core::inventory::MAX_EXACT_NUMBER + 1)),
        )],
    )?;
    assert_eq!(
        refusal(&inexact),
        Some("lattice_workflow_selection_invalid")
    );
    Ok(())
}

#[test]
fn three_hundred_pins_are_admitted_within_the_slots_bound() -> TestResult {
    let pins = (0..300)
        .map(|index| pin_on(SECOND_REVISION, &format!("session-{index:04}")))
        .collect::<TestResult<Vec<_>>>()?;
    let document: Value = serde_json::from_slice(&with_pins(pins)?).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    assert!(
        bytes.len() > 262_144,
        "the general producer-document bound would refuse it"
    );
    assert!(bytes.len() <= MAX_WORKFLOW_REVISION_SLOTS_BYTES);
    let fact = only(&bytes)?;
    assert_eq!(
        fact.pinned,
        vec![WorkflowPin {
            revision: SECOND_REVISION.into(),
            status: PinStatus::Live,
            blocked: false,
            sessions: 300
        }]
    );
    Ok(())
}

#[test]
fn documents_differing_only_in_sessions_or_names_are_refused_as_a_conflict() -> TestResult {
    for (pointer, value) in [
        ("/pinned/1/session", json!("session-c")),
        ("/name", json!("verify-change-2")),
    ] {
        let renamed = edited(SECOND, &[(pointer, Some(value))])?;
        assert!(only(&renamed).is_ok(), "{pointer}: valid on its own");
        assert_eq!(
            map(&[SECOND, &renamed]).err(),
            Some("lattice_workflow_definition_conflict"),
            "{pointer}"
        );
    }
    Ok(())
}

#[test]
fn streaming_accumulator_collapses_4096_documents_and_enforces_aggregate_bound() -> TestResult {
    use super::WorkflowRevisionSlotsAccumulator;

    let mut acc = WorkflowRevisionSlotsAccumulator::new(source(), joins())?;
    for _ in 0..kiln_core::inventory::MAX_INVENTORY_ITEMS {
        acc.push_document(SECOND).map_err(str::to_owned)?;
    }
    assert_eq!(
        acc.push_document(SECOND),
        Err("lattice_workflow_slots_document_bound")
    );
    let fragment = acc.finish();
    assert_eq!(fragment.workflow_revisions.len(), 1);

    let mut bounded = WorkflowRevisionSlotsAccumulator::new(source(), joins())?;
    let limit = u64::try_from(SECOND.len()).map_err(|e| e.to_string())?;
    bounded
        .push_document_within(SECOND, limit)
        .map_err(str::to_owned)?;
    assert_eq!(
        bounded.push_document_within(SECOND, limit),
        Err("lattice_workflow_slots_aggregate_byte_bound")
    );
    Ok(())
}
