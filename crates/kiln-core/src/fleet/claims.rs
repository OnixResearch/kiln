//! Fenced, leased fleet claims with Kiln as the only grantor.
//!
//! [`decide`] is the whole state machine: it admits one command against the
//! registry, the active implementation attempts and the open foreign swarm
//! claims, and returns the next registry, the outcome and the swarm articles
//! the shell must post. Kiln is the swarm writer for fleet claims; mirror
//! outcomes come back through [`ClaimCommand::MirrorObserved`].

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::scope::{ClaimVerb, FleetScope, overlaps, scope_subject};
use super::{bounded_line, bounded_token, content_identity, push_field, refusal};
use crate::implementation::swarm::{
    FleetClaimBodyFacts, FleetReleaseBodyFacts, FleetSignature, MAX_MESSAGE_ID_BYTES,
    STALE_CLAIM_MS, claim_subject, format_fleet_claim_body, format_fleet_release_body,
    release_subject,
};
use crate::types::{ContentId, CoreResult, Diagnostic};

pub const FLEET_CLAIM_SCHEMA: &str = "kiln.fleet-claim.v1";
pub const MS_PER_MINUTE: u64 = 60 * 1_000;
pub const DEFAULT_CLAIM_LEASE_MS: u64 = 4 * 60 * MS_PER_MINUTE;
pub const MIN_CLAIM_LEASE_MS: u64 = MS_PER_MINUTE;
pub const MAX_CLAIM_LEASE_MS: u64 = 24 * 60 * MS_PER_MINUTE;
pub const MAX_OPEN_FLEET_CLAIMS: usize = 256;
pub const MAX_PURPOSE_BYTES: usize = 256;
pub const MAX_RELEASE_NOTE_BYTES: usize = 256;
/// Bound for each of a holder's tool, host and session fields.
pub const MAX_AGENT_FIELD_BYTES: usize = 64;
/// A foreign swarm claim older than this no longer conflicts.
pub const FOREIGN_CLAIM_STALE_MS: u64 = STALE_CLAIM_MS;
/// First fence a fresh registry hands out; fences never repeat.
pub const FIRST_FLEET_FENCE: u64 = 1;
/// Release reason rendered for a claim swept after its lease.
pub const EXPIRED_RELEASE_REASON: &str = "lease expired";
/// Release reason rendered when a release carries no note.
pub const DEFAULT_RELEASE_REASON: &str = "released";

/// The agent holding a claim. All agents on one machine share its swarm
/// account, so the tool and session distinguish them.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetAgent {
    pub tool: String,
    pub host: String,
    pub session: String,
}

impl FleetAgent {
    /// Human-facing holder label: `<tool> on <host>, session <session>`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{} on {}, session {}", self.tool, self.host, self.session)
    }

    /// # Errors
    /// Refuses empty, oversized, non-ASCII or whitespace-bearing fields.
    pub fn validate(&self) -> CoreResult<()> {
        for (value, field) in [
            (&self.tool, "holder.tool"),
            (&self.host, "holder.host"),
            (&self.session, "holder.session"),
        ] {
            if !bounded_token(value, MAX_AGENT_FIELD_BYTES) {
                return Err(refusal(
                    "fleet_claim_holder",
                    field,
                    "holder field is empty, oversized, non-ASCII or contains whitespace",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Open,
    Released { at_ms: u64, note: String },
    Expired { at_ms: u64 },
}

/// Outcome of one swarm article post.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorStatus {
    /// Requested and not yet reported.
    Pending,
    /// Posted; the message id once the shell has read it back.
    Posted { message_id: Option<String> },
    /// The post outcome is ambiguous. The shell reconciles it by reading the
    /// newest window and never reposts blindly.
    Unknown,
    /// The post definitely did not happen.
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetClaim {
    pub identity: ContentId,
    pub scope: FleetScope,
    pub holder: FleetAgent,
    pub purpose: String,
    pub fence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_fence: Option<u64>,
    pub granted_ms: u64,
    pub expires_ms: u64,
    pub state: ClaimState,
    pub attempt: Option<ContentId>,
    pub claim_mirror: MirrorStatus,
    pub release_mirror: MirrorStatus,
}

impl FleetClaim {
    /// Open and unexpired at `now_ms`.
    #[must_use]
    pub fn is_live(&self, now_ms: u64) -> bool {
        self.state == ClaimState::Open && self.expires_ms > now_ms
    }

    /// Fence of the original `CLAIM` article; falls back to [`Self::fence`]
    /// for claims persisted before `initial_fence` was recorded.
    #[must_use]
    pub const fn initial_fence(&self) -> u64 {
        match self.initial_fence {
            Some(fence) => fence,
            None => self.fence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetClaimRegistry {
    pub claims: BTreeMap<ContentId, FleetClaim>,
    pub next_fence: u64,
}

impl Default for FleetClaimRegistry {
    fn default() -> Self {
        Self {
            claims: BTreeMap::new(),
            next_fence: FIRST_FLEET_FENCE,
        }
    }
}

impl FleetClaimRegistry {
    /// Live claims at `now_ms`, in identity order.
    pub fn live(&self, now_ms: u64) -> impl Iterator<Item = &FleetClaim> {
        self.claims
            .values()
            .filter(move |claim| claim.is_live(now_ms))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimCommand {
    Acquire {
        scope: FleetScope,
        holder: FleetAgent,
        purpose: String,
        lease_ms: u64,
        attempt: Option<ContentId>,
    },
    Renew {
        identity: ContentId,
        fence: u64,
        lease_ms: u64,
    },
    Release {
        identity: ContentId,
        fence: u64,
        note: String,
    },
    Sweep,
    MirrorObserved {
        identity: ContentId,
        release: bool,
        status: MirrorStatus,
    },
}

/// The repository and one permitted path of a claimed, executing,
/// candidate-produced or verifying attempt. The shell derives these.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptScope {
    pub attempt: ContentId,
    pub scope: FleetScope,
}

/// An open swarm claim not written by this Kiln.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignClaim {
    pub scope: FleetScope,
    pub message_id: String,
    pub author: String,
    pub date_unix_ms: u64,
}

/// Facts outside the registry that one decision reads.
#[derive(Clone, Copy, Debug)]
pub struct ClaimContext<'a> {
    pub now_ms: u64,
    pub attempts: &'a [AttemptScope],
    pub foreign: &'a [ForeignClaim],
    /// Machine label of the coordinating Kiln, for article signatures.
    pub coordinator: &'a str,
}

/// The overlapping source that refused an acquisition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimConflict {
    Claim {
        identity: ContentId,
        holder: String,
        scope: String,
    },
    Attempt {
        attempt: ContentId,
        scope: String,
    },
    Foreign {
        message_id: String,
        author: String,
        scope: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimOutcome {
    Granted(FleetClaim),
    Existing(FleetClaim),
    Renewed(FleetClaim),
    Released(FleetClaim),
    Swept(Vec<ContentId>),
    Recorded,
    Conflict(ClaimConflict),
}

/// Swarm articles the shell must post, exactly once each.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimEffect {
    PostClaim {
        identity: ContentId,
        subject: String,
        body: String,
    },
    PostRelease {
        identity: ContentId,
        subject: String,
        references: Option<String>,
        body: String,
    },
}

type Decided = (FleetClaimRegistry, ClaimOutcome, Vec<ClaimEffect>);

// r[impl kiln.fleet.claims] r[impl kiln.fleet.mirror]
/// Decide one claim command.
///
/// # Errors
/// Refuses malformed commands, unknown identities, stale fences, closed
/// claims, leases outside their bounds, the open-claim bound and mirror
/// observations that would rewrite a settled post. A conflict is an outcome,
/// not an error.
pub fn decide(
    registry: &FleetClaimRegistry,
    command: ClaimCommand,
    context: &ClaimContext<'_>,
) -> CoreResult<Decided> {
    match command {
        ClaimCommand::Acquire {
            scope,
            holder,
            purpose,
            lease_ms,
            attempt,
        } => acquire(
            registry,
            Acquisition {
                scope,
                holder,
                purpose,
                lease_ms,
                attempt,
            },
            context,
        ),
        ClaimCommand::Renew {
            identity,
            fence,
            lease_ms,
        } => renew(registry, &identity, fence, lease_ms, context.now_ms),
        ClaimCommand::Release {
            identity,
            fence,
            note,
        } => release(registry, &identity, fence, note, context),
        ClaimCommand::Sweep => sweep(registry, context),
        ClaimCommand::MirrorObserved {
            identity,
            release,
            status,
        } => mirror_observed(registry, &identity, release, status),
    }
}

/// Articles still owed for claims whose mirror status is `Pending`, for a
/// shell recovering after a crash between saving and posting. `Unknown` posts
/// are never returned: they are reconciled by reading the newest window.
///
/// # Errors
/// Returns a diagnostic when a stored claim can no longer be rendered.
pub fn pending_mirror_effects(
    registry: &FleetClaimRegistry,
    coordinator: &str,
) -> CoreResult<Vec<ClaimEffect>> {
    let mut effects = Vec::new();
    for claim in registry.claims.values() {
        if claim.claim_mirror == MirrorStatus::Pending {
            effects.push(claim_effect(claim, coordinator)?);
        }
        if claim.state != ClaimState::Open && claim.release_mirror == MirrorStatus::Pending {
            effects.extend(release_effect(claim, coordinator)?);
        }
    }
    Ok(effects)
}

/// Deterministic claim identity over the scope subject, holder, grant time
/// and fence.
///
/// # Errors
/// Returns a diagnostic only when a field exceeds the encoding bound.
pub fn claim_identity(
    scope: &FleetScope,
    holder: &FleetAgent,
    granted_ms: u64,
    fence: u64,
) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_field(&mut bytes, FLEET_CLAIM_SCHEMA, "claim.identity")?;
    push_field(
        &mut bytes,
        &scope_subject(ClaimVerb::Claim, scope),
        "claim.identity",
    )?;
    push_field(&mut bytes, &holder.tool, "claim.identity")?;
    push_field(&mut bytes, &holder.host, "claim.identity")?;
    push_field(&mut bytes, &holder.session, "claim.identity")?;
    bytes.extend_from_slice(&granted_ms.to_be_bytes());
    bytes.extend_from_slice(&fence.to_be_bytes());
    content_identity(&bytes)
}

struct Acquisition {
    scope: FleetScope,
    holder: FleetAgent,
    purpose: String,
    lease_ms: u64,
    attempt: Option<ContentId>,
}

fn acquire(
    registry: &FleetClaimRegistry,
    request: Acquisition,
    context: &ClaimContext<'_>,
) -> CoreResult<Decided> {
    let now_ms = context.now_ms;
    request.scope.validate()?;
    request.holder.validate()?;
    if !bounded_line(&request.purpose, MAX_PURPOSE_BYTES) {
        return Err(refusal(
            "fleet_claim_purpose",
            "purpose",
            "purpose is empty, oversized or contains a control character",
        ));
    }
    admit_lease(request.lease_ms)?;
    if let Some(attempt) = &request.attempt {
        attempt.validate()?;
    }
    if let Some(existing) = registry
        .live(now_ms)
        .find(|claim| claim.holder == request.holder && claim.scope == request.scope)
    {
        return Ok((
            registry.clone(),
            ClaimOutcome::Existing(existing.clone()),
            Vec::new(),
        ));
    }
    if let Some(conflict) = conflict(registry, &request, context) {
        return Ok((
            registry.clone(),
            ClaimOutcome::Conflict(conflict),
            Vec::new(),
        ));
    }
    if registry.live(now_ms).count() >= MAX_OPEN_FLEET_CLAIMS {
        return Err(refusal(
            "fleet_claim_limit",
            "claims",
            "open fleet claims reached their bound",
        ));
    }
    let fence = registry.next_fence;
    let next_fence = fence.checked_add(1).ok_or_else(|| {
        refusal(
            "fleet_claim_fence",
            "next_fence",
            "fleet claim fences are exhausted",
        )
    })?;
    let expires_ms = now_ms.checked_add(request.lease_ms).ok_or_else(|| {
        refusal(
            "fleet_claim_lease",
            "lease_ms",
            "claim lease overflows the clock",
        )
    })?;
    let identity = claim_identity(&request.scope, &request.holder, now_ms, fence)?;
    if registry.claims.contains_key(&identity) {
        return Err(refusal(
            "fleet_claim_identity",
            "claim.identity",
            "claim identity already exists",
        ));
    }
    let claim = FleetClaim {
        identity: identity.clone(),
        scope: request.scope,
        holder: request.holder,
        purpose: request.purpose,
        fence,
        initial_fence: Some(fence),
        granted_ms: now_ms,
        expires_ms,
        state: ClaimState::Open,
        attempt: request.attempt,
        claim_mirror: MirrorStatus::Pending,
        release_mirror: MirrorStatus::Pending,
    };
    let effect = claim_effect(&claim, context.coordinator)?;
    let mut next = registry.clone();
    next.next_fence = next_fence;
    next.claims.insert(identity, claim.clone());
    Ok((next, ClaimOutcome::Granted(claim), alloc::vec![effect]))
}

fn conflict(
    registry: &FleetClaimRegistry,
    request: &Acquisition,
    context: &ClaimContext<'_>,
) -> Option<ClaimConflict> {
    let now_ms = context.now_ms;
    if let Some(claim) = registry
        .live(now_ms)
        .find(|claim| claim.holder != request.holder && overlaps(&claim.scope, &request.scope))
    {
        return Some(ClaimConflict::Claim {
            identity: claim.identity.clone(),
            holder: claim.holder.label(),
            scope: claim.scope.text(),
        });
    }
    if let Some(active) = context.attempts.iter().find(|active| {
        request.attempt.as_ref() != Some(&active.attempt) && overlaps(&active.scope, &request.scope)
    }) {
        return Some(ClaimConflict::Attempt {
            attempt: active.attempt.clone(),
            scope: active.scope.text(),
        });
    }
    context
        .foreign
        .iter()
        .find(|foreign| {
            now_ms.saturating_sub(foreign.date_unix_ms) <= FOREIGN_CLAIM_STALE_MS
                && overlaps(&foreign.scope, &request.scope)
        })
        .map(|foreign| ClaimConflict::Foreign {
            message_id: foreign.message_id.clone(),
            author: foreign.author.clone(),
            scope: foreign.scope.text(),
        })
}

fn renew(
    registry: &FleetClaimRegistry,
    identity: &ContentId,
    fence: u64,
    lease_ms: u64,
    now_ms: u64,
) -> CoreResult<Decided> {
    admit_lease(lease_ms)?;
    let claim = current_claim(registry, identity, fence)?;
    if claim.expires_ms <= now_ms {
        return Err(stale("claim lease has already expired"));
    }
    let mut renewed = claim.clone();
    renewed.expires_ms = now_ms.checked_add(lease_ms).ok_or_else(|| {
        refusal(
            "fleet_claim_lease",
            "lease_ms",
            "claim lease overflows the clock",
        )
    })?;
    let mut next = registry.clone();
    next.claims.insert(identity.clone(), renewed.clone());
    Ok((next, ClaimOutcome::Renewed(renewed), Vec::new()))
}

fn release(
    registry: &FleetClaimRegistry,
    identity: &ContentId,
    fence: u64,
    note: String,
    context: &ClaimContext<'_>,
) -> CoreResult<Decided> {
    if !note.is_empty() && !bounded_line(&note, MAX_RELEASE_NOTE_BYTES) {
        return Err(refusal(
            "fleet_claim_note",
            "note",
            "release note is oversized or contains a control character",
        ));
    }
    let claim = current_claim(registry, identity, fence)?;
    let mut released = claim.clone();
    released.state = ClaimState::Released {
        at_ms: context.now_ms,
        note,
    };
    released.release_mirror = MirrorStatus::Pending;
    let effects = release_effect(&released, context.coordinator)?
        .into_iter()
        .collect();
    let mut next = registry.clone();
    next.claims.insert(identity.clone(), released.clone());
    Ok((next, ClaimOutcome::Released(released), effects))
}

fn sweep(registry: &FleetClaimRegistry, context: &ClaimContext<'_>) -> CoreResult<Decided> {
    let mut next = registry.clone();
    let mut swept = Vec::new();
    let mut effects = Vec::new();
    for claim in next.claims.values_mut() {
        if claim.state == ClaimState::Open && claim.expires_ms <= context.now_ms {
            claim.state = ClaimState::Expired {
                at_ms: context.now_ms,
            };
            claim.release_mirror = MirrorStatus::Pending;
            effects.extend(release_effect(claim, context.coordinator)?);
            swept.push(claim.identity.clone());
        }
    }
    Ok((next, ClaimOutcome::Swept(swept), effects))
}

fn mirror_observed(
    registry: &FleetClaimRegistry,
    identity: &ContentId,
    release: bool,
    status: MirrorStatus,
) -> CoreResult<Decided> {
    let claim = registry.claims.get(identity).ok_or_else(unknown_claim)?;
    match &status {
        MirrorStatus::Pending => {
            return Err(refusal(
                "fleet_claim_mirror",
                "status",
                "a mirror observation cannot return a post to pending",
            ));
        }
        MirrorStatus::Posted {
            message_id: Some(message_id),
        } if !bounded_token(message_id, MAX_MESSAGE_ID_BYTES) => {
            return Err(refusal(
                "fleet_claim_mirror",
                "message_id",
                "message id is empty, oversized or malformed",
            ));
        }
        _ => {}
    }
    if release && claim.state == ClaimState::Open {
        return Err(refusal(
            "fleet_claim_mirror",
            "release",
            "no release article was requested for an open claim",
        ));
    }
    let current = if release {
        &claim.release_mirror
    } else {
        &claim.claim_mirror
    };
    if matches!(current, MirrorStatus::Posted { .. }) {
        if *current == status {
            return Ok((registry.clone(), ClaimOutcome::Recorded, Vec::new()));
        }
        // A posted article may learn its message id once; nothing else moves.
        let learns_id = matches!(
            (current, &status),
            (
                MirrorStatus::Posted { message_id: None },
                MirrorStatus::Posted {
                    message_id: Some(_)
                }
            )
        );
        if !learns_id {
            return Err(refusal(
                "fleet_claim_mirror",
                "status",
                "a posted article cannot be rewritten",
            ));
        }
    }
    let mut updated = claim.clone();
    if release {
        updated.release_mirror = status;
    } else {
        updated.claim_mirror = status;
    }
    let mut next = registry.clone();
    next.claims.insert(identity.clone(), updated);
    Ok((next, ClaimOutcome::Recorded, Vec::new()))
}

fn current_claim<'a>(
    registry: &'a FleetClaimRegistry,
    identity: &ContentId,
    fence: u64,
) -> CoreResult<&'a FleetClaim> {
    let claim = registry.claims.get(identity).ok_or_else(unknown_claim)?;
    if claim.state != ClaimState::Open {
        return Err(stale("claim is no longer open"));
    }
    if claim.fence != fence {
        return Err(stale("claim fence is not current"));
    }
    Ok(claim)
}

fn admit_lease(lease_ms: u64) -> CoreResult<()> {
    if !(MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS).contains(&lease_ms) {
        return Err(refusal(
            "fleet_claim_lease",
            "lease_ms",
            "claim lease is outside its bounds",
        ));
    }
    Ok(())
}

fn claim_effect(claim: &FleetClaim, coordinator: &str) -> CoreResult<ClaimEffect> {
    let target = claim.scope.target_text();
    let path = claim.scope.path_text();
    let subject = claim_subject(target, path).map_err(mirror_refusal)?;
    let lease_ms = claim.expires_ms.saturating_sub(claim.granted_ms);
    let body = format_fleet_claim_body(&FleetClaimBodyFacts {
        target,
        path,
        purpose: &claim.purpose,
        fence: claim.initial_fence(),
        lease_minutes: lease_ms.div_ceil(MS_PER_MINUTE),
        signature: signature(claim, coordinator),
    })
    .map_err(mirror_refusal)?;
    Ok(ClaimEffect::PostClaim {
        identity: claim.identity.clone(),
        subject,
        body,
    })
}

fn release_effect(claim: &FleetClaim, coordinator: &str) -> CoreResult<Option<ClaimEffect>> {
    let target = claim.scope.target_text();
    let path = claim.scope.path_text();
    let subject = release_subject(target, path).map_err(mirror_refusal)?;
    let reason = match &claim.state {
        ClaimState::Released { note, .. } if !note.is_empty() => note.as_str(),
        ClaimState::Expired { .. } => EXPIRED_RELEASE_REASON,
        _ => DEFAULT_RELEASE_REASON,
    };
    let body = format_fleet_release_body(&FleetReleaseBodyFacts {
        target,
        path,
        reason,
        fence: claim.fence,
        signature: signature(claim, coordinator),
    })
    .map_err(mirror_refusal)?;
    let MirrorStatus::Posted {
        message_id: Some(message_id),
    } = &claim.claim_mirror
    else {
        return Ok(None);
    };
    Ok(Some(ClaimEffect::PostRelease {
        identity: claim.identity.clone(),
        subject,
        references: Some(message_id.clone()),
        body,
    }))
}

fn signature<'a>(claim: &'a FleetClaim, coordinator: &'a str) -> FleetSignature<'a> {
    FleetSignature {
        coordinator,
        tool: &claim.holder.tool,
        host: &claim.holder.host,
        session: &claim.holder.session,
    }
}

fn mirror_refusal(message: String) -> Diagnostic {
    Diagnostic {
        code: "fleet_claim_mirror".to_string(),
        field: "article".to_string(),
        message,
    }
}

fn unknown_claim() -> Diagnostic {
    refusal("fleet_claim_unknown", "identity", "no such fleet claim")
}

fn stale(message: &str) -> Diagnostic {
    refusal("fleet_claim_stale", "fence", message)
}

#[cfg(test)]
#[path = "claims_tests.rs"]
mod tests;
