//! Bounded delegation and claim admission for swarm workers.
//!
//! Enforces every authority invariant fail-closed:
//! - parent `SwarmEnvelope` containment (zero scope widening),
//! - `max_concurrency` cap over active members,
//! - remaining budget > 0,
//! - `Verifier` read-only invariant (forbidden from holding write scopes or claims),
//! - non-overlapping `Author` write scopes against active swarm authors and the
//!   live `FleetClaimRegistry`.

use alloc::string::{String, ToString};
use alloc::vec;

use serde::{Deserialize, Serialize};

use super::board::{BoardPost, BoardPostKind, MentionTarget};
use super::{
    MAX_POST_BODY_BYTES, SwarmMember, SwarmMemberState, SwarmRole, SwarmRun, SwarmRunPhase,
    valid_body_text, valid_harness_field, valid_member_name,
};
use crate::fleet::claims::FleetClaimRegistry;
use crate::fleet::refusal;
use crate::fleet::scope::{FleetScope, overlaps};
use crate::types::{ContentId, CoreResult};

/// Request to spawn one bounded member inside an active swarm run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnMemberRequest {
    pub name: String,
    pub role: SwarmRole,
    pub host: String,
    pub harness: String,
    pub model: String,
    #[serde(default)]
    pub scope: Option<FleetScope>,
    pub brief: String,
    #[serde(default)]
    pub attempt: Option<ContentId>,
    #[serde(default)]
    pub claim_identity: Option<ContentId>,
    #[serde(default)]
    pub claim_fence: Option<u64>,
}

// r[impl kiln.fleet.swarm.delegation]
// r[impl kiln.fleet.swarm.delegation.scenario.scope_widening]
// r[impl kiln.fleet.swarm.delegation.scenario.verifier_read_only]
// r[impl kiln.fleet.swarm.delegation.scenario.overlap_and_bounds]
/// Deterministically admit one worker spawn request into `run`.
///
/// `requester` is `None` for direct operator/control-plane spawns or
/// `Some(name)` when an existing member delegates work; in that case only an
/// active `Coordinator` may delegate.
///
/// # Errors
/// Refuses non-active runs, non-coordinator requesters, duplicate or malformed
/// member fields, concurrency overflow, exhausted budget, any write scope or
/// claim on a `Verifier` (or `Coordinator`), scope widening outside the parent
/// `SwarmEnvelope`, and overlapping author write scopes against either active
/// swarm authors or live `FleetClaimRegistry` claims.
pub fn admit_spawn(
    run: &SwarmRun,
    requester: Option<&str>,
    request: SpawnMemberRequest,
    fleet_claims: &FleetClaimRegistry,
    now_ms: u64,
) -> CoreResult<(SwarmRun, SwarmMember, BoardPost)> {
    if run.phase != SwarmRunPhase::Active {
        return Err(refusal(
            "fleet_swarm_phase",
            "phase",
            "members may only be spawned into an active swarm run",
        ));
    }

    let (brief_sender, brief_sender_role) = match requester {
        Some(name) => {
            let member = run.members.get(name).ok_or_else(|| {
                refusal(
                    "fleet_swarm_member_unknown",
                    "requester",
                    "spawning requester is not a member of this swarm run",
                )
            })?;
            if !member.state.is_active() || member.role != SwarmRole::Coordinator {
                return Err(refusal(
                    "fleet_swarm_coordinator_only",
                    "requester",
                    "only an active coordinator or operator may spawn swarm members",
                ));
            }
            (member.name.clone(), SwarmRole::Coordinator)
        }
        None => ("operator".to_string(), SwarmRole::Coordinator),
    };

    if !valid_member_name(&request.name) {
        return Err(refusal(
            "fleet_swarm_member",
            "name",
            "member name must match [a-z0-9][a-z0-9-]* within 64 bytes",
        ));
    }
    if run.members.contains_key(&request.name) {
        return Err(refusal(
            "fleet_swarm_member_exists",
            "name",
            "a member with this name already exists in the swarm run",
        ));
    }
    for (value, field) in [
        (&request.host, "host"),
        (&request.harness, "harness"),
        (&request.model, "model"),
    ] {
        if !valid_harness_field(value) {
            return Err(refusal(
                "fleet_swarm_member",
                field,
                "member host, harness, and model must be bounded ASCII tokens",
            ));
        }
    }
    if !valid_body_text(&request.brief, MAX_POST_BODY_BYTES) {
        return Err(refusal(
            "fleet_swarm_body",
            "brief",
            "member brief is empty, oversized, or contains NUL",
        ));
    }
    if let Some(attempt) = &request.attempt {
        attempt.validate()?;
    }
    if let Some(claim_id) = &request.claim_identity {
        claim_id.validate()?;
    }

    if run.active_member_count() >= usize::try_from(run.config.max_concurrency).unwrap_or(0) {
        return Err(refusal(
            "fleet_swarm_concurrency",
            "max_concurrency",
            "swarm run reached its active concurrency limit",
        ));
    }
    if !run.budget.has_remaining() {
        return Err(refusal(
            "fleet_swarm_budget",
            "budget",
            "swarm run has no remaining budget",
        ));
    }

    match request.role {
        SwarmRole::Verifier => {
            if request.scope.is_some()
                || request.claim_identity.is_some()
                || request.claim_fence.is_some()
            {
                return Err(refusal(
                    "fleet_swarm_verifier_scope",
                    "scope",
                    "verifier members are strictly read-only and may never hold write scopes or claims",
                ));
            }
        }
        SwarmRole::Coordinator => {
            if request.scope.is_some()
                || request.claim_identity.is_some()
                || request.claim_fence.is_some()
            {
                return Err(refusal(
                    "fleet_swarm_scope_widening",
                    "scope",
                    "coordinator members may not hold author write scopes or claims",
                ));
            }
        }
        SwarmRole::Author => {
            let scope = request.scope.as_ref().ok_or_else(|| {
                refusal(
                    "fleet_swarm_scope",
                    "scope",
                    "author members require an explicit canonical write scope",
                )
            })?;
            scope.validate()?;
            if !run.config.envelope.contains(scope) {
                return Err(refusal(
                    "fleet_swarm_scope_widening",
                    "scope",
                    "requested author scope widens or falls outside the parent swarm envelope",
                ));
            }
            for existing in run.members.values() {
                if existing.state.is_active()
                    && existing.role == SwarmRole::Author
                    && existing
                        .scope
                        .as_ref()
                        .is_some_and(|held| overlaps(held, scope))
                {
                    return Err(refusal(
                        "fleet_swarm_scope_conflict",
                        "scope",
                        "requested author scope overlaps an active author in this swarm run",
                    ));
                }
            }
            match (&request.claim_identity, request.claim_fence) {
                (Some(claim_id), Some(fence)) => {
                    let Some(claim) = fleet_claims.claims.get(claim_id) else {
                        return Err(refusal(
                            "fleet_swarm_claim_conflict",
                            "claim_identity",
                            "referenced fleet claim does not exist",
                        ));
                    };
                    if !claim.is_live(now_ms)
                        || claim.fence != fence
                        || claim.holder.host != request.host
                        || !crate::fleet::scope::contains_scope(&claim.scope, scope)
                    {
                        return Err(refusal(
                            "fleet_swarm_claim_conflict",
                            "claim_identity",
                            "referenced fleet claim is not a live, matching fence/host/scope grant",
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(refusal(
                        "fleet_swarm_claim_conflict",
                        "claim_identity",
                        "claim_identity and claim_fence must be supplied together",
                    ));
                }
            }
            for claim in fleet_claims.live(now_ms) {
                let same_claim = request.claim_identity.as_ref().is_some_and(|id| {
                    id == &claim.identity && request.claim_fence == Some(claim.fence)
                });
                if !same_claim && overlaps(&claim.scope, scope) {
                    return Err(refusal(
                        "fleet_swarm_claim_conflict",
                        "scope",
                        "requested author scope overlaps a live fleet claim",
                    ));
                }
            }
        }
    }

    let head_before = run.board.head_seq();
    let attempt_lineage = request.attempt.into_iter().collect();
    let member = SwarmMember {
        name: request.name.clone(),
        role: request.role,
        state: SwarmMemberState::Working,
        host: request.host,
        harness: request.harness,
        model: request.model,
        scope: request.scope,
        claim_identity: request.claim_identity,
        claim_fence: request.claim_fence,
        attempt_lineage,
        budget_draw: 0,
        last_read_seq: head_before,
        last_post_seq: None,
        spawned_ms: now_ms,
        observed_ms: now_ms,
        failover_count: 0,
    };

    let mut next = run.clone();
    next.members.insert(member.name.clone(), member.clone());
    let post = next.board.append(
        brief_sender,
        brief_sender_role,
        BoardPostKind::Brief,
        vec![MentionTarget::Member(member.name.clone())],
        request.brief,
        None,
        None,
        now_ms,
    )?;
    if let Some(sender_member) = requester.and_then(|name| next.members.get_mut(name)) {
        sender_member.last_post_seq = Some(post.seq);
        sender_member.observed_ms = sender_member.observed_ms.max(now_ms);
    }
    next.updated_ms = next.updated_ms.max(now_ms);
    Ok((next, member, post))
}
