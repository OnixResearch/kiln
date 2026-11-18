use std::collections::{BTreeMap, BTreeSet};

use kiln_build_graph::{ManifestTree, cargo_graph, cargo_layout};
use kiln_core::monorepo::GraphResult;
use kiln_core::projection::build_inputs::{
    CargoLayout, CargoLayoutResult, CargoMembers, CargoPackage,
};

fn tree(files: &[(&str, &str)], other_paths: &[&str]) -> ManifestTree {
    ManifestTree {
        paths: files
            .iter()
            .map(|(path, _)| (*path).to_string())
            .chain(other_paths.iter().map(|path| (*path).to_string()))
            .collect::<BTreeSet<_>>(),
        files: files
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn available(result: &CargoLayoutResult) -> &CargoLayout {
    match result {
        CargoLayoutResult::Available(layout) => layout,
        CargoLayoutResult::Unavailable(reason) => unreachable!("layout unavailable: {reason}"),
    }
}

fn package<'a>(layout: &'a CargoLayout, dir: &str) -> &'a CargoPackage {
    layout
        .packages
        .iter()
        .find(|package| package.dir == dir)
        .unwrap_or_else(|| unreachable!("no package at {dir}"))
}

fn members<'a>(layout: &'a CargoLayout, dir: &str) -> &'a CargoMembers {
    layout
        .workspaces
        .iter()
        .find(|workspace| workspace.dir == dir)
        .map(|workspace| &workspace.members)
        .unwrap_or_else(|| unreachable!("no workspace at {dir}"))
}

fn known(literal: &[&str], glob: &[&str]) -> CargoMembers {
    CargoMembers::Known {
        literal: literal.iter().map(|dir| (*dir).to_string()).collect(),
        glob: glob.iter().map(|dir| (*dir).to_string()).collect(),
    }
}

const ROOT: &str = r#"
[workspace]
members = ["projects/alpha", "shared/*"]
exclude = ["shared/legacy"]
[workspace.package]
version = "0.1.0"
[workspace.dependencies]
proto = { path = "shared/proto" }
"#;

const ALPHA: &str = r#"
[package]
name = "alpha"
version.workspace = true
[dependencies]
proto.workspace = true
serde = "1"
[dev-dependencies]
fixtures = { path = "../../../elsewhere/fixtures" }
"#;

// r[verify kiln.source.projection.build_inputs]
#[test]
fn layouts_record_membership_and_dependencies_from_outside_the_tree() {
    let files = [
        ("Cargo.toml", ROOT),
        ("projects/alpha/Cargo.toml", ALPHA),
        ("shared/proto/Cargo.toml", "[package]\nname = \"proto\"\n"),
        ("shared/legacy/Cargo.toml", "[package]\nname = \"legacy\"\n"),
        (
            "vendor/nested/Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n",
        ),
        (
            "vendor/nested/crates/x/Cargo.toml",
            "[package]\nname = \"x\"\n",
        ),
        (
            "vendor/broken/Cargo.toml",
            "[workspace]\nmembers = \"crates\"\n",
        ),
    ];
    let listing = ["projects/alpha/src/lib.rs", "shared/proto/src/lib.rs"];
    let result = cargo_layout(&tree(&files, &listing));
    let layout = available(&result);

    let alpha = package(layout, "projects/alpha");
    assert_eq!(alpha.name, "alpha");
    assert_eq!(alpha.workspace.as_deref(), Some(""));
    assert_eq!(alpha.outside_paths, 1);
    assert!(alpha.external_packages);
    let proto = package(layout, "shared/proto");
    assert_eq!(proto.workspace.as_deref(), Some(""));
    assert_eq!((proto.outside_paths, proto.external_packages), (0, false));
    // An excluded directory is outside the workspace.
    assert_eq!(package(layout, "shared/legacy").workspace, None);
    assert_eq!(
        package(layout, "vendor/nested/crates/x")
            .workspace
            .as_deref(),
        Some("vendor/nested")
    );

    assert_eq!(
        *members(layout, ""),
        known(&["projects/alpha"], &["shared/proto"])
    );
    assert_eq!(
        *members(layout, "vendor/nested"),
        known(&[], &["vendor/nested/crates/x"])
    );
    // A nested workspace whose members cannot be read keeps the layout.
    assert!(matches!(
        members(layout, "vendor/broken"),
        CargoMembers::Unknown { reason } if reason.contains("non-array members")
    ));

    // Only path dependencies inside the repository are edges; the graph adds
    // its nested-workspace edges on top of the same parse.
    assert_eq!(
        layout.edges,
        [("projects/alpha".to_string(), "shared/proto".to_string())]
    );
    let graph = cargo_graph(&tree(&files, &listing));
    let GraphResult::Available(graph) = graph.result else {
        unreachable!("the graph of a valid tree is available");
    };
    assert!(
        graph
            .edges
            .contains(&("projects/alpha".to_string(), "shared/proto".to_string()))
    );
    assert!(graph.edges.contains(&(
        "vendor/nested/crates/x".to_string(),
        "vendor/nested".to_string()
    )));
}

#[test]
fn patches_decide_whether_a_dependency_is_inside_outside_or_external() {
    let root = r#"
[workspace]
members = ["a", "log"]
[patch.crates-io]
serde = { path = "../vendor-serde" }
log = { path = "log" }
"#;
    let a = r#"
[package]
name = "a"
[dependencies]
serde = "1"
log = "0.4"
rand = { version = "0.8" }
"#;
    let result = cargo_layout(&tree(
        &[
            ("Cargo.toml", root),
            ("a/Cargo.toml", a),
            ("log/Cargo.toml", "[package]\nname = \"log\"\n"),
        ],
        &[],
    ));
    let layout = available(&result);
    let a = package(layout, "a");
    assert_eq!((a.outside_paths, a.external_packages), (1, true));
    assert_eq!(layout.edges, [("a".to_string(), "log".to_string())]);
    let log = package(layout, "log");
    assert_eq!((log.outside_paths, log.external_packages), (0, false));
}

#[test]
fn uncertain_root_manifests_make_the_layout_unavailable() {
    let missing = cargo_layout(&tree(
        &[("Cargo.toml", "[workspace]\nmembers = [\"crates/gone\"]\n")],
        &[],
    ));
    assert!(matches!(
        missing,
        CargoLayoutResult::Unavailable(reason) if reason.contains("missing workspace member crates/gone/Cargo.toml")
    ));
    let unparsable = cargo_layout(&tree(&[("Cargo.toml", "[package")], &[]));
    assert!(matches!(
        unparsable,
        CargoLayoutResult::Unavailable(reason) if reason.contains("unparsable manifest")
    ));
}
