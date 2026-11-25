use super::agents::InventoryObservation;
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
    detail(
        &fixture.snapshot,
        &key(repository, change),
        &InventoryObservation::NotConfigured,
    )
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
        super::detail(
            &fixture.snapshot,
            &key(REPOSITORY, "absent"),
            &InventoryObservation::NotConfigured,
        )
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
    let crossed = super::detail(
        &snapshot,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
    )
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
        detail(
            &snapshot,
            &key(REPOSITORY, change),
            &InventoryObservation::NotConfigured,
        )
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

#[test]
// r[verify kiln.cairn.workspace.agents]
// r[verify kiln.cairn.workspace.agents.scenario.source]
fn the_agents_section_fails_apart_and_binds_the_view_identity() {
    let fixture = fixture();
    let read = |inventory: &InventoryObservation| {
        detail(&fixture.snapshot, &key(REPOSITORY, CHANGE), inventory)
            .unwrap_or_else(|code| unreachable!("{code}"))
            .unwrap_or_else(|| unreachable!("{CHANGE} is named"))
    };
    let unconfigured = read(&InventoryObservation::NotConfigured);
    let refused = read(&InventoryObservation::Unavailable(
        "inventory_input_unavailable".into(),
    ));
    assert_eq!(unconfigured.schema, "kiln.cairn-workspace-detail.v4");
    assert_eq!(
        (
            unconfigured.agents.source,
            unconfigured.agents.reason.as_deref()
        ),
        ("not_configured", None)
    );
    assert_eq!(
        (refused.agents.source, refused.agents.reason.as_deref()),
        ("unavailable", Some("inventory_input_unavailable"))
    );
    assert_eq!(
        (
            refused.agents.workflow_revisions.source,
            refused.agents.workflow_revisions.reason.as_deref()
        ),
        ("unavailable", Some("inventory_input_unavailable"))
    );
    // The canonical facts are the same; only the view identity moves.
    let canonical = |detail: &Detail| {
        serde_json::to_value((&detail.units, &detail.memberships, &detail.blockers))
            .unwrap_or_else(|error| unreachable!("{error}"))
    };
    assert_eq!(canonical(&unconfigured), canonical(&refused));
    assert_ne!(unconfigured.identity, refused.identity);
    assert_eq!(
        unconfigured.identity,
        read(&InventoryObservation::NotConfigured).identity
    );
}

/// The pilot inventory projection, with the workflow source naming `workflows`.
fn pilot_inventory(workflows: &str) -> InventoryObservation {
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inventory/producers");
    let read = || -> Result<_, String> {
        let mut set = crate::inventory_collection::CollectionSet::load(
            &fixtures.join("pilot.collection.json"),
        )?;
        for collection in set
            .collections
            .iter_mut()
            .filter(|c| c.source == "lattice-workflows")
        {
            collection.documents = vec![workflows.to_owned()];
        }
        let snapshot = set.collect(&fixtures)?;
        let profile: kiln_core::inventory::InventoryProfile = serde_json::from_slice(
            &std::fs::read(fixtures.join("profile.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        kiln_core::inventory::project_inventory(&profile, &snapshot, None, 1_795_237_895_000)
            .map_err(str::to_owned)
    };
    InventoryObservation::Admitted(Box::new(
        read().unwrap_or_else(|error| unreachable!("{error}")),
    ))
}

#[test]
// r[verify kiln.cairn.workspace.agents.scenario.workflow]
// r[verify kiln.cairn.workspace.agents.scenario.workflow_source]
fn workflow_revisions_render_in_the_detail_and_fail_apart() {
    let fixture = fixture();
    let read = |inventory: &InventoryObservation| {
        detail(&fixture.snapshot, &key(REPOSITORY, CHANGE), inventory)
            .unwrap_or_else(|code| unreachable!("{code}"))
            .unwrap_or_else(|| unreachable!("{CHANGE} is named"))
    };
    let rendered = read(&pilot_inventory(
        "lattice-workflows/workflow-4-second-revision.json",
    ));
    let workflows = &rendered.agents.workflow_revisions;
    assert_eq!(workflows.source, "admitted");
    let [workflow] = workflows.definitions.as_slice() else {
        unreachable!("one workflow definition")
    };
    assert_eq!(workflow.selected_for_new_sessions.state, "recorded");
    assert_eq!(workflow.pinned.len(), 2);

    let missing = read(&pilot_inventory("lattice-workflows/missing.json"));
    assert_eq!(
        (
            missing.agents.workflow_revisions.source,
            missing.agents.workflow_revisions.reason.as_deref()
        ),
        (
            "unavailable",
            Some("inventory_collection_document_unavailable")
        )
    );
    assert_eq!(missing.agents.source, "admitted");
    assert_eq!(missing.agents.definitions, rendered.agents.definitions);
    assert_eq!(missing.agents.workers, rendered.agents.workers);
    let canonical = |detail: &Detail| {
        serde_json::to_value((&detail.units, &detail.memberships, &detail.blockers))
            .unwrap_or_else(|error| unreachable!("{error}"))
    };
    assert_eq!(canonical(&missing), canonical(&rendered));
    assert_ne!(
        missing.identity, rendered.identity,
        "the view identity binds the workflow revisions it shows"
    );
}

#[test]
// r[verify kiln.source.projection.disclosure.cairn]
// r[verify kiln.source.projection.disclosure.unavailable]
fn restricted_cairn_observer_omits_hidden_changes_and_redacts_prerequisites() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRED_AT_S, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::ProviderProfileId;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut snap = fixture().snapshot;
    let w_secret = work(REPOSITORY, "secret/task-1", 1, 'a', Vec::new());
    let w_secret_id = w_secret
        .identity()
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_chg = work(REPOSITORY, "chg/task-z", 1, 'b', vec![w_secret_id.clone()]);
    let w_chg_id = w_chg.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    if let Some(ledger) = &mut snap.work {
        ledger
            .admit(&w_secret, None, &enrollment)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ledger
            .admit(&w_chg, None, &enrollment)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
    }
    let mut state = CampaignState::start(contract(
        "campaign-restricted",
        REPOSITORY,
        vec![
            binding(MemberSelector::Tasks {
                change: CHANGE.into(),
                tasks: vec!["task-z".into()],
            }),
            binding(MemberSelector::WholeChange {
                change: "secret".into(),
            }),
        ],
    ))
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    state
        .observe(observed("chg/task-z", MemberStage::Ready, Some(w_chg_id)))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    state
        .observe(observed("secret/*", MemberStage::Ready, Some(w_secret_id)))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    snap.campaigns.push((state, STORE_VERSION));

    let admitted_repo = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: REPOSITORY.into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: git("repository", REPOSITORY),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/tmp/kiln-cairn-restricted-fixture".into(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "cairn-restricted".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec![".cairn/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let rev = git("commit", &"a".repeat(40));
    let blob =
        ExternalRef::new("git", "blob", &"b".repeat(40)).unwrap_or_else(|e| unreachable!("{e:?}"));
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let facts = ProjectionFacts {
        repository: admitted_repo.identity().clone(),
        exact_revision: rev.clone(),
        canonical_tree: id('e'),
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: ".cairn/changes/chg/tasks.md".into(),
                object: blob.clone(),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: ".cairn/changes/secret/tasks.md".into(),
                object: blob,
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };

    // Grant Discover ONLY on `.cairn/changes/chg` (NOT ReadContent, NOT `secret`).
    let chg_scope = PathScope::cairn_change(CHANGE).unwrap_or_else(|| unreachable!("chg scope"));
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        admitted_repo.identity(),
        None,
        &[(chg_scope, vec![SourceViewOperation::Discover])],
        EXPIRES_S,
    );
    let mut authority = fixture.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = kiln_adapter_basalt::SourceHolderKey::new(
        kiln_adapter_basalt::test_support::seed(
            kiln_adapter_basalt::test_support::COORDINATOR_SEED,
        ),
        fixture.token.clone(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let signer = kiln_adapter_basalt::SourceSigner::new(
        &fixture.config(),
        holder,
        fixture.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let handle_key = HandleKey::from_bytes([0x55; 32]);
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    // 1. Index omits `secret` and keeps only `chg`.
    let r_idx = restricted_index(&snap, &mut observer)
        .unwrap_or_else(|e| unreachable!("restricted_index: {e}"));
    let visible_names: Vec<&str> = r_idx
        .index
        .changes
        .iter()
        .map(|c| c.change.as_str())
        .collect();
    assert_eq!(visible_names, vec![CHANGE]);

    // 2. Detail for hidden `secret` returns Ok(None), matching non-existent change.
    let hidden_detail = restricted_detail(
        &snap,
        &key(REPOSITORY, "secret"),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail secret: {e}"));
    assert!(hidden_detail.is_none());

    // 3. Detail for `chg` redacts text (since ReadContent was omitted) and
    //    redacts prerequisite `secret` without leaking its ID.
    let chg_detail = restricted_detail(
        &snap,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail chg: {e}"))
    .unwrap_or_else(|| unreachable!("chg detail"));
    let serialized = serde_json::to_string(&chg_detail).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(
        !serialized.contains("secret"),
        "hidden change leaked: {serialized}"
    );
    assert!(
        !serialized.contains("chg-task"),
        "ungranted text leaked: {serialized}"
    );
    assert!(serialized.contains("[unavailable]"));

    // 4. HTTP shell restricted_response fails closed with 404 unavailable when
    //    no observer is wired, and returns 403 for Role::Worker.
    let no_obs = http::restricted_response(
        None,
        "GET",
        http::INDEX_ROUTE,
        crate::engine::Role::Viewer,
        &|| InventoryObservation::NotConfigured,
        None,
    );
    assert_eq!(no_obs.status, 404);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&no_obs.body).ok(),
        Some(serde_json::json!({"error": "unavailable"}))
    );
    let worker_obs = http::restricted_response(
        None,
        "GET",
        http::INDEX_ROUTE,
        crate::engine::Role::Worker,
        &|| InventoryObservation::NotConfigured,
        Some(&mut observer),
    );
    assert_eq!(worker_obs.status, 403);

    // 5. Expired observer fails closed on restricted_index.
    observer.now_unix_s = EXPIRED_AT_S;
    assert_eq!(
        restricted_index(&snap, &mut observer).err(),
        Some("unavailable")
    );
}

fn restricted_cairn_fixture(
    entries: Vec<&str>,
    grants: &[(PathScope, Vec<SourceViewOperation>)],
) -> (
    AdmittedRepository,
    AdmittedDefinition,
    ProjectionFacts,
    SourceCurrentness,
    kiln_adapter_basalt::test_support::SourceFixture,
    HandleKey,
) {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture,
    };
    use kiln_core::implementation::authority::AttemptCurrentness;
    use kiln_core::projection::authority::SnapshotObservation;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionProfile, ProviderBinding, RepositoryEnrollment, RepositoryLocator, Selection,
        SourceCapability, SourceEntry, TargetProfile, admit_definition, admit_repositories,
    };
    use kiln_core::types::ProviderProfileId;

    let admitted_repo = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: REPOSITORY.into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: git("repository", REPOSITORY),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/tmp/kiln-cairn-restricted-fixture".into(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "cairn-restricted".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec![".cairn/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let rev = git("commit", &"a".repeat(40));
    let blob =
        ExternalRef::new("git", "blob", &"b".repeat(40)).unwrap_or_else(|e| unreachable!("{e:?}"));
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let facts = ProjectionFacts {
        repository: admitted_repo.identity().clone(),
        exact_revision: rev.clone(),
        canonical_tree: id('e'),
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: entries
            .into_iter()
            .map(|path| SourceEntry {
                path: path.into(),
                object: blob.clone(),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        admitted_repo.identity(),
        None,
        grants,
        EXPIRES_S,
    );
    let handle_key = HandleKey::from_bytes([0x55; 32]);
    (admitted_repo, def, facts, current, fixture, handle_key)
}

#[test]
fn restricted_cairn_observer_redacts_missing_dependency_first_blocker_and_block_reasons() {
    use kiln_adapter_basalt::test_support::VERIFY_AT_S;
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let w_secret_v1 = work(REPOSITORY, "secret/task-1", 1, 'a', Vec::new());
    let w_secret_v1_id = w_secret_v1
        .identity()
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_secret_v2 = work(REPOSITORY, "secret/task-1", 2, 'b', Vec::new());
    let w_chg = work(
        REPOSITORY,
        "chg/task-z",
        1,
        'c',
        vec![w_secret_v1_id.clone()],
    );
    for w in [&w_secret_v1, &w_secret_v2, &w_chg] {
        ledger
            .admit(w, None, &enrollment)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
    }
    let snap = CanonicalSnapshot {
        work: Some(ledger),
        ..CanonicalSnapshot::default()
    };

    let chg_scope = PathScope::cairn_change(CHANGE).unwrap_or_else(|| unreachable!("chg scope"));
    let (admitted_repo, def, facts, current, fixture, handle_key) = restricted_cairn_fixture(
        vec![
            ".cairn/changes/chg/tasks.md",
            ".cairn/changes/secret/tasks.md",
        ],
        &[(chg_scope, vec![SourceViewOperation::Discover])],
    );
    let mut authority = fixture.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = kiln_adapter_basalt::SourceHolderKey::new(
        kiln_adapter_basalt::test_support::seed(
            kiln_adapter_basalt::test_support::COORDINATOR_SEED,
        ),
        fixture.token.clone(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let signer = kiln_adapter_basalt::SourceSigner::new(
        &fixture.config(),
        holder,
        fixture.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    let r_idx = restricted_index(&snap, &mut observer)
        .unwrap_or_else(|e| unreachable!("restricted_index: {e}"));
    let idx_json = serde_json::to_string(&r_idx).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(
        !idx_json.contains(w_secret_v1_id.as_str()),
        "hidden superseded work id leaked in restricted_index: {idx_json}"
    );
    assert_eq!(
        r_idx
            .index
            .changes
            .first()
            .and_then(|c| c.first_blocker.as_ref())
            .map(|b| b.detail.as_str()),
        Some("prerequisite unavailable")
    );

    let r_det = restricted_detail(
        &snap,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail: {e}"))
    .unwrap_or_else(|| unreachable!("chg detail"));
    let det_json = serde_json::to_string(&r_det).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(
        !det_json.contains(w_secret_v1_id.as_str()),
        "hidden superseded work id leaked in restricted_detail: {det_json}"
    );
    assert_eq!(
        r_det.detail.overview.first_blocker.as_ref(),
        r_det.detail.blockers.first()
    );
    let block_reasons = r_det
        .detail
        .units
        .first()
        .and_then(|u| u.current_work.first())
        .map(|w| &w.block_reasons)
        .unwrap_or_else(|| unreachable!("work view"));
    assert_eq!(
        block_reasons,
        &vec![BlockReason::MissingDependency { identity: id('0') }]
    );
}

#[test]
fn restricted_cairn_observer_redacts_hidden_membership_dependencies_and_discover_only_text() {
    use kiln_adapter_basalt::test_support::VERIFY_AT_S;
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::implementation::campaign::{
        DecisionKind, DecisionRequestState, HumanDecisionRequest,
    };
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let mut attempts = AttemptLedger::default();
    let w_chg = work(REPOSITORY, "chg/task-z", 1, 'a', Vec::new());
    let w_chg_id = w_chg.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    ledger
        .admit(&w_chg, None, &enrollment)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let att = attempts
        .claim(&w_chg, NOW_MS, LEASE_MS)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    attempts
        .fail(
            &att.identity,
            att.fence,
            NOW_MS + 1,
            FailureClass::Unknown,
            "secret-cause",
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let mut state = CampaignState::start(contract(
        "campaign-text-redact",
        REPOSITORY,
        vec![binding(MemberSelector::Tasks {
            change: CHANGE.into(),
            tasks: vec!["task-z".into()],
        })],
    ))
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut obs = observed("chg/task-z", MemberStage::Ready, Some(w_chg_id.clone()));
    obs.dependencies = vec!["secret/*".into()];
    obs.blockers = vec!["secret-member-blocker".into()];
    state.observe(obs).unwrap_or_else(|e| unreachable!("{e:?}"));
    state.decisions.insert(
        id('1'),
        HumanDecisionRequest {
            identity: id('1'),
            kind: DecisionKind::Preference,
            member: "chg/task-z".into(),
            work_identity: Some(w_chg_id),
            decision_revision: None,
            policy_identity: id('a'),
            candidate_identity: None,
            state: DecisionRequestState::Outstanding,
            created_ms: 1,
            question: "secret-question".into(),
            options: vec!["secret-opt".into()],
            recommendation: Some("secret-rec".into()),
            priority: 1,
            pending_reason: Some("secret-pend".into()),
            resolved_answer: Some("secret-ans".into()),
            active_effort_ms: None,
        },
    );

    let snap = CanonicalSnapshot {
        work: Some(ledger),
        attempts: Some(attempts),
        campaigns: vec![(state, STORE_VERSION)],
        ..CanonicalSnapshot::default()
    };

    let chg_scope = PathScope::cairn_change(CHANGE).unwrap_or_else(|| unreachable!("chg scope"));
    let (admitted_repo, def, facts, current, fixture, handle_key) = restricted_cairn_fixture(
        vec![".cairn/changes/chg/tasks.md"],
        &[(chg_scope, vec![SourceViewOperation::Discover])],
    );
    let mut authority = fixture.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = kiln_adapter_basalt::SourceHolderKey::new(
        kiln_adapter_basalt::test_support::seed(
            kiln_adapter_basalt::test_support::COORDINATOR_SEED,
        ),
        fixture.token.clone(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let signer = kiln_adapter_basalt::SourceSigner::new(
        &fixture.config(),
        holder,
        fixture.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    let r_det = restricted_detail(
        &snap,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail: {e}"))
    .unwrap_or_else(|| unreachable!("chg detail"));
    let serialized = serde_json::to_string(&r_det).unwrap_or_else(|e| unreachable!("{e:?}"));
    for forbidden in [
        "secret",
        "secret-member-blocker",
        "secret-cause",
        "secret-question",
        "secret-opt",
        "secret-rec",
        "secret-pend",
        "secret-ans",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "ungranted text {forbidden:?} leaked in {serialized}"
        );
    }
}

#[test]
fn restricted_cairn_observer_distinguishes_distinct_hidden_prerequisite_handles() {
    use kiln_adapter_basalt::test_support::VERIFY_AT_S;
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let w_secret1 = work(REPOSITORY, "secret1/task-1", 1, 'a', Vec::new());
    let w_secret1_id = w_secret1
        .identity()
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_secret2 = work(REPOSITORY, "secret2/task-1", 1, 'b', Vec::new());
    let w_secret2_id = w_secret2
        .identity()
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_chg = work(
        REPOSITORY,
        "chg/task-z",
        1,
        'c',
        vec![w_secret1_id, w_secret2_id],
    );
    let w_chg_id = w_chg.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_secret3 = work(REPOSITORY, "secret3/task-1", 1, 'd', vec![w_chg_id]);
    for w in [&w_secret1, &w_secret2, &w_chg, &w_secret3] {
        ledger
            .admit(w, None, &enrollment)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
    }
    let snap = CanonicalSnapshot {
        work: Some(ledger),
        ..CanonicalSnapshot::default()
    };

    let chg_scope = PathScope::cairn_change(CHANGE).unwrap_or_else(|| unreachable!("chg scope"));
    let (admitted_repo, def, facts, current, fixture, handle_key) = restricted_cairn_fixture(
        vec![
            ".cairn/changes/chg/tasks.md",
            ".cairn/changes/secret1/tasks.md",
            ".cairn/changes/secret2/tasks.md",
            ".cairn/changes/secret3/tasks.md",
        ],
        &[(chg_scope, vec![SourceViewOperation::Discover])],
    );
    let mut authority = fixture.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = kiln_adapter_basalt::SourceHolderKey::new(
        kiln_adapter_basalt::test_support::seed(
            kiln_adapter_basalt::test_support::COORDINATOR_SEED,
        ),
        fixture.token.clone(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let signer = kiln_adapter_basalt::SourceSigner::new(
        &fixture.config(),
        holder,
        fixture.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    let r_det = restricted_detail(
        &snap,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail: {e}"))
    .unwrap_or_else(|| unreachable!("chg detail"));
    let w_view = r_det
        .detail
        .units
        .first()
        .and_then(|u| u.current_work.first())
        .unwrap_or_else(|| unreachable!("work view"));
    let dep0 = w_view
        .dependencies
        .first()
        .map(|d| d.identity.as_str())
        .unwrap_or_else(|| unreachable!("dep0"));
    let dep1 = w_view
        .dependencies
        .get(1)
        .map(|d| d.identity.as_str())
        .unwrap_or_else(|| unreachable!("dep1"));
    let depnt = w_view
        .dependents
        .first()
        .map(|d| d.identity.as_str())
        .unwrap_or_else(|| unreachable!("depnt"));
    assert!(dep0.starts_with("handle:"), "{dep0}");
    assert!(dep1.starts_with("handle:"), "{dep1}");
    assert!(depnt.starts_with("handle:"), "{depnt}");
    assert_ne!(dep0, dep1);
    assert_ne!(depnt, dep0);
    assert_ne!(depnt, dep1);

    let ctx_prereqs: BTreeSet<String> = r_det
        .restricted_context
        .pointer("/items/0/prerequisites")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(|h| format!("handle:{h}"))
                .collect()
        })
        .unwrap_or_default();
    assert!(ctx_prereqs.contains(dep0), "{ctx_prereqs:?} vs {dep0}");
    assert!(ctx_prereqs.contains(dep1), "{ctx_prereqs:?} vs {dep1}");
}

#[test]
fn restricted_index_filters_before_truncation_bound() {
    use kiln_adapter_basalt::test_support::VERIFY_AT_S;
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let mut members = Vec::new();
    for i in 0..512 {
        members.push(binding(MemberSelector::WholeChange {
            change: format!("aaa-{i:03}"),
        }));
    }
    let camp = CampaignState::start(contract("camp-512", REPOSITORY, members))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_vis = work(REPOSITORY, "zzz-vis/task-1", 1, 'a', Vec::new());
    ledger
        .admit(&w_vis, None, &enrollment)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_vis2 = work(REPOSITORY, "zzz-vis2/task-1", 1, 'b', Vec::new());
    let mut ledger_two = ledger.clone();
    ledger_two
        .admit(&w_vis2, None, &enrollment)
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let snap = CanonicalSnapshot {
        work: Some(ledger),
        campaigns: vec![(camp.clone(), STORE_VERSION)],
        ..CanonicalSnapshot::default()
    };
    let snap_two = CanonicalSnapshot {
        work: Some(ledger_two),
        campaigns: vec![(camp, STORE_VERSION)],
        ..CanonicalSnapshot::default()
    };

    let vis_scope = PathScope::cairn_change("zzz-vis").unwrap_or_else(|| unreachable!("vis scope"));
    let vis2_scope =
        PathScope::cairn_change("zzz-vis2").unwrap_or_else(|| unreachable!("vis2 scope"));
    let (admitted_repo, def, facts, current, fixture, handle_key) = restricted_cairn_fixture(
        vec![
            ".cairn/changes/zzz-vis/tasks.md",
            ".cairn/changes/zzz-vis2/tasks.md",
        ],
        &[
            (vis_scope, vec![SourceViewOperation::Discover]),
            (vis2_scope, vec![SourceViewOperation::Discover]),
        ],
    );
    let mut authority = fixture.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = kiln_adapter_basalt::SourceHolderKey::new(
        kiln_adapter_basalt::test_support::seed(
            kiln_adapter_basalt::test_support::COORDINATOR_SEED,
        ),
        fixture.token.clone(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let signer = kiln_adapter_basalt::SourceSigner::new(
        &fixture.config(),
        holder,
        fixture.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    let r_idx = restricted_index(&snap, &mut observer)
        .unwrap_or_else(|e| unreachable!("restricted_index: {e}"));
    let names: Vec<&str> = r_idx
        .index
        .changes
        .iter()
        .map(|c| c.change.as_str())
        .collect();
    assert_eq!(names, vec!["zzz-vis"]);
    assert!(!r_idx.index.truncated);

    let r_idx_bounded = restricted_index_bounded(&snap_two, &mut observer, 1)
        .unwrap_or_else(|e| unreachable!("restricted_index_bounded: {e}"));
    assert_eq!(r_idx_bounded.index.changes.len(), 1);
    assert!(r_idx_bounded.index.truncated);
}

#[test]
fn restricted_cairn_observer_scales_past_max_source_grants_under_basalt_broad_grant() {
    use kiln_adapter_basalt::test_support::{COORDINATOR_SEED, SourceFixture, VERIFY_AT_S, seed};
    use kiln_adapter_basalt::{SourceHolderKey, SourceSigner};
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };

    let build_snap_and_entries = |vis_count: usize, hidden_count: usize| {
        let mut ledger = WorkLedger::default();
        let mut paths = Vec::with_capacity(vis_count + hidden_count);
        for i in 0..vis_count {
            let w = work(
                REPOSITORY,
                &format!("vis-{i:02}/task-1"),
                1,
                char::from(b'a' + u8::try_from(i % 6).unwrap_or(0)),
                Vec::new(),
            );
            ledger
                .admit(&w, None, &enrollment)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            paths.push(format!(".cairn/changes/vis-{i:02}/tasks.md"));
        }
        for i in 0..hidden_count {
            let w = work(
                REPOSITORY,
                &format!("secret-{i:02}/task-1"),
                1,
                'f',
                Vec::new(),
            );
            ledger
                .admit(&w, None, &enrollment)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            paths.push(format!(".cairn/changes/secret-{i:02}/tasks.md"));
        }
        let snap = CanonicalSnapshot {
            work: Some(ledger),
            ..CanonicalSnapshot::default()
        };
        (snap, paths)
    };

    let make_u64_signer = |fixture: &SourceFixture| {
        let nonce_counter = std::cell::Cell::new(0_u64);
        let holder = SourceHolderKey::new(seed(COORDINATOR_SEED), fixture.token.clone())
            .unwrap_or_else(|e| unreachable!("{e}"));
        SourceSigner::new(
            &fixture.config(),
            holder,
            fixture.max_delegation_depth,
            Box::new(move || {
                let next = nonce_counter.get().checked_add(1)?;
                nonce_counter.set(next);
                let mut bytes = [0_u8; 32];
                if let Some(head) = bytes.get_mut(..8) {
                    head.copy_from_slice(&next.to_le_bytes());
                }
                Some(bytes)
            }),
        )
        .unwrap_or_else(|e| unreachable!("{e}"))
    };

    // 1. 35 visible changes under PathScope::root() grant.
    let (snap35, paths35) = build_snap_and_entries(35, 0);
    let path_refs35: Vec<&str> = paths35.iter().map(String::as_str).collect();
    let (admitted_repo, def, facts35, current, fixture35, handle_key) = restricted_cairn_fixture(
        path_refs35,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
    );
    let mut authority35 = fixture35.authority();
    let signer35 = make_u64_signer(&fixture35);
    let sign35 = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer35
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut obs35 = RestrictedCairnObserver {
        authority: &mut authority35,
        profile: RequestProfile::Signer(&sign35),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts35,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };
    let r_idx35 = restricted_index(&snap35, &mut obs35)
        .unwrap_or_else(|e| unreachable!("restricted_index 35: {e}"));
    assert_eq!(r_idx35.index.changes.len(), 35);
    assert!(!r_idx35.index.truncated);
    let items35 = r_idx35
        .restricted_context
        .get("items")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| unreachable!("items35"));
    assert_eq!(items35.len(), 35);
    assert!(items35.iter().all(|item| {
        item.get("text")
            .and_then(serde_json::Value::as_str)
            .is_some()
    }));
    assert!(
        fixture35.replay.calls() <= 36,
        "ReadContent subtree probes must not waste replay entries, got {}",
        fixture35.replay.calls()
    );

    // 2. 70 visible changes + 5 hidden changes under `.cairn/changes/vis-*` + `.cairn/changes`?
    // Test `.cairn/changes/**` broad grant with 70 visible changes and also check 5 hidden
    // changes when only `vis-00..vis-69` are granted via `.cairn/changes/vis-*` or broad `.cairn/changes/**`.
    let (snap70, paths70) = build_snap_and_entries(70, 5);
    let path_refs70: Vec<&str> = paths70.iter().map(String::as_str).collect();
    let changes_scope =
        PathScope::from_path(".cairn/changes", true).unwrap_or_else(|| unreachable!("changes"));
    let (admitted_repo70, def70, facts70, current70, fixture70, handle_key70) =
        restricted_cairn_fixture(
            path_refs70,
            &[(
                changes_scope,
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            )],
        );
    let mut authority70 = fixture70.authority();
    let signer70 = make_u64_signer(&fixture70);
    let sign70 = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer70
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut obs70 = RestrictedCairnObserver {
        authority: &mut authority70,
        profile: RequestProfile::Signer(&sign70),
        current: &current70,
        repository_id: admitted_repo70.identity(),
        definition: &def70,
        admitted_repository: &admitted_repo70,
        facts: &facts70,
        handle_key: &handle_key70,
        now_unix_s: VERIFY_AT_S,
    };
    let r_idx70 = restricted_index(&snap70, &mut obs70)
        .unwrap_or_else(|e| unreachable!("restricted_index 70: {e}"));
    assert_eq!(r_idx70.index.changes.len(), 75);
    assert!(!r_idx70.index.truncated);
    let r_det_vis69 = restricted_detail(
        &snap70,
        &key(REPOSITORY, "vis-69"),
        &InventoryObservation::NotConfigured,
        &mut obs70,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail vis-69: {e}"))
    .unwrap_or_else(|| unreachable!("vis-69 detail"));
    let det_json = serde_json::to_string(&r_det_vis69).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(
        !det_json.contains("[unavailable]"),
        "expected unredacted detail for vis-69: {det_json}"
    );
}

#[test]
fn restricted_cairn_observer_broad_read_and_narrow_grants_under_local_and_basalt() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, SourceFixture, VERIFY_AT_S, seed,
    };
    use kiln_adapter_basalt::{SourceHolderKey, SourceSigner};
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::{
        LocalSourceAuthority, LocalSourceGrant, SourceAuthorityRequest, SourceDenialReason,
    };

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };

    let build_snap_and_entries = |vis_count: usize, hidden_count: usize| {
        let mut ledger = WorkLedger::default();
        let mut paths = Vec::with_capacity(vis_count + hidden_count);
        for i in 0..vis_count {
            let w = work(
                REPOSITORY,
                &format!("vis-{i:02}/task-1"),
                1,
                char::from(b'a' + u8::try_from(i % 6).unwrap_or(0)),
                Vec::new(),
            );
            ledger
                .admit(&w, None, &enrollment)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            paths.push(format!(".cairn/changes/vis-{i:02}/tasks.md"));
        }
        for i in 0..hidden_count {
            let w = work(
                REPOSITORY,
                &format!("secret-{i:02}/task-1"),
                1,
                'f',
                Vec::new(),
            );
            ledger
                .admit(&w, None, &enrollment)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            paths.push(format!(".cairn/changes/secret-{i:02}/tasks.md"));
        }
        let snap = CanonicalSnapshot {
            work: Some(ledger),
            ..CanonicalSnapshot::default()
        };
        (snap, paths)
    };

    // 1. LocalSourceAuthority with PathScope::root() and 70 visible changes.
    let (snap70, paths70) = build_snap_and_entries(70, 0);
    let path_refs70: Vec<&str> = paths70.iter().map(String::as_str).collect();
    let (admitted_repo, def, facts70, current70, _unused_fix, handle_key) =
        restricted_cairn_fixture(
            path_refs70,
            &[(
                PathScope::root(),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            )],
        );
    let mut local_auth = LocalSourceAuthority::new(
        "operator@host",
        LocalSourceGrant {
            repositories: BTreeSet::from([admitted_repo.identity().clone()]),
            operations: BTreeSet::from([
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ]),
        },
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let local_profile = AuthorityProfile::LocalOperator {
        operator: "operator@host".into(),
    };
    let mut local_obs = RestrictedCairnObserver {
        authority: &mut local_auth,
        profile: RequestProfile::Static(&local_profile),
        current: &current70,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts70,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };
    let local_idx = restricted_index(&snap70, &mut local_obs)
        .unwrap_or_else(|e| unreachable!("local restricted_index: {e}"));
    assert_eq!(local_idx.index.changes.len(), 70);
    assert!(!local_idx.index.truncated);
    let local_items = local_idx
        .restricted_context
        .get("items")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| unreachable!("local items"));
    assert_eq!(local_items.len(), 70);
    assert!(local_items.iter().all(|item| {
        item.get("text")
            .and_then(serde_json::Value::as_str)
            .is_some()
    }));
    let local_det69 = restricted_detail(
        &snap70,
        &key(REPOSITORY, "vis-69"),
        &InventoryObservation::NotConfigured,
        &mut local_obs,
    )
    .unwrap_or_else(|e| unreachable!("local restricted_detail vis-69: {e}"))
    .unwrap_or_else(|| unreachable!("local vis-69 detail"));
    let local_det_json =
        serde_json::to_string(&local_det69).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(!local_det_json.contains("[unavailable]"));

    // 2. BasaltSourceAuthority with 35 narrow per-change Discover grants + 6
    // ReadContent grants (fitting within UCAN's token byte/capability limits)
    // + 5 hidden changes.
    let (snap35, paths35) = build_snap_and_entries(35, 5);
    let path_refs35: Vec<&str> = paths35.iter().map(String::as_str).collect();
    let narrow_grants: Vec<(PathScope, Vec<SourceViewOperation>)> = (0..35)
        .map(|i| {
            let ops = if i < 5 || i == 34 {
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ]
            } else {
                vec![SourceViewOperation::Discover]
            };
            (
                PathScope::cairn_change(&format!("vis-{i:02}"))
                    .unwrap_or_else(|| unreachable!("cairn_change")),
                ops,
            )
        })
        .collect();
    let (admitted_repo35, def35, facts35, current35, fixture35, handle_key35) =
        restricted_cairn_fixture(path_refs35, &narrow_grants);
    let mut authority35 = fixture35.authority();
    let nonce_counter = std::cell::Cell::new(0_u64);
    let holder = SourceHolderKey::new(seed(COORDINATOR_SEED), fixture35.token.clone())
        .unwrap_or_else(|e| unreachable!("{e}"));
    let signer35 = SourceSigner::new(
        &fixture35.config(),
        holder,
        fixture35.max_delegation_depth,
        Box::new(move || {
            let next = nonce_counter.get().checked_add(1)?;
            nonce_counter.set(next);
            let mut bytes = [0_u8; 32];
            if let Some(head) = bytes.get_mut(..8) {
                head.copy_from_slice(&next.to_le_bytes());
            }
            Some(bytes)
        }),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let sign35 = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer35
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut obs35 = RestrictedCairnObserver {
        authority: &mut authority35,
        profile: RequestProfile::Signer(&sign35),
        current: &current35,
        repository_id: admitted_repo35.identity(),
        definition: &def35,
        admitted_repository: &admitted_repo35,
        facts: &facts35,
        handle_key: &handle_key35,
        now_unix_s: VERIFY_AT_S,
    };
    let narrow_idx = restricted_index(&snap35, &mut obs35)
        .unwrap_or_else(|e| unreachable!("narrow restricted_index: {e}"));
    assert_eq!(narrow_idx.index.changes.len(), 35);
    assert!(
        narrow_idx
            .index
            .changes
            .iter()
            .all(|c| c.change.starts_with("vis-"))
    );
    let narrow_det34 = restricted_detail(
        &snap35,
        &key(REPOSITORY, "vis-34"),
        &InventoryObservation::NotConfigured,
        &mut obs35,
    )
    .unwrap_or_else(|e| unreachable!("narrow restricted_detail vis-34: {e}"))
    .unwrap_or_else(|| unreachable!("narrow vis-34 detail"));
    let narrow_det_json =
        serde_json::to_string(&narrow_det34).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(!narrow_det_json.contains("[unavailable]"));

    // 3. Direct BasaltSourceAuthority::admit on ReadContent with PathScope::root()
    // returns RequestInvalid and consumes 0 replay entries.
    let fixture_root = SourceFixture::direct(
        COORDINATOR_SEED,
        admitted_repo35.identity(),
        None,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        EXPIRES_S,
    );
    let mut auth_root = fixture_root.authority();
    let req = SourceAuthorityRequest {
        operation: SourceViewOperation::ReadContent,
        repository: admitted_repo35.identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: facts35.exact_revision.clone(),
        },
        view_binding: def35.identity().clone(),
        scope: OriginScope::Components {
            path: PathScope::root(),
        },
        request_digest: [0x11; 32],
        attempt: None,
    };
    let static_cred = kiln_core::implementation::authority::AuthorityCredential::new(
        "did:key:holder",
        fixture_root.token.clone(),
        [1_u8; 32],
        [2_u8; 64],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let static_profile = AuthorityProfile::VerifiedUcan(static_cred);
    let denied = auth_root
        .admit(&req, &static_profile, &current35, VERIFY_AT_S)
        .err()
        .unwrap_or_else(|| unreachable!("expected subtree ReadContent denial"));
    assert_eq!(denied.reason, SourceDenialReason::RequestInvalid);
    assert_eq!(fixture_root.replay.calls(), 0);
}

#[test]
fn restricted_cairn_observer_redacts_hidden_campaign_count_in_snapshot_facts() {
    use kiln_adapter_basalt::test_support::VERIFY_AT_S;
    use kiln_core::implementation::authority::AuthorityProfile;
    use kiln_core::projection::authority::SourceAuthorityRequest;

    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string(), OTHER_REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut ledger = WorkLedger::default();
    let w_vis = work(REPOSITORY, "chg/task-1", 1, 'a', Vec::new());
    let w_vis_id = w_vis.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_hid = work(REPOSITORY, "secret/task-1", 1, 'b', Vec::new());
    let w_hid_id = w_hid.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    let w_oth = work(OTHER_REPOSITORY, "chg/other-task-1", 1, 'c', Vec::new());
    let w_oth_id = w_oth.identity().unwrap_or_else(|e| unreachable!("{e:?}"));
    for w in [&w_vis, &w_hid, &w_oth] {
        ledger
            .admit(w, None, &enrollment)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
    }

    let mut camp_vis = CampaignState::start(contract(
        "camp-vis",
        REPOSITORY,
        vec![binding(MemberSelector::WholeChange {
            change: CHANGE.into(),
        })],
    ))
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    camp_vis
        .observe(observed("chg/*", MemberStage::Ready, Some(w_vis_id)))
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let mut camp_hidden = CampaignState::start(contract(
        "camp-hidden",
        REPOSITORY,
        vec![binding(MemberSelector::WholeChange {
            change: "secret".into(),
        })],
    ))
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    camp_hidden
        .observe(observed("secret/*", MemberStage::Ready, Some(w_hid_id)))
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let mut camp_other = CampaignState::start(contract(
        "camp-other",
        OTHER_REPOSITORY,
        vec![binding(MemberSelector::Tasks {
            change: CHANGE.into(),
            tasks: vec!["other-task-1".into()],
        })],
    ))
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    camp_other
        .observe(observed(
            "chg/other-task-1",
            MemberStage::Ready,
            Some(w_oth_id),
        ))
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let snap = CanonicalSnapshot {
        work: Some(ledger),
        campaigns: vec![
            (camp_vis, STORE_VERSION),
            (camp_hidden, STORE_VERSION),
            (camp_other, STORE_VERSION),
        ],
        ..CanonicalSnapshot::default()
    };

    let chg_scope = PathScope::cairn_change(CHANGE).unwrap_or_else(|| unreachable!("chg scope"));
    let (admitted_repo, def, facts, current, fixture, handle_key) = restricted_cairn_fixture(
        vec![
            ".cairn/changes/chg/tasks.md",
            ".cairn/changes/secret/tasks.md",
        ],
        &[(chg_scope, vec![SourceViewOperation::Discover])],
    );
    let mut authority = fixture.authority();
    let signer = fixture.signer();
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut observer = RestrictedCairnObserver {
        authority: &mut authority,
        profile: RequestProfile::Signer(&sign),
        current: &current,
        repository_id: admitted_repo.identity(),
        definition: &def,
        admitted_repository: &admitted_repo,
        facts: &facts,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };

    let r_idx = restricted_index(&snap, &mut observer)
        .unwrap_or_else(|e| unreachable!("restricted_index: {e}"));
    assert_eq!(r_idx.index.snapshot.campaigns_read, 1);

    let r_det = restricted_detail(
        &snap,
        &key(REPOSITORY, CHANGE),
        &InventoryObservation::NotConfigured,
        &mut observer,
    )
    .unwrap_or_else(|e| unreachable!("restricted_detail: {e}"))
    .unwrap_or_else(|| unreachable!("chg detail"));
    assert_eq!(r_det.detail.snapshot.campaigns_read, 1);
}
