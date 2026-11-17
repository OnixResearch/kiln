//! Source projections over one exact repository snapshot.
//!
//! A projection selects files from one exact source snapshot, lays them out at
//! destinations and records where each destination came from. This module is
//! the trusted, pure contract: [`admit_repositories`] gives repositories a
//! stable identity, [`admit_definition`] admits a typed definition and
//! [`resolve`] turns supplied exact facts into a frozen
//! [`ResolvedProjectionManifest`]. A shell supplies every fact; nothing here
//! reads files, clocks or the network.
//!
//! Visibility is not access. A manifest embeds no grant, token or proof, and
//! neither an [`AdmittedPartialView`] nor a [`CompleteProjection`] authorizes
//! reading, writing, delegating or publishing. The authority owner rechecks
//! current access when an effect is admitted.
//!
//! A complete workspace is a type, not a flag. Only
//! [`AdmittedPartialView::require_complete`] produces one:
//!
//! ```
//! use kiln_core::projection::{AdmittedPartialView, CompleteProjection, ProjectionOperation};
//!
//! fn build(_: &CompleteProjection) {}
//!
//! fn admit(view: AdmittedPartialView) {
//!     match view.require_complete(ProjectionOperation::Build) {
//!         Ok(complete) => build(&complete),
//!         Err(incomplete) => drop(incomplete.diagnostics),
//!     }
//! }
//! ```
//!
//! A partial view cannot stand in for a complete one:
//!
//! ```compile_fail,E0308
//! use kiln_core::projection::{AdmittedPartialView, CompleteProjection};
//!
//! fn build(_: &CompleteProjection) {}
//!
//! fn misuse(view: &AdmittedPartialView) {
//!     build(view);
//! }
//! ```
//!
//! Callers cannot construct the witness themselves:
//!
//! ```compile_fail,E0451
//! use kiln_core::projection::{AdmittedPartialView, CompleteProjection, ProjectionOperation};
//!
//! fn forge(view: AdmittedPartialView) -> CompleteProjection {
//!     CompleteProjection { view, operation: ProjectionOperation::Build }
//! }
//! ```
//!
//! Neither admitted type can be deserialized; a serialized manifest is only a
//! claim to verify with [`verify_manifest`]:
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<kiln_core::projection::AdmittedPartialView>("{}");
//! ```
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<kiln_core::projection::CompleteProjection>("{}");
//! ```

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::monorepo::{
    AdmittedScope, MAX_SCOPE_PATTERN_BYTES, ProjectScope, admit_scope, finish, update_field,
    update_set, update_usize,
};
use crate::paths;
use crate::types::{ContentId, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef, ProviderProfileId};

pub const REPOSITORY_IDENTITY_SCHEMA: &str = "kiln.repository-identity.v1";
pub const DEFINITION_SCHEMA: &str = "kiln.source-projection.definition.v1";
pub const MANIFEST_SCHEMA: &str = "kiln.source-projection.manifest.v1";
const DEFINITION_IDENTITY_SCHEMA: &str = "kiln.source-projection.definition-identity.v1";
const CONTENT_IDENTITY_SCHEMA: &str = "kiln.source-projection.content.v1";
const ORIGIN_IDENTITY_SCHEMA: &str = "kiln.source-projection.origin.v1";

/// Enrollments admitted in one repository registry.
pub const MAX_REPOSITORY_ENROLLMENTS: usize = 256;
/// Locator bindings per enrollment.
pub const MAX_LOCATORS: usize = 16;
/// Bytes per locator.
pub const MAX_LOCATOR_BYTES: usize = 4_096;
/// Binding reconciliations in one repository registry.
pub const MAX_RECONCILIATIONS: usize = 64;
/// Mount rules per definition.
pub const MAX_MOUNTS: usize = 32;
/// Declared dependencies per definition, and dependency facts per resolution.
pub const MAX_DEPENDENCIES: usize = 32;
/// Creation destinations per writable definition.
pub const MAX_CREATION_DESTINATIONS: usize = 16;
/// Source entries one resolution considers.
pub const MAX_SOURCE_ENTRIES: usize = 65_536;
/// Generated entries one resolution adds.
pub const MAX_GENERATED_ENTRIES: usize = 1_024;
/// Entries in one manifest, aliases included.
pub const MAX_MANIFEST_ENTRIES: usize = 65_536;
/// Largest admitted path, for sources and target profiles.
pub const MAX_TARGET_PATH_BYTES: usize = 4_096;
/// Diagnostics reported by one admission before a truncation marker.
pub const MAX_DIAGNOSTICS: usize = 64;
/// Characters of untrusted text echoed into one diagnostic field.
const MAX_SHOWN_CHARS: usize = 160;
/// Characters that make a path a glob rather than a literal.
const GLOB_CHARS: [char; 6] = ['*', '?', '[', ']', '{', '}'];
/// Scope admission needs a valid project id; this stands in for an invalid
/// projection id so selection faults are still reported.
const SELECTION_PLACEHOLDER_ID: &str = "projection";

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

/// Bounded, escaped rendering of untrusted text inside a diagnostic field.
fn shown(value: &str) -> String {
    let mut rendered: String = value
        .chars()
        .take(MAX_SHOWN_CHARS)
        .flat_map(char::escape_debug)
        .collect();
    if value.chars().nth(MAX_SHOWN_CHARS).is_some() {
        rendered.push('…');
    }
    rendered
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

/// A canonical relative path without glob metacharacters.
fn literal_path(value: &str) -> bool {
    paths::canonical(value) && !value.contains(GLOB_CHARS)
}

/// Empty (the root) or a literal path within the scope pattern bound.
fn literal_prefix(value: &str) -> bool {
    value.is_empty() || (value.len() <= MAX_SCOPE_PATTERN_BYTES && literal_path(value))
}

fn update_bool(hasher: &mut blake3::Hasher, label: &str, value: bool) {
    update_field(hasher, label, &[u8::from(value)]);
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

// ---------------------------------------------------------------------------
// Repository identity

/// A repository named by its source provider and provider-native reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBinding {
    pub source_provider: ProviderProfileId,
    pub native_repository: ExternalRef,
}

impl ProviderBinding {
    /// Revalidate a binding that may have arrived through serde.
    fn admissible(&self) -> bool {
        valid_text(self.source_provider.as_str(), DEFAULT_MAX_ID_BYTES)
            && self.native_repository.validate().is_ok()
    }

    // r[impl kiln.source.repository_identity]
    /// BLAKE3 identity of the named repository. Handles, locators and cache
    /// paths never enter it.
    #[must_use]
    pub fn identity(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", REPOSITORY_IDENTITY_SCHEMA.as_bytes());
        update_field(
            &mut hasher,
            "source_provider",
            self.source_provider.as_str().as_bytes(),
        );
        update_external(&mut hasher, "native_repository", &self.native_repository);
        finish(hasher)
    }
}

/// Where one repository can be read. Locators never enter repository identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepositoryLocator {
    /// An absolute local checkout or cache path.
    LocalPath { path: String },
    /// A remote URL or provider locator.
    Remote { url: String },
}

impl RepositoryLocator {
    fn admissible(&self) -> bool {
        match self {
            Self::LocalPath { path } => {
                path.len() <= MAX_LOCATOR_BYTES
                    && path.strip_prefix('/').is_some_and(paths::canonical)
            }
            Self::Remote { url } => {
                !url.is_empty()
                    && url.len() <= MAX_LOCATOR_BYTES
                    && !url.chars().any(|c| c.is_control() || c.is_whitespace())
            }
        }
    }
}

/// One enrolled repository as authored by an operator.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryEnrollment {
    /// Operator-chosen Kiln handle, used for display and selection only.
    pub repository_id: String,
    pub binding: ProviderBinding,
    #[serde(default)]
    pub locators: Vec<RepositoryLocator>,
}

/// Explicit operator record that one handle moved from `retired` to `current`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingReconciliation {
    pub repository_id: String,
    pub retired: ProviderBinding,
    pub current: ProviderBinding,
}

impl BindingReconciliation {
    fn names(&self, first: &ProviderBinding, second: &ProviderBinding) -> bool {
        (self.retired == *first && self.current == *second)
            || (self.retired == *second && self.current == *first)
    }
}

/// An admitted repository: one handle, its current provider binding and
/// identity, its admitted locators and any identity it superseded.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdmittedRepository {
    repository_id: String,
    binding: ProviderBinding,
    locators: BTreeSet<RepositoryLocator>,
    superseded: BTreeSet<ContentId>,
    identity: ContentId,
}

impl AdmittedRepository {
    #[must_use]
    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }
    #[must_use]
    pub fn binding(&self) -> &ProviderBinding {
        &self.binding
    }
    #[must_use]
    pub fn locators(&self) -> &BTreeSet<RepositoryLocator> {
        &self.locators
    }
    /// Identities of bindings this repository replaced through reconciliation.
    #[must_use]
    pub fn superseded(&self) -> &BTreeSet<ContentId> {
        &self.superseded
    }
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
}

fn admitted_repository(
    handle: &str,
    binding: &ProviderBinding,
    locators: BTreeSet<RepositoryLocator>,
    superseded: BTreeSet<ContentId>,
) -> AdmittedRepository {
    AdmittedRepository {
        repository_id: handle.to_string(),
        identity: binding.identity(),
        binding: binding.clone(),
        locators,
        superseded,
    }
}

// r[impl kiln.source.repository_identity.scenario.cache_move]
// r[impl kiln.source.repository_identity.scenario.conflict]
/// Admit a repository registry.
///
/// Enrollments with one handle and one binding merge their locators. Two
/// bindings under one handle need exactly one reconciliation naming both; the
/// current binding is admitted and the retired identity recorded as
/// superseded. One binding under two handles, more than two bindings under one
/// handle and reconciliations that match nothing are refused. The result is
/// ordered by handle.
pub fn admit_repositories(
    enrollments: &[RepositoryEnrollment],
    reconciliations: &[BindingReconciliation],
) -> Result<Vec<AdmittedRepository>, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if enrollments.len() > MAX_REPOSITORY_ENROLLMENTS {
        findings.push(
            "bound_exceeded",
            "repositories",
            "more enrollments than the named bound",
        );
    }
    if reconciliations.len() > MAX_RECONCILIATIONS {
        findings.push(
            "bound_exceeded",
            "reconciliations",
            "more reconciliations than the named bound",
        );
    }
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }
    let mut handles: BTreeMap<&str, BTreeMap<&ProviderBinding, BTreeSet<RepositoryLocator>>> =
        BTreeMap::new();
    for enrollment in enrollments {
        let field = format!("repositories.{}", shown(&enrollment.repository_id));
        if !valid_text(&enrollment.repository_id, DEFAULT_MAX_ID_BYTES) {
            findings.push(
                "invalid_repository_id",
                field,
                "repository handle must be non-empty bounded text without control characters",
            );
            continue;
        }
        if !enrollment.binding.admissible() {
            findings.push(
                "invalid_provider_binding",
                format!("{field}.binding"),
                "provider binding is empty, oversized or malformed",
            );
            continue;
        }
        if enrollment.locators.len() > MAX_LOCATORS {
            findings.push(
                "bound_exceeded",
                format!("{field}.locators"),
                "more locators than the named bound",
            );
            continue;
        }
        let mut locators = BTreeSet::new();
        for locator in &enrollment.locators {
            if locator.admissible() {
                locators.insert(locator.clone());
            } else {
                findings.push(
                    "invalid_locator",
                    format!("{field}.locators"),
                    "a local locator must be an absolute canonical path and a remote locator bounded text without whitespace",
                );
            }
        }
        handles
            .entry(enrollment.repository_id.as_str())
            .or_default()
            .entry(&enrollment.binding)
            .or_default()
            .extend(locators);
    }
    let mut matched = BTreeSet::new();
    let mut admitted = Vec::new();
    for (handle, bindings) in &handles {
        let field = format!("repositories.{}", shown(handle));
        let mut each = bindings.iter();
        match (each.next(), each.next(), each.next()) {
            (Some((binding, locators)), None, _) => admitted.push(admitted_repository(
                handle,
                binding,
                locators.clone(),
                BTreeSet::new(),
            )),
            (Some(first), Some(second), None) => {
                let candidates: Vec<(usize, &BindingReconciliation)> = reconciliations
                    .iter()
                    .enumerate()
                    .filter(|(_, reconciliation)| {
                        reconciliation.repository_id == *handle
                            && reconciliation.names(first.0, second.0)
                    })
                    .collect();
                if let [(index, reconciliation)] = candidates.as_slice() {
                    matched.insert(*index);
                    let (binding, locators) = if reconciliation.current == **first.0 {
                        first
                    } else {
                        second
                    };
                    admitted.push(admitted_repository(
                        handle,
                        binding,
                        locators.clone(),
                        BTreeSet::from([reconciliation.retired.identity()]),
                    ));
                } else {
                    findings.push(
                        "repository_binding_conflict",
                        field,
                        "one handle names two provider bindings without exactly one reconciliation naming both",
                    );
                }
            }
            _ => findings.push(
                "repository_binding_conflict",
                field,
                "one handle names more than two provider bindings",
            ),
        }
    }
    for (index, reconciliation) in reconciliations.iter().enumerate() {
        if !matched.contains(&index) {
            findings.push(
                "reconciliation_unmatched",
                format!("reconciliations.{}", shown(&reconciliation.repository_id)),
                "reconciliation does not name exactly the conflicting enrolled bindings",
            );
        }
    }
    {
        let mut owners: BTreeMap<&ContentId, BTreeSet<&str>> = BTreeMap::new();
        for repository in &admitted {
            for identity in core::iter::once(&repository.identity).chain(&repository.superseded) {
                owners
                    .entry(identity)
                    .or_default()
                    .insert(repository.repository_id.as_str());
            }
        }
        for handles in owners.values().filter(|handles| handles.len() > 1) {
            let names: Vec<String> = handles.iter().map(|handle| shown(handle)).collect();
            findings.push(
                "repository_alias_conflict",
                format!("repositories.{}", names.join(",")),
                "one provider binding is enrolled under several repository handles",
            );
        }
    }
    findings.finish(admitted)
}

// ---------------------------------------------------------------------------
// Projection definitions

/// What a projection may become.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionProfile {
    /// Browsing only; never complete build or write input.
    Navigation,
    /// Exact build or test input.
    Runnable,
    /// Exact input whose native entries may be edited through mapped writes.
    Writable,
}

impl ProjectionProfile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Navigation => "navigation",
            Self::Runnable => "runnable",
            Self::Writable => "writable",
        }
    }
}

/// Source capabilities a definition requires from the source adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCapability {
    /// Entries come from one exact tree with a canonical identity.
    ExactTree,
    /// Entry kinds preserve the executable bit.
    ExecutableBit,
    /// Symbolic links are reported as links, not followed.
    Symlinks,
}

impl SourceCapability {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExactTree => "exact_tree",
            Self::ExecutableBit => "executable_bit",
            Self::Symlinks => "symlinks",
        }
    }
}

/// Whether generated entries may count as build input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedInputs {
    Refused,
    /// Runnable-only: generated entries with explicit input identities count.
    Admitted,
}

impl GeneratedInputs {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Admitted => "admitted",
        }
    }
}

fn default_include() -> Vec<String> {
    vec!["**".to_string()]
}

/// Files selected from the repository, with monorepo scope semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Repository-relative directory; empty is the repository root.
    pub root: String,
    /// Globs relative to `root`.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    /// Globs relative to `root`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Every selected path under `source_prefix` appears below
/// `destination_prefix`. Empty prefixes name the repository or target root.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    pub source_prefix: String,
    pub destination_prefix: String,
}

/// A named input the projection needs from outside its selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyDeclaration {
    pub name: String,
    pub required: bool,
}

/// What the destination filesystem can represent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetProfile {
    pub case_sensitive: bool,
    pub max_path_bytes: usize,
    pub symlinks: bool,
    pub executable_bit: bool,
}

impl TargetProfile {
    /// A case-sensitive POSIX tree with symbolic links and executable bits.
    pub const POSIX: Self = Self {
        case_sensitive: true,
        max_path_bytes: MAX_TARGET_PATH_BYTES,
        symlinks: true,
        executable_bit: true,
    };

    const fn represents(self, kind: EntryKind) -> bool {
        match kind {
            EntryKind::File => true,
            EntryKind::Executable => self.executable_bit,
            EntryKind::Symlink => self.symlinks,
        }
    }
}

/// Explicitly labeled source of a definition that was not authored directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CompatibilityProfile {
    /// Converted from an admitted monorepo `ProjectScope`.
    #[serde(rename = "monorepo_project_scope.v1")]
    MonorepoProjectScopeV1,
}

impl CompatibilityProfile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MonorepoProjectScopeV1 => "monorepo_project_scope.v1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    pub profile: CompatibilityProfile,
    pub scope_identity: ContentId,
}

/// A projection definition as authored in Nickel and exported, before
/// admission. Nothing here is trusted until [`admit_definition`] accepts it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionDefinition {
    pub schema: String,
    pub id: String,
    pub profile: ProjectionProfile,
    pub selection: Selection,
    pub mounts: Vec<Mount>,
    #[serde(default)]
    pub dependencies: Vec<DependencyDeclaration>,
    #[serde(default)]
    pub creation_destinations: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<SourceCapability>,
    pub generated_inputs: GeneratedInputs,
    pub target: TargetProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<Compatibility>,
}

impl ProjectionDefinition {
    // r[impl kiln.source.projection.compatibility]
    /// The equivalent definition of an admitted monorepo scope: same root,
    /// include and exclude, an identity layout and no creation destinations.
    /// `depends_on` and graph kinds stay affected-project facts, not inputs.
    #[must_use]
    pub fn from_project_scope(scope: &AdmittedScope, profile: ProjectionProfile) -> Self {
        Self {
            schema: DEFINITION_SCHEMA.to_string(),
            id: scope.project_id().to_string(),
            profile,
            selection: Selection {
                root: scope.root().to_string(),
                include: scope.include().iter().cloned().collect(),
                exclude: scope.exclude().iter().cloned().collect(),
            },
            mounts: vec![Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            }],
            dependencies: Vec::new(),
            creation_destinations: Vec::new(),
            required_capabilities: if profile == ProjectionProfile::Navigation {
                Vec::new()
            } else {
                vec![SourceCapability::ExactTree]
            },
            generated_inputs: GeneratedInputs::Refused,
            target: TargetProfile::POSIX,
            compatibility: Some(Compatibility {
                profile: CompatibilityProfile::MonorepoProjectScopeV1,
                scope_identity: scope.identity().clone(),
            }),
        }
    }
}

/// A validated definition in canonical form, bound to its BLAKE3 identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdmittedDefinition {
    id: String,
    profile: ProjectionProfile,
    selection: AdmittedScope,
    mounts: BTreeSet<Mount>,
    dependencies: BTreeMap<String, bool>,
    creation_destinations: BTreeSet<String>,
    required_capabilities: BTreeSet<SourceCapability>,
    generated_inputs: GeneratedInputs,
    target: TargetProfile,
    compatibility: Option<Compatibility>,
    identity: ContentId,
}

impl AdmittedDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn profile(&self) -> ProjectionProfile {
        self.profile
    }
    #[must_use]
    pub fn target(&self) -> TargetProfile {
        self.target
    }
    #[must_use]
    pub fn compatibility(&self) -> Option<&Compatibility> {
        self.compatibility.as_ref()
    }
    /// Whether a repository-relative path is selected.
    #[must_use]
    pub fn selects(&self, path: &str) -> bool {
        self.selection.contains(path)
    }
    /// BLAKE3 identity over the canonical definition.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
}

// r[impl kiln.source.projection.definition.scenario.profile_rules]
/// Admit an authored definition.
///
/// Selection reuses monorepo scope admission. Unordered declarations become
/// sets, so their order never changes the identity. Creation destinations are
/// writable-only, runnable and writable profiles require `exact_tree`,
/// admitted generated inputs are runnable-only and the target must represent
/// every required capability.
pub fn admit_definition(
    definition: &ProjectionDefinition,
) -> Result<AdmittedDefinition, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if definition.schema != DEFINITION_SCHEMA {
        findings.push(
            "unsupported_schema",
            "schema",
            "projection definition schema is unsupported",
        );
    }
    let id_valid = valid_text(&definition.id, DEFAULT_MAX_ID_BYTES);
    if !id_valid {
        findings.push(
            "invalid_projection_id",
            "id",
            "projection id must be non-empty bounded text without control characters",
        );
    }
    let scope = ProjectScope {
        root: definition.selection.root.clone(),
        include: definition.selection.include.clone(),
        exclude: definition.selection.exclude.clone(),
        depends_on: Vec::new(),
        graphs: BTreeSet::new(),
    };
    let scope_id = if id_valid {
        definition.id.as_str()
    } else {
        SELECTION_PLACEHOLDER_ID
    };
    let selection = match admit_scope(scope_id, &scope) {
        Ok(selection) => Some(selection),
        Err(error) => {
            findings.push(
                &error.code,
                error.field.replacen("scope.", "selection.", 1),
                &error.message,
            );
            None
        }
    };
    let target = definition.target;
    if !(1..=MAX_TARGET_PATH_BYTES).contains(&target.max_path_bytes) {
        findings.push(
            "bound_exceeded",
            "target.max_path_bytes",
            "target path bound must be positive and within the named maximum",
        );
    }
    if definition.mounts.is_empty() {
        findings.push(
            "mounts_required",
            "mounts",
            "a projection needs a mount; empty source and destination prefixes give an identity layout",
        );
    }
    if definition.mounts.len() > MAX_MOUNTS {
        findings.push(
            "bound_exceeded",
            "mounts",
            "more mounts than the named bound",
        );
    }
    let mut mounts = BTreeSet::new();
    for mount in &definition.mounts {
        if literal_prefix(&mount.source_prefix) && literal_prefix(&mount.destination_prefix) {
            mounts.insert(mount.clone());
        } else {
            findings.push(
                "invalid_mount",
                format!("mounts.{}", shown(&mount.source_prefix)),
                "mount prefixes must be empty or literal canonical relative paths within the pattern byte bound",
            );
        }
    }
    if definition.dependencies.len() > MAX_DEPENDENCIES {
        findings.push(
            "bound_exceeded",
            "dependencies",
            "more dependencies than the named bound",
        );
    }
    let mut dependencies = BTreeMap::new();
    for dependency in &definition.dependencies {
        let field = format!("dependencies.{}", shown(&dependency.name));
        if !valid_text(&dependency.name, DEFAULT_MAX_ID_BYTES) {
            findings.push(
                "invalid_dependency",
                field,
                "dependency name must be non-empty bounded text without control characters",
            );
        } else if dependencies
            .insert(dependency.name.clone(), dependency.required)
            .is_some_and(|previous| previous != dependency.required)
        {
            findings.push(
                "dependency_conflict",
                field,
                "one dependency is declared both required and optional",
            );
        }
    }
    if definition.creation_destinations.len() > MAX_CREATION_DESTINATIONS {
        findings.push(
            "bound_exceeded",
            "creation_destinations",
            "more creation destinations than the named bound",
        );
    }
    if !definition.creation_destinations.is_empty()
        && definition.profile != ProjectionProfile::Writable
    {
        findings.push(
            "creation_destination_profile",
            "creation_destinations",
            "only a writable projection may declare creation destinations",
        );
    }
    let mut creation_destinations = BTreeSet::new();
    for destination in &definition.creation_destinations {
        if literal_path(destination) && destination.len() <= target.max_path_bytes {
            creation_destinations.insert(destination.clone());
        } else {
            findings.push(
                "invalid_creation_destination",
                format!("creation_destinations.{}", shown(destination)),
                "creation destination must be a literal canonical relative path within the target bound",
            );
        }
    }
    let required_capabilities: BTreeSet<SourceCapability> =
        definition.required_capabilities.iter().copied().collect();
    if definition.profile != ProjectionProfile::Navigation
        && !required_capabilities.contains(&SourceCapability::ExactTree)
    {
        findings.push(
            "exact_tree_required",
            "required_capabilities",
            "runnable and writable projections require an exact source tree",
        );
    }
    for (capability, represented) in [
        (SourceCapability::Symlinks, target.symlinks),
        (SourceCapability::ExecutableBit, target.executable_bit),
    ] {
        if required_capabilities.contains(&capability) && !represented {
            findings.push(
                "target_capability_conflict",
                format!("required_capabilities.{}", capability.as_str()),
                "the target cannot represent a required source capability",
            );
        }
    }
    if definition.generated_inputs == GeneratedInputs::Admitted
        && definition.profile != ProjectionProfile::Runnable
    {
        findings.push(
            "generated_inputs_profile",
            "generated_inputs",
            "only a runnable projection may admit generated inputs",
        );
    }
    if definition
        .compatibility
        .as_ref()
        .is_some_and(|compatibility| compatibility.scope_identity.validate().is_err())
    {
        findings.push(
            "invalid_compatibility",
            "compatibility.scope_identity",
            "compatibility scope identity must be a BLAKE3 identity",
        );
    }
    let (true, Some(selection)) = (findings.is_empty(), selection) else {
        return Err(findings.into_sorted());
    };
    let mut admitted = AdmittedDefinition {
        id: definition.id.clone(),
        profile: definition.profile,
        selection,
        mounts,
        dependencies,
        creation_destinations,
        required_capabilities,
        generated_inputs: definition.generated_inputs,
        target,
        compatibility: definition.compatibility.clone(),
        identity: definition_placeholder(),
    };
    admitted.identity = definition_identity(&admitted);
    Ok(admitted)
}

/// Overwritten by the computed identity before the definition leaves admission.
fn definition_placeholder() -> ContentId {
    finish(blake3::Hasher::new())
}

// r[impl kiln.source.projection.definition.scenario.canonical]
fn definition_identity(definition: &AdmittedDefinition) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", DEFINITION_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "id", definition.id.as_bytes());
    update_field(
        &mut hasher,
        "profile",
        definition.profile.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "selection.root",
        definition.selection.root().as_bytes(),
    );
    update_set(
        &mut hasher,
        "selection.include",
        definition.selection.include().iter().map(String::as_str),
    );
    update_set(
        &mut hasher,
        "selection.exclude",
        definition.selection.exclude().iter().map(String::as_str),
    );
    update_usize(&mut hasher, "mounts", definition.mounts.len());
    for mount in &definition.mounts {
        update_field(&mut hasher, "mount.source", mount.source_prefix.as_bytes());
        update_field(
            &mut hasher,
            "mount.destination",
            mount.destination_prefix.as_bytes(),
        );
    }
    update_usize(&mut hasher, "dependencies", definition.dependencies.len());
    for (name, required) in &definition.dependencies {
        update_field(&mut hasher, "dependency.name", name.as_bytes());
        update_bool(&mut hasher, "dependency.required", *required);
    }
    update_set(
        &mut hasher,
        "creation_destinations",
        definition.creation_destinations.iter().map(String::as_str),
    );
    update_set(
        &mut hasher,
        "required_capabilities",
        definition
            .required_capabilities
            .iter()
            .map(|capability| capability.as_str()),
    );
    update_field(
        &mut hasher,
        "generated_inputs",
        definition.generated_inputs.as_str().as_bytes(),
    );
    update_target(&mut hasher, definition.target);
    match &definition.compatibility {
        None => update_bool(&mut hasher, "compatibility", false),
        Some(compatibility) => {
            update_bool(&mut hasher, "compatibility", true);
            update_field(
                &mut hasher,
                "compatibility.profile",
                compatibility.profile.as_str().as_bytes(),
            );
            update_field(
                &mut hasher,
                "compatibility.scope_identity",
                compatibility.scope_identity.as_str().as_bytes(),
            );
        }
    }
    finish(hasher)
}

fn update_target(hasher: &mut blake3::Hasher, target: TargetProfile) {
    update_bool(hasher, "target.case_sensitive", target.case_sensitive);
    update_usize(hasher, "target.max_path_bytes", target.max_path_bytes);
    update_bool(hasher, "target.symlinks", target.symlinks);
    update_bool(hasher, "target.executable_bit", target.executable_bit);
}

// ---------------------------------------------------------------------------
// Facts and manifests

/// A named, versioned component: the evaluator, a source adapter or a generator.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentVersion {
    pub name: String,
    pub version: String,
}

impl ComponentVersion {
    fn admissible(&self) -> bool {
        valid_text(&self.name, DEFAULT_MAX_ID_BYTES)
            && valid_text(&self.version, DEFAULT_MAX_ID_BYTES)
    }
}

/// Whether the source adapter enumerated the whole selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    /// An enumeration bound was reached; entries may be missing.
    Truncated,
}

impl Coverage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Truncated => "truncated",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Executable,
    Symlink,
}

impl EntryKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Executable => "executable",
            Self::Symlink => "symlink",
        }
    }
}

/// One entry of the exact source tree, with its provider-native object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEntry {
    pub path: String,
    pub object: ExternalRef,
    pub kind: EntryKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DependencyStatus {
    /// Resolved to an exact identity.
    Present { identity: ContentId },
    /// No fact was supplied, or the owner reports it unavailable.
    Missing,
    /// The owner refused access to it.
    Denied,
}

impl DependencyStatus {
    fn update(&self, hasher: &mut blake3::Hasher) {
        match self {
            Self::Present { identity } => {
                update_field(hasher, "dependency.state", b"present");
                update_field(hasher, "dependency.identity", identity.as_str().as_bytes());
            }
            Self::Missing => update_field(hasher, "dependency.state", b"missing"),
            Self::Denied => update_field(hasher, "dependency.state", b"denied"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyFact {
    pub name: String,
    pub status: DependencyStatus,
}

/// An entry produced by a generator rather than read from the source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedEntry {
    pub destination: String,
    pub kind: EntryKind,
    pub generator: ComponentVersion,
    pub input_identity: ContentId,
}

/// Exact facts a shell observed for one resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionFacts {
    /// Identity of the admitted repository the facts were read from.
    pub repository: ContentId,
    pub exact_revision: ExternalRef,
    /// The existing canonical tree identity of `exact_revision`.
    pub canonical_tree: ContentId,
    pub evaluator: ComponentVersion,
    pub adapter: ComponentVersion,
    pub source_capabilities: Vec<SourceCapability>,
    pub coverage: Coverage,
    pub entries: Vec<SourceEntry>,
    #[serde(default)]
    pub dependencies: Vec<DependencyFact>,
    #[serde(default)]
    pub generated: Vec<GeneratedEntry>,
}

/// Where one destination came from.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryOrigin {
    /// A source path at the exact revision. `origin_identity` binds
    /// repository, revision, path, object and kind, so equal bytes at two
    /// paths remain two origins and an alias keeps its one origin.
    Native {
        path: String,
        object: ExternalRef,
        origin_identity: ContentId,
    },
    /// Output of a named generator over an exact input identity.
    Generated {
        generator: ComponentVersion,
        input_identity: ContentId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    pub destination: String,
    pub kind: EntryKind,
    pub origin: EntryOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyBinding {
    pub name: String,
    pub required: bool,
    pub status: DependencyStatus,
}

/// The frozen result of one resolution. Deserializing one yields a claim, not
/// an admitted view; check it with [`verify_manifest`] and re-resolve current
/// facts to admit it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedProjectionManifest {
    pub schema: String,
    pub repository: ContentId,
    pub exact_revision: ExternalRef,
    pub canonical_tree: ContentId,
    pub definition: ContentId,
    pub profile: ProjectionProfile,
    pub evaluator: ComponentVersion,
    pub adapter: ComponentVersion,
    pub source_capabilities: BTreeSet<SourceCapability>,
    pub target: TargetProfile,
    pub coverage: Coverage,
    /// Ordered by destination; one entry per destination.
    pub entries: Vec<ManifestEntry>,
    /// Ordered by name; one binding per declared dependency.
    pub dependencies: Vec<DependencyBinding>,
    /// Identity of the materialized content: destinations, kinds, objects or
    /// generated inputs, and dependency bindings.
    pub content_identity: ContentId,
    /// Identity of every other field.
    pub identity: ContentId,
}

fn origin_identity(
    repository: &ContentId,
    revision: &ExternalRef,
    path: &str,
    object: &ExternalRef,
    kind: EntryKind,
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", ORIGIN_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "repository", repository.as_str().as_bytes());
    update_external(&mut hasher, "revision", revision);
    update_field(&mut hasher, "path", path.as_bytes());
    update_external(&mut hasher, "object", object);
    update_field(&mut hasher, "kind", kind.as_str().as_bytes());
    finish(hasher)
}

fn update_component(hasher: &mut blake3::Hasher, label: &str, component: &ComponentVersion) {
    update_field(hasher, label, component.name.as_bytes());
    update_field(hasher, label, component.version.as_bytes());
}

fn update_dependencies(hasher: &mut blake3::Hasher, dependencies: &[DependencyBinding]) {
    update_usize(hasher, "dependencies", dependencies.len());
    for dependency in dependencies {
        update_field(hasher, "dependency.name", dependency.name.as_bytes());
        update_bool(hasher, "dependency.required", dependency.required);
        dependency.status.update(hasher);
    }
}

// r[impl kiln.source.projection.manifest.scenario.identity]
fn content_identity(entries: &[ManifestEntry], dependencies: &[DependencyBinding]) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", CONTENT_IDENTITY_SCHEMA.as_bytes());
    update_usize(&mut hasher, "entries", entries.len());
    for entry in entries {
        update_field(&mut hasher, "destination", entry.destination.as_bytes());
        update_field(&mut hasher, "kind", entry.kind.as_str().as_bytes());
        match &entry.origin {
            EntryOrigin::Native { object, .. } => {
                update_field(&mut hasher, "origin", b"native");
                update_external(&mut hasher, "object", object);
            }
            EntryOrigin::Generated {
                generator,
                input_identity,
            } => {
                update_field(&mut hasher, "origin", b"generated");
                update_component(&mut hasher, "generator", generator);
                update_field(&mut hasher, "input", input_identity.as_str().as_bytes());
            }
        }
    }
    update_dependencies(&mut hasher, dependencies);
    finish(hasher)
}

fn manifest_identity(manifest: &ResolvedProjectionManifest) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", manifest.schema.as_bytes());
    update_field(
        &mut hasher,
        "repository",
        manifest.repository.as_str().as_bytes(),
    );
    update_external(&mut hasher, "revision", &manifest.exact_revision);
    update_field(
        &mut hasher,
        "canonical_tree",
        manifest.canonical_tree.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "definition",
        manifest.definition.as_str().as_bytes(),
    );
    update_field(&mut hasher, "profile", manifest.profile.as_str().as_bytes());
    update_component(&mut hasher, "evaluator", &manifest.evaluator);
    update_component(&mut hasher, "adapter", &manifest.adapter);
    update_set(
        &mut hasher,
        "source_capabilities",
        manifest
            .source_capabilities
            .iter()
            .map(|capability| capability.as_str()),
    );
    update_target(&mut hasher, manifest.target);
    update_field(
        &mut hasher,
        "coverage",
        manifest.coverage.as_str().as_bytes(),
    );
    update_usize(&mut hasher, "entries", manifest.entries.len());
    for entry in &manifest.entries {
        update_field(&mut hasher, "destination", entry.destination.as_bytes());
        update_field(&mut hasher, "kind", entry.kind.as_str().as_bytes());
        match &entry.origin {
            EntryOrigin::Native {
                path,
                object,
                origin_identity,
            } => {
                update_field(&mut hasher, "origin", b"native");
                update_field(&mut hasher, "path", path.as_bytes());
                update_external(&mut hasher, "object", object);
                update_field(
                    &mut hasher,
                    "origin_identity",
                    origin_identity.as_str().as_bytes(),
                );
            }
            EntryOrigin::Generated {
                generator,
                input_identity,
            } => {
                update_field(&mut hasher, "origin", b"generated");
                update_component(&mut hasher, "generator", generator);
                update_field(&mut hasher, "input", input_identity.as_str().as_bytes());
            }
        }
    }
    update_dependencies(&mut hasher, &manifest.dependencies);
    update_field(
        &mut hasher,
        "content_identity",
        manifest.content_identity.as_str().as_bytes(),
    );
    finish(hasher)
}

/// Destinations of `path` under every mount whose source prefix contains it.
/// `None` marks a mount that would place a file at the target root itself.
fn mount_destinations(mounts: &BTreeSet<Mount>, path: &str) -> Vec<Option<String>> {
    mounts
        .iter()
        .filter_map(|mount| {
            let remainder = if mount.source_prefix.is_empty() {
                path
            } else if path == mount.source_prefix {
                ""
            } else {
                path.strip_prefix(mount.source_prefix.as_str())?
                    .strip_prefix('/')?
            };
            Some(
                match (mount.destination_prefix.is_empty(), remainder.is_empty()) {
                    (true, true) => None,
                    (true, false) => Some(remainder.to_string()),
                    (false, true) => Some(mount.destination_prefix.clone()),
                    (false, false) => Some(format!("{}/{remainder}", mount.destination_prefix)),
                },
            )
        })
        .collect()
}

/// Destinations placed so far. A destination holds one origin; an alias of
/// the same origin is idempotent.
#[derive(Debug, Default)]
struct Placement {
    entries: BTreeMap<String, (EntryKind, EntryOrigin)>,
    overflow: bool,
}

impl Placement {
    fn place(
        &mut self,
        findings: &mut Findings,
        destination: String,
        kind: EntryKind,
        origin: EntryOrigin,
    ) {
        if let Some((placed_kind, placed_origin)) = self.entries.get(&destination) {
            if *placed_kind != kind || *placed_origin != origin {
                findings.push(
                    "destination_collision",
                    format!("destinations.{}", shown(&destination)),
                    "two different origins map to one destination",
                );
            }
            return;
        }
        if self.entries.len() >= MAX_MANIFEST_ENTRIES {
            self.overflow = true;
            return;
        }
        self.entries.insert(destination, (kind, origin));
    }
}

/// Every proper ancestor directory of a `/`-separated path.
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/')
        .filter_map(move |(index, _)| path.get(..index))
}

/// Case-fold and file/directory conflicts among destinations, and creation
/// destinations that would land on or under a file.
fn layout_conflicts(
    findings: &mut Findings,
    placement: &Placement,
    creation_destinations: &BTreeSet<String>,
    case_sensitive: bool,
) {
    let fold = |path: &str| {
        if case_sensitive {
            path.to_string()
        } else {
            path.to_lowercase()
        }
    };
    let mut keys: BTreeMap<String, &str> = BTreeMap::new();
    for destination in placement.entries.keys() {
        let key = fold(destination);
        if let Some(previous) = keys.insert(key.clone(), destination.as_str()) {
            if previous != destination.as_str() {
                findings.push(
                    "case_fold_collision",
                    format!("destinations.{}", shown(&key)),
                    "two destinations differ only by case on a case-insensitive target",
                );
            }
        }
    }
    for key in keys.keys() {
        for ancestor in ancestors(key) {
            if keys.contains_key(ancestor) {
                findings.push(
                    "file_directory_collision",
                    format!("destinations.{}", shown(ancestor)),
                    "a destination file is also the parent directory of another destination",
                );
            }
        }
    }
    for destination in creation_destinations {
        let key = fold(destination);
        if keys.contains_key(&key) || ancestors(&key).any(|ancestor| keys.contains_key(ancestor)) {
            findings.push(
                "creation_destination_collision",
                format!("creation_destinations.{}", shown(destination)),
                "a creation destination is a projected file or lies under one",
            );
        }
    }
}

/// Dependency bindings for every declared dependency; an absent fact is
/// recorded as missing rather than assumed present.
fn bind_dependencies(
    findings: &mut Findings,
    declared: &BTreeMap<String, bool>,
    facts: &[DependencyFact],
) -> Vec<DependencyBinding> {
    if facts.len() > MAX_DEPENDENCIES {
        findings.push(
            "bound_exceeded",
            "dependencies",
            "more dependency facts than the named bound",
        );
    }
    let mut observed: BTreeMap<&str, &DependencyStatus> = BTreeMap::new();
    for fact in facts {
        let field = format!("dependencies.{}", shown(&fact.name));
        if !declared.contains_key(&fact.name) {
            findings.push(
                "undeclared_dependency_fact",
                field,
                "a dependency fact names no declared dependency",
            );
        } else if observed.insert(fact.name.as_str(), &fact.status).is_some() {
            findings.push(
                "duplicate_dependency_fact",
                field,
                "one dependency has several facts",
            );
        } else if let DependencyStatus::Present { identity } = &fact.status {
            if identity.validate().is_err() {
                findings.push(
                    "invalid_dependency_identity",
                    field,
                    "a present dependency needs a BLAKE3 identity",
                );
            }
        }
    }
    declared
        .iter()
        .map(|(name, required)| DependencyBinding {
            name: name.clone(),
            required: *required,
            status: observed
                .get(name.as_str())
                .map_or(DependencyStatus::Missing, |status| (*status).clone()),
        })
        .collect()
}

// r[impl kiln.source.projection.manifest]
// r[impl kiln.source.projection.manifest.scenario.reproducible]
// r[impl kiln.source.projection.manifest.scenario.diagnostics]
/// Resolve a definition over exact facts into an admitted partial view.
///
/// Source entries are considered in path order and mounts in canonical
/// order, so input order never changes the manifest or the diagnostics. Every
/// fault is reported before any effect: unsafe or duplicate paths, unmapped
/// selected paths, collisions, unsupported capabilities, stale repository
/// facts and bound violations.
pub fn resolve(
    definition: &AdmittedDefinition,
    repository: &AdmittedRepository,
    facts: &ProjectionFacts,
) -> Result<AdmittedPartialView, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if facts.entries.len() > MAX_SOURCE_ENTRIES {
        findings.push(
            "bound_exceeded",
            "entries",
            "more source entries than the named bound",
        );
    }
    if facts.generated.len() > MAX_GENERATED_ENTRIES {
        findings.push(
            "bound_exceeded",
            "generated",
            "more generated entries than the named bound",
        );
    }
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }
    if facts.repository != repository.identity {
        findings.push(
            "stale_repository",
            "repository",
            "facts were read from another repository identity",
        );
    }
    if facts.exact_revision.validate().is_err() {
        findings.push(
            "invalid_revision",
            "exact_revision",
            "exact revision is not a valid provider-native reference",
        );
    }
    if facts.canonical_tree.validate().is_err() {
        findings.push(
            "invalid_canonical_tree",
            "canonical_tree",
            "canonical tree must be a BLAKE3 identity",
        );
    }
    for (label, component) in [("evaluator", &facts.evaluator), ("adapter", &facts.adapter)] {
        if !component.admissible() {
            findings.push(
                "invalid_component",
                label,
                "component name and version must be non-empty bounded text",
            );
        }
    }
    let source_capabilities: BTreeSet<SourceCapability> =
        facts.source_capabilities.iter().copied().collect();
    for required in &definition.required_capabilities {
        if !source_capabilities.contains(required) {
            findings.push(
                "unsupported_source_capability",
                format!("required_capabilities.{}", required.as_str()),
                "the source adapter does not provide a required capability",
            );
        }
    }
    let mut sources: BTreeMap<&str, &SourceEntry> = BTreeMap::new();
    for entry in &facts.entries {
        let field = format!("entries.{}", shown(&entry.path));
        if !paths::canonical(&entry.path) || entry.path.len() > MAX_TARGET_PATH_BYTES {
            findings.push(
                "unsafe_source_path",
                field,
                "source path must be a canonical relative path within the named bound",
            );
        } else if entry.object.validate().is_err() {
            findings.push(
                "invalid_object_binding",
                field,
                "source object is not a valid provider-native reference",
            );
        } else if sources.insert(entry.path.as_str(), entry).is_some() {
            findings.push(
                "duplicate_source_path",
                field,
                "one source path appears several times",
            );
        }
    }
    let target = definition.target;
    let mut placement = Placement::default();
    for (path, entry) in &sources {
        if !definition.selection.contains(path) {
            continue;
        }
        let field = format!("entries.{}", shown(path));
        if !target.represents(entry.kind) {
            findings.push(
                "unsupported_target_capability",
                field,
                "the target cannot represent this entry kind",
            );
            continue;
        }
        let destinations = mount_destinations(&definition.mounts, path);
        if destinations.is_empty() {
            findings.push(
                "unmapped_selected_path",
                field,
                "a selected path lies under no mount",
            );
            continue;
        }
        let origin = EntryOrigin::Native {
            path: (*path).to_string(),
            object: entry.object.clone(),
            origin_identity: origin_identity(
                &repository.identity,
                &facts.exact_revision,
                path,
                &entry.object,
                entry.kind,
            ),
        };
        for destination in destinations {
            let Some(destination) = destination else {
                findings.push(
                    "empty_destination",
                    field.clone(),
                    "a mount would place a file at the target root",
                );
                continue;
            };
            if destination.len() > target.max_path_bytes {
                findings.push(
                    "destination_path_bound",
                    format!("destinations.{}", shown(&destination)),
                    "destination path exceeds the target bound",
                );
                continue;
            }
            placement.place(&mut findings, destination, entry.kind, origin.clone());
        }
    }
    for generated in &facts.generated {
        let field = format!("generated.{}", shown(&generated.destination));
        if !paths::canonical(&generated.destination)
            || generated.destination.len() > target.max_path_bytes
        {
            findings.push(
                "unsafe_generated_destination",
                field,
                "generated destination must be a canonical relative path within the target bound",
            );
        } else if !generated.generator.admissible() || generated.input_identity.validate().is_err()
        {
            findings.push(
                "invalid_generated_entry",
                field,
                "a generated entry needs a named generator and a BLAKE3 input identity",
            );
        } else if !target.represents(generated.kind) {
            findings.push(
                "unsupported_target_capability",
                field,
                "the target cannot represent this entry kind",
            );
        } else {
            placement.place(
                &mut findings,
                generated.destination.clone(),
                generated.kind,
                EntryOrigin::Generated {
                    generator: generated.generator.clone(),
                    input_identity: generated.input_identity.clone(),
                },
            );
        }
    }
    if placement.overflow {
        findings.push(
            "bound_exceeded",
            "entries",
            "more manifest entries than the named bound",
        );
    }
    layout_conflicts(
        &mut findings,
        &placement,
        &definition.creation_destinations,
        target.case_sensitive,
    );
    let dependencies =
        bind_dependencies(&mut findings, &definition.dependencies, &facts.dependencies);
    if !findings.is_empty() {
        return Err(findings.into_sorted());
    }
    let entries: Vec<ManifestEntry> = placement
        .entries
        .into_iter()
        .map(|(destination, (kind, origin))| ManifestEntry {
            destination,
            kind,
            origin,
        })
        .collect();
    let content = content_identity(&entries, &dependencies);
    let mut manifest = ResolvedProjectionManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        repository: repository.identity.clone(),
        exact_revision: facts.exact_revision.clone(),
        canonical_tree: facts.canonical_tree.clone(),
        definition: definition.identity.clone(),
        profile: definition.profile,
        evaluator: facts.evaluator.clone(),
        adapter: facts.adapter.clone(),
        source_capabilities,
        target,
        coverage: facts.coverage,
        entries,
        dependencies,
        identity: content.clone(),
        content_identity: content,
    };
    manifest.identity = manifest_identity(&manifest);
    Ok(AdmittedPartialView {
        manifest: Box::new(manifest),
        generated_inputs: definition.generated_inputs,
        creation_destinations: definition.creation_destinations.clone(),
    })
}

// ---------------------------------------------------------------------------
// Type states

/// What a completeness witness is requested for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionOperation {
    Build,
    Write,
}

// r[impl kiln.source.projection.completeness]
/// A resolved projection: exactly what the selection, layout and facts gave,
/// possibly truncated or missing inputs. Only [`resolve`] constructs one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedPartialView {
    manifest: Box<ResolvedProjectionManifest>,
    generated_inputs: GeneratedInputs,
    creation_destinations: BTreeSet<String>,
}

/// A refused completeness request, returning the unchanged view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncompleteProjection {
    pub view: AdmittedPartialView,
    pub diagnostics: Vec<Diagnostic>,
}

/// Witness that a view is structurally complete for one operation. It grants
/// no authority: reads, writes, delegation and publication are admitted by
/// their owners against current access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteProjection {
    view: AdmittedPartialView,
    operation: ProjectionOperation,
}

impl CompleteProjection {
    #[must_use]
    pub fn view(&self) -> &AdmittedPartialView {
        &self.view
    }
    #[must_use]
    pub fn manifest(&self) -> &ResolvedProjectionManifest {
        &self.view.manifest
    }
    #[must_use]
    pub fn operation(&self) -> ProjectionOperation {
        self.operation
    }
    #[must_use]
    pub fn into_view(self) -> AdmittedPartialView {
        self.view
    }
}

impl AdmittedPartialView {
    #[must_use]
    pub fn manifest(&self) -> &ResolvedProjectionManifest {
        &self.manifest
    }
    /// Destination directories where a writable projection may create files.
    #[must_use]
    pub fn creation_destinations(&self) -> &BTreeSet<String> {
        &self.creation_destinations
    }

    // r[impl kiln.source.projection.completeness.scenario.partial]
    /// Check this view for a build or write operation.
    ///
    /// Build needs a runnable or writable profile, write a writable one. Both
    /// need complete coverage and every required dependency present. Write
    /// refuses generated entries; build refuses them unless the definition
    /// admitted generated inputs. A refusal returns the view unchanged.
    pub fn require_complete(
        self,
        operation: ProjectionOperation,
    ) -> Result<CompleteProjection, IncompleteProjection> {
        let mut findings = Findings::default();
        let manifest = &self.manifest;
        let permitted = matches!(
            (operation, manifest.profile),
            (
                ProjectionOperation::Build,
                ProjectionProfile::Runnable | ProjectionProfile::Writable
            ) | (ProjectionOperation::Write, ProjectionProfile::Writable)
        );
        if !permitted {
            findings.push(
                "operation_not_permitted",
                "profile",
                "the projection profile does not permit this operation",
            );
        }
        if manifest.coverage != Coverage::Complete {
            findings.push(
                "incomplete_coverage",
                "coverage",
                "the source enumeration was truncated",
            );
        }
        for dependency in manifest.dependencies.iter().filter(|d| d.required) {
            let field = format!("dependencies.{}", shown(&dependency.name));
            match dependency.status {
                DependencyStatus::Present { .. } => {}
                DependencyStatus::Missing => findings.push(
                    "required_dependency_missing",
                    field,
                    "a required dependency is missing",
                ),
                DependencyStatus::Denied => findings.push(
                    "required_dependency_denied",
                    field,
                    "access to a required dependency was refused",
                ),
            }
        }
        for entry in &manifest.entries {
            if !matches!(entry.origin, EntryOrigin::Generated { .. }) {
                continue;
            }
            let field = format!("destinations.{}", shown(&entry.destination));
            if operation == ProjectionOperation::Write {
                findings.push(
                    "generated_entry_not_writable",
                    field,
                    "a generated entry cannot be writable source",
                );
            } else if self.generated_inputs == GeneratedInputs::Refused {
                findings.push(
                    "generated_input_not_admitted",
                    field,
                    "the definition does not admit generated build inputs",
                );
            }
        }
        if findings.is_empty() {
            Ok(CompleteProjection {
                view: self,
                operation,
            })
        } else {
            Err(IncompleteProjection {
                view: self,
                diagnostics: findings.into_sorted(),
            })
        }
    }
}

/// Bindings a stored manifest must still match, from trusted current state.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedProjection<'a> {
    pub repository: &'a ContentId,
    pub exact_revision: &'a ExternalRef,
    pub definition: &'a ContentId,
    pub evaluator: &'a ComponentVersion,
    pub adapter: &'a ComponentVersion,
}

// r[impl kiln.source.projection.completeness.scenario.forged]
/// Check a stored manifest for internal consistency and staleness.
///
/// Every origin, content and manifest identity is recomputed from the
/// manifest's own fields, so a changed field (a flipped coverage, a swapped
/// object) is reported. BLAKE3 is not a MAC: a writer that recomputes every
/// identity passes this check. That is why verification yields only a
/// verdict: only [`resolve`] over current facts produces an admitted view.
pub fn verify_manifest(
    manifest: &ResolvedProjectionManifest,
    expected: ExpectedProjection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if manifest.schema != MANIFEST_SCHEMA {
        findings.push(
            "unsupported_schema",
            "schema",
            "projection manifest schema is unsupported",
        );
    }
    if !manifest
        .entries
        .windows(2)
        .all(|pair| matches!(pair, [first, second] if first.destination < second.destination))
    {
        findings.push(
            "manifest_entry_order",
            "entries",
            "entries must be strictly ordered by destination",
        );
    }
    for entry in &manifest.entries {
        if let EntryOrigin::Native {
            path,
            object,
            origin_identity: claimed,
        } = &entry.origin
        {
            if origin_identity(
                &manifest.repository,
                &manifest.exact_revision,
                path,
                object,
                entry.kind,
            ) != *claimed
            {
                findings.push(
                    "origin_identity_mismatch",
                    format!("entries.{}", shown(&entry.destination)),
                    "an origin identity does not match its fields",
                );
            }
        }
    }
    if content_identity(&manifest.entries, &manifest.dependencies) != manifest.content_identity {
        findings.push(
            "content_identity_mismatch",
            "content_identity",
            "content identity does not match the entries and dependencies",
        );
    }
    if manifest_identity(manifest) != manifest.identity {
        findings.push(
            "manifest_identity_mismatch",
            "identity",
            "manifest identity does not match its fields",
        );
    }
    for (stale, field, code) in [
        (
            manifest.repository != *expected.repository,
            "repository",
            "stale_repository",
        ),
        (
            manifest.exact_revision != *expected.exact_revision,
            "exact_revision",
            "stale_revision",
        ),
        (
            manifest.definition != *expected.definition,
            "definition",
            "stale_definition",
        ),
        (
            manifest.evaluator != *expected.evaluator,
            "evaluator",
            "stale_evaluator",
        ),
        (
            manifest.adapter != *expected.adapter,
            "adapter",
            "stale_adapter",
        ),
    ] {
        if stale {
            findings.push(
                code,
                field,
                "the manifest binds a value other than the expected one",
            );
        }
    }
    findings.finish(())
}

#[cfg(test)]
mod tests;
