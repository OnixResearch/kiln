//! Closed `kiln://source/v1/` operations over projected source views.
//!
//! Each operation has one reviewed contract and one ability; no operation implies
//! another. Path scope is framed per component with a terminal marker, so an exact
//! entry, a subtree and a sibling whose name shares a byte prefix never alias.

use super::framing;
use super::{DIGEST_BYTES, Denied, Grant};
use crate::ucan_enforcement::EnforcementAction;

/// Closed Kiln source-view operations. Unknown operations do not deserialize and have
/// no contract, so they deny.
// r[impl basalt.kiln_authority.source_actions]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SourceOperation {
    /// List or locate entries inside an authorized scope.
    #[serde(rename = "discover")]
    Discover,
    /// Read the content of one exact entry.
    #[serde(rename = "read")]
    ReadContent,
    /// Read the history of an entry or subtree.
    #[serde(rename = "history")]
    ReadHistory,
    /// Prepare a projected workspace for an entry or subtree.
    #[serde(rename = "prepare")]
    PrepareWorkspace,
    /// Modify one exact existing entry.
    #[serde(rename = "modify")]
    ModifySource,
    /// Create one exact new entry.
    #[serde(rename = "create")]
    CreateSource,
    /// Share an entry or subtree onward; also the explicit chain-delegation ability.
    #[serde(rename = "delegate")]
    Delegate,
    /// Retrieve a result produced for an entry or subtree.
    #[serde(rename = "result")]
    RetrieveResult,
}

impl SourceOperation {
    /// Return the distinct reviewed action; no action implies another grant.
    #[must_use]
    pub const fn action(self) -> EnforcementAction {
        match self {
            Self::Discover => EnforcementAction::KilnSourceDiscover,
            Self::ReadContent => EnforcementAction::KilnSourceRead,
            Self::ReadHistory => EnforcementAction::KilnSourceHistory,
            Self::PrepareWorkspace => EnforcementAction::KilnSourcePrepare,
            Self::ModifySource => EnforcementAction::KilnSourceModify,
            Self::CreateSource => EnforcementAction::KilnSourceCreate,
            Self::Delegate => EnforcementAction::KilnSourceDelegate,
            Self::RetrieveResult => EnforcementAction::KilnSourceResult,
        }
    }

    /// Return whether this operation acts on exactly one entry, never a subtree.
    ///
    /// Content reads and mutations name one destination after normalization; the
    /// other operations accept either an entry or a subtree.
    #[must_use]
    pub const fn requires_entry(self) -> bool {
        match self {
            Self::ReadContent | Self::ModifySource | Self::CreateSource => true,
            Self::Discover
            | Self::ReadHistory
            | Self::PrepareWorkspace
            | Self::Delegate
            | Self::RetrieveResult => false,
        }
    }
}

/// Whether a path names one exact entry or the subtree rooted at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathScope {
    /// Exactly this entry: terminal marker `e/`.
    Entry,
    /// This node and everything below it: terminal marker `s/`.
    Subtree,
}

/// The current attempt a source operation acts for, framed before the path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAttempt {
    pub attempt: String,
    pub fence: u64,
}

/// Untrusted source request, not a grant. `subject` is the signing UCAN holder DID and
/// is bound by token audience equality and the holder signature, not by the resource.
#[derive(Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRequest {
    pub operation: SourceOperation,
    pub subject: String,
    pub audience: String,
    pub generation: u64,
    pub repository: String,
    pub attempt: Option<SourceAttempt>,
    /// Normalized path components; an empty path with `Subtree` is the repository root.
    pub path: Vec<String>,
    pub scope: PathScope,
    pub snapshot: String,
    pub view: String,
    pub request_digest: [u8; DIGEST_BYTES],
}

/// Typed grant scope for issuing a `kiln://source/v1/` capability.
///
/// Without `terminal` the prefix covers the whole subtree at `path` (every entry and
/// subtree request below it). `Some(Entry)` covers exactly one entry; `Some(Subtree)`
/// covers only subtree-scoped requests at that node.
pub struct SourceScope<'a> {
    pub audience: &'a str,
    pub generation: u64,
    pub repository: &'a str,
    pub attempt: Option<&'a SourceAttempt>,
    pub path: &'a [String],
    pub terminal: Option<PathScope>,
}

impl SourceScope<'_> {
    /// Frame the exact resource prefix a grant or ceiling must carry.
    ///
    /// # Errors
    /// Returns `Denied` for a zero generation, an inadmissible field or component, an
    /// entry marker on the repository root, or an oversized resource.
    // r[impl basalt.kiln_authority.source_path]
    pub fn prefix(&self) -> Result<String, Denied> {
        framing::source_prefix(
            self.audience,
            self.generation,
            self.repository,
            self.attempt,
            self.path,
            self.terminal,
        )
    }
}

/// Non-deserializable, non-cloneable source authority minted only after final replay.
pub struct SourceSession {
    pub(super) request: SourceRequest,
    pub(super) grant: Grant,
}

impl SourceSession {
    /// Return the exact signed request observation, not a reusable grant DTO.
    #[must_use]
    pub const fn request(&self) -> &SourceRequest {
        &self.request
    }
    /// Leaf grants that cover the admitted request, plus other leaf grants under this
    /// request's `kiln://source/v1/<audience>/<generation>/<repository>/<context>`
    /// prefix that pass the chain rule with their own resource. Ability, view ceiling and
    /// snapshot checks stay the consumer's job. These observations are not transferable
    /// authority; do not log payloads.
    #[must_use]
    pub const fn effective_capabilities(&self) -> &ucan::CapabilitySet {
        &self.grant.effective
    }
    /// Return the effective signed deadline.
    #[must_use]
    pub const fn expires_unix_s(&self) -> u64 {
        self.grant.expires_unix_s
    }
    /// Return the reviewed policy artifact reference.
    #[must_use]
    pub fn policy_identity(&self) -> &str {
        &self.grant.policy_ref
    }
    /// Return the real successful Basalt admission receipt reference.
    #[must_use]
    pub fn decision_id(&self) -> &str {
        &self.grant.decision_id
    }
    /// Check the exact operation, path, scope and digest before any effect.
    ///
    /// # Errors
    /// Returns `Denied` for any operation, path, scope or digest mismatch.
    pub fn require(
        &self,
        operation: SourceOperation,
        path: &[String],
        scope: PathScope,
        digest: [u8; DIGEST_BYTES],
    ) -> Result<(), Denied> {
        if self.request.operation == operation
            && self.request.path == path
            && self.request.scope == scope
            && self.request.request_digest == digest
        {
            Ok(())
        } else {
            Err(Denied)
        }
    }
}

impl std::fmt::Debug for SourceSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KilnSourceSession").finish_non_exhaustive()
    }
}
