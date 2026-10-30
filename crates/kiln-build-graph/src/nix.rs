//! Nix flake `path` inputs.
//!
//! Every listed `flake.lock` is parsed. Its directory is a package. A node whose
//! `locked` (or else `original`) reference has `type = "path"` and a relative
//! `path` resolves against the flake that declares it: the node named by its
//! `parent` input path, or the lock's flake when `parent` is absent or empty.
//! That declaring flake depends on the resolved directory, which is a package.
//! Absolute paths and paths above the repository root cannot change in this
//! repository and are ignored, as are relative paths of non-local flakes.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::monorepo::{GraphFacts, GraphKind};
use serde_json::{Map, Value};

use crate::{
    FLAKE_LOCK, Facts, Joined, ManifestTree, NIX_ROOTS_TOUCHING_ALL, admit_tree, bounded, dir_of,
    facts, file_name, finish_graph, join, manifest_text,
};

/// Input-path hops followed when resolving `parent` and `follows` references.
pub const MAX_INPUT_PATH_DEPTH: usize = 32;
const ROOT_PACKAGE_ID: &str = ".";

/// Derive the Nix flake path-input graph of the exact tree.
// r[impl kiln.monorepo.graph]
#[must_use]
pub fn nix_graph(tree: &ManifestTree) -> GraphFacts {
    facts(GraphKind::Nix, nix(tree))
}

fn package_id(dir: &str) -> String {
    if dir.is_empty() {
        ROOT_PACKAGE_ID.to_string()
    } else {
        dir.to_string()
    }
}

fn nix(tree: &ManifestTree) -> Facts<crate::BuildGraph> {
    admit_tree(tree)?;
    let mut packages = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for path in tree
        .paths
        .iter()
        .filter(|path| file_name(path) == FLAKE_LOCK)
    {
        let lock: Value = serde_json::from_str(manifest_text(tree, path)?)
            .map_err(|error| format!("unparsable lock {path}: {}", bounded(&error.to_string())))?;
        let lock = Lock::new(path, dir_of(path), &lock)?;
        packages.insert(lock.dir.to_string(), package_id(lock.dir));
        for name in lock.nodes.keys() {
            if let Some((declaring, resolved)) = lock.path_input(name)? {
                packages.insert(declaring.clone(), package_id(&declaring));
                packages.insert(resolved.clone(), package_id(&resolved));
                if declaring != resolved {
                    edges.insert((declaring, resolved));
                }
            }
        }
    }
    finish_graph(packages, edges, &NIX_ROOTS_TOUCHING_ALL)
}

struct Lock<'a> {
    path: &'a str,
    dir: &'a str,
    nodes: &'a Map<String, Value>,
    root: &'a str,
}

impl<'a> Lock<'a> {
    fn new(path: &'a str, dir: &'a str, lock: &'a Value) -> Facts<Self> {
        let nodes = lock
            .get("nodes")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("lock {path} has no nodes object"))?;
        let root = lock
            .get("root")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("lock {path} has no root node name"))?;
        if !nodes.contains_key(root) {
            return Err(format!("lock {path} names a missing root node"));
        }
        Ok(Self {
            path,
            dir,
            nodes,
            root,
        })
    }

    fn node(&self, name: &str) -> Facts<&'a Value> {
        self.nodes
            .get(name)
            .ok_or_else(|| format!("lock {} references missing node {name:?}", self.path))
    }

    /// The relative `path` of a local path node, if it is one.
    fn relative_path(&self, name: &str) -> Facts<Option<&'a str>> {
        let node = self.node(name)?;
        for reference in ["locked", "original"] {
            let Some(reference) = node.get(reference) else {
                continue;
            };
            if reference.get("type").and_then(Value::as_str) != Some("path") {
                continue;
            }
            let path = reference
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("lock {} path node {name:?} has no path", self.path))?;
            if !path.starts_with('/') {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    /// The node an input path from the root names, following `follows` arrays.
    fn resolve_input_path(&self, input_path: &[Value], depth: usize) -> Facts<&'a str> {
        if depth > MAX_INPUT_PATH_DEPTH {
            return Err(format!(
                "lock {} input paths exceed the follow depth",
                self.path
            ));
        }
        let mut node = self.root;
        for input in input_path {
            let input = input
                .as_str()
                .ok_or_else(|| format!("lock {} has a non-string input path", self.path))?;
            let target = self
                .node(node)?
                .get("inputs")
                .and_then(|inputs| inputs.get(input))
                .ok_or_else(|| format!("lock {} references missing input {input:?}", self.path))?;
            node = match target {
                Value::String(name) => name.as_str(),
                Value::Array(follows) => self.resolve_input_path(follows, depth + 1)?,
                _ => {
                    return Err(format!(
                        "lock {} has a malformed input {input:?}",
                        self.path
                    ));
                }
            };
        }
        Ok(node)
    }

    /// The repository directory of a node's source, when it is local.
    fn node_dir(&self, name: &str, depth: usize) -> Facts<Option<String>> {
        if name == self.root {
            return Ok(Some(self.dir.to_string()));
        }
        let Some(path) = self.relative_path(name)? else {
            return Ok(None);
        };
        let Some(base) = self.declaring_dir(name, depth)? else {
            return Ok(None);
        };
        Ok(match join(&base, path)? {
            Joined::Inside(dir) => Some(dir),
            Joined::Outside => None,
        })
    }

    /// The repository directory of the flake that declares a node.
    fn declaring_dir(&self, name: &str, depth: usize) -> Facts<Option<String>> {
        if depth > MAX_INPUT_PATH_DEPTH {
            return Err(format!(
                "lock {} parent chain exceeds the depth bound",
                self.path
            ));
        }
        let parent = match self.node(name)?.get("parent") {
            None => return Ok(Some(self.dir.to_string())),
            Some(Value::Array(parent)) if parent.is_empty() => {
                return Ok(Some(self.dir.to_string()));
            }
            Some(Value::Array(parent)) => self.resolve_input_path(parent, depth + 1)?,
            Some(_) => {
                return Err(format!(
                    "lock {} node {name:?} has a malformed parent",
                    self.path
                ));
            }
        };
        self.node_dir(parent, depth + 1)
    }

    /// `(declaring_dir, resolved_dir)` for a local path input inside the repository.
    fn path_input(&self, name: &str) -> Facts<Option<(String, String)>> {
        if name == self.root || self.relative_path(name)?.is_none() {
            return Ok(None);
        }
        let Some(declaring) = self.declaring_dir(name, 0)? else {
            return Ok(None);
        };
        Ok(self
            .node_dir(name, 0)?
            .map(|resolved| (declaring, resolved)))
    }
}
