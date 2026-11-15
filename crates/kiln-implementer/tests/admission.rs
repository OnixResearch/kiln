use kiln_core::fair_admission::*;
use kiln_core::implementation::*;
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::{
    Implementer,
    admission::{
        AdmissionCommand as Cmd, AdmissionOwnerPortV1, AdmissionResult as ResultFact,
        OwnerPortFailure,
    },
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
fn id(s: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(s.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}
fn a(s: &str) -> Alias {
    Alias::new(s).unwrap_or_else(|_| unreachable!())
}
fn p() -> ProviderProfileId {
    ProviderProfileId::new("serving-v1").unwrap_or_else(|_| unreachable!())
}
fn amounts(n: u64) -> Amounts {
    BTreeMap::from([(a("requests"), n)])
}
fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}
fn open(path: &Path) -> Implementer {
    Implementer::open(path, enrollment()).unwrap_or_else(|_| unreachable!())
}
fn work(n: &str) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: None,
        title: n.into(),
        requested_outcome: "implement admitted slice".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a".into(),
            statement: "tested".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: id("verification"),
        budget: WorkBudget {
            attempts: 1,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", n).unwrap_or_else(|_| unreachable!()),
            native_id: n.into(),
            version: id(n),
        }],
    }
}
fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        version: 1,
        accounting_basis: id("basis"),
        max_items: 32,
        max_bytes: 100_000,
        max_wait_ms: 1000,
        max_observation_age_ms: 100,
        max_retry_after_ms: 50,
        max_children: 4,
        max_active: 2,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(a("a"), 1), (a("b"), 1)]),
        group_limits: vec![],
    }
}
fn account(c: &str) -> BudgetAccount {
    BudgetAccount {
        identity: id(c),
        parent: None,
        parent_attempt: None,
        campaign: a(c),
        tenant: a("tenant"),
        deadline_ms: 1000,
        available: amounts(100),
        consumed: Amounts::new(),
        children: 0,
    }
}
fn offer(w: &ImplementationWork) -> EligibilityInput {
    EligibilityInput {
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        reason: AdmissionReason::Eligible,
        provider: p(),
        generation: 1,
        observed_at_ms: 100,
        valid_until_ms: 500,
    }
}
fn item(c: &str, w: &ImplementationWork) -> QueueItem {
    QueueItem {
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        account: id(c),
        tenant: a("tenant"),
        campaign: a(c),
        repository: a("repo"),
        priority: 255,
        bytes: 1,
        enqueued_at_ms: 100,
        deadline_ms: 500,
        eligible_at_ms: 100,
        claim: None,
        phase: AdmissionPhase::PreClaim,
        reason: AdmissionReason::Waiting,
        order: 999,
        required_owners: BTreeMap::from([(p(), ChargeKind::WorkerRequest)]),
        shared_allocations: vec![],
        budget: amounts(30),
    }
}
fn setup(path: &Path, works: &[(&str, ImplementationWork)]) -> Implementer {
    let mut i = open(path);
    i.admission_apply(Cmd::Configure { policy: policy() }, 100)
        .unwrap_or_else(|_| unreachable!());
    for c in ["a", "b"] {
        i.admission_apply(
            Cmd::Root {
                account: account(c),
            },
            100,
        )
        .unwrap_or_else(|_| unreachable!());
        let observation = UsageObservation {
            usage: TenantUsage {
                tenant: a(c),
                share: 1,
                provider: p(),
                interval_start: 0,
                interval_end: 100,
                observed_at: 100,
                resources: BTreeMap::from([(
                    a("requests"),
                    UsageValue {
                        amount: 0,
                        capacity: Some(100),
                    },
                )]),
            },
            generation: 1,
            accounting_basis: id("basis"),
            covered: BTreeMap::new(),
            complete: true,
        };
        i.admission_apply(Cmd::Observe { observation }, 100)
            .unwrap_or_else(|_| unreachable!());
    }
    for (c, w) in works {
        i.admit_with_supersession(w, None, SupersessionRule::AbortActive, 100)
            .unwrap_or_else(|_| unreachable!());
        i.admission_apply(Cmd::Enqueue { item: item(c, w) }, 100)
            .unwrap_or_else(|_| unreachable!());
    }
    i
}
fn claim(i: &mut Implementer, w: &ImplementationWork) -> ImplementationAttempt {
    match i
        .admission_apply(
            Cmd::Claim {
                inputs: vec![offer(w)],
                lease_ms: 200,
            },
            100,
        )
        .unwrap_or_else(|_| unreachable!())
    {
        ResultFact::Claimed { attempt: Some(a) } => *a,
        other => unreachable!("{other:?}"),
    }
}
fn reservation(
    w: &ImplementationWork,
    attempt: &ImplementationAttempt,
    op: &str,
) -> OwnerReservation {
    OwnerReservation {
        version: 1,
        operation: id(op),
        provider: p(),
        generation: 1,
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        claim: ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        },
        account: id("a"),
        kind: ChargeKind::WorkerRequest,
        subject: id(op),
        accounting_basis: id("basis"),
        amounts: amounts(10),
        expires_at_ms: 400,
        state: ReservationState::Tentative,
        receipt: None,
        serving_bounds: Some(ServingBounds {
            scope: id("aggregate-service"),
            max_ongoing: 2,
            max_queued: 8,
        }),
    }
}
fn confirm(i: &mut Implementer, r: &OwnerReservation) {
    i.admission_apply(
        Cmd::Reply {
            reply: OwnerReply {
                version: 1,
                operation: r.operation.clone(),
                provider: p(),
                generation: 1,
                receipt: id("grant"),
                state: ReservationState::Committed,
                retry_at_ms: None,
                serving_bounds: r.serving_bounds.clone(),
            },
        },
        101,
    )
    .unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.admission.durable]
#[test]
fn reopen_preserves_claim_order_held_debit_and_unknown_dispatch_without_refund() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let b = work("b1");
    let mut i = setup(&path, &[("a", w.clone()), ("b", b.clone())]);
    let attempt = claim(&mut i, &w);
    let r = reservation(&w, &attempt, "op");
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Tentative
    );
    confirm(&mut i, &r);
    let before = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    assert_eq!(
        i.admission_snapshot().unwrap_or_else(|_| unreachable!()),
        before
    );
    assert!(
        matches!(i.admission_apply(Cmd::Select{inputs:vec![offer(&w),offer(&b)]},102).unwrap_or_else(|_| unreachable!()),ResultFact::Selected{work_version:Some(id)} if id==b.identity().unwrap_or_else(|_| unreachable!()))
    );
    i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 102)
        .unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    let state = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Unknown
    );
    assert_eq!(
        state
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 103)
            .is_err()
    );
    i.admission_apply(
        Cmd::Cancel {
            work_version: w.identity().unwrap_or_else(|_| unreachable!()),
            expired: false,
        },
        103,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
    i.admission_apply(
        Cmd::Settle {
            operation: r.operation.clone(),
            provider: p(),
            generation: 1,
            receipt: id("termination"),
            consumed: amounts(6),
        },
        104,
    )
    .unwrap_or_else(|_| unreachable!());
    i.admission_apply(
        Cmd::Settle {
            operation: r.operation,
            provider: p(),
            generation: 1,
            receipt: id("termination"),
            consumed: amounts(6),
        },
        104,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(94)
    );
    assert_eq!(
        i.attempt(&attempt.identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .ordinal,
        1
    );
}
#[test]
fn competing_schedulers_get_one_current_claim_and_a_burst_cannot_outrun_confirmed_debit() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let w2 = work("a2");
    let b = work("b1");
    drop(setup(
        &path,
        &[("a", w.clone()), ("a", w2.clone()), ("b", b.clone())],
    ));
    let path = std::sync::Arc::new(path);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            let w = w.clone();
            std::thread::spawn(move || {
                let mut i = open(&path);
                barrier.wait();
                i.admission_apply(
                    Cmd::Claim {
                        inputs: vec![offer(&w)],
                        lease_ms: 200,
                    },
                    100,
                )
                .unwrap_or_else(|_| unreachable!())
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or_else(|_| unreachable!()))
        .collect();
    let mut i = open(&path);
    assert_eq!(
        i.attempts()
            .unwrap_or_else(|_| unreachable!())
            .attempts()
            .count(),
        1
    );
    // Idempotent postclaim waiting may return the same current claim to both callers.
    let attempts: Vec<_> = results
        .into_iter()
        .filter_map(|r| match r {
            ResultFact::Claimed { attempt } => attempt,
            _ => None,
        })
        .collect();
    assert!(!attempts.is_empty());
    assert!(
        attempts
            .iter()
            .all(|a| a.identity == attempts.first().unwrap_or_else(|| unreachable!()).identity)
    );
    let r = reservation(&w, attempts.first().unwrap_or_else(|| unreachable!()), "op");
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    confirm(&mut i, &r);
    assert!(
        matches!(i.admission_apply(Cmd::Claim{inputs:vec![offer(&w2),offer(&b)],lease_ms:200},102).unwrap_or_else(|_| unreachable!()),ResultFact::Claimed{attempt:Some(a)}if a.work_identity==b.identity().unwrap_or_else(|_| unreachable!()))
    );
}
#[test]
fn fresh_generation_authority_and_missing_required_grants_block_stale_dispatch() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let attempt = claim(&mut i, &w);
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 100)
            .is_err()
    );
    let r = reservation(&w, &attempt, "op");
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    confirm(&mut i, &r);
    let mut changed = offer(&w);
    changed.generation = 2;
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[changed], 102)
            .is_err()
    );
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[], 102)
            .is_err()
    );
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence + 1, &[offer(&w)], 102)
            .is_err()
    );
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Committed
    );
    drop(i);
    let mut revoked = enrollment();
    revoked.actors.clear();
    let mut i = Implementer::open(&path, revoked).unwrap_or_else(|_| unreachable!());
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 102)
            .is_err()
    );
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
}
#[test]
fn preclaim_wait_does_not_consume_attempt_but_postclaim_wait_cannot_reset_one() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let mut loading = offer(&w);
    loading.reason = AdmissionReason::Waiting;
    assert!(matches!(
        i.admission_apply(
            Cmd::Claim {
                inputs: vec![loading],
                lease_ms: 20
            },
            100
        )
        .unwrap_or_else(|_| unreachable!()),
        ResultFact::Claimed { attempt: None }
    ));
    assert_eq!(
        i.attempts()
            .unwrap_or_else(|_| unreachable!())
            .attempts()
            .count(),
        0
    );
    let attempt = match i
        .admission_apply(
            Cmd::Claim {
                inputs: vec![offer(&w)],
                lease_ms: 20,
            },
            100,
        )
        .unwrap_or_else(|_| unreachable!())
    {
        ResultFact::Claimed { attempt: Some(a) } => a,
        _ => unreachable!(),
    };
    let r = reservation(&w, &attempt, "op");
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    i.admission_apply(
        Cmd::OwnerFailure {
            operation: r.operation,
            provider: p(),
            generation: 1,
            reason: AdmissionReason::Waiting,
            retry_at_ms: Some(110),
        },
        101,
    )
    .unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    i.recover(121).unwrap_or_else(|_| unreachable!());
    let mut fresh = offer(&w);
    fresh.observed_at_ms = 121;
    assert!(matches!(
        i.admission_apply(
            Cmd::Claim {
                inputs: vec![fresh],
                lease_ms: 20
            },
            121
        )
        .unwrap_or_else(|_| unreachable!()),
        ResultFact::Claimed { attempt: None }
    ));
    assert_eq!(
        i.attempts()
            .unwrap_or_else(|_| unreachable!())
            .attempts()
            .count(),
        1
    );
    assert_eq!(
        i.attempt(&attempt.identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .ordinal,
        1
    );
    assert!(
        i.claim_next(121, 20)
            .unwrap_or_else(|_| unreachable!())
            .is_none()
    );
}
struct Owner {
    reserves: u32,
    reconciles: u32,
    cancels: u32,
}
impl AdmissionOwnerPortV1 for Owner {
    fn reserve(&mut self, r: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        self.reserves += 1;
        Ok(OwnerReply {
            version: 1,
            operation: r.operation.clone(),
            provider: r.provider.clone(),
            generation: r.generation,
            receipt: id("grant"),
            state: ReservationState::Committed,
            retry_at_ms: None,
            serving_bounds: r.serving_bounds.clone(),
        })
    }
    fn reconcile(&mut self, r: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        self.reconciles += 1;
        Ok(OwnerReply {
            version: 1,
            operation: r.operation.clone(),
            provider: r.provider.clone(),
            generation: r.generation,
            receipt: id("grant"),
            state: ReservationState::Committed,
            retry_at_ms: None,
            serving_bounds: r.serving_bounds.clone(),
        })
    }
    fn cancel(&mut self, _: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        self.cancels += 1;
        Err(OwnerPortFailure::CancellationObserved)
    }
}
#[test]
fn owner_port_reconnect_reconciles_one_operation_instead_of_duplicate_reservation() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let attempt = claim(&mut i, &w);
    let r = reservation(&w, &attempt, "op");
    let mut owner = Owner {
        reserves: 0,
        reconciles: 0,
        cancels: 0,
    };
    i.admission_reserve_with_owner(r.clone(), vec![offer(&w)], &mut owner, 100)
        .unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    i.admission_reserve_with_owner(r, vec![offer(&w)], &mut owner, 101)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!((owner.reserves, owner.reconciles), (1, 1));
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
}
#[test]
fn default_unconfigured_transport_path_is_unchanged() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut i = open(&dir.path().join("ledger.db"));
    i.admission_before_transport(&id("not-an-admission-attempt"), 1, &[], 100)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .policy
            .is_none()
    );
}

#[test]
fn child_transfer_reconnect_and_competing_campaign_follow_one_aggregate_quota() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let parent = work("parent");
    let child = work("child");
    let sibling = work("sibling");
    let b = work("b1");
    let mut i = setup(&path, &[("a", parent.clone()), ("b", b.clone())]);
    let parent_attempt = claim(&mut i, &parent);
    let mut quota = account("a");
    quota.identity = id("child-account");
    quota.parent = Some(id("a"));
    quota.parent_attempt = Some(ClaimRef {
        attempt: parent_attempt.identity.clone(),
        fence: parent_attempt.fence,
    });
    quota.available = amounts(60);
    assert!(
        i.admission_apply(
            Cmd::Transfer {
                parent: id("a"),
                child: quota.clone()
            },
            100
        )
        .is_err()
    );
    quota.deadline_ms = parent_attempt.lease_until_ms;
    i.admission_apply(
        Cmd::Transfer {
            parent: id("a"),
            child: quota.clone(),
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .consumed
            .is_empty()
    );
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(40)
    );
    for w in [&child, &sibling] {
        i.admit_with_supersession(w, None, SupersessionRule::AbortActive, 100)
            .unwrap_or_else(|_| unreachable!());
        let mut queued = item("a", w);
        queued.account = id("child-account");
        queued.deadline_ms = quota.deadline_ms;
        i.admission_apply(Cmd::Enqueue { item: queued }, 100)
            .unwrap_or_else(|_| unreachable!());
    }
    let mut parent_wait = offer(&parent);
    parent_wait.reason = AdmissionReason::Waiting;
    i.admission_apply(
        Cmd::Select {
            inputs: vec![parent_wait, offer(&child), offer(&sibling), offer(&b)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    let child_attempt = claim(&mut i, &child);
    let mut r = reservation(&child, &child_attempt, "child-op");
    r.account = id("child-account");
    r.expires_at_ms = 250;
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&child)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    confirm(&mut i, &r);
    drop(i);
    let mut i = open(&path);
    i.admission_apply(
        Cmd::Transfer {
            parent: id("a"),
            child: quota.clone(),
        },
        102,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("child-account"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(50)
    );
    assert!(
        matches!(i.admission_apply(Cmd::Select{inputs:vec![offer(&sibling)]},102).unwrap_or_else(|_| unreachable!()),
        ResultFact::Selected{work_version:Some(id)}if id==b.identity().unwrap_or_else(|_| unreachable!()))
    );
    quota.identity = id("minted-child");
    assert!(
        i.admission_apply(
            Cmd::Transfer {
                parent: id("a"),
                child: quota
            },
            102
        )
        .is_err()
    );
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(40)
    );
}
#[test]
fn racing_owner_requests_preserve_one_intent_and_authoritative_queue_bytes() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let attempt = claim(&mut i, &w);
    let state = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    assert!(
        state
            .items
            .get(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|| unreachable!())
            .bytes
            > 1
    );
    assert_eq!(
        state
            .items
            .get(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|| unreachable!())
            .priority,
        w.priority
    );
    let retained = state
        .items
        .get(&w.identity().unwrap_or_else(|_| unreachable!()))
        .unwrap_or_else(|| unreachable!());
    assert!(
        serde_json::to_vec(retained)
            .unwrap_or_else(|_| unreachable!())
            .len() as u64
            <= retained.bytes
    );
    let r = reservation(&w, &attempt, "op");
    drop(i);
    let path = std::sync::Arc::new(path);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            let r = r.clone();
            let w = w.clone();
            std::thread::spawn(move || {
                let mut i = open(&path);
                barrier.wait();
                i.admission_apply(
                    Cmd::Request {
                        reservation: r,
                        inputs: vec![offer(&w)],
                    },
                    100,
                )
                .unwrap_or_else(|_| unreachable!())
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|t| t.join().unwrap_or_else(|_| unreachable!()))
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, ResultFact::Updated))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, ResultFact::Retained { .. }))
            .count(),
        1
    );
    let mut i = open(&path);
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
}
#[test]
fn exhausted_child_and_incomplete_provider_do_not_block_independent_ready_work() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let b = work("b1");
    let mut i = setup(&path, &[("a", w.clone()), ("b", b.clone())]);
    let mut partial = UsageObservation {
        usage: TenantUsage {
            tenant: a("a"),
            share: 1,
            provider: p(),
            interval_start: 0,
            interval_end: 100,
            observed_at: 100,
            resources: BTreeMap::new(),
        },
        generation: 2,
        accounting_basis: id("basis"),
        covered: BTreeMap::new(),
        complete: false,
    };
    i.admission_apply(
        Cmd::Observe {
            observation: partial.clone(),
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(
        matches!(i.admission_apply(Cmd::Select{inputs:vec![offer(&w),offer(&b)]},100).unwrap_or_else(|_| unreachable!()),ResultFact::Selected{work_version:Some(id)}if id==b.identity().unwrap_or_else(|_| unreachable!()))
    );
    let state = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .explanation(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::Incomplete
    );
    assert_eq!(
        state
            .observations
            .get(&a("a"))
            .unwrap_or_else(|| unreachable!())
            .usage
            .resources
            .get(&a("requests"))
            .unwrap_or_else(|| unreachable!())
            .amount,
        0
    );
    partial.generation = 1;
    assert!(
        i.admission_apply(
            Cmd::Observe {
                observation: partial
            },
            100
        )
        .is_err()
    );
}

#[test]
fn observed_owner_cancellation_keeps_unknown_usage_until_exact_settlement() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let attempt = claim(&mut i, &w);
    let r = reservation(&w, &attempt, "op");
    let mut owner = Owner {
        reserves: 0,
        reconciles: 0,
        cancels: 0,
    };
    i.admission_reserve_with_owner(r.clone(), vec![offer(&w)], &mut owner, 100)
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        i.admission_cancel_with_owner(&r.operation, &mut owner, 101)
            .unwrap_or_else(|_| unreachable!()),
        ResultFact::Owner {
            reason: AdmissionReason::Cancelled
        }
    ));
    drop(i);
    let mut i = open(&path);
    i.admission_cancel_with_owner(&r.operation, &mut owner, 102)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!((owner.cancels, owner.reconciles), (1, 1));
    let state = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .items
            .get(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|| unreachable!())
            .reason,
        AdmissionReason::Cancelled
    );
    assert_eq!(
        state
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Unknown
    );
    assert_eq!(
        state
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
    assert!(
        i.admission_apply(
            Cmd::Reply {
                reply: OwnerReply {
                    version: 1,
                    operation: r.operation.clone(),
                    provider: p(),
                    generation: 1,
                    receipt: id("not-a-settlement"),
                    state: ReservationState::Rejected,
                    retry_at_ms: None,
                    serving_bounds: r.serving_bounds.clone()
                }
            },
            103
        )
        .is_err()
    );
    i.admission_apply(
        Cmd::Settle {
            operation: r.operation,
            provider: p(),
            generation: 1,
            receipt: id("cancelled-usage"),
            consumed: amounts(4),
        },
        103,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(96)
    );
}

#[test]
fn native_attempt_cancel_atomically_stops_admission_without_freeing_possible_effect() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("a1");
    let mut i = setup(&path, &[("a", w.clone())]);
    let attempt = claim(&mut i, &w);
    let r = reservation(&w, &attempt, "op");
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        100,
    )
    .unwrap_or_else(|_| unreachable!());
    confirm(&mut i, &r);
    i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 102)
        .unwrap_or_else(|_| unreachable!());
    i.cancel(&attempt.identity, attempt.fence, 103, "operator cancelled")
        .unwrap_or_else(|_| unreachable!());
    drop(i);
    let mut i = open(&path);
    let state = i.admission_snapshot().unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .items
            .get(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|| unreachable!())
            .phase,
        AdmissionPhase::Terminal
    );
    assert_eq!(
        state
            .items
            .get(&w.identity().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|| unreachable!())
            .reason,
        AdmissionReason::CancellationRequested
    );
    assert_eq!(
        state
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Unknown
    );
    assert_eq!(
        state
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], 104)
            .is_err()
    );
}

#[test]
fn configured_legacy_claim_without_queue_item_is_still_revocable() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let w = work("legacy");
    let mut i = setup(&path, &[]);
    i.admit_with_supersession(&w, None, SupersessionRule::AbortActive, 100)
        .unwrap_or_else(|_| unreachable!());
    let attempt = i
        .claim_next(100, 200)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    i.cancel(
        &attempt.identity,
        attempt.fence,
        101,
        "revoke unqueued legacy claim",
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        i.attempt(&attempt.identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .phase,
        AttemptPhase::Cancelled
    );
    assert!(
        i.admission_snapshot()
            .unwrap_or_else(|_| unreachable!())
            .items
            .is_empty()
    );
}

#[test]
fn campaign_claim_scopes_ranking_before_claiming_unrelated_work() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let first = work("campaign-a");
    let second = work("campaign-b");
    let mut implementer = setup(&path, &[("a", first.clone()), ("b", second.clone())]);
    let inputs = vec![offer(&first), offer(&second)];
    let global_winner = match implementer
        .admission_apply(Cmd::Select { inputs }, 100)
        .unwrap_or_else(|_| unreachable!())
    {
        ResultFact::Selected {
            work_version: Some(identity),
        } => identity,
        other => unreachable!("unexpected selection {other:?}"),
    };
    let first_id = first.identity().unwrap_or_else(|_| unreachable!());
    let second_id = second.identity().unwrap_or_else(|_| unreachable!());
    let global_parent = if global_winner == first_id {
        id("a")
    } else {
        id("b")
    };
    let (scoped_work, parent) = if global_winner == first_id {
        (second_id, id("b"))
    } else {
        (first_id, id("a"))
    };
    let attempt = match implementer
        .claim_campaign_work(&scoped_work, &parent, 100, 200)
        .unwrap_or_else(|_| unreachable!())
    {
        kiln_implementer::campaign_consumer::CampaignClaimOutcome::Claimed(attempt) => attempt,
        other => unreachable!("unexpected scoped claim {other:?}"),
    };
    assert_eq!(
        implementer
            .claim_campaign_work(&scoped_work, &parent, 101, 200)
            .unwrap_or_else(|_| unreachable!()),
        kiln_implementer::campaign_consumer::CampaignClaimOutcome::Claimed(attempt.clone())
    );
    assert!(
        implementer
            .claim_campaign_work(&global_winner, &global_parent, 101, 200)
            .is_err()
    );
    let after_claim = implementer
        .admission_snapshot()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        after_claim
            .items
            .get(&scoped_work)
            .unwrap_or_else(|| unreachable!())
            .claim
            .is_some()
    );
    assert!(
        after_claim
            .items
            .get(&global_winner)
            .unwrap_or_else(|| unreachable!())
            .claim
            .is_none()
    );
    assert_eq!(
        after_claim
            .items
            .get(&global_winner)
            .unwrap_or_else(|| unreachable!())
            .reason,
        AdmissionReason::Eligible
    );
}

#[test]
fn campaign_claim_refuses_a_parent_outside_the_canonical_account_lineage() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let selected = work("campaign-a");
    let mut implementer = setup(&path, &[("a", selected.clone())]);
    let identity = selected.identity().unwrap_or_else(|_| unreachable!());
    assert!(
        implementer
            .claim_campaign_work(&identity, &id("b"), 100, 200)
            .is_err()
    );
    assert!(
        implementer
            .claim_campaign_work(&identity, &id("missing"), 100, 200)
            .is_err()
    );
    let ledger = implementer
        .admission_snapshot()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        ledger
            .items
            .get(&identity)
            .unwrap_or_else(|| unreachable!())
            .claim
            .is_none()
    );
}

#[test]
fn campaign_claim_reattaches_reserved_current_claim_and_rechecks_parent_lineage() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let selected = work("campaign-reserved");
    let identity = selected.identity().unwrap_or_else(|_| unreachable!());
    let mut implementer = setup(&path, &[("a", selected.clone())]);
    let original = claim(&mut implementer, &selected);
    let request = reservation(&selected, &original, "reserved-claim-op");
    implementer
        .admission_apply(
            Cmd::Request {
                reservation: request.clone(),
                inputs: vec![offer(&selected)],
            },
            100,
        )
        .unwrap_or_else(|_| unreachable!());
    confirm(&mut implementer, &request);
    let before = implementer
        .admission_snapshot()
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        before
            .items
            .get(&identity)
            .unwrap_or_else(|| unreachable!())
            .phase,
        AdmissionPhase::Reserved
    );
    assert_eq!(
        before
            .reservations
            .get(&request.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Committed
    );
    let attempts_before =
        kiln_implementer::campaign_consumer::CanonicalCampaignLedgers::read(&path)
            .unwrap_or_else(|_| unreachable!())
            .attempts
            .attempts()
            .count();

    assert_eq!(
        implementer
            .claim_campaign_work(&identity, &id("a"), 102, 200)
            .unwrap_or_else(|_| unreachable!()),
        kiln_implementer::campaign_consumer::CampaignClaimOutcome::Claimed(Box::new(
            original.clone()
        ))
    );
    assert!(
        implementer
            .claim_campaign_work(&identity, &id("b"), 102, 200)
            .is_err()
    );

    let after = implementer
        .admission_snapshot()
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        after
            .items
            .get(&identity)
            .unwrap_or_else(|| unreachable!())
            .claim,
        before
            .items
            .get(&identity)
            .unwrap_or_else(|| unreachable!())
            .claim
    );
    assert_eq!(
        after
            .items
            .get(&identity)
            .unwrap_or_else(|| unreachable!())
            .phase,
        AdmissionPhase::Reserved
    );
    assert_eq!(
        after
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        before
            .accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available
    );
    let attempts_after = kiln_implementer::campaign_consumer::CanonicalCampaignLedgers::read(&path)
        .unwrap_or_else(|_| unreachable!())
        .attempts
        .attempts()
        .count();
    assert_eq!(attempts_after, attempts_before);
}

#[test]
fn campaign_claim_requires_explicit_policy_instead_of_legacy_fallback() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("ledger.db");
    let selected = work("campaign-a");
    let mut implementer = open(&path);
    implementer
        .admit_with_supersession(&selected, None, SupersessionRule::AbortActive, 100)
        .unwrap_or_else(|_| unreachable!());
    let identity = selected.identity().unwrap_or_else(|_| unreachable!());
    assert!(
        implementer
            .claim_campaign_work(&identity, &id("a"), 100, 200)
            .is_err()
    );
}
