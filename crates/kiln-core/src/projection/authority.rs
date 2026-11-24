//! Source-view authority vocabulary, `kiln.source-authority-coverage.v1`.
//!
//! Reading source through a view needs no fabricated work or attempt, so the
//! source operations form their own closed vocabulary beside
//! [`ProtectedOperation`]. Candidate submission and publication are not source
//! operations: they reuse [`ProtectedOperation::CandidateSubmit`] and
//! [`ProtectedOperation::PublicationPublish`].
//!
//! Each operation maps one-to-one to a reviewed Basalt ability
//! (`kiln/source/...`). No operation implies another: reading does not imply
//! sharing, editing or publishing. Path scope is framed per component with an
//! explicit subtree marker, so `a/bc` is never covered by a grant for `a/b`.
//!
//! [`SourceAuthority`] mirrors the implementation authority port. An adapter
//! admits one exact [`SourceAuthorityRequest`] and exposes the session's
//! [`EffectiveSourceScope`]; the disclosure core filters a trusted view under
//! that scope. [`LocalSourceAuthority`] is the explicit full-source
//! compatibility profile. Nothing here verifies a token, reads a clock or
//! touches a file.

// The denial is the bounded redacted audit record itself, as in the
// implementation authority port.
#![allow(clippy::result_large_err)]

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::disclosure::OpaqueHandle;
use super::{GLOB_CHARS, MAX_TARGET_PATH_BYTES, update_bool, update_external};
use crate::implementation::authority::{
    AttemptCurrentness, AuthorityProfile, AuthorityProvenance, AuthoritySessionId,
    ProtectedOperation,
};
use crate::monorepo::{finish, update_field, update_usize};
use crate::types::{ContentId, CoreResult, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef};

pub const SOURCE_AUTHORITY_COVERAGE_SCHEMA: &str = "kiln.source-authority-coverage.v1";
pub const SOURCE_AUTHORITY_DECISION_SCHEMA: &str = "kiln.source-authority-decision.v1";
const SOURCE_AUTHORITY_REQUEST_SCHEMA: &str = "kiln.source-authority-request.v1";
const EFFECTIVE_SCOPE_SCHEMA: &str = "kiln.source-effective-scope.v1";
const LOCAL_SOURCE_POLICY_SCHEMA: &str = "kiln.local-source-policy.v1";

/// Path components in one scope.
pub const MAX_PATH_COMPONENTS: usize = 256;
/// Grants in one effective scope or share.
pub const MAX_SOURCE_GRANTS: usize = 4_096;
/// Bytes in a holder, audience or operator identity.
pub const MAX_SOURCE_PRINCIPAL_BYTES: usize = 512;
/// Bytes in a reference name of a live view.
pub const MAX_REF_NAME_BYTES: usize = 256;
/// Delegation depth a scope may carry.
pub const MAX_DELEGATION_DEPTH: u8 = 8;
/// Retained redacted decisions.
pub const MAX_SOURCE_DECISIONS: usize = 256;
/// Live sessions retained by one adapter.
pub const MAX_SOURCE_SESSIONS: usize = 64;
/// Digest width of the payload a request binds.
pub const SOURCE_DIGEST_BYTES: usize = 32;

// ---------------------------------------------------------------------------
// Operations and coverage

/// Closed source-view operations, one-to-one with the Basalt
/// `kiln/source/...` abilities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceViewOperation {
    /// Learn that a path exists: names, kinds and status.
    Discover,
    /// Read the bytes of an entry.
    ReadContent,
    /// Read history and diffs.
    ReadHistory,
    /// Prepare a projected workspace.
    PrepareWorkspace,
    /// Modify existing source through a view.
    ModifySource,
    /// Create source in a declared creation destination.
    CreateSource,
    /// Share attenuated access with another audience.
    Delegate,
    /// Retrieve a restricted result projection.
    RetrieveResult,
}

impl SourceViewOperation {
    /// Every operation in review order.
    pub const ALL: [Self; 8] = [
        Self::Discover,
        Self::ReadContent,
        Self::ReadHistory,
        Self::PrepareWorkspace,
        Self::ModifySource,
        Self::CreateSource,
        Self::Delegate,
        Self::RetrieveResult,
    ];

    /// Stable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::ReadContent => "read_content",
            Self::ReadHistory => "read_history",
            Self::PrepareWorkspace => "prepare_workspace",
            Self::ModifySource => "modify_source",
            Self::CreateSource => "create_source",
            Self::Delegate => "delegate",
            Self::RetrieveResult => "retrieve_result",
        }
    }
}

/// What an operation does once admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceEffect {
    /// Discloses source metadata, bytes or history.
    Disclosure,
    /// Prepares a workspace from admitted input.
    Preparation,
    /// Changes source.
    Mutation,
    /// Derives attenuated authority for another audience.
    Delegation,
    /// Discloses a result projection.
    Result,
}

/// Whether a request must name an attempt and fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptRule {
    /// Human browsing needs no fabricated attempt; one may still be bound.
    Optional,
    /// The operation acts for one exact attempt and fence.
    Required,
}

/// One reviewed coverage entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCoverageEntry {
    pub operation: SourceViewOperation,
    /// Basalt ability, `kiln/source/<name>`.
    pub ability: &'static str,
    pub effect: SourceEffect,
    pub attempt: AttemptRule,
}

// r[impl kiln.source.projection.authority.vocabulary]
/// The closed source-authority coverage map.
pub const SOURCE_COVERAGE: [SourceCoverageEntry; 8] = [
    SourceCoverageEntry {
        operation: SourceViewOperation::Discover,
        ability: "kiln/source/discover",
        effect: SourceEffect::Disclosure,
        attempt: AttemptRule::Optional,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::ReadContent,
        ability: "kiln/source/read",
        effect: SourceEffect::Disclosure,
        attempt: AttemptRule::Optional,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::ReadHistory,
        ability: "kiln/source/history",
        effect: SourceEffect::Disclosure,
        attempt: AttemptRule::Optional,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::PrepareWorkspace,
        ability: "kiln/source/prepare",
        effect: SourceEffect::Preparation,
        attempt: AttemptRule::Optional,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::ModifySource,
        ability: "kiln/source/modify",
        effect: SourceEffect::Mutation,
        attempt: AttemptRule::Required,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::CreateSource,
        ability: "kiln/source/create",
        effect: SourceEffect::Mutation,
        attempt: AttemptRule::Required,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::Delegate,
        ability: "kiln/source/delegate",
        effect: SourceEffect::Delegation,
        attempt: AttemptRule::Optional,
    },
    SourceCoverageEntry {
        operation: SourceViewOperation::RetrieveResult,
        ability: "kiln/source/result",
        effect: SourceEffect::Result,
        attempt: AttemptRule::Optional,
    },
];

/// Protected operations that source views reuse instead of duplicating.
pub const REUSED_PROTECTED_OPERATIONS: [ProtectedOperation; 2] = [
    ProtectedOperation::CandidateSubmit,
    ProtectedOperation::PublicationPublish,
];

/// The reviewed entry of one operation.
#[must_use]
pub fn source_coverage(operation: SourceViewOperation) -> &'static SourceCoverageEntry {
    match SOURCE_COVERAGE
        .iter()
        .find(|entry| entry.operation == operation)
    {
        Some(entry) => entry,
        None => unreachable!("SOURCE_COVERAGE names every operation"),
    }
}

/// The operation a Basalt ability names. An unknown ability maps to none and
/// therefore denies.
#[must_use]
pub fn operation_for_ability(ability: &str) -> Option<SourceViewOperation> {
    SOURCE_COVERAGE
        .iter()
        .find(|entry| entry.ability == ability)
        .map(|entry| entry.operation)
}

/// Check a coverage map: every operation exactly once, unique abilities in the
/// `kiln/source/` namespace, and mutation exactly where an attempt is required.
///
/// # Errors
/// Returns the first inconsistency.
pub fn validate_source_coverage(entries: &[SourceCoverageEntry]) -> CoreResult<()> {
    for operation in SourceViewOperation::ALL {
        if entries.iter().filter(|e| e.operation == operation).count() != 1 {
            return Err(Diagnostic::new(
                "source_coverage_operation",
                operation.as_str(),
                "every source operation must appear exactly once",
            ));
        }
    }
    if entries.len() != SourceViewOperation::ALL.len() {
        return Err(Diagnostic::new(
            "source_coverage_extra",
            "coverage",
            "the coverage map holds an entry for no reviewed operation",
        ));
    }
    let abilities: BTreeSet<&str> = entries.iter().map(|e| e.ability).collect();
    if abilities.len() != entries.len() {
        return Err(Diagnostic::new(
            "source_coverage_ability",
            "ability",
            "every source operation must have its own ability",
        ));
    }
    for entry in entries {
        if !entry.ability.starts_with("kiln/source/") {
            return Err(Diagnostic::new(
                "source_coverage_namespace",
                entry.operation.as_str(),
                "source abilities must stay in the kiln/source namespace",
            ));
        }
        let mutation = entry.effect == SourceEffect::Mutation;
        if mutation != (entry.attempt == AttemptRule::Required) {
            return Err(Diagnostic::new(
                "source_coverage_attempt",
                entry.operation.as_str(),
                "exactly the mutating operations must require an attempt",
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Path scope

/// A repository path scope framed per component. `subtree` marks a directory
/// grant covering everything below it; otherwise exactly one path is named.
/// The repository root is the empty component list with `subtree`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathScope {
    pub components: Vec<String>,
    pub subtree: bool,
}

impl PathScope {
    /// The whole repository.
    #[must_use]
    pub fn root() -> Self {
        Self {
            components: Vec::new(),
            subtree: true,
        }
    }

    /// A scope over a canonical repository path, or `None` for an unsafe one.
    #[must_use]
    pub fn from_path(path: &str, subtree: bool) -> Option<Self> {
        let scope = Self {
            components: if path.is_empty() {
                Vec::new()
            } else {
                path.split('/').map(ToString::to_string).collect()
            },
            subtree,
        };
        scope.validate().ok().map(|()| scope)
    }

    // r[impl kiln.source.projection.disclosure.cairn]
    /// The path authority over one Cairn change, `.cairn/changes/<id>/**`.
    /// Cairn disclosure adds no resource family of its own.
    #[must_use]
    pub fn cairn_change(change: &str) -> Option<Self> {
        if change.is_empty() || change.contains('/') {
            return None;
        }
        Self::from_path(&format!(".cairn/changes/{change}"), true)
    }

    /// Check component safety and bounds.
    ///
    /// # Errors
    /// Returns a diagnostic for an empty, dot, glob, separator or control
    /// component, an exact empty scope, or a bound violation.
    pub fn validate(&self) -> CoreResult<()> {
        if self.components.is_empty() && !self.subtree {
            return Err(Diagnostic::new(
                "source_scope_empty",
                "components",
                "an exact scope must name a path",
            ));
        }
        if self.components.len() > MAX_PATH_COMPONENTS
            || self.components.iter().map(|c| c.len() + 1).sum::<usize>() > MAX_TARGET_PATH_BYTES
        {
            return Err(Diagnostic::new(
                "source_scope_bound",
                "components",
                "the scope exceeds its named path bound",
            ));
        }
        let unsafe_component = |component: &String| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.contains('/')
                || component.contains('\\')
                || component.contains(GLOB_CHARS)
                || component.chars().any(char::is_control)
        };
        if self.components.iter().any(unsafe_component) {
            return Err(Diagnostic::new(
                "source_scope_component",
                "components",
                "scope components must be literal, non-empty path segments",
            ));
        }
        Ok(())
    }

    /// Whether a repository path lies in this scope, compared by component.
    #[must_use]
    pub fn covers_path(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let mut parts = path.split('/');
        for component in &self.components {
            if parts.next() != Some(component.as_str()) {
                return false;
            }
        }
        match parts.next() {
            None => true,
            Some(_) => self.subtree,
        }
    }

    /// Whether every path of `other` lies in this scope.
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        if self.subtree {
            other.components.starts_with(&self.components)
        } else {
            !other.subtree && other.components == self.components
        }
    }

    pub(super) fn update(&self, hasher: &mut blake3::Hasher) {
        update_usize(hasher, "scope.components", self.components.len());
        for component in &self.components {
            update_field(hasher, "scope.component", component.as_bytes());
        }
        update_bool(hasher, "scope.subtree", self.subtree);
    }
}

/// Operations granted over one path scope.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceGrant {
    pub path: PathScope,
    pub operations: BTreeSet<SourceViewOperation>,
}

impl SourceGrant {
    pub(super) fn update(&self, hasher: &mut blake3::Hasher) {
        self.path.update(hasher);
        update_operations(hasher, &self.operations);
    }
}

fn update_operations(hasher: &mut blake3::Hasher, operations: &BTreeSet<SourceViewOperation>) {
    update_usize(hasher, "operations", operations.len());
    for operation in operations {
        update_field(hasher, "operation", operation.as_str().as_bytes());
    }
}

// ---------------------------------------------------------------------------
// Snapshots

/// How a live view may follow its reference. `Rewrite` admits every
/// `FastForward` update and also non-descendant moves.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefEvolution {
    FastForward,
    Rewrite,
}

impl RefEvolution {
    const fn as_str(self) -> &'static str {
        match self {
            Self::FastForward => "fast_forward",
            Self::Rewrite => "rewrite",
        }
    }
}

/// Which snapshots a request or grant names.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum SnapshotRule {
    /// One exact revision.
    Exact { revision: ExternalRef },
    /// A live reference under explicitly admitted evolution semantics.
    Ref {
        name: String,
        evolution: RefEvolution,
    },
}

/// The source owner's current observation for a snapshot rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotObservation {
    /// The exact revision the request would read.
    pub revision: ExternalRef,
    /// The reference that currently resolves to `revision`, if any.
    #[serde(default)]
    pub ref_name: Option<String>,
    /// The revision the live view last admitted, if any.
    #[serde(default)]
    pub previous: Option<ExternalRef>,
    /// Whether `revision` descends from `previous`.
    #[serde(default)]
    pub fast_forward: bool,
}

impl SnapshotRule {
    pub(super) fn validate(&self) -> CoreResult<()> {
        match self {
            Self::Exact { .. } => Ok(()),
            Self::Ref { name, .. } => {
                if valid_principal(name, MAX_REF_NAME_BYTES) {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        "source_snapshot_ref",
                        "snapshot.name",
                        "a reference name must be bounded printable ASCII",
                    ))
                }
            }
        }
    }

    // r[impl kiln.source.projection.authority.port.scenario.evolution]
    /// Whether the observation is admitted: an exact rule only its revision;
    /// a reference only while it resolves to the observed revision and, for a
    /// refresh, only along its admitted evolution.
    #[must_use]
    pub fn admits(&self, observation: &SnapshotObservation) -> bool {
        match self {
            Self::Exact { revision } => &observation.revision == revision,
            Self::Ref { name, evolution } => {
                observation.ref_name.as_deref() == Some(name.as_str())
                    && match &observation.previous {
                        None => true,
                        Some(previous) if previous == &observation.revision => true,
                        Some(_) => *evolution == RefEvolution::Rewrite || observation.fast_forward,
                    }
            }
        }
    }

    /// Whether this rule names no snapshot outside `parent`, where `parent`
    /// was admitted at `parent_revision`.
    #[must_use]
    pub fn within(&self, parent: &Self, parent_revision: &ExternalRef) -> bool {
        match (self, parent) {
            (Self::Exact { revision }, Self::Exact { revision: granted }) => revision == granted,
            (Self::Exact { revision }, Self::Ref { .. }) => revision == parent_revision,
            (
                Self::Ref { name, evolution },
                Self::Ref {
                    name: granted,
                    evolution: granted_evolution,
                },
            ) => name == granted && evolution <= granted_evolution,
            (Self::Ref { .. }, Self::Exact { .. }) => false,
        }
    }

    pub(super) fn update(&self, hasher: &mut blake3::Hasher) {
        match self {
            Self::Exact { revision } => {
                update_field(hasher, "snapshot.rule", b"exact");
                update_external(hasher, "snapshot.revision", revision);
            }
            Self::Ref { name, evolution } => {
                update_field(hasher, "snapshot.rule", b"ref");
                update_field(hasher, "snapshot.name", name.as_bytes());
                update_field(hasher, "snapshot.evolution", evolution.as_str().as_bytes());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Effective scope

/// The context a raw session capability was observed under. A capability
/// confirms an admitted grant only when every key equals the scope's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityContext {
    /// The holder the capability was delegated to.
    pub audience: String,
    pub repository: ContentId,
    pub snapshot: SnapshotRule,
    pub view_binding: ContentId,
    pub generation: u64,
    pub revocation_generation: u64,
}

/// One leaf grant from a session's raw capability set, with the context the
/// adapter observed it under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCapability {
    pub grant: SourceGrant,
    pub context: CapabilityContext,
}

impl SessionCapability {
    /// Raw capabilities observed under one context.
    #[must_use]
    pub fn under(context: &CapabilityContext, grants: &[SourceGrant]) -> Vec<Self> {
        grants
            .iter()
            .map(|grant| Self {
                grant: grant.clone(),
                context: context.clone(),
            })
            .collect()
    }
}

/// Facts an authority adapter read from one admitted session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveScopeFacts {
    /// The holder the session admitted.
    pub audience: String,
    pub repository: ContentId,
    /// The granted snapshot rule.
    pub snapshot: SnapshotRule,
    /// The exact revision this session admitted under `snapshot`.
    pub revision: ExternalRef,
    /// The view binding or ceiling the admissions were made through.
    pub view_binding: ContentId,
    /// Grants the port admitted one by one: the resource and operation of
    /// each admitted request, or a grant individually re-admitted for it.
    pub admitted: Vec<SourceGrant>,
    /// The session's raw effective capability set. It only confirms an
    /// admitted grant, and only when observed for this audience, repository,
    /// snapshot, view binding and both generations; a grant present here but
    /// never admitted, such as a sibling the delegation chain rule did not
    /// check, confers nothing.
    pub capabilities: Vec<SessionCapability>,
    pub not_after_s: u64,
    /// Remaining delegation depth.
    pub depth: u8,
    pub generation: u64,
    pub revocation_generation: u64,
    pub provenance: AuthorityProvenance,
}

impl EffectiveScopeFacts {
    /// The context a confirming capability must have been observed under.
    #[must_use]
    pub fn context(&self) -> CapabilityContext {
        CapabilityContext {
            audience: self.audience.clone(),
            repository: self.repository.clone(),
            snapshot: self.snapshot.clone(),
            view_binding: self.view_binding.clone(),
            generation: self.generation,
            revocation_generation: self.revocation_generation,
        }
    }
}

// r[impl kiln.source.projection.authority.scope]
/// What current authority lets one holder do with one repository snapshot.
///
/// Only an authority adapter builds one, from a verified session; it cannot
/// be deserialized, and its identity binds every field, so a narrowed or
/// revoked scope never equals the broader one it replaces.
///
/// Its grants are only what the port admitted request by request, narrowed
/// to what the session's capability set confirms. A raw capability set is
/// never authority on its own: a leaf grant the delegation chain rule did not
/// check for that exact resource must be admitted individually before it
/// discloses anything. Scopes of several admissions combine with
/// [`EffectiveSourceScope::join`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveSourceScope {
    audience: String,
    repository: ContentId,
    snapshot: SnapshotRule,
    revision: ExternalRef,
    view_binding: ContentId,
    grants: Vec<SourceGrant>,
    not_after_s: u64,
    depth: u8,
    generation: u64,
    revocation_generation: u64,
    provenance: AuthorityProvenance,
    identity: ContentId,
}

impl EffectiveSourceScope {
    /// Admit session facts as an effective scope. Grants are put in canonical
    /// order; a grant without operations is dropped.
    ///
    /// # Errors
    /// Returns a diagnostic for an invalid audience, path, snapshot or bound.
    pub fn new(facts: EffectiveScopeFacts) -> CoreResult<Self> {
        if !valid_principal(&facts.audience, MAX_SOURCE_PRINCIPAL_BYTES) {
            return Err(Diagnostic::new(
                "source_scope_audience",
                "audience",
                "the audience must be bounded printable ASCII",
            ));
        }
        facts.snapshot.validate()?;
        if facts.admitted.len() > MAX_SOURCE_GRANTS
            || facts.capabilities.len() > MAX_SOURCE_GRANTS
            || facts.depth > MAX_DELEGATION_DEPTH
        {
            return Err(Diagnostic::new(
                "source_scope_bound",
                "grants",
                "the scope exceeds its named grant or depth bound",
            ));
        }
        for grant in facts.admitted.iter().chain(
            facts
                .capabilities
                .iter()
                .map(|capability| &capability.grant),
        ) {
            grant.path.validate()?;
        }
        // Effective grants are the admitted ones, each narrowed to the
        // operations a capability observed under this exact context confirms
        // over its whole path. Capabilities of another audience, repository,
        // snapshot, view binding or generation confirm nothing.
        let context = facts.context();
        let confirming: Vec<&SourceGrant> = facts
            .capabilities
            .iter()
            .filter(|capability| capability.context == context)
            .map(|capability| &capability.grant)
            .collect();
        let mut grants = Vec::with_capacity(facts.admitted.len());
        for admitted in facts.admitted {
            let operations: BTreeSet<SourceViewOperation> = admitted
                .operations
                .iter()
                .copied()
                .filter(|operation| {
                    confirming.iter().any(|capability| {
                        capability.operations.contains(operation)
                            && capability.path.contains(&admitted.path)
                    })
                })
                .collect();
            if !operations.is_empty() {
                grants.push(SourceGrant {
                    path: admitted.path,
                    operations,
                });
            }
        }
        grants.sort();
        grants.dedup();
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", EFFECTIVE_SCOPE_SCHEMA.as_bytes());
        update_field(&mut hasher, "audience", facts.audience.as_bytes());
        update_field(
            &mut hasher,
            "repository",
            facts.repository.as_str().as_bytes(),
        );
        facts.snapshot.update(&mut hasher);
        update_external(&mut hasher, "revision", &facts.revision);
        update_field(
            &mut hasher,
            "view_binding",
            facts.view_binding.as_str().as_bytes(),
        );
        update_usize(&mut hasher, "grants", grants.len());
        for grant in &grants {
            grant.update(&mut hasher);
        }
        hasher.update(&facts.not_after_s.to_le_bytes());
        hasher.update(&[facts.depth]);
        hasher.update(&facts.generation.to_le_bytes());
        hasher.update(&facts.revocation_generation.to_le_bytes());
        update_field(
            &mut hasher,
            "provenance",
            facts.provenance.as_str().as_bytes(),
        );
        Ok(Self {
            audience: facts.audience,
            repository: facts.repository,
            snapshot: facts.snapshot,
            revision: facts.revision,
            view_binding: facts.view_binding,
            grants,
            not_after_s: facts.not_after_s,
            depth: facts.depth,
            generation: facts.generation,
            revocation_generation: facts.revocation_generation,
            provenance: facts.provenance,
            identity: finish(hasher),
        })
    }

    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
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
    pub fn revision(&self) -> &ExternalRef {
        &self.revision
    }
    #[must_use]
    pub fn view_binding(&self) -> &ContentId {
        &self.view_binding
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
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn revocation_generation(&self) -> u64 {
        self.revocation_generation
    }
    #[must_use]
    pub fn provenance(&self) -> AuthorityProvenance {
        self.provenance
    }
    /// BLAKE3 identity over every field.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    /// Whether the scope is still live at `now_unix_s`.
    #[must_use]
    pub fn live_at(&self, now_unix_s: u64) -> bool {
        now_unix_s < self.not_after_s
    }

    /// Whether one grant covering `path` names `operation`. Operations never
    /// imply each other.
    #[must_use]
    pub fn permits(&self, operation: SourceViewOperation, path: &str) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.operations.contains(&operation) && grant.path.covers_path(path))
    }

    /// Whether one grant containing all of `scope` names `operation`.
    #[must_use]
    pub fn permits_scope(&self, operation: SourceViewOperation, scope: &PathScope) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.operations.contains(&operation) && grant.path.contains(scope))
    }

    /// Whether any grant names `operation` at all.
    #[must_use]
    pub fn names(&self, operation: SourceViewOperation) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.operations.contains(&operation))
    }

    /// Combine scopes of individually admitted requests by one holder over
    /// one repository snapshot and generation. The result lives only as long
    /// as the shortest, delegates only as deep as the shallowest, and grants
    /// the union of what each admission granted.
    ///
    /// # Errors
    /// Returns a diagnostic for no scopes, or scopes of different holders,
    /// repositories, snapshots, generations or provenance.
    pub fn join(scopes: &[Self]) -> CoreResult<Self> {
        let Some(first) = scopes.first() else {
            return Err(Diagnostic::new(
                "source_scope_join",
                "scopes",
                "joining needs at least one admitted scope",
            ));
        };
        let compatible = scopes.iter().all(|scope| {
            scope.audience == first.audience
                && scope.repository == first.repository
                && scope.snapshot == first.snapshot
                && scope.revision == first.revision
                && scope.view_binding == first.view_binding
                && scope.generation == first.generation
                && scope.revocation_generation == first.revocation_generation
                && scope.provenance == first.provenance
        });
        if !compatible {
            return Err(Diagnostic::new(
                "source_scope_join",
                "scopes",
                "joined scopes must share holder, source, generations and provenance",
            ));
        }
        let mut merged: BTreeMap<PathScope, BTreeSet<SourceViewOperation>> = BTreeMap::new();
        for scope in scopes {
            for grant in &scope.grants {
                merged
                    .entry(grant.path.clone())
                    .or_default()
                    .extend(grant.operations.iter().copied());
            }
        }
        let grants: Vec<SourceGrant> = merged
            .into_iter()
            .filter(|(_, operations)| !operations.is_empty())
            .map(|(path, operations)| SourceGrant { path, operations })
            .collect();
        let mut facts = EffectiveScopeFacts {
            audience: first.audience.clone(),
            repository: first.repository.clone(),
            snapshot: first.snapshot.clone(),
            revision: first.revision.clone(),
            view_binding: first.view_binding.clone(),
            admitted: grants.clone(),
            capabilities: Vec::new(),
            not_after_s: scopes.iter().map(|s| s.not_after_s).min().unwrap_or(0),
            depth: scopes.iter().map(|s| s.depth).min().unwrap_or(0),
            generation: first.generation,
            revocation_generation: first.revocation_generation,
            provenance: first.provenance,
        };
        // Each joined grant was confirmed under this same context already.
        facts.capabilities = SessionCapability::under(&facts.context(), &grants);
        Self::new(facts)
    }
}

// ---------------------------------------------------------------------------
// Requests and decisions

/// The attempt a mutating request acts for.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptFence {
    pub attempt: ContentId,
    pub fence: u64,
}

/// The resource a request names. A handle is only a view reference: the port
/// denies it, so the shell must resolve it through the holder's audience view
/// and request the origin's components.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum OriginScope {
    Components { path: PathScope },
    Handle { handle: OpaqueHandle },
}

// r[impl kiln.source.projection.authority.request]
/// One exact source-view request built from trusted owner facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAuthorityRequest {
    pub operation: SourceViewOperation,
    pub repository: ContentId,
    pub snapshot: SnapshotRule,
    /// The admitted view binding the request is made through, or the view
    /// ceiling before a binding exists.
    pub view_binding: ContentId,
    pub scope: OriginScope,
    /// Digest of the exact payload the effect intends.
    pub request_digest: [u8; SOURCE_DIGEST_BYTES],
    #[serde(default)]
    pub attempt: Option<AttemptFence>,
}

impl SourceAuthorityRequest {
    /// Validate bounds and the operation's attempt rule.
    ///
    /// # Errors
    /// Returns a diagnostic for an invalid path or reference name, a missing
    /// attempt on a mutating operation, or a zero fence.
    pub fn validate(&self) -> CoreResult<()> {
        self.snapshot.validate()?;
        if let OriginScope::Components { path } = &self.scope {
            path.validate()?;
        }
        let required = source_coverage(self.operation).attempt == AttemptRule::Required;
        match &self.attempt {
            None if required => Err(Diagnostic::new(
                "source_request_attempt",
                "attempt",
                "a mutating source operation must name its attempt and fence",
            )),
            Some(attempt) if attempt.fence == 0 => Err(Diagnostic::new(
                "source_request_fence",
                "attempt.fence",
                "a bound attempt needs a non-zero fence",
            )),
            _ => Ok(()),
        }
    }

    /// The one grant an admission of this request confers: its operation
    /// over its own path. A handle confers none.
    #[must_use]
    pub fn requested_grant(&self) -> Option<SourceGrant> {
        match &self.scope {
            OriginScope::Components { path } => Some(SourceGrant {
                path: path.clone(),
                operations: [self.operation].into_iter().collect(),
            }),
            OriginScope::Handle { .. } => None,
        }
    }

    /// Canonical BLAKE3 hex digest over every bound field.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        update_field(
            &mut hasher,
            "schema",
            SOURCE_AUTHORITY_REQUEST_SCHEMA.as_bytes(),
        );
        update_field(
            &mut hasher,
            "ability",
            source_coverage(self.operation).ability.as_bytes(),
        );
        update_field(
            &mut hasher,
            "repository",
            self.repository.as_str().as_bytes(),
        );
        self.snapshot.update(&mut hasher);
        update_field(
            &mut hasher,
            "view_binding",
            self.view_binding.as_str().as_bytes(),
        );
        match &self.scope {
            OriginScope::Components { path } => {
                update_field(&mut hasher, "origin", b"components");
                path.update(&mut hasher);
            }
            OriginScope::Handle { handle } => {
                update_field(&mut hasher, "origin", b"handle");
                update_field(&mut hasher, "handle", handle.as_bytes());
            }
        }
        update_field(&mut hasher, "request_digest", &self.request_digest);
        match &self.attempt {
            None => update_field(&mut hasher, "attempt", b""),
            Some(attempt) => {
                update_field(&mut hasher, "attempt", attempt.attempt.as_str().as_bytes());
                hasher.update(&attempt.fence.to_le_bytes());
            }
        }
        hasher.finalize().to_hex().to_string()
    }
}

/// Current facts the shell supplies at admission and every revalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCurrentness {
    /// The deployment's current authority generation.
    pub generation: u64,
    /// The current revocation-view generation.
    pub revocation_generation: u64,
    pub snapshot: SnapshotObservation,
    /// Currentness of the bound attempt; ignored when none is bound.
    pub attempt: AttemptCurrentness,
}

/// Closed source decision reason. `Allowed` is the only admitting reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceDenialReason {
    Allowed,
    /// No authority profile was selected.
    MissingEvidence,
    /// The selected profile cannot verify this evidence.
    VerifierUnavailable,
    /// The credential was malformed, unknown or denied upstream.
    CredentialDenied,
    /// The local operator differs from the selected one.
    PrincipalMismatch,
    RequestInvalid,
    /// The bound attempt is not current.
    AttemptNotCurrent,
    /// No grant names the operation over the resource.
    OperationNotGranted,
    /// No grant covers the resource.
    ResourceOutOfScope,
    /// The repository is outside the grant.
    RepositoryOutOfScope,
    /// The snapshot rule does not admit the observed revision.
    SnapshotNotAdmitted,
    /// A view handle was presented as a resource.
    HandleConfersNothing,
    /// The grant expired.
    Expired,
    /// The authority generation changed.
    StaleGeneration,
    /// The revocation view changed or revoked the grant.
    Revoked,
    SessionNotFound,
    /// A bound field differs from the admitted session.
    BindingMismatch,
}

impl SourceDenialReason {
    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::MissingEvidence => "missing-evidence",
            Self::VerifierUnavailable => "verifier-unavailable",
            Self::CredentialDenied => "credential-denied",
            Self::PrincipalMismatch => "principal-mismatch",
            Self::RequestInvalid => "request-invalid",
            Self::AttemptNotCurrent => "attempt-not-current",
            Self::OperationNotGranted => "operation-not-granted",
            Self::ResourceOutOfScope => "resource-out-of-scope",
            Self::RepositoryOutOfScope => "repository-out-of-scope",
            Self::SnapshotNotAdmitted => "snapshot-not-admitted",
            Self::HandleConfersNothing => "handle-confers-nothing",
            Self::Expired => "expired",
            Self::StaleGeneration => "stale-generation",
            Self::Revoked => "revoked",
            Self::SessionNotFound => "session-not-found",
            Self::BindingMismatch => "binding-mismatch",
        }
    }
}

/// Bounded, redacted source-authority decision. It holds no capability
/// material and cannot authorize anything.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAuthorityDecision {
    pub schema: String,
    pub allowed: bool,
    pub operation: SourceViewOperation,
    pub ability: String,
    pub provenance: AuthorityProvenance,
    pub principal: String,
    pub generation: u64,
    pub revocation_generation: u64,
    pub policy_ref: String,
    pub request_identity: String,
    /// Identity of the admitted effective scope, present only when allowed.
    pub scope_identity: Option<ContentId>,
    pub session: Option<AuthoritySessionId>,
    pub reason: SourceDenialReason,
}

impl SourceAuthorityDecision {
    /// A denial for one request, without principal or credential material.
    #[must_use]
    pub fn deny(request: &SourceAuthorityRequest, reason: SourceDenialReason) -> Self {
        Self {
            schema: SOURCE_AUTHORITY_DECISION_SCHEMA.into(),
            allowed: false,
            operation: request.operation,
            ability: source_coverage(request.operation).ability.into(),
            provenance: AuthorityProvenance::None,
            principal: String::new(),
            generation: 0,
            revocation_generation: 0,
            policy_ref: String::new(),
            request_identity: request.digest_hex(),
            scope_identity: None,
            session: None,
            reason,
        }
    }

    /// An allowed decision for one request, session and scope.
    #[must_use]
    pub fn allowed(
        request: &SourceAuthorityRequest,
        session: AuthoritySessionId,
        scope: &EffectiveSourceScope,
        policy_ref: String,
    ) -> Self {
        Self {
            schema: SOURCE_AUTHORITY_DECISION_SCHEMA.into(),
            allowed: true,
            operation: request.operation,
            ability: source_coverage(request.operation).ability.into(),
            provenance: scope.provenance(),
            principal: scope.audience().into(),
            generation: scope.generation(),
            revocation_generation: scope.revocation_generation(),
            policy_ref,
            request_identity: request.digest_hex(),
            scope_identity: Some(scope.identity().clone()),
            session: Some(session),
            reason: SourceDenialReason::Allowed,
        }
    }
}

// r[impl kiln.source.projection.authority.port]
/// Provider-neutral source-view authority port, mirroring the protected
/// implementation port. The adapter owns live sessions; the shell owns the
/// current snapshot, generation and attempt facts.
pub trait SourceAuthority {
    /// Verify one exact request and retain a live session when allowed.
    ///
    /// # Errors
    /// Returns the redacted denial before any disclosure or effect.
    fn admit(
        &mut self,
        request: &SourceAuthorityRequest,
        profile: &AuthorityProfile,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision>;

    /// Recheck one live session against current facts before each use.
    ///
    /// # Errors
    /// Returns the redacted denial; the session is discarded.
    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &SourceAuthorityRequest,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<(), SourceAuthorityDecision>;

    /// Discard one live session.
    fn close(&mut self, session: AuthoritySessionId);

    /// The effective scope of one live session: only the admitted request's
    /// resource and operation, confirmed by the session's capability set.
    /// An adapter MUST NOT widen it with other grants the session's raw
    /// capability set reports; each must be admitted on its own.
    fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope>;

    /// Bounded redacted decision log.
    fn decisions(&self) -> &[SourceAuthorityDecision];
}

/// Checks every port shares once the principal is known: request shape,
/// attempt currentness, repository, snapshot, handle refusal and coverage.
///
/// # Errors
/// Returns the first denial reason.
pub fn check_request(
    scope: &EffectiveSourceScope,
    request: &SourceAuthorityRequest,
    current: &SourceCurrentness,
    now_unix_s: u64,
) -> Result<(), SourceDenialReason> {
    if request.validate().is_err() {
        return Err(SourceDenialReason::RequestInvalid);
    }
    if request.attempt.is_some() && current.attempt != AttemptCurrentness::Current {
        return Err(SourceDenialReason::AttemptNotCurrent);
    }
    if !scope.live_at(now_unix_s) {
        return Err(SourceDenialReason::Expired);
    }
    if scope.generation() != current.generation {
        return Err(SourceDenialReason::StaleGeneration);
    }
    if scope.revocation_generation() != current.revocation_generation {
        return Err(SourceDenialReason::Revoked);
    }
    if &request.repository != scope.repository() {
        return Err(SourceDenialReason::RepositoryOutOfScope);
    }
    // A session binds one exact revision: a moved reference needs a fresh
    // admission under the rule's evolution, never a silent refresh.
    if &current.snapshot.revision != scope.revision()
        || !request.snapshot.admits(&current.snapshot)
        || !request.snapshot.within(scope.snapshot(), scope.revision())
        || !scope.snapshot().admits(&current.snapshot)
    {
        return Err(SourceDenialReason::SnapshotNotAdmitted);
    }
    let path = match &request.scope {
        OriginScope::Handle { .. } => return Err(SourceDenialReason::HandleConfersNothing),
        OriginScope::Components { path } => path,
    };
    if scope.permits_scope(request.operation, path) {
        Ok(())
    } else if scope.grants().iter().any(|grant| grant.path.contains(path)) {
        Err(SourceDenialReason::OperationNotGranted)
    } else {
        Err(SourceDenialReason::ResourceOutOfScope)
    }
}

// ---------------------------------------------------------------------------
// Local-operator compatibility profile

/// The explicit full-source grant of the local-operator profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSourceGrant {
    pub repositories: BTreeSet<ContentId>,
    pub operations: BTreeSet<SourceViewOperation>,
}

struct LocalSourceSession {
    request_identity: String,
    scope: EffectiveSourceScope,
}

// r[impl kiln.source.projection.authority.compatibility]
/// Explicit local-operator full-source compatibility profile.
///
/// It admits only the selected operator, over the repositories and
/// operations its grant names. There is no role path: a missing profile, a
/// verified credential and another operator all deny. Delegation needs a
/// verified issuer, so this profile refuses a grant naming it.
pub struct LocalSourceAuthority {
    operator: String,
    grant: LocalSourceGrant,
    sessions: BTreeMap<u64, LocalSourceSession>,
    next_session: u64,
    decisions: Vec<SourceAuthorityDecision>,
}

impl core::fmt::Debug for LocalSourceAuthority {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LocalSourceAuthority")
            .field("operator", &self.operator)
            .field("repositories", &self.grant.repositories.len())
            .field("operations", &self.grant.operations)
            .field("sessions", &self.sessions.len())
            .finish_non_exhaustive()
    }
}

impl LocalSourceAuthority {
    /// Select the local-operator profile with an explicit full-source grant.
    ///
    /// # Errors
    /// Returns a diagnostic for an invalid operator, an empty grant or a grant
    /// naming delegation.
    pub fn new(operator: impl Into<String>, grant: LocalSourceGrant) -> CoreResult<Self> {
        let operator = operator.into();
        if !valid_principal(&operator, MAX_SOURCE_PRINCIPAL_BYTES) {
            return Err(Diagnostic::new(
                "source_local_operator",
                "operator",
                "the local operator must be bounded printable ASCII",
            ));
        }
        if grant.repositories.is_empty() || grant.operations.is_empty() {
            return Err(Diagnostic::new(
                "source_local_grant",
                "grant",
                "the full-source grant must name repositories and operations",
            ));
        }
        if grant.operations.contains(&SourceViewOperation::Delegate) {
            return Err(Diagnostic::new(
                "source_local_delegate",
                "grant.operations",
                "the local-operator profile cannot delegate",
            ));
        }
        Ok(Self {
            operator,
            grant,
            sessions: BTreeMap::new(),
            next_session: 1,
            decisions: Vec::new(),
        })
    }

    fn policy_reference(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", LOCAL_SOURCE_POLICY_SCHEMA.as_bytes());
        update_field(&mut hasher, "operator", self.operator.as_bytes());
        for repository in &self.grant.repositories {
            update_field(&mut hasher, "repository", repository.as_str().as_bytes());
        }
        update_operations(&mut hasher, &self.grant.operations);
        format!("local-source:{}", hasher.finalize().to_hex())
    }

    fn record(&mut self, decision: SourceAuthorityDecision) -> SourceAuthorityDecision {
        if self.decisions.len() >= MAX_SOURCE_DECISIONS {
            self.decisions.remove(0);
        }
        self.decisions.push(decision.clone());
        decision
    }

    fn deny(
        &mut self,
        request: &SourceAuthorityRequest,
        reason: SourceDenialReason,
    ) -> SourceAuthorityDecision {
        self.record(SourceAuthorityDecision::deny(request, reason))
    }

    /// A scope over the configured full-source grant, holding `admitted`.
    fn scope_for(
        &self,
        request: &SourceAuthorityRequest,
        current: &SourceCurrentness,
        admitted: Vec<SourceGrant>,
    ) -> CoreResult<EffectiveSourceScope> {
        let mut facts = EffectiveScopeFacts {
            audience: self.operator.clone(),
            repository: request.repository.clone(),
            snapshot: request.snapshot.clone(),
            revision: current.snapshot.revision.clone(),
            view_binding: request.view_binding.clone(),
            admitted,
            capabilities: Vec::new(),
            not_after_s: u64::MAX,
            depth: 0,
            generation: current.generation,
            revocation_generation: current.revocation_generation,
            provenance: AuthorityProvenance::LocalOperator,
        };
        // The configured grant is current for this exact request context.
        facts.capabilities = SessionCapability::under(&facts.context(), &[self.configured()]);
        EffectiveSourceScope::new(facts)
    }

    fn configured(&self) -> SourceGrant {
        SourceGrant {
            path: PathScope::root(),
            operations: self.grant.operations.clone(),
        }
    }
}

impl SourceAuthority for LocalSourceAuthority {
    fn admit(
        &mut self,
        request: &SourceAuthorityRequest,
        profile: &AuthorityProfile,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision> {
        let operator = match profile {
            AuthorityProfile::Missing => {
                return Err(self.deny(request, SourceDenialReason::MissingEvidence));
            }
            AuthorityProfile::VerifiedUcan(_) => {
                return Err(self.deny(request, SourceDenialReason::VerifierUnavailable));
            }
            AuthorityProfile::LocalOperator { operator } => operator,
        };
        if operator != &self.operator {
            return Err(self.deny(request, SourceDenialReason::PrincipalMismatch));
        }
        if !self.grant.repositories.contains(&request.repository) {
            return Err(self.deny(request, SourceDenialReason::RepositoryOutOfScope));
        }
        // Check against the configured grant, then keep only the admitted
        // request in the session: the full-source grant is not disclosed
        // wholesale through one admission.
        let Ok(configured) = self.scope_for(request, current, alloc::vec![self.configured()])
        else {
            return Err(self.deny(request, SourceDenialReason::RequestInvalid));
        };
        if let Err(reason) = check_request(&configured, request, current, now_unix_s) {
            return Err(self.deny(request, reason));
        }
        let admitted = request.requested_grant().into_iter().collect();
        let Ok(scope) = self.scope_for(request, current, admitted) else {
            return Err(self.deny(request, SourceDenialReason::RequestInvalid));
        };
        if self.sessions.len() >= MAX_SOURCE_SESSIONS
            && let Some(oldest) = self.sessions.keys().next().copied()
        {
            self.sessions.remove(&oldest);
        }
        let id = self.next_session;
        self.next_session = self.next_session.saturating_add(1);
        let decision = SourceAuthorityDecision::allowed(
            request,
            AuthoritySessionId(id),
            &scope,
            self.policy_reference(),
        );
        self.sessions.insert(
            id,
            LocalSourceSession {
                request_identity: request.digest_hex(),
                scope,
            },
        );
        Ok(self.record(decision))
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &SourceAuthorityRequest,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<(), SourceAuthorityDecision> {
        let Some(live) = self.sessions.get(&session.0) else {
            return Err(self.deny(request, SourceDenialReason::SessionNotFound));
        };
        let outcome = if live.request_identity == request.digest_hex() {
            check_request(&live.scope, request, current, now_unix_s).map(|()| live.scope.clone())
        } else {
            Err(SourceDenialReason::BindingMismatch)
        };
        match outcome {
            Ok(scope) => {
                let decision = SourceAuthorityDecision::allowed(
                    request,
                    session,
                    &scope,
                    self.policy_reference(),
                );
                self.record(decision);
                Ok(())
            }
            Err(reason) => {
                self.sessions.remove(&session.0);
                Err(self.deny(request, reason))
            }
        }
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.sessions.remove(&session.0);
    }

    fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope> {
        self.sessions.get(&session.0).map(|live| &live.scope)
    }

    fn decisions(&self) -> &[SourceAuthorityDecision] {
        &self.decisions
    }
}

/// Bounded printable ASCII without surrounding space.
pub(super) fn valid_principal(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

/// Bounded identifier text, as for other projection identifiers.
pub(super) fn valid_identifier(value: &str) -> bool {
    valid_principal(value, DEFAULT_MAX_ID_BYTES)
}

#[cfg(test)]
mod tests;
