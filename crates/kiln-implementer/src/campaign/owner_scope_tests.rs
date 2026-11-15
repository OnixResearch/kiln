use super::*;
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CompletionGoal, DecisionKind, EnumerationState, Freshness, MemberBinding,
    MemberSelector, MemberStage, WorkflowProfile,
};

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn contract() -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: "owner-scope".into(),
        revision: 1,
        repository: "repo".into(),
        members: vec![MemberBinding {
            selector: MemberSelector::WholeChange {
                change: "change".into(),
            },
            profile: WorkflowProfile::NoSpecDelta,
            profile_version: id('a'),
            repository_instructions: id('a'),
            policy: id('a'),
            required_checks: Vec::new(),
            required_reviews: Vec::new(),
            integration_policy: id('a'),
            goal: CompletionGoal::VerifiedCandidate,
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

fn observation() -> MemberObservation {
    MemberObservation {
        member: "change/*".into(),
        stage: MemberStage::Ready,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: Some(id('a')),
        integration_target_identity: Some(id('a')),
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: Default::default(),
        dependency_reason: None,
        canonical_work_key: Some(id('a')),
        work_identity: Some(id('a')),
        attempt_identity: Some(id('a')),
        attempt_fence: Some(7),
        context_identity: Some(id('a')),
        native_context_identity: Some(id('a')),
        context_freshness: Freshness::Current,
        candidate_freshness: Freshness::Current,
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

fn retain_intent(store: &mut CampaignStore) -> PendingEffect {
    let state = store.start(contract()).unwrap_or_else(|_| unreachable!());
    let member = observation().member;
    let version = store
        .version(&state.contract.campaign_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    store
        .observe(&state.contract.campaign_id, version, observation())
        .unwrap_or_else(|_| unreachable!());
    let version = store
        .version(&state.contract.campaign_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    store
        .persist_effect_intent(
            &state.contract.campaign_id,
            version,
            member,
            CampaignOperation::Code,
            "lattice-attempt-ledger".into(),
            id('b'),
        )
        .unwrap_or_else(|_| unreachable!())
}

fn decision(identity: ContentId) -> HumanDecisionRequest {
    HumanDecisionRequest {
        identity,
        kind: DecisionKind::Preference,
        member: "change/*".into(),
        work_identity: Some(id('a')),
        decision_revision: None,
        policy_identity: id('a'),
        candidate_identity: None,
        state: DecisionRequestState::Outstanding,
        created_ms: 1,
        question: "select a bounded option".into(),
        options: vec!["continue".into(), "wait".into()],
        recommendation: None,
        priority: 1,
        pending_reason: None,
        resolved_answer: None,
        active_effort_ms: None,
    }
}

#[test]
fn exact_owner_observation_applies_after_intent_survives_reopen() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let effect = retain_intent(&mut store);
    drop(store);

    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let applied = store
        .apply_owner_observation(
            "owner-scope",
            version,
            &effect.effect_id,
            EffectState::ObservedComplete,
            id('c'),
            observation(),
        )
        .unwrap_or_else(|_| unreachable!());
    let applied_effect = applied
        .pending_effects
        .get(&effect.effect_id)
        .unwrap_or_else(|| unreachable!("retained owner effect"));
    assert!(applied_effect.progress_applied);
    assert_eq!(applied_effect.evidence_identity, Some(id('c')));
    assert!(
        applied
            .observations
            .get("change/*")
            .unwrap_or_else(|| unreachable!("observed member"))
            .evidence
            .contains(&id('c'))
    );
}

#[test]
fn changed_policy_cannot_apply_a_retained_owner_effect() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let effect = retain_intent(&mut store);
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let mut changed = observation();
    changed.policy_identity = id('d');
    store
        .observe("owner-scope", version, changed)
        .unwrap_or_else(|_| unreachable!());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let stale = store
        .apply_owner_observation(
            "owner-scope",
            version,
            &effect.effect_id,
            EffectState::ObservedComplete,
            id('c'),
            observation(),
        )
        .unwrap_or_else(|_| unreachable!());
    let stale_effect = stale
        .pending_effects
        .get(&effect.effect_id)
        .unwrap_or_else(|| unreachable!("retained owner effect"));
    assert_eq!(stale_effect.state, EffectState::RejectedStale);
    assert!(stale_effect.progress_applied);
    assert!(
        !stale
            .observations
            .get("change/*")
            .unwrap_or_else(|| unreachable!("observed member"))
            .evidence
            .contains(&id('c'))
    );
    assert_eq!(stale_effect.evidence_identity, Some(id('c')));
}

#[test]
fn cancelled_campaign_retains_late_evidence_without_member_progress() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let effect = retain_intent(&mut store);
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    store
        .cancel("owner-scope", version)
        .unwrap_or_else(|_| unreachable!());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let cancelled = store
        .apply_owner_observation(
            "owner-scope",
            version,
            &effect.effect_id,
            EffectState::ObservedComplete,
            id('c'),
            observation(),
        )
        .unwrap_or_else(|_| unreachable!());
    assert!(cancelled.cancelled);
    assert!(
        !cancelled
            .observations
            .get("change/*")
            .unwrap_or_else(|| unreachable!("observed member"))
            .evidence
            .contains(&id('c'))
    );
    let cancelled_effect = cancelled
        .pending_effects
        .get(&effect.effect_id)
        .unwrap_or_else(|| unreachable!("retained owner effect"));
    assert_eq!(cancelled_effect.state, EffectState::RejectedStale);
    assert_eq!(cancelled_effect.evidence_identity, Some(id('c')));
    assert!(cancelled_effect.progress_applied);
}

#[test]
fn parent_budget_scope_bounds_outstanding_decisions_across_campaigns() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    for campaign_id in ["owner-scope", "child-campaign"] {
        let mut scope = contract();
        scope.campaign_id = campaign_id.into();
        let _state = store.start(scope).unwrap_or_else(|_| unreachable!());
        let version = store
            .version(campaign_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        store
            .observe(campaign_id, version, observation())
            .unwrap_or_else(|_| unreachable!());
        let version = store
            .version(campaign_id)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let request_id = if campaign_id == "owner-scope" {
            id('b')
        } else {
            id('c')
        };
        let inserted = store
            .add_decision(campaign_id, version, decision(request_id))
            .unwrap_or_else(|_| unreachable!());
        let retained = inserted
            .decisions
            .values()
            .next()
            .unwrap_or_else(|| unreachable!());
        if campaign_id == "owner-scope" {
            assert_eq!(retained.state, DecisionRequestState::Outstanding);
        } else {
            assert_eq!(retained.state, DecisionRequestState::Unissued);
            assert!(
                retained
                    .pending_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("shared operator decision scope"))
            );
        }
    }
}

#[test]
fn qualified_source_successor_survives_reopen_without_hiding_policy_drift() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let mut selected = contract();
    selected
        .members
        .first_mut()
        .unwrap_or_else(|| unreachable!("contract has a member"))
        .goal = CompletionGoal::IntegratedChange;
    store.start(selected).unwrap_or_else(|_| unreachable!());
    let mut current = observation();
    current.stage = MemberStage::VerifiedCandidate;
    current.verifier_freshness = Freshness::Current;
    current
        .remaining_obligations
        .insert("change/tasks/B".into());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    store
        .observe("owner-scope", version, current.clone())
        .unwrap_or_else(|_| unreachable!());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let effect = store
        .persist_effect_intent(
            "owner-scope",
            version,
            current.member.clone(),
            CampaignOperation::Integrate,
            "native-git".into(),
            id('b'),
        )
        .unwrap_or_else(|_| unreachable!());
    let mut successor = observation();
    successor.source_identity = Some(id('b'));
    successor.native_context_identity = Some(id('b'));
    successor.work_identity = Some(id('b'));
    successor.canonical_work_key = Some(id('b'));
    successor.integration_target_identity = Some(id('b'));
    successor.subject_identity = None;
    successor.attempt_identity = None;
    successor.attempt_fence = None;
    successor
        .remaining_obligations
        .insert("change/tasks/B".into());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert!(
        store
            .observe_successor("owner-scope", version, successor.clone(), &effect.effect_id)
            .is_err()
    );
    current.stage = MemberStage::Integrated;
    store
        .apply_owner_observation(
            "owner-scope",
            version,
            &effect.effect_id,
            EffectState::ObservedComplete,
            id('c'),
            current,
        )
        .unwrap_or_else(|_| unreachable!());
    drop(store);
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let advanced = store
        .observe_successor("owner-scope", version, successor.clone(), &effect.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        advanced
            .observations
            .get("change/*")
            .unwrap_or_else(|| unreachable!("observed member"))
            .stage,
        MemberStage::Ready
    );
    assert_eq!(
        advanced
            .impacts
            .last()
            .unwrap_or_else(|| unreachable!())
            .action,
        CampaignOperation::Code
    );
    drop(store);
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let retained = store
        .inspect("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let retained_member = retained
        .observations
        .get("change/*")
        .unwrap_or_else(|| unreachable!("observed member"));
    assert_eq!(retained_member.source_identity, Some(id('b')));
    assert_eq!(retained_member.native_context_identity, Some(id('b')));
    assert_eq!(retained_member.context_identity, Some(id('a')));
    successor.policy_identity = id('d');
    let version = store
        .version("owner-scope")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let stale = store
        .observe_successor("owner-scope", version, successor, &effect.effect_id)
        .unwrap_or_else(|_| unreachable!());
    let stale_member = stale
        .observations
        .get("change/*")
        .unwrap_or_else(|| unreachable!("observed member"));
    assert_eq!(stale_member.stage, MemberStage::Blocked);
    assert_eq!(stale_member.policy_identity, id('d'));
    assert_eq!(
        stale
            .impacts
            .last()
            .unwrap_or_else(|| unreachable!())
            .action,
        CampaignOperation::Replan
    );
}
