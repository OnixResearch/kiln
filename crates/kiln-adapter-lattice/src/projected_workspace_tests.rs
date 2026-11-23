use super::*;
use std::collections::BTreeSet;

use kiln_core::implementation::{
    DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, WorkspaceIsolation,
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

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn a_request_equal_to_the_route_of_the_pinned_plan_is_admitted() {
    let fixture = fixture();
    // The writable creation destination holds the writable origin.
    assert_eq!(fixture.route.write_paths, ["alpha/src"]);
    assert_eq!(fixture.route.read_paths, ["alpha"]);
    ok(check_route(
        Some(&fixture.route_file),
        &fixture.plan,
        &fixture.request,
        ENABLED,
    ));
    // A plan without a pin keeps the earlier route and takes no route file,
    // under any profile.
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    for authority in [
        ENABLED,
        ProjectionAuthority::LocalOperator,
        ProjectionAuthority::Absent,
    ] {
        ok(check_route(None, &unpinned, &fixture.request, authority));
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
            let refused = check_route(route_file, plan, &fixture.request, authority);
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
    let refused = check_route(
        Some(&fixture.route_file),
        &fixture.plan,
        &request_of(&fixture.wider),
        ENABLED,
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
    ok(check_route(
        Some(&fixture.route_file),
        &wide_plan,
        &request_of(&fixture.wider),
        ENABLED,
    ));
    // The plan's own route is refused for the wider plan too: equality, not
    // a subset.
    write_route(&fixture, &fixture.route);
    assert!(
        check_route(
            Some(&fixture.route_file),
            &wide_plan,
            &fixture.request,
            ENABLED
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
        let refused = check_route(
            Some(&fixture.route_file),
            &fixture.plan,
            &request_of(route),
            ENABLED,
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
        check_route(Some(&fixture.route_file), &fixture.plan, request, ENABLED)
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
    assert!(check_route(None, &fixture.plan, &fixture.request, ENABLED).is_err());
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    assert!(
        check_route(
            Some(&fixture.route_file),
            &unpinned,
            &fixture.request,
            ENABLED
        )
        .is_err()
    );
    // A route for another pin, a forged route and a linked overlay.
    let mut other = fixture.plan.clone();
    if let Some(pin) = other.workspace.projection.as_mut() {
        pin.fence = 2;
    }
    assert!(check_route(Some(&fixture.route_file), &other, &fixture.request, ENABLED).is_err());
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
