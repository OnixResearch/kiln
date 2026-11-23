//! The dispatch thread: one tick every `policy.tick_ms`.
//!
//! Each tick recovers expired attempts, sweeps claims, reconciles and
//! performs swarm mirror posts, observes live herdr sessions, asks the pure
//! core for effects (`fleet_plan`), performs them and records what it
//! performed (`fleet_record_issued`). Every decision is the core's; this
//! shell only performs effects and reports observations. Herdr outcomes that
//! are unknown are reconciled by listing before any retry.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use kiln_adapter_herdr::{
    AgentKind, AgentObservation, AgentStatus as HerdrStatus, FailureClass, HerdrClient,
    HerdrFailure, HerdrInventory, HerdrMachine, MachineLoad, Occupancy, agent_name, attempt_short,
    place,
};
use kiln_core::fleet::{
    AgentStatus, ClaimCommand, ClaimEffect, ClaimOutcome, FleetAgent, FleetEffect, FleetScope,
    FleetSession, ForeignClaim, HandoffInstructions, Observation, Placement, SessionState,
    SessionWorkspace, branch_name, overlaps, parse_scope, render_attempt_prompt,
};
use kiln_core::implementation::swarm::ArticleView;
use kiln_core::implementation::{
    DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, FailureClass as AttemptFailure,
    INTERACTIVE_AGENT_EFFECT, INTERACTIVE_PROVIDER_PROFILE, ImplementationAttempt,
    ImplementationDispatchPlan, ImplementationWork, MAX_DISPATCH_TOKENS, WorkspaceIsolation,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::verification::HERDR_CANDIDATE_RECEIPT_SCHEMA;
use serde_json::json;

use crate::api::Service;
use crate::capture::{Capturer, HandoffSource, Placeholders};
use crate::config::read_bounded;
use crate::log;
use crate::swarm::{learned_message_ids, mirror_status};

/// Holder tool of the claims Kiln takes for its own attempts.
pub const KILN_HOLDER_TOOL: &str = "kiln";
/// Interactive agents are not token-metered by Kiln; the plan declares the
/// admitted maximum explicitly.
pub const INTERACTIVE_TOKEN_BUDGET: u64 = MAX_DISPATCH_TOKENS;
/// One interactive session is one invocation.
pub const INTERACTIVE_INVOCATION_BUDGET: u32 = 1;
/// Upper bound of an interactive plan's deadline; the work budget may be lower.
pub const MAX_INTERACTIVE_DEADLINE_MINUTES: u32 = 24 * 60;
pub const WORKSPACE_IDENTITY_SCHEMA: &str = "kiln.fleet-dispatch-workspace.v1";
pub const EXPECTED_OUTPUT_SCHEMA: &str = HERDR_CANDIDATE_RECEIPT_SCHEMA;
/// External reference recorded as the attempt's execution.
pub const SESSION_REFERENCE_NAMESPACE: &str = "herdr";
pub const SESSION_REFERENCE_KIND: &str = "fleet-session";
pub const WORKSPACE_CLOSED_KIND: &str = "workspace-closed";
pub const MAX_MIRROR_POSTS_PER_TICK: usize = 16;
pub const MAX_INVENTORY_BYTES: u64 = 1_024 * 1_024;
pub const STOP_NOTE: &str = "attempt ended";
pub const DECISION_SUBJECT_PREFIX: &str = "DECISION";
/// Characters of an identity used in human-facing labels.
pub const SHORT_IDENTITY_CHARS: usize = 12;
/// Characters of a stored failure reason.
pub const MAX_REASON_CHARS: usize = 400;

/// What one tick did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub free_slots: u32,
    pub planned: Vec<FleetEffect>,
    pub performed: Vec<FleetEffect>,
}

/// The dispatch thread's state. It owns its own store connection.
#[derive(Debug)]
pub struct Dispatcher {
    service: Arc<Service>,
    store: Implementer,
    herdr: Option<HerdrClient>,
    capturer: Option<Capturer>,
}

impl Dispatcher {
    /// Load the inventory when dispatch is enabled and open the store.
    ///
    /// # Errors
    /// Returns an inventory, store or cross-configuration diagnostic.
    pub fn new(service: Arc<Service>) -> Result<Self, String> {
        let config = Arc::clone(&service.config);
        let (herdr, capturer) = if config.dispatch.enabled {
            let inventory = HerdrInventory::from_json(
                std::str::from_utf8(&read_bounded(&config.inventory, MAX_INVENTORY_BYTES)?)
                    .map_err(|_| "inventory is not UTF-8".to_owned())?,
            )?;
            let machines = config.machines();
            if let Some(unknown) = inventory
                .machines
                .iter()
                .find(|machine| !machines.contains(machine.label.as_str()))
            {
                return Err(format!(
                    "inventory machine {} is neither the coordinator nor a peer, so its handoffs could not be authenticated",
                    unknown.label
                ));
            }
            if let Some(unreachable) = inventory
                .machines
                .iter()
                .find(|machine| machine.ssh_target.is_none() && machine.label != config.machine)
            {
                return Err(format!(
                    "inventory machine {} has no ssh_target, so Kiln could not fetch its handoffs",
                    unreachable.label
                ));
            }
            let environment: Vec<(OsString, OsString)> = inventory
                .environment
                .entries()
                .into_iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                .collect();
            let capturer = config
                .capture
                .clone()
                .map(|capture| Capturer::new(capture, environment));
            (Some(HerdrClient::new(inventory)?), capturer)
        } else {
            (None, None)
        };
        Ok(Self {
            store: service.store.open()?,
            service,
            herdr,
            capturer,
        })
    }

    /// Tick forever on the configured cadence.
    pub fn run(mut self) {
        let tick = Duration::from_millis(self.service.config.policy.tick_ms);
        loop {
            let now_ms = (self.service.clock)();
            match self.tick(now_ms) {
                Ok(report) if !report.performed.is_empty() => log::info(
                    "dispatch.tick",
                    json!({ "free_slots": report.free_slots, "performed": report.performed }),
                ),
                Ok(_) => {}
                Err(error) => log::warn("dispatch.tick_failed", json!({ "error": error })),
            }
            std::thread::sleep(tick);
        }
    }

    // r[impl kiln.fleet.dispatch]
    /// One tick at `now_ms`.
    ///
    /// # Errors
    /// Returns a store diagnostic that stopped the tick.
    pub fn tick(&mut self, now_ms: u64) -> Result<TickReport, String> {
        let config = Arc::clone(&self.service.config);
        if self.herdr.is_some() {
            let recovery = self.store.recover(now_ms)?;
            if !recovery.expired.is_empty() {
                log::info("dispatch.expired", json!({ "attempts": recovery.expired }));
            }
        }
        let window = match &self.service.swarm {
            Some(swarm) => match swarm.refresh(now_ms) {
                Ok(articles) => Some(articles),
                Err(error) => {
                    log::warn("swarm.read_failed", json!({ "error": error }));
                    None
                }
            },
            None => None,
        };
        let foreign: Option<Vec<ForeignClaim>> = match (&self.service.swarm, &window) {
            (Some(swarm), Some(articles)) => Some(crate::swarm::foreign_claims(
                articles,
                swarm.own_address(),
                now_ms,
            )),
            (Some(_), None) => None,
            (None, _) => Some(Vec::new()),
        };
        let (outcome, _) = self.store.fleet_claim(
            ClaimCommand::Sweep,
            now_ms,
            &config.machine,
            foreign.as_deref().unwrap_or_default(),
        )?;
        if let ClaimOutcome::Swept(swept) = &outcome
            && !swept.is_empty()
        {
            log::info("claims.swept", json!({ "claims": swept }));
        }
        self.mirror(window.as_deref())?;
        let Some(herdr) = self.herdr.clone() else {
            return Ok(TickReport::default());
        };
        self.observe_sessions(&herdr, now_ms)?;
        let occupancy = herdr.occupancy();
        let free_slots = free_slots(herdr.inventory(), &occupancy, &config.dispatch.kinds);
        let policy = config.policy.fleet();
        let planned = self.store.fleet_plan(now_ms, free_slots, &policy)?;
        let mut performed = Vec::new();
        for effect in &planned {
            if self.perform(&herdr, effect, now_ms, &occupancy, foreign.as_deref()) {
                performed.push(effect.clone());
            }
        }
        self.store
            .fleet_record_issued(now_ms, free_slots, &performed)?;
        Ok(TickReport {
            free_slots,
            planned,
            performed,
        })
    }

    // r[impl kiln.fleet.mirror]
    fn mirror(&mut self, window: Option<&[ArticleView]>) -> Result<(), String> {
        let Some(swarm) = self.service.swarm.clone() else {
            return Ok(());
        };
        let coordinator = self.service.config.machine.clone();
        if let Some(articles) = window {
            let state = self.store.fleet_state()?;
            for (identity, release, message_id) in
                learned_message_ids(&state.claims, articles, swarm.own_address())
            {
                let observed = self.store.fleet_claim(
                    ClaimCommand::MirrorObserved {
                        identity: identity.clone(),
                        release,
                        status: kiln_core::fleet::MirrorStatus::Posted {
                            message_id: Some(message_id.clone()),
                        },
                    },
                    (self.service.clock)(),
                    &coordinator,
                    &[],
                );
                if let Err(error) = observed {
                    log::warn(
                        "mirror.learn_failed",
                        json!({ "claim": identity, "error": error }),
                    );
                }
            }
        }
        for effect in self
            .store
            .fleet_pending_mirror(&coordinator)?
            .into_iter()
            .take(MAX_MIRROR_POSTS_PER_TICK)
        {
            let (identity, release) = match &effect {
                ClaimEffect::PostClaim { identity, .. } => (identity.clone(), false),
                ClaimEffect::PostRelease { identity, .. } => (identity.clone(), true),
            };
            let outcome = swarm.post(&effect);
            log::info(
                "mirror.post",
                json!({ "claim": identity, "release": release, "outcome": outcome.as_str() }),
            );
            if let Some(status) = mirror_status(outcome) {
                self.store.fleet_claim(
                    ClaimCommand::MirrorObserved {
                        identity,
                        release,
                        status,
                    },
                    (self.service.clock)(),
                    &coordinator,
                    &[],
                )?;
            }
        }
        Ok(())
    }

    fn observe(
        &mut self,
        session: &ContentId,
        observation: Observation,
        now_ms: u64,
    ) -> Option<FleetSession> {
        let label = format!("{observation:?}");
        match self.store.fleet_observe(session, observation, now_ms) {
            Ok(observed) => Some(observed),
            Err(error) => {
                log::warn(
                    "dispatch.observe_failed",
                    json!({ "session": session, "observation": label, "error": error }),
                );
                None
            }
        }
    }

    fn observe_sessions(&mut self, herdr: &HerdrClient, now_ms: u64) -> Result<(), String> {
        let state = self.store.fleet_state()?;
        for session in state.sessions.sessions.values() {
            if session.state.is_ended() || session.stop_requested {
                continue;
            }
            match &session.state {
                SessionState::Placing => self.advance_placing(herdr, &session.identity, now_ms),
                SessionState::HandoffReceived { .. } | SessionState::Captured => {}
                SessionState::Starting => self.reconcile_starting(herdr, session, now_ms),
                _ => self.observe_status(herdr, session, now_ms),
            }
        }
        Ok(())
    }

    fn status(herdr: &HerdrClient, session: &FleetSession) -> Option<AgentObservation> {
        let agent = session.agent_name.as_deref()?;
        match herdr.status(&session.machine, agent) {
            Ok(observation) => Some(observation),
            Err(failure) => {
                log::warn(
                    "herdr.status_failed",
                    json!({ "session": session.identity, "failure": failure.to_string() }),
                );
                None
            }
        }
    }

    fn observe_status(&mut self, herdr: &HerdrClient, session: &FleetSession, now_ms: u64) {
        let Some(observation) = Self::status(herdr, session) else {
            return;
        };
        match observation {
            AgentObservation::Gone => {
                self.observe(&session.identity, Observation::Gone, now_ms);
            }
            AgentObservation::Status(status) => {
                // A prompt whose delivery was unknown is settled by the next
                // known status: the agent either took the turn or is idle.
                if session.prompt_pending && status != HerdrStatus::Unknown {
                    self.observe(&session.identity, Observation::Prompted, now_ms);
                }
                self.observe(
                    &session.identity,
                    Observation::Status {
                        status: core_status(status),
                    },
                    now_ms,
                );
            }
        }
    }

    /// A started agent whose first prompt is unconfirmed: a working or
    /// blocked agent took it; an idle one did not and is prompted again.
    fn reconcile_starting(&mut self, herdr: &HerdrClient, session: &FleetSession, now_ms: u64) {
        match Self::status(herdr, session) {
            Some(AgentObservation::Gone) => {
                self.observe(&session.identity, Observation::Gone, now_ms);
            }
            Some(AgentObservation::Status(
                status @ (HerdrStatus::Working | HerdrStatus::Blocked),
            )) => {
                self.observe(&session.identity, Observation::Prompted, now_ms);
                self.observe(
                    &session.identity,
                    Observation::Status {
                        status: core_status(status),
                    },
                    now_ms,
                );
            }
            Some(AgentObservation::Status(HerdrStatus::Idle | HerdrStatus::Done)) => {
                self.send_first_prompt(herdr, &session.identity, now_ms);
            }
            Some(AgentObservation::Status(HerdrStatus::Unknown)) | None => {}
        }
    }

    fn perform(
        &mut self,
        herdr: &HerdrClient,
        effect: &FleetEffect,
        now_ms: u64,
        occupancy: &Occupancy,
        foreign: Option<&[ForeignClaim]>,
    ) -> bool {
        let outcome = match effect {
            FleetEffect::ClaimNext => self.claim_next(herdr, now_ms, occupancy, foreign),
            FleetEffect::Place { attempt, fence } => {
                self.place(herdr, attempt, *fence, now_ms, occupancy)
            }
            FleetEffect::Prompt { session, text } => self.prompt(herdr, session, text, now_ms),
            FleetEffect::Renew { attempt, fence } => self.renew(attempt, *fence, now_ms),
            FleetEffect::Capture { session, commit } => {
                self.capture(herdr, session, commit, now_ms)
            }
            FleetEffect::RaiseDecision { session, question } => {
                self.raise_decision(session, question, now_ms)
            }
            FleetEffect::Cancel {
                attempt,
                fence,
                cause,
            } => self.cancel(attempt, *fence, cause, now_ms),
            FleetEffect::Stop { session } => self.stop(herdr, session, now_ms),
        };
        match outcome {
            Ok(performed) => performed,
            Err(error) => {
                log::warn(
                    "dispatch.effect_failed",
                    json!({ "effect": effect, "error": error }),
                );
                false
            }
        }
    }

    // r[impl kiln.fleet.dispatch]
    fn claim_next(
        &mut self,
        herdr: &HerdrClient,
        now_ms: u64,
        occupancy: &Occupancy,
        foreign: Option<&[ForeignClaim]>,
    ) -> Result<bool, String> {
        let Some(foreign) = foreign else {
            // Foreign claims could not be read: take no new work blind.
            return Ok(false);
        };
        let config = Arc::clone(&self.service.config);
        let state = self.store.fleet_state()?;
        let held: Vec<FleetScope> = state
            .claims
            .live(now_ms)
            .map(|claim| claim.scope.clone())
            .chain(foreign.iter().map(|claim| claim.scope.clone()))
            .collect();
        let inventory = herdr.inventory();
        let chosen: RefCell<Option<(ContentId, String)>> = RefCell::new(None);
        let defer = |work: &ImplementationWork| -> Option<String> {
            if !work.permitted_effects.contains(INTERACTIVE_AGENT_EFFECT) {
                return Some("work does not permit interactive_agent; left for Lattice".to_owned());
            }
            if git_base(work).is_none() {
                return Some("work has no exact git base commit".to_owned());
            }
            let scopes = work_scopes(work);
            if let Some(scope) = held
                .iter()
                .find(|claim| scopes.iter().any(|scope| overlaps(scope, claim)))
            {
                return Some(format!("overlaps the live claim on {}", scope.text()));
            }
            let Some(machine) = choose_machine(
                inventory,
                occupancy,
                &config.dispatch.kinds,
                &work.repository,
            ) else {
                return Some(format!(
                    "no free slot on a machine with a checkout of {}",
                    work.repository
                ));
            };
            let identity = work.identity().ok()?;
            chosen.replace(Some((identity, machine)));
            None
        };
        let selection =
            self.store
                .claim_next_deferred(now_ms, config.policy.attempt_lease_ms, &defer)?;
        if !selection.deferred.is_empty() {
            log::info(
                "dispatch.deferred",
                json!({ "deferred": selection.deferred }),
            );
        }
        let Some(attempt) = selection.attempt else {
            return Ok(true);
        };
        let Some((work_identity, machine)) = chosen.into_inner() else {
            return Err("claimed work had no chosen machine".to_owned());
        };
        if work_identity != attempt.work_identity {
            self.abandon(
                &attempt,
                "claimed work differs from the placed work",
                now_ms,
            );
            return Ok(true);
        }
        let work = self
            .store
            .active_work(&attempt.identity, attempt.fence, now_ms)?;
        let Some(machine) = inventory.machine(&machine).cloned() else {
            return Err("chosen machine left the inventory".to_owned());
        };
        let plan = match interactive_plan(&work, &attempt, &machine) {
            Ok(plan) => plan,
            Err(error) => {
                self.abandon(&attempt, &error, now_ms);
                return Ok(true);
            }
        };
        if let Err(error) =
            self.store
                .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, now_ms, &plan)
        {
            self.abandon(
                &attempt,
                &format!("interactive plan refused: {error}"),
                now_ms,
            );
            return Ok(true);
        }
        let short = short_identity(&attempt.identity);
        let scope = work_scopes(&work)
            .into_iter()
            .next()
            .ok_or("work has no claimable scope")?;
        let (outcome, _) = self.store.fleet_claim(
            ClaimCommand::Acquire {
                scope,
                holder: FleetAgent {
                    tool: KILN_HOLDER_TOOL.to_owned(),
                    host: config.machine.clone(),
                    session: short.clone(),
                },
                purpose: format!("Kiln implementation attempt {short}"),
                lease_ms: config.policy.claim_lease_ms,
                attempt: Some(attempt.identity.clone()),
            },
            now_ms,
            &config.machine,
            foreign,
        )?;
        if let ClaimOutcome::Conflict(conflict) = outcome {
            self.abandon(
                &attempt,
                &format!("attempt scope conflicts with {}", json!(conflict)),
                now_ms,
            );
            return Ok(true);
        }
        log::info(
            "dispatch.claimed",
            json!({ "attempt": attempt.identity, "fence": attempt.fence, "work": attempt.work_identity, "machine": machine.label }),
        );
        Ok(true)
    }

    fn abandon(&mut self, attempt: &ImplementationAttempt, cause: &str, now_ms: u64) {
        let cause = bounded_reason(cause);
        log::warn(
            "dispatch.abandoned",
            json!({ "attempt": attempt.identity, "cause": cause }),
        );
        if let Err(error) = self
            .store
            .cancel(&attempt.identity, attempt.fence, now_ms, &cause)
        {
            log::warn(
                "dispatch.cancel_failed",
                json!({ "attempt": attempt.identity, "error": error }),
            );
        }
    }

    fn place(
        &mut self,
        herdr: &HerdrClient,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
        occupancy: &Occupancy,
    ) -> Result<bool, String> {
        let config = Arc::clone(&self.service.config);
        let unplaced = |store: &mut Implementer, reason: &str| -> Result<bool, String> {
            log::info(
                "dispatch.place_deferred",
                json!({ "attempt": attempt, "reason": reason }),
            );
            store.fleet_clear_placing(attempt)?;
            Ok(false)
        };
        let Some(plan) = self.store.dispatch_plan(attempt)? else {
            return unplaced(&mut self.store, "attempt has no dispatch plan");
        };
        let record = self
            .store
            .attempt(attempt)?
            .ok_or("attempt is not retained")?;
        let work = self.store.active_work(attempt, fence, now_ms)?;
        let inventory = herdr.inventory();
        let Some(machine) = planned_machine(inventory, &plan, &record, &work) else {
            return unplaced(
                &mut self.store,
                "no inventory machine matches the planned workspace",
            );
        };
        let only: Occupancy = Occupancy {
            machines: occupancy
                .machines
                .iter()
                .filter(|(label, _)| **label == machine.label)
                .map(|(label, load)| (label.clone(), load.clone()))
                .collect(),
        };
        let Some(chosen) = place(inventory, &only, &config.dispatch.kinds) else {
            return unplaced(&mut self.store, "the planned machine has no free slot");
        };
        let session = match self.store.fleet_bind_session(
            Placement {
                attempt: attempt.clone(),
                fence,
                work: record.work_identity.clone(),
                work_revision: record.work_revision,
                machine: chosen.machine.clone(),
                kind: chosen.kind.as_str().to_owned(),
            },
            now_ms,
        ) {
            Ok(session) => session,
            Err(error) => return unplaced(&mut self.store, &error),
        };
        log::info(
            "dispatch.placed",
            json!({ "attempt": attempt, "session": session.identity, "machine": chosen.machine, "kind": chosen.kind.as_str() }),
        );
        self.advance_placing(herdr, &session.identity, now_ms);
        Ok(true)
    }

    /// Bring a `Placing` session to a started, prompted agent. Every step is
    /// reconciled first, so a retry after an unknown outcome never creates a
    /// second worktree or agent.
    fn advance_placing(&mut self, herdr: &HerdrClient, session_id: &ContentId, now_ms: u64) {
        if let Err(error) = self.try_advance_placing(herdr, session_id, now_ms) {
            log::warn(
                "dispatch.placing_pending",
                json!({ "session": session_id, "error": error }),
            );
        }
    }

    fn try_advance_placing(
        &mut self,
        herdr: &HerdrClient,
        session_id: &ContentId,
        now_ms: u64,
    ) -> Result<(), String> {
        let state = self.store.fleet_state()?;
        let session = state
            .sessions
            .sessions
            .get(session_id)
            .ok_or("session is not recorded")?
            .clone();
        if session.state != SessionState::Placing {
            return Ok(());
        }
        let attempt = self
            .store
            .attempt(&session.attempt)?
            .ok_or("attempt is not retained")?;
        let work = self
            .store
            .active_work(&attempt.identity, attempt.fence, now_ms)?;
        let base = git_base(&work).ok_or("work has no exact git base commit")?;
        let machine = herdr
            .inventory()
            .machine(&session.machine)
            .cloned()
            .ok_or("session machine left the inventory")?;
        let kind = AgentKind::parse(&session.kind).ok_or("session kind is not admitted")?;
        let name = agent_name(&attempt.identity)?;
        let branch = branch_name(
            &work.identity().map_err(|error| error.message)?,
            &attempt.identity,
        );
        let path = worktree_path(&machine, &attempt.identity)?;

        let agents = herdr
            .list_agents(&machine.label)
            .map_err(|failure| failure.to_string())?;
        if let Some(agent) = agents
            .iter()
            .find(|agent| agent.name.as_deref() == Some(name.as_str()) && agent.kind.is_some())
        {
            let workspace =
                SessionWorkspace::new(&agent.workspace_id, &agent.pane_id, &path, &branch, &base)
                    .map_err(|error| error.message)?;
            return self.started(herdr, &session, name, workspace, now_ms);
        }
        let worktrees = herdr
            .list_worktrees(&machine.label, &work.repository)
            .map_err(|failure| failure.to_string())?;
        if worktrees.iter().any(|worktree| worktree.path == path) {
            self.observe(
                &session.identity,
                Observation::StartFailed {
                    reason: "an unconfirmed worktree exists without a recorded pane".to_owned(),
                },
                now_ms,
            );
            return Ok(());
        }
        // The launch effects below start a credentialed agent. Its exact
        // Dispatch request is admitted again here, so a grant revoked or a
        // fence replaced since the bind starts nothing; a denial leaves the
        // session placing for the next tick.
        self.store
            .fleet_authorize_launch(&session.identity, now_ms)?;
        let created = match herdr.create_worktree(
            &machine.label,
            &work.repository,
            &branch,
            &base,
            &attempt.identity,
        ) {
            Ok(created) => created,
            Err(failure) => return self.start_failure(&session, &failure, now_ms),
        };
        if let Err(failure) = herdr.start_agent(&machine.label, &created.pane_id, &name, kind) {
            if failure.class == FailureClass::Rejected {
                let _ = herdr.close_workspace(&machine.label, &created.workspace_id);
            }
            return self.start_failure(&session, &failure, now_ms);
        }
        let workspace = SessionWorkspace::new(
            &created.workspace_id,
            &created.pane_id,
            &created.path,
            &created.branch,
            &base,
        )
        .map_err(|error| error.message)?;
        self.started(herdr, &session, name, workspace, now_ms)
    }

    fn start_failure(
        &mut self,
        session: &FleetSession,
        failure: &HerdrFailure,
        now_ms: u64,
    ) -> Result<(), String> {
        if failure.class == FailureClass::Rejected {
            self.observe(
                &session.identity,
                Observation::StartFailed {
                    reason: bounded_reason(&failure.to_string()),
                },
                now_ms,
            );
            return Ok(());
        }
        // Unavailable changed nothing; unknown is reconciled by listing on
        // the next tick before any retry.
        Err(failure.to_string())
    }

    fn started(
        &mut self,
        herdr: &HerdrClient,
        session: &FleetSession,
        name: String,
        workspace: SessionWorkspace,
        now_ms: u64,
    ) -> Result<(), String> {
        if self
            .observe(
                &session.identity,
                Observation::Started {
                    agent_name: name,
                    workspace,
                },
                now_ms,
            )
            .is_none()
        {
            return Err("the started agent could not be recorded".to_owned());
        }
        let reference = ExternalRef::new(
            SESSION_REFERENCE_NAMESPACE,
            SESSION_REFERENCE_KIND,
            session.identity.as_str(),
        )
        .map_err(|error| error.message)?;
        if let Err(error) =
            self.store
                .start_execution(&session.attempt, session.fence, now_ms, &reference)
        {
            log::warn(
                "dispatch.execution_failed",
                json!({ "session": session.identity, "error": error }),
            );
        }
        self.send_first_prompt(herdr, &session.identity, now_ms);
        Ok(())
    }

    fn send_first_prompt(&mut self, herdr: &HerdrClient, session_id: &ContentId, now_ms: u64) {
        let outcome = (|| -> Result<(), String> {
            let state = self.store.fleet_state()?;
            let session = state
                .sessions
                .sessions
                .get(session_id)
                .ok_or("session is not recorded")?;
            let workspace = session
                .workspace
                .as_ref()
                .ok_or("session has no workspace")?;
            let agent = session
                .agent_name
                .as_deref()
                .ok_or("session has no agent")?;
            let attempt = self
                .store
                .attempt(&session.attempt)?
                .ok_or("attempt is not retained")?;
            let work = self
                .store
                .active_work(&attempt.identity, attempt.fence, now_ms)?;
            let text =
                render_attempt_prompt(&work, &attempt, workspace, &HandoffInstructions::default())
                    .map_err(|error| error.message)?;
            herdr
                .prompt(&session.machine, agent, &text)
                .map_err(|failure| failure.to_string())?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.observe(session_id, Observation::Prompted, now_ms);
            }
            Err(error) => log::warn(
                "dispatch.first_prompt_failed",
                json!({ "session": session_id, "error": error }),
            ),
        }
    }

    fn session(&mut self, session: &ContentId) -> Result<FleetSession, String> {
        self.store
            .fleet_state()?
            .sessions
            .sessions
            .get(session)
            .cloned()
            .ok_or_else(|| "fleet_session_unknown: no such fleet session".to_owned())
    }

    fn prompt(
        &mut self,
        herdr: &HerdrClient,
        session_id: &ContentId,
        text: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        let session = self.session(session_id)?;
        let agent = session
            .agent_name
            .as_deref()
            .ok_or("session has no agent")?;
        match herdr.prompt(&session.machine, agent, text) {
            Ok(()) => {
                self.observe(session_id, Observation::Prompted, now_ms);
                Ok(true)
            }
            // The prompt may have landed: record it as issued; the next known
            // status settles it.
            Err(failure) if failure.class == FailureClass::Unknown => Ok(true),
            Err(failure) => Err(failure.to_string()),
        }
    }

    fn renew(&mut self, attempt: &ContentId, fence: u64, now_ms: u64) -> Result<bool, String> {
        let config = Arc::clone(&self.service.config);
        self.store
            .renew(attempt, fence, now_ms, config.policy.attempt_lease_ms)?;
        let state = self.store.fleet_state()?;
        let claims: Vec<_> = state
            .claims
            .live(now_ms)
            .filter(|claim| claim.attempt.as_ref() == Some(attempt))
            .map(|claim| (claim.identity.clone(), claim.fence))
            .collect();
        for (identity, claim_fence) in claims {
            if let Err(error) = self.store.fleet_claim(
                ClaimCommand::Renew {
                    identity: identity.clone(),
                    fence: claim_fence,
                    lease_ms: config.policy.claim_lease_ms,
                },
                now_ms,
                &config.machine,
                &[],
            ) {
                log::warn(
                    "dispatch.claim_renew_failed",
                    json!({ "claim": identity, "error": error }),
                );
            }
        }
        Ok(true)
    }

    // r[impl kiln.fleet.decisions]
    fn raise_decision(
        &mut self,
        session_id: &ContentId,
        question: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        let decision = self
            .store
            .fleet_raise_decision(session_id, question.to_owned(), now_ms)?;
        let config = Arc::clone(&self.service.config);
        log::info(
            "dispatch.decision",
            json!({ "decision": decision.identity, "session": session_id }),
        );
        if let Some(swarm) = &self.service.swarm {
            let subject = format!(
                "{DECISION_SUBJECT_PREFIX} {} attempt {}",
                config.machine,
                short_identity(&decision.attempt)
            );
            let body = format!(
                "{}\n\nAnswer from an operator machine:\nkiln-fleet answer --decision {} --text 'ANSWER'\n\n— kiln on {}\n",
                decision.question,
                decision.identity.as_str(),
                config.machine
            );
            if let Err(error) = swarm.post_article(&config.decision_group, &subject, &body) {
                log::warn(
                    "dispatch.decision_post_failed",
                    json!({ "decision": decision.identity, "error": error }),
                );
            }
        }
        Ok(true)
    }

    fn cancel(
        &mut self,
        attempt: &ContentId,
        fence: u64,
        cause: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        self.store
            .cancel(attempt, fence, now_ms, &bounded_reason(cause))?;
        let state = self.store.fleet_state()?;
        let live: Vec<ContentId> = state
            .sessions
            .sessions
            .values()
            .filter(|session| {
                &session.attempt == attempt && session.fence == fence && !session.state.is_ended()
            })
            .map(|session| session.identity.clone())
            .collect();
        for session in live {
            self.observe(
                &session,
                Observation::Cancelled {
                    cause: bounded_reason(cause),
                },
                now_ms,
            );
        }
        Ok(true)
    }

    fn stop(
        &mut self,
        herdr: &HerdrClient,
        session_id: &ContentId,
        now_ms: u64,
    ) -> Result<bool, String> {
        let session = self.session(session_id)?;
        let tolerated = |outcome: Result<(), HerdrFailure>| match outcome {
            Ok(()) => Ok(()),
            Err(failure) if failure.class == FailureClass::Rejected => Ok(()),
            Err(failure) => Err(failure.to_string()),
        };
        if let Some(agent) = &session.agent_name {
            tolerated(herdr.interrupt(&session.machine, agent))?;
        }
        if let Some(workspace) = &session.workspace {
            tolerated(herdr.close_workspace(&session.machine, &workspace.herdr_workspace))?;
        }
        let config = Arc::clone(&self.service.config);
        let state = self.store.fleet_state()?;
        let claims: Vec<_> = state
            .claims
            .claims
            .values()
            .filter(|claim| {
                claim.attempt.as_ref() == Some(&session.attempt)
                    && claim.state == kiln_core::fleet::ClaimState::Open
            })
            .map(|claim| (claim.identity.clone(), claim.fence))
            .collect();
        for (identity, fence) in claims {
            self.store.fleet_claim(
                ClaimCommand::Release {
                    identity,
                    fence,
                    note: STOP_NOTE.to_owned(),
                },
                now_ms,
                &config.machine,
                &[],
            )?;
        }
        if let Some(attempt) = self.store.attempt(&session.attempt)?
            && attempt.fence == session.fence
            && attempt.failure == Some(AttemptFailure::Unknown)
        {
            let termination = ExternalRef::new(
                SESSION_REFERENCE_NAMESPACE,
                WORKSPACE_CLOSED_KIND,
                session.identity.as_str(),
            )
            .map_err(|error| error.message)?;
            if let Err(error) =
                self.store
                    .reconcile_unknown(&attempt.identity, attempt.fence, &termination)
            {
                log::warn(
                    "dispatch.reconcile_failed",
                    json!({ "attempt": attempt.identity, "error": error }),
                );
            }
        }
        log::info(
            "dispatch.stopped",
            json!({ "session": session_id, "attempt": session.attempt }),
        );
        Ok(true)
    }

    // r[impl kiln.fleet.capture]
    fn capture(
        &mut self,
        herdr: &HerdrClient,
        session_id: &ContentId,
        commit: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        let outcome = self.try_capture(herdr, session_id, commit, now_ms);
        let observation = match &outcome {
            Ok(_) => Observation::Captured,
            Err(error) => Observation::CaptureFailed {
                reason: bounded_reason(error),
            },
        };
        log::event(
            if outcome.is_ok() { "info" } else { "warn" },
            "dispatch.capture",
            json!({ "session": session_id, "commit": commit, "error": outcome.as_ref().err() }),
        );
        self.store.fleet_observe(session_id, observation, now_ms)?;
        if let (Ok(placeholders), Some(capturer)) = (outcome, self.capturer.clone()) {
            let config = Arc::clone(&self.service.config);
            let spawned = std::thread::Builder::new()
                .name("kiln-fleet-verify".into())
                .spawn(move || capturer.run_commands(&config, &placeholders));
            if let Err(error) = spawned {
                log::warn(
                    "dispatch.verify_spawn_failed",
                    json!({ "error": error.to_string() }),
                );
            }
        }
        Ok(true)
    }

    fn try_capture(
        &mut self,
        herdr: &HerdrClient,
        session_id: &ContentId,
        commit: &str,
        now_ms: u64,
    ) -> Result<Placeholders, String> {
        let capturer = self.capturer.clone().ok_or("capture is not configured")?;
        let session = self.session(session_id)?;
        let workspace = session
            .workspace
            .clone()
            .ok_or("session has no workspace")?;
        let attempt = self
            .store
            .attempt(&session.attempt)?
            .ok_or("attempt is not retained")?;
        if attempt.fence != session.fence {
            return Err("the attempt fence moved past this session".to_owned());
        }
        let work = self
            .store
            .active_work(&attempt.identity, attempt.fence, now_ms)?;
        let plan = self
            .store
            .dispatch_plan(&attempt.identity)?
            .ok_or("attempt has no dispatch plan")?;
        let machine = herdr
            .inventory()
            .machine(&session.machine)
            .ok_or("session machine left the inventory")?;
        let location = fetch_location(machine, &self.service.config.machine, &workspace.path)?;
        let source = HandoffSource {
            location,
            branch: workspace.branch.clone(),
            commit: commit.to_owned(),
            session: session.identity.clone(),
        };
        let prepared = capturer.prepare(&work, &attempt, &plan, &source)?;
        log::info(
            "dispatch.exported",
            json!({ "session": session_id, "export": prepared.candidate_root.display().to_string() }),
        );
        let handoff = Capturer::materialize(&prepared, &work, &attempt, &plan)?;
        self.store.admit_candidate_handoff(
            &work,
            &attempt.identity,
            attempt.fence,
            now_ms,
            &handoff,
        )?;
        Ok(Placeholders {
            attempt: attempt.identity.as_str().to_owned(),
            fence: attempt.fence,
            verification_profile: prepared.profile_path,
            authority: prepared.authority_path,
            handoff: prepared.handoff_path,
            repository: work.repository,
        })
    }
}

/// Free slots of the dispatch kinds on machines observed this round.
#[must_use]
pub fn free_slots(inventory: &HerdrInventory, occupancy: &Occupancy, kinds: &[AgentKind]) -> u32 {
    inventory
        .machines
        .iter()
        .filter_map(|machine| match occupancy.machines.get(&machine.label) {
            Some(MachineLoad::Available(counts)) => Some(
                kinds
                    .iter()
                    .map(|kind| {
                        machine
                            .slots(*kind)
                            .saturating_sub(counts.get(kind).copied().unwrap_or(0))
                    })
                    .fold(0_u32, u32::saturating_add),
            ),
            _ => None,
        })
        .fold(0_u32, u32::saturating_add)
}

/// The machine the provider would place `repository`'s work on now.
#[must_use]
pub fn choose_machine(
    inventory: &HerdrInventory,
    occupancy: &Occupancy,
    kinds: &[AgentKind],
    repository: &str,
) -> Option<String> {
    let with_checkout: BTreeSet<&str> = inventory
        .machines
        .iter()
        .filter(|machine| machine.checkout(repository).is_some())
        .map(|machine| machine.label.as_str())
        .collect();
    let filtered = Occupancy {
        machines: occupancy
            .machines
            .iter()
            .filter(|(label, _)| with_checkout.contains(label.as_str()))
            .map(|(label, load)| (label.clone(), load.clone()))
            .collect(),
    };
    place(inventory, &filtered, kinds).map(|placement| placement.machine)
}

/// The exact git base commit of the work.
#[must_use]
pub fn git_base(work: &ImplementationWork) -> Option<String> {
    work.base
        .as_ref()
        .filter(|base| {
            base.provider_namespace.as_str() == "git" && base.native_kind.as_str() == "commit"
        })
        .map(|base| base.native_id.as_str().to_owned())
}

/// Fleet scopes of the work's permitted paths: each path up to its first
/// glob segment, the whole repository when a path is empty, all glob or
/// unparseable (never narrower than the work).
#[must_use]
pub fn work_scopes(work: &ImplementationWork) -> Vec<FleetScope> {
    let whole = || parse_scope(&work.repository, kiln_core::fleet::WHOLE_REPOSITORY_PATH).ok();
    if work.permitted_paths.is_empty() {
        return whole().into_iter().collect();
    }
    let mut scopes = Vec::new();
    for path in &work.permitted_paths {
        let root: Vec<&str> = path
            .split('/')
            .take_while(|segment| !segment.contains(['*', '?', '[', '{']))
            .collect();
        let root = root.join("/");
        let scope = if root.is_empty() {
            whole()
        } else {
            parse_scope(&work.repository, &root).ok().or_else(whole)
        };
        if let Some(scope) = scope
            && !scopes.contains(&scope)
        {
            scopes.push(scope);
        }
    }
    scopes
}

/// `<worktree_root>/<attempt12>`, the path the herdr adapter creates.
///
/// # Errors
/// Returns a diagnostic for an invalid attempt identity.
pub fn worktree_path(machine: &HerdrMachine, attempt: &ContentId) -> Result<String, String> {
    Ok(format!(
        "{}/{}",
        machine.worktree_root.trim_end_matches('/'),
        attempt_short(attempt)?
    ))
}

/// BLAKE3 over the attempt, fence, machine, worktree path, branch and base:
/// the interactive plan's workspace identity, which the herdr receipt binds.
///
/// # Errors
/// Returns a diagnostic for an invalid identity.
pub fn workspace_identity(
    attempt: &ContentId,
    fence: u64,
    machine: &str,
    path: &str,
    branch: &str,
    base: &str,
) -> Result<ContentId, String> {
    let mut hasher = blake3::Hasher::new();
    for field in [WORKSPACE_IDENTITY_SCHEMA, attempt.as_str()] {
        length_prefixed(&mut hasher, field);
    }
    hasher.update(&fence.to_be_bytes());
    for field in [machine, path, branch, base] {
        length_prefixed(&mut hasher, field);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .map_err(|error| error.message)
}

fn length_prefixed(hasher: &mut blake3::Hasher, field: &str) {
    hasher.update(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(field.as_bytes());
}

fn expected_output() -> Result<ContentId, String> {
    ContentId::from_blake3_hex(
        blake3::hash(EXPECTED_OUTPUT_SCHEMA.as_bytes())
            .to_hex()
            .to_string(),
    )
    .map_err(|error| error.message)
}

// r[impl kiln.fleet.transport]
/// The interactive dispatch plan for one claimed attempt on `machine`.
///
/// # Errors
/// Returns a diagnostic when the work has no git base or identities fail.
pub fn interactive_plan(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    machine: &HerdrMachine,
) -> Result<ImplementationDispatchPlan, String> {
    let work_identity = work.identity().map_err(|error| error.message)?;
    let base = git_base(work).ok_or("work has no exact git base commit")?;
    let branch = branch_name(&work_identity, &attempt.identity);
    let path = worktree_path(machine, &attempt.identity)?;
    let mut paths = Vec::new();
    for path in &work.permitted_paths {
        if !paths.contains(path) {
            paths.push(path.clone());
        }
    }
    Ok(ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.to_owned(),
        work_identity,
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: workspace_identity(
                &attempt.identity,
                attempt.fence,
                &machine.label,
                &path,
                &branch,
                &base,
            )?,
            isolation: WorkspaceIsolation::InteractiveWorktree,
            network: true,
            credentials: true,
            projection: None,
        },
        provider_profile: ProviderProfileId::new(INTERACTIVE_PROVIDER_PROFILE)
            .map_err(|error| error.message)?,
        permitted_paths: paths,
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: work
            .budget
            .minutes_per_attempt
            .min(MAX_INTERACTIVE_DEADLINE_MINUTES),
        token_budget: INTERACTIVE_TOKEN_BUDGET,
        invocation_budget: INTERACTIVE_INVOCATION_BUDGET,
        expected_output: expected_output()?,
        execution: None,
    })
}

/// The inventory machine whose workspace identity the plan carries.
#[must_use]
pub fn planned_machine<'a>(
    inventory: &'a HerdrInventory,
    plan: &ImplementationDispatchPlan,
    attempt: &ImplementationAttempt,
    work: &ImplementationWork,
) -> Option<&'a HerdrMachine> {
    let work_identity = work.identity().ok()?;
    let base = git_base(work)?;
    let branch = branch_name(&work_identity, &attempt.identity);
    inventory.machines.iter().find(|machine| {
        worktree_path(machine, &attempt.identity)
            .and_then(|path| {
                workspace_identity(
                    &attempt.identity,
                    attempt.fence,
                    &machine.label,
                    &path,
                    &branch,
                    &base,
                )
            })
            .is_ok_and(|identity| identity == plan.workspace.identity)
    })
}

const fn core_status(status: HerdrStatus) -> AgentStatus {
    match status {
        HerdrStatus::Idle => AgentStatus::Idle,
        HerdrStatus::Working => AgentStatus::Working,
        HerdrStatus::Blocked => AgentStatus::Blocked,
        HerdrStatus::Done => AgentStatus::Done,
        HerdrStatus::Unknown => AgentStatus::Unknown,
    }
}

/// Map a herdr status observation onto the core observation.
#[must_use]
pub const fn core_observation(observation: AgentObservation) -> Observation {
    match observation {
        AgentObservation::Gone => Observation::Gone,
        AgentObservation::Status(status) => Observation::Status {
            status: core_status(status),
        },
    }
}

fn short_identity(identity: &ContentId) -> String {
    identity
        .as_str()
        .chars()
        .take(SHORT_IDENTITY_CHARS)
        .collect()
}

/// Where Kiln fetches a session's commit: through the machine's ssh target,
/// which keeps Kiln's account out of the agents' homes, or, only on the
/// coordinator without one, from the worktree path directly.
fn fetch_location(machine: &HerdrMachine, coordinator: &str, path: &str) -> Result<String, String> {
    match &machine.ssh_target {
        Some(target) => Ok(format!("{target}:{path}")),
        None if machine.label == coordinator => Ok(path.to_owned()),
        None => Err(format!(
            "machine {} has no ssh_target to fetch handoffs from",
            machine.label
        )),
    }
}

fn bounded_reason(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_REASON_CHARS)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "unspecified".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_adapter_herdr::{AgentSlots, HerdrEnvironment, INVENTORY_SCHEMA, KindCounts};
    use std::collections::BTreeMap;

    fn machine(label: &str, repository: bool, slots: u32) -> HerdrMachine {
        HerdrMachine {
            label: label.into(),
            herdr_machine: None,
            ssh_target: Some(format!("kiln@{label}")),
            worktree_root: "/srv/worktrees".into(),
            repositories: if repository {
                BTreeMap::from([("OnixResearch/kiln".into(), "/srv/kiln".into())])
            } else {
                BTreeMap::new()
            },
            agents: vec![AgentSlots {
                kind: AgentKind::Omp,
                slots,
                args: vec![],
            }],
        }
    }

    fn inventory() -> HerdrInventory {
        HerdrInventory {
            schema: INVENTORY_SCHEMA.into(),
            executable: "/bin/herdr".into(),
            session: "kiln".into(),
            environment: HerdrEnvironment {
                home: "/home/kiln".into(),
                path: "/bin".into(),
                xdg_runtime_dir: None,
                xdg_config_home: None,
                ssh_auth_sock: None,
            },
            timeout_ms: 1_000,
            start_timeout_ms: 1_000,
            max_output_bytes: 4_096,
            machines: vec![machine("aspen1", false, 4), machine("aspen3", true, 2)],
        }
    }

    #[test]
    // r[verify kiln.fleet.capture]
    fn handoffs_are_fetched_through_the_ssh_target_even_on_the_coordinator() {
        let path = "/srv/worktrees/aaaaaaaaaaaa";
        let mut local = machine("britton-desktop", true, 1);
        assert_eq!(
            fetch_location(&local, "britton-desktop", path),
            Ok(format!("kiln@britton-desktop:{path}"))
        );
        assert_eq!(
            fetch_location(&machine("aspen3", true, 1), "britton-desktop", path),
            Ok(format!("kiln@aspen3:{path}"))
        );
        local.ssh_target = None;
        assert_eq!(
            fetch_location(&local, "britton-desktop", path),
            Ok(path.to_owned())
        );
        let mut remote = machine("aspen3", true, 1);
        remote.ssh_target = None;
        assert!(fetch_location(&remote, "britton-desktop", path).is_err());
    }

    #[test]
    // r[verify kiln.fleet.dispatch]
    fn free_slots_and_placement_follow_observed_occupancy_and_checkouts() {
        let inventory = inventory();
        let counts = |used: u32| MachineLoad::Available(KindCounts::from([(AgentKind::Omp, used)]));
        let occupancy = Occupancy {
            machines: BTreeMap::from([("aspen1".into(), counts(1)), ("aspen3".into(), counts(1))]),
        };
        assert_eq!(free_slots(&inventory, &occupancy, &[AgentKind::Omp]), 4);
        assert_eq!(free_slots(&inventory, &occupancy, &[AgentKind::Claude]), 0);
        assert_eq!(
            choose_machine(
                &inventory,
                &occupancy,
                &[AgentKind::Omp],
                "onixresearch/KILN"
            ),
            Some("aspen3".to_owned())
        );
        assert_eq!(
            choose_machine(&inventory, &occupancy, &[AgentKind::Omp], "other/repo"),
            None
        );
        let full = Occupancy {
            machines: BTreeMap::from([("aspen3".into(), counts(2))]),
        };
        assert_eq!(free_slots(&inventory, &full, &[AgentKind::Omp]), 0);
        assert_eq!(
            choose_machine(&inventory, &full, &[AgentKind::Omp], "OnixResearch/kiln"),
            None
        );
    }

    #[test]
    fn statuses_map_one_to_one_and_missing_agents_are_gone() {
        assert_eq!(core_observation(AgentObservation::Gone), Observation::Gone);
        for (herdr, core) in [
            (HerdrStatus::Idle, AgentStatus::Idle),
            (HerdrStatus::Working, AgentStatus::Working),
            (HerdrStatus::Blocked, AgentStatus::Blocked),
            (HerdrStatus::Done, AgentStatus::Done),
            (HerdrStatus::Unknown, AgentStatus::Unknown),
        ] {
            assert_eq!(
                core_observation(AgentObservation::Status(herdr)),
                Observation::Status { status: core }
            );
        }
    }

    #[test]
    fn workspace_identity_binds_every_field() {
        let attempt = ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!());
        let base = workspace_identity(&attempt, 1, "aspen3", "/w/a", "b", "c");
        assert!(base.is_ok());
        for other in [
            workspace_identity(&attempt, 2, "aspen3", "/w/a", "b", "c"),
            workspace_identity(&attempt, 1, "aspen1", "/w/a", "b", "c"),
            workspace_identity(&attempt, 1, "aspen3", "/w/b", "b", "c"),
            workspace_identity(&attempt, 1, "aspen3", "/w/a", "x", "c"),
            workspace_identity(&attempt, 1, "aspen3", "/w/a", "b", "d"),
        ] {
            assert_ne!(other, base);
        }
    }
}
