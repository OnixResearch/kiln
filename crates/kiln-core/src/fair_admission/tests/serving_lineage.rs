// r[verify kiln.admission.serving]
// r[verify kiln.admission.lineage]
use super::*;

const POLICY_EXPORT: &str =
    include_str!("../../../../../fixtures/fair-admission/serving-policy.json");
const LINEAGE_EXPORT: &str =
    include_str!("../../../../../fixtures/fair-admission/recursive-lineage.json");

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
fn other_provider() -> ProviderProfileId {
    ok(ProviderProfileId::new("serving-b"))
}
fn bounds(scope: &str) -> ServingBounds {
    ServingBounds {
        scope: id(scope),
        max_ongoing: 2,
        max_queued: 8,
    }
}
fn snapshot(sequence: u64, slots: Option<u32>, queued: Option<u32>) -> ServingObservation {
    ServingObservation {
        version: 1,
        provider: provider(),
        scope: id("aggregate-service"),
        source_generation: 1,
        sequence,
        deployment_generation: 1,
        readiness: ServingReadiness::Ready,
        origin: ServingOrigin::ServingOwner,
        observed_at_ms: 100,
        valid_until_ms: 150,
        bounds: bounds("aggregate-service"),
        remaining_slots: slots,
        queued,
    }
}
fn serving_ledger(rule: ServingCapacityRule) -> AdmissionLedger {
    let mut p = policy(FairnessStrategy::DominantShare);
    p.serving = Some(ServingAdmissionPolicy {
        max_snapshot_age_ms: 10,
        capacity_rule: rule,
    });
    let mut l = AdmissionLedger::default();
    ok(l.configure(p));
    for c in ["a", "b"] {
        ok(l.add_root(root(c)));
        ok(l.observe(observation(c, 0)));
    }
    l
}
fn reason(l: &AdmissionLedger, w: &str) -> AdmissionReason {
    ok(l.explanation(&id(w), 100)).reason
}
fn claim_ref(w: &str) -> ClaimRef {
    ClaimRef {
        attempt: id(w),
        fence: 1,
    }
}
/// A tentative worker request after the caller refreshed selection reasons.
fn tentative(c: &str, w: &str, op: &str, amount: u64) -> OwnerReservation {
    OwnerReservation {
        version: 1,
        operation: id(op),
        provider: provider(),
        generation: 1,
        work_version: id(w),
        claim: claim_ref(w),
        account: id(c),
        kind: ChargeKind::WorkerRequest,
        subject: id(op),
        accounting_basis: id("window-capacity-v1"),
        serving_bounds: Some(bounds("aggregate-service")),
        amounts: amounts(amount),
        expires_at_ms: 180,
        state: ReservationState::Tentative,
        receipt: None,
    }
}
fn reply(r: &OwnerReservation, state: ReservationState) -> OwnerReply {
    OwnerReply {
        version: 1,
        operation: r.operation.clone(),
        provider: r.provider.clone(),
        generation: r.generation,
        receipt: id(r.operation.as_str()),
        state,
        retry_at_ms: None,
        serving_bounds: r.serving_bounds.clone(),
    }
}

#[test]
fn serving_snapshots_are_bounded_monotonic_and_never_reservations() {
    let mut l = serving_ledger(ServingCapacityRule::KnownRequired);
    assert_eq!(
        code(l.observe_serving(snapshot(0, Some(1), Some(0)))),
        "admission_serving_snapshot"
    );
    assert!(l.observe_serving(snapshot(1, Some(3), Some(0))).is_err());
    assert!(l.observe_serving(snapshot(1, Some(1), Some(9))).is_err());
    let mut crossed = snapshot(1, Some(1), Some(0));
    crossed.bounds.scope = id("worker-local-queue");
    assert!(l.observe_serving(crossed).is_err());
    let mut inverted = snapshot(1, Some(1), Some(0));
    inverted.valid_until_ms = inverted.observed_at_ms;
    assert!(l.observe_serving(inverted).is_err());

    ok(l.observe_serving(snapshot(1, Some(1), Some(0))));
    // Duplicate delivery of the same snapshot is one observation.
    ok(l.observe_serving(snapshot(1, Some(1), Some(0))));
    assert_eq!(
        code(l.observe_serving(snapshot(1, Some(2), Some(0)))),
        "admission_serving_ambiguous"
    );
    ok(l.observe_serving(snapshot(2, Some(0), Some(0))));
    assert_eq!(
        code(l.observe_serving(snapshot(1, Some(1), Some(0)))),
        "admission_serving_order"
    );
    let mut hijack = snapshot(3, Some(2), Some(0));
    hijack.provider = other_provider();
    assert_eq!(code(l.observe_serving(hijack)), "admission_serving_owner");
    // A restarted source generation may restart its sequence.
    let mut restarted = snapshot(1, Some(2), Some(0));
    restarted.source_generation = 2;
    ok(l.observe_serving(restarted));
    // Evidence only: nothing was reserved, debited or refunded.
    assert!(l.reservations.is_empty());
    assert_eq!(
        ok(l.accounts.get(&id("a")).ok_or(())).available,
        amounts(100)
    );
}

// r[verify kiln.admission.serving.scenario.last_slot]
#[test]
fn saturated_owner_holds_work_before_and_after_claim_while_independent_work_progresses() {
    let mut l = serving_ledger(ServingCapacityRule::KnownRequired);
    ok(l.observe_serving(snapshot(1, Some(0), Some(8))));
    let mut b1 = item("b", "b1");
    b1.required_owners = BTreeMap::from([(other_provider(), ChargeKind::WorkerRequest)]);
    let mut b_snapshot = snapshot(1, Some(1), Some(0));
    b_snapshot.provider = other_provider();
    b_snapshot.scope = id("service-b");
    b_snapshot.bounds = bounds("service-b");
    ok(l.observe_serving(b_snapshot));
    ok(l.enqueue(item("a", "a1"), 100));
    ok(l.enqueue(b1, 100));
    let mut b_offer = input("b1");
    b_offer.provider = other_provider();
    // Pre-claim wait: the saturated owner does not get a claim; b progresses.
    assert_eq!(ok(l.select(&[input("a1"), b_offer], 100)), Some(id("b1")));
    let explained = ok(l.explanation(&id("a1"), 100));
    assert_eq!(explained.reason, AdmissionReason::Overloaded);
    assert_eq!(explained.phase, AdmissionPhase::PreClaim);
    let serving = ok(explained.serving.ok_or(()));
    assert_eq!(serving.gate, AdmissionReason::Overloaded);
    assert_eq!(ok(serving.observation.ok_or(())).remaining_slots, Some(0));
    // Owner queue room is not saturation; the owner still decides.
    ok(l.observe_serving(snapshot(2, Some(0), Some(3))));
    assert_eq!(ok(l.select(&[], 100)), Some(id("a1")));
    // Post-claim wait keeps the same claim instead of minting an attempt.
    ok(l.bind_claim(&id("a1"), claim_ref("a1")));
    ok(l.observe_serving(snapshot(3, Some(0), Some(8))));
    assert_eq!(ok(l.select(&[], 100)), Some(id("b1")));
    assert_eq!(reason(&l, "a1"), AdmissionReason::Overloaded);
    let held = ok(l.items.get(&id("a1")).ok_or(()));
    assert_eq!(held.claim, Some(claim_ref("a1")));
    assert_eq!(held.phase, AdmissionPhase::PostClaim);
    assert_eq!(
        code(l.request(tentative("a", "a1", "op-a", 10), 100)),
        "admission_serving_backpressure"
    );
    assert!(l.reservations.is_empty());
}

// r[verify kiln.admission.serving.scenario.last_slot]
#[test]
fn two_callers_on_the_last_slot_still_need_the_owner_grant() {
    let mut l = serving_ledger(ServingCapacityRule::KnownRequired);
    ok(l.observe_serving(snapshot(1, Some(1), Some(8))));
    for (c, w) in [("a", "a1"), ("b", "b1")] {
        ok(l.enqueue(item(c, w), 100));
        ok(l.bind_claim(&id(w), claim_ref(w)));
    }
    assert_eq!(
        ok(l.select(&[input("a1"), input("b1")], 100)),
        Some(id("a1"))
    );
    let ra = tentative("a", "a1", "op-a", 10);
    let rb = tentative("b", "b1", "op-b", 10);
    // Both callers saw one slot; neither observation is a reservation.
    ok(l.request(ra.clone(), 100));
    ok(l.request(rb.clone(), 100));
    // Duplicate request is one logical operation; a changed one is refused.
    ok(l.request(ra.clone(), 100));
    let mut changed = ra.clone();
    changed.amounts = amounts(11);
    assert_eq!(
        code(l.request(changed, 100)),
        "admission_operation_conflict"
    );
    ok(l.owner_reply(&reply(&ra, ReservationState::Committed), 100));
    assert_eq!(
        ok(l.owner_reply(&reply(&rb, ReservationState::Rejected), 100)),
        AdmissionReason::ProviderRejected
    );
    // Only the owner's rejection refunded b; a holds its debit.
    assert_eq!(
        ok(l.accounts.get(&id("a")).ok_or(())).available,
        amounts(90)
    );
    assert_eq!(
        ok(l.accounts.get(&id("b")).ok_or(())).available,
        amounts(100)
    );
    let policy = ok(l.policy_ref());
    assert_eq!(
        code(l.dispatch_intent(&rb.operation, &policy, &rb.claim, 1, 100)),
        "admission_dispatch_stale"
    );
    // A saturated snapshot after the grant does not revoke the owner grant.
    ok(l.observe_serving(snapshot(2, Some(0), Some(8))));
    assert_eq!(
        ok(ok(l.explanation(&id("a1"), 100)).serving.ok_or(())).gate,
        AdmissionReason::Eligible
    );
    ok(l.dispatch_intent(&ra.operation, &policy, &ra.claim, 1, 100));
}

// r[verify kiln.admission.serving.scenario.stale]
#[test]
fn stale_untrusted_missing_or_mismatched_snapshots_block_with_distinct_reasons() {
    type Mutate = fn(&mut ServingObservation);
    let cases: [(ServingCapacityRule, Option<Mutate>, AdmissionReason); 10] = [
        (
            ServingCapacityRule::KnownRequired,
            None,
            AdmissionReason::Incomplete,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.observed_at_ms = 85),
            AdmissionReason::Stale,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| {
                s.observed_at_ms = 95;
                s.valid_until_ms = 100;
            }),
            AdmissionReason::Stale,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.observed_at_ms = 101),
            AdmissionReason::Stale,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.readiness = ServingReadiness::Stale),
            AdmissionReason::Stale,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.readiness = ServingReadiness::Loading),
            AdmissionReason::Waiting,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.readiness = ServingReadiness::Absent),
            AdmissionReason::Incompatible,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.origin = ServingOrigin::HostProbe),
            AdmissionReason::Incomplete,
        ),
        (
            ServingCapacityRule::KnownRequired,
            Some(|s| s.remaining_slots = None),
            AdmissionReason::Incomplete,
        ),
        (
            ServingCapacityRule::OwnerAdmissionDecides,
            Some(|s| s.remaining_slots = None),
            AdmissionReason::Eligible,
        ),
    ];
    for (rule, mutate, expected) in cases {
        let mut l = serving_ledger(rule);
        ok(l.enqueue(item("a", "a1"), 100));
        let mut b1 = item("b", "b1");
        b1.required_owners = BTreeMap::from([(other_provider(), ChargeKind::Execution)]);
        ok(l.enqueue(b1, 100));
        if let Some(mutate) = mutate {
            let mut s = snapshot(1, Some(2), Some(0));
            mutate(&mut s);
            ok(l.observe_serving(s));
        }
        let mut b_offer = input("b1");
        b_offer.provider = other_provider();
        let selected = ok(l.select(&[input("a1"), b_offer], 100));
        assert_eq!(reason(&l, "a1"), expected, "{rule:?}");
        // Compatible independent work progresses whenever a1 is blocked.
        let expected_selection = if expected == AdmissionReason::Eligible {
            id("a1")
        } else {
            id("b1")
        };
        assert_eq!(selected, Some(expected_selection));
    }
    // Request-time exact gate: bounds and generation must match the snapshot.
    let mut l = serving_ledger(ServingCapacityRule::KnownRequired);
    ok(l.observe_serving(snapshot(1, Some(2), Some(0))));
    ok(l.enqueue(item("a", "a1"), 100));
    ok(l.bind_claim(&id("a1"), claim_ref("a1")));
    assert_eq!(ok(l.select(&[input("a1")], 100)), Some(id("a1")));
    let mut crossed = tentative("a", "a1", "op-a", 10);
    crossed.serving_bounds = Some(ServingBounds {
        max_queued: 9,
        ..bounds("aggregate-service")
    });
    assert_eq!(
        l.request_serving_gate(&crossed, 100),
        Some(AdmissionReason::Incompatible)
    );
    assert_eq!(
        code(l.request(crossed, 100)),
        "admission_serving_backpressure"
    );
    let mut replaced = tentative("a", "a1", "op-a", 10);
    replaced.generation = 2;
    assert_eq!(
        l.request_serving_gate(&replaced, 100),
        Some(AdmissionReason::Stale)
    );
    let mut unknown_scope = tentative("a", "a1", "op-a", 10);
    unknown_scope.serving_bounds = Some(bounds("unobserved"));
    assert_eq!(
        l.request_serving_gate(&unknown_scope, 100),
        Some(AdmissionReason::Incomplete)
    );
    // A snapshot that ages out between ranking and request blocks the request.
    assert_eq!(
        code(l.request(tentative("a", "a1", "op-a", 10), 111)),
        "admission_serving_backpressure"
    );
    ok(l.request(tentative("a", "a1", "op-a", 10), 100));
    // Without the opt-in block the gate is off and identities are unchanged.
    let plain = policy(FairnessStrategy::DominantShare);
    let mut with = plain.clone();
    with.serving = Some(ServingAdmissionPolicy {
        max_snapshot_age_ms: 10,
        capacity_rule: ServingCapacityRule::KnownRequired,
    });
    assert_ne!(ok(plain.identity()), ok(with.identity()));
    with.serving = Some(ServingAdmissionPolicy {
        max_snapshot_age_ms: 0,
        capacity_rule: ServingCapacityRule::KnownRequired,
    });
    assert!(with.validate().is_err());
    let plain_ledger = ledger(FairnessStrategy::DominantShare);
    assert_eq!(
        plain_ledger.request_serving_gate(&tentative("a", "a1", "op", 1), 100),
        None
    );
}

fn child_account(name: &str, parent: &str, parent_work: &str, amount: u64) -> BudgetAccount {
    BudgetAccount {
        identity: id(name),
        parent: Some(id(parent)),
        parent_attempt: Some(claim_ref(parent_work)),
        campaign: alias("a"),
        tenant: alias("tenant"),
        deadline_ms: 200,
        available: amounts(amount),
        consumed: Amounts::new(),
        children: 0,
    }
}
/// Parent work a1 is claimed; child account `kid` holds a committed request
/// for child work c1.
fn child_lineage() -> (AdmissionLedger, OwnerReservation) {
    let mut l = ledger(weighted());
    ok(l.enqueue(item("a", "a1"), 100));
    ok(l.bind_claim(&id("a1"), claim_ref("a1")));
    ok(l.transfer(&id("a"), child_account("kid", "a", "a1", 40)));
    let mut c1 = item("a", "c1");
    c1.account = id("kid");
    ok(l.enqueue(c1, 100));
    ok(l.bind_claim(&id("c1"), claim_ref("c1")));
    ok(l.select(&[input("a1"), input("c1")], 100));
    let r = tentative("kid", "c1", "op-c", 10);
    ok(l.request(r.clone(), 100));
    ok(l.owner_reply(&reply(&r, ReservationState::Committed), 100));
    (l, r)
}
fn descendant_negatives(l: &mut AdmissionLedger, r: &OwnerReservation, blocker: AdmissionReason) {
    assert_eq!(ok(l.lineage_blocker(&id("kid"), Some(100))), Some(blocker));
    let mut c2 = item("a", "c2");
    c2.account = id("kid");
    assert_eq!(ok(l.enqueue(c2, 100)), blocker);
    assert!(!l.items.contains_key(&id("c2")));
    ok(l.select(&[input("c1")], 100));
    let explained = ok(l.explanation(&id("c1"), 100));
    assert_eq!(explained.reason, blocker);
    assert_eq!(ok(explained.lineage.ok_or(())).blocker, Some(blocker));
    assert_eq!(
        code(l.transfer(&id("kid"), child_account("grand", "kid", "c1", 1))),
        "admission_lineage_blocked"
    );
    assert_eq!(
        code(l.request(tentative("kid", "c1", "op-c2", 1), 100)),
        "admission_lineage_blocked"
    );
    let policy = ok(l.policy_ref());
    assert_eq!(
        code(l.dispatch_intent(&r.operation, &policy, &r.claim, 1, 100)),
        "admission_lineage_blocked"
    );
    // No refund: the committed debit stays held in the lineage.
    assert_eq!(
        ok(l.reservations.get(&r.operation).ok_or(())).state,
        ReservationState::Committed
    );
    assert_eq!(
        ok(l.accounts.get(&id("kid")).ok_or(())).available,
        amounts(30)
    );
    let usage = ok(l.lineage_usage(&id("a")));
    assert_eq!(usage.held, amounts(10));
    assert_eq!(usage.allocated, amounts(100));
    // An unrelated campaign still progresses.
    ok(l.enqueue(item("b", "b1"), 100));
    assert_eq!(ok(l.select(&[input("b1")], 100)), Some(id("b1")));
}

// r[verify kiln.admission.lineage.scenario.blocked]
#[test]
fn revoked_lineage_blocks_descendants_without_refunding_held_usage() {
    let (mut l, r) = child_lineage();
    assert_eq!(ok(l.revoke_account(&id("kid"))), 1);
    assert_eq!(ok(l.revoke_account(&id("kid"))), 0);
    assert!(l.revoke_account(&id("missing")).is_err());
    descendant_negatives(&mut l, &r, AdmissionReason::Denied);
    assert_eq!(ok(l.lineage_usage(&id("a"))).revoked, 1);
    // The un-revoked parent keeps working.
    assert_eq!(ok(l.lineage_blocker(&id("a"), Some(100))), None);
}

// r[verify kiln.admission.lineage.scenario.blocked]
#[test]
fn cancelled_or_expired_parent_stops_new_descendant_dispatch() {
    let (mut l, r) = child_lineage();
    assert_eq!(ok(l.cancel(&id("a1"), false)), AdmissionReason::Cancelled);
    descendant_negatives(&mut l, &r, AdmissionReason::Cancelled);

    let (mut l, r) = child_lineage();
    assert_eq!(ok(l.cancel(&id("a1"), true)), AdmissionReason::Expired);
    descendant_negatives(&mut l, &r, AdmissionReason::Expired);

    // An ancestor deadline blocks descendants even before any cancellation.
    let (l, _) = child_lineage();
    assert_eq!(
        ok(l.lineage_blocker(&id("kid"), Some(200))),
        Some(AdmissionReason::Expired)
    );
    assert_eq!(ok(l.lineage_blocker(&id("kid"), Some(199))), None);
}

struct Rlm {
    l: AdmissionLedger,
    profile: LineageProfile,
    basis: ContentId,
}
const NOW: u64 = 1_000;
fn rlm_dims(model: u64, cpu: u64) -> Amounts {
    let mut out = Amounts::new();
    if model > 0 {
        out.insert(alias("model_requests"), model);
    }
    if cpu > 0 {
        out.insert(alias("cpu_ms"), cpu);
    }
    out
}
fn model() -> ProviderProfileId {
    ok(ProviderProfileId::new("model-serving"))
}
fn exec() -> ProviderProfileId {
    ok(ProviderProfileId::new("cpu-exec"))
}
fn coordinator() -> ProviderProfileId {
    ok(ProviderProfileId::new("coordinator"))
}
fn model_bounds() -> ServingBounds {
    ServingBounds {
        scope: id("model-scope"),
        max_ongoing: 4,
        max_queued: 16,
    }
}
impl Rlm {
    fn new() -> Self {
        let policy: AdmissionPolicy = ok(serde_json::from_str(POLICY_EXPORT));
        let profile: LineageProfile = ok(serde_json::from_str(LINEAGE_EXPORT));
        ok(profile.validate());
        let mut l = AdmissionLedger::default();
        ok(l.configure(policy.clone()));
        let mut rlm = Self {
            l,
            basis: policy.accounting_basis,
            profile,
        };
        for campaign in ["rlm", "peer"] {
            let identity = if campaign == "rlm" {
                rlm.account("root")
            } else {
                id("peer-root")
            };
            ok(rlm.l.add_root(BudgetAccount {
                identity,
                parent: None,
                parent_attempt: None,
                campaign: alias(campaign),
                tenant: rlm.profile.tenant.clone(),
                deadline_ms: rlm.profile.root.deadline_ms,
                available: rlm.profile.root.quota.clone(),
                consumed: Amounts::new(),
                children: 0,
            }));
            ok(rlm.l.observe(UsageObservation {
                usage: TenantUsage {
                    tenant: alias(campaign),
                    share: 1,
                    provider: model(),
                    interval_start: 0,
                    interval_end: NOW,
                    observed_at: NOW,
                    resources: BTreeMap::from([
                        (
                            alias("model_requests"),
                            crate::operations::UsageValue {
                                amount: 0,
                                capacity: Some(1_000),
                            },
                        ),
                        (
                            alias("cpu_ms"),
                            crate::operations::UsageValue {
                                amount: 0,
                                capacity: Some(1_000_000),
                            },
                        ),
                    ]),
                },
                generation: 1,
                accounting_basis: rlm.basis.clone(),
                covered: BTreeMap::new(),
                complete: true,
            }));
        }
        ok(rlm.l.observe_serving(ServingObservation {
            version: 1,
            provider: model(),
            scope: id("model-scope"),
            source_generation: 1,
            sequence: 1,
            deployment_generation: 1,
            readiness: ServingReadiness::Ready,
            origin: ServingOrigin::ServingOwner,
            observed_at_ms: NOW,
            valid_until_ms: 5_000,
            bounds: model_bounds(),
            remaining_slots: Some(4),
            queued: Some(0),
        }));
        rlm
    }
    fn account(&self, name: &str) -> ContentId {
        ok(self.profile.account_identity(name))
    }
    fn transfer(&mut self, name: &str, parent_work: &str) {
        let spec = ok(self
            .profile
            .accounts
            .iter()
            .find(|a| a.name.as_str() == name)
            .ok_or(()));
        let parent = self.account(spec.parent.as_str());
        ok(self.l.transfer(
            &parent,
            BudgetAccount {
                identity: self.account(name),
                parent: Some(parent.clone()),
                parent_attempt: Some(claim_ref(parent_work)),
                campaign: self.profile.campaign.clone(),
                tenant: self.profile.tenant.clone(),
                deadline_ms: spec.deadline_ms,
                available: spec.allocation.clone(),
                consumed: Amounts::new(),
                children: 0,
            },
        ));
    }
    fn work(&mut self, campaign: &str, account: ContentId, work: &str, budget: Amounts) {
        let item = QueueItem {
            work_version: id(work),
            account,
            tenant: self.profile.tenant.clone(),
            campaign: alias(campaign),
            repository: alias("repo"),
            priority: 0,
            bytes: 10,
            enqueued_at_ms: NOW,
            deadline_ms: 30_000,
            eligible_at_ms: NOW,
            claim: None,
            phase: AdmissionPhase::PreClaim,
            reason: AdmissionReason::Waiting,
            order: 0,
            required_owners: BTreeMap::from([
                (coordinator(), ChargeKind::Coordination),
                (exec(), ChargeKind::Execution),
                (model(), ChargeKind::WorkerRequest),
            ]),
            shared_allocations: vec![],
            budget,
        };
        assert_eq!(ok(self.l.enqueue(item, NOW)), AdmissionReason::Waiting);
        ok(self.l.bind_claim(&id(work), claim_ref(work)));
    }
    fn refresh(&mut self, works: &[&str]) -> Option<ContentId> {
        let offers: Vec<_> = works
            .iter()
            .map(|w| EligibilityInput {
                work_version: id(w),
                reason: AdmissionReason::Eligible,
                provider: model(),
                generation: 1,
                observed_at_ms: NOW,
                valid_until_ms: 5_000,
            })
            .collect();
        ok(self.l.select(&offers, NOW))
    }
    fn op(
        &self,
        work: &str,
        account: &str,
        name: &str,
        kind: ChargeKind,
        amounts: Amounts,
    ) -> OwnerReservation {
        let (provider, serving_bounds) = match kind {
            ChargeKind::Coordination => (coordinator(), None),
            ChargeKind::Execution => (exec(), None),
            _ => (model(), Some(model_bounds())),
        };
        OwnerReservation {
            version: 1,
            operation: id(name),
            provider,
            generation: 1,
            work_version: id(work),
            claim: claim_ref(work),
            account: self.account(account),
            kind,
            subject: id(name),
            accounting_basis: self.basis.clone(),
            serving_bounds,
            amounts,
            expires_at_ms: 20_000,
            state: ReservationState::Tentative,
            receipt: None,
        }
    }
    fn charge(&mut self, r: &OwnerReservation) {
        ok(self.l.request(r.clone(), NOW));
        ok(self
            .l
            .owner_reply(&reply(r, ReservationState::Committed), NOW));
    }
}

// r[verify kiln.admission.lineage.scenario.recursive]
#[test]
fn recursive_profile_charges_one_lineage_without_minting_or_double_charging() {
    let mut rlm = Rlm::new();
    let root = rlm.account("root");
    let child_a = rlm.account("child_a");
    let child_b = rlm.account("child_b");
    // Root coordination work owns the parent attempt for both children.
    rlm.work("rlm", root.clone(), "root-work", rlm_dims(16, 20_000));
    rlm.transfer("child_a", "root-work");
    rlm.transfer("child_b", "root-work");
    assert_eq!(
        ok(rlm.l.accounts.get(&root).ok_or(())).available,
        rlm_dims(16, 20_000)
    );
    rlm.refresh(&["root-work"]);
    let coordination = rlm.op(
        "root-work",
        "root",
        "root-coordination",
        ChargeKind::Coordination,
        rlm_dims(0, 10_000),
    );
    rlm.charge(&coordination);
    // Plain model subcalls from generated code: three single calls plus one batch.
    let mut root_calls = Vec::new();
    for (name, n) in [("call-1", 2), ("call-2", 2), ("call-3", 2), ("batch-1", 6)] {
        rlm.refresh(&["root-work"]);
        let r = rlm.op(
            "root-work",
            "root",
            name,
            ChargeKind::WorkerRequest,
            rlm_dims(n, 0),
        );
        rlm.charge(&r);
        root_calls.push(r);
    }
    // A fresh call ID or a bigger batch cannot exceed the work's finite budget.
    rlm.refresh(&["root-work"]);
    assert_eq!(
        code(rlm.l.request(
            rlm.op(
                "root-work",
                "root",
                "call-new-id",
                ChargeKind::WorkerRequest,
                rlm_dims(5, 0)
            ),
            NOW
        )),
        "admission_budget_exhausted"
    );
    // A second root or renamed worker identity cannot mint a campaign root.
    assert!(
        rlm.l
            .add_root(BudgetAccount {
                identity: id("reconnected-worker-root"),
                parent: None,
                parent_attempt: None,
                campaign: alias("rlm"),
                tenant: alias("tenant"),
                deadline_ms: 50_000,
                available: rlm_dims(40, 60_000),
                consumed: Amounts::new(),
                children: 0,
            })
            .is_err()
    );

    // Nested child execution plus its own model subcall, then a grandchild.
    rlm.work("rlm", child_a.clone(), "child-a-work", rlm_dims(12, 20_000));
    rlm.refresh(&["child-a-work"]);
    rlm.charge(&rlm.op(
        "child-a-work",
        "child_a",
        "child-a-exec",
        ChargeKind::Execution,
        rlm_dims(0, 15_000),
    ));
    rlm.refresh(&["child-a-work"]);
    rlm.charge(&rlm.op(
        "child-a-work",
        "child_a",
        "child-a-call",
        ChargeKind::WorkerRequest,
        rlm_dims(4, 0),
    ));
    rlm.transfer("grandchild_a1", "child-a-work");
    let grandchild = rlm.account("grandchild_a1");
    // Deeper nesting cannot mint quota: the grandchild holds only its transfer.
    assert_eq!(
        code(rlm.l.transfer(
            &grandchild,
            BudgetAccount {
                identity: id("great-grandchild"),
                parent: Some(grandchild.clone()),
                parent_attempt: Some(claim_ref("child-a-work")),
                campaign: alias("rlm"),
                tenant: alias("tenant"),
                deadline_ms: 30_000,
                available: rlm_dims(5, 0),
                consumed: Amounts::new(),
                children: 0,
            },
        )),
        "admission_budget_exhausted"
    );
    rlm.work(
        "rlm",
        grandchild.clone(),
        "grandchild-work",
        rlm_dims(4, 5_000),
    );
    rlm.refresh(&["grandchild-work"]);
    let grandchild_call = rlm.op(
        "grandchild-work",
        "grandchild_a1",
        "grandchild-call",
        ChargeKind::WorkerRequest,
        rlm_dims(4, 0),
    );
    rlm.charge(&grandchild_call);

    // Simultaneous children compete for child_b's final allowance: both are
    // ranked eligible, the first request wins and the second is refused.
    rlm.work("rlm", child_b.clone(), "child-b-1", rlm_dims(8, 0));
    rlm.work("rlm", child_b.clone(), "child-b-2", rlm_dims(8, 0));
    rlm.refresh(&["child-b-1", "child-b-2"]);
    rlm.charge(&rlm.op(
        "child-b-1",
        "child_b",
        "b1-call",
        ChargeKind::WorkerRequest,
        rlm_dims(8, 0),
    ));
    assert_eq!(
        code(rlm.l.request(
            rlm.op(
                "child-b-2",
                "child_b",
                "b2-call",
                ChargeKind::WorkerRequest,
                rlm_dims(8, 0)
            ),
            NOW
        )),
        "admission_budget_exhausted"
    );
    // The loser is then an explicit pre-dispatch blocker, not silent waiting.
    rlm.refresh(&["child-b-1", "child-b-2"]);
    assert_eq!(
        ok(rlm.l.explanation(&id("child-b-2"), NOW)).reason,
        AdmissionReason::BudgetExhausted
    );
    assert_eq!(
        code(rlm.l.request(
            rlm.op(
                "child-b-1",
                "child_b",
                "b1-after",
                ChargeKind::WorkerRequest,
                rlm_dims(1, 0)
            ),
            NOW
        )),
        "admission_budget_exhausted"
    );

    // Duplicate grant and settlement receipts update one logical record.
    let [first, second, ..] = root_calls.as_slice() else {
        unreachable!("four root calls were charged")
    };
    ok(rlm
        .l
        .owner_reply(&reply(first, ReservationState::Committed), NOW));
    ok(rlm.l.settle(
        &first.operation,
        &model(),
        1,
        id("settled-call-1"),
        rlm_dims(2, 0),
    ));
    ok(rlm.l.settle(
        &first.operation,
        &model(),
        1,
        id("settled-call-1"),
        rlm_dims(2, 0),
    ));
    assert_eq!(
        code(rlm.l.settle(
            &first.operation,
            &model(),
            1,
            id("another-receipt"),
            rlm_dims(2, 0)
        )),
        "admission_settlement_conflict"
    );
    // A stale owner incarnation cannot settle or reply.
    assert_eq!(
        code(
            rlm.l
                .settle(&second.operation, &model(), 2, id("old"), rlm_dims(2, 0))
        ),
        "admission_settlement_owner"
    );
    // Possible transport makes two root operations unknown; never refunded.
    let policy = ok(rlm.l.policy_ref());
    ok(rlm.l.dispatch_intent(
        &coordination.operation,
        &policy,
        &coordination.claim,
        1,
        NOW,
    ));
    ok(rlm
        .l
        .dispatch_intent(&second.operation, &policy, &second.claim, 1, NOW));

    let usage = ok(rlm.l.lineage_usage(&root));
    assert_eq!(usage.accounts, 4);
    assert_eq!(usage.max_depth, 2);
    assert_eq!(usage.allocated, rlm_dims(40, 60_000));
    assert_eq!(usage.settled, rlm_dims(2, 0));
    assert_eq!(usage.unknown, rlm_dims(2, 10_000));
    assert_eq!(usage.unknown_operations, 2);
    assert!(usage.tentative.is_empty());
    // A small root transcript still reports every descendant model subcall.
    assert_eq!(
        usage.by_kind.get(&ChargeKind::WorkerRequest),
        Some(&rlm_dims(12 + 4 + 4 + 8, 0))
    );
    assert_eq!(
        usage.by_kind.get(&ChargeKind::Coordination),
        Some(&rlm_dims(0, 10_000))
    );
    assert_eq!(
        usage.by_kind.get(&ChargeKind::Execution),
        Some(&rlm_dims(0, 15_000))
    );
    assert_eq!(
        ok(rlm.l.lineage_usage(&id("peer-root"))).held,
        Amounts::new()
    );
    assert!(rlm.l.lineage_usage(&child_a).is_err());

    // The child burst is charged immediately: a comparable peer outranks a
    // new root-funded rlm item.
    rlm.work("rlm", root.clone(), "rlm-late", rlm_dims(2, 0));
    rlm.work("peer", id("peer-root"), "peer-1", rlm_dims(2, 0));
    assert_eq!(rlm.refresh(&["rlm-late", "peer-1"]), Some(id("peer-1")));

    // Cancelling the child's parent work stops new grandchild dispatch.
    assert_eq!(
        ok(rlm.l.cancel(&id("child-a-work"), false)),
        AdmissionReason::CancellationRequested
    );
    assert_eq!(
        code(rlm.l.dispatch_intent(
            &grandchild_call.operation,
            &policy,
            &grandchild_call.claim,
            1,
            NOW
        )),
        "admission_lineage_blocked"
    );
    assert_eq!(
        ok(rlm.l.lineage_usage(&root)).unknown,
        rlm_dims(2, 10_000),
        "removing local work is not a refund"
    );
}

// r[verify kiln.admission.profiles.scenario.exports]
#[test]
fn committed_profile_exports_admit_and_invalid_profiles_refuse() {
    let policy: AdmissionPolicy = ok(serde_json::from_str(POLICY_EXPORT));
    ok(policy.validate());
    assert_eq!(
        policy.serving,
        Some(ServingAdmissionPolicy {
            max_snapshot_age_ms: 5_000,
            capacity_rule: ServingCapacityRule::KnownRequired,
        })
    );
    let profile: LineageProfile = ok(serde_json::from_str(LINEAGE_EXPORT));
    ok(profile.validate());
    assert_ne!(
        ok(profile.account_identity("root")),
        ok(profile.account_identity("child_a"))
    );
    let mut over = profile.clone();
    ok(over.accounts.get_mut(1).ok_or(())).allocation = rlm_dims(29, 20_000);
    assert!(over.validate().is_err());
    let mut late = profile.clone();
    ok(late.accounts.get_mut(2).ok_or(())).deadline_ms = 45_000;
    assert!(late.validate().is_err());
    let mut deep = profile.clone();
    deep.max_depth = 1;
    assert!(deep.validate().is_err());
    let mut forward = profile.clone();
    ok(forward.accounts.get_mut(0).ok_or(())).parent = alias("child_b");
    assert!(forward.validate().is_err());
    let mut fanout = profile.clone();
    fanout.max_children = 1;
    assert!(fanout.validate().is_err());
    let mut unbounded = profile;
    unbounded.max_depth = MAX_LINEAGE_DEPTH + 1;
    assert!(unbounded.validate().is_err());
}
