//! Audience-restricted views over a trusted projection.
//!
//! The trusted resolver's view holds a manifest with every origin; a holder
//! never receives it. A shell enumerates the source over one holder's
//! [`ScopedSelection`] (the ceiling intersected with what its current grants
//! let it discover or read) and wraps the result in a [`ScopedEnumeration`],
//! so the size and truncation of the enumeration never depend on entries the
//! holder may not see. [`restrict`] then filters it under the holder's
//! [`EffectiveSourceScope`] and a frozen [`ViewCeiling`] *before* anything is
//! counted, paged, grouped or reported, so the effective view is
//! `selection ∩ ceiling ∩ snapshot ∩ current grant`.
//!
//! Handles and cursors are BLAKE3 hashes keyed by a [`HandleKey`] the shell
//! holds, over a keying identity of holder-visible data only. Knowing a
//! native digest, an equal-byte file or an alias forges nothing, a handle
//! from another view resolves nothing, and every use rechecks the
//! [`ViewBinding`] against the current scope. Missing and forbidden objects
//! share one [`PublicUnavailable`] shape.
//!
//! Cairn context is disclosed through `.cairn/changes/<id>/**` path authority.
//! A [`RestrictedContext`] is a presentation, never a complete lifecycle
//! snapshot:
//!
//! ```compile_fail,E0308
//! use kiln_core::projection::disclosure::{CairnObservation, RestrictedContext};
//!
//! fn submit(_: &CairnObservation) {}
//!
//! fn misuse(context: &RestrictedContext) {
//!     submit(context);
//! }
//! ```
//!
//! ```compile_fail,E0277
//! use kiln_core::projection::disclosure::{CairnObservation, RestrictedContext};
//!
//! fn widen(context: RestrictedContext) -> CairnObservation {
//!     context.into()
//! }
//! ```
//!
//! Restricted views, contexts and scopes cannot be deserialized; they come
//! only from current authority:
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<kiln_core::projection::disclosure::AudienceView>("{}");
//! ```
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<kiln_core::projection::disclosure::RestrictedContext>("{}");
//! ```
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<kiln_core::projection::authority::EffectiveSourceScope>("{}");
//! ```

// r[verify kiln.source.projection.disclosure.cairn.scenario.not_complete]
// r[verify kiln.source.projection.disclosure.restricted.scenario.unforgeable]
// The compile-fail examples above are the evidence for both scenarios.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::authority::{EffectiveSourceScope, PathScope, SourceViewOperation, valid_identifier};
use super::{
    AdmittedDefinition, AdmittedPartialView, Coverage, DependencyStatus, EntryKind, EntryOrigin,
    GLOB_CHARS, ManifestEntry, ProjectionFacts, ProjectionProfile, ResolvedProjectionManifest,
    update_bool, update_external,
};
use crate::monorepo::{finish, update_field, update_usize};
use crate::paths;
use crate::types::{ContentId, Diagnostic, ExternalRef};

pub const VIEW_BINDING_SCHEMA: &str = "kiln.source-view-binding.v1";
pub const VIEW_PRESENTATION_SCHEMA: &str = "kiln.source-view-presentation.v1";
pub const RESTRICTED_CONTEXT_SCHEMA: &str = "kiln.restricted-context.v1";
const VIEW_CEILING_SCHEMA: &str = "kiln.source-view-ceiling.v1";
const SCOPED_SELECTION_SCHEMA: &str = "kiln.source-view-scoped-selection.v1";
const VISIBLE_CONTENT_SCHEMA: &str = "kiln.source-view-visible-content.v1";
const VIEW_KEYING_SCHEMA: &str = "kiln.source-view-keying.v1";
const CAIRN_OBSERVATION_SCHEMA: &str = "kiln.cairn-observation.v1";
const HANDLE_DOMAIN: &str = "kiln.source-view-handle.v1";
const REFERENCE_DOMAIN: &str = "kiln.source-view-reference.v1";
const CURSOR_DOMAIN: &str = "kiln.source-view-cursor.v1";
const CAIRN_HANDLE_DOMAIN: &str = "kiln.cairn-item-handle.v1";

/// Width of a handle key, handle or cursor tag.
pub const HANDLE_BYTES: usize = 32;
/// Visible entries one restriction emits before reporting truncation.
pub const MAX_VISIBLE_ENTRIES: usize = 4_096;
/// Entries one page returns.
pub const MAX_PAGE_ENTRIES: usize = 256;
/// Items in one Cairn observation.
pub const MAX_CAIRN_ITEMS: usize = 1_024;
/// Bytes of one Cairn item's text.
pub const MAX_CAIRN_TEXT_BYTES: usize = 16_384;
/// Prerequisites of one Cairn item.
pub const MAX_CAIRN_PREREQUISITES: usize = 64;

// ---------------------------------------------------------------------------
// Keys, handles and cursors

/// The shell's secret key for handles and cursors. It is never serialized
/// or printed.
pub struct HandleKey([u8; HANDLE_BYTES]);

impl core::fmt::Debug for HandleKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("HandleKey(<redacted>)")
    }
}

impl HandleKey {
    /// Wrap key bytes the shell generated and keeps.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HANDLE_BYTES]) -> Self {
        Self(bytes)
    }

    fn keyed(&self, domain: &str, parts: &[&[u8]]) -> [u8; HANDLE_BYTES] {
        let mut hasher = blake3::Hasher::new_keyed(&self.0);
        update_field(&mut hasher, "domain", domain.as_bytes());
        for part in parts {
            update_field(&mut hasher, "part", part);
        }
        *hasher.finalize().as_bytes()
    }
}

/// A view-scoped opaque reference. It names one entry of one binding and
/// confers no authority.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueHandle([u8; HANDLE_BYTES]);

impl OpaqueHandle {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HANDLE_BYTES] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }

    /// Parse 64 lowercase or uppercase hexadecimal characters.
    #[must_use]
    pub fn from_hex(value: &str) -> Option<Self> {
        unhex(value).map(Self)
    }
}

impl core::fmt::Debug for OpaqueHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "OpaqueHandle({})", self.to_hex())
    }
}

impl Serialize for OpaqueHandle {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for OpaqueHandle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_hex(&text).ok_or_else(|| serde::de::Error::custom("invalid opaque handle"))
    }
}

/// A keyed continuation bound to one view binding and position.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ViewCursor {
    position: u32,
    tag: [u8; HANDLE_BYTES],
}

impl ViewCursor {
    /// `<8 hex position><64 hex tag>`.
    #[must_use]
    pub fn to_token(&self) -> String {
        let mut token = hex(&self.position.to_be_bytes());
        token.push_str(&hex(&self.tag));
        token
    }

    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        let position = token.get(..8)?;
        let tag = unhex(token.get(8..)?)?;
        let mut bytes = [0_u8; 4];
        for (slot, pair) in bytes.iter_mut().zip(position.as_bytes().chunks(2)) {
            *slot = hex_pair(pair)?;
        }
        Some(Self {
            position: u32::from_be_bytes(bytes),
            tag,
        })
    }
}

impl core::fmt::Debug for ViewCursor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "ViewCursor({})", self.to_token())
    }
}

impl Serialize for ViewCursor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_token())
    }
}

impl<'de> Deserialize<'de> for ViewCursor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_token(&text).ok_or_else(|| serde::de::Error::custom("invalid view cursor"))
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = DIGITS.get(usize::from(nibble)) {
                text.push(char::from(*digit));
            }
        }
    }
    text
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_pair(pair: &[u8]) -> Option<u8> {
    match pair {
        [high, low] => Some(hex_digit(*high)? << 4 | hex_digit(*low)?),
        _ => None,
    }
}

fn unhex(value: &str) -> Option<[u8; HANDLE_BYTES]> {
    if value.len() != HANDLE_BYTES * 2 {
        return None;
    }
    let mut bytes = [0_u8; HANDLE_BYTES];
    for (slot, pair) in bytes.iter_mut().zip(value.as_bytes().chunks(2)) {
        *slot = hex_pair(pair)?;
    }
    Some(bytes)
}

// ---------------------------------------------------------------------------
// Refusals

/// The single public shape for a missing, forbidden or stale object when
/// discovery is not authorized.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PublicUnavailable {
    error: &'static str,
}

impl PublicUnavailable {
    const SHAPE: Self = Self {
        error: "unavailable",
    };
}

/// Why an object was unavailable, kept for the trusted owner's audit only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    /// Nothing exists under that name.
    Missing,
    /// It exists but the holder is not authorized for it.
    Forbidden,
    /// The holder may discover it, but the view's own bound cut it from the
    /// listing.
    Truncated,
    /// The holder may discover it, but the holder-scoped enumeration was
    /// truncated before listing it.
    Unknown,
    /// The view, cursor or handle no longer matches current authority.
    Stale,
    /// The owner's audit enumeration disagrees with this view's repository,
    /// snapshot, definition, or listed destinations.
    AuditMismatch,
}

/// An internally attributable refusal whose public form is uniform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unavailable {
    reason: UnavailableReason,
}

impl Unavailable {
    const fn new(reason: UnavailableReason) -> Self {
        Self { reason }
    }
    #[must_use]
    pub const fn reason(&self) -> UnavailableReason {
        self.reason
    }
    #[must_use]
    pub const fn public(&self) -> PublicUnavailable {
        PublicUnavailable::SHAPE
    }
}

/// Why a restriction was refused, for the trusted owner's audit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisclosureRefusal {
    /// The scope expired.
    Expired,
    /// The scope names another repository.
    RepositoryMismatch,
    /// The scope admitted another exact revision.
    SnapshotMismatch,
    /// The scope grants neither discovery nor reading anywhere.
    NotAuthorized,
    /// The enumeration was made over another selection than the holder's
    /// current scoped selection.
    Stale,
    /// An enumeration holds an entry outside the selection it claims.
    OutsideSelection,
}

impl DisclosureRefusal {
    #[must_use]
    pub const fn public(&self) -> PublicUnavailable {
        PublicUnavailable::SHAPE
    }
}

/// Why holder filtering refused a set of facts. It is a shell-side error
/// with no public form, so the only handling is to enumerate again over the
/// scoped selection; it can never be shown to a holder as availability.
///
/// ```compile_fail,E0599
/// fn shown(error: kiln_core::projection::disclosure::ScopingError) {
///     let _ = error.public();
/// }
/// ```
///
/// ```compile_fail,E0277
/// fn shown(error: kiln_core::projection::disclosure::ScopingError) {
///     let _ = serde_json::to_string(&error);
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopingError {
    /// The facts come from a truncated enumeration of a wider selection,
    /// whose truncation hidden entries may have caused.
    TruncatedInput,
}

// ---------------------------------------------------------------------------
// Ceiling

// r[impl kiln.source.projection.sharing.ceiling]
/// The frozen maximum scope of one view definition: path-component prefixes
/// covering its selection, and the operations its profile can ever admit.
/// A share binds the ceiling it was admitted under, so a later definition
/// edit cannot expand it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewCeiling {
    definition: ContentId,
    paths: BTreeSet<PathScope>,
    operations: BTreeSet<SourceViewOperation>,
    identity: ContentId,
}

impl ViewCeiling {
    /// Derive the ceiling from an admitted definition. Each include pattern
    /// contributes the selection root plus its literal leading components:
    /// an exact path when the whole pattern is literal, a subtree otherwise.
    /// Excludes only narrow, so they never widen the ceiling.
    #[must_use]
    pub fn from_definition(definition: &AdmittedDefinition) -> Self {
        let selection = &definition.selection;
        let root: Vec<String> = if selection.root().is_empty() {
            Vec::new()
        } else {
            selection
                .root()
                .split('/')
                .map(ToString::to_string)
                .collect()
        };
        let mut candidates = BTreeSet::new();
        for pattern in selection.include() {
            let mut components = root.clone();
            let mut subtree = false;
            for part in pattern.split('/') {
                if part.contains(GLOB_CHARS) {
                    subtree = true;
                    break;
                }
                components.push(part.to_string());
            }
            candidates.insert(PathScope {
                components,
                subtree,
            });
        }
        let paths = outermost(candidates);
        let operations = profile_operations(definition.profile());
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", VIEW_CEILING_SCHEMA.as_bytes());
        update_field(
            &mut hasher,
            "definition",
            definition.identity().as_str().as_bytes(),
        );
        update_usize(&mut hasher, "paths", paths.len());
        for path in &paths {
            update_usize(&mut hasher, "components", path.components.len());
            for component in &path.components {
                update_field(&mut hasher, "component", component.as_bytes());
            }
            update_bool(&mut hasher, "subtree", path.subtree);
        }
        for operation in &operations {
            update_field(&mut hasher, "operation", operation.as_str().as_bytes());
        }
        Self {
            definition: definition.identity().clone(),
            paths,
            operations,
            identity: finish(hasher),
        }
    }

    #[must_use]
    pub fn definition(&self) -> &ContentId {
        &self.definition
    }
    #[must_use]
    pub fn paths(&self) -> &BTreeSet<PathScope> {
        &self.paths
    }
    #[must_use]
    pub fn operations(&self) -> &BTreeSet<SourceViewOperation> {
        &self.operations
    }
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    /// Whether the ceiling admits `operation` over a repository path.
    #[must_use]
    pub fn allows(&self, operation: SourceViewOperation, path: &str) -> bool {
        self.operations.contains(&operation) && self.paths.iter().any(|p| p.covers_path(path))
    }

    /// Whether the ceiling contains every path of `scope`.
    #[must_use]
    pub fn contains(&self, scope: &PathScope) -> bool {
        self.paths.iter().any(|path| path.contains(scope))
    }
}

/// Whether `definition` covers the whole repository root with no excluded paths.
#[must_use]
pub fn permits_unfiltered_history(definition: &AdmittedDefinition) -> bool {
    ViewCeiling::from_definition(definition).contains(&PathScope::root())
        && definition.selection.exclude().is_empty()
}

/// Drop every path another path of the set already contains.
fn outermost(candidates: BTreeSet<PathScope>) -> BTreeSet<PathScope> {
    candidates
        .iter()
        .filter(|path| {
            !candidates
                .iter()
                .any(|other| other != *path && other.contains(path))
        })
        .cloned()
        .collect()
}

// r[impl kiln.source.projection.disclosure.unavailable.scenario.scoped]
/// The part of a view's selection one holder may see: the ceiling's paths
/// intersected with the paths of current grants that let it discover or
/// read. It covers the repository root only for a full-source holder. A
/// shell enumerates the source over this selection, so whether an
/// enumeration is truncated never depends on entries the holder may not see.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedSelection {
    paths: BTreeSet<PathScope>,
    identity: ContentId,
}

impl ScopedSelection {
    /// The selection of `scope`'s holder through `ceiling`.
    #[must_use]
    pub fn for_holder(scope: &EffectiveSourceScope, ceiling: &ViewCeiling) -> Self {
        use SourceViewOperation as Op;
        let mut candidates = BTreeSet::new();
        for grant in scope.grants() {
            let discloses = [Op::Discover, Op::ReadContent].iter().any(|operation| {
                grant.operations.contains(operation) && ceiling.operations.contains(operation)
            });
            if !discloses {
                continue;
            }
            // Per-component scopes nest or are disjoint, so an intersection
            // is the narrower of the two or nothing.
            for path in &ceiling.paths {
                if path.contains(&grant.path) {
                    candidates.insert(grant.path.clone());
                } else if grant.path.contains(path) {
                    candidates.insert(path.clone());
                }
            }
        }
        Self::over(candidates)
    }

    /// The whole-ceiling selection: the definition's selection within the
    /// frozen ceiling, with no holder's grants applied. The owner's audit
    /// enumeration for [`AudienceView::attribute`] is made over it. For a
    /// holder whose grants cover the whole ceiling, such as a full-source
    /// holder, it equals [`for_holder`](Self::for_holder), so that holder's
    /// own enumeration is also the audit enumeration.
    #[must_use]
    pub fn whole(ceiling: &ViewCeiling) -> Self {
        Self::over(ceiling.paths.clone())
    }

    /// The whole-repository selection, covering every native and generated
    /// path regardless of any view ceiling.
    #[must_use]
    pub fn root() -> Self {
        Self::over(BTreeSet::from([PathScope::root()]))
    }

    /// Whether every path covered by `other` is also covered by `self`.
    #[must_use]
    pub fn covers(&self, other: &Self) -> bool {
        other
            .paths
            .iter()
            .all(|needed| self.paths.iter().any(|held| held.contains(needed)))
    }

    fn over(candidates: BTreeSet<PathScope>) -> Self {
        let paths = outermost(candidates);
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", SCOPED_SELECTION_SCHEMA.as_bytes());
        update_usize(&mut hasher, "paths", paths.len());
        for path in &paths {
            path.update(&mut hasher);
        }
        Self {
            paths,
            identity: finish(hasher),
        }
    }

    #[must_use]
    pub fn paths(&self) -> &BTreeSet<PathScope> {
        &self.paths
    }
    /// BLAKE3 identity over the selected paths.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    /// Whether the selection is the whole repository, which only a
    /// full-source holder's is. Generated entries need it.
    #[must_use]
    pub fn covers_root(&self) -> bool {
        self.paths.contains(&PathScope::root())
    }

    /// Whether a repository path lies in the selection.
    #[must_use]
    pub fn covers_path(&self, path: &str) -> bool {
        self.paths.iter().any(|scope| scope.covers_path(path))
    }

    /// Keep the facts inside the selection: native entries it covers, and
    /// generated entries only when it covers the root. This models over
    /// complete facts what a real shell gets by asking the source adapter to
    /// enumerate over the selection.
    ///
    /// Truncated input is refused. Where a wider enumeration stopped depends
    /// on entries outside this selection, so carrying its truncation into a
    /// holder's view would let hidden entries decide that view's coverage and
    /// its unknown answers. A truncated case must be enumerated again over
    /// this selection, whose own truncation depends only on what the holder
    /// may see.
    ///
    /// # Errors
    /// Returns [`ScopingError::TruncatedInput`] for truncated facts.
    pub fn filter_facts(&self, facts: &ProjectionFacts) -> Result<ProjectionFacts, ScopingError> {
        if facts.coverage != Coverage::Complete {
            return Err(ScopingError::TruncatedInput);
        }
        Ok(ProjectionFacts {
            repository: facts.repository.clone(),
            exact_revision: facts.exact_revision.clone(),
            canonical_tree: facts.canonical_tree.clone(),
            evaluator: facts.evaluator.clone(),
            adapter: facts.adapter.clone(),
            source_capabilities: facts.source_capabilities.clone(),
            coverage: facts.coverage,
            entries: facts
                .entries
                .iter()
                .filter(|entry| self.covers_path(&entry.path))
                .cloned()
                .collect(),
            dependencies: facts.dependencies.clone(),
            generated: if self.covers_root() {
                facts.generated.clone()
            } else {
                Vec::new()
            },
        })
    }
}

/// A trusted view enumerated over one holder's [`ScopedSelection`]. It
/// records the selection it was made over; [`restrict`] refuses it for a
/// holder whose current selection differs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedEnumeration {
    selection: ScopedSelection,
    view: AdmittedPartialView,
}

impl ScopedEnumeration {
    /// Record that `view` was enumerated over `selection`.
    ///
    /// # Errors
    /// Returns [`DisclosureRefusal::OutsideSelection`] when a native origin
    /// lies outside the selection, or a generated entry exists although the
    /// selection does not cover the root.
    pub fn new(
        selection: &ScopedSelection,
        view: AdmittedPartialView,
    ) -> Result<Self, DisclosureRefusal> {
        let inside = view
            .manifest()
            .entries
            .iter()
            .all(|entry| match &entry.origin {
                EntryOrigin::Native { path, .. } => selection.covers_path(path),
                EntryOrigin::Generated { .. } => selection.covers_root(),
            });
        if !inside {
            return Err(DisclosureRefusal::OutsideSelection);
        }
        Ok(Self {
            selection: selection.clone(),
            view,
        })
    }

    /// Identity of the selection the enumeration was made over.
    #[must_use]
    pub fn selection(&self) -> &ContentId {
        self.selection.identity()
    }
    /// The selection the enumeration was made over.
    #[must_use]
    pub fn scoped_selection(&self) -> &ScopedSelection {
        &self.selection
    }
    /// The holder-scoped trusted view, server-side only.
    #[must_use]
    pub fn view(&self) -> &AdmittedPartialView {
        &self.view
    }
}

/// Operations a profile can ever admit. Navigation never prepares or
/// mutates; only a writable view can modify or create source.
fn profile_operations(profile: ProjectionProfile) -> BTreeSet<SourceViewOperation> {
    use SourceViewOperation as Op;
    let mut operations: BTreeSet<Op> =
        [Op::Discover, Op::ReadContent, Op::ReadHistory, Op::Delegate]
            .into_iter()
            .collect();
    if profile != ProjectionProfile::Navigation {
        operations.extend([Op::PrepareWorkspace, Op::RetrieveResult]);
    }
    if profile == ProjectionProfile::Writable {
        operations.extend([Op::ModifySource, Op::CreateSource]);
    }
    operations
}

// ---------------------------------------------------------------------------
// Binding and restricted view

// r[impl kiln.source.projection.disclosure.handles]
/// Server-side binding of one restricted view: the exact source, definition
/// and trusted manifest, the ceiling, the audience and its effective scope,
/// and the authority and revocation generations. Its identity keys caches
/// and stale checks. Handles, cursors and the view reference are keyed by a
/// separate identity over holder-visible data only (audience, scope, ceiling,
/// definition and the visible content), so a change confined to hidden
/// entries or hidden dependency identities never re-keys them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewBinding {
    repository: ContentId,
    snapshot: ExternalRef,
    definition: ContentId,
    manifest: ContentId,
    ceiling: ContentId,
    audience: String,
    effective_scope: ContentId,
    generation: u64,
    revocation_generation: u64,
    identity: ContentId,
    keying: ContentId,
}

impl ViewBinding {
    fn new(
        manifest: &ResolvedProjectionManifest,
        ceiling: &ViewCeiling,
        scope: &EffectiveSourceScope,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", VIEW_BINDING_SCHEMA.as_bytes());
        update_field(
            &mut hasher,
            "repository",
            manifest.repository.as_str().as_bytes(),
        );
        update_external(&mut hasher, "snapshot", &manifest.exact_revision);
        update_field(
            &mut hasher,
            "definition",
            manifest.definition.as_str().as_bytes(),
        );
        update_field(
            &mut hasher,
            "manifest",
            manifest.identity.as_str().as_bytes(),
        );
        update_field(&mut hasher, "ceiling", ceiling.identity.as_str().as_bytes());
        update_field(&mut hasher, "audience", scope.audience().as_bytes());
        update_field(&mut hasher, "scope", scope.identity().as_str().as_bytes());
        hasher.update(&scope.generation().to_le_bytes());
        hasher.update(&scope.revocation_generation().to_le_bytes());
        let identity = finish(hasher);
        Self {
            repository: manifest.repository.clone(),
            snapshot: manifest.exact_revision.clone(),
            definition: manifest.definition.clone(),
            manifest: manifest.identity.clone(),
            ceiling: ceiling.identity.clone(),
            audience: scope.audience().to_string(),
            effective_scope: scope.identity().clone(),
            generation: scope.generation(),
            revocation_generation: scope.revocation_generation(),
            keying: identity.clone(),
            identity,
        }
    }

    /// Derive the handle keying identity from holder-visible data.
    fn key_over(&mut self, visible: &ContentId) {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", VIEW_KEYING_SCHEMA.as_bytes());
        update_field(&mut hasher, "audience", self.audience.as_bytes());
        update_field(
            &mut hasher,
            "scope",
            self.effective_scope.as_str().as_bytes(),
        );
        update_field(&mut hasher, "ceiling", self.ceiling.as_str().as_bytes());
        update_field(
            &mut hasher,
            "definition",
            self.definition.as_str().as_bytes(),
        );
        update_field(&mut hasher, "visible", visible.as_str().as_bytes());
        self.keying = finish(hasher);
    }

    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }
    #[must_use]
    pub fn snapshot(&self) -> &ExternalRef {
        &self.snapshot
    }
    #[must_use]
    pub fn definition(&self) -> &ContentId {
        &self.definition
    }
    #[must_use]
    pub fn manifest(&self) -> &ContentId {
        &self.manifest
    }
    #[must_use]
    pub fn ceiling(&self) -> &ContentId {
        &self.ceiling
    }
    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }
    #[must_use]
    pub fn effective_scope(&self) -> &ContentId {
        &self.effective_scope
    }
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn revocation_generation(&self) -> u64 {
        self.revocation_generation
    }
    /// Cache and stale-check key of the restricted view. Handles, cursors
    /// and the view reference are keyed over a separate identity of
    /// holder-visible data, never over this one.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    /// Whether `scope` is the live scope this binding was made under.
    fn current_for(&self, scope: &EffectiveSourceScope, now_unix_s: u64) -> bool {
        scope.live_at(now_unix_s)
            && scope.identity() == &self.effective_scope
            && scope.audience() == self.audience
            && scope.generation() == self.generation
            && scope.revocation_generation() == self.revocation_generation
    }
}

/// How one visible entry may be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibleStatus {
    /// Native source the holder may also modify through this view.
    Present,
    /// Native source the holder may discover or read but not modify.
    ReadOnly,
    /// Generator output, never source.
    Generated,
}

/// One entry as the holder sees it. `display_path` and `kind` are withheld
/// unless the holder may discover the origin; `readable` reports read
/// authority. Entries the holder may read but not discover come after every
/// displayed entry, ordered by handle, so their source order leaks nothing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VisibleEntry {
    pub handle: OpaqueHandle,
    pub display_path: Option<String>,
    pub kind: Option<EntryKind>,
    pub status: VisibleStatus,
    pub readable: bool,
}

// r[impl kiln.source.projection.disclosure.unavailable.scenario.coverage]
/// Coverage of the holder-visible entries only. The enumeration behind a
/// view is made over the holder's [`ScopedSelection`], so coverage never
/// depends on hidden entries or on the size of the whole snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VisibleCoverage {
    /// Every authorized entry is listed.
    Complete,
    /// Listing stopped after `bound` entries: the holder-scoped enumeration
    /// was truncated, or the view reached its own bound.
    Truncated { bound: usize },
}

/// A declared dependency as the holder sees it. Missing and denied are the
/// same `unavailable`; the identity is disclosed only to a full-source holder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VisibleDependency {
    pub name: String,
    pub required: bool,
    pub available: bool,
    pub identity: Option<ContentId>,
}

/// What a holder learns by looking up a source path it may discover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginLookup<'a> {
    /// The path is present, under one destination per alias.
    Entries(Vec<&'a VisibleEntry>),
    /// The definition does not select the path.
    Unrequested,
    /// The path is selected and known to be absent.
    Absent,
    /// The holder-scoped enumeration was truncated, so presence is unknown.
    Unknown,
    /// The view's own bound cut the listing before the path could be found.
    Truncated { bound: usize },
}

/// Server-side origin of one visible entry; never presented.
#[derive(Clone, Eq, PartialEq)]
struct TrustedSlot {
    destination: String,
    origin: Option<String>,
}

// r[impl kiln.source.projection.disclosure.restricted]
/// A restricted view for one holder. Only [`restrict`] builds one; it cannot
/// be deserialized, and its presentation omits every trusted binding.
#[derive(Clone, Eq, PartialEq)]
pub struct AudienceView {
    binding: ViewBinding,
    ceiling: ViewCeiling,
    reference: OpaqueHandle,
    entries: Vec<VisibleEntry>,
    coverage: VisibleCoverage,
    dependencies: Vec<VisibleDependency>,
    trusted: Vec<TrustedSlot>,
    /// Whether the holder-scoped enumeration was complete; server-side only.
    enumeration_complete: bool,
}

impl core::fmt::Debug for AudienceView {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AudienceView")
            .field("binding", &self.binding.identity)
            .field("entries", &self.entries.len())
            .field("coverage", &self.coverage)
            .finish_non_exhaustive()
    }
}

/// The serializable presentation of an audience view.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewPresentation<'a> {
    pub schema: &'static str,
    pub view: OpaqueHandle,
    pub entries: &'a [VisibleEntry],
    pub coverage: VisibleCoverage,
    pub dependencies: &'a [VisibleDependency],
}

/// One page of visible entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPage<'a> {
    pub entries: &'a [VisibleEntry],
    pub next: Option<ViewCursor>,
}

impl AudienceView {
    #[must_use]
    pub fn binding(&self) -> &ViewBinding {
        &self.binding
    }
    #[must_use]
    pub fn ceiling(&self) -> &ViewCeiling {
        &self.ceiling
    }
    /// Opaque reference to this view, keyed over its holder-visible keying
    /// identity.
    #[must_use]
    pub fn reference(&self) -> OpaqueHandle {
        self.reference
    }
    #[must_use]
    pub fn entries(&self) -> &[VisibleEntry] {
        &self.entries
    }
    #[must_use]
    pub fn coverage(&self) -> VisibleCoverage {
        self.coverage
    }
    #[must_use]
    pub fn dependencies(&self) -> &[VisibleDependency] {
        &self.dependencies
    }

    #[must_use]
    pub fn presentation(&self) -> ViewPresentation<'_> {
        ViewPresentation {
            schema: VIEW_PRESENTATION_SCHEMA,
            view: self.reference,
            entries: &self.entries,
            coverage: self.coverage,
            dependencies: &self.dependencies,
        }
    }

    /// BLAKE3 identity of what the holder can see, excluding the keyed
    /// handles: equal authorized input yields equal content identities.
    #[must_use]
    pub fn content_identity(&self) -> ContentId {
        visible_content(&self.entries, self.coverage, &self.dependencies)
    }

    /// Recheck the binding against the holder's current scope.
    ///
    /// # Errors
    /// Returns a stale refusal when the scope changed, narrowed, was revoked
    /// or expired.
    pub fn check_current(
        &self,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
    ) -> Result<(), Unavailable> {
        if self.binding.current_for(scope, now_unix_s) {
            Ok(())
        } else {
            Err(Unavailable::new(UnavailableReason::Stale))
        }
    }

    /// Whether `scope` may discover an origin through this view's ceiling.
    /// Generated output, which has no native path, needs full source.
    fn discoverable(&self, scope: &EffectiveSourceScope, origin: &EntryOrigin) -> bool {
        match origin {
            EntryOrigin::Native { path, .. } => {
                self.ceiling.allows(SourceViewOperation::Discover, path)
                    && scope.permits(SourceViewOperation::Discover, path)
            }
            EntryOrigin::Generated { .. } => {
                full_source(scope, &self.ceiling, SourceViewOperation::Discover)
            }
        }
    }

    // r[impl kiln.source.projection.disclosure.unavailable]
    /// Look up a displayed destination path after rechecking the binding
    /// against the holder's current scope. A hidden and a missing path give
    /// the same public refusal. The internal reason here is judged against
    /// the holder-scoped enumeration: an enumerated entry the holder may
    /// discover but the view's bound cut is `Truncated`, one it may not
    /// discover by name is `Forbidden`. [`attribute`](Self::attribute) gives
    /// the owner's attribution against the unscoped trusted view.
    ///
    /// # Errors
    /// Returns `Unavailable` for a hidden, missing, unlisted or stale lookup.
    pub fn lookup(
        &self,
        trusted: &AdmittedPartialView,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
        destination: &str,
    ) -> Result<&VisibleEntry, Unavailable> {
        self.check_current(scope, now_unix_s)?;
        if trusted.manifest().identity != self.binding.manifest {
            return Err(Unavailable::new(UnavailableReason::Stale));
        }
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.display_path.as_deref() == Some(destination))
        {
            return Ok(entry);
        }
        let reason = match trusted
            .manifest()
            .entries
            .iter()
            .find(|entry| entry.destination == destination)
        {
            None => UnavailableReason::Missing,
            // Every discoverable enumerated entry is displayed unless the
            // view's own bound cut it.
            Some(entry) if self.discoverable(scope, &entry.origin) => UnavailableReason::Truncated,
            Some(_) => UnavailableReason::Forbidden,
        };
        Err(Unavailable::new(reason))
    }

    /// Attribute a destination this view does not display, for the trusted
    /// owner's audit only, against the owner's audit enumeration and the
    /// holder's current `scope`. A destination the audit does not hold is
    /// `Missing`. One the holder may not discover through the view's ceiling
    /// under a current grant is `Forbidden`; one it may discover but that is
    /// not listed is `Truncated` or `Unknown`.
    ///
    /// The audit must be a complete enumeration over
    /// [`ScopedSelection::whole`] of this view's ceiling. Any other
    /// enumeration, such as another holder's narrower cached view, gets no
    /// attribution, since it would report a hidden file as missing. For a
    /// holder whose grants cover the whole ceiling, such as a full-source
    /// holder, the two selections are equal and its own enumeration is a
    /// valid audit; nothing in the ceiling is hidden from it.
    ///
    /// There is also no attribution for a displayed destination. A scope
    /// that is no longer current is `Stale`; an audit of another repository,
    /// revision or definition, or one lacking a destination this view lists,
    /// is `AuditMismatch`. The reason never reaches the holder, whose refusal
    /// keeps its one public shape.
    #[must_use]
    pub fn attribute(
        &self,
        audit: &ScopedEnumeration,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
        destination: &str,
    ) -> Option<UnavailableReason> {
        let manifest = audit.view.manifest();
        let whole_ceiling = ScopedSelection::whole(&self.ceiling);
        let holder_selection = ScopedSelection::for_holder(scope, &self.ceiling);
        if !audit.selection.covers(&whole_ceiling) || manifest.coverage != Coverage::Complete {
            return None;
        }
        // Backstop against a shell mislabelling the holder's own narrowed view
        // as an audit: only a holder whose own selection equals the audit's
        // whole-ceiling selection has nothing hidden from it.
        if manifest.identity == self.binding.manifest
            && (holder_selection != whole_ceiling || audit.selection != holder_selection)
        {
            return None;
        }
        if !self.binding.current_for(scope, now_unix_s) {
            return Some(UnavailableReason::Stale);
        }
        let lacks_listed = self.trusted.iter().any(|slot| {
            !manifest
                .entries
                .iter()
                .any(|entry| entry.destination == slot.destination)
        });
        if manifest.repository != self.binding.repository
            || manifest.exact_revision != self.binding.snapshot
            || manifest.definition != self.binding.definition
            || lacks_listed
        {
            return Some(UnavailableReason::AuditMismatch);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.display_path.as_deref() == Some(destination))
        {
            return None;
        }
        let Some(entry) = manifest
            .entries
            .iter()
            .find(|entry| entry.destination == destination)
        else {
            return Some(UnavailableReason::Missing);
        };
        if !self.discoverable(scope, &entry.origin) {
            return Some(UnavailableReason::Forbidden);
        }
        // Discoverable yet undisplayed is unlisted only when something cut
        // the listing; otherwise the audit disagrees with this view.
        Some(if !self.enumeration_complete {
            UnavailableReason::Unknown
        } else if self.coverage == VisibleCoverage::Complete {
            UnavailableReason::AuditMismatch
        } else {
            UnavailableReason::Truncated
        })
    }

    /// Classify a source path the holder may discover: present, unrequested
    /// by the definition, known absent, unknown or beyond the bound.
    ///
    /// # Errors
    /// Returns `Unavailable` when the holder may not discover the path, the
    /// scope is no longer current or the definition is another one.
    pub fn lookup_origin(
        &self,
        definition: &AdmittedDefinition,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
        path: &str,
    ) -> Result<OriginLookup<'_>, Unavailable> {
        self.check_current(scope, now_unix_s)?;
        if definition.identity() != &self.binding.definition {
            return Err(Unavailable::new(UnavailableReason::Stale));
        }
        if !paths::canonical(path)
            || !self.ceiling.allows(SourceViewOperation::Discover, path)
            || !scope.permits(SourceViewOperation::Discover, path)
        {
            return Err(Unavailable::new(UnavailableReason::Forbidden));
        }
        if !definition.selects(path) {
            return Ok(OriginLookup::Unrequested);
        }
        let found: Vec<&VisibleEntry> = self
            .entries
            .iter()
            .zip(&self.trusted)
            .filter(|(_, slot)| slot.origin.as_deref() == Some(path))
            .map(|(entry, _)| entry)
            .collect();
        Ok(if !found.is_empty() {
            OriginLookup::Entries(found)
        } else if !self.enumeration_complete {
            OriginLookup::Unknown
        } else {
            match self.coverage {
                VisibleCoverage::Truncated { bound } => OriginLookup::Truncated { bound },
                VisibleCoverage::Complete => OriginLookup::Absent,
            }
        })
    }

    /// Resolve a handle to its trusted manifest entry for one operation, after
    /// rechecking the binding and the operation against current authority.
    /// The shell then admits the origin's exact components at the port.
    ///
    /// # Errors
    /// Returns `Unavailable` for a foreign, forged or stale handle, or an
    /// operation the scope or ceiling does not grant over the origin.
    pub fn resolve<'t>(
        &self,
        trusted: &'t AdmittedPartialView,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
        handle: &OpaqueHandle,
        operation: SourceViewOperation,
    ) -> Result<&'t ManifestEntry, Unavailable> {
        self.check_current(scope, now_unix_s)?;
        if trusted.manifest().identity != self.binding.manifest {
            return Err(Unavailable::new(UnavailableReason::Stale));
        }
        let Some(slot) = self
            .entries
            .iter()
            .zip(&self.trusted)
            .find(|(entry, _)| &entry.handle == handle)
            .map(|(_, slot)| slot)
        else {
            return Err(Unavailable::new(UnavailableReason::Missing));
        };
        let permitted = match &slot.origin {
            Some(origin) => {
                self.ceiling.allows(operation, origin) && scope.permits(operation, origin)
            }
            None => full_source(scope, &self.ceiling, operation),
        };
        if !permitted {
            return Err(Unavailable::new(UnavailableReason::Forbidden));
        }
        trusted
            .manifest()
            .entries
            .iter()
            .find(|entry| entry.destination == slot.destination)
            .ok_or(Unavailable::new(UnavailableReason::Stale))
    }

    // r[impl kiln.source.projection.disclosure.handles.scenario.cursor]
    /// One bounded page, continuing from a cursor of this binding.
    ///
    /// # Errors
    /// Returns a stale refusal for a cursor of another binding or key, a
    /// position past the end, or a scope that is no longer current.
    pub fn page(
        &self,
        key: &HandleKey,
        scope: &EffectiveSourceScope,
        now_unix_s: u64,
        cursor: Option<&ViewCursor>,
        limit: usize,
    ) -> Result<ViewPage<'_>, Unavailable> {
        self.check_current(scope, now_unix_s)?;
        let start = match cursor {
            None => 0,
            Some(cursor) => {
                if cursor.tag != self.cursor_tag(key, cursor.position) {
                    return Err(Unavailable::new(UnavailableReason::Stale));
                }
                usize::try_from(cursor.position).unwrap_or(usize::MAX)
            }
        };
        let end = start
            .saturating_add(limit.clamp(1, MAX_PAGE_ENTRIES))
            .min(self.entries.len());
        let entries = self
            .entries
            .get(start..end)
            .ok_or(Unavailable::new(UnavailableReason::Stale))?;
        let next = if end < self.entries.len() {
            let position = u32::try_from(end).unwrap_or(u32::MAX);
            Some(ViewCursor {
                position,
                tag: self.cursor_tag(key, position),
            })
        } else {
            None
        };
        Ok(ViewPage { entries, next })
    }

    fn cursor_tag(&self, key: &HandleKey, position: u32) -> [u8; HANDLE_BYTES] {
        key.keyed(
            CURSOR_DOMAIN,
            &[
                self.binding.keying.as_str().as_bytes(),
                &position.to_le_bytes(),
            ],
        )
    }
}

const fn status_label(status: VisibleStatus) -> &'static str {
    match status {
        VisibleStatus::Present => "present",
        VisibleStatus::ReadOnly => "read_only",
        VisibleStatus::Generated => "generated",
    }
}

/// BLAKE3 over what the holder may see: displayed entries in order, entries
/// it may only read as an unordered multiset of status and readability,
/// coverage and the visible dependency projection. Handles are excluded.
fn visible_content(
    entries: &[VisibleEntry],
    coverage: VisibleCoverage,
    dependencies: &[VisibleDependency],
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", VISIBLE_CONTENT_SCHEMA.as_bytes());
    let mut undisplayed: Vec<(&'static str, bool)> = Vec::new();
    for entry in entries {
        let Some(path) = &entry.display_path else {
            undisplayed.push((status_label(entry.status), entry.readable));
            continue;
        };
        update_field(&mut hasher, "display_path", path.as_bytes());
        update_field(
            &mut hasher,
            "kind",
            entry.kind.map_or("", EntryKind::as_str).as_bytes(),
        );
        update_field(&mut hasher, "status", status_label(entry.status).as_bytes());
        update_bool(&mut hasher, "readable", entry.readable);
    }
    undisplayed.sort_unstable();
    update_usize(&mut hasher, "undisplayed", undisplayed.len());
    for (status, readable) in undisplayed {
        update_field(&mut hasher, "status", status.as_bytes());
        update_bool(&mut hasher, "readable", readable);
    }
    match coverage {
        VisibleCoverage::Complete => update_field(&mut hasher, "coverage", b"complete"),
        VisibleCoverage::Truncated { bound } => {
            update_field(&mut hasher, "coverage", b"truncated");
            update_usize(&mut hasher, "bound", bound);
        }
    }
    for dependency in dependencies {
        update_field(&mut hasher, "dependency", dependency.name.as_bytes());
        update_bool(&mut hasher, "required", dependency.required);
        update_bool(&mut hasher, "available", dependency.available);
        update_field(
            &mut hasher,
            "identity",
            dependency
                .identity
                .as_ref()
                .map_or("", ContentId::as_str)
                .as_bytes(),
        );
    }
    finish(hasher)
}

/// A full-source holder: the ceiling and one grant cover the whole repository
/// for `operation`. Generated entries and dependency identities, which have no
/// native path, are disclosed only to one.
fn full_source(
    scope: &EffectiveSourceScope,
    ceiling: &ViewCeiling,
    operation: SourceViewOperation,
) -> bool {
    let root = PathScope::root();
    ceiling.operations.contains(&operation)
        && ceiling.contains(&root)
        && scope.permits_scope(operation, &root)
}

// r[impl kiln.source.projection.disclosure.restricted.scenario.filter_first]
/// Restrict a holder-scoped enumeration to one holder.
///
/// The enumeration must have been made over the holder's current
/// [`ScopedSelection`] through `ceiling`. An entry is visible only when its
/// native origin lies in the ceiling and a current grant lets the holder
/// discover or read it. Aliases are judged by their origin, never by their
/// destination. Filtering happens before the entries are counted, truncated,
/// paged or turned into coverage, so nothing outside the scope shapes the
/// result.
///
/// # Errors
/// Returns the internal refusal for an expired scope, another repository or
/// revision, a scope without discovery or read anywhere, or an enumeration
/// made over another selection.
pub fn restrict(
    enumeration: &ScopedEnumeration,
    scope: &EffectiveSourceScope,
    ceiling: &ViewCeiling,
    key: &HandleKey,
    now_unix_s: u64,
) -> Result<AudienceView, DisclosureRefusal> {
    restrict_bounded(
        enumeration,
        scope,
        ceiling,
        key,
        now_unix_s,
        MAX_VISIBLE_ENTRIES,
    )
}

fn restrict_bounded(
    enumeration: &ScopedEnumeration,
    scope: &EffectiveSourceScope,
    ceiling: &ViewCeiling,
    key: &HandleKey,
    now_unix_s: u64,
    bound: usize,
) -> Result<AudienceView, DisclosureRefusal> {
    use SourceViewOperation as Op;
    let manifest = enumeration.view.manifest();
    if !scope.live_at(now_unix_s) {
        return Err(DisclosureRefusal::Expired);
    }
    if scope.repository() != &manifest.repository {
        return Err(DisclosureRefusal::RepositoryMismatch);
    }
    if scope.revision() != &manifest.exact_revision {
        return Err(DisclosureRefusal::SnapshotMismatch);
    }
    if !scope.names(Op::Discover) && !scope.names(Op::ReadContent) {
        return Err(DisclosureRefusal::NotAuthorized);
    }
    if enumeration.selection() != ScopedSelection::for_holder(scope, ceiling).identity() {
        return Err(DisclosureRefusal::Stale);
    }
    let mut binding = ViewBinding::new(manifest, ceiling, scope);
    let writable = manifest.profile == ProjectionProfile::Writable;
    // Filter first: only authorized entries are ever counted or ordered.
    let mut displayed = Vec::new();
    let mut undisplayed = Vec::new();
    for entry in &manifest.entries {
        let allowed = |operation: Op| match &entry.origin {
            EntryOrigin::Native { path, .. } => {
                ceiling.allows(operation, path) && scope.permits(operation, path)
            }
            EntryOrigin::Generated { .. } => full_source(scope, ceiling, operation),
        };
        let discover = allowed(Op::Discover);
        let readable = allowed(Op::ReadContent);
        if !discover && !readable {
            continue;
        }
        let (status, origin) = match &entry.origin {
            EntryOrigin::Generated { .. } => (VisibleStatus::Generated, None),
            EntryOrigin::Native { path, .. } => (
                if writable && allowed(Op::ModifySource) {
                    VisibleStatus::Present
                } else {
                    VisibleStatus::ReadOnly
                },
                Some(path.clone()),
            ),
        };
        let visible = VisibleEntry {
            handle: OpaqueHandle([0; HANDLE_BYTES]),
            display_path: discover.then(|| entry.destination.clone()),
            kind: discover.then_some(entry.kind),
            status,
            readable,
        };
        let slot = TrustedSlot {
            destination: entry.destination.clone(),
            origin,
        };
        if discover {
            displayed.push((visible, slot));
        } else {
            undisplayed.push((visible, slot));
        }
    }
    let total = displayed.len() + undisplayed.len();
    let enumeration_complete = manifest.coverage == Coverage::Complete;
    let coverage = if total > bound || !enumeration_complete {
        VisibleCoverage::Truncated {
            bound: total.min(bound),
        }
    } else {
        VisibleCoverage::Complete
    };
    let disclose_identity = full_source(scope, ceiling, Op::Discover);
    let dependencies: Vec<VisibleDependency> = manifest
        .dependencies
        .iter()
        .map(|dependency| {
            let identity = match &dependency.status {
                DependencyStatus::Present { identity } => Some(identity),
                DependencyStatus::Missing | DependencyStatus::Denied => None,
            };
            VisibleDependency {
                name: dependency.name.clone(),
                required: dependency.required,
                available: identity.is_some(),
                identity: identity.filter(|_| disclose_identity).cloned(),
            }
        })
        .collect();
    // Key handles over what the holder may see, never over hidden entries or
    // hidden dependency identities.
    let authorized: Vec<VisibleEntry> = displayed
        .iter()
        .chain(&undisplayed)
        .map(|(entry, _)| entry.clone())
        .collect();
    binding.key_over(&visible_content(&authorized, coverage, &dependencies));
    let handle_for = |destination: &str| {
        OpaqueHandle(key.keyed(
            HANDLE_DOMAIN,
            &[binding.keying.as_str().as_bytes(), destination.as_bytes()],
        ))
    };
    for (entry, slot) in displayed.iter_mut().chain(undisplayed.iter_mut()) {
        entry.handle = handle_for(&slot.destination);
    }
    // Read-only-by-handle entries follow every displayed entry, by handle.
    undisplayed.sort_by_key(|(entry, _)| entry.handle);
    let (entries, trusted): (Vec<VisibleEntry>, Vec<TrustedSlot>) =
        displayed.into_iter().chain(undisplayed).take(bound).unzip();
    let reference =
        OpaqueHandle(key.keyed(REFERENCE_DOMAIN, &[binding.keying.as_str().as_bytes()]));
    Ok(AudienceView {
        binding,
        ceiling: ceiling.clone(),
        reference,
        entries,
        coverage,
        dependencies,
        trusted,
        enumeration_complete,
    })
}

// ---------------------------------------------------------------------------
// Cairn context

/// The kind of one Cairn lifecycle item.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnItemKind {
    Change,
    Requirement,
    Task,
    Decision,
    Evidence,
}

impl CairnItemKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Change => "change",
            Self::Requirement => "requirement",
            Self::Task => "task",
            Self::Decision => "decision",
            Self::Evidence => "evidence",
        }
    }
}

/// The lifecycle status of one item.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnStatus {
    Open,
    Blocked,
    Done,
}

impl CairnStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }
}

/// One Cairn item as the trusted owner observed it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnItem {
    /// The change directory, `.cairn/changes/<change>`.
    pub change: String,
    pub kind: CairnItemKind,
    /// Native identity, such as a requirement or task id.
    pub native_id: String,
    /// Repository path of the item's source document.
    pub path: String,
    pub text: String,
    pub status: CairnStatus,
    /// Native ids this item depends on.
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

/// The complete canonical Cairn observation retained by the trusted owner.
/// A restricted context never converts into one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CairnObservation {
    repository: ContentId,
    revision: ExternalRef,
    items: Vec<CairnItem>,
    identity: ContentId,
}

impl CairnObservation {
    /// Admit an observation of one repository at one exact revision: items
    /// in canonical order, each under its own change path, with unique
    /// native ids and known prerequisites.
    ///
    /// # Errors
    /// Returns sorted diagnostics for every fault.
    pub fn new(
        repository: ContentId,
        revision: ExternalRef,
        mut items: Vec<CairnItem>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut findings = super::Findings::default();
        if items.len() > MAX_CAIRN_ITEMS {
            findings.push("bound_exceeded", "items", "too many Cairn items");
        }
        items.sort_by(|a, b| a.native_id.cmp(&b.native_id));
        let ids: BTreeSet<&str> = items.iter().map(|item| item.native_id.as_str()).collect();
        if ids.len() != items.len() {
            findings.push(
                "duplicate_native_id",
                "items",
                "Cairn native ids must be unique",
            );
        }
        for item in &items {
            let field = alloc::format!("items.{}", super::shown(&item.native_id));
            if !valid_identifier(&item.native_id) || !valid_identifier(&item.change) {
                findings.push("invalid_identifier", field.clone(), "invalid Cairn id");
            }
            let under_change = PathScope::cairn_change(&item.change)
                .is_some_and(|scope| paths::canonical(&item.path) && scope.covers_path(&item.path));
            if !under_change {
                findings.push(
                    "path_outside_change",
                    field.clone(),
                    "a Cairn item must live under its own change path",
                );
            }
            if item.text.len() > MAX_CAIRN_TEXT_BYTES
                || item.prerequisites.len() > MAX_CAIRN_PREREQUISITES
            {
                findings.push(
                    "bound_exceeded",
                    field.clone(),
                    "a Cairn item exceeds a bound",
                );
            }
            if item
                .prerequisites
                .iter()
                .any(|id| !ids.contains(id.as_str()) || id == &item.native_id)
            {
                findings.push(
                    "unknown_prerequisite",
                    field,
                    "a prerequisite must name another observed item",
                );
            }
        }
        if !findings.is_empty() {
            return Err(findings.into_sorted());
        }
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", CAIRN_OBSERVATION_SCHEMA.as_bytes());
        update_field(&mut hasher, "repository", repository.as_str().as_bytes());
        update_external(&mut hasher, "revision", &revision);
        update_usize(&mut hasher, "items", items.len());
        for item in &items {
            update_field(&mut hasher, "change", item.change.as_bytes());
            update_field(&mut hasher, "kind", item.kind.as_str().as_bytes());
            update_field(&mut hasher, "native_id", item.native_id.as_bytes());
            update_field(&mut hasher, "path", item.path.as_bytes());
            update_field(&mut hasher, "text", item.text.as_bytes());
            update_field(&mut hasher, "status", item.status.as_str().as_bytes());
            let prerequisites: BTreeSet<&str> =
                item.prerequisites.iter().map(String::as_str).collect();
            update_usize(&mut hasher, "prerequisites", prerequisites.len());
            for prerequisite in prerequisites {
                update_field(&mut hasher, "prerequisite", prerequisite.as_bytes());
            }
        }
        Ok(Self {
            repository,
            revision,
            items,
            identity: finish(hasher),
        })
    }

    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }
    #[must_use]
    pub fn revision(&self) -> &ExternalRef {
        &self.revision
    }
    #[must_use]
    pub fn items(&self) -> &[CairnItem] {
        &self.items
    }
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
}

/// One Cairn item the holder may discover. `text` needs read authority;
/// prerequisites are handles whether visible or hidden.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VisibleCairnItem {
    pub handle: OpaqueHandle,
    pub change: String,
    pub kind: CairnItemKind,
    pub native_id: String,
    pub text: Option<String>,
    pub status: CairnStatus,
    pub prerequisites: Vec<OpaqueHandle>,
}

/// The status of a hidden prerequisite as the holder may see it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrerequisiteStatus {
    Open,
    Blocked,
    Done,
    /// The holder may not discover the prerequisite's change.
    Unavailable,
}

impl PrerequisiteStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A hidden prerequisite of a visible item: an opaque handle, and its status
/// only where the holder may discover the prerequisite's change.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct HiddenPrerequisite {
    pub handle: OpaqueHandle,
    pub status: PrerequisiteStatus,
}

// r[impl kiln.source.projection.disclosure.cairn]
/// Cairn context restricted to one holder and bound to its view binding.
/// It is a presentation; it cannot be deserialized or converted into a
/// [`CairnObservation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestrictedContext {
    binding: ViewBinding,
    observation: ContentId,
    items: Vec<VisibleCairnItem>,
    hidden_prerequisites: Vec<HiddenPrerequisite>,
    identity: ContentId,
}

/// The serializable presentation of a restricted context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContextPresentation<'a> {
    pub schema: &'static str,
    pub context: &'a ContentId,
    pub items: &'a [VisibleCairnItem],
    pub hidden_prerequisites: &'a [HiddenPrerequisite],
}

impl RestrictedContext {
    #[must_use]
    pub fn binding(&self) -> &ViewBinding {
        &self.binding
    }
    /// The complete observation this context was derived from, server-side.
    #[must_use]
    pub fn observation(&self) -> &ContentId {
        &self.observation
    }
    #[must_use]
    pub fn items(&self) -> &[VisibleCairnItem] {
        &self.items
    }
    #[must_use]
    pub fn hidden_prerequisites(&self) -> &[HiddenPrerequisite] {
        &self.hidden_prerequisites
    }
    /// Identity binding the delivered visible items and hidden-prerequisite
    /// statuses to the view's holder-visible keying identity (audience,
    /// effective scope, ceiling, definition and visible content). It does not
    /// bind the exact observation; [`observation`](Self::observation) does,
    /// server-side.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    #[must_use]
    pub fn presentation(&self) -> ContextPresentation<'_> {
        ContextPresentation {
            schema: RESTRICTED_CONTEXT_SCHEMA,
            context: &self.identity,
            items: &self.items,
            hidden_prerequisites: &self.hidden_prerequisites,
        }
    }

    /// Derive the opaque handle for a Cairn item `native_id` under this
    /// context's view binding and handle key (server-side only).
    #[must_use]
    pub fn item_handle(&self, key: &HandleKey, native_id: &str) -> OpaqueHandle {
        cairn_item_handle(key, &self.binding, native_id)
    }

    /// Whether a visible item still waits on any prerequisite, hidden ones
    /// included; `None` for a handle this context does not hold.
    #[must_use]
    pub fn blocked(&self, handle: &OpaqueHandle) -> Option<bool> {
        let item = self.items.iter().find(|item| &item.handle == handle)?;
        Some(item.prerequisites.iter().any(|prerequisite| {
            let visible = self
                .items
                .iter()
                .find(|other| &other.handle == prerequisite)
                .map(|other| other.status != CairnStatus::Done);
            let hidden = self
                .hidden_prerequisites
                .iter()
                .find(|other| &other.handle == prerequisite)
                .map(|other| other.status != PrerequisiteStatus::Done);
            visible.or(hidden).unwrap_or(true)
        }))
    }
}

fn cairn_item_handle(key: &HandleKey, binding: &ViewBinding, native_id: &str) -> OpaqueHandle {
    OpaqueHandle(key.keyed(
        CAIRN_HANDLE_DOMAIN,
        &[binding.keying.as_str().as_bytes(), native_id.as_bytes()],
    ))
}

/// Restrict a Cairn observation to the holder of a current audience view.
/// The observation must be of the view's repository at its exact revision.
/// An item is visible only when the view's ceiling and a current grant let the
/// holder discover its change path; its text also needs read authority. A
/// hidden prerequisite of a visible item appears as an opaque handle, with its
/// status only when the holder may discover the prerequisite's change.
///
/// # Errors
/// Returns a stale refusal when the scope is no longer the view's, or the
/// observation is of another repository or revision.
pub fn restrict_context(
    view: &AudienceView,
    observation: &CairnObservation,
    scope: &EffectiveSourceScope,
    key: &HandleKey,
    now_unix_s: u64,
) -> Result<RestrictedContext, Unavailable> {
    use SourceViewOperation as Op;
    view.check_current(scope, now_unix_s)?;
    let binding = &view.binding;
    if observation.repository != binding.repository || observation.revision != binding.snapshot {
        return Err(Unavailable::new(UnavailableReason::Stale));
    }
    let handle = |native_id: &str| cairn_item_handle(key, binding, native_id);
    let allowed = |operation: Op, path: &str| {
        view.ceiling.allows(operation, path) && scope.permits(operation, path)
    };
    let by_id: BTreeMap<&str, &CairnItem> = observation
        .items
        .iter()
        .map(|item| (item.native_id.as_str(), item))
        .collect();
    let mut items = Vec::new();
    let mut hidden = BTreeMap::new();
    for item in &observation.items {
        if !allowed(Op::Discover, &item.path) {
            continue;
        }
        for prerequisite in &item.prerequisites {
            if let Some(other) = by_id.get(prerequisite.as_str())
                && !allowed(Op::Discover, &other.path)
            {
                let status = match PathScope::cairn_change(&other.change) {
                    Some(change) if scope.permits_scope(Op::Discover, &change) => {
                        match other.status {
                            CairnStatus::Open => PrerequisiteStatus::Open,
                            CairnStatus::Blocked => PrerequisiteStatus::Blocked,
                            CairnStatus::Done => PrerequisiteStatus::Done,
                        }
                    }
                    _ => PrerequisiteStatus::Unavailable,
                };
                hidden.insert(handle(&other.native_id), status);
            }
        }
        items.push(VisibleCairnItem {
            handle: handle(&item.native_id),
            change: item.change.clone(),
            kind: item.kind,
            native_id: item.native_id.clone(),
            text: allowed(Op::ReadContent, &item.path).then(|| item.text.clone()),
            status: item.status,
            prerequisites: item.prerequisites.iter().map(|id| handle(id)).collect(),
        });
    }
    let hidden_prerequisites: Vec<HiddenPrerequisite> = hidden
        .into_iter()
        .map(|(handle, status)| HiddenPrerequisite { handle, status })
        .collect();
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", RESTRICTED_CONTEXT_SCHEMA.as_bytes());
    // The presented identity covers holder-visible data only; the exact
    // observation stays bound server-side in `observation`.
    update_field(&mut hasher, "binding", binding.keying.as_str().as_bytes());
    update_usize(&mut hasher, "items", items.len());
    for item in &items {
        update_field(&mut hasher, "handle", item.handle.as_bytes());
        update_field(
            &mut hasher,
            "text",
            item.text.as_deref().unwrap_or("").as_bytes(),
        );
        update_field(&mut hasher, "status", item.status.as_str().as_bytes());
    }
    update_usize(&mut hasher, "hidden", hidden_prerequisites.len());
    for prerequisite in &hidden_prerequisites {
        update_field(&mut hasher, "hidden", prerequisite.handle.as_bytes());
        update_field(
            &mut hasher,
            "hidden_status",
            prerequisite.status.as_str().as_bytes(),
        );
    }
    Ok(RestrictedContext {
        binding: binding.clone(),
        observation: observation.identity.clone(),
        items,
        hidden_prerequisites,
        identity: finish(hasher),
    })
}

#[cfg(test)]
mod tests;
