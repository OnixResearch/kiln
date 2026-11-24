use super::*;
use std::collections::BTreeSet;

use kiln_adapter_basalt::test_support::{
    COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
};
use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
use kiln_core::implementation::{
    DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, WorkspaceIsolation,
};
use kiln_core::projection::authority::{
    AttemptFence, PathScope, SnapshotObservation, SourceCurrentness, SourceViewOperation,
};
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializerKind, MaterializerProfile, RouteRequest, plan, route,
};
use kiln_core::projection::{
    ComponentVersion, Coverage, DEFINITION_SCHEMA, EntryKind, GeneratedInputs, Mount,
    ProjectionDefinition, ProjectionFacts, ProjectionOperation, ProjectionProfile, ProviderBinding,
    RepositoryEnrollment, RepositoryLocator, Selection, SourceCapability, SourceEntry,
    TargetProfile, admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn component(name: &str) -> ComponentVersion {
    ComponentVersion {
        name: name.into(),
        version: "1".into(),
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    plan: ImplementationDispatchPlan,
    route: ProjectedWorkspaceRoute,
    /// The route of the same pin derived for `alpha`, wider than the plan's
    /// permitted `alpha/src`.
    wider: ProjectedWorkspaceRoute,
    route_file: std::path::PathBuf,
    request: serde_json::Value,
}

const ENABLED: ProjectionAuthority = ProjectionAuthority::ProjectionEnabled;

fn fixture() -> Fixture {
    let root = ok(tempfile::tempdir());
    let store = ok(std::fs::canonicalize(root.path())).join("store");
    let repository = ok(admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "fixture".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: ok(ExternalRef::new("git", "repository", "fixture")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/fixture".into(),
            }],
        }],
        &[],
    ))
    .pop()
    .unwrap_or_else(|| unreachable!("one repository"));
    let base = ok(ExternalRef::new("git", "commit", &"a".repeat(40)));
    let facts = ProjectionFacts {
        repository: repository.identity().clone(),
        exact_revision: base.clone(),
        canonical_tree: digest("tree"),
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: ["alpha/src/lib.rs", "alpha/Cargo.toml"]
            .iter()
            .map(|path| SourceEntry {
                path: (*path).into(),
                object: ok(ExternalRef::new("git", "blob", path)),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let definition = ok(admit_definition(&ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: "alpha".into(),
        profile: ProjectionProfile::Writable,
        selection: Selection {
            root: String::new(),
            include: vec!["alpha/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: vec!["alpha/src".into()],
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }));
    let complete =
        ok(ok(resolve(&definition, &repository, &facts))
            .require_complete(ProjectionOperation::Write));
    let materialization = ok(plan(
        &complete,
        &MaterializerProfile {
            schema: MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: MaterializerKind::EagerLocalDirectory,
            materializer: component("kiln-eager-local"),
            platform: TargetProfile::POSIX,
        },
    ));
    let attempt = digest("attempt");
    let pin = ok(materialization.pin(&attempt, 1));
    let writable = vec!["alpha/src".to_string()];
    let derive = |writable: &[String]| {
        ok(route(
            &definition,
            &complete,
            &materialization,
            &pin,
            RouteRequest {
                store: ok(store.to_str().ok_or("utf-8")),
                writable,
                build_plan: None,
            },
        ))
    };
    let derived = derive(&writable);
    let wider = derive(&["alpha".to_string()]);
    ok(std::fs::create_dir_all(&derived.overlay_root));
    let route_file = root.path().join("route.json");
    ok(std::fs::write(
        &route_file,
        ok(serde_json::to_vec(&derived)),
    ));
    let plan = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: digest("work"),
        work_revision: 1,
        attempt_identity: attempt,
        fence: 1,
        repository: "qualification/disposable".into(),
        base: Some(base),
        workspace: DispatchWorkspace {
            identity: pin.overlay.clone(),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: Some(pin),
        },
        provider_profile: ok(ProviderProfileId::new("scripted-local-omp")),
        permitted_paths: writable,
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1,
        invocation_budget: 1,
        expected_output: digest("output"),
        execution: None,
    };
    let request = serde_json::json!({
        "workspace_root": derived.overlay_root,
        "read_paths": derived.read_paths,
        "write_paths": derived.write_paths,
        "network": false,
    });
    Fixture {
        _root: root,
        plan,
        route: derived,
        wider,
        route_file,
        request,
    }
}

fn write_route(fixture: &Fixture, route: &ProjectedWorkspaceRoute) {
    ok(std::fs::write(
        &fixture.route_file,
        ok(serde_json::to_vec(route)),
    ));
}

fn request_of(route: &ProjectedWorkspaceRoute) -> serde_json::Value {
    serde_json::json!({
        "workspace_root": route.overlay_root,
        "read_paths": route.read_paths,
        "write_paths": route.write_paths,
        "network": false,
    })
}

fn check_with_basalt(
    route_file: Option<&Path>,
    plan: &ImplementationDispatchPlan,
    request: &serde_json::Value,
    authority: ProjectionAuthority,
    fixture: &Fixture,
    grants: &[(PathScope, Vec<SourceViewOperation>)],
) -> Result<(), String> {
    let pin = fixture.route.pin.clone();
    let attempt = AttemptFence {
        attempt: plan.attempt_identity.clone(),
        fence: plan.fence,
    };
    let source_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        &pin.repository,
        Some(&attempt),
        grants,
        EXPIRES_S,
    );
    let signer = source_fix.signer();
    let mut source_auth = source_fix.authority();
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: pin.source,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let sign_request = |req: &SourceAuthorityRequest| {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|error| format!("{}: {}", error.code, error.message))
    };
    let mut dispatch_auth = SourceDispatchAuthority {
        authority: &mut source_auth,
        sign_request: &sign_request,
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    check_route(
        route_file,
        plan,
        request,
        authority,
        Some(&mut dispatch_auth),
    )
}

fn full_source_grants() -> Vec<(PathScope, Vec<SourceViewOperation>)> {
    vec![(
        PathScope::root(),
        vec![
            SourceViewOperation::PrepareWorkspace,
            SourceViewOperation::Discover,
            SourceViewOperation::ReadContent,
            SourceViewOperation::ModifySource,
            SourceViewOperation::CreateSource,
        ],
    )]
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn a_request_equal_to_the_route_of_the_pinned_plan_is_admitted() {
    let fixture = fixture();
    // The writable creation destination holds the writable origin.
    assert_eq!(fixture.route.write_paths, ["alpha/src"]);
    assert_eq!(fixture.route.read_paths, ["alpha"]);
    ok(check_with_basalt(
        Some(&fixture.route_file),
        &fixture.plan,
        &fixture.request,
        ENABLED,
        &fixture,
        &full_source_grants(),
    ));
    // ProjectionEnabled alone without SourceAuthority is insufficient!
    assert!(
        check_route(
            Some(&fixture.route_file),
            &fixture.plan,
            &fixture.request,
            ENABLED,
            None,
        )
        .is_err_and(|error| error.ends_with("nothing was dispatched"))
    );
    // A plan without a pin keeps the earlier route and takes no route file,
    // under any profile.
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    for authority in [
        ENABLED,
        ProjectionAuthority::LocalOperator,
        ProjectionAuthority::Absent,
    ] {
        ok(check_route(
            None,
            &unpinned,
            &fixture.request,
            authority,
            None,
        ));
    }
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn the_local_operator_profile_never_dispatches_a_projected_workspace() {
    let fixture = fixture();
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    for authority in [
        ProjectionAuthority::LocalOperator,
        ProjectionAuthority::Absent,
    ] {
        // The exact route and request that the enabled profile admits, the
        // pinned plan without its route, and a route for an unpinned plan all
        // take the projection denial.
        for (route_file, plan) in [
            (Some(fixture.route_file.as_path()), &fixture.plan),
            (None, &fixture.plan),
            (Some(fixture.route_file.as_path()), &unpinned),
        ] {
            let refused = check_route(route_file, plan, &fixture.request, authority, None);
            assert!(
                refused.as_ref().is_err_and(|error| {
                    error.starts_with("projection-requires-operator:")
                        && error.ends_with("nothing was dispatched")
                }),
                "{authority:?}: {refused:?}"
            );
        }
    }
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn a_same_pin_route_wider_than_the_plan_permits_is_refused() {
    let fixture = fixture();
    // Derived for `alpha` on the same pin, it is well formed and its request
    // matches it exactly, but the plan permits only `alpha/src`.
    assert_eq!(fixture.wider.pin, fixture.route.pin);
    assert_eq!(fixture.wider.write_paths, ["alpha/Cargo.toml", "alpha/src"]);
    ok(fixture.wider.verify());
    write_route(&fixture, &fixture.wider);
    let refused = check_with_basalt(
        Some(&fixture.route_file),
        &fixture.plan,
        &request_of(&fixture.wider),
        ENABLED,
        &fixture,
        &full_source_grants(),
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("route_permission_mismatch")),
        "{refused:?}"
    );
    // The same route runs for a plan that permits exactly `alpha`.
    let mut wide_plan = fixture.plan.clone();
    wide_plan.permitted_paths = vec!["alpha".into()];
    ok(check_with_basalt(
        Some(&fixture.route_file),
        &wide_plan,
        &request_of(&fixture.wider),
        ENABLED,
        &fixture,
        &full_source_grants(),
    ));
    // The plan's own route is refused for the wider plan too: equality, not
    // a subset.
    write_route(&fixture, &fixture.route);
    assert!(
        check_with_basalt(
            Some(&fixture.route_file),
            &wide_plan,
            &fixture.request,
            ENABLED,
            &fixture,
            &full_source_grants(),
        )
        .is_err_and(|error| error.contains("route_permission_mismatch"))
    );
}

// r[verify kiln.source.projection.route.scenario.refused]
#[test]
fn vcs_metadata_and_runtime_mounted_stores_are_refused_before_dispatch() {
    let fixture = fixture();
    let refused_with = |route: &ProjectedWorkspaceRoute, code: &str| {
        write_route(&fixture, route);
        let refused = check_with_basalt(
            Some(&fixture.route_file),
            &fixture.plan,
            &request_of(route),
            ENABLED,
            &fixture,
            &full_source_grants(),
        );
        assert!(
            refused.as_ref().is_err_and(|error| error.contains(code)),
            "{code}: {refused:?}"
        );
    };
    let mut git = fixture.route.clone();
    git.read_paths = vec![".git".into(), "alpha".into()];
    refused_with(&git, "route_vcs_metadata");
    let mut exposed = fixture.route.clone();
    exposed.overlay_root = format!(
        "/usr/local/kiln/store/overlays/{}",
        exposed.pin.overlay.as_str()
    );
    exposed.input_root = format!(
        "/usr/local/kiln/store/inputs/{}",
        exposed.pin.input.as_str()
    );
    refused_with(&exposed, "route_roots");
}

// r[verify kiln.source.projection.route.scenario.refused]
#[test]
fn a_store_behind_a_linked_mount_root_is_refused_before_dispatch() {
    let dir = ok(tempfile::tempdir());
    let base = ok(std::fs::canonicalize(dir.path())).join("host");
    let target = base.join("srv");
    let behind = target.join("kiln/store");
    let unrelated = base.join("var/kiln/store");
    ok(std::fs::create_dir_all(&behind));
    ok(std::fs::create_dir_all(&unrelated));
    // A mount root that is a link, as `/usr -> /srv` would be.
    let linked = base.join("usr");
    ok(std::os::unix::fs::symlink(&target, &linked));
    let root = |path: &std::path::Path| path.to_str().map(str::to_owned).unwrap_or_default();
    let linked_root = root(&linked);
    let refused = check_store_placement(&behind, &[linked_root.as_str()]);
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("through a link")),
        "{refused:?}"
    );
    ok(check_store_placement(&unrelated, &[linked_root.as_str()]));
    // A root with no entry is not bound; a root that cannot be resolved
    // refuses even an unrelated store.
    let absent = root(&base.join("lib64"));
    ok(check_store_placement(&unrelated, &[absent.as_str()]));
    let dangling = base.join("lib");
    ok(std::os::unix::fs::symlink(base.join("missing"), &dangling));
    let dangling = root(&dangling);
    let unresolved = check_store_placement(&unrelated, &[dangling.as_str()]);
    assert!(
        unresolved
            .as_ref()
            .is_err_and(|error| error.contains("cannot be resolved")),
        "{unresolved:?}"
    );
    // A store named through the link is not canonical.
    assert!(check_store_placement(&linked.join("kiln/store"), &[]).is_err());
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn requests_differing_from_the_route_are_refused_before_dispatch() {
    let fixture = fixture();
    let refused = |request: &serde_json::Value| {
        check_with_basalt(
            Some(&fixture.route_file),
            &fixture.plan,
            request,
            ENABLED,
            &fixture,
            &full_source_grants(),
        )
        .is_err_and(|error| error.ends_with("nothing was dispatched"))
    };
    let mutate = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut request = fixture.request.clone();
        change(&mut request);
        request
    };
    for request in [
        mutate(&|request| request["write_paths"] = serde_json::json!(["alpha"])),
        mutate(&|request| {
            request["write_paths"] = serde_json::json!(["alpha/src/lib.rs", "alpha/Cargo.toml"])
        }),
        mutate(&|request| request["write_paths"] = serde_json::json!([])),
        mutate(&|request| request["read_paths"] = serde_json::json!(["alpha", ".git"])),
        mutate(&|request| request["workspace_root"] = serde_json::json!(fixture.route.input_root)),
        mutate(&|request| request["workspace_root"] = serde_json::json!("/")),
        mutate(&|request| request["network"] = serde_json::json!(true)),
        mutate(&|request| {
            request
                .as_object_mut()
                .map(|object| object.remove("read_paths"));
        }),
    ] {
        assert!(refused(&request), "{request}");
    }
    // A pinned plan needs its route; an unpinned plan takes none.
    assert!(check_route(None, &fixture.plan, &fixture.request, ENABLED, None).is_err());
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    assert!(
        check_route(
            Some(&fixture.route_file),
            &unpinned,
            &fixture.request,
            ENABLED,
            None
        )
        .is_err()
    );
    // A route for another pin, a forged route and a linked overlay.
    let mut other = fixture.plan.clone();
    if let Some(pin) = other.workspace.projection.as_mut() {
        pin.fence = 2;
    }
    assert!(
        check_route(
            Some(&fixture.route_file),
            &other,
            &fixture.request,
            ENABLED,
            None
        )
        .is_err()
    );
    let mut forged = fixture.route.clone();
    forged.write_paths = vec!["alpha".into()];
    ok(std::fs::write(
        &fixture.route_file,
        ok(serde_json::to_vec(&forged)),
    ));
    let widened = mutate(&|request| request["write_paths"] = serde_json::json!(["alpha"]));
    assert!(refused(&widened));
    ok(std::fs::write(
        &fixture.route_file,
        ok(serde_json::to_vec(&fixture.route)),
    ));
    let overlay = std::path::Path::new(&fixture.route.overlay_root);
    let moved = overlay.with_extension("moved");
    ok(std::fs::rename(overlay, &moved));
    ok(std::os::unix::fs::symlink(&moved, overlay));
    assert!(refused(&fixture.request));
}

#[test]
fn projected_dispatch_requires_complete_non_revoked_current_source_authority() {
    let fixture = fixture();
    // Missing PrepareWorkspace
    let missing_pw = match check_with_basalt(
        Some(&fixture.route_file),
        &fixture.plan,
        &fixture.request,
        ENABLED,
        &fixture,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
            ],
        )],
    ) {
        Ok(()) => unreachable!("missing PrepareWorkspace must deny"),
        Err(err) => err,
    };
    assert!(missing_pw.contains("credential-denied"), "{missing_pw}");
    // Missing ModifySource / CreateSource on writable path
    let missing_mod = match check_with_basalt(
        Some(&fixture.route_file),
        &fixture.plan,
        &fixture.request,
        ENABLED,
        &fixture,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::PrepareWorkspace,
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
    ) {
        Ok(()) => unreachable!("missing ModifySource/CreateSource must deny"),
        Err(err) => err,
    };
    assert!(missing_mod.contains("credential-denied"), "{missing_mod}");
    // Revoked token or cancelled attempt denies before dispatch
    let pin = fixture.route.pin.clone();
    let attempt = AttemptFence {
        attempt: fixture.plan.attempt_identity.clone(),
        fence: fixture.plan.fence,
    };
    let source_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        &pin.repository,
        Some(&attempt),
        &full_source_grants(),
        EXPIRES_S,
    );
    source_fix.revocations.revoke(&source_fix.token);
    let signer = source_fix.signer();
    let mut source_auth = source_fix.authority();
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: pin.source,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let sign_request = |req: &SourceAuthorityRequest| {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|error| format!("{}: {}", error.code, error.message))
    };
    let mut dispatch_auth = SourceDispatchAuthority {
        authority: &mut source_auth,
        sign_request: &sign_request,
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let err = match check_route(
        Some(&fixture.route_file),
        &fixture.plan,
        &fixture.request,
        ENABLED,
        Some(&mut dispatch_auth),
    ) {
        Ok(()) => unreachable!("revoked grant must deny"),
        Err(err) => err,
    };
    assert!(err.contains("revoked"), "{err}");
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn only_a_selected_verified_profile_with_projection_enables_projection() {
    use kiln_implementer::authority_config::{
        HoldersConfig, PolicyConfig, ProfileConfig, ReplayConfig, RevocationConfig, SecretRef,
        VerifiedUcanConfig,
    };
    let verified = |projection_enabled: bool| {
        let seed = |name: &str| SecretRef {
            seed_path: format!("/run/kiln/{name}.seed").into(),
        };
        let mut selection = ImplementationAuthorityConfig::absent();
        selection.profile = ProfileConfig::VerifiedUcan(Box::new(VerifiedUcanConfig {
            audience: "kiln".into(),
            generation: 1,
            policy: PolicyConfig::Packaged,
            trusted_roots: vec!["did:key:root".into()],
            keys: Vec::new(),
            proofs_dir: "/run/kiln/proofs".into(),
            revocation: RevocationConfig {
                path: "/run/kiln/revocations.json".into(),
                max_age_ms: 60_000,
            },
            replay: ReplayConfig {
                retention_ms: 60_000,
            },
            holders: HoldersConfig {
                coordinator: seed("coordinator"),
                verifier: seed("verifier"),
                publisher: seed("publisher"),
            },
            projection_enabled,
        }));
        selection
    };
    assert_eq!(
        ProjectionAuthority::of_selected(&verified(true)),
        ProjectionAuthority::ProjectionEnabled
    );
    assert_eq!(
        ProjectionAuthority::of_selected(&ImplementationAuthorityConfig::local_operator(
            "operator"
        )),
        ProjectionAuthority::LocalOperator
    );
    for selection in [verified(false), ImplementationAuthorityConfig::absent()] {
        assert_eq!(
            ProjectionAuthority::of_selected(&selection),
            ProjectionAuthority::Absent
        );
    }
}

#[test]
fn mapped_creation_probes_match_exact_prefix_empty_source_and_overlapping_mounts() {
    let root = ok(tempfile::tempdir());
    let store = ok(std::fs::canonicalize(root.path())).join("store");
    let repository = ok(admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "fixture".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: ok(ExternalRef::new("git", "repository", "fixture")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/fixture".into(),
            }],
        }],
        &[],
    ))
    .pop()
    .unwrap_or_else(|| unreachable!("one repository"));
    let base = ok(ExternalRef::new("git", "commit", &"a".repeat(40)));
    let component_fn = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };

    // 1. Exact-prefix mount: workspace_app -> crates/app with creation_destinations = ["workspace_app"]
    let facts_exact = ProjectionFacts {
        repository: repository.identity().clone(),
        exact_revision: base.clone(),
        canonical_tree: digest("tree-exact"),
        evaluator: component_fn("kiln-projection"),
        adapter: component_fn("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "crates/app/src/lib.rs".into(),
            object: ok(ExternalRef::new("git", "blob", "crates/app/src/lib.rs")),
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let def_exact = ok(admit_definition(&ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
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
    let complete_exact = ok(ok(resolve(&def_exact, &repository, &facts_exact))
        .require_complete(ProjectionOperation::Write));
    let mat_exact = ok(plan(
        &complete_exact,
        &MaterializerProfile {
            schema: MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: MaterializerKind::EagerLocalDirectory,
            materializer: component_fn("kiln-eager-local"),
            platform: TargetProfile::POSIX,
        },
    ));
    let attempt = digest("attempt-exact");
    let pin_exact = ok(mat_exact.pin(&attempt, 1));
    let writable_exact = vec!["crates/app".to_string()];
    let route_exact = ok(route(
        &def_exact,
        &complete_exact,
        &mat_exact,
        &pin_exact,
        RouteRequest {
            store: ok(store.to_str().ok_or("utf-8")),
            writable: &writable_exact,
            build_plan: None,
        },
    ));
    ok(std::fs::create_dir_all(&route_exact.overlay_root));
    let route_file_exact = root.path().join("route-exact.json");
    ok(std::fs::write(
        &route_file_exact,
        ok(serde_json::to_vec(&route_exact)),
    ));
    let plan_exact = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: digest("work-exact"),
        work_revision: 1,
        attempt_identity: attempt.clone(),
        fence: 1,
        repository: "qualification/disposable".into(),
        base: Some(base.clone()),
        workspace: DispatchWorkspace {
            identity: pin_exact.overlay.clone(),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: Some(pin_exact.clone()),
        },
        provider_profile: ok(ProviderProfileId::new("scripted-local-omp")),
        permitted_paths: writable_exact,
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1,
        invocation_budget: 1,
        expected_output: digest("output"),
        execution: None,
    };
    let req_exact = request_of(&route_exact);
    let check_exact = |grants: &[(PathScope, Vec<SourceViewOperation>)]| {
        let f = Fixture {
            _root: ok(tempfile::tempdir()),
            plan: plan_exact.clone(),
            route: route_exact.clone(),
            wider: route_exact.clone(),
            route_file: route_file_exact.clone(),
            request: req_exact.clone(),
        };
        check_with_basalt(
            Some(&route_file_exact),
            &plan_exact,
            &req_exact,
            ENABLED,
            &f,
            grants,
        )
    };
    let crates_app = ok(PathScope::from_path("crates/app", true).ok_or("path"));
    ok(check_exact(&[
        (
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        ),
        (
            crates_app.clone(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
            ],
        ),
    ]));
    let dest_only = check_exact(&[
        (
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        ),
        (
            crates_app,
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
            ],
        ),
        (
            ok(PathScope::from_path("workspace_app/_dispatch_create", false).ok_or("path")),
            vec![SourceViewOperation::CreateSource],
        ),
    ]);
    assert!(
        dest_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{dest_only:?}"
    );

    // 2. Overlapping/shadowed mounts: "" -> "" and "shared" -> "vendor/shared"
    let facts_shadow = ProjectionFacts {
        repository: repository.identity().clone(),
        exact_revision: base.clone(),
        canonical_tree: digest("tree-shadow"),
        evaluator: component_fn("kiln-projection"),
        adapter: component_fn("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "shared/proto/lib.rs".into(),
            object: ok(ExternalRef::new("git", "blob", "shared/proto/lib.rs")),
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let def_shadow = ok(admit_definition(&ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
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
    let complete_shadow = ok(ok(resolve(&def_shadow, &repository, &facts_shadow))
        .require_complete(ProjectionOperation::Write));
    let mat_shadow = ok(plan(
        &complete_shadow,
        &MaterializerProfile {
            schema: MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: MaterializerKind::EagerLocalDirectory,
            materializer: component_fn("kiln-eager-local"),
            platform: TargetProfile::POSIX,
        },
    ));
    let pin_shadow = ok(mat_shadow.pin(&attempt, 2));
    let writable_shadow = vec!["shared".to_string(), "vendor".to_string()];
    let route_shadow = ok(route(
        &def_shadow,
        &complete_shadow,
        &mat_shadow,
        &pin_shadow,
        RouteRequest {
            store: ok(store.to_str().ok_or("utf-8")),
            writable: &writable_shadow,
            build_plan: None,
        },
    ));
    ok(std::fs::create_dir_all(&route_shadow.overlay_root));
    let route_file_shadow = root.path().join("route-shadow.json");
    ok(std::fs::write(
        &route_file_shadow,
        ok(serde_json::to_vec(&route_shadow)),
    ));
    let plan_shadow = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: digest("work-shadow"),
        work_revision: 1,
        attempt_identity: attempt,
        fence: 2,
        repository: "qualification/disposable".into(),
        base: Some(base),
        workspace: DispatchWorkspace {
            identity: pin_shadow.overlay.clone(),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: Some(pin_shadow),
        },
        provider_profile: ok(ProviderProfileId::new("scripted-local-omp")),
        permitted_paths: writable_shadow,
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1,
        invocation_budget: 1,
        expected_output: digest("output"),
        execution: None,
    };
    let req_shadow = request_of(&route_shadow);
    let check_shadow = |grants: &[(PathScope, Vec<SourceViewOperation>)]| {
        let f = Fixture {
            _root: ok(tempfile::tempdir()),
            plan: plan_shadow.clone(),
            route: route_shadow.clone(),
            wider: route_shadow.clone(),
            route_file: route_file_shadow.clone(),
            request: req_shadow.clone(),
        };
        check_with_basalt(
            Some(&route_file_shadow),
            &plan_shadow,
            &req_shadow,
            ENABLED,
            &f,
            grants,
        )
    };
    let shared_scope = ok(PathScope::from_path("shared", true).ok_or("path"));
    let vendor_scope = ok(PathScope::from_path("vendor", true).ok_or("path"));
    let vendor_sub_scope = ok(PathScope::from_path("vendor/shared/sub", true).ok_or("path"));
    let shared_sub_scope = ok(PathScope::from_path("shared/sub", true).ok_or("path"));
    let vendor_only = check_shadow(&[
        (
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        ),
        (
            shared_scope.clone(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
            ],
        ),
        (vendor_sub_scope, vec![SourceViewOperation::CreateSource]),
    ]);
    assert!(
        vendor_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{vendor_only:?}"
    );
    let shared_only = check_shadow(&[
        (
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        ),
        (
            shared_scope.clone(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
            ],
        ),
        (shared_sub_scope, vec![SourceViewOperation::CreateSource]),
    ]);
    assert!(
        shared_only
            .as_ref()
            .is_err_and(|e| e.contains("credential-denied")),
        "{shared_only:?}"
    );
    ok(check_shadow(&[
        (
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        ),
        (
            shared_scope,
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
            ],
        ),
        (vendor_scope, vec![SourceViewOperation::CreateSource]),
    ]));

    // 3. Unit test on mapped_creation_probes when source_prefix is empty and write == destination_prefix
    let mut route_empty_src = route_exact;
    route_empty_src.definition.mounts = vec![Mount {
        source_prefix: String::new(),
        destination_prefix: "workspace_app".into(),
    }];
    let probes = mapped_creation_probes(&route_empty_src, "workspace_app");
    assert_eq!(probes, vec!["_dispatch_create"]);
    assert!(PathScope::from_path("_dispatch_create", false).is_some());
}
