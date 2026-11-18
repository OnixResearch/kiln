//! Capture and seal edits made in a writable projected workspace (#79).
//!
//! A projected workspace omits and rearranges source files, so its directory
//! diff is not a repository diff. [`capture`] maps what a shell observed in an
//! attempt overlay back through the frozen manifest and the definition's
//! mounts into typed source operations, each naming the base object it
//! expects, and seals them under one identity. A rename is captured as a
//! delete and a create. The shell reads and writes; this module performs no
//! I/O and grants no publication authority.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::materialization::WorkspacePin;
use super::{
    AdmittedDefinition, CompleteProjection, Coverage, EntryKind, EntryOrigin, Findings,
    MAX_MANIFEST_ENTRIES, Mount, ProjectionFacts, ProjectionOperation, SourceEntry, shown,
    update_external,
};
use crate::monorepo::{finish, update_field, update_usize};
use crate::paths;
use crate::types::{ContentId, Diagnostic, ExternalRef};

pub const CANDIDATE_SCHEMA: &str = "kiln.projection.writeback-candidate.v1";
const CANDIDATE_IDENTITY_SCHEMA: &str = "kiln.projection.writeback-candidate-identity.v1";
/// Files one overlay observation may hold.
pub const MAX_OBSERVED_FILES: usize = MAX_MANIFEST_ENTRIES;

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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
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
}

impl SourceOperation {
    /// The source path the operation changes.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Edit { path, .. } | Self::Create { path, .. } | Self::Delete { path, .. } => path,
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Edit { .. } => "edit",
            Self::Create { .. } => "create",
            Self::Delete { .. } => "delete",
        }
    }
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
    /// The overlay the operations were captured from.
    #[must_use]
    pub fn overlay(&self) -> &ContentId {
        &self.overlay
    }

    // r[impl kiln.source.projection.writeback.scenario.round_trip]
    /// The full candidate: the base entries with the operations applied,
    /// every other entry unchanged, in path order. `base` must be the
    /// candidate's base, and each operation must find the object it expects.
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
                    entries
                        .get(path)
                        .is_some_and(|entry| entry.object == *expected)
                        && entries.remove(path).is_some()
                }
                SourceOperation::Create {
                    path, object, kind, ..
                } => {
                    !entries.contains_key(path)
                        && entries
                            .insert(
                                path.clone(),
                                SourceEntry {
                                    path: path.clone(),
                                    object: object.clone(),
                                    kind: *kind,
                                },
                            )
                            .is_none()
                }
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
/// is mapped.
fn admit(
    findings: &mut Findings,
    definition: &AdmittedDefinition,
    complete: &CompleteProjection,
    pin: &WorkspacePin,
    current: CurrentAttempt<'_>,
) {
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
    if source.repository != manifest.repository
        || source.exact_revision != manifest.exact_revision
        || source.canonical_tree != manifest.canonical_tree
    {
        findings.push(
            "source_moved",
            "source",
            "the source moved since the view was resolved; an explicit rebase is required",
        );
    }
    if source.coverage != Coverage::Complete {
        findings.push(
            "incomplete_source_facts",
            "source.coverage",
            "the current source was not enumerated completely",
        );
    }
}

// r[impl kiln.source.projection.writeback]
// r[impl kiln.source.projection.writeback.scenario.independent]
// r[impl kiln.source.projection.writeback.scenario.refused]
// r[impl kiln.source.projection.writeback.scenario.stale]
/// Capture the edits a shell observed in a writable view's overlay.
///
/// Native manifest entries are grouped by origin: every destination of an
/// origin unchanged is no operation, every one changed to the same object and
/// kind is one edit, every one absent is one delete, and any other mix is a
/// contradictory alias. A file the manifest does not hold is a create when it
/// lies under a creation destination, exactly one source path results from
/// the mounts that hold it, and that path is absent from the source and
/// selected by the definition. Every fault is reported before any effect.
pub fn capture(
    definition: &AdmittedDefinition,
    complete: &CompleteProjection,
    pin: &WorkspacePin,
    observed: &[ObservedFile],
    current: CurrentAttempt<'_>,
) -> Result<SealedCandidate, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    admit(&mut findings, definition, complete, pin, current);
    if observed.len() > MAX_OBSERVED_FILES {
        findings.push(
            "observation_bound",
            "observed",
            "the overlay holds more files than the named bound",
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
    let source: BTreeMap<&str, &ExternalRef> = current
        .source
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), &entry.object))
        .collect();
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
    let mut operations = Vec::new();
    for (path, origin) in &origins {
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
    let creation = complete.view().creation_destinations();
    let mut created: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (destination, file) in &files {
        if held.contains(destination) || file.kind == EntryKind::Symlink {
            continue;
        }
        let field = format!("observed.{}", shown(destination));
        if !creation
            .iter()
            .any(|directory| dir_contains(directory, destination))
        {
            findings.push(
                "undeclared_new_file",
                field,
                "a new file lies outside every declared creation destination",
            );
            continue;
        }
        let sources: BTreeSet<String> = definition
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
            continue;
        };
        if source.contains_key(path.as_str()) {
            findings.push(
                "create_over_hidden_source",
                field,
                "the new file would replace a source file the view does not hold",
            );
            continue;
        }
        if !definition.selects(&path) {
            findings.push(
                "new_file_outside_selection",
                field,
                "the new file's source path is outside the definition's selection",
            );
            continue;
        }
        created.entry(path.clone()).or_default().push(*destination);
        operations.push(SourceOperation::Create {
            path,
            destination: (*destination).to_string(),
            object: file.object.clone(),
            kind: file.kind,
        });
    }
    for (path, destinations) in &created {
        if destinations.len() > 1 {
            findings.push(
                "ambiguous_new_file",
                format!("origins.{}", shown(path)),
                "several new files map to one source path",
            );
        }
    }
    operations.sort_by(|left, right| left.path().cmp(right.path()));
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
        identity: finish(blake3::Hasher::new()),
    };
    candidate.identity = candidate_identity(&candidate);
    findings.finish(candidate)
}

// r[impl kiln.source.projection.writeback.scenario.sealed]
fn candidate_identity(candidate: &SealedCandidate) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", CANDIDATE_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "candidate.schema", candidate.schema.as_bytes());
    for (label, identity) in [
        ("repository", &candidate.repository),
        ("base_tree", &candidate.base_tree),
        ("definition", &candidate.definition),
        ("manifest", &candidate.manifest),
        ("overlay", &candidate.overlay),
        ("attempt", &candidate.attempt),
    ] {
        update_field(&mut hasher, label, identity.as_str().as_bytes());
    }
    update_external(&mut hasher, "base_revision", &candidate.base_revision);
    update_field(&mut hasher, "fence", &candidate.fence.to_le_bytes());
    update_usize(&mut hasher, "operations", candidate.operations.len());
    for operation in &candidate.operations {
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
        }
    }
    finish(hasher)
}

#[cfg(test)]
mod tests;
