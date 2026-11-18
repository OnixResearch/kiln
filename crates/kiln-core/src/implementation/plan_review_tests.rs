//! Pinned plans, annotations, the plan gate, feedback, comparison, opt-in
//! work eligibility and the frozen comparison receipt.
use super::*;
use crate::implementation::{
    AcceptanceObligation, BlockReason, Eligibility, Enrollment, IMPLEMENTATION_WORK_SCHEMA,
    SourceRef, TerminalPolicy, WorkBudget, WorkLedger, WorkSourceKind,
};
use crate::types::ExternalRef;
use alloc::format;
use alloc::vec;

const REVIEWER: &str = "reviewer";
const AUTHOR: &str = "agent";
const REPOSITORY: &str = "onix/kiln";

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn code<T: core::fmt::Debug>(result: CoreResult<T>) -> String {
    match result {
        Ok(value) => format!("ok {value:?}"),
        Err(error) => error.code,
    }
}

fn reviewers() -> BTreeSet<String> {
    BTreeSet::from([REVIEWER.to_string()])
}

fn steps(pairs: &[(&str, &str)]) -> Vec<PlanStep> {
    pairs
        .iter()
        .map(|(id, text)| PlanStep {
            id: (*id).to_string(),
            text: (*text).to_string(),
        })
        .collect()
}

fn plan(
    work: &ContentId,
    revision: u32,
    previous: Option<ContentId>,
    pairs: &[(&str, &str)],
) -> ImplementationPlan {
    ImplementationPlan {
        schema: PLAN_SCHEMA.to_string(),
        work_identity: work.clone(),
        revision,
        previous,
        author: AUTHOR.to_string(),
        steps: steps(pairs),
    }
}

fn identity(plan: &ImplementationPlan) -> ContentId {
    plan.identity().unwrap_or_else(|_| unreachable!())
}

const FIRST: [(&str, &str); 3] = [
    ("types", "Add the plan type."),
    ("push", "Force-push every branch."),
    ("merge", "Merge the stack."),
];

fn region(step: &str, span: Option<(u32, u32)>) -> PlanRegion {
    PlanRegion {
        step: step.to_string(),
        span: span.map(|(start, end)| TextSpan { start, end }),
    }
}

fn annotation(
    id: &str,
    plan: &ContentId,
    region: PlanRegion,
    note: PlanNote,
    blocking: bool,
) -> PlanAnnotation {
    PlanAnnotation {
        id: id.to_string(),
        plan: plan.clone(),
        reviewer: REVIEWER.to_string(),
        region,
        note,
        blocking,
    }
}

fn comment(text: &str) -> PlanNote {
    PlanNote::Comment {
        text: text.to_string(),
    }
}

fn decision(plan: &ContentId, verdict: PlanVerdict) -> PlanDecision {
    PlanDecision {
        plan: plan.clone(),
        reviewer: REVIEWER.to_string(),
        verdict,
    }
}

fn resolution(annotation: &str, plan: &ContentId) -> PlanResolution {
    PlanResolution {
        annotation: annotation.to_string(),
        plan: plan.clone(),
        reviewer: REVIEWER.to_string(),
    }
}

/// A thread holding revision 1 of `FIRST` for the work identity `content('a')`.
fn thread() -> (PlanThread, ContentId) {
    let mut thread = PlanThread::default();
    let first = plan(&content('a'), 1, None, &FIRST);
    let identity = thread
        .propose(&content('a'), &first)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    (thread, identity)
}

// r[verify kiln.implementation.plan_review.scenario.refused]
#[test]
fn plans_are_pinned_into_one_linear_thread_per_work_version() {
    let work = content('a');
    let (mut thread, first) = thread();
    let again = plan(&work, 1, None, &FIRST);
    assert_eq!(thread.propose(&work, &again), Ok(first.clone()));
    assert_eq!(thread.plans.len(), 1);
    let second = plan(
        &work,
        2,
        Some(first.clone()),
        &[("types", "Add the plan type.")],
    );
    let second_identity = thread
        .propose(&work, &second)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_ne!(second_identity, first);
    let before = thread.clone();
    let mut duplicate_steps = plan(&work, 3, Some(second_identity.clone()), &FIRST);
    if let Some(step) = duplicate_steps.steps.get_mut(1) {
        step.id = "types".to_string();
    }
    let refused = [
        (
            "fork",
            plan(&work, 2, Some(first.clone()), &[("other", "Other.")]),
        ),
        (
            "skip",
            plan(&work, 4, Some(second_identity.clone()), &FIRST),
        ),
        (
            "wrong previous",
            plan(&work, 3, Some(first.clone()), &FIRST),
        ),
        (
            "other work",
            plan(&content('b'), 3, Some(second_identity.clone()), &FIRST),
        ),
        ("first with previous", plan(&work, 1, Some(first), &FIRST)),
        ("duplicate steps", duplicate_steps),
        (
            "no steps",
            plan(&work, 3, Some(second_identity.clone()), &[]),
        ),
        (
            "blank step",
            plan(&work, 3, Some(second_identity.clone()), &[("types", "  ")]),
        ),
        (
            "oversized step",
            plan(
                &work,
                3,
                Some(second_identity.clone()),
                &[("types", "x".repeat(MAX_PLAN_TEXT_BYTES + 1).as_str())],
            ),
        ),
        ("schema", {
            let mut other = plan(&work, 3, Some(second_identity), &FIRST);
            other.schema = "kiln.other.v1".to_string();
            other
        }),
    ];
    for (name, candidate) in refused {
        assert!(thread.propose(&work, &candidate).is_err(), "{name} pinned");
        assert_eq!(thread, before, "{name} changed the thread");
    }
}

// r[verify kiln.implementation.plan_review.scenario.refused]
#[test]
fn annotations_name_an_existing_region_of_the_latest_plan() {
    let (mut thread, first) = thread();
    let whole = annotation(
        "whole",
        &first,
        region("push", None),
        comment("Why?"),
        false,
    );
    let span = annotation(
        "span",
        &first,
        region("push", Some((0, 10))),
        PlanNote::Replace {
            text: "Push".to_string(),
        },
        true,
    );
    let delete = annotation(
        "delete",
        &first,
        region("merge", None),
        PlanNote::Delete,
        true,
    );
    for accepted in [&whole, &span, &delete] {
        assert_eq!(thread.annotate(&reviewers(), accepted), Ok(()));
    }
    let before = thread.clone();
    let mut by_author = annotation(
        "author",
        &first,
        region("push", None),
        comment("Mine."),
        false,
    );
    by_author.reviewer = AUTHOR.to_string();
    let mut outsider = by_author.clone();
    outsider.id = "outsider".to_string();
    outsider.reviewer = "someone".to_string();
    let refused = [
        ("duplicate id", whole.clone()),
        (
            "missing step",
            annotation(
                "missing",
                &first,
                region("absent", None),
                comment("?"),
                false,
            ),
        ),
        (
            "span past the end",
            annotation(
                "past",
                &first,
                region("types", Some((0, 99))),
                comment("?"),
                false,
            ),
        ),
        (
            "empty span",
            annotation(
                "empty",
                &first,
                region("types", Some((3, 3))),
                comment("?"),
                false,
            ),
        ),
        (
            "partial deletion",
            annotation(
                "partial",
                &first,
                region("push", Some((0, 5))),
                PlanNote::Delete,
                true,
            ),
        ),
        (
            "blank comment",
            annotation("blank", &first, region("push", None), comment(" "), false),
        ),
        (
            "empty replacement",
            annotation(
                "replace",
                &first,
                region("push", None),
                PlanNote::Replace {
                    text: String::new(),
                },
                false,
            ),
        ),
        (
            "unknown plan",
            annotation(
                "unknown",
                &content('c'),
                region("push", None),
                comment("?"),
                false,
            ),
        ),
        ("plan author", by_author),
        ("reviewer outside the policy", outsider),
    ];
    for (name, candidate) in refused {
        assert!(
            thread.annotate(&reviewers(), &candidate).is_err(),
            "{name} admitted"
        );
        assert_eq!(thread, before, "{name} changed the thread");
    }
    // A span must end on a character boundary.
    let mut accented = PlanThread::default();
    let unicode = plan(&content('a'), 1, None, &[("step", "Résumé the work.")]);
    let pinned = accented
        .propose(&content('a'), &unicode)
        .unwrap_or_else(|_| unreachable!());
    let split = annotation(
        "split",
        &pinned,
        region("step", Some((0, 2))),
        comment("?"),
        false,
    );
    assert!(accented.annotate(&reviewers(), &split).is_err());
    // Only the latest revision is annotated.
    let second = plan(
        &content('a'),
        2,
        Some(first.clone()),
        &[("types", "Add the plan type.")],
    );
    thread
        .propose(&content('a'), &second)
        .unwrap_or_else(|_| unreachable!());
    let stale = annotation("stale", &first, region("push", None), comment("?"), false);
    assert!(thread.annotate(&reviewers(), &stale).is_err());
}

// r[verify kiln.implementation.plan_review.scenario.blocked]
#[test]
fn acceptance_needs_every_blocking_annotation_resolved_on_the_latest_revision() {
    let work = content('a');
    assert_eq!(PlanThread::default().gate(), Ok(PlanGate::NoPlan));
    let (mut thread, first) = thread();
    assert_eq!(
        thread.gate(),
        Ok(PlanGate::Unreviewed {
            plan: first.clone()
        })
    );
    let blocking = annotation(
        "force",
        &first,
        region("push", None),
        PlanNote::Delete,
        true,
    );
    let advisory = annotation(
        "note",
        &first,
        region("types", None),
        comment("Fine."),
        false,
    );
    for item in [&blocking, &advisory] {
        thread
            .annotate(&reviewers(), item)
            .unwrap_or_else(|_| unreachable!());
    }
    let blocked = PlanGate::Unresolved {
        annotations: vec!["force".to_string()],
    };
    assert_eq!(thread.gate(), Ok(blocked.clone()));
    assert_eq!(
        code(thread.decide(&reviewers(), &decision(&first, PlanVerdict::Accepted))),
        "implementation_plan_decision"
    );
    assert_eq!(
        thread.decide(
            &reviewers(),
            &decision(&first, PlanVerdict::ChangesRequested)
        ),
        Ok(blocked)
    );
    let second = plan(
        &work,
        2,
        Some(first.clone()),
        &[("types", "Add the plan type.")],
    );
    let revised = thread
        .propose(&work, &second)
        .unwrap_or_else(|_| unreachable!());
    let mut by_author = resolution("force", &revised);
    by_author.reviewer = AUTHOR.to_string();
    for (name, refused) in [
        ("old revision", resolution("force", &first)),
        ("advisory", resolution("note", &revised)),
        ("unknown", resolution("absent", &revised)),
        ("author", by_author),
    ] {
        assert!(
            thread.resolve(&reviewers(), &refused).is_err(),
            "{name} resolved"
        );
    }
    thread
        .resolve(&reviewers(), &resolution("force", &revised))
        .unwrap_or_else(|_| unreachable!());
    assert!(
        thread
            .resolve(&reviewers(), &resolution("force", &revised))
            .is_err(),
        "resolved twice"
    );
    assert_eq!(
        thread.gate(),
        Ok(PlanGate::Unreviewed {
            plan: revised.clone()
        })
    );
    assert!(
        thread
            .decide(&reviewers(), &decision(&first, PlanVerdict::Accepted))
            .is_err(),
        "an older revision was accepted"
    );
    assert_eq!(
        thread.decide(&reviewers(), &decision(&revised, PlanVerdict::Accepted)),
        Ok(PlanGate::Accepted {
            plan: revised.clone()
        })
    );
    // New blocking feedback after acceptance closes the gate again.
    let late = annotation(
        "late",
        &revised,
        region("types", None),
        comment("Wait."),
        true,
    );
    thread
        .annotate(&reviewers(), &late)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        thread.gate(),
        Ok(PlanGate::Unresolved {
            annotations: vec!["late".to_string()],
        })
    );
}

// r[verify kiln.implementation.plan_review.scenario.revision]
#[test]
fn feedback_quotes_every_annotation_and_compares_the_revision() {
    let work = content('a');
    let (mut thread, first) = thread();
    let notes = [
        annotation(
            "push",
            &first,
            region("push", Some((0, 10))),
            PlanNote::Replace {
                text: "Push".to_string(),
            },
            true,
        ),
        annotation(
            "merge",
            &first,
            region("merge", None),
            PlanNote::Delete,
            true,
        ),
        annotation(
            "types",
            &first,
            region("types", None),
            comment("Good."),
            false,
        ),
    ];
    for note in &notes {
        thread
            .annotate(&reviewers(), note)
            .unwrap_or_else(|_| unreachable!());
    }
    let second = plan(
        &work,
        2,
        Some(first.clone()),
        &[
            ("types", "Add the plan type."),
            ("push", "Push each absent branch."),
            ("check", "Read every branch first."),
        ],
    );
    let revised = thread
        .propose(&work, &second)
        .unwrap_or_else(|_| unreachable!());
    thread
        .resolve(&reviewers(), &resolution("push", &revised))
        .unwrap_or_else(|_| unreachable!());
    let feedback = thread
        .feedback()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(
        (feedback.plan.clone(), feedback.revision),
        (revised.clone(), 2)
    );
    assert_eq!(
        feedback.gate,
        PlanGate::Unresolved {
            annotations: vec!["merge".to_string()],
        }
    );
    let summary: Vec<(&str, u32, &str, bool, Option<u32>)> = feedback
        .items
        .iter()
        .map(|item| {
            (
                item.annotation.as_str(),
                item.revision,
                item.quoted.as_str(),
                item.blocking,
                item.resolved_in,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("push", 1, "Force-push", true, Some(2)),
            ("merge", 1, "Merge the stack.", true, None),
            ("types", 1, "Add the plan type.", false, None),
        ]
    );
    assert_eq!(
        feedback.comparison,
        Some(PlanComparison {
            previous: first.clone(),
            current: revised,
            kept: vec!["types".to_string()],
            changed: vec!["push".to_string()],
            added: vec!["check".to_string()],
            removed: vec!["merge".to_string()],
            reordered: false,
        })
    );
    let reordered = plan(
        &work,
        2,
        Some(first.clone()),
        &[
            ("push", "Force-push every branch."),
            ("types", "Add the plan type."),
        ],
    );
    let original = plan(&work, 1, None, &FIRST);
    let comparison = compare_plans(&original, &reordered).unwrap_or_else(|_| unreachable!());
    assert!(comparison.reordered);
    assert_eq!(
        comparison.kept,
        vec!["push".to_string(), "types".to_string()]
    );
    assert_eq!(identity(&original), first);
}

fn alias(native_id: &str) -> SourceRef {
    SourceRef {
        kind: WorkSourceKind::GitHubIssue,
        source: ExternalRef::new("github", "issue", native_id).unwrap_or_else(|_| unreachable!()),
        native_id: native_id.to_string(),
        version: content('1'),
    }
}

fn work(outcome: &str, revision: u64) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision,
        actor: "operator".to_string(),
        repository: REPOSITORY.to_string(),
        project: "kiln".to_string(),
        base: None,
        title: "Review the plan first".to_string(),
        requested_outcome: outcome.to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "the reviewed plan is implemented".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates/kiln-core".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 10,
        aliases: vec![alias("93")],
    }
}

fn ledger(work: &ImplementationWork) -> WorkLedger {
    let mut ledger = WorkLedger::default();
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    ledger
        .admit(work, None, &enrollment)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    ledger
}

fn policy(repository: &str) -> PlanReviewCommand {
    PlanReviewCommand::Configure {
        policy: Some(PlanReviewPolicy {
            schema: PLAN_REVIEW_POLICY_SCHEMA.to_string(),
            repositories: BTreeSet::from([repository.to_string()]),
            reviewers: reviewers(),
        }),
    }
}

fn eligibility(ledger: &WorkLedger, work: &ImplementationWork) -> Option<Eligibility> {
    ledger.eligibility_of(&work.canonical_key().unwrap_or_else(|_| unreachable!()))
}

// r[verify kiln.implementation.plan_review.scenario.default]
#[test]
fn admission_and_the_stored_ledger_are_unchanged_without_a_selecting_policy() {
    let current = work("Implement plan review", 1);
    let mut ledger = ledger(&current);
    let stored = serde_json::to_string(&ledger).unwrap_or_else(|_| unreachable!());
    assert!(
        !stored.contains("\"plan_review\"") && !stored.contains("\"plan_threads\""),
        "{stored}"
    );
    assert_eq!(eligibility(&ledger, &current), Some(Eligibility::Ready));
    let identity = current.identity().unwrap_or_else(|_| unreachable!());
    let proposal = PlanReviewCommand::Propose {
        plan: plan(&identity, 1, None, &FIRST),
    };
    assert_eq!(
        code(ledger.apply_plan_review(&proposal)),
        "implementation_plan_review_disabled"
    );
    ledger
        .apply_plan_review(&policy("onix/other"))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(eligibility(&ledger, &current), Some(Eligibility::Ready));
    assert_eq!(ledger.ranked().len(), 1);
    assert_eq!(
        code(ledger.apply_plan_review(&proposal)),
        "implementation_plan_work"
    );
    ledger
        .apply_plan_review(&PlanReviewCommand::Configure { policy: None })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        serde_json::to_string(&ledger).unwrap_or_else(|_| unreachable!()),
        stored
    );
    let mut invalid = policy(REPOSITORY);
    if let PlanReviewCommand::Configure {
        policy: Some(policy),
    } = &mut invalid
    {
        policy.reviewers.clear();
    }
    assert_eq!(
        code(ledger.apply_plan_review(&invalid)),
        "implementation_plan_policy"
    );
}

// r[verify kiln.implementation.plan_review] r[verify kiln.implementation.plan_review.scenario.blocked]
#[test]
fn selected_work_waits_for_an_accepted_plan_of_its_exact_version() {
    let current = work("Implement plan review", 1);
    let identity = current.identity().unwrap_or_else(|_| unreachable!());
    let mut ledger = ledger(&current);
    ledger
        .apply_plan_review(&policy(REPOSITORY))
        .unwrap_or_else(|_| unreachable!());
    let blocked = Some(Eligibility::Blocked {
        reasons: vec![BlockReason::PlanNotAccepted],
    });
    assert_eq!(eligibility(&ledger, &current), blocked);
    assert!(ledger.ranked().is_empty());
    let first = plan(&identity, 1, None, &FIRST);
    let pinned = match ledger.apply_plan_review(&PlanReviewCommand::Propose { plan: first }) {
        Ok(PlanReviewResult::Proposed { plan }) => plan,
        other => unreachable!("{other:?}"),
    };
    assert_eq!(eligibility(&ledger, &current), blocked);
    let mut by_author = decision(&pinned, PlanVerdict::Accepted);
    by_author.reviewer = AUTHOR.to_string();
    let before = ledger.clone();
    assert_eq!(
        code(ledger.apply_plan_review(&PlanReviewCommand::Decide {
            work: identity.clone(),
            decision: by_author,
        })),
        "implementation_plan_reviewer"
    );
    assert_eq!(ledger, before);
    assert_eq!(
        ledger.apply_plan_review(&PlanReviewCommand::Decide {
            work: identity.clone(),
            decision: decision(&pinned, PlanVerdict::Accepted),
        }),
        Ok(PlanReviewResult::Reviewed {
            gate: PlanGate::Accepted {
                plan: pinned.clone()
            },
        })
    );
    assert_eq!(eligibility(&ledger, &current), Some(Eligibility::Ready));
    assert_eq!(ledger.ranked().len(), 1);
    assert_eq!(
        ledger
            .plan_feedback(&identity)
            .map(|feedback| feedback.gate),
        Ok(PlanGate::Accepted { plan: pinned })
    );
    // A new version of the same work needs its own accepted plan.
    let next = work("Implement plan review with feedback", 2);
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    ledger
        .admit(&next, None, &enrollment)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(eligibility(&ledger, &next), blocked);
    assert_eq!(
        code(ledger.apply_plan_review(&PlanReviewCommand::Propose {
            plan: plan(&identity, 2, None, &FIRST),
        })),
        "implementation_plan_work"
    );
}

const CASES: &str = include_str!("../../../../fixtures/plan-review/cases.json");
const EXPECTED: &str = include_str!("../../../../fixtures/plan-review/expected.json");

fn cases() -> PlanCases {
    serde_json::from_str(CASES).unwrap_or_else(|error| unreachable!("{error:?}"))
}

// r[verify kiln.implementation.plan_review.comparison] r[verify kiln.implementation.plan_review.comparison.scenario.frozen]
#[test]
fn frozen_cases_produce_the_committed_receipt() {
    let receipt = plan_review_receipt(&cases()).unwrap_or_else(|error| unreachable!("{error:?}"));
    let expected: serde_json::Value =
        serde_json::from_str(EXPECTED).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        serde_json::to_value(&receipt).unwrap_or_else(|_| unreachable!()),
        expected
    );
    let outcomes: Vec<(&str, String)> = receipt
        .results
        .iter()
        .map(|result| {
            let outcome = match &result.outcome {
                CaseOutcome::Changed { .. } => "changed".to_string(),
                CaseOutcome::Prevented { .. } => "prevented".to_string(),
                CaseOutcome::Unchanged { .. } => "unchanged".to_string(),
                CaseOutcome::Unavailable { reason } => format!("unavailable {reason:?}"),
            };
            (result.id.as_str(), outcome)
        })
        .collect();
    assert_eq!(
        outcomes,
        vec![
            ("overwrite-branch", "changed".to_string()),
            ("unverified-layers", "prevented".to_string()),
            ("comment-only", "unchanged".to_string()),
            ("never-reviewed", "unavailable NotReviewed".to_string()),
            ("captured-agent-plan", "unavailable MissingPlan".to_string()),
        ]
    );
    assert_eq!(
        receipt.counts,
        CaseCounts {
            changed: 1,
            prevented: 1,
            unchanged: 1,
            unavailable: 2,
        }
    );
    type Change = fn(&mut PlanCases);
    let refusals: [(&str, Change); 5] = [
        ("schema", |cases| cases.schema = "kiln.other.v1".to_string()),
        ("no cases", |cases| cases.cases.clear()),
        ("duplicate case", |cases| {
            if let Some(first) = cases.cases.first().cloned() {
                cases.cases.push(first);
            }
        }),
        ("decision past the revisions", |cases| {
            if let Some(case) = cases.cases.first_mut() {
                case.decisions.push(CaseDecision {
                    revision: 9,
                    verdict: PlanVerdict::Accepted,
                });
            }
        }),
        ("self-review", |cases| cases.reviewer = AUTHOR.to_string()),
    ];
    for (name, change) in refusals {
        let mut changed = cases();
        change(&mut changed);
        assert!(plan_review_receipt(&changed).is_err(), "{name} admitted");
    }
}
