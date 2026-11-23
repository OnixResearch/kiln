//! Fence-at-commit interleavings and lost-response reconciliation under the
//! verified profile, with two connections on one ledger file.

#[path = "authority_common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use kiln_adapter_basalt::test_support::{CapturingSigner, Fixture, ReplayingSigner};
use kiln_core::fair_admission::ReservationState;
use kiln_core::implementation::SupersessionRule;
use kiln_core::implementation::authority::{
    NO_EXPECTED_TARGET, ProtectedOperation, payload_digest_v2,
};
use kiln_core::types::{ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::admission::AdmissionCommand as Cmd;
use kiln_implementer::authority_state::{SqliteReplayLedger, authority_state_path};

fn supersede_from_second_connection(
    db: &std::path::Path,
    w: &kiln_core::implementation::ImplementationWork,
) {
    let mut second =
        Implementer::open(db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    let mut newer = w.clone();
    newer.revision += 1;
    second
        .admit_with_supersession(&newer, None, SupersessionRule::AbortActive, T + 2)
        .unwrap_or_else(|error| unreachable!("{error}"));
}

fn request_and_confirm(
    i: &mut Implementer,
    w: &kiln_core::implementation::ImplementationWork,
) -> (
    kiln_core::implementation::ImplementationAttempt,
    kiln_core::fair_admission::OwnerReservation,
) {
    let attempt = claim(i, w).unwrap_or_else(|error| unreachable!("{error}"));
    let r = reservation(w, &attempt);
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(w)],
        },
        T,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    confirm(i, &r);
    (attempt, r)
}

#[test]
// r[verify kiln.implementation.authority.interleaving]
// r[verify kiln.implementation.authority.interleaving.scenario.after_check]
fn fence_change_between_authority_check_and_transport_commit_refuses_transport() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let w = work("interleave-after-check");
    let mut i =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    let armed = Rc::new(RefCell::new(false));
    let fired = Rc::new(RefCell::new(false));
    let hook_db = db.clone();
    let hook_work = w.clone();
    let (hook_armed, hook_fired) = (Rc::clone(&armed), Rc::clone(&fired));
    install(
        &mut i,
        durable_verified(&db, &fixture),
        Box::new(fixture.signer()),
        None,
        // Runs after the Dispatch revalidation succeeded (admission.rs
        // `authorize_and_revalidate`) and before the IMMEDIATE transport
        // transaction opens: another connection replaces the attempt.
        Some(Box::new(move |request| {
            if *hook_armed.borrow() && request.operation == ProtectedOperation::Dispatch {
                supersede_from_second_connection(&hook_db, &hook_work);
                *hook_fired.borrow_mut() = true;
            }
        })),
    );
    queue(&mut i, &w);
    let (attempt, r) = request_and_confirm(&mut i, &w);
    *armed.borrow_mut() = true;
    let error = i
        .admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], T + 3)
        .err()
        .unwrap_or_else(|| unreachable!("a replaced fence must refuse transport"));
    assert!(*fired.borrow());
    assert!(error.contains("stale attempt/fence"), "{error}");
    // The authority itself admitted the request: the refusal is the
    // commit-time fence re-read, not the credential.
    let last = i
        .authority_decisions()
        .last()
        .cloned()
        .unwrap_or_else(|| unreachable!("dispatch decision"));
    assert!(last.allowed);
    assert_eq!(last.operation, ProtectedOperation::Dispatch);
    // Nothing was dispatched: the owner grant stays committed, not unknown.
    let state = i
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        state
            .reservations
            .get(&r.operation)
            .unwrap_or_else(|| unreachable!("reservation"))
            .state,
        ReservationState::Committed
    );
}

#[test]
// r[verify kiln.implementation.authority.interleaving.scenario.inside_transaction]
fn fence_cannot_change_inside_the_admission_apply_transaction() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let w = work("interleave-inside-tx");
    let mut i =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    let attempts: Rc<RefCell<Vec<Result<(), String>>>> = Rc::new(RefCell::new(Vec::new()));
    let hook_attempts = Rc::clone(&attempts);
    let hook_db = db.clone();
    install(
        &mut i,
        durable_verified(&db, &fixture),
        Box::new(fixture.signer()),
        // Runs inside `admission_apply`'s IMMEDIATE transaction, at the
        // Dispatch authority check of `AdmissionCommand::Request`: a second
        // connection tries to take the writer lock to change the fence.
        Some(Box::new(move |request| {
            if request.operation != ProtectedOperation::Dispatch {
                return;
            }
            let second = rusqlite::Connection::open(&hook_db)
                .unwrap_or_else(|error| unreachable!("{error}"));
            second
                .busy_timeout(std::time::Duration::ZERO)
                .unwrap_or_else(|error| unreachable!("{error}"));
            hook_attempts.borrow_mut().push(
                second
                    .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
                    .map_err(|error| error.to_string()),
            );
        })),
        None,
    );
    queue(&mut i, &w);
    let (attempt, r) = request_and_confirm(&mut i, &w);
    let observed = attempts.borrow().clone();
    assert_eq!(observed.len(), 1, "one in-transaction dispatch check");
    let error = observed
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!())
        .err()
        .unwrap_or_else(|| unreachable!("a writer must not enter the admission transaction"));
    assert!(
        error.contains("locked") || error.contains("busy"),
        "{error}"
    );
    // The committed request is bound to the fence that held during the
    // transaction; a change committed afterwards refuses transport.
    let state = i
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let committed = state
        .reservations
        .get(&r.operation)
        .unwrap_or_else(|| unreachable!("reservation"));
    assert_eq!(committed.claim.fence, attempt.fence);
    supersede_from_second_connection(&db, &w);
    assert!(
        i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], T + 3)
            .err()
            .is_some_and(|error| error.contains("stale attempt/fence")
                || error.contains("attempt-not-current"))
    );
}

#[test]
// r[verify kiln.implementation.authority.reconcile]
// r[verify kiln.implementation.authority.state.scenario.replay]
// r[verify kiln.implementation.authority.reconcile.scenario.fresh_nonce]
fn reconciliation_reuses_the_dispatch_effect_identity_with_a_fresh_nonce() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let w = work("lost-response");
    let plan = id("dispatch-plan");
    let profile = ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!());

    // Process A: claim, reserve the dispatch effect and admit transport.
    let mut a =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    let (signer, captured) = CapturingSigner::new(Box::new(fixture.signer_from(1)));
    install(
        &mut a,
        durable_verified(&db, &fixture),
        Box::new(signer),
        None,
        None,
    );
    queue(&mut a, &w);
    let attempt = claim(&mut a, &w).unwrap_or_else(|error| unreachable!("{error}"));
    let dispatch = a
        .reserve_dispatch(&attempt.identity, attempt.fence, T, &plan, &profile)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let effect = dispatch.effect_id.as_str().to_string();
    let r = reservation(&w, &attempt);
    a.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        T,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    confirm(&mut a, &r);
    a.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], T + 2)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let original = captured
        .borrow()
        .last()
        .cloned()
        .unwrap_or_else(|| unreachable!("original dispatch invocation"));
    assert_eq!(original.operation, ProtectedOperation::Dispatch);
    assert_eq!(
        original.request_digest,
        payload_digest_v2(
            ProtectedOperation::Dispatch,
            None,
            &effect,
            NO_EXPECTED_TARGET,
            &[plan.as_str()]
        )
    );
    // No transport launch was recorded: the response to the admitted
    // dispatch is lost before the transport intent commits.
    let recorded = SqliteReplayLedger::open(&authority_state_path(&db))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let before = recorded
        .recorded()
        .unwrap_or_else(|error| unreachable!("{error}"));
    // The provider response is lost; the process dies.
    drop(a);

    // Process B presents the captured original invocation again: the durable
    // replay ledger refuses its nonce even though its effect is unchanged.
    let mut b =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    install(
        &mut b,
        durable_verified(&db, &fixture),
        Box::new(ReplayingSigner(original.clone())),
        None,
        None,
    );
    let observed = ExternalRef::new("lattice", "run", "run-1").unwrap_or_else(|_| unreachable!());
    assert!(
        b.reconcile_dispatch(&attempt.identity, attempt.fence, T + 3, &observed)
            .err()
            .is_some_and(|error| error.contains("credential-denied")),
        "a replayed nonce must not reconcile"
    );
    drop(b);

    // Process C reconciles with a fresh nonce for the original effect identity.
    let mut c =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    let (signer, captured) = CapturingSigner::new(Box::new(fixture.signer_from(100)));
    install(
        &mut c,
        durable_verified(&db, &fixture),
        Box::new(signer),
        None,
        None,
    );
    c.reconcile_dispatch(&attempt.identity, attempt.fence, T + 3, &observed)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let fresh = captured
        .borrow()
        .last()
        .cloned()
        .unwrap_or_else(|| unreachable!("reconciliation invocation"));
    assert_ne!(fresh.nonce, original.nonce);
    assert_eq!(
        fresh.request_digest,
        payload_digest_v2(
            ProtectedOperation::Dispatch,
            None,
            &effect,
            NO_EXPECTED_TARGET,
            &[
                observed.provider_namespace.as_str(),
                observed.native_kind.as_str(),
                observed.native_id.as_str()
            ]
        ),
        "reconciliation binds the original reservation effect, not a new one"
    );
    assert_eq!(
        recorded
            .recorded()
            .unwrap_or_else(|error| unreachable!("{error}")),
        before + 1,
        "only the fresh invocation consumed replay"
    );
}
