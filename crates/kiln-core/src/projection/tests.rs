use super::*;
use crate::monorepo::{ProjectScope, admit_scope};

/// `nickel export --format json config/fixtures/source-projection-positive.ncl`.
const EXPORTED_DEFINITION: &str =
    include_str!("../../../../fixtures/source-projection/definition.json");

type DefinitionChange = fn(&mut ProjectionDefinition);
type FactsChange = fn(&mut ProjectionFacts);

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn refused<T: core::fmt::Debug>(result: Result<T, Vec<Diagnostic>>) -> Vec<Diagnostic> {
    match result {
        Ok(value) => unreachable!("unexpectedly admitted: {value:?}"),
        Err(diagnostics) => diagnostics,
    }
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn has(diagnostics: &[Diagnostic], code: &str, field: &str) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code == code && d.field == field)
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

fn binding(native: &str) -> ProviderBinding {
    ProviderBinding {
        source_provider: ok(ProviderProfileId::new("git-source-v1")),
        native_repository: external("github", "repository", native),
    }
}

fn local(path: &str) -> RepositoryLocator {
    RepositoryLocator::LocalPath { path: path.into() }
}

fn enrollment(
    handle: &str,
    native: &str,
    locators: Vec<RepositoryLocator>,
) -> RepositoryEnrollment {
    RepositoryEnrollment {
        repository_id: handle.into(),
        binding: binding(native),
        locators,
    }
}

fn kiln_repository() -> AdmittedRepository {
    let enrolled = [enrollment(
        "kiln",
        "OnixResearch/kiln",
        vec![local("/srv/cache/kiln")],
    )];
    let mut admitted = ok(admit_repositories(&enrolled, &[]));
    admitted
        .pop()
        .unwrap_or_else(|| unreachable!("one admitted repository"))
}

fn exported() -> ProjectionDefinition {
    ok(serde_json::from_str(EXPORTED_DEFINITION))
}

fn navigation_docs() -> ProjectionDefinition {
    ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: "docs-browse".into(),
        profile: ProjectionProfile::Navigation,
        selection: Selection {
            root: String::new(),
            include: vec!["docs/**".into(), "scripts/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: Vec::new(),
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }
}

fn object(blob: &str) -> ExternalRef {
    external("git", "blob", blob)
}

fn entry(path: &str, blob: &str, kind: EntryKind) -> SourceEntry {
    SourceEntry {
        path: path.into(),
        object: object(blob),
        kind,
    }
}

fn generated(destination: &str) -> GeneratedEntry {
    GeneratedEntry {
        destination: destination.into(),
        kind: EntryKind::File,
        generator: component("codegen", "1"),
        input_identity: digest("codegen-input"),
    }
}

fn present(name: &str, label: &str) -> DependencyFact {
    DependencyFact {
        name: name.into(),
        status: DependencyStatus::Present {
            identity: digest(label),
        },
    }
}

/// One exact snapshot: the kiln-core crate (with two files sharing one blob),
/// an excluded build output, documentation and an executable script. The
/// dependency fact answers the exported definition's declared dependency.
fn facts(repository: &AdmittedRepository) -> ProjectionFacts {
    ProjectionFacts {
        repository: repository.identity().clone(),
        exact_revision: external("git", "commit", &"a".repeat(40)),
        canonical_tree: digest("tree-1"),
        evaluator: component("kiln-projection", "1"),
        adapter: component("git-source", "1"),
        source_capabilities: vec![
            SourceCapability::ExactTree,
            SourceCapability::ExecutableBit,
            SourceCapability::Symlinks,
        ],
        coverage: Coverage::Complete,
        entries: vec![
            entry("crates/kiln-core/Cargo.toml", "b1", EntryKind::File),
            entry("crates/kiln-core/src/lib.rs", "b2", EntryKind::File),
            entry("crates/kiln-core/src/copy.rs", "b2", EntryKind::File),
            entry("crates/kiln-core/target/debug/out", "b3", EntryKind::File),
            entry("docs/README.md", "b4", EntryKind::File),
            entry("scripts/run.sh", "b5", EntryKind::Executable),
        ],
        dependencies: vec![present("workspace-manifest", "workspace-manifest")],
        generated: Vec::new(),
    }
}

/// The same snapshot for a projection that declares no dependencies.
fn snapshot_without_dependencies(repository: &AdmittedRepository) -> ProjectionFacts {
    let mut snapshot = facts(repository);
    snapshot.dependencies.clear();
    snapshot
}

fn set_object(facts: &mut ProjectionFacts, path: &str, blob: &str) {
    for entry in facts.entries.iter_mut().filter(|entry| entry.path == path) {
        entry.object = object(blob);
    }
}

fn admitted_view(
    definition: &ProjectionDefinition,
    repository: &AdmittedRepository,
    facts: &ProjectionFacts,
) -> AdmittedPartialView {
    ok(resolve(
        &ok(admit_definition(definition)),
        repository,
        facts,
    ))
}

fn incomplete(view: &AdmittedPartialView, operation: ProjectionOperation) -> Vec<Diagnostic> {
    match view.clone().require_complete(operation) {
        Ok(complete) => unreachable!("unexpectedly complete for {:?}", complete.operation()),
        Err(refusal) => {
            assert_eq!(&refusal.view, view);
            refusal.diagnostics
        }
    }
}

// r[verify kiln.source.repository_identity]
// r[verify kiln.source.repository_identity.scenario.cache_move]
#[test]
fn repository_identity_survives_cache_moves_and_equal_names_stay_distinct() {
    let enrollments = [
        enrollment(
            "kiln",
            "OnixResearch/kiln",
            vec![local("/srv/cache-a/kiln")],
        ),
        enrollment(
            "kiln",
            "OnixResearch/kiln",
            vec![
                local("/home/ci/cache-b/kiln"),
                RepositoryLocator::Remote {
                    url: "https://github.com/OnixResearch/kiln.git".into(),
                },
            ],
        ),
        enrollment("fork", "someone/kiln", vec![local("/srv/cache-c/kiln")]),
    ];
    let admitted = ok(admit_repositories(&enrollments, &[]));
    let [fork, kiln] = admitted.as_slice() else {
        unreachable!("two repositories ordered by handle")
    };
    assert_eq!(kiln.repository_id(), "kiln");
    assert_eq!(kiln.identity(), &binding("OnixResearch/kiln").identity());
    assert_eq!(kiln.locators().len(), 3);
    assert!(kiln.superseded().is_empty());
    assert_ne!(fork.identity(), kiln.identity());

    let moved = ok(admit_repositories(
        &[enrollment(
            "kiln",
            "OnixResearch/kiln",
            vec![local("/var/lib/elsewhere/kiln")],
        )],
        &[],
    ));
    assert_eq!(
        moved.first().map(AdmittedRepository::identity),
        Some(kiln.identity())
    );
    let mut reversed = enrollments.to_vec();
    reversed.reverse();
    assert_eq!(ok(admit_repositories(&reversed, &[])), admitted);

    let relative = refused(admit_repositories(
        &[enrollment(
            "kiln",
            "OnixResearch/kiln",
            vec![local("cache/kiln")],
        )],
        &[],
    ));
    assert!(has(
        &relative,
        "invalid_locator",
        "repositories.kiln.locators"
    ));
}

// r[verify kiln.source.repository_identity.scenario.conflict]
#[test]
fn conflicting_provider_bindings_need_one_exact_reconciliation() {
    let github = enrollment("kiln", "OnixResearch/kiln", vec![local("/srv/github/kiln")]);
    let mut radicle = enrollment("kiln", "z2wsvXm5S2sJGvuV1k5JHiwi1PbKE", Vec::new());
    radicle.binding.native_repository =
        external("radicle", "repository", "z2wsvXm5S2sJGvuV1k5JHiwi1PbKE");
    radicle.locators = vec![local("/srv/radicle/kiln")];
    let enrollments = [github.clone(), radicle.clone()];
    assert_eq!(
        codes(&refused(admit_repositories(&enrollments, &[]))),
        ["repository_binding_conflict"]
    );

    let migration = BindingReconciliation {
        repository_id: "kiln".into(),
        retired: github.binding.clone(),
        current: radicle.binding.clone(),
    };
    let admitted = ok(admit_repositories(
        &enrollments,
        core::slice::from_ref(&migration),
    ));
    let [kiln] = admitted.as_slice() else {
        unreachable!("one reconciled repository")
    };
    assert_eq!(kiln.identity(), &radicle.binding.identity());
    assert_eq!(
        kiln.superseded(),
        &BTreeSet::from([github.binding.identity()])
    );
    assert_eq!(
        kiln.locators(),
        &BTreeSet::from([local("/srv/radicle/kiln")])
    );
    assert_eq!(
        ok(admit_repositories(
            &[radicle.clone(), github.clone()],
            core::slice::from_ref(&migration),
        )),
        admitted
    );

    let unrelated = BindingReconciliation {
        current: binding("someone/else"),
        ..migration.clone()
    };
    assert_eq!(
        codes(&refused(admit_repositories(&enrollments, &[unrelated]))),
        ["reconciliation_unmatched", "repository_binding_conflict"]
    );
    assert_eq!(
        codes(&refused(admit_repositories(
            core::slice::from_ref(&github),
            &[migration],
        ))),
        ["reconciliation_unmatched"]
    );

    let mirror = RepositoryEnrollment {
        repository_id: "kiln-mirror".into(),
        ..github.clone()
    };
    let alias = refused(admit_repositories(&[github, mirror], &[]));
    assert!(has(
        &alias,
        "repository_alias_conflict",
        "repositories.kiln,kiln-mirror"
    ));
}

// r[verify kiln.source.projection.definition]
// r[verify kiln.source.projection.definition.scenario.canonical]
#[test]
fn exported_definition_is_admitted_and_declaration_order_is_canonical() {
    let mut base = exported();
    let admitted = ok(admit_definition(&base));
    assert_eq!(admitted.id(), "kiln-core-build");
    assert_eq!(admitted.profile(), ProjectionProfile::Runnable);
    assert!(admitted.selects("crates/kiln-core/src/lib.rs"));
    assert!(!admitted.selects("crates/kiln-core/target/debug/out"));

    base.selection.exclude = vec!["target/**".into(), "**/*.orig".into()];
    let mut reordered = base.clone();
    reordered.selection.include = vec!["**".into(), "**".into()];
    reordered.selection.exclude.reverse();
    reordered.mounts.reverse();
    reordered.required_capabilities.reverse();
    reordered.dependencies.push(DependencyDeclaration {
        name: "workspace-manifest".into(),
        required: true,
    });
    let identity = |definition: &ProjectionDefinition| ok(admit_definition(definition)).identity;
    assert_eq!(identity(&reordered), identity(&base));

    let mut remounted = base.clone();
    if let Some(mount) = remounted.mounts.first_mut() {
        mount.destination_prefix = "core".into();
    }
    assert_ne!(identity(&remounted), identity(&base));
}

// r[verify kiln.source.projection.definition.scenario.profile_rules]
#[test]
fn profile_rules_refuse_implicit_capability() {
    let cases: [(DefinitionChange, &str, &str); 10] = [
        (
            |d| {
                d.profile = ProjectionProfile::Navigation;
                d.required_capabilities.clear();
                d.creation_destinations = vec!["notes".into()];
            },
            "creation_destination_profile",
            "creation_destinations",
        ),
        (
            |d| d.required_capabilities = vec![SourceCapability::ExecutableBit],
            "exact_tree_required",
            "required_capabilities",
        ),
        (
            |d| {
                d.profile = ProjectionProfile::Writable;
                d.generated_inputs = GeneratedInputs::Admitted;
            },
            "generated_inputs_profile",
            "generated_inputs",
        ),
        (
            |d| d.target.executable_bit = false,
            "target_capability_conflict",
            "required_capabilities.executable_bit",
        ),
        (
            |d| d.selection.root = "../outside".into(),
            "invalid_root",
            "selection.root",
        ),
        (|d| d.mounts.clear(), "mounts_required", "mounts"),
        (
            |d| {
                d.mounts.push(Mount {
                    source_prefix: "crates/*".into(),
                    destination_prefix: String::new(),
                });
            },
            "invalid_mount",
            "mounts.crates/*",
        ),
        (
            |d| {
                d.dependencies.push(DependencyDeclaration {
                    name: "workspace-manifest".into(),
                    required: false,
                });
            },
            "dependency_conflict",
            "dependencies.workspace-manifest",
        ),
        (
            |d| d.schema = "kiln.source-projection.definition.v0".into(),
            "unsupported_schema",
            "schema",
        ),
        (
            |d| d.target.max_path_bytes = 0,
            "bound_exceeded",
            "target.max_path_bytes",
        ),
    ];
    for (change, code, field) in cases {
        let mut definition = exported();
        change(&mut definition);
        let diagnostics = refused(admit_definition(&definition));
        assert!(
            has(&diagnostics, code, field),
            "{code} {field}: {diagnostics:?}"
        );
    }
    let mut writable = exported();
    writable.profile = ProjectionProfile::Writable;
    writable.creation_destinations = vec!["kiln-core/src/new".into()];
    ok(admit_definition(&writable));
}

// r[verify kiln.source.projection.manifest]
// r[verify kiln.source.projection.manifest.scenario.reproducible]
#[test]
fn manifests_are_reproducible_and_aliases_share_one_origin() {
    let repository = kiln_repository();
    let build = ok(admit_definition(&exported()));
    let docs = ok(admit_definition(&navigation_docs()));
    let forward = facts(&repository);
    let docs_facts = snapshot_without_dependencies(&repository);
    for (definition, snapshot) in [(&build, &forward), (&docs, &docs_facts)] {
        let mut reversed = snapshot.clone();
        reversed.entries.reverse();
        assert_eq!(
            ok(resolve(definition, &repository, snapshot)),
            ok(resolve(definition, &repository, &reversed))
        );
    }

    let view = ok(resolve(&build, &repository, &forward));
    let manifest = view.manifest();
    let destinations: Vec<&str> = manifest
        .entries
        .iter()
        .map(|entry| entry.destination.as_str())
        .collect();
    assert_eq!(
        destinations,
        [
            "kiln-core/Cargo.toml",
            "kiln-core/src/copy.rs",
            "kiln-core/src/lib.rs",
            "manifests/kiln-core.toml",
        ]
    );
    let origin = |destination: &str| {
        manifest
            .entries
            .iter()
            .find(|entry| entry.destination == destination)
            .map(|entry| entry.origin.clone())
            .unwrap_or_else(|| unreachable!("destination {destination}"))
    };
    assert_eq!(
        origin("kiln-core/Cargo.toml"),
        origin("manifests/kiln-core.toml")
    );
    let (
        EntryOrigin::Native {
            object: lib_object,
            origin_identity: lib,
            ..
        },
        EntryOrigin::Native {
            object: copy_object,
            origin_identity: copy,
            ..
        },
    ) = (
        origin("kiln-core/src/lib.rs"),
        origin("kiln-core/src/copy.rs"),
    )
    else {
        unreachable!("native origins")
    };
    assert_eq!(lib_object, copy_object);
    assert_ne!(lib, copy);

    let docs_view = ok(resolve(&docs, &repository, &docs_facts));
    assert_eq!(docs_view.manifest().exact_revision, manifest.exact_revision);
    assert_eq!(docs_view.manifest().entries.len(), 2);
    assert_ne!(docs_view.manifest().identity, manifest.identity);
}

// r[verify kiln.source.projection.manifest.scenario.identity]
#[test]
fn identities_follow_relevant_inputs() {
    let repository = kiln_repository();
    let definition = ok(admit_definition(&exported()));
    let base_facts = facts(&repository);
    let resolved = |definition: &AdmittedDefinition, facts: &ProjectionFacts| {
        ok(resolve(definition, &repository, facts))
            .manifest()
            .clone()
    };
    let base = resolved(&definition, &base_facts);
    let cases: [(&str, FactsChange, bool, bool); 6] = [
        (
            "selected content",
            |f| set_object(f, "crates/kiln-core/src/lib.rs", "b9"),
            true,
            true,
        ),
        (
            "revision",
            |f| f.exact_revision = external("git", "commit", &"b".repeat(40)),
            true,
            false,
        ),
        (
            "evaluator",
            |f| f.evaluator.version = "2".into(),
            true,
            false,
        ),
        ("adapter", |f| f.adapter.version = "2".into(), true, false),
        (
            "dependency",
            |f| f.dependencies = vec![present("workspace-manifest", "workspace-manifest-2")],
            true,
            true,
        ),
        (
            "unselected content",
            |f| set_object(f, "docs/README.md", "b9"),
            false,
            false,
        ),
    ];
    for (label, change, manifest_changes, content_changes) in cases {
        let mut changed = base_facts.clone();
        change(&mut changed);
        let manifest = resolved(&definition, &changed);
        assert_eq!(
            manifest.identity != base.identity,
            manifest_changes,
            "{label}"
        );
        assert_eq!(
            manifest.content_identity != base.content_identity,
            content_changes,
            "{label}"
        );
    }

    let mut layout = exported();
    if let Some(mount) = layout.mounts.first_mut() {
        mount.destination_prefix = "core".into();
    }
    let remounted = ok(admit_definition(&layout));
    assert_ne!(remounted.identity(), definition.identity());
    let manifest = resolved(&remounted, &base_facts);
    assert_ne!(manifest.identity, base.identity);
    assert_ne!(manifest.content_identity, base.content_identity);
}

// r[verify kiln.source.projection.manifest.scenario.diagnostics]
#[test]
fn diagnostics_are_sorted_and_independent_of_input_order() {
    let repository = kiln_repository();
    let mut writable = exported();
    writable.profile = ProjectionProfile::Writable;
    writable.target.case_sensitive = false;
    writable.target.symlinks = false;
    writable.creation_destinations = vec!["kiln-core/Cargo.toml/new".into()];
    writable.mounts.push(Mount {
        source_prefix: "crates/kiln-core/src/lib.rs".into(),
        destination_prefix: "kiln-core/Cargo.toml".into(),
    });
    let definition = ok(admit_definition(&writable));
    let mut faulty = facts(&repository);
    faulty
        .source_capabilities
        .retain(|capability| *capability != SourceCapability::ExecutableBit);
    faulty.entries.extend([
        entry("crates/kiln-core/../escape", "b6", EntryKind::File),
        entry("crates/kiln-core/src/lib.rs", "b2", EntryKind::File),
        entry("crates/kiln-core/README.md", "b7", EntryKind::File),
        entry("crates/kiln-core/readme.md", "b8", EntryKind::File),
        entry("crates/kiln-core/assets/link", "b9", EntryKind::Symlink),
    ]);
    faulty
        .generated
        .push(generated("kiln-core/src/lib.rs/generated"));
    faulty.dependencies.push(DependencyFact {
        name: "undeclared".into(),
        status: DependencyStatus::Missing,
    });
    let diagnostics = refused(resolve(&definition, &repository, &faulty));
    let mut reversed = faulty.clone();
    reversed.entries.reverse();
    reversed.generated.reverse();
    reversed.dependencies.reverse();
    assert_eq!(
        refused(resolve(&definition, &repository, &reversed)),
        diagnostics
    );
    assert!(
        diagnostics
            .windows(2)
            .all(|pair| matches!(pair, [a, b] if (&a.code, &a.field) <= (&b.code, &b.field)))
    );
    for (code, field) in [
        ("case_fold_collision", "destinations.kiln-core/readme.md"),
        (
            "creation_destination_collision",
            "creation_destinations.kiln-core/Cargo.toml/new",
        ),
        ("destination_collision", "destinations.kiln-core/Cargo.toml"),
        (
            "duplicate_source_path",
            "entries.crates/kiln-core/src/lib.rs",
        ),
        (
            "file_directory_collision",
            "destinations.kiln-core/src/lib.rs",
        ),
        ("undeclared_dependency_fact", "dependencies.undeclared"),
        ("unsafe_source_path", "entries.crates/kiln-core/../escape"),
        (
            "unsupported_source_capability",
            "required_capabilities.executable_bit",
        ),
        (
            "unsupported_target_capability",
            "entries.crates/kiln-core/assets/link",
        ),
    ] {
        assert!(
            has(&diagnostics, code, field),
            "{code} {field}: {diagnostics:?}"
        );
    }

    let build = ok(admit_definition(&exported()));
    let mut other = facts(&repository);
    other.repository = digest("another-repository");
    assert!(has(
        &refused(resolve(&build, &repository, &other)),
        "stale_repository",
        "repository"
    ));
    let mut oversized = facts(&repository);
    oversized.entries = (0..=MAX_SOURCE_ENTRIES)
        .map(|index| entry(&format!("crates/kiln-core/f{index}"), "b1", EntryKind::File))
        .collect();
    assert_eq!(
        codes(&refused(resolve(&build, &repository, &oversized))),
        ["bound_exceeded"]
    );
}

// r[verify kiln.source.projection.completeness]
// r[verify kiln.source.projection.completeness.scenario.partial]
#[test]
fn partial_views_cannot_become_complete_workspaces() {
    let repository = kiln_repository();
    let base = facts(&repository);
    let runnable = exported();
    let mut writable = exported();
    writable.profile = ProjectionProfile::Writable;
    writable.creation_destinations = vec!["kiln-core/src/new".into()];

    let navigation = admitted_view(
        &navigation_docs(),
        &repository,
        &snapshot_without_dependencies(&repository),
    );
    assert!(has(
        &incomplete(&navigation, ProjectionOperation::Build),
        "operation_not_permitted",
        "profile"
    ));
    let build_only = admitted_view(&runnable, &repository, &base);
    assert!(has(
        &incomplete(&build_only, ProjectionOperation::Write),
        "operation_not_permitted",
        "profile"
    ));

    let cases: [(FactsChange, &str, &str); 4] = [
        (
            |f| f.coverage = Coverage::Truncated,
            "incomplete_coverage",
            "coverage",
        ),
        (
            |f| f.dependencies.clear(),
            "required_dependency_missing",
            "dependencies.workspace-manifest",
        ),
        (
            |f| {
                f.dependencies = vec![DependencyFact {
                    name: "workspace-manifest".into(),
                    status: DependencyStatus::Denied,
                }];
            },
            "required_dependency_denied",
            "dependencies.workspace-manifest",
        ),
        (
            |f| f.generated.push(generated("kiln-core/generated/build.rs")),
            "generated_input_not_admitted",
            "destinations.kiln-core/generated/build.rs",
        ),
    ];
    for (change, code, field) in cases {
        let mut changed = base.clone();
        change(&mut changed);
        let view = admitted_view(&runnable, &repository, &changed);
        assert!(
            has(&incomplete(&view, ProjectionOperation::Build), code, field),
            "{code}"
        );
    }
    let mut with_generated = base.clone();
    with_generated
        .generated
        .push(generated("kiln-core/generated/build.rs"));
    let generated_writable = admitted_view(&writable, &repository, &with_generated);
    assert!(has(
        &incomplete(&generated_writable, ProjectionOperation::Write),
        "generated_entry_not_writable",
        "destinations.kiln-core/generated/build.rs"
    ));

    let mut admits_generated = exported();
    admits_generated.generated_inputs = GeneratedInputs::Admitted;
    let explicit = admitted_view(&admits_generated, &repository, &with_generated);
    ok(explicit.require_complete(ProjectionOperation::Build));
    let complete = ok(build_only
        .clone()
        .require_complete(ProjectionOperation::Build));
    assert_eq!(complete.operation(), ProjectionOperation::Build);
    assert_eq!(complete.view(), &build_only);
    let write = admitted_view(&writable, &repository, &base);
    let complete = ok(write.require_complete(ProjectionOperation::Write));
    assert_eq!(
        complete.view().creation_destinations(),
        &BTreeSet::from(["kiln-core/src/new".to_string()])
    );
}

// r[verify kiln.source.projection.completeness.scenario.forged]
#[test]
fn forged_and_stale_manifests_are_reported_and_never_admitted() {
    let repository = kiln_repository();
    let definition = ok(admit_definition(&exported()));
    let mut truncated = facts(&repository);
    truncated.coverage = Coverage::Truncated;
    let view = ok(resolve(&definition, &repository, &truncated));
    let manifest = view.manifest().clone();
    let expected = ExpectedProjection {
        repository: repository.identity(),
        exact_revision: &truncated.exact_revision,
        definition: definition.identity(),
        evaluator: &truncated.evaluator,
        adapter: &truncated.adapter,
    };
    ok(verify_manifest(&manifest, expected));

    let mut wire = ok(serde_json::to_value(&manifest));
    wire.as_object_mut()
        .and_then(|fields| fields.insert("coverage".into(), "complete".into()))
        .unwrap_or_else(|| unreachable!("manifest has a coverage field"));
    let forged: ResolvedProjectionManifest = ok(serde_json::from_value(wire));
    assert_eq!(forged.coverage, Coverage::Complete);
    assert_eq!(
        codes(&refused(verify_manifest(&forged, expected))),
        ["manifest_identity_mismatch"]
    );

    let mut swapped = manifest.clone();
    if let Some(ManifestEntry {
        origin: EntryOrigin::Native { object, .. },
        ..
    }) = swapped.entries.first_mut()
    {
        *object = external("git", "blob", "forged");
    }
    assert!(
        codes(&refused(verify_manifest(&swapped, expected))).contains(&"origin_identity_mismatch")
    );

    let newer = external("git", "commit", &"c".repeat(40));
    let evaluator = component("kiln-projection", "2");
    let other_definition = digest("other-definition");
    let stale = refused(verify_manifest(
        &manifest,
        ExpectedProjection {
            exact_revision: &newer,
            definition: &other_definition,
            evaluator: &evaluator,
            ..expected
        },
    ));
    assert_eq!(
        codes(&stale),
        ["stale_definition", "stale_evaluator", "stale_revision"]
    );
    assert!(has(
        &incomplete(&view, ProjectionOperation::Build),
        "incomplete_coverage",
        "coverage"
    ));
}

// r[verify kiln.source.projection.compatibility]
// r[verify kiln.source.projection.compatibility.scenario.scope]
#[test]
fn converted_scope_selects_exactly_the_scope_files() {
    let scope = ok(admit_scope(
        "api",
        &ProjectScope {
            root: "crates/api".into(),
            include: vec!["**/*.rs".into(), "Cargo.toml".into()],
            exclude: vec!["src/generated/**".into()],
            depends_on: vec!["Cargo.lock".into()],
            graphs: BTreeSet::new(),
        },
    ));
    let definition = ProjectionDefinition::from_project_scope(&scope, ProjectionProfile::Runnable);
    let admitted = ok(admit_definition(&definition));
    assert_eq!(
        admitted.compatibility(),
        Some(&Compatibility {
            profile: CompatibilityProfile::MonorepoProjectScopeV1,
            scope_identity: scope.identity().clone(),
        })
    );
    let navigation =
        ProjectionDefinition::from_project_scope(&scope, ProjectionProfile::Navigation);
    assert!(navigation.required_capabilities.is_empty());
    ok(admit_definition(&navigation));

    let repository = kiln_repository();
    let paths = [
        "Cargo.lock",
        "crates/api/Cargo.toml",
        "crates/api/README.md",
        "crates/api/src/lib.rs",
        "crates/api/src/generated/schema.rs",
        "crates/api-extra/src/lib.rs",
        "crates/web/src/main.rs",
    ];
    let mut observed = facts(&repository);
    observed.dependencies.clear();
    observed.entries = paths
        .iter()
        .map(|path| entry(path, path, EntryKind::File))
        .collect();
    let view = ok(resolve(&admitted, &repository, &observed));
    let projected: Vec<&str> = view
        .manifest()
        .entries
        .iter()
        .map(|entry| entry.destination.as_str())
        .collect();
    let in_scope: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|path| scope.contains(path))
        .collect();
    assert_eq!(projected, in_scope);
    assert_eq!(
        projected,
        ["crates/api/Cargo.toml", "crates/api/src/lib.rs"]
    );
}
