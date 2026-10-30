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

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::monorepo::{GraphFacts, GraphKind};
use kiln_core::paths;
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

/// Derive the Cargo graph of the exact tree.
// r[impl kiln.monorepo.graph]
#[must_use]
pub fn cargo_graph(tree: &ManifestTree) -> GraphFacts {
    facts(GraphKind::Cargo, cargo(tree))
}

struct Workspace<'a> {
    dependencies: Option<&'a Table>,
    /// Excluded directories, repository-relative.
    exclude: Vec<String>,
}

fn cargo(tree: &ManifestTree) -> Facts<crate::BuildGraph> {
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
    let workspaces = workspaces(&manifests, &tree.paths)?;
    let mut packages: BTreeMap<String, String> = BTreeMap::new();
    for (dir, manifest) in &manifests {
        if let Some(package) = manifest.get("package") {
            let name = package
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("manifest {} has no package name", manifest_path(dir)))?;
            packages.insert((*dir).to_string(), name.to_string());
        }
    }
    let patches = patches(&manifests, &packages)?;
    let mut edges = BTreeSet::new();
    let mut workspace_roots = BTreeSet::new();
    for (dir, manifest) in &manifests {
        let Some(package) = manifest.get("package") else {
            continue;
        };
        let workspace = workspace_of(dir, package, &workspaces)?;
        if let Some(root) = workspace
            && !root.is_empty()
            && root != *dir
        {
            workspace_roots.insert(root.to_string());
            edges.insert(((*dir).to_string(), root.to_string()));
        }
        let context = Context {
            dir,
            workspace: workspace.and_then(|root| Some((root, workspaces.get(root)?))),
            packages: &packages,
            patches: &patches,
        };
        for table in dependency_tables(manifest) {
            for (key, value) in table? {
                for target in context.dependency(key, value)? {
                    edges.insert(((*dir).to_string(), target));
                }
            }
        }
    }
    for root in workspace_roots {
        packages
            .entry(root.clone())
            .or_insert_with(|| format!("{WORKSPACE_PACKAGE_PREFIX}{root}"));
    }
    finish_graph(packages, edges, &CARGO_ROOTS_TOUCHING_ALL)
}

fn manifest_path(dir: &str) -> String {
    in_dir(dir, CARGO_MANIFEST)
}

fn workspaces<'a>(
    manifests: &'a BTreeMap<&str, Table>,
    listing: &BTreeSet<String>,
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
        if dir.is_empty() {
            check_root_members(manifests, listing, workspace, &exclude)?;
        }
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

/// Every root-workspace member, literal or glob-expanded against the listing's
/// directories and minus `exclude`, must have a package manifest.
fn check_root_members(
    manifests: &BTreeMap<&str, Table>,
    listing: &BTreeSet<String>,
    workspace: &Table,
    exclude: &[String],
) -> Facts<()> {
    let path = manifest_path("");
    let mut directories: BTreeSet<&str> = BTreeSet::new();
    for file in listing {
        let mut dir = dir_of(file);
        while !dir.is_empty() && directories.insert(dir) {
            dir = dir_of(dir);
        }
    }
    for member in strings(workspace, "members", &path)? {
        let Joined::Inside(member) = join("", member)? else {
            return Err(format!(
                "manifest {path} has a member outside the repository"
            ));
        };
        let expanded: Vec<&str> = if member.contains(['*', '?', '[', ']', '{', '}']) {
            if !paths::valid_pattern(&member) {
                return Err(format!(
                    "manifest {path} has an unsupported member pattern {:?}",
                    bounded(&member)
                ));
            }
            directories
                .iter()
                .copied()
                .filter(|dir| paths::matches(&member, dir))
                .filter(|dir| !exclude.iter().any(|excluded| dir_under(dir, excluded)))
                .collect()
        } else {
            vec![member.as_str()]
        };
        for member in expanded {
            if !manifests
                .get(member)
                .is_some_and(|manifest| manifest.contains_key("package"))
            {
                return Err(format!(
                    "missing workspace member {}",
                    manifest_path(member)
                ));
            }
        }
    }
    Ok(())
}

/// Path `[patch.<source>]` entries of every manifest, by replaced package name.
fn patches(
    manifests: &BTreeMap<&str, Table>,
    packages: &BTreeMap<String, String>,
) -> Facts<BTreeMap<String, BTreeSet<String>>> {
    let mut patches: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
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
                let Joined::Inside(target) = join(dir, path)? else {
                    continue;
                };
                require_package(packages, &target, dir)?;
                let name = entry.get("package").and_then(Value::as_str).unwrap_or(key);
                patches.entry(name.to_string()).or_default().insert(target);
            }
        }
    }
    Ok(patches)
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

struct Context<'a> {
    dir: &'a str,
    workspace: Option<(&'a str, &'a Workspace<'a>)>,
    packages: &'a BTreeMap<String, String>,
    patches: &'a BTreeMap<String, BTreeSet<String>>,
}

impl Context<'_> {
    /// The package directories a dependency entry resolves to inside the repository.
    fn dependency(&self, key: &str, value: &Value) -> Facts<Vec<String>> {
        let Some(entry) = value.as_table() else {
            return self.patched(key);
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
                None => return self.patched(key),
            }
        } else {
            (self.dir, entry)
        };
        let name = entry.get("package").and_then(Value::as_str).unwrap_or(key);
        let Some(path) = entry.get("path") else {
            return self.patched(name);
        };
        let path = path.as_str().ok_or_else(|| {
            format!(
                "dependency {key} of {} has a non-string path",
                manifest_path(self.dir)
            )
        })?;
        match join(base, path)? {
            Joined::Outside => Ok(Vec::new()),
            Joined::Inside(target) => {
                require_package(self.packages, &target, self.dir)?;
                Ok((target != self.dir).then_some(target).into_iter().collect())
            }
        }
    }

    /// A registry or Git dependency replaced by a path `[patch]` entry.
    fn patched(&self, name: &str) -> Facts<Vec<String>> {
        Ok(self
            .patches
            .get(name)
            .into_iter()
            .flatten()
            .filter(|target| *target != self.dir)
            .cloned()
            .collect())
    }
}
