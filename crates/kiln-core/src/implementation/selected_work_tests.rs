use super::*;
use alloc::boxed::Box;
use alloc::vec;

fn h(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn requirement(predicate: &str, kind: EvidenceKind, revision: char) -> EvidenceRequirement {
    EvidenceRequirement {
        predicate: predicate.into(),
        kind,
        requirement_id: alloc::format!("req.{predicate}"),
        owner: "cairn.receipt-owner".into(),
        accepted_revision: h(revision),
        reference: h('e'),
    }
}

fn leaf(id: &str, selection: NodeSelection, evidence: EvidenceRequirement) -> ObligationNode {
    ObligationNode {
        id: id.into(),
        kind: ObligationNodeKind::Evidence,
        parent: Some("verify".into()),
        alternative: None,
        children: Vec::new(),
        selection,
        conditions: Vec::new(),
        evidence: vec![evidence],
        obligation: h('1'),
        guidance: h('2'),
        condition: h('3'),
    }
}

fn plan(outcome: ObligationOutcome, satisfied: &[&str], remaining: &[&str]) -> ObligationPlan {
    let root = ObligationNode {
        id: "verify".into(),
        kind: ObligationNodeKind::AllOf,
        parent: None,
        alternative: None,
        children: vec![
            "verify.contract".into(),
            "verify.receipt".into(),
            "verify.manual".into(),
        ],
        selection: NodeSelection::Required,
        conditions: Vec::new(),
        evidence: Vec::new(),
        obligation: h('4'),
        guidance: h('5'),
        condition: h('6'),
    };
    ObligationPlan {
        workflow_id: "cairn.verify".into(),
        workflow_version: 1,
        definition_id: "verify".into(),
        binding: h('7'),
        intent: h('8'),
        workflow_policy: h('a'),
        predicate: h('9'),
        subject: h('b'),
        context: h('c'),
        selected_alternatives: BTreeMap::from([("verify.trace".into(), "receipt".into())]),
        nodes: vec![
            root,
            leaf(
                "verify.contract",
                NodeSelection::Required,
                requirement("receipt.contract", EvidenceKind::Evidence, 'd'),
            ),
            leaf(
                "verify.receipt",
                NodeSelection::SelectedAlternative,
                requirement("receipt.freshness", EvidenceKind::Evidence, 'd'),
            ),
            leaf(
                "verify.manual",
                NodeSelection::UnselectedAlternative,
                requirement("manual.review", EvidenceKind::Evidence, 'd'),
            ),
        ],
        observations_supplied: !satisfied.is_empty(),
        evaluation: ObligationEvaluation {
            outcome,
            proved: outcome == ObligationOutcome::Proved,
            successful_delivery: outcome == ObligationOutcome::Proved,
            satisfied: satisfied.iter().map(|id| String::from(*id)).collect(),
            remaining: remaining
                .iter()
                .map(|id| RemainingCondition {
                    id: String::from(*id),
                    kind: "evidence".into(),
                    state: "unknown".into(),
                    reason: "observation missing".into(),
                })
                .collect(),
            report: h('f'),
        },
    }
}

fn snapshot(snapshot: char, obligations: char, section: ObligationSection) -> AdmittedSelectedWork {
    AdmittedSelectedWork {
        snapshot: h(snapshot),
        intake: IntakeProfile::TypedSnapshotV1,
        producer_schema: "cairn.selected-work.v1".into(),
        selection: WorkSelection {
            change: "sample".into(),
            task: Some("t1".into()),
        },
        cohort: SelectedWorkCohort {
            source: h('0'),
            intent: h('1'),
            context: h('c'),
            verification_subject: h('b'),
            dependencies: h('2'),
            obligations: h(obligations),
            selection: h('3'),
            observation: h(obligations),
        },
        source: SourceFacts {
            revision_kind: SourceRevisionKind::Projection,
            revision: "0".repeat(64),
            artifacts: Vec::new(),
        },
        policy: h('a'),
        profile: "spec-driven".into(),
        profile_identity: h('4'),
        completion: CompletionInputs {
            state: FactState::Ready,
            contract: Some(h('5')),
            results: None,
            subject: Some(h('b')),
        },
        context: ContextFacts {
            selection: h('6'),
            portable: h('7'),
            entries: Vec::new(),
        },
        readiness: ReadinessFacts {
            receipt: h('8'),
            input: h('9'),
            policy: h('a'),
            source: h('0'),
            evidence: h('1'),
            valid: true,
            classification: "blocked".into(),
            stages: vec![StageReadiness {
                stage: "completion".into(),
                state: StageState::Blocked,
                decision: h('2'),
            }],
            blockers: vec![StableBlocker {
                id: "completion.missing-results".into(),
                stage: "completion".into(),
                code: "missing_results".into(),
                target: "sample".into(),
            }],
            advisory_actions: Vec::new(),
        },
        units: vec![WorkUnitFact {
            task: Some("t1".into()),
            state: UnitState::Complete,
            references: vec![
                WorkReference {
                    kind: ReferenceKind::Requirement,
                    id: "kiln.sample".into(),
                    scope: ReferenceScope::Change,
                },
                WorkReference {
                    kind: ReferenceKind::Acceptance,
                    id: "kiln.sample".into(),
                    scope: ReferenceScope::Task("t1".into()),
                },
            ],
            verification_contracts: Vec::new(),
        }],
        remaining: vec![RemainingWorkFact {
            kind: RemainingKind::Completion,
            state: FactState::Blocked,
            subject: None,
        }],
        dependencies: Vec::new(),
        enumeration: EnumerationFacts {
            complete: true,
            observed: 1,
            total: Some(1),
        },
        obligations: section,
    }
}

fn planned() -> AdmittedSelectedWork {
    snapshot(
        'd',
        '5',
        ObligationSection::Exported(Box::new(plan(
            ObligationOutcome::Unknown,
            &[],
            &["verify.contract", "verify.receipt"],
        ))),
    )
}

fn proved() -> AdmittedSelectedWork {
    snapshot(
        'e',
        '6',
        ObligationSection::Exported(Box::new(plan(
            ObligationOutcome::Proved,
            &["verify", "verify.contract", "verify.receipt"],
            &[],
        ))),
    )
}

fn contract() -> AttemptSnapshotContract {
    bind_attempt(h('a'), h('b'), Some(h('c')), planned()).unwrap_or_else(|_| unreachable!())
}

/// `_node` names the obligation the predicate belongs to, for readability.
fn check(_node: &str, predicate: &str, state: CheckState) -> CanonicalCheckObservation {
    CanonicalCheckObservation {
        check: h(if state == CheckState::Passed {
            'd'
        } else {
            'e'
        }),
        predicate: predicate.into(),
        requirement_id: alloc::format!("req.{predicate}"),
        revision: h('d'),
        subject: h('b'),
        context: h('c'),
        policy: h('a'),
        candidate: h('9'),
        state,
    }
}

fn passing_checks() -> Vec<CanonicalCheckObservation> {
    vec![
        check("verify.contract", "receipt.contract", CheckState::Passed),
        check("verify.receipt", "receipt.freshness", CheckState::Passed),
    ]
}

fn authoritative(verdict: AuthoritativeVerdict) -> AuthoritativeCompletion {
    AuthoritativeCompletion {
        subject: h('b'),
        policy: h('a'),
        candidate: h('9'),
        verdict,
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
    evaluated: &AdmittedSelectedWork,
    checks: &[CanonicalCheckObservation],
    verdict: Option<AuthoritativeVerdict>,
) -> ObligationCompletion {
    let result = verdict.map(authoritative);
    decide_obligation_completion(&contract(), evaluated, &h('9'), checks, result.as_ref())
        .unwrap_or_else(|_| unreachable!())
}

#[test]
// r[verify kiln.selected_work.references.scenario.distinct]
fn reference_kinds_and_scopes_stay_distinct_and_facts_survive() {
    let work = planned();
    let references = work.references();
    assert_eq!(references.len(), 2);
    assert!(
        references
            .iter()
            .any(|r| r.kind == ReferenceKind::Requirement && r.scope == ReferenceScope::Change)
    );
    assert!(references.iter().any(
        |r| r.kind == ReferenceKind::Acceptance && r.scope == ReferenceScope::Task("t1".into())
    ));
    assert!(work.completion_only());
    assert_eq!(
        work.stage("completion").map(|stage| stage.state),
        Some(StageState::Blocked)
    );
    assert_eq!(
        work.readiness.blockers.first().map(|b| b.id.as_str()),
        Some("completion.missing-results")
    );
    let mut implementation = work.clone();
    implementation.remaining.push(RemainingWorkFact {
        kind: RemainingKind::Implementation,
        state: FactState::Ready,
        subject: None,
    });
    assert!(!implementation.completion_only());
}

#[test]
fn guidance_is_deterministic_and_unselected_alternatives_never_contribute() {
    let first = guidance_projection(&planned()).unwrap_or_else(|_| unreachable!());
    let second = guidance_projection(&planned()).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    let state = |id: &str| {
        first
            .obligations
            .iter()
            .find(|brief| brief.id == id)
            .map(|brief| brief.state)
    };
    assert_eq!(state("verify.contract"), Some(BriefState::Remaining));
    assert_eq!(state("verify.manual"), Some(BriefState::NotContributing));
    let done = guidance_projection(&proved()).unwrap_or_else(|_| unreachable!());
    assert_ne!(done.identity, first.identity);
    assert_eq!(
        done.obligations
            .iter()
            .find(|brief| brief.id == "verify.receipt")
            .map(|brief| brief.state),
        Some(BriefState::Satisfied)
    );
}

#[test]
// r[verify kiln.selected_work.attempts.scenario.replan]
fn changed_inputs_replan_while_evaluation_changes_refresh() {
    let bound = contract();
    let same = classify_observation(&bound, &planned(), Some(&h('c')));
    assert_eq!(same.kind, TransitionKind::Unchanged);
    let refreshed = classify_observation(&bound, &proved(), Some(&h('c')));
    assert_eq!(refreshed.kind, TransitionKind::Refresh);
    assert_eq!(
        refreshed.changed,
        vec![CohortField::Obligations, CohortField::Observation]
    );
    type Edit = fn(&mut AdmittedSelectedWork);
    let edits: [(CohortField, Edit); 6] = [
        (CohortField::Source, |w| w.cohort.source = h('e')),
        (CohortField::Intent, |w| w.cohort.intent = h('e')),
        (CohortField::Context, |w| w.cohort.context = h('e')),
        (CohortField::VerificationSubject, |w| {
            w.cohort.verification_subject = h('e');
        }),
        (CohortField::Policy, |w| w.policy = h('e')),
        (CohortField::Dependencies, |w| {
            w.cohort.dependencies = h('e')
        }),
    ];
    for (field, edit) in edits {
        let mut observed = planned();
        observed.snapshot = h('e');
        edit(&mut observed);
        let transition = classify_observation(&bound, &observed, Some(&h('c')));
        assert_eq!(transition.kind, TransitionKind::Replan, "{field:?}");
        assert!(transition.changed.contains(&field));
    }
    let moved = classify_observation(&bound, &planned(), Some(&h('d')));
    assert_eq!(moved.kind, TransitionKind::Replan);
    assert_eq!(moved.changed, vec![CohortField::Target]);
    assert_eq!(bound, contract());
}

#[test]
fn history_keeps_in_flight_contract_and_retains_superseded_ones() {
    let mut history = SelectedWorkHistory::new(IntakeProfile::TypedSnapshotV1);
    history
        .observe(SelectedWorkObservation::Admitted {
            snapshot: Box::new(planned()),
            target: Some(h('c')),
        })
        .unwrap_or_else(|_| unreachable!());
    let first = contract();
    history
        .bind(first.clone())
        .unwrap_or_else(|_| unreachable!());
    let mut changed = planned();
    changed.snapshot = h('e');
    changed.cohort.source = h('e');
    let transition = history
        .observe(SelectedWorkObservation::Admitted {
            snapshot: Box::new(changed.clone()),
            target: Some(h('c')),
        })
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(transition.kind, TransitionKind::Replan);
    assert_eq!(history.current.as_ref(), Some(&first));
    let rewrite = bind_attempt(h('a'), h('b'), Some(h('c')), changed.clone())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        history.bind(rewrite).err().map(|error| error.code),
        Some("selected_work_in_flight".into())
    );
    let successor =
        bind_attempt(h('d'), h('b'), Some(h('c')), changed).unwrap_or_else(|_| unreachable!());
    history
        .bind(successor.clone())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        history
            .contract_for(&h('a'))
            .map(|(currency, c)| (currency, c.identity.clone())),
        Some((Currency::Historical, first.identity.clone()))
    );
    assert_eq!(
        history.contract_for(&h('d')).map(|(currency, _)| currency),
        Some(Currency::Current)
    );
    assert_eq!(
        history.historical.first().map(|item| item.transition),
        Some(TransitionKind::Replan)
    );
    let surface = history.surface().unwrap_or_else(|_| unreachable!());
    assert_eq!(surface.current_contract, Some(successor.identity));
    assert_eq!(surface.historical_contracts, vec![first.identity]);
}

#[test]
// r[verify kiln.selected_work.attempts.scenario.unavailable]
fn unavailable_is_distinct_from_an_unselected_or_empty_section() {
    let mut unavailable = SelectedWorkHistory::new(IntakeProfile::TypedSnapshotV1);
    unavailable
        .observe(SelectedWorkObservation::Unavailable {
            reason: "selected-work read timed out".into(),
            refusal: None,
        })
        .unwrap_or_else(|_| unreachable!());
    let surface = unavailable.surface().unwrap_or_else(|_| unreachable!());
    assert_eq!(surface.observation, SurfaceObservation::Unavailable);
    assert!(surface.guidance.is_none());

    let mut empty = SelectedWorkHistory::new(IntakeProfile::TypedSnapshotV1);
    empty
        .observe(SelectedWorkObservation::Admitted {
            snapshot: Box::new(snapshot('d', '5', ObligationSection::NotSelected)),
            target: None,
        })
        .unwrap_or_else(|_| unreachable!());
    let surface = empty.surface().unwrap_or_else(|_| unreachable!());
    assert_eq!(surface.observation, SurfaceObservation::Admitted);
    let guidance = surface.guidance.unwrap_or_else(|| unreachable!());
    assert_eq!(guidance.availability, GuidanceAvailability::NotSelected);
    assert!(guidance.obligations.is_empty());

    let mut legacy = SelectedWorkHistory::new(IntakeProfile::LegacyReadinessCompatibility);
    assert!(
        legacy
            .observe(SelectedWorkObservation::Admitted {
                snapshot: Box::new(planned()),
                target: None,
            })
            .is_err()
    );
}

#[test]
// r[verify kiln.selected_work.completion]
fn only_exact_passing_evidence_with_authoritative_completion_completes() {
    let complete = decide(
        &proved(),
        &passing_checks(),
        Some(AuthoritativeVerdict::Passed),
    );
    assert!(complete.complete(), "{complete:?}");

    let missing = decide(
        &proved(),
        &passing_checks().into_iter().take(1).collect::<Vec<_>>(),
        Some(AuthoritativeVerdict::Passed),
    );
    assert_eq!(
        gaps(&missing),
        vec![("verify.receipt".into(), GapReason::MissingCheck)]
    );

    for (state, reason) in [
        (CheckState::Skipped, GapReason::SkippedCheck),
        (CheckState::Unknown, GapReason::UnknownCheck),
        (CheckState::Interrupted, GapReason::InterruptedCheck),
        (CheckState::Failed, GapReason::FailedCheck),
    ] {
        let mut checks = passing_checks();
        if let Some(check) = checks.get_mut(1) {
            check.state = state;
        }
        let decision = decide(&proved(), &checks, Some(AuthoritativeVerdict::Passed));
        assert_eq!(gaps(&decision), vec![("verify.receipt".into(), reason)]);
    }

    let mut stale = passing_checks();
    if let Some(check) = stale.first_mut() {
        check.candidate = h('8');
    }
    let decision = decide(&proved(), &stale, Some(AuthoritativeVerdict::Passed));
    assert_eq!(
        gaps(&decision),
        vec![("verify.contract".into(), GapReason::StaleCandidate)]
    );

    // A complete unselected alternative never substitutes for the selected one.
    let mut crossed = passing_checks();
    crossed.truncate(1);
    crossed.push(check("verify.manual", "manual.review", CheckState::Passed));
    let decision = decide(&proved(), &crossed, Some(AuthoritativeVerdict::Passed));
    assert_eq!(
        gaps(&decision),
        vec![("verify.receipt".into(), GapReason::MissingCheck)]
    );

    // Passing checks cannot override the producer's unproved evaluation.
    let decision = decide(
        &planned(),
        &passing_checks(),
        Some(AuthoritativeVerdict::Passed),
    );
    assert!(gaps(&decision).contains(&("verify".into(), GapReason::ProducerNotDelivered)));
    assert!(gaps(&decision).contains(&("verify.receipt".into(), GapReason::ProducerRemaining)));

    let decision = decide(&proved(), &passing_checks(), None);
    assert_eq!(
        gaps(&decision),
        vec![("verify".into(), GapReason::AuthoritativeMissing)]
    );
    let decision = decide(
        &proved(),
        &passing_checks(),
        Some(AuthoritativeVerdict::Unknown),
    );
    assert_eq!(
        gaps(&decision),
        vec![("verify".into(), GapReason::AuthoritativeUnknown)]
    );

    let mut selection_missing = proved();
    if let ObligationSection::Exported(plan) = &mut selection_missing.obligations
        && let Some(node) = plan.nodes.get_mut(2)
    {
        node.selection = NodeSelection::SelectionMissing;
    }
    let decision = decide(
        &selection_missing,
        &passing_checks(),
        Some(AuthoritativeVerdict::Passed),
    );
    assert!(gaps(&decision).contains(&("verify.receipt".into(), GapReason::SelectionMissing)));
}

#[test]
fn producer_evidence_comes_only_from_bound_canonical_checks() {
    let mut checks = passing_checks();
    checks.push(check(
        "verify.contract",
        "receipt.contract",
        CheckState::Skipped,
    ));
    let mut stale = check("verify.receipt", "receipt.freshness", CheckState::Passed);
    stale.subject = h('e');
    checks.push(stale);
    let observations = evidence_observations(&contract(), &h('9'), &checks);
    assert_eq!(observations.len(), 3);
    assert_eq!(
        observations.last().map(|item| item.state),
        Some(EvidenceObservationState::Unknown)
    );
    assert!(observations.iter().all(|item| item.subject == h('b')));
}

#[test]
// r[verify kiln.selected_work.display.scenario.resubmit]
fn restricted_display_cannot_become_an_admitted_snapshot() {
    let display = restricted_display(&planned(), "operator").unwrap_or_else(|_| unreachable!());
    assert_eq!(display.schema, SELECTED_WORK_DISPLAY_SCHEMA);
    assert_eq!(display.coverage, DisplayCoverage::Restricted);
    let bytes = serde_json::to_vec(&display).unwrap_or_else(|_| unreachable!());
    assert!(serde_json::from_slice::<AdmittedSelectedWork>(&bytes).is_err());
    let mut tampered = contract();
    tampered.snapshot.policy = h('e');
    assert!(tampered.verify().is_err());
    let mut incomplete = planned();
    incomplete.enumeration.complete = false;
    assert!(bind_attempt(h('a'), h('b'), None, incomplete).is_err());
}

fn implementing() -> AdmittedSelectedWork {
    let mut work = planned();
    work.remaining.push(RemainingWorkFact {
        kind: RemainingKind::Implementation,
        state: FactState::Ready,
        subject: None,
    });
    for unit in &mut work.units {
        unit.state = UnitState::Ready;
    }
    work.readiness.stages = PLANNING_STAGES
        .iter()
        .map(|stage| StageReadiness {
            stage: (*stage).into(),
            state: StageState::Ready,
            decision: h('2'),
        })
        .collect();
    work.readiness.blockers.clear();
    work
}

fn admitted(work: AdmittedSelectedWork) -> SelectedWorkObservation {
    SelectedWorkObservation::Admitted {
        snapshot: Box::new(work),
        target: None,
    }
}

fn member(
    stage: crate::implementation::campaign::MemberStage,
) -> crate::implementation::campaign::MemberObservation {
    use crate::implementation::campaign::{EnumerationState, Freshness, MemberObservation};
    MemberObservation {
        member: "change/t1".into(),
        stage,
        enumeration: EnumerationState::Complete,
        source_identity: Some(h('a')),
        policy_identity: h('a'),
        subject_identity: Some(h('b')),
        integration_target_identity: None,
        profile_identity: h('a'),
        dependencies: Vec::new(),
        remaining_obligations: BTreeSet::new(),
        dependency_reason: None,
        canonical_work_key: Some(h('a')),
        work_identity: Some(h('b')),
        attempt_identity: None,
        attempt_fence: None,
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::NotApplicable,
        candidate_freshness: Freshness::Unknown,
        policy_freshness: Freshness::Current,
        verifier_freshness: Freshness::Unknown,
        target_freshness: Freshness::Current,
        evidence: Vec::new(),
        blockers: Vec::new(),
        required_checks_satisfied: false,
        required_reviews_satisfied: false,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

#[test]
// r[verify kiln.selected_work.stages]
// r[verify kiln.selected_work.stages.scenario.disagree]
fn typed_profile_follows_the_snapshot_when_legacy_readiness_disagrees() {
    use crate::implementation::campaign::MemberStage;
    assert_eq!(
        typed_readiness(&admitted(implementing())),
        TypedReadiness::Ready
    );
    // Legacy says ready; the snapshot holds the design phase with a stable blocker.
    let mut blocked = implementing();
    for stage in &mut blocked.readiness.stages {
        if stage.stage == "design" {
            stage.state = StageState::Blocked;
        }
    }
    blocked.readiness.blockers.push(StableBlocker {
        id: "design.missing-decision".into(),
        stage: "design".into(),
        code: "missing_decision".into(),
        target: "sample".into(),
    });
    let mut observation = member(MemberStage::Ready);
    observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &admitted(blocked));
    assert_eq!(observation.stage, MemberStage::Blocked);
    assert_eq!(
        observation.blockers,
        vec![String::from("design.missing-decision")]
    );
    // Legacy says blocked; the snapshot is ready and canonical work exists.
    let mut observation = member(MemberStage::Blocked);
    observation
        .blockers
        .push("native Cairn workflow status is invalid".into());
    observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &admitted(implementing()));
    assert_eq!(observation.stage, MemberStage::Ready);
    assert!(observation.blockers.is_empty());
    // Ready without canonical work, or legacy artifact work the snapshot no
    // longer calls for, is never selectable.
    let mut observation = member(MemberStage::Blocked);
    observation.work_identity = None;
    observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &admitted(implementing()));
    assert_eq!(observation.stage, MemberStage::Blocked);
    let mut observation = member(MemberStage::Selected);
    observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &admitted(implementing()));
    assert_eq!(observation.stage, MemberStage::Blocked);
}

#[test]
// r[verify kiln.selected_work.stages]
fn unavailable_incomplete_or_blocked_snapshot_members_are_never_ready() {
    use crate::implementation::campaign::{EnumerationState, MemberStage};
    let mut incomplete = implementing();
    incomplete.enumeration.complete = false;
    let mut unit = implementing();
    for item in &mut unit.units {
        item.state = UnitState::Blocked;
    }
    let mut dependency = implementing();
    dependency.dependencies.push(DependencyFact {
        task: "t1".into(),
        prerequisite: "t0".into(),
        state: FactState::Unavailable,
        identity: h('3'),
    });
    let mut missing_phase = implementing();
    missing_phase
        .readiness
        .stages
        .retain(|stage| stage.stage != "tasks");
    for (typed, reason, enumeration) in [
        (
            SelectedWorkObservation::NotObserved,
            HoldReason::Unobserved,
            EnumerationState::NotObserved,
        ),
        (
            SelectedWorkObservation::Unavailable {
                reason: "selected-work read timed out".into(),
                refusal: None,
            },
            HoldReason::Unavailable,
            EnumerationState::NotObserved,
        ),
        (
            admitted(incomplete),
            HoldReason::IncompleteEnumeration,
            EnumerationState::IncompleteLimit,
        ),
        (
            admitted(unit),
            HoldReason::UnitBlocked,
            EnumerationState::Complete,
        ),
        (
            admitted(dependency),
            HoldReason::DependencyBlocked,
            EnumerationState::Complete,
        ),
        (
            admitted(missing_phase),
            HoldReason::PhaseBlocked,
            EnumerationState::Complete,
        ),
    ] {
        assert!(matches!(
            typed_readiness(&typed),
            TypedReadiness::Held { reason: held, .. } if held == reason
        ));
        let mut observation = member(MemberStage::Ready);
        observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &typed);
        assert_eq!(observation.stage, MemberStage::Blocked, "{reason:?}");
        assert_eq!(observation.enumeration, enumeration, "{reason:?}");
        assert!(!observation.blockers.is_empty());
    }
    // Completion-only work is not implementation work.
    assert_eq!(
        typed_readiness(&admitted(planned())),
        TypedReadiness::CompletionOnly
    );
    let mut observation = member(MemberStage::Ready);
    observation.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &admitted(planned()));
    assert_eq!(observation.stage, MemberStage::Blocked);
}

#[test]
// r[verify kiln.selected_work.stages]
fn legacy_profile_and_in_flight_members_are_unchanged() {
    use crate::implementation::campaign::MemberStage;
    let held = SelectedWorkObservation::Unavailable {
        reason: "selected-work read timed out".into(),
        refusal: None,
    };
    for stage in [
        MemberStage::Ready,
        MemberStage::Blocked,
        MemberStage::Selected,
    ] {
        let before = member(stage);
        let mut after = before.clone();
        after.apply_intake_readiness(IntakeProfile::LegacyReadinessCompatibility, &held);
        assert_eq!(after, before);
    }
    let before = member(MemberStage::Executing);
    let mut after = before.clone();
    after.apply_intake_readiness(IntakeProfile::TypedSnapshotV1, &held);
    assert_eq!(after, before);
}
