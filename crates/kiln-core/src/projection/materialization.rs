//! Plan the materialization of an admitted complete projection (#78).
//!
//! A plan turns a [`CompleteProjection`] and a versioned materializer profile
//! into the exact entries to write and two identities: a reusable input
//! identity shared by every attempt over the same manifest and profile, and an
//! overlay identity bound to one attempt and fence. A [`WorkspacePin`] carries
//! both, with the source, definition, manifest and profile they came from, so
//! dispatch and the retained overlay agree on one workspace. A
//! [`ProjectedWorkspaceRoute`] derives, from the pin and the plan, the exact
//! workspace root, read paths and write paths a restricted runtime receives.
//! The shell reads, verifies and writes; this module performs no I/O and
//! grants no read, write or execution authority.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::build_inputs::BuildInputPlan;
use super::{
    AdmittedDefinition, CompleteProjection, ComponentVersion, DEFINITION_SCHEMA,
    DependencyDeclaration, EntryKind, EntryOrigin, ExpectedProjection, Findings,
    MAX_TARGET_PATH_BYTES, ProjectionDefinition, ProjectionOperation, ProjectionProfile,
    ResolvedProjectionManifest, Selection, TargetProfile, admit_definition, shown, update_target,
    verify_manifest,
};
use crate::monorepo::{finish, update_field, update_set};
use crate::paths;
use crate::types::{ContentId, Diagnostic, ExternalRef};

pub const MATERIALIZER_PROFILE_SCHEMA: &str = "kiln.materializer-profile.v1";
pub const WORKSPACE_PIN_SCHEMA: &str = "kiln.materialization.workspace-pin.v1";
const PROFILE_IDENTITY_SCHEMA: &str = "kiln.materializer-profile.identity.v1";
const INPUT_IDENTITY_SCHEMA: &str = "kiln.materialization.input.v1";
const OVERLAY_IDENTITY_SCHEMA: &str = "kiln.materialization.overlay.v1";
pub const WORKSPACE_ROUTE_SCHEMA: &str = "kiln.projected-workspace-route.v1";
const ROUTE_IDENTITY_SCHEMA: &str = "kiln.projected-workspace-route.identity.v1";
/// The one restricted runtime a route names: Lattice's bubblewrap provider,
/// which mounts only the route's paths below a private workspace.
pub const LATTICE_BWRAP_RUNTIME: &str = "lattice-bwrap";
/// Directory of published inputs below a materialization store.
pub const INPUTS_DIRECTORY: &str = "inputs";
/// Directory of attempt overlays below a materialization store.
pub const OVERLAYS_DIRECTORY: &str = "overlays";
/// Read paths, and separately write paths, one route may name. Each is one
/// runtime mount.
pub const MAX_ROUTE_PATHS: usize = 256;
/// Writable source paths one route request may name.
pub const MAX_ROUTE_WRITABLE: usize = 64;
/// Host roots Lattice's bubblewrap provider read-only binds into every
/// sandbox: `SYSTEM_BASES` and the `/run/current-system/sw/bin` bind at
/// `/usr/bin` (Lattice `src/candidate_worker/sandbox.rs:1` and `:168-178`,
/// identical at the pinned cohort revision `5a8e1d51`, at the earlier pin
/// `661499d2` and at the qualified `7fb6dc47`). Every other host path the
/// sandbox sees is a fresh `/proc`, `/dev`, `/tmp` or `/workspace`, or a
/// route path. A materialization store at or below one of these roots would
/// let the worker read every input and overlay outside its route, so no route
/// places a store there.
pub const RUNTIME_MOUNTED_ROOTS: [&str; 6] = [
    "/nix/store",
    "/usr",
    "/bin",
    "/lib",
    "/lib64",
    "/run/current-system/sw/bin",
];
/// Path components Lattice refuses in a workspace path as VCS metadata
/// (Lattice `src/candidate_capture.rs:116`, `VcsMetadataRejected`). A route
/// never names one, so the refusal comes before any durable dispatch step.
const VCS_METADATA: [&str; 2] = [".git", ".jj"];

/// How a materializer constructs the initial tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializerKind {
    /// Every entry is written to a local directory before the attempt starts.
    EagerLocalDirectory,
}

impl MaterializerKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::EagerLocalDirectory => "eager_local_directory",
        }
    }
}

/// The materializer implementation and the platform it writes to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializerProfile {
    pub schema: String,
    pub kind: MaterializerKind,
    pub materializer: ComponentVersion,
    pub platform: TargetProfile,
}

impl MaterializerProfile {
    /// BLAKE3 over the schema, kind, materializer and platform.
    #[must_use]
    pub fn identity(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", PROFILE_IDENTITY_SCHEMA.as_bytes());
        update_field(&mut hasher, "profile.schema", self.schema.as_bytes());
        update_field(&mut hasher, "kind", self.kind.as_str().as_bytes());
        update_field(
            &mut hasher,
            "materializer.name",
            self.materializer.name.as_bytes(),
        );
        update_field(
            &mut hasher,
            "materializer.version",
            self.materializer.version.as_bytes(),
        );
        update_target(&mut hasher, self.platform);
        finish(hasher)
    }
}

/// One entry to write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedEntry {
    pub destination: String,
    pub executable: bool,
    /// Provider-native object holding the bytes; the shell verifies it.
    pub object: ExternalRef,
}

/// The entries and identities of one materialization. Only [`plan`]
/// constructs one, from a completeness witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationPlan {
    repository: ContentId,
    source: ExternalRef,
    definition: ContentId,
    manifest: ContentId,
    profile: ContentId,
    operation: ProjectionOperation,
    input_identity: ContentId,
    entries: Vec<PlannedEntry>,
}

impl MaterializationPlan {
    /// Identity of the manifest this plan materializes.
    #[must_use]
    pub fn manifest(&self) -> &ContentId {
        &self.manifest
    }

    /// The operation the completeness witness admitted.
    #[must_use]
    pub fn operation(&self) -> ProjectionOperation {
        self.operation
    }

    /// Reusable identity of the immutable input every attempt shares.
    #[must_use]
    pub fn input_identity(&self) -> &ContentId {
        &self.input_identity
    }

    /// Entries in destination order.
    #[must_use]
    pub fn entries(&self) -> &[PlannedEntry] {
        &self.entries
    }

    /// Identity of one attempt's private writable overlay over this input.
    pub fn overlay_identity(
        &self,
        attempt: &ContentId,
        fence: u64,
    ) -> Result<ContentId, Diagnostic> {
        overlay_identity(&self.input_identity, attempt, fence)
    }

    /// The pin dispatch and the retained overlay of one attempt carry.
    // r[impl kiln.source.projection.materialization.scenario.pinned]
    pub fn pin(&self, attempt: &ContentId, fence: u64) -> Result<WorkspacePin, Diagnostic> {
        Ok(WorkspacePin {
            schema: WORKSPACE_PIN_SCHEMA.into(),
            repository: self.repository.clone(),
            source: self.source.clone(),
            definition: self.definition.clone(),
            manifest: self.manifest.clone(),
            profile: self.profile.clone(),
            input: self.input_identity.clone(),
            overlay: self.overlay_identity(attempt, fence)?,
            attempt: attempt.clone(),
            fence,
        })
    }
}

/// Identity of one attempt's private writable overlay over an input.
pub fn overlay_identity(
    input: &ContentId,
    attempt: &ContentId,
    fence: u64,
) -> Result<ContentId, Diagnostic> {
    input.validate()?;
    attempt.validate()?;
    if fence == 0 {
        return Err(Diagnostic::new(
            "materialization_fence",
            "materialization.fence",
            "an overlay needs a non-zero attempt fence",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", OVERLAY_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "input", input.as_str().as_bytes());
    update_field(&mut hasher, "attempt", attempt.as_str().as_bytes());
    update_field(&mut hasher, "fence", &fence.to_le_bytes());
    Ok(finish(hasher))
}

/// Everything one attempt's projected workspace is prepared from: the
/// admitted repository and exact source revision, the projection definition
/// and resolved manifest, the materializer profile, the shared input and this
/// attempt's overlay generation. Dispatch and the retained overlay binding
/// carry the same pin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePin {
    pub schema: String,
    pub repository: ContentId,
    pub source: ExternalRef,
    pub definition: ContentId,
    pub manifest: ContentId,
    pub profile: ContentId,
    pub input: ContentId,
    pub overlay: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
}

impl WorkspacePin {
    /// The pin's identities are well formed and its overlay is the identity
    /// of its input, attempt and fence.
    // r[impl kiln.source.projection.materialization.scenario.pinned]
    pub fn validate(&self) -> Result<(), Diagnostic> {
        for identity in [
            &self.repository,
            &self.definition,
            &self.manifest,
            &self.profile,
        ] {
            identity.validate()?;
        }
        if self.schema != WORKSPACE_PIN_SCHEMA
            || overlay_identity(&self.input, &self.attempt, self.fence)? != self.overlay
        {
            return Err(Diagnostic::new(
                "materialization_pin",
                "materialization.pin",
                "workspace pin schema or overlay generation is inconsistent",
            ));
        }
        Ok(())
    }
}

/// What a route is derived for, besides its view, plan and pin.
#[derive(Clone, Copy, Debug)]
pub struct RouteRequest<'a> {
    /// Absolute materialization store holding the input and the overlay.
    pub store: &'a str,
    /// Source paths, or source directories, the attempt may change. Only a
    /// completeness witness for a write admits any.
    pub writable: &'a [String],
    /// The build-input plan the attempt runs, when it runs one.
    pub build_plan: Option<&'a BuildInputPlan>,
}

/// `kiln.projected-workspace-route.v1`: what one attempt's restricted runtime
/// receives. The workspace root is the attempt overlay, which holds the
/// planned entries of the immutable input; read paths are its top-level
/// directories and files, mounted read-only; write paths are the overlay
/// destinations of the writable origins and the writable creation
/// destinations. Every path is relative to the overlay, as Lattice's
/// implementation request names them. The route carries the authored
/// definition and the resolved manifest it was derived from, so a dispatcher
/// can derive it again for its plan. A recorded route is a claim:
/// [`ProjectedWorkspaceRoute::verify`] checks it,
/// [`ProjectedWorkspaceRoute::verify_permitted`] derives it again for a
/// plan's permitted paths, and only [`route`] derives one from a view.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedWorkspaceRoute {
    pub schema: String,
    pub pin: WorkspacePin,
    pub input_root: String,
    pub overlay_root: String,
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub build_plan: Option<ContentId>,
    pub runtime: String,
    /// The authored definition the route was derived from; admitting it
    /// yields the pin's definition.
    pub definition: ProjectionDefinition,
    /// The resolved manifest the route was derived from; it is the pin's.
    pub manifest: ResolvedProjectionManifest,
    /// BLAKE3 over every other field. The definition and manifest are bound
    /// through the pin's identities, which [`ProjectedWorkspaceRoute::verify`]
    /// recomputes.
    pub identity: ContentId,
}

impl ProjectedWorkspaceRoute {
    // r[impl kiln.source.projection.route.scenario.refused]
    /// The route is internally consistent: its schema and runtime are the
    /// supported ones, its pin is valid, its roots are the pin's input and
    /// overlay in one store outside every runtime-mounted root, its paths are
    /// sorted canonical relative paths within the bounds and without VCS
    /// metadata, every write path lies within a read path and no write path
    /// lies within another, its definition and manifest are the pin's, and
    /// its identity covers every field.
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        let mut findings = Findings::default();
        if self.schema != WORKSPACE_ROUTE_SCHEMA || self.runtime != LATTICE_BWRAP_RUNTIME {
            findings.push(
                "route_schema",
                "schema",
                "unsupported route schema or runtime",
            );
        }
        if let Err(invalid) = self.pin.validate() {
            findings.push(&invalid.code, invalid.field, &invalid.message);
        }
        let roots = self
            .store()
            .is_some_and(|store| self.input_root == root(store, INPUTS_DIRECTORY, &self.pin.input));
        if !roots {
            findings.push(
                "route_roots",
                "roots",
                "the roots are not the pin's input and overlay in one store",
            );
        }
        let sorted = |paths: &[String]| {
            paths.len() <= MAX_ROUTE_PATHS
                && paths.iter().all(|path| route_path(path))
                && paths
                    .windows(2)
                    .all(|pair| matches!(pair, [left, right] if left < right))
        };
        let nested = self.write_paths.iter().all(|path| {
            within(&self.read_paths, path)
                && !self
                    .write_paths
                    .iter()
                    .any(|other| other != path && dir_contains(other, path))
        });
        if !sorted(&self.read_paths) || !sorted(&self.write_paths) || !nested {
            findings.push(
                "route_paths",
                "paths",
                "route paths must be sorted, canonical, bounded and writable only within read paths",
            );
        }
        if self
            .read_paths
            .iter()
            .chain(&self.write_paths)
            .any(|path| vcs_metadata(path))
        {
            findings.push(
                "route_vcs_metadata",
                "paths",
                "a route never names VCS metadata; Lattice refuses a .git or .jj path component",
            );
        }
        if !self.evidence() {
            findings.push(
                "route_evidence",
                "evidence",
                "the route's definition or manifest is not the pin's",
            );
        }
        if route_identity(self) != self.identity {
            findings.push(
                "route_identity",
                "identity",
                "the route identity does not cover its fields",
            );
        }
        findings.finish(())
    }

    /// Whether the definition admits to the pin's definition and the manifest
    /// is consistent and is the pin's.
    fn evidence(&self) -> bool {
        let pin = &self.pin;
        let manifest = &self.manifest;
        admit_definition(&self.definition)
            .is_ok_and(|admitted| *admitted.identity() == pin.definition)
            && manifest.identity == pin.manifest
            && verify_manifest(
                manifest,
                ExpectedProjection {
                    repository: &pin.repository,
                    exact_revision: &pin.source,
                    definition: &pin.definition,
                    evaluator: &manifest.evaluator,
                    adapter: &manifest.adapter,
                },
            )
            .is_ok()
    }

    // r[impl kiln.source.projection.route.scenario.dispatch]
    /// Derive this route again for a dispatch plan's permitted source paths
    /// and require exactly its read and write paths. A projected dispatch runs
    /// the write route of exactly the plan's permitted paths over a writable
    /// manifest: a route derived for other or wider paths, for a build or
    /// over VCS metadata is refused, as is an invalid route.
    pub fn verify_permitted(&self, permitted: &[String]) -> Result<(), Vec<Diagnostic>> {
        self.verify()?;
        let mut findings = Findings::default();
        if permitted.is_empty()
            || permitted.len() > MAX_ROUTE_WRITABLE
            || !permitted.iter().all(|path| paths::canonical(path))
        {
            findings.push(
                "route_permitted_paths",
                "permitted_paths",
                "a projected dispatch needs one to the named bound of canonical permitted source paths",
            );
        }
        if self.manifest.profile != ProjectionProfile::Writable {
            findings.push(
                "route_write_requires_write_witness",
                "manifest.profile",
                "a projected dispatch writes its permitted paths and needs a writable projection",
            );
        }
        let definition = admit_definition(&self.definition)?;
        let (read, write) =
            derive_paths(&mut findings, &definition, &self.manifest, true, permitted);
        if read != self.read_paths || write != self.write_paths {
            findings.push(
                "route_permission_mismatch",
                "paths",
                "the route's read or write paths are not the ones derived for the plan's permitted paths",
            );
        }
        findings.finish(())
    }

    /// The store both roots lie in, when the overlay root is the pin's.
    #[must_use]
    pub fn store(&self) -> Option<&str> {
        let suffix = format!("/{OVERLAYS_DIRECTORY}/{}", self.pin.overlay.as_str());
        self.overlay_root
            .strip_suffix(suffix.as_str())
            .filter(|store| store_path(store))
    }

    /// Whether a runtime request names exactly this route's workspace root,
    /// read paths and write paths.
    #[must_use]
    pub fn admits(
        &self,
        workspace_root: &str,
        read_paths: &[String],
        write_paths: &[String],
    ) -> bool {
        workspace_root == self.overlay_root
            && read_paths == self.read_paths.as_slice()
            && write_paths == self.write_paths.as_slice()
    }
}

// r[impl kiln.source.projection.route]
// r[impl kiln.source.projection.route.scenario.derived]
// r[impl kiln.source.projection.route.scenario.refused]
/// Derive the restricted-runtime route of one attempt's projected workspace.
///
/// The plan must be the projection's and the pin the one the plan derives
/// for its attempt and fence. The paths are those [`derive_paths`] gives for
/// the request's writable source paths. A build witness with writable paths,
/// a build plan of another manifest, a store that is unsafe or that the
/// restricted runtime mounts, an unsafe writable path, a destination with VCS
/// metadata and every bound violation are refused before anything is
/// derived.
pub fn route(
    definition: &AdmittedDefinition,
    complete: &CompleteProjection,
    plan: &MaterializationPlan,
    pin: &WorkspacePin,
    request: RouteRequest<'_>,
) -> Result<ProjectedWorkspaceRoute, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let manifest = complete.manifest();
    if plan.manifest != manifest.identity || *definition.identity() != manifest.definition {
        findings.push(
            "route_plan_mismatch",
            "plan",
            "the plan or definition is not the projection's",
        );
    }
    if plan
        .pin(&pin.attempt, pin.fence)
        .ok()
        .is_none_or(|derived| derived != *pin)
    {
        findings.push(
            "route_pin_mismatch",
            "pin",
            "the pin is not the one the plan derives for its attempt and fence",
        );
    }
    if !store_path(request.store) {
        findings.push(
            "route_store",
            "store",
            "the store must be an absolute canonical path within the path bound, outside every root the restricted runtime mounts",
        );
    }
    if request.writable.len() > MAX_ROUTE_WRITABLE {
        findings.push(
            "route_bound",
            "writable",
            "more writable paths than the named bound",
        );
    }
    for path in request.writable {
        if !paths::canonical(path) {
            findings.push(
                "route_writable_invalid",
                format!("writable.{}", shown(path)),
                "a writable path must be a canonical relative source path",
            );
        }
    }
    let write = complete.operation() == ProjectionOperation::Write;
    if !write && !request.writable.is_empty() {
        findings.push(
            "route_write_requires_write_witness",
            "writable",
            "only a completeness witness for a write admits writable paths",
        );
    }
    if request
        .build_plan
        .is_some_and(|build| *build.manifest() != manifest.identity)
    {
        findings.push(
            "route_build_plan_mismatch",
            "build_plan",
            "the build plan was made for another manifest",
        );
    }
    let (read_paths, write_paths) =
        derive_paths(&mut findings, definition, manifest, write, request.writable);
    if read_paths.len() > MAX_ROUTE_PATHS || write_paths.len() > MAX_ROUTE_PATHS {
        findings.push(
            "route_bound",
            "paths",
            "the route needs more runtime mounts than the named bound",
        );
    }
    let mut derived = ProjectedWorkspaceRoute {
        schema: WORKSPACE_ROUTE_SCHEMA.into(),
        pin: pin.clone(),
        input_root: root(request.store, INPUTS_DIRECTORY, &pin.input),
        overlay_root: root(request.store, OVERLAYS_DIRECTORY, &pin.overlay),
        read_paths,
        write_paths,
        build_plan: request.build_plan.map(|build| build.identity().clone()),
        runtime: LATTICE_BWRAP_RUNTIME.into(),
        definition: authored(definition),
        manifest: manifest.clone(),
        identity: finish(blake3::Hasher::new()),
    };
    derived.identity = route_identity(&derived);
    findings.finish(derived)
}

/// The read and write paths of a projected workspace over `manifest`.
///
/// Read paths are the top-level components of every planned destination
/// and, for a write, of every creation destination. Write paths exist only
/// for a write: each destination whose native origin lies within a writable
/// source path, and each creation destination [`creation_writable`] grants
/// whole; a write path within another is dropped. A planned or creation
/// destination with a VCS metadata component is refused.
fn derive_paths(
    findings: &mut Findings,
    definition: &AdmittedDefinition,
    manifest: &ResolvedProjectionManifest,
    write: bool,
    writable: &[String],
) -> (Vec<String>, Vec<String>) {
    let none = BTreeSet::new();
    let creation = if write {
        &definition.creation_destinations
    } else {
        &none
    };
    let mut read = BTreeSet::new();
    for destination in manifest
        .entries
        .iter()
        .map(|entry| &entry.destination)
        .chain(creation)
    {
        if vcs_metadata(destination) {
            findings.push(
                "route_vcs_metadata",
                format!("destinations.{}", shown(destination)),
                "a route never names VCS metadata; Lattice refuses a .git or .jj path component",
            );
        }
        read.insert(top(destination).to_string());
    }
    let mut written = BTreeSet::new();
    for entry in manifest.entries.iter().filter(|_| write) {
        if let EntryOrigin::Native { path, .. } = &entry.origin
            && within(writable, path)
        {
            written.insert(entry.destination.clone());
        }
    }
    for destination in creation {
        if creation_writable(definition, manifest, destination, writable) {
            written.insert(destination.clone());
        }
    }
    let write_paths = written
        .iter()
        .filter(|path| {
            !written
                .iter()
                .any(|other| other != *path && dir_contains(other, path))
        })
        .cloned()
        .collect();
    (read.into_iter().collect(), write_paths)
}

/// Whether creation directory `destination` may be granted whole: every
/// source path a mount gives it, every native origin planned at or below it
/// and the source prefix of every mount placed below it lie within a
/// writable source path. A generated entry below it, or an alias of an
/// unwritable origin nested inside it, refuses the directory grant.
fn creation_writable(
    definition: &AdmittedDefinition,
    manifest: &ResolvedProjectionManifest,
    destination: &str,
    writable: &[String],
) -> bool {
    let sources: Vec<String> = definition
        .mounts()
        .iter()
        .filter_map(|mount| mounted(mount, destination))
        .collect();
    let at_or_below = |path: &str| path == destination || dir_contains(destination, path);
    !sources.is_empty()
        && sources.iter().all(|source| within(writable, source))
        && manifest
            .entries
            .iter()
            .filter(|entry| at_or_below(&entry.destination))
            .all(|entry| {
                matches!(&entry.origin, EntryOrigin::Native { path, .. } if within(writable, path))
            })
        && definition
            .mounts()
            .iter()
            .filter(|mount| dir_contains(destination, &mount.destination_prefix))
            .all(|mount| within(writable, &mount.source_prefix))
}

/// The authored form of an admitted definition; admitting it again yields
/// the same definition and identity.
fn authored(definition: &AdmittedDefinition) -> ProjectionDefinition {
    ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: definition.id.clone(),
        profile: definition.profile,
        selection: Selection {
            root: definition.selection.root().into(),
            include: definition.selection.include().iter().cloned().collect(),
            exclude: definition.selection.exclude().iter().cloned().collect(),
        },
        mounts: definition.mounts.iter().cloned().collect(),
        dependencies: definition
            .dependencies
            .iter()
            .map(|(name, required)| DependencyDeclaration {
                name: name.clone(),
                required: *required,
            })
            .collect(),
        creation_destinations: definition.creation_destinations.iter().cloned().collect(),
        required_capabilities: definition.required_capabilities.iter().copied().collect(),
        generated_inputs: definition.generated_inputs,
        target: definition.target,
        compatibility: definition.compatibility.clone(),
    }
}

fn route_identity(route: &ProjectedWorkspaceRoute) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", ROUTE_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "route.schema", route.schema.as_bytes());
    let pin = &route.pin;
    update_field(&mut hasher, "pin.schema", pin.schema.as_bytes());
    for (label, identity) in [
        ("pin.repository", &pin.repository),
        ("pin.definition", &pin.definition),
        ("pin.manifest", &pin.manifest),
        ("pin.profile", &pin.profile),
        ("pin.input", &pin.input),
        ("pin.overlay", &pin.overlay),
        ("pin.attempt", &pin.attempt),
    ] {
        update_field(&mut hasher, label, identity.as_str().as_bytes());
    }
    super::update_external(&mut hasher, "pin.source", &pin.source);
    update_field(&mut hasher, "pin.fence", &pin.fence.to_le_bytes());
    update_field(&mut hasher, "input_root", route.input_root.as_bytes());
    update_field(&mut hasher, "overlay_root", route.overlay_root.as_bytes());
    update_set(
        &mut hasher,
        "read_paths",
        route.read_paths.iter().map(String::as_str),
    );
    update_set(
        &mut hasher,
        "write_paths",
        route.write_paths.iter().map(String::as_str),
    );
    match &route.build_plan {
        Some(plan) => update_field(&mut hasher, "build_plan", plan.as_str().as_bytes()),
        None => update_field(&mut hasher, "build_plan.none", b""),
    }
    update_field(&mut hasher, "runtime", route.runtime.as_bytes());
    finish(hasher)
}

/// An absolute canonical path within the target path bound, at or below no
/// root the restricted runtime mounts.
fn store_path(path: &str) -> bool {
    path.len() <= MAX_TARGET_PATH_BYTES
        && path.strip_prefix('/').is_some_and(paths::canonical)
        && !RUNTIME_MOUNTED_ROOTS
            .iter()
            .any(|root| path == *root || dir_contains(root, path))
}

/// Whether a canonical store path lies outside every runtime-mounted root,
/// both by the roots' lexical names and by `resolved_roots`, the canonical
/// paths the shell resolved the existing [`RUNTIME_MOUNTED_ROOTS`] to.
/// Lattice's bind follows a symbolic link, so with `/usr -> /srv` a store
/// below `/srv` is visible at `/usr`. This performs no I/O: the caller
/// canonicalizes the store, resolves the roots and refuses when it cannot.
#[must_use]
pub fn store_outside_runtime_mounts(store: &str, resolved_roots: &[String]) -> bool {
    store_path(store)
        && !resolved_roots
            .iter()
            .any(|root| root == "/" || root == store || dir_contains(root, store))
}

/// Whether a relative path has a component Lattice refuses as VCS metadata.
fn vcs_metadata(path: &str) -> bool {
    path.split('/').any(|part| VCS_METADATA.contains(&part))
}

/// A canonical relative route path within the target path bound.
fn route_path(path: &str) -> bool {
    path.len() <= MAX_TARGET_PATH_BYTES && paths::canonical(path)
}

fn root(store: &str, directory: &str, identity: &ContentId) -> String {
    format!("{store}/{directory}/{}", identity.as_str())
}

/// The first component of a relative path.
fn top(path: &str) -> &str {
    path.split_once('/').map_or(path, |(first, _)| first)
}

/// Whether `path` lies strictly below directory `dir`.
fn dir_contains(dir: &str, path: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `path` is one of `roots` or lies below one.
fn within(roots: &[String], path: &str) -> bool {
    roots
        .iter()
        .any(|root| root == path || dir_contains(root, path))
}

/// The source path `mount` gives a destination at or below its prefix.
fn mounted(mount: &super::Mount, destination: &str) -> Option<String> {
    let rest = if mount.destination_prefix.is_empty() {
        destination
    } else if destination == mount.destination_prefix {
        ""
    } else {
        destination
            .strip_prefix(mount.destination_prefix.as_str())?
            .strip_prefix('/')?
    };
    match (mount.source_prefix.is_empty(), rest.is_empty()) {
        (true, _) => Some(rest.to_string()),
        (false, true) => Some(mount.source_prefix.clone()),
        (false, false) => Some(format!("{}/{rest}", mount.source_prefix)),
    }
}

// r[impl kiln.source.projection.materialization]
// r[impl kiln.source.projection.materialization.scenario.refused]
/// Plan the materialization of a complete projection under one profile.
///
/// Eager local directories write native file and executable entries only:
/// symbolic links and generated entries are refused until a profile
/// qualifies them, as are executable entries on a platform without
/// executable bits, a platform whose case rules differ from the projection
/// target and destinations longer than the platform bound. Every fault is
/// reported before any effect.
pub fn plan(
    complete: &CompleteProjection,
    profile: &MaterializerProfile,
) -> Result<MaterializationPlan, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let manifest = complete.manifest();
    if profile.schema != MATERIALIZER_PROFILE_SCHEMA {
        findings.push(
            "materializer_profile_schema",
            "profile.schema",
            "unsupported materializer profile schema",
        );
    }
    if !profile.materializer.admissible() {
        findings.push(
            "materializer_component",
            "profile.materializer",
            "materializer name or version is empty, oversized or malformed",
        );
    }
    let platform = profile.platform;
    if platform.case_sensitive != manifest.target.case_sensitive {
        findings.push(
            "materializer_case_rules",
            "profile.platform.case_sensitive",
            "the platform's case rules differ from the projection target",
        );
    }
    let mut entries = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let field = format!("destinations.{}", shown(&entry.destination));
        if entry.destination.len() > platform.max_path_bytes {
            findings.push(
                "materializer_path_length",
                field.clone(),
                "the destination exceeds the platform path bound",
            );
        }
        match (&entry.origin, entry.kind) {
            (EntryOrigin::Generated { .. }, _) => findings.push(
                "materializer_generated_entry",
                field,
                "eager local materialization writes source entries only; generated entries need a generator provider",
            ),
            (EntryOrigin::Native { .. }, EntryKind::Symlink) => findings.push(
                "materializer_symlink",
                field,
                "symbolic links are refused until a materializer profile qualifies them",
            ),
            (EntryOrigin::Native { .. }, EntryKind::Executable) if !platform.executable_bit => {
                findings.push(
                    "materializer_executable_bit",
                    field,
                    "the platform cannot represent an executable entry",
                );
            }
            (EntryOrigin::Native { object, .. }, kind) => entries.push(PlannedEntry {
                destination: entry.destination.clone(),
                executable: kind == EntryKind::Executable,
                object: object.clone(),
            }),
        }
    }
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", INPUT_IDENTITY_SCHEMA.as_bytes());
    update_field(
        &mut hasher,
        "manifest",
        manifest.identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "content",
        manifest.content_identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.kind",
        profile.kind.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.name",
        profile.materializer.name.as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.version",
        profile.materializer.version.as_bytes(),
    );
    update_target(&mut hasher, platform);
    findings.finish(MaterializationPlan {
        repository: manifest.repository.clone(),
        source: manifest.exact_revision.clone(),
        definition: manifest.definition.clone(),
        manifest: manifest.identity.clone(),
        profile: profile.identity(),
        operation: complete.operation(),
        input_identity: finish(hasher),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{
        AdmittedRepository, Coverage, DependencyFact, DependencyStatus, GeneratedEntry,
        GeneratedInputs, ProjectionDefinition, ProjectionFacts, ProviderBinding,
        RepositoryEnrollment, RepositoryLocator, SourceCapability, SourceEntry, admit_definition,
        admit_repositories, resolve,
    };
    use super::*;
    use crate::types::ProviderProfileId;
    use alloc::string::ToString;
    use alloc::vec;

    const EXPORTED_DEFINITION: &str =
        include_str!("../../../../fixtures/source-projection/definition.json");

    fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn digest(label: &str) -> ContentId {
        ok(ContentId::from_blake3_hex(
            blake3::hash(label.as_bytes()).to_hex().to_string(),
        ))
    }

    fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
        ok(ExternalRef::new(namespace, kind, id))
    }

    fn component(name: &str, version: &str) -> ComponentVersion {
        ComponentVersion {
            name: name.into(),
            version: version.into(),
        }
    }

    fn repository() -> AdmittedRepository {
        let enrolled = [RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: external("github", "repository", "OnixResearch/kiln"),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/cache/kiln".into(),
            }],
        }];
        ok(admit_repositories(&enrolled, &[]))
            .pop()
            .unwrap_or_else(|| unreachable!("one admitted repository"))
    }

    fn entry(path: &str, blob: &str, kind: EntryKind) -> SourceEntry {
        SourceEntry {
            path: path.into(),
            object: external("git", "blob", blob),
            kind,
        }
    }

    fn facts(repository: &AdmittedRepository, entries: Vec<SourceEntry>) -> ProjectionFacts {
        ProjectionFacts {
            repository: repository.identity().clone(),
            exact_revision: external("git", "commit", &"a".repeat(40)),
            canonical_tree: digest("tree-1"),
            evaluator: component("kiln-projection", "1"),
            adapter: component("git-source", "1"),
            source_capabilities: vec![
                SourceCapability::ExactTree,
                SourceCapability::ExecutableBit,
                SourceCapability::Symlinks,
            ],
            coverage: Coverage::Complete,
            entries,
            dependencies: vec![DependencyFact {
                name: "workspace-manifest".into(),
                status: DependencyStatus::Present {
                    identity: digest("workspace-manifest"),
                },
            }],
            generated: Vec::new(),
        }
    }

    fn base_entries() -> Vec<SourceEntry> {
        vec![
            entry("crates/kiln-core/Cargo.toml", "b1", EntryKind::File),
            entry("crates/kiln-core/src/lib.rs", "b2", EntryKind::File),
            entry("crates/kiln-core/run.sh", "b3", EntryKind::Executable),
        ]
    }

    fn complete(definition: &ProjectionDefinition, facts: &ProjectionFacts) -> CompleteProjection {
        let repository = repository();
        ok(ok(resolve(
            &ok(admit_definition(definition)),
            &repository,
            facts,
        ))
        .require_complete(ProjectionOperation::Build))
    }

    fn definition() -> ProjectionDefinition {
        ok(serde_json::from_str(EXPORTED_DEFINITION))
    }

    fn profile() -> MaterializerProfile {
        MaterializerProfile {
            schema: MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: MaterializerKind::EagerLocalDirectory,
            materializer: component("kiln-eager-local", "1"),
            platform: TargetProfile::POSIX,
        }
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    // r[verify kiln.source.projection.materialization]
    #[test]
    fn plan_identities_follow_the_manifest_profile_attempt_and_fence() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let first = ok(plan(&projection, &profile()));
        assert_eq!(first, ok(plan(&projection, &profile())));
        assert_eq!(first.manifest(), &projection.manifest().identity);
        assert_eq!(first.operation(), ProjectionOperation::Build);
        let planned: Vec<(&str, bool)> = first
            .entries()
            .iter()
            .map(|entry| (entry.destination.as_str(), entry.executable))
            .collect();
        assert_eq!(
            planned,
            vec![
                ("kiln-core/Cargo.toml", false),
                ("kiln-core/run.sh", true),
                ("kiln-core/src/lib.rs", false),
                ("manifests/kiln-core.toml", false),
            ]
        );

        let mut newer = profile();
        newer.materializer.version = "2".into();
        assert_ne!(
            first.input_identity(),
            ok(plan(&projection, &newer)).input_identity()
        );
        let changed = vec![
            entry("crates/kiln-core/Cargo.toml", "b1", EntryKind::File),
            entry("crates/kiln-core/src/lib.rs", "b9", EntryKind::File),
            entry("crates/kiln-core/run.sh", "b3", EntryKind::Executable),
        ];
        let other = complete(&definition(), &facts(&repository, changed));
        assert_ne!(
            first.input_identity(),
            ok(plan(&other, &profile())).input_identity()
        );

        let attempt_a = digest("attempt-a");
        let attempt_b = digest("attempt-b");
        let overlay = ok(first.overlay_identity(&attempt_a, 1));
        assert_eq!(overlay, ok(first.overlay_identity(&attempt_a, 1)));
        assert_ne!(overlay, ok(first.overlay_identity(&attempt_a, 2)));
        assert_ne!(overlay, ok(first.overlay_identity(&attempt_b, 1)));
        assert_ne!(&overlay, first.input_identity());
        assert!(first.overlay_identity(&attempt_a, 0).is_err());
    }

    // r[verify kiln.source.projection.materialization.scenario.pinned]
    #[test]
    fn pins_bind_the_exact_source_definition_manifest_profile_and_overlay() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let planned = ok(plan(&projection, &profile()));
        let attempt = digest("attempt-a");
        let pin = ok(planned.pin(&attempt, 3));
        let manifest = projection.manifest();
        assert_eq!(
            (&pin.repository, &pin.source, &pin.definition, &pin.manifest),
            (
                &manifest.repository,
                &manifest.exact_revision,
                &manifest.definition,
                &manifest.identity
            )
        );
        assert_eq!(pin.profile, profile().identity());
        assert_eq!(&pin.input, planned.input_identity());
        assert_eq!(pin.overlay, ok(planned.overlay_identity(&attempt, 3)));
        assert_eq!(pin.validate(), Ok(()));
        let mut newer = profile();
        newer.materializer.version = "2".into();
        assert_ne!(newer.identity(), profile().identity());
        assert_ne!(
            ok(plan(&projection, &newer)).pin(&attempt, 3),
            Ok(pin.clone())
        );
        type PinChange = fn(&mut WorkspacePin);
        let changes: [PinChange; 4] = [
            |pin| pin.fence = 4,
            |pin| pin.attempt = digest("attempt-b"),
            |pin| pin.input = digest("other-input"),
            |pin| pin.schema = "kiln.materialization.workspace-pin.v0".into(),
        ];
        for change in changes {
            let mut altered = pin.clone();
            change(&mut altered);
            assert!(altered.validate().is_err(), "{altered:?}");
        }
        assert!(planned.pin(&attempt, 0).is_err());
    }

    type ProfileChange = fn(&mut MaterializerProfile);

    // r[verify kiln.source.projection.materialization.scenario.refused]
    #[test]
    fn unsupported_entries_and_platforms_are_refused_before_any_effect() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let cases: [(ProfileChange, &str); 5] = [
            (
                |p| p.schema = "kiln.materializer-profile.v0".into(),
                "materializer_profile_schema",
            ),
            (
                |p| p.materializer.version = String::new(),
                "materializer_component",
            ),
            (
                |p| p.platform.case_sensitive = false,
                "materializer_case_rules",
            ),
            (
                |p| p.platform.executable_bit = false,
                "materializer_executable_bit",
            ),
            (
                |p| p.platform.max_path_bytes = 12,
                "materializer_path_length",
            ),
        ];
        for (change, code) in cases {
            let mut refused = profile();
            change(&mut refused);
            let diagnostics = plan(&projection, &refused).err().unwrap_or_default();
            assert!(
                codes(&diagnostics).contains(&code),
                "{code}: {diagnostics:?}"
            );
        }

        let mut linked = base_entries();
        linked.push(entry("crates/kiln-core/link", "b4", EntryKind::Symlink));
        let with_link = complete(&definition(), &facts(&repository, linked));
        let diagnostics = plan(&with_link, &profile()).err().unwrap_or_default();
        assert_eq!(codes(&diagnostics), vec!["materializer_symlink"]);
        assert_eq!(
            diagnostics.first().map(|d| d.field.as_str()),
            Some("destinations.kiln-core/link")
        );

        let mut generating = definition();
        generating.generated_inputs = GeneratedInputs::Admitted;
        let mut generated = facts(&repository, base_entries());
        generated.generated.push(GeneratedEntry {
            destination: "kiln-core/src/generated.rs".into(),
            kind: EntryKind::File,
            generator: component("codegen", "1"),
            input_identity: digest("codegen-input"),
        });
        let with_generated = complete(&generating, &generated);
        let diagnostics = plan(&with_generated, &profile()).err().unwrap_or_default();
        assert_eq!(codes(&diagnostics), vec!["materializer_generated_entry"]);
    }
}

#[cfg(test)]
mod route_tests;
