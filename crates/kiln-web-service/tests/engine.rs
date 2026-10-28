use base64::Engine as _;
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

#[derive(Default)]
struct Backend {
    plan: Option<WorkflowEffectPlan>,
    dispatches: usize,
    recoveries: usize,
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
    let profile:ServiceProfile=serde_json::from_value(json!({"schema":"kiln.web-service.v1","service_id":"isolated-tests","listen_address":"127.0.0.1:9443","origin":"https://localhost:9443","tls":{"certificate_path":root.path().join("cert.pem"),"private_key_path":root.path().join("key.pem"),"min_version":"TLS1.3"},"operator_token_file":root.path().join("operator.token"),"worker_token_file":root.path().join("worker.token"),"state_directory":root.path().join("state"),"projects":[{"id":"fixture","label":"Fixture","repository_path":repository_path,"reference":"refs/heads/main","source_limits":{"max_entries":32,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":16384},"workflow":{"socket_path":root.path().join("lattice.sock"),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],"transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},"adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},"policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64","nodes":[{"id":"job","kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"edges":[],"controls":{"cancel":true,"pause":false,"resume":false},"bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}}],"bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,"timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":4,"max_artifact_bytes":1024,"max_artifacts":2,"max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,"max_requests":32,"max_events_per_run":8,"max_state_bytes":1048576}})).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    (root, profile, revision.to_string())
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| unreachable!("fixture field {name}"))
}
fn set(value: &mut Value, name: &str, replacement: Value) {
    value
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("fixture object"))
        .insert(name.to_owned(), replacement);
}
fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}
fn open(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    checked(try_open(profile, backend))
}
fn finish_notifications(
    engine: &mut Engine,
    notifications: Vec<kiln_web_service::engine::NotificationWork>,
) {
    for notification in notifications {
        checked(engine.record_notification(checked(notification.deliver())));
    }
}
fn poll(engine: &mut Engine) {
    let notifications = checked(engine.poll());
    finish_notifications(engine, notifications);
}
fn try_open(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Result<Engine, String> {
    let capability = profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!("fixture first element"))
        .lattice(profile.bounds.max_runs)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .provider;
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    ports.insert(
        "fixture".into(),
        Box::new(Port {
            profile: capability,
            backend,
        }),
    );
    Engine::open_with_ports(profile, ports)
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

#[test]
fn allowlist_exact_source_and_request_replay_survive_restart() {
    let (_root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    assert_eq!(start(&mut engine, "not-enrolled").status, 409);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id":"other","request_id":"other"}),
            Role::Operator
        )
        .status,
        404
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id":"fixture","request_id":"worker-enroll"}),
            Role::Worker
        )
        .status,
        403
    );
    enroll(&mut engine);
    let response = start(&mut engine, "start");
    assert_eq!(response.status, 201);
    let run = value(&response);
    assert_eq!(
        field(&run, "source_revision").as_str(),
        Some(revision.as_str())
    );
    assert_eq!(field(&run, "outcome"), "Pending");
    let plan = backend
        .lock()
        .plan
        .clone()
        .unwrap_or_else(|| unreachable!("fixture plan"));
    let source = plan
        .source
        .unwrap_or_else(|| unreachable!("fixture source"));
    assert_eq!(source.provider.as_str(), "git");
    assert_eq!(source.revision.native_id.as_str(), revision);
    assert_eq!(source.revision.native_kind.as_str(), "revision");
    assert_eq!(
        &serde_json::to_value(source.repository)
            .unwrap_or_else(|error| unreachable!("fixture encoding: {error}")),
        field(&run, "repository")
    );
    assert_eq!(start(&mut engine, "start").body, response.body);
    assert_eq!(backend.lock().dispatches, 1);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id":"fixture","request_id":"start"}),
            Role::Operator
        )
        .status,
        409
    );
    assert_eq!(start(&mut engine, "second-active").status, 202);
    let id = field(&run, "id")
        .as_str()
        .unwrap_or_else(|| unreachable!("fixture string"))
        .to_string();
    drop(engine);
    let mut engine = open(profile.clone(), backend.clone());
    assert_eq!(start(&mut engine, "start").body, response.body);
    poll(&mut engine);
    assert_eq!(backend.lock().dispatches, 1);
    assert_eq!(backend.lock().recoveries, 1);
    assert_eq!(
        value(&get(
            &mut engine,
            &format!("/api/runs/{id}"),
            Role::Operator
        ))
        .get("execution")
        .unwrap_or_else(|| unreachable!("fixture field")),
        field(&run, "execution")
    );
    drop(engine);
    let mut conflict = profile;
    conflict
        .projects
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture first element"))
        .reference = "refs/heads/changed".into();
    let expected = conflict
        .projects
        .first()
        .unwrap_or_else(|| unreachable!("fixture first element"))
        .lattice(conflict.bounds.max_runs)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .provider;
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    ports.insert(
        "fixture".into(),
        Box::new(Port {
            profile: expected,
            backend,
        }),
    );
    assert!(Engine::open_with_ports(conflict, ports).is_err());
}

#[test]
fn accepted_disconnect_blocks_new_dispatch_and_recovers_original_execution() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        disconnect: true,
        ..Backend::default()
    }));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let response = start(&mut engine, "ambiguous");
    let run = value(&response);
    assert_eq!(field(&run, "outcome"), "Unknown");
    assert_eq!(start(&mut engine, "not-retry").status, 202);
    drop(engine);
    {
        let mut state = backend.lock();
        state.disconnect = false;
        state.sequence = 3;
        state.terminal = true;
    }
    let mut engine = open(profile, backend.clone());
    poll(&mut engine);
    let resolved = value(&get(
        &mut engine,
        &format!(
            "/api/runs/{}",
            field(&run, "id")
                .as_str()
                .unwrap_or_else(|| unreachable!("fixture string"))
        ),
        Role::Operator,
    ));
    assert_eq!(field(&resolved, "outcome"), "Success");
    assert_eq!(field(&resolved, "phase"), "terminal");
    assert_eq!(
        field(&resolved, "execution")
            .get("native_id")
            .unwrap_or_else(|| unreachable!("fixture execution")),
        "original-execution"
    );
    assert_eq!(backend.lock().dispatches, 2);
    assert_eq!(backend.lock().recoveries, 1);
}

#[test]
fn controls_reject_stale_and_unsupported_before_mutation_and_replay_exact_result() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "start"));
    let path = format!(
        "/api/runs/{}/control",
        field(&run, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("fixture string"))
    );
    let request = json!({"request_id":"cancel","attempt_id":field(&run, "attempt_id"),"expected_revision":field(&run, "revision"),"expected_sequence":field(&run, "sequence"),"execution":field(&run, "execution"),"action":"cancel"});
    let mut stale = request.clone();
    set(&mut stale, "expected_revision", json!(99));
    assert_eq!(
        call(&mut engine, "POST", &path, stale, Role::Operator).status,
        409
    );
    let mut unsupported = request.clone();
    set(&mut unsupported, "action", json!("pause"));
    assert_eq!(
        call(&mut engine, "POST", &path, unsupported, Role::Operator).status,
        409
    );
    assert_eq!(backend.lock().controls, 0);
    assert_eq!(
        call(&mut engine, "POST", &path, request.clone(), Role::Worker).status,
        403
    );
    let response = call(&mut engine, "POST", &path, request.clone(), Role::Operator);
    assert_eq!(response.status, 200);
    assert_eq!(field(&value(&response), "outcome"), "Cancelled");
    assert_eq!(
        call(&mut engine, "POST", &path, request, Role::Operator).body,
        response.body
    );
    assert_eq!(backend.lock().controls, 1);
}

#[test]
fn artifacts_are_real_bounded_identity_checked_retained_bytes_and_worker_is_narrow() {
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "start"));
    let id = field(&run, "id")
        .as_str()
        .unwrap_or_else(|| unreachable!("fixture string"));
    let path = format!("/api/runs/{id}/artifacts");
    let bytes = b"actual uploaded report\n";
    assert_eq!(get(&mut engine, "/api/runs", Role::Worker).status, 403);
    assert_eq!(
        get(
            &mut engine,
            &format!("/api/runs?project_id=fixture&source_revision={revision}"),
            Role::Worker
        )
        .status,
        200
    );
    let upload = json!({"attempt_id":field(&run, "attempt_id"),"execution":field(&run, "execution"),"source_revision":revision,"node_id":"job","name":"report.txt","body_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"blake3":blake3::hash(bytes).to_hex().as_str()});
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Operator).status,
        403
    );
    let mut bad = upload.clone();
    set(&mut bad, "blake3", json!("a".repeat(64)));
    assert_eq!(
        call(&mut engine, "POST", &path, bad, Role::Worker).status,
        400
    );
    let mut bad = upload.clone();
    set(&mut bad, "source_revision", json!("b".repeat(40)));
    assert_eq!(
        call(&mut engine, "POST", &path, bad, Role::Worker).status,
        409
    );
    let mut bad = upload.clone();
    set(&mut bad, "node_id", json!("unknown"));
    assert_eq!(
        call(&mut engine, "POST", &path, bad, Role::Worker).status,
        409
    );
    let mut bad = upload.clone();
    set(&mut bad, "name", json!("../outside"));
    assert_eq!(
        call(&mut engine, "POST", &path, bad, Role::Worker).status,
        400
    );
    let oversized = vec![b'x'; 1025];
    let mut bad = upload.clone();
    set(
        &mut bad,
        "body_base64",
        json!(base64::engine::general_purpose::STANDARD.encode(&oversized)),
    );
    set(
        &mut bad,
        "blake3",
        json!(blake3::hash(&oversized).to_hex().as_str()),
    );
    assert_eq!(
        call(&mut engine, "POST", &path, bad, Role::Worker).status,
        400
    );
    let response = call(&mut engine, "POST", &path, upload.clone(), Role::Worker);
    assert_eq!(response.status, 201);
    let artifact = value(&response);
    let download = field(&artifact, "download_path")
        .as_str()
        .unwrap_or_else(|| unreachable!("fixture string"))
        .to_string();
    assert_eq!(get(&mut engine, &download, Role::Operator).body, bytes);
    assert_eq!(get(&mut engine, &download, Role::Worker).status, 403);
    assert_eq!(
        call(&mut engine, "POST", &path, upload, Role::Worker).status,
        200
    );
    drop(engine);
    let mut engine = open(profile, backend);
    assert_eq!(get(&mut engine, &download, Role::Operator).body, bytes);
    let stored = root.path().join("state/artifacts").join(
        field(&artifact, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("fixture string")),
    );
    std::fs::remove_file(&stored).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::os::unix::fs::symlink(root.path().join("repository/source.txt"), stored)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(get(&mut engine, &download, Role::Operator).status, 503);
}

#[test]
fn native_log_cursor_gap_and_bounds_are_preserved() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend);
    enroll(&mut engine);
    let run = value(&start(&mut engine, "start"));
    let path = format!(
        "/api/runs/{}/logs",
        field(&run, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("fixture string"))
    );
    let response = get(
        &mut engine,
        &format!("{path}?node_id=job&stream=diagnostic&cursor=0&max_bytes=3"),
        Role::Operator,
    );
    assert_eq!(response.status, 200);
    let chunk = value(&response);
    assert_eq!(field(&chunk, "cursor"), 4);
    assert_eq!(field(&chunk, "next_cursor"), 7);
    assert_eq!(field(&chunk, "text"), "lin");
    assert_eq!(field(&chunk, "gap"), true);
    assert_eq!(field(&chunk, "retained"), true);
    assert_eq!(field(&chunk, "source"), "native-provider");
    assert_eq!(
        get(
            &mut engine,
            &format!("{path}?node_id=job&stream=diagnostic&cursor=0&max_bytes=4097"),
            Role::Operator
        )
        .status,
        400
    );
    assert_eq!(
        get(
            &mut engine,
            &format!("{path}?node_id=unknown&stream=diagnostic&cursor=0&max_bytes=3"),
            Role::Operator
        )
        .status,
        400
    );
}

#[test]
fn private_state_ownership_and_unknown_profile_authority_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let (root, profile, _) = fixture();
    let mut unknown = serde_json::to_value(&profile)
        .unwrap_or_else(|error| unreachable!("fixture encoding: {error}"));
    set(&mut unknown, "command", json!("/bin/sh"));
    assert!(serde_json::from_value::<ServiceProfile>(unknown).is_err());
    std::fs::create_dir(&profile.state_directory)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::fs::set_permissions(
        &profile.state_directory,
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(Engine::open(profile.clone()).is_err());
    std::fs::remove_dir(&profile.state_directory)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir(&elsewhere)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::os::unix::fs::symlink(&elsewhere, &profile.state_directory)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(Engine::open(profile).is_err());
}

#[test]
fn timed_out_dispatch_remains_unknown_and_control_disconnect_never_replays_mutation() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        timeout: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "timeout"));
    assert_eq!(field(&run, "outcome"), "Unknown");
    assert_eq!(field(&run, "error_code"), "provider_timed_out");
    assert_eq!(start(&mut engine, "new-after-timeout").status, 202);
    assert_eq!(backend.lock().dispatches, 1);
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        control_disconnect: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "start"));
    let path = format!(
        "/api/runs/{}/control",
        field(&run, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("fixture string"))
    );
    let request = json!({"request_id":"ambiguous-control","attempt_id":field(&run, "attempt_id"),"expected_revision":field(&run, "revision"),"expected_sequence":field(&run, "sequence"),"execution":field(&run, "execution"),"action":"cancel"});
    let response = call(&mut engine, "POST", &path, request.clone(), Role::Operator);
    assert_eq!(response.status, 503);
    assert_eq!(
        call(&mut engine, "POST", &path, request.clone(), Role::Operator).body,
        response.body
    );
    poll(&mut engine);
    assert_eq!(
        value(&get(
            &mut engine,
            &format!(
                "/api/runs/{}",
                field(&run, "id")
                    .as_str()
                    .unwrap_or_else(|| unreachable!("fixture string"))
            ),
            Role::Operator
        ))
        .get("outcome")
        .unwrap_or_else(|| unreachable!("fixture field")),
        "Cancelled"
    );
    assert_eq!(
        call(&mut engine, "POST", &path, request, Role::Operator).body,
        response.body
    );
    assert_eq!(backend.lock().controls, 1);
}

#[test]
fn restart_discards_unadmitted_artifact_bytes_without_inventing_metadata() {
    use std::os::unix::fs::PermissionsExt;
    let (root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let engine = open(profile.clone(), backend.clone());
    drop(engine);
    let temporary = root
        .path()
        .join("state/artifacts")
        .join(format!("{}.tmp", "a".repeat(64)));
    std::fs::write(&temporary, b"interrupted upload")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut engine = open(profile, backend);
    assert!(!temporary.exists());
    assert_eq!(
        value(&get(&mut engine, "/api/runs", Role::Operator))
            .get("runs")
            .unwrap_or(&serde_json::Value::Null),
        &json!([])
    );
}

#[test]
fn profile_load_accepts_reviewed_nested_fields_and_rejects_unknown_native_bounds() {
    let (root, profile, _) = fixture();
    let path = root.path().join("profile.json");
    let admitted = serde_json::to_value(&profile)
        .unwrap_or_else(|error| unreachable!("fixture encoding: {error}"));
    std::fs::write(
        &path,
        serde_json::to_vec(&admitted)
            .unwrap_or_else(|error| unreachable!("fixture encoding: {error}")),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(ServiceProfile::load(&path).is_ok());
    for location in [
        "source_limits",
        "workflow-bounds",
        "workflow-controls",
        "transport_limits",
    ] {
        let mut altered = admitted.clone();
        let project = altered
            .pointer_mut("/projects/0")
            .unwrap_or_else(|| unreachable!("fixture project"));
        let target = match location {
            "source_limits" => project.pointer_mut("/source_limits"),
            "workflow-bounds" => project.pointer_mut("/workflow/bounds"),
            "workflow-controls" => project.pointer_mut("/workflow/controls"),
            _ => project.pointer_mut("/workflow/transport_limits"),
        }
        .unwrap_or_else(|| unreachable!("fixture nested contract"));
        target
            .as_object_mut()
            .unwrap_or_else(|| unreachable!("fixture object"))
            .insert("undeclared_authority".into(), json!(true));
        std::fs::write(
            &path,
            serde_json::to_vec(&altered)
                .unwrap_or_else(|error| unreachable!("fixture encoding: {error}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(
            ServiceProfile::load(&path).is_err(),
            "accepted unknown {location}"
        );
    }
}

#[test]
fn missing_reply_after_durable_start_returns_original_resource_without_redispatch() {
    let (root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let original = value(&start(&mut engine, "start"));
    drop(engine);
    let database = rusqlite::Connection::open(root.path().join("state/service.sqlite"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let bytes: Vec<u8> = database
        .query_row("SELECT state FROM service_state WHERE id=1", [], |row| {
            row.get(0)
        })
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut state: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let request = state
        .pointer_mut("/requests/start")
        .unwrap_or_else(|| unreachable!("fixture start request"));
    request
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("fixture object"))
        .insert("response".into(), Value::Null);
    database
        .execute(
            "UPDATE service_state SET state=?1 WHERE id=1",
            rusqlite::params![
                serde_json::to_vec(&state)
                    .unwrap_or_else(|error| unreachable!("fixture encoding: {error}"))
            ],
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    drop(database);
    let mut engine = open(profile, backend.clone());
    let replay = start(&mut engine, "start");
    assert_eq!(replay.status, 201);
    assert_eq!(value(&replay).get("id"), original.get("id"));
    assert_eq!(backend.lock().dispatches, 1);
}

#[test]
fn failed_durable_reservation_prevents_provider_effects_and_further_authority() {
    let (root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let database = rusqlite::Connection::open(root.path().join("state/service.sqlite"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    database.execute_batch("CREATE TRIGGER reject_state BEFORE UPDATE ON service_state BEGIN SELECT RAISE(ABORT, 'isolated disk failure'); END;").unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let request = serde_json::to_vec(&json!({"project_id":"fixture","request_id":"failed-start"}))
        .unwrap_or_else(|error| unreachable!("fixture encoding: {error}"));
    assert!(
        engine
            .api("POST", "/api/runs", &request, Role::Operator)
            .is_err()
    );
    assert_eq!(start(&mut engine, "no-effects-after-failure").status, 503);
    assert!(engine.poll().is_err());
    assert_eq!(backend.lock().dispatches, 0);
}

fn log_upload(run: &Value, stream: &str, cursor: u64, bytes: &[u8], truncated: bool) -> Value {
    json!({"attempt_id":field(run,"attempt_id"),"execution":field(run,"execution"),
        "source_revision":field(run,"source_revision"),"node_id":"job","stream":stream,
        "cursor":cursor,"body_base64":base64::engine::general_purpose::STANDARD.encode(bytes),
        "blake3":blake3::hash(bytes).to_hex().as_str(),"truncated":truncated})
}
fn log_path(run: &Value) -> String {
    format!(
        "/api/runs/{}/logs",
        field(run, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("run id"))
    )
}
fn read_capture(
    engine: &mut Engine,
    path: &str,
    stream: &str,
    cursor: u64,
    max: usize,
) -> ApiResponse {
    get(
        engine,
        &format!("{path}?node_id=job&stream={stream}&cursor={cursor}&max_bytes={max}"),
        Role::Operator,
    )
}

#[test]
fn captured_logs_round_trip_empty_chunks_byte_cursors_and_restart_without_native_reads() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "captured"));
    let path = log_path(&run);
    let missing = value(&read_capture(&mut engine, &path, "stderr", 0, 4));
    assert_eq!(field(&missing, "retained"), false);
    let empty = log_upload(&run, "stderr", 0, b"", false);
    assert_eq!(
        call(&mut engine, "POST", &path, empty.clone(), Role::Worker).status,
        201
    );
    assert_eq!(
        call(&mut engine, "POST", &path, empty, Role::Worker).status,
        200
    );
    let stderr = value(&read_capture(&mut engine, &path, "stderr", 0, 4));
    assert_eq!(field(&stderr, "text"), "");
    assert_eq!(field(&stderr, "next_cursor"), 0);
    assert_eq!(field(&stderr, "retained"), true);
    // Empty capture markers do not consume a byte cursor; a later step can append.
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stderr", 0, b"err", false),
            Role::Worker
        )
        .status,
        201
    );
    let first = log_upload(&run, "stdout", 0, b"A\xc3", false);
    let second = log_upload(&run, "stdout", 2, b"\xa9\n", true);
    let response = call(&mut engine, "POST", &path, first.clone(), Role::Worker);
    assert_eq!(response.status, 201);
    assert_eq!(field(&value(&response), "next_cursor"), 2);
    assert_eq!(
        call(&mut engine, "POST", &path, second, Role::Worker).status,
        201
    );
    let all = value(&read_capture(&mut engine, &path, "stdout", 0, 4));
    assert_eq!(field(&all, "text"), "Aé\n");
    assert_eq!(field(&all, "next_cursor"), 4);
    assert_eq!(field(&all, "source"), "worker-capture");
    assert_eq!(field(&all, "gap"), false);
    assert_eq!(field(&all, "truncated"), true);
    // Lossy text length never masquerades as consumed source-byte length.
    let split = value(&read_capture(&mut engine, &path, "stdout", 1, 1));
    assert_eq!(field(&split, "text"), "�");
    assert_eq!(field(&split, "cursor"), 1);
    assert_eq!(field(&split, "next_cursor"), 2);
    assert_eq!(read_capture(&mut engine, &path, "stdout", 5, 4).status, 400);
    let done = value(&read_capture(&mut engine, &path, "stdout", 4, 4));
    assert_eq!(field(&done, "text"), "");
    assert_eq!(field(&done, "next_cursor"), 4);
    drop(engine);
    let mut engine = open(profile, backend.clone());
    assert_eq!(
        value(&read_capture(&mut engine, &path, "stdout", 0, 4)),
        all
    );
    assert_eq!(backend.lock().recoveries, 0);
    assert_eq!(
        call(&mut engine, "POST", &path, first, Role::Worker).status,
        200
    );
    assert_eq!(backend.lock().dispatches, 1);
}

#[test]
fn log_ingress_rejects_private_roles_stale_identity_conflicts_and_open_json() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "ingress"));
    let path = log_path(&run);
    let upload = log_upload(&run, "stdout", 0, b"original", false);
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Operator).status,
        403
    );
    for suffix in ["", "?node_id=job&stream=stdout&cursor=0&max_bytes=8"] {
        assert_eq!(
            get(&mut engine, &format!("{path}{suffix}"), Role::Worker).status,
            403
        );
    }
    for (key, replacement, status) in [
        ("attempt_id", json!("old-attempt"), 409),
        ("source_revision", json!("b".repeat(40)), 409),
        ("node_id", json!("other"), 409),
        ("stream", json!("diagnostic"), 400),
        ("blake3", json!("a".repeat(64)), 400),
        ("body_base64", json!("not base64"), 400),
        ("undeclared", json!(true), 400),
    ] {
        let mut changed = upload.clone();
        set(&mut changed, key, replacement);
        assert_eq!(
            call(&mut engine, "POST", &path, changed, Role::Worker).status,
            status,
            "{key}"
        );
    }
    let mut stale_execution = upload.clone();
    let mut execution = field(&run, "execution").clone();
    set(&mut execution, "native_id", json!("different-execution"));
    set(&mut stale_execution, "execution", execution);
    assert_eq!(
        call(&mut engine, "POST", &path, stale_execution, Role::Worker).status,
        409
    );
    let mut missing = upload.clone();
    checked(missing.as_object_mut().ok_or("object")).remove("truncated");
    assert_eq!(
        call(&mut engine, "POST", &path, missing, Role::Worker).status,
        400
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 1, b"out-of-order", false),
            Role::Worker
        )
        .status,
        409
    );
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Worker).status,
        201
    );
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Worker).status,
        200
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 0, b"changed!", false),
            Role::Worker
        )
        .status,
        409
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 0, b"original", true),
            Role::Worker
        )
        .status,
        409
    );
    assert_eq!(
        field(
            &value(&read_capture(&mut engine, &path, "stdout", 0, 8)),
            "text"
        ),
        "original"
    );
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 8, b"late", false),
            Role::Worker
        )
        .status,
        409
    );
    assert_eq!(backend.lock().controls, 0);
    assert_eq!(backend.lock().dispatches, 1);
}

#[test]
fn log_chunk_count_total_bytes_and_chunk_bytes_are_actual_run_bounds() {
    for count_limited in [false, true] {
        let (_root, mut profile, _) = fixture();
        let project = profile
            .projects
            .first_mut()
            .unwrap_or_else(|| unreachable!("project"));
        project.workflow.bounds.max_log_read_bytes = 4;
        project.workflow.bounds.max_total_log_bytes = if count_limited { 16 } else { 6 };
        project.workflow.bounds.max_log_references = if count_limited { 2 } else { 8 };
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = open(profile, backend);
        enroll(&mut engine);
        let run = value(&start(&mut engine, "bounded"));
        assert_eq!(
            field(&run, "log_bounds"),
            &json!({"maximum_chunk_bytes":4,"maximum_total_bytes":if count_limited {16}else{6},"maximum_chunks":if count_limited {2}else{8}})
        );
        let path = log_path(&run);
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path,
                log_upload(&run, "stdout", 0, b"12345", false),
                Role::Worker
            )
            .status,
            400
        );
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path,
                log_upload(&run, "stdout", 0, b"1234", false),
                Role::Worker
            )
            .status,
            201
        );
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path,
                log_upload(&run, "stderr", 0, b"56", true),
                Role::Worker
            )
            .status,
            201
        );
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path,
                log_upload(&run, "stdout", 4, b"7", false),
                Role::Worker
            )
            .status,
            409
        );
        assert_eq!(
            field(
                &value(&read_capture(&mut engine, &path, "stdout", 0, 4)),
                "text"
            ),
            "1234"
        );
        assert_eq!(
            field(
                &value(&read_capture(&mut engine, &path, "stderr", 0, 4)),
                "truncated"
            ),
            true
        );
    }
}

fn stored_log(profile: &ServiceProfile) -> std::path::PathBuf {
    let mut entries = checked(std::fs::read_dir(profile.state_directory.join("logs")));
    let entry = checked(entries.next().ok_or("retained log missing"));
    checked(entry).path()
}

#[test]
fn log_bytes_and_metadata_tampering_fail_on_read_and_recovery() {
    for tamper in ["bytes", "symlink", "hardlink", "mode", "metadata"] {
        use std::os::unix::fs::PermissionsExt;
        let (_root, profile, _) = fixture();
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = open(profile.clone(), backend.clone());
        enroll(&mut engine);
        let run = value(&start(&mut engine, "tamper"));
        let path = log_path(&run);
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path,
                log_upload(&run, "stdout", 0, b"actual", false),
                Role::Worker
            )
            .status,
            201
        );
        let retained = stored_log(&profile);
        match tamper {
            "bytes" => checked(std::fs::write(&retained, b"forged")),
            "symlink" => {
                checked(std::fs::remove_file(&retained));
                checked(std::os::unix::fs::symlink(
                    profile.state_directory.join("service.sqlite"),
                    &retained,
                ));
            }
            "hardlink" => checked(std::fs::hard_link(
                &retained,
                profile.state_directory.join("extra-link"),
            )),
            "mode" => checked(std::fs::set_permissions(
                &retained,
                std::fs::Permissions::from_mode(0o644),
            )),
            _ => {}
        }
        if tamper != "metadata" {
            assert_eq!(
                read_capture(&mut engine, &path, "stdout", 0, 6).status,
                503,
                "{tamper}"
            );
            assert_eq!(
                call(
                    &mut engine,
                    "POST",
                    &path,
                    log_upload(&run, "stdout", 0, b"actual", false),
                    Role::Worker
                )
                .status,
                503
            );
        }
        drop(engine);
        if tamper == "metadata" {
            let database = checked(rusqlite::Connection::open(
                profile.state_directory.join("service.sqlite"),
            ));
            let bytes: Vec<u8> = checked(database.query_row(
                "SELECT state FROM service_state WHERE id=1",
                [],
                |row| row.get(0),
            ));
            let mut state: Value = checked(serde_json::from_slice(&bytes));
            let pointer = format!(
                "/runs/{}/logs/0/truncated",
                field(&run, "id")
                    .as_str()
                    .unwrap_or_else(|| unreachable!("run id"))
            );
            *state
                .pointer_mut(&pointer)
                .unwrap_or_else(|| unreachable!("stored chunk")) = json!(true);
            checked(database.execute(
                "UPDATE service_state SET state=?1 WHERE id=1",
                rusqlite::params![checked(serde_json::to_vec(&state))],
            ));
        }
        assert!(try_open(profile, backend).is_err(), "recovered {tamper}");
    }
}

#[test]
fn restart_removes_only_owned_unadmitted_log_files_and_recovers_publication_pairs() {
    use std::os::unix::fs::PermissionsExt;
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "orphan"));
    let path = log_path(&run);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 0, b"kept", false),
            Role::Worker
        )
        .status,
        201
    );
    let retained = stored_log(&profile);
    drop(engine);
    let pair = retained.with_extension("tmp");
    checked(std::fs::hard_link(&retained, &pair));
    let orphan = profile.state_directory.join("logs").join("a".repeat(64));
    checked(std::fs::write(&orphan, b"unadmitted"));
    checked(std::fs::set_permissions(
        &orphan,
        std::fs::Permissions::from_mode(0o600),
    ));
    let mut engine = open(profile.clone(), backend.clone());
    assert!(!pair.exists());
    assert!(!orphan.exists());
    assert_eq!(
        field(
            &value(&read_capture(&mut engine, &path, "stdout", 0, 4)),
            "text"
        ),
        "kept"
    );
    drop(engine);
    checked(std::os::unix::fs::symlink(&retained, &orphan));
    assert!(try_open(profile, backend).is_err());
}

#[path = "engine/runtime.rs"]
mod runtime;

#[path = "engine/automation.rs"]
mod automation;
#[path = "engine/dashboard.rs"]
mod dashboard;
#[path = "engine/reports.rs"]
mod reports;

#[path = "engine/workers.rs"]
mod workers;

#[path = "engine/registry.rs"]
mod registry;

#[path = "engine/telemetry.rs"]
mod telemetry;

#[path = "engine/cache.rs"]
mod cache;

#[path = "engine/snapshots.rs"]
mod snapshots;
