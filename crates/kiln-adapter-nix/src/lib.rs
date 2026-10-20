mod adapter;
mod mapping;
mod profile;
mod runtime;
mod wire;

use std::path::PathBuf;

use kiln_core::types::EffectId;
use kiln_provider_api::PortResult;

pub use adapter::NixBuildAdapter;
pub use profile::*;
pub use runtime::{
    BoundedNixBuildRuntime, NixBuildProcessOutput, NixBuildRuntime, ProcessInvocation,
};

/// Returns the deterministic run-scoped GC-root directory for one effect.
///
/// The adapter creates and removes this directory. Exposing the path supports
/// operator diagnostics without granting direct mutation authority.
pub fn gc_root_directory(profile: &NixBuildProfile, effect_id: &EffectId) -> PortResult<PathBuf> {
    mapping::gc_root_directory(profile, effect_id)
}

#[cfg(test)]
mod tests;
