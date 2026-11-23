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

fn open_at(
    db: &std::path::Path,
    config: &std::path::Path,
    now_ms: u64,
) -> Result<Implementer, String> {
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
    assert_eq!(
        replay_rows(&link),
        1,
        "the invocation consumed through the real path"
    );

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
    // An older generation adds no revocation here and never moves the view back.
    write_feed(dir.path(), 1, T, &[]);
    drop(open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}")));
    assert_eq!(view_generation(&db), 2);
    assert_eq!(revoked_rows(&db), 1);
}

/// Feed `feed`'s recorded `(generation, observed_at_ms)` and the revoked
/// references of one authority file.
fn view_and_revocations(path: &std::path::Path, feed: &str) -> ((u64, u64), Vec<Vec<u8>>) {
    let connection =
        rusqlite::Connection::open(path).unwrap_or_else(|error| unreachable!("{error}"));
    let (generation, observed_at_ms): (i64, i64) = connection
        .query_row(
            "SELECT revocation_generation, observed_at_ms FROM authority_revocation_feed_view WHERE feed = ?1",
            [feed],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let unsigned =
        |value: i64| u64::try_from(value).unwrap_or_else(|error| unreachable!("{error}"));
    let view = (unsigned(generation), unsigned(observed_at_ms));
    let mut statement = connection
        .prepare("SELECT proof_ref FROM authority_revocation ORDER BY proof_ref")
        .unwrap_or_else(|error| unreachable!("{error}"));
    let revoked = statement
        .query_map([], |row| row.get(0))
        .and_then(Iterator::collect)
        .unwrap_or_else(|error| unreachable!("{error}"));
    (view, revoked)
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
// r[verify kiln.implementation.authority.state.scenario.revoked]
fn an_older_feed_never_moves_the_view_back_and_only_adds_revocations() {
    use kiln_implementer::authority_state::SqliteAuthorityState;
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let path = dir.path().join("ledger.db.authority");
    let state = SqliteAuthorityState::open(&path).unwrap_or_else(|error| unreachable!("{error}"));
    let (fresh, stale, later, other) = ([1; 32], [2; 32], [3; 32], [4; 32]);
    const FEED: &str = "/feeds/a.json";
    const OTHER: &str = "/feeds/b.json";
    let import = |feed: &str, generation, observed_at_ms, revoked: &[[u8; 32]]| {
        state
            .import_revocation_feed(feed, generation, observed_at_ms, revoked)
            .unwrap_or_else(|error| unreachable!("{error}"));
    };

    // Process B imports the refreshed generation-1 feed; process A then
    // writes the copy it read earlier: the fresh time stays, and A's extra
    // revocation is kept.
    import(FEED, 1, T + 30_000, &[fresh]);
    import(FEED, 1, T, &[stale]);
    assert_eq!(
        view_and_revocations(&path, FEED),
        ((1, T + 30_000), vec![fresh.to_vec(), stale.to_vec()])
    );
    // An equal pair is idempotent.
    import(FEED, 1, T + 30_000, &[fresh]);
    assert_eq!(
        view_and_revocations(&path, FEED),
        ((1, T + 30_000), vec![fresh.to_vec(), stale.to_vec()])
    );
    // A newer generation advances the view; an older one, even observed
    // later, does not move it back but still adds its revocation.
    import(FEED, 2, T + 40_000, &[]);
    import(FEED, 1, T + 50_000, &[later]);
    assert_eq!(
        view_and_revocations(&path, FEED),
        (
            (2, T + 40_000),
            vec![fresh.to_vec(), stale.to_vec(), later.to_vec()]
        )
    );
    // Another feed sharing the store keeps its own view and never touches
    // this one; its revocations join the one shared union.
    import(OTHER, 1, T + 90_000, &[other]);
    assert_eq!(
        view_and_revocations(&path, FEED),
        (
            (2, T + 40_000),
            vec![
                fresh.to_vec(),
                stale.to_vec(),
                later.to_vec(),
                other.to_vec()
            ]
        )
    );
    assert_eq!(view_and_revocations(&path, OTHER).0, (1, T + 90_000));
}

/// Two verified configurations on one ledger, so one authority file, each
/// with its own feed file; returns the ledger and both configuration paths.
fn two_feed_configs(dir: &std::path::Path) -> (std::path::PathBuf, [std::path::PathBuf; 2]) {
    let fixture = Fixture::direct();
    let configs = ["fresh", "stale"].map(|name| {
        let config_dir = dir.join(name);
        std::fs::create_dir_all(&config_dir).unwrap_or_else(|error| unreachable!("{error}"));
        write_config(&config_dir, &fixture, &[])
    });
    (dir.join("ledger.db"), configs)
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn a_stale_feed_never_borrows_another_feeds_freshness() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let (db, [fresh, stale]) = two_feed_configs(dir.path());
    // The second configuration's producer observed its feed past its age.
    write_feed(&dir.path().join("stale"), 1, T - MAX_AGE_MS - 1_000, &[]);
    // The first configuration imports its fresh feed into the shared file.
    drop(open_at(&db, &fresh, T).unwrap_or_else(|error| unreachable!("{error}")));
    let mut i = open_at(&db, &stale, T).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("borrowed-freshness"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS).is_err(),
        "a stale feed must deny although another feed on the ledger is fresh"
    );
    assert!(no_attempts(&mut i), "a refused claim commits nothing");
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn a_long_lived_authority_denies_once_its_own_feed_ages_out() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let (db, [fresh, stale]) = two_feed_configs(dir.path());
    // Both feeds are fresh at `T`; the second authority stays selected.
    let mut i = open_at(&db, &stale, T).unwrap_or_else(|error| unreachable!("{error}"));
    i.admit(&work("own-feed-fresh"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(T, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some()
    );
    // Its feed ages past `max_age_ms` while the first feed keeps refreshing.
    let later = T + MAX_AGE_MS + 1_000;
    assert!(later / 1_000 < test_support::EXPIRES_S);
    write_feed(&dir.path().join("fresh"), 1, later, &[]);
    drop(open_at(&db, &fresh, later).unwrap_or_else(|error| unreachable!("{error}")));
    i.set_clock_ms(later);
    i.admit(&work("own-feed-stale"), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        i.claim_next(later, LEASE_MS).is_err(),
        "an authority's own aged feed must deny however recently another feed refreshed"
    );
}

/// A verified configuration under `dir/name` whose revocation feed is the
/// configured path `feed`.
fn feed_config(dir: &std::path::Path, name: &str, feed: &str) -> std::path::PathBuf {
    let config_dir = dir.join(name);
    std::fs::create_dir_all(&config_dir).unwrap_or_else(|error| unreachable!("{error}"));
    let config = write_config(&config_dir, &Fixture::direct(), &[]);
    let mut value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&config).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    *value
        .pointer_mut("/profile/verified_ucan/revocation/path")
        .unwrap_or_else(|| unreachable!("revocation path")) = feed.into();
    std::fs::write(
        &config,
        serde_json::to_vec(&value).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    config
}

/// Feed directories `a` and `b` under `dir/feeds` and the symlinked
/// directory `dir/current -> a`.
fn rotating_feeds(dir: &std::path::Path) -> [std::path::PathBuf; 2] {
    let feeds = ["a", "b"].map(|name| dir.join("feeds").join(name));
    for feed in &feeds {
        std::fs::create_dir_all(feed).unwrap_or_else(|error| unreachable!("{error}"));
    }
    point_current(dir, &feeds[0]);
    feeds
}

/// Re-point `dir/current` at `target`, as a producer rotating feeds does.
fn point_current(dir: &std::path::Path, target: &std::path::Path) {
    let current = dir.join("current");
    if current.symlink_metadata().is_ok() {
        std::fs::remove_file(&current).unwrap_or_else(|error| unreachable!("{error}"));
    }
    std::os::unix::fs::symlink(target, &current).unwrap_or_else(|error| unreachable!("{error}"));
}

/// Every feed view row of the ledger's authority file: key and observation.
fn feed_rows(db: &std::path::Path) -> Vec<(String, i64)> {
    let path = authority_state_path(db).unwrap_or_else(|error| unreachable!("{error}"));
    let connection =
        rusqlite::Connection::open(path).unwrap_or_else(|error| unreachable!("{error}"));
    let mut statement = connection
        .prepare("SELECT feed, observed_at_ms FROM authority_revocation_feed_view ORDER BY feed")
        .unwrap_or_else(|error| unreachable!("{error}"));
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .and_then(Iterator::collect)
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn claims(i: &mut Implementer, name: &str, now_ms: u64) -> bool {
    i.set_clock_ms(now_ms);
    i.admit(&work(name), None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.claim_next(now_ms, LEASE_MS)
        .is_ok_and(|claimed| claimed.is_some())
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn rotating_a_symlinked_feed_directory_keeps_the_configured_feed_fresh() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let [a, b] = rotating_feeds(dir.path());
    write_feed(&a, 1, T, &[]);
    let configured = format!("{}/current/revocations.json", dir.path().display());
    let config = feed_config(dir.path(), "x", &configured);
    let mut i = open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(claims(&mut i, "before-rotation", T));

    // The producer publishes a fresh feed in `b` and re-points `current`;
    // another process imports it through the same configured path.
    let later = T + MAX_AGE_MS + 1_000;
    assert!(later / 1_000 < test_support::EXPIRES_S);
    write_feed(&b, 1, later, &[]);
    point_current(dir.path(), &b);
    drop(open_at(&db, &config, later).unwrap_or_else(|error| unreachable!("{error}")));
    assert!(
        claims(&mut i, "after-rotation", later),
        "a long-lived authority stays fresh when its configured feed directory is re-pointed"
    );
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn two_spellings_of_one_feed_file_keep_separate_views() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let [a, b] = rotating_feeds(dir.path());
    let through_current = format!("{}/current/revocations.json", dir.path().display());
    let direct = format!("{}/revocations.json", a.display());
    let x_config = feed_config(dir.path(), "x", &through_current);
    let y_config = feed_config(dir.path(), "y", &direct);

    // Y imports `a` while it is stale; the producer then refreshes `a` and X
    // imports it through `current -> a`. X's row never refreshes Y's.
    write_feed(&a, 1, T - MAX_AGE_MS - 1_000, &[]);
    let mut y = open_at(&db, &y_config, T).unwrap_or_else(|error| unreachable!("{error}"));
    write_feed(&a, 1, T, &[]);
    let mut x = open_at(&db, &x_config, T).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(claims(&mut x, "x-before", T));
    assert!(
        !claims(&mut y, "y-before", T),
        "Y's stale view denies although X just imported the same file fresh"
    );

    // After `current` is re-pointed at a fresh `b` and X's feed re-imported,
    // X stays fresh and Y, whose own view is still its stale import, denies.
    let later = T + MAX_AGE_MS + 1_000;
    write_feed(&b, 1, later, &[]);
    point_current(dir.path(), &b);
    drop(open_at(&db, &x_config, later).unwrap_or_else(|error| unreachable!("{error}")));
    assert!(claims(&mut x, "x-after", later));
    assert!(!claims(&mut y, "y-after", later));
    let keys: Vec<String> = feed_rows(&db).into_iter().map(|(key, _)| key).collect();
    assert_eq!(keys, {
        let mut expected = vec![direct, through_current];
        expected.sort();
        expected
    });
}

#[test]
// r[verify kiln.implementation.authority.state.scenario.stale]
fn the_view_key_is_the_configured_path_the_feed_was_read_from() {
    use kiln_implementer::authority_state::revocation_feed_key;
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let [a, _] = rotating_feeds(dir.path());
    write_feed(&a, 1, T - 5, &[]);
    // Repeated `/` normalize away; the symlinked directory is not resolved.
    let spelled = format!("{}//current///revocations.json", dir.path().display());
    let configured = format!("{}/current/revocations.json", dir.path().display());
    let config = feed_config(dir.path(), "x", &spelled);
    drop(open_at(&db, &config, T).unwrap_or_else(|error| unreachable!("{error}")));
    let observed = i64::try_from(T - 5).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(feed_rows(&db), vec![(configured, observed)]);

    // Relative, dot and empty paths are refused before anything is read.
    for refused in [
        "feeds/revocations.json",
        "/a/./b.json",
        "/a/../b.json",
        "/",
        "",
    ] {
        assert!(
            revocation_feed_key(std::path::Path::new(refused)).is_err(),
            "{refused}"
        );
    }
    // A final-component symlink is refused at read time.
    let link = dir.path().join("link.json");
    std::os::unix::fs::symlink(a.join("revocations.json"), &link)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let linked = feed_config(dir.path(), "linked", &link.display().to_string());
    assert!(open_at(&db, &linked, T).is_err());
}
