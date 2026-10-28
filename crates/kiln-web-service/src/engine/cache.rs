use super::api::{error, json_response};
use super::{ApiResponse, Engine, Role};
use crate::cache::{CacheEnvelope, CacheKey, CacheProfile, CacheStore};
use kiln_core::types::{AttemptId, ExternalRef};
use kiln_core::workflow::WorkflowState;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    attempt_id: AttemptId,
    #[serde(deserialize_with = "super::api::deserialize_execution")]
    execution: ExternalRef,
    source_revision: String,
    node_id: String,
    key: CacheKey,
    #[serde(default)]
    entry: Option<CacheEnvelope>,
    #[serde(default)]
    observed_success: bool,
}
fn active(run: &super::model::RunRecord, request: &Request) -> bool {
    run.project_id == request.key.project_id
        && request.key.node_id == request.node_id
        && run.plan.attempt_id == request.attempt_id
        && run.source.exact_revision.native_id.as_str() == request.source_revision
        && run.phase == "active"
        && run.error_code.is_none()
        && run.observation.as_ref().is_some_and(|o| {
            o.execution == request.execution && !o.terminal && o.state != WorkflowState::Unknown
        })
        && run
            .plan
            .nodes
            .iter()
            .any(|n| n.id.as_str() == request.node_id)
}
impl Engine {
    pub fn install_cache(&mut self, profile: CacheProfile) -> Result<(), String> {
        profile.preflight_for(&self.profile)?;
        if self.cache_store.is_some() {
            return Err("cache already installed".into());
        }
        self.cache_store = Some(CacheStore::open(profile)?);
        Ok(())
    }
    pub(super) fn cache_request(
        &mut self,
        role: Role,
        actor: &str,
        id: &str,
        operation: &str,
        body: &[u8],
    ) -> Result<ApiResponse, String> {
        // Authorization precedes decoding/cache reads/provider calls.
        if role != Role::Worker || actor.is_empty() {
            return error(403, "cache_worker_scope");
        }
        let Some(store) = self.cache_store.as_ref() else {
            return error(404, "cache_not_configured");
        };
        if !matches!(operation, "restore" | "publish") {
            return error(404, "cache_operation_missing");
        }
        if body.len() > store.profile.maximum_wire_bytes() {
            return error(400, "cache_request_bound");
        }
        let request: Request = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "cache_request_invalid"),
        };
        if !store.profile.admits_key(&request.key) {
            return error(403, "cache_key_scope");
        }
        let Some(run) = self.state.runs.get(id) else {
            return error(404, "run_missing");
        };
        if !active(run, &request) {
            return error(409, "cache_stale_identity");
        }
        self.refresh(id)?;
        if self
            .state
            .runs
            .get(id)
            .is_none_or(|run| !active(run, &request))
        {
            return error(409, "cache_stale_identity");
        }
        let now = super::host_time_ms()? / 1000;
        let store = self.cache_store.as_mut().ok_or("cache disappeared")?;
        match operation {
            "restore" => {
                if request.entry.is_some() || request.observed_success {
                    return error(400, "cache_restore_invalid");
                }
                match store.restore(&request.key, now) {
                    Ok(entry) => json_response(200, json!({"entry":entry})),
                    Err(_) => error(409, "cache_restore_integrity_or_policy"),
                }
            }
            "publish" => {
                // Same authenticated worker observer authority as step telemetry:
                // publication requires observed child success AND admitted ingress.
                let Some(entry) = request.entry else {
                    return error(400, "cache_publish_missing");
                };
                if !request.observed_success || entry.entry.key != request.key {
                    return error(409, "cache_publish_not_successful_or_key_conflict");
                }
                match store.publish(&entry, now) {
                    Ok(created) => json_response(
                        if created { 201 } else { 200 },
                        json!({"published":true,"blake3":entry.blake3}),
                    ),
                    Err(_) => error(409, "cache_publish_integrity_policy_or_conflict"),
                }
            }
            _ => error(404, "cache_operation_missing"),
        }
    }
}
