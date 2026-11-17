//! Bounded continuation over the real SQLite store and scripted owners.
use super::*;
use crate::campaign_operator::{CampaignCommand, apply};
use kiln_core::implementation::continuation::{
    DEFAULT_RESCAN_INTERVAL_MS, DEFAULT_RETRY_BASE_MS, StopReason, WaitReason,
};

const START_MS: u64 = 1_000_000;
const OPERATIONS: u32 = 8;
const DAY_MS: u64 = 86_400_000;

/// Owner timing around [`ScriptedOwners`]: refresh failures, a concurrent
/// writer, a hint recorded mid-batch and synchronous owner results.
#[derive(Debug, Default)]
struct Owners {
    scripted: ScriptedOwners,
    failing_refreshes: u32,
    hint_during_batch: Option<WakeKind>,
    concurrent_write: bool,
    synchronous_results: bool,
    refreshes: u32,
}

impl CampaignEffectConsumer for Owners {
    fn dispatch(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        let observed = self.scripted.dispatch(effect)?;
        let stage = match effect.operation {
            CampaignOperation::Code if self.synchronous_results => MemberStage::Candidate,
            CampaignOperation::Verify if self.synchronous_results => MemberStage::VerifiedCandidate,
            _ => return Ok(observed),
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
    fn reconcile(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        self.scripted.reconcile(effect)
    }
}

impl CampaignDriveShell for Owners {
    fn refresh(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
    ) -> Result<u64, String> {
        self.refreshes += 1;
        if let Some(kind) = self.hint_during_batch.take() {
            store.record_wake(
                campaign_id,
                WakeHint {
                    kind,
                    observed_version: None,
                },
            )?;
        }
        if self.failing_refreshes > 0 {
            self.failing_refreshes -= 1;
            return Err("owner refresh unavailable".into());
        }
        if std::mem::take(&mut self.concurrent_write) {
            // Another writer moves the campaign after this batch read it.
            let state = store.inspect(campaign_id)?.ok_or("campaign not found")?;
            let member = state.observations.get(B).cloned().ok_or("B unobserved")?;
            store.observe(campaign_id, version, member)?;
            return Ok(version);
        }
        self.scripted.refresh(store, campaign_id, version)
    }
    fn recover_executing(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member: &str,
    ) -> Result<Option<u64>, String> {
        self.scripted
            .recover_executing(store, campaign_id, version, state, member)
    }
    fn claim(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member: &str,
    ) -> Result<CampaignClaim, String> {
        self.scripted
            .claim(store, campaign_id, version, state, member)
    }
}

fn step_with(
    store: &mut CampaignStore,
    now_ms: u64,
    operations: u32,
    policy: &ContinuationPolicy,
    owners: &mut Owners,
) -> ContinuationRun {
    continue_bounded(
        store,
        CAMPAIGN,
        policy,
        operations,
        &mut || Ok(now_ms),
        owners,
    )
    .unwrap_or_else(|error| unreachable!("{error}"))
}

fn step(store: &mut CampaignStore, now_ms: u64, owners: &mut Owners) -> ContinuationRun {
    step_with(
        store,
        now_ms,
        OPERATIONS,
        &ContinuationPolicy::DEFAULT,
        owners,
    )
}

fn reasons(run: &ContinuationRun) -> Vec<DriveReason> {
    run.batches.iter().map(|batch| batch.reason).collect()
}

fn outcomes(run: &ContinuationRun) -> Vec<BatchOutcome> {
    run.batches.iter().map(|batch| batch.outcome).collect()
}

fn awaiting(until_ms: u64) -> ContinuationStep {
    ContinuationStep::Wait {
        reason: WaitReason::AwaitingObservation,
        until_ms: Some(until_ms),
    }
}

fn wake(store: &mut CampaignStore, kind: WakeKind, observed_version: Option<u64>) {
    store
        .record_wake(
            CAMPAIGN,
            WakeHint {
                kind,
                observed_version,
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
}

fn operate(database: &Path, command: CampaignCommand) {
    apply(command, database).unwrap_or_else(|error| unreachable!("{error}"));
}

fn record(store: &CampaignStore) -> ContinuationRecord {
    store
        .continuation(CAMPAIGN)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .0
}

fn state(store: &CampaignStore) -> CampaignState {
    store
        .inspect(CAMPAIGN)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("campaign state"))
}

fn open(directory: &tempfile::TempDir) -> (std::path::PathBuf, CampaignStore) {
    let database = directory.path().join("campaign.sqlite");
    let store = CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
    (database, store)
}

/// Only A, whose verified candidate awaits a required review.
fn seed_review_only(store: &mut CampaignStore) {
    let mut only_a = contract();
    only_a.members.truncate(1);
    store.start(only_a).unwrap_or_else(|_| unreachable!());
    let version = current(store);
    store
        .observe(
            CAMPAIGN,
            version,
            observation(A, MemberStage::VerifiedCandidate, Vec::new()),
        )
        .unwrap_or_else(|_| unreachable!());
    let version = current(store);
    store
        .add_decision(CAMPAIGN, version, review())
        .unwrap_or_else(|_| unreachable!());
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.resume]
fn resolved_review_resumes_the_campaign_without_a_new_start() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (database, mut store) = open(&directory);
    seed_review_only(&mut store);
    let selection = state(&store).selection_identity;
    let mut owners = Owners::default();

    let waiting = step(&mut store, START_MS, &mut owners);
    assert_eq!(reasons(&waiting), [DriveReason::FirstStep]);
    assert_eq!(outcomes(&waiting), [BatchOutcome::Waiting]);
    assert!(
        waiting
            .batches
            .iter()
            .flat_map(|batch| &batch.steps)
            .all(|step| step.effect.is_none())
    );
    assert_eq!(
        waiting.next,
        awaiting(START_MS + DEFAULT_RESCAN_INTERVAL_MS)
    );
    assert!(owners.scripted.dispatched.is_empty());

    operate(
        &database,
        CampaignCommand::ResolveDecision {
            campaign_id: CAMPAIGN.into(),
            expected_version: current(&store),
            identity: id('e'),
            work_identity: Some(id('a')),
            policy_identity: id('a'),
            candidate_identity: Some(id('c')),
            answer: "approve".into(),
        },
    );
    assert!(
        record(&store).pending_demand(),
        "resolution records a review hint"
    );
    let mut approved = observation(A, MemberStage::VerifiedCandidate, Vec::new());
    approved.required_reviews_satisfied = true;
    let version = current(&store);
    store
        .observe(CAMPAIGN, version, approved)
        .unwrap_or_else(|_| unreachable!());

    let resumed = step(&mut store, START_MS + 1, &mut owners);
    assert_eq!(reasons(&resumed), [DriveReason::WakeDemand]);
    assert_eq!(outcomes(&resumed), [BatchOutcome::AwaitingOwner]);
    assert_eq!(
        owners.scripted.dispatched,
        [(A.to_string(), CampaignOperation::Publish)]
    );
    let resumed_state = state(&store);
    assert_eq!(resumed_state.selection_identity, selection);
    assert_eq!(resumed_state.contract.revision, 1);
    assert_eq!(
        resumed.next,
        awaiting(START_MS + 1 + DEFAULT_RESCAN_INTERVAL_MS)
    );
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.lifecycle]
fn lifecycle_only_members_continue_after_an_integration_hint() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    let mut lifecycle_only = contract();
    lifecycle_only.members = vec![binding(
        "change-a",
        CompletionGoal::NativeLifecycleComplete,
        Vec::new(),
    )];
    store
        .start(lifecycle_only)
        .unwrap_or_else(|_| unreachable!());
    // Integrated, but no current verifier evidence for the integrated result yet.
    let integrated = observation(A, MemberStage::Integrated, Vec::new());
    let version = current(&store);
    store
        .observe(CAMPAIGN, version, integrated.clone())
        .unwrap_or_else(|_| unreachable!());
    let mut owners = Owners::default();
    let waiting = step(&mut store, START_MS, &mut owners);
    assert_eq!(outcomes(&waiting), [BatchOutcome::Waiting]);
    assert!(owners.scripted.dispatched.is_empty());

    let mut verified = integrated;
    verified.verifier_freshness = Freshness::Current;
    let version = current(&store);
    store
        .observe(CAMPAIGN, version, verified)
        .unwrap_or_else(|_| unreachable!());
    let observed = current(&store);
    wake(&mut store, WakeKind::Integration, Some(observed));
    let resumed = step(&mut store, START_MS + 1, &mut owners);
    assert_eq!(reasons(&resumed), [DriveReason::WakeDemand]);
    assert_eq!(outcomes(&resumed), [BatchOutcome::AwaitingOwner]);
    assert_eq!(
        owners.scripted.dispatched,
        [(A.to_string(), CampaignOperation::Lifecycle)]
    );
    assert!(owners.scripted.claims.is_empty());
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.coalesce]
fn duplicate_hints_coalesce_and_a_mid_batch_hint_stays_pending() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    seed(&mut store);
    for (kind, observed) in [
        (WakeKind::Verification, Some(9)),
        (WakeKind::Verification, Some(8)),
        (WakeKind::Source, None),
        (WakeKind::Capacity, None),
    ] {
        wake(&mut store, kind, observed);
    }
    let mut owners = Owners {
        hint_during_batch: Some(WakeKind::Reconciliation),
        ..Owners::default()
    };
    let single = ContinuationPolicy {
        max_batches: 1,
        ..ContinuationPolicy::DEFAULT
    };
    let first = step_with(&mut store, START_MS, OPERATIONS, &single, &mut owners);
    assert_eq!(reasons(&first), [DriveReason::WakeDemand]);
    assert_eq!(outcomes(&first), [BatchOutcome::AwaitingOwner]);
    assert_eq!(
        first.next,
        ContinuationStep::Drive {
            reason: DriveReason::WakeDemand
        },
        "the hint recorded during the batch remains pending"
    );
    assert_eq!(
        (first.record.consumed_sequence, first.record.demand_sequence),
        (4, 5)
    );

    let read_back = step(&mut store, START_MS, &mut owners);
    assert_eq!(reasons(&read_back), [DriveReason::WakeDemand]);
    assert!(!read_back.record.pending_demand());
    assert!(read_back.record.demand.is_empty());
    let idle = step(&mut store, START_MS, &mut owners);
    assert!(idle.batches.is_empty());

    wake(&mut store, WakeKind::Verification, None);
    wake(&mut store, WakeKind::Verification, None);
    let duplicate = step(&mut store, START_MS, &mut owners);
    assert_eq!(reasons(&duplicate), [DriveReason::WakeDemand]);
    assert_eq!(owners.scripted.claims, [C]);
    assert_eq!(
        owners.scripted.dispatched,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert_eq!(
        owners.scripted.reconciled,
        [
            (C.to_string(), CampaignOperation::Code),
            (C.to_string(), CampaignOperation::Code)
        ],
        "hints only reread the retained effect; nothing is claimed or dispatched again"
    );
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.rescan]
fn stale_versions_reread_and_lost_notifications_resume_on_the_rescan() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    seed(&mut store);
    let mut owners = Owners {
        concurrent_write: true,
        ..Owners::default()
    };
    let raced = step(&mut store, START_MS, &mut owners);
    assert_eq!(
        reasons(&raced),
        [DriveReason::FirstStep, DriveReason::VersionChanged]
    );
    assert_eq!(
        outcomes(&raced),
        [BatchOutcome::StaleVersion, BatchOutcome::AwaitingOwner]
    );
    let [stale, reread] = raced.batches.as_slice() else {
        unreachable!("a stale batch and its reread")
    };
    assert!(
        stale
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with(STALE_CAMPAIGN_VERSION))
    );
    assert_eq!(reread.store_version, current(&store));
    assert_eq!(raced.record.consecutive_failures, 0);
    assert_eq!(
        owners.scripted.dispatched,
        [(C.to_string(), CampaignOperation::Code)],
        "the stale batch stopped before intent or dispatch"
    );

    // The owner finished C's code but its notification was lost.
    owners.scripted.reported_stage = Some(MemberStage::Candidate);
    let rescan = START_MS + DEFAULT_RESCAN_INTERVAL_MS;
    let quiet = step(&mut store, rescan - 1, &mut owners);
    assert!(quiet.batches.is_empty());
    assert_eq!(quiet.next, awaiting(rescan));
    assert!(owners.scripted.reconciled.is_empty());
    let rescanned = step(&mut store, rescan, &mut owners);
    assert_eq!(reasons(&rescanned), [DriveReason::Rescan]);
    assert_eq!(
        owners.scripted.reconciled,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert_eq!(
        owners.scripted.dispatched,
        [
            (C.to_string(), CampaignOperation::Code),
            (C.to_string(), CampaignOperation::Verify)
        ]
    );
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.rescan]
fn only_a_complete_canonical_read_completes_the_continuation() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    let mut owners = Owners::default();
    assert!(
        continue_bounded(
            &mut store,
            CAMPAIGN,
            &ContinuationPolicy::DEFAULT,
            OPERATIONS,
            &mut || Ok(START_MS),
            &mut owners,
        )
        .is_err(),
        "an absent campaign is an error, never completion"
    );
    let mut only_c = contract();
    only_c.members = vec![binding(
        "change-c",
        CompletionGoal::VerifiedCandidate,
        Vec::new(),
    )];
    store.start(only_c).unwrap_or_else(|_| unreachable!());
    let mut incomplete = observation(C, MemberStage::VerifiedCandidate, Vec::new());
    incomplete.enumeration = EnumerationState::IncompleteLimit;
    let version = current(&store);
    store
        .observe(CAMPAIGN, version, incomplete)
        .unwrap_or_else(|_| unreachable!());
    let partial = step(&mut store, START_MS, &mut owners);
    assert!(!outcomes(&partial).contains(&BatchOutcome::Complete));
    assert_eq!(
        partial.next,
        awaiting(START_MS + DEFAULT_RESCAN_INTERVAL_MS)
    );

    let version = current(&store);
    store
        .observe(
            CAMPAIGN,
            version,
            observation(C, MemberStage::VerifiedCandidate, Vec::new()),
        )
        .unwrap_or_else(|_| unreachable!());
    let complete = step(&mut store, START_MS + 1, &mut owners);
    assert!(
        complete.batches.is_empty(),
        "a complete canonical read stops without driving"
    );
    assert_eq!(
        complete.next,
        ContinuationStep::Stop {
            reason: StopReason::Complete
        }
    );
    assert!(owners.scripted.dispatched.is_empty());
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.idle]
fn waiting_campaigns_idle_and_operation_limit_batches_stay_bounded() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    seed_review_only(&mut store);
    let mut owners = Owners::default();
    let first = step(&mut store, START_MS, &mut owners);
    assert_eq!(outcomes(&first), [BatchOutcome::Waiting]);
    for now_ms in [
        START_MS,
        START_MS + 1,
        START_MS + DEFAULT_RESCAN_INTERVAL_MS - 1,
    ] {
        assert!(step(&mut store, now_ms, &mut owners).batches.is_empty());
    }
    assert_eq!(owners.refreshes, 1, "a waiting campaign is not reread");

    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    seed(&mut store);
    let mut owners = Owners {
        synchronous_results: true,
        ..Owners::default()
    };
    let single = ContinuationPolicy {
        max_batches: 1,
        ..ContinuationPolicy::DEFAULT
    };
    let limited = step_with(&mut store, START_MS, 1, &single, &mut owners);
    assert_eq!(outcomes(&limited), [BatchOutcome::OperationLimit]);
    assert_eq!(
        limited.next,
        ContinuationStep::Drive {
            reason: DriveReason::OperationLimit
        },
        "the batch bound ends the step with eligible work left"
    );
    let finished = step_with(&mut store, START_MS, 1, &single, &mut owners);
    assert_eq!(reasons(&finished), [DriveReason::OperationLimit]);
    assert_eq!(outcomes(&finished), [BatchOutcome::Waiting]);
    assert_eq!(
        finished.next,
        awaiting(START_MS + DEFAULT_RESCAN_INTERVAL_MS)
    );
    assert_eq!(
        owners.scripted.dispatched,
        [
            (C.to_string(), CampaignOperation::Code),
            (C.to_string(), CampaignOperation::Verify)
        ]
    );
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.pause]
fn pause_and_cancellation_close_admission_but_retained_effects_reconcile() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (database, mut store) = open(&directory);
    seed(&mut store);
    let version = current(&store);
    store
        .pause(CAMPAIGN, version)
        .unwrap_or_else(|_| unreachable!());
    let mut owners = Owners::default();
    let paused = ContinuationStep::Wait {
        reason: WaitReason::Paused,
        until_ms: None,
    };
    for now_ms in [START_MS, START_MS + DAY_MS] {
        let run = step(&mut store, now_ms, &mut owners);
        assert!(run.batches.is_empty());
        assert_eq!(run.next, paused);
    }
    assert_eq!(owners.refreshes, 0);

    // Observations made while paused stay visible but cannot widen the
    // frozen selection.
    let selection = state(&store).selection_identity;
    let mut observed = observation(C, MemberStage::Ready, Vec::new());
    observed.evidence = vec![id('d')];
    let version = current(&store);
    store
        .observe(CAMPAIGN, version, observed)
        .unwrap_or_else(|_| unreachable!());
    let version = current(&store);
    assert!(
        store
            .observe(
                CAMPAIGN,
                version,
                observation("change-z/*", MemberStage::Ready, Vec::new()),
            )
            .is_err()
    );
    assert_eq!(
        step(&mut store, START_MS + DAY_MS, &mut owners).next,
        paused
    );

    operate(
        &database,
        CampaignCommand::Continue {
            campaign_id: CAMPAIGN.into(),
            expected_version: current(&store),
        },
    );
    let admitted = step(&mut store, START_MS + DAY_MS, &mut owners);
    assert_eq!(reasons(&admitted), [DriveReason::WakeDemand]);
    assert_eq!(outcomes(&admitted), [BatchOutcome::AwaitingOwner]);
    let resumed = state(&store);
    assert_eq!(resumed.selection_identity, selection);
    assert_eq!(resumed.contract.members.len(), 3);
    assert!(
        resumed
            .observations
            .get(C)
            .is_some_and(|member| member.evidence.contains(&id('d')))
    );
    assert!(!resumed.observations.contains_key("change-z/*"));

    let version = current(&store);
    store
        .cancel(CAMPAIGN, version)
        .unwrap_or_else(|_| unreachable!());
    owners.scripted.reported_stage = Some(MemberStage::Candidate);
    let cancelled = step(&mut store, START_MS + DAY_MS, &mut owners);
    assert_eq!(reasons(&cancelled), [DriveReason::VersionChanged]);
    assert_eq!(outcomes(&cancelled), [BatchOutcome::Cancelled]);
    assert_eq!(
        cancelled.next,
        ContinuationStep::Stop {
            reason: StopReason::Cancelled
        }
    );
    assert_eq!(
        owners.scripted.reconciled,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert_eq!(owners.scripted.claims, [C]);
    assert_eq!(
        owners.scripted.dispatched,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert!(
        state(&store)
            .pending_effects
            .values()
            .all(|effect| effect.progress_applied)
    );
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.backoff]
fn failures_back_off_under_the_budget_and_refusals_wait_for_capacity() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (database, mut store) = open(&directory);
    seed(&mut store);
    let policy = ContinuationPolicy {
        max_consecutive_failures: 3,
        ..ContinuationPolicy::DEFAULT
    };
    let mut owners = Owners {
        failing_refreshes: 3,
        ..Owners::default()
    };
    let first_retry = START_MS + DEFAULT_RETRY_BASE_MS;
    let second_retry = first_retry + 2 * DEFAULT_RETRY_BASE_MS;
    for (now_ms, reason, next) in [
        (
            START_MS,
            DriveReason::FirstStep,
            ContinuationStep::Wait {
                reason: WaitReason::Backoff,
                until_ms: Some(first_retry),
            },
        ),
        (
            first_retry,
            DriveReason::Retry,
            ContinuationStep::Wait {
                reason: WaitReason::Backoff,
                until_ms: Some(second_retry),
            },
        ),
        (
            second_retry,
            DriveReason::Retry,
            ContinuationStep::Stop {
                reason: StopReason::FailureBudgetExhausted,
            },
        ),
    ] {
        let run = step_with(&mut store, now_ms, OPERATIONS, &policy, &mut owners);
        assert_eq!(reasons(&run), [reason]);
        assert_eq!(outcomes(&run), [BatchOutcome::Failed]);
        assert_eq!(run.next, next);
    }
    let exhausted = step_with(
        &mut store,
        START_MS + DAY_MS,
        OPERATIONS,
        &policy,
        &mut owners,
    );
    assert!(exhausted.batches.is_empty());
    assert!(owners.scripted.claims.is_empty());

    operate(
        &database,
        CampaignCommand::Continue {
            campaign_id: CAMPAIGN.into(),
            expected_version: current(&store),
        },
    );
    let resumed = step_with(
        &mut store,
        START_MS + DAY_MS,
        OPERATIONS,
        &policy,
        &mut owners,
    );
    assert_eq!(reasons(&resumed), [DriveReason::WakeDemand]);
    assert_eq!(outcomes(&resumed), [BatchOutcome::AwaitingOwner]);
    assert_eq!(resumed.record.consecutive_failures, 0);
    assert_eq!(owners.scripted.claims, [C]);

    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (_, mut store) = open(&directory);
    seed(&mut store);
    let mut owners = Owners::default();
    owners.scripted.refuse_claims = true;
    let refused = step_with(&mut store, START_MS, OPERATIONS, &policy, &mut owners);
    assert_eq!(outcomes(&refused), [BatchOutcome::AdmissionRefused]);
    assert!(refused.batches.iter().all(|batch| batch.refusal.is_some()));
    assert_eq!(
        refused.next,
        awaiting(START_MS + DEFAULT_RESCAN_INTERVAL_MS)
    );
    assert_eq!(
        (
            refused.record.consecutive_failures,
            refused.record.next_eligible_ms
        ),
        (0, 0),
        "a refused claim waits for capacity without burning the budget"
    );
    owners.scripted.refuse_claims = false;
    wake(&mut store, WakeKind::Capacity, None);
    let capacity = step_with(&mut store, START_MS + 1, OPERATIONS, &policy, &mut owners);
    assert_eq!(reasons(&capacity), [DriveReason::WakeDemand]);
    assert_eq!(owners.scripted.claims, [C]);
}

#[test]
// r[verify kiln.implementation.campaign.continuation.scenario.restart]
fn restart_preserves_demand_failures_backoff_selection_and_pending_effects() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let (database, mut store) = open(&directory);
    seed(&mut store);
    let mut owners = Owners::default();
    let dispatched = step(&mut store, START_MS, &mut owners);
    assert_eq!(outcomes(&dispatched), [BatchOutcome::AwaitingOwner]);
    wake(&mut store, WakeKind::Verification, None);
    owners.failing_refreshes = 1;
    let failed = step(&mut store, START_MS + 1, &mut owners);
    assert_eq!(outcomes(&failed), [BatchOutcome::Failed]);
    let retry = START_MS + 1 + DEFAULT_RETRY_BASE_MS;
    let before = store
        .continuation(CAMPAIGN)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(before.0.pending_demand());
    assert_eq!(before.0.consecutive_failures, 1);
    assert_eq!(before.0.next_eligible_ms, retry);
    let campaign = state(&store);
    let version = current(&store);
    drop(store);

    let mut store = CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        store
            .continuation(CAMPAIGN)
            .unwrap_or_else(|error| unreachable!("{error}")),
        before
    );
    assert_eq!(state(&store), campaign);
    assert_eq!(current(&store), version);
    assert_eq!(
        store
            .recover_pending(CAMPAIGN)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .len(),
        1
    );

    let mut restarted = Owners::default();
    restarted.scripted.reported_stage = Some(MemberStage::Candidate);
    let backing_off = step(&mut store, START_MS + 2, &mut restarted);
    assert!(backing_off.batches.is_empty());
    assert_eq!(
        backing_off.next,
        ContinuationStep::Wait {
            reason: WaitReason::Backoff,
            until_ms: Some(retry),
        }
    );
    let resumed = step(&mut store, retry, &mut restarted);
    assert_eq!(reasons(&resumed), [DriveReason::WakeDemand]);
    assert!(restarted.scripted.claims.is_empty());
    assert_eq!(
        restarted.scripted.reconciled,
        [(C.to_string(), CampaignOperation::Code)]
    );
    assert_eq!(
        restarted.scripted.dispatched,
        [(C.to_string(), CampaignOperation::Verify)],
        "the retained code effect reconciles; it is never dispatched again"
    );
}
