#![forbid(unsafe_code)]

mod auth;
mod coordinator;
pub mod observation;
pub mod probe;
mod storage;

use kiln_adapter_aspen::AspenRuntimeProfile;
use kiln_core::build::{BuildEffectPlan, BuildEvaluationObservation};
use kiln_core::discovery::{DiscoveryEffectPlan, DiscoveryObservation};
use kiln_core::{Alias, AttemptId, ContentId, RunId};
use kiln_provider_api::TriggerRequest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use auth::{authenticate, keygen, now_unix, prepare_request, sign};
pub use coordinator::{PilotJournal, reconcile, replay, start};
pub use storage::{load_json, publish_new};
pub type Result<T, E = String> = std::result::Result<T, E>;
pub const PROFILE_SCHEMA: &str = "kiln.onixos-pilot.profile.v1";
pub const REQUEST_SCHEMA: &str = "kiln.onixos-pilot.operator-request.v1";
pub const WORKER_RESULT_SCHEMA: &str = "kiln.onixos-pilot.worker-result.v1";
pub const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;
pub const KEY_BYTES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotProfile {
    pub schema: String,
    pub allowed_operator_keys: Vec<String>,
    pub cohort_identity: ContentId,
    pub source_uri: String,
    pub source_identity: ContentId,
    pub job_identity: ContentId,
    pub selected_task: Alias,
    pub max_request_age_seconds: u64,
    pub max_request_bytes: usize,
    pub max_journal_bytes: usize,
    pub max_journal_records: usize,
    pub socket_path: PathBuf,
    pub transport_timeout_seconds: u64,
    pub maximum_frame_bytes: usize,
    pub runtime_profile: AspenRuntimeProfile,
    pub discovery_plan: DiscoveryEffectPlan,
    pub build_plan: BuildEffectPlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorRequest {
    pub schema: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub cohort_identity: ContentId,
    pub source_uri: String,
    pub source_identity: ContentId,
    pub job_identity: ContentId,
    pub pilot_profile_identity: ContentId,
    pub runtime_profile_identity: ContentId,
    pub run_id: RunId,
    pub attempt_id: AttemptId,
    pub trigger: TriggerRequest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRequest {
    pub request: OperatorRequest,
    pub public_key: String,
    pub signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub schema: String,
    pub job_identity: ContentId,
    pub source_uri: String,
    pub discovery_plan: DiscoveryEffectPlan,
    pub discovery_observation: DiscoveryObservation,
    pub build_plan: Option<BuildEffectPlan>,
    pub build_observation: Option<BuildEvaluationObservation>,
}

pub fn identity<T: Serialize>(value: &T) -> Result<ContentId> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
        .map_err(|error| format!("identity: {error:?}"))
}

impl PilotProfile {
    pub fn admit(&self) -> Result<kiln_adapter_aspen::AdmittedAspenRuntimeProfile> {
        if self.schema != PROFILE_SCHEMA
            || self.allowed_operator_keys.is_empty()
            || self.max_request_age_seconds == 0
            || self.max_request_bytes == 0
            || self.max_request_bytes > MAX_CONFIG_BYTES
            || self.max_journal_bytes == 0
            || self.max_journal_bytes > MAX_CONFIG_BYTES
            || self.max_journal_records == 0
            || self.transport_timeout_seconds == 0
            || self.maximum_frame_bytes == 0
        {
            return Err("invalid pilot profile schema or bounds".into());
        }
        kiln_core::discovery::validate_discovery_plan(&self.discovery_plan)
            .map_err(|error| format!("discovery plan: {error:?}"))?;
        kiln_core::build::validate_build_effect_plan(
            &self.build_plan,
            kiln_core::types::DEFAULT_MAX_COLLECTION_ITEMS,
        )
        .map_err(|error| format!("build plan: {error:?}"))?;
        if self.discovery_plan.source_identity != self.source_identity
            || self.discovery_plan.build_provider_profile != self.build_plan.provider_profile
            || self.discovery_plan.system != self.build_plan.platform
        {
            return Err("pilot source or provider differs from its admitted plans".into());
        }
        kiln_adapter_aspen::admit_profile(&self.runtime_profile)
            .map_err(|error| format!("runtime profile: {error:?}"))
    }
}
