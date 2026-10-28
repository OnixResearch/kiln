// r[verify kiln.review.provider]
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{Oid, Repository, Signature, Time};
use kiln_adapter_git::change_review::{LocalGitReviewProfile, LocalGitReviewProvider};
use kiln_provider_api::change_review::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    path: PathBuf,
    repo: Repository,
    base: Oid,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "kiln-review-provider-{}-{label}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = Repository::init(&path)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
        let base = make_commit(&repo, &[], &[("file.txt", b"one\ntwo\nthree\n")], "base");
        repo.reference("refs/heads/main", base, true, "fixture target")
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
        Self { path, repo, base }
    }
    fn profile(&self) -> LocalGitReviewProfile {
        LocalGitReviewProfile {
            repository_id: "test-repository".into(),
            repository_path: self.path.clone(),
            target_refs: vec!["refs/heads/main".into()],
            capabilities: vec![
                ReviewCapability::Inspect,
                ReviewCapability::Compare,
                ReviewCapability::Register,
                ReviewCapability::Submit,
                ReviewCapability::Rebase,
                ReviewCapability::Revert,
                ReviewCapability::CherryPick,
                ReviewCapability::ApplySuggestions,
                ReviewCapability::BuildIntegration,
            ],
        }
    }
    fn provider(&self) -> LocalGitReviewProvider {
        LocalGitReviewProvider::open(self.profile())
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
    }
    fn request(&self, number: u64, operation: ReviewOperation) -> ReviewOperationRequest {
        ReviewOperationRequest {
            version: REVIEW_PORT_VERSION,
            operation_id: format!("{number:064x}"),
            repository_id: "test-repository".into(),
            target_ref: "refs/heads/main".into(),
            operation,
            signature: ReviewSignature {
                name: "Review Author".into(),
                email: "review@example.invalid".into(),
                time_seconds: 1_700_000_000,
            },
        }
    }
    fn current(&self) -> Oid {
        self.repo
            .refname_to_id("refs/heads/main")
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn make_commit(repo: &Repository, parents: &[Oid], files: &[(&str, &[u8])], message: &str) -> Oid {
    let mut builder = repo
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    for (path, content) in files {
        builder
            .insert(
                *path,
                repo.blob(content)
                    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
                0o100644,
            )
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    }
    let tree = repo
        .find_tree(
            builder
                .write()
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_600_000_000, 0),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let parents: Vec<_> = parents
        .iter()
        .map(|id| {
            repo.find_commit(*id)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        })
        .collect();
    let references: Vec<_> = parents.iter().collect();
    repo.commit(None, &signature, &signature, message, &tree, &references)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
}
fn applied(effect: ReviewEffect) -> String {
    match effect {
        ReviewEffect::Applied { commit, .. } => commit,
        other => unreachable!("expected applied, got {other:?}"),
    }
}
fn comparison(old: Oid, new: Oid) -> CompareRequest {
    CompareRequest {
        old_commit: old.to_string(),
        new_commit: new.to_string(),
        limits: DiffLimits::default(),
        whitespace: WhitespaceMode::Exact,
    }
}

#[test]
fn review_provider_exact_submit_and_revision_registration_are_idempotent() {
    let f = Fixture::new("submit");
    let candidate = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one\nchanged\nthree\n")],
        "candidate",
    );
    let provider = f.provider();
    let registration = f.request(
        1,
        ReviewOperation::Register {
            commit: candidate.to_string(),
        },
    );
    assert_eq!(
        applied(
            provider
                .execute(&registration)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        candidate.to_string()
    );
    assert_eq!(f.current(), f.base, "upload never submits target");
    let request = f.request(
        2,
        ReviewOperation::Submit {
            commit: candidate.to_string(),
            expected_target: f.base.to_string(),
        },
    );
    assert!(matches!(
        provider
            .reconcile(&request)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        ReviewEffect::NotApplied { .. }
    ));
    assert_eq!(
        applied(
            provider
                .execute(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        candidate.to_string()
    );
    assert_eq!(f.current(), candidate);
    let count = f
        .repo
        .reflog("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        .len();
    assert_eq!(
        applied(
            provider
                .execute(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        candidate.to_string()
    );
    assert_eq!(
        f.repo
            .reflog("refs/heads/main")
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .len(),
        count,
        "retry must not repeat external effect"
    );
    let inspection = provider
        .inspect(&candidate.to_string(), "refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(inspection.parents, vec![f.base.to_string()]);
    assert!(inspection.ancestor_of_target && inspection.target_is_ancestor);
    let mut profile = f.profile();
    profile.capabilities = vec![ReviewCapability::Inspect];
    let withdrawn = LocalGitReviewProvider::open(profile)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(
        applied(
            withdrawn
                .reconcile(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        candidate.to_string()
    );
    assert_eq!(
        withdrawn
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::Unsupported
    );
}

#[test]
fn review_provider_refuses_target_race_non_fast_forward_and_locked_target() {
    let f = Fixture::new("race");
    let candidate = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"candidate\n")],
        "candidate",
    );
    let rival = make_commit(&f.repo, &[f.base], &[("file.txt", b"rival\n")], "rival");
    let provider = f.provider();
    let request = f.request(
        3,
        ReviewOperation::Submit {
            commit: candidate.to_string(),
            expected_target: f.base.to_string(),
        },
    );
    f.repo
        .reference("refs/heads/main", rival, true, "racing writer")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(
        provider
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::TargetMoved
    );
    assert_eq!(f.current(), rival);
    let divergent = f.request(
        4,
        ReviewOperation::Submit {
            commit: candidate.to_string(),
            expected_target: rival.to_string(),
        },
    );
    assert_eq!(
        provider
            .execute(&divergent)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::Conflict
    );
    let mut lock = f
        .repo
        .transaction()
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    lock.lock_ref("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let next = make_commit(&f.repo, &[rival], &[("file.txt", b"next\n")], "next");
    let locked = f.request(
        5,
        ReviewOperation::Submit {
            commit: next.to_string(),
            expected_target: rival.to_string(),
        },
    );
    assert!(provider.execute(&locked).is_err());
    assert_eq!(f.current(), rival);
}

#[test]
fn review_provider_refuses_malformed_escaped_and_disabled_requests_without_effects() {
    let f = Fixture::new("refusals");
    let mut profile = f.profile();
    profile.capabilities = vec![ReviewCapability::Inspect];
    let disabled = LocalGitReviewProvider::open(profile)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let request = f.request(
        6,
        ReviewOperation::Register {
            commit: f.base.to_string(),
        },
    );
    assert_eq!(
        disabled
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::Unsupported
    );
    assert!(
        f.repo
            .find_reference(&format!(
                "refs/kiln/review-operations/{}/request",
                request.operation_id
            ))
            .is_err()
    );
    let provider = f.provider();
    let mut escaped = request.clone();
    escaped.target_ref = "refs/heads/main/../../other".into();
    assert_eq!(
        provider
            .execute(&escaped)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    let mut malformed = request.clone();
    malformed.operation = ReviewOperation::Register {
        commit: "HEAD".into(),
    };
    assert_eq!(
        provider
            .execute(&malformed)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    malformed = request.clone();
    malformed.operation_id = "../escape".into();
    assert_eq!(
        provider
            .execute(&malformed)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    malformed = request.clone();
    malformed.version = 2;
    assert_eq!(
        provider
            .execute(&malformed)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    malformed = request.clone();
    malformed.repository_id = "another-repository".into();
    assert_eq!(
        provider
            .execute(&malformed)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    assert_eq!(f.current(), f.base);
    applied(
        provider
            .execute(&request)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    );
    let next = make_commit(&f.repo, &[f.base], &[("file.txt", b"new\n")], "next");
    let mut reused = request;
    reused.operation = ReviewOperation::Register {
        commit: next.to_string(),
    };
    assert_eq!(
        provider
            .execute(&reused)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    let mut profile = f.profile();
    profile.target_refs = vec!["HEAD".into()];
    assert!(LocalGitReviewProvider::open(profile).is_err());
}

#[test]
fn review_provider_diff_has_exact_paths_coordinates_renames_binary_and_bounds() {
    let f = Fixture::new("diff");
    let old = make_commit(
        &f.repo,
        &[f.base],
        &[
            ("old.txt", b"identical rename\n"),
            ("file.txt", b"one\ntwo\nthree\n"),
            ("binary", b"a\0b"),
        ],
        "old",
    );
    let new = make_commit(
        &f.repo,
        &[old],
        &[
            ("new.txt", b"identical rename\n"),
            ("file.txt", b"one\nchanged\nthree\n"),
            ("binary", b"b\0c"),
        ],
        "new",
    );
    let provider = f.provider();
    let diff = provider
        .compare(&comparison(old, new))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(diff.old_commit, old.to_string());
    assert_eq!(diff.new_commit, new.to_string());
    assert!(!diff.truncated);
    let rename = diff
        .files
        .iter()
        .find(|file| file.renamed)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(rename.old_path.as_deref(), Some("old.txt"));
    assert_eq!(rename.new_path.as_deref(), Some("new.txt"));
    let binary = diff
        .files
        .iter()
        .find(|file| file.new_path.as_deref() == Some("binary"))
        .unwrap_or_else(|| unreachable!());
    assert!(binary.binary && binary.hunks.is_empty());
    let text = diff
        .files
        .iter()
        .find(|file| file.new_path.as_deref() == Some("file.txt"))
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        (text.old_line_count, text.new_line_count),
        (Some(3), Some(3))
    );
    let lines: Vec<_> = text.hunks.iter().flat_map(|h| &h.lines).collect();
    assert!(lines.iter().any(|l| l.kind == DiffLineKind::Deletion
        && l.old_line == Some(2)
        && l.new_line.is_none()
        && l.text == "two\n"));
    assert!(lines.iter().any(|l| l.kind == DiffLineKind::Addition
        && l.old_line.is_none()
        && l.new_line == Some(2)
        && l.text == "changed\n"));
    assert!(lines.iter().any(|l| l.kind == DiffLineKind::Context
        && l.old_line == Some(3)
        && l.new_line == Some(3)));
    let mut bounded = comparison(old, new);
    bounded.limits.max_files = 1;
    assert!(
        provider
            .compare(&bounded)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .truncated
    );
    bounded = comparison(old, new);
    bounded.limits.max_lines = 1;
    assert!(
        provider
            .compare(&bounded)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .files
            .iter()
            .any(|file| file.truncated)
    );
    bounded.limits.max_lines = 0;
    assert_eq!(
        provider
            .compare(&bounded)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
}

#[test]
fn review_provider_author_actions_return_exact_revisions_without_target_mutation() {
    let f = Fixture::new("author");
    let topic = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one\ntwo\nthree\n"), ("topic", b"topic\n")],
        "topic",
    );
    let onto = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one\ntwo\nthree\n"), ("target", b"target\n")],
        "onto",
    );
    let provider = f.provider();
    for (number, operation) in [
        (
            7,
            ReviewOperation::Rebase {
                commit: topic.to_string(),
                onto: onto.to_string(),
            },
        ),
        (
            8,
            ReviewOperation::CherryPick {
                commit: topic.to_string(),
                onto: onto.to_string(),
            },
        ),
    ] {
        let request = f.request(number, operation);
        let result = applied(
            provider
                .execute(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        );
        let transformed = f
            .repo
            .find_commit(
                Oid::from_str(&result)
                    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
            )
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
        assert_eq!(
            transformed
                .parent_id(0)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
            onto
        );
        assert!(
            transformed
                .tree()
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
                .get_name("topic")
                .is_some()
        );
        assert!(
            transformed
                .tree()
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
                .get_name("target")
                .is_some()
        );
        assert_eq!(
            applied(
                provider
                    .execute(&request)
                    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            ),
            result
        );
        assert_eq!(f.current(), f.base);
    }
    let revert = f.request(
        9,
        ReviewOperation::Revert {
            commit: topic.to_string(),
            onto: topic.to_string(),
        },
    );
    let reverted = applied(
        provider
            .execute(&revert)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    );
    let reverted = f
        .repo
        .find_commit(
            Oid::from_str(&reverted)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(
        reverted
            .parent_id(0)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        topic
    );
    assert_eq!(
        reverted.tree_id(),
        f.repo
            .find_commit(f.base)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .tree_id()
    );
    assert_eq!(f.current(), f.base);
    let second = make_commit(
        &f.repo,
        &[topic],
        &[
            ("file.txt", b"one\ntwo\nthree\n"),
            ("topic", b"topic\n"),
            ("second", b"second\n"),
        ],
        "second topic commit",
    );
    let request = f.request(
        13,
        ReviewOperation::Rebase {
            commit: second.to_string(),
            onto: onto.to_string(),
        },
    );
    let rebased = applied(
        provider
            .execute(&request)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    );
    let rebased = f
        .repo
        .find_commit(
            Oid::from_str(&rebased)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert!(
        rebased
            .tree()
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .get_name("second")
            .is_some()
    );
    assert!(
        rebased
            .tree()
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .get_name("target")
            .is_some()
    );
    assert_eq!(
        rebased
            .parent(0)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .parent_id(0)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        onto,
        "rebase must replay the full linear series"
    );
    assert_eq!(f.current(), f.base);
}

#[test]
fn review_provider_conflicting_author_operation_never_changes_target() {
    let f = Fixture::new("conflict");
    let topic = make_commit(&f.repo, &[f.base], &[("file.txt", b"topic\n")], "topic");
    let onto = make_commit(&f.repo, &[f.base], &[("file.txt", b"target\n")], "onto");
    let provider = f.provider();
    for (number, operation) in [
        (
            10,
            ReviewOperation::Rebase {
                commit: topic.to_string(),
                onto: onto.to_string(),
            },
        ),
        (
            11,
            ReviewOperation::CherryPick {
                commit: topic.to_string(),
                onto: onto.to_string(),
            },
        ),
    ] {
        let request = f.request(number, operation);
        assert_eq!(
            provider
                .execute(&request)
                .err()
                .unwrap_or_else(|| unreachable!("expected refusal"))
                .kind,
            ReviewErrorKind::Conflict
        );
        assert!(matches!(
            provider
                .reconcile(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
            ReviewEffect::NotApplied { .. }
        ));
        assert_eq!(f.current(), f.base);
    }
}

#[test]
fn review_provider_reconciles_crash_after_effect_without_duplicate_mutation() {
    let f = Fixture::new("reconcile");
    let next = make_commit(&f.repo, &[f.base], &[("file.txt", b"next\n")], "next");
    let provider = f.provider();
    let request = f.request(
        12,
        ReviewOperation::Submit {
            commit: next.to_string(),
            expected_target: f.base.to_string(),
        },
    );
    applied(
        provider
            .execute(&request)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    );
    // Simulate the exact crash window: target CAS succeeded, durable shell
    // operation and provider preparation exist, but result journal was not saved.
    f.repo
        .find_reference(&format!(
            "refs/kiln/review-operations/{}/result",
            request.operation_id
        ))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        .delete()
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let count = f
        .repo
        .reflog("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        .len();
    assert_eq!(
        applied(
            provider
                .reconcile(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        next.to_string()
    );
    assert_eq!(
        applied(
            provider
                .execute(&request)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
        ),
        next.to_string()
    );
    assert_eq!(
        f.repo
            .reflog("refs/heads/main")
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
            .len(),
        count
    );
    f.repo
        .reference(
            "refs/heads/main",
            f.base,
            true,
            "another writer reset target",
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert!(matches!(
        provider
            .reconcile(&request)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        ReviewEffect::Uncertain { .. }
    ));
    assert_eq!(
        provider
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!("expected refusal"))
            .kind,
        ReviewErrorKind::Uncertain
    );
    assert_eq!(
        f.current(),
        f.base,
        "uncertain effect must never be repeated"
    );
}

#[test]
fn review_provider_admission_rejects_parent_paths_and_symbolic_target_refs() {
    let f = Fixture::new("admission");
    let mut profile = f.profile();
    profile.repository_path = f.path.join("missing-child");
    assert!(LocalGitReviewProvider::open(profile).is_err());
    f.repo
        .reference_symbolic(
            "refs/heads/alias",
            "refs/heads/main",
            true,
            "symbolic fixture",
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let mut profile = f.profile();
    profile.target_refs = vec!["refs/heads/alias".into()];
    assert_eq!(
        LocalGitReviewProvider::open(profile)
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind,
        ReviewErrorKind::InvalidRequest
    );
}

#[test]
fn review_provider_refuses_duplicate_unbounded_and_oversized_configuration() {
    let f = Fixture::new("input-boundaries");
    let mut duplicate_refs = f.profile();
    duplicate_refs.target_refs.push("refs/heads/main".into());
    let mut duplicate_capabilities = f.profile();
    duplicate_capabilities
        .capabilities
        .push(ReviewCapability::Inspect);
    let mut unbounded_refs = f.profile();
    unbounded_refs.target_refs = vec!["refs/heads/main".into(); 65];
    let mut unbounded_capabilities = f.profile();
    unbounded_capabilities.capabilities = vec![ReviewCapability::Inspect; 10];
    let mut oversized_identity = f.profile();
    oversized_identity.repository_id = "r".repeat(257);
    let mut oversized_ref = f.profile();
    oversized_ref.target_refs = vec![format!("refs/heads/{}", "r".repeat(1_025))];
    let mut oversized_path = f.profile();
    oversized_path.repository_path = PathBuf::from(format!("/{}", "r".repeat(4_096)));
    for profile in [
        duplicate_refs,
        duplicate_capabilities,
        unbounded_refs,
        unbounded_capabilities,
        oversized_identity,
        oversized_ref,
        oversized_path,
    ] {
        let error = LocalGitReviewProvider::open(profile)
            .err()
            .unwrap_or_else(|| unreachable!("invalid profile was admitted"));
        assert_eq!(error.kind, ReviewErrorKind::InvalidRequest);
    }
    let provider = f.provider();
    let mut request = f.request(
        14,
        ReviewOperation::Register {
            commit: f.base.to_string(),
        },
    );
    request.signature.name = "a".repeat(257);
    assert_eq!(
        provider
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!("oversized signature was admitted"))
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    assert!(
        f.repo
            .find_reference(&format!(
                "refs/kiln/review-operations/{}/request",
                request.operation_id
            ))
            .is_err()
    );
    assert_eq!(f.current(), f.base);
}

#[test]
fn review_provider_ordered_stack_preserves_exact_child_revision_and_ancestry_proof() {
    let f = Fixture::new("ordered-stack");
    let parent = make_commit(&f.repo, &[f.base], &[("parent", b"parent\n")], "parent");
    let child = make_commit(
        &f.repo,
        &[parent],
        &[("parent", b"parent\n"), ("child", b"child\n")],
        "child",
    );
    let unrelated = make_commit(
        &f.repo,
        &[f.base],
        &[("unrelated", b"other\n")],
        "unrelated",
    );
    let provider = f.provider();
    let parent_request = f.request(
        15,
        ReviewOperation::Submit {
            commit: parent.to_string(),
            expected_target: f.base.to_string(),
        },
    );
    assert_eq!(
        applied(
            provider
                .execute(&parent_request)
                .unwrap_or_else(|error| unreachable!("parent submit failed: {error:?}"))
        ),
        parent.to_string()
    );
    let proof = provider
        .inspect(&child.to_string(), "refs/heads/main")
        .unwrap_or_else(|error| unreachable!("child inspection failed: {error:?}"));
    assert_eq!(proof.commit, child.to_string());
    assert_eq!(proof.target_commit, parent.to_string());
    assert!(proof.target_is_ancestor);
    let child_request = f.request(
        16,
        ReviewOperation::Submit {
            commit: child.to_string(),
            expected_target: parent.to_string(),
        },
    );
    assert_eq!(
        applied(
            provider
                .execute(&child_request)
                .unwrap_or_else(|error| unreachable!("child submit failed: {error:?}"))
        ),
        child.to_string()
    );
    assert_eq!(f.current(), child);
    f.repo
        .reference("refs/heads/main", unrelated, true, "unrelated target move")
        .unwrap_or_else(|error| unreachable!("fixture target move failed: {error:?}"));
    let proof = provider
        .inspect(&child.to_string(), "refs/heads/main")
        .unwrap_or_else(|error| unreachable!("unrelated inspection failed: {error:?}"));
    assert_eq!(proof.target_commit, unrelated.to_string());
    assert!(!proof.target_is_ancestor);
    let invalid = f.request(
        17,
        ReviewOperation::Submit {
            commit: child.to_string(),
            expected_target: unrelated.to_string(),
        },
    );
    assert_eq!(
        provider
            .execute(&invalid)
            .err()
            .unwrap_or_else(|| unreachable!("unrelated target was admitted"))
            .kind,
        ReviewErrorKind::Conflict
    );
    assert_eq!(f.current(), unrelated);
}

// r[verify kiln.review.parity.ergonomics]
#[test]
fn review_suggestions_apply_atomically_and_recover_identical_commit() {
    let f = Fixture::new("suggestions");
    let provider = f.provider();
    let edits = vec![
        SuggestionEdit {
            path: "file.txt".into(),
            start_line: 3,
            end_line: 3,
            expected_original: "three\n".into(),
            replacement: "THREE\nlast".into(),
        },
        SuggestionEdit {
            path: "file.txt".into(),
            start_line: 1,
            end_line: 1,
            expected_original: "one\n".into(),
            replacement: "ONE\n".into(),
        },
    ];
    let request = f.request(
        100,
        ReviewOperation::ApplySuggestions {
            commit: f.base.to_string(),
            suggestions: edits,
        },
    );
    let result = applied(
        provider
            .execute(&request)
            .unwrap_or_else(|e| unreachable!("{e}")),
    );
    assert_eq!(f.current(), f.base);
    let selected = f
        .repo
        .find_commit(Oid::from_str(&result).unwrap_or_else(|e| unreachable!("{e}")))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(selected.parent_count(), 1);
    assert_eq!(
        selected
            .parent_id(0)
            .unwrap_or_else(|e| unreachable!("{e}")),
        f.base
    );
    let tree = selected.tree().unwrap_or_else(|e| unreachable!("{e}"));
    let entry = tree
        .get_path(std::path::Path::new("file.txt"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        f.repo
            .find_blob(entry.id())
            .unwrap_or_else(|e| unreachable!("{e}"))
            .content(),
        b"ONE\ntwo\nTHREE\nlast"
    );
    assert_eq!(
        applied(
            provider
                .execute(&request)
                .unwrap_or_else(|e| unreachable!("{e}"))
        ),
        result
    );
    assert_eq!(
        applied(
            provider
                .reconcile(&request)
                .unwrap_or_else(|e| unreachable!("{e}"))
        ),
        result
    );
    f.repo
        .find_reference(&format!(
            "refs/kiln/review-operations/{}/result",
            request.operation_id
        ))
        .unwrap_or_else(|e| unreachable!("{e}"))
        .delete()
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(matches!(
        provider
            .reconcile(&request)
            .unwrap_or_else(|e| unreachable!("{e}")),
        ReviewEffect::NotApplied { .. }
    ));
    let reopened = f.provider();
    assert_eq!(
        applied(
            reopened
                .execute(&request)
                .unwrap_or_else(|e| unreachable!("{e}"))
        ),
        result
    );
    let mut reused = request;
    if let ReviewOperation::ApplySuggestions { suggestions, .. } = &mut reused.operation {
        suggestions
            .first_mut()
            .unwrap_or_else(|| unreachable!("suggestion fixture is nonempty"))
            .replacement = "different\n".into();
    }
    assert_eq!(
        provider
            .execute(&reused)
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind,
        ReviewErrorKind::InvalidRequest
    );
    assert_eq!(f.current(), f.base);
}

#[test]
fn review_suggestions_refuse_overlap_stale_content_paths_binary_and_bounds() {
    let f = Fixture::new("suggestion-refusals");
    let provider = f.provider();
    let valid = SuggestionEdit {
        path: "file.txt".into(),
        start_line: 2,
        end_line: 2,
        expected_original: "two\n".into(),
        replacement: "TWO\n".into(),
    };
    let mut stale = valid.clone();
    stale.expected_original = "stale\n".into();
    let mut outside = valid.clone();
    outside.end_line = 4;
    let mut zero = valid.clone();
    zero.start_line = 0;
    let mut traversal = valid.clone();
    traversal.path = "../file.txt".into();
    let mut absolute = valid.clone();
    absolute.path = "/file.txt".into();
    let mut nested_traversal = valid.clone();
    nested_traversal.path = "x/../file.txt".into();
    let mut binary_text = valid.clone();
    binary_text.replacement = "\0".into();
    let first = SuggestionEdit {
        start_line: 1,
        end_line: 1,
        expected_original: "one\n".into(),
        ..valid.clone()
    };
    for (index, (edits, kind)) in [
        (
            vec![valid.clone(), valid.clone()],
            ReviewErrorKind::InvalidRequest,
        ),
        (vec![first, stale.clone()], ReviewErrorKind::Conflict),
        (vec![stale], ReviewErrorKind::Conflict),
        (vec![outside], ReviewErrorKind::InvalidRequest),
        (vec![zero], ReviewErrorKind::InvalidRequest),
        (vec![traversal], ReviewErrorKind::InvalidRequest),
        (vec![absolute], ReviewErrorKind::InvalidRequest),
        (vec![nested_traversal], ReviewErrorKind::InvalidRequest),
        (vec![binary_text], ReviewErrorKind::InvalidRequest),
        (vec![], ReviewErrorKind::InvalidRequest),
    ]
    .into_iter()
    .enumerate()
    {
        let request = f.request(
            110 + index as u64,
            ReviewOperation::ApplySuggestions {
                commit: f.base.to_string(),
                suggestions: edits,
            },
        );
        assert_eq!(
            provider
                .execute(&request)
                .err()
                .unwrap_or_else(|| unreachable!())
                .kind,
            kind
        );
        assert!(
            f.repo
                .find_reference(&format!(
                    "refs/kiln/review-operations/{}/result",
                    request.operation_id
                ))
                .is_err()
        );
        assert_eq!(f.current(), f.base);
    }
    for (index, bytes) in [
        b"\0binary".as_slice(),
        b"bad\xff\n".as_slice(),
        b"".as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let source = make_commit(&f.repo, &[f.base], &[("file.txt", bytes)], "invalid text");
        let request = f.request(
            130 + index as u64,
            ReviewOperation::ApplySuggestions {
                commit: source.to_string(),
                suggestions: vec![SuggestionEdit {
                    start_line: 1,
                    end_line: 1,
                    expected_original: String::new(),
                    ..valid.clone()
                }],
            },
        );
        assert_eq!(
            provider
                .execute(&request)
                .err()
                .unwrap_or_else(|| unreachable!())
                .kind,
            if bytes.is_empty() {
                ReviewErrorKind::InvalidRequest
            } else {
                ReviewErrorKind::Unsupported
            }
        );
        assert_eq!(f.current(), f.base);
    }
    let mut profile = f.profile();
    profile
        .capabilities
        .retain(|cap| *cap != ReviewCapability::ApplySuggestions);
    let disabled = LocalGitReviewProvider::open(profile).unwrap_or_else(|e| unreachable!("{e}"));
    let request = f.request(
        140,
        ReviewOperation::ApplySuggestions {
            commit: f.base.to_string(),
            suggestions: vec![valid],
        },
    );
    assert_eq!(
        disabled
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind,
        ReviewErrorKind::Unsupported
    );
    assert!(
        f.repo
            .find_reference(&format!(
                "refs/kiln/review-operations/{}/request",
                request.operation_id
            ))
            .is_err()
    );
}

// r[verify kiln.review.parity.queue]
#[test]
fn review_integration_combines_exact_source_target_then_submits_only_by_cas() {
    let f = Fixture::new("integration");
    let source = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one\ntwo\nthree\n"), ("source", b"source\n")],
        "source",
    );
    let target_commit = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one\ntwo\nthree\n"), ("target", b"target\n")],
        "target",
    );
    f.repo
        .reference("refs/heads/main", target_commit, true, "fixture target")
        .unwrap_or_else(|e| unreachable!("{e}"));
    let provider = f.provider();
    for (index, strategy) in [IntegrationStrategy::Merge, IntegrationStrategy::Rebase]
        .into_iter()
        .enumerate()
    {
        let request = f.request(
            150 + index as u64,
            ReviewOperation::BuildIntegration {
                commit: source.to_string(),
                expected_target: target_commit.to_string(),
                strategy,
            },
        );
        let result = applied(
            provider
                .execute(&request)
                .unwrap_or_else(|e| unreachable!("{e}")),
        );
        assert_eq!(
            applied(
                provider
                    .reconcile(&request)
                    .unwrap_or_else(|e| unreachable!("{e}"))
            ),
            result
        );
        assert_eq!(
            applied(
                provider
                    .execute(&request)
                    .unwrap_or_else(|e| unreachable!("{e}"))
            ),
            result
        );
        f.repo
            .find_reference(&format!(
                "refs/kiln/review-operations/{}/result",
                request.operation_id
            ))
            .unwrap_or_else(|e| unreachable!("{e}"))
            .delete()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(matches!(
            provider
                .reconcile(&request)
                .unwrap_or_else(|e| unreachable!("{e}")),
            ReviewEffect::NotApplied { .. }
        ));
        assert_eq!(
            applied(
                f.provider()
                    .execute(&request)
                    .unwrap_or_else(|e| unreachable!("{e}"))
            ),
            result
        );
        let mut reused = request.clone();
        if let ReviewOperation::BuildIntegration { strategy, .. } = &mut reused.operation {
            *strategy = if *strategy == IntegrationStrategy::Merge {
                IntegrationStrategy::Rebase
            } else {
                IntegrationStrategy::Merge
            };
        }
        assert_eq!(
            provider
                .execute(&reused)
                .err()
                .unwrap_or_else(|| unreachable!())
                .kind,
            ReviewErrorKind::InvalidRequest
        );
        assert_eq!(f.current(), target_commit);
        let candidate = f
            .repo
            .find_commit(Oid::from_str(&result).unwrap_or_else(|e| unreachable!("{e}")))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            candidate
                .parent_id(0)
                .unwrap_or_else(|e| unreachable!("{e}")),
            target_commit
        );
        if strategy == IntegrationStrategy::Merge {
            assert_eq!(
                candidate
                    .parent_id(1)
                    .unwrap_or_else(|e| unreachable!("{e}")),
                source
            );
        }
        let tree = candidate.tree().unwrap_or_else(|e| unreachable!("{e}"));
        for path in ["source", "target"] {
            assert!(tree.get_name(path).is_some());
        }
        let rival = make_commit(&f.repo, &[target_commit], &[("rival", b"rival\n")], "rival");
        f.repo
            .reference("refs/heads/main", rival, true, "fixture move")
            .unwrap_or_else(|e| unreachable!("{e}"));
        let submit = f.request(
            160 + index as u64,
            ReviewOperation::Submit {
                commit: result.clone(),
                expected_target: target_commit.to_string(),
            },
        );
        assert_eq!(
            provider
                .execute(&submit)
                .err()
                .unwrap_or_else(|| unreachable!())
                .kind,
            ReviewErrorKind::TargetMoved
        );
        assert_eq!(f.current(), rival);
        assert_eq!(
            applied(
                provider
                    .reconcile(&request)
                    .unwrap_or_else(|e| unreachable!("{e}"))
            ),
            result
        );
        f.repo
            .reference("refs/heads/main", target_commit, true, "fixture restore")
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            applied(
                provider
                    .execute(&submit)
                    .unwrap_or_else(|e| unreachable!("{e}"))
            ),
            result
        );
        assert_eq!(f.current().to_string(), result);
        f.repo
            .reference("refs/heads/main", target_commit, true, "fixture restore")
            .unwrap_or_else(|e| unreachable!("{e}"));
    }
}

#[test]
fn review_integration_refuses_conflicts_moved_targets_and_disabled_capabilities() {
    let f = Fixture::new("integration-refusals");
    let source = make_commit(&f.repo, &[f.base], &[("file.txt", b"source\n")], "source");
    let other = make_commit(&f.repo, &[f.base], &[("file.txt", b"target\n")], "target");
    f.repo
        .reference("refs/heads/main", other, true, "fixture target")
        .unwrap_or_else(|e| unreachable!("{e}"));
    let provider = f.provider();
    for (index, strategy) in [IntegrationStrategy::Merge, IntegrationStrategy::Rebase]
        .into_iter()
        .enumerate()
    {
        let request = f.request(
            170 + index as u64,
            ReviewOperation::BuildIntegration {
                commit: source.to_string(),
                expected_target: other.to_string(),
                strategy,
            },
        );
        assert_eq!(
            provider
                .execute(&request)
                .err()
                .unwrap_or_else(|| unreachable!())
                .kind,
            ReviewErrorKind::Conflict
        );
        assert!(
            f.repo
                .find_reference(&format!(
                    "refs/kiln/review-operations/{}/result",
                    request.operation_id
                ))
                .is_err()
        );
        assert_eq!(f.current(), other);
    }
    let request = f.request(
        180,
        ReviewOperation::BuildIntegration {
            commit: source.to_string(),
            expected_target: f.base.to_string(),
            strategy: IntegrationStrategy::Merge,
        },
    );
    assert_eq!(
        provider
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind,
        ReviewErrorKind::TargetMoved
    );
    assert!(
        f.repo
            .find_reference(&format!(
                "refs/kiln/review-operations/{}/request",
                request.operation_id
            ))
            .is_err()
    );
    let mut profile = f.profile();
    profile
        .capabilities
        .retain(|cap| *cap != ReviewCapability::BuildIntegration);
    let disabled = LocalGitReviewProvider::open(profile).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        disabled
            .execute(&request)
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind,
        ReviewErrorKind::Unsupported
    );
}

#[test]
fn review_whitespace_filter_retains_exact_blob_line_coordinates() {
    let f = Fixture::new("whitespace");
    let source = make_commit(
        &f.repo,
        &[f.base],
        &[("file.txt", b"one  \ntwo changed\nthree\n")],
        "whitespace",
    );
    let provider = f.provider();
    let mut request = comparison(f.base, source);
    request.whitespace = WhitespaceMode::IgnoreEol;
    let diff = provider
        .compare(&request)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let lines: Vec<_> = diff
        .files
        .iter()
        .flat_map(|file| &file.hunks)
        .flat_map(|hunk| &hunk.lines)
        .collect();
    assert!(
        !lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Addition && line.new_line == Some(1))
    );
    assert!(lines.iter().any(|line| line.kind == DiffLineKind::Addition
        && line.new_line == Some(2)
        && line.text == "two changed\n"));
    request.whitespace = WhitespaceMode::Exact;
    let diff = provider
        .compare(&request)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(
        diff.files
            .iter()
            .flat_map(|file| &file.hunks)
            .flat_map(|hunk| &hunk.lines)
            .any(|line| line.kind == DiffLineKind::Addition
                && line.new_line == Some(1)
                && line.text == "one  \n")
    );
}
