use alloc::string::ToString;
use alloc::vec;

use super::super::{
    AdmittedRepository, DEFINITION_SCHEMA, DependencyFact, EntryKind, GeneratedEntry,
    GeneratedInputs, Mount, ProjectionDefinition, ProjectionProfile, ProviderBinding,
    RepositoryEnrollment, RepositoryLocator, Selection, SourceCapability, SourceEntry,
    TargetProfile, admit_definition, admit_repositories, resolve,
};
use super::*;
use crate::monorepo::{BuildGraph, Package};
use crate::types::ProviderProfileId;

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn refused<T: core::fmt::Debug>(result: Result<T, Vec<Diagnostic>>) -> Vec<Diagnostic> {
    match result {
        Ok(value) => unreachable!("expected a refusal, planned {value:?}"),
        Err(diagnostics) => diagnostics,
    }
}

fn codes(diagnostics: &[Diagnostic]) -> BTreeSet<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
    ok(ExternalRef::new(namespace, kind, id))
}

fn component(name: &str, version: &str) -> ComponentVersion {
    ComponentVersion {
        name: name.into(),
        version: version.into(),
    }
}

fn repository() -> AdmittedRepository {
    let enrolled = [RepositoryEnrollment {
        repository_id: "fixture".into(),
        binding: ProviderBinding {
            source_provider: ok(ProviderProfileId::new("git-source-v1")),
            native_repository: external("git", "repository", "fixture"),
        },
        locators: vec![RepositoryLocator::LocalPath {
            path: "/srv/fixture".into(),
        }],
    }];
    ok(admit_repositories(&enrolled, &[]))
        .pop()
        .unwrap_or_else(|| unreachable!("one admitted repository"))
}

/// A two-project Cargo workspace with a shared path dependency.
const WORKSPACE: [&str; 13] = [
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "docs/README.md",
    "projects/alpha/Cargo.toml",
    "projects/alpha/assets/banner.txt",
    "projects/alpha/build.rs",
    "projects/alpha/src/lib.rs",
    "projects/beta/Cargo.toml",
    "projects/beta/src/lib.rs",
    "rust-toolchain.toml",
    "shared/proto/Cargo.toml",
    "shared/proto/src/lib.rs",
];

fn facts(paths: &[&str], tree: &str) -> ProjectionFacts {
    ProjectionFacts {
        repository: repository().identity().clone(),
        exact_revision: external("git", "commit", &"c".repeat(40)),
        canonical_tree: digest(tree),
        evaluator: component("kiln-projection", "1"),
        adapter: component("git-source", "1"),
        source_capabilities: vec![
            SourceCapability::ExactTree,
            SourceCapability::ExecutableBit,
            SourceCapability::Symlinks,
        ],
        coverage: Coverage::Complete,
        entries: paths
            .iter()
            .map(|path| SourceEntry {
                path: (*path).to_string(),
                object: external("git", "blob", &format!("{tree}:{path}")),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::<DependencyFact>::new(),
        generated: Vec::new(),
    }
}

fn definition(include: &[&str], mounts: &[(&str, &str)]) -> ProjectionDefinition {
    ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: "view".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: include
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect(),
            exclude: Vec::new(),
        },
        mounts: mounts
            .iter()
            .map(|(source, destination)| Mount {
                source_prefix: (*source).into(),
                destination_prefix: (*destination).into(),
            })
            .collect(),
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }
}

fn complete_for(
    definition: &ProjectionDefinition,
    facts: &ProjectionFacts,
    operation: ProjectionOperation,
) -> CompleteProjection {
    ok(ok(resolve(
        &ok(admit_definition(definition)),
        &repository(),
        facts,
    ))
    .require_complete(operation))
}

fn view(include: &[&str]) -> CompleteProjection {
    complete_for(
        &definition(include, &[("", "")]),
        &facts(&WORKSPACE, "tree-1"),
        ProjectionOperation::Build,
    )
}

/// Alpha, the shared dependency and every root file.
const ALPHA_VIEW: [&str; 6] = [
    "projects/alpha/**",
    "shared/**",
    "Cargo.toml",
    "Cargo.lock",
    ".cargo/**",
    "rust-toolchain.toml",
];

fn package(name: &str, dir: &str, workspace: Option<&str>) -> CargoPackage {
    CargoPackage {
        name: name.into(),
        dir: dir.into(),
        workspace: workspace.map(Into::into),
        outside_paths: 0,
        external_packages: false,
    }
}

fn members(literal: &[&str], glob: &[&str]) -> CargoMembers {
    CargoMembers::Known {
        literal: literal.iter().map(|dir| (*dir).to_string()).collect(),
        glob: glob.iter().map(|dir| (*dir).to_string()).collect(),
    }
}

fn layout(members: CargoMembers) -> CargoLayout {
    CargoLayout {
        packages: vec![
            package("alpha", "projects/alpha", Some("")),
            package("beta", "projects/beta", Some("")),
            package("proto", "shared/proto", Some("")),
        ],
        workspaces: vec![CargoWorkspace {
            dir: String::new(),
            members,
        }],
        edges: vec![
            ("projects/alpha".into(), "shared/proto".into()),
            ("projects/beta".into(), "shared/proto".into()),
        ],
    }
}

fn glob_layout() -> CargoLayoutResult {
    CargoLayoutResult::Available(layout(members(
        &[],
        &["projects/alpha", "projects/beta", "shared/proto"],
    )))
}

fn request(targets: &[&str]) -> BuildRequest {
    BuildRequest {
        schema: BUILD_REQUEST_SCHEMA.into(),
        profile: BuildProfile::CargoCompatibilityV1,
        targets: targets.iter().map(|target| (*target).to_string()).collect(),
        command: ["cargo", "test", "--offline", "--locked", "-p", "alpha"]
            .map(Into::into)
            .to_vec(),
        toolchain: component("rust", "1.88.0"),
        environment: BTreeMap::from([("CARGO_TERM_COLOR".into(), "never".into())]),
    }
}

fn plan_alpha(
    view: &CompleteProjection,
    layout: &CargoLayoutResult,
) -> Result<BuildInputPlan, Vec<Diagnostic>> {
    plan_cargo(
        view,
        &facts(&WORKSPACE, "tree-1"),
        layout,
        &request(&["alpha"]),
    )
}

fn roles(plan: &BuildInputPlan) -> Vec<(&str, InputRole)> {
    plan.inputs()
        .iter()
        .map(|input| (input.path.as_str(), input.role))
        .collect()
}

// r[verify kiln.source.projection.build_inputs.scenario.planned]
#[test]
fn a_project_view_with_its_shared_dependency_is_planned() {
    let plan = ok(plan_alpha(&view(&ALPHA_VIEW), &glob_layout()));
    assert_eq!(plan.build_root(), "");
    assert_eq!(plan.packages(), ["projects/alpha", "shared/proto"]);
    assert_eq!(
        roles(&plan),
        [
            (".cargo/config.toml", InputRole::Configuration),
            ("Cargo.lock", InputRole::Lockfile),
            ("Cargo.toml", InputRole::WorkspaceManifest),
            ("projects/alpha/Cargo.toml", InputRole::Package),
            ("projects/alpha/assets/banner.txt", InputRole::Package),
            ("projects/alpha/build.rs", InputRole::Package),
            ("projects/alpha/src/lib.rs", InputRole::Package),
            ("rust-toolchain.toml", InputRole::Toolchain),
            ("shared/proto/Cargo.toml", InputRole::Package),
            ("shared/proto/src/lib.rs", InputRole::Package),
        ]
    );
    assert_eq!(plan.external_packages(), ExternalPackages::None);
    assert_eq!(plan.profile(), BuildProfile::CargoCompatibilityV1);
    // Planning again over the same facts is reproducible.
    let again = ok(plan_alpha(&view(&ALPHA_VIEW), &glob_layout()));
    assert_eq!(again.identity(), plan.identity());
}

#[test]
fn registry_and_git_dependencies_stay_outside_the_source_closure() {
    let mut facts_layout = layout(members(
        &[],
        &["projects/alpha", "projects/beta", "shared/proto"],
    ));
    if let Some(proto) = facts_layout.packages.get_mut(2) {
        proto.external_packages = true;
    }
    let plan = ok(plan_alpha(
        &view(&ALPHA_VIEW),
        &CargoLayoutResult::Available(facts_layout),
    ));
    assert_eq!(plan.external_packages(), ExternalPackages::PinnedByLockfile);
}

// r[verify kiln.source.projection.build_inputs.scenario.refused]
#[test]
fn missing_root_files_and_dependencies_are_refused_by_role() {
    let cases: [(&[&str], &str); 4] = [
        (
            &[
                "projects/alpha/**",
                "Cargo.toml",
                "Cargo.lock",
                ".cargo/**",
                "rust-toolchain.toml",
            ],
            "inputs.shared/proto/src/lib.rs",
        ),
        (
            &[
                "projects/alpha/**",
                "shared/**",
                "Cargo.toml",
                ".cargo/**",
                "rust-toolchain.toml",
            ],
            "inputs.Cargo.lock",
        ),
        (
            &[
                "projects/alpha/**",
                "shared/**",
                "Cargo.toml",
                "Cargo.lock",
                "rust-toolchain.toml",
            ],
            "inputs..cargo/config.toml",
        ),
        (
            &[
                "projects/alpha/**",
                "shared/**",
                "Cargo.lock",
                ".cargo/**",
                "rust-toolchain.toml",
            ],
            "inputs.Cargo.toml",
        ),
    ];
    for (include, field) in cases {
        let diagnostics = refused(plan_alpha(&view(include), &glob_layout()));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "required_input_absent" && d.field == field),
            "{field}: {diagnostics:?}"
        );
    }
    let toolchain = refused(plan_alpha(
        &view(&[
            "projects/alpha/**",
            "shared/**",
            "Cargo.toml",
            "Cargo.lock",
            ".cargo/**",
        ]),
        &glob_layout(),
    ));
    assert!(
        toolchain
            .iter()
            .any(|d| d.field == "inputs.rust-toolchain.toml" && d.message.contains("toolchain"))
    );
}

#[test]
fn a_candidate_without_a_lockfile_is_refused() {
    let paths: Vec<&str> = WORKSPACE
        .iter()
        .copied()
        .filter(|path| *path != "Cargo.lock")
        .collect();
    let source = facts(&paths, "tree-1");
    let complete = complete_for(
        &definition(&ALPHA_VIEW, &[("", "")]),
        &source,
        ProjectionOperation::Build,
    );
    let diagnostics = refused(plan_cargo(
        &complete,
        &source,
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert_eq!(codes(&diagnostics), BTreeSet::from(["lockfile_absent"]));
}

#[test]
fn workspace_members_cargo_loads_must_be_whole() {
    // A literal member is loaded even when no target needs it.
    let literal = CargoLayoutResult::Available(layout(members(
        &["projects/alpha", "projects/beta", "shared/proto"],
        &[],
    )));
    let diagnostics = refused(plan_alpha(&view(&ALPHA_VIEW), &literal));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "required_input_absent"
                && d.field == "inputs.projects/beta/src/lib.rs")
    );
    // A glob member the view holds part of is loaded and must be whole.
    let mut partial: Vec<&str> = ALPHA_VIEW.to_vec();
    partial.push("projects/beta/Cargo.toml");
    let diagnostics = refused(plan_alpha(&view(&partial), &glob_layout()));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "required_input_absent"
                && d.field == "inputs.projects/beta/src/lib.rs")
    );
    // A wholly absent glob member is not loaded.
    ok(plan_alpha(&view(&ALPHA_VIEW), &glob_layout()));
    // Unknown members are never guessed.
    let unknown = CargoLayoutResult::Available(layout(CargoMembers::Unknown {
        reason: "non-array members".into(),
    }));
    let diagnostics = refused(plan_alpha(&view(&ALPHA_VIEW), &unknown));
    assert!(codes(&diagnostics).contains("workspace_members_unknown"));
    // A literal member without a package manifest cannot be loaded.
    let missing = CargoLayoutResult::Available(layout(members(&["projects/gamma"], &[])));
    let diagnostics = refused(plan_alpha(&view(&ALPHA_VIEW), &missing));
    assert!(codes(&diagnostics).contains("workspace_member_missing"));
}

#[test]
fn unavailable_inconsistent_or_unsupported_layouts_are_refused() {
    let complete = view(&ALPHA_VIEW);
    let unavailable = refused(plan_alpha(
        &complete,
        &CargoLayoutResult::Unavailable("unparsable manifest".into()),
    ));
    assert_eq!(codes(&unavailable), BTreeSet::from(["layout_unavailable"]));
    let mut dangling = layout(members(&[], &["projects/alpha"]));
    dangling
        .edges
        .push(("projects/alpha".into(), "vendor/gone".into()));
    let inconsistent = refused(plan_alpha(
        &complete,
        &CargoLayoutResult::Available(dangling),
    ));
    assert_eq!(
        codes(&inconsistent),
        BTreeSet::from(["layout_inconsistent"])
    );
    let mut outside = layout(members(&[], &["projects/alpha", "shared/proto"]));
    if let Some(proto) = outside.packages.get_mut(2) {
        proto.outside_paths = 1;
    }
    let outside = refused(plan_alpha(
        &complete,
        &CargoLayoutResult::Available(outside),
    ));
    assert!(
        outside
            .iter()
            .any(|d| d.code == "outside_path_dependency" && d.field == "packages.shared/proto")
    );
}

#[test]
fn targets_must_name_one_package_under_one_build_root() {
    let complete = view(&ALPHA_VIEW);
    let source = facts(&WORKSPACE, "tree-1");
    let unknown = refused(plan_cargo(
        &complete,
        &source,
        &glob_layout(),
        &request(&["gamma"]),
    ));
    assert_eq!(codes(&unknown), BTreeSet::from(["unknown_target"]));
    let mut twice = layout(members(&[], &["projects/alpha"]));
    twice.packages.push(package("alpha", "vendor/alpha", None));
    let ambiguous = refused(plan_cargo(
        &complete,
        &source,
        &CargoLayoutResult::Available(twice),
        &request(&["alpha"]),
    ));
    assert_eq!(codes(&ambiguous), BTreeSet::from(["ambiguous_target"]));
    let mut nested = layout(members(&[], &["projects/alpha", "shared/proto"]));
    nested.workspaces.push(CargoWorkspace {
        dir: "projects/beta".into(),
        members: members(&[], &[]),
    });
    if let Some(beta) = nested.packages.get_mut(1) {
        beta.workspace = Some("projects/beta".into());
    }
    let several = refused(plan_cargo(
        &complete,
        &source,
        &CargoLayoutResult::Available(nested),
        &request(&["alpha", "beta"]),
    ));
    assert_eq!(codes(&several), BTreeSet::from(["several_build_roots"]));
}

#[test]
fn relocated_generated_and_stale_entries_are_refused_before_planning() {
    let source = facts(&WORKSPACE, "tree-1");
    let relocated = complete_for(
        &definition(
            &ALPHA_VIEW,
            &[("projects/alpha", "alpha"), ("shared", "shared"), ("", "")],
        ),
        &source,
        ProjectionOperation::Build,
    );
    let diagnostics = refused(plan_cargo(
        &relocated,
        &source,
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert!(codes(&diagnostics).contains("relocation_not_qualified"));

    let mut generated_facts = source.clone();
    generated_facts.generated.push(GeneratedEntry {
        destination: "projects/alpha/src/generated.rs".into(),
        kind: EntryKind::File,
        generator: component("protoc", "1"),
        input_identity: digest("schema"),
    });
    let mut admitted = definition(&ALPHA_VIEW, &[("", "")]);
    admitted.generated_inputs = GeneratedInputs::Admitted;
    let generated = complete_for(&admitted, &generated_facts, ProjectionOperation::Build);
    let diagnostics = refused(plan_cargo(
        &generated,
        &generated_facts,
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert_eq!(
        codes(&diagnostics),
        BTreeSet::from(["generated_input_not_modeled"])
    );

    let stale = refused(plan_cargo(
        &view(&ALPHA_VIEW),
        &facts(&WORKSPACE, "tree-2"),
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert!(codes(&stale).contains("stale_source_facts"));
    let mut truncated = source.clone();
    truncated.coverage = Coverage::Truncated;
    let incomplete = refused(plan_cargo(
        &view(&ALPHA_VIEW),
        &truncated,
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert_eq!(
        codes(&incomplete),
        BTreeSet::from(["incomplete_source_facts"])
    );
}

#[test]
fn malformed_requests_and_write_witnesses_are_refused() {
    let complete = view(&ALPHA_VIEW);
    let source = facts(&WORKSPACE, "tree-1");
    let mut wrong = request(&["alpha"]);
    wrong.schema = "kiln.build-input.request.v0".into();
    wrong.profile = BuildProfile::NixCompatibilityV1;
    wrong.command = Vec::new();
    wrong.toolchain = component("", "1");
    wrong.environment = BTreeMap::from([("1BAD".into(), "x".into())]);
    let diagnostics = refused(plan_cargo(&complete, &source, &glob_layout(), &wrong));
    assert_eq!(
        codes(&diagnostics),
        BTreeSet::from([
            "build_command",
            "build_environment",
            "build_profile_mismatch",
            "build_request_schema",
            "build_toolchain",
        ])
    );
    let mut writable = definition(&ALPHA_VIEW, &[("", "")]);
    writable.profile = ProjectionProfile::Writable;
    let write = complete_for(&writable, &source, ProjectionOperation::Write);
    let diagnostics = refused(plan_cargo(
        &write,
        &source,
        &glob_layout(),
        &request(&["alpha"]),
    ));
    assert_eq!(
        codes(&diagnostics),
        BTreeSet::from(["build_witness_required"])
    );
}

// r[verify kiln.source.projection.build_inputs.scenario.applicability]
#[test]
fn evidence_applies_only_to_the_same_plan() {
    let base = ok(plan_alpha(&view(&ALPHA_VIEW), &glob_layout()));
    assert!(base.accepts_evidence(base.identity()));

    let mut wider: Vec<&str> = ALPHA_VIEW.to_vec();
    wider.push("docs/**");
    let layout_changed = ok(plan_alpha(&view(&wider), &glob_layout()));

    let source = facts(&WORKSPACE, "tree-2");
    let input_changed = ok(plan_cargo(
        &complete_for(
            &definition(&ALPHA_VIEW, &[("", "")]),
            &source,
            ProjectionOperation::Build,
        ),
        &source,
        &glob_layout(),
        &request(&["alpha"]),
    ));

    let mut toolchain = request(&["alpha"]);
    toolchain.toolchain = component("rust", "1.89.0");
    let mut command = request(&["alpha"]);
    command.command.push("--release".into());
    let mut environment = request(&["alpha"]);
    environment
        .environment
        .insert("RUSTFLAGS".into(), "-Cdebuginfo=0".into());
    let source = facts(&WORKSPACE, "tree-1");
    let complete = view(&ALPHA_VIEW);
    let variants = [
        layout_changed,
        input_changed,
        ok(plan_cargo(&complete, &source, &glob_layout(), &toolchain)),
        ok(plan_cargo(&complete, &source, &glob_layout(), &command)),
        ok(plan_cargo(&complete, &source, &glob_layout(), &environment)),
    ];
    for variant in &variants {
        assert_ne!(variant.identity(), base.identity());
        assert!(!variant.accepts_evidence(base.identity()));
        assert!(!base.accepts_evidence(variant.identity()));
    }
}

const FLAKES: [&str; 6] = [
    "README.md",
    "nix/app/flake.lock",
    "nix/app/flake.nix",
    "nix/lib/default.nix",
    "nix/lib/flake.nix",
    "nix/other/default.nix",
];

fn nix_graph() -> GraphFacts {
    GraphFacts {
        kind: GraphKind::Nix,
        result: GraphResult::Available(BuildGraph {
            packages: vec![
                Package {
                    id: "nix/app".into(),
                    dir: "nix/app".into(),
                },
                Package {
                    id: "nix/lib".into(),
                    dir: "nix/lib".into(),
                },
            ],
            edges: vec![("nix/app".into(), "nix/lib".into())],
            roots_touching_all: vec!["flake.lock".into(), "flake.nix".into()],
        }),
    }
}

fn nix_request() -> BuildRequest {
    BuildRequest {
        schema: BUILD_REQUEST_SCHEMA.into(),
        profile: BuildProfile::NixCompatibilityV1,
        targets: vec!["nix/app".into()],
        command: ["nix", "build", "./nix/app"].map(Into::into).to_vec(),
        toolchain: component("nix", "2.28"),
        environment: BTreeMap::new(),
    }
}

// r[verify kiln.source.projection.build_inputs.scenario.nix]
#[test]
fn restricted_nix_views_are_refused_and_whole_views_include_path_inputs() {
    let source = facts(&FLAKES, "flakes-1");
    let restricted = complete_for(
        &definition(&["nix/app/**", "nix/lib/**"], &[("", "")]),
        &source,
        ProjectionOperation::Build,
    );
    let diagnostics = refused(plan_nix(&restricted, &source, &nix_graph(), &nix_request()));
    assert_eq!(codes(&diagnostics), BTreeSet::from(["restricted_nix_view"]));

    let whole = complete_for(
        &definition(&["**"], &[("", "")]),
        &source,
        ProjectionOperation::Build,
    );
    let plan = ok(plan_nix(&whole, &source, &nix_graph(), &nix_request()));
    assert_eq!(plan.packages(), ["nix/app", "nix/lib"]);
    assert_eq!(
        roles(&plan),
        [
            ("nix/app/flake.lock", InputRole::Lockfile),
            ("nix/app/flake.nix", InputRole::Package),
            ("nix/lib/default.nix", InputRole::Package),
            ("nix/lib/flake.nix", InputRole::Package),
        ]
    );
    assert_eq!(plan.external_packages(), ExternalPackages::PinnedByLockfile);

    let cargo_facts = GraphFacts {
        kind: GraphKind::Cargo,
        result: GraphResult::Available(BuildGraph::default()),
    };
    let wrong_kind = refused(plan_nix(&whole, &source, &cargo_facts, &nix_request()));
    assert_eq!(codes(&wrong_kind), BTreeSet::from(["layout_kind"]));
    let mut unknown = nix_request();
    unknown.targets = vec!["nix/gone".into()];
    let unknown = refused(plan_nix(&whole, &source, &nix_graph(), &unknown));
    assert_eq!(codes(&unknown), BTreeSet::from(["unknown_target"]));
}

// r[verify kiln.source.projection.route.scenario.refused]
#[test]
fn a_route_binds_only_the_build_plan_of_its_own_manifest() {
    use super::super::materialization::{
        MATERIALIZER_PROFILE_SCHEMA, MaterializerKind, MaterializerProfile, RouteRequest, plan,
        route,
    };
    let alpha = view(&ALPHA_VIEW);
    let build = ok(plan_alpha(&alpha, &glob_layout()));
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: component("kiln-eager-local", "1"),
        platform: TargetProfile::POSIX,
    };
    let materialized = ok(plan(&alpha, &profile));
    let pin = ok(materialized.pin(&digest("attempt"), 1));
    let admitted = ok(admit_definition(&definition(&ALPHA_VIEW, &[("", "")])));
    fn request(build_plan: &BuildInputPlan) -> RouteRequest<'_> {
        RouteRequest {
            store: "/var/lib/kiln/store",
            writable: &[],
            build_plan: Some(build_plan),
        }
    }
    let routed = ok(route(&admitted, &alpha, &materialized, &pin, request(&build)));
    assert_eq!(routed.build_plan.as_ref(), Some(build.identity()));
    assert!(routed.write_paths.is_empty());
    let whole = ok(plan_alpha(&view(&WORKSPACE), &glob_layout()));
    let crossed = refused(route(&admitted, &alpha, &materialized, &pin, request(&whole)));
    assert_eq!(codes(&crossed), BTreeSet::from(["route_build_plan_mismatch"]));
}
