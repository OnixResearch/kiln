use super::api::{error, json_response};
use super::model::AuditRecord;
use super::{ApiResponse, Engine, Role, host_time_ms};
use crate::access::Principal;
use serde_json::json;

impl Engine {
    pub fn record_denial(
        &mut self,
        method: &str,
        target: &str,
        principal: Option<&Principal>,
        code: &str,
    ) -> Result<(), String> {
        self.append_audit(method, target, principal, code, 403)
    }
    pub fn configure_access_identity(&mut self, id: &str) -> Result<(), String> {
        if !super::storage::hex_identity(id) {
            return Err("access policy identity must be canonical BLAKE3".into());
        }
        self.state.access_policy_identity = Some(id.into());
        let principal = Principal {
            id: "system:access-configuration".into(),
            label: "Runtime access admission".into(),
            role: Role::Operator,
        };
        self.append_audit(
            "CONFIGURE",
            &format!("/runtime/access/{id}"),
            Some(&principal),
            "configured",
            200,
        )
    }
    pub(super) fn append_audit(
        &mut self,
        method: &str,
        target: &str,
        principal: Option<&Principal>,
        decision: &str,
        status: u16,
    ) -> Result<(), String> {
        if self.storage_failed {
            return Err("durable state unavailable".into());
        }
        let role = principal
            .map(|p| match p.role {
                Role::Viewer => "viewer",
                Role::Operator => "operator",
                Role::Worker => "worker",
            })
            .unwrap_or("unauthenticated");
        let path = target.split('?').next().unwrap_or("");
        let run_id = path
            .strip_prefix("/api/runs/")
            .and_then(|p| p.split('/').next())
            .filter(|id| self.state.runs.contains_key(*id))
            .map(str::to_owned);
        let bound = self.profile.bounds.max_requests.saturating_mul(4).max(64);
        if self.state.audit.len() >= bound {
            self.state.audit.remove(0);
        }
        self.state.audit.push(AuditRecord {
            ordinal: self.state.version,
            recorded_at_ms: host_time_ms()?,
            actor_id: principal
                .map(|p| p.id.chars().take(256).collect())
                .unwrap_or_else(|| "anonymous".into()),
            actor_label: principal
                .map(|p| p.label.chars().take(256).collect())
                .unwrap_or_else(|| "Unauthenticated".into()),
            role: role.into(),
            method: method.chars().take(16).collect(),
            target: path.chars().take(512).collect(),
            decision: decision.chars().take(128).collect(),
            status,
            run_id,
        });
        self.persist()
    }
    pub fn api_as(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        if self.storage_failed {
            return error(503, "durable_state_unavailable");
        }
        let write = !matches!(method, "GET" | "HEAD");
        if principal.role == Role::Viewer && write {
            self.append_audit(method, target, Some(principal), "viewer_read_only", 403)?;
            return error(403, "viewer_read_only");
        }
        if principal.id.is_empty()
            || principal.id.len() > 256
            || principal.label.is_empty()
            || principal.label.len() > 256
        {
            self.append_audit(method, target, Some(principal), "principal_invalid", 403)?;
            return error(403, "principal_invalid");
        }
        if method == "GET" && target == "/api/session" && body.is_empty() {
            return json_response(
                200,
                json!({"principal":principal,"permissions":{"read":true,"operate":principal.role==Role::Operator,"configure":principal.role==Role::Operator}}),
            );
        }
        if write {
            self.append_audit(method, target, Some(principal), "admitted", 0)?;
        }
        let response = self.route(method, target, body, principal.role)?;
        if write || response.status == 403 {
            self.append_audit(
                method,
                target,
                Some(principal),
                if response.status < 400 {
                    "accepted"
                } else {
                    "denied"
                },
                response.status,
            )?;
            if write && response.status < 400 && target == "/api/runs" {
                if let Some(id) = serde_json::from_slice::<serde_json::Value>(&response.body)
                    .ok()
                    .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
                {
                    if self.state.runs.contains_key(&id) {
                        if let Some(audit) = self.state.audit.last_mut() {
                            audit.run_id = Some(id);
                        }
                        self.persist()?;
                    }
                }
            }
        }
        let maximum = if response.content_type == "application/octet-stream" {
            if target
                .split('?')
                .next()
                .is_some_and(|p| p.ends_with("/logs/download"))
            {
                self.profile
                    .projects
                    .iter()
                    .map(|p| p.workflow.bounds.max_total_log_bytes)
                    .max()
                    .unwrap_or(0)
            } else {
                self.profile.bounds.max_artifact_bytes
            }
        } else {
            self.profile.bounds.max_response_bytes
        };
        if response.body.len() > maximum {
            return error(503, "response_bound");
        }
        Ok(response)
    }
}
