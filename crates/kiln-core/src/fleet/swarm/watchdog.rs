//! Pure deterministic watchdog and quota failover.
//!
//! Replaces prompt-driven `@monitor` loops with deterministic evaluation over
//! roster states, board cursors, observation timestamps (#105), and claim
//! leases. Zero LLM calls, zero I/O.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::board::{BoardPost, BoardPostKind, MentionTarget};
use super::{
    MAX_PAUSE_REASON_BYTES, SwarmMember, SwarmMemberState, SwarmRole, SwarmRun, SwarmRunPhase,
    valid_harness_field,
};
use crate::fleet::claims::FleetClaimRegistry;
use crate::fleet::{bounded_line, refusal};
use crate::types::CoreResult;

/// Deterministic action emitted by [`evaluate_watchdog`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WatchdogAction {
    WakeMember {
        member: String,
        unread_seqs: Vec<u64>,
        reason: String,
    },
    DeadlockEscalation {
        waiting_members: Vec<String>,
        reason: String,
    },
    StaleObservationEscalation {
        member: String,
        last_observed_ms: u64,
        stale_for_ms: u64,
        reason: String,
    },
    QuotaFailoverCandidate {
        member: String,
        checkpoint_commit: Option<String>,
        board_cursor: u64,
        paused_reason: String,
    },
}

/// Request to transition a `PausedQuota` member onto a replacement model,
/// harness, or host while preserving cursor, checkpoint, and attempt lineage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaFailoverRequest {
    pub member: String,
    pub replacement_model: String,
    #[serde(default)]
    pub replacement_harness: Option<String>,
    #[serde(default)]
    pub replacement_host: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

// r[impl kiln.fleet.swarm.watchdog]
// r[impl kiln.fleet.swarm.watchdog.scenario.wake_deadlock_stale]
/// Pure deterministic watchdog evaluation over `run`, `fleet_claims`, and
/// `now_ms`. Emits zero side effects.
#[must_use]
pub fn evaluate_watchdog(
    run: &SwarmRun,
    fleet_claims: &FleetClaimRegistry,
    now_ms: u64,
) -> Vec<WatchdogAction> {
    if run.phase != SwarmRunPhase::Active {
        return Vec::new();
    }
    let mut actions = Vec::new();
    let mut active_count: usize = 0;
    let mut waiting_members: Vec<String> = Vec::new();
    let mut any_wake = false;

    for member in run.members.values() {
        if !member.state.is_active() {
            continue;
        }
        active_count = active_count.saturating_add(1);
        match &member.state {
            SwarmMemberState::Waiting { since_seq } => {
                waiting_members.push(member.name.clone());
                let unread = run.board.unread_mentions_for(member, *since_seq);
                if !unread.is_empty() {
                    any_wake = true;
                    actions.push(WatchdogAction::WakeMember {
                        member: member.name.clone(),
                        unread_seqs: unread,
                        reason: format!(
                            "waiting member {} has unread @mentions after seq {since_seq}",
                            member.name
                        ),
                    });
                }
            }
            SwarmMemberState::Working => {
                let stale_for_ms = now_ms.saturating_sub(member.observed_ms);
                let claim_expired = member.claim_identity.as_ref().is_some_and(|claim_id| {
                    fleet_claims
                        .claims
                        .get(claim_id)
                        .is_some_and(|claim| !claim.is_live(now_ms))
                });
                if stale_for_ms >= run.config.stale_after_ms || claim_expired {
                    let reason = if claim_expired {
                        format!(
                            "working member {} holds an expired fleet claim",
                            member.name
                        )
                    } else {
                        format!(
                            "working member {} exceeded stale_after_ms ({stale_for_ms}ms >= {}ms)",
                            member.name, run.config.stale_after_ms
                        )
                    };
                    actions.push(WatchdogAction::StaleObservationEscalation {
                        member: member.name.clone(),
                        last_observed_ms: member.observed_ms,
                        stale_for_ms,
                        reason,
                    });
                }
            }
            SwarmMemberState::PausedQuota {
                checkpoint_commit,
                board_cursor,
                reason,
                ..
            } => {
                actions.push(WatchdogAction::QuotaFailoverCandidate {
                    member: member.name.clone(),
                    checkpoint_commit: checkpoint_commit.clone(),
                    board_cursor: *board_cursor,
                    paused_reason: reason.clone(),
                });
            }
            SwarmMemberState::Retired | SwarmMemberState::Terminal => {}
        }
    }

    if active_count > 0 && waiting_members.len() == active_count && !any_wake {
        actions.push(WatchdogAction::DeadlockEscalation {
            waiting_members,
            reason: "all active swarm members are waiting with no unread @mentions".to_string(),
        });
    }

    actions
}

// r[impl kiln.fleet.swarm.watchdog]
/// Evaluate the watchdog and, when `apply` is true, wake waiting members with
/// unread mentions and record one auditable `Watchdog` board entry summarizing
/// all emitted actions. Never mutates claims, scopes, or roles.
///
/// # Errors
/// Returns a diagnostic if appending the watchdog board entry exceeds bounds.
pub fn record_watchdog_pass(
    run: &SwarmRun,
    fleet_claims: &FleetClaimRegistry,
    apply: bool,
    now_ms: u64,
) -> CoreResult<(SwarmRun, Vec<WatchdogAction>, Option<BoardPost>)> {
    let actions = evaluate_watchdog(run, fleet_claims, now_ms);
    if !apply || actions.is_empty() {
        return Ok((run.clone(), actions, None));
    }

    let mut next = run.clone();
    let mut mentions = Vec::new();
    let mut summaries = Vec::new();

    for action in &actions {
        match action {
            WatchdogAction::WakeMember {
                member,
                unread_seqs,
                ..
            } => {
                if let Some(target) = next.members.get_mut(member) {
                    target.state = SwarmMemberState::Working;
                    target.observed_ms = target.observed_ms.max(now_ms);
                }
                let mention = MentionTarget::Member(member.clone());
                if !mentions.contains(&mention) {
                    mentions.push(mention);
                }
                summaries.push(format!("wake {member} (unread {:?})", unread_seqs));
            }
            WatchdogAction::DeadlockEscalation {
                waiting_members,
                reason,
            } => {
                let mention = MentionTarget::Role(SwarmRole::Coordinator);
                if !mentions.contains(&mention) {
                    mentions.push(mention);
                }
                summaries.push(format!("deadlock {:?}: {reason}", waiting_members));
            }
            WatchdogAction::StaleObservationEscalation { member, reason, .. } => {
                let mention = MentionTarget::Role(SwarmRole::Coordinator);
                if !mentions.contains(&mention) {
                    mentions.push(mention);
                }
                summaries.push(format!("stale {member}: {reason}"));
            }
            WatchdogAction::QuotaFailoverCandidate {
                member,
                board_cursor,
                ..
            } => {
                let mention = MentionTarget::Role(SwarmRole::Coordinator);
                if !mentions.contains(&mention) {
                    mentions.push(mention);
                }
                summaries.push(format!("quota_failover {member} (cursor {board_cursor})"));
            }
        }
    }

    let post = next.board.append(
        "kiln-watchdog".to_string(),
        SwarmRole::Coordinator,
        BoardPostKind::Watchdog,
        mentions,
        format!("watchdog: {}", summaries.join("; ")),
        None,
        None,
        now_ms,
    )?;
    next.updated_ms = next.updated_ms.max(now_ms);
    Ok((next, actions, Some(post)))
}

// r[impl kiln.fleet.swarm.watchdog]
// r[impl kiln.fleet.swarm.watchdog.scenario.quota_failover]
/// Transition a `PausedQuota` member onto a replacement model (and optional
/// replacement harness/host), preserving `board_cursor`, `checkpoint_commit`,
/// scope/claim binding, and `attempt_lineage`, and posting an auditable
/// `Failover` board entry.
///
/// # Errors
/// Refuses non-active runs, unknown members, members not currently in
/// `PausedQuota`, invalid replacement tokens or reasons, or board bounds.
pub fn apply_quota_failover(
    run: &SwarmRun,
    request: QuotaFailoverRequest,
    now_ms: u64,
) -> CoreResult<(SwarmRun, SwarmMember, BoardPost)> {
    if run.phase != SwarmRunPhase::Active {
        return Err(refusal(
            "fleet_swarm_phase",
            "phase",
            "quota failover may only apply to an active swarm run",
        ));
    }
    let current = run.members.get(&request.member).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "member",
            "no such member in this swarm run",
        )
    })?;
    let (checkpoint_commit, board_cursor, paused_reason) = match &current.state {
        SwarmMemberState::PausedQuota {
            checkpoint_commit,
            board_cursor,
            reason,
            ..
        } => (checkpoint_commit.clone(), *board_cursor, reason.clone()),
        _ => {
            return Err(refusal(
                "fleet_swarm_failover_state",
                "state",
                "quota failover requires the member to be in paused_quota state",
            ));
        }
    };
    if !valid_harness_field(&request.replacement_model) {
        return Err(refusal(
            "fleet_swarm_failover",
            "replacement_model",
            "replacement_model must be a bounded ASCII token",
        ));
    }
    if let Some(harness) = &request.replacement_harness
        && !valid_harness_field(harness)
    {
        return Err(refusal(
            "fleet_swarm_failover",
            "replacement_harness",
            "replacement_harness must be a bounded ASCII token",
        ));
    }
    if let Some(host) = &request.replacement_host
        && !valid_harness_field(host)
    {
        return Err(refusal(
            "fleet_swarm_failover",
            "replacement_host",
            "replacement_host must be a bounded ASCII token",
        ));
    }
    if let Some(reason) = &request.reason
        && !bounded_line(reason, MAX_PAUSE_REASON_BYTES)
    {
        return Err(refusal(
            "fleet_swarm_failover",
            "reason",
            "failover reason is empty, oversized, or contains control characters",
        ));
    }

    let previous_model = current.model.clone();
    let previous_harness = current.harness.clone();
    let mut next = run.clone();
    let updated = {
        let member = next.members.get_mut(&request.member).ok_or_else(|| {
            refusal(
                "fleet_swarm_member_unknown",
                "member",
                "no such member in this swarm run",
            )
        })?;
        member.model = request.replacement_model.clone();
        if let Some(harness) = request.replacement_harness.clone() {
            member.harness = harness;
        }
        if let Some(host) = request.replacement_host.clone() {
            member.host = host;
        }
        // Preserve board_cursor, scope/claim, and attempt_lineage verbatim.
        member.last_read_seq = board_cursor;
        member.state = SwarmMemberState::Working;
        member.observed_ms = member.observed_ms.max(now_ms);
        member.failover_count = member.failover_count.saturating_add(1);
        member.clone()
    };

    let note = request.reason.unwrap_or(paused_reason);
    let checkpoint_label = checkpoint_commit.as_deref().unwrap_or("none");
    let body = format!(
        "FAILOVER {} ({previous_harness}/{previous_model} -> {}/{}) cursor={board_cursor} checkpoint={checkpoint_label} reason={note}",
        updated.name, updated.harness, updated.model
    );
    let post = next.board.append(
        "kiln-failover".to_string(),
        SwarmRole::Coordinator,
        BoardPostKind::Failover,
        vec![MentionTarget::Member(updated.name.clone())],
        body,
        None,
        checkpoint_commit,
        now_ms,
    )?;
    next.updated_ms = next.updated_ms.max(now_ms);
    Ok((next, updated, post))
}
