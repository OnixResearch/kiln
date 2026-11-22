//! Producer/consumer fixtures composed by the pinned producer library.
//!
//! These are synthetic producer inputs, not native `cairn selected-work`
//! executions, live campaigns or Lattice runs (Kiln #49 owns live
//! qualification).
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use cairn_core::verified::generated_agent_workflow::{
    GeneratedAgentWorkflowPolicy, admit_generated_agent_workflow_policy,
};
use cairn_core::verified::selected_work::{
    ObligationObservationInput, ObligationSource, SelectedWorkInput,
};
use cairn_core::{
    AdmittedWorkflowObligationCatalog, WorkflowObligationPolicy, admit_workflow_obligation_policy,
};
use kiln_core::implementation::selected_work::{
    AttemptSnapshotContract, AuthoritativeCompletion, AuthoritativeVerdict,
    CanonicalCheckObservation, CheckState, CohortField, GapReason, IntakeProfile, NodeSelection,
    ObligationCompletion, SelectedWorkHistory, SelectedWorkObservation, SurfaceObservation,
    TransitionKind, bind_attempt, classify_observation, decide_obligation_completion,
    evidence_observations, restricted_display,
};
use kiln_provider_api::PortResult;

use super::*;

const SHIPPED_DEFAULT: &str =
    include_str!("../../../vendor/cairn-verifier/cairn-policy/generated/cairn-policy.json");
const SHIPPED_OBLIGATIONS: &str =
    include_str!("../../../vendor/cairn-verifier/cairn-policy/generated/workflow-obligations.json");
const FIXTURE_DIRECTORY: &str = "../../fixtures/cairn/selected-work";
const UPDATE_FIXTURES: &str = "KILN_UPDATE_SELECTED_WORK_FIXTURES";
const SELECTED_ALTERNATIVE: (&str, &str) = ("verify.traceability", "receipt-chain");
const PASSING_PREDICATES: [&str; 5] = [
    "agent-receipt.verification-contract",
    "agent-receipt.pass-only",
    "agent-receipt.completion-authority",
    "verification-obligation.claim-revision",
    "agent-receipt.freshness",
];
const APPLICABILITY_PREDICATE: &str = "verification-scope.applicable";

fn hash(n: u8) -> String {
    format!("{n:064x}")
}

fn cid_of(n: u8) -> ContentId {
    ContentId::from_blake3_hex(hash(n)).unwrap_or_else(|_| unreachable!())
}

fn input(policy: u8, subject: Option<u8>) -> SelectedWorkInput {
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
    .map(
        |stage| json!({"stage":stage, "state":"blocked", "decision_hash":hash(10), "decision":{}}),
    );
    serde_json::from_value(json!({
        "producer": "cairn", "selection": {"change":"sample", "task":null},
        "source": {"revision":{"kind":"projection", "identity":revision}, "artifacts":artifacts},
        "policy_hash":hash(policy), "profile":"spec-driven", "profile_hash":hash(4),
        "dependencies":[{"task":"t1", "prerequisite":"t0", "state":"unavailable", "identity":hash(20)}],
        "completion":{"state":"blocked", "contract_hash":hash(21), "results_hash":null,
            "subject_hash":subject.map(hash)},
        "context":{"selection_hash":hash(5), "local_hash":hash(6), "portable_hash":hash(7), "entries":[]},
        "readiness":{"schema":"cairn.readiness.v1", "scope":{"change":"sample",
            "policy_hash":hash(policy), "source_hash":hash(8), "evidence_hash":hash(9)},
            "valid":true, "classification":"blocked", "stages":stages,
            "blockers":[{"id":"completion.missing-results", "stage":"completion",
                "code":"missing_results", "target":"sample", "message":"results are absent"}],
            "actions":[], "diagnostics":[], "readiness_policy_hash":hash(11),
            "input_hash":hash(12), "receipt_hash":hash(13), "mutation_authorized":false,
            "non_claims":[]},
        "units":[{"task":"t1", "state":"complete", "references":[
            {"kind":"requirement", "id":"kiln.sample", "scope":{"kind":"change"}},
            {"kind":"acceptance", "id":"kiln.sample", "scope":{"kind":"task", "task":"t1"}}],
            "verification_contracts":[]}],
        "remaining":[{"kind":"completion", "state":"blocked", "subject":null},
            {"kind":"sync", "state":"blocked", "subject":null},
            {"kind":"archive", "state":"blocked", "subject":null}],
        "enumeration":{"complete":true, "observed":1, "total":1,
            "budget_bytes":67108864, "consumed_bytes":21}, "local_root":"/private/cairn"
    }))
    .unwrap_or_else(|error| panic_free(&error.to_string()))
}

fn panic_free<T>(message: &str) -> T {
    assert!(message.is_empty(), "{message}");
    unreachable!()
}

fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).unwrap_or(&Value::Null)
}

/// Replace an existing field, or add a new top-level field.
fn set(value: &mut Value, pointer: &str, replacement: Value) {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = replacement;
    } else if let (Some(object), Some(key)) = (value.as_object_mut(), pointer.strip_prefix('/')) {
        object.insert(key.to_owned(), replacement);
    }
}
fn catalog(policy: u8) -> AdmittedWorkflowObligationCatalog {
    let value: Value = serde_json::from_str(SHIPPED_DEFAULT).unwrap_or_else(|_| unreachable!());
    let generated: GeneratedAgentWorkflowPolicy =
        serde_json::from_value(at(&value, "/generated_agent_workflow_policy").clone())
            .unwrap_or_else(|_| unreachable!());
    let workflows =
        admit_generated_agent_workflow_policy(&generated).unwrap_or_else(|_| unreachable!());
    let obligations: WorkflowObligationPolicy =
        serde_json::from_str(SHIPPED_OBLIGATIONS).unwrap_or_else(|_| unreachable!());
    admit_workflow_obligation_policy(&obligations, &workflows, &hash(policy))
        .unwrap_or_else(|_| unreachable!())
}

/// Exact portable producer wire bytes.
fn produce(policy: u8, subject: Option<u8>, observations: &ObligationObservationInput) -> Vec<u8> {
    let catalog = catalog(policy);
    let source = ObligationSource {
        catalog: &catalog,
        definition: "verify",
        observations: Some(observations),
    };
    let snapshot = producer::compose(
        &SelectedWorkPolicy::bundled().unwrap_or_else(|_| unreachable!()),
        &input(policy, subject),
        Some(&source),
    )
    .unwrap_or_else(|error| panic_free(&error.to_string()))
    .into_portable();
    let mut bytes = serde_json::to_vec_pretty(&snapshot).unwrap_or_else(|_| unreachable!());
    bytes.push(b'\n');
    bytes
}

fn selection_only() -> ObligationObservationInput {
    ObligationObservationInput {
        selected_alternatives: BTreeMap::from([(
            SELECTED_ALTERNATIVE.0.to_string(),
            SELECTED_ALTERNATIVE.1.to_string(),
        )]),
        ..ObligationObservationInput::default()
    }
}

fn admit(wire: &[u8]) -> AdmittedSelectedWork {
    admit_selected_work(wire, None).unwrap_or_else(|error| panic_free(&error.to_string()))
}

fn contract(planning: &AdmittedSelectedWork) -> AttemptSnapshotContract {
    bind_attempt(cid_of(31), cid_of(32), Some(cid_of(33)), planning.clone())
        .unwrap_or_else(|_| unreachable!())
}

fn candidate() -> ContentId {
    cid_of(34)
}

/// Canonical Kiln checks for named predicates, bound to the contract.
fn checks(
    contract: &AttemptSnapshotContract,
    predicates: &[(&str, CheckState)],
) -> Vec<CanonicalCheckObservation> {
    let plan = contract.snapshot.plan().unwrap_or_else(|| unreachable!());
    predicates
        .iter()
        .enumerate()
        .map(|(index, (predicate, state))| {
            let requirement = plan
                .nodes
                .iter()
                .find_map(|node| {
                    node.evidence
                        .iter()
                        .find(|item| item.predicate == *predicate)
                })
                .unwrap_or_else(|| unreachable!());
            CanonicalCheckObservation {
                check: cid_of(u8::try_from(index).unwrap_or_default().saturating_add(100)),
                predicate: requirement.predicate.clone(),
                requirement_id: requirement.requirement_id.clone(),
                revision: requirement.accepted_revision.clone(),
                subject: contract.snapshot.cohort.verification_subject.clone(),
                context: contract.snapshot.cohort.context.clone(),
                policy: contract.snapshot.policy.clone(),
                candidate: candidate(),
                state: *state,
            }
        })
        .collect()
}

fn all_checks(contract: &AttemptSnapshotContract) -> Vec<CanonicalCheckObservation> {
    let mut named: Vec<(&str, CheckState)> = PASSING_PREDICATES
        .iter()
        .map(|predicate| (*predicate, CheckState::Passed))
        .collect();
    // Out of scope: the conditional subtree is not applicable.
    named.push((APPLICABILITY_PREDICATE, CheckState::Failed));
    checks(contract, &named)
}

/// Feed Kiln's canonical checks to the producer, as a real loop would.
fn evaluate(
    contract: &AttemptSnapshotContract,
    policy: u8,
    subject: Option<u8>,
    checks: &[CanonicalCheckObservation],
) -> Vec<u8> {
    evaluate_evidence(
        contract,
        policy,
        subject,
        &evidence_observations(contract, &candidate(), checks),
    )
}

fn evaluate_evidence(
    contract: &AttemptSnapshotContract,
    policy: u8,
    subject: Option<u8>,
    evidence: &[ObligationEvidenceObservation],
) -> Vec<u8> {
    let plan = contract.snapshot.plan().unwrap_or_else(|| unreachable!());
    let file = producer_observation_file(plan, evidence);
    let observations: ObligationObservationInput =
        serde_json::from_value(file).unwrap_or_else(|error| panic_free(&error.to_string()));
    produce(policy, subject, &observations)
}

/// Stale evidence handed straight to the producer, bypassing Kiln's binding
/// filter, to show the producer keeps it out of scope too.
fn unfiltered(checks: &[CanonicalCheckObservation]) -> Vec<ObligationEvidenceObservation> {
    checks
        .iter()
        .map(|check| ObligationEvidenceObservation {
            predicate: check.predicate.clone(),
            requirement_id: check.requirement_id.clone(),
            revision: check.revision.clone(),
            subject: check.subject.clone(),
            context: check.context.clone(),
            policy: check.policy.clone(),
            observation: check.check.clone(),
            state: match check.state {
                CheckState::Passed => EvidenceObservationState::Satisfied,
                CheckState::Failed => EvidenceObservationState::Unsatisfied,
                _ => EvidenceObservationState::Unknown,
            },
        })
        .collect()
}

fn passed() -> AuthoritativeCompletion {
    AuthoritativeCompletion {
        subject: cid_of(0),
        policy: cid_of(0),
        candidate: candidate(),
        verdict: AuthoritativeVerdict::Passed,
    }
}

fn authoritative(contract: &AttemptSnapshotContract) -> AuthoritativeCompletion {
    AuthoritativeCompletion {
        subject: contract.snapshot.cohort.verification_subject.clone(),
        policy: contract.snapshot.policy.clone(),
        ..passed()
    }
}

fn gaps(decision: &ObligationCompletion) -> Vec<(String, GapReason)> {
    match decision {
        ObligationCompletion::Complete { .. } => Vec::new(),
        ObligationCompletion::Incomplete { gaps, .. } => gaps
            .iter()
            .map(|gap| (gap.obligation.clone(), gap.reason))
            .collect(),
    }
}

fn decide(
    contract: &AttemptSnapshotContract,
    evaluated: &AdmittedSelectedWork,
    checks: &[CanonicalCheckObservation],
) -> ObligationCompletion {
    decide_obligation_completion(
        contract,
        evaluated,
        &candidate(),
        checks,
        Some(&authoritative(contract)),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn leaf_of(snapshot: &AdmittedSelectedWork, predicate: &str) -> String {
    snapshot
        .plan()
        .and_then(|plan| {
            plan.nodes.iter().find(|node| {
                node.contributing_leaf()
                    && node.evidence.iter().any(|item| item.predicate == predicate)
            })
        })
        .map(|node| node.id.clone())
        .unwrap_or_else(|| unreachable!())
}

/// The finite verify pilot over one retained attempt contract.
struct Pilot {
    planning: Vec<u8>,
    passing: Vec<u8>,
    missing_check: Vec<u8>,
    stale_subject: Vec<u8>,
    changed_policy: Vec<u8>,
    interrupted_observation: Vec<u8>,
}

fn pilot() -> (Pilot, AttemptSnapshotContract) {
    let planning = produce(3, Some(22), &selection_only());
    let bound = contract(&admit(&planning));
    let full = all_checks(&bound);
    let mut missing = full.clone();
    missing.retain(|check| check.predicate != "agent-receipt.freshness");
    let mut stale = full.clone();
    for check in &mut stale {
        check.subject = cid_of(40);
    }
    let mut interrupted = full.clone();
    for check in &mut interrupted {
        if check.predicate == "agent-receipt.pass-only" {
            check.state = CheckState::Interrupted;
        }
    }
    let pilot = Pilot {
        passing: evaluate(&bound, 3, Some(22), &full),
        missing_check: evaluate(&bound, 3, Some(22), &missing),
        stale_subject: evaluate_evidence(&bound, 3, Some(22), &unfiltered(&stale)),
        changed_policy: produce(4, Some(22), &selection_only()),
        interrupted_observation: evaluate(&bound, 3, Some(22), &interrupted),
        planning,
    };
    (pilot, bound)
}

#[test]
// r[verify kiln.selected_work.admission.scenario.valid]
fn native_snapshot_admits_with_exact_identities_and_typed_facts() {
    let (pilot, bound) = pilot();
    let admitted = admit(&pilot.planning);
    assert_eq!(
        admitted.snapshot.as_str(),
        blake3::hash(&pilot.planning).to_hex().as_str()
    );
    assert_eq!(admitted, bound.snapshot);
    let wire: Value = serde_json::from_slice(&pilot.planning).unwrap_or_else(|_| unreachable!());
    for (field, value) in [
        ("source", &admitted.cohort.source),
        ("intent", &admitted.cohort.intent),
        ("context", &admitted.cohort.context),
        (
            "verification_subject",
            &admitted.cohort.verification_subject,
        ),
        ("dependencies", &admitted.cohort.dependencies),
        ("obligations", &admitted.cohort.obligations),
        ("selection", &admitted.cohort.selection),
        ("observation", &admitted.cohort.observation),
    ] {
        assert_eq!(
            at(&wire, &format!("/cohort/{field}")).as_str(),
            Some(value.as_str()),
            "{field}"
        );
    }
    assert_eq!(
        at(&wire, "/policy_hash").as_str(),
        Some(admitted.policy.as_str())
    );
    assert_eq!(
        at(&wire, "/profile_hash").as_str(),
        Some(admitted.profile_identity.as_str())
    );
    assert_eq!(
        at(&wire, "/source/revision/identity").as_str(),
        Some(admitted.source.revision.as_str())
    );
    assert_eq!(admitted.dependencies.len(), 1);
    assert_eq!(
        admitted
            .dependencies
            .first()
            .map(|fact| fact.prerequisite.as_str()),
        Some("t0")
    );
    assert!(admitted.completion_only());
    assert_eq!(
        admitted
            .readiness
            .blockers
            .first()
            .map(|blocker| blocker.id.as_str()),
        Some("completion.missing-results")
    );
    assert_eq!(admitted.readiness.stages.len(), 7);
    // r[verify kiln.selected_work.references.scenario.distinct]
    let references = admitted.references();
    assert_eq!(references.len(), 2);
    assert!(references.iter().any(|reference| reference.kind
        == kiln_core::implementation::selected_work::ReferenceKind::Requirement));
    assert!(references.iter().any(|reference| reference.kind
        == kiln_core::implementation::selected_work::ReferenceKind::Acceptance));
    let plan = admitted.plan().unwrap_or_else(|| unreachable!());
    assert_eq!(plan.definition_id, "verify");
    assert_eq!(
        plan.selected_alternatives
            .get(SELECTED_ALTERNATIVE.0)
            .map(String::as_str),
        Some(SELECTED_ALTERNATIVE.1)
    );
    assert!(
        plan.nodes
            .iter()
            .any(|node| node.selection == NodeSelection::UnselectedAlternative)
    );
    assert!(
        !plan
            .nodes
            .iter()
            .any(|node| node.selection == NodeSelection::SelectionMissing)
    );
}

#[test]
// r[verify kiln.selected_work.admission.scenario.refused]
fn unsupported_malformed_incomplete_stale_mixed_and_crossed_records_refuse() {
    let (pilot, bound) = pilot();
    let planning: Value =
        serde_json::from_slice(&pilot.planning).unwrap_or_else(|_| unreachable!());
    let passing: Value = serde_json::from_slice(&pilot.passing).unwrap_or_else(|_| unreachable!());
    let refuse = |value: &Value, expected: Option<&SelectedWorkCohort>| {
        admit_selected_work(
            &serde_json::to_vec(value).unwrap_or_else(|_| unreachable!()),
            expected,
        )
        .err()
    };
    let mut unsupported = planning.clone();
    set(&mut unsupported, "/schema", json!("cairn.selected-work.v2"));
    assert_eq!(
        refuse(&unsupported, None),
        Some(SelectedWorkRefusal::UnsupportedSchema)
    );
    assert_eq!(
        admit_selected_work(
            pilot
                .planning
                .get(..pilot.planning.len() / 2)
                .unwrap_or_default(),
            None
        )
        .err(),
        Some(SelectedWorkRefusal::Malformed)
    );
    let mut extra = planning.clone();
    set(&mut extra, "/forged_authority", json!(true));
    assert_eq!(refuse(&extra, None), Some(SelectedWorkRefusal::Malformed));
    let mut incomplete = planning.clone();
    set(&mut incomplete, "/enumeration/complete", json!(false));
    assert!(refuse(&incomplete, None).is_some());
    let mut emptied = planning.clone();
    set(&mut emptied, "/remaining", json!([]));
    assert!(refuse(&emptied, None).is_some());
    // Stale: a later evaluation against the cohort retained for the contract.
    assert_eq!(
        refuse(&passing, Some(&bound.snapshot.cohort)),
        Some(SelectedWorkRefusal::StaleCohort)
    );
    // Mixed: the planning cohort with the passing obligation section.
    let mut mixed = planning.clone();
    set(
        &mut mixed,
        "/obligations",
        at(&passing, "/obligations").clone(),
    );
    assert!(matches!(
        refuse(&mixed, None),
        Some(SelectedWorkRefusal::StaleCohort | SelectedWorkRefusal::InvalidObligations)
    ));
    let mut mixed_cohort = planning.clone();
    set(&mut mixed_cohort, "/cohort/intent", json!(hash(50)));
    assert_eq!(
        refuse(&mixed_cohort, None),
        Some(SelectedWorkRefusal::StaleCohort)
    );
    // Crossed: an expected cohort naming another verification subject.
    let mut crossed = bound.snapshot.cohort.clone();
    crossed.verification_subject = cid_of(51);
    assert_eq!(
        refuse(&planning, Some(&crossed)),
        Some(SelectedWorkRefusal::CrossedSubject)
    );
    // Restricted display output is not producer input.
    // r[verify kiln.selected_work.display.scenario.resubmit]
    let display =
        restricted_display(&bound.snapshot, "operator").unwrap_or_else(|_| unreachable!());
    let display_bytes = serde_json::to_vec(&display).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_selected_work(&display_bytes, None).err(),
        Some(SelectedWorkRefusal::UnsupportedSchema)
    );
    let mut disguised = serde_json::to_value(&display).unwrap_or_else(|_| unreachable!());
    set(&mut disguised, "/schema", json!(SELECTED_WORK_SCHEMA));
    assert_eq!(
        refuse(&disguised, None),
        Some(SelectedWorkRefusal::Malformed)
    );
    let kiln_mapping = serde_json::to_value(&bound.snapshot).unwrap_or_else(|_| unreachable!());
    assert!(refuse(&kiln_mapping, None).is_some());
}

#[test]
// r[verify kiln.selected_work.completion.scenario.pilot]
fn finite_verify_pilot_completes_only_the_passing_case() {
    let (pilot, bound) = pilot();
    let full = all_checks(&bound);

    let passing = admit(&pilot.passing);
    let transition = classify_observation(&bound, &passing, Some(&cid_of(33)));
    assert_eq!(transition.kind, TransitionKind::Refresh);
    let decision = decide(&bound, &passing, &full);
    assert!(decision.complete(), "{decision:?}");

    // Missing check: the producer leaves the node remaining, and Kiln names it.
    let freshness = leaf_of(&passing, "agent-receipt.freshness");
    let missing = admit(&pilot.missing_check);
    let mut fewer = full.clone();
    fewer.retain(|check| check.predicate != "agent-receipt.freshness");
    let gaps_missing = gaps(&decide(&bound, &missing, &fewer));
    assert!(gaps_missing.contains(&(freshness.clone(), GapReason::ProducerRemaining)));
    assert!(gaps_missing.contains(&(freshness.clone(), GapReason::MissingCheck)));
    // Kiln's cross-check refuses a producer success its own checks do not
    // back, naming every obligation that requires the missing predicate.
    let cross_checked = gaps(&decide(&bound, &passing, &fewer));
    assert!(cross_checked.contains(&(freshness, GapReason::MissingCheck)));
    assert!(
        cross_checked
            .iter()
            .all(|(_, reason)| *reason == GapReason::MissingCheck)
    );

    // Stale subject: evidence for another subject satisfies nothing.
    let stale = admit(&pilot.stale_subject);
    let mut stale_checks = full.clone();
    for check in &mut stale_checks {
        check.subject = cid_of(40);
    }
    let stale_gaps = gaps(&decide(&bound, &stale, &stale_checks));
    assert!(
        stale_gaps
            .iter()
            .any(|(_, reason)| *reason == GapReason::StaleSubject)
    );
    assert!(stale_gaps.contains(&("verify".into(), GapReason::ProducerNotDelivered)));
    // A moved verification subject replans and cannot complete the old contract.
    let moved = admit(&produce(3, Some(23), &selection_only()));
    let replan = classify_observation(&bound, &moved, Some(&cid_of(33)));
    assert_eq!(replan.kind, TransitionKind::Replan);
    assert!(replan.changed.contains(&CohortField::VerificationSubject));
    assert!(
        gaps(&decide(&bound, &moved, &full)).contains(&("verify".into(), GapReason::StaleSubject))
    );

    // Changed policy: rebinds every obligation identity and replans.
    let changed = admit(&pilot.changed_policy);
    let replan = classify_observation(&bound, &changed, Some(&cid_of(33)));
    assert_eq!(replan.kind, TransitionKind::Replan);
    assert!(replan.changed.contains(&CohortField::Policy));
    let changed_gaps = gaps(&decide(&bound, &changed, &full));
    assert!(changed_gaps.contains(&("verify".into(), GapReason::StalePolicy)));

    // Interrupted observation: an interrupted check stays unknown, never passed.
    let pass_only = leaf_of(&passing, "agent-receipt.pass-only");
    let interrupted = admit(&pilot.interrupted_observation);
    let mut interrupted_checks = full.clone();
    for check in &mut interrupted_checks {
        if check.predicate == "agent-receipt.pass-only" {
            check.state = CheckState::Interrupted;
        }
    }
    let interrupted_gaps = gaps(&decide(&bound, &interrupted, &interrupted_checks));
    assert!(interrupted_gaps.contains(&(pass_only.clone(), GapReason::InterruptedCheck)));
    assert!(interrupted_gaps.contains(&(pass_only, GapReason::ProducerRemaining)));

    // An interrupted selected-work read is unavailable, not an empty set.
    let mut runtime = ScriptedRuntime::new(vec![CairnProcessOutput::TimedOut]);
    runtime.identity = profile().executable_identity;
    let mut reader =
        CairnSelectedWorkReader::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let refusal = reader.read(&request()).err();
    assert_eq!(refusal, Some(SelectedWorkRefusal::Unavailable));
    let mut history = SelectedWorkHistory::new(IntakeProfile::TypedSnapshotV1);
    history
        .observe(SelectedWorkObservation::Admitted {
            snapshot: Box::new(bound.snapshot.clone()),
            target: Some(cid_of(33)),
        })
        .unwrap_or_else(|_| unreachable!());
    history
        .bind(bound.clone())
        .unwrap_or_else(|_| unreachable!());
    history
        .observe(SelectedWorkObservation::Unavailable {
            reason: "selected-work read timed out".into(),
            refusal: refusal.map(|item| item.to_string()),
        })
        .unwrap_or_else(|_| unreachable!());
    let surface = history.surface().unwrap_or_else(|_| unreachable!());
    assert_eq!(surface.observation, SurfaceObservation::Unavailable);
    assert_eq!(surface.current_contract, Some(bound.identity.clone()));
    assert_eq!(history.current.as_ref(), Some(&bound));
}

#[derive(Debug)]
struct ScriptedRuntime {
    identity: ContentId,
    outputs: VecDeque<CairnProcessOutput>,
    invocations: Vec<ProcessInvocation>,
}

impl ScriptedRuntime {
    fn new(outputs: Vec<CairnProcessOutput>) -> Self {
        Self {
            identity: cid_of(60),
            outputs: outputs.into(),
            invocations: Vec::new(),
        }
    }
}

impl CairnRuntime for ScriptedRuntime {
    fn executable_identity(&self, _path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        Ok(self.identity.clone())
    }
    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<CairnProcessOutput> {
        self.invocations.push(invocation.clone());
        Ok(self
            .outputs
            .pop_front()
            .unwrap_or(CairnProcessOutput::Unknown))
    }
    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<CairnProcessOutput> {
        self.run(invocation)
    }
}

fn profile() -> CairnCampaignProfile {
    CairnCampaignProfile {
        executable: PathBuf::from("/nix/store/00000000000000000000000000000000-cairn/bin/cairn"),
        repository_root: PathBuf::from("/repository"),
        policy_path: PathBuf::from("/repository/cairn-policy.json"),
        executable_identity: cid_of(61),
        policy_identity: cid_of(3),
    }
}

fn request() -> SelectedWorkRequest {
    SelectedWorkRequest {
        change: "sample".into(),
        task: None,
        obligation_definition: Some("verify".into()),
        obligation_observations: Some(PathBuf::from(".kiln/observations.json")),
        expected: None,
    }
}

fn exited(status: i32, stdout: Vec<u8>, stderr: &str) -> CairnProcessOutput {
    CairnProcessOutput::Exited {
        process_status: Some(status),
        stdout,
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn reader_maps_native_outcomes_to_typed_admission_or_refusal() {
    let (pilot, _) = pilot();
    let mut runtime = ScriptedRuntime::new(vec![
        exited(0, pilot.planning.clone(), ""),
        exited(
            1,
            Vec::new(),
            "error: selected-work rejected: StaleCohort\n",
        ),
        exited(1, Vec::new(), "error: unexpected failure\n"),
        CairnProcessOutput::Cancelled,
        exited(0, pilot.changed_policy.clone(), ""),
    ]);
    runtime.identity = cid_of(61);
    let mut reader =
        CairnSelectedWorkReader::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let admitted = reader.read(&request()).unwrap_or_else(|_| unreachable!());
    assert_eq!(admitted, admit(&pilot.planning));
    assert_eq!(
        reader.read(&request()).err(),
        Some(SelectedWorkRefusal::StaleCohort)
    );
    assert_eq!(
        reader.read(&request()).err(),
        Some(SelectedWorkRefusal::Unavailable)
    );
    assert_eq!(
        reader.read(&request()).err(),
        Some(SelectedWorkRefusal::Unavailable)
    );
    // The selected policy identity binds the reader.
    assert_eq!(
        reader.read(&request()).err(),
        Some(SelectedWorkRefusal::CrossedSubject)
    );
    let args: Vec<String> = reader
        .runtime
        .invocations
        .first()
        .map(|invocation| {
            invocation
                .args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        args,
        [
            "selected-work",
            "sample",
            "--root",
            "/repository",
            "--policy",
            "/repository/cairn-policy.json",
            "--disclosure",
            "portable",
            "--obligation-definition",
            "verify",
            "--obligation-observations",
            ".kiln/observations.json",
        ]
    );

    // A moved executable is refused before any process runs.
    let mut moved = ScriptedRuntime::new(Vec::new());
    moved.identity = cid_of(62);
    let mut reader =
        CairnSelectedWorkReader::new(profile(), moved).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        reader.read(&request()).err(),
        Some(SelectedWorkRefusal::Unavailable)
    );
    assert!(reader.runtime.invocations.is_empty());
    let mut escaping = request();
    escaping.obligation_observations = Some(PathBuf::from("../observations.json"));
    assert_eq!(
        reader.read(&escaping).err(),
        Some(SelectedWorkRefusal::InvalidSelection)
    );
    assert_eq!(
        SelectedWorkRefusal::from_stderr(b"error: selected-work rejected: InvalidObligations"),
        SelectedWorkRefusal::InvalidObligations
    );
}

/// Retained producer wire and consumer mappings for the pinned producer. Set
/// `KILN_UPDATE_SELECTED_WORK_FIXTURES=1` after an intentional producer pin
/// change to rewrite them.
#[test]
fn retained_fixtures_match_the_pinned_producer() {
    let (pilot, bound) = pilot();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIRECTORY);
    let consumer = |wire: &[u8]| {
        let mut bytes = serde_json::to_vec_pretty(&admit(wire)).unwrap_or_else(|_| unreachable!());
        bytes.push(b'\n');
        bytes
    };
    let mut contract_bytes = serde_json::to_vec_pretty(&bound).unwrap_or_else(|_| unreachable!());
    contract_bytes.push(b'\n');
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, wire) in [
        ("planning", &pilot.planning),
        ("passing", &pilot.passing),
        ("missing-check", &pilot.missing_check),
        ("stale-subject", &pilot.stale_subject),
        ("changed-policy", &pilot.changed_policy),
        ("interrupted-observation", &pilot.interrupted_observation),
    ] {
        files.push((format!("{name}.producer.json"), wire.clone()));
        files.push((format!("{name}.admitted.json"), consumer(wire)));
    }
    files.push(("planning.contract.json".into(), contract_bytes));
    let update = std::env::var_os(UPDATE_FIXTURES).is_some();
    for (name, bytes) in files {
        let path = directory.join(&name);
        if update {
            std::fs::create_dir_all(&directory).unwrap_or_else(|_| unreachable!());
            std::fs::write(&path, &bytes).unwrap_or_else(|_| unreachable!());
        }
        let retained = std::fs::read(&path).unwrap_or_default();
        assert!(retained == bytes, "{name} drifted from the pinned producer");
    }
    // The retained contract still verifies from its own bytes.
    let retained: AttemptSnapshotContract = serde_json::from_slice(
        &std::fs::read(directory.join("planning.contract.json")).unwrap_or_default(),
    )
    .unwrap_or_else(|_| unreachable!());
    retained.verify().unwrap_or_else(|_| unreachable!());
}
