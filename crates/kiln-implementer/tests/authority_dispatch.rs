//! Dispatch authority at every reservation and launch step, through the
//! durable store: the fleet launch (`fleet_bind_session` and its launch
//! recheck), `reserve_dispatch`, `reserve_dispatch_plan`, the credential
//! pre-check `authorize_dispatch_plan` and the CLI `start`
//! (`start_execution`). Admitted principals reserve and launch; every denial
//! commits nothing, starts nothing and consumes no invocation.

#[path = "authority_common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use common::*;
use kiln_adapter_basalt::test_support::{self, Fixture};
use kiln_core::fleet::Placement;
use kiln_core::implementation::SupersessionRule;
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityDecision, AuthorityProfile, AuthorityProvenance, AuthorityRequest,
    AuthoritySelection, AuthoritySessionId, LocalOperatorAuthority, ProtectedAuthority,
    ProtectedOperation,
};
use kiln_core::implementation::grant::GrantRole;
use kiln_core::implementation::{
    AttemptPhase, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, ImplementationAttempt,
    ImplementationDispatchPlan, ImplementationWork, WorkspaceIsolation, admit_dispatch_plan,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::authority_state::{SqliteReplayLedger, authority_state_path};

/// Outlives the stale-view step, so only the revocation view is stale there.
const LEASE_MS: u64 = 2_000_000;
/// One second past the revocation view's maximum age, inside the grant window.
const STALE_MS: u64 = T + MAX_AGE_MS + 1_000;

fn ok<V>(result: Result<V, String>) -> V {
    result.unwrap_or_else(|error| unreachable!("{error}"))
}

fn based_work(name: &str) -> ImplementationWork {
    let mut work = work(name);
    work.base =
        Some(ExternalRef::new("git", "commit", &"a".repeat(40)).unwrap_or_else(|_| unreachable!()));
    work
}

fn plan(work: &ImplementationWork, attempt: &ImplementationAttempt) -> ImplementationDispatchPlan {
    ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: id("workspace"),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: ProviderProfileId::new("lattice-implementation")
            .unwrap_or_else(|_| unreachable!()),
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 5,
        token_budget: 1_000,
        invocation_budget: 1,
        expected_output: id("expected-output"),
        execution: None,
    }
}

fn workflow() -> ExternalRef {
    ExternalRef::new("lattice", "run", "run-1").unwrap_or_else(|_| unreachable!())
}

fn replay_rows(db: &Path) -> u64 {
    ok(SqliteReplayLedger::open(&authority_state_path(db)).and_then(|ledger| ledger.recorded()))
}

fn decisions(db: &Path) -> Vec<AuthorityDecision> {
    ok(Implementer::open(db, enrollment())
        .and_then(|i| i.persisted_authority_decisions(0..u64::MAX)))
    .into_iter()
    .map(|(_, decision)| decision)
    .collect()
}

/// Every durable record a reservation or launch step writes for an attempt.
#[derive(Debug, PartialEq)]
struct Records {
    reservation: bool,
    plan: bool,
    phase: AttemptPhase,
    workflow: Option<ExternalRef>,
    sessions: usize,
}

fn records(db: &Path, attempt: &ContentId) -> Records {
    // Reading needs no authority; the absent profile is selected.
    let mut i = ok(Implementer::open(db, enrollment()));
    let stored = ok(i.attempt(attempt)).unwrap_or_else(|| unreachable!("attempt"));
    Records {
        reservation: stored.dispatch.is_some(),
        plan: ok(i.dispatch_plan(attempt)).is_some(),
        phase: stored.phase,
        workflow: stored.workflow,
        sessions: ok(i.fleet_state()).sessions.sessions.len(),
    }
}

/// A ledger with one attempt claimed under the verified profile.
struct Ledger {
    dir: tempfile::TempDir,
    db: PathBuf,
    fixture: Fixture,
    config: PathBuf,
    work: ImplementationWork,
    attempt: ImplementationAttempt,
}

impl Ledger {
    fn claimed(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
        let db = dir.path().join("ledger.db");
        let fixture = Fixture::direct();
        let config = write_config(dir.path(), &fixture, &[]);
        let work = based_work(name);
        let mut i = open_verified(&db, &config);
        ok(i.admit(&work, None));
        let attempt = ok(i.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!("claim"));
        Self {
            dir,
            db,
            fixture,
            config,
            work,
            attempt,
        }
    }

    fn plan(&self) -> ImplementationDispatchPlan {
        plan(&self.work, &self.attempt)
    }

    /// Reserve the plan under the verified profile, as the fleet tick and
    /// the Lattice worker do before they launch.
    fn reserved(self) -> Self {
        let mut i = open_verified(&self.db, &self.config);
        ok(i.reserve_dispatch_plan(
            &self.work,
            &self.attempt.identity,
            self.attempt.fence,
            T,
            &self.plan(),
        ));
        self
    }
}

#[derive(Clone, Copy, Debug)]
enum Step {
    /// The Lattice worker's pre-check before it reads its credential.
    PreCheck,
    /// `reserve-plan`, the fleet tick and the Lattice worker.
    ReservePlan,
    /// `reserve`.
    Reserve,
    /// The fleet tick's launch record.
    FleetBind,
    /// `start` and the recorded start of every launched workflow.
    Start,
}

impl Step {
    const ALL: [Self; 5] = [
        Self::PreCheck,
        Self::ReservePlan,
        Self::Reserve,
        Self::FleetBind,
        Self::Start,
    ];

    /// Launch steps follow a reservation; reservation steps do not.
    fn ledger(self, name: &str) -> Ledger {
        let ledger = Ledger::claimed(name);
        match self {
            Self::PreCheck | Self::ReservePlan | Self::Reserve => ledger,
            Self::FleetBind | Self::Start => ledger.reserved(),
        }
    }

    fn run(
        self,
        i: &mut Implementer,
        ledger: &Ledger,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        let plan = ledger.plan();
        match self {
            Self::PreCheck => i.authorize_dispatch_plan(attempt, fence, now_ms, &plan),
            Self::ReservePlan => i
                .reserve_dispatch_plan(&ledger.work, attempt, fence, now_ms, &plan)
                .map(drop),
            Self::Reserve => {
                let digest = admit_dispatch_plan(&ledger.work, &ledger.attempt, &plan)
                    .unwrap_or_else(|_| unreachable!());
                i.reserve_dispatch(attempt, fence, now_ms, &digest, &plan.provider_profile)
                    .map(drop)
            }
            Self::FleetBind => i
                .fleet_bind_session(
                    Placement {
                        attempt: attempt.clone(),
                        fence,
                        work: ledger.attempt.work_identity.clone(),
                        work_revision: ledger.attempt.work_revision,
                        machine: "aspen3".into(),
                        kind: "omp".into(),
                    },
                    now_ms,
                )
                .map(drop),
            Self::Start => i.start_execution(attempt, fence, now_ms, &workflow()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Denial {
    AbsentProfile,
    RevokedGrant,
    StaleRevocationView,
    WrongFence,
    WrongAttempt,
}

impl Denial {
    const ALL: [Self; 5] = [
        Self::AbsentProfile,
        Self::RevokedGrant,
        Self::StaleRevocationView,
        Self::WrongFence,
        Self::WrongAttempt,
    ];

    /// The instance, attempt, fence and time this denial presents.
    fn present(self, ledger: &Ledger) -> (Implementer, ContentId, u64, u64) {
        let attempt = ledger.attempt.identity.clone();
        let fence = ledger.attempt.fence;
        match self {
            Self::AbsentProfile => {
                let mut i = ok(Implementer::open(&ledger.db, enrollment()));
                i.set_clock_ms(T);
                (i, attempt, fence, T)
            }
            Self::RevokedGrant => {
                // The operator revokes the coordinator grant mid-flight; the
                // next process imports the feed when it selects the profile.
                let revoked = test_support::token_ref(ledger.fixture.grant(GrantRole::Coding));
                let config = write_config(ledger.dir.path(), &ledger.fixture, &[revoked]);
                (open_verified(&ledger.db, &config), attempt, fence, T)
            }
            Self::StaleRevocationView => (
                open_verified(&ledger.db, &ledger.config),
                attempt,
                fence,
                STALE_MS,
            ),
            Self::WrongFence => (
                open_verified(&ledger.db, &ledger.config),
                attempt,
                fence + 1,
                T,
            ),
            Self::WrongAttempt => (
                open_verified(&ledger.db, &ledger.config),
                id("not-an-attempt"),
                fence,
                T,
            ),
        }
    }
}

#[test]
// r[verify kiln.implementation.authority.dispatch]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
fn verified_profile_reserves_and_launches_through_every_dispatch_step() {
    let ledger = Ledger::claimed("verified-dispatch");
    let attempt = &ledger.attempt;
    let plan = ledger.plan();
    let mut i = open_verified(&ledger.db, &ledger.config);
    assert_eq!(replay_rows(&ledger.db), 1, "the claim");

    // The pre-check admits the reservation's exact request and changes nothing.
    ok(i.authorize_dispatch_plan(&attempt.identity, attempt.fence, T, &plan));
    assert_eq!(replay_rows(&ledger.db), 2);
    assert!(!records(&ledger.db, &attempt.identity).reservation);
    // The reservation revalidates that session; it grants nothing twice.
    let reservation =
        ok(i.reserve_dispatch_plan(&ledger.work, &attempt.identity, attempt.fence, T, &plan));
    assert_eq!(
        ok(i.reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            T,
            &reservation.plan,
            &reservation.provider_profile,
        )),
        reservation
    );
    assert_eq!(replay_rows(&ledger.db), 2, "one grant for the reservation");

    // The fleet launch: the bind admits the launch request, the recheck
    // before the launch effects revalidates it.
    let session = ok(i.fleet_bind_session(
        Placement {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            machine: "aspen3".into(),
            kind: "omp".into(),
        },
        T + 1,
    ));
    ok(i.fleet_authorize_launch(&session.identity, T + 2));
    assert_eq!(replay_rows(&ledger.db), 3);
    ok(i.start_execution(&attempt.identity, attempt.fence, T + 3, &workflow()));
    assert_eq!(replay_rows(&ledger.db), 4);
    drop(i);

    assert_eq!(
        records(&ledger.db, &attempt.identity),
        Records {
            reservation: true,
            plan: true,
            phase: AttemptPhase::Executing,
            workflow: Some(workflow()),
            sessions: 1,
        }
    );
    let dispatch: Vec<_> = decisions(&ledger.db)
        .into_iter()
        .filter(|decision| decision.operation == ProtectedOperation::Dispatch)
        .collect();
    // The pre-check, the bind and the start each admitted a Dispatch request.
    assert!(dispatch.len() >= 3, "{dispatch:?}");
    assert!(
        dispatch
            .iter()
            .all(|decision| decision.allowed
                && decision.provenance == AuthorityProvenance::VerifiedUcan),
        "{dispatch:?}"
    );
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
fn local_operator_reserves_and_launches_through_every_dispatch_step() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let work = based_work("local-operator-dispatch");
    let mut i = ok(Implementer::open_with_local_operator(
        &db,
        enrollment(),
        "test-operator",
    ));
    ok(i.admit(&work, None));
    let attempt = ok(i.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!("claim"));
    let plan = plan(&work, &attempt);
    ok(i.authorize_dispatch_plan(&attempt.identity, attempt.fence, T, &plan));
    ok(i.reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, T, &plan));
    let session = ok(i.fleet_bind_session(
        Placement {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            machine: "aspen3".into(),
            kind: "omp".into(),
        },
        T + 1,
    ));
    ok(i.fleet_authorize_launch(&session.identity, T + 2));
    ok(i.start_execution(&attempt.identity, attempt.fence, T + 3, &workflow()));
    let dispatch: Vec<_> = i
        .authority_decisions()
        .iter()
        .filter(|decision| decision.operation == ProtectedOperation::Dispatch)
        .collect();
    assert!(!dispatch.is_empty());
    assert!(
        dispatch.iter().all(|decision| decision.allowed
            && decision.provenance == AuthorityProvenance::LocalOperator),
        "{dispatch:?}"
    );
    drop(i);
    assert_eq!(
        records(&db, &attempt.identity).phase,
        AttemptPhase::Executing
    );
}

#[test]
// r[verify kiln.implementation.authority.dispatch]
// r[verify kiln.implementation.authority.dispatch.scenario.denied]
fn every_denial_of_every_dispatch_step_commits_nothing_and_starts_nothing() {
    for step in Step::ALL {
        for denial in Denial::ALL {
            let case = format!("{step:?} / {denial:?}");
            let ledger = step.ledger(&format!("{step:?}-{denial:?}"));
            let before = records(&ledger.db, &ledger.attempt.identity);
            let rows = replay_rows(&ledger.db);
            let persisted = decisions(&ledger.db).len();

            let (mut i, attempt, fence, now_ms) = denial.present(&ledger);
            let error = step
                .run(&mut i, &ledger, &attempt, fence, now_ms)
                .err()
                .unwrap_or_else(|| unreachable!("{case}: must deny"));
            drop(i);

            // Nothing committed, nothing started, no invocation consumed.
            assert_eq!(
                records(&ledger.db, &ledger.attempt.identity),
                before,
                "{case}"
            );
            assert_eq!(replay_rows(&ledger.db), rows, "{case}");
            let after = decisions(&ledger.db);
            match denial {
                Denial::AbsentProfile => {
                    assert!(
                        error.contains("no authority profile selected"),
                        "{case}: {error}"
                    );
                    assert_eq!(after.len(), persisted, "{case}");
                }
                Denial::RevokedGrant | Denial::StaleRevocationView => {
                    // The authority itself refused this exact Dispatch request.
                    assert!(error.starts_with("authority:"), "{case}: {error}");
                    let last = after.last().unwrap_or_else(|| unreachable!("{case}"));
                    assert!(!last.allowed, "{case}");
                    assert_eq!(last.operation, ProtectedOperation::Dispatch, "{case}");
                }
                Denial::WrongFence | Denial::WrongAttempt => {
                    // The attempt ledger refuses before the authority is asked.
                    assert_eq!(after.len(), persisted, "{case}: {error}");
                }
            }
        }
    }
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.denied]
fn a_revoked_grant_stops_a_bound_fleet_session_before_its_launch_effects() {
    let ledger = Ledger::reserved(Ledger::claimed("fleet-recheck"));
    let attempt = &ledger.attempt;
    let mut i = open_verified(&ledger.db, &ledger.config);
    let session = ok(i.fleet_bind_session(
        Placement {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            machine: "aspen3".into(),
            kind: "omp".into(),
        },
        T,
    ));
    drop(i);
    let rows = replay_rows(&ledger.db);
    let before = records(&ledger.db, &attempt.identity);

    // A later tick in a fresh process, after the grant was revoked.
    let revoked = test_support::token_ref(ledger.fixture.grant(GrantRole::Coding));
    let config = write_config(ledger.dir.path(), &ledger.fixture, &[revoked]);
    let mut i = open_verified(&ledger.db, &config);
    let error = i
        .fleet_authorize_launch(&session.identity, T + 1)
        .err()
        .unwrap_or_else(|| unreachable!("a revoked grant must not launch"));
    assert!(error.starts_with("authority:"), "{error}");
    // A stale revocation view refuses the same way.
    let mut stale = open_verified(&ledger.db, &ledger.config);
    assert!(
        stale
            .fleet_authorize_launch(&session.identity, STALE_MS)
            .is_err()
    );
    drop((i, stale));
    assert_eq!(records(&ledger.db, &attempt.identity), before);
    assert_eq!(replay_rows(&ledger.db), rows);
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
// r[verify kiln.implementation.authority.config.scenario.roles]
fn a_worker_selected_from_the_drives_authority_config_reaches_the_transport_check() {
    // Shells select from the wall clock and act afterwards, so these grants
    // are valid now and every decision reads the clock after its selection.
    let wall_ms = || {
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| unreachable!())
                .as_millis(),
        )
        .unwrap_or_else(|_| unreachable!())
    };
    let now_ms = wall_ms();
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct_from(test_support::ROOT_SEED, now_ms / 1_000 + 3_600);
    let config = write_config(dir.path(), &fixture, &[]);
    let work = based_work("drive-worker");
    // Drive and worker are coordinator entry points: they load the
    // coordinator seed only.
    let coding = std::collections::BTreeSet::from([GrantRole::Coding]);

    // The campaign drive claims under its exported configuration.
    let mut drive = ok(Implementer::open_with_authority(
        &db,
        enrollment(),
        None,
        Some(&config),
        &coding,
    ));
    ok(drive.admit(&work, None));
    let attempt =
        ok(drive.claim_next(wall_ms(), LEASE_MS)).unwrap_or_else(|| unreachable!("claim"));
    drop(drive);
    let plan = plan(&work, &attempt);

    // Without the drive's source the worker selects the absent profile and is
    // refused at the pre-check, before its credential read and reservation.
    let mut absent = ok(Implementer::open_with_authority(
        &db,
        enrollment(),
        None,
        None,
        &coding,
    ));
    assert!(
        absent
            .authorize_dispatch_plan(&attempt.identity, attempt.fence, wall_ms(), &plan)
            .is_err()
    );
    drop(absent);
    assert!(!records(&db, &attempt.identity).reservation);

    // With the drive's `--authority-config`, the worker's own selection admits
    // its pre-check, reservation and transport recheck.
    let mut worker = ok(Implementer::open_with_authority(
        &db,
        enrollment(),
        None,
        Some(&config),
        &coding,
    ));
    let now_ms = wall_ms();
    ok(worker.authorize_dispatch_plan(&attempt.identity, attempt.fence, now_ms, &plan));
    ok(worker.reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, now_ms, &plan));
    ok(worker.admission_before_transport(&attempt.identity, attempt.fence, &[], now_ms));
    drop(worker);
    // The claim, the pre-check shared with the reservation, and the
    // transport recheck.
    assert_eq!(replay_rows(&db), 3);
    assert!(records(&db, &attempt.identity).reservation);
    assert!(decisions(&db).iter().all(
        |decision| decision.allowed && decision.provenance == AuthorityProvenance::VerifiedUcan
    ));
}

/// A stand-in for a grant scoped to one exact work version: a request naming
/// any other work reaches the local-operator port as superseded, so it denies.
struct WorkScoped {
    inner: LocalOperatorAuthority,
    work: String,
    version: u64,
}

impl WorkScoped {
    fn install(i: &mut Implementer, work: &ImplementationWork) {
        let scoped = Self {
            inner: LocalOperatorAuthority::new("test-operator", false)
                .unwrap_or_else(|error| unreachable!("{}", error.message)),
            work: work
                .identity()
                .unwrap_or_else(|error| unreachable!("{}", error.message))
                .as_str()
                .into(),
            version: work.revision,
        };
        i.set_authority(
            Box::new(scoped),
            AuthoritySelection::LocalOperator {
                operator: "test-operator".into(),
            },
        );
    }

    fn scope(&self, request: &AuthorityRequest, currentness: AttemptCurrentness) -> AttemptCurrentness {
        if request.work == self.work && request.work_version == self.version {
            currentness
        } else {
            AttemptCurrentness::Superseded
        }
    }
}

impl ProtectedAuthority for WorkScoped {
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<AuthorityDecision, AuthorityDecision> {
        let currentness = self.scope(request, currentness);
        self.inner.admit(request, profile, currentness, now_unix_s)
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<(), AuthorityDecision> {
        let currentness = self.scope(request, currentness);
        self.inner
            .revalidate(session, request, currentness, now_unix_s)
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.inner.close(session);
    }

    fn decisions(&self) -> &[AuthorityDecision] {
        self.inner.decisions()
    }
}

/// A claimed attempt superseded under `FinishActive` by a newer revision of
/// its work: it stays current on the revision it froze.
struct FinishActive {
    _dir: tempfile::TempDir,
    db: PathBuf,
    frozen: ImplementationWork,
    newer: ImplementationWork,
    attempt: ImplementationAttempt,
}

impl FinishActive {
    fn new(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
        let db = dir.path().join("ledger.db");
        let frozen = based_work(name);
        let mut i = ok(Implementer::open_with_local_operator(
            &db,
            enrollment(),
            "test-operator",
        ));
        ok(i.admit(&frozen, None));
        let attempt = ok(i.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!("claim"));
        let mut newer = frozen.clone();
        newer.revision += 1;
        newer.title.push_str(" with changed requirements");
        ok(i.admit_with_supersession(&newer, None, SupersessionRule::FinishActive, T));
        let current = ok(i.attempt(&attempt.identity)).unwrap_or_else(|| unreachable!("attempt"));
        assert_eq!(current.superseded_by, Some(newer.revision));
        assert_eq!(current.phase, AttemptPhase::Claimed);
        Self {
            _dir: dir,
            db,
            frozen,
            newer,
            attempt,
        }
    }

    fn reserve(&self, i: &mut Implementer) -> Result<(), String> {
        let plan = plan(&self.frozen, &self.attempt);
        let digest = admit_dispatch_plan(&self.frozen, &self.attempt, &plan)
            .unwrap_or_else(|error| unreachable!("{}", error.message));
        i.reserve_dispatch(
            &self.attempt.identity,
            self.attempt.fence,
            T,
            &digest,
            &plan.provider_profile,
        )
        .map(drop)
    }

    fn start(&self, i: &mut Implementer) -> Result<(), String> {
        i.start_execution(&self.attempt.identity, self.attempt.fence, T + 1, &workflow())
    }
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
fn a_finish_active_attempt_dispatches_under_a_grant_for_its_frozen_work() {
    let ledger = FinishActive::new("frozen-work-dispatch");
    let mut i = ok(Implementer::open(&ledger.db, enrollment()));
    WorkScoped::install(&mut i, &ledger.frozen);
    ok(ledger.reserve(&mut i));
    ok(ledger.start(&mut i));
    let dispatch: Vec<_> = i
        .authority_decisions()
        .iter()
        .filter(|decision| decision.operation == ProtectedOperation::Dispatch)
        .collect();
    assert!(dispatch.len() >= 2, "{dispatch:?}");
    assert!(dispatch.iter().all(|decision| decision.allowed), "{dispatch:?}");
    drop(i);
    let records = records(&ledger.db, &ledger.attempt.identity);
    assert!(records.reservation);
    assert_eq!(records.workflow, Some(workflow()));
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.denied]
fn a_grant_for_the_newer_work_cannot_dispatch_a_finish_active_attempt() {
    let ledger = FinishActive::new("newer-work-dispatch");
    let before = records(&ledger.db, &ledger.attempt.identity);
    let mut i = ok(Implementer::open(&ledger.db, enrollment()));
    WorkScoped::install(&mut i, &ledger.newer);
    let error = ledger
        .reserve(&mut i)
        .err()
        .unwrap_or_else(|| unreachable!("the newer work's grant must not reserve"));
    assert!(error.starts_with("authority:"), "{error}");
    drop(i);
    assert_eq!(records(&ledger.db, &ledger.attempt.identity), before);

    // Reserved under the frozen work's grant, the launch record is refused
    // to the newer work's grant in the same way.
    let mut i = ok(Implementer::open(&ledger.db, enrollment()));
    WorkScoped::install(&mut i, &ledger.frozen);
    ok(ledger.reserve(&mut i));
    drop(i);
    let reserved = records(&ledger.db, &ledger.attempt.identity);
    let mut i = ok(Implementer::open(&ledger.db, enrollment()));
    WorkScoped::install(&mut i, &ledger.newer);
    let error = ledger
        .start(&mut i)
        .err()
        .unwrap_or_else(|| unreachable!("the newer work's grant must not start"));
    assert!(error.starts_with("authority:"), "{error}");
    drop(i);
    assert_eq!(records(&ledger.db, &ledger.attempt.identity), reserved);
}
