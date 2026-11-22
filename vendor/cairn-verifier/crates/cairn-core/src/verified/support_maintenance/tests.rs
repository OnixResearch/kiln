use super::*;

use std::collections::{BTreeMap, BTreeSet};

use super::super::content_bound_requirement_registry::{
    ContentBoundRequirementRow, ContentBoundSourceSpan,
};
use super::super::scoped_claim_derivations::{
    Proposition, CLAIM_SCHEMA_V1, INDEPENDENCE_NOT_ESTABLISHED,
};

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn row(tag: &str) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: format!("cairn.test.support.{tag}"),
        specification_path: format!(".cairn/specs/support/{tag}.md"),
        specification_blake3: HASH_A.into(),
        source_span: ContentBoundSourceSpan {
            start_byte: 0,
            end_byte: 64,
            start_line: 1,
            end_line: 1,
        },
        requirement_body_blake3: HASH_B.into(),
        row_blake3: HASH_C.into(),
    }
}

fn context(policy_blake3: &str) -> ClaimContext {
    ClaimContext {
        subject_blake3: HASH_A.into(),
        policy_blake3: policy_blake3.into(),
        target: "onixresearch/cairn".into(),
        environment: "linux-x86_64".into(),
        bounds: BTreeMap::from([("max_checks".to_string(), "4".to_string())]),
    }
}

fn claim(tag: &str, proposition: Proposition, policy_blake3: &str) -> ScopedClaim {
    ScopedClaim {
        schema: CLAIM_SCHEMA_V1.into(),
        kind: proposition.claim_kind(),
        requirement: row(tag),
        interpretation_blake3: HASH_B.into(),
        contract_blake3: HASH_C.into(),
        context: context(policy_blake3),
        proposition,
    }
}

fn observation(check_id: &str) -> Proposition {
    Proposition::CheckPassed {
        check_id: check_id.into(),
    }
}

fn id(claim: &ScopedClaim) -> String {
    claim_identity(claim).unwrap()
}

fn support(id: &str, claim: &str) -> EvidenceRecord {
    EvidenceRecord {
        id: id.into(),
        claim: claim.into(),
        polarity: EvidencePolarity::Support,
        rule: "native-check-observation.v1".into(),
        version: 1,
        assurance_scope: "exact saved observation".into(),
        assumptions: BTreeSet::new(),
    }
}

fn counter(id: &str, claim: &str) -> EvidenceRecord {
    EvidenceRecord {
        id: id.into(),
        claim: claim.into(),
        polarity: EvidencePolarity::Counterevidence,
        rule: "native-check-observation.v1".into(),
        version: 1,
        assurance_scope: "exact saved counter-observation".into(),
        assumptions: BTreeSet::new(),
    }
}

fn branch(id: &str, conclusion: &str, premises: &[&str], assumptions: &[&str]) -> SupportBranch {
    SupportBranch {
        id: id.into(),
        conclusion: conclusion.into(),
        polarity: BranchPolarity::Support,
        rule: "reviewed_completion_v1".into(),
        version: 1,
        premises: premises
            .iter()
            .map(|premise| (*premise).to_string())
            .collect(),
        assumptions: assumptions
            .iter()
            .map(|assumption| (*assumption).to_string())
            .collect(),
        assurance_scope: "bounded derivation".into(),
        applicability: context(HASH_B),
        cross_revision_relation: None,
    }
}

fn change(id: &str, sequence: u64, change: Change) -> ChangeEvent {
    ChangeEvent {
        id: id.into(),
        sequence,
        reason: format!("test change {id}"),
        change,
    }
}

fn policy() -> SupportPolicy {
    SupportPolicy {
        schema: SUPPORT_POLICY_SCHEMA_V1.into(),
        change_kinds: [
            "evidence_stale",
            "evidence_withdrawn",
            "assumption_withdrawn",
            "evidence_admitted",
            "claim_revised",
            "policy_changed",
            "context_changed",
        ]
        .iter()
        .map(|kind| (*kind).into())
        .collect(),
        bounds: SupportBounds {
            max_claims: 32,
            max_branches: 32,
            max_evidence: 32,
            max_premises: 4,
            max_assumptions: 32,
            max_events: 16,
            max_obligations: 16,
            max_depth: 8,
        },
    }
}

fn empty_case() -> SupportCase {
    SupportCase {
        schema: SUPPORT_CASE_SCHEMA_V1.into(),
        claims: Vec::new(),
        branches: Vec::new(),
        evidence: Vec::new(),
        changes: Vec::new(),
        obligations: Vec::new(),
    }
}

fn find_branch<'a>(report: &'a MaintenanceReport, id: &str) -> &'a BranchExplanation {
    report
        .branches
        .iter()
        .find(|branch| branch.id == id)
        .expect("branch explanation exists")
}

fn find_claim<'a>(report: &'a MaintenanceReport, id: &str) -> &'a ClaimMaintenance {
    report
        .claims
        .iter()
        .find(|claim| claim.claim_id == id)
        .expect("claim explanation exists")
}

#[test]
// r[verify cairn.support_maintenance.alternatives]
fn withdrawing_a_unique_premise_keeps_the_alternative_support() {
    let a = claim("shared", observation("a"), HASH_B);
    let b = claim("shared", observation("b"), HASH_B);
    let c = claim("shared", observation("c"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("a", id(&a)), ("b", id(&b)), ("c", id(&c)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![a, b, c, d];
    case.branches = vec![
        branch("branch.ab", &ids["d"], &[&ids["a"], &ids["b"]], &[]),
        branch("branch.c", &ids["d"], &[&ids["c"]], &[]),
    ];
    case.evidence = vec![
        support("evidence.a", &ids["a"]),
        support("evidence.b", &ids["b"]),
        support("evidence.c", &ids["c"]),
    ];
    case.changes = vec![change(
        "stale-b",
        1,
        Change::EvidenceStale {
            evidence: "evidence.b".into(),
        },
    )];
    case.obligations = vec![LifecycleObligation {
        id: "obligation.d".into(),
        claim: ids["d"].clone(),
        description: "completion obligation for d".into(),
    }];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let d = find_claim(&report, &ids["d"]);
    assert_eq!(d.baseline_state, ClaimState::Supported);
    assert_eq!(d.state, ClaimState::Supported);
    assert_eq!(d.surviving_branches, vec!["branch.c".to_string()]);
    assert_eq!(d.withdrawn_branches, vec!["branch.ab".to_string()]);
    assert!(d.counterevidence.is_empty());

    let withdrawn = find_branch(&report, "branch.ab");
    assert_eq!(withdrawn.status, BranchStatus::Withdrawn);
    assert!(withdrawn.link.is_none());
    let cause = withdrawn
        .cause
        .as_ref()
        .expect("historical reason retained");
    assert_eq!(cause.origin.kind, "evidence_stale");
    assert_eq!(cause.origin.subject, "evidence.b");
    assert_eq!(cause.origin.event.as_ref().unwrap().sequence, 1);
    assert!(cause.origin.detail.contains("historical observation"));

    let survived = find_branch(&report, "branch.c");
    assert_eq!(survived.status, BranchStatus::Survived);
    assert!(survived
        .discharge_paths
        .iter()
        .any(|path| path.ends_with("via evidence.c")));
    assert!(survived.link.is_some());

    let impact = report
        .affected_obligations
        .iter()
        .find(|impact| impact.id == "obligation.d")
        .unwrap();
    assert_eq!(impact.status, "alternative_withdrawn");
    assert!(report.affected_claims.contains(&ids["d"]));
}

#[test]
// r[verify cairn.support_maintenance.assumptions]
fn withdrawing_a_shared_assumption_withdraws_every_dependent_branch() {
    let a = claim("shared", observation("a"), HASH_B);
    let b = claim("shared", observation("b"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("a", id(&a)), ("b", id(&b)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![a, b, d];
    case.branches = vec![
        branch("branch.a", &ids["d"], &[&ids["a"]], &["shared-oracle"]),
        branch("branch.b", &ids["d"], &[&ids["b"]], &["shared-oracle"]),
    ];
    case.evidence = vec![
        support("evidence.a", &ids["a"]),
        support("evidence.b", &ids["b"]),
    ];
    case.changes = vec![change(
        "withdraw-shared",
        2,
        Change::AssumptionWithdrawn {
            assumption: "shared-oracle".into(),
        },
    )];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let d = find_claim(&report, &ids["d"]);
    assert_eq!(d.baseline_state, ClaimState::Supported);
    assert_eq!(d.state, ClaimState::Unresolved);
    assert_eq!(
        d.withdrawn_branches,
        vec!["branch.a".to_string(), "branch.b".to_string()]
    );
    assert!(d.support.is_empty());
    assert!(d.counterevidence.is_empty());
    for branch_id in ["branch.a", "branch.b"] {
        let withdrawn = find_branch(&report, branch_id);
        let cause = withdrawn.cause.as_ref().unwrap();
        assert_eq!(cause.origin.kind, "assumption_withdrawn");
        assert_eq!(cause.origin.subject, "shared-oracle");
    }
}

#[test]
// r[verify cairn.support_maintenance.invalidation]
fn stale_evidence_is_inapplicable_and_not_refutation() {
    let a = claim("shared", observation("a"), HASH_B);
    let ids = BTreeMap::from([("a", id(&a))]);
    let mut case = empty_case();
    case.claims = vec![a];
    case.evidence = vec![support("receipt.a", &ids["a"])];
    case.changes = vec![change(
        "stale-a",
        3,
        Change::EvidenceStale {
            evidence: "receipt.a".into(),
        },
    )];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let a = find_claim(&report, &ids["a"]);
    assert_eq!(a.baseline_state, ClaimState::Supported);
    assert_eq!(a.state, ClaimState::Unresolved);
    assert_ne!(a.state, ClaimState::Refuted);
    assert!(a.support.is_empty());
    assert!(a.counterevidence.is_empty());

    let evidence = report
        .evidence
        .iter()
        .find(|evidence| evidence.evidence.id == "receipt.a")
        .unwrap();
    assert!(evidence.historical);
    assert!(!evidence.effective);
    assert_eq!(
        evidence.cause.as_ref().unwrap().origin.kind,
        "evidence_stale"
    );
    assert!(report.newly_unresolved.contains(&ids["a"]));
}

#[test]
// r[verify cairn.support_maintenance.dispute]
fn declared_counterevidence_remains_visible_when_another_branch_succeeds() {
    let c = claim("shared", observation("c"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("c", id(&c)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![c, d];
    case.branches = vec![branch("branch.c", &ids["d"], &[&ids["c"]], &[])];
    case.evidence = vec![support("evidence.c", &ids["c"])];
    case.changes = vec![change(
        "admit-counterevidence",
        4,
        Change::EvidenceAdmitted {
            evidence: counter("evidence.counter", &ids["d"]),
        },
    )];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let d = find_claim(&report, &ids["d"]);
    assert_eq!(d.baseline_state, ClaimState::Supported);
    assert_eq!(d.state, ClaimState::Disputed);
    assert_eq!(d.support.len(), 1);
    assert_eq!(d.counterevidence.len(), 1);
    assert_eq!(d.counterevidence[0].source, "evidence.counter");
    assert_eq!(d.surviving_branches, vec!["branch.c".to_string()]);
    assert!(find_claim(&report, &ids["d"])
        .state_reason
        .contains("does not cancel declared counterevidence"));
}

#[test]
// r[verify cairn.support_maintenance.revisions]
fn a_revision_classification_never_transfers_support_by_itself() {
    let old = claim("old", Proposition::NativeCompletionEligible, HASH_B);
    let new = claim("new", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("old", id(&old)), ("new", id(&new))]);
    assert_ne!(ids["old"], ids["new"]);

    let mut case = empty_case();
    case.claims = vec![old.clone(), new.clone()];
    case.evidence = vec![support("evidence.old", &ids["old"])];
    case.changes = vec![change(
        "revision",
        5,
        Change::ClaimRevised {
            from_claim: ids["old"].clone(),
            to_claim: ids["new"].clone(),
            classification: "equivalent".into(),
            classification_identity: HASH_D.into(),
        },
    )];
    case.obligations = vec![LifecycleObligation {
        id: "obligation.new".into(),
        claim: ids["new"].clone(),
        description: "new revision obligation".into(),
    }];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let old_view = find_claim(&report, &ids["old"]);
    let new_view = find_claim(&report, &ids["new"]);
    assert_eq!(old_view.state, ClaimState::Supported);
    assert!(!old_view.current);
    assert_eq!(old_view.superseded_by.as_deref(), Some(ids["new"].as_str()));
    assert_eq!(new_view.state, ClaimState::Unresolved);
    assert!(new_view.support.is_empty());
    assert_eq!(report.revisions.len(), 1);
    assert!(!report.revisions[0].automatic_support_transfer);
    assert_eq!(
        report
            .affected_obligations
            .iter()
            .find(|impact| impact.id == "obligation.new")
            .unwrap()
            .status,
        "support_lost"
    );

    let mut linked = case.clone();
    let mut relation = branch("branch.relation", &ids["new"], &[&ids["old"]], &[]);
    relation.cross_revision_relation = Some(HASH_D.into());
    linked.branches = vec![relation];
    let report = evaluate_support_maintenance(&policy(), &linked).unwrap();
    let new_view = find_claim(&report, &ids["new"]);
    assert_eq!(new_view.state, ClaimState::Supported);
    assert_eq!(new_view.support[0].source, "branch.relation");
    assert_eq!(
        find_branch(&report, "branch.relation").status,
        BranchStatus::Survived
    );

    let mut undeclared = case.clone();
    let mut relation = branch("branch.relation", &ids["new"], &[&ids["old"]], &[]);
    relation.cross_revision_relation = None;
    undeclared.branches = vec![relation];
    let error = evaluate_support_maintenance(&policy(), &undeclared).unwrap_err();
    assert!(matches!(error, DerivationError::ScopeMismatch(_)));
}

#[test]
// r[verify cairn.support_maintenance.scope]
fn policy_and_context_changes_leave_new_scopes_untouched() {
    let old = claim("old", observation("old"), HASH_B);
    let new = claim("new", observation("new"), HASH_C);
    let ids = BTreeMap::from([("old", id(&old)), ("new", id(&new))]);
    let old_context = support_context_identity(&context(HASH_B)).unwrap();
    let mut case = empty_case();
    case.claims = vec![old, new];
    case.evidence = vec![
        support("evidence.old", &ids["old"]),
        support("evidence.new", &ids["new"]),
    ];
    case.changes = vec![
        change(
            "policy",
            1,
            Change::PolicyChanged {
                previous_policy_blake3: HASH_B.into(),
            },
        ),
        change(
            "context",
            2,
            Change::ContextChanged {
                previous_context_blake3: old_context,
            },
        ),
    ];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    assert_eq!(
        find_claim(&report, &ids["old"]).state,
        ClaimState::Unresolved
    );
    assert_eq!(
        find_claim(&report, &ids["new"]).state,
        ClaimState::Supported
    );
    assert_eq!(report.events.len(), 2);
    assert!(report
        .events
        .iter()
        .any(|event| event.kind == "policy_changed"));
    assert!(report
        .events
        .iter()
        .any(|event| event.kind == "context_changed"));
}

#[test]
// r[verify cairn.support_maintenance.determinism]
fn equivalent_inputs_replay_identically_and_event_order_is_explicit() {
    let a = claim("shared", observation("a"), HASH_B);
    let b = claim("shared", observation("b"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("a", id(&a)), ("b", id(&b)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![a, b, d];
    case.branches = vec![
        branch("branch.a", &ids["d"], &[&ids["a"]], &[]),
        branch("branch.b", &ids["d"], &[&ids["b"]], &[]),
    ];
    case.evidence = vec![
        support("evidence.a", &ids["a"]),
        support("evidence.b", &ids["b"]),
    ];
    case.changes = vec![
        change(
            "first",
            1,
            Change::EvidenceStale {
                evidence: "evidence.a".into(),
            },
        ),
        change(
            "second",
            2,
            Change::EvidenceStale {
                evidence: "evidence.b".into(),
            },
        ),
    ];

    let first = evaluate_support_maintenance(&policy(), &case).unwrap();
    let mut reversed = case.clone();
    reversed.claims.reverse();
    reversed.branches.reverse();
    reversed.evidence.reverse();
    reversed.changes.reverse();
    let second = evaluate_support_maintenance(&policy(), &reversed).unwrap();
    assert_eq!(first.input_blake3, second.input_blake3);
    assert_eq!(first.report_blake3, second.report_blake3);
    assert_eq!(first.analysis.mode, "full_recomputation");
    assert!(first.analysis.complete);
    assert!(!first.analysis.incremental);
    assert_eq!(first.bounds.limits, policy().bounds);

    let mut reordered = case.clone();
    reordered.changes[0].sequence = 2;
    reordered.changes[1].sequence = 1;
    let third = evaluate_support_maintenance(&policy(), &reordered).unwrap();
    assert_eq!(
        third
            .events
            .iter()
            .map(|event| event.event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first"]
    );
    assert_ne!(first.report_blake3, third.report_blake3);
}

#[test]
// r[verify cairn.support_maintenance.bounds]
fn bounds_fail_explicitly_and_cycles_are_reported() {
    let a = claim("shared", observation("a"), HASH_B);
    let b = claim("shared", observation("b"), HASH_B);
    let c = claim("shared", observation("c"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("a", id(&a)), ("b", id(&b)), ("c", id(&c)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![a, b, c, d];
    case.branches = vec![
        branch("branch.ab", &ids["d"], &[&ids["a"], &ids["b"]], &[]),
        branch("branch.c", &ids["d"], &[&ids["c"]], &[]),
    ];
    let mut bounded = policy();
    bounded.bounds.max_branches = 1;
    let error = evaluate_support_maintenance(&bounded, &case).unwrap_err();
    assert!(matches!(error, DerivationError::BoundExhaustion(_)));

    let a = claim("cycle", observation("cycle-a"), HASH_B);
    let b = claim("cycle", observation("cycle-b"), HASH_B);
    let ids = BTreeMap::from([("a", id(&a)), ("b", id(&b))]);
    let mut cyclic = empty_case();
    cyclic.claims = vec![a, b];
    cyclic.branches = vec![
        branch("branch.ab", &ids["a"], &[&ids["b"]], &[]),
        branch("branch.ba", &ids["b"], &[&ids["a"]], &[]),
    ];
    let error = evaluate_support_maintenance(&policy(), &cyclic).unwrap_err();
    assert!(matches!(error, DerivationError::Cycle(_)));
}

#[test]
// r[verify cairn.support_maintenance.inspection]
// r[verify cairn.support_maintenance.authority]
fn advisory_boundary_and_invalid_targets_are_explicit() {
    let a = claim("shared", observation("a"), HASH_B);
    let ids = BTreeMap::from([("a", id(&a))]);
    let mut case = empty_case();
    case.claims = vec![a];
    case.evidence = vec![support("evidence.a", &ids["a"])];
    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    assert!(report.advisory);
    assert_eq!(report.boundary, SUPPORT_BOUNDARY);
    assert_eq!(report.independence, INDEPENDENCE_NOT_ESTABLISHED);
    assert!(report.boundary.contains("no automatic belief repair"));

    let mut stale = case.clone();
    stale.changes = vec![change(
        "unknown",
        1,
        Change::EvidenceStale {
            evidence: "missing".into(),
        },
    )];
    let error = evaluate_support_maintenance(&policy(), &stale).unwrap_err();
    assert!(matches!(error, DerivationError::InvalidInput(_)));

    let mut wrong_schema = case.clone();
    wrong_schema.schema = "cairn.support-maintenance-case.v2".into();
    let error = evaluate_support_maintenance(&policy(), &wrong_schema).unwrap_err();
    assert!(matches!(error, DerivationError::Unsupported(_)));
}

#[test]
// r[verify cairn.support_maintenance.alternatives]
// r[verify cairn.support_maintenance.dispute]
fn a_refuting_branch_remains_a_separate_counterevidence_alternative() {
    let c = claim("shared", observation("c"), HASH_B);
    let e = claim("shared", observation("e"), HASH_B);
    let d = claim("shared", Proposition::NativeCompletionEligible, HASH_B);
    let ids = BTreeMap::from([("c", id(&c)), ("e", id(&e)), ("d", id(&d))]);
    let mut case = empty_case();
    case.claims = vec![c, e, d];
    let support_branch = branch("branch.support", &ids["d"], &[&ids["c"]], &[]);
    let mut refute_branch = branch("branch.refute", &ids["d"], &[&ids["e"]], &[]);
    refute_branch.polarity = BranchPolarity::Refute;
    case.branches = vec![support_branch, refute_branch];
    case.evidence = vec![
        support("evidence.c", &ids["c"]),
        support("evidence.e", &ids["e"]),
    ];

    let report = evaluate_support_maintenance(&policy(), &case).unwrap();
    let d = find_claim(&report, &ids["d"]);
    assert_eq!(d.state, ClaimState::Disputed);
    assert_eq!(d.support.len(), 1);
    assert_eq!(d.counterevidence.len(), 1);
    assert_eq!(d.counterevidence[0].source, "branch.refute");
    assert_eq!(
        find_branch(&report, "branch.refute").polarity,
        BranchPolarity::Refute
    );
    assert_eq!(
        find_branch(&report, "branch.support").status,
        BranchStatus::Survived
    );
    assert_eq!(
        find_branch(&report, "branch.refute").status,
        BranchStatus::Survived
    );
}
