//! Real local Git fixtures for exact, bounded, read-only session trace observations.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{Oid, Repository, Signature, Time};
use kiln_adapter_git::change_review::{LocalGitReviewProfile, LocalGitReviewProvider};
use kiln_core::change_review::{
    MAX_SESSION_TRACE_NOTE_BYTES, MAX_SESSION_TRACES, RevisionSessionTraces, SessionTraceStatus,
    SessionTraceUnavailable,
};
use kiln_provider_api::change_review::{ChangeReviewPort, ReviewCapability, ReviewErrorKind};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const NOTES: &str = "refs/notes/traces";

fn ok<T, E: std::fmt::Debug>(value: Result<T, E>) -> T {
    value.unwrap_or_else(|error| unreachable!("fixture: {error:?}"))
}
struct Fixture {
    path: PathBuf,
    repo: Repository,
    first: Oid,
    second: Oid,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "kiln-session-traces-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = ok(Repository::init(&path));
        let signature = ok(Signature::new(
            "Fixture",
            "fixture@example.invalid",
            &Time::new(1_600_000_000, 0),
        ));
        let tree_id = ok(ok(repo.treebuilder(None)).write());
        let (first, second) = {
            let tree = ok(repo.find_tree(tree_id));
            let first = ok(repo.commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "first",
                &tree,
                &[],
            ));
            let parent = ok(repo.find_commit(first));
            let second = ok(repo.commit(None, &signature, &signature, "second", &tree, &[&parent]));
            (first, second)
        };
        Self {
            path,
            repo,
            first,
            second,
        }
    }
    fn provider(&self, enabled: bool) -> LocalGitReviewProvider {
        ok(LocalGitReviewProvider::open(LocalGitReviewProfile {
            repository_id: "fixture".into(),
            repository_path: self.path.clone(),
            target_refs: vec!["refs/heads/main".into()],
            capabilities: if enabled {
                vec![ReviewCapability::Inspect]
            } else {
                Vec::new()
            },
        }))
    }
    fn note(&self, commit: Oid, bytes: &[u8], fanout: bool) {
        let previous = self
            .repo
            .find_reference(NOTES)
            .ok()
            .map(|reference| ok(reference.peel_to_commit()));
        let previous_tree = previous.as_ref().map(|commit| ok(commit.tree()));
        let mut builder = ok(self.repo.treebuilder(previous_tree.as_ref()));
        let blob = ok(self.repo.blob(bytes));
        let id = commit.to_string();
        if fanout {
            let (prefix, suffix) = id.split_at(2);
            let mut child = ok(self.repo.treebuilder(None));
            ok(child.insert(suffix, blob, 0o100644));
            let child_id = ok(child.write());
            ok(builder.insert(prefix, child_id, 0o040000));
        } else {
            ok(builder.insert(&id, blob, 0o100644));
        }
        let tree = ok(self.repo.find_tree(ok(builder.write())));
        let signature = ok(Signature::new(
            "Fixture",
            "fixture@example.invalid",
            &Time::new(1_600_000_000, 0),
        ));
        let parents: Vec<_> = previous.iter().collect();
        ok(self.repo.commit(
            Some(NOTES),
            &signature,
            &signature,
            "pointer note",
            &tree,
            &parents,
        ));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn traces(
    observation: &RevisionSessionTraces,
) -> &[kiln_core::change_review::SessionTraceIdentity] {
    let SessionTraceStatus::Attached { traces } = &observation.observation else {
        unreachable!("expected attached pointers: {observation:?}")
    };
    traces
}
fn assert_unavailable(value: RevisionSessionTraces, reason: SessionTraceUnavailable) {
    assert_eq!(
        value.observation,
        SessionTraceStatus::Unavailable { reason }
    );
    assert!(value.note_digest.is_none());
    assert!(value.validate_for(&value.commit).is_ok());
}

// r[verify kiln.review.session_traces.scenario.notes]
#[test]
fn exact_local_notes_are_bound_distinct_deduplicated_and_never_body_or_link_evidence() {
    let f = Fixture::new();
    let provider = f.provider(true);
    let first = f.first.to_string();
    let second = f.second.to_string();
    let note = b"traces:session-1 https://TRACES.Example/private-session?secret=not-retained#body\ntraces:session-2\ntraces:session-2\n";
    f.note(f.first, note, false);
    f.note(f.second, note, false);
    let before_ref = ok(f.repo.refname_to_id(NOTES));
    let first_observation = ok(provider.session_traces(&first));
    let second_observation = ok(provider.session_traces(&second));
    assert!(first_observation.validate_for(&first).is_ok());
    assert!(first_observation.validate_for(&second).is_err());
    assert_eq!(
        first_observation.note_digest,
        Some(ok(kiln_codec::identity_from_bytes(note)))
    );
    assert_eq!(traces(&first_observation).len(), 2);
    let first_trace = traces(&first_observation)
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(first_trace.external_id, "session-1");
    assert_eq!(
        first_trace.shared_url_host.as_deref(),
        Some("traces.example")
    );
    assert_ne!(
        first_trace.identity,
        traces(&second_observation)
            .first()
            .unwrap_or_else(|| unreachable!())
            .identity
    );
    let serialized = ok(kiln_codec::encode_packed(
        &first_observation,
        MAX_SESSION_TRACE_NOTE_BYTES,
    ));
    for forbidden in [
        "private-session",
        "secret",
        "not-retained",
        "https://",
        "body",
    ] {
        assert!(
            !serialized
                .windows(forbidden.len())
                .any(|window| window == forbidden.as_bytes())
        );
    }
    assert_eq!(ok(f.repo.refname_to_id(NOTES)), before_ref);
    let captured = first_observation.clone();
    f.note(
        f.first,
        b"traces:session-1 https://traces.example/changed\n",
        false,
    );
    let changed = ok(provider.session_traces(&first));
    assert_ne!(captured.note_digest, changed.note_digest);
    assert_ne!(traces(&captured).first(), traces(&changed).first());
    let mut wrong_digest = captured.clone();
    wrong_digest.note_digest = changed.note_digest;
    assert!(wrong_digest.validate_for(&first).is_err());
    assert_eq!(captured, first_observation);
}

#[test]
fn absent_note_is_not_malformed_and_unrelated_notes_do_not_attach() {
    let f = Fixture::new();
    let provider = f.provider(true);
    let first = f.first.to_string();
    let absent = ok(provider.session_traces(&first));
    assert_eq!(absent.observation, SessionTraceStatus::Absent);
    assert!(absent.note_digest.is_none());
    f.note(f.second, b"traces:other-session\n", false);
    assert_eq!(ok(provider.session_traces(&first)), absent);
    f.note(f.first, b"traces:fanout-session\n", true);
    assert_eq!(traces(&ok(provider.session_traces(&first))).len(), 1);
}

#[test]
fn malformed_notes_credentials_urls_and_raw_bodies_never_become_absence() {
    let f = Fixture::new();
    let provider = f.provider(true);
    let first = f.first.to_string();
    for note in [
        b"".as_slice(),
        b" \t\n",
        b"raw session body",
        b"traces:",
        b"\xff",
        b"traces:id\n\ntraces:second",
        b"traces:id extra tokens here",
        b"traces:id http://traces.example/session",
        b"traces:id javascript:alert(1)",
        b"traces:id https://user:secret@traces.example/session",
        b"traces:id https://@traces.example/session",
        b"traces:id https:///traces.example/session",
        b"traces:id https://traces.example\\session",
        b"traces:https://traces.example/session",
        b"traces:user:secret@example",
        b"traces:id https://traces.example/a\ntraces:id https://other.example/b",
    ] {
        f.note(f.first, note, false);
        assert_unavailable(
            ok(provider.session_traces(&first)),
            SessionTraceUnavailable::Malformed,
        );
    }
}

#[test]
fn oversized_and_too_many_notes_are_explicit_and_bounded() {
    let f = Fixture::new();
    let provider = f.provider(true);
    let first = f.first.to_string();
    f.note(
        f.first,
        &vec![b'x'; MAX_SESSION_TRACE_NOTE_BYTES + 1],
        false,
    );
    assert_unavailable(
        ok(provider.session_traces(&first)),
        SessionTraceUnavailable::Oversized,
    );
    let note: String = (0..=MAX_SESSION_TRACES)
        .map(|index| format!("traces:session-{index}\n"))
        .collect();
    f.note(f.first, note.as_bytes(), false);
    assert_unavailable(
        ok(provider.session_traces(&first)),
        SessionTraceUnavailable::TooMany,
    );
    let repeated = "traces:same\n".repeat(MAX_SESSION_TRACES + 1);
    f.note(f.first, repeated.as_bytes(), false);
    assert_eq!(traces(&ok(provider.session_traces(&first))).len(), 1);
}

#[test]
fn inspect_permission_and_independent_exact_commit_resolution_are_required() {
    let f = Fixture::new();
    assert_eq!(
        f.provider(false)
            .session_traces(&f.first.to_string())
            .err()
            .map(|error| error.kind),
        Some(ReviewErrorKind::Unsupported)
    );
    let provider = f.provider(true);
    for invalid in ["HEAD", "refs/heads/main", "abcd"] {
        assert_eq!(
            provider
                .session_traces(invalid)
                .err()
                .map(|error| error.kind),
            Some(ReviewErrorKind::InvalidRequest)
        );
    }
    assert_eq!(
        provider
            .session_traces(&"f".repeat(40))
            .err()
            .map(|error| error.kind),
        Some(ReviewErrorKind::NotFound)
    );
    let blob = ok(f.repo.blob(b"not a commit"));
    assert_eq!(
        provider
            .session_traces(&blob.to_string())
            .err()
            .map(|error| error.kind),
        Some(ReviewErrorKind::NotFound)
    );
}

#[test]
fn provider_storage_failure_is_not_reported_as_a_missing_note() {
    let f = Fixture::new();
    let blob = ok(f.repo.blob(b"not a notes commit"));
    ok(f.repo
        .reference(NOTES, blob, true, "malformed notes ref fixture"));
    assert_eq!(
        f.provider(true)
            .session_traces(&f.first.to_string())
            .err()
            .map(|error| error.kind),
        Some(ReviewErrorKind::Storage)
    );
}

/// Existing providers need not implement the new optional read method.
struct LegacyProvider(LocalGitReviewProvider);
impl ChangeReviewPort for LegacyProvider {
    fn info(&self) -> kiln_provider_api::change_review::ReviewProviderInfo {
        self.0.info()
    }
    fn inspect(
        &self,
        commit: &str,
        target: &str,
    ) -> kiln_provider_api::change_review::ReviewResult<
        kiln_provider_api::change_review::CommitInspection,
    > {
        self.0.inspect(commit, target)
    }
    fn compare(
        &self,
        request: &kiln_provider_api::change_review::CompareRequest,
    ) -> kiln_provider_api::change_review::ReviewResult<
        kiln_provider_api::change_review::CommitComparison,
    > {
        self.0.compare(request)
    }
    fn execute(
        &self,
        request: &kiln_provider_api::change_review::ReviewOperationRequest,
    ) -> kiln_provider_api::change_review::ReviewResult<
        kiln_provider_api::change_review::ReviewEffect,
    > {
        self.0.execute(request)
    }
    fn reconcile(
        &self,
        request: &kiln_provider_api::change_review::ReviewOperationRequest,
    ) -> kiln_provider_api::change_review::ReviewResult<
        kiln_provider_api::change_review::ReviewEffect,
    > {
        self.0.reconcile(request)
    }
}

#[test]
fn optional_port_default_is_an_explicit_unsupported_error() {
    let f = Fixture::new();
    f.note(f.first, b"traces:session\n", false);
    let legacy = LegacyProvider(f.provider(true));
    assert_eq!(
        legacy
            .session_traces(&f.first.to_string())
            .err()
            .map(|error| error.kind),
        Some(ReviewErrorKind::Unsupported)
    );
}
