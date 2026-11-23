//! The projected workspace route (C9, `kiln.projected-workspace-route.v1`).
use alloc::string::ToString;
use alloc::vec;

use super::super::{
    AdmittedDefinition, AdmittedRepository, Coverage, DEFINITION_SCHEMA, GeneratedInputs, Mount,
    ProjectionDefinition, ProjectionFacts, ProjectionProfile, ProviderBinding,
    RepositoryEnrollment, RepositoryLocator, Selection, SourceCapability, SourceEntry,
    admit_definition, admit_repositories, resolve,
};
use super::*;
use crate::types::ProviderProfileId;

const STORE: &str = "/var/lib/kiln/materialized";

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn codes<T: core::fmt::Debug>(result: Result<T, Vec<Diagnostic>>) -> BTreeSet<String> {
    match result {
        Ok(value) => unreachable!("expected a refusal, derived {value:?}"),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn repository() -> AdmittedRepository {
    let enrolled = [RepositoryEnrollment {
        repository_id: "fixture".into(),
        binding: ProviderBinding {
            source_provider: ok(ProviderProfileId::new("git-source-v1")),
            native_repository: ok(ExternalRef::new("git", "repository", "fixture")),
        },
        locators: vec![RepositoryLocator::LocalPath {
            path: "/srv/fixture".into(),
        }],
    }];
    ok(admit_repositories(&enrolled, &[]))
        .pop()
        .unwrap_or_else(|| unreachable!("one admitted repository"))
}

const SOURCE: [&str; 7] = [
    "Cargo.toml",
    "projects/alpha/Cargo.toml",
    "projects/alpha/src/lib.rs",
    "projects/beta/src/lib.rs",
    "shared/proto/lib.rs",
    "shared/proto/build.rs",
    "tools/release.sh",
];

fn facts() -> ProjectionFacts {
    ProjectionFacts {
        repository: repository().identity().clone(),
        exact_revision: ok(ExternalRef::new("git", "commit", &"a".repeat(40))),
        canonical_tree: digest("tree"),
        evaluator: ComponentVersion {
            name: "kiln-projection".into(),
            version: "1".into(),
        },
        adapter: ComponentVersion {
            name: "git-source".into(),
            version: "1".into(),
        },
        source_capabilities: vec![SourceCapability::ExactTree, SourceCapability::ExecutableBit],
        coverage: Coverage::Complete,
        entries: SOURCE
            .iter()
            .map(|path| SourceEntry {
                path: (*path).into(),
                object: ok(ExternalRef::new("git", "blob", path)),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::new(),
        generated: Vec::new(),
    }
}

/// Alpha and the root manifest, with the shared directory also mounted at
/// `vendor/shared`, and two creation destinations.
fn writable() -> ProjectionDefinition {
    ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: "alpha-write".into(),
        profile: ProjectionProfile::Writable,
        selection: Selection {
            root: String::new(),
            include: vec![
                "Cargo.toml".into(),
                "projects/alpha/**".into(),
                "shared/**".into(),
            ],
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
        creation_destinations: vec!["projects/alpha/src/new".into(), "notes".into()],
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }
}

struct View {
    definition: AdmittedDefinition,
    complete: CompleteProjection,
    plan: MaterializationPlan,
    pin: WorkspacePin,
}

fn view_for(definition: &ProjectionDefinition, operation: ProjectionOperation) -> View {
    let definition = ok(admit_definition(definition));
    let complete =
        ok(ok(resolve(&definition, &repository(), &facts())).require_complete(operation));
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: ComponentVersion {
            name: "kiln-eager-local".into(),
            version: "1".into(),
        },
        platform: TargetProfile::POSIX,
    };
    let plan = ok(plan(&complete, &profile));
    let pin = ok(plan.pin(&digest("attempt"), 3));
    View {
        definition,
        complete,
        plan,
        pin,
    }
}

fn derive(view: &View, writable: &[&str]) -> Result<ProjectedWorkspaceRoute, Vec<Diagnostic>> {
    let writable: Vec<String> = writable.iter().map(|path| (*path).to_string()).collect();
    route(
        &view.definition,
        &view.complete,
        &view.plan,
        &view.pin,
        RouteRequest {
            store: STORE,
            writable: &writable,
            build_plan: None,
        },
    )
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

// r[verify kiln.source.projection.route]
// r[verify kiln.source.projection.route.scenario.derived]
#[test]
fn a_write_route_grants_only_writable_origins_and_creation_destinations() {
    let view = view_for(&writable(), ProjectionOperation::Write);
    let derived = ok(derive(
        &view,
        &["projects/alpha/src", "shared/proto/lib.rs"],
    ));
    assert_eq!(derived.schema, WORKSPACE_ROUTE_SCHEMA);
    assert_eq!(derived.runtime, LATTICE_BWRAP_RUNTIME);
    assert_eq!(
        derived.overlay_root,
        format!("{STORE}/overlays/{}", view.pin.overlay.as_str())
    );
    assert_eq!(
        derived.input_root,
        format!("{STORE}/inputs/{}", view.pin.input.as_str())
    );
    assert_eq!(derived.store(), Some(STORE));
    // Every top-level destination, the creation destination `notes` included.
    assert_eq!(
        derived.read_paths,
        strings(&["Cargo.toml", "notes", "projects", "shared", "vendor"])
    );
    // Both aliases of a writable origin, the alpha sources, and the creation
    // destination below them; `notes` maps to an unwritable source path and
    // the alpha manifest, the root manifest and `build.rs` stay read-only.
    assert_eq!(
        derived.write_paths,
        strings(&[
            "projects/alpha/src/lib.rs",
            "projects/alpha/src/new",
            "shared/proto/lib.rs",
            "vendor/shared/proto/lib.rs",
        ])
    );
    ok(derived.verify());
    assert!(derived.admits(
        &derived.overlay_root,
        &derived.read_paths,
        &derived.write_paths
    ));
    // Derivation is reproducible and the identity follows the grant.
    assert_eq!(
        ok(derive(
            &view,
            &["projects/alpha/src", "shared/proto/lib.rs"]
        )),
        derived
    );
    let narrower = ok(derive(&view, &["projects/alpha/src"]));
    assert_ne!(narrower.identity, derived.identity);
    assert_eq!(
        narrower.write_paths,
        strings(&["projects/alpha/src/lib.rs", "projects/alpha/src/new"])
    );
    // A writable directory grants each origin below it and the creation
    // destinations it holds, never the directory itself.
    let whole = ok(derive(&view, &["projects", "notes"]));
    assert_eq!(
        whole.write_paths,
        strings(&[
            "notes",
            "projects/alpha/Cargo.toml",
            "projects/alpha/src/lib.rs",
            "projects/alpha/src/new",
        ])
    );
}

// r[verify kiln.source.projection.route.scenario.derived]
#[test]
fn a_build_route_is_read_only() {
    let view = view_for(&writable(), ProjectionOperation::Build);
    let derived = ok(derive(&view, &[]));
    assert!(derived.write_paths.is_empty());
    // Creation destinations belong to writes only.
    assert_eq!(
        derived.read_paths,
        strings(&["Cargo.toml", "projects", "shared", "vendor"])
    );
    ok(derived.verify());
}

// r[verify kiln.source.projection.route.scenario.refused]
#[test]
fn unsafe_or_crossed_route_requests_are_refused() {
    let view = view_for(&writable(), ProjectionOperation::Write);
    let build = view_for(&writable(), ProjectionOperation::Build);
    assert_eq!(
        codes(derive(&build, &["projects/alpha/src"])),
        BTreeSet::from(["route_write_requires_write_witness".to_string()])
    );
    assert_eq!(
        codes(derive(&view, &["../escape"])),
        BTreeSet::from(["route_writable_invalid".to_string()])
    );
    let too_many: Vec<String> = (0..=MAX_ROUTE_WRITABLE)
        .map(|index| format!("projects/p{index}"))
        .collect();
    assert_eq!(
        codes(route(
            &view.definition,
            &view.complete,
            &view.plan,
            &view.pin,
            RouteRequest {
                store: STORE,
                writable: &too_many,
                build_plan: None,
            },
        )),
        BTreeSet::from(["route_bound".to_string()])
    );
    for store in ["relative/store", "/var/../etc", "/", "/var//lib"] {
        assert_eq!(
            codes(route(
                &view.definition,
                &view.complete,
                &view.plan,
                &view.pin,
                RouteRequest {
                    store,
                    writable: &[],
                    build_plan: None,
                },
            )),
            BTreeSet::from(["route_store".to_string()]),
            "{store}"
        );
    }
    // A pin the plan does not derive, or another view's plan.
    let mut forged = view.pin.clone();
    forged.manifest = digest("other-manifest");
    assert_eq!(
        codes(route(
            &view.definition,
            &view.complete,
            &view.plan,
            &forged,
            RouteRequest {
                store: STORE,
                writable: &[],
                build_plan: None,
            },
        )),
        BTreeSet::from(["route_pin_mismatch".to_string()])
    );
    let mut narrower = writable();
    narrower.selection.include.pop();
    let other = view_for(&narrower, ProjectionOperation::Write);
    assert_eq!(
        codes(route(
            &view.definition,
            &view.complete,
            &other.plan,
            &other.pin,
            RouteRequest {
                store: STORE,
                writable: &[],
                build_plan: None,
            },
        )),
        BTreeSet::from(["route_plan_mismatch".to_string()])
    );
}

// r[verify kiln.source.projection.route.scenario.refused]
#[test]
fn a_recorded_route_verifies_only_unchanged() {
    let view = view_for(&writable(), ProjectionOperation::Write);
    let derived = ok(derive(&view, &["projects/alpha/src"]));
    let recorded: ProjectedWorkspaceRoute =
        ok(serde_json::from_slice(&ok(serde_json::to_vec(&derived))));
    ok(recorded.verify());
    let refused = |change: fn(&mut ProjectedWorkspaceRoute), code: &str| {
        let mut changed = derived.clone();
        change(&mut changed);
        assert!(
            changed
                .verify()
                .is_err_and(|diagnostics| diagnostics.iter().any(|d| d.code == code)),
            "{code}"
        );
    };
    // A widened grant that is still well formed fails the identity.
    refused(
        |route| route.write_paths.push("shared".into()),
        "route_identity",
    );
    refused(
        |route| route.write_paths.insert(0, "projects".into()),
        "route_paths",
    );
    refused(
        |route| route.write_paths = vec!["outside/file".into()],
        "route_paths",
    );
    refused(|route| route.read_paths.reverse(), "route_paths");
    refused(|route| route.read_paths.push("/etc".into()), "route_paths");
    refused(
        |route| route.overlay_root = "/tmp/elsewhere".into(),
        "route_roots",
    );
    refused(
        |route| route.input_root = format!("{STORE}/inputs/other"),
        "route_roots",
    );
    refused(|route| route.runtime = "host".into(), "route_schema");
    refused(|route| route.pin.fence = 9, "materialization_pin");
    refused(
        |route| route.build_plan = Some(digest("other-plan")),
        "route_identity",
    );
    // A request that differs in any path is not admitted.
    let mut wider = derived.write_paths.clone();
    wider.push("vendor".into());
    assert!(!derived.admits(&derived.overlay_root, &derived.read_paths, &wider));
    assert!(!derived.admits(
        &derived.input_root,
        &derived.read_paths,
        &derived.write_paths
    ));
    let mut fewer = derived.read_paths.clone();
    fewer.pop();
    assert!(!derived.admits(&derived.overlay_root, &fewer, &derived.write_paths));
}
