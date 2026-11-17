//! Operator commands over the real SQLite campaign store.
use super::*;
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CompletionGoal, DecisionKind, EnumerationState, Freshness, MemberBinding,
    MemberSelector, WorkflowProfile,
};

const CAMPAIGN: &str = "operator-commands";
const MEMBER: &str = "change-a/*";

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn contract() -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: CAMPAIGN.into(),
        revision: 1,
        repository: "repo".into(),
        members: vec![MemberBinding {
            selector: MemberSelector::WholeChange {
                change: "change-a".into(),
            },
            profile: WorkflowProfile::SpecDriven,
            profile_version: id('a'),
            repository_instructions: id('a'),
            policy: id('a'),
            required_checks: Vec::new(),
            required_reviews: vec!["maintainer".into()],
            integration_policy: id('a'),
            goal: CompletionGoal::VerifiedPullRequest,
            prerequisite_inclusion: false,
            context: Vec::new(),
        }],
        max_concurrency: 1,
        max_outstanding_decisions: 1,
        parent_budget_id: id('a'),
        agent_definition: id('a'),
        require_one_writer_per_repository: true,
    }
}

/// A verified candidate `c` awaiting review.
fn verified(evidence: char) -> MemberObservation {
    MemberObservation {
        member: MEMBER.into(),
        stage: MemberStage::VerifiedCandidate,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: Some(id('a')),
        integration_target_identity: None,
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: Default::default(),
        dependency_reason: None,
        canonical_work_key: Some(id('a')),
        work_identity: Some(id('a')),
        attempt_identity: None,
        attempt_fence: None,
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::NotApplicable,
        candidate_freshness: Freshness::Current,
        policy_freshness: Freshness::Current,
        verifier_freshness: Freshness::Current,
        target_freshness: Freshness::Unknown,
        evidence: vec![id(evidence)],
        blockers: Vec::new(),
        required_checks_satisfied: true,
        required_reviews_satisfied: false,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

fn decision(identity: char, kind: DecisionKind) -> HumanDecisionRequest {
    HumanDecisionRequest {
        identity: id(identity),
        kind,
        member: MEMBER.into(),
        work_identity: Some(id('a')),
        decision_revision: None,
        policy_identity: id('a'),
        candidate_identity: Some(id('c')),
        state: DecisionRequestState::Outstanding,
        created_ms: 1,
        question: "review the verified candidate".into(),
        options: vec!["approve".into(), "reject".into()],
        recommendation: Some("approve".into()),
        priority: 1,
        pending_reason: None,
        resolved_answer: None,
        active_effort_ms: None,
    }
}

fn command(request_id: &str, intent: OperatorIntent) -> OperatorCommand {
    OperatorCommand {
        request_id: request_id.into(),
        actor: "local:operator".into(),
        intent,
    }
}

fn version(store: &CampaignStore) -> u64 {
    store
        .version(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("campaign version"))
}

fn applied(store: &mut CampaignStore, command: OperatorCommand) -> AppliedCommand {
    store
        .apply_operator(command)
        .unwrap_or_else(|error| unreachable!("{error:?}"))
}

/// A started campaign whose member awaits an outstanding review `e` and an
/// unissued preference `f` beyond the outstanding bound.
fn seeded(store: &mut CampaignStore) {
    applied(
        store,
        command(
            "start-request",
            OperatorIntent::Start {
                contract: contract(),
            },
        ),
    );
    store
        .observe(CAMPAIGN, version(store), verified('c'))
        .unwrap_or_else(|_| unreachable!());
    store
        .add_decision(
            CAMPAIGN,
            version(store),
            decision('e', DecisionKind::RequiredReview),
        )
        .unwrap_or_else(|_| unreachable!());
    store
        .add_decision(
            CAMPAIGN,
            version(store),
            decision('f', DecisionKind::Preference),
        )
        .unwrap_or_else(|_| unreachable!());
}

fn resolve(expected_version: u64, identity: char, answer: &str) -> OperatorIntent {
    OperatorIntent::ResolveDecision {
        campaign_id: CAMPAIGN.into(),
        expected_version,
        identity: id(identity),
        work_identity: Some(id('a')),
        policy_identity: id('a'),
        candidate_identity: Some(id('c')),
        answer: answer.into(),
    }
}

fn refused_code(result: Result<AppliedCommand, OperatorFailure>) -> String {
    match result {
        Err(OperatorFailure::Refused { code, .. }) => code,
        other => unreachable!("expected an owner refusal, got {other:?}"),
    }
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.replay]
fn an_applied_request_replays_its_outcome_and_a_reused_id_conflicts() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let start = command(
        "start-request",
        OperatorIntent::Start {
            contract: contract(),
        },
    );
    let first = applied(&mut store, start.clone());
    assert!(!first.replayed);
    assert_eq!(first.outcome.store_version, 1);
    assert_eq!(first.outcome.admission, AdmissionState::Open);
    let pause = command(
        "pause-request",
        OperatorIntent::Pause {
            campaign_id: CAMPAIGN.into(),
            expected_version: 1,
        },
    );
    let paused = applied(&mut store, pause.clone());
    assert_eq!(
        (paused.outcome.store_version, paused.outcome.admission),
        (2, AdmissionState::Paused)
    );

    // The responses were lost; both retries return the recorded outcomes.
    let replayed_start = applied(&mut store, start.clone());
    assert!(replayed_start.replayed);
    assert_eq!(replayed_start.outcome, first.outcome);
    assert_eq!(applied(&mut store, pause.clone()).outcome, paused.outcome);
    assert_eq!(version(&store), 2, "no retry transitioned again");

    let mut other_actor = pause.clone();
    other_actor.actor = "local:other".into();
    let mut other_intent = pause.clone();
    other_intent.intent = OperatorIntent::Cancel {
        campaign_id: CAMPAIGN.into(),
        expected_version: 2,
    };
    for conflicting in [other_actor, other_intent] {
        assert_eq!(
            store.apply_operator(conflicting),
            Err(OperatorFailure::RequestConflict)
        );
    }
    drop(store);

    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let after_restart = applied(&mut store, pause);
    assert!(after_restart.replayed);
    assert_eq!(after_restart.outcome, paused.outcome);
    assert_eq!(version(&store), 2);
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.admission]
fn admission_commands_follow_the_owner_and_stale_or_absent_targets_change_nothing() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut store = CampaignStore::open(&directory.path().join("campaign.sqlite"))
        .unwrap_or_else(|_| unreachable!());
    seeded(&mut store);
    let current = version(&store);
    let admission = |kind: &str, expected_version: u64| {
        let campaign_id = CAMPAIGN.to_owned();
        match kind {
            "pause" => OperatorIntent::Pause {
                campaign_id,
                expected_version,
            },
            "continue" => OperatorIntent::Continue {
                campaign_id,
                expected_version,
            },
            _ => OperatorIntent::Cancel {
                campaign_id,
                expected_version,
            },
        }
    };
    assert_eq!(
        store.apply_operator(command("stale-pause", admission("pause", current - 1))),
        Err(OperatorFailure::Stale {
            expected: current - 1,
            current,
        })
    );
    let mut missing = admission("pause", current);
    if let OperatorIntent::Pause { campaign_id, .. } = &mut missing {
        *campaign_id = "absent".into();
    }
    assert_eq!(
        store.apply_operator(command("absent-pause", missing)),
        Err(OperatorFailure::Missing)
    );
    assert_eq!(version(&store), current);

    let paused = applied(
        &mut store,
        command("pause-request-1", admission("pause", current)),
    );
    assert_eq!(paused.outcome.admission, AdmissionState::Paused);
    assert_eq!(
        refused_code(store.apply_operator(command(
            "issue-while-paused",
            OperatorIntent::IssueDecision {
                campaign_id: CAMPAIGN.into(),
                expected_version: paused.outcome.store_version,
                identity: id('f'),
            },
        ))),
        "campaign_admission_closed"
    );
    let reopened = applied(
        &mut store,
        command(
            "continue-request-1",
            admission("continue", paused.outcome.store_version),
        ),
    );
    assert_eq!(reopened.outcome.admission, AdmissionState::Open);
    assert!(
        store
            .continuation(CAMPAIGN)
            .unwrap_or_else(|_| unreachable!())
            .0
            .demand
            .contains(&WakeKind::Operator),
        "continuing admission wakes the continuation"
    );
    let cancelled = applied(
        &mut store,
        command(
            "cancel-request-1",
            admission("cancel", reopened.outcome.store_version),
        ),
    );
    assert_eq!(cancelled.outcome.admission, AdmissionState::Cancelled);
    let still_cancelled = applied(
        &mut store,
        command(
            "continue-request-2",
            admission("continue", cancelled.outcome.store_version),
        ),
    );
    assert_eq!(
        still_cancelled.outcome.admission,
        AdmissionState::Cancelled,
        "continuing never reopens a cancelled campaign"
    );
    assert_eq!(
        refused_code(store.apply_operator(command(
            "second-start",
            OperatorIntent::Start {
                contract: contract(),
            },
        ))),
        "campaign_exists"
    );
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.decisions]
fn answers_resolve_only_their_own_outstanding_decision_in_current_scope() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut store = CampaignStore::open(&directory.path().join("campaign.sqlite"))
        .unwrap_or_else(|_| unreachable!());
    seeded(&mut store);
    let current = version(&store);
    let state = store
        .inspect(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        state.decisions.get(&id('f')).map(|request| &request.state),
        Some(&DecisionRequestState::Unissued),
        "the preference waits beyond the outstanding bound"
    );
    assert_eq!(
        refused_code(store.apply_operator(command(
            "issue-at-bound",
            OperatorIntent::IssueDecision {
                campaign_id: CAMPAIGN.into(),
                expected_version: current,
                identity: id('f'),
            },
        ))),
        "campaign_decision_queue_full"
    );
    for (request_id, intent, code) in [
        (
            "wrong-option",
            resolve(current, 'e', "merge anyway"),
            "campaign_decision_stale",
        ),
        // An unissued decision cannot be answered, even with matching bindings.
        (
            "other-decision",
            resolve(current, 'f', "approve"),
            "campaign_decision_stale",
        ),
        (
            "unknown-decision",
            resolve(current, '9', "approve"),
            "campaign_decision_unknown",
        ),
    ] {
        assert_eq!(
            refused_code(store.apply_operator(command(request_id, intent))),
            code
        );
    }
    assert_eq!(version(&store), current, "no refused answer transitioned");

    // A new candidate supersedes the reviewed one; its answer is stale now.
    store
        .observe(CAMPAIGN, current, verified('d'))
        .unwrap_or_else(|_| unreachable!());
    let superseded = version(&store);
    assert_eq!(
        refused_code(store.apply_operator(command(
            "superseded-answer",
            resolve(superseded, 'e', "approve"),
        ))),
        "campaign_decision_scope_changed"
    );
    store
        .observe(CAMPAIGN, superseded, verified('c'))
        .unwrap_or_else(|_| unreachable!());

    let answer = command("answer-review", resolve(version(&store), 'e', "approve"));
    let answered = applied(&mut store, answer.clone());
    assert_eq!(
        answered.outcome.decision,
        Some(DecisionOutcome {
            identity: id('e'),
            state: DecisionRequestState::Resolved,
        })
    );
    assert_eq!(applied(&mut store, answer).outcome, answered.outcome);
    let state = store
        .inspect(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        state
            .decisions
            .get(&id('e'))
            .and_then(|request| request.resolved_answer.as_deref()),
        Some("approve"),
        "the resolved decision stays visible"
    );
    assert_eq!(
        state.decisions.get(&id('f')).map(|request| &request.state),
        Some(&DecisionRequestState::Unissued),
        "the other decision is untouched"
    );
    assert!(
        store
            .continuation(CAMPAIGN)
            .unwrap_or_else(|_| unreachable!())
            .0
            .demand
            .contains(&WakeKind::Review)
    );
    let issued = applied(
        &mut store,
        command(
            "issue-preference",
            OperatorIntent::IssueDecision {
                campaign_id: CAMPAIGN.into(),
                expected_version: answered.outcome.store_version,
                identity: id('f'),
            },
        ),
    );
    assert_eq!(
        issued.outcome.decision.map(|decision| decision.state),
        Some(DecisionRequestState::Outstanding)
    );
}

#[test]
fn malformed_requests_are_refused_before_storage() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut store = CampaignStore::open(&directory.path().join("campaign.sqlite"))
        .unwrap_or_else(|_| unreachable!());
    seeded(&mut store);
    let current = version(&store);
    let pause = |request_id: &str| {
        command(
            request_id,
            OperatorIntent::Pause {
                campaign_id: CAMPAIGN.into(),
                expected_version: current,
            },
        )
    };
    for request_id in [
        "short",
        "has space here",
        &"x".repeat(MAX_REQUEST_ID_BYTES + 1),
    ] {
        assert_eq!(
            store.apply_operator(pause(request_id)),
            Err(OperatorFailure::Invalid("request_id"))
        );
    }
    let mut anonymous = pause("anonymous-pause");
    anonymous.actor = " ".into();
    assert_eq!(
        store.apply_operator(anonymous),
        Err(OperatorFailure::Invalid("actor"))
    );
    assert_eq!(
        store.apply_operator(command(
            "huge-answer",
            resolve(current, 'e', &"a".repeat(513))
        )),
        Err(OperatorFailure::Invalid("answer"))
    );
    assert_eq!(version(&store), current);
}
