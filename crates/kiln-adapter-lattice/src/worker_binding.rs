//! The Lattice program and state root bound at an attempt's first launch.
//!
//! Lattice proves that an attempt never started only within the state root that
//! holds its execution intent. Before Kiln persists the transport intent for the
//! first `run`, it durably retains the exact program and state root it launches,
//! and `recover` uses only those. The bound root is assumed durable: losing or
//! replacing it in place voids every no-start observation recovered from it.

use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const BINDING_SCHEMA: &str = "kiln.lattice-transport-binding.v1";
/// A binding is a few paths and identities.
const MAX_BINDING_BYTES: u64 = 16 * 1024;

/// The first launch's exact program and state root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportBinding {
    pub schema: String,
    pub attempt: String,
    pub fence: u64,
    pub lattice_program: PathBuf,
    pub executable_blake3: String,
    pub state_directory: PathBuf,
}

impl TransportBinding {
    #[must_use]
    pub fn new(
        attempt: &ContentId,
        fence: u64,
        lattice_program: &Path,
        executable_blake3: &ContentId,
        state_directory: &Path,
    ) -> Self {
        Self {
            schema: BINDING_SCHEMA.to_owned(),
            attempt: attempt.as_str().to_owned(),
            fence,
            lattice_program: lattice_program.to_path_buf(),
            executable_blake3: executable_blake3.as_str().to_owned(),
            state_directory: state_directory.to_path_buf(),
        }
    }

    /// Where the binding of one attempt and fence is retained.
    #[must_use]
    pub fn path(output_directory: &Path, attempt: &ContentId, fence: u64) -> PathBuf {
        output_directory.join(format!(
            "transport-binding-{}-{fence}.json",
            attempt.as_str()
        ))
    }
}

// r[impl kiln.implementation.lattice_cli.scenario.bound_root]
/// Decide the recovery launch from the retained binding and whether its state
/// root still exists as a directory.
///
/// # Errors
/// Refuses an absent binding (an intent persisted before bindings existed), a
/// binding of another attempt, fence or schema, and a missing bound root, so
/// Lattice is never invoked in a fresh root.
pub fn recovery_launch(
    retained: Option<TransportBinding>,
    attempt: &ContentId,
    fence: u64,
    state_is_directory: bool,
) -> Result<TransportBinding, &'static str> {
    let binding = retained.ok_or(
        "no transport binding is retained for this attempt; Lattice was not invoked; retain the unknown outcome and reconcile",
    )?;
    if binding.schema != BINDING_SCHEMA
        || binding.attempt != attempt.as_str()
        || binding.fence != fence
    {
        return Err("retained transport binding names another attempt; Lattice was not invoked");
    }
    if !state_is_directory {
        return Err(
            "the bound Lattice state root is missing; Lattice was not invoked in a fresh root; retain the unknown outcome and reconcile",
        );
    }
    Ok(binding)
}

/// Durably retain the binding once: exclusive create, mode 0400, fsync of the
/// file and its directory. A retained binding with other bytes is refused.
///
/// # Errors
/// Refuses a conflicting binding and any I/O failure.
pub fn retain(path: &Path, binding: &TransportBinding) -> Result<(), String> {
    let bytes = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o400);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| error.to_string())?;
            let parent = path.parent().ok_or("transport binding has no directory")?;
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| error.to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if read_bounded(path)? == bytes {
                Ok(())
            } else {
                Err(
                    "a different Lattice program or state root is already bound to this attempt"
                        .into(),
                )
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Read the retained binding, if any.
///
/// # Errors
/// Refuses a non-file, oversized or malformed binding.
pub fn load(path: &Path) -> Result<Option<TransportBinding>, String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
        Ok(_) => serde_json::from_slice(&read_bounded(path)?)
            .map(Some)
            .map_err(|_| "retained transport binding is malformed".to_string()),
    }
}

/// Whether the path is an existing directory, not a symbolic link.
#[must_use]
pub fn is_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BINDING_BYTES {
        return Err("transport binding is not a bounded ordinary file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_BINDING_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "worker_binding_tests.rs"]
mod tests;
