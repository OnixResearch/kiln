// r[verify kiln.admission.host_owner]
// r[verify kiln.admission.host_owner.scenario.reserve]
// r[verify kiln.admission.host_owner.scenario.refused]
// r[verify kiln.admission.host_owner.scenario.replay]
//! Durable host-local owner admission through the real implementation store.
use kiln_core::fair_admission::{
    AdmissionPhase, EligibilityInput, HostOwnerProfile, ReservationState,
};
use kiln_core::implementation::{
    AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt,
    ImplementationWork, SourceRef, SupersessionRule, TerminalPolicy, WorkBudget, WorkSourceKind,
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
        base: None,
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
    // A claim taken outside fair admission still gets no host reservation.
    let attempt = ok(ok(i.claim_next(NOW, LEASE_MS)).ok_or(()));
    refused(
        i.host_owner_reserve(&profile, &attempt.identity, attempt.fence, NOW + 1),
        "host_owner_unregistered",
    );
    assert!(ok(i.admission_snapshot()).reservations.is_empty());
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

/// The real dispatch sequence: plan reservation, owner-grant gate, transport
/// intent, Lattice acknowledgement and execution observation.
fn dispatch(i: &mut Implementer, attempt: &ImplementationAttempt, now: u64) {
    let plan = ok(ContentId::from_blake3_hex("7".repeat(64)));
    let provider = ok(kiln_core::types::ProviderProfileId::new(
        "onix-local-pilot-omp",
    ));
    ok(i.reserve_dispatch(&attempt.identity, attempt.fence, now, &plan, &provider));
    let observations = fresh(i, now);
    ok(i.admission_before_transport(&attempt.identity, attempt.fence, &observations, now));
    // Transport intent needs the sealed request bytes; the acknowledged
    // workflow below is the possible-effect fact release must respect.
    let workflow = ok(ExternalRef::new("lattice", "workflow", "pilot-run-1"));
    ok(i.start_execution(&attempt.identity, attempt.fence, now, &workflow));
    ok(i.admission_execution_observed(&attempt.identity, attempt.fence, now));
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
    dispatch(&mut i, &attempt, NOW + 3);
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
    dispatch(&mut i, &attempt, NOW + 3);
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
