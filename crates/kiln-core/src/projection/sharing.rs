//! Attenuated view sharing.
//!
//! A view reference is only a link: it confers nothing. Sharing is its own
//! operation. [`admit_share`] derives a [`SharePlan`] for one recipient that is
//! no broader than the sharer's current [`EffectiveSourceScope`] and the view's
//! frozen [`ViewCeiling`] in resource, operation, snapshot, lifetime and depth,
//! and it requires the `delegate` ability over every shared path. Reading does
//! not imply sharing, editing or publishing. Requests carry origin path
//! components only, so filters, tags, display aliases and definition edits
//! have no way to widen a child. Issuing the delegation is a shell effect.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::authority::{
    EffectiveSourceScope, MAX_DELEGATION_DEPTH, MAX_SOURCE_PRINCIPAL_BYTES, PathScope,
    SnapshotRule, SourceGrant, SourceViewOperation, valid_principal,
};
use super::disclosure::ViewCeiling;
use super::update_external;
use crate::monorepo::{finish, update_field, update_usize};
use crate::types::{ContentId, ExternalRef};

pub const SHARE_PLAN_SCHEMA: &str = "kiln.source-share-plan.v1";
/// Paths one share may name.
pub const MAX_SHARE_PATHS: usize = 32;

/// A request to share attenuated access with one recipient.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShareRequest {
    /// The recipient audience, one exact DID.
    pub recipient: String,
    /// Origin path scopes; never display aliases.
    pub paths: Vec<PathScope>,
    pub operations: BTreeSet<SourceViewOperation>,
    pub snapshot: SnapshotRule,
    pub not_after_s: u64,
    /// Delegation depth the recipient may use in turn.
    pub depth: u8,
}

/// Why a share was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareRefusal {
    /// A field is malformed or out of bounds.
    RequestInvalid,
    /// The sharer's scope expired.
    ParentExpired,
    /// The sharer lacks `delegate` over a shared path, or has no depth left.
    DelegationNotPermitted,
    /// A path lies outside the sharer's grants or the view ceiling.
    WidenResource,
    /// An operation exceeds the sharer's grants or the view ceiling.
    WidenOperation,
    /// The snapshot rule names revisions outside the sharer's.
    WidenSnapshot,
    /// The share would outlive the sharer's scope.
    OutlivesParent,
    /// The recipient's depth is not below the sharer's.
    DepthExceeded,
}

// r[impl kiln.source.projection.sharing.attenuation]
/// An admitted, not yet issued, attenuated delegation. It binds the sharer's
/// scope and generations and the frozen ceiling, so neither a later scope nor
/// a broadened view definition changes what it grants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SharePlan {
    issuer: String,
    recipient: String,
    repository: ContentId,
    snapshot: SnapshotRule,
    parent_revision: ExternalRef,
    ceiling: ViewCeiling,
    grants: Vec<SourceGrant>,
    not_after_s: u64,
    depth: u8,
    parent_scope: ContentId,
    generation: u64,
    revocation_generation: u64,
    identity: ContentId,
}

impl SharePlan {
    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }
    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }
    #[must_use]
    pub fn snapshot(&self) -> &SnapshotRule {
        &self.snapshot
    }
    #[must_use]
    pub fn parent_revision(&self) -> &ExternalRef {
        &self.parent_revision
    }
    /// The ceiling frozen at admission; the recipient's views use it.
    #[must_use]
    pub fn ceiling(&self) -> &ViewCeiling {
        &self.ceiling
    }
    #[must_use]
    pub fn grants(&self) -> &[SourceGrant] {
        &self.grants
    }
    #[must_use]
    pub fn not_after_s(&self) -> u64 {
        self.not_after_s
    }
    #[must_use]
    pub fn depth(&self) -> u8 {
        self.depth
    }
    #[must_use]
    pub fn parent_scope(&self) -> &ContentId {
        &self.parent_scope
    }
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn revocation_generation(&self) -> u64 {
        self.revocation_generation
    }
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
}

fn valid_recipient(recipient: &str) -> bool {
    valid_principal(recipient, MAX_SOURCE_PRINCIPAL_BYTES)
        && recipient.starts_with("did:")
        && !recipient.contains('*')
        && !recipient.bytes().any(|byte| byte.is_ascii_whitespace())
}

fn validate(request: &ShareRequest) -> bool {
    valid_recipient(&request.recipient)
        && !request.paths.is_empty()
        && request.paths.len() <= MAX_SHARE_PATHS
        && request.paths.iter().all(|path| path.validate().is_ok())
        && !request.operations.is_empty()
        && request.snapshot.validate().is_ok()
        && request.depth <= MAX_DELEGATION_DEPTH
}

/// Admit one share against the sharer's current scope and the view ceiling.
///
/// # Errors
/// Returns the first refusal: invalid request, expired or outlived parent,
/// widened resource, missing delegation, depth, widened operation or widened
/// snapshot.
pub fn admit_share(
    parent: &EffectiveSourceScope,
    ceiling: &ViewCeiling,
    request: &ShareRequest,
    now_unix_s: u64,
) -> Result<SharePlan, ShareRefusal> {
    use SourceViewOperation as Op;
    if !validate(request) || request.not_after_s <= now_unix_s {
        return Err(ShareRefusal::RequestInvalid);
    }
    if !parent.live_at(now_unix_s) {
        return Err(ShareRefusal::ParentExpired);
    }
    if request.not_after_s > parent.not_after_s() {
        return Err(ShareRefusal::OutlivesParent);
    }
    for path in &request.paths {
        let held = parent
            .grants()
            .iter()
            .any(|grant| grant.path.contains(path));
        if !held || !ceiling.contains(path) {
            return Err(ShareRefusal::WidenResource);
        }
    }
    if parent.depth() == 0
        || request
            .paths
            .iter()
            .any(|path| !parent.permits_scope(Op::Delegate, path))
    {
        return Err(ShareRefusal::DelegationNotPermitted);
    }
    if request.depth >= parent.depth()
        || (request.operations.contains(&Op::Delegate) && request.depth == 0)
    {
        return Err(ShareRefusal::DepthExceeded);
    }
    for operation in &request.operations {
        if !ceiling.operations().contains(operation)
            || request
                .paths
                .iter()
                .any(|path| !parent.permits_scope(*operation, path))
        {
            return Err(ShareRefusal::WidenOperation);
        }
    }
    if !request
        .snapshot
        .within(parent.snapshot(), parent.revision())
    {
        return Err(ShareRefusal::WidenSnapshot);
    }
    let mut grants: Vec<SourceGrant> = request
        .paths
        .iter()
        .map(|path| SourceGrant {
            path: path.clone(),
            operations: request.operations.clone(),
        })
        .collect();
    grants.sort();
    grants.dedup();
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", SHARE_PLAN_SCHEMA.as_bytes());
    update_field(&mut hasher, "issuer", parent.audience().as_bytes());
    update_field(&mut hasher, "recipient", request.recipient.as_bytes());
    update_field(
        &mut hasher,
        "repository",
        parent.repository().as_str().as_bytes(),
    );
    request.snapshot.update(&mut hasher);
    update_external(&mut hasher, "parent_revision", parent.revision());
    update_field(
        &mut hasher,
        "ceiling",
        ceiling.identity().as_str().as_bytes(),
    );
    update_usize(&mut hasher, "grants", grants.len());
    for grant in &grants {
        grant.update(&mut hasher);
    }
    hasher.update(&request.not_after_s.to_le_bytes());
    hasher.update(&[request.depth]);
    update_field(
        &mut hasher,
        "parent_scope",
        parent.identity().as_str().as_bytes(),
    );
    hasher.update(&parent.generation().to_le_bytes());
    hasher.update(&parent.revocation_generation().to_le_bytes());
    Ok(SharePlan {
        issuer: parent.audience().into(),
        recipient: request.recipient.clone(),
        repository: parent.repository().clone(),
        snapshot: request.snapshot.clone(),
        parent_revision: parent.revision().clone(),
        ceiling: ceiling.clone(),
        grants,
        not_after_s: request.not_after_s,
        depth: request.depth,
        parent_scope: parent.identity().clone(),
        generation: parent.generation(),
        revocation_generation: parent.revocation_generation(),
        identity: finish(hasher),
    })
}

#[cfg(test)]
mod tests;
