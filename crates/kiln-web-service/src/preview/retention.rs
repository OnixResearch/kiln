//! Finite leases for owned preview exports only. Never touches Nix GC roots,
//! arbitrary scratch files, symlinks, source trees or another user's directories.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
const PREFIX: &str = "kiln-preview-export-";
const LEASE: &str = ".kiln-preview-lease.json";
const MAX_LEASE_BYTES: u64 = 1024;
const MAX_SCRATCH_ENTRIES: usize = 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lease {
    schema: String,
    created_at: u64,
    expires_at: u64,
    purpose: String,
}
const SCHEMA: &str = "kiln.preview.export-lease.v1";

pub fn create(scratch: &Path, purpose: &str, expires_at: u64) -> Result<tempfile::TempDir, String> {
    let now = super::service::now()?;
    if expires_at <= now || expires_at - now > kiln_core::preview::MAX_PREVIEW_LIFETIME_SECONDS {
        return Err("preview_export_expired".into());
    }
    cleanup(scratch, now)?;
    let directory = tempfile::Builder::new()
        .prefix(PREFIX)
        .tempdir_in(scratch)
        .map_err(|e| e.to_string())?;
    let lease = Lease {
        schema: SCHEMA.into(),
        created_at: now,
        expires_at,
        purpose: purpose.into(),
    };
    fs::write(
        directory.path().join(LEASE),
        serde_json::to_vec(&lease).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(directory)
}

/// Run on startup, normal expiry/stop, and as an operator-owned scheduled cleanup
/// after process/host crashes. A dead process cannot promise its own timer runs.
pub fn cleanup(scratch: &Path, now: u64) -> Result<usize, String> {
    use std::os::unix::fs::MetadataExt;
    let mut removed = 0;
    for (i, entry) in fs::read_dir(scratch)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if i >= MAX_SCRATCH_ENTRIES {
            return Err("preview_cleanup_scan_exhausted".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_name().to_string_lossy().starts_with(PREFIX) {
            continue;
        }
        let metadata = entry.path().symlink_metadata().map_err(|e| e.to_string())?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != ::nix::unistd::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
        {
            continue;
        }
        let path = entry.path().join(LEASE);
        let Ok(file) = File::open(&path) else {
            continue;
        };
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_LEASE_BYTES {
            continue;
        }
        let mut bytes = Vec::new();
        file.take(MAX_LEASE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let Ok(lease) = serde_json::from_slice::<Lease>(&bytes) else {
            continue;
        };
        if lease.schema != SCHEMA
            || lease.expires_at < lease.created_at
            || lease.expires_at - lease.created_at
                > kiln_core::preview::MAX_PREVIEW_LIFETIME_SECONDS
            || lease.expires_at > now
        {
            continue;
        }
        fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_removes_only_expired_owned_leases() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = tempfile::tempdir()?;
        let now = super::super::service::now()?;
        let export = create(scratch.path(), "runtime", now + 60)?;
        let path = export.keep();
        let unrelated = scratch.path().join("unrelated");
        fs::create_dir(&unrelated)?;
        assert_eq!(cleanup(scratch.path(), now)?, 0);
        assert!(path.exists());
        assert_eq!(cleanup(scratch.path(), now + 60)?, 1);
        assert!(!path.exists());
        assert!(unrelated.exists());
        Ok(())
    }
}
