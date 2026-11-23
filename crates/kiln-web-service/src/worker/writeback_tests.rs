use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::DirBuilderExt;

use git2::build::TreeUpdateBuilder;
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializationPlan, MaterializerKind, MaterializerProfile,
    WorkspacePin, plan,
};
use kiln_core::projection::writeback::{Captured, CurrentAttempt, Declaration, capture};
use kiln_core::projection::{
    AdmittedDefinition, AdmittedRepository, CompleteProjection, ComponentVersion, Coverage,
    EntryOrigin, GeneratedInputs, Mount, ProjectionDefinition, ProjectionOperation,
    ProjectionProfile, ProviderBinding, RepositoryEnrollment, RepositoryLocator, Selection,
    SourceCapability, TargetProfile, admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ContentId, ProviderProfileId};

use crate::worker::materialize::{prepare_input, prepare_overlay};

const PRIVATE_DIRECTORY: u32 = 0o700;
const SOURCE: [(&str, &[u8]); 7] = [
    (
        "projects/alpha/Cargo.toml",
        b"[package]\nname = \"alpha\"\n",
    ),
    ("projects/alpha/LICENSE", b"MIT OR Apache-2.0\n"),
    (
        "projects/alpha/src/hidden.rs",
        b"// excluded from the view\n",
    ),
    ("projects/alpha/src/lib.rs", b"pub fn alpha() {}\n"),
    ("projects/beta/LICENSE", b"MIT OR Apache-2.0\n"),
    ("projects/beta/src/lib.rs", b"pub fn beta() {}\n"),
    ("shared/proto/lib.rs", b"pub const VERSION: u32 = 1;\n"),
];
const RUN: (&str, &[u8]) = ("projects/alpha/run.sh", b"#!/bin/sh\nexit 0\n");

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn private(path: PathBuf) -> PathBuf {
    ok(fs::DirBuilder::new().mode(PRIVATE_DIRECTORY).create(&path));
    path
}

fn repository_identity() -> AdmittedRepository {
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

/// Facts of one commit, read from its tree.
fn facts_of(repository: &Repository, commit: Oid) -> ProjectionFacts {
    let tree = ok(ok(repository.find_commit(commit)).tree());
    ProjectionFacts {
        repository: repository_identity().identity().clone(),
        exact_revision: ok(ExternalRef::new("git", "commit", &commit.to_string())),
        canonical_tree: digest(&tree.id().to_string()),
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
        entries: ok(listing(&tree)),
        dependencies: Vec::new(),
        generated: Vec::new(),
    }
}

fn definition(id: &str, profile: ProjectionProfile, include: &[&str]) -> ProjectionDefinition {
    ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: id.into(),
        profile,
        selection: Selection {
            root: String::new(),
            include: include
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect(),
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
        creation_destinations: if profile == ProjectionProfile::Writable {
            vec!["projects/alpha/src".into()]
        } else {
            Vec::new()
        },
        required_capabilities: vec![SourceCapability::ExactTree, SourceCapability::ExecutableBit],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    repository: Repository,
    base: Oid,
    facts: ProjectionFacts,
    definition: AdmittedDefinition,
    complete: CompleteProjection,
    plan: MaterializationPlan,
    store: PathBuf,
}

/// One commit of the source at a fixed time and the writable alpha view of it.
fn fixture() -> Fixture {
    let root = ok(tempfile::tempdir());
    let repository = ok(Repository::init_bare(root.path().join("source.git")));
    let base = {
        let mut update = TreeUpdateBuilder::new();
        for (path, bytes) in SOURCE {
            update.upsert(path, ok(repository.blob(bytes)), FileMode::Blob);
        }
        update.upsert(RUN.0, ok(repository.blob(RUN.1)), FileMode::BlobExecutable);
        let empty = ok(repository.find_tree(ok(ok(repository.treebuilder(None)).write())));
        let tree = ok(repository.find_tree(ok(update.create_updated(&repository, &empty))));
        let signature = ok(Signature::new(
            "Fixture",
            "fixture@example.invalid",
            &Time::new(0, 0),
        ));
        ok(repository.commit(None, &signature, &signature, "base", &tree, &[]))
    };
    let facts = facts_of(&repository, base);
    let definition = ok(admit_definition(&definition(
        "alpha-write",
        ProjectionProfile::Writable,
        &["projects/alpha/**", "shared/**"],
    )));
    let complete = ok(ok(resolve(&definition, &repository_identity(), &facts))
        .require_complete(ProjectionOperation::Write));
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
    let store = private(root.path().join("store"));
    ok(prepare_input(&store, &plan, &repository));
    Fixture {
        _root: root,
        repository,
        base,
        facts,
        definition,
        complete,
        plan,
        store,
    }
}

impl Fixture {
    /// A pinned overlay for one attempt at fence 1.
    fn overlay(&self, attempt: &str) -> (WorkspacePin, PathBuf) {
        let pin = ok(self.plan.pin(&digest(attempt), 1));
        let overlay = ok(prepare_overlay(&self.store, &self.plan, &pin));
        (pin, overlay)
    }

    fn capture(&self, pin: &WorkspacePin, overlay: &Path) -> SealedCandidate {
        self.declared(pin, overlay, &[])
    }

    fn declared(
        &self,
        pin: &WorkspacePin,
        overlay: &Path,
        declared: &[Declaration],
    ) -> SealedCandidate {
        match ok(capture(
            &self.definition,
            &self.complete,
            pin,
            &ok(observe(overlay)),
            declared,
            CurrentAttempt {
                source: &self.facts,
                fence: 1,
            },
        )) {
            Captured::Sealed(candidate) => candidate,
            Captured::Conflict(conflict) => unreachable!("unexpected conflict {conflict:?}"),
        }
    }
}

/// Path to (object, mode) of every blob in a commit's tree.
fn tree_of(repository: &Repository, commit: Oid) -> BTreeMap<String, (Oid, i32)> {
    let tree = ok(ok(repository.find_commit(commit)).tree());
    let mut found = BTreeMap::new();
    ok(tree.walk(TreeWalkMode::PreOrder, |root, entry| {
        if entry.kind() == Some(ObjectType::Blob) {
            found.insert(
                format!("{root}{}", entry.name().unwrap_or_default()),
                (entry.id(), entry.filemode()),
            );
        }
        TreeWalkResult::Ok
    }));
    found
}

// r[verify kiln.source.projection.writeback.scenario.round_trip]
#[test]
fn a_shared_edit_round_trips_into_one_source_update() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-one");
    let (_, other) = fixture.overlay("attempt-two");
    let edited = b"pub const VERSION: u32 = 2;\n";
    for alias in ["shared/proto/lib.rs", "vendor/shared/proto/lib.rs"] {
        ok(fs::write(overlay.join(alias), edited));
    }
    ok(fs::write(
        overlay.join("projects/alpha/src/new.rs"),
        b"pub fn new() {}\n",
    ));
    ok(fs::remove_file(overlay.join("projects/alpha/Cargo.toml")));
    let candidate = fixture.capture(&pin, &overlay);
    let paths: Vec<&str> = candidate
        .operations()
        .iter()
        .map(SourceOperation::path)
        .collect();
    assert_eq!(
        paths,
        [
            "projects/alpha/Cargo.toml",
            "projects/alpha/src/new.rs",
            "shared/proto/lib.rs"
        ]
    );
    let sealed = ok(seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
    ));
    let commit = ok(fixture.repository.find_commit(sealed));
    assert_eq!(commit.parent_ids().collect::<Vec<_>>(), [fixture.base]);
    // No reference moved.
    assert!(ok(fixture.repository.references()).next().is_none());

    let before = tree_of(&fixture.repository, fixture.base);
    let after = tree_of(&fixture.repository, sealed);
    for (path, entry) in &before {
        if !paths.contains(&path.as_str()) {
            assert_eq!(after.get(path), Some(entry), "{path} changed");
        }
    }
    assert!(!after.contains_key("projects/alpha/Cargo.toml"));
    assert_eq!(
        after.get("shared/proto/lib.rs").map(|(oid, _)| *oid),
        Some(ok(Oid::hash_object(ObjectType::Blob, edited)))
    );
    assert!(after.contains_key("projects/alpha/src/new.rs"));
    assert_eq!(after.len(), before.len());

    // A view resolved at the candidate sees the new version...
    let refreshed = facts_of(&fixture.repository, sealed);
    let beta = ok(admit_definition(&definition(
        "beta-read",
        ProjectionProfile::Runnable,
        &["projects/beta/**", "shared/**"],
    )));
    let beta = ok(resolve(&beta, &repository_identity(), &refreshed));
    let shared = beta
        .manifest()
        .entries
        .iter()
        .find(|entry| entry.destination == "shared/proto/lib.rs")
        .map(|entry| entry.origin.clone());
    assert!(matches!(
        shared,
        Some(EntryOrigin::Native { object, .. })
            if object.native_id.as_str() == ok(Oid::hash_object(ObjectType::Blob, edited)).to_string()
    ));
    // ...while another attempt's pinned overlay keeps the old one.
    assert_eq!(
        ok(fs::read(other.join("shared/proto/lib.rs"))),
        b"pub const VERSION: u32 = 1;\n"
    );
}

// r[verify kiln.source.projection.writeback.scenario.sealed]
#[test]
fn a_sealed_candidate_is_immutable() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-one");
    for alias in ["shared/proto/lib.rs", "vendor/shared/proto/lib.rs"] {
        ok(fs::write(
            overlay.join(alias),
            b"pub const VERSION: u32 = 2;\n",
        ));
    }
    let first = fixture.capture(&pin, &overlay);
    let sealed = ok(seal(&fixture.repository, &overlay, &fixture.facts, &first));
    let sealed_tree = ok(fixture.repository.find_commit(sealed)).tree_id();

    for alias in ["shared/proto/lib.rs", "vendor/shared/proto/lib.rs"] {
        ok(fs::write(
            overlay.join(alias),
            b"pub const VERSION: u32 = 3;\n",
        ));
    }
    let error = seal(&fixture.repository, &overlay, &fixture.facts, &first)
        .err()
        .unwrap_or_default();
    assert!(error.contains("changed after capture"), "{error}");
    let second = fixture.capture(&pin, &overlay);
    assert_ne!(second.identity(), first.identity());
    assert_eq!(
        ok(fixture.repository.find_commit(sealed)).tree_id(),
        sealed_tree
    );
}

// r[verify kiln.source.projection.writeback.structural]
#[test]
fn declared_renames_and_forks_seal_exactly_their_paths() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-one");
    let lib = overlay.join("projects/alpha/src/lib.rs");
    let bytes = ok(fs::read(&lib));
    ok(fs::copy(&lib, overlay.join("projects/alpha/src/copy.rs")));
    ok(fs::rename(
        overlay.join("projects/alpha/LICENSE"),
        overlay.join("projects/alpha/src/LICENSE"),
    ));
    let candidate = fixture.declared(
        &pin,
        &overlay,
        &[
            Declaration::Fork {
                from: "projects/alpha/src/lib.rs".into(),
                to: "projects/alpha/src/copy.rs".into(),
            },
            Declaration::Rename {
                from: "projects/alpha/LICENSE".into(),
                to: "projects/alpha/src/LICENSE".into(),
            },
        ],
    );
    let labels: Vec<&str> = candidate
        .operations()
        .iter()
        .map(|operation| match operation {
            SourceOperation::Rename { .. } => "rename",
            SourceOperation::Fork { .. } => "fork",
            _ => "other",
        })
        .collect();
    assert_eq!(labels, ["rename", "fork"]);
    let sealed = ok(seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
    ));
    let before = tree_of(&fixture.repository, fixture.base);
    let after = tree_of(&fixture.repository, sealed);
    assert!(!after.contains_key("projects/alpha/LICENSE"));
    assert_eq!(
        after.get("projects/alpha/src/LICENSE"),
        before.get("projects/alpha/LICENSE")
    );
    assert_eq!(
        after.get("projects/alpha/src/copy.rs").map(|(oid, _)| *oid),
        Some(ok(Oid::hash_object(ObjectType::Blob, &bytes)))
    );
    assert_eq!(after.len(), before.len() + 1);
    // Changed bytes after capture are refused, as for edits.
    ok(fs::write(
        overlay.join("projects/alpha/src/copy.rs"),
        b"changed\n",
    ));
    assert!(seal(&fixture.repository, &overlay, &fixture.facts, &candidate).is_err());
}

#[test]
fn observation_reports_kinds_and_links_for_capture_to_refuse() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-one");
    let observed = ok(observe(&overlay));
    let run = observed
        .iter()
        .find(|file| file.destination == RUN.0)
        .map(|file| file.kind);
    assert_eq!(run, Some(EntryKind::Executable));
    ok(std::os::unix::fs::symlink(
        "lib.rs",
        overlay.join("projects/alpha/src/link.rs"),
    ));
    let refused = capture(
        &fixture.definition,
        &fixture.complete,
        &pin,
        &ok(observe(&overlay)),
        &[],
        CurrentAttempt {
            source: &fixture.facts,
            fence: 1,
        },
    );
    let codes: Vec<String> = refused
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(codes, ["unsupported_entry"]);
}
