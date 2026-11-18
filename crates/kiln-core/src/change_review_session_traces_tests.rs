use super::*;
use crate::change_review::{Authorship, Revision};
use alloc::string::ToString;
use alloc::{format, vec};

fn digest(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64))
        .unwrap_or_else(|error| unreachable!("fixture: {error:?}"))
}
fn attached(commit: &str) -> RevisionSessionTraces {
    let note_digest = digest('a');
    let trace = SessionTraceIdentity::new(
        commit,
        &note_digest,
        "session-1".into(),
        Some("traces.example".into()),
    )
    .unwrap_or_else(|error| unreachable!("fixture: {error}"));
    RevisionSessionTraces {
        commit: commit.into(),
        note_digest: Some(note_digest),
        observation: SessionTraceStatus::Attached {
            traces: vec![trace],
        },
    }
}

#[test]
fn identities_frame_commit_digest_id_and_host_without_source_body_claims() {
    let first = attached("generic-commit");
    assert!(first.validate_for("generic-commit").is_ok());
    assert_eq!(first, attached("generic-commit"));
    let SessionTraceStatus::Attached { traces } = &first.observation else {
        unreachable!()
    };
    let trace = traces.first().unwrap_or_else(|| unreachable!());
    for (commit, note, id, host) in [
        (
            "other-commit",
            digest('a'),
            "session-1",
            Some("traces.example"),
        ),
        (
            "generic-commit",
            digest('b'),
            "session-1",
            Some("traces.example"),
        ),
        (
            "generic-commit",
            digest('a'),
            "session-2",
            Some("traces.example"),
        ),
        ("generic-commit", digest('a'), "session-1", None),
        (
            "generic-commit",
            digest('a'),
            "session-1",
            Some("other.example"),
        ),
    ] {
        let other = SessionTraceIdentity::new(commit, &note, id.into(), host.map(Into::into))
            .unwrap_or_else(|error| unreachable!("fixture: {error}"));
        assert_ne!(trace.identity, other.identity);
    }
    let mut rebound = first.clone();
    rebound.commit = "other-commit".into();
    assert!(rebound.validate_for("other-commit").is_err());
    rebound = first.clone();
    rebound.note_digest = Some(digest('b'));
    assert!(rebound.validate_for("generic-commit").is_err());
    assert!(first.validate_for("other-commit").is_err());
    rebound = first.clone();
    let SessionTraceStatus::Attached { traces } = &mut rebound.observation else {
        unreachable!()
    };
    traces
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .shared_url_host = Some("other.example".into());
    assert!(rebound.validate_for("generic-commit").is_err());
}

#[test]
fn status_digest_bounds_and_unique_ids_are_strict() {
    let valid = attached("commit");
    for observation in [
        SessionTraceStatus::Absent,
        SessionTraceStatus::Unavailable {
            reason: SessionTraceUnavailable::Malformed,
        },
    ] {
        let mut candidate = valid.clone();
        candidate.observation = observation;
        assert!(candidate.validate_for("commit").is_err());
        candidate.note_digest = None;
        assert!(candidate.validate_for("commit").is_ok());
    }
    let mut candidate = valid.clone();
    candidate.note_digest = None;
    assert!(candidate.validate_for("commit").is_err());
    let SessionTraceStatus::Attached { traces } = valid.observation else {
        unreachable!()
    };
    let trace = traces.first().unwrap_or_else(|| unreachable!());
    for count in [0, 2, MAX_SESSION_TRACES + 1] {
        candidate = attached("commit");
        candidate.observation = SessionTraceStatus::Attached {
            traces: vec![trace.clone(); count],
        };
        assert!(candidate.validate_for("commit").is_err());
    }
    let note = digest('a');
    let traces = (0..MAX_SESSION_TRACES)
        .map(|index| {
            SessionTraceIdentity::new("commit", &note, format!("session-{index}"), None)
                .unwrap_or_else(|error| unreachable!("fixture: {error}"))
        })
        .collect();
    candidate.observation = SessionTraceStatus::Attached { traces };
    assert!(candidate.validate_for("commit").is_ok());
    for id in [
        "".into(),
        " ".into(),
        "https://traces.example/session".into(),
        "user:secret@example".into(),
        "id\nbody".into(),
        "x".repeat(MAX_SESSION_TRACE_TEXT_BYTES + 1),
    ] {
        assert!(SessionTraceIdentity::new("commit", &note, id, None).is_err());
    }
    for host in [
        "https://traces.example",
        "user@example",
        "Traces.Example",
        "example/path",
        "x:y",
        "-host",
    ] {
        assert!(
            SessionTraceIdentity::new("commit", &note, "id".into(), Some(host.into())).is_err()
        );
    }
    assert!(SessionTraceIdentity::new("", &note, "id".into(), None).is_err());
    assert!(
        SessionTraceIdentity::new(
            &"x".repeat(MAX_SESSION_TRACE_TEXT_BYTES + 1),
            &note,
            "id".into(),
            None
        )
        .is_err()
    );
    assert!(
        SessionTraceIdentity::new(
            "commit",
            &note,
            "x".repeat(MAX_SESSION_TRACE_TEXT_BYTES),
            None
        )
        .is_ok()
    );
}

// r[verify kiln.review.session_traces]
#[test]
fn legacy_revision_json_replays_and_serializes_byte_identically() {
    const LEGACY: &str = r#"{"id":"r1","number":1,"commit":"commit","base":"base","files":[],"author":"alice","created_ms":0}"#;
    let revision: Revision =
        serde_json::from_str(LEGACY).unwrap_or_else(|error| unreachable!("fixture: {error}"));
    assert_eq!(revision.authorship, Authorship::Undeclared);
    assert!(revision.session_traces.is_none());
    assert_eq!(
        serde_json::to_string(&revision).ok().as_deref(),
        Some(LEGACY)
    );
    let mut with_trace = revision;
    with_trace.session_traces = Some(attached("commit"));
    let bytes =
        serde_json::to_vec(&with_trace).unwrap_or_else(|error| unreachable!("fixture: {error}"));
    assert_eq!(
        serde_json::from_slice::<Revision>(&bytes).ok(),
        Some(with_trace)
    );
}
