// r[impl kiln.web.http_contract]
// r[verify kiln.web.http_contract]
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult, WorkflowPort};
use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::profile::ServiceProfile;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Lockstep client fixture bound on received response bytes.
const CLIENT_BODY_LIMIT: usize = 64 * 1024;

#[path = "http_contract/served_client.rs"]
mod served_client;

#[derive(Default)]
struct Backend {
    plan: Option<WorkflowEffectPlan>,
    dispatches: usize,
    recoveries: usize,
    observations: usize,
    controls: usize,
    sequence: u64,
    terminal: bool,
    disconnect: bool,
    cancelled: bool,
    control_disconnect: bool,
    timeout: bool,
    failed_nodes: std::collections::BTreeSet<String>,
    skipped_nodes: std::collections::BTreeSet<String>,
    registered_node_sets: Option<std::collections::BTreeSet<Vec<String>>>,
    /// Fail only the first N dispatch attempts, then observe success.
    fail_first_dispatches: Option<usize>,
}
struct Port {
    profile: CapabilityProfile,
    backend: Arc<Mutex<Backend>>,
}
fn observation(plan: &WorkflowEffectPlan, state: &Backend) -> WorkflowObservation {
    let terminal = state.terminal || state.cancelled;
    let node_state = if state.cancelled {
        WorkflowNodeState::Cancelled
    } else if state.terminal {
        WorkflowNodeState::Completed
    } else {
        WorkflowNodeState::Running
    };
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: ExternalRef::new("lattice", "workflow", "test-workflow")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        execution: ExternalRef::new("lattice", "execution", "original-execution")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        sequence: state.sequence,
        state: if state.cancelled {
            WorkflowState::Cancelled
        } else if state.terminal && !state.failed_nodes.is_empty() {
            WorkflowState::Failed
        } else if state.terminal {
            WorkflowState::Completed
        } else {
            WorkflowState::Running
        },
        terminal,
        successful: state.terminal && !state.cancelled && state.failed_nodes.is_empty(),
        outcome: if state.cancelled {
            Outcome::Cancelled
        } else if state.terminal && !state.failed_nodes.is_empty() {
            Outcome::DomainFailure
        } else if state.terminal {
            Outcome::Success
        } else {
            Outcome::Pending
        },
        nodes: plan
            .nodes
            .iter()
            .map(|n| WorkflowNodeObservation {
                node_id: n.id.clone(),
                state: if state.terminal && state.failed_nodes.contains(n.id.as_str()) {
                    WorkflowNodeState::Failed
                } else if state.terminal && state.skipped_nodes.contains(n.id.as_str()) {
                    WorkflowNodeState::Skipped
                } else {
                    node_state
                },
                terminal,
                successful: state.terminal
                    && !state.cancelled
                    && !state.failed_nodes.contains(n.id.as_str()),
                adapter: ExternalRef::new("lattice", "adapter", "local")
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
                receipt: (terminal && !state.skipped_nodes.contains(n.id.as_str())).then(|| {
                    ExternalRef::new("lattice", "receipt", "node-receipt")
                        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
                }),
            })
            .collect(),
        logs: Vec::new(),
        receipt: terminal.then(|| {
            ExternalRef::new("lattice", "receipt", "execution-receipt")
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        }),
    }
}
impl WorkflowPort for Port {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut state = self.backend.lock();
        state.dispatches += 1;
        state.plan = Some(plan.clone());
        if state
            .fail_first_dispatches
            .is_some_and(|limit| state.dispatches > limit)
        {
            state.failed_nodes.clear();
        }
        let selected: Vec<_> = plan
            .nodes
            .iter()
            .map(|n| n.id.as_str().to_owned())
            .collect();
        if state
            .registered_node_sets
            .as_ref()
            .is_some_and(|sets| !sets.contains(&selected))
        {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "route_not_registered",
                "workflow.nodes",
                "No exact registered route for selected graph",
            )));
        }
        if state.disconnect {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "accepted_disconnect".into(),
                message: "test boundary".into(),
            }));
        }
        if state.timeout {
            return Err(PortFailure::TimedOut(InfrastructureFailure {
                code: "provider_deadline".into(),
                message: "test boundary".into(),
            }));
        }
        Ok(observation(plan, &state))
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut state = self.backend.lock();
        state.recoveries += 1;
        assert_eq!(state.plan.as_ref(), Some(plan));
        Ok(observation(plan, &state))
    }
    fn observe(&mut self, _: &EffectId) -> PortResult<WorkflowObservation> {
        let mut state = self.backend.lock();
        state.observations += 1;
        Ok(observation(
            state
                .plan
                .as_ref()
                .unwrap_or_else(|| unreachable!("fixture plan")),
            &state,
        ))
    }
    fn control(&mut self, request: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        let mut state = self.backend.lock();
        state.controls += 1;
        assert_eq!(request.expected_sequence, state.sequence);
        state.sequence += 1;
        state.cancelled = true;
        if state.control_disconnect {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "accepted_control_disconnect".into(),
                message: "test boundary".into(),
            }));
        }
        Ok(observation(
            state
                .plan
                .as_ref()
                .unwrap_or_else(|| unreachable!("fixture plan")),
            &state,
        ))
    }
    fn reconcile(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(id)
    }
    fn read_log(&mut self, request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        let start = request.cursor.max(4);
        let retained = b"line\n";
        let offset = (start - 4).min(retained.len() as u64) as usize;
        let bytes = retained
            .iter()
            .skip(offset)
            .copied()
            .take(request.max_bytes)
            .collect::<Vec<_>>();
        Ok(WorkflowLogChunk {
            reference: WorkflowLogReference {
                node_id: request.node_id.clone(),
                stream: request.stream,
                reference: ExternalRef::new("lattice", "log", "native-log")
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
                cursor: start,
                bytes: bytes.len(),
                truncated: false,
                gap_before: request.cursor < 4,
                retained: true,
            },
            bytes,
        })
    }
}

// helpers
fn fixture() -> (tempfile::TempDir, ServiceProfile, String) {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository_path = root.path().join("repository");
    let repository = git2::Repository::init(&repository_path)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::fs::write(
        repository_path.join("source.txt"),
        b"exact reviewed source\n",
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut index = repository
        .index()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    index
        .add_path(std::path::Path::new("source.txt"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree_id = index
        .write_tree()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let signature = git2::Signature::now("test", "test@example.invalid")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let revision = repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "fixture",
            &tree,
            &[],
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let profile: ServiceProfile = serde_json::from_value(json!({"schema":"kiln.web-service.v1","service_id":"isolated-tests","listen_address":"127.0.0.1:9443","origin":"https://localhost:9443","tls":{"certificate_path":root.path().join("cert.pem"),"private_key_path":root.path().join("key.pem"),"min_version":"TLS1.3"},"operator_token_file":root.path().join("operator.token"),"worker_token_file":root.path().join("worker.token"),"state_directory":root.path().join("state"),"projects":[{"id":"fixture","label":"Fixture","repository_path":repository_path,"reference":"refs/heads/main","source_limits":{"max_entries":32,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":16384},"workflow":{"socket_path":root.path().join("lattice.sock"),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],"transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},"adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},"policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64","nodes":[{"id":"job","kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"edges":[],"controls":{"cancel":true,"pause":false,"resume":false},"bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}}],"bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,"timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":4,"max_artifact_bytes":1024,"max_artifacts":2,"max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,"max_requests":32,"max_events_per_run":8,"max_state_bytes":1048576}})).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    (root, profile, revision.to_string())
}

fn open(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    let capability = profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!("fixture first element"))
        .execution_capabilities(profile.bounds.max_runs)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    ports.insert(
        "fixture".into(),
        Box::new(Port {
            profile: capability,
            backend,
        }),
    );
    Engine::open_with_ports(profile, ports)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn call(engine: &mut Engine, method: &str, path: &str, body: Value, role: Role) -> ApiResponse {
    engine
        .api(
            method,
            path,
            &serde_json::to_vec(&body)
                .unwrap_or_else(|error| unreachable!("fixture encoding: {error}")),
            role,
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn get(engine: &mut Engine, path: &str, role: Role) -> ApiResponse {
    engine
        .api("GET", path, &[], role)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn value(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| unreachable!("fixture field {name}"))
}

fn enroll(engine: &mut Engine) {
    assert_eq!(
        call(
            engine,
            "POST",
            "/api/projects",
            json!({"project_id":"fixture","request_id":"enroll"}),
            Role::Operator
        )
        .status,
        200
    );
}

fn start(engine: &mut Engine, key: &str) -> ApiResponse {
    call(
        engine,
        "POST",
        "/api/runs",
        json!({"project_id":"fixture","request_id":key}),
        Role::Operator,
    )
}

fn load_openapi() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/http-api/openapi.json"
    );
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|error| unreachable!("openapi read: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| unreachable!("openapi parse: {error}"))
}

fn resolve_schema<'a>(schema: &'a Value, components: &'a Value) -> Result<&'a Value, String> {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return Ok(schema);
    };
    let name = reference
        .strip_prefix("#/components/schemas/")
        .ok_or_else(|| format!("unsupported reference {reference}"))?;
    components
        .get("schemas")
        .and_then(|schemas| schemas.get(name))
        .ok_or_else(|| format!("unresolved reference {reference}"))
}

fn type_matches(name: &str, value: &Value) -> Result<bool, String> {
    Ok(match name {
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        other => return Err(format!("unsupported schema type {other}")),
    })
}

/// Independent OpenAPI 3.0 subset validator; unknown constructs fail closed.
fn validate(value: &Value, schema: &Value, components: &Value) -> Result<(), String> {
    let schema = resolve_schema(schema, components)?;
    const SUPPORTED: [&str; 10] = [
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "maxItems",
        "enum",
        "nullable",
        "description",
        "format",
    ];
    if let Some(keyword) = schema
        .as_object()
        .ok_or("schema must be an object")?
        .keys()
        .find(|keyword| !SUPPORTED.contains(&keyword.as_str()))
    {
        return Err(format!("unsupported schema keyword {keyword}"));
    }
    if value.is_null() {
        return if schema.get("nullable").and_then(Value::as_bool) == Some(true) {
            Ok(())
        } else {
            Err("unexpected null".into())
        };
    }
    if let Some(name) = schema.get("type") {
        let name = name.as_str().ok_or("schema type must be a string")?;
        if !type_matches(name, value)? {
            return Err(format!("type mismatch: expected {name}"));
        }
    }
    if let Some(allowed) = schema.get("enum")
        && !allowed
            .as_array()
            .is_some_and(|allowed| allowed.contains(value))
    {
        return Err("enum mismatch".into());
    }
    if let Some(object) = value.as_object() {
        for required in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = required
                .as_str()
                .ok_or("required entries must be strings")?;
            if !object.contains_key(key) {
                return Err(format!("missing required {key}"));
            }
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        for (key, field) in object {
            match properties.and_then(|properties| properties.get(key)) {
                Some(property) => validate(field, property, components)?,
                None if schema.get("additionalProperties").and_then(Value::as_bool)
                    == Some(false) =>
                {
                    return Err(format!("unexpected property {key}"));
                }
                None => {}
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(max) = schema.get("maxItems").and_then(Value::as_u64)
            && items.len() as u64 > max
        {
            return Err("too many items".into());
        }
        if let Some(item_schema) = schema.get("items") {
            for item in items {
                validate(item, item_schema, components)?;
            }
        }
    }
    Ok(())
}

fn schema_for(openapi: &Value, name: &str) -> Value {
    openapi
        .get("components")
        .unwrap_or(&Value::Null)
        .get("schemas")
        .unwrap_or(&Value::Null)
        .get(name)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Bounded lockstep client fixture: the byte bound applies before parsing and
/// no request is retried. A 2xx classification is never execution evidence.
fn classify(
    status: u16,
    body: &[u8],
    truncated: bool,
    schema: &Value,
    components: &Value,
) -> &'static str {
    if truncated || body.len() > CLIENT_BODY_LIMIT {
        return "transport";
    }
    match status {
        401 => "authentication",
        403 => "authorization",
        404 => "not_found",
        200..=299 => match serde_json::from_slice::<Value>(body) {
            Ok(value) if validate(&value, schema, components).is_ok() => "ok",
            _ => "schema_mismatch",
        },
        _ => "unknown",
    }
}

#[test]
fn http_contract_openapi_canonical() {
    let openapi = load_openapi();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/http-api/openapi.json"
    );
    let committed =
        std::fs::read_to_string(path).unwrap_or_else(|error| unreachable!("openapi read: {error}"));
    let canonical = serde_json::to_string_pretty(&openapi)
        .unwrap_or_else(|error| unreachable!("openapi serialize: {error}"))
        + "\n";
    assert_eq!(committed, canonical);
}

#[test]
fn http_contract_real_responses_validate_schema() {
    let openapi = load_openapi();
    let components = openapi.get("components").cloned().unwrap_or(Value::Null);
    let (_root, profile, _rev) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend);
    enroll(&mut engine);
    let run = value(&start(&mut engine, "contract-run"));
    let run_id = field(&run, "id")
        .as_str()
        .unwrap_or_else(|| unreachable!("run id"));

    // projects
    let resp = get(&mut engine, "/api/projects", Role::Operator);
    assert_eq!(resp.status, 200);
    validate(
        &value(&resp),
        &schema_for(&openapi, "ProjectList"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("projects schema: {e}"));

    // runs with limit/cursor
    let resp = get(&mut engine, "/api/runs?limit=1", Role::Viewer);
    assert_eq!(resp.status, 200);
    let page = value(&resp);
    validate(&page, &schema_for(&openapi, "RunPage"), &components)
        .unwrap_or_else(|e| unreachable!("runs schema: {e}"));

    // run view
    let path = format!("/api/runs/{run_id}");
    let resp = get(&mut engine, &path, Role::Viewer);
    assert_eq!(resp.status, 200);
    validate(&value(&resp), &schema_for(&openapi, "RunView"), &components)
        .unwrap_or_else(|e| unreachable!("run schema: {e}"));

    // execution binding worker/operator
    let ep = format!("/api/runs/{run_id}/execution-binding");
    for role in [Role::Worker, Role::Operator] {
        let resp = get(&mut engine, &ep, role);
        assert_eq!(resp.status, 200);
        validate(
            &value(&resp),
            &schema_for(&openapi, "ExecutionBinding"),
            &components,
        )
        .unwrap_or_else(|e| unreachable!("binding schema: {e}"));
    }

    // artifacts
    let ap = format!("/api/runs/{run_id}/artifacts");
    let resp = get(&mut engine, &ap, Role::Viewer);
    assert_eq!(resp.status, 200);
    validate(
        &value(&resp),
        &schema_for(&openapi, "ArtifactList"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("artifacts schema: {e}"));

    // errors
    let resp = get(&mut engine, "/api/runs/no-such-run", Role::Viewer);
    assert_eq!(resp.status, 404);
    validate(&value(&resp), &schema_for(&openapi, "Error"), &components)
        .unwrap_or_else(|e| unreachable!("error schema: {e}"));

    let resp = get(&mut engine, "/api/projects", Role::Worker);
    assert_eq!(resp.status, 403);
    validate(&value(&resp), &schema_for(&openapi, "Error"), &components)
        .unwrap_or_else(|e| unreachable!("error schema: {e}"));

    let resp = get(&mut engine, "/api/runs/any/execution-binding", Role::Viewer);
    assert_eq!(resp.status, 403);
    validate(&value(&resp), &schema_for(&openapi, "Error"), &components)
        .unwrap_or_else(|e| unreachable!("error schema: {e}"));

    let resp = get(&mut engine, "/api/runs?limit=0", Role::Viewer);
    assert_eq!(resp.status, 400);
    validate(&value(&resp), &schema_for(&openapi, "Error"), &components)
        .unwrap_or_else(|e| unreachable!("error schema: {e}"));

    // runbooks listing & dispatch
    let registry: kiln_web_service::runbooks::RunbookRegistry = serde_json::from_value(json!({
        "schema": "kiln.runbook-registry.v1",
        "runbooks": [{
            "id": "contract-audit",
            "display_name": "Contract Audit",
            "description": "Read-only contract audit",
            "mode": "read_only",
            "lattice": {
                "bundle_id": "contract-audit",
                "bundle_identity": "b".repeat(64),
                "workflow_name": "contract-audit",
                "workflow_revision": "c".repeat(64),
                "nodes": [{
                    "id": "job",
                    "kind": "test",
                    "effect_class": "test",
                    "adapter_kind": "local",
                    "operation_identity": "a".repeat(64)
                }],
                "edges": []
            },
            "params": [{
                "name": "strict",
                "type": { "kind": "bool" },
                "required": false,
                "default": true,
                "bindings": [{ "node": "job", "shell_env": "LATTICE_PARAM_STRICT" }]
            }],
            "allowed_targets": [{
                "repository": "onixresearch/kiln",
                "project_id": "fixture",
                "references": ["refs/heads/main"],
                "allow_commit_sha": false
            }],
            "fanout": {
                "enabled": true,
                "default_max_concurrency": 1,
                "max_concurrency_limit": 2,
                "default_budget_cap": 2,
                "max_budget_cap": 4,
                "max_targets": 4,
                "write_scopes": []
            }
        }],
        "grants": []
    }))
    .unwrap_or_else(|e| unreachable!("runbook registry fixture: {e}"));
    engine
        .configure_runbooks(registry)
        .unwrap_or_else(|e| unreachable!("configure runbooks: {e}"));

    let resp = get(&mut engine, "/api/runbooks", Role::Operator);
    assert_eq!(resp.status, 200);
    validate(
        &value(&resp),
        &schema_for(&openapi, "RunbookList"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("runbook list schema: {e}"));

    let preview_resp = call(
        &mut engine,
        "POST",
        "/api/runbooks/contract-audit/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "strict": true }
        }),
        Role::Operator,
    );
    assert_eq!(preview_resp.status, 200);
    validate(
        &value(&preview_resp),
        &schema_for(&openapi, "RunbookPreviewResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("runbook preview schema: {e}"));

    let resp = call(
        &mut engine,
        "POST",
        "/api/runbooks/contract-audit/dispatch",
        json!({
            "request_id": "contract-rb-1",
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "strict": true }
        }),
        Role::Operator,
    );
    assert!(resp.status == 201 || resp.status == 202);
    validate(
        &value(&resp),
        &schema_for(&openapi, "RunbookDispatchResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("runbook dispatch schema: {e}"));

    // fan-out dispatch, list, inspect, pause, continue, cancel
    let fanout_resp = call(
        &mut engine,
        "POST",
        "/api/runbooks/contract-audit/fanout",
        json!({
            "request_id": "contract-fo-1",
            "targets": ["onixresearch/kiln@refs/heads/main"],
            "params": { "strict": true },
            "max_concurrency": 1,
            "budget_cap": 1
        }),
        Role::Operator,
    );
    assert!(fanout_resp.status == 201 || fanout_resp.status == 202);
    let fanout_val = value(&fanout_resp);
    validate(
        &fanout_val,
        &schema_for(&openapi, "FanoutDispatchResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout dispatch schema: {e}"));
    let fanout_id = fanout_val
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"));

    let fanout_list = get(&mut engine, "/api/runbooks/fanouts", Role::Viewer);
    assert_eq!(fanout_list.status, 200);
    validate(
        &value(&fanout_list),
        &schema_for(&openapi, "FanoutPage"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout page schema: {e}"));

    let fanout_get = get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        Role::Viewer,
    );
    assert_eq!(fanout_get.status, 200);
    validate(
        &value(&fanout_get),
        &schema_for(&openapi, "FanoutView"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout view schema: {e}"));

    let pause_resp = call(
        &mut engine,
        "POST",
        &format!("/api/runbooks/fanouts/{fanout_id}/pause"),
        json!({ "request_id": "contract-fo-pause" }),
        Role::Operator,
    );
    assert_eq!(pause_resp.status, 200);
    validate(
        &value(&pause_resp),
        &schema_for(&openapi, "FanoutControlResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout pause schema: {e}"));

    let cont_resp = call(
        &mut engine,
        "POST",
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "contract-fo-cont" }),
        Role::Operator,
    );
    assert_eq!(cont_resp.status, 200);
    validate(
        &value(&cont_resp),
        &schema_for(&openapi, "FanoutControlResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout continue schema: {e}"));

    let cancel_resp = call(
        &mut engine,
        "POST",
        &format!("/api/runbooks/fanouts/{fanout_id}/cancel"),
        json!({ "request_id": "contract-fo-cancel" }),
        Role::Operator,
    );
    assert_eq!(cancel_resp.status, 200);
    validate(
        &value(&cancel_resp),
        &schema_for(&openapi, "FanoutControlResponse"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout cancel schema: {e}"));

    // #100 result roll-ups: dispatches list, dispatch rollup, fanout rollup, RBAC & errors
    let dispatches_resp = get(
        &mut engine,
        "/api/runbooks/dispatches?limit=1",
        Role::Viewer,
    );
    assert_eq!(dispatches_resp.status, 200);
    let dispatches_val = value(&dispatches_resp);
    validate(
        &dispatches_val,
        &schema_for(&openapi, "RunbookDispatchPage"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("runbook dispatch page schema: {e}"));
    let dispatch_run_id = dispatches_val
        .pointer("/dispatches/0/run_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("dispatch run_id missing"));

    let dispatch_rollup_resp = get(
        &mut engine,
        &format!("/api/runbooks/dispatches/{dispatch_run_id}/rollup"),
        Role::Viewer,
    );
    assert_eq!(dispatch_rollup_resp.status, 200);
    validate(
        &value(&dispatch_rollup_resp),
        &schema_for(&openapi, "RunbookRollupPage"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("dispatch rollup schema: {e}"));

    let fanout_rollup_resp = get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/rollup?limit=1"),
        Role::Viewer,
    );
    assert_eq!(fanout_rollup_resp.status, 200);
    validate(
        &value(&fanout_rollup_resp),
        &schema_for(&openapi, "RunbookRollupPage"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("fanout rollup schema: {e}"));

    // Worker denied & missing resource errors validate against Error schema
    let worker_denied = get(&mut engine, "/api/runbooks/dispatches", Role::Worker);
    assert_eq!(worker_denied.status, 403);
    validate(
        &value(&worker_denied),
        &schema_for(&openapi, "Error"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("worker error schema: {e}"));

    let missing_rollup = get(
        &mut engine,
        "/api/runbooks/fanouts/rb-fanout-missing/rollup",
        Role::Viewer,
    );
    assert_eq!(missing_rollup.status, 404);
    validate(
        &value(&missing_rollup),
        &schema_for(&openapi, "Error"),
        &components,
    )
    .unwrap_or_else(|e| unreachable!("missing rollup error schema: {e}"));
}

#[test]
fn http_contract_bounded_client_classification() {
    let openapi = load_openapi();
    let components = openapi.get("components").cloned().unwrap_or(Value::Null);
    let projects = schema_for(&openapi, "ProjectList");
    let (_root, profile, _rev) = fixture();
    let mut engine = open(profile, Arc::new(Mutex::new(Backend::default())));
    let real = get(&mut engine, "/api/projects", Role::Operator);
    assert_eq!(
        classify(real.status, &real.body, false, &projects, &components),
        "ok"
    );
    let forbidden = get(&mut engine, "/api/projects", Role::Worker);
    assert_eq!(
        classify(
            forbidden.status,
            &forbidden.body,
            false,
            &projects,
            &components
        ),
        "authorization"
    );
    let missing = get(&mut engine, "/api/runs/no-such-run", Role::Viewer);
    assert_eq!(
        classify(missing.status, &missing.body, false, &projects, &components),
        "not_found"
    );
    assert_eq!(
        classify(401, b"{}", false, &projects, &components),
        "authentication"
    );
    assert_eq!(
        classify(418, b"{}", false, &projects, &components),
        "unknown"
    );
    // The bound applies to received bytes before any parse.
    let oversized = vec![b' '; CLIENT_BODY_LIMIT + 1];
    assert_eq!(
        classify(200, &oversized, false, &projects, &components),
        "transport"
    );
    assert_eq!(
        classify(real.status, &real.body, true, &projects, &components),
        "transport"
    );
    let body = value(&real);
    let mut mutations = Vec::new();
    let mut missing_field = body.clone();
    missing_field
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("projects object"))
        .remove("enrolled");
    mutations.push(missing_field);
    let mut wrong_type = body.clone();
    wrong_type
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("projects object"))
        .insert("available".into(), json!("not a list"));
    mutations.push(wrong_type);
    let mut extra = body;
    extra
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("projects object"))
        .insert("unexpected".into(), json!(1));
    mutations.push(extra);
    for mutation in mutations {
        let bytes = serde_json::to_vec(&mutation).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            classify(200, &bytes, false, &projects, &components),
            "schema_mismatch",
            "{mutation}"
        );
    }
    assert_eq!(
        classify(200, b"{\"available\":", false, &projects, &components),
        "schema_mismatch"
    );
}

#[test]
fn http_contract_golden_error_bodies_match_real_responses() {
    let (_root, profile, _rev) = fixture();
    let mut engine = open(profile, Arc::new(Mutex::new(Backend::default())));
    for (path, role, status, golden) in [
        (
            "/api/projects",
            Role::Worker,
            403,
            "error-authentication.json",
        ),
        (
            "/api/runs/no-such-run",
            Role::Viewer,
            404,
            "error-missing-run.json",
        ),
        (
            "/api/runs?limit=0",
            Role::Viewer,
            400,
            "error-page-bound.json",
        ),
    ] {
        let response = get(&mut engine, path, role);
        assert_eq!(response.status, status, "{path}");
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/tests/http-contract/{golden}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap_or_else(|error| unreachable!("golden {golden}: {error}")),
        )
        .unwrap_or_else(|error| unreachable!("golden {golden}: {error}"));
        assert_eq!(value(&response), expected, "{path}");
    }
}
