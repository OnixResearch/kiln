//! End-to-end verified profile: the profile is selected from exported
//! configuration files (C1) and real ledger operations run through the
//! durable authority state (C3) beside the ledger.

#[path = "authority_common/mod.rs"]
mod common;

use common::*;
use kiln_adapter_basalt::test_support::{self, Fixture};
use kiln_core::implementation::authority::{AuthorityProvenance, ProtectedOperation};
use kiln_core::implementation::grant::GrantRole;
use kiln_implementer::Implementer;
use kiln_implementer::admission::AdmissionCommand as Cmd;
use kiln_implementer::authority_config::ImplementationAuthorityConfig;
use kiln_implementer::authority_state::{SqliteReplayLedger, authority_state_path};

const LEASE_MS: u64 = 400_000;

fn decisions(i: &Implementer) -> Vec<kiln_core::implementation::authority::AuthorityDecision> {
    i.persisted_authority_decisions(0..u64::MAX)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .into_iter()
        .map(|(_, decision)| decision)
        .collect()
}

fn replay_rows(db: &std::path::Path) -> u64 {
    SqliteReplayLedger::open(&authority_state_path(db))
        .and_then(|ledger| ledger.recorded())
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn no_attempts(i: &mut Implementer) -> bool {
    i.attempts()
        .unwrap_or_else(|error| unreachable!("{error}"))
        .attempts()
        .next()
        .is_none()
}

#[test]
// r[verify kiln.implementation.authority.config]
// r[verify kiln.implementation.authority.selection]
// r[verify kiln.implementation.authority.state]
fn verified_profile_from_config_admits_coordinator_operations_through_durable_state() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    let w = work("verified-end-to-end");
    let mut i = open_verified(&db, &config);
    queue(&mut i, &w);
    let attempt = claim(&mut i, &w).unwrap_or_else(|error| unreachable!("{error}"));
    let r = reservation(&w, &attempt);
    i.admission_apply(
        Cmd::Request {
            reservation: r.clone(),
            inputs: vec![offer(&w)],
        },
        T,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    confirm(&mut i, &r);
    i.admission_before_transport(&attempt.identity, attempt.fence, &[offer(&w)], T + 3)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.renew(&attempt.identity, attempt.fence, T + 4, LEASE_MS)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.cancel(&attempt.identity, attempt.fence, T + 5, "operator stop")
        .unwrap_or_else(|error| unreachable!("{error}"));
    drop(i);

    // A fresh process selects the same profile from the same files and reads
    // the decisions and consumed invocations the first one persisted.
    let i = open_verified(&db, &config);
    let persisted = decisions(&i);
    assert!(
        persisted.iter().all(|decision| decision.allowed
            && decision.provenance == AuthorityProvenance::VerifiedUcan
            && decision.audience == test_support::AUDIENCE
            && decision.generation == test_support::GENERATION),
        "{persisted:?}"
    );
    for operation in [
        ProtectedOperation::ClaimAcquire,
        ProtectedOperation::Dispatch,
        ProtectedOperation::ClaimRenew,
        ProtectedOperation::AttemptCancel,
    ] {
        assert!(
            persisted
                .iter()
                .any(|decision| decision.operation == operation),
            "{operation:?} decision persisted"
        );
    }
    // One consumed holder invocation per admission: claim, the in-transaction
    // dispatch request, the pre-transport dispatch, renew and cancel.
    assert_eq!(replay_rows(&db), 5);
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
fn verified_profile_admits_claim_next_as_an_exact_claim_acquire() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    let w = work("verified-claim-next");
    let mut i = open_verified(&db, &config);
    i.admit(&w, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempt = i
        .claim_next(T, LEASE_MS)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("claim"));
    let last = decisions(&i)
        .last()
        .cloned()
        .unwrap_or_else(|| unreachable!("claim decision"));
    assert!(last.allowed);
    assert_eq!(last.operation, ProtectedOperation::ClaimAcquire);
    assert_eq!(last.provenance, AuthorityProvenance::VerifiedUcan);
    assert_eq!(replay_rows(&db), 1);
    assert_eq!(
        i.attempt(&attempt.identity)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .map(|stored| stored.fence),
        Some(attempt.fence)
    );
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
fn absent_profile_refuses_claim_next_and_commits_no_attempt() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let mut i =
        Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("absent-claim-next"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let error = i
        .claim_next(T, LEASE_MS)
        .err()
        .unwrap_or_else(|| unreachable!("absent profile must refuse the claim"));
    assert!(error.contains("no authority profile selected"), "{error}");
    let deferred = i
        .claim_next_deferred(T, LEASE_MS, &|_| None)
        .err()
        .unwrap_or_else(|| unreachable!("absent profile must refuse the deferred claim"));
    assert!(
        deferred.contains("no authority profile selected"),
        "{deferred}"
    );
    assert!(no_attempts(&mut i), "a refused claim commits nothing");
}

#[test]
// r[verify kiln.implementation.authority.state]
fn revoked_grant_in_the_configured_feed_refuses_the_claim_and_commits_nothing() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let revoked = test_support::token_ref(fixture.grant(GrantRole::Coding));
    let config = write_config(dir.path(), &fixture, &[revoked]);
    let w = work("revoked-claim");
    let mut i = open_verified(&db, &config);
    queue(&mut i, &w);
    assert!(claim(&mut i, &w).is_err(), "revoked grant must not claim");
    i.admit(&work("revoked-claim-next"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(i.claim_next(T, LEASE_MS).is_err());
    assert!(no_attempts(&mut i), "a refused claim commits nothing");
    let persisted = decisions(&i);
    assert!(!persisted.is_empty());
    assert!(
        persisted
            .iter()
            .all(|decision| !decision.allowed
                && decision.operation == ProtectedOperation::ClaimAcquire),
        "{persisted:?}"
    );
    assert_eq!(replay_rows(&db), 0, "a denial consumes no invocation");
}

#[test]
// r[verify kiln.implementation.authority.state]
fn stale_revocation_view_refuses_the_claim_and_commits_nothing() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    let mut i = open_verified(&db, &config);
    i.admit(&work("stale-view"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    // The feed was imported at `T`; the view is older than its maximum age
    // while the grant itself is still inside its validity window. Authority
    // time has whole-second resolution, so step past the age by one second.
    let stale = T + MAX_AGE_MS + 1_000;
    assert!(stale / 1_000 < test_support::EXPIRES_S);
    assert!(i.claim_next(stale, LEASE_MS).is_err());
    assert!(no_attempts(&mut i), "a refused claim commits nothing");
    assert_eq!(replay_rows(&db), 0);
}

#[test]
// r[verify kiln.implementation.authority.config]
fn expired_or_untrusted_grants_are_never_selected_from_config() {
    for (name, fixture, now_ms) in [
        (
            "expired",
            Fixture::direct(),
            test_support::EXPIRED_AT_S * 1_000,
        ),
        (
            "untrusted",
            Fixture::direct_from(test_support::UNTRUSTED_ROOT_SEED, test_support::EXPIRES_S),
            T,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
        let db = dir.path().join("ledger.db");
        let config = write_config(dir.path(), &fixture, &[]);
        let mut i =
            Implementer::open(&db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
        i.set_clock_ms(now_ms);
        let error = i
            .select_authority(
                &ImplementationAuthorityConfig::load(&config)
                    .unwrap_or_else(|error| unreachable!("{error}")),
            )
            .err()
            .unwrap_or_else(|| unreachable!("{name} grant must not be selected"));
        assert!(error.contains("no current grant"), "{name}: {error}");
        // The refused selection left the absent profile, which denies.
        i.admit(&work(name), None)
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(i.claim_next(T, LEASE_MS).is_err(), "{name}");
        assert!(no_attempts(&mut i), "{name}");
    }
}
