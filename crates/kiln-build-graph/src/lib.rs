//! Build-graph facts from exact manifest bytes, without I/O.
//!
//! The shell lists the exact candidate tree, reads the blobs named by
//! [`wanted_files`] and hands both to [`cargo_graph`] or [`nix_graph`]. The result
//! is a [`kiln_core::monorepo::GraphFacts`]: either an available
//! [`BuildGraph`] or an explicit unavailable reason. Uncertain input (missing or
//! unparsable manifests, unsupported forms, inputs beyond the named bounds) is
//! always unavailable, never a partial graph.
//!
//! Coverage is Cargo path dependencies (including `workspace = true` and path
//! `[patch]` entries) and Nix flake `path` inputs. Paths that resolve outside the
//! repository cannot change in its diffs and are ignored. Registry dependencies,
//! files read across package boundaries and Nix expressions that read sibling
//! directories without a `path` input are invisible; scopes declare those with
//! `depends_on`.

mod cargo;
mod nix;

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::monorepo::{
    BuildGraph, GraphFacts, GraphKind, GraphResult, MAX_GRAPH_EDGES, MAX_GRAPH_PACKAGES, Package,
};

pub use cargo::cargo_graph;
pub use nix::nix_graph;

/// Paths accepted in one tree listing.
pub const MAX_TREE_PATHS: usize = 100_000;
/// Total bytes of the paths in one tree listing.
pub const MAX_TREE_PATH_BYTES: usize = 16 * 1_048_576;
/// Manifest blobs accepted in one tree.
pub const MAX_MANIFEST_FILES: usize = 4_096;
/// Bytes accepted per manifest blob.
pub const MAX_MANIFEST_FILE_BYTES: usize = 1_048_576;
/// Total manifest bytes accepted in one tree.
pub const MAX_MANIFEST_TOTAL_BYTES: usize = 32 * 1_048_576;
/// Bytes kept from a parser message in an unavailable reason.
pub const MAX_REASON_DETAIL_BYTES: usize = 256;

/// Cargo manifest file name.
pub const CARGO_MANIFEST: &str = "Cargo.toml";
/// Nix flake lock file name.
pub const FLAKE_LOCK: &str = "flake.lock";
/// Repository-root files whose change touches every Cargo package.
pub const CARGO_ROOTS_TOUCHING_ALL: [&str; 6] = [
    ".cargo/config",
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
];
/// Repository-root files whose change touches every Nix package.
pub const NIX_ROOTS_TOUCHING_ALL: [&str; 2] = ["flake.lock", "flake.nix"];

/// Exact candidate facts: the full listing of file paths and the bytes of the
/// manifests named by [`wanted_files`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManifestTree {
    pub paths: BTreeSet<String>,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Manifest blobs the shell must read: every `Cargo.toml` and `flake.lock` in the
/// listing, in path order. A wanted file absent from [`ManifestTree::files`]
/// makes the graph that needs it unavailable.
#[must_use]
pub fn wanted_files(listing: &BTreeSet<String>) -> Vec<String> {
    listing
        .iter()
        .filter(|path| matches!(file_name(path), CARGO_MANIFEST | FLAKE_LOCK))
        .cloned()
        .collect()
}

type Facts<T> = Result<T, String>;

fn facts(kind: GraphKind, result: Facts<BuildGraph>) -> GraphFacts {
    GraphFacts {
        kind,
        result: match result {
            Ok(graph) => GraphResult::Available(graph),
            Err(reason) => GraphResult::Unavailable(reason),
        },
    }
}

fn admit_tree(tree: &ManifestTree) -> Facts<()> {
    if tree.paths.len() > MAX_TREE_PATHS {
        return Err("tree listing exceeds the maximum path count".to_string());
    }
    if tree.paths.iter().map(String::len).sum::<usize>() > MAX_TREE_PATH_BYTES {
        return Err("tree listing exceeds the maximum path bytes".to_string());
    }
    if tree.files.len() > MAX_MANIFEST_FILES {
        return Err("manifests exceed the maximum file count".to_string());
    }
    let mut total = 0_usize;
    for (path, bytes) in &tree.files {
        if bytes.len() > MAX_MANIFEST_FILE_BYTES {
            return Err(format!("manifest {path} exceeds the maximum file bytes"));
        }
        total = total.saturating_add(bytes.len());
    }
    if total > MAX_MANIFEST_TOTAL_BYTES {
        return Err("manifests exceed the maximum total bytes".to_string());
    }
    Ok(())
}

/// The bytes of a listed manifest as UTF-8 text.
fn manifest_text<'a>(tree: &'a ManifestTree, path: &str) -> Facts<&'a str> {
    let bytes = tree
        .files
        .get(path)
        .ok_or_else(|| format!("manifest {path} not supplied"))?;
    std::str::from_utf8(bytes).map_err(|_| format!("manifest {path} is not UTF-8"))
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// The directory containing a repository-relative file; empty for the root.
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Repository-relative path of `file` in `dir`.
fn in_dir(dir: &str, file: &str) -> String {
    if dir.is_empty() {
        file.to_string()
    } else {
        format!("{dir}/{file}")
    }
}

/// Whether `dir` is `root` or lies below it.
fn dir_under(dir: &str, root: &str) -> bool {
    root.is_empty()
        || dir == root
        || dir
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Where a manifest-relative path resolves.
#[derive(Debug, Eq, PartialEq)]
enum Joined {
    Inside(String),
    /// Absolute, or above the repository root: cannot change in this repository.
    Outside,
}

fn join(base: &str, relative: &str) -> Facts<Joined> {
    if relative.contains('\\') || relative.chars().any(char::is_control) {
        return Err(format!("unsupported path {:?}", bounded(relative)));
    }
    if relative.starts_with('/') {
        return Ok(Joined::Outside);
    }
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for segment in relative.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Ok(Joined::Outside);
                }
            }
            segment => parts.push(segment),
        }
    }
    Ok(Joined::Inside(parts.join("/")))
}

/// A parser message cut to [`MAX_REASON_DETAIL_BYTES`] on a character boundary,
/// on one line.
fn bounded(detail: &str) -> String {
    let line = detail.lines().next().unwrap_or_default();
    let mut end = line.len().min(MAX_REASON_DETAIL_BYTES);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line.get(..end).unwrap_or_default().to_string()
}

/// Assemble a sorted, deduplicated, bounded graph.
fn finish_graph(
    packages: BTreeMap<String, String>,
    edges: BTreeSet<(String, String)>,
    roots: &[&str],
) -> Facts<BuildGraph> {
    if packages.len() > MAX_GRAPH_PACKAGES {
        return Err("graph exceeds the maximum package count".to_string());
    }
    if edges.len() > MAX_GRAPH_EDGES {
        return Err("graph exceeds the maximum edge count".to_string());
    }
    Ok(BuildGraph {
        packages: packages
            .into_iter()
            .map(|(dir, id)| Package { id, dir })
            .collect(),
        edges: edges.into_iter().collect(),
        roots_touching_all: roots.iter().map(|root| (*root).to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_stay_inside_or_report_outside() {
        let cases = [
            ("crates/a", "../b", Joined::Inside("crates/b".to_string())),
            (
                "crates/a",
                "./sub/../x",
                Joined::Inside("crates/a/x".to_string()),
            ),
            ("", "vendor/c", Joined::Inside("vendor/c".to_string())),
            ("crates/a", "../..", Joined::Inside(String::new())),
            ("crates/a", "../../..", Joined::Outside),
            ("crates/a", "/nix/store/x", Joined::Outside),
        ];
        for (base, relative, expected) in cases {
            assert_eq!(join(base, relative), Ok(expected), "{base} + {relative}");
        }
        assert!(join("", "a\\b").is_err());
        assert!(join("", "a\0b").is_err());
    }

    #[test]
    fn tree_bounds_are_enforced() {
        let manifests = |count: usize, bytes: usize| ManifestTree {
            files: (0..count)
                .map(|index| (index.to_string(), vec![b' '; bytes]))
                .collect(),
            ..ManifestTree::default()
        };
        let listing = |count: usize, bytes: usize| ManifestTree {
            paths: (0..count)
                .map(|index| format!("{index}{}", "x".repeat(bytes)))
                .collect(),
            ..ManifestTree::default()
        };
        assert_eq!(admit_tree(&ManifestTree::default()), Ok(()));
        assert_eq!(admit_tree(&manifests(1, MAX_MANIFEST_FILE_BYTES)), Ok(()));
        assert!(admit_tree(&manifests(1, MAX_MANIFEST_FILE_BYTES + 1)).is_err());
        assert_eq!(admit_tree(&manifests(MAX_MANIFEST_FILES, 0)), Ok(()));
        assert!(admit_tree(&manifests(MAX_MANIFEST_FILES + 1, 0)).is_err());
        let full_files = MAX_MANIFEST_TOTAL_BYTES / MAX_MANIFEST_FILE_BYTES;
        assert_eq!(
            admit_tree(&manifests(full_files, MAX_MANIFEST_FILE_BYTES)),
            Ok(())
        );
        assert!(admit_tree(&manifests(full_files + 1, MAX_MANIFEST_FILE_BYTES)).is_err());
        assert_eq!(admit_tree(&listing(MAX_TREE_PATHS, 1)), Ok(()));
        assert!(admit_tree(&listing(MAX_TREE_PATHS + 1, 1)).is_err());
        assert!(admit_tree(&listing(1, MAX_TREE_PATH_BYTES + 1)).is_err());
    }

    #[test]
    fn bounded_details_are_one_short_line() {
        assert_eq!(bounded("first\nsecond"), "first");
        let long = "é".repeat(MAX_REASON_DETAIL_BYTES);
        assert!(bounded(&long).len() <= MAX_REASON_DETAIL_BYTES);
    }

    #[test]
    fn wanted_files_are_exact_manifest_names() {
        let listing: BTreeSet<String> = [
            "Cargo.toml",
            "crates/a/Cargo.toml",
            "crates/a/Cargo.toml.orig",
            "crates/a/NotCargo.toml",
            "flake.lock",
            "sub/flake.lock",
            "sub/flake.nix",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            wanted_files(&listing),
            [
                "Cargo.toml",
                "crates/a/Cargo.toml",
                "flake.lock",
                "sub/flake.lock"
            ]
        );
    }
}
