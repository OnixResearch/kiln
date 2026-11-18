//! Optional graph-backed source identity.
//!
//! A graph source gives every file and package a stable logical identity minted
//! once, by the operation that created it. Identities never hash bytes or paths:
//! a rename keeps its identity, and equal bytes never share one. Byte
//! [`ContentId`]s, immutable entity revisions and graph snapshot roots are
//! separate BLAKE3 identities over versioned, length-framed encodings.
//!
//! Membership, aliases, copies, forks and pinned dependencies are versioned
//! records, not directory structure. Native provider references appear only in
//! [`ImportRecord`] and [`ExportRecord`]. One [`AuthorityState`] per repository
//! names a single publication owner and refuses stale publications.
//!
//! The module is pure and optional. Nothing here reads files, clocks or the
//! network, native-source repositories never depend on it, and no record carries
//! a capability, grant or delegation. A deserialized [`GraphSnapshot`] is a claim;
//! only [`verify_snapshot`] turns it back into a [`SourceGraph`].
//!
//! r[impl kiln.source.graph.identity]
//! r[impl kiln.source.graph.relations]

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::monorepo::{finish, update_field, update_usize};
use crate::paths;
use crate::types::{ContentId, Diagnostic};

mod authority;
mod interchange;

pub use authority::*;
pub use interchange::*;

/// Schema of a serialized [`GraphSnapshot`] and of its root identity.
pub const SNAPSHOT_SCHEMA: &str = "kiln.source-graph.snapshot.v1";
const FILE_ID_SCHEMA: &str = "kiln.source-graph.file-id.v1";
const PACKAGE_ID_SCHEMA: &str = "kiln.source-graph.package-id.v1";
const REVISION_SCHEMA: &str = "kiln.source-graph.revision.v1";

/// Packages in one graph.
pub const MAX_PACKAGES: usize = 1_024;
/// File entities, deleted ones included, in one graph.
pub const MAX_ENTITIES: usize = 65_536;
/// Memberships, aliases included, in one graph.
pub const MAX_MEMBERSHIPS: usize = 131_072;
/// Pinned dependency edges in one graph.
pub const MAX_DEPENDENCIES: usize = 4_096;
/// Bytes in one package name, package-relative name or tree path.
pub const MAX_NAME_BYTES: usize = 4_096;
/// Bytes in one principal, layout or component text field.
pub const MAX_TEXT_BYTES: usize = 256;
/// Diagnostics reported by one admission before a truncation marker.
pub const MAX_DIAGNOSTICS: usize = 64;

/// Diagnostics collected by one admission, reported sorted and bounded.
#[derive(Debug, Default)]
struct Findings(Vec<Diagnostic>);

impl Findings {
    fn push(&mut self, code: &str, field: impl Into<String>, message: &str) {
        self.0.push(Diagnostic {
            code: code.to_string(),
            field: field.into(),
            message: message.to_string(),
        });
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sorted by code, field and message, so input order never changes them.
    fn into_sorted(self) -> Vec<Diagnostic> {
        let mut diagnostics = self.0;
        diagnostics
            .sort_by(|a, b| (&a.code, &a.field, &a.message).cmp(&(&b.code, &b.field, &b.message)));
        diagnostics.dedup();
        if diagnostics.len() > MAX_DIAGNOSTICS {
            diagnostics.truncate(MAX_DIAGNOSTICS);
            diagnostics.push(Diagnostic::new(
                "diagnostics_truncated",
                "diagnostics",
                "further diagnostics were omitted at the named bound",
            ));
        }
        diagnostics
    }

    fn finish<T>(self, value: T) -> Result<T, Vec<Diagnostic>> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self.into_sorted())
        }
    }
}

fn refuse<T>(code: &str, field: impl Into<String>, message: &str) -> Result<T, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    findings.push(code, field, message);
    Err(findings.into_sorted())
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

/// A canonical relative name within the name bound.
fn valid_name(name: &str) -> bool {
    name.len() <= MAX_NAME_BYTES && paths::canonical(name)
}

fn check_identity(id: &ContentId, field: &str) -> Result<(), Vec<Diagnostic>> {
    if id.validate().is_err() {
        return refuse(
            "invalid_identity",
            field,
            "identity must be a 64-character BLAKE3 digest",
        );
    }
    Ok(())
}

fn check_bound(
    current: usize,
    adding: usize,
    max: usize,
    field: &str,
) -> Result<(), Vec<Diagnostic>> {
    if current.checked_add(adding).is_none_or(|total| total > max) {
        return refuse(
            "bound_exceeded",
            field,
            "the named graph bound would be exceeded",
        );
    }
    Ok(())
}

macro_rules! graph_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(ContentId);

        impl $name {
            /// The BLAKE3 identity.
            #[must_use]
            pub fn as_content_id(&self) -> &ContentId {
                &self.0
            }

            /// The hexadecimal identity.
            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

graph_id!(
    /// Stable logical identity of one file entity, minted once and never derived
    /// from its bytes or path.
    FileId
);
graph_id!(
    /// Stable logical identity of one package, minted once.
    PackageId
);
graph_id!(
    /// Identity of one immutable entity revision.
    RevisionId
);

/// The operation that creates entities and its per-operation sequence number.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mint {
    pub operation: ContentId,
    pub sequence: u32,
}

fn minted(schema: &str, repository: &ContentId, mint: &Mint) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", schema.as_bytes());
    update_field(&mut hasher, "repository", repository.as_str().as_bytes());
    update_field(&mut hasher, "operation", mint.operation.as_str().as_bytes());
    update_usize(
        &mut hasher,
        "sequence",
        usize::try_from(mint.sequence).unwrap_or(usize::MAX),
    );
    finish(hasher)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMode {
    Regular,
    Executable,
}

impl FileMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Executable => "executable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RevisionState {
    Content { content: ContentId, mode: FileMode },
    Tombstone,
}

/// One immutable revision of one entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRevision {
    pub id: RevisionId,
    pub file: FileId,
    pub parent: Option<RevisionId>,
    pub state: RevisionState,
}

impl EntityRevision {
    fn new(file: FileId, parent: Option<RevisionId>, state: RevisionState) -> Self {
        let id = revision_id(&file, parent.as_ref(), &state);
        Self {
            id,
            file,
            parent,
            state,
        }
    }

    /// Bytes and mode, or `None` for a tombstone.
    #[must_use]
    pub fn content(&self) -> Option<(&ContentId, FileMode)> {
        match &self.state {
            RevisionState::Content { content, mode } => Some((content, *mode)),
            RevisionState::Tombstone => None,
        }
    }
}

fn revision_id(file: &FileId, parent: Option<&RevisionId>, state: &RevisionState) -> RevisionId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", REVISION_SCHEMA.as_bytes());
    update_field(&mut hasher, "file", file.as_str().as_bytes());
    match parent {
        Some(parent) => update_field(&mut hasher, "parent", parent.as_str().as_bytes()),
        None => update_field(&mut hasher, "initial", &[]),
    }
    match state {
        RevisionState::Content { content, mode } => {
            update_field(&mut hasher, "content", content.as_str().as_bytes());
            update_field(&mut hasher, "mode", mode.as_str().as_bytes());
        }
        RevisionState::Tombstone => update_field(&mut hasher, "tombstone", &[]),
    }
    RevisionId(finish(hasher))
}

/// How an entity came to exist.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provenance {
    /// Created in the graph.
    Created,
    /// Minted by a native import whose continuity is not asserted.
    Imported,
    /// A copy: a new identity whose first revision repeats a source revision.
    Copied { file: FileId, revision: RevisionId },
    /// A member of a forked package.
    Forked {
        package: PackageId,
        file: FileId,
        revision: RevisionId,
    },
}

/// One file entity and its current revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub id: FileId,
    pub mint: Mint,
    pub provenance: Provenance,
    pub revision: EntityRevision,
}

impl Entity {
    /// Whether an explicit deletion wrote a tombstone revision.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        matches!(self.revision.state, RevisionState::Tombstone)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub id: PackageId,
    pub mint: Mint,
    pub name: String,
    pub forked_from: Option<PackageId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipKind {
    /// The membership created with the entity.
    Primary,
    /// An additional membership of the same entity.
    Alias,
}

impl MembershipKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Alias => "alias",
        }
    }
}

/// A package-relative name for one entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub package: PackageId,
    pub name: String,
    pub file: FileId,
    pub kind: MembershipKind,
}

/// A package edge with an exact pinned selection, such as a snapshot root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub from: PackageId,
    pub to: PackageId,
    pub selection: ContentId,
}

/// The distinct result of one graph operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    PackageCreated {
        package: PackageId,
    },
    Created {
        file: FileId,
        revision: RevisionId,
    },
    Edited {
        file: FileId,
        revision: RevisionId,
    },
    Renamed {
        file: FileId,
        package: PackageId,
        from: String,
        to: String,
    },
    Aliased {
        file: FileId,
        package: PackageId,
        name: String,
    },
    Copied {
        file: FileId,
        source: FileId,
        revision: RevisionId,
    },
    Forked {
        package: PackageId,
        source: PackageId,
        /// `(source entity, new entity)` in source membership order.
        files: Vec<(FileId, FileId)>,
    },
    /// A primary membership was removed; the entity remains.
    MembershipRemoved {
        file: FileId,
        remaining: usize,
    },
    /// An alias was removed; the entity remains.
    AliasRemoved {
        file: FileId,
        remaining: usize,
    },
    /// The entity was explicitly deleted with a tombstone revision.
    Deleted {
        file: FileId,
        revision: RevisionId,
        memberships: usize,
    },
    DependencyPinned {
        from: PackageId,
        to: PackageId,
        selection: ContentId,
    },
}

/// One pinned graph state: packages, entities with their current revisions,
/// memberships and dependencies. Only its operations, [`SourceGraph::new`],
/// [`verify_snapshot`] and an import construct one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceGraph {
    repository: ContentId,
    packages: BTreeMap<PackageId, Package>,
    entities: BTreeMap<FileId, Entity>,
    memberships: BTreeMap<PackageId, BTreeMap<String, Membership>>,
    by_file: BTreeMap<FileId, BTreeMap<PackageId, BTreeSet<String>>>,
    dependencies: BTreeMap<(PackageId, PackageId), Dependency>,
}

impl SourceGraph {
    /// An empty graph for one admitted repository identity.
    pub fn new(repository: ContentId) -> Result<Self, Vec<Diagnostic>> {
        check_identity(&repository, "repository")?;
        Ok(Self {
            repository,
            packages: BTreeMap::new(),
            entities: BTreeMap::new(),
            memberships: BTreeMap::new(),
            by_file: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }

    #[must_use]
    pub fn package(&self, id: &PackageId) -> Option<&Package> {
        self.packages.get(id)
    }

    #[must_use]
    pub fn entity(&self, id: &FileId) -> Option<&Entity> {
        self.entities.get(id)
    }

    #[must_use]
    pub fn membership(&self, package: &PackageId, name: &str) -> Option<&Membership> {
        self.memberships.get(package)?.get(name)
    }

    pub fn packages(&self) -> impl Iterator<Item = &Package> {
        self.packages.values()
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    /// Every membership in package, then name, order.
    pub fn memberships(&self) -> impl Iterator<Item = &Membership> {
        self.memberships.values().flat_map(BTreeMap::values)
    }

    /// Memberships of one package in name order.
    pub fn package_memberships(&self, package: &PackageId) -> impl Iterator<Item = &Membership> {
        self.memberships
            .get(package)
            .into_iter()
            .flat_map(BTreeMap::values)
    }

    /// Memberships of one entity in package, then name, order.
    pub fn memberships_of<'a>(
        &'a self,
        file: &FileId,
    ) -> impl Iterator<Item = &'a Membership> + 'a {
        self.by_file
            .get(file)
            .into_iter()
            .flat_map(|packages| packages.iter())
            .flat_map(move |(package, names)| {
                names
                    .iter()
                    .filter_map(move |name| self.membership(package, name))
            })
    }

    pub fn dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies.values()
    }

    fn membership_total(&self) -> usize {
        self.memberships.values().map(BTreeMap::len).sum()
    }

    fn membership_count(&self, file: &FileId) -> usize {
        self.by_file
            .get(file)
            .map_or(0, |packages| packages.values().map(BTreeSet::len).sum())
    }

    fn live_entity(&self, file: &FileId) -> Result<&Entity, Vec<Diagnostic>> {
        let Some(entity) = self.entities.get(file) else {
            return refuse(
                "unknown_entity",
                "file",
                "the entity does not exist in this graph",
            );
        };
        if entity.is_deleted() {
            return refuse(
                "entity_deleted",
                "file",
                "the entity was explicitly deleted",
            );
        }
        Ok(entity)
    }

    fn check_mint(&self, mint: &Mint, schema: &str) -> Result<ContentId, Vec<Diagnostic>> {
        check_identity(&mint.operation, "mint.operation")?;
        Ok(minted(schema, &self.repository, mint))
    }

    fn check_free_name(&self, package: &PackageId, name: &str) -> Result<(), Vec<Diagnostic>> {
        if !self.packages.contains_key(package) {
            return refuse(
                "unknown_package",
                "package",
                "the package does not exist in this graph",
            );
        }
        if !valid_name(name) {
            return refuse(
                "invalid_name",
                "name",
                "a package-relative name must be a canonical relative path within the name bound",
            );
        }
        if self.membership(package, name).is_some() {
            return refuse(
                "name_taken",
                "name",
                "the package already has a member with this name",
            );
        }
        Ok(())
    }

    fn insert_membership(
        &mut self,
        package: PackageId,
        name: String,
        file: FileId,
        kind: MembershipKind,
    ) {
        self.by_file
            .entry(file.clone())
            .or_default()
            .entry(package.clone())
            .or_default()
            .insert(name.clone());
        self.memberships.entry(package.clone()).or_default().insert(
            name.clone(),
            Membership {
                package,
                name,
                file,
                kind,
            },
        );
    }

    fn remove_membership_record(&mut self, package: &PackageId, name: &str) -> Option<Membership> {
        let names = self.memberships.get_mut(package)?;
        let membership = names.remove(name)?;
        if names.is_empty() {
            self.memberships.remove(package);
        }
        if let Some(packages) = self.by_file.get_mut(&membership.file) {
            if let Some(names) = packages.get_mut(package) {
                names.remove(name);
                if names.is_empty() {
                    packages.remove(package);
                }
            }
            if packages.is_empty() {
                self.by_file.remove(&membership.file);
            }
        }
        Some(membership)
    }

    fn insert_entity(
        &mut self,
        id: FileId,
        mint: Mint,
        provenance: Provenance,
        content: ContentId,
        mode: FileMode,
    ) -> RevisionId {
        let revision =
            EntityRevision::new(id.clone(), None, RevisionState::Content { content, mode });
        let revision_id = revision.id.clone();
        self.entities.insert(
            id.clone(),
            Entity {
                id,
                mint,
                provenance,
                revision,
            },
        );
        revision_id
    }

    fn new_file_id(&self, mint: &Mint) -> Result<FileId, Vec<Diagnostic>> {
        let id = FileId(self.check_mint(mint, FILE_ID_SCHEMA)?);
        if self.entities.contains_key(&id) {
            return refuse(
                "identity_already_minted",
                "mint",
                "this operation and sequence number already minted an entity",
            );
        }
        Ok(id)
    }

    /// Mint one package with a unique name.
    pub fn create_package(&mut self, mint: Mint, name: &str) -> Result<Outcome, Vec<Diagnostic>> {
        let id = PackageId(self.check_mint(&mint, PACKAGE_ID_SCHEMA)?);
        if !valid_name(name) {
            return refuse(
                "invalid_name",
                "package.name",
                "a package name must be a canonical relative path",
            );
        }
        if self.packages.contains_key(&id) {
            return refuse(
                "identity_already_minted",
                "mint",
                "this operation and sequence number already minted a package",
            );
        }
        if self.packages.values().any(|package| package.name == name) {
            return refuse(
                "package_name_taken",
                "package.name",
                "another package has this name",
            );
        }
        check_bound(self.packages.len(), 1, MAX_PACKAGES, "packages")?;
        self.packages.insert(
            id.clone(),
            Package {
                id: id.clone(),
                mint,
                name: name.to_string(),
                forked_from: None,
            },
        );
        Ok(Outcome::PackageCreated { package: id })
    }

    /// Mint one entity with its first revision and primary membership.
    pub fn create_file(
        &mut self,
        mint: Mint,
        package: &PackageId,
        name: &str,
        content: ContentId,
        mode: FileMode,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        check_identity(&content, "content")?;
        let id = self.new_file_id(&mint)?;
        self.check_free_name(package, name)?;
        check_bound(self.entities.len(), 1, MAX_ENTITIES, "entities")?;
        check_bound(self.membership_total(), 1, MAX_MEMBERSHIPS, "memberships")?;
        let revision = self.insert_entity(id.clone(), mint, Provenance::Created, content, mode);
        self.insert_membership(
            package.clone(),
            name.to_string(),
            id.clone(),
            MembershipKind::Primary,
        );
        Ok(Outcome::Created { file: id, revision })
    }

    /// Record a new revision of the same entity.
    pub fn edit(
        &mut self,
        file: &FileId,
        content: ContentId,
        mode: FileMode,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        check_identity(&content, "content")?;
        let state = RevisionState::Content { content, mode };
        let entity = self.live_entity(file)?;
        if entity.revision.state == state {
            return refuse(
                "revision_unchanged",
                "content",
                "an edit must change the bytes or the mode",
            );
        }
        let revision = EntityRevision::new(file.clone(), Some(entity.revision.id.clone()), state);
        let revision_id = revision.id.clone();
        if let Some(entity) = self.entities.get_mut(file) {
            entity.revision = revision;
        }
        Ok(Outcome::Edited {
            file: file.clone(),
            revision: revision_id,
        })
    }

    // r[impl kiln.source.graph.identity.scenario.rename]
    /// Replace one membership name. The entity keeps its identity and revision.
    pub fn rename(
        &mut self,
        package: &PackageId,
        from: &str,
        to: &str,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        let Some(membership) = self.membership(package, from) else {
            return refuse(
                "unknown_membership",
                "from",
                "the package has no member with this name",
            );
        };
        let (file, kind) = (membership.file.clone(), membership.kind);
        self.check_free_name(package, to)?;
        self.remove_membership_record(package, from);
        self.insert_membership(package.clone(), to.to_string(), file.clone(), kind);
        Ok(Outcome::Renamed {
            file,
            package: package.clone(),
            from: from.to_string(),
            to: to.to_string(),
        })
    }

    /// Add another membership of the same entity.
    pub fn alias(
        &mut self,
        file: &FileId,
        package: &PackageId,
        name: &str,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        self.live_entity(file)?;
        self.check_free_name(package, name)?;
        check_bound(self.membership_total(), 1, MAX_MEMBERSHIPS, "memberships")?;
        self.insert_membership(
            package.clone(),
            name.to_string(),
            file.clone(),
            MembershipKind::Alias,
        );
        Ok(Outcome::Aliased {
            file: file.clone(),
            package: package.clone(),
            name: name.to_string(),
        })
    }

    /// Mint a new entity whose first revision repeats the source's current bytes.
    pub fn copy(
        &mut self,
        mint: Mint,
        source: &FileId,
        package: &PackageId,
        name: &str,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        let entity = self.live_entity(source)?;
        let Some((content, mode)) = entity.revision.content() else {
            return refuse(
                "entity_deleted",
                "source",
                "the entity was explicitly deleted",
            );
        };
        let (content, source_revision) = (content.clone(), entity.revision.id.clone());
        let id = self.new_file_id(&mint)?;
        self.check_free_name(package, name)?;
        check_bound(self.entities.len(), 1, MAX_ENTITIES, "entities")?;
        check_bound(self.membership_total(), 1, MAX_MEMBERSHIPS, "memberships")?;
        let provenance = Provenance::Copied {
            file: source.clone(),
            revision: source_revision,
        };
        let revision = self.insert_entity(id.clone(), mint, provenance, content, mode);
        self.insert_membership(
            package.clone(),
            name.to_string(),
            id.clone(),
            MembershipKind::Primary,
        );
        Ok(Outcome::Copied {
            file: id,
            source: source.clone(),
            revision,
        })
    }

    /// Copy a package: one new package identity (sequence 0) and one new entity
    /// for every distinct member (sequences 1..), each with fork provenance.
    pub fn fork_package(
        &mut self,
        operation: ContentId,
        source: &PackageId,
        name: &str,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        if !self.packages.contains_key(source) {
            return refuse(
                "unknown_package",
                "source",
                "the package does not exist in this graph",
            );
        }
        let package_mint = Mint {
            operation: operation.clone(),
            sequence: 0,
        };
        let package = PackageId(self.check_mint(&package_mint, PACKAGE_ID_SCHEMA)?);
        if !valid_name(name) {
            return refuse(
                "invalid_name",
                "package.name",
                "a package name must be a canonical relative path",
            );
        }
        if self.packages.contains_key(&package) {
            return refuse(
                "identity_already_minted",
                "mint",
                "this operation already minted a package",
            );
        }
        if self.packages.values().any(|existing| existing.name == name) {
            return refuse(
                "package_name_taken",
                "package.name",
                "another package has this name",
            );
        }
        let members: Vec<&Membership> = self.package_memberships(source).collect();
        let mut forks: Vec<(FileId, FileId, Mint)> = Vec::new();
        for member in &members {
            if forks
                .iter()
                .any(|(original, _, _)| *original == member.file)
            {
                continue;
            }
            let Some(sequence) = forks
                .len()
                .checked_add(1)
                .and_then(|next| u32::try_from(next).ok())
            else {
                return refuse(
                    "bound_exceeded",
                    "entities",
                    "the named graph bound would be exceeded",
                );
            };
            let mint = Mint {
                operation: operation.clone(),
                sequence,
            };
            let id = self.new_file_id(&mint)?;
            forks.push((member.file.clone(), id, mint));
        }
        check_bound(self.packages.len(), 1, MAX_PACKAGES, "packages")?;
        check_bound(self.entities.len(), forks.len(), MAX_ENTITIES, "entities")?;
        check_bound(
            self.membership_total(),
            members.len(),
            MAX_MEMBERSHIPS,
            "memberships",
        )?;
        let names: Vec<(String, FileId)> = members
            .iter()
            .map(|member| (member.name.clone(), member.file.clone()))
            .collect();
        let mut sources: Vec<(ContentId, FileMode, RevisionId)> = Vec::with_capacity(forks.len());
        for (original, _, _) in &forks {
            let entity = self.live_entity(original)?;
            let Some((content, mode)) = entity.revision.content() else {
                return refuse(
                    "entity_deleted",
                    "source",
                    "a member entity was explicitly deleted",
                );
            };
            sources.push((content.clone(), mode, entity.revision.id.clone()));
        }
        self.packages.insert(
            package.clone(),
            Package {
                id: package.clone(),
                mint: package_mint,
                name: name.to_string(),
                forked_from: Some(source.clone()),
            },
        );
        for ((original, id, mint), (content, mode, revision)) in forks.iter().zip(sources) {
            let provenance = Provenance::Forked {
                package: source.clone(),
                file: original.clone(),
                revision,
            };
            self.insert_entity(id.clone(), mint.clone(), provenance, content, mode);
        }
        let mut placed: BTreeSet<FileId> = BTreeSet::new();
        for (member_name, original) in names {
            let Some((_, id, _)) = forks
                .iter()
                .find(|(source_file, _, _)| *source_file == original)
            else {
                continue;
            };
            let kind = if placed.insert(id.clone()) {
                MembershipKind::Primary
            } else {
                MembershipKind::Alias
            };
            self.insert_membership(package.clone(), member_name, id.clone(), kind);
        }
        Ok(Outcome::Forked {
            package,
            source: source.clone(),
            files: forks
                .into_iter()
                .map(|(original, id, _)| (original, id))
                .collect(),
        })
    }

    // r[impl kiln.source.graph.relations.scenario.removal]
    /// Remove one membership or alias. The entity remains, even with no
    /// membership; only [`SourceGraph::delete`] removes it.
    pub fn remove_membership(
        &mut self,
        package: &PackageId,
        name: &str,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        let Some(membership) = self.remove_membership_record(package, name) else {
            return refuse(
                "unknown_membership",
                "name",
                "the package has no member with this name",
            );
        };
        let remaining = self.membership_count(&membership.file);
        Ok(match membership.kind {
            MembershipKind::Primary => Outcome::MembershipRemoved {
                file: membership.file,
                remaining,
            },
            MembershipKind::Alias => Outcome::AliasRemoved {
                file: membership.file,
                remaining,
            },
        })
    }

    /// Explicitly delete an entity: write a tombstone revision and remove every
    /// membership.
    pub fn delete(&mut self, file: &FileId) -> Result<Outcome, Vec<Diagnostic>> {
        let entity = self.live_entity(file)?;
        let revision = EntityRevision::new(
            file.clone(),
            Some(entity.revision.id.clone()),
            RevisionState::Tombstone,
        );
        let revision_id = revision.id.clone();
        let mut removed = 0_usize;
        if let Some(packages) = self.by_file.remove(file) {
            for (package, names) in packages {
                for name in names {
                    if let Some(member_names) = self.memberships.get_mut(&package) {
                        if member_names.remove(&name).is_some() {
                            removed = removed.saturating_add(1);
                        }
                        if member_names.is_empty() {
                            self.memberships.remove(&package);
                        }
                    }
                }
            }
        }
        if let Some(entity) = self.entities.get_mut(file) {
            entity.revision = revision;
        }
        Ok(Outcome::Deleted {
            file: file.clone(),
            revision: revision_id,
            memberships: removed,
        })
    }

    /// Pin, or re-pin, a package dependency to an exact selection.
    pub fn pin_dependency(
        &mut self,
        from: &PackageId,
        to: &PackageId,
        selection: ContentId,
    ) -> Result<Outcome, Vec<Diagnostic>> {
        check_identity(&selection, "selection")?;
        if !self.packages.contains_key(from) || !self.packages.contains_key(to) {
            return refuse(
                "unknown_package",
                "dependency",
                "both packages must exist in this graph",
            );
        }
        if from == to {
            return refuse(
                "self_dependency",
                "dependency",
                "a package cannot depend on itself",
            );
        }
        let key = (from.clone(), to.clone());
        if !self.dependencies.contains_key(&key) {
            check_bound(self.dependencies.len(), 1, MAX_DEPENDENCIES, "dependencies")?;
        }
        self.dependencies.insert(
            key,
            Dependency {
                from: from.clone(),
                to: to.clone(),
                selection: selection.clone(),
            },
        );
        Ok(Outcome::DependencyPinned {
            from: from.clone(),
            to: to.clone(),
            selection,
        })
    }

    /// The snapshot root: schema, repository, every package, entity revision,
    /// membership and pinned dependency in canonical order.
    #[must_use]
    pub fn root(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", SNAPSHOT_SCHEMA.as_bytes());
        update_field(
            &mut hasher,
            "repository",
            self.repository.as_str().as_bytes(),
        );
        update_usize(&mut hasher, "packages", self.packages.len());
        for package in self.packages.values() {
            update_field(&mut hasher, "package", package.id.as_str().as_bytes());
            update_mint(&mut hasher, &package.mint);
            update_field(&mut hasher, "name", package.name.as_bytes());
            update_option(
                &mut hasher,
                "forked_from",
                package.forked_from.as_ref().map(PackageId::as_str),
            );
        }
        update_usize(&mut hasher, "entities", self.entities.len());
        for entity in self.entities.values() {
            update_field(&mut hasher, "entity", entity.id.as_str().as_bytes());
            update_mint(&mut hasher, &entity.mint);
            update_provenance(&mut hasher, &entity.provenance);
            update_field(
                &mut hasher,
                "revision",
                entity.revision.id.as_str().as_bytes(),
            );
        }
        update_usize(&mut hasher, "memberships", self.membership_total());
        for membership in self.memberships() {
            update_field(
                &mut hasher,
                "package",
                membership.package.as_str().as_bytes(),
            );
            update_field(&mut hasher, "name", membership.name.as_bytes());
            update_field(&mut hasher, "file", membership.file.as_str().as_bytes());
            update_field(&mut hasher, "kind", membership.kind.as_str().as_bytes());
        }
        update_usize(&mut hasher, "dependencies", self.dependencies.len());
        for dependency in self.dependencies.values() {
            update_field(&mut hasher, "from", dependency.from.as_str().as_bytes());
            update_field(&mut hasher, "to", dependency.to.as_str().as_bytes());
            update_field(
                &mut hasher,
                "selection",
                dependency.selection.as_str().as_bytes(),
            );
        }
        finish(hasher)
    }

    /// The serializable claim for this exact state.
    #[must_use]
    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot {
            schema: SNAPSHOT_SCHEMA.to_string(),
            repository: self.repository.clone(),
            packages: self.packages.values().cloned().collect(),
            entities: self.entities.values().cloned().collect(),
            memberships: self.memberships().cloned().collect(),
            dependencies: self.dependencies.values().cloned().collect(),
            root: self.root(),
        }
    }
}

fn update_mint(hasher: &mut blake3::Hasher, mint: &Mint) {
    update_field(hasher, "operation", mint.operation.as_str().as_bytes());
    update_usize(
        hasher,
        "sequence",
        usize::try_from(mint.sequence).unwrap_or(usize::MAX),
    );
}

fn update_option(hasher: &mut blake3::Hasher, label: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            update_usize(hasher, label, 1);
            update_field(hasher, label, value.as_bytes());
        }
        None => update_usize(hasher, label, 0),
    }
}

fn update_provenance(hasher: &mut blake3::Hasher, provenance: &Provenance) {
    match provenance {
        Provenance::Created => update_field(hasher, "provenance", b"created"),
        Provenance::Imported => update_field(hasher, "provenance", b"imported"),
        Provenance::Copied { file, revision } => {
            update_field(hasher, "provenance", b"copied");
            update_field(hasher, "file", file.as_str().as_bytes());
            update_field(hasher, "revision", revision.as_str().as_bytes());
        }
        Provenance::Forked {
            package,
            file,
            revision,
        } => {
            update_field(hasher, "provenance", b"forked");
            update_field(hasher, "package", package.as_str().as_bytes());
            update_field(hasher, "file", file.as_str().as_bytes());
            update_field(hasher, "revision", revision.as_str().as_bytes());
        }
    }
}

/// A stored snapshot. Deserializing one yields a claim, not a graph; re-admit
/// it with [`verify_snapshot`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSnapshot {
    pub schema: String,
    pub repository: ContentId,
    pub packages: Vec<Package>,
    pub entities: Vec<Entity>,
    pub memberships: Vec<Membership>,
    pub dependencies: Vec<Dependency>,
    pub root: ContentId,
}

/// Re-admit a stored snapshot. Every package, entity and revision identity is
/// recomputed from its own fields, every relation must reference what the
/// snapshot holds, and the root must match. BLAKE3 is not a MAC: a writer that
/// recomputes every identity passes, so a verified graph is a reproduction of a
/// consistent claim, not proof of who wrote it.
pub fn verify_snapshot(snapshot: &GraphSnapshot) -> Result<SourceGraph, Vec<Diagnostic>> {
    if snapshot.packages.len() > MAX_PACKAGES
        || snapshot.entities.len() > MAX_ENTITIES
        || snapshot.memberships.len() > MAX_MEMBERSHIPS
        || snapshot.dependencies.len() > MAX_DEPENDENCIES
    {
        return refuse(
            "bound_exceeded",
            "snapshot",
            "the snapshot exceeds a named graph bound",
        );
    }
    let mut graph = SourceGraph::new(snapshot.repository.clone())?;
    let mut findings = Findings::default();
    if snapshot.schema != SNAPSHOT_SCHEMA {
        findings.push(
            "unsupported_schema",
            "schema",
            "the snapshot schema is not supported",
        );
    }
    check_identity(&snapshot.root, "root")
        .err()
        .into_iter()
        .flatten()
        .for_each(|diagnostic| findings.0.push(diagnostic));
    for package in &snapshot.packages {
        let valid_mint = package.mint.operation.validate().is_ok();
        if !valid_mint
            || package.id.0 != minted(PACKAGE_ID_SCHEMA, &graph.repository, &package.mint)
        {
            findings.push(
                "forged_identity",
                "packages",
                "a package identity does not match its mint",
            );
        }
        if !valid_name(&package.name)
            || graph
                .packages
                .values()
                .any(|other| other.name == package.name)
        {
            findings.push(
                "invalid_name",
                "packages",
                "package names must be unique canonical relative paths",
            );
        }
        if graph
            .packages
            .insert(package.id.clone(), package.clone())
            .is_some()
        {
            findings.push("duplicate_record", "packages", "a package appears twice");
        }
    }
    for package in &snapshot.packages {
        if package
            .forked_from
            .as_ref()
            .is_some_and(|source| !graph.packages.contains_key(source))
        {
            findings.push(
                "unknown_package",
                "packages",
                "a fork names a package the snapshot does not hold",
            );
        }
    }
    for entity in &snapshot.entities {
        let valid_mint = entity.mint.operation.validate().is_ok();
        if !valid_mint || entity.id.0 != minted(FILE_ID_SCHEMA, &graph.repository, &entity.mint) {
            findings.push(
                "forged_identity",
                "entities",
                "an entity identity does not match its mint",
            );
        }
        let revision = &entity.revision;
        let content_valid = revision
            .content()
            .is_none_or(|(content, _)| content.validate().is_ok());
        if revision.file != entity.id
            || !content_valid
            || revision.id != revision_id(&revision.file, revision.parent.as_ref(), &revision.state)
        {
            findings.push(
                "forged_revision",
                "entities",
                "a revision identity does not match its fields",
            );
        }
        if graph
            .entities
            .insert(entity.id.clone(), entity.clone())
            .is_some()
        {
            findings.push("duplicate_record", "entities", "an entity appears twice");
        }
    }
    for entity in &snapshot.entities {
        let source = match &entity.provenance {
            Provenance::Copied { file, .. } | Provenance::Forked { file, .. } => Some(file),
            Provenance::Created | Provenance::Imported => None,
        };
        if source.is_some_and(|file| !graph.entities.contains_key(file)) {
            findings.push(
                "unknown_entity",
                "entities",
                "a provenance record names an entity the snapshot does not hold",
            );
        }
    }
    for membership in &snapshot.memberships {
        if !graph.packages.contains_key(&membership.package) || !valid_name(&membership.name) {
            findings.push(
                "invalid_membership",
                "memberships",
                "a membership needs a held package and a canonical name",
            );
            continue;
        }
        match graph.entities.get(&membership.file) {
            Some(entity) if !entity.is_deleted() => {}
            Some(_) => findings.push(
                "entity_deleted",
                "memberships",
                "a deleted entity cannot hold a membership",
            ),
            None => findings.push(
                "unknown_entity",
                "memberships",
                "a membership names an entity the snapshot does not hold",
            ),
        }
        if graph
            .membership(&membership.package, &membership.name)
            .is_some()
        {
            findings.push(
                "duplicate_record",
                "memberships",
                "a package member name appears twice",
            );
            continue;
        }
        graph.insert_membership(
            membership.package.clone(),
            membership.name.clone(),
            membership.file.clone(),
            membership.kind,
        );
    }
    for dependency in &snapshot.dependencies {
        let key = (dependency.from.clone(), dependency.to.clone());
        if !graph.packages.contains_key(&dependency.from)
            || !graph.packages.contains_key(&dependency.to)
            || dependency.from == dependency.to
            || dependency.selection.validate().is_err()
        {
            findings.push(
                "invalid_dependency",
                "dependencies",
                "a dependency needs two distinct held packages and an exact selection",
            );
        }
        if graph.dependencies.insert(key, dependency.clone()).is_some() {
            findings.push(
                "duplicate_record",
                "dependencies",
                "a dependency edge appears twice",
            );
        }
    }
    if findings.is_empty() && graph.root() != snapshot.root {
        findings.push(
            "snapshot_root_mismatch",
            "root",
            "the recorded root does not match the snapshot's records",
        );
    }
    findings.finish(graph)
}

#[cfg(test)]
mod tests;
