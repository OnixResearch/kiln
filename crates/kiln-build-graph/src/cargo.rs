//! Cargo packages and path-dependency edges.
//!
//! Every listed `Cargo.toml` is parsed; manifests with `[package]` are packages
//! and manifests with `[workspace]` are workspace roots. A package belongs to its
//! explicit `package.workspace`, otherwise to the nearest ancestor workspace that
//! does not exclude it. Members of the repository-root workspace, literal or glob
//! (expanded against the directories of the listing, minus `exclude`), must have
//! a package manifest. Nested workspaces (often partial vendored imports) are
//! consulted only for inheritance, so their member lists are not checked. Edges
//! come from `path` dependencies in `[dependencies]`, `[dev-dependencies]`,
//! `[build-dependencies]` and their `[target.*]` forms, from `workspace = true`
//! dependencies resolved through the workspace's `[workspace.dependencies]`, and
//! from path `[patch]` entries that replace a dependency by name. A package
//! inheriting from a workspace below the repository root also depends on that
//! workspace root, so a change to its manifest reaches the members.
//!
//! The same parse yields [`cargo_layout`] for build-input planning: each
//! package's workspace, its path dependencies outside the repository and
//! whether a registry or Git dependency remains, and each workspace's literal
//! and glob members. A nested workspace whose members cannot be read, or name a
//! directory without a package manifest, has unknown members rather than
//! making the layout unavailable.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::monorepo::{GraphFacts, GraphKind, MAX_GRAPH_EDGES, MAX_GRAPH_PACKAGES};
use kiln_core::paths;
use kiln_core::projection::build_inputs::{
    CargoLayout, CargoLayoutResult, CargoMembers, CargoPackage, CargoWorkspace,
};
use toml::{Table, Value};

use crate::{
    CARGO_MANIFEST, CARGO_ROOTS_TOUCHING_ALL, Facts, Joined, ManifestTree, admit_tree, bounded,
    dir_of, dir_under, facts, file_name, finish_graph, in_dir, join, manifest_text,
};

const DEPENDENCY_TABLES: [&str; 5] = [
    "dependencies",
    "dev-dependencies",
    "dev_dependencies",
    "build-dependencies",
    "build_dependencies",
];
const WORKSPACE_PACKAGE_PREFIX: &str = "workspace:";
const GLOB_CHARS: [char; 6] = ['*', '?', '[', ']', '{', '}'];

/// Derive the Cargo graph of the exact tree.
// r[impl kiln.monorepo.graph]
#[must_use]
pub fn cargo_graph(tree: &ManifestTree) -> GraphFacts {
    facts(GraphKind::Cargo, parse(tree).and_then(Parsed::graph))
}

/// Derive the Cargo layout of the exact tree. Uncertain input is unavailable,
/// never a partial layout.
// r[impl kiln.source.projection.build_inputs]
#[must_use]
pub fn cargo_layout(tree: &ManifestTree) -> CargoLayoutResult {
    match parse(tree).and_then(Parsed::layout) {
        Ok(layout) => CargoLayoutResult::Available(layout),
        Err(reason) => CargoLayoutResult::Unavailable(reason),
    }
}

struct Workspace<'a> {
    dependencies: Option<&'a Table>,
    /// Excluded directories, repository-relative.
    exclude: Vec<String>,
    members: CargoMembers,
}

/// One package as the parse saw it.
struct PackageFacts {
    name: String,
    workspace: Option<String>,
    outside_paths: usize,
    external_packages: bool,
}

/// Everything one parse of the tree's manifests yields.
struct Parsed {
    /// Package facts by package directory.
    packages: BTreeMap<String, PackageFacts>,
    /// Path dependencies between package directories.
    edges: BTreeSet<(String, String)>,
    workspaces: Vec<CargoWorkspace>,
}

impl Parsed {
    fn graph(self) -> Facts<crate::BuildGraph> {
        let mut edges = self.edges;
        let mut workspace_roots = BTreeSet::new();
        for (dir, package) in &self.packages {
            if let Some(root) = &package.workspace
                && !root.is_empty()
                && root != dir
            {
                workspace_roots.insert(root.clone());
                edges.insert((dir.clone(), root.clone()));
            }
        }
        let mut packages: BTreeMap<String, String> = self
            .packages
            .into_iter()
            .map(|(dir, package)| (dir, package.name))
            .collect();
        for root in workspace_roots {
            packages
                .entry(root.clone())
                .or_insert_with(|| format!("{WORKSPACE_PACKAGE_PREFIX}{root}"));
        }
        finish_graph(packages, edges, &CARGO_ROOTS_TOUCHING_ALL)
    }

    fn layout(self) -> Facts<CargoLayout> {
        if self.packages.len() > MAX_GRAPH_PACKAGES {
            return Err("layout exceeds the maximum package count".to_string());
        }
        if self.edges.len() > MAX_GRAPH_EDGES {
            return Err("layout exceeds the maximum edge count".to_string());
        }
        Ok(CargoLayout {
            packages: self
                .packages
                .into_iter()
                .map(|(dir, package)| CargoPackage {
                    name: package.name,
                    dir,
                    workspace: package.workspace,
                    outside_paths: package.outside_paths,
                    external_packages: package.external_packages,
                })
                .collect(),
            workspaces: self.workspaces,
            edges: self.edges.into_iter().collect(),
        })
    }
}

fn parse(tree: &ManifestTree) -> Facts<Parsed> {
    admit_tree(tree)?;
    let mut manifests: BTreeMap<&str, Table> = BTreeMap::new();
    for path in tree
        .paths
        .iter()
        .filter(|path| file_name(path) == CARGO_MANIFEST)
    {
        let table: Table = toml::from_str(manifest_text(tree, path)?)
            .map_err(|error| format!("unparsable manifest {path}: {}", bounded(error.message())))?;
        manifests.insert(dir_of(path), table);
    }
    let workspaces = workspaces(&manifests, &directories(&tree.paths))?;
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (dir, manifest) in &manifests {
        if let Some(package) = manifest.get("package") {
            let name = package
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("manifest {} has no package name", manifest_path(dir)))?;
            names.insert((*dir).to_string(), name.to_string());
        }
    }
    let patches = patches(&manifests, &names)?;
    let mut packages = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for (dir, manifest) in &manifests {
        let (Some(package), Some(name)) = (manifest.get("package"), names.get(*dir)) else {
            continue;
        };
        let workspace = workspace_of(dir, package, &workspaces)?;
        let context = Context {
            dir,
            workspace: workspace.and_then(|root| Some((root, workspaces.get(root)?))),
            packages: &names,
            patches: &patches,
        };
        let mut facts = PackageFacts {
            name: name.clone(),
            workspace: workspace.map(str::to_string),
            outside_paths: 0,
            external_packages: false,
        };
        for table in dependency_tables(manifest) {
            for (key, value) in table? {
                match context.dependency(key, value)? {
                    Resolved::Packages(targets) => {
                        edges.extend(
                            targets
                                .into_iter()
                                .map(|target| ((*dir).to_string(), target)),
                        );
                    }
                    Resolved::OutsidePath => {
                        facts.outside_paths = facts.outside_paths.saturating_add(1);
                    }
                    Resolved::External => facts.external_packages = true,
                }
            }
        }
        packages.insert((*dir).to_string(), facts);
    }
    Ok(Parsed {
        packages,
        edges,
        workspaces: workspaces
            .into_iter()
            .map(|(dir, workspace)| CargoWorkspace {
                dir: dir.to_string(),
                members: workspace.members,
            })
            .collect(),
    })
}

fn manifest_path(dir: &str) -> String {
    in_dir(dir, CARGO_MANIFEST)
}

/// Every directory that holds a listed file, at any depth.
fn directories(listing: &BTreeSet<String>) -> BTreeSet<&str> {
    let mut directories: BTreeSet<&str> = BTreeSet::new();
    for file in listing {
        let mut dir = dir_of(file);
        while !dir.is_empty() && directories.insert(dir) {
            dir = dir_of(dir);
        }
    }
    directories
}

fn workspaces<'a>(
    manifests: &'a BTreeMap<&str, Table>,
    directories: &BTreeSet<&str>,
) -> Facts<BTreeMap<&'a str, Workspace<'a>>> {
    let mut workspaces = BTreeMap::new();
    for (dir, manifest) in manifests {
        let Some(workspace) = manifest.get("workspace") else {
            continue;
        };
        let path = manifest_path(dir);
        let workspace = workspace
            .as_table()
            .ok_or_else(|| format!("manifest {path} has a non-table workspace"))?;
        let exclude = strings(workspace, "exclude", &path)?
            .into_iter()
            .filter_map(|excluded| match join(dir, excluded) {
                Ok(Joined::Inside(excluded)) => Some(Ok(excluded)),
                Ok(Joined::Outside) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Facts<Vec<_>>>()?;
        let listed = members(manifests, directories, dir, workspace, &exclude);
        let members = match listed {
            Ok((literal, glob)) => CargoMembers::Known { literal, glob },
            // The root workspace's members are graph facts; a nested workspace
            // may be a partial vendored import.
            Err(reason) if !dir.is_empty() => CargoMembers::Unknown { reason },
            Err(reason) => return Err(reason),
        };
        let dependencies =
            match workspace.get("dependencies") {
                None => None,
                Some(value) => Some(value.as_table().ok_or_else(|| {
                    format!("manifest {path} has non-table workspace dependencies")
                })?),
            };
        workspaces.insert(
            *dir,
            Workspace {
                dependencies,
                exclude,
                members,
            },
        );
    }
    Ok(workspaces)
}

fn strings<'a>(table: &'a Table, key: &str, path: &str) -> Facts<Vec<&'a str>> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .ok_or_else(|| format!("manifest {path} has a non-string {key} entry"))
            })
            .collect(),
        Some(_) => Err(format!("manifest {path} has a non-array {key}")),
    }
}

/// Literal and glob-expanded member directories of the workspace at `dir`.
/// Globs expand against the listing's directories, minus `exclude`; every
/// member must have a package manifest. A directory both named and matched is
/// a literal member.
fn members(
    manifests: &BTreeMap<&str, Table>,
    directories: &BTreeSet<&str>,
    dir: &str,
    workspace: &Table,
    exclude: &[String],
) -> Facts<(Vec<String>, Vec<String>)> {
    let path = manifest_path(dir);
    let (mut literal, mut glob) = (BTreeSet::new(), BTreeSet::new());
    for member in strings(workspace, "members", &path)? {
        let Joined::Inside(member) = join(dir, member)? else {
            return Err(format!(
                "manifest {path} has a member outside the repository"
            ));
        };
        let is_glob = member.contains(GLOB_CHARS);
        let expanded: Vec<String> = if is_glob {
            if !paths::valid_pattern(&member) {
                return Err(format!(
                    "manifest {path} has an unsupported member pattern {:?}",
                    bounded(&member)
                ));
            }
            directories
                .iter()
                .filter(|candidate| paths::matches(&member, candidate))
                .filter(|candidate| {
                    !exclude
                        .iter()
                        .any(|excluded| dir_under(candidate, excluded))
                })
                .map(|candidate| (*candidate).to_string())
                .collect()
        } else {
            vec![member]
        };
        for found in &expanded {
            if !manifests
                .get(found.as_str())
                .is_some_and(|manifest| manifest.contains_key("package"))
            {
                return Err(format!("missing workspace member {}", manifest_path(found)));
            }
        }
        if is_glob {
            glob.extend(expanded);
        } else {
            literal.extend(expanded);
        }
    }
    let glob = glob.difference(&literal).cloned().collect();
    Ok((literal.into_iter().collect(), glob))
}

/// Path `[patch.<source>]` entries of every manifest, by replaced package name,
/// and the names a patch replaces with a path outside the repository.
fn patches(
    manifests: &BTreeMap<&str, Table>,
    packages: &BTreeMap<String, String>,
) -> Facts<Patches> {
    let mut patches = Patches::default();
    for (dir, manifest) in manifests {
        let Some(sources) = manifest.get("patch").and_then(Value::as_table) else {
            continue;
        };
        for source in sources.values().filter_map(Value::as_table) {
            for (key, value) in source {
                let Some(entry) = value.as_table() else {
                    continue;
                };
                let Some(path) = entry.get("path").and_then(Value::as_str) else {
                    continue;
                };
                let name = entry.get("package").and_then(Value::as_str).unwrap_or(key);
                let Joined::Inside(target) = join(dir, path)? else {
                    patches.outside.insert(name.to_string());
                    continue;
                };
                require_package(packages, &target, dir)?;
                patches
                    .inside
                    .entry(name.to_string())
                    .or_default()
                    .insert(target);
            }
        }
    }
    Ok(patches)
}

#[derive(Default)]
struct Patches {
    /// Package directories that replace a dependency, by replaced name.
    inside: BTreeMap<String, BTreeSet<String>>,
    /// Names replaced by a path outside the repository.
    outside: BTreeSet<String>,
}

fn require_package(packages: &BTreeMap<String, String>, target: &str, from: &str) -> Facts<()> {
    if packages.contains_key(target) {
        Ok(())
    } else {
        Err(format!(
            "path dependency {} of {} has no package manifest",
            manifest_path(target),
            manifest_path(from)
        ))
    }
}

/// The workspace root directory of a package, if any.
fn workspace_of<'a>(
    dir: &'a str,
    package: &Value,
    workspaces: &BTreeMap<&'a str, Workspace<'_>>,
) -> Facts<Option<&'a str>> {
    if let Some(explicit) = package.get("workspace").and_then(Value::as_str) {
        let Joined::Inside(root) = join(dir, explicit)? else {
            return Err(format!(
                "manifest {} names a workspace outside the repository",
                manifest_path(dir)
            ));
        };
        return workspaces
            .get_key_value(root.as_str())
            .map(|(root, _)| Some(*root))
            .ok_or_else(|| format!("manifest {} names a missing workspace", manifest_path(dir)));
    }
    let mut candidate = dir;
    loop {
        if let Some((root, workspace)) = workspaces.get_key_value(candidate) {
            let excluded = workspace
                .exclude
                .iter()
                .any(|excluded| dir_under(dir, excluded));
            return Ok((!excluded).then_some(*root));
        }
        if candidate.is_empty() {
            return Ok(None);
        }
        candidate = dir_of(candidate);
    }
}

fn dependency_tables(manifest: &Table) -> Vec<Facts<&Table>> {
    let mut tables: Vec<&Value> = DEPENDENCY_TABLES
        .iter()
        .filter_map(|name| manifest.get(*name))
        .collect();
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values().filter_map(Value::as_table) {
            tables.extend(
                DEPENDENCY_TABLES
                    .iter()
                    .filter_map(|name| target.get(*name)),
            );
        }
    }
    tables
        .into_iter()
        .map(|table| {
            table
                .as_table()
                .ok_or_else(|| "dependency section is not a table".to_string())
        })
        .collect()
}

/// Where one dependency entry resolves.
enum Resolved {
    /// Package directories inside the repository; none for the package itself.
    Packages(Vec<String>),
    /// A `path` that resolves outside the repository.
    OutsidePath,
    /// A registry or Git dependency that no path `[patch]` replaces.
    External,
}

struct Context<'a> {
    dir: &'a str,
    workspace: Option<(&'a str, &'a Workspace<'a>)>,
    packages: &'a BTreeMap<String, String>,
    patches: &'a Patches,
}

impl Context<'_> {
    /// Where a dependency entry resolves.
    fn dependency(&self, key: &str, value: &Value) -> Facts<Resolved> {
        let Some(entry) = value.as_table() else {
            return Ok(self.patched(key));
        };
        let (base, entry) = if entry.get("workspace").and_then(Value::as_bool) == Some(true) {
            let Some((root, workspace)) = self.workspace else {
                return Err(format!(
                    "workspace dependency {key} of {} has no workspace",
                    manifest_path(self.dir)
                ));
            };
            let inherited = workspace
                .dependencies
                .and_then(|dependencies| dependencies.get(key))
                .ok_or_else(|| {
                    format!(
                        "unresolved workspace dependency {key} of {}",
                        manifest_path(self.dir)
                    )
                })?;
            match inherited.as_table() {
                Some(inherited) => (root, inherited),
                None => return Ok(self.patched(key)),
            }
        } else {
            (self.dir, entry)
        };
        let name = entry.get("package").and_then(Value::as_str).unwrap_or(key);
        let Some(path) = entry.get("path") else {
            return Ok(self.patched(name));
        };
        let path = path.as_str().ok_or_else(|| {
            format!(
                "dependency {key} of {} has a non-string path",
                manifest_path(self.dir)
            )
        })?;
        match join(base, path)? {
            Joined::Outside => Ok(Resolved::OutsidePath),
            Joined::Inside(target) => {
                require_package(self.packages, &target, self.dir)?;
                Ok(Resolved::Packages(
                    (target != self.dir).then_some(target).into_iter().collect(),
                ))
            }
        }
    }

    /// A registry or Git dependency, unless a path `[patch]` replaces it.
    fn patched(&self, name: &str) -> Resolved {
        if let Some(targets) = self.patches.inside.get(name) {
            Resolved::Packages(
                targets
                    .iter()
                    .filter(|target| *target != self.dir)
                    .cloned()
                    .collect(),
            )
        } else if self.patches.outside.contains(name) {
            Resolved::OutsidePath
        } else {
            Resolved::External
        }
    }
}
