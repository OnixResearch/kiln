// r[verify kiln.admission.host_owner]
// r[verify kiln.admission.host_owner.scenario.reserve]
// r[verify kiln.admission.host_owner.scenario.refused]
// r[verify kiln.admission.host_owner.scenario.replay]
//! Durable host-local owner admission through the real implementation store.
#[path = "authority_common/mod.rs"]
mod authority_common;

use kiln_core::fair_admission::{
    AdmissionPhase, EligibilityInput, HostOwnerProfile, ReservationState,
};
use kiln_core::implementation::{
    AcceptanceObligation, AttemptUsage, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace,
    Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt, ImplementationDispatchPlan,
    ImplementationWork, SourceRef, SupersessionRule, TerminalPolicy, WorkBudget, WorkSourceKind,
    WorkerObservation, WorkerTerminalState, WorkspaceIsolation,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::{
    Implementer,
    admission::{AdmissionCommand as Cmd, AdmissionResult},
    host_owner::HostOwnerReserved,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const EXPORT: &str = include_str!("../../../fixtures/fair-admission/host-owner.json");
const SANDBOX: &str = "onixresearch/implementer-sandbox";
const OTHER: &str = "onixresearch/other";
/// A wall-clock-shaped instant inside the exported campaign deadline.
const NOW: u64 = 1_800_000_000_000;
const LEASE_MS: u64 = 3_600_000;

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}
fn refused<T: std::fmt::Debug>(result: Result<T, String>, code: &str) {
    match result {
        Ok(value) => unreachable!("expected {code}, got {value:?}"),
        Err(error) => assert!(error.contains(code), "expected {code}, got {error}"),
    }
}
fn profile() -> HostOwnerProfile {
    let profile: HostOwnerProfile = ok(serde_json::from_str(EXPORT));
    ok(profile.validate());
    profile
}
fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from([SANDBOX.into(), OTHER.into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}
fn open(path: &Path) -> Implementer {
    ok(Implementer::open_with_local_operator(
        path,
        enrollment(),
        "pilot-operator",
    ))
}
fn work(profile: &HostOwnerProfile, repository: &str, issue: u64) -> ImplementationWork {
    let native = format!("{repository}#{issue}");
    let digest = blake3::hash(native.as_bytes()).to_hex().to_string();
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: repository.into(),
        project: "implementer-sandbox".into(),
        base: Some(ok(ExternalRef::new(
            "git",
            "commit",
            "f1adb86f6320f2bbb6c02ae077905a6bb8c22955",
        ))),
        title: format!("issue {issue}"),
        requested_outcome: "fix the issue".into(),
        acceptance: vec![AcceptanceObligation {
            id: "acceptance".into(),
            statement: "the pinned acceptance test passes".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["src".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: ok(profile
            .repositories
            .first()
            .and_then(|entry| entry.classes.first())
            .ok_or(()))
        .verification_profile
        .clone(),
        budget: WorkBudget {
            attempts: 1,
            minutes_per_attempt: 20,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ok(ExternalRef::new("github", "issue", &native)),
            native_id: native,
            version: ok(ContentId::from_blake3_hex(digest)),
        }],
    }
}
fn intake(i: &mut Implementer, works: &[&ImplementationWork]) {
    for work in works {
        ok(i.admit_with_supersession(work, None, SupersessionRule::AbortActive, NOW));
    }
}
/// Claim through fair admission, as the admission-governed operator claim does.
fn claim(i: &mut Implementer, now: u64) -> ImplementationAttempt {
    match ok(i.admission_apply(
        Cmd::Claim {
            inputs: Vec::new(),
            lease_ms: LEASE_MS,
        },
        now,
    )) {
        AdmissionResult::Claimed {
            attempt: Some(attempt),
        } => *attempt,
        other => unreachable!("{other:?}"),
    }
}
/// The fresh observations the dispatch binding seals from `admission-status`.
fn fresh(i: &mut Implementer, now: u64) -> Vec<EligibilityInput> {
    ok(i.admission_snapshot())
        .eligibility
        .into_values()
        .filter(|input| input.valid_until_ms > now)
        .collect()
}
fn reserved(result: HostOwnerReserved) -> kiln_core::fair_admission::OwnerReservation {
    match result {
        HostOwnerReserved::Committed { reservation } => *reservation,
        HostOwnerReserved::Retained { reservation } => unreachable!("not new: {reservation:?}"),
    }
}

#[test]
fn sandbox_reservation_commits_and_passes_the_dispatch_gate() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    let admitted = ok(i.host_owner_admit(&profile, NOW));
    assert_eq!(admitted.queued.len(), 1);
    assert_eq!(ok(admitted.queued.first().ok_or(())).class, "bug");
    assert!(admitted.refused.is_empty());
    let observed = ok(i.host_owner_observe(&profile, NOW));
    assert_eq!(observed.selected, Some(ok(issue.identity())));
    let attempt = claim(&mut i, NOW + 1);
    assert_eq!(attempt.work_identity, ok(issue.identity()));
    // Without the owner's committed reservation dispatch is refused.
    let observations = fresh(&mut i, NOW + 2);
    refused(
        i.admission_before_transport(&attempt.identity, attempt.fence, &observations, NOW + 2),
        "missing required owner grant",
    );
    let reservation = reserved(ok(i.host_owner_reserve(
        &profile,
        &attempt.identity,
        attempt.fence,
        NOW + 2,
    )));
    assert_eq!(reservation.state, ReservationState::Committed);
    assert_eq!(reservation.provider, profile.owner.provider);
    // The same owner contract dispatch checks now admits transport intent.
    let observations = fresh(&mut i, NOW + 3);
    ok(i.admission_before_transport(&attempt.identity, attempt.fence, &observations, NOW + 3));
    let ledger = ok(i.admission_snapshot());
    let item = ok(ledger.items.get(&ok(issue.identity())).ok_or(()));
    assert_eq!(item.phase, AdmissionPhase::DispatchIntent);
}

#[test]
fn unregistered_repository_is_observed_but_never_queued_or_reserved() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let other = work(&profile, OTHER, 7);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&other]);
    let admitted = ok(i.host_owner_admit(&profile, NOW));
    assert!(admitted.queued.is_empty());
    assert_eq!(admitted.refused.len(), 1);
    assert!(
        ok(admitted.refused.first().ok_or(()))
            .cause
            .contains("host_owner_unregistered")
    );
    assert!(ok(i.admission_snapshot()).items.is_empty());
    // Under a host-owner policy no claim may bypass admission, even for
    // unregistered work; nothing is claimed or reserved.
    refused(i.claim_next(NOW, LEASE_MS), "host_owner_admission_required");
    assert!(ok(i.attempts()).attempts().next().is_none());
    assert!(ok(i.admission_snapshot()).reservations.is_empty());
}

#[test]
fn claims_bypassing_admission_are_refused_only_under_a_host_owner_policy() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    // A policy without a host-owner root leaves the compatibility claim as it was.
    let mut plain = open(&dir.path().join("plain.sqlite"));
    intake(&mut plain, &[&issue]);
    ok(plain.admission_apply(
        Cmd::Configure {
            policy: ok(profile.policy()),
        },
        NOW,
    ));
    assert!(ok(plain.claim_next(NOW, LEASE_MS)).is_some());
    // A host-owner root governs every claim: the bypass is refused atomically.
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    refused(
        i.claim_next(NOW + 1, LEASE_MS),
        "host_owner_admission_required",
    );
    assert!(ok(i.attempts()).attempts().next().is_none());
    // The admission claim is unaffected and binds the transport start.
    let attempt = claim(&mut i, NOW + 2);
    assert!(ok(i.attempts()).transport_start_bound(&attempt.identity));
}

#[test]
fn stale_observation_is_refused_without_a_request() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    let attempt = claim(&mut i, NOW + 1);
    let late = NOW + profile.owner.observation_ttl_ms;
    refused(
        i.host_owner_reserve(&profile, &attempt.identity, attempt.fence, late),
        "host_owner_stale",
    );
    assert!(ok(i.admission_snapshot()).reservations.is_empty());
    // A fresh observation by the same owner then permits the reservation.
    ok(i.host_owner_observe(&profile, late));
    reserved(ok(i.host_owner_reserve(
        &profile,
        &attempt.identity,
        attempt.fence,
        late + 1,
    )));
}

#[test]
fn second_work_beyond_host_capacity_is_refused() {
    let dir = ok(tempfile::tempdir());
    let mut profile = profile();
    // The campaign would allow two active works; the host has one slot.
    profile.limits.max_active = 2;
    let first = work(&profile, SANDBOX, 1);
    let second = work(&profile, SANDBOX, 2);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&first, &second]);
    assert_eq!(ok(i.host_owner_admit(&profile, NOW)).queued.len(), 2);
    // One free slot invites only the first queued work.
    let observed = ok(i.host_owner_observe(&profile, NOW));
    assert_eq!(
        observed
            .observations
            .iter()
            .filter(|input| input.reason == kiln_core::fair_admission::AdmissionReason::Eligible)
            .count(),
        1
    );
    let a = claim(&mut i, NOW + 1);
    reserved(ok(i.host_owner_reserve(
        &profile,
        &a.identity,
        a.fence,
        NOW + 2,
    )));
    // No claim is invited for the second work while the slot is held.
    assert!(matches!(
        ok(i.admission_apply(
            Cmd::Claim {
                inputs: Vec::new(),
                lease_ms: LEASE_MS,
            },
            NOW + 3,
        )),
        AdmissionResult::Claimed { attempt: None }
    ));
    // A misregistered two-slot observation invites it; the owner still refuses.
    let mut wide = profile.clone();
    wide.owner.capacity = BTreeMap::from([(ok(kiln_core::types::Alias::new("worker_slots")), 2)]);
    ok(i.host_owner_observe(&wide, NOW + 3));
    let b = claim(&mut i, NOW + 3);
    assert_ne!(a.work_identity, b.work_identity);
    refused(
        i.host_owner_reserve(&profile, &b.identity, b.fence, NOW + 4),
        "host_owner_capacity",
    );
    let ledger = ok(i.admission_snapshot());
    assert_eq!(ledger.reservations.len(), 1);
    // The next observation reports the second work waiting on the host.
    let observed = ok(i.host_owner_observe(&profile, NOW + 5));
    let waiting = ok(observed
        .observations
        .iter()
        .find(|input| input.work_version == b.work_identity)
        .ok_or(()));
    assert_eq!(
        waiting.reason,
        kiln_core::fair_admission::AdmissionReason::Waiting
    );
}

#[test]
fn duplicate_reservation_is_refused_and_repeat_is_retained() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    let attempt = claim(&mut i, NOW + 1);
    let first = reserved(ok(i.host_owner_reserve(
        &profile,
        &attempt.identity,
        attempt.fence,
        NOW + 2,
    )));
    let before = ok(i.admission_snapshot());
    match ok(i.host_owner_reserve(&profile, &attempt.identity, attempt.fence, NOW + 3)) {
        HostOwnerReserved::Retained { reservation } => assert_eq!(*reservation, first),
        HostOwnerReserved::Committed { .. } => unreachable!("repeat must not request again"),
    }
    assert_eq!(ok(i.admission_snapshot()), before);
    // A re-registered generation cannot mint a second grant for held work.
    let mut bumped = profile.clone();
    bumped.owner.generation = 2;
    ok(i.host_owner_observe(&bumped, NOW + 4));
    refused(
        i.host_owner_reserve(&bumped, &attempt.identity, attempt.fence, NOW + 5),
        "host_owner_duplicate",
    );
    assert_eq!(ok(i.admission_snapshot()).reservations.len(), 1);
}

#[test]
fn restart_replay_neither_duplicates_nor_resets_and_release_needs_observation() {
    let dir = ok(tempfile::tempdir());
    let path = dir.path().join("work.sqlite");
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&path);
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    let attempt = claim(&mut i, NOW + 1);
    let reservation = reserved(ok(i.host_owner_reserve(
        &profile,
        &attempt.identity,
        attempt.fence,
        NOW + 2,
    )));
    let account = ok(profile.account_identity());
    let held = ok(i.admission_snapshot());
    drop(i);

    // Restart: every stage replays against the retained ledger.
    let mut i = open(&path);
    let replayed = ok(i.host_owner_admit(&profile, NOW + 10));
    assert_eq!(replayed.queued.len(), 1);
    ok(i.host_owner_observe(&profile, NOW + 11));
    match ok(i.host_owner_reserve(&profile, &attempt.identity, attempt.fence, NOW + 12)) {
        HostOwnerReserved::Retained {
            reservation: retained,
        } => {
            assert_eq!(retained.operation, reservation.operation);
        }
        HostOwnerReserved::Committed { .. } => unreachable!("replay must not request again"),
    }
    let after = ok(i.admission_snapshot());
    assert_eq!(after.reservations, held.reservations);
    assert_eq!(after.accounts.get(&account), held.accounts.get(&account));
    assert_eq!(after.quotas, held.quotas);
    // A registration with a larger budget cannot reset the retained root.
    let mut reset = profile.clone();
    reset.quota = BTreeMap::from([(ok(kiln_core::types::Alias::new("worker_slots")), 9)]);
    refused(i.host_owner_admit(&reset, NOW + 13), "cannot reset quota");

    // Neither restart nor elapsed time releases the slot.
    refused(
        i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 14),
        "host_owner_release_unobserved",
    );
    ok(i.cancel(
        &attempt.identity,
        attempt.fence,
        NOW + 15,
        "operator stopped the pilot",
    ));
    let settled = ok(i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 16));
    let encoded = ok(serde_json::to_value(&settled));
    assert_eq!(
        encoded.get("outcome").and_then(serde_json::Value::as_str),
        Some("settled")
    );
    let ledger = ok(i.admission_snapshot());
    let retained = ok(ledger.reservations.get(&reservation.operation).ok_or(()));
    assert_eq!(retained.state, ReservationState::Settled);
    let root = ok(ledger.accounts.get(&account).ok_or(()));
    // Cancelled before any dispatch intent: the slot returns without consumption.
    assert_eq!(
        root.available,
        BTreeMap::from([(ok(kiln_core::types::Alias::new("worker_slots")), 4)])
    );
    assert!(root.consumed.is_empty());
    drop(i);
    let mut i = open(&path);
    let again = ok(i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 17));
    assert_eq!(
        ok(serde_json::to_value(&again))
            .get("outcome")
            .and_then(serde_json::Value::as_str),
        Some("already_settled")
    );
}

fn slots(n: u64) -> BTreeMap<kiln_core::types::Alias, u64> {
    BTreeMap::from([(ok(kiln_core::types::Alias::new("worker_slots")), n)])
}

/// Admit, observe, claim and reserve one sandbox issue.
fn reserved_attempt(
    i: &mut Implementer,
    profile: &HostOwnerProfile,
    issue: &ImplementationWork,
) -> (
    ImplementationAttempt,
    kiln_core::fair_admission::OwnerReservation,
) {
    intake(i, &[issue]);
    ok(i.host_owner_admit(profile, NOW));
    ok(i.host_owner_observe(profile, NOW));
    let attempt = claim(i, NOW + 1);
    let reservation = reserved(ok(i.host_owner_reserve(
        profile,
        &attempt.identity,
        attempt.fence,
        NOW + 2,
    )));
    (attempt, reservation)
}

/// The two in-band dispatch routes: the worker request reconciles the native
/// workflow, the workflow exchange acknowledges it with `start_execution`.
#[derive(Clone, Copy)]
enum Route {
    Worker,
    Exchange,
}

/// The real dispatch sequence: plan reservation, owner-grant gate, sealed
/// request, transport start, Lattice acknowledgement and execution observation.
fn dispatch_via(
    i: &mut Implementer,
    issue: &ImplementationWork,
    attempt: &ImplementationAttempt,
    now: u64,
    route: Route,
) {
    let plan = plan_for(issue, attempt);
    let reserved = ok(i.reserve_dispatch_plan(issue, &attempt.identity, attempt.fence, now, &plan));
    let observations = fresh(i, now);
    ok(i.admission_before_transport(&attempt.identity, attempt.fence, &observations, now));
    ok(i.bind_campaign_request(issue, &attempt.identity, attempt.fence, now, b"{}"));
    ok(i.begin_dispatch_transport(&attempt.identity, attempt.fence, now, &reserved.plan, None));
    let workflow = ok(ExternalRef::new("lattice", "workflow", "pilot-run-1"));
    match route {
        Route::Worker => ok(i.reconcile_dispatch(&attempt.identity, attempt.fence, now, &workflow)),
        Route::Exchange => ok(i.start_execution(&attempt.identity, attempt.fence, now, &workflow)),
    }
    ok(i.admission_execution_observed(&attempt.identity, attempt.fence, now));
}

fn dispatch(
    i: &mut Implementer,
    issue: &ImplementationWork,
    attempt: &ImplementationAttempt,
    now: u64,
) {
    dispatch_via(i, issue, attempt, now, Route::Exchange);
}

fn reservation_state(
    i: &mut Implementer,
    reservation: &kiln_core::fair_admission::OwnerReservation,
) -> ReservationState {
    ok(ok(i.admission_snapshot())
        .reservations
        .get(&reservation.operation)
        .map(|r| r.state)
        .ok_or(()))
}

#[test]
fn superseded_dispatched_holder_keeps_its_slot_until_termination_is_observed() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, reservation) = reserved_attempt(&mut i, &profile, &issue);
    dispatch(&mut i, &issue, &attempt, NOW + 3);
    // A newer issue revision aborts the attempt without observing its worker.
    let mut newer = issue.clone();
    newer.revision = 2;
    newer.title = "issue 1, edited".into();
    ok(i.admit_with_supersession(&newer, None, SupersessionRule::AbortActive, NOW + 4));
    refused(
        i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 5),
        "host_owner_release_unobserved",
    );
    assert_eq!(
        reservation_state(&mut i, &reservation),
        ReservationState::Committed
    );
    // An independent termination observation then releases and consumes it.
    let stopped = ok(ExternalRef::new("lattice", "termination", "pilot-run-1"));
    ok(i.reconcile_unknown(&attempt.identity, attempt.fence, &stopped));
    ok(i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 6));
    let ledger = ok(i.admission_snapshot());
    let root = ok(ledger
        .accounts
        .get(&ok(profile.account_identity()))
        .ok_or(()));
    assert_eq!(root.available, slots(3));
    assert_eq!(root.consumed, slots(1));
}

#[test]
fn lease_expired_acknowledged_holder_keeps_its_slot() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, reservation) = reserved_attempt(&mut i, &profile, &issue);
    dispatch(&mut i, &issue, &attempt, NOW + 3);
    let recovery = ok(i.recover(attempt.lease_until_ms + 1));
    assert!(recovery.expired.contains(&attempt.identity));
    refused(
        i.host_owner_release(
            &profile,
            &attempt.identity,
            attempt.fence,
            attempt.lease_until_ms + 2,
        ),
        "host_owner_release_unobserved",
    );
    assert_ne!(
        reservation_state(&mut i, &reservation),
        ReservationState::Settled
    );
}

#[test]
fn dispatch_refuses_a_held_grant_after_a_generation_bump() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, _) = reserved_attempt(&mut i, &profile, &issue);
    let mut bumped = profile.clone();
    bumped.owner.generation = 2;
    ok(i.host_owner_observe(&bumped, NOW + 3));
    let observations = fresh(&mut i, NOW + 4);
    refused(
        i.admission_before_transport(&attempt.identity, attempt.fence, &observations, NOW + 4),
        "grant generation, eligibility or freshness changed",
    );
}

#[test]
fn interrupted_reserve_is_resolved_from_its_own_record() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    let attempt = claim(&mut i, NOW + 1);
    let claim_ref = kiln_core::fair_admission::ClaimRef {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    // Crash after the tentative request committed, before the owner replied.
    let snapshot = ok(i.admission_snapshot());
    let kiln_core::fair_admission::HostReservationPlan::Request(pending) = ok(profile
        .plan_reservation(
            &snapshot,
            &issue,
            &claim_ref,
            attempt.lease_until_ms,
            NOW + 2,
        ))
    else {
        unreachable!("fresh claim plans a request")
    };
    ok(i.admission_apply(
        Cmd::Request {
            reservation: pending.clone(),
            inputs: Vec::new(),
        },
        NOW + 2,
    ));
    // The registration then changes generation; the stale request is rejected
    // with refund and a fresh request of the new generation is committed.
    let mut bumped = profile.clone();
    bumped.owner.generation = 2;
    ok(i.host_owner_observe(&bumped, NOW + 3));
    let committed = reserved(ok(i.host_owner_reserve(
        &bumped,
        &attempt.identity,
        attempt.fence,
        NOW + 4,
    )));
    assert_eq!(committed.generation, 2);
    assert_eq!(
        reservation_state(&mut i, &pending),
        ReservationState::Rejected
    );
    let ledger = ok(i.admission_snapshot());
    let root = ok(ledger
        .accounts
        .get(&ok(profile.account_identity()))
        .ok_or(()));
    assert_eq!(root.available, slots(3));
}

fn plan_for(
    issue: &ImplementationWork,
    attempt: &ImplementationAttempt,
) -> ImplementationDispatchPlan {
    ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.to_string(),
        work_identity: ok(issue.identity()),
        work_revision: issue.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: issue.repository.clone(),
        base: issue.base.clone(),
        workspace: DispatchWorkspace {
            identity: ok(ContentId::from_blake3_hex("d".repeat(64))),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: ok(kiln_core::types::ProviderProfileId::new(
            "onix-local-pilot-omp",
        )),
        permitted_paths: vec!["src".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 20,
        token_budget: 10_000,
        invocation_budget: 10,
        expected_output: ok(ContentId::from_blake3_hex("e".repeat(64))),
        execution: None,
    }
}

fn assert_consumed(i: &mut Implementer, profile: &HostOwnerProfile) {
    let ledger = ok(i.admission_snapshot());
    let root = ok(ledger
        .accounts
        .get(&ok(profile.account_identity()))
        .ok_or(()));
    assert_eq!(root.available, slots(3));
    assert_eq!(root.consumed, slots(1));
}

#[test]
fn lattice_refusal_before_any_effect_releases_the_slot() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, reservation) = reserved_attempt(&mut i, &profile, &issue);
    let plan = plan_for(&issue, &attempt);
    let reserved_plan =
        ok(i.reserve_dispatch_plan(&issue, &attempt.identity, attempt.fence, NOW + 3, &plan));
    let observations = fresh(&mut i, NOW + 3);
    ok(i.admission_before_transport(&attempt.identity, attempt.fence, &observations, NOW + 3));
    ok(i.bind_campaign_request(&issue, &attempt.identity, attempt.fence, NOW + 3, b"{}"));
    ok(i.begin_dispatch_transport(
        &attempt.identity,
        attempt.fence,
        NOW + 3,
        &reserved_plan.plan,
        None,
    ));
    // While transport is started and unanswered, the slot is held.
    refused(
        i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 4),
        "host_owner_release_unobserved",
    );
    // Lattice's no-effect witness settles the attempt and clears the start.
    ok(i.settle_dispatch_without_execution(
        &attempt.identity,
        attempt.fence,
        NOW + 5,
        "lattice_refused_before_effect:crossed_attempt",
    ));
    ok(i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 6));
    assert_eq!(
        reservation_state(&mut i, &reservation),
        ReservationState::Settled
    );
    // The recorded dispatch intent consumes the slot's budget.
    assert_consumed(&mut i, &profile);
}

#[test]
fn crash_between_the_grant_gate_and_transport_does_not_hold_forever() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, _) = reserved_attempt(&mut i, &profile, &issue);
    let observations = fresh(&mut i, NOW + 3);
    ok(i.admission_before_transport(&attempt.identity, attempt.fence, &observations, NOW + 3));
    // The process stops before any transport start; the lease then expires.
    ok(i.recover(attempt.lease_until_ms + 1));
    ok(i.host_owner_release(
        &profile,
        &attempt.identity,
        attempt.fence,
        attempt.lease_until_ms + 2,
    ));
    assert_consumed(&mut i, &profile);
}

#[test]
fn observed_failed_run_without_a_candidate_is_released_and_consumed() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, _) = reserved_attempt(&mut i, &profile, &issue);
    dispatch(&mut i, &issue, &attempt, NOW + 3);
    // Kiln retains Lattice's complete native result: the run ended, failed.
    let observation = WorkerObservation {
        terminal: WorkerTerminalState::Failed,
        worker_profile: None,
        harness: None,
        model: None,
        usage: AttemptUsage::default(),
    };
    ok(i.record_worker_observation(&attempt.identity, attempt.fence, &observation));
    ok(i.recover(attempt.lease_until_ms + 1));
    ok(i.host_owner_release(
        &profile,
        &attempt.identity,
        attempt.fence,
        attempt.lease_until_ms + 2,
    ));
    assert_consumed(&mut i, &profile);
}

fn eligible(observed: &kiln_implementer::host_owner::HostOwnerObserved, work: &ContentId) -> bool {
    observed.observations.iter().any(|input| {
        &input.work_version == work
            && input.reason == kiln_core::fair_admission::AdmissionReason::Eligible
    })
}

#[test]
fn a_dead_claim_does_not_block_usable_work() {
    let dir = ok(tempfile::tempdir());
    let mut profile = profile();
    profile.limits.max_active = 2;
    let first = work(&profile, SANDBOX, 1);
    let second = work(&profile, SANDBOX, 2);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&first, &second]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    // The first work is claimed with a short lease that then lapses unreserved.
    let AdmissionResult::Claimed {
        attempt: Some(short),
    } = ok(i.admission_apply(
        Cmd::Claim {
            inputs: Vec::new(),
            lease_ms: 1_000,
        },
        NOW + 1,
    ))
    else {
        unreachable!("the top-ranked work is invited")
    };
    let (dead, other) = if short.work_identity == ok(first.identity()) {
        (ok(first.identity()), ok(second.identity()))
    } else {
        (ok(second.identity()), ok(first.identity()))
    };
    // Before the lease lapses the live claim holds the only invitation.
    let live = ok(i.host_owner_observe(&profile, NOW + 2));
    assert!(eligible(&live, &dead) && !eligible(&live, &other));
    let observed = ok(i.host_owner_observe(&profile, NOW + 5_000));
    assert!(eligible(&observed, &other));
    assert!(!eligible(&observed, &dead));
}

#[test]
fn scarce_slots_follow_the_work_ranking() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let low = work(&profile, SANDBOX, 1);
    let mut high = work(&profile, SANDBOX, 2);
    high.priority = 9;
    let mut i = open(&dir.path().join("work.sqlite"));
    // The lower-priority work is queued first.
    intake(&mut i, &[&low, &high]);
    ok(i.host_owner_admit(&profile, NOW));
    let observed = ok(i.host_owner_observe(&profile, NOW));
    assert!(eligible(&observed, &ok(high.identity())));
    assert!(!eligible(&observed, &ok(low.identity())));
}

#[test]
fn older_same_priority_work_keeps_the_single_slot() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let a = work(&profile, SANDBOX, 1);
    let b = work(&profile, SANDBOX, 2);
    // The newer work has the smaller canonical key, so key order would pick it.
    let (older, newer) = if ok(a.canonical_key()) > ok(b.canonical_key()) {
        (a, b)
    } else {
        (b, a)
    };
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&older]);
    ok(i.host_owner_admit(&profile, NOW));
    intake(&mut i, &[&newer]);
    ok(i.host_owner_admit(&profile, NOW + 1));
    for at in [NOW + 2, NOW + 3, NOW + 4] {
        let observed = ok(i.host_owner_observe(&profile, at));
        assert!(eligible(&observed, &ok(older.identity())));
        assert!(!eligible(&observed, &ok(newer.identity())));
    }
}

#[test]
fn a_de_enrolled_actor_does_not_block_other_work() {
    let dir = ok(tempfile::tempdir());
    let path = dir.path().join("work.sqlite");
    let profile = profile();
    let mut former = work(&profile, SANDBOX, 1);
    former.actor = "former".into();
    let current = work(&profile, SANDBOX, 2);
    let both = Enrollment {
        repositories: BTreeSet::from([SANDBOX.into(), OTHER.into()]),
        actors: BTreeSet::from(["operator".into(), "former".into()]),
    };
    let mut i = ok(Implementer::open_with_local_operator(
        &path,
        both,
        "pilot-operator",
    ));
    intake(&mut i, &[&former]);
    ok(i.host_owner_admit(&profile, NOW));
    intake(&mut i, &[&current]);
    ok(i.host_owner_admit(&profile, NOW + 1));
    drop(i);
    // The former actor is de-enrolled; its older row no longer competes.
    let mut i = open(&path);
    let observed = ok(i.host_owner_observe(&profile, NOW + 2));
    assert!(eligible(&observed, &ok(current.identity())));
    assert!(!eligible(&observed, &ok(former.identity())));
}

const TRANSPORT_START_REQUIRED: &str = "implementation_transport_start_required";

fn plan_id() -> ContentId {
    ok(ContentId::from_blake3_hex("7".repeat(64)))
}

fn provider() -> kiln_core::types::ProviderProfileId {
    ok(kiln_core::types::ProviderProfileId::new(
        "onix-local-pilot-omp",
    ))
}

#[test]
fn out_of_band_acknowledgement_of_a_granted_host_owner_claim_is_refused() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    let (attempt, _) = reserved_attempt(&mut i, &profile, &issue);
    // `reserve`, an out-of-band launch, then `reconcile` or `start`.
    ok(i.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        NOW + 3,
        &plan_id(),
        &provider(),
    ));
    let workflow = ok(ExternalRef::new("lattice", "workflow", "out-of-band"));
    refused(
        i.reconcile_dispatch(&attempt.identity, attempt.fence, NOW + 4, &workflow),
        TRANSPORT_START_REQUIRED,
    );
    refused(
        i.start_execution(&attempt.identity, attempt.fence, NOW + 4, &workflow),
        TRANSPORT_START_REQUIRED,
    );
    let attempts = ok(i.attempts());
    assert!(
        ok(attempts.get(&attempt.identity).ok_or(()))
            .workflow
            .is_none()
    );
}

#[test]
fn an_ungranted_host_owner_claim_cannot_adopt_a_worker() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&issue]);
    ok(i.host_owner_admit(&profile, NOW));
    ok(i.host_owner_observe(&profile, NOW));
    // Claimed through admission, but never granted a host-owner reservation.
    let attempt = claim(&mut i, NOW + 1);
    assert!(ok(i.attempts()).transport_start_bound(&attempt.identity));
    ok(i.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        NOW + 2,
        &plan_id(),
        &provider(),
    ));
    let workflow = ok(ExternalRef::new("lattice", "workflow", "ungranted"));
    refused(
        i.start_execution(&attempt.identity, attempt.fence, NOW + 3, &workflow),
        TRANSPORT_START_REQUIRED,
    );
    refused(
        i.reconcile_dispatch(&attempt.identity, attempt.fence, NOW + 3, &workflow),
        TRANSPORT_START_REQUIRED,
    );
}

#[test]
fn both_in_band_routes_still_record_the_workflow() {
    for route in [Route::Worker, Route::Exchange] {
        let dir = ok(tempfile::tempdir());
        let profile = profile();
        let issue = work(&profile, SANDBOX, 1);
        let mut i = open(&dir.path().join("work.sqlite"));
        let (attempt, _) = reserved_attempt(&mut i, &profile, &issue);
        dispatch_via(&mut i, &issue, &attempt, NOW + 3, route);
        let attempts = ok(i.attempts());
        assert!(
            ok(attempts.get(&attempt.identity).ok_or(()))
                .workflow
                .is_some()
        );
    }
}

#[test]
fn start_and_reconcile_are_unchanged_without_host_owner_admission() {
    let dir = ok(tempfile::tempdir());
    let profile = profile();
    let first = work(&profile, SANDBOX, 1);
    let second = work(&profile, SANDBOX, 2);
    let mut i = open(&dir.path().join("work.sqlite"));
    intake(&mut i, &[&first, &second]);
    // No admission policy: the compatibility claim, out-of-band acknowledgement.
    let started = ok(ok(i.claim_next(NOW, LEASE_MS)).ok_or(()));
    let reconciled = ok(ok(i.claim_next(NOW, LEASE_MS)).ok_or(()));
    for attempt in [&started, &reconciled] {
        assert!(!ok(i.attempts()).transport_start_bound(&attempt.identity));
        ok(i.reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            NOW + 1,
            &plan_id(),
            &provider(),
        ));
    }
    let workflow = ok(ExternalRef::new("lattice", "workflow", "operator-a"));
    ok(i.start_execution(&started.identity, started.fence, NOW + 2, &workflow));
    let workflow = ok(ExternalRef::new("lattice", "workflow", "operator-b"));
    ok(i.reconcile_dispatch(&reconciled.identity, reconciled.fence, NOW + 2, &workflow));
}

/// A claim whose owner request committed tentatively and whose owner reply
/// was lost, as a crash between request and reply leaves it.
struct Interrupted {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
    profile: HostOwnerProfile,
    attempt: ImplementationAttempt,
    pending: kiln_core::fair_admission::OwnerReservation,
}

impl Interrupted {
    fn new() -> Self {
        let dir = ok(tempfile::tempdir());
        let path = dir.path().join("work.sqlite");
        let profile = profile();
        let issue = work(&profile, SANDBOX, 1);
        let mut i = open(&path);
        intake(&mut i, &[&issue]);
        ok(i.host_owner_admit(&profile, NOW));
        ok(i.host_owner_observe(&profile, NOW));
        let attempt = claim(&mut i, NOW + 1);
        let claim_ref = kiln_core::fair_admission::ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        };
        let snapshot = ok(i.admission_snapshot());
        let kiln_core::fair_admission::HostReservationPlan::Request(pending) = ok(profile
            .plan_reservation(
                &snapshot,
                &issue,
                &claim_ref,
                attempt.lease_until_ms,
                NOW + 2,
            ))
        else {
            unreachable!("fresh claim plans a request")
        };
        ok(i.admission_apply(
            Cmd::Request {
                reservation: pending.clone(),
                inputs: Vec::new(),
            },
            NOW + 2,
        ));
        assert_eq!(
            reservation_state(&mut i, &pending),
            ReservationState::Tentative
        );
        Self {
            _dir: dir,
            path,
            profile,
            attempt,
            pending,
        }
    }

    /// Resume the pending request through `host_owner_reserve`, or through
    /// the retained-request path of `admission_reserve_with_owner`.
    fn resume(&self, i: &mut Implementer, retained: bool) -> Result<(), String> {
        if retained {
            let ledger = i.admission_snapshot()?;
            let mut owner =
                kiln_implementer::host_owner::HostLocalOwner::new(&self.profile, ledger, NOW + 3);
            i.admission_reserve_with_owner(self.pending.clone(), Vec::new(), &mut owner, NOW + 3)
                .map(drop)
        } else {
            i.host_owner_reserve(
                &self.profile,
                &self.attempt.identity,
                self.attempt.fence,
                NOW + 3,
            )
            .map(drop)
        }
    }
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.denied]
fn a_tentative_owner_request_is_never_committed_without_a_fresh_dispatch_decision() {
    for retained in [false, true] {
        let interrupted = Interrupted::new();
        // The retry selects a profile that refuses Dispatch, as a revoked
        // grant does: the owner is never asked and nothing commits.
        let mut i = ok(Implementer::open(&interrupted.path, enrollment()));
        let error = interrupted
            .resume(&mut i, retained)
            .err()
            .unwrap_or_else(|| unreachable!("retained={retained}: must be refused"));
        assert!(error.contains("authority:"), "retained={retained}: {error}");
        assert_eq!(
            reservation_state(&mut i, &interrupted.pending),
            ReservationState::Tentative,
            "retained={retained}"
        );
    }
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
fn a_tentative_owner_request_commits_after_a_fresh_dispatch_decision() {
    for retained in [false, true] {
        let interrupted = Interrupted::new();
        let mut i = open(&interrupted.path);
        ok(interrupted.resume(&mut i, retained));
        assert_eq!(
            reservation_state(&mut i, &interrupted.pending),
            ReservationState::Committed,
            "retained={retained}"
        );
        assert!(
            i.authority_decisions()
                .iter()
                .any(|decision| decision.allowed
                    && decision.operation
                        == kiln_core::implementation::authority::ProtectedOperation::Dispatch),
            "retained={retained}: the resumed request presented a fresh invocation"
        );
    }
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn host_owner_release_settles_under_the_verified_profile_without_the_coordinator_seed() {
    use kiln_core::implementation::authority::{ENTRY_POINTS, IMPLEMENTER_CLI, entry_point_roles};
    use kiln_core::implementation::grant::GrantRole;
    use kiln_implementer::authority_config::ImplementationAuthorityConfig;

    // The cleanup-only commands check no authority, so they load no seed.
    for command in ["host-owner-release", "metrics", "reconcile-termination"] {
        assert!(
            entry_point_roles(ENTRY_POINTS, IMPLEMENTER_CLI, command).is_empty(),
            "{command}"
        );
    }
    let dir = ok(tempfile::tempdir());
    let path = dir.path().join("work.sqlite");
    let profile = profile();
    let issue = work(&profile, SANDBOX, 1);
    let mut i = open(&path);
    let (attempt, reservation) = reserved_attempt(&mut i, &profile, &issue);
    ok(i.cancel(
        &attempt.identity,
        attempt.fence,
        NOW + 3,
        "operator stopped the pilot",
    ));
    drop(i);

    // The host switches to the verified profile and the coordinator's seed
    // is gone: a coordinator entry point can no longer select it.
    let config = authority_common::write_config(
        dir.path(),
        &kiln_adapter_basalt::test_support::Fixture::direct(),
        &[],
    );
    ok(std::fs::remove_file(dir.path().join("coordinator.seed")));
    let select = |roles: &BTreeSet<GrantRole>| -> Result<Implementer, String> {
        let mut i = Implementer::open(&path, enrollment())?;
        i.set_clock_ms(authority_common::T);
        i.select_authority(&ImplementationAuthorityConfig::load(&config)?, roles)?;
        Ok(i)
    };
    assert!(select(&BTreeSet::from([GrantRole::Coding])).is_err());

    // `host-owner-release` selects exactly its table roles and settles.
    let mut i = ok(select(&entry_point_roles(
        ENTRY_POINTS,
        IMPLEMENTER_CLI,
        "host-owner-release",
    )));
    let settled = ok(i.host_owner_release(&profile, &attempt.identity, attempt.fence, NOW + 4));
    assert_eq!(
        ok(serde_json::to_value(&settled))
            .get("outcome")
            .and_then(serde_json::Value::as_str),
        Some("settled")
    );
    assert_eq!(
        reservation_state(&mut i, &reservation),
        ReservationState::Settled
    );
    assert!(i.authority_decisions().is_empty(), "release checks nothing");
}
