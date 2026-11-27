//! Integration tests for durable grant issuance, #61 concurrent child budgets,
//! unknown-termination cleanup reconciliation, and redacted #60 explanations (#70, #60).

#[path = "authority_common/mod.rs"]
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use common::*;
use kiln_adapter_basalt::RevocationView as _;
use kiln_adapter_basalt::test_support::{self, Fixture};
use kiln_core::fair_admission::*;
use kiln_core::implementation::authority::ProtectedOperation as Op;
use kiln_core::implementation::grant::{
    AuthorityCeiling, CeilingTerms, ChildGrantRequest, GrantBudget, GrantLifecyclePhase,
    RootGrantRequest,
};
use kiln_core::implementation::{
    DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, FailureClass,
    ImplementationAttempt, ImplementationDispatchPlan, ImplementationWork, SupersessionRule,
    WorkspaceIsolation,
};
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::admission::{AdmissionCommand as Cmd, AdmissionResult as Res};
use kiln_implementer::authority_state::{
    AuthorityStateStore as _, SqliteAuthorityState, SqliteRevocationView, authority_state_path,
};
use kiln_implementer::grant_issuance::{
    GrantIssuanceAction, GrantRef, explain_work_admission,
};
use rusqlite::Connection;
use ucan::ProofReference;

const NOW: u64 = test_support::VERIFY_AT_S * 1_000;
const NOT_AFTER_S: u64 = test_support::EXPIRES_S;
const AUTH_GEN: u64 = test_support::GENERATION;
const LEASE_MS: u64 = 400_000;
const COORDINATOR_DID: &str = "did:key:coordinator";
const WORKER_A: &str = "did:key:worker-a";
const WORKER_B: &str = "did:key:worker-b";
const WORKER_C: &str = "did:key:worker-c";

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("{error:?}"),
    }
}

fn err<T: std::fmt::Debug>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => unreachable!("expected refusal, got {value:?}"),
        Err(error) => error,
    }
}

fn a(name: &str) -> Alias {
    ok(Alias::new(name))
}

fn amounts(n: u64) -> Amounts {
    BTreeMap::from([(a("requests"), n)])
}

fn proof_ref(byte: u8) -> GrantRef {
    [byte; 32]
}

fn open_local(db: &Path) -> Implementer {
    ok(Implementer::open_with_local_operator(
        db,
        enrollment(),
        "test-operator",
    ))
}

fn setup_work_and_claim(i: &mut Implementer, name: &str) -> (ImplementationWork, ImplementationAttempt) {
    let mut w = work(name);
    w.base = Some(ok(ExternalRef::new("git", "commit", &"b".repeat(40))));
    let policy = AdmissionPolicy {
        version: 1,
        accounting_basis: id("basis"),
        max_items: 32,
        max_bytes: 100_000,
        max_wait_ms: 1_000_000,
        max_observation_age_ms: 100_000,
        max_retry_after_ms: 50,
        max_children: 4,
        max_active: 2,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(a("a"), 1)]),
        group_limits: vec![],
        serving: None,
    };
    ok(i.admission_apply(Cmd::Configure { policy }, NOW));
    ok(i.admission_apply(
        Cmd::Root {
            account: BudgetAccount {
                identity: id("a"),
                parent: None,
                parent_attempt: None,
                campaign: a("a"),
                tenant: a("tenant"),
                deadline_ms: NOW + 1_000_000,
                available: amounts(100),
                consumed: Amounts::new(),
                children: 0,
            },
        },
        NOW,
    ));
    let observation = UsageObservation {
        usage: TenantUsage {
            tenant: a("a"),
            share: 1,
            provider: p(),
            interval_start: 0,
            interval_end: NOW,
            observed_at: NOW,
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
    ok(i.admission_apply(Cmd::Observe { observation }, NOW));
    ok(i.admit_with_supersession(&w, None, SupersessionRule::AbortActive, NOW));
    ok(i.admission_apply(
        Cmd::Enqueue {
            item: QueueItem {
                work_version: ok(w.identity()),
                account: id("a"),
                tenant: a("tenant"),
                campaign: a("a"),
                repository: a("repo"),
                priority: 1,
                bytes: 1,
                enqueued_at_ms: NOW,
                deadline_ms: NOW + 500_000,
                eligible_at_ms: NOW,
                claim: None,
                phase: AdmissionPhase::PreClaim,
                reason: AdmissionReason::Waiting,
                order: 0,
                required_owners: BTreeMap::from([(p(), ChargeKind::WorkerRequest)]),
                shared_allocations: vec![],
                budget: amounts(30),
            },
        },
        NOW,
    ));
    let attempt = match ok(i.admission_apply(
        Cmd::Claim {
            inputs: vec![offer(&w)],
            lease_ms: LEASE_MS,
        },
        NOW,
    )) {
        Res::Claimed { attempt: Some(a) } => *a,
        other => unreachable!("expected Claimed, got {other:?}"),
    };
    (w, attempt)
}

fn root_request(work_id: &ContentId, account: &ContentId) -> RootGrantRequest {
    RootGrantRequest {
        ceiling: ok(AuthorityCeiling::new(
            AUTH_GEN,
            CeilingTerms {
                audience: test_support::AUDIENCE.into(),
                project: "kiln".into(),
                work: Some(work_id.clone()),
                operations: BTreeSet::from([
                    Op::ClaimRenew,
                    Op::Dispatch,
                    Op::CandidateSubmit,
                    Op::AttemptCancel,
                ]),
                not_after_s: NOT_AFTER_S,
                max_depth: 3,
                disclosure_routes: BTreeSet::new(),
            },
        )),
        root_policy_ref: "policy:kiln".into(),
        holder: COORDINATOR_DID.into(),
        incarnation: None,
        budget: GrantBudget {
            parent_budget_identity: id("root-policy-budget"),
            account: account.clone(),
        },
    }
}

fn child_request(
    work_id: &ContentId,
    holder: &str,
    parent_account: &ContentId,
    child_account: &ContentId,
    ops: BTreeSet<Op>,
) -> ChildGrantRequest {
    ChildGrantRequest {
        terms: CeilingTerms {
            audience: test_support::AUDIENCE.into(),
            project: "kiln".into(),
            work: Some(work_id.clone()),
            operations: ops,
            not_after_s: NOT_AFTER_S - 10,
            max_depth: 1,
            disclosure_routes: BTreeSet::new(),
        },
        holder: holder.into(),
        incarnation: Some(id(holder)),
        budget: GrantBudget {
            parent_budget_identity: parent_account.clone(),
            account: child_account.clone(),
        },
    }
}

fn transfer_child_account(
    i: &mut Implementer,
    parent_account: &ContentId,
    child_account: &ContentId,
    attempt: &ImplementationAttempt,
    quota: u64,
) {
    ok(i.admission_apply(
        Cmd::Transfer {
            parent: parent_account.clone(),
            child: BudgetAccount {
                identity: child_account.clone(),
                parent: Some(parent_account.clone()),
                parent_attempt: Some(ClaimRef {
                    attempt: attempt.identity.clone(),
                    fence: attempt.fence,
                }),
                campaign: a("a"),
                tenant: a("tenant"),
                deadline_ms: NOW + 400_000,
                available: amounts(quota),
                consumed: Amounts::new(),
                children: 0,
            },
        },
        NOW,
    ));
}

#[test]
fn legacy_authority_db_migrates_cleanly_to_grant_issuance_schema() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let _imp = open_local(&db);
    let auth_path = ok(authority_state_path(&db));

    // Create a DB with only the legacy 4 tables and an existing revocation row.
    {
        let conn = ok(Connection::open(&auth_path));
        ok(conn.execute_batch(
            "CREATE TABLE authority_replay (
                audience TEXT NOT NULL,
                generation INTEGER NOT NULL,
                token_ref BLOB NOT NULL,
                nonce BLOB NOT NULL,
                resource_digest BLOB NOT NULL,
                expires_ms INTEGER NOT NULL,
                PRIMARY KEY (audience, generation, token_ref, nonce)
            );
            CREATE TABLE authority_revocation (
                proof_ref BLOB PRIMARY KEY,
                revoked_at_ms INTEGER NOT NULL,
                revocation_generation INTEGER NOT NULL
            );
            CREATE TABLE authority_revocation_feed_view (
                feed TEXT PRIMARY KEY,
                revocation_generation INTEGER NOT NULL,
                observed_at_ms INTEGER NOT NULL
            );
            CREATE TABLE authority_decision (
                seq INTEGER PRIMARY KEY,
                record TEXT NOT NULL
            );",
        ));
        ok(conn.execute(
            "INSERT INTO authority_revocation (proof_ref, revoked_at_ms, revocation_generation)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![proof_ref(99).as_slice(), 1_000_i64, 4_i64],
        ));
    }

    // Reopen through SqliteAuthorityState: migrates tables without losing prior rows.
    let state = ok(SqliteAuthorityState::open(&auth_path));
    assert_eq!(ok(state.revocation_generation()), 4);
    let loaded = ok(state.load_grant_issuance_state());
    assert!(loaded.is_revoked(&proof_ref(99)));
}

#[test]
fn durable_grant_issuance_and_crash_recovery_survive_sqlite_reopen() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let mut i = open_local(&db);
    let (w, attempt) = setup_work_and_claim(&mut i, "durable-roundtrip");
    let work_id = ok(w.identity());

    // Intend root grant (before minting token).
    let intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };

    // Simulate process restart between intent and issuance.
    drop(i);
    let mut i = open_local(&db);

    // Reconciliation resumes the original intent rather than deriving a second one.
    let actions = match ok(i.admission_apply(
        Cmd::ReconcileGrants {
            authority_generation: AUTH_GEN,
        },
        NOW + 1_000,
    )) {
        Res::GrantsReconciled { actions } => actions,
        other => unreachable!("{other:?}"),
    };
    assert_eq!(actions, vec![GrantIssuanceAction::Issue { intent: intent.clone() }]);

    // Now issue the grant under that exact intent.
    let binding = match ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: intent.clone(),
            grant_ref: proof_ref(1),
        },
        NOW + 1_000,
    )) {
        Res::GrantIssued { binding, .. } => *binding,
        other => unreachable!("{other:?}"),
    };
    assert_eq!(binding.attempt, attempt.identity);

    // Replay of exact issuance is idempotent; conflicting proof or proof reuse fails.
    assert!(
        i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: intent.clone(),
                grant_ref: proof_ref(1),
            },
            NOW + 1_000,
        )
        .is_ok()
    );
    assert!(
        err(i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: intent.clone(),
                grant_ref: proof_ref(2),
            },
            NOW + 1_000,
        ))
        .contains("grant_issuance_duplicate")
    );

    // Restart again: AttemptLedger retains the binding and ReconcileGrants owes nothing.
    drop(i);
    let mut i = open_local(&db);
    let attempts = ok(i.attempts());
    assert_eq!(attempts.authority(&attempt.identity), Some(&binding));
    let actions = match ok(i.admission_apply(
        Cmd::ReconcileGrants {
            authority_generation: AUTH_GEN,
        },
        NOW + 2_000,
    )) {
        Res::GrantsReconciled { actions } => actions,
        other => unreachable!("{other:?}"),
    };
    assert!(actions.is_empty());

    let auth_path = ok(authority_state_path(&db));
    let slots = ok(ok(SqliteAuthorityState::open(&auth_path)).grant_slots_for_work(&work_id));
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].phase, GrantLifecyclePhase::Issued);
    assert_eq!(slots[0].grant_ref, Some(proof_ref(1)));
}

#[test]
fn concurrent_child_budget_cap_enforces_exhaustion_release_and_non_escalation() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let mut i = open_local(&db);
    let (w, attempt) = setup_work_and_claim(&mut i, "child-concurrency");
    let work_id = ok(w.identity());

    let root_intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: root_intent.clone(),
            grant_ref: proof_ref(10),
        },
        NOW,
    ));

    transfer_child_account(&mut i, &id("a"), &id("c1"), &attempt, 10);
    transfer_child_account(&mut i, &id("a"), &id("c2"), &attempt, 10);
    transfer_child_account(&mut i, &id("a"), &id("c3"), &attempt, 10);

    // Role escalation (Coding -> Verification) is refused and logged redacted.
    let escalation_err = err(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("a"),
                &id("c1"),
                BTreeSet::from([Op::VerificationAdmit]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW,
    ));
    assert!(escalation_err.contains("role_escalation"), "{escalation_err}");

    // Budget conflict (claiming child c1 from a foreign parent budget) is refused.
    let foreign_budget_err = err(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("foreign-budget"),
                &id("c1"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW,
    ));
    assert!(foreign_budget_err.contains("budget_unbound"), "{foreign_budget_err}");

    // Admit and issue 2 concurrent children (cap = 2).
    let child1 = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("a"),
                &id("c1"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: child1.clone(),
            grant_ref: proof_ref(11),
        },
        NOW,
    ));

    let child2 = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_B,
                &id("a"),
                &id("c2"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    // Note: child2 is still Pending (intended, not yet issued) and ALREADY counts toward cap!

    // 3rd child must be refused due to concurrency_full.
    let cap_err = err(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_C,
                &id("a"),
                &id("c3"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW,
    ));
    assert!(cap_err.contains("concurrency"), "{cap_err}");

    // Reopen across restart: cap still holds, and releasing child1 frees a slot!
    drop(i);
    let mut i = open_local(&db);
    ok(i.admission_apply(Cmd::ReleaseGrant { intent: child1 }, NOW + 1_000));
    let child3 = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent,
            request: Box::new(child_request(
                &work_id,
                WORKER_C,
                &id("a"),
                &id("c3"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW + 1_000,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    // Reusing proof_ref(11) for child3 must be refused.
    assert!(
        err(i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: child3,
                grant_ref: proof_ref(11),
            },
            NOW + 1_000,
        ))
        .contains("grant_issuance_reused_proof")
    );
    // child2 can still issue with its own proof.
    assert!(
        i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: child2,
                grant_ref: proof_ref(12),
            },
            NOW + 1_000,
        )
        .is_ok()
    );
}

#[test]
fn parent_revocation_cascades_across_restart_and_blocks_verified_authority() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let config = write_config(dir.path(), &fixture, &[]);
    let mut i = open_verified(&db, &config);
    let (w, attempt) = setup_work_and_claim(&mut i, "revocation-cascade");
    let work_id = ok(w.identity());

    let root_intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    let root_proof = test_support::token_ref(fixture.grant(kiln_core::implementation::grant::GrantRole::Coding));
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: root_intent.clone(),
            grant_ref: root_proof,
        },
        NOW,
    ));

    transfer_child_account(&mut i, &id("a"), &id("c1"), &attempt, 10);
    transfer_child_account(&mut i, &id("a"), &id("c2"), &attempt, 10);

    let issued_child = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("a"),
                &id("c1"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 4,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: issued_child.clone(),
            grant_ref: proof_ref(21),
        },
        NOW,
    ));

    let pending_child = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_B,
                &id("a"),
                &id("c2"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 4,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };

    // Revoke the parent grant.
    ok(i.admission_apply(
        Cmd::RevokeGrant {
            grant_ref: root_proof,
            revocation_generation: 5,
        },
        NOW + 1_000,
    ));

    // Restart completely with fresh verified Implementer + SqliteRevocationView.
    drop(i);
    let mut i = open_verified(&db, &config);

    // Both parent and issued child are durably revoked in SqliteRevocationView!
    let auth_path = ok(authority_state_path(&db));
    let feed_key = kiln_implementer::authority_state::revocation_feed_key(&dir.path().join("revocations.json"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let rev_view = ok(SqliteRevocationView::open(&auth_path, feed_key, MAX_AGE_MS));
    assert_eq!(
        ok(rev_view.is_revoked(&ProofReference::new(root_proof.to_vec()), NOW + 2_000)),
        true
    );
    assert_eq!(
        ok(rev_view.is_revoked(&ProofReference::new(proof_ref(21).to_vec()), NOW + 2_000)),
        true
    );

    // Pending child cannot be issued; new child cannot be admitted; verified claim/renew/request fail!
    assert!(
        err(i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: pending_child,
                grant_ref: proof_ref(22),
            },
            NOW + 2_000,
        ))
        .contains("revoked or fenced")
    );
    assert!(
        err(i.admission_apply(
            Cmd::IntendChildGrant {
                parent_intent: root_intent,
                request: Box::new(child_request(
                    &work_id,
                    WORKER_C,
                    &id("a"),
                    &id("c1"),
                    BTreeSet::from([Op::Dispatch]),
                )),
                delegator: COORDINATOR_DID.into(),
                delegate_permitted: true,
                authority_generation: AUTH_GEN,
                max_concurrent_children: 4,
            },
            NOW + 2_000,
        ))
        .contains("revoked or fenced")
    );
    assert!(i.renew(&attempt.identity, attempt.fence, NOW + 2_000, LEASE_MS).is_err());
    assert!(
        i.admission_apply(
            Cmd::Request {
                reservation: reservation(&w, &attempt),
                inputs: vec![offer(&w)],
            },
            NOW + 2_000,
        )
        .is_err()
    );
}

#[test]
fn unknown_termination_holds_capacity_and_revokes_grant_until_independently_reconciled() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let mut i = open_local(&db);
    let (w, attempt) = setup_work_and_claim(&mut i, "unknown-reconcile");
    let work_id = ok(w.identity());

    let root_intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: root_intent.clone(),
            grant_ref: proof_ref(30),
        },
        NOW,
    ));

    transfer_child_account(&mut i, &id("a"), &id("c1"), &attempt, 10);
    let child_intent = match ok(i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent.clone(),
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("a"),
                &id("c1"),
                BTreeSet::from([Op::Dispatch]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 1,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: child_intent.clone(),
            grant_ref: proof_ref(31),
        },
        NOW,
    ));

    // Reserve & begin transport, then fail with Unknown.
    let plan = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: work_id.clone(),
        work_revision: w.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: w.repository.clone(),
        base: w.base.clone(),
        workspace: DispatchWorkspace {
            identity: id("ws"),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: ProviderProfileId::new("serving-v1").unwrap_or_else(|_| unreachable!()),
        permitted_paths: w.permitted_paths.clone(),
        permitted_effects: w.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 60,
        token_budget: 1_000,
        invocation_budget: 1,
        expected_output: id("expected-output"),
        execution: None,
    };
    let reserved = ok(i.reserve_dispatch_plan(&w, &attempt.identity, attempt.fence, NOW, &plan));
    ok(i.bind_campaign_request(&w, &attempt.identity, attempt.fence, NOW, b"{}"));
    ok(i.begin_dispatch_transport(&attempt.identity, attempt.fence, NOW, &reserved.plan, None));
    ok(i.fail(
        &attempt.identity,
        attempt.fence,
        NOW + 500,
        FailureClass::Unknown,
        "lost-worker-response",
    ));

    // Reopen after restart: both root and child are UnknownFenced & revoked in authority_revocation.
    drop(i);
    let mut i = open_local(&db);
    let auth_path = ok(authority_state_path(&db));
    let auth_state = ok(SqliteAuthorityState::open(&auth_path));
    let gs = ok(auth_state.load_grant_issuance_state());
    assert_eq!(gs.lifecycle_phase(&root_intent), GrantLifecyclePhase::UnknownFenced);
    assert_eq!(gs.lifecycle_phase(&child_intent), GrantLifecyclePhase::UnknownFenced);
    assert!(gs.is_revoked(&proof_ref(30)));
    assert!(gs.is_revoked(&proof_ref(31)));
    assert_eq!(
        gs.child_concurrency(&ok(gs.binding(&root_intent).ok_or(())).identity)
            .unknown_children,
        1
    );

    // ReleaseGrant is refused while UnknownFenced.
    assert!(
        err(i.admission_apply(
            Cmd::ReleaseGrant {
                intent: child_intent.clone(),
            },
            NOW + 1_000,
        ))
        .contains("unknown")
    );

    // ReconcileGrants owes NO new issuance for unknown-terminated grants.
    let actions = match ok(i.admission_apply(
        Cmd::ReconcileGrants {
            authority_generation: AUTH_GEN,
        },
        NOW + 1_000,
    )) {
        Res::GrantsReconciled { actions } => actions,
        other => unreachable!("{other:?}"),
    };
    assert!(actions.is_empty());

    // Now independently observe termination via reconcile_unknown.
    let term_a = ok(ExternalRef::new("lattice", "termination", "run-1-stopped"));
    ok(i.reconcile_unknown(&attempt.identity, attempt.fence, &term_a));

    // Conflicting second termination fails closed.
    let term_b = ok(ExternalRef::new("lattice", "termination", "run-1-other"));
    assert!(i.reconcile_unknown(&attempt.identity, attempt.fence, &term_b).is_err());

    // After resolution, UnknownResolved keeps proofs revoked AND releases the held child slot!
    let gs_after = ok(auth_state.load_grant_issuance_state());
    assert_eq!(
        gs_after.lifecycle_phase(&root_intent),
        GrantLifecyclePhase::UnknownResolved
    );
    assert_eq!(
        gs_after.lifecycle_phase(&child_intent),
        GrantLifecyclePhase::UnknownResolved
    );
    assert!(gs_after.is_revoked(&proof_ref(30)));
    assert!(gs_after.is_revoked(&proof_ref(31)));
    assert_eq!(
        gs_after
            .child_concurrency(&ok(gs_after.binding(&root_intent).ok_or(())).identity)
            .unknown_children,
        0
    );
}

#[test]
fn stale_lease_and_generation_move_refuse_issuance_and_abandon_cleanly() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let mut i = open_local(&db);
    let (w, attempt) = setup_work_and_claim(&mut i, "stale-issuance");
    let work_id = ok(w.identity());

    let intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };

    // Step past lease expiry without calling recover first: RecordIssuedGrant must refuse.
    let after_lease = attempt.lease_until_ms + 1_000;
    assert!(
        err(i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent: intent.clone(),
                grant_ref: proof_ref(40),
            },
            after_lease,
        ))
        .contains("no longer current")
    );

    // ReconcileGrants abandons the lapsed intent so it can never be minted later.
    let actions = match ok(i.admission_apply(
        Cmd::ReconcileGrants {
            authority_generation: AUTH_GEN + 1,
        },
        after_lease,
    )) {
        Res::GrantsReconciled { actions } => actions,
        other => unreachable!("{other:?}"),
    };
    assert_eq!(
        actions,
        vec![GrantIssuanceAction::Abandon {
            intent: intent.clone()
        }]
    );
    assert!(
        err(i.admission_apply(
            Cmd::RecordIssuedGrant {
                intent,
                grant_ref: proof_ref(40),
            },
            NOW + 100,
        ))
        .contains("revoked or fenced")
    );
}

#[test]
fn redacted_admission_explanation_exposes_denial_and_lineage_without_secrets_or_paths() {
    let dir = ok(tempfile::tempdir());
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let config = write_config(dir.path(), &fixture, &[]);
    let mut i = open_verified(&db, &config);
    let (w, attempt) = setup_work_and_claim(&mut i, "redacted-explanation");
    let work_id = ok(w.identity());

    let root_intent = match ok(i.admission_apply(
        Cmd::IntendRootGrant {
            work: work_id.clone(),
            campaign_revision: None,
            request: Box::new(root_request(&work_id, &id("a"))),
            authority_generation: AUTH_GEN,
        },
        NOW,
    )) {
        Res::GrantIntended { intent } => intent,
        other => unreachable!("{other:?}"),
    };
    let root_proof = proof_ref(50);
    ok(i.admission_apply(
        Cmd::RecordIssuedGrant {
            intent: root_intent.clone(),
            grant_ref: root_proof,
        },
        NOW,
    ));

    // Trigger a denied child grant (role escalation) and a parent revocation.
    transfer_child_account(&mut i, &id("a"), &id("c1"), &attempt, 10);
    let _ = i.admission_apply(
        Cmd::IntendChildGrant {
            parent_intent: root_intent,
            request: Box::new(child_request(
                &work_id,
                WORKER_A,
                &id("a"),
                &id("c1"),
                BTreeSet::from([Op::PublicationPublish]),
            )),
            delegator: COORDINATOR_DID.into(),
            delegate_permitted: true,
            authority_generation: AUTH_GEN,
            max_concurrent_children: 2,
        },
        NOW + 100,
    );
    ok(i.admission_apply(
        Cmd::RevokeGrant {
            grant_ref: root_proof,
            revocation_generation: 8,
        },
        NOW + 200,
    ));
    drop(i);

    // Read via open_read_only to verify read-only #60 observation safety.
    let mut reader = ok(Implementer::open_read_only(&db, enrollment()));
    let explained = ok(explain_work_admission(&mut reader, &work_id, NOW + 300, AUTH_GEN, 2));
    assert_eq!(
        explained.grant_authority.phase,
        GrantLifecyclePhase::Revoked
    );
    assert_eq!(
        explained.grant_authority.denial_reason.as_deref(),
        Some("grant_revoked")
    );
    assert!(
        explained
            .recent_denials
            .iter()
            .any(|d| d.reason == "role_escalation")
    );
    assert_eq!(
        explained
            .admission
            .as_ref()
            .map(|a| a.reason),
        Some(AdmissionReason::Denied)
    );

    let json = ok(serde_json::to_string(&explained));
    let raw_token = fixture.grant(kiln_core::implementation::grant::GrantRole::Coding);
    let dir_str = dir.path().to_string_lossy();
    for forbidden in [
        raw_token,
        dir_str.as_ref(),
        "coordinator.seed",
        "verifier.seed",
        "publisher.seed",
        "eyJ",
        "/tmp",
        "/home",
        COORDINATOR_DID,
    ] {
        assert!(
            !json.contains(forbidden),
            "redacted explanation leaked forbidden material `{forbidden}` in {json}"
        );
    }
}
