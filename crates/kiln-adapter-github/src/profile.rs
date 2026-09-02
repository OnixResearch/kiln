use std::net::SocketAddr;
use std::path::PathBuf;

use kiln_adapter_lattice::{LatticeAdapterLimits, LatticeProfile, lower_request};
use kiln_codec::encode_and_identify;
use kiln_core::types::{Alias, ContentId, ExternalRef, PolicyId, ProviderProfileId};
use kiln_core::workflow::{WorkflowBounds, WorkflowControls, WorkflowEffectClass};
use kiln_provider_api::PortResult;
use lattice_workflow_transport::{
    PeerPolicy, REQUIRED_NON_CLAIMS as TRANSPORT_NON_CLAIMS, TRANSPORT_PROFILE_SCHEMA,
    TransportLimits, TransportProfile, validate_transport_profile,
};
use serde::{Deserialize, Serialize};

use crate::events::{EventPolicy, admit_event_policy};
use crate::secret::{SecretReference, admit_secret_reference};
use crate::status::{StatusConfig, admit_status_config};
use crate::{domain, infrastructure};

pub const GITHUB_PROFILE_SCHEMA: &str = "kiln.github-ci.v1";
pub const MAXIMUM_PROFILE_BYTES: usize = 1_048_576;
pub const MAXIMUM_HEADER_BYTES: usize = 65_536;
pub const MAXIMUM_BODY_BYTES: usize = 33_554_432;
pub const MAXIMUM_ACKNOWLEDGEMENT_MS: u64 = 9_000;
pub const DATABASE_BUSY_TIMEOUT_MS: u64 = 500;
pub const MAXIMUM_HTTP_CONNECTIONS: usize = 64;
pub const REQUIRED_NON_CLAIMS: [&str; 6] = [
    "webhook authentication does not prove delivery freshness",
    "replay admission does not prove distributed exactly-once execution",
    "source identity does not prove source trust or workflow correctness",
    "bounded jobs and temporary checkouts do not provide host confinement",
    "GitHub status publication does not replace canonical Kiln CI state",
    "local checks do not prove deployment or release eligibility",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterLimits {
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigEffectClass {
    Build,
    Test,
}

impl From<ConfigEffectClass> for WorkflowEffectClass {
    fn from(value: ConfigEffectClass) -> Self {
        match value {
            ConfigEffectClass::Build => Self::Build,
            ConfigEffectClass::Test => Self::Test,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigNode {
    pub id: String,
    pub kind: String,
    pub effect_class: ConfigEffectClass,
    pub adapter_kind: String,
    pub operation_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigEdge {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    pub socket_path: PathBuf,
    pub allowed_user_ids: Vec<u32>,
    pub transport_limits: TransportLimits,
    pub adapter_limits: AdapterLimits,
    pub policy_id: String,
    pub provider_profile_id: String,
    pub platform: String,
    pub nodes: Vec<ConfigNode>,
    pub edges: Vec<ConfigEdge>,
    pub controls: WorkflowControls,
    pub bounds: WorkflowBounds,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubConfig {
    pub schema: String,
    pub event_policy: EventPolicy,
    pub listen_address: SocketAddr,
    pub webhook_path: String,
    pub replay_database_path: PathBuf,
    pub maximum_run_records: usize,
    pub maximum_delivery_records: usize,
    pub maximum_record_bytes: usize,
    pub maximum_body_bytes: usize,
    pub maximum_header_bytes: usize,
    pub read_timeout_ms: u64,
    pub write_timeout_ms: u64,
    pub maximum_connections: usize,
    pub worker_poll_interval_ms: u64,
    pub maximum_workflow_polls: usize,
    pub maximum_active_runs: usize,
    pub webhook_secret: SecretReference,
    pub status_secret: SecretReference,
    pub status: StatusConfig,
    pub workflow: WorkflowConfig,
    pub non_claims: Vec<String>,
}

#[derive(Debug)]
pub struct AdmittedGithubConfig {
    pub config: GithubConfig,
    pub identity: ContentId,
    pub pipeline_identity: ContentId,
    pub lattice: LatticeProfile,
    pub transport: TransportProfile,
}

// r[impl kiln.github.profile]
// r[impl kiln.github.nonclaims]
pub fn admit_config(config: GithubConfig) -> PortResult<AdmittedGithubConfig> {
    admit_event_policy(&config.event_policy)?;
    admit_status_config(&config.status)?;
    admit_secret_reference(&config.webhook_secret)?;
    admit_secret_reference(&config.status_secret)?;
    let repository = format!(
        "{}/{}",
        config.event_policy.owner, config.event_policy.repository
    )
    .to_ascii_lowercase();
    ExternalRef::new("github", "repository", &repository)?;
    let acknowledgement_ms = config
        .read_timeout_ms
        .checked_add(config.write_timeout_ms)
        .and_then(|value| value.checked_add(DATABASE_BUSY_TIMEOUT_MS));
    if config.schema != GITHUB_PROFILE_SCHEMA
        || !config.listen_address.ip().is_loopback()
        || config.listen_address.port() == 0
        || !config.webhook_path.starts_with('/')
        || config.webhook_path.len() < 2
        || config.webhook_path.len() > MAXIMUM_HEADER_BYTES
        || config.webhook_path.bytes().any(|byte| {
            byte.is_ascii_control() || byte.is_ascii_whitespace() || matches!(byte, b'?' | b'#')
        })
        || !config.replay_database_path.is_absolute()
        || !config.workflow.socket_path.is_absolute()
        || config.maximum_run_records == 0
        || config.maximum_delivery_records < config.maximum_run_records
        || config.maximum_record_bytes == 0
        || config.maximum_body_bytes == 0
        || config.maximum_body_bytes > MAXIMUM_BODY_BYTES
        || config.maximum_header_bytes == 0
        || config.maximum_header_bytes > MAXIMUM_HEADER_BYTES
        || config.read_timeout_ms == 0
        || config.write_timeout_ms == 0
        || acknowledgement_ms.is_none_or(|value| value > MAXIMUM_ACKNOWLEDGEMENT_MS)
        || config.maximum_connections == 0
        || config.maximum_connections > MAXIMUM_HTTP_CONNECTIONS
        || config.worker_poll_interval_ms == 0
        || config.maximum_workflow_polls == 0
        || config.maximum_active_runs == 0
        || config.maximum_active_runs > config.maximum_run_records
        || config.workflow.controls.pause
        || config.workflow.controls.resume
        || !config
            .status
            .owner
            .eq_ignore_ascii_case(&config.event_policy.owner)
        || !config
            .status
            .repository
            .eq_ignore_ascii_case(&config.event_policy.repository)
        || config.non_claims != REQUIRED_NON_CLAIMS
    {
        return Err(domain(
            "github_profile",
            "github.profile",
            "GitHub profile has an unsupported schema, authority, path, bound or policy",
        ));
    }
    for bound in [
        config.maximum_run_records,
        config.maximum_delivery_records,
        config.maximum_record_bytes,
        config.maximum_workflow_polls,
        config.maximum_connections,
    ] {
        i64::try_from(bound).map_err(|_| {
            domain(
                "github_profile_bound",
                "github.profile",
                "GitHub bound does not fit durable storage counters",
            )
        })?;
    }
    let (_, identity) = encode_and_identify(&config, MAXIMUM_PROFILE_BYTES).map_err(|_| {
        domain(
            "github_profile_identity",
            "github.profile",
            "GitHub profile exceeds its canonical identity bound",
        )
    })?;
    let (_, pipeline_identity) = encode_and_identify(
        &(GITHUB_PROFILE_SCHEMA, &config.workflow),
        MAXIMUM_PROFILE_BYTES,
    )
    .map_err(|_| {
        domain(
            "github_pipeline_identity",
            "github.workflow",
            "GitHub workflow exceeds its canonical identity bound",
        )
    })?;
    let lattice = LatticeProfile::pinned(
        ProviderProfileId::new(config.workflow.provider_profile_id.clone())?,
        LatticeAdapterLimits {
            workflow: config.workflow.bounds,
            max_request_bytes: config.workflow.adapter_limits.maximum_request_bytes,
            max_response_bytes: config.workflow.adapter_limits.maximum_response_bytes,
            max_in_flight: config.maximum_run_records,
        },
    )?;
    let transport = TransportProfile {
        schema: TRANSPORT_PROFILE_SCHEMA.to_string(),
        limits: config.workflow.transport_limits,
        peers: PeerPolicy {
            allowed_user_ids: config.workflow.allowed_user_ids.clone(),
        },
        non_claims: TRANSPORT_NON_CLAIMS.map(str::to_string).to_vec(),
    };
    validate_transport_profile(&transport).map_err(|_| {
        domain(
            "github_transport_profile",
            "github.workflow",
            "GitHub workflow transport profile is invalid",
        )
    })?;
    PolicyId::new(config.workflow.policy_id.clone())?;
    Alias::new(config.workflow.platform.clone())?;
    let validation_plan = crate::service::workflow_plan(
        &config.workflow,
        &identity,
        &pipeline_identity,
        &identity,
        None,
    )?;
    lower_request(&lattice, &validation_plan)?;
    Ok(AdmittedGithubConfig {
        config,
        identity,
        pipeline_identity,
        lattice,
        transport,
    })
}

pub fn load_config(path: &std::path::Path) -> PortResult<AdmittedGithubConfig> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|_| infrastructure("github_profile_open", "GitHub profile could not be opened"))?;
    let mut bytes = Vec::new();
    file.take((MAXIMUM_PROFILE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| infrastructure("github_profile_read", "GitHub profile could not be read"))?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES {
        return Err(domain(
            "github_profile_bound",
            "github.profile",
            "GitHub profile exceeds its byte bound",
        ));
    }
    let config = serde_json::from_slice(&bytes).map_err(|_| {
        domain(
            "github_profile_json",
            "github.profile",
            "GitHub profile JSON is invalid",
        )
    })?;
    admit_config(config)
}
