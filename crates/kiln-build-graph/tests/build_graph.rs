use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kiln_build_graph::ManifestTree;
use kiln_core::monorepo::{BuildGraph, GraphFacts, GraphResult};

/// Fixture manifests carry a `.fixture` suffix so this repository's own graph
/// never mistakes them for real manifests.
pub const FIXTURE_SUFFIX: &str = ".fixture";

pub fn tree(files: &[(&str, &str)], other_paths: &[&str]) -> ManifestTree {
    ManifestTree {
        paths: files
            .iter()
            .map(|(path, _)| (*path).to_string())
            .chain(other_paths.iter().map(|path| (*path).to_string()))
            .collect(),
        files: files
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
            .collect(),
    }
}

/// Load a fixture directory as a manifest tree, mapping `X.fixture` to `X`.
pub fn fixture_tree(name: &str, other_paths: &[&str]) -> ManifestTree {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let mut files = BTreeMap::new();
    let mut pending = vec![root.clone()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            unreachable!("fixture directory {} is readable", dir.display());
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let (Ok(relative), Ok(bytes)) = (path.strip_prefix(&root), std::fs::read(&path)) else {
                unreachable!("fixture file {} is readable", path.display());
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if let Some(manifest) = relative.strip_suffix(FIXTURE_SUFFIX) {
                files.insert(manifest.to_string(), bytes);
            }
        }
    }
    let paths: BTreeSet<String> = files
        .keys()
        .cloned()
        .chain(other_paths.iter().map(|path| (*path).to_string()))
        .collect();
    ManifestTree { paths, files }
}

pub fn available(facts: &GraphFacts) -> &BuildGraph {
    match &facts.result {
        GraphResult::Available(graph) => graph,
        GraphResult::Unavailable(reason) => unreachable!("graph unavailable: {reason}"),
    }
}

pub fn unavailable(facts: &GraphFacts) -> &str {
    match &facts.result {
        GraphResult::Unavailable(reason) => reason,
        GraphResult::Available(graph) => unreachable!("graph available: {graph:?}"),
    }
}

pub fn edges(graph: &BuildGraph) -> Vec<(&str, &str)> {
    graph
        .edges
        .iter()
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect()
}

pub fn packages(graph: &BuildGraph) -> Vec<(&str, &str)> {
    graph
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package.dir.as_str()))
        .collect()
}

use kiln_build_graph::{CARGO_ROOTS_TOUCHING_ALL, NIX_ROOTS_TOUCHING_ALL, cargo_graph, nix_graph};
use kiln_core::monorepo::{
    AdmittedScope, ChangedFiles, GraphKind, MAX_GRAPH_PACKAGES, Outcome, ProjectInput,
    ProjectScope, Reason, admit_scope, affected,
};

const KILN_FIXTURE: &str = "kiln-workspace";

fn scoped(project_id: &str, root: &str, graph: GraphKind) -> ProjectInput {
    let scope = ProjectScope {
        root: root.to_string(),
        include: vec!["**".to_string()],
        exclude: Vec::new(),
        depends_on: Vec::new(),
        graphs: [graph].into_iter().collect(),
    };
    let admitted: AdmittedScope =
        admit_scope(project_id, &scope).unwrap_or_else(|error| unreachable!("{error}"));
    ProjectInput {
        project_id: project_id.to_string(),
        scope: Some(admitted),
    }
}

fn affected_ids(set: &kiln_core::monorepo::AffectedSet) -> Vec<&str> {
    set.decisions
        .iter()
        .filter(|decision| decision.is_affected())
        .map(|decision| decision.project_id.as_str())
        .collect()
}

fn reasons<'a>(set: &'a kiln_core::monorepo::AffectedSet, project_id: &str) -> &'a [Reason] {
    match set.decision(project_id).map(|decision| &decision.outcome) {
        Some(Outcome::Affected { reasons }) => reasons,
        other => unreachable!("{project_id} not affected: {other:?}"),
    }
}

fn graph_reason(graph: GraphKind, from: &str, via: &str) -> Reason {
    Reason::Graph {
        graph,
        from_package: from.to_string(),
        via_package: via.to_string(),
    }
}

// r[verify kiln.monorepo.graph]
#[test]
fn kiln_workspace_manifests_yield_members_path_and_workspace_edges() {
    let facts = cargo_graph(&fixture_tree(KILN_FIXTURE, &["docs/operator.md"]));
    assert_eq!(facts.kind, GraphKind::Cargo);
    let graph = available(&facts);
    assert_eq!(
        packages(graph),
        [
            ("kiln-adapter-valence", "crates/kiln-adapter-valence"),
            ("kiln-codec", "crates/kiln-codec"),
            ("kiln-core", "crates/kiln-core"),
            ("kiln-highlight", "crates/kiln-highlight"),
            ("kiln-observe-core", "crates/kiln-observe-core"),
            ("kiln-provider-api", "crates/kiln-provider-api"),
            ("kiln-standalone", "crates/kiln-standalone"),
            ("workspace:vendor/artifact", "vendor/artifact"),
            (
                "artifact-auth-core",
                "vendor/artifact/crates/artifact-auth-core"
            ),
            ("workspace:vendor/valence", "vendor/valence"),
            ("valence-core", "vendor/valence/crates/valence-core"),
        ]
    );
    assert_eq!(
        edges(graph),
        [
            ("crates/kiln-adapter-valence", "crates/kiln-core"),
            (
                "crates/kiln-adapter-valence",
                "vendor/valence/crates/valence-core"
            ),
            ("crates/kiln-codec", "crates/kiln-core"),
            ("crates/kiln-observe-core", "crates/kiln-core"),
            ("crates/kiln-provider-api", "crates/kiln-core"),
            ("crates/kiln-standalone", "crates/kiln-codec"),
            ("crates/kiln-standalone", "crates/kiln-core"),
            ("crates/kiln-standalone", "crates/kiln-provider-api"),
            (
                "vendor/artifact/crates/artifact-auth-core",
                "vendor/artifact"
            ),
            (
                "vendor/valence/crates/valence-core",
                "vendor/artifact/crates/artifact-auth-core"
            ),
            ("vendor/valence/crates/valence-core", "vendor/valence"),
        ],
        "workspace = true paths, the root [patch] and nested workspace roots"
    );
    assert_eq!(graph.roots_touching_all, CARGO_ROOTS_TOUCHING_ALL);
}

// r[verify kiln.monorepo.affected.scenario.graph]
#[test]
fn kiln_workspace_decision_follows_real_dependants() {
    let facts = cargo_graph(&fixture_tree(KILN_FIXTURE, &[]));
    let projects: Vec<ProjectInput> = [
        "kiln-adapter-valence",
        "kiln-codec",
        "kiln-core",
        "kiln-highlight",
        "kiln-observe-core",
        "kiln-provider-api",
        "kiln-standalone",
    ]
    .into_iter()
    .map(|name| scoped(name, &format!("crates/{name}"), GraphKind::Cargo))
    .collect();
    let core = affected(
        &projects,
        &ChangedFiles::exact(["crates/kiln-core/src/monorepo.rs".to_string()]),
        std::slice::from_ref(&facts),
    );
    assert_eq!(
        affected_ids(&core),
        [
            "kiln-adapter-valence",
            "kiln-codec",
            "kiln-core",
            "kiln-observe-core",
            "kiln-provider-api",
            "kiln-standalone",
        ]
    );
    assert_eq!(
        reasons(&core, "kiln-standalone"),
        [graph_reason(
            GraphKind::Cargo,
            "kiln-core",
            "kiln-standalone"
        ),]
    );
    let highlight = affected(
        &projects,
        &ChangedFiles::exact(["crates/kiln-highlight/src/lib.rs".to_string()]),
        std::slice::from_ref(&facts),
    );
    assert_eq!(affected_ids(&highlight), ["kiln-highlight"]);
    for changed in [
        "vendor/artifact/crates/artifact-auth-core/src/lib.rs",
        "vendor/valence/Cargo.toml",
    ] {
        let vendored = affected(
            &projects,
            &ChangedFiles::exact([changed.to_string()]),
            std::slice::from_ref(&facts),
        );
        assert_eq!(
            affected_ids(&vendored),
            ["kiln-adapter-valence"],
            "{changed}"
        );
    }
    let lock = affected(
        &projects,
        &ChangedFiles::exact(["Cargo.lock".to_string()]),
        std::slice::from_ref(&facts),
    );
    assert_eq!(lock.decisions.len(), affected_ids(&lock).len());
}

const MEMBER_A: &str = r#"
[package]
name = "a"
version = "0.1.0"
"#;

fn workspace_with(members: &str, extra: &str) -> String {
    format!("[workspace]\nmembers = [{members}]\n{extra}")
}

// r[verify kiln.monorepo.graph]
#[test]
fn workspace_true_resolution_renames_and_target_tables() {
    let root = workspace_with(
        r#""crates/*""#,
        r#"exclude = ["crates/docs"]
[workspace.dependencies]
a = { path = "crates/a" }
renamed = { package = "c", path = "crates/c" }
serde = "1"
"#,
    );
    let b = r#"
[package]
name = "b"
[dependencies]
a.workspace = true
serde = { workspace = true }
outside = { path = "../../../elsewhere" }
[target.'cfg(unix)'.dependencies]
d = { path = "../d" }
[target.'cfg(windows)'.build-dependencies]
e = { path = "../e" }
[dev-dependencies]
renamed = { workspace = true, features = ["x"] }
"#;
    let package = |name: &str| format!("[package]\nname = \"{name}\"\n");
    let (c, d, e) = (package("c"), package("d"), package("e"));
    let tree = tree(
        &[
            ("Cargo.toml", &root),
            ("crates/a/Cargo.toml", MEMBER_A),
            ("crates/b/Cargo.toml", b),
            ("crates/c/Cargo.toml", &c),
            ("crates/d/Cargo.toml", &d),
            ("crates/e/Cargo.toml", &e),
        ],
        &["crates/docs/README.md", "crates/a/src/lib.rs"],
    );
    let facts = cargo_graph(&tree);
    assert_eq!(
        edges(available(&facts)),
        [
            ("crates/b", "crates/a"),
            ("crates/b", "crates/c"),
            ("crates/b", "crates/d"),
            ("crates/b", "crates/e"),
        ]
    );
}

// r[verify kiln.monorepo.graph]
#[test]
fn single_package_root_and_empty_repositories() {
    let root = r#"
[package]
name = "solo"
[build-dependencies]
gen = { path = "tools/gen" }
"#;
    let gen_manifest = "[package]\nname = \"gen\"\n";
    let facts = cargo_graph(&tree(
        &[("Cargo.toml", root), ("tools/gen/Cargo.toml", gen_manifest)],
        &["src/main.rs"],
    ));
    let graph = available(&facts);
    assert_eq!(packages(graph), [("solo", ""), ("gen", "tools/gen")]);
    assert_eq!(edges(graph), [("", "tools/gen")]);
    let empty = cargo_graph(&tree(&[], &["README.md"]));
    let empty = available(&empty);
    assert!(empty.packages.is_empty() && empty.edges.is_empty());
    assert_eq!(empty.roots_touching_all, CARGO_ROOTS_TOUCHING_ALL);
}

/// Manifests, other listed paths and the expected unavailable reason fragment.
type UnavailableCase<'a> = (Vec<(&'a str, String)>, Vec<&'a str>, &'a str);

// r[verify kiln.monorepo.graph]
#[test]
fn uncertain_cargo_manifests_are_unavailable() {
    let dependent =
        |dependency: &str| format!("[package]\nname = \"b\"\n[dependencies]\n{dependency}\n");
    let cases: Vec<UnavailableCase<'_>> = vec![
        (
            vec![("Cargo.toml", "[workspace\nmembers = []".to_string())],
            vec![],
            "unparsable manifest Cargo.toml",
        ),
        (
            vec![
                (
                    "Cargo.toml",
                    workspace_with(r#""crates/a", "crates/missing""#, ""),
                ),
                ("crates/a/Cargo.toml", MEMBER_A.to_string()),
            ],
            vec![],
            "missing workspace member crates/missing/Cargo.toml",
        ),
        (
            vec![
                ("Cargo.toml", workspace_with(r#""crates/*""#, "")),
                ("crates/a/Cargo.toml", MEMBER_A.to_string()),
            ],
            vec!["crates/notes/README.md"],
            "missing workspace member crates/notes/Cargo.toml",
        ),
        (
            vec![("Cargo.toml", workspace_with(r#""crates/[ab]""#, ""))],
            vec![],
            "unsupported member pattern",
        ),
        (
            vec![
                ("Cargo.toml", workspace_with(r#""crates/b""#, "")),
                ("crates/b/Cargo.toml", dependent("a = { workspace = true }")),
            ],
            vec![],
            "unresolved workspace dependency a",
        ),
        (
            vec![("crates/b/Cargo.toml", dependent("a.workspace = true"))],
            vec![],
            "has no workspace",
        ),
        (
            vec![("crates/b/Cargo.toml", dependent(r#"a = { path = "../a" }"#))],
            vec!["crates/a/src/lib.rs"],
            "path dependency crates/a/Cargo.toml of crates/b/Cargo.toml has no package manifest",
        ),
        (
            vec![("crates/b/Cargo.toml", dependent("a = { path = 3 }"))],
            vec![],
            "non-string path",
        ),
        (
            vec![(
                "crates/b/Cargo.toml",
                "[package]\nversion = \"1\"".to_string(),
            )],
            vec![],
            "has no package name",
        ),
        (
            vec![(
                "crates/b/Cargo.toml",
                dependent(r#"a = { path = "..\\a" }"#),
            )],
            vec![],
            "unsupported path",
        ),
    ];
    for (files, other, expected) in cases {
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, text)| (*path, text.as_str()))
            .collect();
        let facts = cargo_graph(&tree(&files, &other));
        let reason = unavailable(&facts);
        assert!(reason.contains(expected), "{reason:?} lacks {expected:?}");
        assert!(!reason.contains('\n'));
    }
    let mut unsupplied = tree(&[("Cargo.toml", MEMBER_A)], &[]);
    unsupplied.files.clear();
    assert_eq!(
        unavailable(&cargo_graph(&unsupplied)),
        "manifest Cargo.toml not supplied"
    );
    let mut binary = tree(&[], &[]);
    binary.paths.insert("Cargo.toml".to_string());
    binary
        .files
        .insert("Cargo.toml".to_string(), vec![0xff, 0xfe]);
    assert_eq!(
        unavailable(&cargo_graph(&binary)),
        "manifest Cargo.toml is not UTF-8"
    );
}

// r[verify kiln.monorepo.graph]
#[test]
fn over_bound_cargo_graphs_are_unavailable() {
    let names: Vec<(String, String)> = (0..=MAX_GRAPH_PACKAGES)
        .map(|index| {
            (
                format!("p/{index}/Cargo.toml"),
                format!("[package]\nname = \"p{index}\"\n"),
            )
        })
        .collect();
    let files: Vec<(&str, &str)> = names
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    assert_eq!(
        unavailable(&cargo_graph(&tree(&files, &[]))),
        "graph exceeds the maximum package count"
    );
    let at_bound: Vec<(&str, &str)> = files.iter().copied().take(MAX_GRAPH_PACKAGES).collect();
    assert_eq!(
        available(&cargo_graph(&tree(&at_bound, &[])))
            .packages
            .len(),
        MAX_GRAPH_PACKAGES
    );
}

fn lock(nodes: &str) -> String {
    format!(
        r#"{{"nodes": {{"root": {{"inputs": {{"sub": "sub", "nixpkgs": "nixpkgs", "lib": "lib"}}}}, {nodes}}}, "root": "root", "version": 7}}"#
    )
}

const NIXPKGS_NODE: &str = r#""nixpkgs": {"locked": {"owner": "NixOS", "repo": "nixpkgs", "rev": "0", "type": "github"}, "original": {"owner": "NixOS", "repo": "nixpkgs", "type": "github"}}"#;

// r[verify kiln.monorepo.graph]
#[test]
fn flake_path_inputs_resolve_against_the_declaring_flake() {
    let root_lock = lock(&format!(
        r#"{NIXPKGS_NODE},
        "sub": {{"locked": {{"path": "./tools/sub", "type": "path"}}, "original": {{"path": "./tools/sub", "type": "path"}}, "parent": [], "inputs": {{"inner": "inner", "pinned": ["nixpkgs"]}}}},
        "inner": {{"locked": {{"path": "../inner", "type": "path"}}, "original": {{"path": "../inner", "type": "path"}}, "parent": ["sub"]}},
        "lib": {{"locked": {{"path": "/home/someone/kiln/lib", "type": "path"}}, "original": {{"path": "./lib", "type": "path"}}}},
        "escape": {{"locked": {{"path": "../../outside", "type": "path"}}}},
        "store": {{"locked": {{"path": "/nix/store/abc-src", "type": "path"}}, "original": {{"path": "/nix/store/abc-src", "type": "path"}}}}"#
    ));
    let runtime_lock = r#"{"nodes": {"root": {"inputs": {"kiln": "kiln"}}, "kiln": {"locked": {"path": "../..", "type": "path"}, "original": {"path": "../..", "type": "path"}}}, "root": "root", "version": 7}"#;
    let facts = nix_graph(&tree(
        &[
            ("flake.lock", &root_lock),
            ("runtime/cohort/flake.lock", runtime_lock),
        ],
        &[
            "flake.nix",
            "runtime/cohort/flake.nix",
            "tools/sub/default.nix",
        ],
    ));
    assert_eq!(facts.kind, GraphKind::Nix);
    let graph = available(&facts);
    assert_eq!(
        packages(graph),
        [
            (".", ""),
            ("lib", "lib"),
            ("runtime/cohort", "runtime/cohort"),
            ("tools/inner", "tools/inner"),
            ("tools/sub", "tools/sub"),
        ]
    );
    assert_eq!(
        edges(graph),
        [
            ("", "lib"),
            ("", "tools/sub"),
            ("runtime/cohort", ""),
            ("tools/sub", "tools/inner"),
        ]
    );
    assert_eq!(graph.roots_touching_all, NIX_ROOTS_TOUCHING_ALL);

    let projects = [
        scoped("cohort", "runtime/cohort", GraphKind::Nix),
        scoped("sub", "tools/sub", GraphKind::Nix),
        scoped("lib", "lib", GraphKind::Nix),
    ];
    let set = affected(
        &projects,
        &ChangedFiles::exact(["tools/inner/default.nix".to_string()]),
        std::slice::from_ref(&facts),
    );
    assert_eq!(affected_ids(&set), ["cohort", "sub"]);
    assert_eq!(
        reasons(&set, "sub"),
        [graph_reason(GraphKind::Nix, "tools/inner", "tools/sub")]
    );
    let docs = affected(
        &projects,
        &ChangedFiles::exact(["docs/x.md".to_string()]),
        std::slice::from_ref(&facts),
    );
    assert_eq!(
        affected_ids(&docs),
        ["cohort"],
        "the cohort flake imports the whole repository by path"
    );
}

// r[verify kiln.monorepo.graph]
#[test]
fn uncertain_flake_locks_are_unavailable() {
    let cases = [
        ("{not json".to_string(), "unparsable lock flake.lock"),
        (r#"{"root": "root"}"#.to_string(), "has no nodes object"),
        (r#"{"nodes": {}}"#.to_string(), "has no root node name"),
        (
            r#"{"nodes": {}, "root": "root"}"#.to_string(),
            "names a missing root node",
        ),
        (
            lock(r#""sub": {"locked": {"type": "path"}}"#),
            "path node \"sub\" has no path",
        ),
        (
            lock(r#""sub": {"locked": {"path": "./a", "type": "path"}, "parent": "sub"}"#),
            "malformed parent",
        ),
        (
            lock(r#""sub": {"locked": {"path": "./a", "type": "path"}, "parent": ["ghost"]}"#),
            "references missing input \"ghost\"",
        ),
        (
            lock(r#""sub": {"locked": {"path": "./a", "type": "path"}, "parent": ["sub"]}"#),
            "depth",
        ),
    ];
    for (text, expected) in cases {
        let facts = nix_graph(&tree(&[("flake.lock", &text)], &[]));
        let reason = unavailable(&facts);
        assert!(reason.contains(expected), "{reason:?} lacks {expected:?}");
    }
    let mut unsupplied = tree(&[], &["sub/flake.lock"]);
    unsupplied.files.clear();
    assert_eq!(
        unavailable(&nix_graph(&unsupplied)),
        "manifest sub/flake.lock not supplied"
    );
    let empty = nix_graph(&tree(&[], &["flake.nix"]));
    assert!(available(&empty).packages.is_empty());
}
