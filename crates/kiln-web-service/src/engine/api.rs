use super::model::{RequestKind, RequestRecord, RunRecord, workflow_plan};
use super::{ApiResponse, Engine, Role, identity};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::domain::{Command, State};
use kiln_core::types::{Alias, AttemptId, ExternalRef, ProviderProfileId, RunId};
use kiln_core::workflow::{
    WorkflowControlAction, WorkflowControlRequest, WorkflowLogReadRequest, WorkflowLogStream,
    WorkflowSourceContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn json_response(status: u16, value: impl Serialize) -> Result<ApiResponse, String> {
    Ok(ApiResponse {
        status,
        content_type: "application/json",
        body: serde_json::to_vec(&value).map_err(|_| "response encoding failed")?,
        download_name: None,
    })
}
pub(super) fn error(status: u16, code: &str) -> Result<ApiResponse, String> {
    json_response(
        status,
        json!({"code":code,"message":match status {400=>"Request does not match the bounded API contract",403=>"Role is not authorized for this operation",404=>"Requested resource was not found",409=>"Request conflicts with current identity or retained state",_=>"Provider operation is unavailable or ambiguous"}}),
    )
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectRequest {
    project_id: String,
    request_id: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ControlRequest {
    request_id: String,
    attempt_id: AttemptId,
    expected_revision: u64,
    expected_sequence: u64,
    #[serde(deserialize_with = "deserialize_execution")]
    execution: ExternalRef,
    action: String,
}
pub(super) fn deserialize_execution<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ExternalRef, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fields {
        contract_version: u16,
        provider_namespace: kiln_core::types::BoundedText,
        native_kind: kiln_core::types::BoundedText,
        native_id: kiln_core::types::BoundedText,
    }
    let fields = Fields::deserialize(deserializer)?;
    let value = ExternalRef {
        contract_version: fields.contract_version,
        provider_namespace: fields.provider_namespace,
        native_kind: fields.native_kind,
        native_id: fields.native_id,
    };
    if value.validate().is_err()
        || [
            &value.provider_namespace,
            &value.native_kind,
            &value.native_id,
        ]
        .iter()
        .any(|s| s.as_str().is_empty() || s.as_str().len() > kiln_core::types::DEFAULT_MAX_ID_BYTES)
    {
        return Err(serde::de::Error::custom(
            "invalid canonical execution reference",
        ));
    }
    Ok(value)
}
fn query(target: &str) -> Result<(&str, BTreeMap<&str, &str>), ()> {
    if target.len() > 8192 || target.bytes().any(|b| b.is_ascii_control()) {
        return Err(());
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut values = BTreeMap::new();
    if !query.is_empty() {
        for item in query.split('&') {
            let (k, v) = item.split_once('=').ok_or(())?;
            if values.insert(k, v).is_some() {
                return Err(());
            }
        }
    }
    Ok((path, values))
}
fn decoded(value: &str) -> Result<String, ()> {
    let mut bytes = Vec::new();
    let mut chars = value.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let a = chars.next().ok_or(())?;
            let c = chars.next().ok_or(())?;
            let digit = |v: u8| (v as char).to_digit(16).map(|d| d as u8).ok_or(());
            bytes.push(digit(a)? * 16 + digit(c)?);
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).map_err(|_| ())
}
impl Engine {
    pub fn workflow_request(&self, id: &str) -> Result<Value, String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        let project = self
            .profile
            .projects
            .iter()
            .find(|p| p.id == run.project_id)
            .ok_or("project missing")?;
        serde_json::to_value(
            kiln_adapter_lattice::lower_request(
                &project.lattice(self.profile.bounds.max_runs)?,
                &run.plan,
            )
            .map_err(|_| "native request rejected")?,
        )
        .map_err(|_| "native request encoding failed".into())
    }
    fn request_replay(
        &self,
        key: &str,
        fingerprint: &kiln_core::types::ContentId,
    ) -> Result<Option<ApiResponse>, String> {
        if let Some(prior) = self.state.requests.get(key) {
            if &prior.fingerprint != fingerprint {
                return error(409, "request_id_conflict").map(Some);
            }
            if let Some(response) = &prior.response {
                return json_response(prior.status, response).map(Some);
            }
            match prior.kind {
                RequestKind::Start => {
                    return json_response(
                        201,
                        self.state
                            .runs
                            .get(&prior.resource)
                            .ok_or("reserved run missing")?
                            .view(),
                    )
                    .map(Some);
                }
                RequestKind::Enroll => {
                    let project = self
                        .profile
                        .projects
                        .iter()
                        .find(|p| p.id == prior.resource)
                        .ok_or("reserved project missing")?;
                    return json_response(200,json!({"id":project.id,"label":project.label,"repository":project.repository_path,"reference":project.reference,"pipeline_identity":identity(&project.workflow)?})).map(Some);
                }
                RequestKind::Control => {}
            }
            return error(503, "operation_reserved_unknown").map(Some);
        }
        Ok(None)
    }
    fn reserve_request(
        &mut self,
        key: String,
        fingerprint: kiln_core::types::ContentId,
        resource: String,
        kind: RequestKind,
    ) -> Result<(), String> {
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return Err("request retention exhausted".into());
        }
        self.state.requests.insert(
            key,
            RequestRecord {
                fingerprint,
                resource,
                kind,
                response: None,
                status: 503,
            },
        );
        self.persist()
    }
    fn finish_request(
        &mut self,
        key: &str,
        status: u16,
        value: Value,
    ) -> Result<ApiResponse, String> {
        let request = self
            .state
            .requests
            .get_mut(key)
            .ok_or("request reservation missing")?;
        request.response = Some(value.clone());
        request.status = status;
        self.persist()?;
        json_response(status, value)
    }
    pub fn api(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if self.storage_failed {
            return error(503, "durable_state_unavailable");
        }
        let response = self.route(method, target, body, role)?;
        let maximum = if response.content_type == "application/octet-stream" {
            self.profile.bounds.max_artifact_bytes
        } else {
            self.profile.bounds.max_response_bytes
        };
        if response.body.len() > maximum {
            return error(503, "response_bound");
        }
        Ok(response)
    }
    fn route(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if body.len() > self.profile.bounds.max_request_bytes {
            return error(400, "request_bound");
        }
        let (path, params) = match query(target) {
            Ok(v) => v,
            Err(_) => return error(400, "query_invalid"),
        };
        let worker_lookup = method == "GET"
            && path == "/api/runs"
            && params.len() == 2
            && params.contains_key("project_id")
            && params.contains_key("source_revision");
        let run_route = path.strip_prefix("/api/runs/").and_then(|rest| {
            let mut parts = rest.split('/');
            let id = parts.next()?;
            let resource = parts.next();
            let artifact = parts.next();
            if id.is_empty() || parts.next().is_some() {
                None
            } else {
                Some((id, resource, artifact))
            }
        });
        let artifact_upload =
            method == "POST" && matches!(run_route, Some((_, Some("artifacts"), None)));
        let log_upload = method == "POST" && matches!(run_route, Some((_, Some("logs"), None)));
        if role == Role::Worker && !worker_lookup && !artifact_upload && !log_upload {
            return error(403, "worker_scope");
        }
        if role == Role::Operator && (artifact_upload || log_upload) {
            return error(403, "worker_only");
        }
        if method == "GET" && !body.is_empty() {
            return error(400, "unexpected_body");
        }
        if method == "GET" && path == "/api/projects" && params.is_empty() {
            let available:Vec<Value>=self.profile.projects.iter().map(|p|json!({"id":p.id,"label":p.label,"repository":p.repository_path,"reference":p.reference})).collect();
            let enrolled:Vec<Value>=self.profile.projects.iter().filter(|p|self.state.enrolled.contains(&p.id)).map(|p|Ok(json!({"id":p.id,"label":p.label,"repository":p.repository_path,"reference":p.reference,"pipeline_identity":identity(&p.workflow)?}))).collect::<Result<_,String>>()?;
            return json_response(200, json!({"available":available,"enrolled":enrolled}));
        }
        if method == "POST" && params.is_empty() && (path == "/api/projects" || path == "/api/runs")
        {
            let req: ProjectRequest = match serde_json::from_slice(body) {
                Ok(v) => v,
                Err(_) => return error(400, "request_invalid"),
            };
            if !crate::profile::identifier(&req.request_id) {
                return error(400, "request_id_invalid");
            }
            let fingerprint = identity(&(path, &req))?;
            if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
                return Ok(response);
            }
            if self.state.requests.len() >= self.profile.bounds.max_requests {
                return error(409, "request_retention");
            }
            let project = match self
                .profile
                .projects
                .iter()
                .find(|p| p.id == req.project_id)
                .cloned()
            {
                Some(v) => v,
                None => return error(404, "project_not_allowlisted"),
            };
            if path == "/api/projects" {
                self.state.enrolled.insert(project.id.clone());
                self.reserve_request(
                    req.request_id.clone(),
                    fingerprint,
                    project.id.clone(),
                    RequestKind::Enroll,
                )?;
                return self.finish_request(&req.request_id,200,json!({"id":project.id,"label":project.label,"repository":project.repository_path,"reference":project.reference,"pipeline_identity":identity(&project.workflow)?}));
            }
            if !self.state.enrolled.contains(&project.id) {
                return error(409, "project_not_enrolled");
            }
            if self
                .state
                .runs
                .values()
                .any(|r| r.project_id == project.id && !r.outcome().is_terminal())
            {
                return error(409, "project_active");
            }
            if self.state.runs.len() >= self.profile.bounds.max_runs {
                return error(409, "run_retention");
            }
            let provider = ProviderProfileId::new(format!("web-git-{}", project.id))
                .map_err(|_| "source profile identity invalid")?;
            let adapter = LocalGitAdapter::new(
                project.repository_path.clone(),
                project.reference.clone(),
                GitSourceProfile::pinned_local(provider.clone(), project.source_limits.clone()),
            );
            let source = match adapter.and_then(|a| resolve_and_acquire(&a, provider)) {
                Ok(v) => v,
                Err(_) => return error(503, "source_unavailable"),
            };
            let pipeline = identity(&project.workflow)?;
            let run_identity = identity(&(&self.profile.service_id, &req, &source, &pipeline))?;
            let id = format!("web-run-{}", run_identity.as_str());
            let source_context = WorkflowSourceContext {
                provider: Alias::new("git").map_err(|_| "git alias invalid")?,
                repository: source.repository.clone(),
                revision: ExternalRef::new(
                    "git",
                    "revision",
                    source.exact_revision.native_id.as_str(),
                )
                .map_err(|_| "native Git revision invalid")?,
            };
            let plan = workflow_plan(&project, &run_identity, &pipeline, Some(source_context))?;
            kiln_adapter_lattice::lower_request(
                &project.lattice(self.profile.bounds.max_runs)?,
                &plan,
            )
            .map_err(|_| "source-bound workflow rejected")?;
            let mut record = RunRecord {
                id: id.clone(),
                ordinal: self.state.version,
                project_id: project.id.clone(),
                pipeline_identity: pipeline,
                source,
                plan: plan.clone(),
                events: Vec::new(),
                phase: "dispatch_reserved".into(),
                observation: None,
                error_code: None,
                artifacts: Vec::new(),
                logs: Vec::new(),
                state: State::default(),
            };
            record.apply(Command::StartRun {
                run_id: RunId::new(id.clone()).map_err(|_| "run identity invalid")?,
                attempt_id: plan.attempt_id.clone(),
                provider_profile: plan.provider_profile.clone(),
                effect_id: plan.effect_id.clone(),
            })?;
            self.state.runs.insert(id.clone(), record);
            self.reserve_request(
                req.request_id.clone(),
                fingerprint,
                id.clone(),
                RequestKind::Start,
            )?;
            let result = self
                .ports
                .get_mut(&project.id)
                .ok_or("project port missing")?
                .dispatch(&plan);
            self.accept(&id, result)?;
            let view = serde_json::to_value(self.state.runs.get(&id).ok_or("run missing")?.view())
                .map_err(|_| "run view encoding failed")?;
            return self.finish_request(&req.request_id, 201, view);
        }
        if method == "GET" && path == "/api/runs" {
            if params
                .keys()
                .any(|k| !matches!(*k, "project_id" | "source_revision"))
            {
                return error(400, "query_invalid");
            }
            let project = match params.get("project_id").map(|v| decoded(v)).transpose() {
                Ok(v) => v,
                Err(_) => return error(400, "query_invalid"),
            };
            let revision = match params
                .get("source_revision")
                .map(|v| decoded(v))
                .transpose()
            {
                Ok(v) => v,
                Err(_) => return error(400, "query_invalid"),
            };
            let mut runs: Vec<_> = self
                .state
                .runs
                .values()
                .filter(|r| {
                    project.as_ref().is_none_or(|p| p == &r.project_id)
                        && revision
                            .as_ref()
                            .is_none_or(|v| v == r.source.exact_revision.native_id.as_str())
                        && (role == Role::Operator
                            || (!r.outcome().is_terminal() && r.phase == "active"))
                })
                .collect();
            runs.sort_by(|a, b| b.ordinal.cmp(&a.ordinal).then_with(|| a.id.cmp(&b.id)));
            return json_response(
                200,
                json!({"runs":runs.iter().map(|r|r.view()).collect::<Vec<_>>()}),
            );
        }
        if method == "GET" && path == "/api/live" {
            if params.keys().any(|k| *k != "after") {
                return error(400, "query_invalid");
            }
            let after = match params.get("after").map(|v| v.parse::<u64>()).transpose() {
                Ok(v) => v.unwrap_or(0),
                Err(_) => return error(400, "cursor_invalid"),
            };
            let frame = if after >= self.state.version {
                ": heartbeat\n\n".to_string()
            } else {
                format!("event: snapshot\ndata: {}\n\n",serde_json::to_string(&json!({"version":self.state.version,"runs":self.state.runs.values().map(|r|r.view()).collect::<Vec<_>>()})).map_err(|_| "snapshot encoding failed")?)
            };
            return Ok(ApiResponse {
                status: 200,
                content_type: "text/event-stream",
                body: frame.into_bytes(),
                download_name: None,
            });
        }
        if let Some((id, resource, artifact)) = run_route {
            let Some(run) = self.state.runs.get(id) else {
                return error(404, "run_missing");
            };
            match (method, resource, artifact) {
                ("GET", None, None) if params.is_empty() => return json_response(200, run.view()),
                ("POST", Some("control"), None) if params.is_empty() => {
                    return self.control(id, body);
                }
                ("GET", Some("logs"), None) => return self.logs(id, &params),
                ("POST", Some("logs"), None) if params.is_empty() => {
                    return self.upload_log(id, body);
                }
                ("POST", Some("artifacts"), None) if params.is_empty() => {
                    return self.upload(id, body);
                }
                ("GET", Some("artifacts"), None) if params.is_empty() => {
                    return json_response(200, json!({"artifacts":run.artifacts}));
                }
                ("GET", Some("artifacts"), Some(artifact_id)) if params.is_empty() => {
                    return self.download(id, artifact_id);
                }
                _ => {}
            }
        }
        error(404, "route_missing")
    }
    fn control(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        let req: ControlRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "control_invalid"),
        };
        if !crate::profile::identifier(&req.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&(id, &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }
        // Refresh before exact identity checks; observation never mutates the provider.
        self.refresh(id)?;
        let run = self.state.runs.get(id).ok_or("run missing")?;
        let caps = run.capabilities();
        if run.plan.attempt_id != req.attempt_id
            || run.state.revision != req.expected_revision
            || run
                .observation
                .as_ref()
                .is_none_or(|o| o.execution != req.execution || o.sequence != req.expected_sequence)
        {
            return error(409, "control_stale");
        }
        let action = match req.action.as_str() {
            "cancel" if caps.cancel => WorkflowControlAction::Cancel,
            "pause" if caps.pause => WorkflowControlAction::Pause,
            "resume" if caps.resume => WorkflowControlAction::Resume,
            _ => return error(409, "control_unsupported"),
        };
        if run.events.len().saturating_add(3) > self.profile.bounds.max_events_per_run {
            return error(409, "canonical_event_retention");
        }
        let request = WorkflowControlRequest {
            effect_id: run.plan.effect_id.clone(),
            execution: req.execution,
            expected_sequence: req.expected_sequence,
            action,
        };
        let project = run.project_id.clone();
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        if action == WorkflowControlAction::Cancel {
            run.apply(Command::Cancel {
                attempt_id: run.plan.attempt_id.clone(),
                effect_id: run.plan.effect_id.clone(),
            })?;
        }
        run.phase = "control_reserved".into();
        self.reserve_request(
            req.request_id.clone(),
            fingerprint,
            id.to_string(),
            RequestKind::Control,
        )?;
        let result = self
            .ports
            .get_mut(&project)
            .ok_or("project port missing")?
            .control(&request);
        self.accept(id, result)?;
        let status = if self.state.runs.get(id).ok_or("run missing")?.phase == "unknown" {
            503
        } else {
            200
        };
        if status == 503 {
            return self.finish_request(&req.request_id,status,json!({"code":"control_unknown","message":"Control acceptance is ambiguous; observe before a new action"}));
        }
        self.finish_request(
            &req.request_id,
            status,
            serde_json::to_value(self.state.runs.get(id).ok_or("run missing")?.view())
                .map_err(|_| "run view encoding failed")?,
        )
    }
    fn logs(&mut self, id: &str, params: &BTreeMap<&str, &str>) -> Result<ApiResponse, String> {
        if params.len() != 4
            || params
                .keys()
                .any(|k| !matches!(*k, "node_id" | "stream" | "cursor" | "max_bytes"))
        {
            return error(400, "log_query_invalid");
        }
        let (Some(node_id), Some(stream_name), Some(cursor_text), Some(maximum_text)) = (
            params.get("node_id"),
            params.get("stream"),
            params.get("cursor"),
            params.get("max_bytes"),
        ) else {
            return error(400, "log_query_invalid");
        };
        let node = match decoded(node_id) {
            Ok(v) => v,
            Err(_) => return error(400, "node_invalid"),
        };
        let stream = match *stream_name {
            "stdout" => WorkflowLogStream::Stdout,
            "stderr" => WorkflowLogStream::Stderr,
            "diagnostic" => WorkflowLogStream::Diagnostic,
            _ => return error(400, "stream_invalid"),
        };
        let cursor = match cursor_text.parse::<u64>() {
            Ok(v) => v,
            Err(_) => return error(400, "cursor_invalid"),
        };
        let max = match maximum_text.parse::<usize>() {
            Ok(v) if v > 0 && v <= self.profile.bounds.max_log_read_bytes => v,
            _ => return error(400, "log_bound"),
        };
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if !run.plan.nodes.iter().any(|n| n.id.as_str() == node)
            || max > run.plan.bounds.max_log_read_bytes
        {
            return error(400, "log_node_or_bound");
        }
        if stream != WorkflowLogStream::Diagnostic {
            return self.captured_logs(id, &node, stream_name, cursor, max);
        }
        let Some(observation) = &run.observation else {
            return error(409, "execution_unavailable");
        };
        let project = run.project_id.clone();
        let execution = observation.execution.clone();
        let request = WorkflowLogReadRequest {
            effect_id: run.plan.effect_id.clone(),
            execution,
            node_id: Alias::new(node.clone()).map_err(|_| "node alias invalid")?,
            stream,
            cursor,
            max_bytes: max,
        };
        if !self.known.contains(id) {
            let port = self.ports.get_mut(&project).ok_or("project port missing")?;
            match port.recover_dispatch(&run.plan) {
                Ok(recovered) if recovered.execution == request.execution => {
                    self.known.insert(id.to_string());
                }
                _ => return error(503, "log_recovery_unavailable"),
            };
        }
        let chunk = match self
            .ports
            .get_mut(&project)
            .ok_or("project port missing")?
            .read_log(&request)
        {
            Ok(v) => v,
            Err(_) => return error(503, "log_unavailable"),
        };
        if chunk.bytes.len() > max
            || chunk.reference.node_id != request.node_id
            || chunk.reference.stream != stream
            || chunk.reference.bytes != chunk.bytes.len()
            || chunk.reference.cursor < cursor
        {
            return error(503, "log_identity_conflict");
        }
        let next = match chunk.reference.cursor.checked_add(chunk.bytes.len() as u64) {
            Some(v) => v,
            None => return error(503, "log_cursor_overflow"),
        };
        json_response(
            200,
            json!({"node_id":node,"stream":stream_name,"source":"native-provider","cursor":chunk.reference.cursor,"next_cursor":next,"text":String::from_utf8_lossy(&chunk.bytes),"gap":chunk.reference.gap_before,"truncated":chunk.reference.truncated,"retained":chunk.reference.retained}),
        )
    }
}
