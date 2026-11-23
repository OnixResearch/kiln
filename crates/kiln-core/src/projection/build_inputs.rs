//! Plan the source inputs of one projected build (#77).
//!
//! A plan checks that an admitted complete projection holds every source file
//! a versioned build profile requires, each at its own source path, and binds
//! the build's command, toolchain and environment into one identity. Layout
//! facts come from exact manifest bytes of the same full candidate
//! (`kiln-build-graph`). This module performs no I/O, runs nothing and grants
//! no read, write, execution or publication authority. A plan says that the
//! declared inputs are present, not that the build succeeds: only a qualified
//! run of the plan shows that.
//!
//! Kiln cannot see reads across package boundaries or Nix reads of sibling
//! paths, so the Cargo profile requires whole packages and the Nix profile
//! plans only views of the whole source tree.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    CompleteProjection, ComponentVersion, Coverage, EntryOrigin, Findings, MAX_SOURCE_ENTRIES,
    ProjectionFacts, ProjectionOperation, shown, update_external, valid_text,
};
use crate::monorepo::{
    GraphFacts, GraphKind, GraphResult, MAX_GRAPH_EDGES, MAX_GRAPH_PACKAGES, finish, update_field,
    update_set, update_usize,
};
use crate::types::{ContentId, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef};

pub const BUILD_REQUEST_SCHEMA: &str = "kiln.build-input.request.v1";
pub const BUILD_PLAN_SCHEMA: &str = "kiln.build-input.plan.v1";
const PLAN_IDENTITY_SCHEMA: &str = "kiln.build-input.plan-identity.v1";

/// Targets in one request.
pub const MAX_BUILD_TARGETS: usize = 16;
/// Program and arguments of one command.
pub const MAX_COMMAND_ARGUMENTS: usize = 32;
/// Bytes per command argument.
pub const MAX_ARGUMENT_BYTES: usize = 256;
/// Variables in one declared environment.
pub const MAX_ENVIRONMENT_VARIABLES: usize = 32;
/// Bytes per environment variable name.
pub const MAX_VARIABLE_NAME_BYTES: usize = 128;
/// Bytes per environment variable value.
pub const MAX_VARIABLE_VALUE_BYTES: usize = 1_024;
/// Paths named in diagnostics for one fault before a single count.
pub const MAX_SAMPLED_PATHS: usize = 8;

/// Cargo configuration files read at the build root and its ancestors.
pub const CARGO_CONFIGURATION_FILES: [&str; 2] = [".cargo/config", ".cargo/config.toml"];
/// Toolchain files read at the build root and its ancestors.
pub const CARGO_TOOLCHAIN_FILES: [&str; 2] = ["rust-toolchain", "rust-toolchain.toml"];
const CARGO_MANIFEST: &str = "Cargo.toml";
const CARGO_LOCK: &str = "Cargo.lock";
const FLAKE_LOCK: &str = "flake.lock";
/// Package id of the repository-root flake in Nix graph facts.
const NIX_ROOT_PACKAGE: &str = ".";

// ---------------------------------------------------------------------------
// Layout facts

/// One Cargo package of the exact tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoPackage {
    pub name: String,
    /// Repository-relative package directory; empty is the repository root.
    pub dir: String,
    /// Root directory of the workspace the package belongs to, if any.
    pub workspace: Option<String>,
    /// Path dependencies that resolve outside the repository.
    pub outside_paths: usize,
    /// Whether a registry or Git dependency remains after path patches.
    pub external_packages: bool,
}

/// The members of one Cargo workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CargoMembers {
    /// Member directories named literally, and those a glob matched minus
    /// `exclude`.
    Known {
        literal: Vec<String>,
        glob: Vec<String>,
    },
    /// The member list could not be read; nothing is planned through it.
    Unknown { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoWorkspace {
    /// Repository-relative workspace root.
    pub dir: String,
    pub members: CargoMembers,
}

/// Cargo layout of one exact tree, derived from exact manifest bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoLayout {
    pub packages: Vec<CargoPackage>,
    pub workspaces: Vec<CargoWorkspace>,
    /// `(dependent_dir, dependency_dir)` path dependencies inside the
    /// repository, including development, build, target and patch forms.
    pub edges: Vec<(String, String)>,
}

/// Layout facts, or why they are unavailable. Uncertain manifests are never
/// a partial layout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoLayoutResult {
    Available(CargoLayout),
    Unavailable(String),
}

// ---------------------------------------------------------------------------
// Requests and plans

/// The build-input rules a request asks for.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum BuildProfile {
    /// Whole Cargo packages in the identity layout.
    #[serde(rename = "cargo_compatibility.v1")]
    CargoCompatibilityV1,
    /// Nix flakes over a view of the whole source tree.
    #[serde(rename = "nix_compatibility.v1")]
    NixCompatibilityV1,
}

impl BuildProfile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CargoCompatibilityV1 => "cargo_compatibility.v1",
            Self::NixCompatibilityV1 => "nix_compatibility.v1",
        }
    }
}

/// A build request as authored in Nickel and exported. Nothing here is
/// trusted until a planner admits it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    pub schema: String,
    pub profile: BuildProfile,
    /// Cargo package names, or Nix flake directories (`.` for the root).
    pub targets: Vec<String>,
    /// Program and arguments, run at the build root.
    pub command: Vec<String>,
    pub toolchain: ComponentVersion,
    /// The build's whole declared environment.
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}

/// Why a source file is a required input.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    /// A file of a required package or flake.
    Package,
    /// The manifest of a workspace a required package belongs to.
    WorkspaceManifest,
    Lockfile,
    Configuration,
    Toolchain,
}

impl InputRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::WorkspaceManifest => "workspace_manifest",
            Self::Lockfile => "lockfile",
            Self::Configuration => "configuration",
            Self::Toolchain => "toolchain",
        }
    }
}

/// One source file the build needs, as the view holds it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RequiredInput {
    pub path: String,
    pub role: InputRole,
    pub object: ExternalRef,
    pub origin_identity: ContentId,
}

/// Packages the build reads from outside the source snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPackages {
    /// Every package comes from the source snapshot.
    None,
    /// Registry, Git or flake inputs pinned only by a lockfile; whether they
    /// are available is a provider fact, not part of the source closure.
    PinnedByLockfile,
}

impl ExternalPackages {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PinnedByLockfile => "pinned_by_lockfile",
        }
    }
}

/// An admitted build-input plan. Only a planner constructs one, and it
/// cannot be deserialized: a recorded plan is a claim, and evidence applies
/// only through [`BuildInputPlan::accepts_evidence`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuildInputPlan {
    schema: &'static str,
    profile: BuildProfile,
    repository: ContentId,
    exact_revision: ExternalRef,
    canonical_tree: ContentId,
    manifest: ContentId,
    content: ContentId,
    build_root: String,
    targets: Vec<String>,
    packages: Vec<String>,
    inputs: Vec<RequiredInput>,
    external_packages: ExternalPackages,
    command: Vec<String>,
    toolchain: ComponentVersion,
    environment: BTreeMap<String, String>,
    identity: ContentId,
}

impl BuildInputPlan {
    /// BLAKE3 over every other field of the plan.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }
    #[must_use]
    pub fn profile(&self) -> BuildProfile {
        self.profile
    }
    /// Identity of the manifest the plan was made for.
    #[must_use]
    pub fn manifest(&self) -> &ContentId {
        &self.manifest
    }
    /// The admitted repository and exact source revision the plan reads.
    #[must_use]
    pub fn source(&self) -> (&ContentId, &ExternalRef) {
        (&self.repository, &self.exact_revision)
    }
    /// Destination directory where the command runs; empty is the root.
    #[must_use]
    pub fn build_root(&self) -> &str {
        &self.build_root
    }
    /// Required package or flake directories, sorted.
    #[must_use]
    pub fn packages(&self) -> &[String] {
        &self.packages
    }
    /// Required inputs in path order.
    #[must_use]
    pub fn inputs(&self) -> &[RequiredInput] {
        &self.inputs
    }
    #[must_use]
    pub fn external_packages(&self) -> ExternalPackages {
        self.external_packages
    }
    #[must_use]
    pub fn command(&self) -> &[String] {
        &self.command
    }
    #[must_use]
    pub fn toolchain(&self) -> &ComponentVersion {
        &self.toolchain
    }
    #[must_use]
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }

    // r[impl kiln.source.projection.build_inputs.scenario.applicability]
    /// Whether evidence recorded for `recorded_plan` applies to this plan.
    /// Any change of layout, inputs, toolchain, command or environment yields
    /// another identity, so cached results and verification for the old
    /// plan do not apply.
    #[must_use]
    pub fn accepts_evidence(&self, recorded_plan: &ContentId) -> bool {
        self.identity == *recorded_plan
    }
}

// ---------------------------------------------------------------------------
// Shared admission

/// What both profiles read from an admitted request, view and source.
struct Admitted<'a> {
    /// Source path → native entry the view holds at that same path.
    held: BTreeMap<&'a str, (&'a ExternalRef, &'a ContentId)>,
    /// Source path → object of every entry of the full candidate.
    source: BTreeMap<&'a str, &'a ExternalRef>,
}

impl Admitted<'_> {
    /// Whether the view holds any file under directory `dir`.
    fn holds_under(&self, dir: &str) -> bool {
        self.held.keys().any(|path| dir_contains(dir, path))
    }
}

/// Whether `path` lies below directory `dir` (every path lies below "").
fn dir_contains(dir: &str, path: &str) -> bool {
    dir.is_empty()
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Repository-relative path of `file` in `dir`.
fn in_dir(dir: &str, file: &str) -> String {
    if dir.is_empty() {
        file.to_string()
    } else {
        format!("{dir}/{file}")
    }
}

/// The directory containing a path; empty for the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Push one diagnostic per sampled path and a count for the rest.
fn sampled(findings: &mut Findings, code: &str, field: &str, message: &str, paths: &[&str]) {
    for path in paths.iter().take(MAX_SAMPLED_PATHS) {
        findings.push(code, format!("{field}.{}", shown(path)), message);
    }
    if let Some(rest) = paths.len().checked_sub(MAX_SAMPLED_PATHS)
        && rest > 0
    {
        findings.push(code, field, &format!("{rest} further paths: {message}"));
    }
}

fn admit_request(findings: &mut Findings, request: &BuildRequest, profile: BuildProfile) {
    if request.schema != BUILD_REQUEST_SCHEMA {
        findings.push(
            "build_request_schema",
            "request.schema",
            "unsupported build request schema",
        );
    }
    if request.profile != profile {
        findings.push(
            "build_profile_mismatch",
            "request.profile",
            "the request names another build profile than this planner",
        );
    }
    let unique = request.targets.iter().collect::<BTreeSet<_>>().len() == request.targets.len();
    if request.targets.is_empty()
        || request.targets.len() > MAX_BUILD_TARGETS
        || !unique
        || !request
            .targets
            .iter()
            .all(|target| valid_text(target, DEFAULT_MAX_ID_BYTES))
    {
        findings.push(
            "build_targets",
            "request.targets",
            "targets must be one or more unique, bounded names",
        );
    }
    let argument =
        |value: &String| value.len() <= MAX_ARGUMENT_BYTES && !value.chars().any(char::is_control);
    if request.command.len() > MAX_COMMAND_ARGUMENTS
        || !request
            .command
            .first()
            .is_some_and(|program| valid_text(program, MAX_ARGUMENT_BYTES))
        || !request.command.iter().all(argument)
    {
        findings.push(
            "build_command",
            "request.command",
            "the command needs a program and bounded arguments without control characters",
        );
    }
    if !request.toolchain.admissible() {
        findings.push(
            "build_toolchain",
            "request.toolchain",
            "toolchain name or version is empty, oversized or malformed",
        );
    }
    let name = |name: &str| {
        !name.is_empty()
            && name.len() <= MAX_VARIABLE_NAME_BYTES
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if request.environment.len() > MAX_ENVIRONMENT_VARIABLES
        || !request.environment.iter().all(|(key, value)| {
            name(key.as_str())
                && value.len() <= MAX_VARIABLE_VALUE_BYTES
                && !value.chars().any(char::is_control)
        })
    {
        findings.push(
            "build_environment",
            "request.environment",
            "environment names must be identifiers and values bounded without control characters",
        );
    }
}

/// Check the request, the witness and the binding between view and source,
/// and index both. Relocated and generated entries are refused here.
fn admit<'a>(
    findings: &mut Findings,
    complete: &'a CompleteProjection,
    source: &'a ProjectionFacts,
    request: &BuildRequest,
    profile: BuildProfile,
) -> Admitted<'a> {
    admit_request(findings, request, profile);
    let manifest = complete.manifest();
    if complete.operation() != ProjectionOperation::Build {
        findings.push(
            "build_witness_required",
            "projection",
            "a build plan needs a completeness witness for a build",
        );
    }
    if source.repository != manifest.repository
        || source.exact_revision != manifest.exact_revision
        || source.canonical_tree != manifest.canonical_tree
    {
        findings.push(
            "stale_source_facts",
            "source",
            "the source facts describe another candidate than the projection",
        );
    }
    if source.coverage != Coverage::Complete {
        findings.push(
            "incomplete_source_facts",
            "source.coverage",
            "the full candidate was not enumerated",
        );
    }
    let mut index = BTreeMap::new();
    if source.entries.len() > MAX_SOURCE_ENTRIES {
        findings.push(
            "source_entries_bound",
            "source.entries",
            "the full candidate exceeds the source entry bound",
        );
    } else {
        for entry in &source.entries {
            if index.insert(entry.path.as_str(), &entry.object).is_some() {
                findings.push(
                    "stale_source_facts",
                    format!("source.entries.{}", shown(&entry.path)),
                    "the source facts list one path twice",
                );
            }
        }
    }
    let mut held = BTreeMap::new();
    let (mut relocated, mut generated, mut stale) = (Vec::new(), Vec::new(), Vec::new());
    for entry in &manifest.entries {
        match &entry.origin {
            EntryOrigin::Native {
                path,
                object,
                origin_identity,
            } if *path == entry.destination => {
                if index.get(path.as_str()) != Some(&object) {
                    stale.push(entry.destination.as_str());
                }
                held.insert(path.as_str(), (object, origin_identity));
            }
            EntryOrigin::Native { .. } => relocated.push(entry.destination.as_str()),
            EntryOrigin::Generated { .. } => generated.push(entry.destination.as_str()),
        }
    }
    sampled(
        findings,
        "relocation_not_qualified",
        "destinations",
        "an entry is laid out away from its source path; relocation is not a qualified transformation",
        &relocated,
    );
    sampled(
        findings,
        "generated_input_not_modeled",
        "destinations",
        "no generator profile is modeled for build inputs",
        &generated,
    );
    sampled(
        findings,
        "stale_source_facts",
        "destinations",
        "the view holds another object than the source at this path",
        &stale,
    );
    Admitted {
        held,
        source: index,
    }
}

/// Require each path in `required` that the view lacks, by role.
fn absent_inputs(
    findings: &mut Findings,
    admitted: &Admitted<'_>,
    required: &BTreeMap<String, InputRole>,
) {
    let mut absent: BTreeMap<InputRole, Vec<&str>> = BTreeMap::new();
    for (path, role) in required {
        if !admitted.held.contains_key(path.as_str()) {
            absent.entry(*role).or_default().push(path.as_str());
        }
    }
    for (role, paths) in absent {
        sampled(
            findings,
            "required_input_absent",
            "inputs",
            &format!(
                "the view lacks this {} input at its source path",
                role.as_str()
            ),
            &paths,
        );
    }
}

/// Record `role` for `path`, keeping a specific role over `package`.
fn require(required: &mut BTreeMap<String, InputRole>, path: String, role: InputRole) {
    let slot = required.entry(path).or_insert(role);
    if *slot == InputRole::Package {
        *slot = role;
    }
}

/// The deepest directory in `dirs` that contains `path`.
fn owner<'a>(dirs: &BTreeSet<&'a str>, path: &str) -> Option<&'a str> {
    let mut candidate = path;
    loop {
        candidate = parent_or_root(candidate)?;
        if let Some(dir) = dirs.get(candidate) {
            return Some(*dir);
        }
    }
}

/// The parent directory of `path`, the root for a top-level path, and `None`
/// above the root.
fn parent_or_root(path: &str) -> Option<&str> {
    if path.is_empty() {
        None
    } else {
        Some(parent(path))
    }
}

/// Every directory reachable from `roots` along `edges`.
fn closure<'a>(
    roots: &BTreeSet<&'a str>,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut reached = roots.clone();
    let mut queue: VecDeque<&str> = roots.iter().copied().collect();
    while let Some(dir) = queue.pop_front() {
        for dependency in edges.get(dir).into_iter().flatten() {
            if reached.insert(*dependency) {
                queue.push_back(*dependency);
            }
        }
    }
    reached
}

/// Assemble a plan and compute its identity.
struct Parts {
    profile: BuildProfile,
    build_root: String,
    targets: BTreeSet<String>,
    packages: BTreeSet<String>,
    required: BTreeMap<String, InputRole>,
    external_packages: ExternalPackages,
}

fn assemble(
    complete: &CompleteProjection,
    admitted: &Admitted<'_>,
    request: &BuildRequest,
    parts: Parts,
) -> BuildInputPlan {
    let manifest = complete.manifest();
    let inputs = parts
        .required
        .into_iter()
        .filter_map(|(path, role)| {
            let (object, origin) = admitted.held.get(path.as_str())?;
            Some(RequiredInput {
                object: (*object).clone(),
                origin_identity: (*origin).clone(),
                path,
                role,
            })
        })
        .collect();
    let mut plan = BuildInputPlan {
        schema: BUILD_PLAN_SCHEMA,
        profile: parts.profile,
        repository: manifest.repository.clone(),
        exact_revision: manifest.exact_revision.clone(),
        canonical_tree: manifest.canonical_tree.clone(),
        manifest: manifest.identity.clone(),
        content: manifest.content_identity.clone(),
        build_root: parts.build_root,
        targets: parts.targets.into_iter().collect(),
        packages: parts.packages.into_iter().collect(),
        inputs,
        external_packages: parts.external_packages,
        command: request.command.clone(),
        toolchain: request.toolchain.clone(),
        environment: request.environment.clone(),
        identity: finish(blake3::Hasher::new()),
    };
    plan.identity = plan_identity(&plan);
    plan
}

// r[impl kiln.source.projection.build_inputs.scenario.applicability]
fn plan_identity(plan: &BuildInputPlan) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", PLAN_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "plan.schema", plan.schema.as_bytes());
    update_field(&mut hasher, "profile", plan.profile.as_str().as_bytes());
    update_field(
        &mut hasher,
        "repository",
        plan.repository.as_str().as_bytes(),
    );
    update_external(&mut hasher, "exact_revision", &plan.exact_revision);
    update_field(
        &mut hasher,
        "canonical_tree",
        plan.canonical_tree.as_str().as_bytes(),
    );
    update_field(&mut hasher, "manifest", plan.manifest.as_str().as_bytes());
    update_field(&mut hasher, "content", plan.content.as_str().as_bytes());
    update_field(&mut hasher, "build_root", plan.build_root.as_bytes());
    update_set(
        &mut hasher,
        "targets",
        plan.targets.iter().map(String::as_str),
    );
    update_set(
        &mut hasher,
        "packages",
        plan.packages.iter().map(String::as_str),
    );
    update_usize(&mut hasher, "inputs", plan.inputs.len());
    for input in &plan.inputs {
        update_field(&mut hasher, "input.path", input.path.as_bytes());
        update_field(&mut hasher, "input.role", input.role.as_str().as_bytes());
        update_external(&mut hasher, "input.object", &input.object);
        update_field(
            &mut hasher,
            "input.origin",
            input.origin_identity.as_str().as_bytes(),
        );
    }
    update_field(
        &mut hasher,
        "external_packages",
        plan.external_packages.as_str().as_bytes(),
    );
    update_set(
        &mut hasher,
        "command",
        plan.command.iter().map(String::as_str),
    );
    update_field(
        &mut hasher,
        "toolchain.name",
        plan.toolchain.name.as_bytes(),
    );
    update_field(
        &mut hasher,
        "toolchain.version",
        plan.toolchain.version.as_bytes(),
    );
    update_usize(&mut hasher, "environment", plan.environment.len());
    for (name, value) in &plan.environment {
        update_field(&mut hasher, "environment.name", name.as_bytes());
        update_field(&mut hasher, "environment.value", value.as_bytes());
    }
    finish(hasher)
}

// ---------------------------------------------------------------------------
// Cargo compatibility

// r[impl kiln.source.projection.build_inputs]
// r[impl kiln.source.projection.build_inputs.scenario.planned]
// r[impl kiln.source.projection.build_inputs.scenario.refused]
/// Plan a Cargo build of `complete` under `cargo_compatibility.v1`.
///
/// Targets are package names. The build root is their shared workspace root,
/// or the package directory outside any workspace. Loaded packages are the
/// targets, every literal member and every glob member the view holds any
/// file of; required packages are their path-dependency closure. Every file
/// of a required package, each required package's workspace manifest, the
/// build root's lockfile and the configuration and toolchain files at the
/// build root and its ancestors must be in the view at their own paths.
/// Every fault is reported before any effect.
pub fn plan_cargo(
    complete: &CompleteProjection,
    source: &ProjectionFacts,
    layout: &CargoLayoutResult,
    request: &BuildRequest,
) -> Result<BuildInputPlan, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let admitted = admit(
        &mut findings,
        complete,
        source,
        request,
        BuildProfile::CargoCompatibilityV1,
    );
    let layout = match layout {
        CargoLayoutResult::Available(layout) => layout,
        CargoLayoutResult::Unavailable(reason) => {
            findings.push(
                "layout_unavailable",
                "layout",
                &format!("Cargo layout facts are unavailable: {}", shown(reason)),
            );
            return Err(findings.into_sorted());
        }
    };
    let Some(index) = CargoIndex::new(&mut findings, layout) else {
        return Err(findings.into_sorted());
    };
    let mut targets = BTreeSet::new();
    for name in &request.targets {
        let field = format!("request.targets.{}", shown(name));
        match index.named(name).as_slice() {
            [] => findings.push(
                "unknown_target",
                field,
                "no package of the candidate has this name",
            ),
            [package] => {
                targets.insert(package.dir.as_str());
            }
            _ => findings.push(
                "ambiguous_target",
                field,
                "several packages of the candidate have this name",
            ),
        }
    }
    let roots: BTreeSet<&str> = targets
        .iter()
        .filter_map(|dir| index.packages.get(dir))
        .map(|package| package.workspace.as_deref().unwrap_or(&package.dir))
        .collect();
    let build_root = match roots.iter().collect::<Vec<_>>().as_slice() {
        [root] => **root,
        [] => return Err(findings.into_sorted()),
        _ => {
            findings.push(
                "several_build_roots",
                "request.targets",
                "the targets belong to different workspaces",
            );
            return Err(findings.into_sorted());
        }
    };
    let mut loaded = targets.clone();
    if let Some(workspace) = index.workspaces.get(build_root) {
        match &workspace.members {
            CargoMembers::Unknown { reason } => findings.push(
                "workspace_members_unknown",
                format!("layout.workspaces.{}", shown(build_root)),
                &format!("the build root's members are unknown: {}", shown(reason)),
            ),
            CargoMembers::Known { literal, glob } => {
                loaded.extend(literal.iter().map(String::as_str));
                loaded.extend(
                    glob.iter()
                        .map(String::as_str)
                        .filter(|member| admitted.holds_under(member)),
                );
            }
        }
    }
    let packages = closure(&loaded, &index.edges);
    let mut required = BTreeMap::new();
    let mut external = ExternalPackages::None;
    for dir in &packages {
        let field = format!("packages.{}", shown(dir));
        let Some(package) = index.packages.get(dir) else {
            findings.push(
                "workspace_member_missing",
                field,
                "a loaded workspace member has no package manifest in the candidate",
            );
            continue;
        };
        if package.outside_paths > 0 {
            findings.push(
                "outside_path_dependency",
                field,
                "a path dependency resolves outside the repository",
            );
        }
        if package.external_packages {
            external = ExternalPackages::PinnedByLockfile;
        }
        if let Some(root) = &package.workspace {
            require(
                &mut required,
                in_dir(root, CARGO_MANIFEST),
                InputRole::WorkspaceManifest,
            );
        }
    }
    let package_dirs: BTreeSet<&str> = index.packages.keys().copied().collect();
    for path in admitted.source.keys() {
        if owner(&package_dirs, path).is_some_and(|dir| packages.contains(dir)) {
            require(&mut required, (*path).to_string(), InputRole::Package);
        }
    }
    let lockfile = in_dir(build_root, CARGO_LOCK);
    if admitted.source.contains_key(lockfile.as_str()) {
        require(&mut required, lockfile, InputRole::Lockfile);
    } else {
        findings.push(
            "lockfile_absent",
            format!("inputs.{}", shown(&lockfile)),
            "a locked build needs the lockfile at the build root, and the candidate has none",
        );
    }
    let mut dir = Some(build_root);
    while let Some(current) = dir {
        for (files, role) in [
            (CARGO_CONFIGURATION_FILES, InputRole::Configuration),
            (CARGO_TOOLCHAIN_FILES, InputRole::Toolchain),
        ] {
            for file in files {
                let path = in_dir(current, file);
                if admitted.source.contains_key(path.as_str()) {
                    require(&mut required, path, role);
                }
            }
        }
        dir = parent_or_root(current);
    }
    absent_inputs(&mut findings, &admitted, &required);
    let parts = Parts {
        profile: BuildProfile::CargoCompatibilityV1,
        build_root: build_root.to_string(),
        targets: request.targets.iter().cloned().collect(),
        packages: packages.iter().map(|dir| (*dir).to_string()).collect(),
        required,
        external_packages: external,
    };
    findings
        .finish(())
        .map(|()| assemble(complete, &admitted, request, parts))
}

/// Admitted lookups over Cargo layout facts.
struct CargoIndex<'a> {
    packages: BTreeMap<&'a str, &'a CargoPackage>,
    workspaces: BTreeMap<&'a str, &'a CargoWorkspace>,
    edges: BTreeMap<&'a str, Vec<&'a str>>,
    layout: &'a CargoLayout,
}

impl<'a> CargoIndex<'a> {
    /// Index the layout, or report why it is inconsistent or over bound.
    fn new(findings: &mut Findings, layout: &'a CargoLayout) -> Option<Self> {
        if layout.packages.len() > MAX_GRAPH_PACKAGES || layout.edges.len() > MAX_GRAPH_EDGES {
            findings.push(
                "layout_bound",
                "layout",
                "the Cargo layout exceeds the package or edge bound",
            );
            return None;
        }
        let mut packages = BTreeMap::new();
        let mut workspaces = BTreeMap::new();
        let mut consistent = true;
        for package in &layout.packages {
            consistent &= packages.insert(package.dir.as_str(), package).is_none();
        }
        for workspace in &layout.workspaces {
            consistent &= workspaces
                .insert(workspace.dir.as_str(), workspace)
                .is_none();
        }
        let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (dependent, dependency) in &layout.edges {
            consistent &= packages.contains_key(dependent.as_str())
                && packages.contains_key(dependency.as_str());
            edges
                .entry(dependent.as_str())
                .or_default()
                .push(dependency.as_str());
        }
        consistent &= layout.packages.iter().all(|package| {
            package
                .workspace
                .as_deref()
                .is_none_or(|root| workspaces.contains_key(root))
        });
        if !consistent {
            findings.push(
                "layout_inconsistent",
                "layout",
                "the Cargo layout repeats a directory or names an unknown package or workspace",
            );
            return None;
        }
        Some(Self {
            packages,
            workspaces,
            edges,
            layout,
        })
    }

    fn named(&self, name: &str) -> Vec<&'a CargoPackage> {
        self.layout
            .packages
            .iter()
            .filter(|package| package.name == name)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Nix compatibility

// r[impl kiln.source.projection.build_inputs.scenario.nix]
/// Plan a Nix build of `complete` under `nix_compatibility.v1`.
///
/// Targets are flake directories as the Nix graph names them (`.` for the
/// root). A view that omits any source file is refused: a Nix expression can
/// read a sibling path that no input declares, and Kiln does not evaluate it.
/// A whole-tree view requires the files of the targets' path-input closure;
/// external flake inputs are pinned only by their lockfiles. The command runs
/// at the repository root.
pub fn plan_nix(
    complete: &CompleteProjection,
    source: &ProjectionFacts,
    graph: &GraphFacts,
    request: &BuildRequest,
) -> Result<BuildInputPlan, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let admitted = admit(
        &mut findings,
        complete,
        source,
        request,
        BuildProfile::NixCompatibilityV1,
    );
    let graph = match (&graph.kind, &graph.result) {
        (GraphKind::Nix, GraphResult::Available(graph)) => graph,
        (GraphKind::Nix, GraphResult::Unavailable(reason)) => {
            findings.push(
                "layout_unavailable",
                "layout",
                &format!("Nix graph facts are unavailable: {}", shown(reason)),
            );
            return Err(findings.into_sorted());
        }
        _ => {
            findings.push("layout_kind", "layout", "a Nix plan needs Nix graph facts");
            return Err(findings.into_sorted());
        }
    };
    if graph.packages.len() > MAX_GRAPH_PACKAGES || graph.edges.len() > MAX_GRAPH_EDGES {
        findings.push(
            "layout_bound",
            "layout",
            "the Nix graph exceeds the package or edge bound",
        );
        return Err(findings.into_sorted());
    }
    let omitted = admitted
        .source
        .keys()
        .filter(|path| !admitted.held.contains_key(*path))
        .count();
    if omitted > 0 {
        findings.push(
            "restricted_nix_view",
            "selection",
            &format!(
                "the view omits {omitted} source files; a Nix expression can read paths no input declares, so only a view of the whole source tree is planned"
            ),
        );
    }
    let by_id: BTreeMap<&str, &str> = graph
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package.dir.as_str()))
        .collect();
    let mut targets = BTreeSet::new();
    for target in &request.targets {
        let id = if target.is_empty() {
            NIX_ROOT_PACKAGE
        } else {
            target.as_str()
        };
        match by_id.get(id) {
            Some(dir) => {
                targets.insert(*dir);
            }
            None => findings.push(
                "unknown_target",
                format!("request.targets.{}", shown(target)),
                "no flake of the candidate is at this directory",
            ),
        }
    }
    let package_dirs: BTreeSet<&str> = by_id.values().copied().collect();
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (dependent, dependency) in &graph.edges {
        if !package_dirs.contains(dependent.as_str()) || !package_dirs.contains(dependency.as_str())
        {
            findings.push(
                "layout_inconsistent",
                "layout",
                "the Nix graph names an unknown flake directory",
            );
            return Err(findings.into_sorted());
        }
        edges
            .entry(dependent.as_str())
            .or_default()
            .push(dependency.as_str());
    }
    let packages = closure(&targets, &edges);
    let mut required = BTreeMap::new();
    for path in admitted.source.keys() {
        if owner(&package_dirs, path).is_some_and(|dir| packages.contains(dir)) {
            let role = if path.rsplit('/').next() == Some(FLAKE_LOCK) {
                InputRole::Lockfile
            } else {
                InputRole::Package
            };
            require(&mut required, (*path).to_string(), role);
        }
    }
    absent_inputs(&mut findings, &admitted, &required);
    let parts = Parts {
        profile: BuildProfile::NixCompatibilityV1,
        build_root: String::new(),
        targets: request.targets.iter().cloned().collect(),
        packages: packages.iter().map(|dir| (*dir).to_string()).collect(),
        required,
        external_packages: ExternalPackages::PinnedByLockfile,
    };
    findings
        .finish(())
        .map(|()| assemble(complete, &admitted, request, parts))
}

#[cfg(test)]
mod tests;
