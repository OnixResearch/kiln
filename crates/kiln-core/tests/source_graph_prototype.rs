//! Bounded prototype for the optional graph source: one graph snapshot, two
//! project layouts and one shared entity, reproduced after a restart. Every
//! fact is synthetic; no repository is converted or published.

use kiln_core::projection::ComponentVersion;
use kiln_core::source_graph::{
    ExportOrigin, ExportRecord, FileMode, GraphLayout, GraphSnapshot, LayoutMount, Mint, Outcome,
    PackageId, SourceGraph, admit_layout, export, verify_snapshot,
};
use kiln_core::types::ContentId;

fn id(seed: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(seed.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}

fn mint(operation: &str) -> Mint {
    Mint {
        operation: id(operation),
        sequence: 0,
    }
}

fn package(graph: &mut SourceGraph, operation: &str, name: &str) -> PackageId {
    match graph.create_package(mint(operation), name) {
        Ok(Outcome::PackageCreated { package }) => package,
        other => unreachable!("{other:?}"),
    }
}

fn project(id: &str, package: &PackageId) -> GraphLayout {
    GraphLayout {
        id: id.into(),
        mounts: vec![LayoutMount {
            package: package.clone(),
            prefix: String::new(),
        }],
        generated: Vec::new(),
    }
}

fn exports(
    graph: &SourceGraph,
    layouts: &[GraphLayout],
    exporter: &ComponentVersion,
) -> Vec<ExportRecord> {
    layouts
        .iter()
        .map(|layout| {
            let admitted = admit_layout(layout, graph).unwrap_or_else(|_| unreachable!());
            export(graph, &admitted, exporter, None).unwrap_or_else(|_| unreachable!())
        })
        .collect()
}

// r[verify kiln.source.graph.identity]
#[test]
fn one_snapshot_two_layouts_and_one_shared_entity_reproduce_after_restart() {
    let mut graph = SourceGraph::new(id("prototype repository")).unwrap_or_else(|_| unreachable!());
    let library = package(&mut graph, "create library package", "library");
    let application = package(&mut graph, "create application package", "application");
    let shared = match graph.create_file(
        mint("create shared entity"),
        &library,
        "src/lib.rs",
        id("shared bytes"),
        FileMode::Regular,
    ) {
        Ok(Outcome::Created { file, .. }) => file,
        other => unreachable!("{other:?}"),
    };
    graph
        .alias(&shared, &application, "vendor/library/lib.rs")
        .unwrap_or_else(|_| unreachable!());
    graph
        .create_file(
            mint("create application entity"),
            &application,
            "src/main.rs",
            id("application bytes"),
            FileMode::Regular,
        )
        .unwrap_or_else(|_| unreachable!());
    let library_root = graph.root();
    graph
        .pin_dependency(&application, &library, library_root)
        .unwrap_or_else(|_| unreachable!());

    // Two project layouts over one snapshot; the shared entity appears in both.
    let layouts = [
        project("library-project", &library),
        project("application-project", &application),
    ];
    let exporter = ComponentVersion {
        name: "prototype-exporter".into(),
        version: "1".into(),
    };
    let before = exports(&graph, &layouts, &exporter);
    let shared_paths: Vec<&str> = before
        .iter()
        .flat_map(|record| record.entries.iter())
        .filter(|exported| matches!(&exported.origin, ExportOrigin::Member { file, .. } if *file == shared))
        .map(|exported| exported.entry.path.as_str())
        .collect();
    assert_eq!(shared_paths, ["src/lib.rs", "vendor/library/lib.rs"]);

    // Restart: persist the snapshot, drop the graph and re-admit the stored claim.
    let stored = serde_json::to_vec(&graph.snapshot()).unwrap_or_else(|_| unreachable!());
    let root = graph.root();
    drop(graph);
    let claim: GraphSnapshot = serde_json::from_slice(&stored).unwrap_or_else(|_| unreachable!());
    let restored = verify_snapshot(&claim).unwrap_or_else(|_| unreachable!());
    assert_eq!(restored.root(), root);
    assert_eq!(restored.memberships_of(&shared).count(), 2);
    assert_eq!(restored.dependencies().count(), 1);
    assert_eq!(
        exports(&restored, &layouts, &exporter),
        before,
        "the same relations reproduce the same projections"
    );
}
