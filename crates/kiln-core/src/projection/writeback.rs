//! Capture and seal edits made in a writable projected workspace (#79).
//!
//! A projected workspace omits and rearranges source files, so its directory
//! diff is not a repository diff. [`capture`] maps what a shell observed in an
//! attempt overlay back through the frozen manifest and the definition's
//! mounts into typed source operations, each naming the base object it
//! expects, and seals them under one identity. A rename or fork of a source
//! file, and an alias or membership change of the view, is captured only as
//! the worker declares it; without a declaration a removed file and a new one
//! stay a delete and a create. View-definition changes are recorded beside
//! the source operations and never become one. A source that moved since the
//! view was resolved yields a [`WritebackConflict`] naming the overlapping
//! origins: nothing is rebased. [`candidate_facts`] binds a sealed candidate
//! to its build plan and the projects its changes affect. The shell reads and
//! writes; this module performs no I/O and grants no publication authority.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::build_inputs::BuildInputPlan;
use super::materialization::WorkspacePin;
use super::{
    AdmittedDefinition, CompleteProjection, Coverage, EntryKind, EntryOrigin, Findings,
    MAX_MANIFEST_ENTRIES, Mount, ProjectionFacts, ProjectionOperation, SourceEntry, shown,
    update_external,
};
use crate::monorepo::{
    ChangedFiles, GraphFacts, Outcome, ProjectInput, affected, finish, update_field, update_set,
    update_usize,
};
use crate::paths;
use crate::types::{ContentId, Diagnostic, ExternalRef};

pub const CANDIDATE_SCHEMA: &str = "kiln.projection.writeback-candidate.v2";
/// The first candidate schema: edits, creates and deletes only. Recorded v1
/// candidates still verify; capture no longer produces them.
pub const CANDIDATE_SCHEMA_V1: &str = "kiln.projection.writeback-candidate.v1";
const CANDIDATE_IDENTITY_SCHEMA: &str = "kiln.projection.writeback-candidate-identity.v2";
const CANDIDATE_IDENTITY_SCHEMA_V1: &str = "kiln.projection.writeback-candidate-identity.v1";
pub const CONFLICT_SCHEMA: &str = "kiln.projection.writeback-conflict.v1";
pub const CANDIDATE_FACTS_SCHEMA: &str = "kiln.projected-candidate-facts.v1";
const CANDIDATE_FACTS_IDENTITY_SCHEMA: &str = "kiln.projected-candidate-facts.identity.v1";
/// Files one overlay observation may hold.
pub const MAX_OBSERVED_FILES: usize = MAX_MANIFEST_ENTRIES;
/// Renames, forks and view changes one capture may declare.
pub const MAX_DECLARATIONS: usize = 64;

/// One regular file of an overlay, as the shell observed it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedFile {
    /// Overlay-relative destination.
    pub destination: String,
    pub kind: EntryKind,
    /// Provider-native object of the file's bytes, as the shell hashed them.
    pub object: ExternalRef,
}

/// One typed change to the source, by source path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceOperation {
    /// New bytes or kind for an existing source entry, read from
    /// `destination` in the overlay.
    Edit {
        path: String,
        destination: String,
        expected: ExternalRef,
        object: ExternalRef,
        kind: EntryKind,
    },
    /// A new source entry under a declared creation destination.
    Create {
        path: String,
        destination: String,
        object: ExternalRef,
        kind: EntryKind,
    },
    /// Removal of an existing source entry.
    Delete { path: String, expected: ExternalRef },
    /// A source entry moved, bytes and kind unchanged, to a new source path
    /// read from `destination` in the overlay.
    Rename {
        from: String,
        to: String,
        destination: String,
        expected: ExternalRef,
        kind: EntryKind,
    },
    /// A copy of an unchanged source entry at a new source path, read from
    /// `destination` in the overlay.
    Fork {
        from: String,
        to: String,
        destination: String,
        expected: ExternalRef,
        kind: EntryKind,
    },
}

impl SourceOperation {
    /// The source path the operation writes or removes: the new path of a
    /// rename or fork.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Edit { path, .. } | Self::Create { path, .. } | Self::Delete { path, .. } => path,
            Self::Rename { to, .. } | Self::Fork { to, .. } => to,
        }
    }

    /// Every source path whose entry the operation changes.
    #[must_use]
    pub fn changed_paths(&self) -> Vec<&str> {
        match self {
            Self::Rename { from, to, .. } => alloc::vec![from.as_str(), to.as_str()],
            _ => alloc::vec![self.path()],
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Edit { .. } => "edit",
            Self::Create { .. } => "create",
            Self::Delete { .. } => "delete",
            Self::Rename { .. } => "rename",
            Self::Fork { .. } => "fork",
        }
    }
}

/// A change to the view definition a capture records. It is never applied to
/// the source and confers no change of the definition: its owner decides.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewDefinitionOperation {
    /// The destination `from` of source path `origin` moves to `to`.
    AliasChange {
        origin: String,
        from: String,
        to: String,
    },
    /// Source path `path` joins (`member`) or leaves the view.
    MembershipChange { path: String, member: bool },
}

/// What a worker declares about its overlay beyond its files. Destinations
/// are overlay paths; a membership change names a source path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "declaration", rename_all = "snake_case", deny_unknown_fields)]
pub enum Declaration {
    /// Every destination of the origin at `from` is gone and its bytes and
    /// kind now sit at the new destination `to`.
    Rename { from: String, to: String },
    /// The origin at `from` is unchanged and a copy sits at `to`.
    Fork { from: String, to: String },
    /// Destination `from` of an origin moved to `to` in the view; the
    /// origin's source path stays.
    AliasChange { from: String, to: String },
    /// Source path `path` joins or leaves the view.
    MembershipChange { path: String, member: bool },
}

/// The attempt as its owners know it now: the current source facts at the
/// target and the attempt's current fence.
#[derive(Clone, Copy, Debug)]
pub struct CurrentAttempt<'a> {
    pub source: &'a ProjectionFacts,
    pub fence: u64,
}

/// Typed operations captured from one overlay, bound to their base, view and
/// attempt. Only [`capture`] builds one, and it cannot be deserialized.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SealedCandidate {
    schema: &'static str,
    repository: ContentId,
    base_revision: ExternalRef,
    base_tree: ContentId,
    definition: ContentId,
    manifest: ContentId,
    overlay: ContentId,
    attempt: ContentId,
    fence: u64,
    operations: Vec<SourceOperation>,
    view_operations: Vec<ViewDefinitionOperation>,
    identity: ContentId,
}

impl SealedCandidate {
    /// BLAKE3 over every other field.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
    /// The exact source revision the operations apply to.
    #[must_use]
    pub fn base_revision(&self) -> &ExternalRef {
        &self.base_revision
    }
    /// Operations in source-path order.
    #[must_use]
    pub fn operations(&self) -> &[SourceOperation] {
        &self.operations
    }
    /// Recorded view-definition changes, never applied to the source.
    #[must_use]
    pub fn view_operations(&self) -> &[ViewDefinitionOperation] {
        &self.view_operations
    }
    /// The overlay the operations were captured from.
    #[must_use]
    pub fn overlay(&self) -> &ContentId {
        &self.overlay
    }
    /// The manifest and definition of the view the overlay came from.
    #[must_use]
    pub fn view(&self) -> (&ContentId, &ContentId) {
        (&self.manifest, &self.definition)
    }
    /// Every source path the operations change, sorted.
    #[must_use]
    pub fn changed_paths(&self) -> BTreeSet<String> {
        self.operations
            .iter()
            .flat_map(SourceOperation::changed_paths)
            .map(str::to_string)
            .collect()
    }

    // r[impl kiln.source.projection.writeback.scenario.round_trip]
    // r[impl kiln.source.projection.writeback.structural]
    /// The full candidate: the base entries with the operations applied,
    /// every other entry unchanged, in path order. `base` must be the
    /// candidate's base, and each operation must find the object it expects.
    /// View-definition changes do not touch the source.
    pub fn reconstruct(&self, base: &ProjectionFacts) -> Result<Vec<SourceEntry>, Vec<Diagnostic>> {
        let mut findings = Findings::default();
        if base.repository != self.repository
            || base.exact_revision != self.base_revision
            || base.canonical_tree != self.base_tree
        {
            findings.push(
                "stale_source_facts",
                "base",
                "the facts are not the candidate's base",
            );
        }
        if base.coverage != Coverage::Complete {
            findings.push(
                "incomplete_source_facts",
                "base.coverage",
                "the base was not enumerated completely",
            );
        }
        let mut entries: BTreeMap<String, SourceEntry> = BTreeMap::new();
        for entry in &base.entries {
            if entries.insert(entry.path.clone(), entry.clone()).is_some() {
                findings.push(
                    "stale_source_facts",
                    format!("base.entries.{}", shown(&entry.path)),
                    "the base lists one path twice",
                );
            }
        }
        let holds =
            |entries: &BTreeMap<String, SourceEntry>, path: &str, expected: &ExternalRef| {
                entries
                    .get(path)
                    .is_some_and(|entry| entry.object == *expected)
            };
        let add = |entries: &mut BTreeMap<String, SourceEntry>,
                   path: &str,
                   object: &ExternalRef,
                   kind: EntryKind| {
            !entries.contains_key(path)
                && entries
                    .insert(
                        path.to_string(),
                        SourceEntry {
                            path: path.to_string(),
                            object: object.clone(),
                            kind,
                        },
                    )
                    .is_none()
        };
        for operation in &self.operations {
            let field = format!("operations.{}", shown(operation.path()));
            let applied = match operation {
                SourceOperation::Edit {
                    path,
                    expected,
                    object,
                    kind,
                    ..
                } => match entries.get_mut(path) {
                    Some(entry) if entry.object == *expected => {
                        entry.object = object.clone();
                        entry.kind = *kind;
                        true
                    }
                    _ => false,
                },
                SourceOperation::Delete { path, expected } => {
                    holds(&entries, path, expected) && entries.remove(path).is_some()
                }
                SourceOperation::Create {
                    path, object, kind, ..
                } => add(&mut entries, path, object, *kind),
                SourceOperation::Rename {
                    from,
                    to,
                    expected,
                    kind,
                    ..
                } => {
                    holds(&entries, from, expected)
                        && entries.remove(from).is_some()
                        && add(&mut entries, to, expected, *kind)
                }
                SourceOperation::Fork {
                    from,
                    to,
                    expected,
                    kind,
                    ..
                } => holds(&entries, from, expected) && add(&mut entries, to, expected, *kind),
            };
            if !applied {
                findings.push(
                    "operation_base_mismatch",
                    field,
                    "the base does not hold what this operation expects at its path",
                );
            }
        }
        findings.finish(entries.into_values().collect())
    }
}

/// A capture over a source that moved since the view was resolved. Nothing
/// is sealed or rebased: `overlapping` names every source path where the
/// captured operations and the current source disagree, and a rebase is a
/// new resolution, overlay generation and capture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WritebackConflict {
    schema: &'static str,
    base: ExternalRef,
    base_tree: ContentId,
    current: ExternalRef,
    current_tree: ContentId,
    attempt: ContentId,
    fence: u64,
    overlapping: Vec<String>,
}

impl WritebackConflict {
    /// The revision the view was resolved at.
    #[must_use]
    pub fn base(&self) -> &ExternalRef {
        &self.base
    }
    /// The revision the source holds now.
    #[must_use]
    pub fn current(&self) -> &ExternalRef {
        &self.current
    }
    /// Source paths where the operations and the current source disagree,
    /// sorted. Empty means a re-resolution at `current` would not overlap.
    #[must_use]
    pub fn overlapping(&self) -> &[String] {
        &self.overlapping
    }
}

/// The outcome of one capture that found no fault.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Captured {
    Sealed(SealedCandidate),
    Conflict(WritebackConflict),
}

/// What an origin's destinations show in the overlay.
enum Seen<'a> {
    Unchanged,
    Missing,
    Changed {
        destination: &'a str,
        object: &'a ExternalRef,
        kind: EntryKind,
    },
}

/// One native origin of the manifest and the destinations it appears at.
struct Origin<'a> {
    expected: &'a ExternalRef,
    kind: EntryKind,
    destinations: Vec<&'a str>,
}

/// Whether `path` lies below directory `dir` (every path lies below "").
fn dir_contains(dir: &str, path: &str) -> bool {
    dir.is_empty()
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The source path `mount` gives a destination it holds.
fn source_path(mount: &Mount, destination: &str) -> Option<String> {
    let rest = if mount.destination_prefix.is_empty() {
        destination
    } else {
        destination
            .strip_prefix(mount.destination_prefix.as_str())?
            .strip_prefix('/')?
    };
    Some(if mount.source_prefix.is_empty() {
        rest.to_string()
    } else {
        format!("{}/{rest}", mount.source_prefix)
    })
}

/// Check the witness, definition, pin and current attempt before anything
/// is mapped. Returns whether the source moved since the view was resolved.
fn admit(
    findings: &mut Findings,
    definition: &AdmittedDefinition,
    complete: &CompleteProjection,
    pin: &WorkspacePin,
    current: CurrentAttempt<'_>,
) -> bool {
    let manifest = complete.manifest();
    if complete.operation() != ProjectionOperation::Write {
        findings.push(
            "write_witness_required",
            "projection",
            "a capture needs a completeness witness for a write",
        );
    }
    if *definition.identity() != manifest.definition {
        findings.push(
            "definition_mismatch",
            "definition",
            "the definition is not the one the projection was resolved from",
        );
    }
    if let Err(invalid) = pin.validate() {
        findings.push(&invalid.code, invalid.field, &invalid.message);
    }
    if pin.repository != manifest.repository
        || pin.source != manifest.exact_revision
        || pin.definition != manifest.definition
        || pin.manifest != manifest.identity
    {
        findings.push(
            "pin_mismatch",
            "pin",
            "the workspace pin names another view or source",
        );
    }
    if pin.fence != current.fence {
        findings.push(
            "superseded_attempt",
            "pin.fence",
            "the attempt's current fence is not the pin's fence",
        );
    }
    let source = current.source;
    if source.repository != manifest.repository {
        findings.push(
            "source_moved",
            "source.repository",
            "the current source facts belong to another repository",
        );
    }
    if source.coverage != Coverage::Complete {
        findings.push(
            "incomplete_source_facts",
            "source.coverage",
            "the current source was not enumerated completely",
        );
    }
    source.exact_revision != manifest.exact_revision
        || source.canonical_tree != manifest.canonical_tree
}

/// Everything capture maps an overlay through.
struct Mapping<'a> {
    definition: &'a AdmittedDefinition,
    creation: &'a BTreeSet<String>,
    /// Every current source path with its object and entry kind.
    source: BTreeMap<&'a str, (&'a ExternalRef, EntryKind)>,
    /// Whether the source moved; a new path the current source holds is then
    /// an overlap, not a refusal.
    moved: bool,
}

impl Mapping<'_> {
    /// The one source path a new overlay file at `destination` creates, or
    /// the reason it creates none.
    fn new_source(&self, findings: &mut Findings, destination: &str) -> Option<String> {
        let field = format!("observed.{}", shown(destination));
        if !self
            .creation
            .iter()
            .any(|directory| dir_contains(directory, destination))
        {
            findings.push(
                "undeclared_new_file",
                field,
                "a new file lies outside every declared creation destination",
            );
            return None;
        }
        let sources: BTreeSet<String> = self
            .definition
            .mounts()
            .iter()
            .filter_map(|mount| source_path(mount, destination))
            .collect();
        let mut sources = sources.into_iter();
        let (Some(path), None) = (sources.next(), sources.next()) else {
            findings.push(
                "ambiguous_new_file",
                field,
                "the new file's destination maps to no source path or to several",
            );
            return None;
        };
        if !self.moved && self.source.contains_key(path.as_str()) {
            findings.push(
                "create_over_hidden_source",
                field,
                "the new file would replace a source file the view does not hold",
            );
            return None;
        }
        if !self.definition.selects(&path) {
            findings.push(
                "new_file_outside_selection",
                field,
                "the new file's source path is outside the definition's selection",
            );
            return None;
        }
        Some(path)
    }
}

/// What declarations took from the overlay before ordinary mapping.
#[derive(Default)]
struct Consumed<'a> {
    origins: BTreeSet<&'a str>,
    files: BTreeSet<&'a str>,
}

// r[impl kiln.source.projection.writeback.structural]
/// Map each declaration onto the overlay, consuming the origins and new
/// files it explains.
fn declare<'a>(
    findings: &mut Findings,
    mapping: &Mapping<'_>,
    declared: &'a [Declaration],
    origins: &BTreeMap<&'a str, Origin<'a>>,
    files: &BTreeMap<&'a str, &'a ObservedFile>,
    held: &BTreeSet<&str>,
) -> (
    Consumed<'a>,
    Vec<SourceOperation>,
    Vec<ViewDefinitionOperation>,
) {
    let mut consumed = Consumed::default();
    let mut operations = Vec::new();
    let mut view = Vec::new();
    let by_destination: BTreeMap<&str, &str> = origins
        .iter()
        .flat_map(|(path, origin)| {
            origin
                .destinations
                .iter()
                .map(move |destination| (*destination, *path))
        })
        .collect();
    let seen = |origin: &Origin<'_>| -> Vec<Option<&ObservedFile>> {
        origin
            .destinations
            .iter()
            .map(|destination| files.get(destination).copied())
            .collect()
    };
    let unchanged = |origin: &Origin<'_>, file: Option<&ObservedFile>| {
        file.is_some_and(|file| file.object == *origin.expected && file.kind == origin.kind)
    };
    for (index, declaration) in declared.iter().enumerate() {
        let field = format!("declared.{index}");
        let invalid = |findings: &mut Findings, message: &str| {
            findings.push("declaration_invalid", field.clone(), message);
        };
        match declaration {
            Declaration::Rename { from, to }
            | Declaration::Fork { from, to }
            | Declaration::AliasChange { from, to } => {
                let Some((&path, origin)) = by_destination
                    .get(from.as_str())
                    .and_then(|path| origins.get_key_value(path))
                else {
                    invalid(
                        findings,
                        "the declared source destination holds no native origin",
                    );
                    continue;
                };
                let Some((&new, file)) = files
                    .get_key_value(to.as_str())
                    .filter(|(destination, _)| !held.contains(**destination))
                else {
                    invalid(
                        findings,
                        "the declared new destination is not a new overlay file",
                    );
                    continue;
                };
                if !consumed.origins.insert(path) || !consumed.files.insert(new) {
                    invalid(
                        findings,
                        "an origin or new file appears in two declarations",
                    );
                    continue;
                }
                let states = seen(origin);
                let copied = file.object == *origin.expected && file.kind == origin.kind;
                let shape = match declaration {
                    Declaration::Rename { .. } => states.iter().all(Option::is_none),
                    Declaration::Fork { .. } => {
                        states.iter().all(|state| unchanged(origin, *state))
                    }
                    _ => origin
                        .destinations
                        .iter()
                        .zip(&states)
                        .all(|(destination, state)| {
                            if *destination == from.as_str() {
                                state.is_none()
                            } else {
                                unchanged(origin, *state)
                            }
                        }),
                };
                if !copied || !shape {
                    findings.push(
                        "declaration_mismatch",
                        field.clone(),
                        "the overlay does not show what the declaration states",
                    );
                    continue;
                }
                if let Declaration::AliasChange { .. } = declaration {
                    view.push(ViewDefinitionOperation::AliasChange {
                        origin: path.to_string(),
                        from: from.clone(),
                        to: to.clone(),
                    });
                    continue;
                }
                let Some(target) = mapping.new_source(findings, new) else {
                    continue;
                };
                let (from, to, destination, expected, kind) = (
                    path.to_string(),
                    target,
                    new.to_string(),
                    origin.expected.clone(),
                    origin.kind,
                );
                operations.push(match declaration {
                    Declaration::Rename { .. } => SourceOperation::Rename {
                        from,
                        to,
                        destination,
                        expected,
                        kind,
                    },
                    _ => SourceOperation::Fork {
                        from,
                        to,
                        destination,
                        expected,
                        kind,
                    },
                });
            }
            Declaration::MembershipChange { path, member } => {
                let known = origins.get_key_value(path.as_str());
                let shape = match (*member, known) {
                    (false, Some((_, origin))) => seen(origin).iter().all(Option::is_none),
                    (true, None) => {
                        paths::canonical(path)
                            && mapping.source.contains_key(path.as_str())
                            && !mapping.definition.selects(path)
                    }
                    _ => false,
                };
                if let Some((&origin, _)) = known
                    && !consumed.origins.insert(origin)
                {
                    invalid(findings, "an origin appears in two declarations");
                    continue;
                }
                if !shape {
                    findings.push(
                        "declaration_mismatch",
                        field.clone(),
                        "the membership change does not match the view and overlay",
                    );
                    continue;
                }
                view.push(ViewDefinitionOperation::MembershipChange {
                    path: path.clone(),
                    member: *member,
                });
            }
        }
    }
    (consumed, operations, view)
}

// r[impl kiln.source.projection.writeback]
// r[impl kiln.source.projection.writeback.scenario.independent]
// r[impl kiln.source.projection.writeback.scenario.refused]
// r[impl kiln.source.projection.writeback.scenario.stale]
// r[impl kiln.source.projection.writeback.conflict]
/// Capture the edits a shell observed in a writable view's overlay.
///
/// Declarations are mapped first: a rename or fork names an origin's
/// destination and a new overlay file holding the origin's exact bytes and
/// kind; an alias change moves one destination of an origin whose other
/// destinations are unchanged; a membership change removes an origin whose
/// every destination is gone, or adds a source path the view does not
/// select. The remaining native manifest entries are grouped by origin:
/// every destination of an origin unchanged is no operation, every one
/// changed to the same object and kind is one edit, every one absent is one
/// delete, and any other mix is a contradictory alias. A remaining file the
/// manifest does not hold is a create when it lies under a creation
/// destination, exactly one source path results from the mounts that hold
/// it, and that path is absent from the source and selected by the
/// definition. Every fault is reported before any effect. When the source
/// moved since the view was resolved, the result is a conflict naming the
/// operations' overlapping source paths, never a sealed candidate.
pub fn capture(
    definition: &AdmittedDefinition,
    complete: &CompleteProjection,
    pin: &WorkspacePin,
    observed: &[ObservedFile],
    declared: &[Declaration],
    current: CurrentAttempt<'_>,
) -> Result<Captured, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let moved = admit(&mut findings, definition, complete, pin, current);
    if observed.len() > MAX_OBSERVED_FILES || declared.len() > MAX_DECLARATIONS {
        findings.push(
            "observation_bound",
            "observed",
            "the overlay holds more files, or the capture more declarations, than the named bound",
        );
        return Err(findings.into_sorted());
    }
    let mut files: BTreeMap<&str, &ObservedFile> = BTreeMap::new();
    for file in observed {
        let field = format!("observed.{}", shown(&file.destination));
        if !paths::canonical(&file.destination)
            || files.insert(file.destination.as_str(), file).is_some()
        {
            findings.push(
                "observation_invalid",
                field,
                "an observed destination is not canonical or appears twice",
            );
        } else if file.kind == EntryKind::Symlink {
            findings.push(
                "unsupported_entry",
                field,
                "symbolic links cannot be written back",
            );
        }
    }
    let mapping = Mapping {
        definition,
        creation: complete.view().creation_destinations(),
        source: current
            .source
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), (&entry.object, entry.kind)))
            .collect(),
        moved,
    };
    let manifest = complete.manifest();
    let mut origins: BTreeMap<&str, Origin<'_>> = BTreeMap::new();
    let mut held = BTreeSet::new();
    for entry in &manifest.entries {
        held.insert(entry.destination.as_str());
        match &entry.origin {
            EntryOrigin::Native { path, object, .. } => origins
                .entry(path.as_str())
                .or_insert_with(|| Origin {
                    expected: object,
                    kind: entry.kind,
                    destinations: Vec::new(),
                })
                .destinations
                .push(entry.destination.as_str()),
            EntryOrigin::Generated { .. } => findings.push(
                "generated_entry",
                format!("destinations.{}", shown(&entry.destination)),
                "a generated entry cannot be written back",
            ),
        }
    }
    let (consumed, mut operations, mut view_operations) =
        declare(&mut findings, &mapping, declared, &origins, &files, &held);
    for (path, origin) in &origins {
        if consumed.origins.contains(path) {
            continue;
        }
        let seen: Vec<Seen<'_>> = origin
            .destinations
            .iter()
            .map(|destination| match files.get(destination) {
                None => Seen::Missing,
                Some(file) if file.object == *origin.expected && file.kind == origin.kind => {
                    Seen::Unchanged
                }
                Some(file) => Seen::Changed {
                    destination,
                    object: &file.object,
                    kind: file.kind,
                },
            })
            .collect();
        if seen.iter().all(|seen| matches!(seen, Seen::Unchanged)) {
            continue;
        }
        if seen.iter().all(|seen| matches!(seen, Seen::Missing)) {
            operations.push(SourceOperation::Delete {
                path: (*path).to_string(),
                expected: origin.expected.clone(),
            });
            continue;
        }
        let mut changes = seen.iter().filter_map(|seen| match seen {
            Seen::Changed { object, kind, .. } => Some((*object, *kind)),
            Seen::Unchanged | Seen::Missing => None,
        });
        let first = changes.next();
        let uniform = first.is_some() && changes.all(|change| Some(change) == first);
        let all_changed = seen.iter().all(|seen| matches!(seen, Seen::Changed { .. }));
        match seen.first() {
            Some(Seen::Changed {
                destination,
                object,
                kind,
            }) if uniform && all_changed => {
                operations.push(SourceOperation::Edit {
                    path: (*path).to_string(),
                    destination: (*destination).to_string(),
                    expected: origin.expected.clone(),
                    object: (*object).clone(),
                    kind: *kind,
                });
            }
            _ => findings.push(
                "contradictory_alias",
                format!("origins.{}", shown(path)),
                "the destinations of one origin disagree",
            ),
        }
    }
    for (destination, file) in &files {
        if held.contains(destination)
            || consumed.files.contains(destination)
            || file.kind == EntryKind::Symlink
        {
            continue;
        }
        if let Some(path) = mapping.new_source(&mut findings, destination) {
            operations.push(SourceOperation::Create {
                path,
                destination: (*destination).to_string(),
                object: file.object.clone(),
                kind: file.kind,
            });
        }
    }
    let mut targets: BTreeMap<&str, usize> = BTreeMap::new();
    for operation in &operations {
        if matches!(
            operation,
            SourceOperation::Create { .. }
                | SourceOperation::Rename { .. }
                | SourceOperation::Fork { .. }
        ) {
            *targets.entry(operation.path()).or_default() += 1;
        }
    }
    for (path, count) in &targets {
        if *count > 1 {
            findings.push(
                "ambiguous_new_file",
                format!("origins.{}", shown(path)),
                "several new files map to one source path",
            );
        }
    }
    operations.sort_by(|left, right| left.path().cmp(right.path()));
    view_operations.sort();
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }
    if moved {
        return Ok(Captured::Conflict(conflict(
            &operations,
            &mapping.source,
            &origins,
            complete,
            current,
            pin,
        )));
    }
    let mut candidate = SealedCandidate {
        schema: CANDIDATE_SCHEMA,
        repository: manifest.repository.clone(),
        base_revision: manifest.exact_revision.clone(),
        base_tree: manifest.canonical_tree.clone(),
        definition: manifest.definition.clone(),
        manifest: manifest.identity.clone(),
        overlay: pin.overlay.clone(),
        attempt: pin.attempt.clone(),
        fence: pin.fence,
        operations,
        view_operations,
        identity: finish(blake3::Hasher::new()),
    };
    candidate.identity = candidate_identity(&Parts::of(&candidate), Version::V2);
    Ok(Captured::Sealed(candidate))
}

// r[impl kiln.source.projection.writeback.conflict]
/// Every source path where `operations` and the current source disagree: an
/// origin whose current object or entry kind (a mode-only change included)
/// differs from the frozen one, or a new path the source now holds.
fn conflict(
    operations: &[SourceOperation],
    source: &BTreeMap<&str, (&ExternalRef, EntryKind)>,
    origins: &BTreeMap<&str, Origin<'_>>,
    complete: &CompleteProjection,
    current: CurrentAttempt<'_>,
    pin: &WorkspacePin,
) -> WritebackConflict {
    let manifest = complete.manifest();
    let changed = |path: &str| match origins.get(path) {
        Some(origin) => source.get(path).copied() != Some((origin.expected, origin.kind)),
        None => true,
    };
    let present = |path: &str| source.contains_key(path);
    let mut overlapping = BTreeSet::new();
    for operation in operations {
        match operation {
            SourceOperation::Edit { path, .. } | SourceOperation::Delete { path, .. } => {
                if changed(path) {
                    overlapping.insert(path.clone());
                }
            }
            SourceOperation::Create { path, .. } => {
                if present(path) {
                    overlapping.insert(path.clone());
                }
            }
            SourceOperation::Rename { from, to, .. } | SourceOperation::Fork { from, to, .. } => {
                if changed(from) {
                    overlapping.insert(from.clone());
                }
                if present(to) {
                    overlapping.insert(to.clone());
                }
            }
        }
    }
    WritebackConflict {
        schema: CONFLICT_SCHEMA,
        base: manifest.exact_revision.clone(),
        base_tree: manifest.canonical_tree.clone(),
        current: current.source.exact_revision.clone(),
        current_tree: current.source.canonical_tree.clone(),
        attempt: pin.attempt.clone(),
        fence: pin.fence,
        overlapping: overlapping.into_iter().collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Version {
    V1,
    V2,
}

/// The identity-bearing fields of a sealed or recorded candidate.
struct Parts<'a> {
    schema: &'a str,
    repository: &'a ContentId,
    base_revision: &'a ExternalRef,
    base_tree: &'a ContentId,
    definition: &'a ContentId,
    manifest: &'a ContentId,
    overlay: &'a ContentId,
    attempt: &'a ContentId,
    fence: u64,
    operations: &'a [SourceOperation],
    view_operations: &'a [ViewDefinitionOperation],
}

impl<'a> Parts<'a> {
    fn of(candidate: &'a SealedCandidate) -> Self {
        Self {
            schema: candidate.schema,
            repository: &candidate.repository,
            base_revision: &candidate.base_revision,
            base_tree: &candidate.base_tree,
            definition: &candidate.definition,
            manifest: &candidate.manifest,
            overlay: &candidate.overlay,
            attempt: &candidate.attempt,
            fence: candidate.fence,
            operations: &candidate.operations,
            view_operations: &candidate.view_operations,
        }
    }
}

// r[impl kiln.source.projection.writeback.scenario.sealed]
// r[impl kiln.source.projection.writeback.compatibility]
/// BLAKE3 over every field. The v1 encoding is the original one, so a v1
/// record keeps its identity; v2 also frames renames, forks and view changes.
fn candidate_identity(candidate: &Parts<'_>, version: Version) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    let schema = match version {
        Version::V1 => CANDIDATE_IDENTITY_SCHEMA_V1,
        Version::V2 => CANDIDATE_IDENTITY_SCHEMA,
    };
    update_field(&mut hasher, "schema", schema.as_bytes());
    update_field(&mut hasher, "candidate.schema", candidate.schema.as_bytes());
    for (label, identity) in [
        ("repository", candidate.repository),
        ("base_tree", candidate.base_tree),
        ("definition", candidate.definition),
        ("manifest", candidate.manifest),
        ("overlay", candidate.overlay),
        ("attempt", candidate.attempt),
    ] {
        update_field(&mut hasher, label, identity.as_str().as_bytes());
    }
    update_external(&mut hasher, "base_revision", candidate.base_revision);
    update_field(&mut hasher, "fence", &candidate.fence.to_le_bytes());
    update_usize(&mut hasher, "operations", candidate.operations.len());
    for operation in candidate.operations {
        update_field(&mut hasher, "operation", operation.label().as_bytes());
        update_field(&mut hasher, "operation.path", operation.path().as_bytes());
        match operation {
            SourceOperation::Edit {
                destination,
                expected,
                object,
                kind,
                ..
            } => {
                update_field(&mut hasher, "operation.destination", destination.as_bytes());
                update_external(&mut hasher, "operation.expected", expected);
                update_external(&mut hasher, "operation.object", object);
                update_field(&mut hasher, "operation.kind", kind.as_str().as_bytes());
            }
            SourceOperation::Create {
                destination,
                object,
                kind,
                ..
            } => {
                update_field(&mut hasher, "operation.destination", destination.as_bytes());
                update_external(&mut hasher, "operation.object", object);
                update_field(&mut hasher, "operation.kind", kind.as_str().as_bytes());
            }
            SourceOperation::Delete { expected, .. } => {
                update_external(&mut hasher, "operation.expected", expected);
            }
            SourceOperation::Rename {
                from,
                destination,
                expected,
                kind,
                ..
            }
            | SourceOperation::Fork {
                from,
                destination,
                expected,
                kind,
                ..
            } => {
                update_field(&mut hasher, "operation.from", from.as_bytes());
                update_field(&mut hasher, "operation.destination", destination.as_bytes());
                update_external(&mut hasher, "operation.expected", expected);
                update_field(&mut hasher, "operation.kind", kind.as_str().as_bytes());
            }
        }
    }
    if version == Version::V2 {
        update_usize(
            &mut hasher,
            "view_operations",
            candidate.view_operations.len(),
        );
        for operation in candidate.view_operations {
            match operation {
                ViewDefinitionOperation::AliasChange { origin, from, to } => {
                    update_field(&mut hasher, "view_operation", b"alias_change");
                    update_field(&mut hasher, "view_operation.origin", origin.as_bytes());
                    update_field(&mut hasher, "view_operation.from", from.as_bytes());
                    update_field(&mut hasher, "view_operation.to", to.as_bytes());
                }
                ViewDefinitionOperation::MembershipChange { path, member } => {
                    update_field(&mut hasher, "view_operation", b"membership_change");
                    update_field(&mut hasher, "view_operation.path", path.as_bytes());
                    update_field(&mut hasher, "view_operation.member", &[u8::from(*member)]);
                }
            }
        }
    }
    finish(hasher)
}

/// A serialized candidate, as retained by a shell. It is a claim: only
/// [`verify_recorded`] judges it, and it never becomes a [`SealedCandidate`].
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedCandidate {
    pub schema: String,
    pub repository: ContentId,
    pub base_revision: ExternalRef,
    pub base_tree: ContentId,
    pub definition: ContentId,
    pub manifest: ContentId,
    pub overlay: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub operations: Vec<SourceOperation>,
    #[serde(default)]
    pub view_operations: Vec<ViewDefinitionOperation>,
    pub identity: ContentId,
}

// r[impl kiln.source.projection.writeback.compatibility]
/// Whether a recorded candidate is internally consistent under its schema:
/// a v1 record holds only edits, creates and deletes and no view changes;
/// either version's identity must cover its fields. The verdict is read-only:
/// it neither seals nor admits the record.
pub fn verify_recorded(record: &RecordedCandidate) -> Result<Version, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let version = match record.schema.as_str() {
        CANDIDATE_SCHEMA => Version::V2,
        CANDIDATE_SCHEMA_V1 => Version::V1,
        _ => {
            findings.push(
                "candidate_schema",
                "schema",
                "unsupported writeback candidate schema",
            );
            return Err(findings.into_sorted());
        }
    };
    if version == Version::V1
        && (!record.view_operations.is_empty()
            || record.operations.iter().any(|operation| {
                matches!(
                    operation,
                    SourceOperation::Rename { .. } | SourceOperation::Fork { .. }
                )
            }))
    {
        findings.push(
            "candidate_schema",
            "operations",
            "a v1 candidate holds only edits, creates and deletes",
        );
    }
    let parts = Parts {
        schema: &record.schema,
        repository: &record.repository,
        base_revision: &record.base_revision,
        base_tree: &record.base_tree,
        definition: &record.definition,
        manifest: &record.manifest,
        overlay: &record.overlay,
        attempt: &record.attempt,
        fence: record.fence,
        operations: &record.operations,
        view_operations: &record.view_operations,
    };
    if candidate_identity(&parts, version) != record.identity {
        findings.push(
            "candidate_identity",
            "identity",
            "the recorded identity does not cover the recorded fields",
        );
    }
    findings.finish(version)
}

/// `kiln.projected-candidate-facts.v1`: what verification of a sealed
/// candidate consumes. It binds the sealed identity, the view it was
/// captured from, the build plan its checks run and the projects its changes
/// affect over the full candidate. Only [`candidate_facts`] builds one, and
/// it cannot be deserialized.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectedCandidateFacts {
    schema: &'static str,
    sealed: ContentId,
    manifest: ContentId,
    definition: ContentId,
    build_plan: Option<ContentId>,
    affected_projects: Vec<String>,
    identity: ContentId,
}

impl ProjectedCandidateFacts {
    /// BLAKE3 over every other field: the identity a candidate handoff names.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
    /// The sealed candidate the facts describe.
    #[must_use]
    pub fn sealed(&self) -> &ContentId {
        &self.sealed
    }
    #[must_use]
    pub fn build_plan(&self) -> Option<&ContentId> {
        self.build_plan.as_ref()
    }
    /// Project identifiers the changes affect, sorted, including projects
    /// outside the view.
    #[must_use]
    pub fn affected_projects(&self) -> &[String] {
        &self.affected_projects
    }

    // r[impl kiln.source.projection.candidate_facts.scenario.applicability]
    /// Whether evidence recorded for `recorded` facts applies to these: only
    /// the same sealed candidate, view, plan and affected projects do.
    #[must_use]
    pub fn accepts_evidence(&self, recorded: &ContentId) -> bool {
        self.identity == *recorded
    }
}

// r[impl kiln.source.projection.candidate_facts]
// r[impl kiln.source.projection.candidate_facts.scenario.affected]
/// Bind a sealed candidate to the build plan its checks run and the projects
/// its changed source paths affect. The plan must have been made for the
/// candidate's manifest at its base; `projects` and `graphs` are the full
/// candidate's scopes and graph facts, so a consumer outside the view is
/// selected when its dependency changed.
pub fn candidate_facts(
    sealed: &SealedCandidate,
    build_plan: Option<&BuildInputPlan>,
    projects: &[ProjectInput],
    graphs: &[GraphFacts],
) -> Result<ProjectedCandidateFacts, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if let Some(plan) = build_plan
        && (*plan.manifest() != sealed.manifest
            || plan.source() != (&sealed.repository, &sealed.base_revision))
    {
        findings.push(
            "build_plan_mismatch",
            "build_plan",
            "the build plan was made for another view or base",
        );
    }
    let decided = affected(
        projects,
        &ChangedFiles::Exact(sealed.changed_paths()),
        graphs,
    );
    let affected_projects: Vec<String> = decided
        .decisions
        .into_iter()
        .filter(|decision| matches!(decision.outcome, Outcome::Affected { .. }))
        .map(|decision| decision.project_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut facts = ProjectedCandidateFacts {
        schema: CANDIDATE_FACTS_SCHEMA,
        sealed: sealed.identity.clone(),
        manifest: sealed.manifest.clone(),
        definition: sealed.definition.clone(),
        build_plan: build_plan.map(|plan| plan.identity().clone()),
        affected_projects,
        identity: finish(blake3::Hasher::new()),
    };
    let mut hasher = blake3::Hasher::new();
    update_field(
        &mut hasher,
        "schema",
        CANDIDATE_FACTS_IDENTITY_SCHEMA.as_bytes(),
    );
    update_field(&mut hasher, "facts.schema", facts.schema.as_bytes());
    for (label, identity) in [
        ("sealed", &facts.sealed),
        ("manifest", &facts.manifest),
        ("definition", &facts.definition),
    ] {
        update_field(&mut hasher, label, identity.as_str().as_bytes());
    }
    match &facts.build_plan {
        Some(plan) => update_field(&mut hasher, "build_plan", plan.as_str().as_bytes()),
        None => update_field(&mut hasher, "build_plan.none", b""),
    }
    update_set(
        &mut hasher,
        "affected_projects",
        facts.affected_projects.iter().map(String::as_str),
    );
    facts.identity = finish(hasher);
    findings.finish(facts)
}

#[cfg(test)]
mod tests;
