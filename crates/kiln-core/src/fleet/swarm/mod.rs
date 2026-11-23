//! Pure swarm orchestration control plane: scoped run boards, bounded
//! delegation, separated `@verifier` repair loops, deterministic watchdog,
//! and quota failover.
//!
//! Every transition in this module is a pure function over recorded state and
//! an explicit `now_ms` timestamp; shells own SQLite persistence, HTTP/CLI/MCP
//! transport, and clocks. Board post text is strictly untrusted data and never
//! mutates claims, scopes, roles, budgets, or publication state.

pub mod admission;
pub mod board;
pub mod verdict;
pub mod watchdog;

pub use admission::*;
pub use board::*;
pub use verdict::*;
pub use watchdog::*;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::scope::{FleetScope, contains_scope};
use super::{bounded_line, bounded_token, content_identity, push_field, refusal};
use crate::types::{ContentId, CoreResult};

pub const SWARM_RUN_SCHEMA: &str = "kiln.fleet-swarm.v1";
pub const MAX_SWARM_RUNS: usize = 64;
pub const MAX_SWARM_TITLE_BYTES: usize = 256;
pub const MAX_MEMBER_NAME_BYTES: usize = 64;
pub const MAX_HARNESS_FIELD_BYTES: usize = 64;
pub const MAX_SWARM_CONCURRENCY: u32 = 32;
pub const MAX_SWARM_REPAIR_ROUNDS: u32 = 16;
pub const MAX_ENVELOPE_SCOPES: usize = 32;
pub const MAX_BOARD_POSTS: usize = 4_096;
pub const MAX_POST_BODY_BYTES: usize = 16_384;
pub const MAX_POST_MENTIONS: usize = 32;
pub const MAX_VERDICT_FINDINGS: usize = 64;
pub const MAX_VERDICT_FINDING_BYTES: usize = 1_024;
pub const MAX_PAUSE_REASON_BYTES: usize = 256;
pub const MAX_ESCALATION_REASON_BYTES: usize = 512;
pub const DEFAULT_STALE_AFTER_MS: u64 = 10 * 60 * 1_000;
pub const MIN_STALE_AFTER_MS: u64 = 1_000;
pub const MAX_STALE_AFTER_MS: u64 = 24 * 60 * 60 * 1_000;
pub const MAX_READ_LIMIT: usize = 256;
pub const DEFAULT_READ_LIMIT: usize = 50;
pub const COMMIT_SHA1_HEX_BYTES: usize = 40;
pub const COMMIT_SHA256_HEX_BYTES: usize = 64;

/// Parent budget ceiling and measured draw (#98, #115).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmBudget {
    pub ceiling_units: u64,
    pub drawn_units: u64,
}

impl SwarmBudget {
    #[must_use]
    pub const fn new(ceiling_units: u64) -> Self {
        Self {
            ceiling_units,
            drawn_units: 0,
        }
    }

    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.ceiling_units.saturating_sub(self.drawn_units)
    }

    #[must_use]
    pub const fn has_remaining(&self) -> bool {
        self.drawn_units < self.ceiling_units
    }

    /// Record measured budget draw.
    ///
    /// # Errors
    /// Refuses arithmetic overflow.
    pub fn record_draw(&mut self, delta: u64) -> CoreResult<()> {
        self.drawn_units = self.drawn_units.checked_add(delta).ok_or_else(|| {
            refusal(
                "fleet_swarm_budget",
                "budget.drawn_units",
                "swarm budget draw overflows u64",
            )
        })?;
        Ok(())
    }
}

/// Allowed repository/machine scopes that bound every spawned author in a run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmEnvelope {
    pub allowed_scopes: Vec<FleetScope>,
}

impl SwarmEnvelope {
    /// Validate that the envelope is non-empty, bounded, and canonical.
    ///
    /// # Errors
    /// Refuses an empty or oversized envelope, or any non-canonical scope.
    pub fn validate(&self) -> CoreResult<()> {
        if self.allowed_scopes.is_empty() || self.allowed_scopes.len() > MAX_ENVELOPE_SCOPES {
            return Err(refusal(
                "fleet_swarm_envelope",
                "envelope.allowed_scopes",
                "swarm envelope must contain between 1 and 32 allowed scopes",
            ));
        }
        for scope in &self.allowed_scopes {
            scope.validate()?;
        }
        Ok(())
    }

    /// True iff `candidate` is strictly contained within at least one allowed
    /// scope of this envelope (zero scope widening).
    #[must_use]
    pub fn contains(&self, candidate: &FleetScope) -> bool {
        self.allowed_scopes
            .iter()
            .any(|allowed| contains_scope(allowed, candidate))
    }
}

/// Configuration admitted when creating a swarm run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmRunConfig {
    pub title: String,
    pub envelope: SwarmEnvelope,
    pub max_concurrency: u32,
    pub max_repair_rounds: u32,
    pub budget_ceiling_units: u64,
    pub stale_after_ms: u64,
}

impl SwarmRunConfig {
    /// Validate all named bounds on a swarm run configuration.
    ///
    /// # Errors
    /// Refuses empty/oversized titles, invalid envelopes, out-of-range
    /// concurrency/repair/stale windows, or a zero budget ceiling.
    pub fn validate(&self) -> CoreResult<()> {
        if !bounded_line(&self.title, MAX_SWARM_TITLE_BYTES) {
            return Err(refusal(
                "fleet_swarm_config",
                "title",
                "swarm title is empty, oversized, or contains control characters",
            ));
        }
        self.envelope.validate()?;
        if self.max_concurrency == 0 || self.max_concurrency > MAX_SWARM_CONCURRENCY {
            return Err(refusal(
                "fleet_swarm_config",
                "max_concurrency",
                "max_concurrency must be between 1 and 32",
            ));
        }
        if self.max_repair_rounds == 0 || self.max_repair_rounds > MAX_SWARM_REPAIR_ROUNDS {
            return Err(refusal(
                "fleet_swarm_config",
                "max_repair_rounds",
                "max_repair_rounds must be between 1 and 16",
            ));
        }
        if self.budget_ceiling_units == 0 {
            return Err(refusal(
                "fleet_swarm_config",
                "budget_ceiling_units",
                "budget_ceiling_units must be positive",
            ));
        }
        if !(MIN_STALE_AFTER_MS..=MAX_STALE_AFTER_MS).contains(&self.stale_after_ms) {
            return Err(refusal(
                "fleet_swarm_config",
                "stale_after_ms",
                "stale_after_ms is outside its allowed bounds",
            ));
        }
        Ok(())
    }
}

/// Lifecycle phase of one swarm run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmRunPhase {
    Active,
    ReadyForPublication,
    Escalated,
    Completed,
    Cancelled,
}

/// Explicit authority role of a swarm member.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmRole {
    Coordinator,
    Author,
    Verifier,
}

impl SwarmRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Author => "author",
            Self::Verifier => "verifier",
        }
    }
}

/// First-class member execution state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SwarmMemberState {
    Working,
    Waiting {
        since_seq: u64,
    },
    PausedQuota {
        checkpoint_commit: Option<String>,
        board_cursor: u64,
        reason: String,
        paused_ms: u64,
    },
    Retired,
    Terminal,
}

impl SwarmMemberState {
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self, Self::Retired | Self::Terminal)
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting { .. } => "waiting",
            Self::PausedQuota { .. } => "paused_quota",
            Self::Retired => "retired",
            Self::Terminal => "terminal",
        }
    }
}

/// One member in a swarm run roster.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmMember {
    pub name: String,
    pub role: SwarmRole,
    pub state: SwarmMemberState,
    pub host: String,
    pub harness: String,
    pub model: String,
    pub scope: Option<FleetScope>,
    pub claim_identity: Option<ContentId>,
    pub claim_fence: Option<u64>,
    #[serde(default)]
    pub attempt_lineage: Vec<ContentId>,
    pub budget_draw: u64,
    pub last_read_seq: u64,
    pub last_post_seq: Option<u64>,
    pub spawned_ms: u64,
    pub observed_ms: u64,
    #[serde(default)]
    pub failover_count: u32,
}

/// Complete state of one Kiln-governed swarm run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmRun {
    pub identity: ContentId,
    pub config: SwarmRunConfig,
    pub budget: SwarmBudget,
    pub phase: SwarmRunPhase,
    pub members: BTreeMap<String, SwarmMember>,
    pub board: SwarmBoard,
    pub verdicts: Vec<VerifierVerdict>,
    pub repair_round: u32,
    pub approved_commit: Option<String>,
    pub escalation_reason: Option<String>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

impl SwarmRun {
    /// Count of active (neither retired nor terminal) members.
    #[must_use]
    pub fn active_member_count(&self) -> usize {
        self.members
            .values()
            .filter(|member| member.state.is_active())
            .count()
    }

    /// Deterministically compute the replayable directed weighted mention graph
    /// up to `up_to_seq` (defaults to current board head).
    #[must_use]
    pub fn mention_graph(&self, up_to_seq: Option<u64>) -> MentionGraph {
        board::mention_graph(self, up_to_seq)
    }
}

/// Durable collection of swarm runs inside `FleetState`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmLedger {
    pub runs: BTreeMap<ContentId, SwarmRun>,
}

/// Deterministic swarm run identity over title, envelope scopes, bounds, and
/// creation timestamp.
///
/// # Errors
/// Returns a diagnostic if any field exceeds the encoding bound.
pub fn swarm_run_identity(config: &SwarmRunConfig, created_ms: u64) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_field(&mut bytes, SWARM_RUN_SCHEMA, "swarm.identity")?;
    push_field(&mut bytes, &config.title, "swarm.identity")?;
    for scope in &config.envelope.allowed_scopes {
        push_field(&mut bytes, &scope.text(), "swarm.identity")?;
    }
    bytes.extend_from_slice(&config.max_concurrency.to_be_bytes());
    bytes.extend_from_slice(&config.max_repair_rounds.to_be_bytes());
    bytes.extend_from_slice(&config.budget_ceiling_units.to_be_bytes());
    bytes.extend_from_slice(&config.stale_after_ms.to_be_bytes());
    bytes.extend_from_slice(&created_ms.to_be_bytes());
    content_identity(&bytes)
}

// r[impl kiln.fleet.swarm.model]
/// Create a fresh active swarm run inside `ledger`.
///
/// # Errors
/// Refuses invalid configs, duplicate identities, or exceeding [`MAX_SWARM_RUNS`].
pub fn create_swarm_run(
    ledger: &SwarmLedger,
    config: SwarmRunConfig,
    now_ms: u64,
) -> CoreResult<(SwarmLedger, SwarmRun)> {
    config.validate()?;
    if ledger.runs.len() >= MAX_SWARM_RUNS {
        return Err(refusal(
            "fleet_swarm_limit",
            "runs",
            "swarm runs reached their bound",
        ));
    }
    let identity = swarm_run_identity(&config, now_ms)?;
    if ledger.runs.contains_key(&identity) {
        return Err(refusal(
            "fleet_swarm_identity",
            "identity",
            "swarm run identity already exists",
        ));
    }
    let budget = SwarmBudget::new(config.budget_ceiling_units);
    let run = SwarmRun {
        identity: identity.clone(),
        config,
        budget,
        phase: SwarmRunPhase::Active,
        members: BTreeMap::new(),
        board: SwarmBoard::default(),
        verdicts: Vec::new(),
        repair_round: 0,
        approved_commit: None,
        escalation_reason: None,
        created_ms: now_ms,
        updated_ms: now_ms,
    };
    let mut next = ledger.clone();
    next.runs.insert(identity, run.clone());
    Ok((next, run))
}

// r[impl kiln.fleet.swarm.model]
/// Update a member's execution state and record measured budget draw.
///
/// # Errors
/// Refuses unknown or ended members, invalid cursors, malformed commits or
/// pause reasons, or budget draw overflow.
pub fn update_member_state(
    run: &SwarmRun,
    member_name: &str,
    new_state: SwarmMemberState,
    budget_draw_delta: u64,
    now_ms: u64,
) -> CoreResult<(SwarmRun, SwarmMember)> {
    let current = run.members.get(member_name).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "member",
            "no such member in this swarm run",
        )
    })?;
    if !current.state.is_active() {
        return Err(refusal(
            "fleet_swarm_member_ended",
            "state",
            "retired or terminal swarm members cannot transition",
        ));
    }
    let head = run.board.head_seq();
    match &new_state {
        SwarmMemberState::Waiting { since_seq } => {
            if *since_seq > head {
                return Err(refusal(
                    "fleet_swarm_cursor",
                    "since_seq",
                    "waiting cursor exceeds the board head sequence",
                ));
            }
        }
        SwarmMemberState::PausedQuota {
            checkpoint_commit,
            board_cursor,
            reason,
            ..
        } => {
            if let Some(commit) = checkpoint_commit
                && !valid_commit_sha(commit)
            {
                return Err(refusal(
                    "fleet_swarm_commit",
                    "checkpoint_commit",
                    "checkpoint commit must be a 40- or 64-character lowercase hex id",
                ));
            }
            if *board_cursor > head {
                return Err(refusal(
                    "fleet_swarm_cursor",
                    "board_cursor",
                    "paused board_cursor exceeds the board head sequence",
                ));
            }
            if !bounded_line(reason, MAX_PAUSE_REASON_BYTES) {
                return Err(refusal(
                    "fleet_swarm_pause_reason",
                    "reason",
                    "pause reason is empty, oversized, or contains control characters",
                ));
            }
        }
        SwarmMemberState::Working | SwarmMemberState::Retired | SwarmMemberState::Terminal => {}
    }

    let mut next_run = run.clone();
    next_run.budget.record_draw(budget_draw_delta)?;
    let member = next_run.members.get_mut(member_name).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "member",
            "no such member in this swarm run",
        )
    })?;
    member.budget_draw = member
        .budget_draw
        .checked_add(budget_draw_delta)
        .ok_or_else(|| {
            refusal(
                "fleet_swarm_budget",
                "budget_draw",
                "member budget draw overflows u64",
            )
        })?;
    if let SwarmMemberState::Waiting { since_seq } = &new_state {
        member.last_read_seq = member.last_read_seq.max(*since_seq);
    }
    if let SwarmMemberState::PausedQuota { board_cursor, .. } = &new_state {
        member.last_read_seq = *board_cursor;
    }
    member.state = new_state;
    member.observed_ms = member.observed_ms.max(now_ms);
    let updated = member.clone();
    next_run.updated_ms = next_run.updated_ms.max(now_ms);
    Ok((next_run, updated))
}

/// Advance a member's monotonic board read cursor up to `up_to_seq`.
///
/// # Errors
/// Refuses unknown member names.
pub fn advance_read_cursor(
    run: &SwarmRun,
    member_name: &str,
    up_to_seq: u64,
) -> CoreResult<SwarmRun> {
    if !run.members.contains_key(member_name) {
        return Err(refusal(
            "fleet_swarm_member_unknown",
            "member",
            "no such member in this swarm run",
        ));
    }
    let clamped = up_to_seq.min(run.board.head_seq());
    let mut next = run.clone();
    if let Some(member) = next.members.get_mut(member_name) {
        member.last_read_seq = member.last_read_seq.max(clamped);
    }
    Ok(next)
}

pub(crate) fn valid_member_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    !name.is_empty()
        && name.len() <= MAX_MEMBER_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub(crate) fn valid_harness_field(value: &str) -> bool {
    bounded_token(value, MAX_HARNESS_FIELD_BYTES)
}

pub(crate) fn valid_commit_sha(value: &str) -> bool {
    matches!(value.len(), COMMIT_SHA1_HEX_BYTES | COMMIT_SHA256_HEX_BYTES)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn valid_body_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

/// Truncate `text` to at most `max_bytes` on a UTF-8 character boundary,
/// appending `...` when truncated, so synthesized board summaries cannot
/// self-DoS `SwarmBoard::append`.
pub(crate) fn truncate_for_board(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let suffix = "...";
    let budget = max_bytes.saturating_sub(suffix.len());
    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        let next = idx.saturating_add(ch.len_utf8());
        if next > budget {
            break;
        }
        end = next;
    }
    let mut out = String::with_capacity(end.saturating_add(suffix.len()));
    if let Some(prefix) = text.get(..end) {
        out.push_str(prefix);
    }
    out.push_str(suffix);
    out
}

#[cfg(test)]
mod tests;
