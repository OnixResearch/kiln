use std::collections::BTreeSet;

use kiln_core::source::{
    PijulEntryKind, PijulSnapshotLimits, SourceCapability, SourceProviderProfile,
    admit_source_capability,
};
use kiln_core::types::{Alias, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

pub const PINNED_PROFILE_ID: &str = "pijul-source-v1";
pub const PINNED_PIJUL_VERSION: &str = "1.0.0-beta.11";
pub const PINNED_LIBPIJUL_VERSION: &str = "1.0.0-beta.11";
pub const PINNED_REPOSITORY_ADAPTER_VERSION: &str = "1.0.0-beta.10";
pub const PINNED_PIJUL_SOURCE_REVISION: &str =
    "crates.io:sha256:8a4fc27aa81ee061310d57fce2df9cc45f3149ddb00bdfab2b816beb0359b13d";
pub const PINNED_LIBPIJUL_SOURCE_REVISION: &str =
    "crates.io:sha256:f625e90234ef1fd164dfe7980ffb04a3036a08e31e051cc29e6273a0c9a7a7e7";
pub const PINNED_REPOSITORY_ADAPTER_SOURCE_REVISION: &str =
    "crates.io:sha256:12b9f22e42a5149909b2d1d20706a5ee5042a1ec60330d94124a213e4e294aa5";
pub const PINNED_PIJUL_LICENSE: &str = "GPL-2.0-or-later";
pub const PINNED_LIBPIJUL_LICENSE: &str = "GPL-2.0-or-later";
pub const PINNED_REPOSITORY_ADAPTER_LICENSE: &str = "GPL-2.0";
pub const PINNED_FORMAT_COHORT: &str = "sanakirja-pristine-v1";
pub const PINNED_PLATFORM: &str = "x86_64-linux";
const CURRENT_PLATFORM: &str = if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
    "x86_64-linux"
} else {
    "unsupported"
};

pub const DEFAULT_MAX_CHANNELS: usize = 64;
pub const DEFAULT_MAX_REPOSITORY_MEMBERS: usize = 65_536;
pub const DEFAULT_MAX_REPOSITORY_BYTES: u64 = 512 * 1_024 * 1_024;
pub const DEFAULT_MAX_CHANGES: usize = 4_096;
pub const DEFAULT_MAX_ENTRIES: usize = 16_384;
pub const DEFAULT_MAX_PATH_BYTES: usize = 4_096;
pub const DEFAULT_MAX_FILE_BYTES: usize = 16 * 1_024 * 1_024;
pub const DEFAULT_MAX_TOTAL_BYTES: usize = 256 * 1_024 * 1_024;

const ABSOLUTE_MAX_CHANNELS: usize = 4_096;
const ABSOLUTE_MAX_REPOSITORY_MEMBERS: usize = 1_000_000;
const ABSOLUTE_MAX_REPOSITORY_BYTES: u64 = 8_589_934_592;
const ABSOLUTE_MAX_CHANGES: usize = 1_000_000;
const ABSOLUTE_MAX_ENTRIES: usize = 1_000_000;
const ABSOLUTE_MAX_PATH_BYTES: usize = 65_536;
const ABSOLUTE_MAX_FILE_BYTES: usize = 1_073_741_824;
const ABSOLUTE_MAX_TOTAL_BYTES: usize = 4_294_967_296;

const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "local Pijul observation does not prove source trust",
    "local Pijul observation does not prove signature validity",
    "local Pijul observation does not prove complete history",
    "local Pijul observation does not prove remote availability",
    "local Pijul observation does not grant collaboration authority",
    "local Pijul observation does not grant source mutation authority",
    "equal canonical trees do not prove equal Pijul history",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulSourceProfile {
    pub id: ProviderProfileId,
    pub pijul_version: String,
    pub libpijul_version: String,
    pub repository_adapter_version: String,
    pub pijul_source_revision: String,
    pub libpijul_source_revision: String,
    pub repository_adapter_source_revision: String,
    pub pijul_license: String,
    pub libpijul_license: String,
    pub repository_adapter_license: String,
    pub format_cohort: String,
    pub platforms: BTreeSet<String>,
    pub capabilities: BTreeSet<SourceCapability>,
    pub allowed_entry_kinds: BTreeSet<PijulEntryKind>,
    pub max_channels: usize,
    pub max_repository_members: usize,
    pub max_repository_bytes: u64,
    pub limits: PijulSnapshotLimits,
    pub allow_network: bool,
    pub allow_credentials: bool,
    pub allow_collaboration: bool,
    pub allow_triggers: bool,
    pub allow_status_publication: bool,
    pub allow_mutation: bool,
    pub non_claims: Vec<String>,
}

impl PijulSourceProfile {
    #[must_use]
    pub fn pinned_local() -> Self {
        Self {
            id: ProviderProfileId::new(PINNED_PROFILE_ID).unwrap_or_else(|_| unreachable!()),
            pijul_version: PINNED_PIJUL_VERSION.to_string(),
            libpijul_version: PINNED_LIBPIJUL_VERSION.to_string(),
            repository_adapter_version: PINNED_REPOSITORY_ADAPTER_VERSION.to_string(),
            pijul_source_revision: PINNED_PIJUL_SOURCE_REVISION.to_string(),
            libpijul_source_revision: PINNED_LIBPIJUL_SOURCE_REVISION.to_string(),
            repository_adapter_source_revision: PINNED_REPOSITORY_ADAPTER_SOURCE_REVISION
                .to_string(),
            pijul_license: PINNED_PIJUL_LICENSE.to_string(),
            libpijul_license: PINNED_LIBPIJUL_LICENSE.to_string(),
            repository_adapter_license: PINNED_REPOSITORY_ADAPTER_LICENSE.to_string(),
            format_cohort: PINNED_FORMAT_COHORT.to_string(),
            platforms: BTreeSet::from([PINNED_PLATFORM.to_string()]),
            capabilities: required_capabilities(),
            allowed_entry_kinds: required_entry_kinds(),
            max_channels: DEFAULT_MAX_CHANNELS,
            max_repository_members: DEFAULT_MAX_REPOSITORY_MEMBERS,
            max_repository_bytes: DEFAULT_MAX_REPOSITORY_BYTES,
            limits: PijulSnapshotLimits {
                max_changes: DEFAULT_MAX_CHANGES,
                max_entries: DEFAULT_MAX_ENTRIES,
                max_path_bytes: DEFAULT_MAX_PATH_BYTES,
                max_file_bytes: DEFAULT_MAX_FILE_BYTES,
                max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            },
            allow_network: false,
            allow_credentials: false,
            allow_collaboration: false,
            allow_triggers: false,
            allow_status_publication: false,
            allow_mutation: false,
            non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }
}

fn required_capabilities() -> BTreeSet<SourceCapability> {
    BTreeSet::from([
        SourceCapability::Discover,
        SourceCapability::ResolveExact,
        SourceCapability::Acquire,
        SourceCapability::ObserveIdentity,
    ])
}

fn required_entry_kinds() -> BTreeSet<PijulEntryKind> {
    BTreeSet::from([
        PijulEntryKind::Regular,
        PijulEntryKind::Executable,
        PijulEntryKind::Directory,
    ])
}

fn limits_are_admitted(profile: &PijulSourceProfile) -> bool {
    profile.max_channels > 0
        && profile.max_channels <= ABSOLUTE_MAX_CHANNELS
        && profile.max_repository_members > 0
        && profile.max_repository_members <= ABSOLUTE_MAX_REPOSITORY_MEMBERS
        && profile.max_repository_bytes > 0
        && profile.max_repository_bytes <= ABSOLUTE_MAX_REPOSITORY_BYTES
        && profile.limits.max_changes > 0
        && profile.limits.max_changes <= ABSOLUTE_MAX_CHANGES
        && profile.limits.max_entries > 0
        && profile.limits.max_entries <= ABSOLUTE_MAX_ENTRIES
        && profile.limits.max_path_bytes > 0
        && profile.limits.max_path_bytes <= ABSOLUTE_MAX_PATH_BYTES
        && profile.limits.max_file_bytes > 0
        && profile.limits.max_file_bytes <= ABSOLUTE_MAX_FILE_BYTES
        && profile.limits.max_total_bytes > 0
        && profile.limits.max_total_bytes <= ABSOLUTE_MAX_TOTAL_BYTES
}

// r[impl kiln.source.pijul_configuration]
// r[impl kiln.source.pijul_capabilities]
// r[impl kiln.source.pijul_nonclaims]
pub fn admit_profile(profile: &PijulSourceProfile) -> PortResult<SourceProviderProfile> {
    if profile.id.as_str() != PINNED_PROFILE_ID
        || profile.pijul_version != PINNED_PIJUL_VERSION
        || profile.libpijul_version != PINNED_LIBPIJUL_VERSION
        || profile.repository_adapter_version != PINNED_REPOSITORY_ADAPTER_VERSION
        || profile.pijul_source_revision != PINNED_PIJUL_SOURCE_REVISION
        || profile.libpijul_source_revision != PINNED_LIBPIJUL_SOURCE_REVISION
        || profile.repository_adapter_source_revision != PINNED_REPOSITORY_ADAPTER_SOURCE_REVISION
        || profile.pijul_license != PINNED_PIJUL_LICENSE
        || profile.libpijul_license != PINNED_LIBPIJUL_LICENSE
        || profile.repository_adapter_license != PINNED_REPOSITORY_ADAPTER_LICENSE
        || profile.format_cohort != PINNED_FORMAT_COHORT
        || CURRENT_PLATFORM != PINNED_PLATFORM
        || profile.platforms != BTreeSet::from([PINNED_PLATFORM.to_string()])
        || profile.capabilities != required_capabilities()
        || profile.allowed_entry_kinds != required_entry_kinds()
        || profile.non_claims != REQUIRED_NON_CLAIMS.map(str::to_string)
        || !limits_are_admitted(profile)
    {
        return Err(Diagnostic::new(
            "pijul_profile",
            "pijul.profile",
            "Pijul source profile does not match the pinned read-only cohort",
        )
        .into());
    }
    if profile_has_forbidden_authority(profile) {
        return Err(Diagnostic::new(
            "pijul_profile_authority",
            "pijul.profile.capabilities",
            "Pijul source profile grants authority outside bounded local reads",
        )
        .into());
    }
    let admitted = SourceProviderProfile {
        id: profile.id.clone(),
        native_system: ExternalRef::new(
            "pijul",
            "source-provider",
            PINNED_LIBPIJUL_SOURCE_REVISION,
        )?,
        capabilities: required_capabilities(),
        identity_namespaces: BTreeSet::from([
            Alias::new("pijul-state")?,
            Alias::new("pijul-change")?,
        ]),
        immutable_resolution: true,
    };
    for capability in required_capabilities() {
        admit_source_capability(&admitted, capability, &admitted.id)?;
    }
    Ok(admitted)
}

#[must_use]
pub const fn profile_has_forbidden_authority(profile: &PijulSourceProfile) -> bool {
    profile.allow_network
        || profile.allow_credentials
        || profile.allow_collaboration
        || profile.allow_triggers
        || profile.allow_status_publication
        || profile.allow_mutation
}
