//! Bounded local Git-note pointers. No session body or URL beyond its host is retained.
//! r[impl kiln.review.session_traces.scenario.notes]
use super::{failure, storage};
use git2::{ObjectType, Repository};
use kiln_core::change_review::{
    MAX_SESSION_TRACE_NOTE_BYTES, MAX_SESSION_TRACE_TEXT_BYTES, MAX_SESSION_TRACES,
    RevisionSessionTraces, SessionTraceIdentity, SessionTraceStatus, SessionTraceUnavailable,
};
use kiln_provider_api::change_review::{ReviewErrorKind, ReviewResult};

const TRACES_REF: &str = "refs/notes/traces";
const NOTE_FANOUT_HEX_BYTES: usize = 2;

fn unavailable(commit: &str, reason: SessionTraceUnavailable) -> RevisionSessionTraces {
    RevisionSessionTraces {
        commit: commit.into(),
        note_digest: None,
        observation: SessionTraceStatus::Unavailable { reason },
    }
}

fn parse_pointers(
    commit: &str,
    bytes: &[u8],
) -> Result<RevisionSessionTraces, SessionTraceUnavailable> {
    if bytes.len() > MAX_SESSION_TRACE_NOTE_BYTES {
        return Err(SessionTraceUnavailable::Oversized);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| SessionTraceUnavailable::Malformed)?;
    let digest =
        kiln_codec::identity_from_bytes(bytes).map_err(|_| SessionTraceUnavailable::Malformed)?;
    let mut traces: Vec<SessionTraceIdentity> = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_ascii_whitespace();
        let external_id = fields
            .next()
            .and_then(|pointer| pointer.strip_prefix("traces:"))
            .ok_or(SessionTraceUnavailable::Malformed)?;
        if external_id.len() > MAX_SESSION_TRACE_TEXT_BYTES {
            return Err(SessionTraceUnavailable::Malformed);
        }
        let host = fields
            .next()
            .map(|raw| {
                // Reject URL-parser repair, credential-bearing authorities and non-HTTPS.
                let (scheme, remainder) = raw
                    .split_once("://")
                    .ok_or(SessionTraceUnavailable::Malformed)?;
                if !scheme.eq_ignore_ascii_case("https")
                    || raw
                        .bytes()
                        .any(|byte| byte.is_ascii_control() || byte == b'\\')
                {
                    return Err(SessionTraceUnavailable::Malformed);
                }
                let url = url::Url::parse(raw).map_err(|_| SessionTraceUnavailable::Malformed)?;
                let authority = remainder
                    .split(['/', '?', '#'])
                    .next()
                    .ok_or(SessionTraceUnavailable::Malformed)?;
                if authority.is_empty()
                    || authority.contains('@')
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.scheme() != "https"
                {
                    return Err(SessionTraceUnavailable::Malformed);
                }
                url.host_str()
                    .map(str::to_owned)
                    .ok_or(SessionTraceUnavailable::Malformed)
            })
            .transpose()?;
        if fields.next().is_some() {
            return Err(SessionTraceUnavailable::Malformed);
        }
        if let Some(existing) = traces.iter().find(|other| other.external_id == external_id) {
            if existing.shared_url_host != host {
                return Err(SessionTraceUnavailable::Malformed);
            }
            continue;
        }
        if traces.len() == MAX_SESSION_TRACES {
            return Err(SessionTraceUnavailable::TooMany);
        }
        traces.push(
            SessionTraceIdentity::new(commit, &digest, external_id.into(), host)
                .map_err(|_| SessionTraceUnavailable::Malformed)?,
        );
    }
    if traces.is_empty() {
        return Err(SessionTraceUnavailable::Malformed);
    }
    Ok(RevisionSessionTraces {
        commit: commit.into(),
        note_digest: Some(digest),
        observation: SessionTraceStatus::Attached { traces },
    })
}

/// Resolve only the exact annotated OID in the local note tree. Inspect the blob
/// header before loading content; `find_note` would eagerly load an unbounded body.
pub(super) fn read(repo: &Repository, commit: &str) -> ReviewResult<RevisionSessionTraces> {
    let absent = || RevisionSessionTraces {
        commit: commit.into(),
        note_digest: None,
        observation: SessionTraceStatus::Absent,
    };
    let reference = match repo.find_reference(TRACES_REF) {
        Ok(reference) => reference,
        Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(absent()),
        Err(error) => return Err(storage(error)),
    };
    let mut tree = reference
        .peel_to_commit()
        .and_then(|note| note.tree())
        .map_err(storage)?;
    let mut remaining = commit;
    let note_id = loop {
        if let Some(entry) = tree.get_name(remaining) {
            if entry.kind() != Some(ObjectType::Blob) {
                return Ok(unavailable(commit, SessionTraceUnavailable::Malformed));
            }
            break entry.id();
        }
        let Some((prefix, suffix)) = remaining.split_at_checked(NOTE_FANOUT_HEX_BYTES) else {
            return Ok(absent());
        };
        if suffix.is_empty() {
            return Ok(absent());
        }
        let Some((id, kind)) = tree
            .get_name(prefix)
            .map(|entry| (entry.id(), entry.kind()))
        else {
            return Ok(absent());
        };
        if kind != Some(ObjectType::Tree) {
            return Ok(unavailable(commit, SessionTraceUnavailable::Malformed));
        }
        tree = repo.find_tree(id).map_err(storage)?;
        remaining = suffix;
    };
    let (size, kind) = repo
        .odb()
        .and_then(|odb| odb.read_header(note_id))
        .map_err(storage)?;
    if kind != ObjectType::Blob {
        return Err(failure(
            ReviewErrorKind::Storage,
            "trace note does not name a blob",
        ));
    }
    if size > MAX_SESSION_TRACE_NOTE_BYTES {
        return Ok(unavailable(commit, SessionTraceUnavailable::Oversized));
    }
    let blob = repo.find_blob(note_id).map_err(storage)?;
    Ok(parse_pointers(commit, blob.content()).unwrap_or_else(|reason| unavailable(commit, reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_parser_admits_exact_bounds_and_normalizes_only_hosts() {
        let mut note = b"traces:id".to_vec();
        note.resize(MAX_SESSION_TRACE_NOTE_BYTES, b' ');
        let observation = parse_pointers("commit", &note)
            .unwrap_or_else(|error| unreachable!("fixture: {error:?}"));
        assert!(observation.validate_for("commit").is_ok());
        let note: String = (0..MAX_SESSION_TRACES)
            .map(|index| format!("traces:id-{index} HTTPS://TRACES.Example/path?private=yes\n"))
            .collect();
        let observation = parse_pointers("commit", note.as_bytes())
            .unwrap_or_else(|error| unreachable!("fixture: {error:?}"));
        let SessionTraceStatus::Attached { traces } = observation.observation else {
            unreachable!()
        };
        assert_eq!(traces.len(), MAX_SESSION_TRACES);
        assert!(
            traces
                .iter()
                .all(|trace| trace.shared_url_host.as_deref() == Some("traces.example"))
        );
        assert!(parse_pointers("commit", b"traces:ipv6 https://[2001:db8::1]:443/path").is_ok());
    }

    #[test]
    fn pure_parser_rejects_metadata_bounds_and_pointer_bodies() {
        for note in [
            format!("traces:{}", "x".repeat(MAX_SESSION_TRACE_TEXT_BYTES + 1)),
            format!(
                "traces:id https://{}.example/path",
                "x".repeat(MAX_SESSION_TRACE_TEXT_BYTES + 1)
            ),
            "traces:id\nraw body with credentials".into(),
        ] {
            assert_eq!(
                parse_pointers("commit", note.as_bytes()),
                Err(SessionTraceUnavailable::Malformed)
            );
        }
        assert_eq!(
            parse_pointers("commit", &vec![b'x'; MAX_SESSION_TRACE_NOTE_BYTES + 1]),
            Err(SessionTraceUnavailable::Oversized)
        );
    }
}
