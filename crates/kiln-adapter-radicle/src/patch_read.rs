//! Bounded read-only access to the isolated upstream Radicle patch reducer.
//!
//! The helper is a separate process because the upstream `radicle` crate links
//! a different native SQLite binding. This module performs no mutation and
//! grants no Kiln review, CI or release authority.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const MAX_HELPER_BYTES: usize = 1_048_576;

/// Invoke the reviewed helper and return one reduced patch projection.
/// Append one review through the isolated upstream helper. The signer seed is
/// passed only through the child environment.
pub fn review_patch(
    helper: &Path,
    storage_root: &Path,
    repository_id: &str,
    patch_id: &str,
    revision_id: &str,
    verdict: &str,
    summary: &str,
    seed: &[u8; 32],
) -> Result<Value, String> {
    if repository_id.is_empty()
        || repository_id.len() > 256
        || patch_id.is_empty()
        || patch_id.len() > 256
        || revision_id.is_empty()
        || revision_id.len() > 256
        || !matches!(verdict, "accept" | "reject")
        || summary.is_empty()
        || summary.len() > 16_384
    {
        return Err("invalid Radicle review request".into());
    }
    let seed = seed
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output = Command::new(helper)
        .arg("review")
        .arg(storage_root)
        .arg(repository_id)
        .arg(patch_id)
        .arg(revision_id)
        .arg(verdict)
        .arg(summary)
        .env("RADICLE_SIGNER_SEED", seed)
        .output()
        .map_err(|_| "Radicle patch helper unavailable")?;
    if !output.status.success() {
        return Err("Radicle patch helper rejected the review".into());
    }
    if output.stdout.len() > MAX_HELPER_BYTES || !output.stderr.is_empty() {
        return Err("Radicle patch helper exceeded its output bound".into());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| "Radicle patch helper returned invalid JSON".into())
}

pub fn read_patch(
    helper: &Path,
    storage_root: &Path,
    repository_id: &str,
    patch_id: &str,
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
    let output = Command::new(helper)
        .arg(storage_root)
        .arg(repository_id)
        .arg(patch_id)
        .output()
        .map_err(|_| "Radicle patch helper unavailable")?;
    if !output.status.success() {
        return Err("Radicle patch helper rejected the request".into());
    }
    if output.stdout.len() > MAX_HELPER_BYTES || !output.stderr.is_empty() {
        return Err("Radicle patch helper exceeded its output bound".into());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| "Radicle patch helper returned invalid JSON".into())
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
    fn helper_output_is_bounded_and_parsed() {
        let directory = helper("printf '{\"state\":\"open\",\"revisions\":2}'");
        let value = read_patch(
            &directory.path().join("helper"),
            Path::new("/store"),
            "zRepo",
            "abc",
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(value["state"], "open");
        assert_eq!(value["revisions"], 2);
    }

    #[test]
    fn review_uses_child_environment_and_bounded_output() {
        let directory =
            helper("test -n \"$RADICLE_SIGNER_SEED\" && printf '{\"review_id\":\"r1\"}'");
        let value = review_patch(
            &directory.path().join("helper"),
            Path::new("/store"),
            "zRepo",
            "abc",
            "rev",
            "accept",
            "looks good",
            &[7; 32],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(value["review_id"], "r1");
        assert!(
            review_patch(
                &directory.path().join("helper"),
                Path::new("/store"),
                "zRepo",
                "abc",
                "rev",
                "maybe",
                "looks good",
                &[7; 32]
            )
            .is_err()
        );
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
