// r[verify kiln.admission.bounded]
use super::*;
use crate::operations::UsageValue;
use alloc::vec;

mod serving_lineage;

fn id(s: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(s.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}
fn alias(s: &str) -> Alias {
    Alias::new(s).unwrap_or_else(|_| unreachable!())
}
fn provider() -> ProviderProfileId {
    ProviderProfileId::new("serving-v1").unwrap_or_else(|_| unreachable!())
}
fn amounts(n: u64) -> Amounts {
    BTreeMap::from([(alias("requests"), n)])
}
fn policy(strategy: FairnessStrategy) -> AdmissionPolicy {
    AdmissionPolicy {
        version: 1,
        accounting_basis: id("window-capacity-v1"),
        max_items: 16,
        max_bytes: 100_000,
        max_wait_ms: 100,
        max_observation_age_ms: 20,
        max_retry_after_ms: 10,
        max_children: 2,
        max_active: 2,
        strategy,
        shares: BTreeMap::from([(alias("a"), 1), (alias("b"), 1)]),
        group_limits: vec![],
        serving: None,
    }
}
fn root(c: &str) -> BudgetAccount {
    BudgetAccount {
        identity: id(c),
        parent: None,
        parent_attempt: None,
        campaign: alias(c),
        tenant: alias("tenant"),
        deadline_ms: 200,
        available: amounts(100),
        consumed: Amounts::new(),
        children: 0,
    }
}
fn item(c: &str, w: &str) -> QueueItem {
    QueueItem {
        work_version: id(w),
        account: id(c),
        tenant: alias("tenant"),
        campaign: alias(c),
        repository: alias("repo"),
        priority: 0,
        bytes: 10,
        enqueued_at_ms: 100,
        deadline_ms: 190,
        eligible_at_ms: 100,
        claim: None,
        phase: AdmissionPhase::PreClaim,
        reason: AdmissionReason::Waiting,
        order: 0,
        required_owners: BTreeMap::from([(provider(), ChargeKind::WorkerRequest)]),
        shared_allocations: vec![],
        budget: amounts(30),
    }
}
fn observation(c: &str, n: u64) -> UsageObservation {
    UsageObservation {
        usage: TenantUsage {
            tenant: alias(c),
            share: 1,
            provider: provider(),
            interval_start: 0,
            interval_end: 100,
            observed_at: 100,
            resources: BTreeMap::from([(
                alias("requests"),
                UsageValue {
                    amount: n,
                    capacity: Some(100),
                },
            )]),
        },
        generation: 1,
        accounting_basis: id("window-capacity-v1"),
        covered: BTreeMap::new(),
        complete: true,
    }
}
fn input(w: &str) -> EligibilityInput {
    EligibilityInput {
        work_version: id(w),
        reason: AdmissionReason::Eligible,
        provider: provider(),
        generation: 1,
        observed_at_ms: 100,
        valid_until_ms: 150,
    }
}
fn ledger(strategy: FairnessStrategy) -> AdmissionLedger {
    let mut ledger = AdmissionLedger::default();
    ledger
        .configure(policy(strategy))
        .unwrap_or_else(|_| unreachable!());
    for c in ["a", "b"] {
        ledger.add_root(root(c)).unwrap_or_else(|_| unreachable!());
        ledger
            .observe(observation(c, 0))
            .unwrap_or_else(|_| unreachable!());
    }
    ledger
}
fn weighted() -> FairnessStrategy {
    FairnessStrategy::WeightedScalar {
        weights: amounts(1),
    }
}
fn request(
    ledger: &mut AdmissionLedger,
    c: &str,
    w: &str,
    op: &str,
    amount: u64,
) -> OwnerReservation {
    let claim = ClaimRef {
        attempt: id(w),
        fence: 1,
    };
    ledger
        .bind_claim(&id(w), claim.clone())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        ledger
            .select(&[input(w)], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id(w))
    );
    let r = OwnerReservation {
        version: 1,
        operation: id(op),
        provider: provider(),
        generation: 1,
        work_version: id(w),
        claim,
        account: id(c),
        kind: ChargeKind::WorkerRequest,
        subject: id(w),
        accounting_basis: id("window-capacity-v1"),
        amounts: amounts(amount),
        expires_at_ms: 180,
        state: ReservationState::Tentative,
        receipt: None,
        serving_bounds: Some(ServingBounds {
            scope: id("aggregate-service"),
            max_ongoing: 2,
            max_queued: 8,
        }),
    };
    ledger
        .request(r.clone(), 100)
        .unwrap_or_else(|_| unreachable!());
    r
}
fn grant(ledger: &mut AdmissionLedger, r: &OwnerReservation) -> OwnerReply {
    let reply = OwnerReply {
        version: 1,
        operation: r.operation.clone(),
        provider: r.provider.clone(),
        generation: r.generation,
        receipt: id(r.operation.as_str()),
        state: ReservationState::Committed,
        retry_at_ms: None,
        serving_bounds: r.serving_bounds.clone(),
    };
    ledger
        .owner_reply(&reply, 100)
        .unwrap_or_else(|_| unreachable!());
    reply
}

#[test]
fn both_existing_strategies_charge_immediately_before_child_burst() {
    for strategy in [weighted(), FairnessStrategy::DominantShare] {
        let mut l = ledger(strategy);
        for (c, w) in [("a", "a1"), ("a", "a2"), ("a", "a3"), ("b", "b1")] {
            l.enqueue(item(c, w), 100)
                .unwrap_or_else(|_| unreachable!());
        }
        let offers = [input("a1"), input("a2"), input("a3"), input("b1")];
        assert_eq!(
            l.select(&offers, 100).unwrap_or_else(|_| unreachable!()),
            Some(id("a1"))
        );
        let r = request(&mut l, "a", "a1", "op", 10);
        let reply = grant(&mut l, &r);
        assert_eq!(
            l.select(&offers, 100).unwrap_or_else(|_| unreachable!()),
            Some(id("b1"))
        );
        l.owner_reply(&reply, 100)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            l.composed_usage(&alias("a"))
                .unwrap_or_else(|_| unreachable!())
                .resources
                .get(&alias("requests"))
                .unwrap_or_else(|| unreachable!())
                .amount,
            10
        );
        assert_eq!(
            l.accounts
                .get(&id("a"))
                .unwrap_or_else(|| unreachable!())
                .available,
            amounts(90)
        );
    }
}
#[test]
fn shares_normalization_and_stable_ties_are_real_selection_inputs() {
    for strategy in [weighted(), FairnessStrategy::DominantShare] {
        let mut l = ledger(strategy);
        l.policy
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .shares
            .insert(alias("b"), 2);
        let mut b = observation("b", 20);
        b.usage.share = 2;
        b.generation = 2;
        l.observe(b).unwrap_or_else(|_| unreachable!());
        let mut a = observation("a", 10);
        a.generation = 2;
        l.observe(a).unwrap_or_else(|_| unreachable!());
        l.enqueue(item("b", "b1"), 100)
            .unwrap_or_else(|_| unreachable!());
        l.enqueue(item("a", "a1"), 100)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            l.select(&[input("b1"), input("a1")], 100)
                .unwrap_or_else(|_| unreachable!()),
            Some(id("a1"))
        );
        let mut a = observation("a", 11);
        a.generation = 3;
        l.observe(a).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            l.select(&[input("b1"), input("a1")], 100)
                .unwrap_or_else(|_| unreachable!()),
            Some(id("b1"))
        );
    }
}
#[test]
fn missing_capacity_and_stale_campaign_do_not_block_independent_compatible_work() {
    let mut l = ledger(FairnessStrategy::DominantShare);
    l.observations
        .get_mut(&alias("a"))
        .unwrap_or_else(|| unreachable!())
        .usage
        .resources
        .get_mut(&alias("requests"))
        .unwrap_or_else(|| unreachable!())
        .capacity = None;
    for (c, w) in [("a", "a1"), ("b", "b1")] {
        l.enqueue(item(c, w), 100)
            .unwrap_or_else(|_| unreachable!());
    }
    assert_eq!(
        l.select(&[input("a1"), input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    assert_eq!(
        l.explanation(&id("a1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::Incomplete
    );
    l.observations
        .get_mut(&alias("a"))
        .unwrap_or_else(|| unreachable!())
        .usage
        .observed_at = 1;
    assert_eq!(
        l.select(&[input("a1"), input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    let mut bad = input("a1");
    bad.reason = AdmissionReason::Incompatible;
    assert_eq!(
        l.select(&[bad, input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    assert_eq!(
        l.explanation(&id("a1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::Incompatible
    );
}
#[test]
fn queue_items_bytes_grouping_age_and_fanout_have_explicit_bounds() {
    let mut l = ledger(weighted());
    l.policy
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .max_items = 2;
    l.policy
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .max_active = 2;
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    l.enqueue(item("b", "b1"), 100)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.enqueue(item("a", "a2"), 100)
            .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::QueueFull
    );
    assert_eq!(
        l.select(&[input("a1"), input("b1")], 201)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
    assert_eq!(
        l.explanation(&id("a1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::Expired
    );
    let mut l = ledger(weighted());
    l.policy
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .max_bytes = 9;
    assert_eq!(
        l.enqueue(item("a", "a1"), 100)
            .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::QueueFull
    );
    let mut l = ledger(weighted());
    l.policy
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .group_limits = vec![QueueGroupLimit {
        scope: QueueScope::Tenant,
        group: alias("tenant"),
        max_items: 1,
        max_bytes: 100,
        max_active: 1,
    }];
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.enqueue(item("b", "b1"), 100)
            .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::QueueFull
    );
}
#[test]
fn parent_transfer_is_not_consumption_and_new_children_cannot_mint_quota() {
    let mut l = ledger(weighted());
    let mut child = root("a");
    child.identity = id("child");
    child.parent = Some(id("a"));
    child.parent_attempt = Some(ClaimRef {
        attempt: id("parent-attempt"),
        fence: 1,
    });
    child.available = amounts(60);
    l.transfer(&id("a"), child.clone())
        .unwrap_or_else(|_| unreachable!());
    l.transfer(&id("a"), child.clone())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(40)
    );
    assert!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .consumed
            .is_empty()
    );
    child.identity = id("another");
    assert!(l.transfer(&id("a"), child.clone()).is_err());
    child.available = amounts(40);
    l.transfer(&id("a"), child.clone())
        .unwrap_or_else(|_| unreachable!());
    child.identity = id("third");
    child.available = amounts(1);
    assert!(l.transfer(&id("a"), child).is_err());
    let mut fresh = root("a");
    fresh.identity = id("new-worker-root");
    assert!(l.add_root(fresh).is_err());
}
#[test]
fn confirmed_receipt_and_measured_usage_compose_without_double_charge() {
    let mut l = ledger(weighted());
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "op", 10);
    grant(&mut l, &r);
    l.settle(&r.operation, &r.provider, 1, id("settled"), amounts(7))
        .unwrap_or_else(|_| unreachable!());
    l.settle(&r.operation, &r.provider, 1, id("settled"), amounts(7))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(93)
    );
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .consumed,
        amounts(7)
    );
    assert_eq!(
        l.composed_usage(&alias("a"))
            .unwrap_or_else(|_| unreachable!())
            .resources
            .get(&alias("requests"))
            .unwrap_or_else(|| unreachable!())
            .amount,
        7
    );
    let mut o = observation("a", 7);
    o.generation = 2;
    o.covered.insert(r.operation.clone(), id("settled"));
    l.observe(o.clone()).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.composed_usage(&alias("a"))
            .unwrap_or_else(|_| unreachable!())
            .resources
            .get(&alias("requests"))
            .unwrap_or_else(|| unreachable!())
            .amount,
        7
    );
    o.generation = 3;
    o.usage
        .resources
        .get_mut(&alias("requests"))
        .unwrap_or_else(|| unreachable!())
        .amount = 0;
    assert!(l.observe(o).is_err());
    assert!(
        l.settle(&r.operation, &r.provider, 1, id("conflicting"), amounts(7))
            .is_err()
    );
}
#[test]
fn arithmetic_overflow_capacity_mismatch_and_incompatible_dimensions_fail_closed() {
    let mut l = ledger(weighted());
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "op", 1);
    grant(&mut l, &r);
    l.observations
        .get_mut(&alias("a"))
        .unwrap_or_else(|| unreachable!())
        .usage
        .resources
        .get_mut(&alias("requests"))
        .unwrap_or_else(|| unreachable!())
        .amount = u64::MAX;
    assert!(l.composed_usage(&alias("a")).is_err());
    let mut o = observation("b", 0);
    o.generation = 2;
    o.usage
        .resources
        .get_mut(&alias("requests"))
        .unwrap_or_else(|| unreachable!())
        .capacity = Some(99);
    assert!(l.observe(o).is_err());
    l.reservations
        .get_mut(&r.operation)
        .unwrap_or_else(|| unreachable!())
        .accounting_basis = id("incompatible-gpu-pool");
    assert!(l.composed_usage(&alias("a")).is_err());
    let weights = BTreeMap::from([(alias("requests"), u64::MAX)]);
    let mut l = ledger(FairnessStrategy::WeightedScalar { weights });
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    l.observations
        .get_mut(&alias("a"))
        .unwrap_or_else(|| unreachable!())
        .usage
        .resources
        .insert(
            alias("unsupported"),
            UsageValue {
                amount: 1,
                capacity: Some(1),
            },
        );
    assert_eq!(
        l.select(&[input("a1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
}
#[test]
fn queued_cancel_does_not_acquire_slot_but_possible_acceptance_retains_it() {
    let mut l = ledger(weighted());
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.cancel(&id("a1"), false)
            .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::Cancelled
    );
    assert!(l.reservations.is_empty());
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(100)
    );
    assert_eq!(
        l.select(&[input("a1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
    l.enqueue(item("a", "a2"), 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a2", "op", 10);
    assert_eq!(
        l.cancel(&id("a2"), false)
            .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::CancellationRequested
    );
    grant(&mut l, &r);
    assert_eq!(
        l.reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Unknown
    );
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
    assert!(
        l.dispatch_intent(
            &r.operation,
            &l.policy_ref().unwrap_or_else(|_| unreachable!()),
            &r.claim,
            1,
            100
        )
        .is_err()
    );
}
#[test]
fn postgrant_generation_race_expiry_policy_and_claim_are_checked() {
    let mut l = ledger(weighted());
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "op", 10);
    grant(&mut l, &r);
    let policy = l.policy_ref().unwrap_or_else(|_| unreachable!());
    assert!(
        l.dispatch_intent(&r.operation, &policy, &r.claim, 2, 100)
            .is_err()
    );
    assert!(
        l.dispatch_intent(&r.operation, &id("different-policy"), &r.claim, 1, 100)
            .is_err()
    );
    assert!(
        l.dispatch_intent(
            &r.operation,
            &policy,
            &ClaimRef {
                attempt: id("other"),
                fence: 2
            },
            1,
            100
        )
        .is_err()
    );
    assert!(
        l.dispatch_intent(&r.operation, &policy, &r.claim, 1, 180)
            .is_err()
    );
    l.dispatch_intent(&r.operation, &policy, &r.claim, 1, 101)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Unknown
    );
    assert!(
        l.dispatch_intent(&r.operation, &policy, &r.claim, 1, 102)
            .is_err()
    );
    assert_eq!(
        l.owner_outcome(
            &r.operation,
            &r.provider,
            1,
            AdmissionReason::ProviderFailure,
            None,
            102
        )
        .unwrap_or_else(|_| unreachable!()),
        AdmissionReason::Unknown
    );
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available,
        amounts(90)
    );
}
#[test]
fn before_effect_backpressure_failure_and_unknown_remain_distinct() {
    for reason in [
        AdmissionReason::Waiting,
        AdmissionReason::ProviderRejected,
        AdmissionReason::ProviderFailure,
        AdmissionReason::Expired,
        AdmissionReason::Unknown,
    ] {
        let mut l = ledger(weighted());
        l.enqueue(item("a", "a1"), 100)
            .unwrap_or_else(|_| unreachable!());
        let r = request(&mut l, "a", "a1", "op", 10);
        assert_eq!(
            l.owner_outcome(&r.operation, &r.provider, 1, reason, Some(105), 100)
                .unwrap_or_else(|_| unreachable!()),
            reason
        );
        assert_eq!(
            l.accounts
                .get(&id("a"))
                .unwrap_or_else(|| unreachable!())
                .available,
            amounts(if reason == AdmissionReason::Unknown {
                90
            } else {
                100
            })
        );
        assert!(
            l.owner_outcome(&r.operation, &r.provider, 1, reason, Some(111), 100)
                .is_err()
        );
        assert_eq!(
            l.items
                .get(&id("a1"))
                .unwrap_or_else(|| unreachable!())
                .claim
                .as_ref(),
            Some(&r.claim)
        );
    }
}
#[test]
fn shared_model_physical_charge_once_and_per_worker_requests_separate() {
    let mut l = AdmissionLedger::default();
    l.configure(policy(FairnessStrategy::DominantShare))
        .unwrap_or_else(|_| unreachable!());
    for c in ["a", "b"] {
        let mut account = root(c);
        account.available.insert(alias("gpu"), 1);
        l.add_root(account).unwrap_or_else(|_| unreachable!());
        let mut o = observation(c, 0);
        o.usage.resources.insert(
            alias("gpu"),
            UsageValue {
                amount: 0,
                capacity: Some(1),
            },
        );
        l.observe(o).unwrap_or_else(|_| unreachable!());
    }
    let physical = ProviderProfileId::new("compute-v1").unwrap_or_else(|_| unreachable!());
    let mut first = item("a", "a1");
    first
        .required_owners
        .insert(physical.clone(), ChargeKind::PhysicalDeployment);
    first.budget.insert(alias("gpu"), 1);
    l.enqueue(first, 100).unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "request1", 2);
    grant(&mut l, &r);
    assert_eq!(
        l.select(&[input("a1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a1"))
    );
    let mut gpu = r.clone();
    gpu.operation = id("gpu");
    gpu.provider = physical.clone();
    gpu.kind = ChargeKind::PhysicalDeployment;
    gpu.subject = id("shared-model-deployment");
    gpu.amounts = BTreeMap::from([(alias("gpu"), 1)]);
    gpu.serving_bounds = None;
    l.request(gpu.clone(), 100)
        .unwrap_or_else(|_| unreachable!());
    grant(&mut l, &gpu);
    let mut worker2 = item("a", "a2");
    worker2.shared_allocations.push(gpu.operation.clone());
    l.enqueue(worker2, 100).unwrap_or_else(|_| unreachable!());
    let r2 = request(&mut l, "a", "a2", "request2", 3);
    grant(&mut l, &r2);
    let usage = l
        .composed_usage(&alias("a"))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        usage
            .resources
            .get(&alias("requests"))
            .unwrap_or_else(|| unreachable!())
            .amount,
        5
    );
    assert_eq!(
        usage
            .resources
            .get(&alias("gpu"))
            .unwrap_or_else(|| unreachable!())
            .amount,
        1
    );
    assert_eq!(
        l.accounts
            .get(&id("a"))
            .unwrap_or_else(|| unreachable!())
            .available
            .get(&alias("gpu")),
        Some(&0)
    );
    assert_eq!(
        l.reservations
            .values()
            .filter(|r| r.kind == ChargeKind::PhysicalDeployment)
            .count(),
        1
    );
}

#[test]
fn aggregate_serving_scope_is_required_and_receipt_cannot_change_it() {
    let mut l = ledger(weighted());
    l.enqueue(item("a", "a1"), 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "op", 10);
    let mut missing = r.clone();
    missing.operation = id("unbounded-worker-queue");
    missing.serving_bounds = None;
    assert!(l.request(missing, 100).is_err());
    let mut reply = OwnerReply {
        version: 1,
        operation: r.operation.clone(),
        provider: r.provider.clone(),
        generation: 1,
        receipt: id("grant"),
        state: ReservationState::Committed,
        retry_at_ms: None,
        serving_bounds: r.serving_bounds.clone(),
    };
    reply
        .serving_bounds
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .scope = id("different-caller-local-scope");
    assert!(l.owner_reply(&reply, 100).is_err());
    assert_eq!(
        l.reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!())
            .state,
        ReservationState::Tentative
    );
}
#[test]
fn exhausted_account_and_aggregate_group_concurrency_do_not_head_of_line_block() {
    let mut l = ledger(weighted());
    for (c, w) in [("a", "a1"), ("b", "b1")] {
        l.enqueue(item(c, w), 100)
            .unwrap_or_else(|_| unreachable!());
    }
    l.accounts
        .get_mut(&id("a"))
        .unwrap_or_else(|| unreachable!())
        .available = amounts(1);
    assert_eq!(
        l.select(&[input("a1"), input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    assert_eq!(
        l.explanation(&id("a1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::BudgetExhausted
    );
    let mut l = ledger(weighted());
    l.policy
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .group_limits = vec![QueueGroupLimit {
        scope: QueueScope::Tenant,
        group: alias("tenant"),
        max_items: 16,
        max_bytes: 100000,
        max_active: 1,
    }];
    for (c, w) in [("a", "a1"), ("b", "b1")] {
        l.enqueue(item(c, w), 100)
            .unwrap_or_else(|_| unreachable!());
    }
    let r = request(&mut l, "a", "a1", "op", 10);
    grant(&mut l, &r);
    assert_eq!(
        l.select(&[input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
    assert_eq!(
        l.explanation(&id("b1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::ConcurrencyFull
    );
}
#[test]
fn cancellation_and_expiry_never_revive_when_late_grants_arrive() {
    for expired in [false, true] {
        let mut l = ledger(weighted());
        l.enqueue(item("a", "a1"), 100)
            .unwrap_or_else(|_| unreachable!());
        let r = request(&mut l, "a", "a1", "op", 10);
        l.cancel(&id("a1"), expired)
            .unwrap_or_else(|_| unreachable!());
        grant(&mut l, &r);
        assert_eq!(
            l.select(&[input("a1")], 101)
                .unwrap_or_else(|_| unreachable!()),
            None
        );
        assert_eq!(
            l.items
                .get(&id("a1"))
                .unwrap_or_else(|| unreachable!())
                .phase,
            AdmissionPhase::Terminal
        );
        assert_eq!(
            l.reservations
                .get(&r.operation)
                .unwrap_or_else(|| unreachable!())
                .state,
            ReservationState::Unknown
        );
        l.execution_observed(&id("a1"), &r.claim)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            l.items
                .get(&id("a1"))
                .unwrap_or_else(|| unreachable!())
                .phase,
            AdmissionPhase::Terminal
        );
        assert_eq!(
            l.accounts
                .get(&id("a"))
                .unwrap_or_else(|| unreachable!())
                .available,
            amounts(90)
        );
    }
}

#[test]
fn weighted_u128_overflow_is_a_blocker_not_wrapped_priority() {
    let weights = BTreeMap::from([
        (alias("requests"), u64::MAX),
        (alias("cpu"), u64::MAX),
        (alias("memory"), u64::MAX),
    ]);
    let mut l = AdmissionLedger::default();
    l.configure(policy(FairnessStrategy::WeightedScalar { weights }))
        .unwrap_or_else(|_| unreachable!());
    for c in ["a", "b"] {
        l.add_root(root(c)).unwrap_or_else(|_| unreachable!());
        let mut o = observation(c, if c == "a" { u64::MAX } else { 0 });
        for d in ["cpu", "memory"] {
            o.usage.resources.insert(
                alias(d),
                UsageValue {
                    amount: if c == "a" { u64::MAX } else { 0 },
                    capacity: Some(100),
                },
            );
        }
        l.observe(o).unwrap_or_else(|_| unreachable!());
        l.enqueue(item(c, if c == "a" { "a1" } else { "b1" }), 100)
            .unwrap_or_else(|_| unreachable!());
    }
    assert_eq!(
        l.select(&[input("a1"), input("b1")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    assert_eq!(
        l.explanation(&id("a1"), 100)
            .unwrap_or_else(|_| unreachable!())
            .reason,
        AdmissionReason::Incomplete
    );
}
#[test]
fn compatible_priority_then_durable_enqueue_order_are_stable_within_campaign() {
    let mut l = ledger(weighted());
    let first = item("a", "a1");
    let mut urgent = item("a", "a2");
    urgent.priority = 10;
    l.enqueue(first, 100).unwrap_or_else(|_| unreachable!());
    l.enqueue(urgent, 100).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        l.select(&[input("a1"), input("a2")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a2"))
    );
    let mut incompatible = input("a2");
    incompatible.reason = AdmissionReason::Incompatible;
    assert_eq!(
        l.select(&[input("a1"), incompatible], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a1"))
    );
    l.items
        .get_mut(&id("a2"))
        .unwrap_or_else(|| unreachable!())
        .priority = 0;
    assert_eq!(
        l.select(&[input("a1"), input("a2")], 100)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a1"))
    );
}

#[test]
fn caller_cannot_hide_a_fresh_competitor_by_omitting_its_offer() {
    let mut l = ledger(weighted());
    for (c, w) in [("a", "a1"), ("a", "a2"), ("b", "b1")] {
        l.enqueue(item(c, w), 100)
            .unwrap_or_else(|_| unreachable!());
    }
    l.select(&[input("a1"), input("a2"), input("b1")], 100)
        .unwrap_or_else(|_| unreachable!());
    let r = request(&mut l, "a", "a1", "op", 10);
    grant(&mut l, &r);
    assert_eq!(
        l.select(&[input("a2")], 101)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("b1"))
    );
    let mut withdrawn = input("b1");
    withdrawn.reason = AdmissionReason::Waiting;
    withdrawn.observed_at_ms = 101;
    assert_eq!(
        l.select(&[withdrawn], 101)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a2"))
    );
    // A stale caller cannot revive an explicitly withdrawn offer either.
    assert_eq!(
        l.select(&[input("b1")], 102)
            .unwrap_or_else(|_| unreachable!()),
        Some(id("a2"))
    );
}
