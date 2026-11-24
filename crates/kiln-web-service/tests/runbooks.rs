use kiln_adapter_github::profile::ConfigEffectClass;
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{PortResult, WorkflowPort};
use kiln_web_service::access::Principal;
use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::profile::ServiceProfile;
use kiln_web_service::runbooks::{
    ParamBinding, ParamDeclaration, ParamType, ParamValue, RUNBOOK_REGISTRY_SCHEMA,
    RunbookDefinition, RunbookGrant, RunbookLattice, RunbookMode, RunbookRegistry, RunbookTarget,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
struct Backend {
    plan: Option<WorkflowEffectPlan>,
    sequence: u64,
    terminal: bool,
    cancelled: bool,
    failed: bool,
    dispatches: usize,
}

struct Port {
    profile: CapabilityProfile,
    backend: Arc<Mutex<Backend>>,
}

fn observation(plan: &WorkflowEffectPlan, state: &Backend) -> WorkflowObservation {
    let terminal = state.terminal || state.cancelled || state.failed;
    let node_state = if state.cancelled {
        WorkflowNodeState::Cancelled
    } else if state.failed {
        WorkflowNodeState::Failed
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
        workflow: ExternalRef::new("lattice", "workflow", "runbook-workflow")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        execution: ExternalRef::new("lattice", "execution", "runbook-execution")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        sequence: state.sequence,
        state: if state.cancelled {
            WorkflowState::Cancelled
        } else if state.failed {
            WorkflowState::Failed
        } else if state.terminal {
            WorkflowState::Completed
        } else {
            WorkflowState::Running
        },
        terminal,
        successful: state.terminal && !state.cancelled && !state.failed,
        outcome: if state.cancelled {
            Outcome::Cancelled
        } else if state.failed {
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
                state: node_state,
                terminal,
                successful: state.terminal && !state.cancelled && !state.failed,
                adapter: ExternalRef::new("lattice", "adapter", "local")
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
                receipt: terminal.then(|| {
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
        Ok(observation(plan, &state))
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let state = self.backend.lock();
        Ok(observation(plan, &state))
    }
    fn observe(&mut self, _: &EffectId) -> PortResult<WorkflowObservation> {
        let state = self.backend.lock();
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
        assert_eq!(request.expected_sequence, state.sequence);
        state.sequence += 1;
        state.cancelled = true;
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
        let bytes = b"runbook log\n";
        Ok(WorkflowLogChunk {
            reference: WorkflowLogReference {
                node_id: request.node_id.clone(),
                stream: request.stream,
                reference: ExternalRef::new("lattice", "log", "runbook-log")
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
                cursor: request.cursor,
                bytes: bytes.len(),
                truncated: false,
                gap_before: false,
                retained: true,
            },
            bytes: bytes.to_vec(),
        })
    }
}

fn fixture() -> (tempfile::TempDir, ServiceProfile, String) {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository_path = root.path().join("repository");
    let repository = git2::Repository::init(&repository_path)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::fs::write(repository_path.join("source.txt"), b"runbook source\n")
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
    let profile: ServiceProfile = serde_json::from_value(json!({
        "schema": "kiln.web-service.v1",
        "service_id": "runbook-tests",
        "listen_address": "127.0.0.1:9443",
        "origin": "https://localhost:9443",
        "tls": {
            "certificate_path": root.path().join("cert.pem"),
            "private_key_path": root.path().join("key.pem"),
            "min_version": "TLS1.3"
        },
        "operator_token_file": root.path().join("operator.token"),
        "worker_token_file": root.path().join("worker.token"),
        "state_directory": root.path().join("state"),
        "projects": [{
            "id": "fixture",
            "label": "Fixture",
            "repository_path": repository_path,
            "reference": "refs/heads/main",
            "source_limits": {
                "max_entries": 32,
                "max_path_bytes": 1024,
                "max_file_bytes": 4096,
                "max_total_bytes": 16384
            },
            "workflow": {
                "socket_path": root.path().join("lattice.sock"),
                "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                "transport_limits": {
                    "maximum_frame_bytes": 1048576,
                    "maximum_log_bytes": 65536,
                    "maximum_connections": 4,
                    "read_deadline_milliseconds": 1000,
                    "write_deadline_milliseconds": 1000
                },
                "adapter_limits": {
                    "maximum_request_bytes": 1048576,
                    "maximum_response_bytes": 1048576
                },
                "policy_id": "reviewed-policy",
                "provider_profile_id": "reviewed-lattice",
                "platform": "linux-x86_64",
                "nodes": [{
                    "id": "job",
                    "kind": "test",
                    "effect_class": "test",
                    "adapter_kind": "local",
                    "operation_identity": "a".repeat(64)
                }],
                "edges": [],
                "controls": { "cancel": true, "pause": false, "resume": false },
                "bounds": {
                    "max_nodes": 8,
                    "max_edges": 16,
                    "max_log_references": 32,
                    "max_log_read_bytes": 4096,
                    "max_total_log_bytes": 65536
                }
            }
        }],
        "bounds": {
            "max_request_bytes": 16384,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 4,
            "timeout_seconds": 2,
            "poll_interval_ms": 10,
            "max_projects": 2,
            "max_runs": 16,
            "max_artifact_bytes": 1024,
            "max_artifacts": 2,
            "max_run_artifact_bytes": 2048,
            "max_total_artifact_bytes": 4096,
            "max_log_read_bytes": 4096,
            "max_requests": 32,
            "max_events_per_run": 8,
            "max_state_bytes": 1048576
        }
    }))
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    (root, profile, revision.to_string())
}

fn registry(allow_commit_sha: bool) -> RunbookRegistry {
    let nodes = vec![kiln_adapter_github::profile::ConfigNode {
        id: "job".into(),
        kind: "test".into(),
        effect_class: ConfigEffectClass::Test,
        adapter_kind: "local".into(),
        operation_identity: "a".repeat(64),
    }];
    RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![
            RunbookDefinition {
                id: "security-audit".into(),
                display_name: "Security Audit".into(),
                description: "Read-only audit".into(),
                mode: RunbookMode::ReadOnly,
                lattice: RunbookLattice {
                    bundle_id: Some("security-audit".into()),
                    bundle_identity: "b".repeat(64),
                    workflow_name: "security-audit".into(),
                    workflow_revision: "c".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![
                    ParamDeclaration {
                        name: "strict".into(),
                        param_type: ParamType::Bool {},
                        required: false,
                        default: Some(ParamValue::Bool(true)),
                        bindings: vec![ParamBinding {
                            node: "job".into(),
                            shell_env: "LATTICE_PARAM_STRICT".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "target_dir".into(),
                        param_type: ParamType::RelativePath { max_bytes: 64 },
                        required: true,
                        default: None,
                        bindings: vec![ParamBinding {
                            node: "job".into(),
                            shell_env: "LATTICE_PARAM_TARGET_DIR".into(),
                        }],
                    },
                ],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "fixture".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha,
                }],
                fanout: None,
            },
            RunbookDefinition {
                id: "flake-update".into(),
                display_name: "Flake Update".into(),
                description: "Mutating flake update".into(),
                mode: RunbookMode::Mutating,
                lattice: RunbookLattice {
                    bundle_id: Some("flake-update".into()),
                    bundle_identity: "d".repeat(64),
                    workflow_name: "flake-update".into(),
                    workflow_revision: "e".repeat(64),
                    nodes,
                    edges: vec![],
                },
                params: vec![ParamDeclaration {
                    name: "max_steps".into(),
                    param_type: ParamType::Integer { min: 1, max: 20 },
                    required: false,
                    default: Some(ParamValue::Integer(5)),
                    bindings: vec![ParamBinding {
                        node: "job".into(),
                        shell_env: "LATTICE_PARAM_MAX_STEPS".into(),
                    }],
                }],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "fixture".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha,
                }],
                fanout: None,
            },
        ],
        grants: vec![RunbookGrant {
            principal: "local:operator".into(),
            runbooks: vec!["flake-update".into()],
            not_after_ms: None,
        }],
    }
}

fn open(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    let capability = profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!("project missing"))
        .execution_capabilities(profile.bounds.max_runs)
        .unwrap_or_else(|error| unreachable!("capability: {error}"));
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    ports.insert(
        "fixture".into(),
        Box::new(Port {
            profile: capability,
            backend,
        }),
    );
    Engine::open_with_ports(profile, ports).unwrap_or_else(|error| unreachable!("open: {error}"))
}

fn principal(id: &str, role: Role) -> Principal {
    Principal {
        id: id.into(),
        label: id.into(),
        role,
        agent: None,
    }
}

fn call_as(
    engine: &mut Engine,
    method: &str,
    target: &str,
    body: Value,
    p: &Principal,
) -> ApiResponse {
    let bytes = if body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(&body).unwrap_or_else(|error| unreachable!("{error}"))
    };
    engine
        .api_as(method, target, &bytes, p)
        .unwrap_or_else(|error| unreachable!("api_as: {error}"))
}

fn json_body(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap_or_else(|error| unreachable!("{error}"))
}

fn ptr<'a>(value: &'a Value, path: &str) -> &'a Value {
    value
        .pointer(path)
        .unwrap_or_else(|| unreachable!("missing JSON pointer {path} in {value}"))
}

#[test]
fn runbook_rbac_validation_idempotency_lifecycle_and_restore() {
    let (_root, profile, rev) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    let other_op = principal("local:other", Role::Operator);
    let viewer = principal("local:viewer", Role::Viewer);
    let worker = principal("local:worker", Role::Worker);

    // Unconfigured GET /api/runbooks returns configured: false.
    let unconfigured = call_as(&mut engine, "GET", "/api/runbooks", Value::Null, &op);
    assert_eq!(unconfigured.status, 200);
    assert_eq!(ptr(&json_body(&unconfigured), "/configured"), &json!(false));

    engine
        .configure_runbooks(registry(true))
        .unwrap_or_else(|e| unreachable!("{e}"));

    // Enroll project.
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-1"}),
            &op
        )
        .status,
        200
    );

    // GET /api/runbooks by Operator, Viewer, and Worker.
    let op_list = call_as(&mut engine, "GET", "/api/runbooks", Value::Null, &op);
    assert_eq!(op_list.status, 200);
    let op_json = json_body(&op_list);
    assert_eq!(ptr(&op_json, "/configured"), &json!(true));
    assert_eq!(ptr(&op_json, "/runbooks/0/can_dispatch"), &json!(true));
    assert_eq!(ptr(&op_json, "/runbooks/1/can_dispatch"), &json!(true));

    let viewer_list = call_as(&mut engine, "GET", "/api/runbooks", Value::Null, &viewer);
    assert_eq!(viewer_list.status, 200);
    let viewer_json = json_body(&viewer_list);
    assert_eq!(ptr(&viewer_json, "/runbooks/0/can_dispatch"), &json!(false));
    assert_eq!(ptr(&viewer_json, "/runbooks/1/can_dispatch"), &json!(false));

    let worker_list = call_as(&mut engine, "GET", "/api/runbooks", Value::Null, &worker);
    assert_eq!(worker_list.status, 403);
    assert_eq!(
        ptr(&json_body(&worker_list), "/code"),
        &json!("worker_scope")
    );

    // Viewer & Worker blocked on POST dispatch with audited 403.
    let worker_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({"request_id": "w1", "target": "onixresearch/kiln@main", "params": {"target_dir": "src"}}),
        &worker,
    );
    assert_eq!(worker_dispatch.status, 403);
    let viewer_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({"request_id": "v1", "target": "onixresearch/kiln@main", "params": {"target_dir": "src"}}),
        &viewer,
    );
    assert_eq!(viewer_dispatch.status, 403);

    // Mutating runbook without explicit grant -> audited 403 runbook_grant_required (before body check).
    let ungranted = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"invalid": true}),
        &other_op,
    );
    assert_eq!(ungranted.status, 403);
    assert_eq!(
        ptr(&json_body(&ungranted), "/code"),
        &json!("runbook_grant_required")
    );

    // Pre-dispatch refusals (400 / 404) do not create runs.
    let missing_rb = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/does-not-exist/dispatch",
        json!({"request_id": "r-missing", "target": "onixresearch/kiln@main"}),
        &op,
    );
    assert_eq!(missing_rb.status, 404);

    let bad_target = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({"request_id": "r-target", "target": "other/repo@main", "params": {"target_dir": "src"}}),
        &op,
    );
    assert_eq!(bad_target.status, 400);
    assert_eq!(
        ptr(&json_body(&bad_target), "/code"),
        &json!("runbook_target_not_allowed")
    );

    let secret_param = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({"request_id": "r-sec", "target": "onixresearch/kiln@main", "params": {"target_dir": "ghp_1234567890abcdef1234567890abcdef12"}}),
        &op,
    );
    assert_eq!(secret_param.status, 400);
    assert_eq!(
        ptr(&json_body(&secret_param), "/code"),
        &json!("runbook_param_secret_like")
    );
    assert!(!String::from_utf8_lossy(&secret_param.body).contains("ghp_"));

    // Read-only runbook allowed for Operator without explicit grant; short branch name resolves to commit.
    let dispatched = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "rb-req-1",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "crates/kiln-core"}
        }),
        &other_op,
    );
    assert_eq!(dispatched.status, 201);
    let d_json = json_body(&dispatched);
    assert_eq!(
        ptr(&d_json, "/schema"),
        &json!("kiln.runbook-dispatch-result.v1")
    );
    assert_eq!(ptr(&d_json, "/replayed"), &json!(false));
    assert_eq!(ptr(&d_json, "/source_revision"), &json!(rev));
    assert_eq!(
        ptr(&d_json, "/env_bindings/job/LATTICE_PARAM_TARGET_DIR"),
        &json!("crates/kiln-core")
    );
    assert_eq!(
        ptr(&d_json, "/env_bindings/job/LATTICE_PARAM_STRICT"),
        &json!("true")
    );
    assert_eq!(
        ptr(&d_json, "/source_env/LATTICE_SOURCE_REVISION"),
        &json!(rev)
    );
    let run_id = ptr(&d_json, "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("run_id"))
        .to_owned();

    // Idempotent replay with same principal + same body -> replayed: true, same run_id.
    let replayed = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "rb-req-1",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "crates/kiln-core"}
        }),
        &other_op,
    );
    assert_eq!(replayed.status, 201);
    let r_json = json_body(&replayed);
    assert_eq!(ptr(&r_json, "/replayed"), &json!(true));
    assert_eq!(ptr(&r_json, "/run_id"), &json!(run_id));

    // Same request_id with different principal -> 409 request_id_conflict.
    let diff_principal = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "rb-req-1",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "crates/kiln-core"}
        }),
        &op,
    );
    assert_eq!(diff_principal.status, 409);
    assert_eq!(
        ptr(&json_body(&diff_principal), "/code"),
        &json!("request_id_conflict")
    );

    // ExecutionBinding projects runbook bindings.
    let eb = call_as(
        &mut engine,
        "GET",
        &format!("/api/runs/{run_id}/execution-binding"),
        Value::Null,
        &worker,
    );
    assert_eq!(eb.status, 200);
    assert_eq!(
        ptr(
            &json_body(&eb),
            "/runbook/env_bindings/job/LATTICE_PARAM_TARGET_DIR"
        ),
        &json!("crates/kiln-core")
    );

    // Activity audit includes run_id for the dispatch.
    let activity = call_as(
        &mut engine,
        "GET",
        &format!("/api/runs/{run_id}/activity"),
        Value::Null,
        &viewer,
    );
    assert_eq!(activity.status, 200);
    let events = ptr(&json_body(&activity), "/items")
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(events.iter().any(|e| {
        e.pointer("/kind") == Some(&json!("audit"))
            && e.pointer("/audit/target") == Some(&json!("/api/runbooks/security-audit/dispatch"))
            && e.pointer("/run_id") == Some(&json!(run_id))
    }));

    // Existing logs & cancel work unchanged on runbook run.
    let logs = call_as(
        &mut engine,
        "GET",
        &format!("/api/runs/{run_id}/logs?node_id=job&stream=diagnostic&cursor=0&max_bytes=256"),
        Value::Null,
        &viewer,
    );
    assert_eq!(logs.status, 200);

    let run_view = json_body(&call_as(
        &mut engine,
        "GET",
        &format!("/api/runs/{run_id}"),
        Value::Null,
        &viewer,
    ));
    let cancel = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{run_id}/control"),
        json!({
            "request_id": "cancel-1",
            "attempt_id": ptr(&run_view, "/attempt_id"),
            "expected_revision": ptr(&run_view, "/revision"),
            "expected_sequence": ptr(&run_view, "/sequence"),
            "execution": ptr(&run_view, "/execution"),
            "action": "cancel"
        }),
        &op,
    );
    assert_eq!(cancel.status, 200);
    assert_eq!(ptr(&json_body(&cancel), "/outcome"), &json!("Cancelled"));

    // Retry (rerun) preserves runbook metadata and bindings.
    let rerun = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{run_id}/rerun"),
        json!({"request_id": "rerun-1", "mode": "exact"}),
        &op,
    );
    assert_eq!(rerun.status, 201);
    assert_eq!(
        ptr(
            &json_body(&rerun),
            "/runbook/env_bindings/job/LATTICE_PARAM_TARGET_DIR"
        ),
        &json!("crates/kiln-core")
    );

    // Ordinary non-runbook run omits `runbook` field.
    let ordinary = call_as(
        &mut engine,
        "POST",
        "/api/runs",
        json!({"project_id": "fixture", "request_id": "ordinary-1"}),
        &op,
    );
    assert!(ordinary.status == 201 || ordinary.status == 202);
    assert!(json_body(&ordinary).get("runbook").is_none());

    // Restart / restore durability: Engine::open_with_ports restores runbook runs cleanly.
    drop(engine);
    let mut reopened = open(profile, backend);
    let restored = call_as(
        &mut reopened,
        "GET",
        &format!("/api/runs/{run_id}"),
        Value::Null,
        &viewer,
    );
    assert_eq!(restored.status, 200);
    assert_eq!(
        ptr(&json_body(&restored), "/runbook/runbook_id"),
        &json!("security-audit")
    );
}

#[test]
fn per_request_reload_revokes_mutating_grant_immediately() {
    let (root, profile, rev) = fixture();
    let runbooks_file = root.path().join("runbooks.json");
    std::fs::write(
        &runbooks_file,
        serde_json::to_vec_pretty(&registry(false)).unwrap_or_else(|e| unreachable!("{e}")),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));

    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend);
    let op = principal("local:operator", Role::Operator);
    engine
        .configure_runbooks_path(&runbooks_file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-1"}),
            &op
        )
        .status,
        200
    );

    // allow_commit_sha=false rejects SHA target pre-dispatch.
    let sha_refused = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"request_id": "sha-1", "target": format!("fixture@{rev}")}),
        &op,
    );
    assert_eq!(sha_refused.status, 400);
    assert_eq!(
        ptr(&json_body(&sha_refused), "/code"),
        &json!("runbook_target_not_allowed")
    );

    // Mutating dispatch succeeds while grant exists.
    let ok = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"request_id": "mut-1", "target": "fixture@refs/heads/main"}),
        &op,
    );
    assert_eq!(ok.status, 201);

    // Remove grant on disk and write back; next dispatch must immediately refuse with 403.
    let mut revoked = registry(false);
    revoked.grants.clear();
    std::fs::write(
        &runbooks_file,
        serde_json::to_vec_pretty(&revoked).unwrap_or_else(|e| unreachable!("{e}")),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));

    let denied = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"request_id": "mut-2", "target": "fixture@refs/heads/main"}),
        &op,
    );
    assert_eq!(denied.status, 403);
    assert_eq!(
        ptr(&json_body(&denied), "/code"),
        &json!("runbook_grant_required")
    );
}

fn commit_branch_update(repository_path: &std::path::Path) -> String {
    let repository = git2::Repository::open(repository_path)
        .unwrap_or_else(|error| unreachable!("open repo: {error}"));
    std::fs::write(
        repository_path.join("source.txt"),
        b"updated runbook source\n",
    )
    .unwrap_or_else(|error| unreachable!("write source: {error}"));
    let mut index = repository
        .index()
        .unwrap_or_else(|error| unreachable!("index: {error}"));
    index
        .add_path(std::path::Path::new("source.txt"))
        .unwrap_or_else(|error| unreachable!("add_path: {error}"));
    let tree_id = index
        .write_tree()
        .unwrap_or_else(|error| unreachable!("write_tree: {error}"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("find_tree: {error}"));
    let parent = repository
        .find_reference("refs/heads/main")
        .and_then(|r| r.peel_to_commit())
        .unwrap_or_else(|error| unreachable!("parent commit: {error}"));
    let signature = git2::Signature::now("test", "test@example.invalid")
        .unwrap_or_else(|error| unreachable!("sig: {error}"));
    repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "second commit",
            &tree,
            &[&parent],
        )
        .unwrap_or_else(|error| unreachable!("commit: {error}"))
        .to_string()
}

#[test]
fn runbook_rerun_refuses_latest_and_preserves_exact_source_on_exact_and_failed() {
    let (_root, profile, rev1) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        failed: true,
        ..Backend::default()
    }));
    let mut engine = open(profile.clone(), Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    let worker = principal("local:worker", Role::Worker);
    engine
        .configure_runbooks(registry(true))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-rerun"}),
            &op
        )
        .status,
        200
    );

    // Dispatch a run that terminates as DomainFailure at rev1.
    let failed_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({
            "request_id": "rb-fail-1",
            "target": "onixresearch/kiln@main"
        }),
        &op,
    );
    assert_eq!(failed_dispatch.status, 201);
    let failed_json = json_body(&failed_dispatch);
    assert_eq!(ptr(&failed_json, "/source_revision"), &json!(rev1));
    assert_eq!(ptr(&failed_json, "/run/outcome"), &json!("DomainFailure"));
    let failed_run_id = ptr(&failed_json, "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("failed run_id"))
        .to_owned();

    // Advance refs/heads/main to rev2 after the initial dispatch.
    let repo_path = profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!("project missing"))
        .repository_path
        .clone();
    let rev2 = commit_branch_update(&repo_path);
    assert_ne!(rev1, rev2);
    {
        let mut guard = backend.lock();
        guard.failed = false;
        guard.terminal = true;
    }

    // mode = "latest" must be refused with 409 runbook_retry_requires_exact_source.
    let latest_refused = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{failed_run_id}/rerun"),
        json!({"request_id": "rerun-latest-refused", "mode": "latest"}),
        &op,
    );
    assert_eq!(latest_refused.status, 409);
    assert_eq!(
        ptr(&json_body(&latest_refused), "/code"),
        &json!("runbook_retry_requires_exact_source")
    );

    // mode = "exact" and mode = "failed" succeed and keep rev1 in both source and runbook.source_env.
    for (req_id, mode) in [("rerun-exact-ok", "exact"), ("rerun-failed-ok", "failed")] {
        let rerun = call_as(
            &mut engine,
            "POST",
            &format!("/api/runs/{failed_run_id}/rerun"),
            json!({"request_id": req_id, "mode": mode}),
            &op,
        );
        assert_eq!(rerun.status, 201, "mode {mode} failed: {rerun:?}");
        let rerun_json = json_body(&rerun);
        assert_eq!(ptr(&rerun_json, "/source_revision"), &json!(rev1));
        assert_eq!(
            ptr(&rerun_json, "/runbook/source_env/LATTICE_SOURCE_REVISION"),
            &json!(rev1)
        );
        let rerun_id = ptr(&rerun_json, "/id")
            .as_str()
            .unwrap_or_else(|| unreachable!("rerun id"));
        let eb = call_as(
            &mut engine,
            "GET",
            &format!("/api/runs/{rerun_id}/execution-binding"),
            Value::Null,
            &worker,
        );
        assert_eq!(eb.status, 200);
        let eb_json = json_body(&eb);
        assert_eq!(
            ptr(&eb_json, "/binding/source/exact_revision/native_id"),
            &json!(rev1)
        );
        assert_eq!(
            ptr(&eb_json, "/runbook/source_env/LATTICE_SOURCE_REVISION"),
            &json!(rev1)
        );
    }
}

fn write_runbooks_file(path: &std::path::Path, reg: &RunbookRegistry) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(reg).unwrap_or_else(|e| unreachable!("serialize runbooks: {e}")),
    )
    .unwrap_or_else(|e| unreachable!("write runbooks: {e}"));
}

#[test]
fn runbook_rerun_enforces_rbac_grant_revocation_and_cross_principal_replay() {
    let (root, profile, _rev) = fixture();
    let runbooks_file = root.path().join("runbooks.json");
    write_runbooks_file(&runbooks_file, &registry(true));

    let backend = Arc::new(Mutex::new(Backend {
        failed: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    let other_op = principal("local:other", Role::Operator);
    let viewer = principal("local:viewer", Role::Viewer);
    engine
        .configure_runbooks_path(&runbooks_file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-rbac"}),
            &op
        )
        .status,
        200
    );

    // Granted operator dispatches mutating runbook and fails (`DomainFailure`).
    let dispatched = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"request_id": "mut-fail-1", "target": "onixresearch/kiln@main"}),
        &op,
    );
    assert_eq!(dispatched.status, 201);
    let failed_run_id = ptr(&json_body(&dispatched), "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("run_id"))
        .to_owned();
    assert_eq!(backend.lock().dispatches, 1);

    {
        let mut guard = backend.lock();
        guard.failed = false;
        guard.terminal = true;
    }

    // Ungranted operator calling POST /api/runs/{id}/rerun (exact, failed, or invalid JSON)
    // must be refused with audited 403 runbook_grant_required before body parsing or dispatch.
    for (req_id, mode) in [("ungranted-exact", "exact"), ("ungranted-failed", "failed")] {
        let denied = call_as(
            &mut engine,
            "POST",
            &format!("/api/runs/{failed_run_id}/rerun"),
            json!({"request_id": req_id, "mode": mode}),
            &other_op,
        );
        assert_eq!(denied.status, 403);
        assert_eq!(
            ptr(&json_body(&denied), "/code"),
            &json!("runbook_grant_required")
        );
    }
    let malformed_denied = engine
        .api_as(
            "POST",
            &format!("/api/runs/{failed_run_id}/rerun"),
            b"{invalid",
            &other_op,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(malformed_denied.status, 403);
    assert_eq!(
        ptr(&json_body(&malformed_denied), "/code"),
        &json!("runbook_grant_required")
    );
    assert_eq!(backend.lock().dispatches, 1);

    // Granted operator reruns with request_id "rr-shared" -> 201, then replays own request_id -> 201.
    let op_rerun = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{failed_run_id}/rerun"),
        json!({"request_id": "rr-shared", "mode": "exact"}),
        &op,
    );
    assert_eq!(op_rerun.status, 201);
    let op_rerun_id = ptr(&json_body(&op_rerun), "/id")
        .as_str()
        .unwrap_or_else(|| unreachable!("op_rerun id"))
        .to_owned();
    assert_eq!(backend.lock().dispatches, 2);

    let op_replay = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{failed_run_id}/rerun"),
        json!({"request_id": "rr-shared", "mode": "exact"}),
        &op,
    );
    assert_eq!(op_replay.status, 201);
    assert_eq!(ptr(&json_body(&op_replay), "/id"), &json!(op_rerun_id));
    assert_eq!(backend.lock().dispatches, 2);

    // Ungranted operator reusing granted operator's request_id gets 403 (NOT 201).
    let cross_ungranted = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{failed_run_id}/rerun"),
        json!({"request_id": "rr-shared", "mode": "exact"}),
        &other_op,
    );
    assert_eq!(cross_ungranted.status, 403);
    assert_eq!(
        ptr(&json_body(&cross_ungranted), "/code"),
        &json!("runbook_grant_required")
    );
    assert_eq!(backend.lock().dispatches, 2);

    // Two authorized operators on a read_only runbook reusing the same request_id get 409 request_id_conflict.
    let ro_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "ro-disp-1",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "src"}
        }),
        &op,
    );
    assert_eq!(ro_dispatch.status, 201);
    let ro_run_id = ptr(&json_body(&ro_dispatch), "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("ro_run_id"))
        .to_owned();
    assert_eq!(backend.lock().dispatches, 3);

    let ro_rerun = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "ro-shared-rerun", "mode": "exact"}),
        &op,
    );
    assert_eq!(ro_rerun.status, 201);
    assert_eq!(backend.lock().dispatches, 4);

    let ro_cross = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "ro-shared-rerun", "mode": "exact"}),
        &other_op,
    );
    assert_eq!(ro_cross.status, 409);
    assert_eq!(
        ptr(&json_body(&ro_cross), "/code"),
        &json!("request_id_conflict")
    );
    assert_eq!(backend.lock().dispatches, 4);

    // Revoke grant on disk: previously-granted operator immediately gets 403 for both fresh request_id and replay.
    let mut revoked = registry(true);
    revoked.grants.clear();
    write_runbooks_file(&runbooks_file, &revoked);

    for (req_id, mode) in [
        ("rr-revoked-exact", "exact"),
        ("rr-revoked-failed", "failed"),
        ("rr-shared", "exact"),
    ] {
        let denied = call_as(
            &mut engine,
            "POST",
            &format!("/api/runs/{failed_run_id}/rerun"),
            json!({"request_id": req_id, "mode": mode}),
            &op,
        );
        assert_eq!(denied.status, 403, "expected 403 for {req_id}/{mode}");
        assert_eq!(
            ptr(&json_body(&denied), "/code"),
            &json!("runbook_grant_required")
        );
    }
    assert_eq!(backend.lock().dispatches, 4);

    // Verify /api/runs/{id}/activity records 403 audit entries bound to run_id for both ungranted and revoked callers.
    let activity = call_as(
        &mut engine,
        "GET",
        &format!("/api/runs/{failed_run_id}/activity?limit=100"),
        Value::Null,
        &viewer,
    );
    assert_eq!(activity.status, 200);
    let items = ptr(&json_body(&activity), "/items")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let expected_target = format!("/api/runs/{failed_run_id}/rerun");
    assert!(items.iter().any(|e| {
        e.pointer("/kind") == Some(&json!("audit"))
            && e.pointer("/audit/actor_id") == Some(&json!("local:other"))
            && e.pointer("/audit/target") == Some(&json!(expected_target))
            && e.pointer("/audit/status") == Some(&json!(403))
            && e.pointer("/run_id") == Some(&json!(failed_run_id))
    }));
    assert!(items.iter().any(|e| {
        e.pointer("/kind") == Some(&json!("audit"))
            && e.pointer("/audit/actor_id") == Some(&json!("local:operator"))
            && e.pointer("/audit/target") == Some(&json!(expected_target))
            && e.pointer("/audit/status") == Some(&json!(403))
            && e.pointer("/run_id") == Some(&json!(failed_run_id))
    }));
}

#[test]
fn runbook_rerun_refuses_policy_drift_on_disk() {
    let (root, mut profile, rev) = fixture();
    profile
        .projects
        .first_mut()
        .unwrap_or_else(|| unreachable!("project missing"))
        .admitted_references
        .push("refs/heads/release".into());
    let runbooks_file = root.path().join("runbooks.json");
    write_runbooks_file(&runbooks_file, &registry(true));

    let backend = Arc::new(Mutex::new(Backend {
        terminal: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    let other_op = principal("local:other", Role::Operator);
    engine
        .configure_runbooks_path(&runbooks_file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-drift"}),
            &op
        )
        .status,
        200
    );

    // Dispatch read_only runbook and SHA-targeted mutating runbook while policy permits both.
    let ro_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "drift-ro-1",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "src"}
        }),
        &other_op,
    );
    assert_eq!(ro_dispatch.status, 201);
    let ro_run_id = ptr(&json_body(&ro_dispatch), "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("ro_run_id"))
        .to_owned();

    let sha_dispatch = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({
            "request_id": "drift-sha-1",
            "target": format!("onixresearch/kiln@{rev}")
        }),
        &op,
    );
    assert_eq!(sha_dispatch.status, 201);
    let sha_run_id = ptr(&json_body(&sha_dispatch), "/run_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("sha_run_id"))
        .to_owned();
    assert_eq!(backend.lock().dispatches, 2);

    // 1. Mode changed read_only -> mutating: ungranted caller gets 403, granted caller gets 409 runbook_mode_changed.
    let mut mode_drifted = registry(true);
    mode_drifted
        .runbooks
        .first_mut()
        .unwrap_or_else(|| unreachable!("runbook missing"))
        .mode = RunbookMode::Mutating;
    mode_drifted
        .grants
        .first_mut()
        .unwrap_or_else(|| unreachable!("grant missing"))
        .runbooks
        .push("security-audit".into());
    write_runbooks_file(&runbooks_file, &mode_drifted);

    let mode_denied = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "drift-mode-ungranted", "mode": "exact"}),
        &other_op,
    );
    assert_eq!(mode_denied.status, 403);
    assert_eq!(
        ptr(&json_body(&mode_denied), "/code"),
        &json!("runbook_grant_required")
    );

    let mode_conflict = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "drift-mode-granted", "mode": "exact"}),
        &op,
    );
    assert_eq!(mode_conflict.status, 409);
    assert_eq!(
        ptr(&json_body(&mode_conflict), "/code"),
        &json!("runbook_mode_changed")
    );
    assert_eq!(backend.lock().dispatches, 2);

    // 2. allow_commit_sha disabled on target rule -> 409 runbook_target_not_allowed.
    let sha_disabled = registry(false);
    write_runbooks_file(&runbooks_file, &sha_disabled);
    let sha_conflict = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{sha_run_id}/rerun"),
        json!({"request_id": "drift-sha-disabled", "mode": "exact"}),
        &op,
    );
    assert_eq!(sha_conflict.status, 409);
    assert_eq!(
        ptr(&json_body(&sha_conflict), "/code"),
        &json!("runbook_target_not_allowed")
    );

    // 3. Target reference removed from allowed_targets -> 409 runbook_target_not_allowed.
    let mut ref_removed = registry(true);
    ref_removed
        .runbooks
        .first_mut()
        .unwrap_or_else(|| unreachable!("runbook missing"))
        .allowed_targets
        .first_mut()
        .unwrap_or_else(|| unreachable!("target missing"))
        .references = vec!["refs/heads/release".into()];
    write_runbooks_file(&runbooks_file, &ref_removed);
    let ref_conflict = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "drift-ref-removed", "mode": "exact"}),
        &op,
    );
    assert_eq!(ref_conflict.status, 409);
    assert_eq!(
        ptr(&json_body(&ref_conflict), "/code"),
        &json!("runbook_target_not_allowed")
    );

    // 4. lattice.workflow_revision changed -> 409 runbook_definition_changed.
    let mut rev_changed = registry(true);
    rev_changed
        .runbooks
        .first_mut()
        .unwrap_or_else(|| unreachable!("runbook missing"))
        .lattice
        .workflow_revision = "f".repeat(64);
    write_runbooks_file(&runbooks_file, &rev_changed);
    let rev_conflict = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "drift-wf-rev", "mode": "exact"}),
        &op,
    );
    assert_eq!(rev_conflict.status, 409);
    assert_eq!(
        ptr(&json_body(&rev_conflict), "/code"),
        &json!("runbook_definition_changed")
    );

    // 5. Runbook deleted from runbooks.json -> 404 runbook_not_found.
    let mut deleted = registry(true);
    deleted.runbooks.retain(|r| r.id != "security-audit");
    write_runbooks_file(&runbooks_file, &deleted);
    let not_found = call_as(
        &mut engine,
        "POST",
        &format!("/api/runs/{ro_run_id}/rerun"),
        json!({"request_id": "drift-deleted", "mode": "exact"}),
        &op,
    );
    assert_eq!(not_found.status, 404);
    assert_eq!(
        ptr(&json_body(&not_found), "/code"),
        &json!("runbook_not_found")
    );
    assert_eq!(backend.lock().dispatches, 2);
}

#[test]
fn auto_retry_failed_skips_failed_runbook_runs() {
    let (_root, mut profile, _rev) = fixture();
    profile
        .projects
        .first_mut()
        .unwrap_or_else(|| unreachable!("project missing"))
        .retry_failed = 2;
    let backend = Arc::new(Mutex::new(Backend {
        failed: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    let viewer = principal("local:viewer", Role::Viewer);
    engine
        .configure_runbooks(registry(true))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        call_as(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id": "fixture", "request_id": "enroll-auto-retry"}),
            &op
        )
        .status,
        200
    );

    let mut_fail = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/flake-update/dispatch",
        json!({"request_id": "auto-mut-fail", "target": "onixresearch/kiln@main"}),
        &op,
    );
    assert_eq!(mut_fail.status, 201);
    assert_eq!(
        ptr(&json_body(&mut_fail), "/run/outcome"),
        &json!("DomainFailure")
    );

    let ro_fail = call_as(
        &mut engine,
        "POST",
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "auto-ro-fail",
            "target": "onixresearch/kiln@main",
            "params": {"target_dir": "src"}
        }),
        &op,
    );
    assert_eq!(ro_fail.status, 201);
    assert_eq!(
        ptr(&json_body(&ro_fail), "/run/outcome"),
        &json!("DomainFailure")
    );
    assert_eq!(backend.lock().dispatches, 2);

    {
        let mut guard = backend.lock();
        guard.failed = false;
        guard.terminal = true;
    }
    engine
        .poll()
        .unwrap_or_else(|e| unreachable!("poll 1: {e}"));
    engine
        .poll()
        .unwrap_or_else(|e| unreachable!("poll 2: {e}"));

    assert_eq!(backend.lock().dispatches, 2);
    let runs = call_as(&mut engine, "GET", "/api/runs", Value::Null, &viewer);
    assert_eq!(runs.status, 200);
    let items = ptr(&json_body(&runs), "/runs")
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(items.len(), 2);
}
