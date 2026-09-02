use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_core::types::{Alias, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

pub const PINNED_PROFILE_ID: &str = "radicle-status-v1";
pub const PINNED_PLATFORM: &str = "x86_64-linux";
pub const PINNED_RADICLE_COHORT: &str =
    "radicle-cob-0.21.1-crypto-0.19.0-oid-0.2.2-ref-format-0.2.0-git2-0.21.0";
pub const PINNED_RADICLE_COB_VERSION: &str = "0.21.1";
pub const PINNED_RADICLE_COB_SOURCE_REVISION: &str =
    "crates.io:sha256:8a6b295464ff130975fc151feb8aef4d6fd22ddd1da2a5a2a561f283c7abd41e";
pub const PINNED_RADICLE_CRYPTO_VERSION: &str = "0.19.0";
pub const PINNED_RADICLE_CRYPTO_SOURCE_REVISION: &str =
    "crates.io:sha256:76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5";
pub const PINNED_RADICLE_OID_VERSION: &str = "0.2.2";
pub const PINNED_RADICLE_OID_SOURCE_REVISION: &str =
    "crates.io:sha256:7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c";
pub const PINNED_RADICLE_REF_FORMAT_VERSION: &str = "0.2.0";
pub const PINNED_RADICLE_REF_FORMAT_SOURCE_REVISION: &str =
    "crates.io:sha256:8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6";
pub const PINNED_GIT2_VERSION: &str = "0.21.0";
pub const PINNED_GIT2_SOURCE_REVISION: &str =
    "crates.io:sha256:ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e";
pub const PINNED_RADICLE_SOURCE_REVISION: &str = PINNED_RADICLE_COB_SOURCE_REVISION;
pub const PINNED_SECRETSPEC_VERSION: &str = "0.19.0";
pub const PINNED_SECRETSPEC_SOURCE_REVISION: &str =
    "crates.io:sha256:827137f2e4d608acb1331aaffc91b16e457903d41964939fb8e716c3be557827";
pub const PINNED_SECRETSPEC_NIXPKGS_REVISION: &str = "b7c2ada94fe99c15b0dbcf4d11fd7850b957a436";
pub const PINNED_SECRETSPEC_EXECUTABLE_PATH: &str =
    "/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec";
pub const PINNED_SECRETSPEC_EXECUTABLE_BLAKE3: &str =
    "c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6";
pub const PINNED_SECRETSPEC_LICENSE: &str = "Apache-2.0";
pub const PINNED_RADICLE_LICENSE: &str = "MIT OR Apache-2.0";
pub const CI_JOB_TYPE_NAME: &str = "xyz.radicle.ci-job";

pub const DEFAULT_MAX_PAYLOAD_BYTES: usize = 65_536;
pub const DEFAULT_MAX_OBJECTS_SCANNED: usize = 256;
const ABSOLUTE_MAX_PAYLOAD_BYTES: usize = 1_048_576;
const ABSOLUTE_MAX_OBJECTS_SCANNED: usize = 4_096;
const ABSOLUTE_MAX_RIDS: usize = 256;
const ABSOLUTE_MAX_RID_BYTES: usize = 256;
const ABSOLUTE_MAX_DECLARATION_NAME_BYTES: usize = 256;
const ABSOLUTE_MAX_PROVIDER_BYTES: usize = 64;
const ABSOLUTE_MAX_SECRET_KEY_BYTES: usize = 512;
const BLAKE3_HEX_LENGTH: usize = 64;

/// The pinned SecretSpec executable declaration.
///
/// Kiln verifies the BLAKE3 identity before every resolution and runs the
/// executable through Bounded Exec with a cleared environment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedSecretSpecExecutable {
    pub path: PathBuf,
    pub blake3: String,
    pub version: String,
}

/// One explicit SecretSpec declaration for the signer secret.
///
/// The declaration names where the key lives; it never contains the key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretSpecDeclaration {
    /// Project root that holds the `secretspec.toml` manifest.
    pub project_root: PathBuf,
    /// SecretSpec project name declared in that manifest.
    pub project: String,
    /// SecretSpec profile inside the project.
    pub profile: String,
    /// The declared secret key holding the signer seed.
    pub secret_key: String,
    /// Optional provider restriction; empty means the operator default.
    pub provider: Option<String>,
}

/// A typed Radicle status publication profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleStatusProfile {
    pub id: ProviderProfileId,
    pub platform: String,
    pub radicle_cohort: String,
    pub radicle_cob_version: String,
    pub radicle_cob_source_revision: String,
    pub radicle_crypto_version: String,
    pub radicle_crypto_source_revision: String,
    pub radicle_oid_version: String,
    pub radicle_oid_source_revision: String,
    pub radicle_ref_format_version: String,
    pub radicle_ref_format_source_revision: String,
    pub git2_version: String,
    pub git2_source_revision: String,
    pub secretspec_version: String,
    pub secretspec_source_revision: String,
    pub secretspec_nixpkgs_revision: String,
    pub secretspec_license: String,
    pub radicle_license: String,
    /// Admitted repository identities (`<RID>` without the `rad:` prefix).
    pub allowed_rids: BTreeSet<String>,
    /// The single admitted COB type for status writes.
    pub allowed_object_type: String,
    pub signer: SecretSpecDeclaration,
    pub executable: PinnedSecretSpecExecutable,
    pub max_payload_bytes: usize,
    pub max_objects_scanned: usize,
    pub allow_network_peers: bool,
    pub allow_merge_authority: bool,
    pub allow_canonical_state_mutation: bool,
    pub non_claims: Vec<String>,
}

impl RadicleStatusProfile {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn pinned(
        allowed_rids: BTreeSet<String>,
        signer: SecretSpecDeclaration,
        executable: PinnedSecretSpecExecutable,
    ) -> Self {
        Self {
            id: ProviderProfileId::new(PINNED_PROFILE_ID).unwrap_or_else(|_| unreachable!()),
            platform: PINNED_PLATFORM.to_string(),
            radicle_cohort: PINNED_RADICLE_COHORT.to_string(),
            radicle_cob_version: PINNED_RADICLE_COB_VERSION.to_string(),
            radicle_cob_source_revision: PINNED_RADICLE_COB_SOURCE_REVISION.to_string(),
            radicle_crypto_version: PINNED_RADICLE_CRYPTO_VERSION.to_string(),
            radicle_crypto_source_revision: PINNED_RADICLE_CRYPTO_SOURCE_REVISION.to_string(),
            radicle_oid_version: PINNED_RADICLE_OID_VERSION.to_string(),
            radicle_oid_source_revision: PINNED_RADICLE_OID_SOURCE_REVISION.to_string(),
            radicle_ref_format_version: PINNED_RADICLE_REF_FORMAT_VERSION.to_string(),
            radicle_ref_format_source_revision: PINNED_RADICLE_REF_FORMAT_SOURCE_REVISION
                .to_string(),
            git2_version: PINNED_GIT2_VERSION.to_string(),
            git2_source_revision: PINNED_GIT2_SOURCE_REVISION.to_string(),
            secretspec_version: PINNED_SECRETSPEC_VERSION.to_string(),
            secretspec_source_revision: PINNED_SECRETSPEC_SOURCE_REVISION.to_string(),
            secretspec_nixpkgs_revision: PINNED_SECRETSPEC_NIXPKGS_REVISION.to_string(),
            secretspec_license: PINNED_SECRETSPEC_LICENSE.to_string(),
            radicle_license: PINNED_RADICLE_LICENSE.to_string(),
            allowed_rids,
            allowed_object_type: CI_JOB_TYPE_NAME.to_string(),
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_objects_scanned: DEFAULT_MAX_OBJECTS_SCANNED,
            signer,
            executable,
            allow_network_peers: false,
            allow_merge_authority: false,
            allow_canonical_state_mutation: false,
            non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }
}

// r[impl kiln.radicle.status_store_nonclaims]
const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "status writes do not complete Kiln attempts or mutate canonical state",
    "status writes do not prove peer convergence or remote visibility",
    "status writes do not prove source trust or actor authority",
    "SecretSpec resolution does not grant merge authority",
    "the signer identity does not prove operator intent beyond one admitted plan",
    "equal ci-job objects do not prove equal run outcomes elsewhere",
    "passing conformance does not prove node availability",
];

fn declaration_is_admitted(declaration: &SecretSpecDeclaration) -> bool {
    let provider_admitted = declaration.provider.as_ref().is_none_or(|provider| {
        !provider.is_empty()
            && provider.len() <= ABSOLUTE_MAX_PROVIDER_BYTES
            && provider
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    });
    declaration.project_root.is_absolute()
        && !declaration.project.is_empty()
        && declaration.project.len() <= ABSOLUTE_MAX_DECLARATION_NAME_BYTES
        && !declaration.profile.is_empty()
        && declaration.profile.len() <= ABSOLUTE_MAX_DECLARATION_NAME_BYTES
        && !declaration.secret_key.is_empty()
        && declaration.secret_key.len() <= ABSOLUTE_MAX_SECRET_KEY_BYTES
        && declaration.secret_key.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
        && provider_admitted
}

fn executable_is_admitted(executable: &PinnedSecretSpecExecutable) -> bool {
    executable.blake3.len() == BLAKE3_HEX_LENGTH
        && executable
            .blake3
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && executable.version == PINNED_SECRETSPEC_VERSION
        && executable.path.is_absolute()
}

fn rid_is_admitted(rid: &str) -> bool {
    rid.len() <= ABSOLUTE_MAX_RID_BYTES
        && rid.strip_prefix("rad:z").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

// r[impl kiln.radicle.status_store_pin]
// r[impl kiln.radicle.status_signer]
// r[impl kiln.radicle.status_allowlist]
pub fn admit_profile(profile: &RadicleStatusProfile) -> PortResult<BTreeSet<String>> {
    admit_profile_with_executable_policy(profile, true)
}

#[cfg(feature = "status-native-fixtures")]
pub(crate) fn admit_fixture_profile(
    profile: &RadicleStatusProfile,
) -> PortResult<BTreeSet<String>> {
    admit_profile_with_executable_policy(profile, false)
}

fn admit_profile_with_executable_policy(
    profile: &RadicleStatusProfile,
    require_production_executable: bool,
) -> PortResult<BTreeSet<String>> {
    let production_executable = profile.executable.path
        == PathBuf::from(PINNED_SECRETSPEC_EXECUTABLE_PATH)
        && profile.executable.blake3 == PINNED_SECRETSPEC_EXECUTABLE_BLAKE3;
    if profile.id.as_str() != PINNED_PROFILE_ID
        || profile.platform != PINNED_PLATFORM
        || profile.radicle_cohort != PINNED_RADICLE_COHORT
        || profile.radicle_cob_version != PINNED_RADICLE_COB_VERSION
        || profile.radicle_cob_source_revision != PINNED_RADICLE_COB_SOURCE_REVISION
        || profile.radicle_crypto_version != PINNED_RADICLE_CRYPTO_VERSION
        || profile.radicle_crypto_source_revision != PINNED_RADICLE_CRYPTO_SOURCE_REVISION
        || profile.radicle_oid_version != PINNED_RADICLE_OID_VERSION
        || profile.radicle_oid_source_revision != PINNED_RADICLE_OID_SOURCE_REVISION
        || profile.radicle_ref_format_version != PINNED_RADICLE_REF_FORMAT_VERSION
        || profile.radicle_ref_format_source_revision != PINNED_RADICLE_REF_FORMAT_SOURCE_REVISION
        || profile.git2_version != PINNED_GIT2_VERSION
        || profile.git2_source_revision != PINNED_GIT2_SOURCE_REVISION
        || profile.secretspec_version != PINNED_SECRETSPEC_VERSION
        || profile.secretspec_source_revision != PINNED_SECRETSPEC_SOURCE_REVISION
        || profile.secretspec_nixpkgs_revision != PINNED_SECRETSPEC_NIXPKGS_REVISION
        || profile.secretspec_license != PINNED_SECRETSPEC_LICENSE
        || profile.radicle_license != PINNED_RADICLE_LICENSE
        || profile.allowed_object_type != CI_JOB_TYPE_NAME
        || profile.allowed_rids.is_empty()
        || profile.allowed_rids.len() > ABSOLUTE_MAX_RIDS
        || !profile.allowed_rids.iter().all(|rid| rid_is_admitted(rid))
        || profile.max_payload_bytes == 0
        || profile.max_payload_bytes > ABSOLUTE_MAX_PAYLOAD_BYTES
        || profile.max_objects_scanned == 0
        || profile.max_objects_scanned > ABSOLUTE_MAX_OBJECTS_SCANNED
        || !declaration_is_admitted(&profile.signer)
        || !executable_is_admitted(&profile.executable)
        || (require_production_executable && !production_executable)
        || profile.non_claims != REQUIRED_NON_CLAIMS.map(str::to_string)
    {
        return Err(Diagnostic::new(
            "radicle_status_profile",
            "radicle_status.profile",
            "Radicle status profile does not match the pinned cohort or declaration bounds",
        )
        .into());
    }
    if profile.allow_network_peers
        || profile.allow_merge_authority
        || profile.allow_canonical_state_mutation
    {
        return Err(Diagnostic::new(
            "radicle_status_profile_authority",
            "radicle_status.profile.capabilities",
            "Radicle status profile grants authority outside bounded local writes",
        )
        .into());
    }
    ExternalRef::new("radicle", "cob-type", CI_JOB_TYPE_NAME)?;
    Alias::new("radicle-node")?;
    Ok(profile.allowed_rids.clone())
}
