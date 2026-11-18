//! Untrusted pointer observations bound to one exact revision and raw note digest.
//! r[impl kiln.review.session_traces]
use super::{ReviewResult, fail};
use crate::ContentId;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const MAX_SESSION_TRACE_NOTE_BYTES: usize = 16 * 1024;
pub const MAX_SESSION_TRACES: usize = 8;
pub const MAX_SESSION_TRACE_TEXT_BYTES: usize = 256;
/// Maximum DNS label size, independent of the retained host text bound.
const MAX_HOST_LABEL_BYTES: usize = 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTraceSourceKind {
    GitNotesTraces,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionTraceIdentity {
    pub identity: ContentId,
    pub source_kind: SessionTraceSourceKind,
    pub external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_url_host: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTraceUnavailable {
    Unsupported,
    Malformed,
    Oversized,
    TooMany,
    ProviderUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SessionTraceStatus {
    Absent,
    Attached { traces: Vec<SessionTraceIdentity> },
    Unavailable { reason: SessionTraceUnavailable },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RevisionSessionTraces {
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_digest: Option<ContentId>,
    #[serde(flatten)]
    pub observation: SessionTraceStatus,
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SESSION_TRACE_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn metadata_valid(external_id: &str, host: Option<&str>) -> bool {
    bounded_text(external_id)
        && external_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && host.is_none_or(normalized_host_valid)
}

fn normalized_host_valid(host: &str) -> bool {
    if !bounded_text(host) {
        return false;
    }
    if let Some(address) = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
    {
        return !address.bytes().any(|byte| byte.is_ascii_uppercase())
            && address.parse::<core::net::Ipv6Addr>().is_ok();
    }
    host.strip_suffix('.')
        .unwrap_or(host)
        .split('.')
        .all(|label| {
            !label.is_empty()
                && label.len() <= MAX_HOST_LABEL_BYTES
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn trace_identity(
    commit: &str,
    note_digest: &ContentId,
    external_id: &str,
    host: Option<&str>,
) -> ReviewResult<blake3::Hash> {
    if !bounded_text(commit)
        || note_digest.validate().is_err()
        || !metadata_valid(external_id, host)
    {
        return fail(
            "invalid_session_traces",
            "session trace binding and metadata must be bounded valid pointers",
        );
    }
    let mut hasher = blake3::Hasher::new();
    for field in [
        "kiln.review.session-trace.v1",
        commit,
        note_digest.as_str(),
        "git_notes_traces",
        external_id,
    ] {
        hash_field(&mut hasher, field.as_bytes());
    }
    hash_field(&mut hasher, &[u8::from(host.is_some())]);
    if let Some(host) = host {
        hash_field(&mut hasher, host.as_bytes());
    }
    Ok(hasher.finalize())
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

impl SessionTraceIdentity {
    pub fn new(
        commit: &str,
        note_digest: &ContentId,
        external_id: String,
        shared_url_host: Option<String>,
    ) -> ReviewResult<Self> {
        let hash = trace_identity(
            commit,
            note_digest,
            &external_id,
            shared_url_host.as_deref(),
        )?;
        let identity =
            ContentId::from_blake3_hex(hash.to_hex().as_str()).map_err(|_| super::ReviewError {
                code: "invalid_session_traces".into(),
                message: "invalid session trace identity".into(),
            })?;
        Ok(Self {
            identity,
            source_kind: SessionTraceSourceKind::GitNotesTraces,
            external_id,
            shared_url_host,
        })
    }
}

impl RevisionSessionTraces {
    pub fn validate_for(&self, commit: &str) -> ReviewResult<()> {
        if self.commit != commit || !bounded_text(commit) {
            return fail(
                "invalid_session_traces",
                "session trace observation must name the exact revision",
            );
        }
        match (&self.observation, &self.note_digest) {
            (SessionTraceStatus::Attached { traces }, Some(digest))
                if !traces.is_empty() && traces.len() <= MAX_SESSION_TRACES =>
            {
                for (index, trace) in traces.iter().enumerate() {
                    if traces
                        .iter()
                        .take(index)
                        .any(|other| other.external_id == trace.external_id)
                        || trace.identity.as_str()
                            != trace_identity(
                                commit,
                                digest,
                                &trace.external_id,
                                trace.shared_url_host.as_deref(),
                            )?
                            .to_hex()
                            .as_str()
                    {
                        return fail(
                            "invalid_session_traces",
                            "session trace identities must be unique and exactly bound",
                        );
                    }
                }
                Ok(())
            }
            (SessionTraceStatus::Absent | SessionTraceStatus::Unavailable { .. }, None) => Ok(()),
            _ => fail(
                "invalid_session_traces",
                "session trace status and note digest are inconsistent",
            ),
        }
    }
}

#[cfg(test)]
#[path = "change_review_session_traces_tests.rs"]
mod tests;
