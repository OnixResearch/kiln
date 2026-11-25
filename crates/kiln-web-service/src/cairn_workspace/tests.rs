use super::*;
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, Freshness, MemberObservation, PendingEffect, WorkflowProfile,
};
use kiln_core::implementation::{
    Enrollment, IMPLEMENTATION_WORK_SCHEMA, SupersessionRule, WorkBudget,
};
use kiln_implementer::native_publication::NATIVE_CAIRN_LIFECYCLE_SCHEMA;
use kiln_implementer::publication_effects::CairnLifecycleBinding;

const REPOSITORY: &str = "onix/kiln";
const OTHER_REPOSITORY: &str = "onix/other";
const CHANGE: &str = "chg";
const CAMPAIGN: &str = "campaign-1";
const STORE_VERSION: u64 = 3;
const NOW_MS: u64 = 1_000;
const LEASE_MS: u64 = 60_000;

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn git(kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new("git", kind, value).unwrap_or_else(|_| unreachable!())
}

fn work(
    repository: &str,
    native_id: &str,
    revision: u64,
    version: char,
    dependencies: Vec<ContentId>,
) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision,
        actor: "operator".into(),
        repository: repository.into(),
        project: "kiln".into(),
        base: None,
        title: format!("{native_id} revision {revision}"),
        requested_outcome: "implement".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a1".into(),
            statement: "verified".into(),
        }],
        dependencies,
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: id('9'),
        budget: WorkBudget {
            attempts: 3,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", native_id).unwrap_or_else(|_| unreachable!()),
            native_id: native_id.into(),
            version: id(version),
        }],
    }
}

fn binding(selector: MemberSelector) -> MemberBinding {
    MemberBinding {
        selector,
        profile: WorkflowProfile::SpecDriven,
        profile_version: id('a'),
        repository_instructions: id('a'),
        policy: id('a'),
        required_checks: Vec::new(),
        required_reviews: Vec::new(),
        integration_policy: id('a'),
        goal: CompletionGoal::VerifiedCandidate,
        prerequisite_inclusion: false,
        context: Vec::new(),
    }
}

fn observed(member: &str, stage: MemberStage, work: Option<ContentId>) -> MemberObservation {
    MemberObservation {
        member: member.into(),
        stage,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: None,
        integration_target_identity: None,
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: BTreeSet::new(),
        dependency_reason: None,
        canonical_work_key: None,
        work_identity: work,
        attempt_identity: None,
        attempt_fence: None,
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::NotApplicable,
        candidate_freshness: Freshness::Current,
        policy_freshness: Freshness::Current,
        verifier_freshness: Freshness::Unknown,
        target_freshness: Freshness::Unknown,
        evidence: Vec::new(),
        blockers: Vec::new(),
        required_checks_satisfied: true,
        required_reviews_satisfied: true,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

fn effect(state: &CampaignState, byte: char, progress_applied: bool) -> PendingEffect {
    PendingEffect {
        effect_id: id(byte),
        member: "chg/task-c".into(),
        operation: CampaignOperation::Code,
        owner: "work-attempt-owner".into(),
        intent_identity: id(byte),
        state: EffectState::Unknown,
        progress_applied,
        selection_identity: state.selection_identity.clone(),
        source_identity: None,
        work_identity: None,
        attempt_identity: None,
        attempt_fence: None,
        subject_identity: None,
        policy_identity: id('a'),
        context_identity: None,
        native_context_identity: None,
        profile_identity: id('a'),
        evidence_identity: None,
        integration_target_identity: None,
    }
}

fn receipt(
    attempt: &ImplementationAttempt,
    fence: u64,
    source: SourceRef,
) -> NativeCairnLifecycleReceipt {
    NativeCairnLifecycleReceipt {
        schema: NATIVE_CAIRN_LIFECYCLE_SCHEMA.into(),
        integration: id('5'),
        work_identity: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        attempt: attempt.identity.clone(),
        fence,
        binding: CairnLifecycleBinding {
            source,
            handoff: id('3'),
            verification: id('4'),
            authoritative_completion: id('7'),
            candidate: git("commit", &"c".repeat(40)),
            action: CairnLifecycleAction::ObserveReviewedArchive,
        },
        observed_tree: git("tree", &"d".repeat(40)),
        lifecycle_policy: id('8'),
        lifecycle_receipt: ExternalRef::new("cairn", "receipt", &"e".repeat(64))
            .unwrap_or_else(|_| unreachable!()),
    }
}

struct Fixture {
    snapshot: CanonicalSnapshot,
    x: ContentId,
    c1: ContentId,
    c2: ContentId,
    attempt_a: ContentId,
    attempt_c1: ContentId,
    attempt_c2: ContentId,
}

fn contract(campaign_id: &str, repository: &str, members: Vec<MemberBinding>) -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: campaign_id.into(),
        revision: 1,
        repository: repository.into(),
        members,
        max_concurrency: 1,
        max_outstanding_decisions: 1,
        parent_budget_id: id('a'),
        agent_definition: id('a'),
        require_one_writer_per_repository: true,
    }
}

/// A failed with an unreconciled unknown outcome; B depends on A; X depends on
/// absent work; Y's unknown outcome is reconciled; C revision 1 was claimed,
/// then superseded by revision 2 and claimed again while the campaign member
/// still binds revision 1; D is selected by the campaign only; `other` is a
/// planning whole-change member with an incompletely enumerated unit. A second
/// campaign selects a change with the same name in another repository.
fn fixture() -> Fixture {
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let mut attempts = AttemptLedger::default();
    let mut admit = |work: &ImplementationWork| {
        ledger
            .admit(work, None, &enrollment)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        work.identity().unwrap_or_else(|_| unreachable!())
    };
    let a = work(REPOSITORY, "chg/task-a", 1, '1', Vec::new());
    let a_identity = admit(&a);
    admit(&work(REPOSITORY, "chg/task-b", 1, '2', vec![a_identity]));
    let x = admit(&work(REPOSITORY, "chg/task-x", 1, '3', vec![id('f')]));
    let y = work(REPOSITORY, "chg/task-y", 1, '4', Vec::new());
    admit(&y);
    let c1_work = work(REPOSITORY, "chg/task-c", 1, '5', Vec::new());
    let c1 = admit(&c1_work);
    let c2_work = work(REPOSITORY, "chg/task-c", 2, '7', Vec::new());

    let claim = |attempts: &mut AttemptLedger, work: &ImplementationWork| {
        attempts
            .claim(work, NOW_MS, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
    };
    let attempt_a = claim(&mut attempts, &a);
    let attempt_y = claim(&mut attempts, &y);
    for attempt in [&attempt_a, &attempt_y] {
        attempts
            .fail(
                &attempt.identity,
                attempt.fence,
                NOW_MS,
                FailureClass::Unknown,
                "dispatch_outcome_unknown",
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    }
    attempts
        .reconcile_unknown(
            &attempt_y.identity,
            attempt_y.fence,
            &ExternalRef::new("lattice", "termination", "run-y").unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let attempt_c1 = claim(&mut attempts, &c1_work);
    let c2 = admit(&c2_work);
    attempts
        .supersede(
            &c2_work.canonical_key().unwrap_or_else(|_| unreachable!()),
            c2_work.revision,
            SupersessionRule::AbortActive,
            NOW_MS,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let attempt_c2 = claim(&mut attempts, &c2_work);

    let mut state = CampaignState::start(contract(
        CAMPAIGN,
        REPOSITORY,
        vec![
            binding(MemberSelector::Tasks {
                change: CHANGE.into(),
                tasks: vec!["task-c".into()],
            }),
            binding(MemberSelector::Tasks {
                change: CHANGE.into(),
                tasks: vec!["task-d".into()],
            }),
            binding(MemberSelector::WholeChange {
                change: "other".into(),
            }),
        ],
    ))
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let elsewhere = CampaignState::start(contract(
        "campaign-2",
        OTHER_REPOSITORY,
        vec![binding(MemberSelector::WholeChange {
            change: CHANGE.into(),
        })],
    ))
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let mut planning = observed("other/*", MemberStage::Planning, None);
    planning.enumeration = EnumerationState::IncompleteLimit;
    planning.remaining_obligations.insert("other/task-r".into());
    for observation in [
        observed("chg/task-c", MemberStage::Ready, Some(c1.clone())),
        planning,
    ] {
        state
            .observe(observation)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    }
    let unapplied = effect(&state, 'b', false);
    let applied = effect(&state, 'c', true);
    state.pending_effects.insert(id('b'), unapplied);
    state.pending_effects.insert(id('c'), applied);

    let source = c1_work
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let lifecycle = BTreeMap::from([
        (
            attempt_c1.identity.as_str().to_owned(),
            receipt(&attempt_c1, attempt_c1.fence, source.clone()),
        ),
        (
            attempt_c2.identity.as_str().to_owned(),
            receipt(&attempt_c2, attempt_c2.fence + 1, source),
        ),
    ]);
    Fixture {
        snapshot: CanonicalSnapshot {
            work: Some(ledger),
            attempts: Some(attempts),
            lifecycle,
            lifecycle_truncated: false,
            campaigns: vec![(state, STORE_VERSION), (elsewhere, 1)],
            campaigns_truncated: false,
        },
        x,
        c1,
        c2,
        attempt_a: attempt_a.identity,
        attempt_c1: attempt_c1.identity,
        attempt_c2: attempt_c2.identity,
    }
}

fn key(repository: &str, change: &str) -> CairnKey {
    CairnKey {
        repository_identity: repository_identity(repository),
        change: change.into(),
    }
}

fn detail_in(fixture: &Fixture, repository: &str, change: &str) -> Detail {
    detail(&fixture.snapshot, &key(repository, change))
        .unwrap_or_else(|code| unreachable!("{code}"))
        .unwrap_or_else(|| unreachable!("{change} is named"))
}

fn read_detail(fixture: &Fixture, change: &str) -> Detail {
    detail_in(fixture, REPOSITORY, change)
}

fn unit<'a>(detail: &'a Detail, native_id: &str) -> &'a UnitView {
    detail
        .units
        .iter()
        .find(|unit| unit.native_id == native_id)
        .unwrap_or_else(|| unreachable!("{native_id} is listed"))
}

#[test]
// r[verify kiln.cairn.workspace.scenario.identity]
fn every_source_resolves_one_cairn_identity() {
    let fixture = fixture();
    let index = index(&fixture.snapshot).unwrap_or_else(|code| unreachable!("{code}"));
    let ours = repository_identity(REPOSITORY);
    let theirs = repository_identity(OTHER_REPOSITORY);
    let mut expected = vec![
        (ours.as_str(), CHANGE),
        (ours.as_str(), "other"),
        (theirs.as_str(), CHANGE),
    ];
    expected.sort_unstable();
    let listed: Vec<(&str, &str)> = index
        .changes
        .iter()
        .map(|entry| (entry.repository_identity.as_str(), entry.change.as_str()))
        .collect();
    assert_eq!(listed, expected);
    assert!(!index.truncated);

    let detail = read_detail(&fixture, CHANGE);
    let units: Vec<(&str, Vec<&str>)> = detail
        .units
        .iter()
        .map(|unit| {
            (
                unit.native_id.as_str(),
                unit.sources.iter().copied().collect(),
            )
        })
        .collect();
    assert_eq!(
        units,
        vec![
            ("chg/task-a", vec!["canonical_work"]),
            ("chg/task-b", vec!["canonical_work"]),
            ("chg/task-c", vec!["campaign_selection", "canonical_work"]),
            ("chg/task-d", vec!["campaign_selection"]),
            ("chg/task-x", vec!["canonical_work"]),
            ("chg/task-y", vec!["canonical_work"]),
        ]
    );
    let task_c = unit(&detail, "chg/task-c");
    let [current] = task_c.current_work.as_slice() else {
        unreachable!("one current version")
    };
    assert_eq!(
        (
            current.identity.as_str(),
            current.revision,
            current.readiness_snapshot.as_str()
        ),
        (fixture.c2.as_str(), 2, id('7').as_str())
    );
    let [superseded] = task_c.superseded_work.as_slice() else {
        unreachable!("one superseded version")
    };
    assert_eq!(
        (superseded.identity.as_str(), superseded.revision),
        (fixture.c1.as_str(), 1)
    );
    // The same change name in another repository stays a separate change.
    let elsewhere = detail_in(&fixture, OTHER_REPOSITORY, CHANGE);
    let members: Vec<(&str, &str)> = elsewhere
        .memberships
        .iter()
        .map(|membership| (membership.campaign_id.as_str(), membership.member.as_str()))
        .collect();
    assert_eq!(members, vec![("campaign-2", "chg/*")]);
    assert!(elsewhere.units.is_empty() && elsewhere.projects.is_empty());
    assert!(
        detail
            .memberships
            .iter()
            .all(|membership| membership.campaign_id == CAMPAIGN)
    );
    let dependencies: Vec<&DependencyView> = unit(&detail, "chg/task-b")
        .current_work
        .iter()
        .flat_map(|work| &work.dependencies)
        .collect();
    let [dependency] = dependencies.as_slice() else {
        unreachable!("one dependency")
    };
    assert_eq!(
        (dependency.state, dependency.native_ids.as_slice()),
        ("current", ["chg/task-a".to_owned()].as_slice())
    );
    let bindings: Vec<(&str, &str)> = detail
        .memberships
        .iter()
        .map(|membership| (membership.member.as_str(), membership.work_binding))
        .collect();
    assert_eq!(
        bindings,
        vec![("chg/task-c", "stale"), ("chg/task-d", "not_bound")]
    );
    assert_eq!(detail.identity.len(), 64);
    assert_eq!(detail.identity, read_detail(&fixture, CHANGE).identity);
    assert!(
        super::detail(&fixture.snapshot, &key(REPOSITORY, "absent"))
            .unwrap_or_else(|code| unreachable!("{code}"))
            .is_none()
    );
}

#[test]
// r[verify kiln.cairn.workspace.scenario.visibility]
fn waiting_members_stay_visible_without_ready_coding_work() {
    let fixture = fixture();
    let other = read_detail(&fixture, "other");
    let [member] = other.memberships.as_slice() else {
        unreachable!("one member")
    };
    assert_eq!(
        (
            member.selector,
            member.observed,
            member.stage,
            member.enumeration.clone()
        ),
        (
            "whole_change",
            true,
            Some(MemberStage::Planning),
            Some(EnumerationState::IncompleteLimit)
        )
    );
    assert_eq!(member.remaining_obligations, ["other/task-r"]);
    let [unit] = other.units.as_slice() else {
        unreachable!("one remaining unit")
    };
    assert_eq!(
        (unit.native_id.as_str(), unit.facets.readiness),
        ("other/task-r", "not_admitted")
    );
    assert_eq!(
        unit.sources,
        BTreeSet::from(["campaign_remaining_obligation"])
    );
    assert_eq!(other.overview.current_work, 0);

    let selected = read_detail(&fixture, CHANGE);
    let unobserved = selected
        .memberships
        .iter()
        .find(|membership| membership.member == "chg/task-d")
        .unwrap_or_else(|| unreachable!("selected member is listed"));
    assert!(!unobserved.observed && unobserved.stage.is_none());
}

#[test]
// r[verify kiln.cairn.workspace.scenario.stages]
fn stages_are_separately_legible() {
    let detail = read_detail(&fixture(), CHANGE);
    let facets = |native_id| {
        let facets = &unit(&detail, native_id).facets;
        (
            facets.readiness,
            facets.execution.clone(),
            facets.evidence,
            facets.integration.clone(),
            facets.lifecycle,
        )
    };
    let not_started = |readiness| {
        (
            readiness,
            "not_started".to_owned(),
            "none",
            "not_published".to_owned(),
            "not_observed",
        )
    };
    assert_eq!(facets("chg/task-b"), not_started("ready"));
    assert_eq!(facets("chg/task-x"), not_started("blocked"));
    assert_eq!(facets("chg/task-d"), not_started("not_admitted"));
    assert_eq!(
        facets("chg/task-a"),
        (
            "ready",
            "failed".to_owned(),
            "none",
            "not_published".to_owned(),
            "not_observed"
        )
    );
    assert_eq!(facets("chg/task-c").1, "claimed");
    assert_eq!(
        (detail.overview.attempts, detail.overview.active_attempts),
        (4, 1)
    );
}

#[test]
// r[verify kiln.cairn.workspace.scenario.evidence]
fn historical_evidence_never_reads_as_current() {
    let fixture = fixture();
    let detail = read_detail(&fixture, CHANGE);
    let task_c = unit(&detail, "chg/task-c");
    let attempts: Vec<(&str, &str, Option<&str>)> = task_c
        .attempts
        .iter()
        .map(|attempt| {
            (
                attempt.identity.as_str(),
                attempt.applicability,
                attempt.lifecycle.as_ref().map(|receipt| receipt.binding),
            )
        })
        .collect();
    assert_eq!(
        attempts,
        vec![
            (
                fixture.attempt_c1.as_str(),
                "historical",
                Some("matches_attempt")
            ),
            (fixture.attempt_c2.as_str(), "current", Some("crossed")),
        ]
    );
    // Neither the historical receipt nor the crossed one is current lifecycle.
    assert_eq!(task_c.facets.lifecycle, "not_observed");

    let [historical, current] = task_c.attempts.as_slice() else {
        unreachable!("two attempts")
    };
    let verified = |success| {
        Some(VerificationView {
            run_id: "run".into(),
            success,
            candidate: git("commit", &"c".repeat(40)),
            tree: git("tree", &"d".repeat(40)),
            verification_profile: id('9').as_str().to_owned(),
            checks: Vec::new(),
        })
    };
    let evidence = |historical_success, current_success: Option<bool>| {
        let mut old = historical.clone();
        old.verification = verified(historical_success);
        let mut new = current.clone();
        new.verification = current_success.and_then(verified);
        facets(&task_c.current_work, &[old, new]).evidence
    };
    assert_eq!(evidence(true, None), "historical_only");
    assert_eq!(evidence(true, Some(false)), "current_failed");
    assert_eq!(evidence(false, Some(true)), "current_verified");
    let mut matching = current.clone();
    if let Some(receipt) = &mut matching.lifecycle {
        receipt.binding = "matches_attempt";
    }
    assert_eq!(
        facets(&task_c.current_work, &[matching]).lifecycle,
        "receipt_observed"
    );
}

#[test]
// r[verify kiln.cairn.workspace.scenario.blockers]
fn blockers_name_condition_observation_operation_and_owner() {
    let fixture = fixture();
    let detail = read_detail(&fixture, CHANGE);
    let mut found: Vec<(&str, &str, &str, CampaignOperation, &str)> = detail
        .blockers
        .iter()
        .filter(|blocker| blocker.condition != "campaign_hold")
        .map(|blocker| {
            (
                blocker.condition,
                blocker.subject.as_str(),
                blocker.observation.as_str(),
                blocker.operation,
                blocker.owner.as_str(),
            )
        })
        .collect();
    found.sort_by_key(|blocker| (blocker.0, blocker.1, blocker.2));
    let attempt = format!("attempt:{}", fixture.attempt_a.as_str());
    let work = format!("work:{}", fixture.x.as_str());
    let effect = format!("effect:{}", id('b').as_str());
    let campaign = format!("campaign:{CAMPAIGN}@{STORE_VERSION}");
    assert_eq!(
        found,
        vec![
            (
                "dispatch_recovery",
                "chg/task-a",
                attempt.as_str(),
                CampaignOperation::Reconcile,
                "attempt-reconciliation-owner"
            ),
            (
                "missing_dependency",
                "chg/task-x",
                work.as_str(),
                CampaignOperation::Wait,
                "native-source-owner"
            ),
            (
                "pending_effect",
                "chg/task-c",
                effect.as_str(),
                CampaignOperation::Reconcile,
                "work-attempt-owner"
            ),
            (
                "stale_work_binding",
                "chg/task-c",
                campaign.as_str(),
                CampaignOperation::Replan,
                "campaign-policy"
            ),
        ]
    );
    let outcomes: Vec<(&str, Option<&str>)> = ["chg/task-a", "chg/task-y"]
        .into_iter()
        .flat_map(|native_id| &unit(&detail, native_id).attempts)
        .map(|attempt| (attempt.identity.as_str(), attempt.unknown_outcome))
        .collect();
    assert_eq!(outcomes.len(), 2);
    assert_eq!(
        outcomes.first().map(|outcome| outcome.1),
        Some(Some("unreconciled"))
    );
    assert_eq!(
        outcomes.last().map(|outcome| outcome.1),
        Some(Some("reconciled"))
    );
    // Holds keep their own operation and owner; the reason is display text.
    for membership in &detail.memberships {
        let holds: Vec<&Blocker> = detail
            .blockers
            .iter()
            .filter(|blocker| {
                blocker.condition == "campaign_hold" && blocker.subject == membership.member
            })
            .collect();
        match (&membership.hold, holds.as_slice()) {
            (None, []) => {}
            (Some(hold), [blocker]) => {
                assert_eq!(blocker.operation, hold.operation);
                assert_eq!(blocker.detail, hold.reason);
                assert_eq!(
                    blocker.owner,
                    hold.owner
                        .clone()
                        .unwrap_or_else(|| hold.operation.owner().to_owned())
                );
            }
            _ => unreachable!("one blocker per hold"),
        }
    }
    assert_eq!(detail.overview.blockers, detail.blockers.len());
    assert_eq!(
        detail.overview.first_blocker.as_ref(),
        detail.blockers.first()
    );
}

#[test]
// r[verify kiln.cairn.workspace]
fn the_index_reports_truncation_and_empty_stores() {
    let fixture = fixture();
    let bounded = index_bounded(&fixture.snapshot, 1).unwrap_or_else(|code| unreachable!("{code}"));
    assert_eq!((bounded.changes.len(), bounded.truncated), (1, true));
    let snapshot = CanonicalSnapshot::default();
    let empty = index(&snapshot).unwrap_or_else(|code| unreachable!("{code}"));
    assert!(empty.changes.is_empty() && !empty.truncated && !empty.snapshot.work_recorded);
}

#[test]
fn index_attributes_only_current_active_attempts() {
    let fixture = fixture();
    let projection = index(&fixture.snapshot).unwrap_or_else(|code| unreachable!("{code}"));
    let assignments: Vec<_> = projection
        .changes
        .iter()
        .flat_map(|change| &change.active_assignments)
        .collect();
    let [assignment] = assignments.as_slice() else {
        unreachable!("expected only the current active attempt");
    };
    let attempt = fixture
        .snapshot
        .attempts
        .as_ref()
        .and_then(|ledger| ledger.get(&fixture.attempt_c2))
        .unwrap_or_else(|| unreachable!("fixture requires a current attempt"));
    assert_eq!(assignment.attempt, attempt.identity.as_str());
    assert_eq!(assignment.fence, attempt.fence);
    assert!(assignment.worker.is_none());
}

fn facets_of(
    readiness: &'static str,
    execution: &str,
    evidence: &'static str,
    integration: &str,
    lifecycle: &'static str,
) -> Facets {
    Facets {
        readiness,
        execution: execution.into(),
        evidence,
        integration: integration.into(),
        lifecycle,
    }
}

#[test]
// r[verify kiln.cairn.workspace.scenario.lanes]
fn every_stop_needs_attention_and_only_current_facts_advance_a_unit() {
    let base = || {
        facets_of(
            "ready",
            "not_started",
            "none",
            "not_published",
            "not_observed",
        )
    };
    assert_eq!(lane(&base()), "ready");
    // Every stop wins over any advance, including an observed lifecycle receipt.
    for stopped in [
        Facets {
            readiness: "blocked",
            ..base()
        },
        Facets {
            readiness: "unknown",
            ..base()
        },
        Facets {
            evidence: "current_failed",
            execution: "verifying".into(),
            ..base()
        },
        Facets {
            readiness: "blocked",
            lifecycle: "receipt_observed",
            ..base()
        },
    ] {
        assert_eq!(lane(&stopped), "attention", "{stopped:?}");
    }
    for phase in [
        "blocked",
        "failed",
        "cancelled",
        "superseded",
        "paused-by-a-newer-owner",
    ] {
        assert_eq!(
            lane(&Facets {
                execution: phase.into(),
                ..base()
            }),
            "attention",
            "{phase}"
        );
    }
    assert_eq!(
        lane(&Facets {
            readiness: "not_admitted",
            ..base()
        }),
        "awaiting"
    );
    for phase in ["claimed", "executing", "candidate_produced", "verifying"] {
        assert_eq!(
            lane(&Facets {
                execution: phase.into(),
                ..base()
            }),
            "in_progress",
            "{phase}"
        );
    }
    // Historical verification never advances a unit.
    assert_eq!(
        lane(&Facets {
            evidence: "historical_only",
            ..base()
        }),
        "ready"
    );
    let verified = || Facets {
        execution: "verified".into(),
        evidence: "current_verified",
        ..base()
    };
    assert_eq!(lane(&verified()), "verified");
    assert_eq!(
        lane(&Facets {
            integration: "pull_request_published".into(),
            ..verified()
        }),
        "verified"
    );
    for integrated in ["integrated", "source_reconciled"] {
        assert_eq!(
            lane(&Facets {
                integration: integrated.into(),
                ..verified()
            }),
            "integrated"
        );
    }
    assert_eq!(
        lane(&Facets {
            integration: "integrated".into(),
            lifecycle: "receipt_observed",
            ..verified()
        }),
        "recorded"
    );

    let fixture = fixture();
    let detail = read_detail(&fixture, CHANGE);
    let lanes: Vec<(&str, &str)> = detail
        .units
        .iter()
        .map(|unit| (unit.native_id.as_str(), unit.lane))
        .collect();
    assert_eq!(
        lanes,
        vec![
            ("chg/task-a", "attention"),
            ("chg/task-b", "ready"),
            ("chg/task-c", "in_progress"),
            ("chg/task-d", "awaiting"),
            ("chg/task-x", "attention"),
            ("chg/task-y", "attention"),
        ]
    );
    assert_eq!(detail.overview.lane, "attention");
    assert_eq!(
        detail.overview.lanes,
        BTreeMap::from([
            ("attention", 3),
            ("awaiting", 1),
            ("ready", 1),
            ("in_progress", 1),
            ("verified", 0),
            ("integrated", 0),
            ("recorded", 0),
        ])
    );
    // Without a stop a change sits with its least-advanced unit; naming no
    // unit awaits work.
    let moving: Vec<UnitView> = detail
        .units
        .iter()
        .filter(|unit| unit.lane != "attention")
        .cloned()
        .collect();
    assert_eq!(change_lane(&moving), "awaiting");
    let started: Vec<UnitView> = moving
        .iter()
        .filter(|unit| unit.lane != "awaiting")
        .cloned()
        .collect();
    assert_eq!(change_lane(&started), "ready");
    assert_eq!(change_lane(&[]), "awaiting");
    let other = read_detail(&fixture, "other");
    assert_eq!((other.overview.lane, other.units.len()), ("awaiting", 1));
    let elsewhere = detail_in(&fixture, OTHER_REPOSITORY, CHANGE);
    assert_eq!(
        (
            elsewhere.overview.lane,
            elsewhere.overview.lanes.values().sum::<usize>()
        ),
        ("awaiting", 0)
    );

    let index = index(&fixture.snapshot).unwrap_or_else(|code| unreachable!("{code}"));
    for entry in &index.changes {
        let detail = detail_in(
            &fixture,
            if entry.repository_identity == repository_identity(REPOSITORY) {
                REPOSITORY
            } else {
                OTHER_REPOSITORY
            },
            &entry.change,
        );
        assert_eq!(
            (
                entry.lane,
                &entry.lanes,
                entry.lanes.values().sum::<usize>(),
                entry.open_decisions,
                entry.first_blocker.as_ref()
            ),
            (
                detail.overview.lane,
                &detail.overview.lanes,
                entry.units,
                detail.overview.open_decisions,
                detail.blockers.first()
            )
        );
    }
}

#[test]
// r[verify kiln.cairn.workspace.scenario.blockers]
fn blockers_rank_by_condition_and_name_their_target() {
    let detail = read_detail(&fixture(), CHANGE);
    let ranks: Vec<usize> = detail
        .blockers
        .iter()
        .map(|blocker| blocker_rank(blocker.condition))
        .collect();
    assert!(
        ranks
            .iter()
            .zip(ranks.iter().skip(1))
            .all(|(left, right)| left <= right),
        "{ranks:?}"
    );
    assert!(ranks.iter().all(|rank| *rank < BLOCKER_RANK.len()));
    assert_eq!(
        detail
            .overview
            .first_blocker
            .as_ref()
            .map(|blocker| blocker.condition),
        Some("dispatch_recovery")
    );
    for blocker in &detail.blockers {
        let expected = match blocker.condition {
            "dispatch_recovery"
            | "missing_dependency"
            | "no_acceptance_obligations"
            | "dependency_cycle" => ("unit", None),
            _ => ("member", Some(CAMPAIGN)),
        };
        assert_eq!(
            (blocker.subject_kind, blocker.campaign_id.as_deref()),
            expected,
            "{blocker:?}"
        );
    }
    assert!(
        detail
            .blockers
            .iter()
            .any(|blocker| blocker.subject_kind == "member")
    );
    assert_eq!(blocker_rank("unrecognized_condition"), BLOCKER_RANK.len());
}

#[test]
// r[verify kiln.cairn.workspace.scenario.dependencies]
fn dependents_name_current_work_waiting_on_a_unit_across_changes() {
    let fixture = fixture();
    let detail = read_detail(&fixture, CHANGE);
    let dependents = |native_id| -> Vec<(String, &'static str, Vec<String>, Option<String>)> {
        unit(&detail, native_id)
            .current_work
            .iter()
            .flat_map(|work| &work.dependents)
            .map(|view| {
                (
                    view.identity.clone(),
                    view.state,
                    view.native_ids.clone(),
                    view.repository_identity.clone(),
                )
            })
            .collect()
    };
    let task_b = unit(&detail, "chg/task-b")
        .current_work
        .first()
        .map(|work| work.identity.clone())
        .unwrap_or_else(|| unreachable!("task-b is current"));
    assert_eq!(
        dependents("chg/task-a"),
        vec![(
            task_b,
            "current",
            vec!["chg/task-b".to_owned()],
            Some(repository_identity(REPOSITORY))
        )]
    );
    assert!(dependents("chg/task-b").is_empty());
    // A dependency on work that is not current stays visible without a unit
    // or repository.
    let missing: Vec<(&str, &str, usize, Option<&str>)> = unit(&detail, "chg/task-x")
        .current_work
        .iter()
        .flat_map(|work| &work.dependencies)
        .map(|view| {
            (
                view.identity.as_str(),
                view.state,
                view.native_ids.len(),
                view.repository_identity.as_deref(),
            )
        })
        .collect();
    assert_eq!(missing, vec![(id('f').as_str(), "not_current", 0, None)]);

    // A unit of another change that depends on this change's work is named.
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let prerequisite = work(REPOSITORY, "chg/task-a", 1, '1', Vec::new());
    let prerequisite_identity = prerequisite.identity().unwrap_or_else(|_| unreachable!());
    let dependent = work(
        REPOSITORY,
        "next/task-z",
        1,
        '2',
        vec![prerequisite_identity],
    );
    for admitted in [&prerequisite, &dependent] {
        ledger
            .admit(admitted, None, &enrollment)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    }
    let snapshot = CanonicalSnapshot {
        work: Some(ledger),
        ..CanonicalSnapshot::default()
    };
    let crossed = super::detail(&snapshot, &key(REPOSITORY, CHANGE))
        .unwrap_or_else(|code| unreachable!("{code}"))
        .unwrap_or_else(|| unreachable!("chg is named"));
    let names: Vec<&str> = crossed
        .units
        .iter()
        .flat_map(|unit| &unit.current_work)
        .flat_map(|work| &work.dependents)
        .flat_map(|view| &view.native_ids)
        .map(String::as_str)
        .collect();
    assert_eq!(names, vec!["next/task-z"]);
}

#[test]
// r[verify kiln.cairn.workspace.scenario.lanes]
fn open_decisions_count_unissued_and_outstanding_requests_only() {
    let fixture = fixture();
    let mut snapshot = fixture.snapshot.clone();
    let (state, _) = snapshot
        .campaigns
        .first_mut()
        .unwrap_or_else(|| unreachable!("the fixture has a campaign"));
    for (byte, member, request_state) in [
        ('1', "chg/task-c", DecisionRequestState::Outstanding),
        ('2', "chg/task-d", DecisionRequestState::Unissued),
        ('3', "chg/task-c", DecisionRequestState::Resolved),
        ('4', "chg/task-c", DecisionRequestState::Expired),
        ('5', "other/*", DecisionRequestState::Outstanding),
    ] {
        state.decisions.insert(
            id(byte),
            kiln_core::implementation::campaign::HumanDecisionRequest {
                identity: id(byte),
                kind: DecisionKind::Preference,
                member: member.into(),
                work_identity: None,
                decision_revision: None,
                policy_identity: id('a'),
                candidate_identity: None,
                state: request_state,
                created_ms: 1,
                question: "which?".into(),
                options: vec!["one".into(), "two".into()],
                recommendation: None,
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            },
        );
    }
    let read = |change: &str| {
        detail(&snapshot, &key(REPOSITORY, change))
            .unwrap_or_else(|code| unreachable!("{code}"))
            .unwrap_or_else(|| unreachable!("{change} is named"))
    };
    let selected = read(CHANGE);
    assert_eq!(
        (selected.decisions.len(), selected.overview.open_decisions),
        (4, 2)
    );
    assert_eq!(read("other").overview.open_decisions, 1);
    assert_eq!(read_detail(&fixture, CHANGE).overview.open_decisions, 0);
}
