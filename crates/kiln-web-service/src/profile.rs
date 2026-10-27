use std::collections::BTreeSet;
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};

use kiln_adapter_github::profile::WorkflowConfig;
use kiln_adapter_http_observe::TlsProfile;
use kiln_adapter_lattice::{LatticeAdapterLimits, LatticeProfile};
use kiln_core::source::GitSnapshotLimits;
use kiln_core::types::ProviderProfileId;
use lattice_workflow_transport::{
    PeerPolicy, REQUIRED_NON_CLAIMS, TRANSPORT_PROFILE_SCHEMA, TransportProfile,
    validate_transport_profile,
};
use serde::{Deserialize, Serialize};

pub const PROFILE_SCHEMA: &str = "kiln.web-service.v1";
pub const MAX_PROFILE_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceBounds {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_header_bytes: usize,
    pub max_connections: usize,
    pub timeout_seconds: u64,
    pub poll_interval_ms: u64,
    pub max_projects: usize,
    pub max_runs: usize,
    pub max_artifact_bytes: usize,
    pub max_artifacts: usize,
    pub max_run_artifact_bytes: usize,
    pub max_total_artifact_bytes: usize,
    pub max_log_read_bytes: usize,
    pub max_requests: usize,
    pub max_events_per_run: usize,
    pub max_state_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectProfile {
    pub id: String,
    pub label: String,
    pub repository_path: PathBuf,
    pub reference: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admitted_references: Vec<String>,
    pub source_limits: GitSnapshotLimits,
    pub workflow: WorkflowConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceProfile {
    pub schema: String,
    pub service_id: String,
    pub listen_address: SocketAddr,
    pub origin: String,
    pub tls: TlsProfile,
    pub operator_token_file: PathBuf,
    pub worker_token_file: PathBuf,
    pub state_directory: PathBuf,
    pub projects: Vec<ProjectProfile>,
    pub bounds: ServiceBounds,
}

pub(crate) fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}
pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn read_token(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = crate::worker::files::regular(path, 64, true)
        .map_err(|_| "credential must be a bounded owned private regular file")?;
    if bytes.len() != 64 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err("credential must contain exactly 64 ASCII hex bytes".into());
    }
    Ok(bytes)
}

impl ProjectProfile {
    pub fn reviewed_request(&self) -> Result<serde_json::Value, String> {
        self.reviewed_request_for_nodes(&[])
    }
    pub fn reviewed_request_for_nodes(
        &self,
        selected_nodes: &[String],
    ) -> Result<serde_json::Value, String> {
        let id = crate::engine::identity(&self.workflow)?;
        let plan = crate::engine::model::selected_plan(
            crate::engine::model::workflow_plan(self, &id, &id, None)?,
            selected_nodes,
        )?;
        let request = kiln_adapter_lattice::lower_request(&self.lattice(1)?, &plan)
            .map_err(|_| "workflow lowering rejected")?;
        serde_json::to_value(request).map_err(|_| "workflow request encoding failed".into())
    }
    pub fn lattice(&self, maximum_runs: usize) -> Result<LatticeProfile, String> {
        LatticeProfile::pinned(
            ProviderProfileId::new(self.workflow.provider_profile_id.clone())
                .map_err(|_| "invalid provider identity")?,
            LatticeAdapterLimits {
                workflow: self.workflow.bounds,
                max_request_bytes: self.workflow.adapter_limits.maximum_request_bytes,
                max_response_bytes: self.workflow.adapter_limits.maximum_response_bytes,
                max_in_flight: maximum_runs,
            },
        )
        .map_err(|_| "invalid Lattice profile".into())
    }
    pub fn transport(&self) -> Result<TransportProfile, String> {
        let profile = TransportProfile {
            schema: TRANSPORT_PROFILE_SCHEMA.into(),
            limits: self.workflow.transport_limits,
            peers: PeerPolicy {
                allowed_user_ids: self.workflow.allowed_user_ids.clone(),
            },
            non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
        };
        validate_transport_profile(&profile).map_err(|_| "invalid workflow transport profile")?;
        Ok(profile)
    }
}

impl ServiceProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|_| "profile cannot be opened")?;
        let mut bytes = Vec::new();
        file.take((MAX_PROFILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "profile cannot be read")?;
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err("profile exceeds byte bound".into());
        }
        let mut ignored = false;
        let mut decoder = serde_json::Deserializer::from_slice(&bytes);
        let profile: Self = serde_ignored::deserialize(&mut decoder, |_| ignored = true)
            .map_err(|_| "profile is not a typed service profile")?;
        decoder.end().map_err(|_| "profile has trailing input")?;
        if ignored {
            return Err("profile contains unsupported nested fields".into());
        }
        profile.admit()?;
        Ok(profile)
    }
    pub fn admit(&self) -> Result<(), String> {
        let b = &self.bounds;
        if self.schema != PROFILE_SCHEMA
            || !identifier(&self.service_id)
            || self.listen_address.port() == 0
            || !absolute(&self.state_directory)
            || !absolute(&self.operator_token_file)
            || !absolute(&self.worker_token_file)
            || self.operator_token_file == self.worker_token_file
            || self.tls.min_version != "TLS1.3"
            || !absolute(Path::new(&self.tls.certificate_path))
            || !absolute(Path::new(&self.tls.private_key_path))
        {
            return Err("invalid service schema, identity, TLS or authority paths".into());
        }
        let authority = self
            .origin
            .strip_prefix("https://")
            .ok_or("origin must use HTTPS")?;
        if authority.is_empty()
            || authority.contains(['/', '?', '#', '@'])
            || authority
                .bytes()
                .any(|c| c.is_ascii_whitespace() || c.is_ascii_control())
        {
            return Err("origin must be one registered HTTPS authority".into());
        }
        if [
            b.max_request_bytes,
            b.max_response_bytes,
            b.max_header_bytes,
            b.max_connections,
            b.max_projects,
            b.max_runs,
            b.max_artifact_bytes,
            b.max_artifacts,
            b.max_run_artifact_bytes,
            b.max_total_artifact_bytes,
            b.max_log_read_bytes,
            b.max_requests,
            b.max_events_per_run,
            b.max_state_bytes,
        ]
        .contains(&0)
            || b.timeout_seconds == 0
            || b.poll_interval_ms == 0
            || b.max_connections > 64
            || b.max_request_bytes > 67_108_864
            || b.max_response_bytes < 1024
            || b.max_header_bytes > 65_536
            || b.max_events_per_run < 4
            || b.max_requests < b.max_runs
            || b.max_state_bytes < MAX_PROFILE_BYTES
            || b.max_artifact_bytes > b.max_run_artifact_bytes
            || b.max_run_artifact_bytes > b.max_total_artifact_bytes
            || b.max_artifact_bytes
                .checked_mul(4)
                .and_then(|n| n.checked_div(3))
                .and_then(|n| n.checked_add(4096))
                .is_none_or(|n| n > b.max_request_bytes)
            || self.projects.is_empty()
            || self.projects.len() > b.max_projects
        {
            return Err("invalid named service bounds".into());
        }
        let mut ids = BTreeSet::new();
        for project in &self.projects {
            if !identifier(&project.id)
                || project.id.len() > 100
                || !ids.insert(&project.id)
                || project.label.is_empty()
                || project.label.len() > 256
                || !absolute(&project.repository_path)
                || project.reference.is_empty()
                || project.reference.len() > 1024
                || !absolute(&project.workflow.socket_path)
                || project.workflow.bounds.max_log_read_bytes > b.max_log_read_bytes
                || [
                    project.source_limits.max_entries,
                    project.source_limits.max_path_bytes,
                    project.source_limits.max_file_bytes,
                    project.source_limits.max_total_bytes,
                ]
                .contains(&0)
            {
                return Err("invalid or duplicate allowlisted project".into());
            }
            let mut references = BTreeSet::new();
            if project.admitted_references.len() > 64
                || project.admitted_references.iter().any(|r| {
                    r.is_empty()
                        || r.len() > 1024
                        || r.starts_with('-')
                        || r.bytes()
                            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
                        || !references.insert(r)
                })
            {
                return Err("invalid admitted source references".into());
            }
            let lattice = project.lattice(b.max_runs)?;
            kiln_adapter_lattice::admit_profile(&lattice)
                .map_err(|_| "unsupported Lattice profile")?;
            project.transport()?;
            let identity = crate::engine::identity(&project.workflow)?;
            let plan = crate::engine::model::workflow_plan(project, &identity, &identity, None)?;
            kiln_adapter_lattice::lower_request(&lattice, &plan)
                .map_err(|_| "unsupported reviewed workflow")?;
        }
        Ok(())
    }
}
