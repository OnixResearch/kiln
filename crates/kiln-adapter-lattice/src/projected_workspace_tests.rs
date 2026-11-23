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
    ProjectionDefinition, ProjectionFacts, ProjectionOperation, ProjectionProfile,
    ProviderBinding, RepositoryEnrollment, RepositoryLocator, Selection, SourceCapability,
    SourceEntry, TargetProfile, admit_definition, admit_repositories, resolve,
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
    route_file: std::path::PathBuf,
    request: serde_json::Value,
}

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
    let complete = ok(ok(resolve(&definition, &repository, &facts))
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
    let derived = ok(route(
        &definition,
        &complete,
        &materialization,
        &pin,
        RouteRequest {
            store: ok(store.to_str().ok_or("utf-8")),
            writable: &writable,
            build_plan: None,
        },
    ));
    ok(std::fs::create_dir_all(&derived.overlay_root));
    let route_file = root.path().join("route.json");
    ok(std::fs::write(&route_file, ok(serde_json::to_vec(&derived))));
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
        route_file,
        request,
    }
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
    ));
    // A plan without a pin keeps the earlier route and takes no route file.
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    ok(check_route(None, &unpinned, &fixture.request));
}

// r[verify kiln.source.projection.route.scenario.dispatch]
#[test]
fn requests_differing_from_the_route_are_refused_before_dispatch() {
    let fixture = fixture();
    let refused = |request: &serde_json::Value| {
        check_route(Some(&fixture.route_file), &fixture.plan, request)
            .is_err_and(|error| error.ends_with("nothing was dispatched"))
    };
    let mutate = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut request = fixture.request.clone();
        change(&mut request);
        request
    };
    for request in [
        mutate(&|request| request["write_paths"] = serde_json::json!(["alpha"])),
        mutate(&|request| request["write_paths"] = serde_json::json!(["alpha/src/lib.rs", "alpha/Cargo.toml"])),
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
    assert!(check_route(None, &fixture.plan, &fixture.request).is_err());
    let mut unpinned = fixture.plan.clone();
    unpinned.workspace.projection = None;
    assert!(check_route(Some(&fixture.route_file), &unpinned, &fixture.request).is_err());
    // A route for another pin, a forged route and a linked overlay.
    let mut other = fixture.plan.clone();
    if let Some(pin) = other.workspace.projection.as_mut() {
        pin.fence = 2;
    }
    assert!(check_route(Some(&fixture.route_file), &other, &fixture.request).is_err());
    let mut forged = fixture.route.clone();
    forged.write_paths = vec!["alpha".into()];
    ok(std::fs::write(&fixture.route_file, ok(serde_json::to_vec(&forged))));
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
