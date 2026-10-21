use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;

use kiln_core::source::{SourceCapability, SourceProviderProfile, admit_source_capability};
use kiln_core::types::{Alias, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::{PortFailure, PortResult};
use kiln_radicle_source_contract::{
    CONTRACT_SCHEMA_VERSION, SnapshotLimits, SourceRequest, admit_request,
};
use radicle_core::RepoId;
use radicle_crypto::PublicKey;
use radicle_git_ref_format::RefString;
use serde::{Deserialize, Serialize};

pub const RADICLE_SOURCE_PROFILE_SCHEMA: &str = "kiln.radicle-source-profile.v1";
pub const PINNED_SOURCE_CONTRACT_REVISION: &str = "0a5924dd2092243a0fe5df94d43008f0402e46f6";
pub const PINNED_RADICLE_PROTOCOL_VERSION: &str = "0.25.1";
pub const PINNED_RADICLE_PROTOCOL_SOURCE_SHA256: &str =
    "fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4";
pub const PINNED_RADICLE_CORE_VERSION: &str = "0.4.1";
pub const PINNED_RADICLE_CORE_SOURCE_SHA256: &str =
    "077e01c4da810fed233fa67ce87b94fe0dd6ffa03b6640e3b71caa9769a5ec9f";
pub const PINNED_RADICLE_CRYPTO_VERSION: &str = "0.19.0";
pub const PINNED_RADICLE_CRYPTO_SOURCE_SHA256: &str =
    "76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5";
pub const PINNED_RADICLE_OID_VERSION: &str = "0.2.2";
pub const PINNED_RADICLE_OID_SOURCE_SHA256: &str =
    "7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c";
pub const PINNED_RADICLE_REF_FORMAT_VERSION: &str = "0.2.0";
pub const PINNED_RADICLE_REF_FORMAT_SOURCE_SHA256: &str =
    "8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6";
pub const PINNED_GIT2_VERSION: &str = "0.21.0";
pub const PINNED_GIT2_SOURCE_SHA256: &str =
    "ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e";
pub const PINNED_RADICLE_LICENSE: &str = "MIT OR Apache-2.0";

pub const REQUIRED_SOURCE_NON_CLAIMS: &[&str] = &[
    "local source admission does not prove signer or repository trust",
    "an expected signed-refs commit does not prove global freshness",
    "local acquisition does not prove complete history or peer convergence",
    "local acquisition does not prove remote seed availability",
    "equal canonical trees do not prove equal history or authority",
    "source capability does not grant network, credential, signing, mutation, or collaboration authority",
    "an absolute repository path does not prevent filesystem substitution",
    "source receipt admission does not prove build correctness",
    "source receipt admission does not establish release readiness",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleSourceCohort {
    pub protocol_version: String,
    pub protocol_source_sha256: String,
    pub radicle_core_version: String,
    pub radicle_core_source_sha256: String,
    pub radicle_crypto_version: String,
    pub radicle_crypto_source_sha256: String,
    pub radicle_oid_version: String,
    pub radicle_oid_source_sha256: String,
    pub radicle_ref_format_version: String,
    pub radicle_ref_format_source_sha256: String,
    pub git2_version: String,
    pub git2_source_sha256: String,
    pub license: String,
    pub full_radicle_runtime: bool,
    pub sqlite_backend: bool,
}

impl RadicleSourceCohort {
    #[must_use]
    pub fn pinned() -> Self {
        Self {
            protocol_version: PINNED_RADICLE_PROTOCOL_VERSION.to_string(),
            protocol_source_sha256: PINNED_RADICLE_PROTOCOL_SOURCE_SHA256.to_string(),
            radicle_core_version: PINNED_RADICLE_CORE_VERSION.to_string(),
            radicle_core_source_sha256: PINNED_RADICLE_CORE_SOURCE_SHA256.to_string(),
            radicle_crypto_version: PINNED_RADICLE_CRYPTO_VERSION.to_string(),
            radicle_crypto_source_sha256: PINNED_RADICLE_CRYPTO_SOURCE_SHA256.to_string(),
            radicle_oid_version: PINNED_RADICLE_OID_VERSION.to_string(),
            radicle_oid_source_sha256: PINNED_RADICLE_OID_SOURCE_SHA256.to_string(),
            radicle_ref_format_version: PINNED_RADICLE_REF_FORMAT_VERSION.to_string(),
            radicle_ref_format_source_sha256: PINNED_RADICLE_REF_FORMAT_SOURCE_SHA256.to_string(),
            git2_version: PINNED_GIT2_VERSION.to_string(),
            git2_source_sha256: PINNED_GIT2_SOURCE_SHA256.to_string(),
            license: PINNED_RADICLE_LICENSE.to_string(),
            full_radicle_runtime: false,
            sqlite_backend: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleSourceProfile {
    pub schema: String,
    pub contract_revision: String,
    pub repository_path: PathBuf,
    pub request: SourceRequest,
    pub cohort: RadicleSourceCohort,
    pub allow_network: bool,
    pub allow_credentials: bool,
    pub allow_signing: bool,
    pub allow_mutation: bool,
    pub allow_collaboration: bool,
    pub non_claims: Vec<String>,
}

impl RadicleSourceProfile {
    #[must_use]
    pub fn pinned(repository_path: PathBuf, request: SourceRequest) -> Self {
        Self {
            schema: RADICLE_SOURCE_PROFILE_SCHEMA.to_string(),
            contract_revision: PINNED_SOURCE_CONTRACT_REVISION.to_string(),
            repository_path,
            request,
            cohort: RadicleSourceCohort::pinned(),
            allow_network: false,
            allow_credentials: false,
            allow_signing: false,
            allow_mutation: false,
            allow_collaboration: false,
            non_claims: REQUIRED_SOURCE_NON_CLAIMS
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

/// Read-only authority for one repository, signer namespace and branch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleSourceSelection {
    pub repository_path: PathBuf,
    pub repository_id: String,
    pub signer_node_id: String,
    pub signed_reference: String,
    pub profile_id: String,
    pub limits: SnapshotLimits,
}

// r[impl kiln.radicle_ci.source]
impl RadicleSourceSelection {
    fn profile(&self, sigrefs: String, revision: String) -> RadicleSourceProfile {
        RadicleSourceProfile::pinned(
            self.repository_path.clone(),
            SourceRequest {
                schema_version: CONTRACT_SCHEMA_VERSION,
                profile_id: self.profile_id.clone(),
                repository_id: self.repository_id.clone(),
                signer_node_id: self.signer_node_id.clone(),
                signed_reference: self.signed_reference.clone(),
                expected_sigrefs_commit: sigrefs,
                expected_revision: revision,
                limits: self.limits.clone(),
            },
        )
    }

    /// Admit fixed policy without reading the repository or following refs.
    pub fn admit(&self) -> PortResult<()> {
        let placeholder = cob_git2::Oid::ZERO_SHA1.to_string();
        admit_source_profile(&self.profile(placeholder.clone(), placeholder))?;
        if RepoId::from_str(&self.repository_id).is_err()
            || PublicKey::from_str(&self.signer_node_id).is_err()
            || RefString::try_from(self.signed_reference.clone()).is_err()
        {
            return Err(domain(
                "radicle_source_selection",
                "radicle_source.selection",
                "Source selection contains an invalid repository, signer or branch identity",
            ));
        }
        Ok(())
    }

    /// Capture the current declared sigrefs exactly once; never retry a moved ref.
    pub fn pin(&self, revision: &ExternalRef) -> PortResult<RadicleSourceProfile> {
        self.admit()?;
        revision.validate()?;
        if revision.provider_namespace.as_str() != "radicle"
            || revision.native_kind.as_str() != "revision"
        {
            return Err(domain(
                "radicle_source_revision_substitution",
                "radicle_source.revision",
                "Source pinning requires an exact Radicle revision",
            ));
        }
        super::protocol::parse_oid(revision.native_id.as_str(), "radicle_source.revision")?;
        let repository =
            cob_git2::Repository::open(&self.repository_path).map_err(super::error::git_failure)?;
        let sigrefs = super::protocol::reference_target(
            &repository,
            &format!("refs/namespaces/{}/refs/rad/sigrefs", self.signer_node_id),
        )?;
        let profile = self.profile(sigrefs.to_string(), revision.native_id.as_str().to_owned());
        admit_source_profile(&profile)?;
        Ok(profile)
    }
}

// r[impl kiln.radicle.shell_profile]
// r[impl kiln.radicle.shell_nonclaims]
pub fn admit_source_profile(profile: &RadicleSourceProfile) -> PortResult<SourceProviderProfile> {
    if profile.schema != RADICLE_SOURCE_PROFILE_SCHEMA
        || profile.contract_revision != PINNED_SOURCE_CONTRACT_REVISION
        || profile.cohort != RadicleSourceCohort::pinned()
        || !profile.repository_path.is_absolute()
        || profile.allow_network
        || profile.allow_credentials
        || profile.allow_signing
        || profile.allow_mutation
        || profile.allow_collaboration
        || profile.non_claims
            != REQUIRED_SOURCE_NON_CLAIMS
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
    {
        return Err(domain(
            "radicle_source_profile",
            "radicle_source.profile",
            "Radicle source profile differs from the pinned read-only cohort",
        ));
    }
    admit_request(&profile.request).map_err(|_| {
        domain(
            "radicle_source_request",
            "radicle_source.request",
            "Radicle source request failed contract admission",
        )
    })?;
    if profile.request.profile_id.is_empty() {
        return Err(domain(
            "radicle_source_profile_id",
            "radicle_source.request.profile_id",
            "Radicle source profile identity is empty",
        ));
    }
    let capabilities = BTreeSet::from([
        SourceCapability::Discover,
        SourceCapability::ResolveExact,
        SourceCapability::Acquire,
        SourceCapability::ObserveIdentity,
    ]);
    let admitted = SourceProviderProfile {
        id: ProviderProfileId::new(profile.request.profile_id.clone())?,
        native_system: ExternalRef::new(
            "radicle",
            "source-provider",
            PINNED_SOURCE_CONTRACT_REVISION,
        )?,
        capabilities: capabilities.clone(),
        identity_namespaces: BTreeSet::from([
            Alias::new("radicle-revision")?,
            Alias::new("radicle-sigrefs")?,
        ]),
        immutable_resolution: true,
    };
    for capability in capabilities {
        admit_source_capability(&admitted, capability, &admitted.id)?;
    }
    Ok(admitted)
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
