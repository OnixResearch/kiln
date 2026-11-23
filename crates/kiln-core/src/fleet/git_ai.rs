//! Bounded, pure parser and rewriter for Git AI Authorship Log v3.0.0 notes
//! (`refs/notes/ai`, schema `authorship/3.0.0`).
//!
//! Preservation is observation-only metadata attached after rematerialization
//! when the exported tree equals the agent commit's tree. Presence, absence,
//! or incompatibility of a Git AI note never grants verification, ownership,
//! or publication authority.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Standard Git notes reference for Git AI authorship logs.
pub const GIT_AI_NOTES_REF: &str = "refs/notes/ai";
/// Required `schema_version` in the JSON metadata section.
pub const GIT_AI_SCHEMA_VERSION: &str = "authorship/3.0.0";
/// Maximum raw byte length of a single Git AI note read from an untrusted repo.
pub const MAX_GIT_AI_NOTE_BYTES: usize = 256 * 1024;
pub const MAX_GIT_AI_FILES: usize = 4096;
pub const MAX_GIT_AI_ENTRIES_PER_FILE: usize = 1024;
pub const MAX_GIT_AI_RANGES_PER_ENTRY: usize = 4096;
pub const MAX_GIT_AI_PATH_BYTES: usize = 1024;
pub const MAX_GIT_AI_HASH_BYTES: usize = 128;
const GIT_SHA1_HEX_LEN: usize = 40;
const GIT_SHA256_HEX_LEN: usize = 64;

/// A single 1-based line number or inclusive line span (`start-end`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitAiLineRange {
    Single(u32),
    Span { start: u32, end: u32 },
}

impl GitAiLineRange {
    #[must_use]
    pub fn render(self) -> String {
        match self {
            Self::Single(line) => format!("{line}"),
            Self::Span { start, end } => format!("{start}-{end}"),
        }
    }
}

/// One attestation entry under a file path: `<prompt_hash> <ranges>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitAiEntry {
    pub hash: String,
    pub ranges: Vec<GitAiLineRange>,
}

/// All attestation entries for one repository-relative file path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitAiFileAttestation {
    pub path: String,
    pub entries: Vec<GitAiEntry>,
}

/// Parsed, validated `authorship/3.0.0` log bound to one exact commit SHA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitAiAuthorshipLog {
    pub attestations: Vec<GitAiFileAttestation>,
    pub schema_version: String,
    pub base_commit_sha: String,
    pub metadata: Map<String, Value>,
}

impl GitAiAuthorshipLog {
    /// Total number of attestation entries across all files.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.attestations
            .iter()
            .map(|file| file.entries.len())
            .sum()
    }
}

/// Observation-only outcome of optional Git AI note preservation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GitAiNoteOutcome {
    Disabled,
    Absent,
    Preserved {
        source_commit: String,
        candidate_commit: String,
        files: u32,
        entries: u32,
    },
    Incompatible {
        reason: String,
    },
}

/// Why an untrusted Git AI note was refused as incompatible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitAiNoteError {
    pub reason: String,
}

impl GitAiNoteError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Parse and validate an untrusted Git AI `authorship/3.0.0` note bound to
/// `expected_commit`.
///
/// # Errors
/// Refuses oversized, non-UTF-8, NUL-bearing, malformed, wrong-schema, or
/// differently-bound notes.
pub fn parse_git_ai_note(
    bytes: &[u8],
    expected_commit: &str,
) -> Result<GitAiAuthorshipLog, GitAiNoteError> {
    if !is_commit_sha(expected_commit) {
        return Err(GitAiNoteError::new(
            "expected commit is not a lowercase hexadecimal commit SHA",
        ));
    }
    if bytes.is_empty() {
        return Err(GitAiNoteError::new("git ai note is empty"));
    }
    if bytes.len() > MAX_GIT_AI_NOTE_BYTES {
        return Err(GitAiNoteError::new(format!(
            "git ai note exceeds maximum size of {MAX_GIT_AI_NOTE_BYTES} bytes"
        )));
    }
    if bytes.contains(&0) {
        return Err(GitAiNoteError::new("git ai note contains NUL bytes"));
    }
    let text = core::str::from_utf8(bytes)
        .map_err(|_| GitAiNoteError::new("git ai note is not valid UTF-8"))?;

    let (attestation_text, metadata_text) = split_sections(text)?;
    let attestations = parse_attestations(attestation_text)?;

    let metadata_value: Value = serde_json::from_str(metadata_text)
        .map_err(|_| GitAiNoteError::new("git ai note metadata is not valid JSON"))?;
    let metadata = match metadata_value {
        Value::Object(map) => map,
        _ => {
            return Err(GitAiNoteError::new(
                "git ai note metadata must be a JSON object",
            ));
        }
    };
    let schema_version = metadata
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| GitAiNoteError::new("git ai note metadata lacks schema_version"))?;
    if schema_version != GIT_AI_SCHEMA_VERSION {
        return Err(GitAiNoteError::new(format!(
            "unsupported git ai note schema_version {schema_version:?}; expected {GIT_AI_SCHEMA_VERSION:?}"
        )));
    }
    let base_commit_sha = metadata
        .get("base_commit_sha")
        .and_then(Value::as_str)
        .ok_or_else(|| GitAiNoteError::new("git ai note metadata lacks base_commit_sha"))?;
    if !is_commit_sha(base_commit_sha) {
        return Err(GitAiNoteError::new(
            "git ai note base_commit_sha is not a valid commit SHA",
        ));
    }
    if base_commit_sha != expected_commit {
        return Err(GitAiNoteError::new(format!(
            "git ai note base_commit_sha {base_commit_sha} does not match agent commit {expected_commit}"
        )));
    }
    if let Some(commit_sha) = metadata.get("commit_sha") {
        let Some(commit_sha) = commit_sha.as_str() else {
            return Err(GitAiNoteError::new(
                "git ai note commit_sha must be a string",
            ));
        };
        if commit_sha != expected_commit {
            return Err(GitAiNoteError::new(format!(
                "git ai note commit_sha {commit_sha} does not match agent commit {expected_commit}"
            )));
        }
    }
    if let Some(prompts) = metadata.get("prompts")
        && !prompts.is_object()
    {
        return Err(GitAiNoteError::new(
            "git ai note prompts field must be a JSON object",
        ));
    }

    Ok(GitAiAuthorshipLog {
        attestations,
        schema_version: schema_version.to_string(),
        base_commit_sha: base_commit_sha.to_string(),
        metadata,
    })
}

/// Rewrite a validated Git AI authorship log onto `candidate_commit`, preserving
/// exact file line ranges while updating commit metadata (`base_commit_sha`,
/// and `commit_sha` if present).
///
/// # Errors
/// Refuses a malformed `candidate_commit` or an oversized rewritten note.
pub fn rewrite_git_ai_note(
    log: &GitAiAuthorshipLog,
    candidate_commit: &str,
) -> Result<String, GitAiNoteError> {
    if !is_commit_sha(candidate_commit) {
        return Err(GitAiNoteError::new(
            "candidate commit is not a lowercase hexadecimal commit SHA",
        ));
    }
    let mut metadata = log.metadata.clone();
    metadata.insert(
        "schema_version".to_string(),
        Value::String(GIT_AI_SCHEMA_VERSION.to_string()),
    );
    metadata.insert(
        "base_commit_sha".to_string(),
        Value::String(candidate_commit.to_string()),
    );
    if metadata.contains_key("commit_sha") {
        metadata.insert(
            "commit_sha".to_string(),
            Value::String(candidate_commit.to_string()),
        );
    }
    let metadata_json = serde_json::to_string_pretty(&Value::Object(metadata))
        .map_err(|_| GitAiNoteError::new("failed to serialize rewritten git ai metadata"))?;

    let mut out = String::new();
    for file in &log.attestations {
        out.push_str(&file.path);
        out.push('\n');
        for entry in &file.entries {
            out.push_str("  ");
            out.push_str(&entry.hash);
            out.push(' ');
            for (index, range) in entry.ranges.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&range.render());
            }
            out.push('\n');
        }
    }
    out.push_str("---\n");
    out.push_str(&metadata_json);
    out.push('\n');
    if out.len() > MAX_GIT_AI_NOTE_BYTES {
        return Err(GitAiNoteError::new(format!(
            "rewritten git ai note exceeds maximum size of {MAX_GIT_AI_NOTE_BYTES} bytes"
        )));
    }
    Ok(out)
}

fn split_sections(text: &str) -> Result<(&str, &str), GitAiNoteError> {
    let mut offset: usize = 0;
    let mut divider_start: Option<usize> = None;
    let mut divider_end: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let stripped = line.trim_end_matches(['\r', '\n']);
        if stripped == "---" {
            divider_start = Some(offset);
            divider_end = Some(offset + line.len());
            break;
        }
        offset += line.len();
    }
    let (Some(start), Some(end)) = (divider_start, divider_end) else {
        return Err(GitAiNoteError::new(
            "git ai note is missing the --- section divider",
        ));
    };
    let head = text.get(..start).unwrap_or_default();
    let tail = text.get(end..).unwrap_or_default().trim();
    if tail.is_empty() {
        return Err(GitAiNoteError::new("git ai note metadata section is empty"));
    }
    Ok((head, tail))
}

fn parse_attestations(section: &str) -> Result<Vec<GitAiFileAttestation>, GitAiNoteError> {
    let mut files: Vec<GitAiFileAttestation> = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for raw_line in section.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("  ") {
            let Some(current) = files.last_mut() else {
                return Err(GitAiNoteError::new(
                    "git ai attestation entry precedes any file path",
                ));
            };
            if current.entries.len() >= MAX_GIT_AI_ENTRIES_PER_FILE {
                return Err(GitAiNoteError::new(
                    "git ai file exceeds maximum attestation entries",
                ));
            }
            let entry = parse_entry(rest)?;
            current.entries.push(entry);
        } else {
            if line.starts_with(char::is_whitespace) {
                return Err(GitAiNoteError::new(
                    "git ai attestation line has invalid indentation",
                ));
            }
            if files.len() >= MAX_GIT_AI_FILES {
                return Err(GitAiNoteError::new("git ai note exceeds maximum files"));
            }
            if !valid_relative_path(line) {
                return Err(GitAiNoteError::new(format!(
                    "git ai note has invalid file path {line:?}"
                )));
            }
            if !seen_paths.insert(line.to_string()) {
                return Err(GitAiNoteError::new(format!(
                    "git ai note repeats file path {line:?}"
                )));
            }
            files.push(GitAiFileAttestation {
                path: line.to_string(),
                entries: Vec::new(),
            });
        }
    }
    if files.iter().any(|file| file.entries.is_empty()) {
        return Err(GitAiNoteError::new(
            "git ai file attestation has no line entries",
        ));
    }
    Ok(files)
}

fn parse_entry(text: &str) -> Result<GitAiEntry, GitAiNoteError> {
    let Some((hash, ranges_text)) = text.split_once(' ') else {
        return Err(GitAiNoteError::new(
            "git ai attestation entry must be '<hash> <ranges>'",
        ));
    };
    if hash.is_empty()
        || hash.len() > MAX_GIT_AI_HASH_BYTES
        || !hash.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GitAiNoteError::new("git ai attestation hash is malformed"));
    }
    let ranges_text = ranges_text.trim();
    if ranges_text.is_empty() {
        return Err(GitAiNoteError::new(
            "git ai attestation entry has no line ranges",
        ));
    }
    let mut ranges = Vec::new();
    for token in ranges_text.split(',') {
        if ranges.len() >= MAX_GIT_AI_RANGES_PER_ENTRY {
            return Err(GitAiNoteError::new(
                "git ai attestation entry exceeds maximum line ranges",
            ));
        }
        let range = if let Some((start, end)) = token.split_once('-') {
            let start = parse_positive_line(start)?;
            let end = parse_positive_line(end)?;
            if start > end {
                return Err(GitAiNoteError::new("git ai line range start exceeds end"));
            }
            GitAiLineRange::Span { start, end }
        } else {
            GitAiLineRange::Single(parse_positive_line(token)?)
        };
        ranges.push(range);
    }
    Ok(GitAiEntry {
        hash: hash.to_string(),
        ranges,
    })
}

fn parse_positive_line(token: &str) -> Result<u32, GitAiNoteError> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(GitAiNoteError::new("git ai line number is malformed"));
    }
    let value = token
        .parse::<u32>()
        .map_err(|_| GitAiNoteError::new("git ai line number overflowed"))?;
    if value == 0 {
        return Err(GitAiNoteError::new("git ai line numbers must be 1-based"));
    }
    Ok(value)
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_GIT_AI_PATH_BYTES
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_commit_sha(value: &str) -> bool {
    matches!(value.len(), GIT_SHA1_HEX_LEN | GIT_SHA256_HEX_LEN)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    const AGENT_COMMIT: &str = "1111111111111111111111111111111111111111";
    const CANDIDATE_COMMIT: &str = "2222222222222222222222222222222222222222";

    fn sample_note(base_commit: &str, schema: &str) -> String {
        format!(
            "src/lib.rs\n  a1b2c3d4 1-5,9,12-20\n  deadbeef 25\nREADME.md\n  a1b2c3d4 3-4\n---\n{{\n  \"schema_version\": \"{schema}\",\n  \"base_commit_sha\": \"{base_commit}\",\n  \"prompts\": {{\n    \"a1b2c3d4\": {{ \"agent_id\": {{ \"tool\": \"claude\", \"id\": \"s1\", \"model\": \"claude-opus\" }} }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn valid_authorship_v3_note_parses_and_rewrites_commit_metadata() {
        let raw = sample_note(AGENT_COMMIT, GIT_AI_SCHEMA_VERSION);
        let parsed = parse_git_ai_note(raw.as_bytes(), AGENT_COMMIT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(parsed.attestations.len(), 2);
        assert_eq!(parsed.entry_count(), 3);
        assert_eq!(parsed.base_commit_sha, AGENT_COMMIT);

        let rewritten = rewrite_git_ai_note(&parsed, CANDIDATE_COMMIT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let reparsed = parse_git_ai_note(rewritten.as_bytes(), CANDIDATE_COMMIT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(reparsed.attestations, parsed.attestations);
        assert_eq!(reparsed.base_commit_sha, CANDIDATE_COMMIT);
        assert_eq!(reparsed.schema_version, GIT_AI_SCHEMA_VERSION);
    }

    #[test]
    fn oversized_wrong_schema_wrong_commit_and_malformed_notes_are_refused() {
        let wrong_schema = sample_note(AGENT_COMMIT, "authorship/2.0.0");
        assert!(parse_git_ai_note(wrong_schema.as_bytes(), AGENT_COMMIT).is_err());

        let wrong_commit = sample_note(CANDIDATE_COMMIT, GIT_AI_SCHEMA_VERSION);
        assert!(parse_git_ai_note(wrong_commit.as_bytes(), AGENT_COMMIT).is_err());

        let oversized = vec![b'a'; MAX_GIT_AI_NOTE_BYTES + 1];
        assert!(parse_git_ai_note(&oversized, AGENT_COMMIT).is_err());

        for bad in [
            "",
            "no divider here",
            "src/lib.rs\n---\nnot-json",
            "../escape.rs\n  abc 1\n---\n{\"schema_version\":\"authorship/3.0.0\",\"base_commit_sha\":\"1111111111111111111111111111111111111111\"}",
            "src/lib.rs\n  abc 10-5\n---\n{\"schema_version\":\"authorship/3.0.0\",\"base_commit_sha\":\"1111111111111111111111111111111111111111\"}",
            "src/lib.rs\n  abc 0\n---\n{\"schema_version\":\"authorship/3.0.0\",\"base_commit_sha\":\"1111111111111111111111111111111111111111\"}",
            "  orphan 1\n---\n{\"schema_version\":\"authorship/3.0.0\",\"base_commit_sha\":\"1111111111111111111111111111111111111111\"}",
        ] {
            assert!(
                parse_git_ai_note(bad.as_bytes(), AGENT_COMMIT).is_err(),
                "{bad}"
            );
        }
    }
}
