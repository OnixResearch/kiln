//! Kiln-oriented consumer contract controls (synthetic, not native execution receipts).
//! The adapter admits only a producer's complete portable wire plus a separately
//! retained expected cohort; transport success never confers execution authority.
use std::collections::BTreeMap;

use cairn_core::verified::generated_agent_workflow::{
    admit_generated_agent_workflow_policy, GeneratedAgentWorkflowPolicy,
};
use cairn_core::verified::selected_work::{
    admit_portable, compose, ObligationObservationInput, ObligationSource, SelectedObligations,
    SelectedWorkError, SelectedWorkInput, SelectedWorkPolicy, SelectedWorkSnapshot,
};
use cairn_core::{
    admit_workflow_obligation_policy, AdmittedWorkflowObligationCatalog, EvidenceObservation,
    ObligationSelectionState, ObservationState, WorkflowObligationPolicy,
};
use serde_json::{json, Value};

fn hash(n: u8) -> String {
    format!("{n:064x}")
}
fn synthetic_input() -> SelectedWorkInput {
    let artifacts = json!([{"root":"lifecycle", "path":".cairn/changes/sample/tasks.md",
        "bytes":21, "blake3":hash(2), "status":"present"}]);
    let revision = cairn_core::canonical_json_hash(&json!({"artifacts":artifacts}));
    let stages = [
        "artifact",
        "proposal",
        "design",
        "tasks",
        "completion",
        "sync",
        "archive",
    ]
    .map(|stage| {
        json!({"stage":stage, "state":"unavailable",
            "decision_hash":hash(10), "decision":{}})
    });
    serde_json::from_value(json!({
        "producer": "cairn", "selection": {"change":"sample", "task":null},
        "source": {"revision":{"kind":"projection", "identity":revision},
            "artifacts":artifacts},
        "policy_hash":hash(3), "profile":"spec-driven", "profile_hash":hash(4),
        "dependencies":[], "completion":{"state":"unavailable", "contract_hash":null,
            "results_hash":null, "subject_hash":null},
        "context":{"selection_hash":hash(5), "local_hash":hash(6), "portable_hash":hash(7),
            "entries":[]},
        "readiness":{"schema":"cairn.readiness.v1", "scope":{"change":"sample",
            "policy_hash":hash(3), "source_hash":hash(8), "evidence_hash":hash(9)},
            "valid":true, "classification":"blocked", "stages":stages,
            "blockers":[], "actions":[], "diagnostics":[], "readiness_policy_hash":hash(11),
            "input_hash":hash(12), "receipt_hash":hash(13), "mutation_authorized":false,
            "non_claims":[]},
        "units":[], "remaining":[{"kind":"completion", "state":"unavailable", "subject":null},
            {"kind":"sync", "state":"blocked", "subject":null},
            {"kind":"archive", "state":"blocked", "subject":null}],
        "enumeration":{"complete":true, "observed":1, "total":1,
            "budget_bytes":67108864, "consumed_bytes":21}, "local_root":"/private/cairn"
    }))
    .unwrap()
}
fn synthetic_producer_wire() -> (Value, Value) {
    let policy = SelectedWorkPolicy::bundled().unwrap();
    let wire = serde_json::to_value(
        compose(&policy, &synthetic_input(), None)
            .unwrap()
            .portable(),
    )
    .unwrap();
    let expected = wire["cohort"].clone();
    (wire, expected)
}
fn consumer_admit(wire: Value, expected: Value) -> bool {
    let Ok(snapshot) = serde_json::from_value::<SelectedWorkSnapshot>(wire) else {
        return false;
    };
    let Ok(expected) = serde_json::from_value(expected) else {
        return false;
    };
    admit_portable(
        &SelectedWorkPolicy::bundled().unwrap(),
        &snapshot,
        &expected,
    )
    .is_ok()
}
#[test]
fn kiln_portable_consumer_requires_complete_exact_cohort_and_no_raw_detail() {
    let (wire, expected) = synthetic_producer_wire();
    assert!(consumer_admit(wire.clone(), expected.clone()));
    let mut stale = wire.clone();
    stale["source"]["artifacts"][0]["blake3"] = json!(hash(14));
    assert!(!consumer_admit(stale, expected.clone()));
    let mut crossed = wire.clone();
    crossed["completion"]["subject_hash"] = json!(hash(15));
    assert!(!consumer_admit(crossed, expected.clone()));
    let mut partial = wire.clone();
    partial["enumeration"]["complete"] = json!(false);
    assert!(!consumer_admit(partial, expected.clone()));
    let mut obligations = wire.clone();
    obligations["remaining"] = json!([]);
    assert!(!consumer_admit(obligations, expected.clone()));
    let mut stages = wire.clone();
    stages["readiness"]["stages"] = json!([]);
    assert!(!consumer_admit(stages, expected.clone()));
    let mut missing = wire.clone();
    missing["context"]["entries"] = json!([{
        "id":"missing", "authority":"project_fact", "source_root":"project",
        "path":"/private/context.md", "status":"unavailable", "content_hash":null }]);
    assert!(!consumer_admit(missing, expected.clone()));
    let mut raw = wire.clone();
    raw["context"]["entries"] = json!([{"text":"local only"}]);
    assert!(!consumer_admit(raw, expected.clone()));
    let mut malformed = wire.clone();
    malformed["schema"] = json!("cairn.selected-work.v2");
    assert!(!consumer_admit(malformed, expected.clone()));
    let mut extra = wire;
    extra["forged_authority"] = json!(true);
    assert!(!consumer_admit(extra, expected));
}

const SHIPPED_DEFAULT: &str = include_str!("../../../cairn-policy/generated/cairn-policy.json");
const SHIPPED_OBLIGATIONS: &str =
    include_str!("../../../cairn-policy/generated/workflow-obligations.json");

fn obligation_catalog(policy_hash: &str) -> AdmittedWorkflowObligationCatalog {
    let value: Value = serde_json::from_str(SHIPPED_DEFAULT).unwrap();
    let generated: GeneratedAgentWorkflowPolicy =
        serde_json::from_value(value["generated_agent_workflow_policy"].clone()).unwrap();
    let workflows = admit_generated_agent_workflow_policy(&generated).unwrap();
    let policy: WorkflowObligationPolicy = serde_json::from_str(SHIPPED_OBLIGATIONS).unwrap();
    admit_workflow_obligation_policy(&policy, &workflows, policy_hash).unwrap()
}
fn produce(
    catalog: &AdmittedWorkflowObligationCatalog,
    input: &SelectedWorkInput,
    observations: Option<&ObligationObservationInput>,
) -> Result<SelectedWorkSnapshot, SelectedWorkError> {
    let source = ObligationSource {
        catalog,
        definition: "verify",
        observations,
    };
    Ok(compose(
        &SelectedWorkPolicy::bundled().unwrap(),
        input,
        Some(&source),
    )?
    .into_portable())
}
fn export(
    snapshot: &SelectedWorkSnapshot,
) -> &cairn_core::verified::selected_work::SelectedObligationExport {
    match &snapshot.obligations {
        SelectedObligations::Exported(export) => export,
        SelectedObligations::NotSelected => panic!("obligations are exported"),
    }
}
fn evidence(
    catalog: &AdmittedWorkflowObligationCatalog,
    snapshot: &SelectedWorkSnapshot,
    predicate: &str,
    state: ObservationState,
) -> EvidenceObservation {
    let admitted = &catalog.definitions[0];
    let known = &admitted.evidence_predicates[predicate];
    EvidenceObservation {
        predicate: predicate.into(),
        requirement_id: known.requirement_id.clone(),
        revision_blake3: known.accepted_revision_blake3.clone(),
        subject_blake3: snapshot.cohort.verification_subject.clone(),
        context_blake3: snapshot.cohort.context.clone(),
        policy_blake3: snapshot.policy_hash.clone(),
        observation_id: format!("check:{predicate}"),
        state,
    }
}
fn admit(
    snapshot: &SelectedWorkSnapshot,
    expected: &SelectedWorkSnapshot,
) -> Result<(), SelectedWorkError> {
    admit_portable(
        &SelectedWorkPolicy::bundled().unwrap(),
        snapshot,
        &expected.cohort,
    )
}

// r[verify cairn.selected_work.obligations]
#[test]
fn kiln_obligation_fixture_keeps_identities_and_refuses_partial_alternatives() {
    let catalog = obligation_catalog(&hash(3));
    assert_eq!(catalog.definitions[0].definition.id, "verify");
    let input = synthetic_input();

    // Planning: the definition is selected, nothing is observed yet.
    let planning = produce(&catalog, &input, None).unwrap();
    admit(&planning, &planning).unwrap();
    let plan = export(&planning);
    assert!(!plan.observations_supplied);
    assert_eq!(plan.evaluation.outcome, "unknown");
    assert!(!plan.evaluation.successful_delivery);
    assert!(plan
        .evaluation
        .remaining
        .iter()
        .any(|remaining| remaining.id == "verify.evidence.contract"));
    assert!(plan
        .nodes
        .iter()
        .filter(|node| node.alternative.is_some())
        .all(|node| node.selection == ObligationSelectionState::SelectionMissing));
    assert_eq!(
        plan.predicate.subject_blake3,
        planning.cohort.verification_subject
    );
    assert_eq!(plan.predicate.context_blake3, planning.cohort.context);

    // An incomplete selected alternative plus a complete unselected one never combine.
    let selected = BTreeMap::from([(
        "verify.traceability".to_string(),
        "receipt-chain".to_string(),
    )]);
    let mut partial_input = ObligationObservationInput {
        selected_alternatives: selected,
        ..ObligationObservationInput::default()
    };
    for predicate in [
        "agent-receipt.verification-contract",
        "agent-receipt.pass-only",
        "agent-receipt.completion-authority",
        "verification-obligation.claim-revision",
    ] {
        partial_input.evidence.push(evidence(
            &catalog,
            &planning,
            predicate,
            ObservationState::Satisfied,
        ));
    }
    let partial = produce(&catalog, &input, Some(&partial_input)).unwrap();
    admit(&partial, &partial).unwrap();
    let partial_export = export(&partial);
    assert_ne!(partial_export.evaluation.outcome, "proved");
    assert!(!partial_export.evaluation.successful_delivery);
    assert!(partial_export
        .evaluation
        .remaining
        .iter()
        .any(|remaining| remaining.id == "verify.traceability"));

    // The complete selected branch proves; obligation and guidance identities stay fixed.
    let mut complete_input = partial_input.clone();
    complete_input.evidence.push(evidence(
        &catalog,
        &planning,
        "agent-receipt.freshness",
        ObservationState::Satisfied,
    ));
    complete_input.evidence.push(evidence(
        &catalog,
        &planning,
        "verification-scope.applicable",
        ObservationState::Unsatisfied,
    ));
    let complete = produce(&catalog, &input, Some(&complete_input)).unwrap();
    admit(&complete, &complete).unwrap();
    let complete_export = export(&complete);
    assert_eq!(complete_export.evaluation.outcome, "proved");
    assert!(complete_export.evaluation.successful_delivery);
    assert_eq!(complete_export.nodes, partial_export.nodes);
    assert_eq!(complete_export.predicate, partial_export.predicate);
    assert_eq!(complete_export.guidance, partial_export.guidance);
    assert_ne!(complete.cohort.obligations, partial.cohort.obligations);
    assert_eq!(
        admit(&complete, &partial),
        Err(SelectedWorkError::StaleCohort)
    );

    // Evidence bound to another verification subject stays unresolved.
    let mut crossed_input = complete_input.clone();
    for observation in &mut crossed_input.evidence {
        observation.subject_blake3 = hash(40);
    }
    let crossed = produce(&catalog, &input, Some(&crossed_input)).unwrap();
    assert_ne!(export(&crossed).evaluation.outcome, "proved");

    // A catalog admitted for another policy cannot bind this snapshot; a
    // policy revision rebinds every obligation identity.
    let revised_catalog = obligation_catalog(&hash(4));
    assert_eq!(
        produce(&revised_catalog, &input, None).unwrap_err(),
        SelectedWorkError::CrossedSubject
    );
    let mut revised_input = input.clone();
    revised_input.policy_hash = hash(4);
    revised_input.readiness.scope.policy_hash = hash(4);
    let revised = produce(&revised_catalog, &revised_input, None).unwrap();
    admit(&revised, &revised).unwrap();
    assert_ne!(
        export(&revised).binding.binding_blake3,
        plan.binding.binding_blake3
    );
    for (left, right) in export(&revised).nodes.iter().zip(&plan.nodes) {
        assert_eq!(left.id, right.id);
        assert_ne!(left.obligation_blake3, right.obligation_blake3);
    }
}

// r[verify cairn.selected_work.obligations]
#[test]
fn kiln_obligation_consumer_refuses_forged_crossed_and_empty_sections() {
    let catalog = obligation_catalog(&hash(3));
    let input = synthetic_input();
    let planning = produce(&catalog, &input, None).unwrap();

    let mut forged = planning.clone();
    if let SelectedObligations::Exported(export) = &mut forged.obligations {
        export.evaluation.outcome = "proved".into();
        export.evaluation.proved = true;
        export.evaluation.successful_delivery = true;
    }
    assert_eq!(
        admit(&forged, &planning),
        Err(SelectedWorkError::InvalidObligations)
    );

    let mut emptied = planning.clone();
    if let SelectedObligations::Exported(export) = &mut emptied.obligations {
        export.evaluation.remaining.clear();
    }
    assert_eq!(
        admit(&emptied, &planning),
        Err(SelectedWorkError::InvalidObligations)
    );

    let mut stripped = planning.clone();
    if let SelectedObligations::Exported(export) = &mut stripped.obligations {
        export.nodes.truncate(1);
    }
    assert_eq!(
        admit(&stripped, &planning),
        Err(SelectedWorkError::InvalidObligations)
    );

    let mut unselected = planning.clone();
    unselected.obligations = SelectedObligations::NotSelected;
    assert_eq!(
        admit(&unselected, &planning),
        Err(SelectedWorkError::UnsupportedSchema)
    );

    // Graft this section onto a snapshot with another verification subject.
    let mut other_input = input.clone();
    other_input.completion.subject_hash = Some(hash(41));
    let mut grafted = produce(&catalog, &other_input, None).unwrap();
    grafted.obligations = planning.obligations.clone();
    assert_eq!(
        admit(&grafted, &grafted),
        Err(SelectedWorkError::CrossedSubject)
    );

    // A wire value with an unknown section state or field is refused at decode.
    let mut wire = serde_json::to_value(&planning).unwrap();
    wire["obligations"]["state"] = json!("satisfied");
    assert!(serde_json::from_value::<SelectedWorkSnapshot>(wire).is_err());
    let mut wire = serde_json::to_value(&planning).unwrap();
    wire["obligations"]["export"]["authority"] = json!("execute");
    assert!(serde_json::from_value::<SelectedWorkSnapshot>(wire).is_err());

    // Without a selected definition the section is explicit, never satisfied.
    let (wire, _) = synthetic_producer_wire();
    assert_eq!(wire["obligations"], json!({"state": "not_selected"}));
    assert_eq!(
        wire["semantic_extensions"]["obligation_explanations"],
        "not_selected"
    );
}
