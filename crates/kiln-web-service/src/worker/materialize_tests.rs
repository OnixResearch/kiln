use super::*;
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializerKind, MaterializerProfile, plan,
};
use kiln_core::projection::{
    ComponentVersion, Coverage, DependencyFact, DependencyStatus, EntryKind, ProjectionDefinition,
    ProjectionFacts, ProjectionOperation, ProviderBinding, RepositoryEnrollment, RepositoryLocator,
    SourceCapability, SourceEntry, TargetProfile, admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ExternalRef, ProviderProfileId};

// Local Git fixture only: no worker, dispatch or authority is involved.
const DEFINITION: &str = include_str!("../../../../fixtures/source-projection/definition.json");
const CARGO: &[u8] = b"[package]\nname = \"kiln-core\"\n";
const LIB: &[u8] = b"pub fn core() {}\n";
const RUN: &[u8] = b"#!/bin/sh\nexit 0\n";

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn content(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
    ok(ExternalRef::new(namespace, kind, id))
}

fn private(path: PathBuf) -> PathBuf {
    ok(fs::DirBuilder::new().mode(PRIVATE_DIRECTORY).create(&path));
    path
}

struct Fixture {
    root: tempfile::TempDir,
    repository: Repository,
    store: PathBuf,
    oids: [Oid; 3],
}

fn fixture() -> Fixture {
    let root = ok(tempfile::tempdir());
    let repository = ok(Repository::init_bare(root.path().join("source.git")));
    let oids = [CARGO, LIB, RUN].map(|bytes| ok(repository.blob(bytes)));
    let store = private(root.path().join("store"));
    Fixture {
        root,
        repository,
        store,
        oids,
    }
}

fn materialization_plan(entries: &[(&str, Oid, EntryKind)]) -> MaterializationPlan {
    let enrolled = [RepositoryEnrollment {
        repository_id: "kiln".into(),
        binding: ProviderBinding {
            source_provider: ok(ProviderProfileId::new("git-source-v1")),
            native_repository: external("github", "repository", "OnixResearch/kiln"),
        },
        locators: vec![RepositoryLocator::LocalPath {
            path: "/srv/cache/kiln".into(),
        }],
    }];
    let repository = ok(admit_repositories(&enrolled, &[]))
        .pop()
        .unwrap_or_else(|| unreachable!("one admitted repository"));
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let facts = ProjectionFacts {
        repository: repository.identity().clone(),
        exact_revision: external("git", "commit", &"a".repeat(40)),
        canonical_tree: content("tree"),
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree, SourceCapability::ExecutableBit],
        coverage: Coverage::Complete,
        entries: entries
            .iter()
            .map(|(path, oid, kind)| SourceEntry {
                path: (*path).into(),
                object: external("git", "blob", &oid.to_string()),
                kind: *kind,
            })
            .collect(),
        dependencies: vec![DependencyFact {
            name: "workspace-manifest".into(),
            status: DependencyStatus::Present {
                identity: content("workspace-manifest"),
            },
        }],
        generated: Vec::new(),
    };
    let definition: ProjectionDefinition = ok(serde_json::from_str(DEFINITION));
    let complete = ok(ok(resolve(
        &ok(admit_definition(&definition)),
        &repository,
        &facts,
    ))
    .require_complete(ProjectionOperation::Build));
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: component("kiln-eager-local"),
        platform: TargetProfile::POSIX,
    };
    ok(plan(&complete, &profile))
}

fn fixture_plan(fixture: &Fixture) -> MaterializationPlan {
    let [cargo, lib, run] = fixture.oids;
    materialization_plan(&[
        ("crates/kiln-core/Cargo.toml", cargo, EntryKind::File),
        ("crates/kiln-core/src/lib.rs", lib, EntryKind::File),
        ("crates/kiln-core/run.sh", run, EntryKind::Executable),
    ])
}

/// Every file below `root` with its mode and bytes, in path order.
fn snapshot(root: &Path) -> Vec<(String, u32, Vec<u8>)> {
    let mut names = Vec::new();
    ok(list_files(root, root, &mut names));
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let path = root.join(&name);
            let mode = ok(fs::symlink_metadata(&path)).mode() & 0o7777;
            (name, mode, ok(read_file(&path)))
        })
        .collect()
}

fn preparing_leftovers(directory: &Path) -> usize {
    fs::read_dir(directory).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(PREPARING_PREFIX)
            })
            .count()
    })
}

/// Make sealed inputs removable so the temporary directory can be dropped.
fn release(fixture: Fixture) {
    let _ = make_writable(fixture.root.path());
}

fn set_mode(path: &Path, mode: u32) {
    ok(fs::set_permissions(path, fs::Permissions::from_mode(mode)));
}

// r[verify kiln.source.projection.materialization.scenario.repeat]
#[test]
fn repeated_materialization_is_byte_and_mode_equivalent() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let other_store = private(fixture.root.path().join("other-store"));
    let first = ok(prepare_input(&fixture.store, &plan, &fixture.repository));
    let second = ok(prepare_input(&other_store, &plan, &fixture.repository));
    assert_eq!(snapshot(&first), snapshot(&second));
    assert_eq!(
        snapshot(&first),
        vec![
            (
                "kiln-core/Cargo.toml".to_owned(),
                SEALED_FILE,
                CARGO.to_vec()
            ),
            (
                "kiln-core/run.sh".to_owned(),
                SEALED_EXECUTABLE,
                RUN.to_vec()
            ),
            ("kiln-core/src/lib.rs".to_owned(), SEALED_FILE, LIB.to_vec()),
            (
                "manifests/kiln-core.toml".to_owned(),
                SEALED_FILE,
                CARGO.to_vec()
            ),
        ]
    );
    assert!(first.ends_with(plan.input_identity().as_str()));
    // A reused input is verified again and returned unchanged.
    assert_eq!(
        ok(prepare_input(&fixture.store, &plan, &fixture.repository)),
        first
    );
    assert_eq!(preparing_leftovers(&fixture.store.join(INPUTS)), 0);
    release(fixture);
}

// r[verify kiln.source.projection.materialization.scenario.isolated]
#[test]
fn attempt_overlays_share_the_input_but_not_their_writes() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let input = ok(prepare_input(&fixture.store, &plan, &fixture.repository));
    let (attempt_a, attempt_b) = (content("attempt-a"), content("attempt-b"));
    let a = ok(prepare_overlay(&fixture.store, &plan, &attempt_a, 1));
    let b = ok(prepare_overlay(&fixture.store, &plan, &attempt_b, 1));
    assert_ne!(a, b);
    let before = snapshot(&input);
    let modes: Vec<(String, u32)> = snapshot(&a)
        .into_iter()
        .map(|(name, mode, _)| (name, mode))
        .collect();
    assert_eq!(
        modes,
        vec![
            ("kiln-core/Cargo.toml".to_owned(), WRITABLE_FILE),
            ("kiln-core/run.sh".to_owned(), WRITABLE_EXECUTABLE),
            ("kiln-core/src/lib.rs".to_owned(), WRITABLE_FILE),
            ("manifests/kiln-core.toml".to_owned(), WRITABLE_FILE),
        ]
    );

    ok(fs::write(
        a.join("kiln-core/src/lib.rs"),
        b"pub fn edited() {}\n",
    ));
    ok(fs::write(a.join("kiln-core/src/new.rs"), b"// new\n"));
    assert_eq!(ok(read_file(&b.join("kiln-core/src/lib.rs"))), LIB);
    assert!(!b.join("kiln-core/src/new.rs").exists());
    assert_eq!(snapshot(&input), before);

    // The attempt resumes its own overlay, edits included; a later fence of
    // the same attempt gets a fresh overlay from the unchanged input.
    assert_eq!(ok(prepare_overlay(&fixture.store, &plan, &attempt_a, 1)), a);
    assert_eq!(
        ok(read_file(&a.join("kiln-core/src/lib.rs"))),
        b"pub fn edited() {}\n"
    );
    let replacement = ok(prepare_overlay(&fixture.store, &plan, &attempt_a, 2));
    assert_eq!(
        ok(read_file(&replacement.join("kiln-core/src/lib.rs"))),
        LIB
    );
    assert!(prepare_overlay(&fixture.store, &plan, &attempt_a, 0).is_err());
    release(fixture);
}

// r[verify kiln.source.projection.materialization.scenario.refused]
#[test]
fn missing_corrupt_damaged_and_unbound_state_is_never_admitted() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let inputs = fixture.store.join(INPUTS);

    // A missing object publishes nothing and leaves no preparing directory.
    let [cargo, _, _] = fixture.oids;
    let absent = ok(Oid::from_str("0123456789abcdef0123456789abcdef01234567"));
    let missing = materialization_plan(&[
        ("crates/kiln-core/Cargo.toml", cargo, EntryKind::File),
        ("crates/kiln-core/src/lib.rs", absent, EntryKind::File),
    ]);
    assert!(prepare_input(&fixture.store, &missing, &fixture.repository).is_err());
    assert!(!inputs.join(missing.input_identity().as_str()).exists());
    assert!(
        !inputs
            .join(format!("{}.json", missing.input_identity().as_str()))
            .exists()
    );
    assert_eq!(preparing_leftovers(&inputs), 0);

    // A corrupt loose object is refused.
    let damaged = ok(fixture.repository.blob(b"bytes that will be damaged\n"));
    let hex = damaged.to_string();
    let (fanout, rest) = hex.split_at_checked(2).unwrap_or_else(|| unreachable!());
    let object = fixture
        .root
        .path()
        .join("source.git/objects")
        .join(fanout)
        .join(rest);
    set_mode(&object, 0o644);
    ok(fs::write(&object, b"not a zlib object"));
    let reopened = ok(Repository::open_bare(
        fixture.root.path().join("source.git"),
    ));
    let corrupt = materialization_plan(&[
        ("crates/kiln-core/Cargo.toml", cargo, EntryKind::File),
        ("crates/kiln-core/src/lib.rs", damaged, EntryKind::File),
    ]);
    assert!(prepare_input(&fixture.store, &corrupt, &reopened).is_err());
    assert!(!inputs.join(corrupt.input_identity().as_str()).exists());

    // A published input damaged afterwards is not reused.
    let input = ok(prepare_input(&fixture.store, &plan, &fixture.repository));
    let file = input.join("kiln-core/src/lib.rs");
    set_mode(&file, 0o644);
    ok(fs::write(&file, b"pub fn tampered() {}\n"));
    set_mode(&file, SEALED_FILE);
    assert!(prepare_input(&fixture.store, &plan, &fixture.repository).is_err());

    // A truncated file is refused the same way.
    let other_store = private(fixture.root.path().join("truncated-store"));
    let truncated = ok(prepare_input(&other_store, &plan, &fixture.repository));
    let file = truncated.join("kiln-core/Cargo.toml");
    set_mode(&file, 0o644);
    let (head, _) = CARGO.split_at_checked(4).unwrap_or_else(|| unreachable!());
    ok(fs::write(&file, head));
    set_mode(&file, SEALED_FILE);
    assert!(prepare_input(&other_store, &plan, &fixture.repository).is_err());

    // A directory without its binding is neither reused nor overwritten.
    let unbound_store = private(fixture.root.path().join("unbound-store"));
    let unbound = unbound_store
        .join(INPUTS)
        .join(plan.input_identity().as_str());
    ok(fs::create_dir_all(&unbound));
    set_mode(&unbound_store.join(INPUTS), PRIVATE_DIRECTORY);
    ok(fs::write(unbound.join("operator-note"), b"keep"));
    assert!(prepare_input(&unbound_store, &plan, &fixture.repository).is_err());
    assert_eq!(ok(read_file(&unbound.join("operator-note"))), b"keep");
    assert!(prepare_overlay(&unbound_store, &plan, &content("attempt"), 1).is_err());

    // An overlay directory without its binding is refused and kept.
    let attempt = content("attempt");
    let identity = ok(plan.overlay_identity(&attempt, 3));
    let overlays = private(fixture.store.join(OVERLAYS));
    let stray = overlays.join(identity.as_str());
    ok(fs::create_dir(&stray));
    assert!(prepare_overlay(&fixture.store, &plan, &attempt, 3).is_err());
    assert!(stray.exists());
    release(fixture);
}

#[test]
fn an_interrupted_preparation_is_never_adopted() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let inputs = private(fixture.store.join(INPUTS));
    let leftover = inputs.join(format!(
        "{PREPARING_PREFIX}{}-1-1",
        plan.input_identity().as_str()
    ));
    ok(fs::create_dir_all(leftover.join("kiln-core")));
    ok(fs::write(leftover.join("kiln-core/Cargo.toml"), b"partial"));
    let input = ok(prepare_input(&fixture.store, &plan, &fixture.repository));
    assert_eq!(snapshot(&input).len(), plan.entries().len());
    assert_eq!(ok(read_file(&input.join("kiln-core/Cargo.toml"))), CARGO);
    assert_eq!(
        ok(read_file(&leftover.join("kiln-core/Cargo.toml"))),
        b"partial"
    );
    release(fixture);
}
