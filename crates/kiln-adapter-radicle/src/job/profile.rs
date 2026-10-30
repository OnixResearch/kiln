use std::collections::BTreeSet;
use std::ops::Deref;

use kiln_core::types::ExternalRef;
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

use crate::status::profile::{
    PinnedSecretSpecExecutable, RadicleStatusProfile, SecretSpecDeclaration, admit_profile_for,
};

/// Pinned profile id for Tangled/Radicle job publication.
pub const PINNED_JOB_PROFILE_ID: &str = "radicle-job-v1";
/// The Tangled/Radicle CI job object type.
pub const JOB_TYPE_NAME: &str = "xyz.radworks.job";
/// Default bound on serialized actions admitted for one publication.
pub const DEFAULT_MAX_JOB_PAYLOAD_BYTES: usize = 65_536;
/// Default bound on one log URL.
pub const DEFAULT_MAX_LOG_URL_BYTES: usize = 2_048;

const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "job writes do not complete Kiln attempts or mutate canonical state",
    "job writes do not prove peer convergence or remote visibility",
    "job writes do not prove source trust or actor authority",
    "SecretSpec resolution does not grant merge authority",
    "the signer identity does not prove operator intent beyond one admitted plan",
    "equal job objects do not prove equal run outcomes elsewhere",
    "passing conformance does not prove node availability",
];

// r[impl kiln.radicle.job.authority]
/// A typed Radicle job publisher profile.
///
/// The wrapper pins the object type and profile identity to job semantics while
/// reusing the reviewed cohort, signer and executable admission of the status
/// profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RadicleJobProfile(RadicleStatusProfile);

impl RadicleJobProfile {
    #[must_use]
    pub fn pinned(
        allowed_rids: BTreeSet<String>,
        signer_identity: ExternalRef,
        signer: SecretSpecDeclaration,
        executable: PinnedSecretSpecExecutable,
    ) -> Self {
        Self(RadicleStatusProfile::pinned_for(
            PINNED_JOB_PROFILE_ID,
            JOB_TYPE_NAME,
            &REQUIRED_NON_CLAIMS,
            allowed_rids,
            signer_identity,
            signer,
            executable,
        ))
    }

    pub fn admit(&self) -> PortResult<BTreeSet<String>> {
        admit_profile_for(
            &self.0,
            PINNED_JOB_PROFILE_ID,
            JOB_TYPE_NAME,
            &REQUIRED_NON_CLAIMS,
            true,
        )
    }

    #[cfg(test)]
    pub(crate) fn inner_mut(&mut self) -> &mut RadicleStatusProfile {
        &mut self.0
    }

    #[cfg(feature = "status-native-fixtures")]
    pub(crate) fn admit_fixture(&self) -> PortResult<BTreeSet<String>> {
        admit_profile_for(
            &self.0,
            PINNED_JOB_PROFILE_ID,
            JOB_TYPE_NAME,
            &REQUIRED_NON_CLAIMS,
            false,
        )
    }
}

impl Deref for RadicleJobProfile {
    type Target = RadicleStatusProfile;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
