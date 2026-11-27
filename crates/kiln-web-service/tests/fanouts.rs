use kiln_adapter_github::profile::ConfigEffectClass;
use kiln_core::domain::Outcome;
use kiln_core::fleet::{ClaimState, FleetAgent, FleetClaim, MirrorStatus, parse_scope_text};
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{ContentId, EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult, WorkflowPort};
use kiln_web_service::access::Principal;
use kiln_web_service::engine::{ApiResponse, Engine, FleetClaimsSnapshot, Role};
use kiln_web_service::profile::ServiceProfile;
use kiln_web_service::runbooks::{
    ParamBinding, ParamDeclaration, ParamType, ParamValue, RUNBOOK_REGISTRY_SCHEMA,
    RunbookDefinition, RunbookFanoutConfig, RunbookGrant, RunbookLattice, RunbookMode,
    RunbookRegistry, RunbookTarget,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PortMode {
    Running,
    Success,
    DomainFailure,
    Unknown,
}

struct Backend {
    mode: PortMode,
    plans: BTreeMap<String, WorkflowEffectPlan>,
    dispatches: usize,
    controls: usize,
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            mode: PortMode::Running,
            plans: BTreeMap::new(),
            dispatches: 0,
            controls: 0,
        }
    }
}

struct Port {
    profile: CapabilityProfile,
    backend: Arc<Mutex<Backend>>,
}

fn make_observation(plan: &WorkflowEffectPlan, mode: PortMode) -> PortResult<WorkflowObservation> {
    if mode == PortMode::Unknown {
        return Err(PortFailure::Unknown(InfrastructureFailure {
            code: "provider_ambiguous".into(),
            message: "provider outcome ambiguous".into(),
        }));
    }
    let terminal = matches!(mode, PortMode::Success | PortMode::DomainFailure);
    let node_state = match mode {
        PortMode::Running | PortMode::Unknown => WorkflowNodeState::Running,
        PortMode::Success => WorkflowNodeState::Completed,
        PortMode::DomainFailure => WorkflowNodeState::Failed,
    };
    let state = match mode {
        PortMode::Running | PortMode::Unknown => WorkflowState::Running,
        PortMode::Success => WorkflowState::Completed,
        PortMode::DomainFailure => WorkflowState::Failed,
    };
    let outcome = match mode {
        PortMode::Running | PortMode::Unknown => Outcome::Pending,
        PortMode::Success => Outcome::Success,
        PortMode::DomainFailure => Outcome::DomainFailure,
    };
    Ok(WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: ExternalRef::new("lattice", "workflow", "fanout-wf")
            .unwrap_or_else(|e| unreachable!("{e:?}")),
        execution: ExternalRef::new("lattice", "execution", plan.effect_id.as_str())
            .unwrap_or_else(|e| unreachable!("{e:?}")),
        sequence: if terminal { 1 } else { 0 },
        state,
        terminal,
        successful: mode == PortMode::Success,
        outcome,
        nodes: plan
            .nodes
            .iter()
            .map(|n| WorkflowNodeObservation {
                node_id: n.id.clone(),
                state: node_state,
                terminal,
                successful: mode == PortMode::Success,
                adapter: ExternalRef::new("lattice", "adapter", "local")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                receipt: terminal.then(|| {
                    ExternalRef::new("lattice", "receipt", "node-receipt")
                        .unwrap_or_else(|e| unreachable!("{e:?}"))
                }),
            })
            .collect(),
        logs: Vec::new(),
        receipt: terminal.then(|| {
            ExternalRef::new("lattice", "receipt", "execution-receipt")
                .unwrap_or_else(|e| unreachable!("{e:?}"))
        }),
    })
}

impl WorkflowPort for Port {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut guard = self.backend.lock();
        guard.dispatches += 1;
        let key = plan.effect_id.as_str().to_owned();
        guard.plans.insert(key, plan.clone());
        make_observation(plan, guard.mode)
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let guard = self.backend.lock();
        make_observation(plan, guard.mode)
    }
    fn observe(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        let guard = self.backend.lock();
        let plan = guard
            .plans
            .get(id.as_str())
            .unwrap_or_else(|| unreachable!("plan missing for {id:?}"));
        make_observation(plan, guard.mode)
    }
    fn control(&mut self, _request: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        let mut guard = self.backend.lock();
        guard.controls += 1;
        Err(PortFailure::Domain(kiln_core::Diagnostic::new(
            "control_unused",
            "control",
            "control unused",
        )))
    }
    fn reconcile(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(id)
    }
    fn read_log(&mut self, _request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        Err(PortFailure::Domain(kiln_core::Diagnostic::new(
            "logs_unused",
            "logs",
            "logs unused",
        )))
    }
}

fn init_git_repo(path: &std::path::Path, msg: &str) -> String {
    let repo = git2::Repository::init(path).unwrap_or_else(|e| unreachable!("{e:?}"));
    std::fs::write(path.join("source.txt"), format!("{msg}\n").as_bytes())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut index = repo.index().unwrap_or_else(|e| unreachable!("{e:?}"));
    index
        .add_path(std::path::Path::new("source.txt"))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let tree_id = index.write_tree().unwrap_or_else(|e| unreachable!("{e:?}"));
    let tree = repo
        .find_tree(tree_id)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let sig = git2::Signature::now("test", "test@example.invalid")
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    repo.commit(Some("refs/heads/main"), &sig, &sig, msg, &tree, &[])
        .unwrap_or_else(|e| unreachable!("{e:?}"))
        .to_string()
}

fn advance_git_repo(path: &std::path::Path, msg: &str) -> String {
    let repo = git2::Repository::open(path).unwrap_or_else(|e| unreachable!("{e:?}"));
    let head = repo
        .find_reference("refs/heads/main")
        .and_then(|r| r.peel_to_commit())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    std::fs::write(path.join("source.txt"), format!("{msg}\n").as_bytes())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut index = repo.index().unwrap_or_else(|e| unreachable!("{e:?}"));
    index
        .add_path(std::path::Path::new("source.txt"))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let tree_id = index.write_tree().unwrap_or_else(|e| unreachable!("{e:?}"));
    let tree = repo
        .find_tree(tree_id)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let sig = git2::Signature::now("test", "test@example.invalid")
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    repo.commit(Some("refs/heads/main"), &sig, &sig, msg, &tree, &[&head])
        .unwrap_or_else(|e| unreachable!("{e:?}"))
        .to_string()
}

struct ProjectSpec<'a> {
    id: &'a str,
    repo_slug: &'a str,
    scope_root: Option<&'a str>,
}

fn multi_fixture(
    specs: &[ProjectSpec<'_>],
) -> (
    tempfile::TempDir,
    ServiceProfile,
    BTreeMap<String, PathBuf>,
    BTreeMap<String, String>,
) {
    let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut repo_paths = BTreeMap::new();
    let mut revisions = BTreeMap::new();
    let mut projects_json = Vec::new();

    for spec in specs {
        let dir = root.path().join(format!("repo-{}", spec.id));
        let rev = init_git_repo(&dir, spec.id);
        repo_paths.insert(spec.id.to_owned(), dir.clone());
        revisions.insert(spec.id.to_owned(), rev);
        let mut obj = json!({
            "id": spec.id,
            "label": spec.repo_slug,
            "repository_path": dir,
            "reference": "refs/heads/main",
            "source_limits": {
                "max_entries": 32,
                "max_path_bytes": 1024,
                "max_file_bytes": 4096,
                "max_total_bytes": 16384
            },
            "workflow": {
                "socket_path": root.path().join(format!("{}.sock", spec.id)),
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
        });
        if let Some(scope_root) = spec.scope_root
            && let Some(map) = obj.as_object_mut()
        {
            map.insert("scope".into(), json!({ "root": scope_root }));
        }
        projects_json.push(obj);
    }

    let profile: ServiceProfile = serde_json::from_value(json!({
        "schema": "kiln.web-service.v1",
        "service_id": "fanout-tests",
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
        "projects": projects_json,
        "bounds": {
            "max_request_bytes": 16384,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 4,
            "timeout_seconds": 2,
            "poll_interval_ms": 10,
            "max_projects": 16,
            "max_runs": 64,
            "max_artifact_bytes": 1024,
            "max_artifacts": 2,
            "max_run_artifact_bytes": 2048,
            "max_total_artifact_bytes": 4096,
            "max_log_read_bytes": 4096,
            "max_requests": 128,
            "max_events_per_run": 8,
            "max_state_bytes": 1048576
        }
    }))
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    (root, profile, repo_paths, revisions)
}

fn open_engine_with_backends(
    profile: ServiceProfile,
) -> (Engine, BTreeMap<String, Arc<Mutex<Backend>>>) {
    let mut ports: BTreeMap<String, Box<dyn kiln_provider_api::WorkflowPort + Send>> =
        BTreeMap::new();
    let mut backends = BTreeMap::new();
    for project in &profile.projects {
        let backend = Arc::new(Mutex::new(Backend::default()));
        let cap = project
            .execution_capabilities(profile.bounds.max_runs)
            .unwrap_or_else(|e| unreachable!("{e}"));
        ports.insert(
            project.id.clone(),
            Box::new(Port {
                profile: cap,
                backend: Arc::clone(&backend),
            }),
        );
        backends.insert(project.id.clone(), backend);
    }
    let engine = Engine::open_with_ports(profile, ports).unwrap_or_else(|e| unreachable!("{e}"));
    (engine, backends)
}

fn build_registry(specs: &[ProjectSpec<'_>]) -> RunbookRegistry {
    let nodes = vec![kiln_adapter_github::profile::ConfigNode {
        id: "job".into(),
        kind: "test".into(),
        effect_class: ConfigEffectClass::Test,
        adapter_kind: "local".into(),
        operation_identity: "a".repeat(64),
    }];
    let targets: Vec<RunbookTarget> = specs
        .iter()
        .map(|s| RunbookTarget {
            repository: s.repo_slug.to_owned(),
            project_id: s.id.to_owned(),
            references: vec!["refs/heads/main".into()],
            allow_commit_sha: true,
        })
        .collect();
    RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![
            RunbookDefinition {
                id: "ro-audit".into(),
                display_name: "RO Audit".into(),
                description: "Read-only audit".into(),
                mode: RunbookMode::ReadOnly,
                approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: Some("ro-audit".into()),
                    bundle_identity: "b".repeat(64),
                    workflow_name: "ro-audit".into(),
                    workflow_revision: "c".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![ParamDeclaration {
                    name: "strict".into(),
                    param_type: ParamType::Bool {},
                    required: false,
                    default: Some(ParamValue::Bool(true)),
                    bindings: vec![ParamBinding {
                        node: "job".into(),
                        shell_env: "LATTICE_PARAM_STRICT".into(),
                    }],
                }],
                allowed_targets: targets.clone(),
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 2,
                    max_concurrency_limit: 8,
                    default_budget_cap: 16,
                    max_budget_cap: 64,
                    max_targets: 32,
                    write_scopes: vec![],
                }),
            },
            RunbookDefinition {
                id: "mut-update".into(),
                display_name: "Mut Update".into(),
                description: "Mutating update".into(),
                mode: RunbookMode::Mutating,
                approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: Some("mut-update".into()),
                    bundle_identity: "d".repeat(64),
                    workflow_name: "mut-update".into(),
                    workflow_revision: "e".repeat(64),
                    nodes,
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: targets,
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 2,
                    max_concurrency_limit: 8,
                    default_budget_cap: 16,
                    max_budget_cap: 64,
                    max_targets: 32,
                    write_scopes: vec![".".into()],
                }),
            },
        ],
        grants: vec![RunbookGrant {
            principal: "op:granted".into(),
            runbooks: vec!["mut-update".into()],
            not_after_ms: None,
        }],
        notifications: Vec::new(),
    }
}

fn operator(id: &str) -> Principal {
    Principal {
        id: id.into(),
        label: id.into(),
        role: Role::Operator,
        agent: None,
    }
}

fn post(engine: &mut Engine, path: &str, body: Value, principal: &Principal) -> ApiResponse {
    let bytes = serde_json::to_vec(&body).unwrap_or_else(|e| unreachable!("{e:?}"));
    engine
        .api_as("POST", path, &bytes, principal)
        .unwrap_or_else(|e| unreachable!("{e}"))
}

fn get(engine: &mut Engine, path: &str, principal: &Principal) -> ApiResponse {
    engine
        .api_as("GET", path, &[], principal)
        .unwrap_or_else(|e| unreachable!("{e}"))
}

fn json_body(resp: &ApiResponse) -> Value {
    serde_json::from_slice(&resp.body).unwrap_or_else(|e| unreachable!("{e:?}"))
}

fn enroll(engine: &mut Engine, ids: &[&str], principal: &Principal) {
    for (idx, id) in ids.iter().enumerate() {
        let resp = post(
            engine,
            "/api/projects",
            json!({ "project_id": id, "request_id": format!("enroll-{idx}-{id}") }),
            principal,
        );
        assert_eq!(resp.status, 200);
    }
}

#[test]
fn glob_expansion_refuses_unenrolled_targets_and_freezes_exact_shas() {
    let specs = [
        ProjectSpec {
            id: "kiln-a",
            repo_slug: "onixresearch/kiln-a",
            scope_root: None,
        },
        ProjectSpec {
            id: "kiln-b",
            repo_slug: "onixresearch/kiln-b",
            scope_root: None,
        },
        ProjectSpec {
            id: "kiln-unenrolled",
            repo_slug: "onixresearch/kiln-c",
            scope_root: None,
        },
        ProjectSpec {
            id: "other",
            repo_slug: "other/repo",
            scope_root: None,
        },
    ];
    let (_root, profile, repo_paths, revisions) = multi_fixture(&specs);
    let (mut engine, backends) = open_engine_with_backends(profile);
    engine
        .configure_runbooks(build_registry(&specs))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["kiln-a", "kiln-b", "other"], &op);

    // Explicit unenrolled target -> 409 project_not_enrolled.
    let unenrolled = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "req-unenrolled",
            "targets": ["onixresearch/kiln-c@refs/heads/main"],
            "max_concurrency": 1,
            "budget_cap": 2
        }),
        &op,
    );
    assert_eq!(unenrolled.status, 409);
    assert_eq!(
        json_body(&unenrolled).get("code"),
        Some(&json!("project_not_enrolled"))
    );

    // Glob matching only unenrolled -> 409 project_not_enrolled.
    let glob_unenrolled = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "req-glob-unenrolled",
            "target_globs": ["onixresearch/kiln-c*@refs/heads/main"],
            "max_concurrency": 1,
            "budget_cap": 2
        }),
        &op,
    );
    assert_eq!(glob_unenrolled.status, 409);

    // Glob over onixresearch/* resolves ONLY enrolled targets (kiln-a, kiln-b),
    // excludes kiln-unenrolled, freezes initial SHAs even if kiln-b advances later!
    let resp = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "req-glob-ok",
            "target_globs": ["onixresearch/*@refs/heads/main"],
            "max_concurrency": 1,
            "budget_cap": 2
        }),
        &op,
    );
    assert_eq!(resp.status, 202);
    let body = json_body(&resp);
    let fanout_id = body
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();
    let children = body
        .pointer("/fanout/children")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("children"));
    assert_eq!(children.len(), 2);
    assert_eq!(
        children.first().and_then(|c| c.get("project_id")),
        Some(&json!("kiln-a"))
    );
    assert_eq!(
        children.get(1).and_then(|c| c.get("project_id")),
        Some(&json!("kiln-b"))
    );
    assert_eq!(
        children.get(1).and_then(|c| c.get("status")),
        Some(&json!("pending"))
    );

    let initial_sha_b = revisions.get("kiln-b").cloned().unwrap_or_default();
    assert_eq!(
        children
            .get(1)
            .and_then(|c| c.get("source_revision"))
            .and_then(Value::as_str),
        Some(initial_sha_b.as_str())
    );

    // Advance branch on kiln-b BEFORE child 1 is admitted.
    let path_b = repo_paths
        .get("kiln-b")
        .unwrap_or_else(|| unreachable!("path"));
    let new_sha_b = advance_git_repo(path_b, "moved-head");
    assert_ne!(new_sha_b, initial_sha_b);

    // Complete kiln-a -> poll() admits kiln-b with FROZEN initial_sha_b!
    if let Some(b_a) = backends.get("kiln-a") {
        b_a.lock().mode = PortMode::Success;
    }
    if let Some(b_b) = backends.get("kiln-b") {
        b_b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));

    let inspected = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        &op,
    ));
    assert_eq!(inspected.get("phase"), Some(&json!("completed")));
    let run_b = inspected
        .pointer("/children/1/run_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("run_id"));
    let run_view = json_body(&get(&mut engine, &format!("/api/runs/{run_b}"), &op));
    assert_eq!(
        run_view.get("source_revision").and_then(Value::as_str),
        Some(initial_sha_b.as_str())
    );
    assert_eq!(
        run_view.get("fanout_run_id").and_then(Value::as_str),
        Some(fanout_id.as_str())
    );
}

#[test]
fn concurrency_budget_cap_and_partial_failure_unknown_isolation() {
    let specs = [
        ProjectSpec {
            id: "p0",
            repo_slug: "onixresearch/p0",
            scope_root: None,
        },
        ProjectSpec {
            id: "p1",
            repo_slug: "onixresearch/p1",
            scope_root: None,
        },
        ProjectSpec {
            id: "p2",
            repo_slug: "onixresearch/p2",
            scope_root: None,
        },
        ProjectSpec {
            id: "p3",
            repo_slug: "onixresearch/p3",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let (mut engine, backends) = open_engine_with_backends(profile);
    engine
        .configure_runbooks(build_registry(&specs))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["p0", "p1", "p2", "p3"], &op);

    // 4 targets, max_concurrency = 2, budget_cap = 3.
    let resp = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "req-conc-budget",
            "target_globs": ["onixresearch/*@refs/heads/main"],
            "max_concurrency": 2,
            "budget_cap": 3
        }),
        &op,
    );
    assert_eq!(resp.status, 202);
    let body = json_body(&resp);
    let fanout_id = body
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();
    assert_eq!(body.pointer("/fanout/counts/active"), Some(&json!(2)));
    assert_eq!(body.pointer("/fanout/counts/pending"), Some(&json!(2)));

    // p0 fails (DomainFailure), p1 becomes Unknown -> both release concurrency slots,
    // no sibling is cancelled, no blind retry happens, and p2 is admitted!
    if let Some(b) = backends.get("p0") {
        b.lock().mode = PortMode::DomainFailure;
    }
    if let Some(b) = backends.get("p1") {
        b.lock().mode = PortMode::Unknown;
    }
    if let Some(b) = backends.get("p2") {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));

    let after_first_poll = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        &op,
    ));
    assert_eq!(after_first_poll.get("phase"), Some(&json!("completed")));
    assert_eq!(
        after_first_poll.get("stop_reason"),
        Some(&json!("fanout_budget_exhausted"))
    );
    assert_eq!(
        after_first_poll.get("counts"),
        Some(&json!({
            "total": 4,
            "pending": 0,
            "active": 0,
            "success": 1,
            "failure": 1,
            "unknown": 1,
            "skipped": 1
        }))
    );
    assert_eq!(
        after_first_poll.pointer("/children/3/reason"),
        Some(&json!("fanout_budget_exhausted"))
    );
    // p0 & p1 were dispatched once only (no auto-retry).
    assert_eq!(backends.get("p0").map(|b| b.lock().dispatches), Some(1));
    assert_eq!(backends.get("p1").map(|b| b.lock().dispatches), Some(1));
    assert_eq!(backends.get("p3").map(|b| b.lock().dispatches), Some(0));

    // Later p1 Unknown reconciles to Success on poll(), updating the ledger!
    if let Some(b) = backends.get("p1") {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));
    let after_reconcile = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        &op,
    ));
    assert_eq!(after_reconcile.pointer("/counts/success"), Some(&json!(2)));
    assert_eq!(after_reconcile.pointer("/counts/unknown"), Some(&json!(0)));
    assert_eq!(backends.get("p1").map(|b| b.lock().dispatches), Some(1));
}

#[test]
fn pause_continue_cancel_rbac_idempotency_and_definition_drift() {
    let specs = [
        ProjectSpec {
            id: "p0",
            repo_slug: "onixresearch/p0",
            scope_root: None,
        },
        ProjectSpec {
            id: "p1",
            repo_slug: "onixresearch/p1",
            scope_root: None,
        },
        ProjectSpec {
            id: "p2",
            repo_slug: "onixresearch/p2",
            scope_root: None,
        },
    ];
    let (root, profile, _paths, _revs) = multi_fixture(&specs);
    let runbooks_path = root.path().join("runbooks.json");
    let mut reg = build_registry(&specs);
    std::fs::write(
        &runbooks_path,
        serde_json::to_vec(&reg).unwrap_or_else(|e| unreachable!("{e:?}")),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    let (mut engine, backends) = open_engine_with_backends(profile);
    engine
        .configure_runbooks_path(&runbooks_path)
        .unwrap_or_else(|e| unreachable!("{e}"));

    let granted = operator("op:granted");
    let ungranted = operator("op:other");
    let viewer = Principal {
        id: "viewer:1".into(),
        label: "Viewer".into(),
        role: Role::Viewer,
        agent: None,
    };
    let worker = Principal {
        id: "worker:1".into(),
        label: "Worker".into(),
        role: Role::Worker,
        agent: None,
    };
    enroll(&mut engine, &["p0", "p1", "p2"], &granted);

    // Viewer & Worker & ungranted Operator refused on mutating fan-out dispatch.
    let v_resp = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({ "request_id": "v1", "targets": ["p0"] }),
        &viewer,
    );
    assert_eq!(v_resp.status, 403);
    let w_resp = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({ "request_id": "w1", "targets": ["p0"] }),
        &worker,
    );
    assert_eq!(w_resp.status, 403);
    let u_resp = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({ "request_id": "u1", "targets": ["p0"] }),
        &ungranted,
    );
    assert_eq!(u_resp.status, 403);
    assert_eq!(
        json_body(&u_resp).get("code"),
        Some(&json!("runbook_grant_required"))
    );

    // Dispatch mutating fan-out with max_concurrency = 1 over 3 targets.
    let dispatch = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "mut-req-1",
            "target_globs": ["onixresearch/*@refs/heads/main"],
            "max_concurrency": 1,
            "budget_cap": 3
        }),
        &granted,
    );
    assert_eq!(dispatch.status, 202);
    let fanout_id = json_body(&dispatch)
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();

    // Exact replay returns replayed: true; mismatch/cross-principal returns 409.
    let replay = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "mut-req-1",
            "target_globs": ["onixresearch/*@refs/heads/main"],
            "max_concurrency": 1,
            "budget_cap": 3
        }),
        &granted,
    );
    assert_eq!(replay.status, 202);
    assert_eq!(json_body(&replay).get("replayed"), Some(&json!(true)));
    let conflict = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "mut-req-1",
            "target_globs": ["onixresearch/*@refs/heads/main"],
            "max_concurrency": 2,
            "budget_cap": 3
        }),
        &granted,
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(
        json_body(&conflict).get("code"),
        Some(&json!("request_id_conflict"))
    );

    // Pause admission: active p0 continues, p1 is NOT admitted even after p0 completes.
    let paused = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/pause"),
        json!({ "request_id": "pause-1" }),
        &granted,
    );
    assert_eq!(paused.status, 200);
    assert_eq!(
        json_body(&paused).pointer("/fanout/admission"),
        Some(&json!("paused"))
    );
    assert_eq!(
        json_body(&paused).pointer("/fanout/phase"),
        Some(&json!("paused"))
    );

    if let Some(b) = backends.get("p0") {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));
    let while_paused = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        &viewer,
    ));
    assert_eq!(while_paused.pointer("/counts/success"), Some(&json!(1)));
    assert_eq!(while_paused.pointer("/counts/pending"), Some(&json!(2)));
    assert_eq!(backends.get("p1").map(|b| b.lock().dispatches), Some(0));

    // Mode drift on disk -> continue refuses with 409 runbook_mode_changed!
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb.mode = RunbookMode::ReadOnly;
        if let Some(fo) = mut_rb.fanout.as_mut() {
            fo.write_scopes.clear();
        }
    }
    std::fs::write(
        &runbooks_path,
        serde_json::to_vec(&reg).unwrap_or_else(|e| unreachable!("{e:?}")),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let drift_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-drift" }),
        &granted,
    );
    assert_eq!(drift_resp.status, 409);
    assert_eq!(
        json_body(&drift_resp).get("code"),
        Some(&json!("runbook_mode_changed"))
    );

    // Restore mutating mode -> continue resumes admission of p1!
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb.mode = RunbookMode::Mutating;
        if let Some(fo) = mut_rb.fanout.as_mut() {
            fo.write_scopes = vec![".".into()];
        }
    }
    std::fs::write(
        &runbooks_path,
        serde_json::to_vec(&reg).unwrap_or_else(|e| unreachable!("{e:?}")),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    let resumed = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-1" }),
        &granted,
    );
    assert_eq!(resumed.status, 200);
    assert_eq!(
        json_body(&resumed).pointer("/fanout/counts/active"),
        Some(&json!(1))
    );
    assert_eq!(
        json_body(&resumed).pointer("/fanout/counts/pending"),
        Some(&json!(1))
    );

    // Cancel marks p2 skipped without cancelling running p1; subsequent continue fails 409.
    let cancelled = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/cancel"),
        json!({ "request_id": "cancel-1" }),
        &granted,
    );
    assert_eq!(cancelled.status, 200);
    assert_eq!(
        json_body(&cancelled).pointer("/fanout/admission"),
        Some(&json!("cancelled"))
    );
    assert_eq!(
        json_body(&cancelled).pointer("/fanout/counts/active"),
        Some(&json!(1))
    );
    assert_eq!(
        json_body(&cancelled).pointer("/fanout/counts/skipped"),
        Some(&json!(1))
    );
    assert_eq!(backends.get("p1").map(|b| b.lock().controls), Some(0));

    let post_cancel_cont = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-after-cancel" }),
        &granted,
    );
    assert_eq!(post_cancel_cont.status, 409);
    assert_eq!(
        json_body(&post_cancel_cont).get("code"),
        Some(&json!("fanout_cancelled"))
    );
}

#[test]
fn fleet_claim_overlap_refuses_skips_serializes_and_checks_monorepo_scopes() {
    let specs = [
        ProjectSpec {
            id: "kiln-core",
            repo_slug: "onixresearch/kiln",
            scope_root: Some("crates/kiln-core"),
        },
        ProjectSpec {
            id: "kiln-web",
            repo_slug: "onixresearch/kiln",
            scope_root: Some("crates/kiln-web-service"),
        },
        ProjectSpec {
            id: "lattice",
            repo_slug: "onixresearch/lattice",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let (mut engine, backends) = open_engine_with_backends(profile);
    let mut reg = build_registry(&specs);
    // Allow same repo for monorepo subprojects by scoping allowed_targets to distinct projects?
    // Note RunbookRegistry::admit requires unique target repositories per runbook, so test external
    // claim overlap against two distinct repos AND internal monorepo sibling serialization across two runbooks or single-run + fanout!
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb
            .allowed_targets
            .retain(|t| t.project_id != "kiln-web");
    }
    if let Some(ro_rb) = reg.runbooks.get_mut(0) {
        ro_rb.allowed_targets.retain(|t| t.project_id != "kiln-web");
    }
    engine
        .configure_runbooks(reg)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["kiln-core", "kiln-web", "lattice"], &op);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1_000_000);
    let claim = FleetClaim {
        identity: ContentId::from_blake3_hex("f".repeat(64))
            .unwrap_or_else(|e| unreachable!("{e:?}")),
        scope: parse_scope_text("OnixResearch/kiln .").unwrap_or_else(|e| unreachable!("{e:?}")),
        holder: FleetAgent {
            tool: "codex".into(),
            host: "aspen3".into(),
            session: "s-claim".into(),
        },
        purpose: "edit kiln".into(),
        fence: 1,
        initial_fence: None,
        granted_ms: now_ms.saturating_sub(1_000),
        expires_ms: now_ms.saturating_add(600_000),
        state: ClaimState::Open,
        attempt: None,
        claim_mirror: MirrorStatus::Posted { message_id: None },
        release_mirror: MirrorStatus::Pending,
    };

    // Fleet unavailable fails closed (503).
    engine.set_fleet_claims_snapshot(FleetClaimsSnapshot::Unavailable);
    let unavail = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({ "request_id": "fc-unavail", "targets": ["kiln-core"] }),
        &op,
    );
    assert_eq!(unavail.status, 503);
    assert_eq!(
        json_body(&unavail).get("code"),
        Some(&json!("fleet_unavailable"))
    );

    // Live claim on onixresearch/kiln . refuses mutating fan-out with Refuse policy.
    engine.set_fleet_claims_snapshot(FleetClaimsSnapshot::Available(vec![claim.clone()]));
    let refused = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "fc-refuse",
            "targets": ["kiln-core", "lattice"],
            "claim_policy": "refuse"
        }),
        &op,
    );
    assert_eq!(refused.status, 409);
    assert_eq!(
        json_body(&refused).get("code"),
        Some(&json!("fleet_claim_conflict"))
    );

    // Read-only fan-out is NOT blocked by write claims.
    let ro_ok = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "fc-ro",
            "targets": ["kiln-core", "lattice"]
        }),
        &op,
    );
    assert_eq!(ro_ok.status, 202);
    for b in backends.values() {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));
    for b in backends.values() {
        b.lock().mode = PortMode::Running;
    }

    // Skip policy marks conflicting kiln-core Skipped and admits non-overlapping lattice.
    let skipped = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "fc-skip",
            "targets": ["kiln-core", "lattice"],
            "claim_policy": "skip"
        }),
        &op,
    );
    assert_eq!(skipped.status, 202);
    let skip_body = json_body(&skipped);
    assert_eq!(skip_body.pointer("/fanout/counts/skipped"), Some(&json!(1)));
    assert_eq!(skip_body.pointer("/fanout/counts/active"), Some(&json!(1)));
    assert_eq!(
        skip_body.pointer("/fanout/children/0/status"),
        Some(&json!("skipped"))
    );
    assert_eq!(
        skip_body.pointer("/fanout/children/1/status"),
        Some(&json!("active"))
    );

    // Serialize policy keeps kiln-core Pending while claim is live, then admits once claim clears!
    let serialized = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "fc-ser",
            "targets": ["kiln-core"],
            "claim_policy": "serialize"
        }),
        &op,
    );
    assert_eq!(serialized.status, 202);
    let ser_body = json_body(&serialized);
    let ser_id = ser_body
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();
    assert_eq!(ser_body.pointer("/fanout/counts/pending"), Some(&json!(1)));
    assert_eq!(ser_body.pointer("/fanout/counts/active"), Some(&json!(0)));

    // Release claim -> poll() admits kiln-core!
    engine.set_fleet_claims_snapshot(FleetClaimsSnapshot::Available(vec![]));
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));
    let after_release = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{ser_id}"),
        &op,
    ));
    assert_eq!(after_release.pointer("/counts/active"), Some(&json!(1)));

    // Ensure no unused import warnings on BTreeSet.
    let _ = BTreeSet::<String>::new();
}

#[test]
fn durable_state_persists_and_reloads_fanouts_across_restart() {
    let specs = [
        ProjectSpec {
            id: "p0",
            repo_slug: "onixresearch/p0",
            scope_root: None,
        },
        ProjectSpec {
            id: "p1",
            repo_slug: "onixresearch/p1",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let op = operator("op:granted");
    let fanout_id = {
        let (mut engine, backends) = open_engine_with_backends(profile.clone());
        engine
            .configure_runbooks(build_registry(&specs))
            .unwrap_or_else(|e| unreachable!("{e}"));
        enroll(&mut engine, &["p0", "p1"], &op);
        if let Some(b) = backends.get("p0") {
            b.lock().mode = PortMode::Success;
        }
        let resp = post(
            &mut engine,
            "/api/runbooks/ro-audit/fanout",
            json!({
                "request_id": "persist-1",
                "targets": ["p0", "p1"],
                "max_concurrency": 1,
                "budget_cap": 2
            }),
            &op,
        );
        assert_eq!(resp.status, 202);
        json_body(&resp)
            .get("fanout_id")
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("fanout_id"))
            .to_owned()
    };

    // Reopen Engine from SQLite durable state.
    let (mut reopened, backends) = open_engine_with_backends(profile);
    reopened
        .configure_runbooks(build_registry(&specs))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let list = json_body(&get(
        &mut reopened,
        "/api/runbooks/fanouts?order=desc&limit=10",
        &op,
    ));
    assert_eq!(list.get("total"), Some(&json!(1)));
    assert_eq!(list.pointer("/fanouts/0/id"), Some(&json!(fanout_id)));

    if let Some(b) = backends.get("p1") {
        b.lock().mode = PortMode::Success;
    }
    reopened.poll().unwrap_or_else(|e| unreachable!("{e}"));
    let done = json_body(&get(
        &mut reopened,
        &format!("/api/runbooks/fanouts/{fanout_id}"),
        &op,
    ));
    assert_eq!(done.get("phase"), Some(&json!("completed")));
    assert_eq!(done.pointer("/counts/success"), Some(&json!(2)));
}

#[test]
fn internal_monorepo_sibling_scopes_serialize_when_overlapping_and_run_concurrently_when_disjoint()
{
    let specs = [
        ProjectSpec {
            id: "kiln-core",
            repo_slug: "onixresearch/kiln",
            scope_root: Some("crates/kiln-core"),
        },
        ProjectSpec {
            id: "kiln-web",
            repo_slug: "onixresearch/kiln",
            scope_root: Some("crates/kiln-web-service"),
        },
        ProjectSpec {
            id: "kiln-root",
            repo_slug: "onixresearch/kiln",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let (mut engine, backends) = open_engine_with_backends(profile);
    let nodes = vec![kiln_adapter_github::profile::ConfigNode {
        id: "job".into(),
        kind: "test".into(),
        effect_class: ConfigEffectClass::Test,
        adapter_kind: "local".into(),
        operation_identity: "a".repeat(64),
    }];
    let reg = RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![
            RunbookDefinition {
                id: "mut-core".into(),
                display_name: "Mut Core".into(),
                description: "Mutating core".into(),
                mode: RunbookMode::Mutating,
                approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: None,
                    bundle_identity: "d".repeat(64),
                    workflow_name: "mut-core".into(),
                    workflow_revision: "e".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-core".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: false,
                }],
                fanout: None,
            },
            RunbookDefinition {
                id: "mut-web".into(),
                display_name: "Mut Web".into(),
                description: "Mutating web".into(),
                mode: RunbookMode::Mutating,
                approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: None,
                    bundle_identity: "d".repeat(64),
                    workflow_name: "mut-web".into(),
                    workflow_revision: "e".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-web".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: false,
                }],
                fanout: None,
            },
            RunbookDefinition {
                id: "mut-root".into(),
                display_name: "Mut Root".into(),
                description: "Mutating root".into(),
                mode: RunbookMode::Mutating,
                approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: None,
                    bundle_identity: "d".repeat(64),
                    workflow_name: "mut-root".into(),
                    workflow_revision: "e".repeat(64),
                    nodes,
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-root".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: false,
                }],
                fanout: None,
            },
        ],
        grants: vec![RunbookGrant {
            principal: "op:granted".into(),
            runbooks: vec!["mut-core".into(), "mut-web".into(), "mut-root".into()],
            not_after_ms: None,
        }],
        notifications: Vec::new(),
    };
    engine
        .configure_runbooks(reg)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["kiln-core", "kiln-web", "kiln-root"], &op);

    // 1. Start mut-core (scope: crates/kiln-core).
    let fo_core = post(
        &mut engine,
        "/api/runbooks/mut-core/fanout",
        json!({ "request_id": "mono-core", "targets": ["kiln-core"] }),
        &op,
    );
    assert_eq!(fo_core.status, 202);
    assert_eq!(
        json_body(&fo_core).pointer("/fanout/counts/active"),
        Some(&json!(1))
    );

    // 2. Disjoint mut-web (scope: crates/kiln-web-service) runs concurrently!
    let fo_web = post(
        &mut engine,
        "/api/runbooks/mut-web/fanout",
        json!({ "request_id": "mono-web", "targets": ["kiln-web"] }),
        &op,
    );
    assert_eq!(fo_web.status, 202);
    assert_eq!(
        json_body(&fo_web).pointer("/fanout/counts/active"),
        Some(&json!(1))
    );

    // 3. Overlapping mut-root (scope: .) stays Pending until active overlapping runs finish!
    let fo_root = post(
        &mut engine,
        "/api/runbooks/mut-root/fanout",
        json!({ "request_id": "mono-root", "targets": ["kiln-root"] }),
        &op,
    );
    assert_eq!(fo_root.status, 202);
    let root_body = json_body(&fo_root);
    let root_id = root_body
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();
    assert_eq!(root_body.pointer("/fanout/counts/pending"), Some(&json!(1)));
    assert_eq!(root_body.pointer("/fanout/counts/active"), Some(&json!(0)));

    // Finish core & web -> poll() admits mut-root!
    if let Some(b) = backends.get("kiln-core") {
        b.lock().mode = PortMode::Success;
    }
    if let Some(b) = backends.get("kiln-web") {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));
    let root_after = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{root_id}"),
        &op,
    ));
    assert_eq!(root_after.pointer("/counts/active"), Some(&json!(1)));
}

#[test]
fn combined_targets_and_globs_over_bound_returns_400_not_503() {
    let specs = [
        ProjectSpec {
            id: "p0",
            repo_slug: "onixresearch/p0",
            scope_root: None,
        },
        ProjectSpec {
            id: "p1",
            repo_slug: "onixresearch/p1",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let (mut engine, _backends) = open_engine_with_backends(profile);
    let mut reg = build_registry(&specs);
    if let Some(ro_rb) = reg.runbooks.get_mut(0)
        && let Some(fo) = ro_rb.fanout.as_mut()
    {
        fo.max_targets = 32;
    }
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(fo) = mut_rb.fanout.as_mut()
    {
        fo.max_targets = 64;
    }
    engine
        .configure_runbooks(reg)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["p0", "p1"], &op);

    // 25 explicit targets + 10 globs = 35 > max_targets (32) -> 400 fanout_targets_bound (not 503).
    let resp_32 = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "bound-35",
            "targets": vec!["p0"; 25],
            "target_globs": vec!["onixresearch/*"; 10]
        }),
        &op,
    );
    assert_eq!(resp_32.status, 400);
    assert_eq!(
        json_body(&resp_32).get("code"),
        Some(&json!("fanout_targets_bound"))
    );

    // 55 explicit targets + 10 globs = 65 > max_targets (64) -> 400 fanout_targets_bound (not 503).
    let resp_64 = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "bound-65",
            "targets": vec!["p0"; 55],
            "target_globs": vec!["onixresearch/*"; 10]
        }),
        &op,
    );
    assert_eq!(resp_64.status, 400);
    assert_eq!(
        json_body(&resp_64).get("code"),
        Some(&json!("fanout_targets_bound"))
    );

    // 17 globs (> MAX_FANOUT_GLOBS = 16) with max_targets = 32 still returns 400 fanout_glob_invalid.
    let resp_glob = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "bound-globs",
            "targets": [],
            "target_globs": vec!["onixresearch/*"; 17]
        }),
        &op,
    );
    assert_eq!(resp_glob.status, 400);
    assert_eq!(
        json_body(&resp_glob).get("code"),
        Some(&json!("fanout_glob_invalid"))
    );
}

#[test]
fn derive_write_scope_enforces_write_scopes_and_rejects_readonly_scope_path() {
    let specs = [
        ProjectSpec {
            id: "kiln-core",
            repo_slug: "onixresearch/kiln",
            scope_root: Some("crates/kiln-core"),
        },
        ProjectSpec {
            id: "kiln-root",
            repo_slug: "onixresearch/kiln",
            scope_root: None,
        },
    ];
    let (_root, profile, _paths, _revs) = multi_fixture(&specs);
    let (mut engine, _backends) = open_engine_with_backends(profile);
    let mut reg = build_registry(&specs);
    // Keep only kiln-core in ro-audit and mut-update (unique repository per runbook),
    // and add mut-docs-core, mut-docs-root, mut-multi-root.
    if let Some(ro_rb) = reg.runbooks.get_mut(0) {
        ro_rb
            .allowed_targets
            .retain(|t| t.project_id == "kiln-core");
    }
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb
            .allowed_targets
            .retain(|t| t.project_id == "kiln-core");
    }
    let nodes = reg
        .runbooks
        .get(1)
        .map(|rb| rb.lattice.nodes.clone())
        .unwrap_or_default();
    let make_mut_rb = |id: &str, project_id: &str, write_scopes: Vec<&str>| RunbookDefinition {
        id: id.into(),
        display_name: id.into(),
        description: id.into(),
        mode: RunbookMode::Mutating,
        approval_gate: kiln_web_service::runbooks::ApprovalGate::None,
        lattice: RunbookLattice {
            bundle_id: Some(id.into()),
            bundle_identity: "d".repeat(64),
            workflow_name: id.into(),
            workflow_revision: "e".repeat(64),
            nodes: nodes.clone(),
            edges: vec![],
        },
        params: vec![],
        allowed_targets: vec![RunbookTarget {
            repository: "onixresearch/kiln".into(),
            project_id: project_id.into(),
            references: vec!["refs/heads/main".into()],
            allow_commit_sha: false,
        }],
        fanout: Some(RunbookFanoutConfig {
            enabled: true,
            default_max_concurrency: 1,
            max_concurrency_limit: 4,
            default_budget_cap: 4,
            max_budget_cap: 16,
            max_targets: 8,
            write_scopes: write_scopes.into_iter().map(String::from).collect(),
        }),
    };
    reg.runbooks
        .push(make_mut_rb("mut-docs-core", "kiln-core", vec!["docs"]));
    reg.runbooks
        .push(make_mut_rb("mut-docs-root", "kiln-root", vec!["docs"]));
    reg.runbooks.push(make_mut_rb(
        "mut-multi-root",
        "kiln-root",
        vec!["docs", "README.md"],
    ));
    if let Some(grant) = reg.grants.get_mut(0) {
        grant.runbooks.extend([
            "mut-docs-core".into(),
            "mut-docs-root".into(),
            "mut-multi-root".into(),
        ]);
    }
    engine
        .configure_runbooks(reg)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["kiln-core", "kiln-root"], &op);

    // 1. ReadOnly runbook with scope_path -> 400 fanout_scope_invalid.
    let ro_scope = post(
        &mut engine,
        "/api/runbooks/ro-audit/fanout",
        json!({
            "request_id": "ro-scope",
            "targets": ["kiln-core"],
            "scope_path": "docs"
        }),
        &op,
    );
    assert_eq!(ro_scope.status, 400);
    assert_eq!(
        json_body(&ro_scope).get("code"),
        Some(&json!("fanout_scope_invalid"))
    );

    // 2. Single disjoint write_scope ("docs") on scoped project ("crates/kiln-core") with omitted scope_path -> 400 fanout_scope_widening.
    let disjoint = post(
        &mut engine,
        "/api/runbooks/mut-docs-core/fanout",
        json!({
            "request_id": "mut-disjoint",
            "targets": ["kiln-core"]
        }),
        &op,
    );
    assert_eq!(disjoint.status, 400);
    assert_eq!(
        json_body(&disjoint).get("code"),
        Some(&json!("fanout_scope_widening"))
    );

    // 3. Single write_scope ("docs") on unscoped project (".") with omitted scope_path -> narrows to "onixresearch/kiln docs".
    let narrowed = post(
        &mut engine,
        "/api/runbooks/mut-docs-root/fanout",
        json!({
            "request_id": "mut-narrowed",
            "targets": ["kiln-root"]
        }),
        &op,
    );
    assert_eq!(narrowed.status, 202);
    assert_eq!(
        json_body(&narrowed).pointer("/fanout/children/0/write_scope"),
        Some(&json!("onixresearch/kiln docs"))
    );

    // 4. Multi-entry write_scopes (["docs", "README.md"]) with omitted scope_path -> 400 fanout_scope_required;
    //    with explicit scope_path: "docs" -> 202 with "onixresearch/kiln docs".
    let multi_missing = post(
        &mut engine,
        "/api/runbooks/mut-multi-root/fanout",
        json!({
            "request_id": "mut-multi-missing",
            "targets": ["kiln-root"]
        }),
        &op,
    );
    assert_eq!(multi_missing.status, 400);
    assert_eq!(
        json_body(&multi_missing).get("code"),
        Some(&json!("fanout_scope_required"))
    );
    let multi_explicit = post(
        &mut engine,
        "/api/runbooks/mut-multi-root/fanout",
        json!({
            "request_id": "mut-multi-explicit",
            "targets": ["kiln-root"],
            "scope_path": "docs"
        }),
        &op,
    );
    assert_eq!(multi_explicit.status, 202);
    assert_eq!(
        json_body(&multi_explicit).pointer("/fanout/children/0/write_scope"),
        Some(&json!("onixresearch/kiln docs"))
    );

    // 5. Existing mut-update (write_scopes: ["."]) on scoped project ("crates/kiln-core") with omitted scope_path
    //    preserves project_scope ("onixresearch/kiln crates/kiln-core").
    let whole_on_core = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "mut-whole-core",
            "targets": ["kiln-core"]
        }),
        &op,
    );
    assert_eq!(whole_on_core.status, 202);
    assert_eq!(
        json_body(&whole_on_core).pointer("/fanout/children/0/write_scope"),
        Some(&json!("onixresearch/kiln crates/kiln-core"))
    );
}

#[test]
fn continue_rejects_disabled_fanout_and_removed_pending_targets() {
    let specs = [
        ProjectSpec {
            id: "p0",
            repo_slug: "onixresearch/p0",
            scope_root: None,
        },
        ProjectSpec {
            id: "p1",
            repo_slug: "onixresearch/p1",
            scope_root: None,
        },
        ProjectSpec {
            id: "p2",
            repo_slug: "onixresearch/p2",
            scope_root: None,
        },
    ];
    let (root, profile, _paths, revs) = multi_fixture(&specs);
    let runbooks_path = root.path().join("runbooks.json");
    let mut reg = build_registry(&specs);
    let write_reg = |reg: &RunbookRegistry| {
        std::fs::write(
            &runbooks_path,
            serde_json::to_vec(reg).unwrap_or_else(|e| unreachable!("{e:?}")),
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    };
    write_reg(&reg);

    let (mut engine, backends) = open_engine_with_backends(profile);
    engine
        .configure_runbooks_path(&runbooks_path)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let op = operator("op:granted");
    enroll(&mut engine, &["p0", "p1", "p2"], &op);

    let rev_p2 = revs
        .get("p2")
        .unwrap_or_else(|| unreachable!("p2 rev"))
        .clone();
    let dispatch = post(
        &mut engine,
        "/api/runbooks/mut-update/fanout",
        json!({
            "request_id": "cont-drift-dispatch",
            "targets": [
                "onixresearch/p0@refs/heads/main",
                "onixresearch/p1@refs/heads/main",
                format!("onixresearch/p2@{rev_p2}")
            ],
            "max_concurrency": 1,
            "budget_cap": 3
        }),
        &op,
    );
    assert_eq!(dispatch.status, 202);
    let fanout_id = json_body(&dispatch)
        .get("fanout_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fanout_id"))
        .to_owned();

    let paused = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/pause"),
        json!({ "request_id": "cont-drift-pause" }),
        &op,
    );
    assert_eq!(paused.status, 200);
    if let Some(b) = backends.get("p0") {
        b.lock().mode = PortMode::Success;
    }
    engine.poll().unwrap_or_else(|e| unreachable!("{e}"));

    // 1. Disable fanout on disk -> continue returns 409 fanout_disabled.
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(fo) = mut_rb.fanout.as_mut()
    {
        fo.enabled = false;
    }
    write_reg(&reg);
    let dis_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-disabled" }),
        &op,
    );
    assert_eq!(dis_resp.status, 409);
    assert_eq!(
        json_body(&dis_resp).get("code"),
        Some(&json!("fanout_disabled"))
    );
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(fo) = mut_rb.fanout.as_mut()
    {
        fo.enabled = true;
    }
    write_reg(&reg);

    // 2. Remove pending target p1 from allowed_targets -> 409 runbook_target_not_allowed, p1 not admitted.
    let saved_targets = reg
        .runbooks
        .get(1)
        .map(|rb| rb.allowed_targets.clone())
        .unwrap_or_default();
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb.allowed_targets.retain(|t| t.project_id != "p1");
    }
    write_reg(&reg);
    let rm_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-rm-target" }),
        &op,
    );
    assert_eq!(rm_resp.status, 409);
    assert_eq!(
        json_body(&rm_resp).get("code"),
        Some(&json!("runbook_target_not_allowed"))
    );
    assert_eq!(backends.get("p1").map(|b| b.lock().dispatches), Some(0));
    if let Some(mut_rb) = reg.runbooks.get_mut(1) {
        mut_rb.allowed_targets = saved_targets;
    }
    write_reg(&reg);

    // 3. Disallow commit SHA on pending target p2 -> 409 runbook_target_not_allowed.
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(p2_rule) = mut_rb
            .allowed_targets
            .iter_mut()
            .find(|t| t.project_id == "p2")
    {
        p2_rule.allow_commit_sha = false;
    }
    write_reg(&reg);
    let sha_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-no-sha" }),
        &op,
    );
    assert_eq!(sha_resp.status, 409);
    assert_eq!(
        json_body(&sha_resp).get("code"),
        Some(&json!("runbook_target_not_allowed"))
    );
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(p2_rule) = mut_rb
            .allowed_targets
            .iter_mut()
            .find(|t| t.project_id == "p2")
    {
        p2_rule.allow_commit_sha = true;
    }
    write_reg(&reg);

    // 4. Narrow write_scopes from ["."] to ["docs"] -> 409 runbook_definition_changed.
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(fo) = mut_rb.fanout.as_mut()
    {
        fo.write_scopes = vec!["docs".into()];
    }
    write_reg(&reg);
    let scope_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-narrow-scope" }),
        &op,
    );
    assert_eq!(scope_resp.status, 409);
    assert_eq!(
        json_body(&scope_resp).get("code"),
        Some(&json!("runbook_definition_changed"))
    );
    if let Some(mut_rb) = reg.runbooks.get_mut(1)
        && let Some(fo) = mut_rb.fanout.as_mut()
    {
        fo.write_scopes = vec![".".into()];
    }
    write_reg(&reg);

    // 5. Restoring valid registry allows continue to succeed and admit p1.
    let ok_resp = post(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/continue"),
        json!({ "request_id": "cont-restored" }),
        &op,
    );
    assert_eq!(ok_resp.status, 200);
    assert_eq!(backends.get("p1").map(|b| b.lock().dispatches), Some(1));
}
