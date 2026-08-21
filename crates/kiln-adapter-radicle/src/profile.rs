use std::path::PathBuf;

use kiln_adapter_lattice::{
    LatticeAdapterLimits, LatticeProfile, PINNED_LATTICE_CONTRACT_LICENSE, PINNED_LATTICE_REVISION,
    PINNED_LATTICE_SOURCE,
};
use kiln_core::types::{Alias, ContentId, Diagnostic, ProviderProfileId};
use kiln_core::workflow::{
    WorkflowBounds, WorkflowControls, WorkflowEdge, WorkflowEffectClass, WorkflowNode,
};
use kiln_provider_api::{PortFailure, PortResult};
use lattice_workflow_transport::{
    PeerPolicy, REQUIRED_NON_CLAIMS as TRANSPORT_NON_CLAIMS, TRANSPORT_PROFILE_SCHEMA,
    TransportLimits, TransportProfile, validate_transport_profile,
};
use serde::{Deserialize, Serialize};

pub const RADICLE_SHELL_SCHEMA: &str = "kiln.radicle-workflow-shell.v1";
pub const PINNED_RADICLE_RUNTIME_REVISION: &str = "radicle-0.24.0";
pub const PINNED_RADICLE_BROKER_REVISION: &str = "a688afde1f730ef60402a809d9cdadc55e8dd749";
pub const PINNED_RADICLE_LICENSE: &str = "MIT OR Apache-2.0";
pub const REQUIRED_NON_CLAIMS: [&str; 6] = [
    "broker admission does not prove event completeness",
    "source context does not prove source trust or revision availability",
    "workflow acceptance does not prove an external effect started",
    "workflow observation does not replace canonical Kiln CI state",
    "local replay admission does not prove distributed exactly-once delivery",
    "adapter checks do not establish release readiness",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    Production,
    Fixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySelection {
    pub source: bool,
    pub trigger: bool,
    pub status: bool,
    pub collaboration: bool,
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
pub struct WorkflowTemplate {
    pub policy_id: String,
    pub provider_profile_id: String,
    pub platform: String,
    pub nodes: Vec<ConfigNode>,
    pub edges: Vec<ConfigEdge>,
    pub controls: WorkflowControls,
    pub bounds: WorkflowBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterLimits {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_in_flight: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleShellConfig {
    pub schema: String,
    pub mode: RuntimeMode,
    pub radicle_runtime_revision: String,
    pub radicle_broker_revision: String,
    pub radicle_license: String,
    pub lattice_source: String,
    pub lattice_revision: String,
    pub lattice_license: String,
    pub capabilities: CapabilitySelection,
    pub socket_path: PathBuf,
    pub replay_database_path: PathBuf,
    pub maximum_replay_records: usize,
    pub maximum_polls: usize,
    pub allowed_user_ids: Vec<u32>,
    pub transport_limits: TransportLimits,
    pub adapter_limits: AdapterLimits,
    pub workflow: WorkflowTemplate,
    pub non_claims: Vec<String>,
}

#[derive(Debug)]
pub struct AdmittedRadicleConfig {
    pub shell: RadicleShellConfig,
    pub lattice: LatticeProfile,
    pub transport: TransportProfile,
}

// r[impl kiln.radicle.protocol]
// r[impl kiln.radicle.nonclaims]
pub fn admit_config(config: RadicleShellConfig) -> PortResult<AdmittedRadicleConfig> {
    if config.schema != RADICLE_SHELL_SCHEMA
        || config.mode != RuntimeMode::Production
        || config.radicle_runtime_revision != PINNED_RADICLE_RUNTIME_REVISION
        || config.radicle_broker_revision != PINNED_RADICLE_BROKER_REVISION
        || config.radicle_license != PINNED_RADICLE_LICENSE
        || config.lattice_source != PINNED_LATTICE_SOURCE
        || config.lattice_revision != PINNED_LATTICE_REVISION
        || config.lattice_license != PINNED_LATTICE_CONTRACT_LICENSE
    {
        return Err(domain(
            "radicle_profile_pin",
            "radicle.profile",
            "Radicle shell profile differs from the reviewed runtime cohort",
        ));
    }
    if !config.capabilities.trigger
        || config.capabilities.source
        || config.capabilities.status
        || config.capabilities.collaboration
    {
        return Err(domain(
            "radicle_profile_capability",
            "radicle.capabilities",
            "this shell admits only the concrete trigger-to-workflow capability",
        ));
    }
    if !config.socket_path.is_absolute()
        || !config.replay_database_path.is_absolute()
        || config.maximum_replay_records == 0
        || config.maximum_polls == 0
        || config.allowed_user_ids.is_empty()
        || config.workflow.nodes.is_empty()
        || config.non_claims != REQUIRED_NON_CLAIMS
    {
        return Err(domain(
            "radicle_profile_value",
            "radicle.profile",
            "Radicle shell profile contains an invalid path, bound, graph, or non-claim",
        ));
    }
    let workflow_bounds = config.workflow.bounds;
    let lattice_limits = LatticeAdapterLimits {
        workflow: workflow_bounds,
        max_request_bytes: config.adapter_limits.max_request_bytes,
        max_response_bytes: config.adapter_limits.max_response_bytes,
        max_in_flight: config.adapter_limits.max_in_flight,
    };
    let lattice = LatticeProfile::pinned(
        ProviderProfileId::new(config.workflow.provider_profile_id.clone())?,
        lattice_limits,
    )?;
    let transport = TransportProfile {
        schema: TRANSPORT_PROFILE_SCHEMA.to_string(),
        limits: config.transport_limits,
        peers: PeerPolicy {
            allowed_user_ids: config.allowed_user_ids.clone(),
        },
        non_claims: TRANSPORT_NON_CLAIMS.map(str::to_string).to_vec(),
    };
    validate_transport_profile(&transport).map_err(|_| {
        domain(
            "radicle_transport_profile",
            "radicle.transport",
            "Radicle shell transport profile is invalid",
        )
    })?;
    let _policy = kiln_core::types::PolicyId::new(config.workflow.policy_id.clone())?;
    let _platform = Alias::new(config.workflow.platform.clone())?;
    let _nodes = workflow_nodes(&config.workflow)?;
    let _edges = workflow_edges(&config.workflow)?;
    Ok(AdmittedRadicleConfig {
        shell: config,
        lattice,
        transport,
    })
}

pub fn workflow_nodes(template: &WorkflowTemplate) -> PortResult<Vec<WorkflowNode>> {
    template
        .nodes
        .iter()
        .map(|node| {
            Ok(WorkflowNode {
                id: Alias::new(node.id.clone())?,
                kind: Alias::new(node.kind.clone())?,
                effect_class: node.effect_class.into(),
                adapter_kind: Alias::new(node.adapter_kind.clone())?,
                operation_identity: ContentId::from_blake3_hex(node.operation_identity.clone())?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(PortFailure::from)
}

pub fn workflow_edges(template: &WorkflowTemplate) -> PortResult<Vec<WorkflowEdge>> {
    template
        .edges
        .iter()
        .map(|edge| {
            Ok(WorkflowEdge {
                source: Alias::new(edge.source.clone())?,
                target: Alias::new(edge.target.clone())?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(PortFailure::from)
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
