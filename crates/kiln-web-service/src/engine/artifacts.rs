use super::api::{error, json_response};
use super::model::ArtifactView;
use super::{ApiResponse, Engine, identity};
use base64::Engine as _;
use kiln_core::types::{AttemptId, ExternalRef};
use kiln_core::workflow::WorkflowState;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    attempt_id: AttemptId,
    #[serde(deserialize_with = "super::api::deserialize_execution")]
    execution: ExternalRef,
    source_revision: String,
    node_id: String,
    name: String,
    body_base64: String,
    blake3: String,
}
fn safe_name(name: &str) -> bool {
    crate::profile::identifier(name) && name != "." && name != ".."
}
impl Engine {
    // r[impl kiln.web.artifacts]
    pub(super) fn upload(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        let upload: Upload = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "artifact_invalid"),
        };
        if !safe_name(&upload.name)
            || upload.blake3.len() != 64
            || !upload
                .blake3
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return error(400, "artifact_identity_invalid");
        }
        let max = self.profile.bounds.max_artifact_bytes;
        if upload.body_base64.len() > max.saturating_add(2).saturating_div(3).saturating_mul(4) {
            return error(400, "artifact_byte_bound");
        }
        let bytes = match base64::engine::general_purpose::STANDARD.decode(&upload.body_base64) {
            Ok(v) if v.len() <= max => v,
            _ => return error(400, "artifact_encoding_or_bound"),
        };
        if blake3::hash(&bytes).to_hex().as_str() != upload.blake3 {
            return error(400, "artifact_hash_conflict");
        }
        self.refresh(id)?;
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if run.plan.attempt_id != upload.attempt_id
            || run.source.exact_revision.native_id.as_str() != upload.source_revision
            || run.phase != "active"
            || run.error_code.is_some()
            || run.observation.as_ref().is_none_or(|o| {
                o.execution != upload.execution || o.terminal || o.state == WorkflowState::Unknown
            })
            || !run
                .plan
                .nodes
                .iter()
                .any(|n| n.id.as_str() == upload.node_id)
        {
            return error(409, "artifact_stale_identity");
        }
        if let Some(prior) = run
            .artifacts
            .iter()
            .find(|a| a.name == upload.name && a.node_id == upload.node_id)
        {
            return if prior.blake3 == upload.blake3 && prior.bytes == bytes.len() {
                json_response(200, prior)
            } else {
                error(409, "artifact_name_conflict")
            };
        }
        let run_bytes = run
            .artifacts
            .iter()
            .try_fold(0usize, |sum, a| sum.checked_add(a.bytes))
            .ok_or("artifact byte counter overflow")?;
        let total_bytes = self
            .state
            .runs
            .values()
            .flat_map(|r| &r.artifacts)
            .try_fold(0usize, |sum, a| sum.checked_add(a.bytes))
            .ok_or("artifact total counter overflow")?;
        let bounds = &self.profile.bounds;
        if run.artifacts.len() >= bounds.max_artifacts
            || run_bytes
                .checked_add(bytes.len())
                .is_none_or(|n| n > bounds.max_run_artifact_bytes)
            || total_bytes
                .checked_add(bytes.len())
                .is_none_or(|n| n > bounds.max_total_artifact_bytes)
        {
            return error(409, "artifact_retention");
        }
        let artifact_id = identity(&(
            "kiln.web-artifact.v1",
            id,
            &upload.attempt_id,
            &upload.execution,
            &upload.node_id,
            &upload.name,
            &upload.blake3,
        ))?
        .as_str()
        .to_string();
        let directory = self.profile.state_directory.join("artifacts");
        if super::storage::publish_retained(&directory, &artifact_id, &bytes).is_err() {
            self.storage_failed = true;
            return error(503, "artifact_storage_unavailable");
        }
        let view = ArtifactView {
            id: artifact_id.clone(),
            name: upload.name,
            node_id: upload.node_id,
            attempt_id: upload.attempt_id,
            bytes: bytes.len(),
            blake3: upload.blake3,
            download_path: format!("/api/runs/{id}/artifacts/{artifact_id}"),
        };
        self.state
            .runs
            .get_mut(id)
            .ok_or("run missing")?
            .artifacts
            .push(view.clone());
        self.persist()?;
        json_response(201, view)
    }
    // r[impl kiln.web.artifacts]
    pub(super) fn download(&self, id: &str, artifact_id: &str) -> Result<ApiResponse, String> {
        let Some(artifact) = self
            .state
            .runs
            .get(id)
            .ok_or("run missing")?
            .artifacts
            .iter()
            .find(|a| a.id == artifact_id)
        else {
            return error(404, "artifact_missing");
        };
        if artifact.id.len() != 64
            || !artifact.id.bytes().all(|b| b.is_ascii_hexdigit())
            || artifact.bytes > self.profile.bounds.max_artifact_bytes
            || !safe_name(&artifact.name)
        {
            return error(503, "artifact_metadata_conflict");
        }
        let bytes = match super::storage::read_retained(
            &self.profile.state_directory.join("artifacts"),
            &artifact.id,
            artifact.bytes,
            &artifact.blake3,
        ) {
            Ok(bytes) => bytes,
            Err(_) => return error(503, "artifact_content_conflict"),
        };
        Ok(ApiResponse {
            status: 200,
            content_type: "application/octet-stream",
            body: bytes,
            download_name: Some(artifact.name.clone()),
        })
    }
}
