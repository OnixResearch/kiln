use alloc::string::ToString;
use alloc::vec;

use super::super::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializerKind, MaterializerProfile, plan,
};
use super::super::{
    AdmittedRepository, ComponentVersion, DEFINITION_SCHEMA, GeneratedInputs, ProjectionDefinition,
    ProjectionProfile, ProviderBinding, RepositoryEnrollment, RepositoryLocator, Selection,
    SourceCapability, TargetProfile, admit_definition, admit_repositories, resolve,
};
use super::*;
use crate::types::ProviderProfileId;

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn codes(result: Result<SealedCandidate, Vec<Diagnostic>>) -> BTreeSet<String> {
    match result {
        Ok(candidate) => unreachable!("expected a refusal, captured {candidate:?}"),
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

fn blob(label: &str) -> ExternalRef {
    ok(ExternalRef::new("git", "blob", label))
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

/// Two projects with equal-byte licenses, a shared file and a file the
/// writable view excludes.
const SOURCE: [(&str, &str); 7] = [
    ("projects/alpha/Cargo.toml", "alpha-manifest"),
    ("projects/alpha/LICENSE", "license"),
    ("projects/alpha/src/hidden.rs", "hidden"),
    ("projects/alpha/src/lib.rs", "alpha-lib"),
    ("projects/beta/LICENSE", "license"),
    ("projects/beta/src/lib.rs", "beta-lib"),
    ("shared/proto/lib.rs", "proto"),
];

fn facts(revision: &str) -> ProjectionFacts {
    ProjectionFacts {
        repository: repository().identity().clone(),
        exact_revision: ok(ExternalRef::new("git", "commit", &revision.repeat(40))),
        canonical_tree: digest(revision),
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
            .map(|(path, object)| SourceEntry {
                path: (*path).into(),
                object: blob(object),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::new(),
        generated: Vec::new(),
    }
}

/// Alpha and the shared directory, which also appears under `vendor/shared`.
fn writable() -> ProjectionDefinition {
    ProjectionDefinition {
        schema: DEFINITION_SCHEMA.into(),
        id: "alpha-write".into(),
        profile: ProjectionProfile::Writable,
        selection: Selection {
            root: String::new(),
            include: vec!["projects/alpha/**".into(), "shared/**".into()],
            exclude: vec!["projects/alpha/src/hidden.rs".into()],
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
        creation_destinations: vec![
            "projects/alpha/src".into(),
            "vendor/shared".into(),
            "docs".into(),
        ],
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }
}

struct View {
    definition: AdmittedDefinition,
    complete: CompleteProjection,
    pin: WorkspacePin,
    base: ProjectionFacts,
}

fn view_for(definition: &ProjectionDefinition, operation: ProjectionOperation) -> View {
    let admitted = ok(admit_definition(definition));
    let base = facts("a");
    let complete = ok(ok(resolve(&admitted, &repository(), &base)).require_complete(operation));
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: ComponentVersion {
            name: "kiln-eager-local".into(),
            version: "1".into(),
        },
        platform: TargetProfile::POSIX,
    };
    let pin = ok(ok(plan(&complete, &profile)).pin(&digest("attempt"), 1));
    View {
        definition: admitted,
        complete,
        pin,
        base,
    }
}

fn view() -> View {
    view_for(&writable(), ProjectionOperation::Write)
}

/// The overlay exactly as materialized.
fn untouched(view: &View) -> Vec<ObservedFile> {
    view.complete
        .manifest()
        .entries
        .iter()
        .filter_map(|entry| match &entry.origin {
            EntryOrigin::Native { object, .. } => Some(ObservedFile {
                destination: entry.destination.clone(),
                kind: entry.kind,
                object: object.clone(),
            }),
            EntryOrigin::Generated { .. } => None,
        })
        .collect()
}

fn with(mut files: Vec<ObservedFile>, destination: &str, object: &str) -> Vec<ObservedFile> {
    files.retain(|file| file.destination != destination);
    files.push(ObservedFile {
        destination: destination.into(),
        kind: EntryKind::File,
        object: blob(object),
    });
    files
}

fn without(mut files: Vec<ObservedFile>, destination: &str) -> Vec<ObservedFile> {
    files.retain(|file| file.destination != destination);
    files
}

fn capture_view(
    view: &View,
    observed: &[ObservedFile],
) -> Result<SealedCandidate, Vec<Diagnostic>> {
    capture(
        &view.definition,
        &view.complete,
        &view.pin,
        observed,
        CurrentAttempt {
            source: &view.base,
            fence: 1,
        },
    )
}

#[test]
fn an_unchanged_overlay_captures_no_operations() {
    let view = view();
    let candidate = ok(capture_view(&view, &untouched(&view)));
    assert!(candidate.operations().is_empty());
    assert_eq!(ok(candidate.reconstruct(&view.base)), view.base.entries);
}

#[test]
fn edits_creates_and_deletes_become_typed_operations() {
    let view = view();
    let observed = untouched(&view);
    let observed = with(observed, "shared/proto/lib.rs", "proto-2");
    let observed = with(observed, "vendor/shared/proto/lib.rs", "proto-2");
    let observed = with(observed, "projects/alpha/src/new.rs", "new");
    let observed = without(observed, "projects/alpha/Cargo.toml");
    let candidate = ok(capture_view(&view, &observed));
    assert_eq!(
        candidate.operations(),
        [
            SourceOperation::Delete {
                path: "projects/alpha/Cargo.toml".into(),
                expected: blob("alpha-manifest"),
            },
            SourceOperation::Create {
                path: "projects/alpha/src/new.rs".into(),
                destination: "projects/alpha/src/new.rs".into(),
                object: blob("new"),
                kind: EntryKind::File,
            },
            SourceOperation::Edit {
                path: "shared/proto/lib.rs".into(),
                destination: "shared/proto/lib.rs".into(),
                expected: blob("proto"),
                object: blob("proto-2"),
                kind: EntryKind::File,
            },
        ]
    );
    // The full candidate keeps every other entry, the hidden one included.
    let rebuilt = ok(candidate.reconstruct(&view.base));
    let rebuilt: Vec<(&str, &str)> = rebuilt
        .iter()
        .map(|entry| (entry.path.as_str(), entry.object.native_id.as_str()))
        .collect();
    assert_eq!(
        rebuilt,
        [
            ("projects/alpha/LICENSE", "license"),
            ("projects/alpha/src/hidden.rs", "hidden"),
            ("projects/alpha/src/lib.rs", "alpha-lib"),
            ("projects/alpha/src/new.rs", "new"),
            ("projects/beta/LICENSE", "license"),
            ("projects/beta/src/lib.rs", "beta-lib"),
            ("shared/proto/lib.rs", "proto-2"),
        ]
    );
    // Capture is reproducible and follows the observation.
    assert_eq!(
        ok(capture_view(&view, &observed)).identity(),
        candidate.identity()
    );
    let later = with(observed, "projects/alpha/src/lib.rs", "alpha-lib-2");
    assert_ne!(
        ok(capture_view(&view, &later)).identity(),
        candidate.identity()
    );
    // Reconstruction needs the candidate's base.
    assert!(candidate.reconstruct(&facts("b")).is_err());
}

// r[verify kiln.source.projection.writeback.scenario.independent]
#[test]
fn equal_bytes_at_two_origins_stay_independent() {
    let view = view();
    let observed = with(untouched(&view), "projects/alpha/LICENSE", "license-2");
    let candidate = ok(capture_view(&view, &observed));
    assert_eq!(
        candidate
            .operations()
            .iter()
            .map(SourceOperation::path)
            .collect::<Vec<_>>(),
        ["projects/alpha/LICENSE"]
    );
    let rebuilt = ok(candidate.reconstruct(&view.base));
    assert!(
        rebuilt
            .iter()
            .any(|entry| entry.path == "projects/beta/LICENSE" && entry.object == blob("license"))
    );
}

// r[verify kiln.source.projection.writeback.scenario.refused]
#[test]
fn ambiguous_and_contradictory_edits_are_refused() {
    let view = view();
    let cases: [(Vec<ObservedFile>, &str); 5] = [
        (
            with(untouched(&view), "vendor/shared/proto/lib.rs", "proto-2"),
            "contradictory_alias",
        ),
        (
            with(untouched(&view), "projects/alpha/notes.txt", "notes"),
            "undeclared_new_file",
        ),
        (
            with(untouched(&view), "vendor/shared/new.txt", "new"),
            "ambiguous_new_file",
        ),
        (
            with(untouched(&view), "projects/alpha/src/hidden.rs", "replaced"),
            "create_over_hidden_source",
        ),
        (
            with(untouched(&view), "docs/new.md", "new"),
            "new_file_outside_selection",
        ),
    ];
    for (observed, code) in cases {
        assert_eq!(
            codes(capture_view(&view, &observed)),
            BTreeSet::from([code.to_string()]),
            "{code}"
        );
    }
    let mut linked = untouched(&view);
    linked.push(ObservedFile {
        destination: "projects/alpha/src/link.rs".into(),
        kind: EntryKind::Symlink,
        object: blob("link"),
    });
    assert_eq!(
        codes(capture_view(&view, &linked)),
        BTreeSet::from(["unsupported_entry".to_string()])
    );
    let mut twice = untouched(&view);
    twice.extend(untouched(&view).into_iter().take(1));
    assert_eq!(
        codes(capture_view(&view, &twice)),
        BTreeSet::from(["observation_invalid".to_string()])
    );
}

// r[verify kiln.source.projection.writeback.scenario.stale]
#[test]
fn stale_and_superseded_captures_are_refused() {
    let view = view();
    let observed = untouched(&view);
    let moved = facts("b");
    assert_eq!(
        codes(capture(
            &view.definition,
            &view.complete,
            &view.pin,
            &observed,
            CurrentAttempt {
                source: &moved,
                fence: 1,
            },
        )),
        BTreeSet::from(["source_moved".to_string()])
    );
    assert_eq!(
        codes(capture(
            &view.definition,
            &view.complete,
            &view.pin,
            &observed,
            CurrentAttempt {
                source: &view.base,
                fence: 2,
            },
        )),
        BTreeSet::from(["superseded_attempt".to_string()])
    );
    let mut other = writable();
    other.id = "other-write".into();
    let other = view_for(&other, ProjectionOperation::Write);
    assert_eq!(
        codes(capture(
            &view.definition,
            &view.complete,
            &other.pin,
            &observed,
            CurrentAttempt {
                source: &view.base,
                fence: 1,
            },
        )),
        BTreeSet::from(["pin_mismatch".to_string()])
    );
    assert_eq!(
        codes(capture(
            &other.definition,
            &view.complete,
            &view.pin,
            &observed,
            CurrentAttempt {
                source: &view.base,
                fence: 1,
            },
        )),
        BTreeSet::from(["definition_mismatch".to_string()])
    );
    let build = view_for(&writable(), ProjectionOperation::Build);
    assert_eq!(
        codes(capture(
            &build.definition,
            &build.complete,
            &build.pin,
            &observed,
            CurrentAttempt {
                source: &build.base,
                fence: 1,
            },
        )),
        BTreeSet::from(["write_witness_required".to_string()])
    );
}
