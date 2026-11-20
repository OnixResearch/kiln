//! Durable recursive lineage and serving backpressure over the committed
//! Nickel exports. Injected owner replies and snapshots only; no live provider.
use kiln_core::fair_admission::*;
use kiln_core::implementation::*;
use kiln_core::operations::{TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::{
    Implementer,
    admission::{AdmissionCommand as Cmd, AdmissionResult as ResultFact},
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const POLICY_EXPORT: &str = include_str!("../../../fixtures/fair-admission/serving-policy.json");
const LINEAGE_EXPORT: &str =
    include_str!("../../../fixtures/fair-admission/recursive-lineage.json");
const NOW: u64 = 1_000;
const LEASE_MS: u64 = 60_000;

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("{error:?}"),
    }
}
fn refused<T: std::fmt::Debug>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => unreachable!("expected refusal, got {value:?}"),
        Err(error) => error,
    }
}
fn id(s: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(s.as_bytes()).to_hex().to_string(),
    ))
}
fn a(s: &str) -> Alias {
    ok(Alias::new(s))
}
fn model() -> ProviderProfileId {
    ok(ProviderProfileId::new("model-serving"))
}
fn requests(n: u64) -> Amounts {
    BTreeMap::from([(a("model_requests"), n)])
}
fn open(path: &Path) -> Implementer {
    ok(Implementer::open(
        path,
        Enrollment {
            repositories: BTreeSet::from(["onix/kiln".into(), "onix/child".into()]),
            actors: BTreeSet::from(["operator".into()]),
        },
    ))
}
fn work(n: &str, repository: &str) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: repository.into(),
        project: "kiln".into(),
        base: None,
        title: n.into(),
        requested_outcome: "recursive model calls".into(),
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
            source: ok(ExternalRef::new("github", "issue", n)),
            native_id: n.into(),
            version: id(n),
        }],
    }
}
fn bounds() -> ServingBounds {
    ServingBounds {
        scope: id("model-scope"),
        max_ongoing: 4,
        max_queued: 16,
    }
}
fn snapshot(sequence: u64, slots: u32, queued: u32) -> ServingObservation {
    ServingObservation {
        version: 1,
        provider: model(),
        scope: id("model-scope"),
        source_generation: 1,
        sequence,
        deployment_generation: 1,
        readiness: ServingReadiness::Ready,
        origin: ServingOrigin::ServingOwner,
        observed_at_ms: NOW,
        valid_until_ms: 5_000,
        bounds: bounds(),
        remaining_slots: Some(slots),
        queued: Some(queued),
    }
}
fn offer(w: &ImplementationWork) -> EligibilityInput {
    EligibilityInput {
        work_version: ok(w.identity()),
        reason: AdmissionReason::Eligible,
        provider: model(),
        generation: 1,
        observed_at_ms: NOW,
        valid_until_ms: 5_000,
    }
}

struct Builder {
    profile: LineageProfile,
    basis: ContentId,
}
struct Fixture {
    b: Builder,
    root: ContentId,
    child_a: ContentId,
    w0: ImplementationWork,
    w1: ImplementationWork,
    a1: ImplementationAttempt,
}
impl Fixture {
    fn op(
        &self,
        attempt: &ImplementationAttempt,
        account: &ContentId,
        name: &str,
        n: u64,
    ) -> OwnerReservation {
        self.b.op(attempt, account, name, n)
    }
}
impl Builder {
    fn item(&self, w: &ImplementationWork, account: &ContentId, deadline_ms: u64) -> QueueItem {
        QueueItem {
            work_version: ok(w.identity()),
            account: account.clone(),
            tenant: self.profile.tenant.clone(),
            campaign: self.profile.campaign.clone(),
            repository: a("repo"),
            priority: 0,
            bytes: 1,
            enqueued_at_ms: NOW,
            deadline_ms,
            eligible_at_ms: NOW,
            claim: None,
            phase: AdmissionPhase::PreClaim,
            reason: AdmissionReason::Waiting,
            order: 0,
            required_owners: BTreeMap::from([(model(), ChargeKind::WorkerRequest)]),
            shared_allocations: vec![],
            budget: requests(4),
        }
    }
    fn op(
        &self,
        attempt: &ImplementationAttempt,
        account: &ContentId,
        name: &str,
        n: u64,
    ) -> OwnerReservation {
        OwnerReservation {
            version: 1,
            operation: id(name),
            provider: model(),
            generation: 1,
            work_version: attempt.work_identity.clone(),
            claim: ClaimRef {
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
            },
            account: account.clone(),
            kind: ChargeKind::WorkerRequest,
            subject: id(name),
            accounting_basis: self.basis.clone(),
            serving_bounds: Some(bounds()),
            amounts: requests(n),
            expires_at_ms: 30_000,
            state: ReservationState::Tentative,
            receipt: None,
        }
    }
}
fn request(i: &mut Implementer, r: &OwnerReservation, inputs: Vec<EligibilityInput>) -> ResultFact {
    ok(i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs,
        },
        NOW,
    ))
}
fn commit(
    i: &mut Implementer,
    r: &OwnerReservation,
    generation: u64,
) -> Result<ResultFact, String> {
    i.admission_apply(
        Cmd::Reply {
            reply: OwnerReply {
                version: 1,
                operation: r.operation.clone(),
                provider: model(),
                generation,
                receipt: id(&format!("grant-{}", r.operation.as_str())),
                state: ReservationState::Committed,
                retry_at_ms: None,
                serving_bounds: Some(bounds()),
            },
        },
        NOW,
    )
}
fn lineage(i: &mut Implementer, root: &ContentId) -> LineageUsage {
    match ok(i.admission_apply(Cmd::Lineage { root: root.clone() }, NOW)) {
        ResultFact::Lineage { usage } => *usage,
        other => unreachable!("{other:?}"),
    }
}

/// Root work W0 holds a committed model request; the profile's `child_a`
/// account is transferred under W0's attempt and W1 is claimed inside it.
fn setup(path: &Path) -> (Implementer, Fixture) {
    let policy: AdmissionPolicy = ok(serde_json::from_str(POLICY_EXPORT));
    let profile: LineageProfile = ok(serde_json::from_str(LINEAGE_EXPORT));
    ok(profile.validate());
    let root = ok(profile.account_identity("root"));
    let child_a = ok(profile.account_identity("child_a"));
    let mut i = open(path);
    ok(i.admission_apply(
        Cmd::Configure {
            policy: policy.clone(),
        },
        NOW,
    ));
    ok(i.admission_apply(
        Cmd::Root {
            account: BudgetAccount {
                identity: root.clone(),
                parent: None,
                parent_attempt: None,
                campaign: profile.campaign.clone(),
                tenant: profile.tenant.clone(),
                deadline_ms: profile.root.deadline_ms,
                available: profile.root.quota.clone(),
                consumed: Amounts::new(),
                children: 0,
            },
        },
        NOW,
    ));
    let usage = TenantUsage {
        tenant: profile.campaign.clone(),
        share: 1,
        provider: model(),
        interval_start: 0,
        interval_end: NOW,
        observed_at: NOW,
        resources: BTreeMap::from([
            (
                a("model_requests"),
                UsageValue {
                    amount: 0,
                    capacity: Some(1_000),
                },
            ),
            (
                a("cpu_ms"),
                UsageValue {
                    amount: 0,
                    capacity: Some(1_000_000),
                },
            ),
        ]),
    };
    ok(i.admission_apply(
        Cmd::Observe {
            observation: UsageObservation {
                usage,
                generation: 1,
                accounting_basis: policy.accounting_basis.clone(),
                covered: BTreeMap::new(),
                complete: true,
            },
        },
        NOW,
    ));
    ok(i.admission_apply(
        Cmd::ObserveServing {
            observation: snapshot(1, 4, 0),
        },
        NOW,
    ));
    let w0 = work("rlm-root", "onix/kiln");
    let w1 = work("rlm-child-a", "onix/child");
    let b = Builder {
        profile,
        basis: policy.accounting_basis,
    };
    ok(i.admit_with_supersession(&w0, None, SupersessionRule::AbortActive, NOW));
    ok(i.admission_apply(
        Cmd::Enqueue {
            item: b.item(&w0, &root, 45_000),
        },
        NOW,
    ));
    let a0 = match ok(i.admission_apply(
        Cmd::Claim {
            inputs: vec![offer(&w0)],
            lease_ms: LEASE_MS,
        },
        NOW,
    )) {
        ResultFact::Claimed { attempt: Some(a) } => *a,
        other => unreachable!("{other:?}"),
    };
    // The root's own model call is committed before any child is selected.
    let root_call = b.op(&a0, &root, "root-call", 2);
    assert!(matches!(
        request(&mut i, &root_call, vec![]),
        ResultFact::Updated
    ));
    ok(commit(&mut i, &root_call, 1));
    let spec = b
        .profile
        .accounts
        .iter()
        .find(|a| a.name.as_str() == "child_a")
        .cloned();
    let spec = ok(spec.ok_or(()));
    ok(i.admission_apply(
        Cmd::Transfer {
            parent: root.clone(),
            child: BudgetAccount {
                identity: child_a.clone(),
                parent: Some(root.clone()),
                parent_attempt: Some(ClaimRef {
                    attempt: a0.identity.clone(),
                    fence: a0.fence,
                }),
                campaign: b.profile.campaign.clone(),
                tenant: b.profile.tenant.clone(),
                deadline_ms: spec.deadline_ms,
                available: spec.allocation,
                consumed: Amounts::new(),
                children: 0,
            },
        },
        NOW,
    ));
    ok(i.admit_with_supersession(&w1, None, SupersessionRule::AbortActive, NOW));
    ok(i.admission_apply(
        Cmd::Enqueue {
            item: b.item(&w1, &child_a, 35_000),
        },
        NOW,
    ));
    let a1 = match ok(i.admission_apply(
        Cmd::ClaimExact {
            expected_work_version: ok(w1.identity()),
            parent_budget_identity: root.clone(),
            inputs: vec![offer(&w1)],
            lease_ms: LEASE_MS,
        },
        NOW,
    )) {
        ResultFact::Claimed { attempt: Some(a) } => *a,
        other => unreachable!("{other:?}"),
    };
    (
        i,
        Fixture {
            b,
            root,
            child_a,
            w0,
            w1,
            a1,
        },
    )
}

// r[verify kiln.admission.serving.scenario.stale]
// r[verify kiln.admission.lineage.scenario.recursive]
// r[verify kiln.admission.profiles.scenario.exports]
#[test]
fn recursive_lineage_and_serving_backpressure_survive_reopen_without_minting() {
    let dir = ok(tempfile::tempdir());
    let path = dir.path().join("ledger.db");
    let (mut i, f) = setup(&path);
    let child_call = f.op(&f.a1, &f.child_a, "child-call", 2);

    // Saturated owner: a structured overload, nothing tentative persisted.
    ok(i.admission_apply(
        Cmd::ObserveServing {
            observation: snapshot(2, 0, 16),
        },
        NOW,
    ));
    assert!(matches!(
        request(&mut i, &child_call, vec![offer(&f.w1)]),
        ResultFact::Owner {
            reason: AdmissionReason::Overloaded
        }
    ));
    // A source-stale snapshot is stale, not capacity.
    let mut stale = snapshot(3, 4, 0);
    stale.readiness = ServingReadiness::Stale;
    ok(i.admission_apply(Cmd::ObserveServing { observation: stale }, NOW));
    assert!(matches!(
        request(&mut i, &child_call, vec![offer(&f.w1)]),
        ResultFact::Owner {
            reason: AdmissionReason::Stale
        }
    ));
    // An out-of-order snapshot replay is refused.
    assert!(
        refused(i.admission_apply(
            Cmd::ObserveServing {
                observation: snapshot(1, 4, 0),
            },
            NOW,
        ))
        .contains("admission_serving_order")
    );
    let state = ok(i.admission_snapshot());
    assert!(!state.reservations.contains_key(&child_call.operation));
    assert_eq!(
        ok(state.accounts.get(&f.child_a).ok_or(())).available,
        BTreeMap::from([(a("cpu_ms"), 20_000), (a("model_requests"), 12)])
    );

    // Fresh capacity: the request persists before the owner, then commits.
    ok(i.admission_apply(
        Cmd::ObserveServing {
            observation: snapshot(4, 1, 0),
        },
        NOW,
    ));
    assert!(matches!(
        request(&mut i, &child_call, vec![offer(&f.w1)]),
        ResultFact::Updated
    ));
    // Duplicate request is retained, a changed one under the same ID refused.
    assert!(matches!(
        request(&mut i, &child_call, vec![offer(&f.w1)]),
        ResultFact::Retained { .. }
    ));
    let mut changed = child_call.clone();
    changed.amounts = requests(3);
    assert!(
        refused(i.admission_apply(
            Cmd::Request {
                reservation: changed,
                inputs: vec![],
            },
            NOW,
        ))
        .contains("retained operation differs")
    );
    // A stale owner incarnation cannot answer.
    assert!(commit(&mut i, &child_call, 2).is_err());
    ok(commit(&mut i, &child_call, 1));

    // Restart retains held debits, snapshots and lineage.
    let before = ok(i.admission_snapshot());
    drop(i);
    let mut i = open(&path);
    assert_eq!(ok(i.admission_snapshot()), before);
    let usage = lineage(&mut i, &f.root);
    assert_eq!(usage.accounts, 2);
    assert_eq!(usage.held, requests(4));
    assert_eq!(usage.allocated, f.b.profile.root.quota);

    // Possible transport: unknown, then restart and stale-incarnation checks.
    assert!(
        i.admission_before_transport(&f.a1.identity, f.a1.fence + 1, &[offer(&f.w1)], NOW)
            .is_err()
    );
    ok(i.admission_before_transport(&f.a1.identity, f.a1.fence, &[offer(&f.w1)], NOW));
    drop(i);
    let mut i = open(&path);
    let usage = lineage(&mut i, &f.root);
    assert_eq!(usage.unknown, requests(2));
    assert_eq!(usage.unknown_operations, 1);
    let settle = |generation: u64, receipt: &str, consumed: u64| Cmd::Settle {
        operation: child_call.operation.clone(),
        provider: model(),
        generation,
        receipt: id(receipt),
        consumed: requests(consumed),
    };
    assert!(
        refused(i.admission_apply(settle(2, "late", 1), NOW))
            .contains("admission_settlement_owner")
    );
    ok(i.admission_apply(settle(1, "settled", 1), NOW));
    ok(i.admission_apply(settle(1, "settled", 1), NOW));
    assert!(
        refused(i.admission_apply(settle(1, "other", 1), NOW))
            .contains("admission_settlement_conflict")
    );
    let usage = lineage(&mut i, &f.root);
    assert_eq!(usage.settled, requests(1));
    assert!(usage.unknown.is_empty());
    assert_eq!(usage.held, requests(2));

    // Revocation persists and blocks new descendant requests and transfers.
    assert!(matches!(
        ok(i.admission_apply(
            Cmd::Revoke {
                account: f.child_a.clone()
            },
            NOW
        )),
        ResultFact::Revoked { accounts: 1 }
    ));
    drop(i);
    let mut i = open(&path);
    assert!(matches!(
        request(
            &mut i,
            &f.op(&f.a1, &f.child_a, "after-revoke", 1),
            vec![offer(&f.w1)]
        ),
        ResultFact::Owner {
            reason: AdmissionReason::Denied
        }
    ));
    assert!(
        refused(i.admission_apply(
            Cmd::Transfer {
                parent: f.child_a.clone(),
                child: BudgetAccount {
                    identity: id("grandchild"),
                    parent: Some(f.child_a.clone()),
                    parent_attempt: Some(ClaimRef {
                        attempt: f.a1.identity.clone(),
                        fence: f.a1.fence,
                    }),
                    campaign: f.b.profile.campaign.clone(),
                    tenant: f.b.profile.tenant.clone(),
                    deadline_ms: 30_000,
                    available: requests(1),
                    consumed: Amounts::new(),
                    children: 0,
                },
            },
            NOW,
        ))
        .contains("admission_lineage_blocked")
    );
    assert_eq!(lineage(&mut i, &f.root).revoked, 1);
}

// r[verify kiln.admission.lineage.scenario.blocked]
#[test]
fn cancelled_parent_stops_child_dispatch_without_refund_across_reopen() {
    let dir = ok(tempfile::tempdir());
    let path = dir.path().join("ledger.db");
    let (mut i, f) = setup(&path);
    let child_call = f.op(&f.a1, &f.child_a, "child-call", 2);
    assert!(matches!(
        request(&mut i, &child_call, vec![offer(&f.w1)]),
        ResultFact::Updated
    ));
    ok(commit(&mut i, &child_call, 1));
    assert!(matches!(
        ok(i.admission_apply(
            Cmd::Cancel {
                work_version: ok(f.w0.identity()),
                expired: false,
            },
            NOW,
        )),
        ResultFact::Owner {
            reason: AdmissionReason::CancellationRequested
        }
    ));
    drop(i);
    let mut i = open(&path);
    assert!(
        refused(i.admission_before_transport(&f.a1.identity, f.a1.fence, &[offer(&f.w1)], NOW))
            .contains("admission_lineage_blocked")
    );
    assert!(matches!(
        request(
            &mut i,
            &f.op(&f.a1, &f.child_a, "after-cancel", 1),
            vec![offer(&f.w1)]
        ),
        ResultFact::Owner {
            reason: AdmissionReason::Cancelled
        }
    ));
    let state = ok(i.admission_snapshot());
    let explained = ok(state.explanation(&ok(f.w1.identity()), NOW));
    assert_eq!(
        ok(explained.lineage.ok_or(())).blocker,
        Some(AdmissionReason::Cancelled)
    );
    // Removing local work is not a refund: both committed debits remain held.
    assert_eq!(lineage(&mut i, &f.root).held, requests(4));
}
