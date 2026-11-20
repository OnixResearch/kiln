//! Lattice `lattice.worker-inventory.v2` documents captured from `lattice
//! implementation-attempt inspect` at Lattice 9a74e7ef (deterministic fixture
//! ports only), copied verbatim; `provenance.json` records their hashes.
// r[verify kiln.inventory.producers.scenario.lattice]
// r[verify kiln.inventory.producers.scenario.capability]
// r[verify kiln.inventory.producers.scenario.wait]
// r[verify kiln.inventory.producers.scenario.refused]
use kiln_core::inventory::{
    AgentDefinition, FactRef, InventoryFragment, InventoryJoins, InventoryKind, Permission,
    SourceObservation, WaitReason, WaitSubject, WorkerInstance, WorkerLifecycle,
};
use serde_json::{Value, json};

use super::map_worker_inventory;

type TestResult<T = ()> = Result<T, String>;

const ADMITTED: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/agent-a-1-admitted-not-dispatched.json"
);
const DECLARED: &[u8] = include_bytes!(
    "../../../fixtures/inventory/producers/lattice/agent-a-1-declared-capability-dispatched.json"
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
const PROVENANCE: &str =
    include_str!("../../../fixtures/inventory/producers/lattice/provenance.json");
/// The captured instances were observed before this collection time.
const COLLECTED_AT_MS: u64 = 1_795_208_980_000;
const VALID_UNTIL_MS: u64 = 1_795_209_040_000;
const DEFINITION: &str = "blake3:c2edbc9f0aa03386b1e8d4eb900e2b10db1083ed568ed934c53e923a2e23a8b6";
const DECLARED_DEFINITION: &str =
    "blake3:367783379a41fd2e834f26af4e7504906c1f3967bd9f21bba979441a1f0789c1";
const PROFILE: &str = "8a836baf7f4924334a2d50ced5590fd4f5a70e91bc67ee04c667c00722b17b72";
const HOST_LOCAL_PROFILE: &str = "2fb8023506390884d4d5974a91d807e8a8ff5af11533ecc67b34f39f79e4fa41";
const DECLARED_PROFILE: &str = "88cbb6128daf0fc09cb3b01ceeaa35109493d28c934ec5adc2e392557578a2f7";

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

fn only_worker(fragment: &InventoryFragment) -> TestResult<&WorkerInstance> {
    match fragment.workers.as_slice() {
        [worker] => Ok(worker),
        other => Err(format!("expected one worker, got {}", other.len())),
    }
}

fn unknown(fields: &[String], name: &str) -> bool {
    fields.iter().any(|f| f == name)
}

#[test]
fn captures_match_their_provenance() -> TestResult {
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|e| e.to_string())?;
    assert_eq!(
        provenance.get("lattice_commit").and_then(Value::as_str),
        Some("9a74e7ef1da4e0ca7610dce0db039b3825a7154d")
    );
    assert_eq!(
        provenance.get("contract").and_then(Value::as_str),
        Some(super::WORKER_INVENTORY_SCHEMA)
    );
    let hashes = provenance
        .get("blake3")
        .and_then(Value::as_object)
        .ok_or("missing hashes")?;
    let captures: [(&str, &[u8]); 8] = [
        ("agent-a-1-admitted-not-dispatched.json", ADMITTED),
        ("agent-a-1-declared-capability-dispatched.json", DECLARED),
        ("agent-a-1-dispatched-outcome-unknown.json", FIRST),
        ("agent-a-1-terminal-failed.json", TERMINAL),
        ("agent-a-2-dispatched-outcome-unknown.json", SECOND),
        ("host-local-served-ready.json", SERVED),
        ("host-local-served-unavailable.json", SERVED_UNAVAILABLE),
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
    let revisions = &undeclared.revisions;
    for slot in [
        &revisions.desired,
        &revisions.prepared,
        &revisions.qualified,
        &revisions.selected_for_new_sessions,
    ] {
        assert_eq!(slot, &None, "a #25 slot is never the current revision");
    }
    assert_eq!(
        undeclared.meta.unknown_fields,
        vec![
            "permitted_models",
            "qualifications",
            "revisions",
            "task_families",
            "tools"
        ]
    );
    assert_eq!(
        undeclared.wait_reasons,
        vec![
            reason("capability_undeclared", "task_families", "configuration"),
            reason("capability_undeclared", "tools", "configuration"),
            reason("qualification_unbound", "qualification_ref", "animus#18"),
            reason("revision_selection_unpublished", "revisions", "lattice#25"),
        ],
        "serving reasons of one inspection never describe the definition"
    );
    let declared = definition(&fragment, DECLARED_DEFINITION)?;
    assert_eq!(declared.profile_revision, DECLARED_PROFILE);
    assert_eq!(
        declared.task_families,
        vec!["cairn-task".to_owned(), "rust-implementation".to_owned()]
    );
    assert_eq!(declared.tools, vec!["edit".to_owned(), "read".to_owned()]);
    assert_eq!(
        declared.meta.unknown_fields,
        vec!["permitted_models", "qualifications", "revisions"]
    );
    assert_eq!(
        declared.wait_reasons,
        vec![
            reason("qualification_unbound", "qualification_ref", "animus#18"),
            reason("revision_selection_unpublished", "revisions", "lattice#25"),
        ]
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
    let qualified = edit(
        DECLARED,
        "/profile/task_families/support",
        "qualified".into(),
    )?;
    let fragment = agents(&[&qualified])?;
    let declared = definition(&fragment, DECLARED_DEFINITION)?;
    assert_eq!(
        declared.task_families,
        vec!["cairn-task".to_owned(), "rust-implementation".to_owned()]
    );
    assert!(!unknown(&declared.meta.unknown_fields, "task_families"));
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
    let rejected = edits(
        ADMITTED,
        &[
            (
                "/instance/lifecycle",
                json!({ "state": "binding_rejected_before_dispatch" }),
            ),
            ("/instance/wait_reasons", json!([])),
        ],
    )?;
    assert_eq!(
        only_worker(&workers(&[&rejected])?)?.lifecycle,
        WorkerLifecycle::Stopped
    );
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

fn refuses(documents: &[&[u8]], expected: &str) {
    assert_eq!(agents(documents).err(), Some(expected), "agents");
    assert_eq!(workers(documents).err(), Some(expected), "workers");
}

#[test]
fn v1_and_unsupported_schemas_refuse_as_unsupported() -> TestResult {
    let mut v1 = edits(
        FIRST,
        &[
            ("/schema", "lattice.worker-inventory.v1".into()),
            ("/profile/schema", "lattice.worker-profile-facts.v1".into()),
            (
                "/instance/schema",
                "lattice.worker-instance-observation.v1".into(),
            ),
        ],
    )
    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string()))?;
    let profile_fields: &[&str] = &[
        "definition",
        "task_families",
        "tools",
        "revisions",
        "wait_reasons",
    ];
    let instance_fields: &[&str] = &["definition_identity", "liveness", "wait_reasons"];
    for (section, fields) in [("/profile", profile_fields), ("/instance", instance_fields)] {
        let object = v1
            .pointer_mut(section)
            .and_then(Value::as_object_mut)
            .ok_or("not an object")?;
        for field in fields {
            object.remove(*field);
        }
    }
    let v1 = serde_json::to_vec(&v1).map_err(|e| e.to_string())?;
    refuses(&[&v1], "lattice_inventory_unsupported_schema");
    for (pointer, schema) in [
        ("/schema", "lattice.worker-inventory.v1"),
        ("/profile/schema", "lattice.worker-profile-facts.v1"),
        ("/instance/schema", "lattice.worker-instance-observation.v1"),
        ("/schema", "lattice.worker-inventory.v3"),
    ] {
        let document = edit(FIRST, pointer, schema.into())?;
        refuses(&[&document], "lattice_inventory_unsupported_schema");
    }
    let wrong_source = edit(FIRST, "/instance/source", "other-state".into())?;
    refuses(&[&wrong_source], "lattice_inventory_unsupported_schema");
    Ok(())
}

#[test]
fn malformed_v2_documents_refuse() -> TestResult {
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
            "/profile/tools/value",
            json!(["edit"]),
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
            "/profile/revisions/pinned/support",
            json!("declared"),
            "lattice_revision_unsupported",
        ),
        (
            "/profile/revisions/desired/owner",
            json!("kiln#49"),
            "lattice_revision_unsupported",
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
    let reasons: Value = serde_json::from_slice::<Value>(TERMINAL)
        .map_err(|e| e.to_string())?
        .pointer("/profile/wait_reasons")
        .cloned()
        .ok_or("missing reasons")?;
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
fn ambiguous_conflicting_and_out_of_window_inputs_refuse() -> TestResult {
    assert_eq!(
        agents(&[FIRST, SERVED]).err(),
        Some("lattice_definition_revision_ambiguous"),
        "the revision selected for new sessions is unpublished (Lattice #25)"
    );
    assert_eq!(
        workers(&[FIRST, SERVED])?.workers.len(),
        2,
        "workers pinned to two revisions of one definition stay visible"
    );
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
