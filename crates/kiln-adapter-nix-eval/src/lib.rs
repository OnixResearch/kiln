mod adapter;
mod mapping;
mod profile;
mod runtime;
mod wire;

pub use adapter::NixEvalAdapter;
pub use mapping::{CohortManifest, admit_cohort_manifest, admit_plan, gc_root_directory};
pub use profile::{
    COHORT_MANIFEST_SCHEMA, NixEvalLimits, NixEvalPolicy, NixEvalProfile, NixEvalRuntimeBinding,
    PINNED_ADAPTER_VERSION, PINNED_BOUNDED_EXEC_LICENSE, PINNED_BOUNDED_EXEC_REVISION,
    PINNED_BOUNDED_EXEC_SOURCE, PINNED_COHORT_MANIFEST_BLAKE3, PINNED_COHORT_MANIFEST_PATH,
    PINNED_EXECUTABLE_BLAKE3, PINNED_EXECUTABLE_PATH, PINNED_NIX_REVISION, PINNED_NIX_SOURCE,
    PINNED_NIX_VERSION, PINNED_NIXPKGS_REVISION, PINNED_PLATFORM, PINNED_SOURCE_VERSION,
    PINNED_UPSTREAM_LICENSE, PINNED_UPSTREAM_REVISION, PINNED_UPSTREAM_SOURCE, REQUIRED_NON_CLAIMS,
    admit_profile,
};
pub use runtime::{BoundedNixEvalRuntime, NixEvalProcessOutput, NixEvalRuntime, ProcessInvocation};
pub use wire::{IncrementalDecoder, StreamAdmission};

#[cfg(test)]
mod tests;
