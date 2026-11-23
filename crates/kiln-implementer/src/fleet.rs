//! Durable fleet state: claims, interactive sessions and decisions.
//!
//! The state is one versioned JSON row, [`FleetState`], under kind
//! [`FLEET_STATE_KIND`] in `implementation_state`. Every mutating method loads
//! it, decides with the pure rules in `kiln_core::fleet` and saves it in one
//! immediate SQLite transaction, like the work and attempt ledgers. Methods
//! that need attempt facts read the work and attempt ledgers in that same
//! transaction; this module never writes them. Read-only methods use a
//! deferred transaction and save nothing.

use kiln_core::fleet::{
    self, AttemptScope, AttemptView, AttemptViewState, ClaimCommand, ClaimContext, ClaimEffect,
    ClaimOutcome, Decision, DecisionLedger, FleetClaimRegistry, FleetEffect, FleetPolicy,
    FleetSession, ForeignClaim, Observation, Placement, SessionLedger, TickInput,
    WHOLE_REPOSITORY_PATH,
};
use kiln_core::implementation::{
    AttemptLedger, DEFAULT_LEASE_MS, INTERACTIVE_PROVIDER_PROFILE, ImplementationAttempt,
    ImplementationWork, TransportLaunches, WorkLedger,
};
use kiln_core::types::{ContentId, Diagnostic};
use rusqlite::{Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::authority_bridge::{self, DispatchGate};
use crate::{
    ATTEMPT_KIND, Implementer, TRANSPORT_LAUNCH_KIND, WORK_KIND, diagnostic, load, save, text,
};

/// `implementation_state` row holding the fleet state.
pub const FLEET_STATE_KIND: &str = "fleet";
pub const FLEET_STATE_SCHEMA: &str = "kiln.fleet-state.v1";
/// Lease used only to probe whether ranked work could be claimed now. The
/// probe runs on a scratch copy of the attempt ledger; nothing is claimed.
const READINESS_PROBE_LEASE_MS: u64 = DEFAULT_LEASE_MS;

/// Fleet claims, sessions and decisions, persisted as one versioned blob.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetState {
    pub schema: String,
    pub claims: FleetClaimRegistry,
    pub sessions: SessionLedger,
    pub decisions: DecisionLedger,
}

impl Default for FleetState {
    fn default() -> Self {
        Self {
            schema: FLEET_STATE_SCHEMA.to_string(),
            claims: FleetClaimRegistry::default(),
            sessions: SessionLedger::default(),
            decisions: DecisionLedger::default(),
        }
    }
}

/// Work and attempt ledgers read beside the fleet state.
struct Ledgers {
    work: WorkLedger,
    attempts: AttemptLedger,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Access {
    Read,
    Write,
}

impl Implementer {
    /// The persisted fleet state; empty before the first fleet write.
    ///
    /// # Errors
    /// Returns the store diagnostic, or `fleet_state_schema` for a row written
    /// under another schema.
    pub fn fleet_state(&mut self) -> Result<FleetState, String> {
        self.with_fleet(Access::Read, |fleet| Ok(fleet.clone()))
    }

    /// Monotonic version of the persisted fleet state row. Readers use it to
    /// observe changes without copying the whole registry.
    ///
    /// # Errors
    /// Returns the store diagnostic, or `fleet_state_schema` for a row written
    /// under another schema.
    pub fn fleet_version(&mut self) -> Result<u64, String> {
        let transaction = begin(&mut self.connection, Access::Read)?;
        let (_, version) = load_fleet(&transaction)?;
        transaction.commit().map_err(text)?;
        Ok(version)
    }

    /// BLAKE3 identity of the fleet, work and attempt row versions. A change in
    /// any projection the console reads changes this value, even when the
    /// fleet claim registry itself is untouched.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn fleet_observation_version(&mut self) -> Result<String, String> {
        let transaction = begin(&mut self.connection, Access::Read)?;
        let (_, fleet) = load_fleet(&transaction)?;
        let (_, work) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (_, attempts) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        transaction.commit().map_err(text)?;
        Ok(
            blake3::hash(format!("{fleet}:{work}:{attempts}").as_bytes())
                .to_hex()
                .to_string(),
        )
    }

    /// The repository and permitted paths of every current, unexpired attempt
    /// in a claimed, executing, candidate-produced or verifying phase.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn fleet_attempt_scopes(&mut self, now_ms: u64) -> Result<Vec<AttemptScope>, String> {
        self.with_fleet_ledgers(Access::Read, |ledgers, _| {
            Ok(attempt_scopes(&ledgers.work, &ledgers.attempts, now_ms))
        })
    }

    // r[impl kiln.fleet.claims]
    /// Decide one claim command against the fleet claims, the live attempts'
    /// scopes (derived in the same transaction) and the caller's open foreign
    /// swarm claims. A conflict is an `Ok` outcome with no effects.
    ///
    /// # Errors
    /// Returns the claim diagnostic (`fleet_claim_stale`, `fleet_claim_lease`,
    /// ...) or the store diagnostic; nothing is saved then.
    pub fn fleet_claim(
        &mut self,
        command: ClaimCommand,
        now_ms: u64,
        coordinator: &str,
        foreign: &[ForeignClaim],
    ) -> Result<(ClaimOutcome, Vec<ClaimEffect>), String> {
        self.with_fleet_ledgers(Access::Write, |ledgers, fleet| {
            let attempts = attempt_scopes(&ledgers.work, &ledgers.attempts, now_ms);
            let context = ClaimContext {
                now_ms,
                attempts: &attempts,
                foreign,
                coordinator,
            };
            let (claims, outcome, effects) =
                fleet::decide(&fleet.claims, command, &context).map_err(diagnostic)?;
            fleet.claims = claims;
            Ok((outcome, effects))
        })
    }

    // r[impl kiln.fleet.mirror]
    /// Claim and release articles still pending, rendered again for posting.
    ///
    /// # Errors
    /// Returns the rendering or store diagnostic.
    pub fn fleet_pending_mirror(&mut self, coordinator: &str) -> Result<Vec<ClaimEffect>, String> {
        self.with_fleet(Access::Read, |fleet| {
            fleet::pending_mirror_effects(&fleet.claims, coordinator).map_err(diagnostic)
        })
    }

    // r[impl kiln.fleet.dispatch] r[impl kiln.implementation.authority.dispatch]
    /// Bind a new `Placing` session for a placed attempt: the durable launch
    /// record every later launch effect of the fleet tick needs.
    ///
    /// After the fence, work and session refusals, the launch's exact
    /// `Dispatch` request (the attempt's reservation effect, plan, machine and
    /// agent kind) is admitted in the same transaction, before the session is
    /// saved; a denial binds nothing.
    ///
    /// # Errors
    /// Refuses an attempt and fence that do not own the current unexpired
    /// claim (`implementation_fence_stale`), a placement whose work is not the
    /// attempt's frozen work, an attempt that already has a live session,
    /// every `session::bind` refusal and the redacted authority denial.
    pub fn fleet_bind_session(
        &mut self,
        placement: Placement,
        now_ms: u64,
    ) -> Result<FleetSession, String> {
        self.with_fleet_dispatch(Access::Write, now_ms, |ledgers, fleet, gate| {
            let attempt = ledgers
                .attempts
                .get(&placement.attempt)
                .filter(|attempt| {
                    ledgers
                        .attempts
                        .is_current(&attempt.identity, placement.fence, now_ms)
                })
                .ok_or_else(fence_stale)?;
            if attempt.work_identity != placement.work
                || attempt.work_revision != placement.work_revision
            {
                return Err(diagnostic(Diagnostic::new(
                    "fleet_session_bind",
                    "placement.work",
                    "placement work is not the attempt's frozen work",
                )));
            }
            if fleet
                .sessions
                .sessions
                .values()
                .any(|session| session.attempt == placement.attempt && !session.state.is_ended())
            {
                return Err(diagnostic(Diagnostic::new(
                    "fleet_session_bind",
                    "session.attempt",
                    "the attempt already has a live session",
                )));
            }
            let session = FleetSession::placing(placement, now_ms).map_err(diagnostic)?;
            let sessions = fleet::bind(&fleet.sessions, session.clone()).map_err(diagnostic)?;
            authorize_launch(gate, ledgers, attempt, &session)?;
            fleet.sessions = sessions;
            Ok(session)
        })
    }

    // r[impl kiln.implementation.authority.dispatch]
    /// Admit a bound `Placing` session's exact `Dispatch` request again,
    /// immediately before its launch effects (worktree creation and agent
    /// start). It saves nothing. In the process that bound the session this
    /// revalidates the bind's session; a revoked grant, a stale revocation
    /// view or a replaced fence therefore starts nothing.
    ///
    /// # Errors
    /// Returns `fleet_session_unknown`, `implementation_fence_stale` or the
    /// redacted authority denial.
    pub fn fleet_authorize_launch(
        &mut self,
        session: &ContentId,
        now_ms: u64,
    ) -> Result<(), String> {
        self.with_fleet_dispatch(Access::Read, now_ms, |ledgers, fleet, gate| {
            let session = fleet
                .sessions
                .sessions
                .get(session)
                .ok_or_else(|| unknown_session("session"))?;
            let attempt = ledgers
                .attempts
                .get(&session.attempt)
                .filter(|attempt| {
                    ledgers
                        .attempts
                        .is_current(&attempt.identity, session.fence, now_ms)
                })
                .ok_or_else(fence_stale)?;
            authorize_launch(gate, ledgers, attempt, session)
        })
    }

    /// Forget an issued `Place` that could not bind a session.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn fleet_clear_placing(&mut self, attempt: &ContentId) -> Result<(), String> {
        self.with_fleet(Access::Write, |fleet| {
            fleet.sessions = fleet::clear_placing(&fleet.sessions, attempt);
            Ok(())
        })
    }

    // r[impl kiln.fleet.dispatch] r[impl kiln.fleet.decisions]
    /// Admit one observation of a session. When the session has ended, its
    /// open decisions close without delivery in the same transaction.
    ///
    /// # Errors
    /// Returns the `session::observe` refusal or the store diagnostic.
    pub fn fleet_observe(
        &mut self,
        session: &ContentId,
        observation: Observation,
        now_ms: u64,
    ) -> Result<FleetSession, String> {
        self.with_fleet(Access::Write, |fleet| {
            fleet.sessions = fleet::observe(&fleet.sessions, session, observation, now_ms)
                .map_err(diagnostic)?;
            let observed = fleet
                .sessions
                .sessions
                .get(session)
                .cloned()
                .ok_or_else(|| unknown_session("session"))?;
            if observed.state.is_ended() {
                let (decisions, _) =
                    fleet::close_for_ended_sessions(&fleet.decisions, &fleet.sessions, now_ms);
                fleet.decisions = decisions;
            }
            Ok(observed)
        })
    }

    // r[impl kiln.fleet.decisions]
    /// Raise an operator decision for a live session.
    ///
    /// # Errors
    /// Refuses an unknown or ended session (an ended session's decision would
    /// close at once without delivery), and every `decision::raise` refusal.
    pub fn fleet_raise_decision(
        &mut self,
        session: &ContentId,
        question: String,
        now_ms: u64,
    ) -> Result<Decision, String> {
        self.with_fleet(Access::Write, |fleet| {
            let current = fleet
                .sessions
                .sessions
                .get(session)
                .ok_or_else(|| unknown_session("session"))?;
            if current.state.is_ended() {
                return Err(diagnostic(Diagnostic::new(
                    "fleet_decision_session",
                    "session",
                    "the session has ended",
                )));
            }
            let (decisions, decision) = fleet::raise(
                &fleet.decisions,
                session,
                &current.attempt,
                &question,
                now_ms,
            )
            .map_err(diagnostic)?;
            fleet.decisions = decisions;
            Ok(decision)
        })
    }

    // r[impl kiln.fleet.decisions]
    /// Record the one answer to a decision and return the prompt that
    /// delivers it. The dispatch tick issues that prompt to the session.
    ///
    /// # Errors
    /// Returns every `decision::answer` refusal, including a non-operator
    /// answerer, or the store diagnostic.
    pub fn fleet_answer_decision(
        &mut self,
        decision: &ContentId,
        text: String,
        by: &str,
        operator: bool,
        now_ms: u64,
    ) -> Result<String, String> {
        self.with_fleet(Access::Write, |fleet| {
            let (decisions, prompt) =
                fleet::answer(&fleet.decisions, decision, &text, by, operator, now_ms)
                    .map_err(diagnostic)?;
            fleet.decisions = decisions;
            Ok(prompt)
        })
    }

    /// One view per attempt that owns its work's current unexpired claim, and
    /// one per attempt a fleet session follows, so the tick sees an ended
    /// attempt as `Terminal`.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn fleet_attempt_views(&mut self, now_ms: u64) -> Result<Vec<AttemptView>, String> {
        self.with_fleet_ledgers(Access::Read, |ledgers, fleet| {
            Ok(attempt_views(&ledgers.attempts, &fleet.sessions, now_ms))
        })
    }

    /// Ranked ready work that a claim would admit now.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn fleet_ready_work(&mut self, now_ms: u64) -> Result<u32, String> {
        self.with_fleet_ledgers(Access::Read, |ledgers, _| {
            Ok(ready_work(&ledgers.work, &ledgers.attempts, now_ms))
        })
    }

    // r[impl kiln.fleet.dispatch]
    /// Plan one dispatch tick over a read-only snapshot.
    ///
    /// # Errors
    /// Refuses a policy outside its bounds, or returns the store diagnostic.
    pub fn fleet_plan(
        &mut self,
        now_ms: u64,
        free_slots: u32,
        policy: &FleetPolicy,
    ) -> Result<Vec<FleetEffect>, String> {
        policy.validate().map_err(diagnostic)?;
        self.with_fleet_ledgers(Access::Read, |ledgers, fleet| {
            let views = attempt_views(&ledgers.attempts, &fleet.sessions, now_ms);
            let input = TickInput {
                now_ms,
                ledger: &fleet.sessions,
                attempts: &views,
                free_slots,
                ready_work: ready_work(&ledgers.work, &ledgers.attempts, now_ms),
                decisions: &fleet.decisions,
            };
            Ok(fleet::plan_tick(&input, policy))
        })
    }

    // r[impl kiln.fleet.dispatch]
    /// Record performed effects so the next tick does not repeat them. The
    /// tick input is rebuilt from the current state in this transaction.
    ///
    /// # Errors
    /// Returns the `session::record_issued` refusal or the store diagnostic.
    pub fn fleet_record_issued(
        &mut self,
        now_ms: u64,
        free_slots: u32,
        effects: &[FleetEffect],
    ) -> Result<(), String> {
        self.with_fleet_ledgers(Access::Write, |ledgers, fleet| {
            let views = attempt_views(&ledgers.attempts, &fleet.sessions, now_ms);
            let input = TickInput {
                now_ms,
                ledger: &fleet.sessions,
                attempts: &views,
                free_slots,
                ready_work: ready_work(&ledgers.work, &ledgers.attempts, now_ms),
                decisions: &fleet.decisions,
            };
            let sessions = fleet::record_issued(&input, effects).map_err(diagnostic)?;
            fleet.sessions = sessions;
            Ok(())
        })
    }

    fn with_fleet<T>(
        &mut self,
        access: Access,
        apply: impl FnOnce(&mut FleetState) -> Result<T, String>,
    ) -> Result<T, String> {
        let transaction = begin(&mut self.connection, access)?;
        let (mut state, version) = load_fleet(&transaction)?;
        let result = apply(&mut state)?;
        finish(transaction, access, version, &state)?;
        Ok(result)
    }

    fn with_fleet_ledgers<T>(
        &mut self,
        access: Access,
        apply: impl FnOnce(&Ledgers, &mut FleetState) -> Result<T, String>,
    ) -> Result<T, String> {
        let transaction = begin(&mut self.connection, access)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (mut state, version) = load_fleet(&transaction)?;
        let result = apply(&Ledgers { work, attempts }, &mut state)?;
        finish(transaction, access, version, &state)?;
        Ok(result)
    }

    /// `with_fleet_ledgers` plus a [`DispatchGate`] over the selected
    /// authority and the attempts' recorded launches, read in the same
    /// transaction. An error from `apply`, including an authority denial,
    /// saves nothing.
    fn with_fleet_dispatch<T>(
        &mut self,
        access: Access,
        now_ms: u64,
        apply: impl FnOnce(&Ledgers, &mut FleetState, &mut DispatchGate<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let transaction = begin(&mut self.connection, access)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (launches, _) = load::<TransportLaunches>(&transaction, TRANSPORT_LAUNCH_KIND)?;
        let (mut state, version) = load_fleet(&transaction)?;
        let mut gate = DispatchGate {
            authority: &mut self.authority,
            profile: &self.authority_profile,
            last: &mut self.authority_last,
            launches: &launches,
            now_ms,
        };
        let result = apply(&Ledgers { work, attempts }, &mut state, &mut gate)?;
        finish(transaction, access, version, &state)?;
        Ok(result)
    }
}

fn begin(connection: &mut rusqlite::Connection, access: Access) -> Result<Transaction<'_>, String> {
    let behavior = match access {
        Access::Read => TransactionBehavior::Deferred,
        Access::Write => TransactionBehavior::Immediate,
    };
    connection.transaction_with_behavior(behavior).map_err(text)
}

fn finish(
    transaction: Transaction<'_>,
    access: Access,
    version: u64,
    state: &FleetState,
) -> Result<(), String> {
    if access == Access::Write {
        save(&transaction, FLEET_STATE_KIND, version, state)?;
    }
    transaction.commit().map_err(text)
}

fn load_fleet(transaction: &Transaction<'_>) -> Result<(FleetState, u64), String> {
    let (state, version) = load::<FleetState>(transaction, FLEET_STATE_KIND)?;
    if state.schema != FLEET_STATE_SCHEMA {
        return Err(diagnostic(Diagnostic::new(
            "fleet_state_schema",
            "fleet.schema",
            "fleet state schema is unsupported",
        )));
    }
    Ok((state, version))
}

fn unknown_session(field: &str) -> String {
    diagnostic(Diagnostic::new(
        "fleet_session_unknown",
        field,
        "no such fleet session",
    ))
}

fn fence_stale() -> String {
    diagnostic(Diagnostic::new(
        "implementation_fence_stale",
        "attempt.fence",
        "attempt does not own the current unexpired claim",
    ))
}

/// Admit the exact `Dispatch` request of one session's launch: the
/// attempt's reservation effect and plan, and the session's machine and
/// agent kind, over the attempt's frozen work.
fn authorize_launch(
    gate: &mut DispatchGate<'_>,
    ledgers: &Ledgers,
    attempt: &ImplementationAttempt,
    session: &FleetSession,
) -> Result<(), String> {
    let work = attempt_work(&ledgers.work, attempt).ok_or_else(|| {
        diagnostic(Diagnostic::new(
            "fleet_session_bind",
            "placement.work",
            "the attempt's frozen work is not retained",
        ))
    })?;
    let plan = attempt
        .dispatch
        .as_ref()
        .map(|dispatch| dispatch.plan.as_str())
        .unwrap_or_default();
    authority_bridge::authorize_dispatch(
        gate,
        work,
        attempt,
        &[plan, session.machine.as_str(), session.kind.as_str()],
    )
}

/// The work version an attempt froze: the current version when it still
/// matches, otherwise the retained superseded version.
fn attempt_work<'a>(
    work: &'a WorkLedger,
    attempt: &ImplementationAttempt,
) -> Option<&'a ImplementationWork> {
    let frozen = |candidate: &&ImplementationWork| {
        candidate.revision == attempt.work_revision
            && candidate
                .identity()
                .is_ok_and(|identity| identity == attempt.work_identity)
    };
    work.get(&attempt.canonical_key)
        .filter(frozen)
        .or_else(|| work.superseded().iter().find(frozen))
}

/// Scopes of every current, unexpired, active attempt.
///
/// Implementation paths are admitted under the implementation path rules,
/// which allow text the fleet scope grammar refuses (whitespace, non-ASCII, a
/// subject over the swarm bound). Such a path cannot be compared by
/// containment, so it is widened to the whole repository rather than dropped:
/// a fleet claim must never be granted inside a live attempt's paths. A
/// repository the fleet grammar refuses is skipped, because no fleet scope can
/// name it and so none can overlap it.
fn attempt_scopes(work: &WorkLedger, attempts: &AttemptLedger, now_ms: u64) -> Vec<AttemptScope> {
    let mut scopes = Vec::new();
    for attempt in attempts
        .attempts()
        .filter(|attempt| attempts.is_current(&attempt.identity, attempt.fence, now_ms))
    {
        let Some(frozen) = attempt_work(work, attempt) else {
            continue;
        };
        let defaults = [WHOLE_REPOSITORY_PATH]
            .into_iter()
            .filter(|_| frozen.permitted_paths.is_empty());
        for path in frozen
            .permitted_paths
            .iter()
            .map(String::as_str)
            .chain(defaults)
        {
            let Ok(scope) = fleet::parse_scope(&frozen.repository, path)
                .or_else(|_| fleet::parse_scope(&frozen.repository, WHOLE_REPOSITORY_PATH))
            else {
                continue;
            };
            scopes.push(AttemptScope {
                attempt: attempt.identity.clone(),
                scope,
            });
        }
    }
    scopes
}

fn attempt_views(
    attempts: &AttemptLedger,
    sessions: &SessionLedger,
    now_ms: u64,
) -> Vec<AttemptView> {
    attempts
        .attempts()
        .filter_map(|attempt| {
            let followed = sessions.for_attempt(&attempt.identity).is_some();
            if !followed && !attempts.is_current(&attempt.identity, attempt.fence, now_ms) {
                return None;
            }
            let interactive = attempts
                .dispatch_plan(&attempt.identity)
                .is_some_and(|plan| plan.provider_profile.as_str() == INTERACTIVE_PROVIDER_PROFILE);
            Some(AttemptView {
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
                state: AttemptViewState::from_phase(attempt.phase),
                lease_expires_ms: attempt.lease_until_ms,
                held_by_fleet: interactive || followed,
            })
        })
        .collect()
}

/// Count ranked ready work whose claim would be admitted now. Each candidate
/// is probed with the ledger's own claim rule on one scratch copy, so held,
/// budget-exhausted, verified and unreconciled work is excluded exactly as
/// `claim_next` skips it. Candidates are distinct canonical keys, so earlier
/// probes cannot change a later one's outcome.
fn ready_work(work: &WorkLedger, attempts: &AttemptLedger, now_ms: u64) -> u32 {
    let mut probe = attempts.clone();
    let mut ready: u32 = 0;
    for row in work.ranked() {
        let Some(candidate) = work.get(&row.canonical_key) else {
            continue;
        };
        if probe
            .claim(candidate, now_ms, READINESS_PROBE_LEASE_MS)
            .is_ok()
        {
            ready = ready.saturating_add(1);
        }
    }
    ready
}
