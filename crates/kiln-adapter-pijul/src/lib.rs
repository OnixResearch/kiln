mod archive;
mod local;
mod mapping;
mod profile;

pub use local::LocalPijulAdapter;
pub use mapping::{discover_channels, resolve_and_acquire};
pub use profile::{
    PINNED_FORMAT_COHORT, PINNED_LIBPIJUL_LICENSE, PINNED_LIBPIJUL_SOURCE_REVISION,
    PINNED_LIBPIJUL_VERSION, PINNED_PIJUL_LICENSE, PINNED_PIJUL_SOURCE_REVISION,
    PINNED_PIJUL_VERSION, PINNED_PLATFORM, PINNED_PROFILE_ID, PINNED_REPOSITORY_ADAPTER_LICENSE,
    PINNED_REPOSITORY_ADAPTER_SOURCE_REVISION, PINNED_REPOSITORY_ADAPTER_VERSION,
    PijulSourceProfile, admit_profile, profile_has_forbidden_authority,
};

#[cfg(all(test, target_arch = "x86_64", target_os = "linux"))]
mod tests;
