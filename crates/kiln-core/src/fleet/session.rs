//! Interactive agent sessions and the pure dispatch tick.
//!
//! A session follows one implementation attempt in one interactive agent from
//! placement to a captured handoff. [`observe`] admits provider observations;
//! [`plan_tick`] decides the next effects from recorded state alone; and
//! [`record_issued`] records the effects a shell performed so that a repeated
//! tick before new observations requests none of them again.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::mem::discriminant;

use serde::{Deserialize, Serialize};

use super::decision::{Decision, DecisionLedger, MAX_QUESTION_BYTES, answer_prompt};
use super::prompt::{HandoffInstructions, render_nudge};
use super::{bounded_line, bounded_token, content_identity, push_field, refusal, short};
use crate::implementation::{AttemptPhase, DEFAULT_LEASE_MS, MAX_LEASE_MS};
use crate::types::{ContentId, CoreResult};

pub const FLEET_SESSION_SCHEMA: &str = "kiln.fleet-session.v1";
pub const FLEET_WORKSPACE_SCHEMA: &str = "kiln.fleet-workspace.v1";
/// Bound for machine labels, agent kinds and names, panes and branches.
pub const MAX_SESSION_TEXT_BYTES: usize = 256;
pub const MAX_WORKSPACE_PATH_BYTES: usize = 512;
pub const MAX_CAUSE_BYTES: usize = 512;
/// Live (not ended) sessions one ledger may hold.
pub const MAX_LIVE_FLEET_SESSIONS: usize = 256;
pub const GIT_SHA1_HEX_BYTES: usize = 40;
pub const GIT_SHA256_HEX_BYTES: usize = 64;
/// Characters of an attempt identity shown in decision questions.
pub const QUESTION_ATTEMPT_CHARS: usize = 12;
const MS_PER_MINUTE: u64 = 60 * 1_000;

pub const DEFAULT_TICK_MS: u64 = 15 * 1_000;
pub const DEFAULT_RENEW_BEFORE_MS: u64 = 10 * MS_PER_MINUTE;
pub const DEFAULT_BLOCKED_DECISION_AFTER_MS: u64 = 5 * MS_PER_MINUTE;
pub const DEFAULT_NUDGE_AFTER_MS: u64 = 10 * MS_PER_MINUTE;
pub const DEFAULT_MAX_NUDGES: u32 = 2;
pub const DEFAULT_ATTEMPT_LEASE_MS: u64 = DEFAULT_LEASE_MS;
pub const MIN_TICK_MS: u64 = 1_000;
pub const MAX_NUDGES_BOUND: u32 = 10;

/// herdr's agent status, mapped one-to-one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionState {
    Placing,
    Starting,
    Prompted,
    Working,
    Blocked { since_ms: u64 },
    Ready { since_ms: u64 },
    HandoffReceived { commit: String },
    Captured,
    Failed { cause: String },
    Cancelled { cause: String },
}

impl SessionState {
    /// Captured, failed and cancelled sessions have ended.
    #[must_use]
    pub const fn is_ended(&self) -> bool {
        matches!(
            self,
            Self::Captured | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }

    /// States whose attempt lease is renewed.
    const fn renews(&self) -> bool {
        matches!(
            self,
            Self::Starting
                | Self::Prompted
                | Self::Working
                | Self::Blocked { .. }
                | Self::Ready { .. }
                | Self::HandoffReceived { .. }
        )
    }

    /// States in which the agent is running and may receive a prompt.
    const fn promptable(&self) -> bool {
        matches!(
            self,
            Self::Prompted | Self::Working | Self::Blocked { .. } | Self::Ready { .. }
        )
    }
}

/// The isolated worktree an agent runs in.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionWorkspace {
    /// BLAKE3 over the other fields.
    pub identity: ContentId,
    pub herdr_workspace: String,
    pub pane: String,
    pub path: String,
    pub branch: String,
    pub base_commit: String,
}

impl SessionWorkspace {
    /// # Errors
    /// Refuses malformed fields: a relative or control-bearing path, a
    /// non-hexadecimal base commit, or empty or oversized labels.
    pub fn new(
        herdr_workspace: &str,
        pane: &str,
        path: &str,
        branch: &str,
        base_commit: &str,
    ) -> CoreResult<Self> {
        for (value, field) in [
            (herdr_workspace, "workspace.herdr_workspace"),
            (pane, "workspace.pane"),
            (branch, "workspace.branch"),
        ] {
            if !bounded_token(value, MAX_SESSION_TEXT_BYTES) {
                return Err(refusal(
                    "fleet_session_workspace",
                    field,
                    "workspace label is empty, oversized or malformed",
                ));
            }
        }
        if !path.starts_with('/') || !bounded_line(path, MAX_WORKSPACE_PATH_BYTES) {
            return Err(refusal(
                "fleet_session_workspace",
                "workspace.path",
                "workspace path must be a bounded absolute path",
            ));
        }
        if !commit(base_commit) {
            return Err(refusal(
                "fleet_session_workspace",
                "workspace.base_commit",
                "base commit must be a lowercase hexadecimal object id",
            ));
        }
        let mut bytes = Vec::new();
        for value in [
            FLEET_WORKSPACE_SCHEMA,
            herdr_workspace,
            pane,
            path,
            branch,
            base_commit,
        ] {
            push_field(&mut bytes, value, "workspace.identity")?;
        }
        Ok(Self {
            identity: content_identity(&bytes)?,
            herdr_workspace: herdr_workspace.to_string(),
            pane: pane.to_string(),
            path: path.to_string(),
            branch: branch.to_string(),
            base_commit: base_commit.to_string(),
        })
    }

    /// Revalidate a workspace that may have arrived through serde.
    ///
    /// # Errors
    /// Refuses malformed fields or an identity that does not match them.
    pub fn validate(&self) -> CoreResult<()> {
        let expected = Self::new(
            &self.herdr_workspace,
            &self.pane,
            &self.path,
            &self.branch,
            &self.base_commit,
        )?;
        if expected.identity != self.identity {
            return Err(refusal(
                "fleet_session_workspace",
                "workspace.identity",
                "workspace identity does not match its fields",
            ));
        }
        Ok(())
    }
}

/// One interactive session.
///
/// Deviation from the design: `agent_name` and `workspace` are optional
/// because a session is bound in `Placing` before the agent exists; both are
/// set by [`Observation::Started`]. The fields after `nudges` are explicit
/// idempotence markers recorded by [`record_issued`] and cleared by the
/// observation that settles each effect.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetSession {
    pub identity: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub work: ContentId,
    pub work_revision: u64,
    pub machine: String,
    pub kind: String,
    pub agent_name: Option<String>,
    pub workspace: Option<SessionWorkspace>,
    pub state: SessionState,
    pub created_ms: u64,
    pub observed_ms: u64,
    /// The agent has been seen working since it was prompted.
    pub worked: bool,
    /// Nudge prompts issued.
    pub nudges: u32,
    /// A prompt was issued and `Prompted` has not been observed yet.
    #[serde(default)]
    pub prompt_pending: bool,
    /// The newest answered decision whose answer was issued as a prompt.
    #[serde(default)]
    pub delivered_decision: Option<ContentId>,
    /// A decision was requested in the current state; reset on a state
    /// change.
    #[serde(default)]
    pub decision_raised: bool,
    /// Attempt lease expiry at which a renewal was last requested.
    #[serde(default)]
    pub renew_requested_for: Option<u64>,
    /// A capture was issued and neither `Captured` nor `CaptureFailed` has
    /// been observed yet.
    #[serde(default)]
    pub capture_pending: bool,
    /// The attempt's cancellation was issued.
    #[serde(default)]
    pub cancel_requested: bool,
    /// The agent's stop was issued.
    #[serde(default)]
    pub stop_requested: bool,
}

/// Placement chosen by the provider adapter for one attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    pub attempt: ContentId,
    pub fence: u64,
    pub work: ContentId,
    pub work_revision: u64,
    pub machine: String,
    pub kind: String,
}

impl FleetSession {
    /// A new session in `Placing`, before the agent starts.
    ///
    /// # Errors
    /// Refuses malformed identities, machine labels and agent kinds.
    pub fn placing(placement: Placement, now_ms: u64) -> CoreResult<Self> {
        placement.attempt.validate()?;
        placement.work.validate()?;
        for (value, field) in [
            (&placement.machine, "session.machine"),
            (&placement.kind, "session.kind"),
        ] {
            if !bounded_token(value, MAX_SESSION_TEXT_BYTES) {
                return Err(refusal(
                    "fleet_session",
                    field,
                    "session label is empty, oversized or malformed",
                ));
            }
        }
        Ok(Self {
            identity: session_identity(&placement.attempt, placement.fence)?,
            attempt: placement.attempt,
            fence: placement.fence,
            work: placement.work,
            work_revision: placement.work_revision,
            machine: placement.machine,
            kind: placement.kind,
            agent_name: None,
            workspace: None,
            state: SessionState::Placing,
            created_ms: now_ms,
            observed_ms: now_ms,
            worked: false,
            nudges: 0,
            prompt_pending: false,
            delivered_decision: None,
            decision_raised: false,
            renew_requested_for: None,
            capture_pending: false,
            cancel_requested: false,
            stop_requested: false,
        })
    }
}

/// One provider or shell observation of a session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation {
    Started {
        agent_name: String,
        workspace: SessionWorkspace,
    },
    StartFailed {
        reason: String,
    },
    Prompted,
    Status {
        status: AgentStatus,
    },
    Gone,
    Handoff {
        commit: String,
    },
    Captured,
    CaptureFailed {
        reason: String,
    },
    Cancelled {
        cause: String,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionLedger {
    pub sessions: BTreeMap<ContentId, FleetSession>,
    /// Attempts whose `Place` was issued and whose session is not bound yet,
    /// with the fence placed. Added beside the designed `sessions` map so a
    /// repeated tick does not place an attempt twice.
    #[serde(default)]
    pub placing: BTreeMap<ContentId, u64>,
}

impl SessionLedger {
    /// The session following `attempt`, if any.
    #[must_use]
    pub fn for_attempt(&self, attempt: &ContentId) -> Option<&FleetSession> {
        self.sessions
            .values()
            .find(|session| &session.attempt == attempt)
    }
}

/// Attempt phases as the fleet tick needs them.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptViewState {
    Claimed,
    Executing,
    CandidateProduced,
    Verifying,
    Terminal,
}

impl AttemptViewState {
    #[must_use]
    pub const fn from_phase(phase: AttemptPhase) -> Self {
        match phase {
            AttemptPhase::Claimed => Self::Claimed,
            AttemptPhase::Executing => Self::Executing,
            AttemptPhase::CandidateProduced => Self::CandidateProduced,
            AttemptPhase::Verifying => Self::Verifying,
            AttemptPhase::Verified
            | AttemptPhase::Blocked
            | AttemptPhase::Failed
            | AttemptPhase::Cancelled
            | AttemptPhase::Superseded => Self::Terminal,
        }
    }
}

/// One current attempt as the tick sees it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptView {
    pub attempt: ContentId,
    pub fence: u64,
    pub state: AttemptViewState,
    pub lease_expires_ms: u64,
    /// The attempt was claimed for the fleet's interactive provider.
    pub held_by_fleet: bool,
}

impl AttemptView {
    fn active(&self, now_ms: u64) -> bool {
        self.state != AttemptViewState::Terminal && self.lease_expires_ms > now_ms
    }

    fn placeable(&self, now_ms: u64) -> bool {
        self.held_by_fleet
            && matches!(
                self.state,
                AttemptViewState::Claimed | AttemptViewState::Executing
            )
            && self.lease_expires_ms > now_ms
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TickInput<'a> {
    pub now_ms: u64,
    pub ledger: &'a SessionLedger,
    pub attempts: &'a [AttemptView],
    pub free_slots: u32,
    pub ready_work: u32,
    pub decisions: &'a DecisionLedger,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetEffect {
    ClaimNext,
    Place {
        attempt: ContentId,
        fence: u64,
    },
    Prompt {
        session: ContentId,
        text: String,
    },
    Renew {
        attempt: ContentId,
        fence: u64,
    },
    Capture {
        session: ContentId,
        commit: String,
    },
    RaiseDecision {
        session: ContentId,
        question: String,
    },
    Cancel {
        attempt: ContentId,
        fence: u64,
        cause: String,
    },
    Stop {
        session: ContentId,
    },
}

/// Named dispatch limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetPolicy {
    pub tick_ms: u64,
    pub renew_before_ms: u64,
    pub blocked_decision_after_ms: u64,
    pub nudge_after_ms: u64,
    pub max_nudges: u32,
    pub attempt_lease_ms: u64,
}

impl Default for FleetPolicy {
    fn default() -> Self {
        Self {
            tick_ms: DEFAULT_TICK_MS,
            renew_before_ms: DEFAULT_RENEW_BEFORE_MS,
            blocked_decision_after_ms: DEFAULT_BLOCKED_DECISION_AFTER_MS,
            nudge_after_ms: DEFAULT_NUDGE_AFTER_MS,
            max_nudges: DEFAULT_MAX_NUDGES,
            attempt_lease_ms: DEFAULT_ATTEMPT_LEASE_MS,
        }
    }
}

impl FleetPolicy {
    /// # Errors
    /// Refuses a tick below [`MIN_TICK_MS`], zero windows, an attempt lease
    /// outside the attempt bounds, a renewal window not inside the lease, and
    /// more than [`MAX_NUDGES_BOUND`] nudges.
    pub fn validate(&self) -> CoreResult<()> {
        let valid = self.tick_ms >= MIN_TICK_MS
            && self.blocked_decision_after_ms > 0
            && self.nudge_after_ms > 0
            && self.max_nudges <= MAX_NUDGES_BOUND
            && self.attempt_lease_ms > 0
            && self.attempt_lease_ms <= MAX_LEASE_MS
            && self.renew_before_ms > 0
            && self.renew_before_ms < self.attempt_lease_ms;
        if !valid {
            return Err(refusal(
                "fleet_policy",
                "policy",
                "fleet policy limits are outside their bounds",
            ));
        }
        Ok(())
    }
}

/// Deterministic session identity over the attempt and fence.
///
/// # Errors
/// Returns a diagnostic only when a field exceeds the encoding bound.
pub fn session_identity(attempt: &ContentId, fence: u64) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_field(&mut bytes, FLEET_SESSION_SCHEMA, "session.identity")?;
    push_field(&mut bytes, attempt.as_str(), "session.identity")?;
    bytes.extend_from_slice(&fence.to_be_bytes());
    content_identity(&bytes)
}

/// Bind a new `Placing` session. Binding the identical session again changes
/// nothing.
///
/// # Errors
/// Refuses a session that is not freshly placing, whose identity does not
/// match its attempt and fence, a different session under the same identity,
/// a second live session for the attempt, and the live-session bound.
pub fn bind(ledger: &SessionLedger, session: FleetSession) -> CoreResult<SessionLedger> {
    let fresh = FleetSession::placing(
        Placement {
            attempt: session.attempt.clone(),
            fence: session.fence,
            work: session.work.clone(),
            work_revision: session.work_revision,
            machine: session.machine.clone(),
            kind: session.kind.clone(),
        },
        session.created_ms,
    )?;
    let mut expected = fresh;
    expected.observed_ms = session.observed_ms;
    if session != expected {
        return Err(refusal(
            "fleet_session_bind",
            "session",
            "only a fresh placing session with its derived identity can be bound",
        ));
    }
    if let Some(existing) = ledger.sessions.get(&session.identity) {
        if *existing == session {
            return Ok(ledger.clone());
        }
        return Err(refusal(
            "fleet_session_bind",
            "session.identity",
            "a different session is already bound under this identity",
        ));
    }
    let live = ledger
        .sessions
        .values()
        .filter(|other| !other.state.is_ended());
    if live.clone().any(|other| other.attempt == session.attempt) {
        return Err(refusal(
            "fleet_session_bind",
            "session.attempt",
            "the attempt already has a live session",
        ));
    }
    if live.count() >= MAX_LIVE_FLEET_SESSIONS {
        return Err(refusal(
            "fleet_session_limit",
            "sessions",
            "live fleet sessions reached their bound",
        ));
    }
    let mut next = ledger.clone();
    next.placing.remove(&session.attempt);
    next.sessions.insert(session.identity.clone(), session);
    Ok(next)
}

/// Forget an issued `Place` that could not bind a session, so a later tick
/// may place the attempt again.
#[must_use]
pub fn clear_placing(ledger: &SessionLedger, attempt: &ContentId) -> SessionLedger {
    let mut next = ledger.clone();
    next.placing.remove(attempt);
    next
}

// r[impl kiln.fleet.dispatch]
/// Admit one observation of a session.
///
/// Status reports never invent liveness: `unknown` changes nothing, and
/// reports for sessions that have handed off or ended are recorded as seen
/// only. A `Gone` agent after a received handoff keeps the handoff, because
/// the commit lives in the worktree, not the agent.
///
/// # Errors
/// Refuses unknown sessions, malformed fields, and transitions that make no
/// sense, such as a second `Started`, a handoff for an ended session or a
/// capture without a handoff.
pub fn observe(
    ledger: &SessionLedger,
    session: &ContentId,
    observation: Observation,
    now_ms: u64,
) -> CoreResult<SessionLedger> {
    let current = ledger
        .sessions
        .get(session)
        .ok_or_else(|| refusal("fleet_session_unknown", "session", "no such fleet session"))?;
    let mut next_session = current.clone();
    next_session.observed_ms = current.observed_ms.max(now_ms);
    let state = &current.state;
    let next_state = match observation {
        Observation::Started {
            agent_name,
            workspace,
        } => {
            if *state != SessionState::Placing {
                return Err(transition("the agent has already started"));
            }
            if !bounded_token(&agent_name, MAX_SESSION_TEXT_BYTES) {
                return Err(refusal(
                    "fleet_session",
                    "agent_name",
                    "agent name is empty, oversized or malformed",
                ));
            }
            workspace.validate()?;
            next_session.agent_name = Some(agent_name);
            next_session.workspace = Some(workspace);
            SessionState::Starting
        }
        Observation::StartFailed { reason } => match state {
            SessionState::Placing | SessionState::Starting => SessionState::Failed {
                cause: cause("start failed", &reason)?,
            },
            _ => return Err(transition("only a starting session can fail to start")),
        },
        Observation::Prompted => {
            if !matches!(state, SessionState::Starting) && !state.promptable() {
                return Err(transition("the session cannot receive a prompt"));
            }
            next_session.prompt_pending = false;
            SessionState::Prompted
        }
        Observation::Status { status } => status_state(&mut next_session, status, now_ms),
        Observation::Gone => match state {
            SessionState::Placing
            | SessionState::Starting
            | SessionState::Prompted
            | SessionState::Working
            | SessionState::Blocked { .. }
            | SessionState::Ready { .. } => SessionState::Failed {
                cause: "agent gone before handoff".to_string(),
            },
            _ => state.clone(),
        },
        Observation::Handoff { commit: handed_off } => {
            if !commit(&handed_off) {
                return Err(refusal(
                    "fleet_session_handoff",
                    "commit",
                    "handoff commit must be a lowercase hexadecimal object id",
                ));
            }
            match state {
                SessionState::HandoffReceived { commit: existing } if *existing == handed_off => {
                    return Ok(ledger.clone());
                }
                state if state.promptable() => SessionState::HandoffReceived { commit: handed_off },
                _ => return Err(transition("the session cannot hand off")),
            }
        }
        Observation::Captured => match state {
            SessionState::HandoffReceived { .. } => {
                next_session.capture_pending = false;
                SessionState::Captured
            }
            SessionState::Captured => return Ok(ledger.clone()),
            _ => return Err(transition("only a received handoff can be captured")),
        },
        Observation::CaptureFailed { reason } => match state {
            SessionState::HandoffReceived { .. } => {
                next_session.capture_pending = false;
                SessionState::Failed {
                    cause: cause("capture failed", &reason)?,
                }
            }
            _ => return Err(transition("only a received handoff can fail capture")),
        },
        Observation::Cancelled { cause: reason } => match state {
            SessionState::Cancelled { .. } => return Ok(ledger.clone()),
            SessionState::Captured | SessionState::Failed { .. } => {
                return Err(transition("the session has already ended"));
            }
            _ => SessionState::Cancelled {
                cause: cause("cancelled", &reason)?,
            },
        },
    };
    if discriminant(&next_state) != discriminant(state) {
        next_session.decision_raised = false;
    }
    next_session.state = next_state;
    let mut next = ledger.clone();
    next.sessions.insert(session.clone(), next_session);
    Ok(next)
}

fn status_state(session: &mut FleetSession, status: AgentStatus, now_ms: u64) -> SessionState {
    let state = session.state.clone();
    if !state.promptable() {
        return state;
    }
    match (status, &state) {
        (AgentStatus::Working, _) => {
            session.worked = true;
            SessionState::Working
        }
        (AgentStatus::Blocked, SessionState::Blocked { .. })
        | (AgentStatus::Idle | AgentStatus::Done, SessionState::Ready { .. })
        | (AgentStatus::Unknown, _) => state,
        (AgentStatus::Blocked, _) => SessionState::Blocked { since_ms: now_ms },
        (AgentStatus::Idle | AgentStatus::Done, _) => SessionState::Ready { since_ms: now_ms },
    }
}

// r[impl kiln.fleet.dispatch]
/// Plan one dispatch tick. Pure and deterministic over the recorded state;
/// effects whose settling observation is still pending are not repeated.
/// `ClaimNext` is the exception: its settling observation is the claimed
/// attempt itself, so it is requested again each tick while free slots exceed
/// the starting and unplaced attempts and ready work remains.
#[must_use]
pub fn plan_tick(input: &TickInput<'_>, policy: &FleetPolicy) -> Vec<FleetEffect> {
    let now_ms = input.now_ms;
    let ledger = input.ledger;
    let mut effects = Vec::new();

    let starting = ledger
        .sessions
        .values()
        .filter(|session| {
            !session.stop_requested
                && matches!(
                    session.state,
                    SessionState::Placing | SessionState::Starting
                )
        })
        .count();
    let unplaced: Vec<&AttemptView> = input
        .attempts
        .iter()
        .filter(|view| view.placeable(now_ms) && ledger.for_attempt(&view.attempt).is_none())
        .collect();
    let committed = starting.saturating_add(unplaced.len());
    if input.ready_work > 0 && usize::try_from(input.free_slots).is_ok_and(|free| free > committed)
    {
        effects.push(FleetEffect::ClaimNext);
    }
    for view in unplaced {
        if ledger.placing.get(&view.attempt) != Some(&view.fence) {
            effects.push(FleetEffect::Place {
                attempt: view.attempt.clone(),
                fence: view.fence,
            });
        }
    }

    for session in ledger.sessions.values() {
        let active = input
            .attempts
            .iter()
            .find(|view| view.attempt == session.attempt && view.fence == session.fence)
            .filter(|view| view.active(now_ms));
        plan_session(session, active, input, policy, &mut effects);
    }
    effects
}

fn plan_session(
    session: &FleetSession,
    active: Option<&AttemptView>,
    input: &TickInput<'_>,
    policy: &FleetPolicy,
    effects: &mut Vec<FleetEffect>,
) {
    let now_ms = input.now_ms;
    let stop = FleetEffect::Stop {
        session: session.identity.clone(),
    };
    match (&session.state, active) {
        (SessionState::Failed { cause }, _) => {
            if let Some(view) = active
                && !session.cancel_requested
            {
                effects.push(FleetEffect::Cancel {
                    attempt: view.attempt.clone(),
                    fence: view.fence,
                    cause: cause.clone(),
                });
            }
            if !session.stop_requested {
                effects.push(stop);
            }
        }
        (SessionState::Cancelled { .. }, _) | (_, None) => {
            if !session.stop_requested {
                effects.push(stop);
            }
        }
        // The candidate awaits Kiln's verification, which may outlast the
        // lease: keep the attempt and its claim leased so the attempt is not
        // expired as abandoned and dispatched again. The agent stays until the
        // attempt resolves.
        (SessionState::Captured, Some(view)) => {
            if renewal_due(session, view, now_ms, policy) {
                effects.push(FleetEffect::Renew {
                    attempt: view.attempt.clone(),
                    fence: view.fence,
                });
            }
        }
        (state, Some(view)) => {
            if state.renews() && renewal_due(session, view, now_ms, policy) {
                effects.push(FleetEffect::Renew {
                    attempt: view.attempt.clone(),
                    fence: view.fence,
                });
            }
            plan_agent(session, input, policy, effects);
        }
    }
}

/// The attempt's lease ends within `renew_before_ms` and no renewal was
/// requested for this lease yet.
fn renewal_due(
    session: &FleetSession,
    view: &AttemptView,
    now_ms: u64,
    policy: &FleetPolicy,
) -> bool {
    view.lease_expires_ms.saturating_sub(now_ms) <= policy.renew_before_ms
        && session.renew_requested_for != Some(view.lease_expires_ms)
}

fn plan_agent(
    session: &FleetSession,
    input: &TickInput<'_>,
    policy: &FleetPolicy,
    effects: &mut Vec<FleetEffect>,
) {
    let now_ms = input.now_ms;
    if let SessionState::HandoffReceived { commit } = &session.state {
        if !session.capture_pending {
            effects.push(FleetEffect::Capture {
                session: session.identity.clone(),
                commit: commit.clone(),
            });
        }
        return;
    }
    if !session.state.promptable() || session.prompt_pending {
        return;
    }
    if let Some(decision) = pending_answer(session, input.decisions) {
        if let Some(text) = answer_prompt(decision) {
            effects.push(FleetEffect::Prompt {
                session: session.identity.clone(),
                text,
            });
        }
        return;
    }
    let may_raise = !session.decision_raised
        && input
            .decisions
            .open_for_session(&session.identity)
            .is_none();
    match session.state {
        SessionState::Blocked { since_ms }
            if may_raise && now_ms.saturating_sub(since_ms) >= policy.blocked_decision_after_ms =>
        {
            effects.push(FleetEffect::RaiseDecision {
                session: session.identity.clone(),
                question: blocked_question(session, now_ms.saturating_sub(since_ms)),
            });
        }
        SessionState::Ready { since_ms }
            if session.worked && now_ms.saturating_sub(since_ms) >= policy.nudge_after_ms =>
        {
            if session.nudges < policy.max_nudges {
                effects.push(FleetEffect::Prompt {
                    session: session.identity.clone(),
                    text: render_nudge(
                        &session.attempt,
                        session.fence,
                        session
                            .workspace
                            .as_ref()
                            .map(|workspace| workspace.branch.as_str()),
                        session.nudges.saturating_add(1),
                        policy.max_nudges,
                        &HandoffInstructions::default(),
                    ),
                });
            } else if may_raise {
                effects.push(FleetEffect::RaiseDecision {
                    session: session.identity.clone(),
                    question: idle_question(session),
                });
            }
        }
        _ => {}
    }
}

/// Record effects a shell has performed, so a repeated tick over the same
/// observations requests none of them again. Call it with the same input the
/// effects were planned from.
///
/// # Errors
/// Refuses effects naming an unknown session, or a renewal for an attempt the
/// input does not hold.
pub fn record_issued(input: &TickInput<'_>, effects: &[FleetEffect]) -> CoreResult<SessionLedger> {
    let mut ledger = input.ledger.clone();
    for effect in effects {
        match effect {
            FleetEffect::ClaimNext => {}
            FleetEffect::Place { attempt, fence } => {
                ledger.placing.insert(attempt.clone(), *fence);
            }
            FleetEffect::Prompt { session, .. } => {
                let delivered = {
                    let current = session_mut(&mut ledger, session)?;
                    pending_answer(current, input.decisions)
                        .map(|decision| decision.identity.clone())
                };
                let current = session_mut(&mut ledger, session)?;
                current.prompt_pending = true;
                match delivered {
                    Some(decision) => current.delivered_decision = Some(decision),
                    None => current.nudges = current.nudges.saturating_add(1),
                }
            }
            FleetEffect::Renew { attempt, fence } => {
                let lease = input
                    .attempts
                    .iter()
                    .find(|view| &view.attempt == attempt && view.fence == *fence)
                    .map(|view| view.lease_expires_ms)
                    .ok_or_else(|| {
                        refusal(
                            "fleet_session_effect",
                            "attempt",
                            "renewal names an attempt the tick does not hold",
                        )
                    })?;
                for current in ledger
                    .sessions
                    .values_mut()
                    .filter(|current| &current.attempt == attempt && current.fence == *fence)
                {
                    current.renew_requested_for = Some(lease);
                }
            }
            FleetEffect::Capture { session, .. } => {
                session_mut(&mut ledger, session)?.capture_pending = true;
            }
            FleetEffect::RaiseDecision { session, .. } => {
                session_mut(&mut ledger, session)?.decision_raised = true;
            }
            FleetEffect::Cancel { attempt, fence, .. } => {
                for current in ledger
                    .sessions
                    .values_mut()
                    .filter(|current| &current.attempt == attempt && current.fence == *fence)
                {
                    current.cancel_requested = true;
                }
            }
            FleetEffect::Stop { session } => {
                session_mut(&mut ledger, session)?.stop_requested = true;
            }
        }
    }
    let attempts = input.attempts;
    ledger.placing.retain(|attempt, fence| {
        attempts.iter().any(|view| {
            &view.attempt == attempt && view.fence == *fence && view.placeable(input.now_ms)
        })
    });
    Ok(ledger)
}

fn session_mut<'a>(
    ledger: &'a mut SessionLedger,
    session: &ContentId,
) -> CoreResult<&'a mut FleetSession> {
    ledger.sessions.get_mut(session).ok_or_else(|| {
        refusal(
            "fleet_session_effect",
            "session",
            "effect names an unknown session",
        )
    })
}

/// The newest answered decision for `session` that has not been issued.
fn pending_answer<'a>(
    session: &FleetSession,
    decisions: &'a DecisionLedger,
) -> Option<&'a Decision> {
    decisions
        .latest_answered(&session.identity)
        .filter(|decision| session.delivered_decision.as_ref() != Some(&decision.identity))
}

fn blocked_question(session: &FleetSession, blocked_ms: u64) -> String {
    bounded_question(format!(
        "{} on {} is blocked on attempt {} (fence {}) for {} minutes. Read its pane and answer its question, or cancel the attempt.",
        session.kind,
        session.machine,
        short(&session.attempt, QUESTION_ATTEMPT_CHARS),
        session.fence,
        blocked_ms / MS_PER_MINUTE,
    ))
}

fn idle_question(session: &FleetSession) -> String {
    bounded_question(format!(
        "{} on {} is idle without a handoff on attempt {} (fence {}) after {} nudges. Answer with guidance, or cancel the attempt.",
        session.kind,
        session.machine,
        short(&session.attempt, QUESTION_ATTEMPT_CHARS),
        session.fence,
        session.nudges,
    ))
}

fn bounded_question(mut question: String) -> String {
    if question.len() > MAX_QUESTION_BYTES {
        let mut end = MAX_QUESTION_BYTES;
        while !question.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        question.truncate(end);
    }
    question
}

fn cause(prefix: &str, reason: &str) -> CoreResult<String> {
    let text = format!("{prefix}: {reason}");
    if reason.trim().is_empty() || !bounded_line(&text, MAX_CAUSE_BYTES) {
        return Err(refusal(
            "fleet_session_cause",
            "cause",
            "cause is empty, oversized or contains a control character",
        ));
    }
    Ok(text)
}

fn commit(value: &str) -> bool {
    matches!(value.len(), GIT_SHA1_HEX_BYTES | GIT_SHA256_HEX_BYTES)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn transition(message: &str) -> crate::types::Diagnostic {
    refusal("fleet_session_transition", "observation", message)
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
