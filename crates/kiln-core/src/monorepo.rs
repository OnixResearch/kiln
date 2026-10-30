//! Path-scoped CI projects sharing one repository and the pure affected-project
//! decision.
//!
//! The shell supplies exact facts: admitted project scopes, the changed files
//! between an exact base and candidate, and build-graph facts derived from
//! manifests at the candidate. [`affected`] decides, without I/O, which projects
//! must run and why. Uncertain facts never skip a project: they affect every
//! project that depends on them with an explicit fallback reason.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::paths;
use crate::types::{ContentId, DEFAULT_MAX_ID_BYTES};

/// Projects that may share one canonical repository.
pub const MAX_PROJECTS_PER_REPOSITORY: usize = 64;
/// Patterns per scope list (`include`, `exclude`, `depends_on`).
pub const MAX_SCOPE_PATTERNS: usize = 32;
/// Bytes per scope pattern, and for the scope root.
pub const MAX_SCOPE_PATTERN_BYTES: usize = 256;
/// Changed paths admitted as exact facts; more is a `truncated` fallback.
pub const MAX_CHANGED_PATHS: usize = 10_000;
/// Total changed-path bytes admitted as exact facts; more is a `truncated` fallback.
pub const MAX_CHANGED_PATH_BYTES: usize = 1_048_576;
/// Paths sampled into one `path` or `declared` reason.
pub const MAX_REASON_SAMPLE: usize = 5;
/// `graph` reasons recorded per graph kind for one project.
pub const MAX_GRAPH_REASONS: usize = 5;
/// Packages accepted in one build graph; more makes the graph unavailable.
pub const MAX_GRAPH_PACKAGES: usize = 2_048;
/// Edges accepted in one build graph; more makes the graph unavailable.
pub const MAX_GRAPH_EDGES: usize = 65_536;

const SCOPE_IDENTITY_SCHEMA: &str = "kiln.monorepo.scope.v1";
const AFFECTED_SET_SCHEMA: &str = "kiln.monorepo.affected.v1";
/// `via_package` of a graph reason caused by a file that touches every package.
pub const ALL_PACKAGES: &str = "*";
/// Prefix of the fallback reason for a consulted graph that is unavailable.
pub const GRAPH_UNAVAILABLE: &str = "graph_unavailable";
const GRAPH_NOT_SUPPLIED: &str = "graph facts not supplied";

/// Build-graph kinds a scope may consult.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphKind {
    Cargo,
    Nix,
}

impl GraphKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Nix => "nix",
        }
    }
}

impl fmt::Display for GraphKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn default_include() -> Vec<String> {
    ["**".to_string()].into()
}

/// Human-authored project scope, before admission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectScope {
    /// Repository-relative directory with `/` separators; empty is the repository root.
    pub root: String,
    /// Globs relative to `root`.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    /// Globs relative to `root`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Repository-relative globs.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Build graphs consulted for this project.
    #[serde(default)]
    pub graphs: BTreeSet<GraphKind>,
}

/// Scope admission failure naming the project and field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeError {
    pub project_id: String,
    pub field: String,
    pub code: String,
    pub message: String,
}

impl ScopeError {
    fn new(project_id: &str, field: &str, code: &str, message: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            field: field.to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "project {:?} field {}: {} ({})",
            self.project_id, self.field, self.message, self.code
        )
    }
}

/// An admitted scope: validated, canonically ordered and bound to its project by
/// a BLAKE3 identity. Pattern order and duplicates do not change semantics, so
/// they do not change the identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedScope {
    project_id: String,
    root: String,
    include: BTreeSet<String>,
    exclude: BTreeSet<String>,
    depends_on: BTreeSet<String>,
    graphs: BTreeSet<GraphKind>,
    identity: ContentId,
}

impl Serialize for AdmittedScope {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            root: &'a str,
            include: &'a BTreeSet<String>,
            exclude: &'a BTreeSet<String>,
            depends_on: &'a BTreeSet<String>,
            graphs: &'a BTreeSet<GraphKind>,
            identity: &'a ContentId,
        }
        Wire {
            root: &self.root,
            include: &self.include,
            exclude: &self.exclude,
            depends_on: &self.depends_on,
            graphs: &self.graphs,
            identity: &self.identity,
        }
        .serialize(serializer)
    }
}

impl AdmittedScope {
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }
    #[must_use]
    pub fn include(&self) -> &BTreeSet<String> {
        &self.include
    }
    #[must_use]
    pub fn exclude(&self) -> &BTreeSet<String> {
        &self.exclude
    }
    #[must_use]
    pub fn depends_on(&self) -> &BTreeSet<String> {
        &self.depends_on
    }
    #[must_use]
    pub fn graphs(&self) -> &BTreeSet<GraphKind> {
        &self.graphs
    }
    /// BLAKE3 identity over the canonical scope and its project id.
    #[must_use]
    pub fn identity(&self) -> &ContentId {
        &self.identity
    }

    /// Whether a repository-relative path lies under `root`, matches some
    /// `include` and no `exclude`.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        let Some(relative) = strip_root(&self.root, path) else {
            return false;
        };
        self.include
            .iter()
            .any(|pattern| paths::matches(pattern, relative))
            && !self
                .exclude
                .iter()
                .any(|pattern| paths::matches(pattern, relative))
    }
}

/// The part of `path` below directory `root`, if any.
fn strip_root<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    if root.is_empty() {
        return Some(path);
    }
    path.strip_prefix(root)?.strip_prefix('/')
}

/// Whether directory `dir` is `root` or lies below it (every dir lies under "").
fn dir_under(dir: &str, root: &str) -> bool {
    root.is_empty() || dir == root || strip_root(root, dir).is_some()
}

fn admit_patterns(
    project_id: &str,
    field: &str,
    patterns: &[String],
) -> Result<BTreeSet<String>, ScopeError> {
    if patterns.len() > MAX_SCOPE_PATTERNS {
        return Err(ScopeError::new(
            project_id,
            field,
            "bound_exceeded",
            "list exceeds the maximum number of scope patterns",
        ));
    }
    let mut admitted = BTreeSet::new();
    for (index, pattern) in patterns.iter().enumerate() {
        let item = format!("{field}[{index}]");
        if pattern.len() > MAX_SCOPE_PATTERN_BYTES {
            return Err(ScopeError::new(
                project_id,
                &item,
                "bound_exceeded",
                "pattern exceeds the maximum scope pattern bytes",
            ));
        }
        if !paths::valid_pattern(pattern) {
            return Err(ScopeError::new(
                project_id,
                &item,
                "invalid_pattern",
                "pattern must be a relative glob without empty, `.` or `..` segments, backslashes, control characters, classes or alternations",
            ));
        }
        admitted.insert(pattern.clone());
    }
    Ok(admitted)
}

/// Admit one project's scope.
///
/// Rejects absolute paths, `..`, empty or `.` segments, backslashes, NUL and other
/// control characters, glob metacharacters in `root`, and lists or patterns beyond
/// [`MAX_SCOPE_PATTERNS`] and [`MAX_SCOPE_PATTERN_BYTES`].
// r[impl kiln.monorepo.scope]
pub fn admit_scope(project_id: &str, scope: &ProjectScope) -> Result<AdmittedScope, ScopeError> {
    if project_id.is_empty() || project_id.len() > DEFAULT_MAX_ID_BYTES {
        return Err(ScopeError::new(
            project_id,
            "id",
            "invalid_project_id",
            "project id must be non-empty and within its byte bound",
        ));
    }
    let root_field = "scope.root";
    if scope.root.len() > MAX_SCOPE_PATTERN_BYTES {
        return Err(ScopeError::new(
            project_id,
            root_field,
            "bound_exceeded",
            "root exceeds the maximum scope pattern bytes",
        ));
    }
    if !scope.root.is_empty()
        && (!paths::canonical(&scope.root) || scope.root.contains(['*', '?', '[', ']', '{', '}']))
    {
        return Err(ScopeError::new(
            project_id,
            root_field,
            "invalid_root",
            "root must be a literal relative directory without empty, `.` or `..` segments, backslashes or control characters",
        ));
    }
    let include = admit_patterns(project_id, "scope.include", &scope.include)?;
    let exclude = admit_patterns(project_id, "scope.exclude", &scope.exclude)?;
    let depends_on = admit_patterns(project_id, "scope.depends_on", &scope.depends_on)?;
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", SCOPE_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "project_id", project_id.as_bytes());
    update_field(&mut hasher, "root", scope.root.as_bytes());
    update_set(&mut hasher, "include", include.iter().map(String::as_str));
    update_set(&mut hasher, "exclude", exclude.iter().map(String::as_str));
    update_set(
        &mut hasher,
        "depends_on",
        depends_on.iter().map(String::as_str),
    );
    update_set(
        &mut hasher,
        "graphs",
        scope.graphs.iter().map(|kind| kind.as_str()),
    );
    Ok(AdmittedScope {
        project_id: project_id.to_string(),
        root: scope.root.clone(),
        include,
        exclude,
        depends_on,
        graphs: scope.graphs.clone(),
        identity: finish(hasher),
    })
}

/// Why exact changed files are unavailable.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedFallback {
    /// A new reference with no previous head, or a manual or scheduled run.
    NoBase,
    /// The diff exceeded [`MAX_CHANGED_PATHS`] or [`MAX_CHANGED_PATH_BYTES`].
    Truncated,
    /// The source provider cannot produce exact tree diffs.
    UnsupportedProvider,
    /// Reading the base, candidate or diff failed.
    ReadError,
}

impl ChangedFallback {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoBase => "no_base",
            Self::Truncated => "truncated",
            Self::UnsupportedProvider => "unsupported_provider",
            Self::ReadError => "read_error",
        }
    }
}

/// Changed-file facts between an exact base and candidate. Renames contribute
/// both paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedFiles {
    Exact(BTreeSet<String>),
    Fallback(ChangedFallback),
}

/// Changed-file summary recorded on runs: `{"exact_count":12}` or
/// `{"fallback":"no_base"}`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedSummary {
    ExactCount(usize),
    Fallback(ChangedFallback),
}

impl ChangedFiles {
    /// Admit changed paths under the named bounds; beyond them the facts are a
    /// `truncated` fallback, never a partial list.
    // r[impl kiln.monorepo.changed]
    pub fn exact(changed: impl IntoIterator<Item = String>) -> Self {
        let mut admitted = BTreeSet::new();
        let mut bytes = 0_usize;
        for path in changed {
            if admitted.contains(&path) {
                continue;
            }
            bytes = bytes.saturating_add(path.len());
            if admitted.len() >= MAX_CHANGED_PATHS || bytes > MAX_CHANGED_PATH_BYTES {
                return Self::Fallback(ChangedFallback::Truncated);
            }
            admitted.insert(path);
        }
        Self::Exact(admitted)
    }

    #[must_use]
    pub fn summary(&self) -> ChangedSummary {
        match self {
            Self::Exact(changed) => ChangedSummary::ExactCount(changed.len()),
            Self::Fallback(reason) => ChangedSummary::Fallback(*reason),
        }
    }
}

/// A build-graph package: an id (Cargo package name, flake directory) and its
/// repository-relative directory (empty for the repository root).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub dir: String,
}

/// Build-graph facts from exact manifests.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildGraph {
    pub packages: Vec<Package>,
    /// `(dependent_dir, dependency_dir)`; both are package directories.
    pub edges: Vec<(String, String)>,
    /// Repository-relative files whose change touches every package.
    pub roots_touching_all: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphResult {
    Available(BuildGraph),
    Unavailable(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphFacts {
    pub kind: GraphKind,
    pub result: GraphResult,
}

/// One project of the repository; `scope: None` is an unscoped project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectInput {
    pub project_id: String,
    pub scope: Option<AdmittedScope>,
}

/// Why a project is affected.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    /// The project has no scope and keeps repository-wide behaviour.
    Unscoped,
    /// `count` changed files lie in the scope; `sample` holds at most
    /// [`MAX_REASON_SAMPLE`] of them.
    Path { sample: Vec<String>, count: usize },
    /// Changed files match a declared `depends_on` pattern.
    Declared {
        pattern: String,
        sample: Vec<String>,
    },
    /// `from_package` changed and `via_package`, under the project root, depends
    /// on it (or is it). For a change to a file that touches every package,
    /// `from_package` is that file and `via_package` is [`ALL_PACKAGES`].
    Graph {
        graph: GraphKind,
        from_package: String,
        via_package: String,
    },
    /// Facts were missing or uncertain: `no_base`, `truncated`,
    /// `unsupported_provider`, `read_error` or `graph_unavailable: <kind>: <detail>`.
    Fallback { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    NoChangesInScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Affected { reasons: Vec<Reason> },
    Skipped { reason: SkipReason },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectDecision {
    pub project_id: String,
    #[serde(flatten)]
    pub outcome: Outcome,
}

impl ProjectDecision {
    #[must_use]
    pub fn is_affected(&self) -> bool {
        matches!(self.outcome, Outcome::Affected { .. })
    }
}

/// Every project's decision, ordered by project id, with a BLAKE3 identity over
/// the changed facts, each project's scope identity and its decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AffectedSet {
    pub identity: ContentId,
    pub decisions: Vec<ProjectDecision>,
}

impl AffectedSet {
    #[must_use]
    pub fn decision(&self, project_id: &str) -> Option<&ProjectDecision> {
        self.decisions
            .iter()
            .find(|decision| decision.project_id == project_id)
    }
}

/// An admitted graph with dependency closure lookups.
struct GraphIndex<'a> {
    graph: &'a BuildGraph,
    /// Package indices ordered by directory.
    by_dir: BTreeMap<&'a str, usize>,
    /// For each package index, the nearest changed package whose change reaches
    /// it through the reverse dependency closure (breadth-first from every
    /// changed package; ties go to the lower package id).
    origin: Vec<Option<usize>>,
    /// Changed files that touch every package, in path order.
    touched_roots: Vec<&'a str>,
}

impl<'a> GraphIndex<'a> {
    fn new(graph: &'a BuildGraph, changed: &'a BTreeSet<String>) -> Result<Self, String> {
        if graph.packages.len() > MAX_GRAPH_PACKAGES {
            return Err("graph exceeds the maximum package count".to_string());
        }
        if graph.edges.len() > MAX_GRAPH_EDGES {
            return Err("graph exceeds the maximum edge count".to_string());
        }
        let mut by_dir = BTreeMap::new();
        for (index, package) in graph.packages.iter().enumerate() {
            if by_dir.insert(package.dir.as_str(), index).is_some() {
                return Err(format!("duplicate package directory {:?}", package.dir));
            }
        }
        let mut dependents: Vec<Vec<usize>> = graph.packages.iter().map(|_| Vec::new()).collect();
        for (dependent, dependency) in &graph.edges {
            let (Some(&from), Some(&to)) = (
                by_dir.get(dependent.as_str()),
                by_dir.get(dependency.as_str()),
            ) else {
                return Err(format!(
                    "edge {dependent:?} -> {dependency:?} names an unknown package directory"
                ));
            };
            if let Some(list) = dependents.get_mut(to) {
                list.push(from);
            }
        }
        let mut sources: Vec<usize> = changed
            .iter()
            .filter_map(|path| package_of(&by_dir, path))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        sources.sort_by(|left, right| {
            let id = |index: &usize| graph.packages.get(*index).map(|package| &package.id);
            id(left).cmp(&id(right))
        });
        let mut origin: Vec<Option<usize>> = graph.packages.iter().map(|_| None).collect();
        let mut queue = VecDeque::new();
        for source in sources {
            if let Some(slot) = origin.get_mut(source)
                && slot.is_none()
            {
                *slot = Some(source);
                queue.push_back((source, source));
            }
        }
        while let Some((node, source)) = queue.pop_front() {
            for &dependent in dependents.get(node).into_iter().flatten() {
                if let Some(slot) = origin.get_mut(dependent)
                    && slot.is_none()
                {
                    *slot = Some(source);
                    queue.push_back((dependent, source));
                }
            }
        }
        let touched_roots = graph
            .roots_touching_all
            .iter()
            .filter(|root| changed.contains(*root))
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            graph,
            by_dir,
            origin,
            touched_roots,
        })
    }

    fn id(&self, index: usize) -> &'a str {
        self.graph
            .packages
            .get(index)
            .map_or("", |package| package.id.as_str())
    }

    /// Graph reasons for a project rooted at `root`, whose packages are those with
    /// a directory under `root` (every package for the repository root).
    fn reasons(&self, kind: GraphKind, root: &str) -> Vec<Reason> {
        let mut reasons: Vec<Reason> = self
            .touched_roots
            .iter()
            .map(|path| Reason::Graph {
                graph: kind,
                from_package: (*path).to_string(),
                via_package: ALL_PACKAGES.to_string(),
            })
            .collect();
        let project_packages = self
            .by_dir
            .iter()
            .filter(|(dir, _)| dir_under(dir, root))
            .map(|(_, index)| *index);
        let mut pairs: Vec<(&str, &str)> = project_packages
            .filter_map(|via| {
                let from = (*self.origin.get(via)?)?;
                Some((self.id(from), self.id(via)))
            })
            .collect();
        pairs.sort_unstable_by(|left, right| left.1.cmp(right.1).then(left.0.cmp(right.0)));
        reasons.extend(pairs.into_iter().map(|(from, via)| Reason::Graph {
            graph: kind,
            from_package: from.to_string(),
            via_package: via.to_string(),
        }));
        reasons.truncate(MAX_GRAPH_REASONS);
        reasons
    }
}

/// The deepest package directory containing `path` (the root package "" contains
/// every path).
fn package_of(by_dir: &BTreeMap<&str, usize>, path: &str) -> Option<usize> {
    let mut candidate = path;
    loop {
        candidate = match candidate.rfind('/') {
            Some(split) => candidate.get(..split)?,
            None if candidate.is_empty() => return None,
            None => "",
        };
        if let Some(&index) = by_dir.get(candidate) {
            return Some(index);
        }
        if candidate.is_empty() {
            return None;
        }
    }
}

fn sample<'a>(paths: impl Iterator<Item = &'a String>) -> (Vec<String>, usize) {
    let mut sample = Vec::new();
    let mut count = 0_usize;
    for path in paths {
        if sample.len() < MAX_REASON_SAMPLE {
            sample.push(path.clone());
        }
        count = count.saturating_add(1);
    }
    (sample, count)
}

fn graph_fallback(kind: GraphKind, detail: &str) -> Reason {
    Reason::Fallback {
        reason: format!("{GRAPH_UNAVAILABLE}: {kind}: {detail}"),
    }
}

fn scoped_reasons(
    scope: &AdmittedScope,
    changed: &BTreeSet<String>,
    indexes: &BTreeMap<GraphKind, Result<GraphIndex<'_>, String>>,
) -> Vec<Reason> {
    let mut reasons = Vec::new();
    let (in_scope, count) = sample(changed.iter().filter(|path| scope.contains(path)));
    if count > 0 {
        reasons.push(Reason::Path {
            sample: in_scope,
            count,
        });
    }
    for pattern in &scope.depends_on {
        let (matched, count) = sample(changed.iter().filter(|path| paths::matches(pattern, path)));
        if count > 0 {
            reasons.push(Reason::Declared {
                pattern: pattern.clone(),
                sample: matched,
            });
        }
    }
    for kind in &scope.graphs {
        match indexes.get(kind) {
            Some(Ok(index)) => reasons.extend(index.reasons(*kind, &scope.root)),
            Some(Err(detail)) => reasons.push(graph_fallback(*kind, detail)),
            None => reasons.push(graph_fallback(*kind, GRAPH_NOT_SUPPLIED)),
        }
    }
    reasons
}

/// Decide every project's outcome from scopes, changed-file facts and graph facts.
///
/// A scoped project is affected when a changed file is in its scope, matches a
/// declared dependency, lies in a package reachable through the reverse
/// dependency closure of a consulted graph from a package under its root, or
/// touches every package of a consulted graph; when changed files fall back; or
/// when a consulted graph is unavailable or missing. Unscoped projects are
/// always affected. When several facts are supplied for one graph kind, the
/// first is used.
// r[impl kiln.monorepo.affected]
#[must_use]
pub fn affected(
    projects: &[ProjectInput],
    changed: &ChangedFiles,
    graphs: &[GraphFacts],
) -> AffectedSet {
    let mut indexes = BTreeMap::new();
    if let ChangedFiles::Exact(changed) = changed {
        for facts in graphs {
            if indexes.contains_key(&facts.kind) {
                continue;
            }
            let index = match &facts.result {
                GraphResult::Available(graph) => GraphIndex::new(graph, changed),
                GraphResult::Unavailable(detail) => Err(detail.clone()),
            };
            indexes.insert(facts.kind, index);
        }
    }
    let mut ordered: Vec<&ProjectInput> = projects.iter().collect();
    ordered.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let decisions: Vec<ProjectDecision> = ordered
        .into_iter()
        .map(|project| {
            let reasons = match (&project.scope, changed) {
                (None, _) => [Reason::Unscoped].into(),
                (Some(_), ChangedFiles::Fallback(reason)) => [Reason::Fallback {
                    reason: reason.as_str().to_string(),
                }]
                .into(),
                (Some(scope), ChangedFiles::Exact(changed)) => {
                    scoped_reasons(scope, changed, &indexes)
                }
            };
            let outcome = if reasons.is_empty() {
                Outcome::Skipped {
                    reason: SkipReason::NoChangesInScope,
                }
            } else {
                Outcome::Affected { reasons }
            };
            ProjectDecision {
                project_id: project.project_id.clone(),
                outcome,
            }
        })
        .collect();
    let identity = affected_identity(projects, changed, &decisions);
    AffectedSet {
        identity,
        decisions,
    }
}

fn affected_identity(
    projects: &[ProjectInput],
    changed: &ChangedFiles,
    decisions: &[ProjectDecision],
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", AFFECTED_SET_SCHEMA.as_bytes());
    match changed {
        ChangedFiles::Exact(paths) => {
            update_set(&mut hasher, "changed", paths.iter().map(String::as_str));
        }
        ChangedFiles::Fallback(reason) => {
            update_field(&mut hasher, "changed_fallback", reason.as_str().as_bytes());
        }
    }
    let scopes: BTreeMap<&str, Vec<&str>> =
        projects
            .iter()
            .fold(BTreeMap::new(), |mut scopes, project| {
                scopes.entry(project.project_id.as_str()).or_default().push(
                    project
                        .scope
                        .as_ref()
                        .map_or("unscoped", |scope| scope.identity().as_str()),
                );
                scopes
            });
    for decision in decisions {
        update_field(&mut hasher, "project", decision.project_id.as_bytes());
        let mut identities = scopes
            .get(decision.project_id.as_str())
            .cloned()
            .unwrap_or_default();
        identities.sort_unstable();
        update_set(&mut hasher, "scope", identities.into_iter());
        match &decision.outcome {
            Outcome::Skipped { reason } => {
                update_field(&mut hasher, "skipped", skip_label(*reason).as_bytes());
            }
            Outcome::Affected { reasons } => {
                update_usize(&mut hasher, "reasons", reasons.len());
                for reason in reasons {
                    update_reason(&mut hasher, reason);
                }
            }
        }
    }
    finish(hasher)
}

const fn skip_label(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::NoChangesInScope => "no_changes_in_scope",
    }
}

fn update_reason(hasher: &mut blake3::Hasher, reason: &Reason) {
    match reason {
        Reason::Unscoped => update_field(hasher, "reason", b"unscoped"),
        Reason::Path { sample, count } => {
            update_field(hasher, "reason", b"path");
            update_set(hasher, "sample", sample.iter().map(String::as_str));
            update_usize(hasher, "count", *count);
        }
        Reason::Declared { pattern, sample } => {
            update_field(hasher, "reason", b"declared");
            update_field(hasher, "pattern", pattern.as_bytes());
            update_set(hasher, "sample", sample.iter().map(String::as_str));
        }
        Reason::Graph {
            graph,
            from_package,
            via_package,
        } => {
            update_field(hasher, "reason", b"graph");
            update_field(hasher, "graph", graph.as_str().as_bytes());
            update_field(hasher, "from_package", from_package.as_bytes());
            update_field(hasher, "via_package", via_package.as_bytes());
        }
        Reason::Fallback { reason } => {
            update_field(hasher, "reason", b"fallback");
            update_field(hasher, "fallback", reason.as_bytes());
        }
    }
}

fn update_field(hasher: &mut blake3::Hasher, label: &str, value: &[u8]) {
    update_usize(hasher, label, value.len());
    hasher.update(value);
}

fn update_usize(hasher: &mut blake3::Hasher, label: &str, value: usize) {
    let label_length = u64::try_from(label.len()).unwrap_or(u64::MAX);
    hasher.update(&label_length.to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(&u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
}

fn update_set<'a>(hasher: &mut blake3::Hasher, label: &str, items: impl Iterator<Item = &'a str>) {
    let items: Vec<&str> = items.collect();
    update_usize(hasher, label, items.len());
    for item in items {
        update_field(hasher, label, item.as_bytes());
    }
}

fn finish(hasher: blake3::Hasher) -> ContentId {
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}

#[cfg(test)]
mod tests;
