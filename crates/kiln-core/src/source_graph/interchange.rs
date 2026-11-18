//! Exact native import and deterministic export.
//!
//! An import binds the exact native revision and tree, acquisition provenance,
//! importer version, layout and the resulting `path -> entity` mapping. A path
//! keeps its entity only when the previous import recorded it at the same path,
//! or when the caller maps it explicitly; equal or similar bytes never decide
//! continuity. Symbolic links, submodules and special objects are refused.
//!
//! An export binds the graph root, layout identity and exporter version to the
//! tree it produced and its root-to-tree mapping. It preserves bytes, executable
//! modes and deletions, and it is deterministic.
//!
//! r[impl kiln.source.graph.interchange]

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    FILE_ID_SCHEMA, FileId, FileMode, Findings, MAX_ENTITIES, MAX_MEMBERSHIPS, MAX_NAME_BYTES,
    MAX_TEXT_BYTES, MembershipKind, Mint, PackageId, Provenance, RevisionId, SourceGraph,
    check_identity, minted, refuse, valid_name, valid_text,
};
use crate::monorepo::{finish, update_field, update_usize};
use crate::projection::ComponentVersion;
use crate::types::{ContentId, Diagnostic, ExternalRef};

/// Schema of an admitted [`GraphLayout`] identity.
pub const LAYOUT_SCHEMA: &str = "kiln.source-graph.layout.v1";
/// Schema of an [`ExportRecord`].
pub const EXPORT_SCHEMA: &str = "kiln.source-graph.export.v1";
/// Schema of an [`ImportRecord`].
pub const IMPORT_SCHEMA: &str = "kiln.source-graph.import.v1";
const TREE_SCHEMA: &str = "kiln.source-graph.tree.v1";

/// Package mounts in one layout.
pub const MAX_LAYOUT_MOUNTS: usize = 256;
/// Generated entries in one layout.
pub const MAX_GENERATED_ENTRIES: usize = 1_024;
/// Native entries, explicit mappings or exported entries in one record.
pub const MAX_TREE_ENTRIES: usize = 65_536;

pub(super) fn valid_component(component: &ComponentVersion) -> bool {
    valid_text(&component.name, MAX_TEXT_BYTES) && valid_text(&component.version, MAX_TEXT_BYTES)
}

fn update_component(hasher: &mut blake3::Hasher, label: &str, component: &ComponentVersion) {
    update_field(hasher, label, component.name.as_bytes());
    update_field(hasher, label, component.version.as_bytes());
}

fn update_external(hasher: &mut blake3::Hasher, label: &str, external: &ExternalRef) {
    update_usize(hasher, label, usize::from(external.contract_version));
    update_field(
        hasher,
        label,
        external.provider_namespace.as_str().as_bytes(),
    );
    update_field(hasher, label, external.native_kind.as_str().as_bytes());
    update_field(hasher, label, external.native_id.as_str().as_bytes());
}

fn update_paths<'a>(
    hasher: &mut blake3::Hasher,
    label: &str,
    mappings: impl ExactSizeIterator<Item = &'a PathMapping>,
) {
    update_usize(hasher, label, mappings.len());
    for mapping in mappings {
        update_field(hasher, "path", mapping.path.as_bytes());
        update_field(hasher, "file", mapping.file.as_str().as_bytes());
    }
}

/// Members of `package` appear under `prefix`; an empty prefix is the tree root.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutMount {
    pub package: PackageId,
    pub prefix: String,
}

/// An entry the exporter generates instead of reading it from the graph. A
/// derived entry is regenerated, so import ignores it; any other generated entry
/// cannot be inverted, so a tree containing it is refused on import.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedEntry {
    pub path: String,
    pub content: ContentId,
    pub mode: FileMode,
    pub derived: bool,
}

/// A graph-level project layout as authored, before admission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphLayout {
    pub id: String,
    pub mounts: Vec<LayoutMount>,
    pub generated: Vec<GeneratedEntry>,
}

/// A validated layout in canonical order, bound to its identity. Only
/// [`admit_layout`] constructs one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedLayout {
    id: String,
    mounts: Vec<LayoutMount>,
    generated: Vec<GeneratedEntry>,
    identity: ContentId,
}

impl AdmittedLayout {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    #[must_use]
    pub fn mounts(&self) -> &[LayoutMount] {
        &self.mounts
    }

    #[must_use]
    pub fn generated(&self) -> &[GeneratedEntry] {
        &self.generated
    }

    /// The single mount covering `path`, and the package-relative name.
    fn locate<'a>(&'a self, path: &'a str) -> Option<(&'a PackageId, &'a str)> {
        self.mounts.iter().find_map(|mount| {
            if mount.prefix.is_empty() {
                return Some((&mount.package, path));
            }
            path.strip_prefix(mount.prefix.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .map(|name| (&mount.package, name))
        })
    }

    fn prefix(&self, package: &PackageId) -> Option<&str> {
        self.mounts
            .iter()
            .find(|mount| mount.package == *package)
            .map(|mount| mount.prefix.as_str())
    }
}

fn place(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn prefixes_overlap(a: &str, b: &str) -> bool {
    a.is_empty()
        || b.is_empty()
        || a == b
        || b.strip_prefix(a).is_some_and(|rest| rest.starts_with('/'))
        || a.strip_prefix(b).is_some_and(|rest| rest.starts_with('/'))
}

fn layout_identity(id: &str, mounts: &[LayoutMount], generated: &[GeneratedEntry]) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", LAYOUT_SCHEMA.as_bytes());
    update_field(&mut hasher, "id", id.as_bytes());
    update_usize(&mut hasher, "mounts", mounts.len());
    for mount in mounts {
        update_field(&mut hasher, "package", mount.package.as_str().as_bytes());
        update_field(&mut hasher, "prefix", mount.prefix.as_bytes());
    }
    update_usize(&mut hasher, "generated", generated.len());
    for entry in generated {
        update_field(&mut hasher, "path", entry.path.as_bytes());
        update_field(&mut hasher, "content", entry.content.as_str().as_bytes());
        update_field(&mut hasher, "mode", entry.mode.as_str().as_bytes());
        update_usize(&mut hasher, "derived", usize::from(entry.derived));
    }
    finish(hasher)
}

/// Admit a layout over a graph's packages. Each package mounts once, mount
/// prefixes never nest, so every tree path maps to at most one package, and
/// generated entries are canonical and unique.
pub fn admit_layout(
    layout: &GraphLayout,
    graph: &SourceGraph,
) -> Result<AdmittedLayout, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if !valid_text(&layout.id, MAX_TEXT_BYTES) {
        findings.push(
            "invalid_layout_id",
            "id",
            "a layout id must be bounded text",
        );
    }
    if layout.mounts.is_empty() || layout.mounts.len() > MAX_LAYOUT_MOUNTS {
        findings.push(
            "bound_exceeded",
            "mounts",
            "a layout needs between one and the named bound of mounts",
        );
    }
    if layout.generated.len() > MAX_GENERATED_ENTRIES {
        findings.push(
            "bound_exceeded",
            "generated",
            "a layout exceeds the generated entry bound",
        );
    }
    let mut mounts = layout.mounts.clone();
    mounts.sort();
    let mut mounted: BTreeSet<&PackageId> = BTreeSet::new();
    for mount in &mounts {
        if graph.package(&mount.package).is_none() {
            findings.push(
                "unknown_package",
                "mounts",
                "a mount names a package the graph does not hold",
            );
        }
        if !mounted.insert(&mount.package) {
            findings.push(
                "duplicate_mount",
                "mounts",
                "a package is mounted more than once",
            );
        }
        if !mount.prefix.is_empty() && !valid_name(&mount.prefix) {
            findings.push(
                "invalid_prefix",
                "mounts",
                "a mount prefix must be empty or a canonical relative path",
            );
        }
    }
    for (index, mount) in mounts.iter().enumerate() {
        if mounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| prefixes_overlap(&mount.prefix, &other.prefix))
        {
            findings.push(
                "overlapping_mounts",
                "mounts",
                "mount prefixes must not nest, so each path has one package",
            );
        }
    }
    let mut generated = layout.generated.clone();
    generated.sort();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for entry in &generated {
        if !valid_name(&entry.path) || entry.content.validate().is_err() {
            findings.push(
                "invalid_generated_entry",
                "generated",
                "a generated entry needs a canonical path and exact bytes",
            );
        }
        if !paths.insert(entry.path.as_str()) {
            findings.push(
                "duplicate_generated_entry",
                "generated",
                "a generated path appears twice",
            );
        }
    }
    let identity = layout_identity(&layout.id, &mounts, &generated);
    findings.finish(AdmittedLayout {
        id: layout.id.clone(),
        mounts,
        generated,
        identity,
    })
}

/// One exported tree entry.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeEntry {
    pub path: String,
    pub content: ContentId,
    pub mode: FileMode,
}

/// Where one exported entry came from.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExportOrigin {
    Member {
        package: PackageId,
        name: String,
        file: FileId,
        revision: RevisionId,
    },
    Generated {
        derived: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedEntry {
    pub entry: TreeEntry,
    pub origin: ExportOrigin,
}

/// An explicit or recorded `path -> entity` binding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathMapping {
    pub path: String,
    pub file: FileId,
}

/// One deterministic export. Deserializing one yields a claim; check it with
/// [`ExportRecord::verify`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRecord {
    pub schema: String,
    pub repository: ContentId,
    pub root: ContentId,
    pub layout: ContentId,
    pub exporter: ComponentVersion,
    /// Entries in path order.
    pub entries: Vec<ExportedEntry>,
    /// Paths the previous export of this layout had and this one does not.
    pub deletions: Vec<String>,
    /// Kiln identity of the produced `(path, bytes, mode)` set. A shell binds
    /// the provider's native tree when it writes one.
    pub tree: ContentId,
    pub identity: ContentId,
}

fn tree_identity(entries: &[ExportedEntry]) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", TREE_SCHEMA.as_bytes());
    update_usize(&mut hasher, "entries", entries.len());
    for exported in entries {
        update_field(&mut hasher, "path", exported.entry.path.as_bytes());
        update_field(
            &mut hasher,
            "content",
            exported.entry.content.as_str().as_bytes(),
        );
        update_field(&mut hasher, "mode", exported.entry.mode.as_str().as_bytes());
    }
    finish(hasher)
}

impl ExportRecord {
    /// The produced tree in path order.
    pub fn tree_entries(&self) -> impl Iterator<Item = &TreeEntry> {
        self.entries.iter().map(|exported| &exported.entry)
    }

    /// Root-to-tree mappings of every graph member, usable as explicit import
    /// mappings for a round trip.
    #[must_use]
    pub fn mappings(&self) -> Vec<PathMapping> {
        self.entries
            .iter()
            .filter_map(|exported| match &exported.origin {
                ExportOrigin::Member { file, .. } => Some(PathMapping {
                    path: exported.entry.path.clone(),
                    file: file.clone(),
                }),
                ExportOrigin::Generated { .. } => None,
            })
            .collect()
    }

    fn compute_identity(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", self.schema.as_bytes());
        update_field(
            &mut hasher,
            "repository",
            self.repository.as_str().as_bytes(),
        );
        update_field(&mut hasher, "root", self.root.as_str().as_bytes());
        update_field(&mut hasher, "layout", self.layout.as_str().as_bytes());
        update_component(&mut hasher, "exporter", &self.exporter);
        update_field(&mut hasher, "tree", self.tree.as_str().as_bytes());
        update_usize(&mut hasher, "entries", self.entries.len());
        for exported in &self.entries {
            update_field(&mut hasher, "path", exported.entry.path.as_bytes());
            match &exported.origin {
                ExportOrigin::Member {
                    package,
                    name,
                    file,
                    revision,
                } => {
                    update_field(&mut hasher, "package", package.as_str().as_bytes());
                    update_field(&mut hasher, "name", name.as_bytes());
                    update_field(&mut hasher, "file", file.as_str().as_bytes());
                    update_field(&mut hasher, "revision", revision.as_str().as_bytes());
                }
                ExportOrigin::Generated { derived } => {
                    update_usize(&mut hasher, "generated", usize::from(*derived));
                }
            }
        }
        update_usize(&mut hasher, "deletions", self.deletions.len());
        for path in &self.deletions {
            update_field(&mut hasher, "deletion", path.as_bytes());
        }
        finish(hasher)
    }

    /// Recompute the tree and record identities from the record's own fields.
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        let mut findings = Findings::default();
        if self.schema != EXPORT_SCHEMA {
            findings.push(
                "unsupported_schema",
                "schema",
                "the export schema is not supported",
            );
        }
        if self.entries.len() > MAX_TREE_ENTRIES {
            findings.push(
                "bound_exceeded",
                "entries",
                "the export exceeds the tree entry bound",
            );
        }
        if self.tree != tree_identity(&self.entries) || self.identity != self.compute_identity() {
            findings.push(
                "forged_record",
                "identity",
                "the export identities do not match its fields",
            );
        }
        findings.finish(())
    }
}

// r[impl kiln.source.graph.interchange.scenario.round_trip]
/// Export a graph root through a layout. Mounted packages' members appear under
/// their prefixes, generated entries are added, and any path that two entries
/// claim, or that is both a file and a directory, is refused.
pub fn export(
    graph: &SourceGraph,
    layout: &AdmittedLayout,
    exporter: &ComponentVersion,
    previous: Option<&ExportRecord>,
) -> Result<ExportRecord, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if !valid_component(exporter) {
        findings.push(
            "invalid_component",
            "exporter",
            "the exporter needs a bounded name and version",
        );
    }
    if previous.is_some_and(|previous| {
        previous.repository != *graph.repository() || previous.layout != layout.identity
    }) {
        findings.push(
            "previous_export_mismatch",
            "previous",
            "a previous export must share this repository and layout",
        );
    }
    let mut entries: BTreeMap<String, ExportedEntry> = BTreeMap::new();
    for mount in &layout.mounts {
        if graph.package(&mount.package).is_none() {
            findings.push(
                "unknown_package",
                "mounts",
                "a mounted package is not in this graph",
            );
            continue;
        }
        for membership in graph.package_memberships(&mount.package) {
            let Some((entity, (content, mode))) = graph
                .entity(&membership.file)
                .and_then(|entity| entity.revision.content().map(|content| (entity, content)))
            else {
                continue;
            };
            let path = place(&mount.prefix, &membership.name);
            if path.len() > MAX_NAME_BYTES {
                findings.push(
                    "bound_exceeded",
                    "entries",
                    "an exported path exceeds the name bound",
                );
                continue;
            }
            let exported = ExportedEntry {
                entry: TreeEntry {
                    path: path.clone(),
                    content: content.clone(),
                    mode,
                },
                origin: ExportOrigin::Member {
                    package: membership.package.clone(),
                    name: membership.name.clone(),
                    file: membership.file.clone(),
                    revision: entity.revision.id.clone(),
                },
            };
            if entries.insert(path, exported).is_some() {
                findings.push(
                    "export_path_conflict",
                    "entries",
                    "two entries export to one path",
                );
            }
        }
    }
    for generated in &layout.generated {
        let exported = ExportedEntry {
            entry: TreeEntry {
                path: generated.path.clone(),
                content: generated.content.clone(),
                mode: generated.mode,
            },
            origin: ExportOrigin::Generated {
                derived: generated.derived,
            },
        };
        if entries.insert(generated.path.clone(), exported).is_some() {
            findings.push(
                "export_path_conflict",
                "entries",
                "a generated entry collides with another entry",
            );
        }
    }
    for path in entries.keys() {
        let mut ancestor = path.as_str();
        while let Some((parent, _)) = ancestor.rsplit_once('/') {
            if entries.contains_key(parent) {
                findings.push(
                    "export_path_conflict",
                    "entries",
                    "an exported path is both a file and a directory",
                );
                break;
            }
            ancestor = parent;
        }
    }
    if entries.len() > MAX_TREE_ENTRIES {
        findings.push(
            "bound_exceeded",
            "entries",
            "the export exceeds the tree entry bound",
        );
    }
    let deletions: Vec<String> = previous
        .map(|previous| {
            previous
                .entries
                .iter()
                .map(|exported| &exported.entry.path)
                .filter(|path| !entries.contains_key(path.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let entries: Vec<ExportedEntry> = entries.into_values().collect();
    let tree = tree_identity(&entries);
    let mut record = ExportRecord {
        schema: EXPORT_SCHEMA.to_string(),
        repository: graph.repository().clone(),
        root: graph.root(),
        layout: layout.identity.clone(),
        exporter: exporter.clone(),
        entries,
        deletions,
        tree: tree.clone(),
        identity: tree,
    };
    record.identity = record.compute_identity();
    findings.finish(record)
}

/// The kind of one native tree entry as the source adapter observed it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeEntryKind {
    File,
    Executable,
    Symlink,
    Submodule,
    Special,
}

/// One entry of an exact native tree, with its provider-native object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEntry {
    pub path: String,
    pub kind: NativeEntryKind,
    pub content: ContentId,
    pub object: ExternalRef,
}

/// One imported path, its entity and the native object read for it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedEntry {
    pub path: String,
    pub file: FileId,
    pub object: ExternalRef,
}

/// One exact import. Deserializing one yields a claim; check it with
/// [`ImportRecord::verify`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRecord {
    pub schema: String,
    pub repository: ContentId,
    pub revision: ExternalRef,
    pub tree: ExternalRef,
    pub provenance: ContentId,
    pub importer: ComponentVersion,
    pub layout: ContentId,
    /// The graph root the import applied to.
    pub base: ContentId,
    /// Every imported path in path order.
    pub entries: Vec<ImportedEntry>,
    /// Newly minted paths whose continuity is unresolved; their candidates are
    /// this import's `removed` memberships. Explicit mappings resolve them.
    pub unresolved: Vec<PathMapping>,
    /// Memberships of mounted packages the tree no longer has.
    pub removed: Vec<PathMapping>,
    /// Entities deleted because no membership remained.
    pub deleted: Vec<FileId>,
    /// The resulting graph root.
    pub root: ContentId,
    pub identity: ContentId,
}

impl ImportRecord {
    fn compute_identity(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", self.schema.as_bytes());
        update_field(
            &mut hasher,
            "repository",
            self.repository.as_str().as_bytes(),
        );
        update_external(&mut hasher, "revision", &self.revision);
        update_external(&mut hasher, "tree", &self.tree);
        update_field(
            &mut hasher,
            "provenance",
            self.provenance.as_str().as_bytes(),
        );
        update_component(&mut hasher, "importer", &self.importer);
        update_field(&mut hasher, "layout", self.layout.as_str().as_bytes());
        update_field(&mut hasher, "base", self.base.as_str().as_bytes());
        update_usize(&mut hasher, "entries", self.entries.len());
        for entry in &self.entries {
            update_field(&mut hasher, "path", entry.path.as_bytes());
            update_field(&mut hasher, "file", entry.file.as_str().as_bytes());
            update_external(&mut hasher, "object", &entry.object);
        }
        update_paths(&mut hasher, "unresolved", self.unresolved.iter());
        update_paths(&mut hasher, "removed", self.removed.iter());
        update_usize(&mut hasher, "deleted", self.deleted.len());
        for file in &self.deleted {
            update_field(&mut hasher, "file", file.as_str().as_bytes());
        }
        update_field(&mut hasher, "root", self.root.as_str().as_bytes());
        finish(hasher)
    }

    /// Recompute the record identity from its own fields.
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        let mut findings = Findings::default();
        if self.schema != IMPORT_SCHEMA {
            findings.push(
                "unsupported_schema",
                "schema",
                "the import schema is not supported",
            );
        }
        if self.entries.len() > MAX_TREE_ENTRIES {
            findings.push(
                "bound_exceeded",
                "entries",
                "the import exceeds the tree entry bound",
            );
        }
        if self.identity != self.compute_identity() {
            findings.push(
                "forged_record",
                "identity",
                "the import identity does not match its fields",
            );
        }
        findings.finish(())
    }
}

/// Exact facts for one import. The shell supplies every one.
#[derive(Clone, Copy, Debug)]
pub struct ImportRequest<'a> {
    pub layout: &'a AdmittedLayout,
    pub revision: &'a ExternalRef,
    pub tree: &'a ExternalRef,
    pub provenance: &'a ContentId,
    pub importer: &'a ComponentVersion,
    /// Mints identities for paths without recorded or explicit continuity.
    pub operation: &'a ContentId,
    pub entries: &'a [NativeEntry],
    pub mappings: &'a [PathMapping],
}

/// One admitted native entry placed by the layout.
struct Placed<'a> {
    entry: &'a NativeEntry,
    package: &'a PackageId,
    name: &'a str,
    mode: FileMode,
}

// r[impl kiln.source.graph.interchange.scenario.ambiguous]
/// Import an exact native tree into a copy of `graph`, returning the new graph
/// and the import record. `graph` itself never changes, so a refusal has no
/// effect. A path keeps its entity only through `previous`'s recorded mapping
/// for the same path, or an explicit mapping; every other path mints a new
/// entity, and outside the first import of the mounted packages it is recorded
/// as unresolved continuity. Memberships the tree no longer has are removed, and
/// entities left without any membership are deleted.
pub fn import(
    graph: &SourceGraph,
    previous: Option<&ImportRecord>,
    request: &ImportRequest<'_>,
) -> Result<(SourceGraph, ImportRecord), Vec<Diagnostic>> {
    if request.entries.len() > MAX_TREE_ENTRIES || request.mappings.len() > MAX_TREE_ENTRIES {
        return refuse(
            "bound_exceeded",
            "entries",
            "the import exceeds the tree entry bound",
        );
    }
    let mut findings = Findings::default();
    if request.revision.validate().is_err() || request.tree.validate().is_err() {
        findings.push(
            "invalid_external_ref",
            "revision",
            "the native revision and tree need valid references",
        );
    }
    for (field, id) in [
        ("provenance", request.provenance),
        ("operation", request.operation),
    ] {
        if let Err(diagnostics) = check_identity(id, field) {
            findings.0.extend(diagnostics);
        }
    }
    if !valid_component(request.importer) {
        findings.push(
            "invalid_component",
            "importer",
            "the importer needs a bounded name and version",
        );
    }
    let base = graph.root();
    if let Some(previous) = previous {
        if previous.repository != *graph.repository() || previous.layout != request.layout.identity
        {
            findings.push(
                "previous_import_mismatch",
                "previous",
                "a previous import must share this repository and layout",
            );
        } else if previous.root != base {
            findings.push(
                "stale_previous_import",
                "previous",
                "the graph changed after the previous import",
            );
        }
    }
    let generated: BTreeMap<&str, bool> = request
        .layout
        .generated
        .iter()
        .map(|entry| (entry.path.as_str(), entry.derived))
        .collect();
    let mut placed: BTreeMap<&str, Placed<'_>> = BTreeMap::new();
    for entry in request.entries {
        if !valid_name(&entry.path) {
            findings.push(
                "invalid_path",
                "entries",
                "a native path must be canonical and within the name bound",
            );
            continue;
        }
        if entry.content.validate().is_err() || entry.object.validate().is_err() {
            findings.push(
                "invalid_identity",
                entry.path.as_str(),
                "an entry needs exact bytes and a valid native object",
            );
        }
        let mode = match entry.kind {
            NativeEntryKind::File => FileMode::Regular,
            NativeEntryKind::Executable => FileMode::Executable,
            NativeEntryKind::Symlink | NativeEntryKind::Submodule | NativeEntryKind::Special => {
                findings.push(
                    "unsupported_entry",
                    entry.path.as_str(),
                    "symbolic links, submodules and special objects are refused until a qualification admits them",
                );
                continue;
            }
        };
        match generated.get(entry.path.as_str()) {
            Some(true) => continue,
            Some(false) => {
                findings.push(
                    "generated_entry",
                    entry.path.as_str(),
                    "a non-invertible generated entry cannot return as source",
                );
                continue;
            }
            None => {}
        }
        let Some((package, name)) = request.layout.locate(&entry.path) else {
            findings.push(
                "unmapped_path",
                entry.path.as_str(),
                "no layout mount covers this path",
            );
            continue;
        };
        let duplicate = placed
            .insert(
                entry.path.as_str(),
                Placed {
                    entry,
                    package,
                    name,
                    mode,
                },
            )
            .is_some();
        if duplicate {
            findings.push(
                "duplicate_path",
                entry.path.as_str(),
                "a path appears twice in the native tree",
            );
        }
    }
    let mut explicit: BTreeMap<&str, &FileId> = BTreeMap::new();
    for mapping in request.mappings {
        if !placed.contains_key(mapping.path.as_str()) {
            findings.push(
                "mapping_without_entry",
                mapping.path.as_str(),
                "a mapping names a path the import does not place",
            );
            continue;
        }
        match graph.entity(&mapping.file) {
            Some(entity) if !entity.is_deleted() => {}
            Some(_) => findings.push(
                "entity_deleted",
                mapping.path.as_str(),
                "a mapping names a deleted entity",
            ),
            None => findings.push(
                "unknown_entity",
                mapping.path.as_str(),
                "a mapping names an entity the graph does not hold",
            ),
        }
        if explicit
            .insert(mapping.path.as_str(), &mapping.file)
            .is_some()
        {
            findings.push(
                "duplicate_mapping",
                mapping.path.as_str(),
                "a path is mapped twice",
            );
        }
    }
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }
    let recorded: BTreeMap<&str, &FileId> = previous
        .map(|previous| {
            previous
                .entries
                .iter()
                .map(|entry| (entry.path.as_str(), &entry.file))
                .collect()
        })
        .unwrap_or_default();

    // Assign every placed path an entity before any change.
    let mut sequence: u32 = 0;
    let mut assigned: Vec<(&Placed<'_>, FileId, Option<Mint>)> = Vec::with_capacity(placed.len());
    let mut alias_bytes: BTreeMap<FileId, (&ContentId, FileMode)> = BTreeMap::new();
    for (path, place_info) in &placed {
        let kept = explicit.get(path).copied().or_else(|| {
            recorded.get(path).copied().filter(|file| {
                graph
                    .entity(file)
                    .is_some_and(|entity| !entity.is_deleted())
            })
        });
        let (file, mint) = match kept {
            Some(file) => (file.clone(), None),
            None => {
                let mint = Mint {
                    operation: request.operation.clone(),
                    sequence,
                };
                sequence = sequence.saturating_add(1);
                let file = FileId(minted(FILE_ID_SCHEMA, graph.repository(), &mint));
                if graph.entity(&file).is_some() {
                    findings.push(
                        "identity_already_minted",
                        *path,
                        "this operation already minted an entity",
                    );
                }
                (file, Some(mint))
            }
        };
        match alias_bytes.get(&file) {
            Some(bytes) if *bytes != (&place_info.entry.content, place_info.mode) => findings.push(
                "contradictory_alias",
                *path,
                "paths of one entity carry different bytes or modes",
            ),
            Some(_) => {}
            None => {
                alias_bytes.insert(file.clone(), (&place_info.entry.content, place_info.mode));
            }
        }
        assigned.push((place_info, file, mint));
    }
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }

    let mut next = graph.clone();
    let initial = request
        .layout
        .mounts
        .iter()
        .all(|mount| next.package_memberships(&mount.package).next().is_none());
    let desired: BTreeMap<(&PackageId, &str), &FileId> = assigned
        .iter()
        .map(|(place_info, file, _)| ((place_info.package, place_info.name), file))
        .collect();
    let existing: Vec<(PackageId, String, FileId)> = request
        .layout
        .mounts
        .iter()
        .flat_map(|mount| next.package_memberships(&mount.package))
        .map(|membership| {
            (
                membership.package.clone(),
                membership.name.clone(),
                membership.file.clone(),
            )
        })
        .collect();
    let mut removed: Vec<PathMapping> = Vec::new();
    for (package, name, file) in existing {
        if desired
            .get(&(&package, name.as_str()))
            .is_some_and(|wanted| **wanted == file)
        {
            continue;
        }
        next.remove_membership_record(&package, &name);
        let prefix = request.layout.prefix(&package).unwrap_or_default();
        removed.push(PathMapping {
            path: place(prefix, &name),
            file,
        });
    }
    let mut unresolved: Vec<PathMapping> = Vec::new();
    let mut edited: BTreeSet<&FileId> = BTreeSet::new();
    for (place_info, file, mint) in &assigned {
        let content = place_info.entry.content.clone();
        match mint {
            Some(mint) => {
                next.insert_entity(
                    file.clone(),
                    mint.clone(),
                    Provenance::Imported,
                    content,
                    place_info.mode,
                );
                if !initial {
                    unresolved.push(PathMapping {
                        path: place_info.entry.path.clone(),
                        file: file.clone(),
                    });
                }
            }
            None => {
                let changed = next
                    .entity(file)
                    .and_then(|entity| entity.revision.content())
                    .is_none_or(|(current, mode)| *current != content || mode != place_info.mode);
                if changed && edited.insert(file) {
                    next.edit(file, content, place_info.mode)?;
                }
            }
        }
        if next
            .membership(place_info.package, place_info.name)
            .is_none()
        {
            let kind = if next.membership_count(file) == 0 {
                MembershipKind::Primary
            } else {
                MembershipKind::Alias
            };
            next.insert_membership(
                place_info.package.clone(),
                place_info.name.to_string(),
                file.clone(),
                kind,
            );
        }
    }
    let mut deleted: BTreeSet<FileId> = BTreeSet::new();
    for mapping in &removed {
        let orphaned = next.membership_count(&mapping.file) == 0
            && next
                .entity(&mapping.file)
                .is_some_and(|entity| !entity.is_deleted());
        if orphaned && deleted.insert(mapping.file.clone()) {
            next.delete(&mapping.file)?;
        }
    }
    if next.entities.len() > MAX_ENTITIES || next.membership_total() > MAX_MEMBERSHIPS {
        return refuse(
            "bound_exceeded",
            "entries",
            "the import would exceed a named graph bound",
        );
    }
    removed.sort();
    let mut record = ImportRecord {
        schema: IMPORT_SCHEMA.to_string(),
        repository: graph.repository().clone(),
        revision: request.revision.clone(),
        tree: request.tree.clone(),
        provenance: request.provenance.clone(),
        importer: request.importer.clone(),
        layout: request.layout.identity.clone(),
        base,
        entries: assigned
            .iter()
            .map(|(place_info, file, _)| ImportedEntry {
                path: place_info.entry.path.clone(),
                file: file.clone(),
                object: place_info.entry.object.clone(),
            })
            .collect(),
        unresolved,
        removed,
        deleted: deleted.into_iter().collect(),
        root: next.root(),
        identity: graph.root(),
    };
    record.identity = record.compute_identity();
    Ok((next, record))
}
