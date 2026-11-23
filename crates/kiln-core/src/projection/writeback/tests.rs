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

/// The sealed candidate of a capture that found no fault and no conflict.
fn sealed(result: Result<Captured, Vec<Diagnostic>>) -> Result<SealedCandidate, Vec<Diagnostic>> {
    result.map(|captured| match captured {
        Captured::Sealed(candidate) => candidate,
        Captured::Conflict(conflict) => unreachable!("expected a sealed candidate: {conflict:?}"),
    })
}

fn declared_capture(
    view: &View,
    observed: &[ObservedFile],
    declared: &[Declaration],
) -> Result<Captured, Vec<Diagnostic>> {
    capture(
        &view.definition,
        &view.complete,
        &view.pin,
        observed,
        declared,
        CurrentAttempt {
            source: &view.base,
            fence: 1,
        },
    )
}

fn capture_view(
    view: &View,
    observed: &[ObservedFile],
) -> Result<SealedCandidate, Vec<Diagnostic>> {
    sealed(declared_capture(view, observed, &[]))
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

fn capture_at(
    view: &View,
    pin: &WorkspacePin,
    observed: &[ObservedFile],
    source: &ProjectionFacts,
    fence: u64,
) -> Result<Captured, Vec<Diagnostic>> {
    capture(
        &view.definition,
        &view.complete,
        pin,
        observed,
        &[],
        CurrentAttempt { source, fence },
    )
}

/// The base facts at another revision, with `changes` applied.
fn moved(changes: &[(&str, Option<&str>)]) -> ProjectionFacts {
    let mut source = facts("b");
    for (path, object) in changes {
        source.entries.retain(|entry| entry.path != *path);
        if let Some(object) = object {
            source.entries.push(SourceEntry {
                path: (*path).into(),
                object: blob(object),
                kind: EntryKind::File,
            });
        }
    }
    source.entries.sort_by(|left, right| left.path.cmp(&right.path));
    source
}

fn conflicted(result: Result<Captured, Vec<Diagnostic>>) -> WritebackConflict {
    match result {
        Ok(Captured::Conflict(conflict)) => conflict,
        other => unreachable!("expected a conflict, got {other:?}"),
    }
}

fn refused_codes(result: Result<Captured, Vec<Diagnostic>>) -> BTreeSet<String> {
    match result {
        Ok(captured) => unreachable!("expected a refusal, got {captured:?}"),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

// r[verify kiln.source.projection.writeback.scenario.stale]
#[test]
fn stale_and_superseded_captures_are_refused() {
    let view = view();
    let observed = untouched(&view);
    // A moved source is a conflict, never a sealed candidate; an untouched
    // overlay overlaps nothing.
    let conflict = conflicted(capture_at(&view, &view.pin, &observed, &moved(&[]), 1));
    assert!(conflict.overlapping().is_empty());
    assert_eq!(conflict.base(), &view.base.exact_revision);
    assert_eq!(conflict.current(), &facts("b").exact_revision);
    let mut foreign = view.base.clone();
    foreign.repository = digest("another repository");
    assert_eq!(
        refused_codes(capture_at(&view, &view.pin, &observed, &foreign, 1)),
        BTreeSet::from(["source_moved".to_string()])
    );
    assert_eq!(
        refused_codes(capture_at(&view, &view.pin, &observed, &view.base, 2)),
        BTreeSet::from(["superseded_attempt".to_string()])
    );
    let mut other = writable();
    other.id = "other-write".into();
    let other = view_for(&other, ProjectionOperation::Write);
    assert_eq!(
        refused_codes(capture_at(&view, &other.pin, &observed, &view.base, 1)),
        BTreeSet::from(["pin_mismatch".to_string()])
    );
    let crossed = View {
        definition: other.definition.clone(),
        complete: view.complete.clone(),
        pin: view.pin.clone(),
        base: view.base.clone(),
    };
    assert_eq!(
        refused_codes(capture_at(&crossed, &view.pin, &observed, &view.base, 1)),
        BTreeSet::from(["definition_mismatch".to_string()])
    );
    let build = view_for(&writable(), ProjectionOperation::Build);
    assert_eq!(
        refused_codes(capture_at(&build, &build.pin, &observed, &build.base, 1)),
        BTreeSet::from(["write_witness_required".to_string()])
    );
}

// r[verify kiln.source.projection.writeback.structural]
// r[verify kiln.source.projection.writeback.structural.scenario.distinct]
#[test]
fn renames_forks_aliases_and_membership_are_distinct_outcomes() {
    let view = view();
    let declared = |observed: &[ObservedFile], declarations: &[Declaration]| {
        ok(sealed(declared_capture(&view, observed, declarations)))
    };
    let rename = Declaration::Rename {
        from: "projects/alpha/src/lib.rs".into(),
        to: "projects/alpha/src/core.rs".into(),
    };
    let moved_file = with(
        without(untouched(&view), "projects/alpha/src/lib.rs"),
        "projects/alpha/src/core.rs",
        "alpha-lib",
    );
    let renamed = declared(&moved_file, core::slice::from_ref(&rename));
    assert_eq!(
        renamed.operations(),
        [SourceOperation::Rename {
            from: "projects/alpha/src/lib.rs".into(),
            to: "projects/alpha/src/core.rs".into(),
            destination: "projects/alpha/src/core.rs".into(),
            expected: blob("alpha-lib"),
            kind: EntryKind::File,
        }]
    );
    assert_eq!(
        renamed.changed_paths(),
        BTreeSet::from([
            "projects/alpha/src/core.rs".to_string(),
            "projects/alpha/src/lib.rs".to_string()
        ])
    );
    let rebuilt = ok(renamed.reconstruct(&view.base));
    assert!(rebuilt.iter().all(|entry| entry.path != "projects/alpha/src/lib.rs"));
    assert!(rebuilt.iter().any(|entry| {
        entry.path == "projects/alpha/src/core.rs" && entry.object == blob("alpha-lib")
    }));
    // Undeclared, the same overlay stays a delete and a create.
    let undeclared = declared(&moved_file, &[]);
    assert_eq!(
        undeclared
            .operations()
            .iter()
            .map(|operation| operation.label())
            .collect::<Vec<_>>(),
        ["create", "delete"]
    );
    assert_ne!(undeclared.identity(), renamed.identity());

    let forked_file = with(untouched(&view), "projects/alpha/src/copy.rs", "alpha-lib");
    let forked = declared(
        &forked_file,
        &[Declaration::Fork {
            from: "projects/alpha/src/lib.rs".into(),
            to: "projects/alpha/src/copy.rs".into(),
        }],
    );
    assert_eq!(
        forked.operations(),
        [SourceOperation::Fork {
            from: "projects/alpha/src/lib.rs".into(),
            to: "projects/alpha/src/copy.rs".into(),
            destination: "projects/alpha/src/copy.rs".into(),
            expected: blob("alpha-lib"),
            kind: EntryKind::File,
        }]
    );
    let rebuilt = ok(forked.reconstruct(&view.base));
    assert_eq!(
        rebuilt
            .iter()
            .filter(|entry| entry.object == blob("alpha-lib"))
            .count(),
        2
    );
    assert_ne!(declared(&forked_file, &[]).identity(), forked.identity());

    // An alias change moves one destination of the shared origin; the
    // source is untouched.
    let realiased = with(
        without(untouched(&view), "vendor/shared/proto/lib.rs"),
        "vendor/proto/lib.rs",
        "proto",
    );
    let aliased = declared(
        &realiased,
        &[Declaration::AliasChange {
            from: "vendor/shared/proto/lib.rs".into(),
            to: "vendor/proto/lib.rs".into(),
        }],
    );
    assert!(aliased.operations().is_empty());
    assert_eq!(
        aliased.view_operations(),
        [ViewDefinitionOperation::AliasChange {
            origin: "shared/proto/lib.rs".into(),
            from: "vendor/shared/proto/lib.rs".into(),
            to: "vendor/proto/lib.rs".into(),
        }]
    );
    assert_eq!(ok(aliased.reconstruct(&view.base)), view.base.entries);

    // Leaving the view is not a delete; joining names a source path the
    // view does not select. Neither touches the source.
    let left = without(untouched(&view), "projects/alpha/LICENSE");
    let membership = declared(
        &left,
        &[
            Declaration::MembershipChange {
                path: "projects/alpha/LICENSE".into(),
                member: false,
            },
            Declaration::MembershipChange {
                path: "projects/beta/src/lib.rs".into(),
                member: true,
            },
        ],
    );
    assert!(membership.operations().is_empty());
    assert_eq!(
        membership.view_operations(),
        [
            ViewDefinitionOperation::MembershipChange {
                path: "projects/alpha/LICENSE".into(),
                member: false,
            },
            ViewDefinitionOperation::MembershipChange {
                path: "projects/beta/src/lib.rs".into(),
                member: true,
            },
        ]
    );
    assert_eq!(ok(membership.reconstruct(&view.base)), view.base.entries);
    let deleted = declared(&left, &[]);
    assert_eq!(
        deleted.operations(),
        [SourceOperation::Delete {
            path: "projects/alpha/LICENSE".into(),
            expected: blob("license"),
        }]
    );
    assert_ne!(deleted.identity(), membership.identity());
}

// r[verify kiln.source.projection.writeback.structural]
#[test]
fn declarations_the_overlay_does_not_show_are_refused() {
    let view = view();
    let moved_file = with(
        without(untouched(&view), "projects/alpha/src/lib.rs"),
        "projects/alpha/src/core.rs",
        "alpha-lib",
    );
    let changed_file = with(
        without(untouched(&view), "projects/alpha/src/lib.rs"),
        "projects/alpha/src/core.rs",
        "alpha-lib-2",
    );
    let rename = |from: &str, to: &str| Declaration::Rename {
        from: from.into(),
        to: to.into(),
    };
    let cases: [(Vec<ObservedFile>, Vec<Declaration>, &str); 8] = [
        (
            changed_file,
            vec![rename("projects/alpha/src/lib.rs", "projects/alpha/src/core.rs")],
            "declaration_mismatch",
        ),
        (
            moved_file.clone(),
            vec![rename("projects/alpha/src/missing.rs", "projects/alpha/src/core.rs")],
            "declaration_invalid",
        ),
        (
            moved_file.clone(),
            vec![rename("projects/alpha/src/lib.rs", "projects/alpha/LICENSE")],
            "declaration_invalid",
        ),
        (
            moved_file.clone(),
            vec![
                rename("projects/alpha/src/lib.rs", "projects/alpha/src/core.rs"),
                Declaration::Fork {
                    from: "projects/alpha/src/lib.rs".into(),
                    to: "projects/alpha/src/core.rs".into(),
                },
            ],
            "declaration_invalid",
        ),
        (
            with(
                with(untouched(&view), "projects/alpha/src/lib.rs", "alpha-lib-2"),
                "projects/alpha/src/copy.rs",
                "alpha-lib",
            ),
            vec![Declaration::Fork {
                from: "projects/alpha/src/lib.rs".into(),
                to: "projects/alpha/src/copy.rs".into(),
            }],
            "declaration_mismatch",
        ),
        (
            with(
                without(untouched(&view), "projects/alpha/src/lib.rs"),
                "projects/alpha/moved.rs",
                "alpha-lib",
            ),
            vec![rename("projects/alpha/src/lib.rs", "projects/alpha/moved.rs")],
            "undeclared_new_file",
        ),
        (
            untouched(&view),
            vec![Declaration::MembershipChange {
                path: "projects/alpha/LICENSE".into(),
                member: false,
            }],
            "declaration_mismatch",
        ),
        (
            untouched(&view),
            vec![Declaration::MembershipChange {
                path: "projects/alpha/src/lib.rs".into(),
                member: true,
            }],
            "declaration_mismatch",
        ),
    ];
    for (observed, declarations, code) in cases {
        assert!(
            refused_codes(declared_capture(&view, &observed, &declarations)).contains(code),
            "{code}: {declarations:?}"
        );
    }
    // An alias change must leave the origin's other destinations unchanged.
    let both_moved = with(
        without(
            with(untouched(&view), "shared/proto/lib.rs", "proto-2"),
            "vendor/shared/proto/lib.rs",
        ),
        "vendor/proto/lib.rs",
        "proto",
    );
    assert!(
        refused_codes(declared_capture(
            &view,
            &both_moved,
            &[Declaration::AliasChange {
                from: "vendor/shared/proto/lib.rs".into(),
                to: "vendor/proto/lib.rs".into(),
            }],
        ))
        .contains("declaration_mismatch")
    );
    let bound: Vec<Declaration> = (0..=MAX_DECLARATIONS)
        .map(|index| Declaration::MembershipChange {
            path: format!("p{index}"),
            member: true,
        })
        .collect();
    assert!(
        refused_codes(declared_capture(&view, &untouched(&view), &bound))
            .contains("observation_bound")
    );
}

// r[verify kiln.source.projection.writeback.conflict]
#[test]
fn a_moved_source_yields_a_conflict_naming_overlapping_origins() {
    let view = view();
    let edited = with(
        with(untouched(&view), "shared/proto/lib.rs", "proto-a"),
        "vendor/shared/proto/lib.rs",
        "proto-a",
    );
    // Another writer changed an origin this attempt does not touch: still a
    // conflict, with nothing overlapping.
    let beside = conflicted(capture_at(
        &view,
        &view.pin,
        &edited,
        &moved(&[("projects/beta/src/lib.rs", Some("beta-b"))]),
        1,
    ));
    assert!(beside.overlapping().is_empty());
    // The same origin, a created path and a deleted expected object overlap.
    let created = with(edited.clone(), "projects/alpha/src/new.rs", "new-a");
    let created = without(created, "projects/alpha/LICENSE");
    let overlapping = conflicted(capture_at(
        &view,
        &view.pin,
        &created,
        &moved(&[
            ("shared/proto/lib.rs", Some("proto-b")),
            ("projects/alpha/src/new.rs", Some("new-b")),
            ("projects/alpha/LICENSE", None),
        ]),
        1,
    ));
    assert_eq!(
        overlapping.overlapping(),
        [
            "projects/alpha/LICENSE".to_string(),
            "projects/alpha/src/new.rs".to_string(),
            "shared/proto/lib.rs".to_string(),
        ]
    );
    // A rename overlaps on its source and on a target someone created.
    let renamed = with(
        without(untouched(&view), "projects/alpha/src/lib.rs"),
        "projects/alpha/src/core.rs",
        "alpha-lib",
    );
    let renaming = conflicted(capture(
        &view.definition,
        &view.complete,
        &view.pin,
        &renamed,
        &[Declaration::Rename {
            from: "projects/alpha/src/lib.rs".into(),
            to: "projects/alpha/src/core.rs".into(),
        }],
        CurrentAttempt {
            source: &moved(&[
                ("projects/alpha/src/lib.rs", Some("alpha-lib-b")),
                ("projects/alpha/src/core.rs", Some("core-b")),
            ]),
            fence: 1,
        },
    ));
    assert_eq!(
        renaming.overlapping(),
        [
            "projects/alpha/src/core.rs".to_string(),
            "projects/alpha/src/lib.rs".to_string(),
        ]
    );
}

// Archil reference fixtures (#79): ownership and observed content are
// independent checks, through different aliases of one origin.
// r[verify kiln.source.projection.writeback.conflict.scenario.archil]
#[test]
fn archil_ownership_and_observed_content_are_independent() {
    let view = view();
    let current = moved(&[("shared/proto/lib.rs", Some("proto-v2"))]);
    // A read origin v1; B published v2; A holds valid current ownership
    // (its fence is current) but its edit would overwrite B's change. The
    // edit is never sealed, whichever alias A wrote through.
    let through_both = with(
        with(untouched(&view), "shared/proto/lib.rs", "proto-a"),
        "vendor/shared/proto/lib.rs",
        "proto-a",
    );
    let conflict = conflicted(capture_at(&view, &view.pin, &through_both, &current, 1));
    // Normalized to the one canonical origin, not to either display path.
    assert_eq!(conflict.overlapping(), ["shared/proto/lib.rs".to_string()]);
    // An edit through one alias alone, the other still showing v1, cannot
    // bypass conflict detection: it is refused outright.
    let through_alias = with(untouched(&view), "vendor/shared/proto/lib.rs", "proto-a");
    assert_eq!(
        refused_codes(capture_at(&view, &view.pin, &through_alias, &current, 1)),
        BTreeSet::from(["contradictory_alias".to_string()])
    );
    // Conversely, an old attempt presenting exactly the current content
    // still fails its fence, through either alias and whether or not the
    // source moved.
    let current_bytes = with(
        with(untouched(&view), "shared/proto/lib.rs", "proto-v2"),
        "vendor/shared/proto/lib.rs",
        "proto-v2",
    );
    for source in [&view.base, &current] {
        assert_eq!(
            refused_codes(capture_at(&view, &view.pin, &current_bytes, source, 2)),
            BTreeSet::from(["superseded_attempt".to_string()])
        );
    }
    // A replacement attempt's pin at the current fence, but resolved at the
    // old base, still reports the overlap rather than sealing.
    let replacement = ok(ok(super::super::materialization::plan(
        &view.complete,
        &super::super::materialization::MaterializerProfile {
            schema: super::super::materialization::MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: super::super::materialization::MaterializerKind::EagerLocalDirectory,
            materializer: ComponentVersion {
                name: "kiln-eager-local".into(),
                version: "1".into(),
            },
            platform: TargetProfile::POSIX,
        },
    ))
    .pin(&digest("attempt-2"), 2));
    let conflict = conflicted(capture_at(&view, &replacement, &through_both, &current, 2));
    assert_eq!(conflict.overlapping(), ["shared/proto/lib.rs".to_string()]);
}

/// A v1 candidate recorded by the v1 capture at Kiln 90a9fb64 over this
/// fixture: an edit of `shared/proto/lib.rs`, a create and a delete.
const RECORDED_V1: &str = include_str!("fixtures/candidate-v1.json");

// r[verify kiln.source.projection.writeback.compatibility]
// r[verify kiln.source.projection.writeback.compatibility.scenario.v1]
#[test]
fn recorded_v1_and_v2_candidates_still_verify_read_only() {
    let recorded: RecordedCandidate = ok(serde_json::from_str(RECORDED_V1));
    assert_eq!(ok(verify_recorded(&recorded)), Version::V1);
    assert_eq!(recorded.schema, CANDIDATE_SCHEMA_V1);
    let mut tampered = recorded.clone();
    tampered.fence = 2;
    assert!(verify_recorded(&tampered).is_err());
    let mut widened = recorded.clone();
    widened.operations.push(SourceOperation::Fork {
        from: "projects/alpha/LICENSE".into(),
        to: "projects/alpha/src/LICENSE".into(),
        destination: "projects/alpha/src/LICENSE".into(),
        expected: blob("license"),
        kind: EntryKind::File,
    });
    assert!(
        verify_recorded(&widened)
            .is_err_and(|diagnostics| diagnostics.iter().any(|d| d.code == "candidate_schema"))
    );
    // The v2 capture of the same overlay records v2 and verifies as such; it
    // never verifies under the v1 schema.
    let view = view();
    let observed = untouched(&view);
    let observed = with(observed, "shared/proto/lib.rs", "proto-2");
    let observed = with(observed, "vendor/shared/proto/lib.rs", "proto-2");
    let observed = with(observed, "projects/alpha/src/new.rs", "new");
    let observed = without(observed, "projects/alpha/Cargo.toml");
    let candidate = ok(capture_view(&view, &observed));
    assert_eq!(
        candidate.operations().len(),
        recorded.operations.len(),
        "the fixture is this overlay"
    );
    assert_eq!(candidate.operations(), recorded.operations.as_slice());
    let v2: RecordedCandidate = ok(serde_json::from_slice(&ok(serde_json::to_vec(&candidate))));
    assert_eq!(ok(verify_recorded(&v2)), Version::V2);
    assert_eq!(&v2.identity, candidate.identity());
    assert_ne!(v2.identity, recorded.identity);
    let mut relabeled = v2;
    relabeled.schema = CANDIDATE_SCHEMA_V1.into();
    assert!(verify_recorded(&relabeled).is_err());
}

fn scope(id: &str, root: &str) -> crate::monorepo::ProjectInput {
    crate::monorepo::ProjectInput {
        project_id: id.into(),
        scope: Some(ok(crate::monorepo::admit_scope(
            id,
            &crate::monorepo::ProjectScope {
                root: root.into(),
                include: vec!["**".into()],
                exclude: Vec::new(),
                depends_on: Vec::new(),
                graphs: BTreeSet::from([crate::monorepo::GraphKind::Cargo]),
            },
        ))),
    }
}

fn graph() -> crate::monorepo::GraphFacts {
    use crate::monorepo::{BuildGraph, GraphFacts, GraphKind, GraphResult, Package};
    GraphFacts {
        kind: GraphKind::Cargo,
        result: GraphResult::Available(BuildGraph {
            packages: vec![
                Package {
                    id: "alpha".into(),
                    dir: "projects/alpha".into(),
                },
                Package {
                    id: "beta".into(),
                    dir: "projects/beta".into(),
                },
                Package {
                    id: "proto".into(),
                    dir: "shared/proto".into(),
                },
            ],
            edges: vec![
                ("projects/alpha".into(), "shared/proto".into()),
                ("projects/beta".into(), "shared/proto".into()),
            ],
            roots_touching_all: Vec::new(),
        }),
    }
}

// r[verify kiln.source.projection.candidate_facts]
// r[verify kiln.source.projection.candidate_facts.scenario.affected]
// r[verify kiln.source.projection.candidate_facts.scenario.applicability]
#[test]
fn candidate_facts_select_consumers_outside_the_view_and_bind_one_subject() {
    let view = view();
    let projects = [
        scope("alpha", "projects/alpha"),
        scope("beta", "projects/beta"),
    ];
    // A shared edit through the alpha view selects beta, which the view
    // does not hold.
    let shared = with(
        with(untouched(&view), "shared/proto/lib.rs", "proto-2"),
        "vendor/shared/proto/lib.rs",
        "proto-2",
    );
    let c1 = ok(capture_view(&view, &shared));
    let facts_c1 = ok(candidate_facts(&c1, None, &projects, &[graph()]));
    assert_eq!(
        facts_c1.affected_projects(),
        ["alpha".to_string(), "beta".to_string()]
    );
    assert_eq!(facts_c1.sealed(), c1.identity());
    assert!(facts_c1.build_plan().is_none());
    // An alpha-only edit selects alpha alone.
    let local = with(untouched(&view), "projects/alpha/src/lib.rs", "alpha-lib-2");
    let c2 = ok(capture_view(&view, &local));
    let facts_c2 = ok(candidate_facts(&c2, None, &projects, &[graph()]));
    assert_eq!(facts_c2.affected_projects(), ["alpha".to_string()]);
    // C1's retained evidence stays valid for C1 and cannot verify C2.
    assert!(facts_c1.accepts_evidence(facts_c1.identity()));
    assert!(!facts_c2.accepts_evidence(facts_c1.identity()));
    // Facts are reproducible; without graph facts every scoped project falls
    // back to affected, never to skipped.
    assert_eq!(
        ok(candidate_facts(&c1, None, &projects, &[graph()])).identity(),
        facts_c1.identity()
    );
    let without_graph = ok(candidate_facts(&c2, None, &projects, &[]));
    assert_eq!(
        without_graph.affected_projects(),
        ["alpha".to_string(), "beta".to_string()]
    );
    assert_ne!(without_graph.identity(), facts_c2.identity());
    // A view-only change affects nothing through the source.
    let realiased = with(
        without(untouched(&view), "vendor/shared/proto/lib.rs"),
        "vendor/proto/lib.rs",
        "proto",
    );
    let alias = ok(sealed(declared_capture(
        &view,
        &realiased,
        &[Declaration::AliasChange {
            from: "vendor/shared/proto/lib.rs".into(),
            to: "vendor/proto/lib.rs".into(),
        }],
    )));
    assert!(
        ok(candidate_facts(&alias, None, &projects, &[graph()]))
            .affected_projects()
            .is_empty()
    );
}
