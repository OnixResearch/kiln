use std::collections::BTreeSet;

use super::*;
use kiln_adapter_basalt::test_support::{
    self, COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
};
use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
use kiln_core::projection::authority::{
    LocalSourceAuthority, LocalSourceGrant, SnapshotObservation, SourceCurrentness,
};
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

fn pin(plan: &MaterializationPlan, attempt: &ContentId, fence: u64) -> WorkspacePin {
    ok(plan.pin(attempt, fence))
}

fn local_prepare(
    plan: &MaterializationPlan,
) -> (LocalSourceAuthority, AuthorityProfile, SourceCurrentness) {
    let authority = ok(LocalSourceAuthority::new(
        "materialize-test",
        LocalSourceGrant {
            repositories: BTreeSet::from([plan.repository().clone()]),
            operations: BTreeSet::from([
                SourceViewOperation::PrepareWorkspace,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
            ]),
        },
    ));
    let profile = AuthorityProfile::LocalOperator {
        operator: "materialize-test".into(),
    };
    let current = SourceCurrentness {
        generation: 0,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: plan.source().clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    (authority, profile, current)
}

fn prep_input(
    store: &Path,
    plan: &MaterializationPlan,
    repository: &Repository,
) -> Result<PathBuf, String> {
    let (mut auth, profile, current) = local_prepare(plan);
    let mut ctx = SourceAuthorization {
        authority: &mut auth,
        profile: crate::worker::source_auth::RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    prepare_input(store, plan, repository, &mut ctx)
}

fn prep_overlay(
    store: &Path,
    plan: &MaterializationPlan,
    pin: &WorkspacePin,
) -> Result<PathBuf, String> {
    let (mut auth, profile, current) = local_prepare(plan);
    let mut ctx = SourceAuthorization {
        authority: &mut auth,
        profile: crate::worker::source_auth::RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    prepare_overlay(store, plan, pin, &mut ctx)
}

// r[verify kiln.source.projection.materialization.scenario.repeat]
#[test]
fn repeated_materialization_is_byte_and_mode_equivalent() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let other_store = private(fixture.root.path().join("other-store"));
    let first = ok(prep_input(&fixture.store, &plan, &fixture.repository));
    let second = ok(prep_input(&other_store, &plan, &fixture.repository));
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
        ok(prep_input(&fixture.store, &plan, &fixture.repository)),
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
    let input = ok(prep_input(&fixture.store, &plan, &fixture.repository));
    let (attempt_a, attempt_b) = (content("attempt-a"), content("attempt-b"));
    let a = ok(prep_overlay(
        &fixture.store,
        &plan,
        &pin(&plan, &attempt_a, 1),
    ));
    let b = ok(prep_overlay(
        &fixture.store,
        &plan,
        &pin(&plan, &attempt_b, 1),
    ));
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
    assert_eq!(
        ok(prep_overlay(
            &fixture.store,
            &plan,
            &pin(&plan, &attempt_a, 1)
        )),
        a
    );
    assert_eq!(
        ok(read_file(&a.join("kiln-core/src/lib.rs"))),
        b"pub fn edited() {}\n"
    );
    let replacement = ok(prep_overlay(
        &fixture.store,
        &plan,
        &pin(&plan, &attempt_a, 2),
    ));
    assert_eq!(
        ok(read_file(&replacement.join("kiln-core/src/lib.rs"))),
        LIB
    );
    let mut zero = pin(&plan, &attempt_a, 1);
    zero.fence = 0;
    assert!(prep_overlay(&fixture.store, &plan, &zero).is_err());
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
    assert!(prep_input(&fixture.store, &missing, &fixture.repository).is_err());
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
    assert!(prep_input(&fixture.store, &corrupt, &reopened).is_err());
    assert!(!inputs.join(corrupt.input_identity().as_str()).exists());

    // A published input damaged afterwards is not reused.
    let input = ok(prep_input(&fixture.store, &plan, &fixture.repository));
    let file = input.join("kiln-core/src/lib.rs");
    set_mode(&file, 0o644);
    ok(fs::write(&file, b"pub fn tampered() {}\n"));
    set_mode(&file, SEALED_FILE);
    assert!(prep_input(&fixture.store, &plan, &fixture.repository).is_err());

    // A truncated file is refused the same way.
    let other_store = private(fixture.root.path().join("truncated-store"));
    let truncated = ok(prep_input(&other_store, &plan, &fixture.repository));
    let file = truncated.join("kiln-core/Cargo.toml");
    set_mode(&file, 0o644);
    let (head, _) = CARGO.split_at_checked(4).unwrap_or_else(|| unreachable!());
    ok(fs::write(&file, head));
    set_mode(&file, SEALED_FILE);
    assert!(prep_input(&other_store, &plan, &fixture.repository).is_err());

    // A directory without its binding is neither reused nor overwritten.
    let unbound_store = private(fixture.root.path().join("unbound-store"));
    let unbound = unbound_store
        .join(INPUTS)
        .join(plan.input_identity().as_str());
    ok(fs::create_dir_all(&unbound));
    set_mode(&unbound_store.join(INPUTS), PRIVATE_DIRECTORY);
    ok(fs::write(unbound.join("operator-note"), b"keep"));
    assert!(prep_input(&unbound_store, &plan, &fixture.repository).is_err());
    assert_eq!(ok(read_file(&unbound.join("operator-note"))), b"keep");
    let attempt = content("attempt");
    assert!(prep_overlay(&unbound_store, &plan, &pin(&plan, &attempt, 1)).is_err());

    // An overlay directory without its binding is refused and kept.
    let identity = ok(plan.overlay_identity(&attempt, 3));
    let overlays = private(fixture.store.join(OVERLAYS));
    let stray = overlays.join(identity.as_str());
    ok(fs::create_dir(&stray));
    assert!(prep_overlay(&fixture.store, &plan, &pin(&plan, &attempt, 3)).is_err());
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
    let input = ok(prep_input(&fixture.store, &plan, &fixture.repository));
    assert_eq!(snapshot(&input).len(), plan.entries().len());
    assert_eq!(ok(read_file(&input.join("kiln-core/Cargo.toml"))), CARGO);
    assert_eq!(
        ok(read_file(&leftover.join("kiln-core/Cargo.toml"))),
        b"partial"
    );
    release(fixture);
}

// r[verify kiln.source.projection.materialization.scenario.pinned]
#[test]
fn an_overlay_is_prepared_and_resumed_only_for_its_exact_pin() {
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    ok(prep_input(&fixture.store, &plan, &fixture.repository));
    let attempt = content("attempt");
    let exact = pin(&plan, &attempt, 1);
    let overlay = ok(prep_overlay(&fixture.store, &plan, &exact));
    let binding_path = fixture
        .store
        .join(OVERLAYS)
        .join(format!("{}.json", exact.overlay.as_str()));
    let binding = ok(read_binding(&binding_path)).unwrap_or_else(|| unreachable!());
    assert_eq!(binding.overlay.as_ref(), Some(&exact));

    // A pin naming another source, definition, manifest or profile than the
    // plan is refused, even though its overlay generation is consistent.
    type PinChange = fn(&mut WorkspacePin);
    let changes: [PinChange; 4] = [
        |pin| pin.source = external("git", "commit", &"c".repeat(40)),
        |pin| pin.definition = content("other-definition"),
        |pin| pin.manifest = content("other-manifest"),
        |pin| pin.profile = content("other-profile"),
    ];
    for change in changes {
        let mut forged = exact.clone();
        change(&mut forged);
        assert_eq!(forged.validate(), Ok(()));
        assert!(prep_overlay(&fixture.store, &plan, &forged).is_err());
    }

    // A retained overlay whose binding records another pin, or a version 1
    // binding without one, is neither resumed nor overwritten.
    let mut recorded = binding;
    if let Some(retained) = recorded.overlay.as_mut() {
        retained.definition = content("other-definition");
    }
    let v1 = serde_json::json!({
        "schema": "kiln.materialization.binding.v1",
        "manifest": plan.manifest(),
        "input": plan.input_identity(),
        "overlay": {"identity": exact.overlay, "attempt": attempt, "fence": 1}
    });
    for retained in [
        ok(serde_json::to_vec(&recorded)),
        v1.to_string().into_bytes(),
    ] {
        ok(fs::remove_file(&binding_path));
        ok(fs::write(&binding_path, retained));
        assert!(prep_overlay(&fixture.store, &plan, &exact).is_err());
        assert!(overlay.join("kiln-core/Cargo.toml").exists());
    }
    release(fixture);
}

#[test]
fn basalt_source_authority_gates_materialization_before_creating_directories() {
    use crate::worker::source_auth::RequestProfile;
    let fixture = fixture();
    let plan = fixture_plan(&fixture);
    let attempt = content("attempt-auth");
    let pin_ok = pin(&plan, &attempt, 1);
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: plan.source().clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };

    // Denied profile (Discover only, no PrepareWorkspace) creates NO directories!
    let denied_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan.repository(),
        None,
        &[(PathScope::root(), vec![SourceViewOperation::Discover])],
        EXPIRES_S,
    );
    let denied_signer = denied_fix.signer();
    let mut denied_auth = denied_fix.authority();
    let uncreated_store = fixture.root.path().join("uncreated-store");
    let sign_denied = |req: &SourceAuthorityRequest| {
        denied_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut ctx = SourceAuthorization {
        authority: &mut denied_auth,
        profile: RequestProfile::Signer(&sign_denied),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let err = match prepare_input(&uncreated_store, &plan, &fixture.repository, &mut ctx) {
        Ok(path) => unreachable!("unexpected ok: {path:?}"),
        Err(err) => err,
    };
    assert!(err.contains("credential-denied"), "{err}");
    assert!(!uncreated_store.exists());

    // Valid PrepareWorkspace (non-attempt for input, attempt-bound for overlay) succeeds:
    let input_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan.repository(),
        None,
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let input_signer = input_fix.signer();
    let mut input_auth = input_fix.authority();
    let sign_in = |req: &SourceAuthorityRequest| {
        input_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut in_ctx = SourceAuthorization {
        authority: &mut input_auth,
        profile: RequestProfile::Signer(&sign_in),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    ok(prepare_input(
        &fixture.store,
        &plan,
        &fixture.repository,
        &mut in_ctx,
    ));

    let fence = AttemptFence {
        attempt: attempt.clone(),
        fence: 1,
    };
    let overlay_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan.repository(),
        Some(&fence),
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let overlay_signer = overlay_fix.signer();
    let mut overlay_auth = overlay_fix.authority();
    let sign_ov = |req: &SourceAuthorityRequest| {
        overlay_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    // Cancelled attempt denies overlay creation before touching overlays dir!
    let mut cancelled = current.clone();
    cancelled.attempt = AttemptCurrentness::Cancelled;
    let mut cancel_ctx = SourceAuthorization {
        authority: &mut overlay_auth,
        profile: RequestProfile::Signer(&sign_ov),
        current: &cancelled,
        now_unix_s: VERIFY_AT_S,
    };
    let err = match prepare_overlay(&fixture.store, &plan, &pin_ok, &mut cancel_ctx) {
        Ok(path) => unreachable!("unexpected ok: {path:?}"),
        Err(err) => err,
    };
    assert!(err.contains("attempt-not-current"), "{err}");
    assert!(!fixture.store.join(OVERLAYS).exists());

    // Current attempt succeeds, revoked token denies:
    let mut ov_ctx = SourceAuthorization {
        authority: &mut overlay_auth,
        profile: RequestProfile::Signer(&sign_ov),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    ok(prepare_overlay(&fixture.store, &plan, &pin_ok, &mut ov_ctx));

    overlay_fix.revocations.revoke(&overlay_fix.token);
    let err = match prepare_overlay(&fixture.store, &plan, &pin_ok, &mut ov_ctx) {
        Ok(path) => unreachable!("unexpected ok: {path:?}"),
        Err(err) => err,
    };
    assert!(err.contains("revoked"), "{err}");
    let _ = test_support::ROOT_SEED;
    release(fixture);
}

#[test]
fn prepare_route_mapped_creation_probes_match_exact_prefix_empty_source_and_overlapping_mounts() {
    use crate::worker::source_auth::RequestProfile;
    use kiln_core::projection::materialization::{RouteRequest, route};
    use kiln_core::projection::{GeneratedInputs, Mount, ProjectionProfile, Selection};

    let root = ok(tempfile::tempdir());
    let repo = ok(Repository::init_bare(root.path().join("source.git")));
    let blob_app = ok(repo.blob(b"pub fn app() {}\n"));
    let blob_shared = ok(repo.blob(b"pub const V: u32 = 1;\n"));
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
    let adm = ok(admit_repositories(&enrolled, &[]))
        .pop()
        .unwrap_or_else(|| unreachable!("one admitted repository"));
    let component_fn = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let rev = external("git", "commit", &"a".repeat(40));
    let mprof = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: component_fn("kiln-eager-local"),
        platform: TargetProfile::POSIX,
    };

    // 1. Exact-prefix mount: workspace_app -> crates/app with creation_destinations = ["workspace_app"]
    let store_exact = private(root.path().join("store-exact"));
    let facts_exact = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: rev.clone(),
        canonical_tree: content("tree-exact"),
        evaluator: component_fn("kiln-projection"),
        adapter: component_fn("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "crates/app/src/lib.rs".into(),
            object: external("git", "blob", &blob_app.to_string()),
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let def_exact = ok(admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "exact-mount".into(),
        profile: ProjectionProfile::Writable,
        selection: Selection {
            root: String::new(),
            include: vec!["crates/app/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: "crates/app".into(),
            destination_prefix: "workspace_app".into(),
        }],
        dependencies: Vec::new(),
        creation_destinations: vec!["workspace_app".into()],
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }));
    let write_exact =
        ok(ok(resolve(&def_exact, &adm, &facts_exact))
            .require_complete(ProjectionOperation::Write));
    let plan_exact = ok(plan(&write_exact, &mprof));
    let attempt = content("att-exact");
    let pin_exact = ok(plan_exact.pin(&attempt, 1));
    let route_exact = ok(route(
        &def_exact,
        &write_exact,
        &plan_exact,
        &pin_exact,
        RouteRequest {
            store: ok(store_exact.to_str().ok_or("utf-8")),
            writable: &["crates/app".into()],
            build_plan: None,
        },
    ));
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev.clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let prep_in_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan_exact.repository(),
        None,
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let prep_in_signer = prep_in_fix.signer();
    let mut prep_in_auth = prep_in_fix.authority();
    let sign_in = |req: &SourceAuthorityRequest| {
        prep_in_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut in_ctx = SourceAuthorization {
        authority: &mut prep_in_auth,
        profile: RequestProfile::Signer(&sign_in),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    ok(prepare_input(&store_exact, &plan_exact, &repo, &mut in_ctx));
    let fence = AttemptFence {
        attempt: attempt.clone(),
        fence: 1,
    };
    let prep_ov_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan_exact.repository(),
        Some(&fence),
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let prep_ov_signer = prep_ov_fix.signer();
    let mut prep_ov_auth = prep_ov_fix.authority();
    let sign_ov = |req: &SourceAuthorityRequest| {
        prep_ov_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut ov_ctx = SourceAuthorization {
        authority: &mut prep_ov_auth,
        profile: RequestProfile::Signer(&sign_ov),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    ok(prepare_overlay(
        &store_exact,
        &plan_exact,
        &pin_exact,
        &mut ov_ctx,
    ));

    let run_route = |store: &Path,
                     mplan: &MaterializationPlan,
                     r: &ProjectedWorkspaceRoute,
                     grants: &[(PathScope, Vec<SourceViewOperation>)]| {
        let f = AttemptFence {
            attempt: r.pin.attempt.clone(),
            fence: r.pin.fence,
        };
        let fix = SourceFixture::direct(
            COORDINATOR_SEED,
            mplan.repository(),
            Some(&f),
            grants,
            EXPIRES_S,
        );
        let signer = fix.signer();
        let mut auth = fix.authority();
        let sign = |req: &SourceAuthorityRequest| {
            signer
                .sign(req)
                .map(AuthorityProfile::VerifiedUcan)
                .map_err(|e| format!("{}: {}", e.code, e.message))
        };
        let mut ctx = SourceAuthorization {
            authority: &mut auth,
            profile: RequestProfile::Signer(&sign),
            current: &current,
            now_unix_s: VERIFY_AT_S,
        };
        prepare_route(store, mplan, r, &mut ctx)
    };
    let crates_app = ok(PathScope::from_path("crates/app", true).ok_or("path"));
    ok(run_route(
        &store_exact,
        &plan_exact,
        &route_exact,
        &[
            (
                PathScope::root(),
                vec![SourceViewOperation::PrepareWorkspace],
            ),
            (
                crates_app.clone(),
                vec![
                    SourceViewOperation::ModifySource,
                    SourceViewOperation::CreateSource,
                ],
            ),
        ],
    ));
    let dest_only = run_route(
        &store_exact,
        &plan_exact,
        &route_exact,
        &[
            (
                PathScope::root(),
                vec![SourceViewOperation::PrepareWorkspace],
            ),
            (crates_app, vec![SourceViewOperation::ModifySource]),
            (
                ok(PathScope::from_path("workspace_app", true).ok_or("path")),
                vec![SourceViewOperation::CreateSource],
            ),
        ],
    );
    assert!(
        dest_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{dest_only:?}"
    );

    // 2. Overlapping/shadowed mounts: "" -> "" and "shared" -> "vendor/shared"
    let store_shadow = private(root.path().join("store-shadow"));
    let facts_shadow = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: rev,
        canonical_tree: content("tree-shadow"),
        evaluator: component_fn("kiln-projection"),
        adapter: component_fn("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "shared/proto/lib.rs".into(),
            object: external("git", "blob", &blob_shared.to_string()),
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let def_shadow = ok(admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "shadow-mount".into(),
        profile: ProjectionProfile::Writable,
        selection: Selection {
            root: String::new(),
            include: vec!["shared/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![
            Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            },
            Mount {
                source_prefix: "shared".into(),
                destination_prefix: "vendor/shared".into(),
            },
        ],
        dependencies: Vec::new(),
        creation_destinations: vec!["vendor/shared/sub".into()],
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }));
    let write_shadow =
        ok(ok(resolve(&def_shadow, &adm, &facts_shadow))
            .require_complete(ProjectionOperation::Write));
    let plan_shadow = ok(plan(&write_shadow, &mprof));
    let pin_shadow = ok(plan_shadow.pin(&attempt, 2));
    let route_shadow = ok(route(
        &def_shadow,
        &write_shadow,
        &plan_shadow,
        &pin_shadow,
        RouteRequest {
            store: ok(store_shadow.to_str().ok_or("utf-8")),
            writable: &["shared".into(), "vendor".into()],
            build_plan: None,
        },
    ));
    ok(prepare_input(
        &store_shadow,
        &plan_shadow,
        &repo,
        &mut in_ctx,
    ));
    let fence2 = AttemptFence { attempt, fence: 2 };
    let prep_ov2_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        plan_shadow.repository(),
        Some(&fence2),
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let prep_ov2_signer = prep_ov2_fix.signer();
    let mut prep_ov2_auth = prep_ov2_fix.authority();
    let sign_ov2 = |req: &SourceAuthorityRequest| {
        prep_ov2_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut ov2_ctx = SourceAuthorization {
        authority: &mut prep_ov2_auth,
        profile: RequestProfile::Signer(&sign_ov2),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    ok(prepare_overlay(
        &store_shadow,
        &plan_shadow,
        &pin_shadow,
        &mut ov2_ctx,
    ));

    let shared_scope = ok(PathScope::from_path("shared", true).ok_or("path"));
    let vendor_scope = ok(PathScope::from_path("vendor", true).ok_or("path"));
    let vendor_sub = ok(PathScope::from_path("vendor/shared/sub", true).ok_or("path"));
    let vendor_only = run_route(
        &store_shadow,
        &plan_shadow,
        &route_shadow,
        &[
            (
                PathScope::root(),
                vec![SourceViewOperation::PrepareWorkspace],
            ),
            (
                shared_scope.clone(),
                vec![SourceViewOperation::ModifySource],
            ),
            (vendor_sub, vec![SourceViewOperation::CreateSource]),
        ],
    );
    assert!(
        vendor_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{vendor_only:?}"
    );
    let shared_only = run_route(
        &store_shadow,
        &plan_shadow,
        &route_shadow,
        &[
            (
                PathScope::root(),
                vec![SourceViewOperation::PrepareWorkspace],
            ),
            (
                shared_scope.clone(),
                vec![
                    SourceViewOperation::ModifySource,
                    SourceViewOperation::CreateSource,
                ],
            ),
        ],
    );
    assert!(
        shared_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{shared_only:?}"
    );
    ok(run_route(
        &store_shadow,
        &plan_shadow,
        &route_shadow,
        &[
            (
                PathScope::root(),
                vec![SourceViewOperation::PrepareWorkspace],
            ),
            (
                shared_scope,
                vec![
                    SourceViewOperation::ModifySource,
                    SourceViewOperation::CreateSource,
                ],
            ),
            (vendor_scope, vec![SourceViewOperation::CreateSource]),
        ],
    ));

    // 3. Unit test on mapped_creation_probes with empty source_prefix and write == destination_prefix
    let mut route_empty_src = route_exact;
    route_empty_src.definition.mounts = vec![Mount {
        source_prefix: String::new(),
        destination_prefix: "workspace_app".into(),
    }];
    let probes = mapped_creation_probes(&route_empty_src, "workspace_app");
    assert_eq!(probes, vec!["_route_create"]);
    assert!(PathScope::from_path("_route_create", false).is_some());
}
