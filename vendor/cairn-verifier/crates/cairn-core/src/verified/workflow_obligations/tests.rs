use super::*;
use crate::verified::generated_agent_workflow::{
    admit_generated_agent_workflow_policy, GeneratedAgentWorkflowPolicy,
    RenderedGeneratedWorkflowFile,
};
use crate::verified::policy_exception::{
    ExceptionRequest, PolicyExceptionState, PolicyScope, ValidityObservation,
};

const SHIPPED_POLICY: &str =
    include_str!("../../../../../cairn-policy/generated/workflow-obligations.json");
const SHIPPED_DEFAULT: &str =
    include_str!("../../../../../cairn-policy/generated/cairn-policy.json");
const POLICY_HASH: &str = "abababababababababababababababababababababababababababababababab";
const SUBJECT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CONTEXT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const EXCEPTION_POLICY: &str = "9f463020b564b252f6322a6f4ecfa8094a28a78ff5bc32e6b9ad1b2e01541542";
const SIGNAL: &str = "signal:selected-change";

fn shipped_policy() -> WorkflowObligationPolicy {
    serde_json::from_str(SHIPPED_POLICY).expect("shipped obligation policy parses")
}

fn shipped_catalog() -> AdmittedGeneratedWorkflowCatalog {
    let value: serde_json::Value =
        serde_json::from_str(SHIPPED_DEFAULT).expect("shipped default policy parses");
    let policy: GeneratedAgentWorkflowPolicy =
        serde_json::from_value(value["generated_agent_workflow_policy"].clone())
            .expect("generated workflow policy parses");
    admit_generated_agent_workflow_policy(&policy).expect("shipped catalog admits")
}

fn admitted_catalog() -> AdmittedWorkflowObligationCatalog {
    admit_workflow_obligation_policy(&shipped_policy(), &shipped_catalog(), POLICY_HASH)
        .expect("shipped obligation policy admits against the shipped catalog")
}

fn definition<'a>(
    catalog: &'a AdmittedWorkflowObligationCatalog,
    id: &str,
) -> &'a AdmittedWorkflowObligation {
    catalog
        .definitions
        .iter()
        .find(|admitted| admitted.definition.id == id)
        .expect("definition is admitted")
}

fn observation(
    admitted: &AdmittedWorkflowObligation,
    predicate: &str,
    state: ObservationState,
) -> EvidenceObservation {
    let known = admitted
        .evidence_predicates
        .get(predicate)
        .expect("predicate is admitted");
    EvidenceObservation {
        predicate: predicate.to_string(),
        requirement_id: known.requirement_id.clone(),
        revision_blake3: known.accepted_revision_blake3.clone(),
        subject_blake3: SUBJECT.to_string(),
        context_blake3: CONTEXT.to_string(),
        policy_blake3: POLICY_HASH.to_string(),
        observation_id: format!("observation:{predicate}"),
        state,
    }
}

fn selection(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(node, alternative)| ((*node).to_string(), (*alternative).to_string()))
        .collect()
}

fn request(
    admitted: &AdmittedWorkflowObligation,
    selected: BTreeMap<String, String>,
) -> ObligationRequest {
    ObligationRequest {
        binding: admitted.binding.clone(),
        subject_blake3: SUBJECT.to_string(),
        context_blake3: CONTEXT.to_string(),
        selected_alternatives: selected,
        evidence: Vec::new(),
        progress: Vec::new(),
        agent_claims: None,
        exceptions: Vec::new(),
        temporal: None,
        terminal: None,
    }
}

fn base_evidence(admitted: &AdmittedWorkflowObligation) -> Vec<EvidenceObservation> {
    vec![
        observation(
            admitted,
            "agent-receipt.verification-contract",
            ObservationState::Satisfied,
        ),
        observation(
            admitted,
            "agent-receipt.pass-only",
            ObservationState::Satisfied,
        ),
        observation(
            admitted,
            "agent-receipt.freshness",
            ObservationState::Satisfied,
        ),
        observation(
            admitted,
            "agent-receipt.completion-authority",
            ObservationState::Satisfied,
        ),
        observation(
            admitted,
            "verification-scope.applicable",
            ObservationState::Unsatisfied,
        ),
    ]
}

fn bounded_evidence(admitted: &AdmittedWorkflowObligation) -> Vec<EvidenceObservation> {
    let mut evidence = base_evidence(admitted);
    evidence.push(observation(
        admitted,
        "verification-obligation.claim-revision",
        ObservationState::Satisfied,
    ));
    evidence
}

fn exception_state(
    request_revision: u64,
    lifecycle: &str,
    controls: Vec<String>,
) -> PolicyExceptionState {
    let mut requesters = std::collections::BTreeSet::new();
    requesters.insert("requester".to_string());
    let mut reviewers = std::collections::BTreeSet::new();
    reviewers.insert("reviewer".to_string());
    let mut approvers = std::collections::BTreeSet::new();
    approvers.insert("approver".to_string());
    let mut revokers = std::collections::BTreeSet::new();
    revokers.insert("revoker".to_string());
    let mut observers = std::collections::BTreeSet::new();
    observers.insert("observer-a".to_string());
    let mut observations = std::collections::BTreeMap::new();
    observations.insert(
        "observer-a".to_string(),
        ValidityObservation {
            id: "validity:1".to_string(),
            observed_epoch: 10,
            finite_valid_until: 100,
            source: "observer-a".to_string(),
        },
    );
    PolicyExceptionState {
        schema: crate::verified::policy_exception::POLICY_EXCEPTION_SCHEMA_V1.to_string(),
        kind: "policy-exception".to_string(),
        exception_id: "policy-exception:verify-freshness".to_string(),
        workspace_id: "workspace".to_string(),
        request: ExceptionRequest {
            policy_identity: EXCEPTION_POLICY.to_string(),
            scope: PolicyScope {
                rule_ids: vec!["allow-fs-write".to_string()],
                targets: vec!["crates/cairn-core".to_string()],
            },
            rationale: "retain the original unsatisfied condition".to_string(),
            compensating_controls: controls,
            request_root: "request-root".to_string(),
        },
        request_revision,
        requesters,
        reviewers,
        approvers,
        revokers,
        observers,
        findings: std::collections::BTreeMap::new(),
        approvals: Vec::new(),
        observations,
        lifecycle: lifecycle.to_string(),
        predecessor: None,
        operations: std::collections::BTreeSet::from(["init".to_string()]),
    }
}

fn exception_snapshot(
    admitted: &AdmittedWorkflowObligation,
    request_revision: u64,
    lifecycle: &str,
    validity_epoch: u64,
    controls: Vec<String>,
    control_state: Option<ObservationState>,
) -> ExceptionSnapshot {
    ExceptionSnapshot {
        path: "verify.exception.freshness".to_string(),
        state: exception_state(request_revision, lifecycle, controls),
        validity_epoch,
        control_evidence: control_state
            .map(|state| {
                vec![observation(
                    admitted,
                    "exception.sandbox-revalidation",
                    state,
                )]
            })
            .unwrap_or_default(),
    }
}

fn trace(
    completeness: TraceCompleteness,
    first: Option<u64>,
    second: Option<u64>,
) -> TemporalObservation {
    let mut events = Vec::new();
    if let Some(epoch) = first {
        events.push(ObservedEvent {
            event: "receipt-recorded".to_string(),
            epoch,
            observer: "cairn-lifecycle".to_string(),
        });
    }
    if let Some(epoch) = second {
        events.push(ObservedEvent {
            event: "candidate-frozen".to_string(),
            epoch,
            observer: "cairn-lifecycle".to_string(),
        });
    }
    TemporalObservation {
        trace_id: "verify:selected-change".to_string(),
        observer: "cairn-lifecycle".to_string(),
        clock: "supplied-u64-epoch.v1".to_string(),
        window_start_epoch: 0,
        horizon_epoch: 86400,
        completeness,
        events,
        observation_id: SIGNAL.to_string(),
    }
}

fn rendered_file<'a>(
    files: &'a [RenderedGeneratedWorkflowFile],
    workflow_id: &str,
) -> &'a RenderedGeneratedWorkflowFile {
    files
        .iter()
        .find(|file| file.workflow_id == workflow_id)
        .expect("workflow is rendered")
}

// r[verify cairn.workflow_obligations.policy]
#[test]
fn shipped_policy_admits_and_binds_exact_catalog_and_policy_identity() {
    let catalog = admitted_catalog();
    assert_eq!(catalog.workflow_policy_blake3, POLICY_HASH);
    assert_eq!(catalog.definitions.len(), 2);
    let verify = definition(&catalog, "verify");
    assert_eq!(
        verify.binding.catalog_identity,
        shipped_catalog().catalog_identity
    );
    assert_eq!(
        verify.command_classes,
        [
            "gate",
            "readiness",
            "receipt-inspect",
            "traceability",
            "workflow-inspect"
        ]
    );
    assert!(definition(&catalog, "verify.bounded-exception")
        .definition
        .temporal
        .is_some());
}

#[test]
fn admission_rejects_unknown_command_class_node_and_predicate() {
    let catalog = shipped_catalog();
    let mut policy = shipped_policy();
    policy.definitions[0].action_preconditions[0].action = "release".to_string();
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());

    let mut policy = shipped_policy();
    if let ObligationExpr::AllOf { obligations, .. } = &mut policy.definitions[0].completion {
        obligations[1] = obligations[0].clone();
    }
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());

    let mut policy = shipped_policy();
    if let ObligationExpr::AllOf { obligations, .. } = &mut policy.definitions[0].completion {
        if let ObligationExpr::Evidence { predicate, .. } = &mut obligations[0] {
            *predicate = "missing.predicate".to_string();
        }
    }
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());
}

#[test]
fn admission_rejects_temporal_and_exception_reference_mismatch() {
    let catalog = shipped_catalog();
    let mut policy = shipped_policy();
    if let ObligationExpr::AllOf { obligations, .. } = &mut policy.definitions[1].completion {
        if let ObligationExpr::Temporal { check, .. } = &mut obligations[1] {
            *check = "verify.other-order".to_string();
        }
    }
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());

    let mut policy = shipped_policy();
    policy.definitions[1].exceptions[0].node = "verify.missing.node".to_string();
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());

    let mut policy = shipped_policy();
    policy.definitions[0].workflow_version = 7;
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());

    let mut policy = shipped_policy();
    policy.evidence_predicates[0].accepted_revision_blake3 = "not-a-digest".to_string();
    assert!(admit_workflow_obligation_policy(&policy, &catalog, POLICY_HASH).is_err());
}

// r[verify cairn.workflow_obligations.evaluation]
#[test]
fn complete_native_evidence_proves_and_reports_satisfied_obligations() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let mut request = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    request.evidence = base_evidence(verify);
    let report =
        evaluate_workflow_obligations(verify, &catalog.policy, &request).expect("evaluation");
    assert_eq!(report.outcome, "proved");
    assert!(report.proved);
    assert!(report.successful_delivery);
    assert!(report.remaining.is_empty(), "{:?}", report.remaining);
    assert!(report
        .completion
        .iter()
        .any(|node| node.id == "verify.traceability" && node.state == "satisfied"));
    assert_eq!(report.selected_alternatives.len(), 1);
}

#[test]
fn missing_alternative_and_partial_branches_do_not_combine() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");

    let mut partial = request(verify, BTreeMap::new());
    partial.evidence = base_evidence(verify);
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &partial).unwrap();
    assert_eq!(report.outcome, "unknown");
    assert!(report
        .remaining
        .iter()
        .any(|item| item.id == "verify.traceability"));

    let mut evidence = base_evidence(verify);
    evidence.push(observation(
        verify,
        "verification-obligation.claim-revision",
        ObservationState::Satisfied,
    ));
    let mut partial_receipt = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    partial_receipt.evidence = evidence.clone();
    if let Some(freshness) = partial_receipt
        .evidence
        .iter_mut()
        .find(|observation| observation.predicate == "agent-receipt.freshness")
    {
        freshness.state = ObservationState::Unsatisfied;
    }
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &partial_receipt).unwrap();
    assert_eq!(report.outcome, "unsatisfied");
    assert!(
        !report
            .completion
            .iter()
            .any(|node| node.id == "verify.traceability.claim"),
        "unselected alternative must not be evaluated or combined"
    );

    let mut proved = request(
        verify,
        selection(&[("verify.traceability", "claim-revision")]),
    );
    proved.evidence = evidence;
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &proved).unwrap();
    assert_eq!(report.outcome, "proved");
}

#[test]
fn activity_counts_and_agent_success_claims_never_substitute_for_evidence() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let mut request = request(
        verify,
        selection(&[("verify.traceability", "claim-revision")]),
    );
    request.agent_claims = Some(AgentClaims {
        success_statement: Some("all work is complete".to_string()),
        checked_task_boxes: 12,
        activity_count: 99,
    });
    request.progress = vec![ProgressObservation {
        fact: "verify.progress.inspected".to_string(),
        observed: true,
        source: "agent".to_string(),
    }];
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap();
    assert!(!report.proved);
    assert!(!report.successful_delivery);
    assert_eq!(report.outcome, "unknown");
    assert_eq!(report.ignored_claims.len(), 3);
    assert!(report
        .progress
        .iter()
        .all(|fact| fact.effect == "none_on_completion"));
    assert!(!report.remaining.is_empty());
}

#[test]
fn stale_or_out_of_scope_observations_remain_unknown() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let mut request = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    let mut evidence = base_evidence(verify);
    evidence[0].revision_blake3 =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();
    request.evidence = evidence;
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unknown");
    assert!(report
        .remaining
        .iter()
        .any(|item| item.id == "verify.evidence.contract" && item.state == "unknown"));
}

#[test]
fn conditional_applicability_requires_the_declared_observation() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let mut request = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    let mut evidence = base_evidence(verify);
    evidence.retain(|observation| observation.predicate != "verification-scope.applicable");
    evidence.push(observation(
        verify,
        "verification-obligation.claim-revision",
        ObservationState::Satisfied,
    ));
    request.evidence = evidence;
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unknown");
}

#[test]
fn evaluation_rejects_a_binding_from_another_snapshot() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let mut request = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    request.evidence = base_evidence(verify);
    request.binding.definition_id.push_str(".other");
    let error = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap_err();
    assert_eq!(error.outcome(), "scope_mismatch");
}

#[test]
fn blocked_and_abandoned_terminals_are_never_successful() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    for state in [TerminalState::Blocked, TerminalState::Abandoned] {
        let mut request = request(
            verify,
            selection(&[("verify.traceability", "receipt-chain")]),
        );
        request.evidence = base_evidence(verify);
        request.terminal = Some(TerminalObservation {
            state,
            reason: "operator stopped the change".to_string(),
            observation_id: "terminal:1".to_string(),
        });
        let report = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap();
        assert!(!report.proved);
        assert!(!report.successful_delivery);
        assert!(matches!(report.outcome.as_str(), "blocked" | "abandoned"));
    }
}

// r[verify cairn.workflow_obligations.temporal]
#[test]
fn temporal_ordering_is_declared_finite_and_missing_evidence_stays_unknown() {
    let catalog = admitted_catalog();
    let bounded = definition(&catalog, "verify.bounded-exception");
    let mut request = request(
        bounded,
        selection(&[("verify.traceability", "claim-revision")]),
    );
    request.evidence = bounded_evidence(bounded);
    request.temporal = Some(trace(TraceCompleteness::Complete, Some(10), Some(20)));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert!(report.proved, "{:?}", report.remaining);
    let temporal = report.temporal.as_ref().unwrap();
    assert_eq!(temporal.state, "satisfied");
    assert!(temporal.non_claim.contains("unbounded safety"));

    let mut partial = request.clone();
    partial.temporal = Some(trace(TraceCompleteness::Partial, Some(10), Some(20)));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &partial).unwrap();
    assert_eq!(report.outcome, "unknown");

    let mut missing = request.clone();
    missing.temporal = Some(trace(TraceCompleteness::Complete, Some(10), None));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &missing).unwrap();
    assert_eq!(report.outcome, "unsatisfied");

    let mut reversed = request.clone();
    reversed.temporal = Some(trace(TraceCompleteness::Complete, Some(30), Some(20)));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &reversed).unwrap();
    assert_eq!(report.outcome, "unsatisfied");

    let mut outside = request.clone();
    outside.temporal = Some(trace(TraceCompleteness::Complete, Some(10), Some(90000)));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &outside).unwrap();
    assert_eq!(report.outcome, "unknown");

    let mut untrusted = request.clone();
    let mut untrusted_trace = trace(TraceCompleteness::Complete, Some(10), Some(20));
    untrusted_trace.events[0].observer = "untrusted-agent".to_string();
    untrusted.temporal = Some(untrusted_trace);
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &untrusted).unwrap();
    assert_eq!(report.outcome, "unknown");

    let mut absent = request.clone();
    absent.temporal = None;
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &absent).unwrap();
    assert_eq!(report.outcome, "unknown");
}

// r[verify cairn.workflow_obligations.exception]
#[test]
fn exception_path_requires_applicability_and_control_and_never_proves_the_original() {
    let catalog = admitted_catalog();
    let bounded = definition(&catalog, "verify.bounded-exception");
    let build = |snapshot: Option<ExceptionSnapshot>| {
        let mut request = request(
            bounded,
            selection(&[("verify.traceability", "claim-revision")]),
        );
        let mut evidence = bounded_evidence(bounded);
        if let Some(freshness) = evidence
            .iter_mut()
            .find(|observation| observation.predicate == "agent-receipt.freshness")
        {
            freshness.state = ObservationState::Unsatisfied;
        }
        request.evidence = evidence;
        request.temporal = Some(trace(TraceCompleteness::Complete, Some(10), Some(20)));
        request.exceptions = snapshot.into_iter().collect();
        request
    };
    let controls = || vec!["sandboxed-revalidation".to_string()];

    let request = build(Some(exception_snapshot(
        bounded,
        3,
        "approved",
        50,
        controls(),
        Some(ObservationState::Satisfied),
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "exception_accepted");
    assert!(!report.proved);
    assert!(report.successful_delivery);
    let freshness = report
        .completion
        .iter()
        .find(|node| node.id == "verify.evidence.freshness")
        .expect("freshness node is explained");
    assert_eq!(freshness.state, "exception_accepted");
    assert_eq!(freshness.original.as_ref().unwrap().state, "unsatisfied");
    assert_eq!(
        report.exceptions[0].applicability, "applicable",
        "{:?}",
        report.exceptions
    );

    let request = build(Some(exception_snapshot(
        bounded,
        3,
        "revoked",
        50,
        controls(),
        Some(ObservationState::Satisfied),
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unsatisfied");
    assert!(!report.proved);
    assert!(!report.successful_delivery);

    let request = build(Some(exception_snapshot(
        bounded,
        3,
        "approved",
        100,
        controls(),
        Some(ObservationState::Satisfied),
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unsatisfied");

    let request = build(Some(exception_snapshot(
        bounded,
        2,
        "approved",
        50,
        controls(),
        Some(ObservationState::Satisfied),
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unsatisfied");

    let request = build(Some(exception_snapshot(
        bounded,
        3,
        "approved",
        50,
        Vec::new(),
        Some(ObservationState::Satisfied),
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unsatisfied");

    let request = build(Some(exception_snapshot(
        bounded,
        3,
        "approved",
        50,
        controls(),
        None,
    )));
    let report = evaluate_workflow_obligations(bounded, &catalog.policy, &request).unwrap();
    assert_eq!(report.outcome, "unsatisfied");
    assert_eq!(report.exceptions[0].applicability, "applicable");
    assert_eq!(report.exceptions[0].control_state, "unknown");
}

// r[verify cairn.workflow_obligations.adapter_invariance]
// r[verify cairn.workflow_obligations.guidance]
#[test]
fn adapters_change_layout_but_not_obligations_evidence_or_authority() {
    let catalog = admitted_catalog();
    let generated = shipped_catalog();
    let verify = definition(&catalog, "verify");
    let guidance = obligation_guidance(verify, &catalog.policy, &BTreeMap::new()).unwrap();
    let baseline =
        export_workflow_obligation(verify, &catalog.policy, &BTreeMap::new(), SUBJECT, CONTEXT)
            .unwrap();

    let mut layouts = Vec::new();
    for adapter in ["agent-skills", "claude", "codex"] {
        let files = render_with_workflow_obligations(&generated, &catalog, adapter)
            .expect("adapter renders with obligations");
        let file = rendered_file(&files, "verify");
        for section in &guidance.sections {
            for item in &section.items {
                assert!(
                    file.contents.contains(item.as_str()),
                    "adapter {adapter} dropped obligation text: {item}"
                );
            }
        }
        for requirement in &baseline.predicate.evidence_requirements {
            assert!(
                file.contents.contains(requirement.requirement_id.as_str())
                    && file
                        .contents
                        .contains(requirement.accepted_revision_blake3.as_str()),
                "adapter {adapter} dropped native evidence revision"
            );
        }
        layouts.push(file.contents.clone());
    }
    assert_ne!(layouts[0], layouts[1], "adapters must differ in layout");
    assert_ne!(layouts[1], layouts[2], "adapters must differ in layout");

    // Presentation-only edits cannot change the predicate or command authority.
    let predicate =
        obligation_predicate(verify, &catalog.policy, &BTreeMap::new(), SUBJECT, CONTEXT).unwrap();
    for presentation in [
        Presentation {
            format: PresentationFormat::Markdown,
            heading_level: 1,
            bullet: "*".to_string(),
        },
        Presentation {
            format: PresentationFormat::Compact,
            heading_level: 5,
            bullet: "-".to_string(),
        },
    ] {
        let text = render_guidance(&guidance, &presentation).unwrap();
        assert!(text.contains("gate, readiness, receipt-inspect, traceability, workflow-inspect"));
        for requirement in &guidance.sections[0].items {
            assert!(text.contains(requirement.as_str()));
        }
    }
    let repeated =
        obligation_predicate(verify, &catalog.policy, &BTreeMap::new(), SUBJECT, CONTEXT).unwrap();
    assert_eq!(predicate.predicate_blake3, repeated.predicate_blake3);
    assert_eq!(predicate.allowed_command_classes, verify.command_classes);
    assert!(render_guidance(
        &guidance,
        &Presentation {
            format: PresentationFormat::Markdown,
            heading_level: 9,
            bullet: "-".to_string(),
        }
    )
    .is_err());
}

// r[verify cairn.workflow_obligations.legacy_compatibility]
#[test]
fn workflows_without_obligations_keep_legacy_render_identity() {
    let generated = shipped_catalog();
    let catalog = admitted_catalog();
    for adapter in ["agent-skills", "claude", "codex"] {
        let legacy = crate::verified::generated_agent_workflow::render_generated_agent_workflows(
            &generated, adapter,
        )
        .unwrap();
        let composed = render_with_workflow_obligations(&generated, &catalog, adapter).unwrap();
        let legacy_explore = rendered_file(&legacy, "explore");
        let composed_explore = rendered_file(&composed, "explore");
        assert_eq!(legacy_explore.contents, composed_explore.contents);
        assert_eq!(
            legacy_explore.content_blake3,
            composed_explore.content_blake3
        );
        assert_eq!(
            legacy_explore.descriptor_identity,
            composed_explore.descriptor_identity
        );
        let legacy_verify = rendered_file(&legacy, "verify");
        let composed_verify = rendered_file(&composed, "verify");
        assert!(composed_verify
            .contents
            .starts_with(legacy_verify.contents.as_str()));
        assert_ne!(legacy_verify.content_blake3, composed_verify.content_blake3);
    }
}

// r[verify cairn.workflow_obligations.export]
// r[verify cairn.workflow_obligations.predicate]
// r[verify cairn.workflow_obligations.authority]
#[test]
fn export_binds_intent_policy_subject_context_and_selected_alternative() {
    let catalog = admitted_catalog();
    let verify = definition(&catalog, "verify");
    let selected = selection(&[("verify.traceability", "claim-revision")]);
    let export =
        export_workflow_obligation(verify, &catalog.policy, &selected, SUBJECT, CONTEXT).unwrap();
    assert_eq!(export.predicate.binding, verify.binding);
    assert_eq!(export.predicate.subject_blake3, SUBJECT);
    assert_eq!(export.predicate.context_blake3, CONTEXT);
    assert_eq!(
        export
            .predicate
            .selected_alternatives
            .get("verify.traceability"),
        Some(&"claim-revision".to_string())
    );
    assert!(export
        .obligations
        .iter()
        .any(|obligation| obligation.id == "verify.traceability"));

    let other_context = export_workflow_obligation(
        verify,
        &catalog.policy,
        &selected,
        SUBJECT,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    )
    .unwrap();
    assert_ne!(
        export.predicate.predicate_blake3,
        other_context.predicate.predicate_blake3
    );

    let other_selection = export_workflow_obligation(
        verify,
        &catalog.policy,
        &selection(&[("verify.traceability", "receipt-chain")]),
        SUBJECT,
        CONTEXT,
    )
    .unwrap();
    assert_ne!(
        export.predicate.predicate_blake3,
        other_selection.predicate.predicate_blake3
    );

    let mut request = request(
        verify,
        selection(&[("verify.traceability", "receipt-chain")]),
    );
    request.evidence = base_evidence(verify);
    let report = evaluate_workflow_obligations(verify, &catalog.policy, &request).unwrap();
    assert_eq!(
        report.predicate_blake3,
        other_selection.predicate.predicate_blake3
    );
    assert_eq!(report.binding, export.binding);
}

fn consumer_facts(
    catalog: &AdmittedWorkflowObligationCatalog,
    id: &str,
    selected: &BTreeMap<String, String>,
    subject: &str,
) -> (
    ObligationPredicate,
    ObligationGuidance,
    Vec<ObligationNodeFact>,
) {
    let admitted = definition(catalog, id);
    let predicate =
        obligation_predicate(admitted, &catalog.policy, selected, subject, CONTEXT).unwrap();
    let guidance = obligation_guidance(admitted, &catalog.policy, selected).unwrap();
    let facts = obligation_node_facts(admitted, &predicate, &guidance).unwrap();
    (predicate, guidance, facts)
}

fn fact<'a>(facts: &'a [ObligationNodeFact], id: &str) -> &'a ObligationNodeFact {
    facts
        .iter()
        .find(|fact| fact.id == id)
        .expect("fact is present")
}

// r[verify cairn.workflow_obligations.consumer_facts]
#[test]
fn consumer_facts_type_every_node_with_selection_and_native_references() {
    let catalog = admitted_catalog();
    let (predicate, guidance, facts) =
        consumer_facts(&catalog, "verify", &BTreeMap::new(), SUBJECT);
    verify_obligation_node_facts(&predicate, &guidance, &facts).unwrap();
    let mut expression_ids = Vec::new();
    fn ids(expression: &ObligationExpr, out: &mut Vec<String>) {
        out.push(expression.id().to_string());
        match expression {
            ObligationExpr::AllOf { obligations, .. } => {
                obligations.iter().for_each(|child| ids(child, out));
            }
            ObligationExpr::AnyOf { alternatives, .. } => alternatives
                .iter()
                .flat_map(|alternative| alternative.obligations.iter())
                .for_each(|child| ids(child, out)),
            ObligationExpr::Applicable { then, .. } => ids(then, out),
            ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => {}
        }
    }
    ids(&predicate.expression, &mut expression_ids);
    assert_eq!(
        facts.iter().map(|fact| fact.id.clone()).collect::<Vec<_>>(),
        expression_ids
    );
    let contract = fact(&facts, "verify.evidence.contract");
    assert_eq!(contract.kind, ObligationNodeKind::Evidence);
    assert_eq!(contract.selection, ObligationSelectionState::Required);
    assert_eq!(contract.parent.as_deref(), Some("verify.completion"));
    assert_eq!(
        contract.evidence[0].requirement_id,
        "cairn.supersede.agent_run_receipts.verification_contract"
    );
    let freshness = fact(&facts, "verify.evidence.freshness");
    assert_eq!(
        freshness.expected_observations,
        vec!["verify.observation.receipt"]
    );
    assert_eq!(freshness.command_classes, vec!["receipt-inspect"]);
    // Without a selected alternative every branch node is unresolved.
    for id in ["verify.traceability.contract", "verify.traceability.claim"] {
        assert_eq!(
            fact(&facts, id).selection,
            ObligationSelectionState::SelectionMissing
        );
    }
    let conditional = fact(&facts, "verify.conditional.claim.revision");
    assert_eq!(conditional.selection, ObligationSelectionState::Conditional);
    assert_eq!(conditional.conditions, vec!["verify.conditional.claim"]);
    assert_eq!(
        fact(&facts, "verify.conditional.claim").evidence[0].kind,
        EvidencePredicateKind::Applicability
    );

    let selected = selection(&[("verify.traceability", "receipt-chain")]);
    let (_, _, chosen) = consumer_facts(&catalog, "verify", &selected, SUBJECT);
    assert_eq!(
        fact(&chosen, "verify.traceability.contract").selection,
        ObligationSelectionState::SelectedAlternative
    );
    assert_eq!(
        fact(&chosen, "verify.traceability.claim").selection,
        ObligationSelectionState::UnselectedAlternative
    );
    // Meaning is selection-independent; guidance and condition identities are not.
    let before = fact(&facts, "verify.traceability.contract");
    let after = fact(&chosen, "verify.traceability.contract");
    assert_eq!(before.obligation_blake3, after.obligation_blake3);
    assert_ne!(before.guidance_blake3, after.guidance_blake3);
    assert_ne!(before.condition_blake3, after.condition_blake3);

    // A different subject changes only the remaining-condition identity.
    let other = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    let (_, _, moved) = consumer_facts(&catalog, "verify", &BTreeMap::new(), other);
    for (left, right) in facts.iter().zip(&moved) {
        assert_eq!(left.obligation_blake3, right.obligation_blake3);
        assert_eq!(left.guidance_blake3, right.guidance_blake3);
        assert_ne!(left.condition_blake3, right.condition_blake3);
    }
}

// r[verify cairn.workflow_obligations.consumer_facts]
#[test]
fn consumer_facts_carry_exception_identity_and_refuse_tampering() {
    let catalog = admitted_catalog();
    let (predicate, guidance, facts) = consumer_facts(
        &catalog,
        "verify.bounded-exception",
        &BTreeMap::new(),
        SUBJECT,
    );
    verify_obligation_node_facts(&predicate, &guidance, &facts).unwrap();
    let exception = fact(&facts, "verify.exception.freshness");
    assert_eq!(exception.kind, ObligationNodeKind::Exception);
    let reference = exception.exception.as_ref().unwrap();
    assert_eq!(reference.replaces, "verify.evidence.freshness");
    assert_eq!(reference.request_revision, 3);
    assert_eq!(reference.control, "sandboxed-revalidation");
    assert_eq!(
        fact(&facts, "verify.bounded.ordering")
            .temporal_check
            .as_deref(),
        Some("verify.receipt-order")
    );

    let mut weakened = facts.clone();
    let index = weakened
        .iter()
        .position(|fact| fact.id == "verify.evidence.pass-only")
        .unwrap();
    weakened[index].evidence[0].accepted_revision_blake3 = "0".repeat(64);
    assert!(verify_obligation_node_facts(&predicate, &guidance, &weakened).is_err());

    let mut promoted = facts.clone();
    let index = promoted
        .iter()
        .position(|fact| fact.id == "verify.traceability.claim")
        .unwrap();
    promoted[index].selection = ObligationSelectionState::Required;
    assert!(verify_obligation_node_facts(&predicate, &guidance, &promoted).is_err());

    let mut dropped = facts.clone();
    dropped.retain(|fact| fact.kind != ObligationNodeKind::Exception);
    assert!(verify_obligation_node_facts(&predicate, &guidance, &dropped).is_err());

    let mut omitted = facts.clone();
    omitted.remove(1);
    assert!(verify_obligation_node_facts(&predicate, &guidance, &omitted).is_err());

    let mut stale = facts.clone();
    stale[0].condition_blake3 = "1".repeat(64);
    assert!(verify_obligation_node_facts(&predicate, &guidance, &stale).is_err());

    let mut resubjected = predicate.clone();
    resubjected.subject_blake3 = "c".repeat(64);
    assert!(!verify_predicate_identity(&resubjected));
    assert!(verify_obligation_node_facts(&resubjected, &guidance, &facts).is_err());

    let (_, other_guidance, _) = consumer_facts(
        &catalog,
        "verify.bounded-exception",
        &selection(&[("verify.traceability", "claim-revision")]),
        SUBJECT,
    );
    assert!(verify_obligation_node_facts(&predicate, &other_guidance, &facts).is_err());

    let verify = definition(&catalog, "verify");
    assert!(obligation_node_facts(verify, &predicate, &guidance).is_err());
}
