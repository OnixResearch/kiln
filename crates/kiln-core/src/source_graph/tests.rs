//! Positive and negative fixtures for the optional graph source.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::projection::ComponentVersion;
use crate::types::{ContentId, Diagnostic, ExternalRef};

fn id(seed: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(seed.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}

fn external(kind: &str, seed: &str) -> ExternalRef {
    ExternalRef::new("git", kind, blake3::hash(seed.as_bytes()).to_hex().as_str())
        .unwrap_or_else(|_| unreachable!())
}

fn mint(operation: &str, sequence: u32) -> Mint {
    Mint {
        operation: id(operation),
        sequence,
    }
}

fn new_graph() -> SourceGraph {
    SourceGraph::new(id("repository")).unwrap_or_else(|_| unreachable!())
}

fn package(graph: &mut SourceGraph, operation: &str, name: &str) -> PackageId {
    match graph.create_package(mint(operation, 0), name) {
        Ok(Outcome::PackageCreated { package }) => package,
        other => unreachable!("{other:?}"),
    }
}

fn file(
    graph: &mut SourceGraph,
    operation: &str,
    package: &PackageId,
    name: &str,
    bytes: &str,
) -> FileId {
    match graph.create_file(
        mint(operation, 0),
        package,
        name,
        id(bytes),
        FileMode::Regular,
    ) {
        Ok(Outcome::Created { file, .. }) => file,
        other => unreachable!("{other:?}"),
    }
}

fn codes<T: core::fmt::Debug>(result: Result<T, Vec<Diagnostic>>) -> Vec<String> {
    match result {
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
        Ok(value) => unreachable!("expected a refusal, got {value:?}"),
    }
}

fn content(graph: &SourceGraph, file: &FileId) -> Option<ContentId> {
    graph
        .entity(file)
        .and_then(|entity| entity.revision.content())
        .map(|(content, _)| content.clone())
}

fn component(name: &str) -> ComponentVersion {
    ComponentVersion {
        name: name.into(),
        version: "1".into(),
    }
}

fn native(path: &str, kind: NativeEntryKind, bytes: &str) -> NativeEntry {
    NativeEntry {
        path: path.into(),
        kind,
        content: id(bytes),
        object: external("blob", path),
    }
}

struct Facts {
    revision: ExternalRef,
    tree: ExternalRef,
    provenance: ContentId,
    importer: ComponentVersion,
    operation: ContentId,
}

fn facts(seed: &str) -> Facts {
    Facts {
        revision: external("commit", seed),
        tree: external("tree", seed),
        provenance: id(&format!("acquired {seed}")),
        importer: component("importer"),
        operation: id(&format!("import {seed}")),
    }
}

fn request<'a>(
    layout: &'a AdmittedLayout,
    facts: &'a Facts,
    entries: &'a [NativeEntry],
    mappings: &'a [PathMapping],
) -> ImportRequest<'a> {
    ImportRequest {
        layout,
        revision: &facts.revision,
        tree: &facts.tree,
        provenance: &facts.provenance,
        importer: &facts.importer,
        operation: &facts.operation,
        entries,
        mappings,
    }
}

fn admitted(
    graph: &SourceGraph,
    layout_id: &str,
    mounts: &[(&PackageId, &str)],
    generated: Vec<GeneratedEntry>,
) -> AdmittedLayout {
    let layout = GraphLayout {
        id: layout_id.into(),
        mounts: mounts
            .iter()
            .map(|(package, prefix)| LayoutMount {
                package: (*package).clone(),
                prefix: (*prefix).into(),
            })
            .collect(),
        generated,
    };
    admit_layout(&layout, graph).unwrap_or_else(|_| unreachable!())
}

fn mapped<'a>(record: &'a ImportRecord, path: &str) -> Option<&'a FileId> {
    record
        .entries
        .iter()
        .find(|entry| entry.path == path)
        .map(|entry| &entry.file)
}

// r[verify kiln.source.graph.identity.scenario.equal_bytes]
#[test]
fn equal_bytes_from_different_operations_keep_distinct_identities() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let first = file(&mut graph, "create-a", &core, "a.txt", "same bytes");
    let second = file(&mut graph, "create-b", &core, "b.txt", "same bytes");
    assert_ne!(first, second);
    assert_eq!(content(&graph, &first), content(&graph, &second));
    // Equal bytes share a content identity, never ownership.
    let untouched = graph.entity(&second).cloned();
    assert!(matches!(
        graph.edit(&first, id("changed"), FileMode::Regular),
        Ok(Outcome::Edited { .. })
    ));
    assert_eq!(graph.entity(&second).cloned(), untouched);
    // The identity comes from the creating operation, not bytes or path.
    let mut elsewhere = new_graph();
    let elsewhere_core = package(&mut elsewhere, "package-core", "core");
    let same_operation = file(
        &mut elsewhere,
        "create-a",
        &elsewhere_core,
        "moved/renamed.txt",
        "other bytes",
    );
    assert_eq!(same_operation, first);
    let root = graph.root();
    assert_eq!(
        codes(graph.create_file(
            mint("create-a", 0),
            &core,
            "c.txt",
            id("same bytes"),
            FileMode::Regular
        )),
        ["identity_already_minted"]
    );
    assert_eq!(graph.root(), root);
}

// r[verify kiln.source.graph.identity.scenario.rename]
#[test]
fn declared_edits_and_renames_keep_identity() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let lib = file(&mut graph, "create-lib", &core, "src/lib.rs", "v1");
    let first_revision = graph.entity(&lib).map(|entity| entity.revision.id.clone());
    let Ok(Outcome::Edited {
        file: edited,
        revision,
    }) = graph.edit(&lib, id("v2"), FileMode::Executable)
    else {
        unreachable!()
    };
    assert_eq!(edited, lib);
    let entity = graph.entity(&lib).unwrap_or_else(|| unreachable!());
    assert_eq!(entity.revision.id, revision);
    assert_eq!(entity.revision.parent, first_revision);
    assert_eq!(
        entity.revision.content(),
        Some((&id("v2"), FileMode::Executable))
    );
    let before = graph.root();
    assert_eq!(
        graph.rename(&core, "src/lib.rs", "src/core.rs"),
        Ok(Outcome::Renamed {
            file: lib.clone(),
            package: core.clone(),
            from: "src/lib.rs".into(),
            to: "src/core.rs".into(),
        })
    );
    assert!(graph.membership(&core, "src/lib.rs").is_none());
    assert_eq!(
        graph.membership(&core, "src/core.rs").map(|m| &m.file),
        Some(&lib)
    );
    assert_eq!(graph.entity(&lib).map(|e| &e.revision.id), Some(&revision));
    assert_ne!(graph.root(), before);
    file(&mut graph, "create-other", &core, "src/other.rs", "other");
    let root = graph.root();
    assert_eq!(
        codes(graph.rename(&core, "src/core.rs", "src/other.rs")),
        ["name_taken"]
    );
    assert_eq!(
        codes(graph.rename(&core, "missing.rs", "found.rs")),
        ["unknown_membership"]
    );
    assert_eq!(
        codes(graph.rename(&core, "src/core.rs", "../escape.rs")),
        ["invalid_name"]
    );
    assert_eq!(
        codes(graph.edit(&lib, id("v2"), FileMode::Executable)),
        ["revision_unchanged"]
    );
    assert_eq!(graph.root(), root);
}

#[test]
fn aliases_copies_and_forks_have_distinct_relations() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let shared = file(&mut graph, "create-shared", &core, "shared.rs", "shared v1");
    assert!(matches!(
        graph.alias(&shared, &app, "vendor/shared.rs"),
        Ok(Outcome::Aliased { .. })
    ));
    assert_eq!(graph.memberships_of(&shared).count(), 2);
    assert_eq!(
        graph.membership(&app, "vendor/shared.rs").map(|m| m.kind),
        Some(MembershipKind::Alias)
    );
    graph
        .edit(&shared, id("shared v2"), FileMode::Regular)
        .unwrap_or_else(|_| unreachable!());
    let Ok(Outcome::Copied {
        file: copy, source, ..
    }) = graph.copy(mint("copy-shared", 0), &shared, &app, "copy.rs")
    else {
        unreachable!()
    };
    assert_eq!(source, shared);
    assert_ne!(copy, shared);
    let shared_revision = graph
        .entity(&shared)
        .map(|entity| entity.revision.id.clone())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        graph.entity(&copy).map(|entity| entity.provenance.clone()),
        Some(Provenance::Copied {
            file: shared.clone(),
            revision: shared_revision,
        })
    );
    assert_eq!(content(&graph, &copy), content(&graph, &shared));
    graph
        .edit(&copy, id("copy v2"), FileMode::Regular)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(content(&graph, &shared), Some(id("shared v2")));
    let Ok(Outcome::Forked {
        package: fork,
        source: forked,
        files,
    }) = graph.fork_package(id("fork-app"), &app, "app-fork")
    else {
        unreachable!()
    };
    assert_eq!(forked, app);
    assert_eq!(
        graph.package(&fork).and_then(|p| p.forked_from.clone()),
        Some(app.clone())
    );
    assert_eq!(files.len(), 2);
    for (original, forked_file) in &files {
        assert_ne!(original, forked_file);
        assert!(matches!(
            graph.entity(forked_file).map(|entity| &entity.provenance),
            Some(Provenance::Forked { package, file, .. }) if *package == app && file == original
        ));
    }
    assert_eq!(
        graph
            .package_memberships(&fork)
            .map(|membership| (membership.name.as_str(), membership.kind))
            .collect::<Vec<_>>(),
        [
            ("copy.rs", MembershipKind::Primary),
            ("vendor/shared.rs", MembershipKind::Primary)
        ]
    );
    let root = graph.root();
    assert_eq!(codes(graph.alias(&shared, &app, "copy.rs")), ["name_taken"]);
    assert_eq!(
        codes(graph.fork_package(id("fork-again"), &app, "app-fork")),
        ["package_name_taken"]
    );
    assert_eq!(
        codes(graph.copy(
            mint("copy-unknown", 0),
            &FileId(id("missing")),
            &app,
            "unknown.rs"
        )),
        ["unknown_entity"]
    );
    assert_eq!(graph.root(), root);
}

// r[verify kiln.source.graph.relations.scenario.removal]
#[test]
fn removal_outcomes_stay_distinct() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let shared = file(&mut graph, "create-shared", &core, "shared.rs", "bytes");
    graph
        .alias(&shared, &app, "shared.rs")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        graph.remove_membership(&core, "shared.rs"),
        Ok(Outcome::MembershipRemoved {
            file: shared.clone(),
            remaining: 1,
        })
    );
    assert_eq!(
        graph.remove_membership(&app, "shared.rs"),
        Ok(Outcome::AliasRemoved {
            file: shared.clone(),
            remaining: 0,
        })
    );
    let entity = graph.entity(&shared).unwrap_or_else(|| unreachable!());
    assert!(
        !entity.is_deleted(),
        "only an explicit deletion removes the entity"
    );
    let live_revision = entity.revision.id.clone();
    let Ok(Outcome::Deleted {
        file: deleted,
        revision,
        memberships,
    }) = graph.delete(&shared)
    else {
        unreachable!()
    };
    assert_eq!((deleted, memberships), (shared.clone(), 0));
    let entity = graph.entity(&shared).unwrap_or_else(|| unreachable!());
    assert!(entity.is_deleted());
    assert_eq!(entity.revision.id, revision);
    assert_eq!(entity.revision.parent, Some(live_revision));
    let other = file(&mut graph, "create-other", &core, "other.rs", "bytes");
    graph
        .alias(&other, &app, "other.rs")
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        graph.delete(&other),
        Ok(Outcome::Deleted { memberships: 2, .. })
    ));
    assert_eq!(graph.memberships_of(&other).count(), 0);
    let root = graph.root();
    assert_eq!(codes(graph.delete(&shared)), ["entity_deleted"]);
    assert_eq!(
        codes(graph.edit(&shared, id("again"), FileMode::Regular)),
        ["entity_deleted"]
    );
    assert_eq!(
        codes(graph.alias(&shared, &app, "back.rs")),
        ["entity_deleted"]
    );
    assert_eq!(
        codes(graph.copy(mint("copy", 0), &shared, &app, "copy.rs")),
        ["entity_deleted"]
    );
    assert_eq!(
        codes(graph.remove_membership(&app, "shared.rs")),
        ["unknown_membership"]
    );
    assert_eq!(graph.root(), root);
}

#[test]
fn dependencies_pin_exact_selections() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let pinned = graph.root();
    assert!(matches!(
        graph.pin_dependency(&app, &core, pinned.clone()),
        Ok(Outcome::DependencyPinned { .. })
    ));
    let first = graph.root();
    graph
        .pin_dependency(&app, &core, id("newer"))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(graph.dependencies().count(), 1);
    assert_ne!(graph.root(), first, "a re-pin is a different snapshot");
    let root = graph.root();
    assert_eq!(
        codes(graph.pin_dependency(&app, &app, pinned.clone())),
        ["self_dependency"]
    );
    assert_eq!(
        codes(graph.pin_dependency(&app, &PackageId(id("missing")), pinned)),
        ["unknown_package"]
    );
    assert_eq!(graph.root(), root);
}

// r[verify kiln.source.graph.identity]
#[test]
fn stored_snapshots_reproduce_the_root_and_refuse_tampering() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let shared = file(&mut graph, "create-shared", &core, "shared.rs", "bytes");
    graph
        .alias(&shared, &app, "vendor/shared.rs")
        .unwrap_or_else(|_| unreachable!());
    let deleted = file(&mut graph, "create-deleted", &core, "deleted.rs", "bytes");
    graph.delete(&deleted).unwrap_or_else(|_| unreachable!());
    graph
        .pin_dependency(&app, &core, id("pinned"))
        .unwrap_or_else(|_| unreachable!());
    let stored = serde_json::to_string(&graph.snapshot()).unwrap_or_else(|_| unreachable!());
    let claim: GraphSnapshot = serde_json::from_str(&stored).unwrap_or_else(|_| unreachable!());
    let restored = verify_snapshot(&claim).unwrap_or_else(|_| unreachable!());
    assert_eq!(restored, graph);
    assert_eq!(restored.snapshot(), graph.snapshot());
    let tampered = |change: &dyn Fn(&mut GraphSnapshot)| {
        let mut forged = claim.clone();
        change(&mut forged);
        codes(verify_snapshot(&forged))
    };
    assert_eq!(
        tampered(&|forged| {
            if let Some(entity) = forged.entities.first_mut() {
                entity.revision.state = RevisionState::Content {
                    content: id("other"),
                    mode: FileMode::Regular,
                };
            }
        }),
        ["forged_revision"]
    );
    assert_eq!(
        tampered(&|forged| {
            if let Some(entity) = forged.entities.first_mut() {
                entity.mint.sequence = 9;
            }
        }),
        ["forged_identity"]
    );
    assert_eq!(
        tampered(&|forged| forged.root = id("other root")),
        ["snapshot_root_mismatch"]
    );
    assert_eq!(
        tampered(&|forged| forged.dependencies.clear()),
        ["snapshot_root_mismatch"]
    );
    assert_eq!(
        tampered(&|forged| forged.schema = "kiln.source-graph.snapshot.v0".into()),
        ["unsupported_schema"]
    );
    assert_eq!(
        tampered(&|forged| {
            if let Some(membership) = forged.memberships.first_mut() {
                membership.file = deleted.clone();
            }
        }),
        ["entity_deleted"]
    );
    assert_eq!(
        tampered(&|forged| {
            if let Some(membership) = forged.memberships.first().cloned() {
                forged.memberships.push(membership);
            }
        }),
        ["duplicate_record"]
    );
}

#[test]
fn names_identities_and_bounds_are_admitted_before_any_change() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let root = graph.root();
    let long = "a".repeat(MAX_NAME_BYTES + 1);
    for name in ["", "/absolute", "a/../b", "a//b", "a\\b", long.as_str()] {
        assert_eq!(
            codes(graph.create_file(
                mint("create", 0),
                &core,
                name,
                id("bytes"),
                FileMode::Regular
            )),
            ["invalid_name"],
            "{name:?}"
        );
    }
    assert_eq!(
        codes(graph.create_package(mint("package-core", 0), "other")),
        ["identity_already_minted"]
    );
    assert_eq!(
        codes(graph.create_package(mint("package-other", 0), "core")),
        ["package_name_taken"]
    );
    assert_eq!(
        codes(graph.create_file(
            mint("create", 0),
            &PackageId(id("missing")),
            "a.rs",
            id("bytes"),
            FileMode::Regular
        )),
        ["unknown_package"]
    );
    let forged: ContentId =
        serde_json::from_str("\"not-a-digest\"").unwrap_or_else(|_| unreachable!());
    assert_eq!(
        codes(graph.create_file(
            mint("create", 0),
            &core,
            "a.rs",
            forged.clone(),
            FileMode::Regular
        )),
        ["invalid_identity"]
    );
    assert_eq!(codes(SourceGraph::new(forged)), ["invalid_identity"]);
    assert_eq!(graph.root(), root);
}

#[test]
fn layouts_admit_one_mount_per_package_without_nesting() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let mount = |package: &PackageId, prefix: &str| LayoutMount {
        package: package.clone(),
        prefix: prefix.into(),
    };
    let admit = |mounts: Vec<LayoutMount>, generated: Vec<GeneratedEntry>| {
        admit_layout(
            &GraphLayout {
                id: "layout".into(),
                mounts,
                generated,
            },
            &graph,
        )
    };
    let good = admit(
        vec![mount(&app, "apps/app"), mount(&core, "crates/core")],
        Vec::new(),
    )
    .unwrap_or_else(|_| unreachable!());
    let reordered = admit(
        vec![mount(&core, "crates/core"), mount(&app, "apps/app")],
        Vec::new(),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        good.identity(),
        reordered.identity(),
        "declaration order never changes the identity"
    );
    assert_eq!(
        codes(admit(
            vec![mount(&core, ""), mount(&app, "apps")],
            Vec::new()
        )),
        ["overlapping_mounts"]
    );
    assert_eq!(
        codes(admit(
            vec![mount(&core, "crates"), mount(&app, "crates/app")],
            Vec::new()
        )),
        ["overlapping_mounts"]
    );
    assert_eq!(
        codes(admit(
            vec![mount(&core, "a"), mount(&core, "b")],
            Vec::new()
        )),
        ["duplicate_mount"]
    );
    assert_eq!(
        codes(admit(
            vec![mount(&PackageId(id("missing")), "x")],
            Vec::new()
        )),
        ["unknown_package"]
    );
    assert_eq!(
        codes(admit(vec![mount(&core, "../x")], Vec::new())),
        ["invalid_prefix"]
    );
    assert_eq!(codes(admit(Vec::new(), Vec::new())), ["bound_exceeded"]);
    let lock = GeneratedEntry {
        path: "Cargo.lock".into(),
        content: id("lock"),
        mode: FileMode::Regular,
        derived: true,
    };
    assert_eq!(
        codes(admit(
            vec![mount(&core, "crates/core")],
            vec![lock.clone(), lock]
        )),
        ["duplicate_generated_entry"]
    );
}

// r[verify kiln.source.graph.interchange.scenario.ambiguous]
#[test]
fn ambiguous_continuity_mints_new_identities_until_mapped() {
    let mut graph = new_graph();
    let root_package = package(&mut graph, "package-root", "root");
    let layout = admitted(&graph, "repository", &[(&root_package, "")], Vec::new());
    let first = facts("first");
    let tree = [
        native("a.txt", NativeEntryKind::File, "same"),
        native("b.txt", NativeEntryKind::File, "other"),
    ];
    let (imported, first_record) = import(&graph, None, &request(&layout, &first, &tree, &[]))
        .unwrap_or_else(|_| unreachable!());
    assert!(
        first_record.unresolved.is_empty(),
        "a first import has no continuity to resolve"
    );
    assert_eq!(first_record.verify(), Ok(()));
    let a = mapped(&first_record, "a.txt")
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let b = mapped(&first_record, "b.txt")
        .cloned()
        .unwrap_or_else(|| unreachable!());
    // a.txt is deleted and its bytes reappear at c.txt; equal bytes decide nothing.
    let second = facts("second");
    let moved = [
        native("b.txt", NativeEntryKind::File, "other"),
        native("c.txt", NativeEntryKind::File, "same"),
    ];
    let (after, record) = import(
        &imported,
        Some(&first_record),
        &request(&layout, &second, &moved, &[]),
    )
    .unwrap_or_else(|_| unreachable!());
    let c = mapped(&record, "c.txt")
        .cloned()
        .unwrap_or_else(|| unreachable!());
    assert_ne!(c, a);
    assert_eq!(mapped(&record, "b.txt"), Some(&b));
    assert_eq!(
        record.unresolved,
        [PathMapping {
            path: "c.txt".into(),
            file: c.clone(),
        }]
    );
    assert_eq!(
        record.removed,
        [PathMapping {
            path: "a.txt".into(),
            file: a.clone(),
        }]
    );
    assert_eq!(record.deleted, core::slice::from_ref(&a));
    assert!(after.entity(&a).is_some_and(Entity::is_deleted));
    assert_eq!(
        after.entity(&c).map(|entity| entity.provenance.clone()),
        Some(Provenance::Imported)
    );
    // An explicit mapping states the continuity instead.
    let mapping = [PathMapping {
        path: "c.txt".into(),
        file: a.clone(),
    }];
    let (renamed, explicit) = import(
        &imported,
        Some(&first_record),
        &request(&layout, &second, &moved, &mapping),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(mapped(&explicit, "c.txt"), Some(&a));
    assert!(explicit.unresolved.is_empty() && explicit.deleted.is_empty());
    assert_eq!(
        renamed.membership(&root_package, "c.txt").map(|m| &m.file),
        Some(&a)
    );
    assert_eq!(
        renamed.entity(&a).map(|entity| &entity.revision.id),
        imported.entity(&a).map(|entity| &entity.revision.id),
        "a mapped rename keeps the revision"
    );
    // A previous import must still describe the graph; mappings name held entities.
    assert_eq!(
        codes(import(
            &after,
            Some(&first_record),
            &request(&layout, &second, &moved, &[])
        )),
        ["stale_previous_import"]
    );
    let unknown = [PathMapping {
        path: "c.txt".into(),
        file: FileId(id("missing")),
    }];
    assert_eq!(
        codes(import(
            &imported,
            Some(&first_record),
            &request(&layout, &second, &moved, &unknown)
        )),
        ["unknown_entity"]
    );
}

// r[verify kiln.source.graph.interchange]
#[test]
fn unsupported_generated_and_unmapped_entries_are_refused() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let generated = vec![
        GeneratedEntry {
            path: "Cargo.lock".into(),
            content: id("lock"),
            mode: FileMode::Regular,
            derived: true,
        },
        GeneratedEntry {
            path: "BUILD.generated".into(),
            content: id("build"),
            mode: FileMode::Regular,
            derived: false,
        },
    ];
    let layout = admitted(&graph, "core-project", &[(&core, "src")], generated);
    let tree_facts = facts("tree");
    let refused = |entries: &[NativeEntry]| {
        codes(import(
            &graph,
            None,
            &request(&layout, &tree_facts, entries, &[]),
        ))
    };
    assert_eq!(
        refused(&[native("src/link", NativeEntryKind::Symlink, "target")]),
        ["unsupported_entry"]
    );
    assert_eq!(
        refused(&[native("src/vendor", NativeEntryKind::Submodule, "commit")]),
        ["unsupported_entry"]
    );
    assert_eq!(
        refused(&[native("src/fifo", NativeEntryKind::Special, "fifo")]),
        ["unsupported_entry"]
    );
    assert_eq!(
        refused(&[native("BUILD.generated", NativeEntryKind::File, "build")]),
        ["generated_entry"]
    );
    assert_eq!(
        refused(&[native("docs/readme.md", NativeEntryKind::File, "doc")]),
        ["unmapped_path"]
    );
    assert_eq!(
        refused(&[
            native("src/a.rs", NativeEntryKind::File, "a"),
            native("src/a.rs", NativeEntryKind::File, "b"),
        ]),
        ["duplicate_path"]
    );
    assert_eq!(
        refused(&[native("src/../a.rs", NativeEntryKind::File, "a")]),
        ["invalid_path"]
    );
    // A derived generated entry is regenerated, never imported as source.
    let entries = [
        native("Cargo.lock", NativeEntryKind::File, "lock"),
        native("src/lib.rs", NativeEntryKind::Executable, "lib"),
    ];
    let (imported, record) = import(&graph, None, &request(&layout, &tree_facts, &entries, &[]))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        record
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["src/lib.rs"]
    );
    let lib = mapped(&record, "src/lib.rs")
        .cloned()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        imported
            .entity(&lib)
            .and_then(|entity| entity.revision.content())
            .map(|(_, mode)| mode),
        Some(FileMode::Executable)
    );
    assert_eq!(
        imported.membership(&core, "lib.rs").map(|m| &m.file),
        Some(&lib)
    );
}

// r[verify kiln.source.graph.interchange.scenario.round_trip]
#[test]
fn export_round_trip_preserves_mappings_and_edits_stay_local() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let app = package(&mut graph, "package-app", "app");
    let lib = file(&mut graph, "create-lib", &core, "src/lib.rs", "lib");
    let tool = match graph.create_file(
        mint("create-tool", 0),
        &core,
        "bin/tool",
        id("tool"),
        FileMode::Executable,
    ) {
        Ok(Outcome::Created { file, .. }) => file,
        other => unreachable!("{other:?}"),
    };
    let main = file(&mut graph, "create-main", &app, "src/main.rs", "main");
    graph
        .alias(&lib, &app, "vendor/lib.rs")
        .unwrap_or_else(|_| unreachable!());
    let lock = GeneratedEntry {
        path: "Cargo.lock".into(),
        content: id("lock"),
        mode: FileMode::Regular,
        derived: true,
    };
    let layout = admitted(
        &graph,
        "workspace",
        &[(&core, "crates/core"), (&app, "apps/app")],
        vec![lock],
    );
    let exporter = component("exporter");
    let exported = export(&graph, &layout, &exporter, None).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        export(&graph, &layout, &exporter, None),
        Ok(exported.clone()),
        "exports are deterministic"
    );
    assert_eq!(exported.verify(), Ok(()));
    assert_eq!(
        exported
            .tree_entries()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        [
            "Cargo.lock",
            "apps/app/src/main.rs",
            "apps/app/vendor/lib.rs",
            "crates/core/bin/tool",
            "crates/core/src/lib.rs",
        ]
    );
    assert_eq!(
        exported
            .tree_entries()
            .find(|entry| entry.path == "crates/core/bin/tool")
            .map(|entry| entry.mode),
        Some(FileMode::Executable)
    );
    // Re-import the exported tree with the export's own mappings.
    let tree: Vec<NativeEntry> = exported
        .tree_entries()
        .map(|entry| NativeEntry {
            path: entry.path.clone(),
            kind: match entry.mode {
                FileMode::Regular => NativeEntryKind::File,
                FileMode::Executable => NativeEntryKind::Executable,
            },
            content: entry.content.clone(),
            object: external("blob", &entry.path),
        })
        .collect();
    let mappings = exported.mappings();
    let round_trip = facts("round-trip");
    let (same, record) = import(
        &graph,
        None,
        &request(&layout, &round_trip, &tree, &mappings),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        same.root(),
        graph.root(),
        "a round trip preserves the declared graph"
    );
    assert!(record.unresolved.is_empty() && record.removed.is_empty() && record.deleted.is_empty());
    assert_eq!(
        record
            .entries
            .iter()
            .map(|entry| PathMapping {
                path: entry.path.clone(),
                file: entry.file.clone(),
            })
            .collect::<Vec<_>>(),
        mappings
    );
    // An accepted projected edit changes only its mapped entity.
    let edited: Vec<NativeEntry> = tree
        .iter()
        .cloned()
        .map(|mut entry| {
            if entry.path == "apps/app/src/main.rs" {
                entry.content = id("main v2");
            }
            entry
        })
        .collect();
    let (changed, _) = import(
        &graph,
        None,
        &request(&layout, &round_trip, &edited, &mappings),
    )
    .unwrap_or_else(|_| unreachable!());
    for entity in graph.entities() {
        let after = changed.entity(&entity.id).unwrap_or_else(|| unreachable!());
        if entity.id == main {
            assert_eq!(after.revision.parent.as_ref(), Some(&entity.revision.id));
            assert_eq!(content(&changed, &main), Some(id("main v2")));
        } else {
            assert_eq!(after, entity);
        }
    }
    assert_eq!(
        changed.memberships().collect::<Vec<_>>(),
        graph.memberships().collect::<Vec<_>>()
    );
    // Two paths of one entity cannot diverge.
    let diverged: Vec<NativeEntry> = tree
        .iter()
        .cloned()
        .map(|mut entry| {
            if entry.path == "apps/app/vendor/lib.rs" {
                entry.content = id("diverged");
            }
            entry
        })
        .collect();
    assert_eq!(
        codes(import(
            &graph,
            None,
            &request(&layout, &round_trip, &diverged, &mappings)
        )),
        ["contradictory_alias"]
    );
    // The next export of the layout preserves a deletion.
    let mut without_tool = graph.clone();
    without_tool
        .delete(&tool)
        .unwrap_or_else(|_| unreachable!());
    let next = export(&without_tool, &layout, &exporter, Some(&exported))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(next.deletions, ["crates/core/bin/tool"]);
    assert!(
        next.tree_entries()
            .all(|entry| entry.path != "crates/core/bin/tool")
    );
    // A path claimed twice, or used as both a file and a directory, is refused.
    for path in ["crates/core/src/lib.rs", "crates/core/src"] {
        let clash = GeneratedEntry {
            path: path.into(),
            content: id("generated"),
            mode: FileMode::Regular,
            derived: true,
        };
        let clashing = admitted(&graph, "clash", &[(&core, "crates/core")], vec![clash]);
        assert_eq!(
            codes(export(&graph, &clashing, &exporter, None)),
            ["export_path_conflict"],
            "{path}"
        );
    }
    let mut forged = exported.clone();
    forged.root = id("forged root");
    assert_eq!(codes(forged.verify()), ["forged_record"]);
}

fn graph_state(root: &ContentId, native: &ExternalRef) -> AuthorityState {
    AuthorityState {
        repository: id("repository"),
        mode: AuthorityMode::GraphAuthoritative,
        owner: "local:owner".into(),
        graph_root: Some(root.clone()),
        native_revision: Some(native.clone()),
    }
}

fn observed(root: &ContentId, native: &ExternalRef) -> ObservedState {
    ObservedState {
        graph_root: Some(root.clone()),
        native_revision: Some(native.clone()),
    }
}

// r[verify kiln.source.graph.authority.scenario.conflict]
#[test]
fn independent_updates_are_refused_and_reconciled_explicitly() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    file(&mut graph, "create-lib", &core, "lib.rs", "lib");
    let layout = admitted(&graph, "workspace", &[(&core, "")], Vec::new());
    let exported =
        export(&graph, &layout, &component("exporter"), None).unwrap_or_else(|_| unreachable!());
    let root = graph.root();
    let native_ref = external("commit", "exported");
    let state = graph_state(&root, &native_ref);
    let graph_publication = |publisher: &str, expected: &ContentId| Publication {
        publisher: publisher.into(),
        target: PublicationTarget::GraphRoot {
            expected: Some(expected.clone()),
            next: id("next root"),
        },
    };
    let published = admit_publication(
        &state,
        &observed(&root, &native_ref),
        &graph_publication("local:owner", &root),
        None,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(published.graph_root, Some(id("next root")));
    assert_eq!(
        codes(admit_publication(
            &published,
            &observed(&id("next root"), &native_ref),
            &graph_publication("local:owner", &root),
            None
        )),
        ["stale_expected_state"]
    );
    assert_eq!(
        codes(admit_publication(
            &state,
            &observed(&root, &native_ref),
            &graph_publication("local:intruder", &root),
            None
        )),
        ["not_publication_owner"]
    );
    // The declared export ref moved independently; no export converges over it.
    let moved = external("commit", "pushed outside Kiln");
    let export_publication = |expected: &ExternalRef| Publication {
        publisher: "local:owner".into(),
        target: PublicationTarget::NativeRevision {
            expected: Some(expected.clone()),
            next: external("commit", "re-export"),
        },
    };
    assert_eq!(
        codes(admit_publication(
            &state,
            &observed(&root, &moved),
            &export_publication(&native_ref),
            Some(&exported)
        )),
        ["native_moved_independently"]
    );
    assert_eq!(
        codes(admit_publication(
            &state,
            &observed(&root, &moved),
            &export_publication(&moved),
            Some(&exported)
        )),
        ["native_moved_independently", "stale_expected_state"]
    );
    // The owner reconciles explicitly; the declared export then replaces the moved ref.
    let reconciled = reconcile(
        &state,
        &observed(&root, &moved),
        &Reconciliation {
            publisher: "local:owner".into(),
            replace_native: moved.clone(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let replaced = admit_publication(
        &reconciled,
        &observed(&root, &moved),
        &export_publication(&moved),
        Some(&exported),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        replaced.native_revision,
        Some(external("commit", "re-export"))
    );
    assert_eq!(
        codes(admit_publication(
            &reconciled,
            &observed(&root, &moved),
            &export_publication(&moved),
            None
        )),
        ["export_required"]
    );
    let older = graph_state(&id("older root"), &moved);
    assert_eq!(
        codes(admit_publication(
            &older,
            &observed(&id("older root"), &moved),
            &export_publication(&moved),
            Some(&exported)
        )),
        ["export_not_current_root"]
    );
    assert_eq!(
        codes(reconcile(
            &state,
            &observed(&root, &moved),
            &Reconciliation {
                publisher: "local:owner".into(),
                replace_native: native_ref.clone(),
            }
        )),
        ["stale_expected_state"]
    );
}

#[test]
fn native_authority_publishes_natively_and_derives_the_graph() {
    let mut graph = new_graph();
    let root_package = package(&mut graph, "package-root", "root");
    let layout = admitted(&graph, "repository", &[(&root_package, "")], Vec::new());
    let first = facts("first");
    let state = AuthorityState::native(
        id("repository"),
        "local:owner",
        Some(first.revision.clone()),
    )
    .unwrap_or_else(|_| unreachable!());
    let tree = [native("a.txt", NativeEntryKind::File, "a")];
    let (_, record) = import(&graph, None, &request(&layout, &first, &tree, &[]))
        .unwrap_or_else(|_| unreachable!());
    let derived = record_import(&state, &record).unwrap_or_else(|_| unreachable!());
    assert_eq!(derived.graph_root, Some(record.root.clone()));
    let seen = ObservedState {
        graph_root: derived.graph_root.clone(),
        native_revision: Some(first.revision.clone()),
    };
    let graph_publication = Publication {
        publisher: "local:owner".into(),
        target: PublicationTarget::GraphRoot {
            expected: derived.graph_root.clone(),
            next: id("graph edit"),
        },
    };
    assert_eq!(
        codes(admit_publication(&derived, &seen, &graph_publication, None)),
        ["non_authoritative_side"]
    );
    let second = external("commit", "second");
    let native_publication = |expected: &ExternalRef| Publication {
        publisher: "local:owner".into(),
        target: PublicationTarget::NativeRevision {
            expected: Some(expected.clone()),
            next: second.clone(),
        },
    };
    let advanced = admit_publication(&derived, &seen, &native_publication(&first.revision), None)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(advanced.native_revision, Some(second.clone()));
    let now = ObservedState {
        graph_root: advanced.graph_root.clone(),
        native_revision: Some(second.clone()),
    };
    assert_eq!(
        codes(admit_publication(
            &advanced,
            &now,
            &native_publication(&first.revision),
            None
        )),
        ["stale_expected_state"]
    );
    // An import of a superseded native revision cannot become the derived graph.
    assert_eq!(codes(record_import(&advanced, &record)), ["stale_import"]);
    let graph_mode = AuthorityState {
        mode: AuthorityMode::GraphAuthoritative,
        ..state.clone()
    };
    assert_eq!(
        codes(record_import(&graph_mode, &record)),
        ["non_authoritative_side"]
    );
    assert_eq!(
        codes(AuthorityState::native(id("repository"), " ", None)),
        ["invalid_owner"]
    );
}

#[test]
fn mode_migration_is_explicit_and_binds_both_sides() {
    let native_ref = external("commit", "last native");
    let root = id("first graph root");
    let state = AuthorityState {
        repository: id("repository"),
        mode: AuthorityMode::NativeAuthoritative,
        owner: "local:owner".into(),
        graph_root: Some(root.clone()),
        native_revision: Some(native_ref.clone()),
    };
    let seen = observed(&root, &native_ref);
    let migration = ModeMigration {
        publisher: "local:owner".into(),
        to: AuthorityMode::GraphAuthoritative,
        owner: "local:graph-owner".into(),
        native_revision: native_ref.clone(),
        graph_root: root.clone(),
    };
    let migrated = migrate(&state, &seen, &migration).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        (migrated.mode, migrated.owner.as_str()),
        (AuthorityMode::GraphAuthoritative, "local:graph-owner")
    );
    let rollback = ModeMigration {
        publisher: "local:graph-owner".into(),
        to: AuthorityMode::NativeAuthoritative,
        owner: "local:owner".into(),
        native_revision: native_ref.clone(),
        graph_root: root.clone(),
    };
    assert_eq!(
        migrate(&migrated, &seen, &rollback).map(|state| state.mode),
        Ok(AuthorityMode::NativeAuthoritative)
    );
    assert_eq!(
        codes(migrate(
            &state,
            &seen,
            &ModeMigration {
                publisher: "local:intruder".into(),
                ..migration.clone()
            }
        )),
        ["not_publication_owner"]
    );
    assert_eq!(
        codes(migrate(
            &state,
            &seen,
            &ModeMigration {
                to: AuthorityMode::NativeAuthoritative,
                ..migration.clone()
            }
        )),
        ["mode_unchanged"]
    );
    assert_eq!(
        codes(migrate(
            &state,
            &seen,
            &ModeMigration {
                graph_root: id("other root"),
                ..migration.clone()
            }
        )),
        ["stale_migration"]
    );
    let moved = ObservedState {
        native_revision: Some(external("commit", "moved")),
        ..seen.clone()
    };
    assert_eq!(
        codes(migrate(&state, &moved, &migration)),
        ["stale_migration"]
    );
    assert_eq!(
        codes(migrate(
            &state,
            &seen,
            &ModeMigration {
                owner: " ".into(),
                ..migration
            }
        )),
        ["invalid_owner"]
    );
}

// r[verify kiln.source.graph.authority]
#[test]
fn evidence_is_inapplicable_when_any_bound_identity_changes() {
    let mut graph = new_graph();
    let core = package(&mut graph, "package-core", "core");
    let lib = file(&mut graph, "create-lib", &core, "lib.rs", "lib");
    let layout = admitted(&graph, "workspace", &[(&core, "")], Vec::new());
    let exporter = component("exporter");
    let exported = export(&graph, &layout, &exporter, None).unwrap_or_else(|_| unreachable!());
    let importer = component("importer");
    let bound = EvidenceSubject::of_export(&exported, Some(&importer));
    assert_eq!(evidence_applicable(&bound, &bound.clone()), Ok(()));
    let changed = |change: &dyn Fn(&mut EvidenceSubject)| {
        let mut current = bound.clone();
        change(&mut current);
        codes(evidence_applicable(&bound, &current))
    };
    assert_eq!(
        changed(&|subject| subject.schema = "kiln.source-graph.snapshot.v2".into()),
        ["evidence_schema_changed"]
    );
    assert_eq!(
        changed(&|subject| subject.graph_root = id("other root")),
        ["evidence_graph_root_changed"]
    );
    assert_eq!(
        changed(&|subject| subject.importer = Some(ComponentVersion {
            name: "importer".into(),
            version: "2".into(),
        })),
        ["evidence_importer_changed"]
    );
    assert_eq!(
        changed(&|subject| subject.exporter = None),
        ["evidence_exporter_changed"]
    );
    assert_eq!(
        changed(&|subject| subject.layout = Some(id("other layout"))),
        ["evidence_layout_changed"]
    );
    assert_eq!(
        changed(&|subject| subject.export = Some(id("other export"))),
        ["evidence_export_changed"]
    );
    // A graph edit moves the root and the export subject together.
    graph
        .edit(&lib, id("lib v2"), FileMode::Regular)
        .unwrap_or_else(|_| unreachable!());
    let current = export(&graph, &layout, &exporter, None).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        codes(evidence_applicable(
            &bound,
            &EvidenceSubject::of_export(&current, Some(&importer))
        )),
        ["evidence_export_changed", "evidence_graph_root_changed"]
    );
}
