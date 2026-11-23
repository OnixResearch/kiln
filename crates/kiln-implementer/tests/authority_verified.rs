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
    authority_state_path(db)
        .and_then(|path| SqliteReplayLedger::open(&path))
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
// r[verify kiln.implementation.authority.coverage.entry_points.scenario.claim]
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
// r[verify kiln.implementation.authority.coverage.entry_points.scenario.claim]
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
// r[verify kiln.implementation.authority.state.scenario.revoked]
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
// r[verify kiln.implementation.authority.state.scenario.stale]
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
// r[verify kiln.implementation.authority.config.scenario.refused]
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
                &std::collections::BTreeSet::from([GrantRole::Coding]),
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

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn a_feed_observed_earlier_in_the_decision_second_admits() {
    // Decisions read time to the whole second and check the view at that
    // second's last millisecond. A feed observed at `T + 500` ms, imported
    // then and used at `T + 700` ms, is not dated after the decision.
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    write_feed(dir.path(), 1, T + 500, &[]);
    let mut i = open_at(&db, &config, T + 500).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("sub-second-view"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T + 700, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
}

fn set_max_age(config: &std::path::Path, max_age_ms: u64) {
    let mut document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(config).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    *document
        .pointer_mut("/profile/verified_ucan/revocation/max_age_ms")
        .unwrap_or_else(|| unreachable!("max_age_ms")) = max_age_ms.into();
    std::fs::write(
        config,
        serde_json::to_vec(&document).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn a_one_second_revocation_age_holds_at_every_instant_of_the_decision_second() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    set_max_age(&config, 1_000);
    let mut i = open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("one-second-fresh"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    // 999 ms after the observation the feed is fresh at every instant.
    assert!(
        i.claim_next(T + 999, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    // 1,999 ms after it the feed is older than its bound, although the
    // decision's whole-second time is only one second later.
    i.admit(&work("one-second-stale"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T + 1_999, LEASE_MS).is_err(),
        "a feed 1,999 ms old must not pass a 1,000 ms bound"
    );
}

#[test]
// r[verify kiln.implementation.authority.config]
fn a_sub_second_revocation_age_is_refused() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    set_max_age(&config, 999);
    let error = ImplementationAuthorityConfig::load(&config)
        .err()
        .unwrap_or_else(|| unreachable!("a 999 ms bound must be refused"));
    assert!(error.contains("max_age_ms"), "{error}");
    set_max_age(&config, 1_000);
    assert!(ImplementationAuthorityConfig::load(&config).is_ok());
}

fn open_at(db: &std::path::Path, config: &std::path::Path, now_ms: u64) -> Result<Implementer, String> {
    let mut i = Implementer::open(db, enrollment())?;
    i.set_clock_ms(now_ms);
    i.select_authority(
        &ImplementationAuthorityConfig::load(config)?,
        &std::collections::BTreeSet::from([GrantRole::Coding]),
    )?;
    Ok(i)
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn importing_an_unchanged_feed_never_makes_it_fresh() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    // The producer observed the feed long before this process imports it.
    write_feed(dir.path(), 1, T - MAX_AGE_MS - 1_000, &[]);
    let mut i = open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("old-feed"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS).is_err(),
        "a feed observed past its age must refuse although it was just imported"
    );
    assert!(no_attempts(&mut i), "a refused claim commits nothing");
    drop(i);

    // A feed fresh at `T` admits; reopening later with the same feed does not
    // refresh it, so a revocation published since cannot be missed.
    write_feed(dir.path(), 1, T, &[]);
    let mut i = open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    drop(i);
    let later = T + MAX_AGE_MS + 1_000;
    assert!(later / 1_000 < test_support::EXPIRES_S);
    let mut i = open_at(&db, &config, later).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("unchanged-feed"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(later, LEASE_MS).is_err(),
        "re-importing an unchanged feed must not refresh its age"
    );
    drop(i);

    // The producer observes again: the refreshed feed admits.
    write_feed(dir.path(), 1, later, &[]);
    let mut i = open_at(&db, &config, later).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(later, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn a_feed_observed_in_the_future_is_refused_at_selection() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    write_feed(dir.path(), 1, T + 1, &[]);
    let error = open_at(&db, &config, T)
        .err()
        .unwrap_or_else(|| unreachable!("a future-dated feed must not be imported"));
    assert!(error.contains("observed in the future"), "{error}");
    assert!(open_at(&db, &config, T + 1).is_ok());
}

fn revoked_rows(db: &std::path::Path) -> i64 {
    let path = authority_state_path(db).unwrap_or_else(|error| unreachable!("{error}"));
    rusqlite::Connection::open(path)
        .and_then(|connection| {
            connection.query_row("SELECT count(*) FROM authority_revocation", [], |row| {
                row.get(0)
            })
        })
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn view_generation(db: &std::path::Path) -> u64 {
    use kiln_implementer::authority_state::AuthorityStateStore as _;
    authority_state_path(db)
        .and_then(|path| kiln_implementer::authority_state::SqliteAuthorityState::open(&path))
        .and_then(|state| state.revocation_generation())
        .unwrap_or_else(|error| unreachable!("{error}"))
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.replay]
fn one_ledger_opened_through_a_symlink_shares_replay_and_revocation() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let link = dir.path().join("link.db");
    let fixture = Fixture::direct();
    let config = write_config(dir.path(), &fixture, &[]);
    let mut i = open_verified(&db, &config);
    i.admit(&work("through-the-ledger"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    drop(i);
    std::os::unix::fs::symlink(&db, &link).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(authority_state_path(&link), authority_state_path(&db));
    assert_eq!(replay_rows(&link), 1, "the invocation consumed through the real path");

    // An invocation consumed through the symlink lands in the same table.
    let mut i = open_verified(&link, &config);
    i.admit(&work("through-the-symlink"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    drop(i);
    assert_eq!(replay_rows(&db), 2);

    // A revocation imported through the symlink is the real path's view.
    let grant = test_support::token_ref(fixture.grant(GrantRole::Coding));
    write_feed(dir.path(), 2, T, &[hex(&grant)]);
    drop(open_at(&link, &config, T).unwrap_or_else(|error| unreachable!("{error}")));
    assert_eq!(view_generation(&db), 2);
    assert_eq!(revoked_rows(&db), 1);
    assert!(
        !dir.path().join("link.db.authority").exists(),
        "no second authority store beside the symlink"
    );
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.revoked]
fn a_feed_with_a_malformed_reference_imports_nothing() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let config = write_config(dir.path(), &fixture, &[]);
    drop(open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}")));
    assert_eq!(view_generation(&db), 1);
    let grant = hex(&test_support::token_ref(fixture.grant(GrantRole::Coding)));

    // A valid reference followed by a malformed one: selection refuses and
    // neither the valid revocation nor the newer generation is written.
    write_feed(
        dir.path(),
        2,
        T,
        &[grant.clone(), "not-a-revoked-reference".into()],
    );
    let error = open_at(&db, &config, T)
        .err()
        .unwrap_or_else(|| unreachable!("a malformed feed must be refused"));
    assert!(error.contains("revoked reference"), "{error}");
    assert_eq!(revoked_rows(&db), 0);
    assert_eq!(view_generation(&db), 1);
    // The coordinator grant was never revoked, so a valid feed still admits.
    write_feed(dir.path(), 1, T, &[]);
    let mut i = open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("malformed-feed"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    drop(i);

    // The well-formed feed imports every reference and its generation together.
    write_feed(dir.path(), 2, T, &[grant]);
    drop(open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}")));
    assert_eq!(revoked_rows(&db), 1);
    assert_eq!(view_generation(&db), 2);
    // An older generation is then refused without a write.
    write_feed(dir.path(), 1, T, &[]);
    assert!(open_at(&db, &config, T).is_err());
    assert_eq!(view_generation(&db), 2);
}
