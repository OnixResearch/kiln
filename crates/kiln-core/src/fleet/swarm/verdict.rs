//! Separated `@verifier` repair loop bound to exact candidate commits.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::board::{BoardPost, BoardPostKind, MentionTarget};
use super::{
    MAX_ESCALATION_REASON_BYTES, MAX_POST_BODY_BYTES, MAX_VERDICT_FINDING_BYTES,
    MAX_VERDICT_FINDINGS, SwarmMemberState, SwarmRole, SwarmRun, SwarmRunPhase, truncate_for_board,
    valid_body_text, valid_commit_sha,
};
use crate::fleet::refusal;
use crate::types::CoreResult;

/// Structured decision emitted by a separated `@verifier`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum VerdictDecision {
    Approve,
    RequestChanges { findings: Vec<String> },
    Blocked { reason: String },
}

/// Recorded verifier verdict bound to an exact candidate commit and author.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierVerdict {
    pub seq: u64,
    pub verifier: String,
    pub author: String,
    pub commit: String,
    pub repair_round: u32,
    pub decision: VerdictDecision,
    pub recorded_ms: u64,
}

/// Control-plane transition produced by admitting one verifier verdict.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum VerdictOutcome {
    ReadyForPublication { commit: String },
    WakeAuthor { author: String, repair_round: u32 },
    Escalated { reason: String, repair_round: u32 },
}

// r[impl kiln.fleet.swarm.verdict]
// r[impl kiln.fleet.swarm.verdict.scenario.repair_and_escalation]
/// Submit a structured verifier verdict bound to `commit` and `author`.
///
/// Enforces:
/// - `verifier` MUST be an active `SwarmRole::Verifier` distinct from `author`,
/// - `author` MUST be a `SwarmRole::Author` in the run,
/// - `commit` MUST be a full lowercase hex commit SHA (40 or 64 chars),
/// - `Approve` transitions the run to `ReadyForPublication` bound to `commit`,
/// - `RequestChanges` increments `repair_round`, wakes `author` if within
///   `max_repair_rounds`, or escalates the run when `max_repair_rounds` is
///   exceeded,
/// - `Blocked` escalates the run for operator decision.
///
/// # Errors
/// Refuses non-active runs, unknown or non-verifier submitters, unknown or
/// non-author targets, self-verification, malformed commits, empty/oversized
/// findings or block reasons, and board bounds.
pub fn submit_verdict(
    run: &SwarmRun,
    verifier: &str,
    author: &str,
    commit: String,
    decision: VerdictDecision,
    now_ms: u64,
) -> CoreResult<(SwarmRun, VerifierVerdict, VerdictOutcome, BoardPost)> {
    if run.phase != SwarmRunPhase::Active {
        return Err(refusal(
            "fleet_swarm_phase",
            "phase",
            "verdicts may only be submitted for an active swarm run",
        ));
    }
    let verifier_member = run.members.get(verifier).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "verifier",
            "no such verifier member in this swarm run",
        )
    })?;
    if !verifier_member.state.is_active() || verifier_member.role != SwarmRole::Verifier {
        return Err(refusal(
            "fleet_swarm_verifier_only",
            "verifier",
            "only an active verifier member may submit verification verdicts",
        ));
    }
    if verifier == author {
        return Err(refusal(
            "fleet_swarm_verifier_only",
            "author",
            "a verifier cannot verify its own work",
        ));
    }
    let author_member = run.members.get(author).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "author",
            "no such author member in this swarm run",
        )
    })?;
    if author_member.role != SwarmRole::Author {
        return Err(refusal(
            "fleet_swarm_verdict_target",
            "author",
            "verdict target must be an author member",
        ));
    }
    if !valid_commit_sha(&commit) {
        return Err(refusal(
            "fleet_swarm_commit",
            "commit",
            "verdict commit must be a 40- or 64-character lowercase hex id",
        ));
    }
    validate_decision(&decision)?;

    let mut next = run.clone();
    let (outcome, post_kind, mentions, body, round_recorded) = match &decision {
        VerdictDecision::Approve => {
            next.phase = SwarmRunPhase::ReadyForPublication;
            next.approved_commit = Some(commit.clone());
            (
                VerdictOutcome::ReadyForPublication {
                    commit: commit.clone(),
                },
                BoardPostKind::Verdict,
                vec![MentionTarget::Role(SwarmRole::Coordinator)],
                format!("APPROVE {author} @ {commit}"),
                next.repair_round,
            )
        }
        VerdictDecision::RequestChanges { findings } => {
            let next_round = next.repair_round.saturating_add(1);
            next.repair_round = next_round;
            let joined = findings.join(" | ");
            if next_round <= next.config.max_repair_rounds {
                if let Some(target_author) = next.members.get_mut(author)
                    && matches!(target_author.state, SwarmMemberState::Waiting { .. })
                {
                    target_author.state = SwarmMemberState::Working;
                    target_author.observed_ms = target_author.observed_ms.max(now_ms);
                }
                (
                    VerdictOutcome::WakeAuthor {
                        author: author.to_string(),
                        repair_round: next_round,
                    },
                    BoardPostKind::Verdict,
                    vec![MentionTarget::Member(author.to_string())],
                    format!(
                        "REQUEST_CHANGES (round {next_round}/{}) for {author} @ {commit}: {joined}",
                        next.config.max_repair_rounds
                    ),
                    next_round,
                )
            } else {
                let reason = format!(
                    "repair rounds exhausted ({next_round} > {}) for {author} @ {commit}",
                    next.config.max_repair_rounds
                );
                next.phase = SwarmRunPhase::Escalated;
                next.escalation_reason = Some(reason.clone());
                (
                    VerdictOutcome::Escalated {
                        reason: reason.clone(),
                        repair_round: next_round,
                    },
                    BoardPostKind::Decision,
                    vec![MentionTarget::Role(SwarmRole::Coordinator)],
                    format!("ESCALATED: {reason}; findings: {joined}"),
                    next_round,
                )
            }
        }
        VerdictDecision::Blocked { reason } => {
            next.phase = SwarmRunPhase::Escalated;
            next.escalation_reason = Some(reason.clone());
            (
                VerdictOutcome::Escalated {
                    reason: reason.clone(),
                    repair_round: next.repair_round,
                },
                BoardPostKind::Decision,
                vec![MentionTarget::Role(SwarmRole::Coordinator)],
                format!("BLOCKED for {author} @ {commit}: {reason}"),
                next.repair_round,
            )
        }
    };

    let body = truncate_for_board(body, MAX_POST_BODY_BYTES);
    let post = next.board.append(
        verifier.to_string(),
        SwarmRole::Verifier,
        post_kind,
        mentions,
        body,
        None,
        Some(commit.clone()),
        now_ms,
    )?;

    let verdict = VerifierVerdict {
        seq: post.seq,
        verifier: verifier.to_string(),
        author: author.to_string(),
        commit,
        repair_round: round_recorded,
        decision,
        recorded_ms: now_ms,
    };
    next.verdicts.push(verdict.clone());
    if let Some(v) = next.members.get_mut(verifier) {
        v.last_post_seq = Some(post.seq);
        v.observed_ms = v.observed_ms.max(now_ms);
    }
    next.updated_ms = next.updated_ms.max(now_ms);
    Ok((next, verdict, outcome, post))
}

fn validate_decision(decision: &VerdictDecision) -> CoreResult<()> {
    match decision {
        VerdictDecision::Approve => Ok(()),
        VerdictDecision::RequestChanges { findings } => {
            if findings.is_empty() || findings.len() > MAX_VERDICT_FINDINGS {
                return Err(refusal(
                    "fleet_swarm_verdict",
                    "findings",
                    "request_changes requires between 1 and 64 findings",
                ));
            }
            for finding in findings {
                if !valid_body_text(finding, MAX_VERDICT_FINDING_BYTES) {
                    return Err(refusal(
                        "fleet_swarm_verdict",
                        "findings",
                        "verdict finding is empty, oversized, or contains NUL",
                    ));
                }
            }
            Ok(())
        }
        VerdictDecision::Blocked { reason } => {
            if !valid_body_text(reason, MAX_ESCALATION_REASON_BYTES) {
                return Err(refusal(
                    "fleet_swarm_verdict",
                    "reason",
                    "blocked verdict reason is empty, oversized, or contains NUL",
                ));
            }
            Ok(())
        }
    }
}
