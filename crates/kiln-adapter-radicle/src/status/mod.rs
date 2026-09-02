pub mod cob;
pub mod plan;
pub mod profile;
mod resolver;
mod secret;
pub mod shell;

pub use plan::{
    HeadAuthor, ObservedHeads, StatusOutcome, StatusPlan, admit_expected_head,
    admit_idempotent_replay, admit_single_writer_heads, admit_status_plan, classify_status_outcome,
};
pub use profile::{
    CI_JOB_TYPE_NAME, DEFAULT_MAX_OBJECTS_SCANNED, PINNED_GIT2_SOURCE_REVISION,
    PINNED_GIT2_VERSION, PINNED_PLATFORM, PINNED_PROFILE_ID, PINNED_RADICLE_COB_SOURCE_REVISION,
    PINNED_RADICLE_COB_VERSION, PINNED_RADICLE_COHORT, PINNED_RADICLE_CRYPTO_SOURCE_REVISION,
    PINNED_RADICLE_CRYPTO_VERSION, PINNED_RADICLE_LICENSE, PINNED_RADICLE_OID_SOURCE_REVISION,
    PINNED_RADICLE_OID_VERSION, PINNED_RADICLE_REF_FORMAT_SOURCE_REVISION,
    PINNED_RADICLE_REF_FORMAT_VERSION, PINNED_RADICLE_SOURCE_REVISION,
    PINNED_SECRETSPEC_EXECUTABLE_BLAKE3, PINNED_SECRETSPEC_EXECUTABLE_PATH,
    PINNED_SECRETSPEC_LICENSE, PINNED_SECRETSPEC_NIXPKGS_REVISION,
    PINNED_SECRETSPEC_SOURCE_REVISION, PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable,
    RadicleStatusProfile, SecretSpecDeclaration, admit_profile,
};
pub use resolver::SignerResolver;
pub use secret::ResolvedSigner;
pub use shell::{
    RadicleStatusShell, StatusDispatch, StatusUnknownObservation, StatusWriteObservation,
};

#[cfg(test)]
mod tests;
