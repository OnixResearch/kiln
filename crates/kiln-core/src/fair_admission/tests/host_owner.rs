// r[verify kiln.admission.host_owner]
// r[verify kiln.admission.host_owner_profile]
// r[verify kiln.admission.host_owner_profile.scenario.exports]
use super::*;
use crate::implementation::{
    AcceptanceObligation, AttemptLedger, Enrollment, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkLedger, WorkSourceKind,
};
use crate::types::ExternalRef;

const HOST_OWNER_EXPORT: &str =
    include_str!("../../../../../fixtures/fair-admission/host-owner.json");
const NOW: u64 = 1_000_000;

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("{error:?}"),
    }
}
fn code<T: core::fmt::Debug>(result: CoreResult<T>) -> alloc::string::String {
    match result {
        Ok(value) => unreachable!("expected refusal, got {value:?}"),
        Err(error) => error.code,
    }
}
fn profile() -> HostOwnerProfile {
    let profile: HostOwnerProfile = ok(serde_json::from_str(HOST_OWNER_EXPORT));
    ok(profile.validate());
    profile
}
/// Two concurrently active works on a one-slot host.
fn two_active() -> HostOwnerProfile {
    let mut profile = profile();
    profile.limits.max_active = 2;
    ok(profile.validate());
    profile
}
fn work(profile: &HostOwnerProfile, repository: &str, issue: u64) -> ImplementationWork {
    let native = alloc::format!("{repository}#{issue}");
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: repository.into(),
        project: "implementer-sandbox".into(),
        base: None,
        title: alloc::format!("issue {issue}"),
        requested_outcome: "fix".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a".into(),
            statement: "tested".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["src".into()],
        permitted_effects: alloc::collections::BTreeSet::from(["write_source".into()]),
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
            native_id: native.clone(),
            version: id(&native),
        }],
    }
}
fn sandbox(profile: &HostOwnerProfile, issue: u64) -> ImplementationWork {
    work(profile, "onixresearch/implementer-sandbox", issue)
}
fn claim(name: &str) -> ClaimRef {
    ClaimRef {
        attempt: id(name),
        fence: 1,
    }
}
fn enrollment() -> Enrollment {
    Enrollment {
        repositories: alloc::collections::BTreeSet::from([
            "onixresearch/implementer-sandbox".into()
        ]),
        actors: alloc::collections::BTreeSet::from(["operator".into()]),
    }
}
/// The current ready work ledger for `works`.
fn work_ledger(works: &[&ImplementationWork]) -> WorkLedger {
    let enrollment = enrollment();
    let mut ledger = WorkLedger::default();
    for work in works {
        ok(ledger.admit(work, None, &enrollment));
    }
    ledger
}
/// Configure, root, queue, observe and claim through the existing ledger.
fn admitted(profile: &HostOwnerProfile, works: &[&ImplementationWork]) -> AdmissionLedger {
    let mut ledger = AdmissionLedger::default();
    ok(ledger.configure(ok(profile.policy())));
    ok(ledger.add_root(ok(profile.root_account())));
    for work in works {
        let reason = ok(ledger.enqueue(ok(profile.queue_item(work, NOW)), NOW));
        assert_eq!(reason, AdmissionReason::Waiting);
    }
    ok(ledger.observe(ok(profile.usage_observation(&ledger, NOW))));
    let inputs = ok(profile.eligibility(
        &ledger,
        &work_ledger(works),
        &AttemptLedger::default(),
        &enrollment(),
        NOW,
    ));
    assert_eq!(inputs.len(), works.len());
    ok(ledger.select(&inputs, NOW));
    ledger
}
fn reserve(
    profile: &HostOwnerProfile,
    ledger: &mut AdmissionLedger,
    work: &ImplementationWork,
    claim: &ClaimRef,
    now: u64,
) -> OwnerReservation {
    let HostReservationPlan::Request(request) =
        ok(profile.plan_reservation(ledger, work, claim, u64::MAX, now))
    else {
        unreachable!("fresh claim must plan a new request")
    };
    ok(ledger.request(request.clone(), now));
    let HostOwnerDecision::Committed(reply) = ok(profile.decide(ledger, &request, now)) else {
        unreachable!("free host capacity must commit")
    };
    assert_eq!(
        ok(ledger.owner_reply(&reply, now)),
        AdmissionReason::Eligible
    );
    request
}

#[test]
fn export_admits_with_the_derived_policy_account_and_queue_row() {
    let profile = profile();
    let policy = ok(profile.policy());
    assert_eq!(policy.accounting_basis, ok(profile.accounting_basis()));
    assert_eq!(policy.shares.len(), 1);
    let item = ok(profile.queue_item(&sandbox(&profile, 1), NOW));
    assert_eq!(item.repository.as_str(), "onixresearch/implementer-sandbox");
    assert_eq!(
        item.required_owners,
        BTreeMap::from([(profile.owner.provider.clone(), ChargeKind::Execution)])
    );
    assert_eq!(item.budget, profile.per_work);
    assert_eq!(item.deadline_ms, profile.deadline_ms);
    // A capacity value change keeps the accounting basis and policy identity.
    let mut grown = profile.clone();
    grown.owner.capacity = BTreeMap::from([(alias("worker_slots"), 2)]);
    grown.owner.generation = 2;
    assert_eq!(ok(grown.policy()), policy);
}

#[test]
fn rust_readmission_refuses_what_the_contract_refuses() {
    let mut over = profile();
    over.per_work = BTreeMap::from([(alias("worker_slots"), 2)]);
    over.quota = BTreeMap::from([(alias("worker_slots"), 4)]);
    assert_eq!(code(over.validate()), "host_owner_profile");
    let mut window = profile();
    window.owner.observation_ttl_ms = window.limits.max_observation_age_ms + 1;
    assert_eq!(code(window.validate()), "host_owner_profile");
    let mut upper = profile();
    ok(upper.repositories.first_mut().ok_or(())).repository =
        "OnixResearch/implementer-sandbox".into();
    assert_eq!(code(upper.validate()), "host_owner_profile");
    let mut empty = profile();
    empty.repositories.clear();
    assert_eq!(code(empty.validate()), "host_owner_profile");
    let mut class = profile();
    ok(class
        .repositories
        .first_mut()
        .and_then(|entry| entry.classes.get_mut(1))
        .ok_or(()))
    .name = alias("bug");
    assert_eq!(code(class.validate()), "host_owner_profile");
    let mut quota = profile();
    quota.quota = BTreeMap::from([(alias("cpu_ms"), 4)]);
    assert_eq!(code(quota.validate()), "host_owner_profile");
    let mut generation = profile();
    generation.owner.generation = 0;
    assert_eq!(code(generation.validate()), "host_owner_profile");
}

#[test]
fn unregistered_repository_or_class_is_never_queued_or_reserved() {
    let profile = profile();
    let other = work(&profile, "onixresearch/other", 1);
    assert_eq!(
        code(profile.queue_item(&other, NOW)),
        "host_owner_unregistered"
    );
    let mut unknown_class = sandbox(&profile, 1);
    unknown_class.verification_profile = id("unregistered-checks");
    assert_eq!(
        code(profile.queue_item(&unknown_class, NOW)),
        "host_owner_unregistered"
    );
    let ledger = admitted(&profile, &[]);
    assert_eq!(
        code(profile.plan_reservation(&ledger, &other, &claim("a"), u64::MAX, NOW)),
        "host_owner_unregistered"
    );
    // Registered but never queued work has no owner request either.
    assert_eq!(
        code(profile.plan_reservation(&ledger, &sandbox(&profile, 1), &claim("a"), u64::MAX, NOW)),
        "host_owner_unqueued"
    );
}

#[test]
fn reservation_commits_for_the_sandbox_claim_and_permits_dispatch_intent() {
    let profile = profile();
    let issue = sandbox(&profile, 1);
    let work_version = ok(issue.identity());
    let mut ledger = admitted(&profile, &[&issue]);
    let attempt = claim("attempt-1");
    // An unclaimed queue row cannot be reserved.
    assert_eq!(
        code(profile.plan_reservation(&ledger, &issue, &attempt, u64::MAX, NOW)),
        "host_owner_claim"
    );
    ok(ledger.bind_claim(&work_version, attempt.clone()));
    let request = reserve(&profile, &mut ledger, &issue, &attempt, NOW);
    let retained = ok(ledger.reservations.get(&request.operation).ok_or(()));
    assert_eq!(retained.state, ReservationState::Committed);
    assert_eq!(retained.kind, ChargeKind::Execution);
    let account = ok(ledger
        .accounts
        .get(&ok(profile.account_identity()))
        .ok_or(()));
    assert_eq!(
        account.available,
        BTreeMap::from([(alias("worker_slots"), 3)])
    );
    // Replay plans the retained operation and never a second request.
    assert_eq!(
        ok(profile.plan_reservation(&ledger, &issue, &attempt, u64::MAX, NOW + 1)),
        HostReservationPlan::Retained(retained.clone())
    );
    // The campaign root is recognised as a host-owner account; another is not.
    assert!(ok(is_host_owner_account(
        &ledger,
        &ok(profile.account_identity())
    )));
    assert!(!ok(is_host_owner_account(&ledger, &id("other-account"))));
    let policy = ok(ledger.policy_ref());
    ok(ledger.dispatch_intent(&request.operation, &policy, &attempt, 1, NOW + 1));
}

#[test]
fn stale_or_foreign_generation_observation_is_refused() {
    let profile = profile();
    let issue = sandbox(&profile, 1);
    let mut ledger = admitted(&profile, &[&issue]);
    let attempt = claim("attempt-1");
    ok(ledger.bind_claim(&ok(issue.identity()), attempt.clone()));
    let later = NOW + profile.owner.observation_ttl_ms;
    assert_eq!(
        code(profile.plan_reservation(&ledger, &issue, &attempt, u64::MAX, later)),
        "host_owner_stale"
    );
    let mut bumped = profile.clone();
    bumped.owner.generation = 2;
    assert_eq!(
        code(bumped.plan_reservation(&ledger, &issue, &attempt, u64::MAX, NOW)),
        "host_owner_stale"
    );
}

#[test]
fn second_work_beyond_registered_capacity_is_refused_and_waits() {
    let profile = two_active();
    let first = sandbox(&profile, 1);
    let second = sandbox(&profile, 2);
    // Eligibility is capped at the one free slot: only the first queued work
    // is invited.
    let mut capped = AdmissionLedger::default();
    ok(capped.configure(ok(profile.policy())));
    ok(capped.add_root(ok(profile.root_account())));
    for work in [&first, &second] {
        ok(capped.enqueue(ok(profile.queue_item(work, NOW)), NOW));
    }
    let invited = ok(profile.eligibility(
        &capped,
        &work_ledger(&[&first, &second]),
        &AttemptLedger::default(),
        &enrollment(),
        NOW,
    ));
    assert_eq!(
        invited
            .iter()
            .filter(|input| input.reason == AdmissionReason::Eligible)
            .count(),
        1
    );
    // A misregistered two-slot view invited both; the real owner still refuses.
    let mut wide = profile.clone();
    wide.owner.capacity = BTreeMap::from([(alias("worker_slots"), 2)]);
    let mut ledger = admitted(&wide, &[&first, &second]);
    let (a, b) = (claim("attempt-1"), claim("attempt-2"));
    ok(ledger.bind_claim(&ok(first.identity()), a.clone()));
    ok(ledger.bind_claim(&ok(second.identity()), b.clone()));
    reserve(&profile, &mut ledger, &first, &a, NOW);
    assert_eq!(
        code(profile.plan_reservation(&ledger, &second, &b, u64::MAX, NOW)),
        "host_owner_capacity"
    );
    // A fresh observation now marks the second work waiting, never eligible.
    let inputs = ok(profile.eligibility(
        &ledger,
        &WorkLedger::default(),
        &AttemptLedger::default(),
        &enrollment(),
        NOW + 1,
    ));
    let waiting = ok(inputs
        .iter()
        .find(|input| input.work_version == ok(second.identity()))
        .ok_or(()));
    assert_eq!(waiting.reason, AdmissionReason::Waiting);
    // Even a request that raced past planning is refused by the owner: plan it
    // against a copy that wrongly believes two slots exist.
    let mut freed = profile.clone();
    freed.owner.capacity = BTreeMap::from([(alias("worker_slots"), 2)]);
    let HostReservationPlan::Request(raced) =
        ok(freed.plan_reservation(&ledger, &second, &b, u64::MAX, NOW))
    else {
        unreachable!("unheld second work plans a request")
    };
    assert_eq!(
        ok(profile.decide(&ledger, &raced, NOW)),
        HostOwnerDecision::OverCapacity
    );
}

#[test]
fn duplicate_reservation_for_the_same_work_is_refused() {
    let profile = profile();
    let issue = sandbox(&profile, 1);
    let mut ledger = admitted(&profile, &[&issue]);
    let attempt = claim("attempt-1");
    ok(ledger.bind_claim(&ok(issue.identity()), attempt.clone()));
    reserve(&profile, &mut ledger, &issue, &attempt, NOW);
    // A new registration generation cannot mint a second grant for held work.
    let mut bumped = profile.clone();
    bumped.owner.generation = 2;
    let fresh = ok(bumped.eligibility(
        &ledger,
        &WorkLedger::default(),
        &AttemptLedger::default(),
        &enrollment(),
        NOW + 1,
    ));
    ok(ledger.select(&fresh, NOW + 1));
    assert_eq!(
        code(bumped.plan_reservation(&ledger, &issue, &attempt, u64::MAX, NOW + 1)),
        "host_owner_duplicate"
    );
    assert_eq!(ledger.reservations.len(), 1);
}

#[test]
fn release_needs_an_observed_terminal_attempt() {
    let profile = profile();
    let issue = sandbox(&profile, 1);
    let mut ledger = admitted(&profile, &[&issue]);
    let attempt = claim("attempt-1");
    ok(ledger.bind_claim(&ok(issue.identity()), attempt.clone()));
    // Nothing reserved: nothing to release.
    assert_eq!(
        code(profile.plan_release(&ledger, &AttemptLedger::default(), &attempt)),
        "host_owner_release_missing"
    );
    reserve(&profile, &mut ledger, &issue, &attempt, NOW);
    // The attempt ledger has no terminal observation of the holder.
    assert_eq!(
        code(profile.plan_release(&ledger, &AttemptLedger::default(), &attempt)),
        "host_owner_release_unobserved"
    );
}

#[test]
fn a_live_claim_on_work_that_is_no_longer_ready_does_not_take_the_slot() {
    let profile = two_active();
    // The claimed work now depends on work that does not exist: not ready.
    let mut blocked = sandbox(&profile, 1);
    blocked.dependencies = vec![id("missing-dependency")];
    let ready = sandbox(&profile, 2);
    let works = work_ledger(&[&blocked, &ready]);
    let mut attempts = AttemptLedger::default();
    let live = ok(attempts.claim(&blocked, NOW, crate::implementation::DEFAULT_LEASE_MS));
    let mut ledger = AdmissionLedger::default();
    ok(ledger.configure(ok(profile.policy())));
    ok(ledger.add_root(ok(profile.root_account())));
    for work in [&blocked, &ready] {
        ok(ledger.enqueue(ok(profile.queue_item(work, NOW)), NOW));
    }
    ok(ledger.bind_claim(
        &ok(blocked.identity()),
        ClaimRef {
            attempt: live.identity.clone(),
            fence: live.fence,
        },
    ));
    let inputs = ok(profile.eligibility(&ledger, &works, &attempts, &enrollment(), NOW + 1));
    let reason = |work: &ImplementationWork| {
        ok(inputs
            .iter()
            .find(|input| input.work_version == ok(work.identity()))
            .map(|input| input.reason)
            .ok_or(()))
    };
    assert_eq!(reason(&ready), AdmissionReason::Eligible);
    assert_eq!(reason(&blocked), AdmissionReason::Waiting);
}
