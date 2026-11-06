//! Bounded process access to the isolated upstream Radicle patch reducer.
//! r[impl kiln.radicle.patch_read]
//!
//! The helper isolates upstream's incompatible native SQLite binding. Reads
//! perform no mutation; review append requires explicit signer material and an
//! exact expected-head request. This port grants no Kiln review, CI or release authority.

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const MAX_HELPER_BYTES: usize = 1_048_576;
const HELPER_TIMEOUT_MS: u64 = 15_000;

fn invoke(command: &mut Command) -> Result<Value, String> {
    let output = run(RunRequest {
        command: CommandSpec {
            program: command.get_program().into(),
            args: command.get_args().map(Into::into).collect(),
            current_dir: Path::new("/").to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: command
                .get_envs()
                .filter_map(|(k, v)| v.map(|v| (k.into(), v.into())))
                .collect(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: HELPER_TIMEOUT_MS,
            stdin_max_bytes: 1,
            stdout_max_bytes: MAX_HELPER_BYTES,
            stderr_max_bytes: MAX_HELPER_BYTES,
            poll_interval_ms: 20,
            teardown_timeout_ms: 2_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|e| format!("{e:?}"))?,
    })
    .map_err(|e| e.to_string())?;
    if output.disposition != Disposition::Succeeded || !output.stderr.bytes.is_empty() {
        return Err("Radicle helper rejected request, timed out or exceeded output bound".into());
    }
    serde_json::from_slice(&output.stdout.bytes)
        .map_err(|_| "Radicle helper returned invalid JSON".into())
}

/// Exact review facts, separate from the borrowed signing authority.
#[derive(Debug)]
pub struct ReviewRequest<'a> {
    pub repository_id: &'a str,
    pub patch_id: &'a str,
    pub revision_id: &'a str,
    pub verdict: &'a str,
    pub summary: &'a str,
    pub expected_heads: &'a BTreeSet<String>,
}

/// Append through the isolated native helper; pass the seed only in its environment.
pub fn review_patch(
    helper: &Path,
    storage_root: &Path,
    request: ReviewRequest<'_>,
    seed: &[u8; 32],
) -> Result<Value, String> {
    let ReviewRequest {
        repository_id,
        patch_id,
        revision_id,
        verdict,
        summary,
        expected_heads,
    } = request;
    if repository_id.is_empty()
        || repository_id.len() > 256
        || patch_id.is_empty()
        || patch_id.len() > 256
        || revision_id.is_empty()
        || revision_id.len() > 256
        || !matches!(verdict, "accept" | "reject")
        || summary.is_empty()
        || summary.len() > 16_384
        || expected_heads.is_empty()
        || expected_heads.len() > 256
    {
        return Err("invalid Radicle review request".into());
    }
    let seed = seed
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    invoke(
        Command::new(helper)
            .arg("review")
            .arg(storage_root)
            .arg(repository_id)
            .arg(patch_id)
            .arg(revision_id)
            .arg(verdict)
            .arg(summary)
            .arg(serde_json::to_string(expected_heads).map_err(|_| "invalid head set")?)
            .env("RADICLE_SIGNER_SEED", seed),
    )
}

pub fn read_patch(
    helper: &Path,
    storage_root: &Path,
    repository_id: &str,
    patch_id: &str,
) -> Result<Value, String> {
    read_patch_with_limits(
        helper,
        storage_root,
        repository_id,
        patch_id,
        1024,
        16_777_216,
    )
}

pub fn read_issue(
    helper: &Path,
    storage_root: &Path,
    repository_id: &str,
    issue_id: &str,
) -> Result<Value, String> {
    invoke(
        Command::new(helper)
            .arg("read-issue")
            .arg(storage_root)
            .arg(repository_id)
            .arg(issue_id)
            .env("KILN_HISTORY_ENTRIES", "1024")
            .env("KILN_HISTORY_BYTES", "16777216"),
    )
}

pub fn read_patch_with_limits(
    helper: &Path,
    storage_root: &Path,
    repository_id: &str,
    patch_id: &str,
    entries: usize,
    bytes: usize,
) -> Result<Value, String> {
    if repository_id.is_empty()
        || repository_id.len() > 256
        || patch_id.is_empty()
        || patch_id.len() > 256
        || repository_id.contains('\0')
        || patch_id.contains('\0')
    {
        return Err("invalid Radicle patch identity".into());
    }
    invoke(
        Command::new(helper)
            .arg("read")
            .env("KILN_HISTORY_ENTRIES", entries.to_string())
            .env("KILN_HISTORY_BYTES", bytes.to_string())
            .arg(storage_root)
            .arg(repository_id)
            .arg(patch_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn helper(body: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
        let path = directory.path().join("helper");
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|error| unreachable!("{error}"));
        directory
    }

    #[test]
    fn unsupported_review_verdict_fails_before_child_effects() {
        let directory = helper("printf invoked > \"$0.called\"; printf '{}'");
        assert!(
            review_patch(
                &directory.path().join("helper"),
                Path::new("/store"),
                ReviewRequest {
                    repository_id: "zRepo",
                    patch_id: "abc",
                    revision_id: "rev",
                    verdict: "maybe",
                    summary: "looks good",
                    expected_heads: &BTreeSet::from(["a".into()]),
                },
                &[7; 32],
            )
            .is_err()
        );
        assert!(!directory.path().join("helper.called").exists());
    }

    #[test]
    fn helper_failure_and_noise_are_rejected() {
        let directory = helper("printf 'noise' >&2; exit 1");
        assert!(
            read_patch(
                &directory.path().join("helper"),
                Path::new("/store"),
                "zRepo",
                "abc"
            )
            .is_err()
        );
        let directory = helper("printf '{}' >&2; printf '{}'");
        assert!(
            read_patch(
                &directory.path().join("helper"),
                Path::new("/store"),
                "zRepo",
                "abc"
            )
            .is_err()
        );
    }
}
