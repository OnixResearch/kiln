//! Deterministic consumer fixtures: the Kiln contract comes from the retained
//! Cairn producer/consumer fixture and the plan from one recorded local
//! `lattice workflow-intent compile` run. Neither is live qualification.
use std::path::Path;

use kiln_core::implementation::selected_work::{NodeSelection, ObligationSection, bind_attempt};

use super::*;

const CONTRACT: &str = "../../fixtures/cairn/selected-work/planning.contract.json";
const DIRECTORY: &str = "../../fixtures/lattice/workflow-intent";
const UPDATE: &str = "KILN_UPDATE_LATTICE_INTENT_FIXTURES";
const ALLOWED_CLASSES: [&str; 4] = [
    "cairn_validate",
    "named_check",
    "review_wait",
    "completion_only",
];

fn read(relative: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)).unwrap_or_default()
}

fn contract() -> AttemptSnapshotContract {
    serde_json::from_slice(&read(CONTRACT)).unwrap_or_else(|_| unreachable!())
}

fn request() -> IntentCompileRequest {
    serde_json::from_slice(&read(&format!("{DIRECTORY}/verify.request.json")))
        .unwrap_or_else(|_| unreachable!())
}

fn plan() -> CompiledPlanObservation {
    serde_json::from_slice(&read(&format!("{DIRECTORY}/verify.plan.json")))
        .unwrap_or_else(|_| unreachable!())
}

fn identity(value: &str) -> bool {
    value.strip_prefix("b3:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[test]
// r[verify kiln.selected_work.lattice]
fn admitted_snapshot_normalizes_to_a_complete_lattice_intent() {
    let contract = contract();
    let intent = normalize_intent(&contract.snapshot).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        intent,
        normalize_intent(&contract.snapshot).unwrap_or_else(|_| unreachable!())
    );
    assert_eq!(intent.schema, INTENT_SCHEMA);
    assert_eq!(intent.coverage, IntentCoverage::Complete);
    let snapshot = &contract.snapshot;
    assert_eq!(
        intent.cohort.selection_identity,
        prefixed(&snapshot.cohort.selection)
    );
    assert_eq!(
        intent.cohort.obligations_identity,
        prefixed(&snapshot.cohort.obligations)
    );
    assert_eq!(
        intent.cohort.context_identity,
        prefixed(&snapshot.cohort.context)
    );
    assert_eq!(
        intent.cohort.source_identity,
        prefixed(&snapshot.cohort.source)
    );
    assert_eq!(intent.cohort.policy_identity, prefixed(&snapshot.policy));
    assert_eq!(
        intent.cohort.profile_identity,
        prefixed(&snapshot.profile_identity)
    );
    assert_eq!(
        intent.cohort.dependency_identity,
        prefixed(&snapshot.cohort.dependencies)
    );
    let plan = snapshot.plan().unwrap_or_else(|| unreachable!());
    let leaves: Vec<&str> = plan
        .nodes
        .iter()
        .filter(|node| node.contributing_leaf())
        .map(|node| node.id.as_str())
        .collect();
    assert!(!leaves.is_empty());
    // Every contributing leaf, with its exact producer identities, and nothing
    // from an unselected alternative.
    for node in plan.nodes.iter() {
        let mapped = intent.obligations.iter().find(|item| item.id == node.id);
        if node.contributing_leaf() {
            let mapped = mapped.unwrap_or_else(|| unreachable!());
            assert_eq!(mapped.obligation_identity, prefixed(&node.obligation));
            assert_eq!(mapped.guidance_identity, prefixed(&node.guidance));
            assert_eq!(
                mapped.remaining_condition_identity,
                prefixed(&node.condition)
            );
            for evidence in &node.evidence {
                assert!(mapped.required_observations.iter().any(|contract| {
                    contract.id == evidence.predicate
                        && contract.predicate_identity == prefixed(&evidence.reference)
                }));
            }
        } else {
            assert!(mapped.is_none(), "{}", node.id);
        }
        if node.selection == NodeSelection::UnselectedAlternative {
            assert!(mapped.is_none());
        }
    }
    let completion = intent
        .obligations
        .iter()
        .find(|item| item.id == COMPLETION_OBLIGATION)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(completion.operation_class, "completion_only");
    let mut expected: Vec<&str> = leaves.clone();
    expected.sort_unstable();
    assert_eq!(completion.prerequisites, expected);
    // Lattice's own admission rules, checked on Kiln's side.
    assert!(
        intent
            .obligations
            .iter()
            .zip(intent.obligations.iter().skip(1))
            .all(|(left, right)| left.id < right.id)
    );
    for obligation in &intent.obligations {
        assert!(identifier(&obligation.id));
        assert!(ALLOWED_CLASSES.contains(&obligation.operation_class.as_str()));
        assert!(!obligation.required_observations.is_empty());
        assert!(obligation.required_observations.len() <= MAX_OBSERVATIONS_PER_OBLIGATION);
        for value in [
            &obligation.obligation_identity,
            &obligation.guidance_identity,
            &obligation.remaining_condition_identity,
            &obligation.action_identity,
            &obligation.precondition_identity,
        ] {
            assert!(identity(value), "{value}");
        }
        assert!(obligation.required_observations.iter().all(|contract| {
            identifier(&contract.id)
                && identifier(&contract.schema)
                && identity(&contract.predicate_identity)
        }));
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DIRECTORY)
        .join("verify.intent.json");
    let mut bytes = serde_json::to_vec_pretty(&intent).unwrap_or_else(|_| unreachable!());
    bytes.push(b'\n');
    if std::env::var_os(UPDATE).is_some() {
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| unreachable!()))
            .unwrap_or_else(|_| unreachable!());
        std::fs::write(&path, &bytes).unwrap_or_else(|_| unreachable!());
    }
    assert!(
        std::fs::read(&path).unwrap_or_default() == bytes,
        "retained intent drifted"
    );
    // The retained compile request carries exactly this intent.
    assert_eq!(request().intent, intent);
}

#[test]
fn incomplete_or_unselected_snapshots_cannot_compile() {
    let contract = contract();
    let mut missing = contract.snapshot.clone();
    if let ObligationSection::Exported(plan) = &mut missing.obligations
        && let Some(node) = plan
            .nodes
            .iter_mut()
            .find(|node| node.alternative.is_some())
    {
        node.selection = NodeSelection::SelectionMissing;
    }
    let intent = normalize_intent(&missing).unwrap_or_else(|_| unreachable!());
    assert_eq!(intent.coverage, IntentCoverage::Incomplete);
    let compiler = request().facts.compiler_artifact_identity;
    let runtime = request().facts.runtime_profile_identity;
    assert_eq!(
        compile_request(&missing, &compiler, &runtime).err(),
        Some(IntentRefusal::IncompleteCoverage)
    );
    let mut unselected = contract.snapshot.clone();
    unselected.obligations = ObligationSection::NotSelected;
    let intent = normalize_intent(&unselected).unwrap_or_else(|_| unreachable!());
    assert_eq!(intent.coverage, IntentCoverage::Incomplete);
    assert_eq!(intent.obligations.len(), 1);
    let mut partial = contract.snapshot.clone();
    partial.enumeration.complete = false;
    assert_eq!(
        normalize_intent(&partial).err(),
        Some(IntentRefusal::IncompleteCoverage)
    );
    assert_eq!(
        compile_request(&contract.snapshot, "b3:short", &runtime).err(),
        Some(IntentRefusal::InvalidIdentity)
    );
}

#[test]
// r[verify kiln.selected_work.lattice.scenario.proposal]
fn compiled_plan_binds_obligations_and_proposals_need_readmission() {
    let contract = contract();
    let request = request();
    assert_eq!(
        compile_request(
            &contract.snapshot,
            &request.facts.compiler_artifact_identity,
            &request.facts.runtime_profile_identity,
        )
        .ok()
        .as_ref(),
        Some(&request)
    );
    let plan = plan();
    let admitted = admit_compiled_plan(&contract, &request, &plan);
    assert_eq!(admitted.as_ref().err(), None);
    let admitted = admitted.unwrap_or_else(|_| unreachable!());
    assert_eq!(admitted.plan_identity, plan.identity);
    assert_eq!(admitted.contract, contract.identity);
    assert_eq!(admitted.attempt, contract.attempt);
    assert_eq!(
        admitted
            .obligations
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        plan.nodes.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(
        review_proposal(&admitted, &plan),
        ProposalReview::IdenticalToAdmitted
    );

    // A worker drops a required obligation and recompiles: never executable.
    let mut dropped = plan.clone();
    let removed = dropped.intent.obligations.remove(0);
    dropped.identity = format!("b3:{}", "1".repeat(64));
    let ProposalReview::OwnerReadmissionRequired(difference) = review_proposal(&admitted, &dropped)
    else {
        unreachable!()
    };
    assert_eq!(difference.removed, vec![removed.id]);
    // Changing an obligation identity under the same plan identity is refused too.
    let mut rewritten = plan.clone();
    if let Some(obligation) = rewritten.intent.obligations.first_mut() {
        obligation.obligation_identity = format!("b3:{}", "2".repeat(64));
    }
    let ProposalReview::OwnerReadmissionRequired(difference) =
        review_proposal(&admitted, &rewritten)
    else {
        unreachable!()
    };
    assert_eq!(difference.changed.len(), 1);
    // The plan cannot be admitted for another attempt's request or a forged echo.
    let mut forged = plan.clone();
    forged.disposition = "execute".into();
    assert_eq!(
        admit_compiled_plan(&contract, &request, &forged).err(),
        Some(IntentRefusal::InvalidPlan)
    );
    let mut echoed = plan.clone();
    echoed.intent.obligations.pop();
    assert_eq!(
        admit_compiled_plan(&contract, &request, &echoed).err(),
        Some(IntentRefusal::BindingMismatch)
    );
    let mut other = contract.snapshot.clone();
    other.cohort.source =
        ContentId::from_blake3_hex("3".repeat(64)).unwrap_or_else(|_| unreachable!());
    let rebound = bind_attempt(
        contract.attempt.clone(),
        contract.work.clone(),
        contract.target.clone(),
        other,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_compiled_plan(&rebound, &request, &plan).err(),
        Some(IntentRefusal::BindingMismatch)
    );
}
