//! Recovery decision over the launch the ledger recorded with the transport
//! intent.
//!
//! Lattice proves that an attempt never started only within the state root that
//! holds its execution intent. The Lattice program and state root of an
//! attempt's first launch are recorded in the same ledger mutation as its
//! transport intent, so only the launch that won the intent can direct
//! recovery. The bound root is assumed durable: losing or replacing it in place
//! voids every no-start observation recovered from it.

use kiln_core::implementation::TransportLaunch;
use std::path::Path;

/// Decide the recovery launch from the recorded launch, whether its state
/// root is still a real directory and whether its program still has the
/// recorded BLAKE3.
///
/// # Errors
/// Refuses an intent without a recorded launch (persisted before launches were
/// recorded), a missing bound root and a changed program, so Lattice is never
/// invoked in a fresh root or with another executable.
// r[impl kiln.implementation.lattice_cli.scenario.bound_root]
pub fn recovery_launch(
    recorded: Option<TransportLaunch>,
    state_is_directory: bool,
    program_matches: bool,
) -> Result<TransportLaunch, &'static str> {
    let launch = recorded.ok_or(
        "the transport intent records no launch; Lattice was not invoked; retain the unknown outcome and reconcile",
    )?;
    if !state_is_directory {
        return Err(
            "the recorded Lattice state root is missing; Lattice was not invoked in a fresh root; retain the unknown outcome and reconcile",
        );
    }
    if !program_matches {
        return Err(
            "the recorded Lattice executable changed; Lattice was not invoked; retain the unknown outcome and reconcile",
        );
    }
    Ok(launch)
}

/// Whether the path is an existing directory, not a symbolic link.
#[must_use]
pub fn is_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

#[cfg(test)]
#[path = "worker_binding_tests.rs"]
mod tests;
