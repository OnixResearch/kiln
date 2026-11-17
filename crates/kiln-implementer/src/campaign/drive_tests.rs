use super::*;
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CompletionGoal, DecisionKind, EnumerationState, Freshness, MemberBinding,
    MemberSelector, WorkflowProfile,
};

const CAMPAIGN: &str = "independent-progress";
/// Awaits a required review before its verified-PR endpoint.
const A: &str = "change-a/*";
/// Depends on A.
const B: &str = "change-b/*";
/// Independent and ready.
const C: &str = "change-c/*";

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn binding(change: &str, goal: CompletionGoal, reviews: Vec<String>) -> MemberBinding {
    MemberBinding {
        selector: MemberSelector::WholeChange {
            change: change.into(),
        },
        profile: WorkflowProfile::SpecDriven,
        profile_version: id('a'),
        repository_instructions: id('a'),
        policy: id('a'),
        required_checks: Vec::new(),
        required_reviews: reviews,
        integration_policy: id('a'),
        goal,
        prerequisite_inclusion: false,
        context: Vec::new(),
    }
}

fn contract() -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: CAMPAIGN.into(),
        revision: 1,
        repository: "repo".into(),
        members: vec![
            binding(
                "change-a",
                CompletionGoal::VerifiedPullRequest,
                vec!["maintainer".into()],
            ),
            binding("change-b", CompletionGoal::VerifiedCandidate, Vec::new()),
            binding("change-c", CompletionGoal::VerifiedCandidate, Vec::new()),
        ],
        max_concurrency: 1,
        max_outstanding_decisions: 1,
        parent_budget_id: id('a'),
        agent_definition: id('a'),
        require_one_writer_per_repository: true,
    }
}

fn observation(member: &str, stage: MemberStage, dependencies: Vec<String>) -> MemberObservation {
    let verified = stage == MemberStage::VerifiedCandidate;
    MemberObservation {
        member: member.into(),
        stage,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: Some(id('a')),
        integration_target_identity: None,
        profile_identity: id('a'),
        dependencies,
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
        verifier_freshness: if verified {
            Freshness::Current
        } else {
            Freshness::Unknown
        },
        target_freshness: Freshness::Unknown,
        evidence: if verified { vec![id('c')] } else { Vec::new() },
        blockers: Vec::new(),
        required_checks_satisfied: true,
        required_reviews_satisfied: !verified,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

fn review() -> HumanDecisionRequest {
    HumanDecisionRequest {
        identity: id('e'),
        kind: DecisionKind::RequiredReview,
        member: A.into(),
        work_identity: Some(id('a')),
        decision_revision: None,
        policy_identity: id('a'),
        candidate_identity: Some(id('c')),
        state: DecisionRequestState::Outstanding,
        created_ms: 1,
        question: "review the verified candidate".into(),
        options: vec!["approve".into(), "reject".into()],
        recommendation: None,
        priority: 1,
        pending_reason: None,
        resolved_answer: None,
        active_effort_ms: None,
    }
}

fn current(store: &CampaignStore) -> u64 {
    store
        .version(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("campaign version"))
}

/// A's verified candidate awaits an outstanding required review, B depends on
/// A and C is independently ready.
fn seed(store: &mut CampaignStore) -> u64 {
    store.start(contract()).unwrap_or_else(|_| unreachable!());
    for member in [
        observation(A, MemberStage::VerifiedCandidate, Vec::new()),
        observation(B, MemberStage::Ready, vec![A.into()]),
        observation(C, MemberStage::Ready, Vec::new()),
    ] {
        let version = current(store);
        store
            .observe(CAMPAIGN, version, member)
            .unwrap_or_else(|_| unreachable!());
    }
    let version = current(store);
    store
        .add_decision(CAMPAIGN, version, review())
        .unwrap_or_else(|_| unreachable!());
    current(store)
}

fn hold(state: &CampaignState, member: &str) -> CampaignDecision {
    state
        .evaluate()
        .holds
        .into_iter()
        .find(|hold| hold.member.as_deref() == Some(member))
        .unwrap_or_else(|| unreachable!("hold for {member}"))
}

/// Scripted canonical owners around the real bounded loop and SQLite store.
#[derive(Debug, Default)]
struct ScriptedOwners {
    claims: Vec<String>,
    recoveries: Vec<String>,
    dispatched: Vec<(String, CampaignOperation)>,
    reconciled: Vec<(String, CampaignOperation)>,
    refuse_claims: bool,
    /// Stage the attempt owner reports on read-back; `None` leaves it unknown.
    reported_stage: Option<MemberStage>,
}

impl CampaignEffectConsumer for ScriptedOwners {
    fn dispatch(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        self.dispatched
            .push((effect.member.clone(), effect.operation));
        Ok(None)
    }
    fn reconcile(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        self.reconciled
            .push((effect.member.clone(), effect.operation));
        let Some(stage) = self.reported_stage else {
            return Ok(None);
        };
        let mut member = observation(&effect.member, stage, Vec::new());
        member.attempt_identity = effect.attempt_identity.clone();
        member.attempt_fence = effect.attempt_fence;
        Ok(Some(CampaignOwnerObservation {
            result: EffectState::ObservedComplete,
            evidence_identity: id('f'),
            member,
        }))
    }
}

impl CampaignDriveShell for ScriptedOwners {
    fn refresh(
        &mut self,
        _store: &mut CampaignStore,
        _campaign_id: &str,
        version: u64,
    ) -> Result<u64, String> {
        Ok(version)
    }
    fn recover_executing(
        &mut self,
        _store: &mut CampaignStore,
        _campaign_id: &str,
        _version: u64,
        _state: &CampaignState,
        member: &str,
    ) -> Result<Option<u64>, String> {
        self.recoveries.push(member.into());
        Ok(None)
    }
    fn claim(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member: &str,
    ) -> Result<CampaignClaim, String> {
        if self.refuse_claims {
            return Ok(CampaignClaim::Refused(
                "canonical admission refused the claim: parent budget exhausted".into(),
            ));
        }
        self.claims.push(member.into());
        let mut bound = state
            .observations
            .get(member)
            .cloned()
            .ok_or("claimed member is not observed")?;
        bound.attempt_identity = Some(id('9'));
        bound.attempt_fence = Some(1);
        store.observe(campaign_id, version, bound)?;
        store
            .version(campaign_id)?
            .map(CampaignClaim::Bound)
            .ok_or_else(|| "campaign disappeared after claim".into())
    }
}

#[test]
// r[verify kiln.implementation.campaign.independent_progress.scenario.driver]
fn bounded_driver_advances_independent_work_and_resumes_without_reclaim_or_redispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("campaign.sqlite");
    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let version = seed(&mut store);
    let review_wait = hold(
        &store
            .inspect(CAMPAIGN)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!()),
        A,
    );
    assert_eq!(review_wait.effect_intent, Some(id('e')));

    let mut owners = ScriptedOwners::default();
    let run = drive_bounded(&mut store, CAMPAIGN, version, 8, &mut owners)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(owners.claims, [C]);
    assert_eq!(
        owners.dispatched,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert!(owners.reconciled.is_empty());
    let [step] = run.steps.as_slice() else {
        unreachable!("one dispatched step")
    };
    let effect = step
        .effect
        .as_ref()
        .unwrap_or_else(|| unreachable!("dispatched effect"));
    assert_eq!(effect.member, C);
    assert_eq!(effect.attempt_identity, Some(id('9')));
    assert_eq!(step.decision.operation, CampaignOperation::Reconcile);
    assert_eq!(hold(&run.state, A), review_wait);
    assert!(hold(&run.state, B).reason.contains("dependency"));
    drop(store);

    let mut store = CampaignStore::open(&database).unwrap_or_else(|_| unreachable!());
    let version = current(&store);
    let mut owners = ScriptedOwners {
        reported_stage: Some(MemberStage::Executing),
        ..ScriptedOwners::default()
    };
    let run = drive_bounded(&mut store, CAMPAIGN, version, 8, &mut owners)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(owners.claims.is_empty());
    assert!(owners.dispatched.is_empty());
    assert_eq!(
        owners.reconciled,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert_eq!(owners.recoveries, [C]);
    assert_eq!(run.steps.len(), 2);
    assert!(
        run.state
            .pending_effects
            .values()
            .all(|effect| effect.progress_applied)
    );
    let evaluation = run.state.evaluate();
    assert_eq!(evaluation.basis, SelectionBasis::MemberHold);
    assert_eq!(evaluation.decision, review_wait);
}

#[test]
// r[verify kiln.implementation.campaign.independent_progress.scenario.barriers]
fn refused_canonical_claim_stops_before_intent_or_dispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut store = CampaignStore::open(&directory.path().join("campaign.sqlite"))
        .unwrap_or_else(|_| unreachable!());
    let version = seed(&mut store);
    let mut owners = ScriptedOwners {
        refuse_claims: true,
        ..ScriptedOwners::default()
    };
    let run = drive_bounded(&mut store, CAMPAIGN, version, 8, &mut owners)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let refused = run
        .refusal
        .as_deref()
        .unwrap_or_else(|| unreachable!("refused claim was driven"));
    assert!(refused.contains("parent budget exhausted"), "{refused}");
    assert!(run.steps.is_empty());
    assert_eq!(run.version, version);
    assert_eq!(classify_batch(&Ok(run), 8), BatchOutcome::AdmissionRefused);
    assert!(owners.dispatched.is_empty());
    let state = store
        .inspect(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert!(state.pending_effects.is_empty());
    assert_eq!(current(&store), version);
    let selected = state.next_action().unwrap_or_else(|_| unreachable!());
    assert_eq!(selected.operation, CampaignOperation::Code);
    assert_eq!(selected.member.as_deref(), Some(C));
}

#[path = "continuation_tests.rs"]
mod continuation_tests;
