use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::DirBuilderExt;

use git2::build::TreeUpdateBuilder;
use kiln_adapter_basalt::test_support::{
    COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
};
use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
use kiln_core::projection::authority::{
    AttemptFence, LocalSourceAuthority, LocalSourceGrant, OriginScope, PathScope,
    SnapshotObservation, SnapshotRule, SourceAuthorityRequest, SourceCurrentness,
    SourceViewOperation,
};
use kiln_core::projection::disclosure::ViewCeiling;
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
use crate::worker::source_auth::{RequestProfile, SourceAuthorization};

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
    let mut local_auth = ok(LocalSourceAuthority::new(
        "writeback-test",
        LocalSourceGrant {
            repositories: BTreeSet::from([plan.repository().clone()]),
            operations: BTreeSet::from([
                SourceViewOperation::PrepareWorkspace,
                SourceViewOperation::RetrieveResult,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
                SourceViewOperation::ReadContent,
            ]),
        },
    ));
    let profile = AuthorityProfile::LocalOperator {
        operator: "writeback-test".into(),
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
    let mut auth = SourceAuthorization {
        authority: &mut local_auth,
        profile: RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    ok(prepare_input(&store, &plan, &repository, &mut auth));
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
    fn local_auth(&self) -> (LocalSourceAuthority, AuthorityProfile, SourceCurrentness) {
        let authority = ok(LocalSourceAuthority::new(
            "writeback-test",
            LocalSourceGrant {
                repositories: BTreeSet::from([self.plan.repository().clone()]),
                operations: BTreeSet::from([
                    SourceViewOperation::PrepareWorkspace,
                    SourceViewOperation::RetrieveResult,
                    SourceViewOperation::ModifySource,
                    SourceViewOperation::CreateSource,
                    SourceViewOperation::ReadContent,
                ]),
            },
        ));
        let profile = AuthorityProfile::LocalOperator {
            operator: "writeback-test".into(),
        };
        let current = SourceCurrentness {
            generation: 0,
            revocation_generation: 0,
            snapshot: SnapshotObservation {
                revision: self.plan.source().clone(),
                ref_name: None,
                previous: None,
                fast_forward: false,
            },
            attempt: AttemptCurrentness::Current,
        };
        (authority, profile, current)
    }

    fn observe_request(&self, pin: &WorkspacePin) -> SourceAuthorityRequest {
        SourceAuthorityRequest {
            operation: SourceViewOperation::RetrieveResult,
            repository: pin.repository.clone(),
            snapshot: SnapshotRule::Exact {
                revision: pin.source.clone(),
            },
            view_binding: pin.manifest.clone(),
            scope: OriginScope::Components {
                path: PathScope::root(),
            },
            request_digest: *blake3::hash(pin.overlay.as_str().as_bytes()).as_bytes(),
            attempt: Some(AttemptFence {
                attempt: pin.attempt.clone(),
                fence: pin.fence,
            }),
        }
    }

    /// A pinned overlay for one attempt at fence 1.
    fn overlay(&self, attempt: &str) -> (WorkspacePin, PathBuf) {
        let pin = ok(self.plan.pin(&digest(attempt), 1));
        let (mut local, profile, current) = self.local_auth();
        let mut auth = SourceAuthorization {
            authority: &mut local,
            profile: RequestProfile::Static(&profile),
            current: &current,
            now_unix_s: 1,
        };
        let overlay = ok(prepare_overlay(&self.store, &self.plan, &pin, &mut auth));
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
        let (mut local, profile, current) = self.local_auth();
        let mut auth = SourceAuthorization {
            authority: &mut local,
            profile: RequestProfile::Static(&profile),
            current: &current,
            now_unix_s: 1,
        };
        let obs_req = self.observe_request(pin);
        match ok(capture(
            &self.definition,
            &self.complete,
            pin,
            &ok(observe(overlay, &obs_req, &mut auth)),
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

    fn seal_local(&self, overlay: &Path, candidate: &SealedCandidate) -> Result<Oid, String> {
        let (mut local, profile, current) = self.local_auth();
        let mut auth = SourceAuthorization {
            authority: &mut local,
            profile: RequestProfile::Static(&profile),
            current: &current,
            now_unix_s: 1,
        };
        seal(&self.repository, overlay, &self.facts, candidate, &mut auth)
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
    let sealed = ok(fixture.seal_local(&overlay, &candidate));
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
    let sealed = ok(fixture.seal_local(&overlay, &first));
    let sealed_tree = ok(fixture.repository.find_commit(sealed)).tree_id();

    for alias in ["shared/proto/lib.rs", "vendor/shared/proto/lib.rs"] {
        ok(fs::write(
            overlay.join(alias),
            b"pub const VERSION: u32 = 3;\n",
        ));
    }
    let error = fixture
        .seal_local(&overlay, &first)
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
    let sealed = ok(fixture.seal_local(&overlay, &candidate));
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
    assert!(fixture.seal_local(&overlay, &candidate).is_err());
}

#[test]
fn observation_reports_kinds_and_links_for_capture_to_refuse() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-one");
    let (mut local, profile, current) = fixture.local_auth();
    let mut auth = SourceAuthorization {
        authority: &mut local,
        profile: RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    let obs_req = fixture.observe_request(&pin);
    let observed = ok(observe(&overlay, &obs_req, &mut auth));
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
        &ok(observe(&overlay, &obs_req, &mut auth)),
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

#[test]
fn basalt_source_authority_enforces_per_operation_seal_and_redacted_result() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-basalt");
    // Exercise Edit (shared/proto/lib.rs), Create (projects/alpha/src/created.rs),
    // Delete (projects/alpha/Cargo.toml), Rename (projects/alpha/LICENSE -> src/LICENSE),
    // and Fork (projects/alpha/src/lib.rs -> src/fork.rs).
    ok(fs::write(
        overlay.join("shared/proto/lib.rs"),
        b"pub const VERSION: u32 = 9;\n",
    ));
    ok(fs::write(
        overlay.join("vendor/shared/proto/lib.rs"),
        b"pub const VERSION: u32 = 9;\n",
    ));
    ok(fs::write(
        overlay.join("projects/alpha/src/created.rs"),
        b"// created\n",
    ));
    ok(fs::remove_file(overlay.join("projects/alpha/Cargo.toml")));
    ok(fs::rename(
        overlay.join("projects/alpha/LICENSE"),
        overlay.join("projects/alpha/src/LICENSE"),
    ));
    ok(fs::copy(
        overlay.join("projects/alpha/src/lib.rs"),
        overlay.join("projects/alpha/src/fork.rs"),
    ));
    let candidate = fixture.declared(
        &pin,
        &overlay,
        &[
            Declaration::Rename {
                from: "projects/alpha/LICENSE".into(),
                to: "projects/alpha/src/LICENSE".into(),
            },
            Declaration::Fork {
                from: "projects/alpha/src/lib.rs".into(),
                to: "projects/alpha/src/fork.rs".into(),
            },
        ],
    );
    let attempt = AttemptFence {
        attempt: candidate.attempt().clone(),
        fence: candidate.fence(),
    };
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: candidate.base_revision().clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };

    // 1. Grant lacking CreateSource on projects/alpha/src MUST deny seal before
    // writing any commit or blob!
    let modify_only_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::ModifySource,
                SourceViewOperation::ReadContent,
                SourceViewOperation::RetrieveResult,
            ],
        )],
        EXPIRES_S,
    );
    let mod_signer = modify_only_fix.signer();
    let mut mod_auth = modify_only_fix.authority();
    let sign_mod = |req: &SourceAuthorityRequest| {
        mod_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut mod_ctx = SourceAuthorization {
        authority: &mut mod_auth,
        profile: RequestProfile::Signer(&sign_mod),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let err = match seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
        &mut mod_ctx,
    ) {
        Ok(oid) => unreachable!("unexpected seal: {oid}"),
        Err(err) => err,
    };
    assert!(err.contains("credential-denied"), "{err}");

    // 2. Grant missing ModifySource on Rename's `from` (grants only projects/alpha/src, not root LICENSE) MUST deny!
    let narrow_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[
            (
                PathScope::from_path("projects/alpha/src", true)
                    .unwrap_or_else(|| unreachable!("path")),
                vec![
                    SourceViewOperation::ModifySource,
                    SourceViewOperation::CreateSource,
                    SourceViewOperation::ReadContent,
                ],
            ),
            (
                PathScope::from_path("shared", true).unwrap_or_else(|| unreachable!("path")),
                vec![SourceViewOperation::ModifySource],
            ),
        ],
        EXPIRES_S,
    );
    let narrow_signer = narrow_fix.signer();
    let mut narrow_auth = narrow_fix.authority();
    let sign_narrow = |req: &SourceAuthorityRequest| {
        narrow_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut narrow_ctx = SourceAuthorization {
        authority: &mut narrow_auth,
        profile: RequestProfile::Signer(&sign_narrow),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let err = match seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
        &mut narrow_ctx,
    ) {
        Ok(oid) => unreachable!("unexpected seal: {oid}"),
        Err(err) => err,
    };
    assert!(err.contains("credential-denied"), "{err}");

    // 3. Full attempt-bound grant seals cleanly, and cancelled attempt denies!
    let full_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
                SourceViewOperation::ReadContent,
                SourceViewOperation::RetrieveResult,
            ],
        )],
        EXPIRES_S,
    );
    let full_signer = full_fix.signer();
    let mut full_auth = full_fix.authority();
    let sign_full = |req: &SourceAuthorityRequest| {
        full_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut cancelled = current.clone();
    cancelled.attempt = AttemptCurrentness::Cancelled;
    let mut cancel_ctx = SourceAuthorization {
        authority: &mut full_auth,
        profile: RequestProfile::Signer(&sign_full),
        current: &cancelled,
        now_unix_s: VERIFY_AT_S,
    };
    let cancel_err = match seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
        &mut cancel_ctx,
    ) {
        Ok(oid) => unreachable!("unexpected seal: {oid}"),
        Err(err) => err,
    };
    assert!(cancel_err.contains("attempt-not-current"), "{cancel_err}");

    let mut full_ctx = SourceAuthorization {
        authority: &mut full_auth,
        profile: RequestProfile::Signer(&sign_full),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let sealed_oid = ok(seal(
        &fixture.repository,
        &overlay,
        &fixture.facts,
        &candidate,
        &mut full_ctx,
    ));
    assert!(fixture.repository.find_commit(sealed_oid).is_ok());

    // 4. RetrieveResult projection filters hidden paths without leaking them
    let ceiling = ViewCeiling::from_definition(&fixture.definition);
    let shared_only_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[(
            PathScope::from_path("shared", true).unwrap_or_else(|| unreachable!("path")),
            vec![SourceViewOperation::RetrieveResult],
        )],
        EXPIRES_S,
    );
    let res_signer = shared_only_fix.signer();
    let mut res_auth = shared_only_fix.authority();
    let sign_res = |req: &SourceAuthorityRequest| {
        res_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut res_ctx = SourceAuthorization {
        authority: &mut res_auth,
        profile: RequestProfile::Signer(&sign_res),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let shared_scope = PathScope::from_path("shared", true).unwrap_or_else(|| unreachable!("path"));
    let projected = ok(retrieve_result(
        &candidate,
        &ceiling,
        &shared_scope,
        &mut res_ctx,
    ));
    assert_eq!(projected.operations.len(), 1);
    assert_eq!(
        projected.operations.first().map(|op| op.path()),
        Some("shared/proto/lib.rs")
    );
    assert_eq!(
        projected.omitted_operations,
        candidate.operations().len() - 1
    );
}

#[test]
fn observe_requires_root_retrieve_result_and_retrieve_result_checks_ceiling_definition() {
    let fixture = fixture();
    let (pin, overlay) = fixture.overlay("attempt-observe-scope");
    ok(fs::write(
        overlay.join("shared/proto/lib.rs"),
        b"pub const VERSION: u32 = 2;\n",
    ));
    ok(fs::write(
        overlay.join("vendor/shared/proto/lib.rs"),
        b"pub const VERSION: u32 = 2;\n",
    ));
    let candidate = fixture.declared(&pin, &overlay, &[]);
    let attempt = AttemptFence {
        attempt: candidate.attempt().clone(),
        fence: candidate.fence(),
    };
    let current = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: candidate.base_revision().clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };

    // 1. Narrow request.scope on observe with narrow RetrieveResult grant MUST fail
    let shared_scope = PathScope::from_path("shared", true).unwrap_or_else(|| unreachable!("path"));
    let narrow_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[(
            shared_scope.clone(),
            vec![SourceViewOperation::RetrieveResult],
        )],
        EXPIRES_S,
    );
    let narrow_signer = narrow_fix.signer();
    let mut narrow_auth = narrow_fix.authority();
    let sign_narrow = |req: &SourceAuthorityRequest| {
        narrow_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut narrow_ctx = SourceAuthorization {
        authority: &mut narrow_auth,
        profile: RequestProfile::Signer(&sign_narrow),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let mut narrow_req = fixture.observe_request(&pin);
    narrow_req.scope = OriginScope::Components {
        path: shared_scope.clone(),
    };
    assert!(observe(&overlay, &narrow_req, &mut narrow_ctx).is_err());

    // 2. OriginScope::Handle on observe MUST fail
    let mut handle_req = fixture.observe_request(&pin);
    handle_req.scope = OriginScope::Handle {
        handle: kiln_core::projection::disclosure::OpaqueHandle::from_hex(&"11".repeat(32))
            .unwrap_or_else(|| unreachable!("handle")),
    };
    assert!(observe(&overlay, &handle_req, &mut narrow_ctx).is_err());

    // 3. Root request.scope when token only grants shared/** MUST fail with credential-denied
    let root_req = fixture.observe_request(&pin);
    let err = match observe(&overlay, &root_req, &mut narrow_ctx) {
        Ok(_) => unreachable!("narrow grant must not satisfy root observe"),
        Err(err) => err,
    };
    assert!(err.contains("credential-denied"), "{err}");

    // 4. retrieve_result with ViewCeiling from a different AdmittedDefinition MUST fail
    let full_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        candidate.repository(),
        Some(&attempt),
        &[(PathScope::root(), vec![SourceViewOperation::RetrieveResult])],
        EXPIRES_S,
    );
    let full_signer = full_fix.signer();
    let mut full_auth = full_fix.authority();
    let sign_full = |req: &SourceAuthorityRequest| {
        full_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| format!("{}: {}", e.code, e.message))
    };
    let mut full_ctx = SourceAuthorization {
        authority: &mut full_auth,
        profile: RequestProfile::Signer(&sign_full),
        current: &current,
        now_unix_s: VERIFY_AT_S,
    };
    let other_def = ok(admit_definition(&definition(
        "other-ceiling",
        ProjectionProfile::Runnable,
        &["shared/**"],
    )));
    let other_ceiling = ViewCeiling::from_definition(&other_def);
    assert!(retrieve_result(&candidate, &other_ceiling, &shared_scope, &mut full_ctx).is_err());
}
