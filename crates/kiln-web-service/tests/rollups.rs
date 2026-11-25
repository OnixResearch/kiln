use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine as _;
use kiln_adapter_github::profile::ConfigEffectClass;
use kiln_core::domain::Outcome;
use kiln_core::implementation::{
    CandidateVerification, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork,
    PublicationPhase, PublicationPlan, PublicationRecord, SourceRef, SourceStatusReceipt,
    TerminalPolicy, VERIFICATION_SCHEMA, VerificationOutcome, WorkBudget, WorkSourceKind,
};
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{ContentId, EffectId, ExternalRef};
use kiln_core::workflow::{
    WorkflowEffectPlan, WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation,
    WorkflowState,
};
use kiln_implementer::Implementer;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult, WorkflowPort};
use kiln_web_service::access::Principal;
use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::profile::{ProjectProfile, ServiceProfile};
use kiln_web_service::rollups::{
    RUNBOOK_DISPATCHES_VIEW_SCHEMA, RUNBOOK_ROLLUP_SCHEMA, RUNBOOK_SUMMARY_NOTIFICATION_SCHEMA,
};
use kiln_web_service::runbooks::{
    ParamBinding, ParamDeclaration, ParamType, ParamValue, RUNBOOK_REGISTRY_SCHEMA,
    RunbookDefinition, RunbookFanoutConfig, RunbookLattice, RunbookMode,
    RunbookNotificationProfile, RunbookRegistry, RunbookTarget,
};
use parking_lot::Mutex;
use serde_json::{Value, json};

const CA_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/key.pem");

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| unreachable!("fixture failed: {e:?}"))
}

fn present<T>(opt: Option<T>) -> T {
    opt.unwrap_or_else(|| unreachable!("expected value missing"))
}

#[derive(Clone, Default)]
struct ProjectBackend {
    outcome: Option<Outcome>,
    fail_unknown: bool,
    sequence: u64,
    plans: BTreeMap<String, WorkflowEffectPlan>,
}

struct MultiPort {
    profile: CapabilityProfile,
    project_id: String,
    backends: Arc<Mutex<BTreeMap<String, ProjectBackend>>>,
}

impl MultiPort {
    fn observation_for(
        &self,
        plan: &WorkflowEffectPlan,
        state: &ProjectBackend,
    ) -> WorkflowObservation {
        let outcome = state.outcome.unwrap_or(Outcome::Pending);
        let terminal = outcome.is_terminal();
        let node_state = match outcome {
            Outcome::Pending => WorkflowNodeState::Running,
            Outcome::Success => WorkflowNodeState::Completed,
            Outcome::Cancelled => WorkflowNodeState::Cancelled,
            Outcome::Unknown => WorkflowNodeState::Unknown,
            _ => WorkflowNodeState::Failed,
        };
        let wf_state = match outcome {
            Outcome::Pending => WorkflowState::Running,
            Outcome::Success => WorkflowState::Completed,
            Outcome::Cancelled => WorkflowState::Cancelled,
            Outcome::Unknown => WorkflowState::Unknown,
            _ => WorkflowState::Failed,
        };
        WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: ExternalRef::new("lattice", "workflow", "rollup-wf")
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            execution: ExternalRef::new(
                "lattice",
                "execution",
                &format!("exec-{}", self.project_id),
            )
            .unwrap_or_else(|e| unreachable!("{e:?}")),
            sequence: state.sequence,
            state: wf_state,
            terminal,
            successful: outcome == Outcome::Success,
            outcome,
            nodes: plan
                .nodes
                .iter()
                .map(|n| WorkflowNodeObservation {
                    node_id: n.id.clone(),
                    state: node_state,
                    terminal,
                    successful: outcome == Outcome::Success,
                    adapter: ExternalRef::new("lattice", "adapter", "local")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                    receipt: terminal.then(|| {
                        ExternalRef::new("lattice", "receipt", "node-rcpt")
                            .unwrap_or_else(|e| unreachable!("{e:?}"))
                    }),
                })
                .collect(),
            logs: Vec::new(),
            receipt: terminal.then(|| {
                ExternalRef::new("lattice", "receipt", "exec-rcpt")
                    .unwrap_or_else(|e| unreachable!("{e:?}"))
            }),
        }
    }
}

impl WorkflowPort for MultiPort {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }

    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut map = self.backends.lock();
        let entry = map.entry(self.project_id.clone()).or_default();
        entry
            .plans
            .insert(plan.effect_id.as_str().to_owned(), plan.clone());
        entry.sequence += 1;
        if entry.fail_unknown {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "provider_ambiguous".into(),
                message: "provider outcome ambiguous".into(),
            }));
        }
        Ok(self.observation_for(plan, entry))
    }

    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let map = self.backends.lock();
        let entry = map.get(&self.project_id).cloned().unwrap_or_default();
        if entry.fail_unknown {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "provider_ambiguous".into(),
                message: "provider outcome ambiguous".into(),
            }));
        }
        Ok(self.observation_for(plan, &entry))
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        let map = self.backends.lock();
        let entry = map.get(&self.project_id).cloned().unwrap_or_default();
        if entry.fail_unknown {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "provider_ambiguous".into(),
                message: "provider outcome ambiguous".into(),
            }));
        }
        let plan = present(entry.plans.get(effect_id.as_str()));
        Ok(self.observation_for(plan, &entry))
    }

    fn reconcile(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(effect_id)
    }

    fn control(
        &mut self,
        request: &kiln_core::workflow::WorkflowControlRequest,
    ) -> PortResult<WorkflowObservation> {
        self.observe(&request.effect_id)
    }

    fn read_log(
        &mut self,
        _: &kiln_core::workflow::WorkflowLogReadRequest,
    ) -> PortResult<kiln_core::workflow::WorkflowLogChunk> {
        Err(PortFailure::Domain(kiln_core::Diagnostic::new(
            "not_supported",
            "logs",
            "not supported",
        )))
    }
}

fn init_git_repo(dir: &Path) -> String {
    let repo = checked(git2::Repository::init(dir));
    checked(std::fs::write(dir.join("README.md"), b"rollup fixture\n"));
    let mut idx = checked(repo.index());
    checked(idx.add_path(Path::new("README.md")));
    checked(idx.write());
    let tree_oid = checked(idx.write_tree());
    let tree = checked(repo.find_tree(tree_oid));
    let sig = checked(git2::Signature::now("Fixture", "fixture@example.invalid"));
    let oid = checked(repo.commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[]));
    checked(repo.set_head("refs/heads/main"));
    oid.to_string()
}

fn project_profile(
    root: &Path,
    id: &str,
    repo_name: &str,
    socket_idx: usize,
) -> (ProjectProfile, String) {
    let repo_dir = root.join(format!("repo-{id}"));
    checked(std::fs::create_dir_all(&repo_dir));
    let sha = init_git_repo(&repo_dir);
    let project: ProjectProfile = checked(serde_json::from_value(json!({
        "id": id,
        "label": repo_name,
        "repository_path": repo_dir,
        "reference": "refs/heads/main",
        "source_limits": {
            "max_entries": 32,
            "max_path_bytes": 1024,
            "max_file_bytes": 4096,
            "max_total_bytes": 16384
        },
        "workflow": {
            "socket_path": root.join(format!("lattice-{socket_idx}.sock")),
            "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
            "transport_limits": {
                "maximum_frame_bytes": 1_048_576,
                "maximum_log_bytes": 65_536,
                "maximum_connections": 4,
                "read_deadline_milliseconds": 1000,
                "write_deadline_milliseconds": 1000
            },
            "adapter_limits": {
                "maximum_request_bytes": 1_048_576,
                "maximum_response_bytes": 1_048_576
            },
            "policy_id": "kiln-default",
            "provider_profile_id": "lattice-local",
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
    })));
    (project, sha)
}

fn make_service_profile(root: &Path, projects: Vec<ProjectProfile>) -> ServiceProfile {
    let cert_path = root.join("cert.pem");
    let key_path = root.join("key.pem");
    checked(std::fs::write(&cert_path, CERT_PEM));
    checked(std::fs::write(&key_path, KEY_PEM));
    checked(std::fs::set_permissions(
        &key_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o400),
    ));

    let state_dir = root.join("state");
    checked(std::fs::create_dir_all(&state_dir));
    checked(std::fs::set_permissions(
        &state_dir,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    ));

    let op_token_path = root.join("operator.token");
    let wk_token_path = root.join("worker.token");
    let vw_token_path = root.join("viewer.token");
    for (path, ch) in [
        (&op_token_path, "a"),
        (&wk_token_path, "b"),
        (&vw_token_path, "c"),
    ] {
        checked(std::fs::write(path, ch.repeat(64)));
        checked(std::fs::set_permissions(
            path,
            std::os::unix::fs::PermissionsExt::from_mode(0o400),
        ));
    }
    let access_path = root.join("access.json");
    checked(std::fs::write(
        &access_path,
        checked(serde_json::to_vec(&json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{
                "id": "reader",
                "label": "Fixture reader",
                "token_file": vw_token_path
            }]
        }))),
    ));

    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    drop(listener);

    checked(serde_json::from_value(json!({
        "schema": "kiln.web-service.v1",
        "service_id": "rollup-fixture",
        "listen_address": addr,
        "origin": format!("https://localhost:{}", addr.port()),
        "tls": {
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "min_version": "TLS1.3"
        },
        "operator_token_file": op_token_path,
        "worker_token_file": wk_token_path,
        "state_directory": state_dir,
        "projects": projects,
        "bounds": {
            "max_request_bytes": 16384,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 8,
            "timeout_seconds": 2,
            "poll_interval_ms": 10,
            "max_projects": 16,
            "max_runs": 64,
            "max_artifact_bytes": 4096,
            "max_artifacts": 8,
            "max_run_artifact_bytes": 16384,
            "max_total_artifact_bytes": 65536,
            "max_log_read_bytes": 4096,
            "max_requests": 128,
            "max_events_per_run": 8,
            "max_state_bytes": 1048576
        }
    })))
}

fn make_registry(
    targets: &[(&str, &str)],
    notifications: Vec<RunbookNotificationProfile>,
) -> RunbookRegistry {
    RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![RunbookDefinition {
            id: "audit-rollup".into(),
            display_name: "Audit Rollup".into(),
            description: "Read-only audit for roll-up tests".into(),
            mode: RunbookMode::ReadOnly,
            lattice: RunbookLattice {
                bundle_id: Some("audit-rollup".into()),
                bundle_identity: "b".repeat(64),
                workflow_name: "audit-rollup".into(),
                workflow_revision: "c".repeat(64),
                nodes: vec![kiln_adapter_github::profile::ConfigNode {
                    id: "job".into(),
                    kind: "test".into(),
                    effect_class: ConfigEffectClass::Test,
                    adapter_kind: "local".into(),
                    operation_identity: "a".repeat(64),
                }],
                edges: Vec::new(),
            },
            params: vec![ParamDeclaration {
                name: "target_tag".into(),
                param_type: ParamType::RelativePath { max_bytes: 64 },
                required: false,
                default: Some(ParamValue::Text("safe/default.txt".into())),
                bindings: vec![ParamBinding {
                    node: "job".into(),
                    shell_env: "LATTICE_PARAM_TAG".into(),
                }],
            }],
            allowed_targets: targets
                .iter()
                .map(|(repo, pid)| RunbookTarget {
                    repository: (*repo).to_owned(),
                    project_id: (*pid).to_owned(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: false,
                })
                .collect(),
            fanout: Some(RunbookFanoutConfig {
                enabled: true,
                default_max_concurrency: 4,
                max_concurrency_limit: 8,
                default_budget_cap: 3,
                max_budget_cap: 16,
                max_targets: 16,
                write_scopes: Vec::new(),
            }),
        }],
        grants: Vec::new(),
        notifications,
    }
}

fn open_engine(
    profile: ServiceProfile,
    backends: Arc<Mutex<BTreeMap<String, ProjectBackend>>>,
) -> Engine {
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    for project in &profile.projects {
        let cap = checked(project.execution_capabilities(profile.bounds.max_runs));
        ports.insert(
            project.id.clone(),
            Box::new(MultiPort {
                profile: cap,
                project_id: project.id.clone(),
                backends: Arc::clone(&backends),
            }),
        );
    }
    checked(Engine::open_with_ports(profile, ports))
}

fn operator() -> Principal {
    Principal {
        id: "local:operator".into(),
        label: "Operator".into(),
        role: Role::Operator,
        agent: None,
    }
}

fn post(engine: &mut Engine, path: &str, body: Value) -> ApiResponse {
    let bytes = checked(serde_json::to_vec(&body));
    checked(engine.api_as("POST", path, &bytes, &operator()))
}

fn get(engine: &mut Engine, path: &str, role: Role) -> ApiResponse {
    checked(engine.api_as(
        "GET",
        path,
        &[],
        &Principal {
            id: format!("system:{role:?}"),
            label: "Principal".into(),
            role,
            agent: None,
        },
    ))
}

fn json_body(resp: &ApiResponse) -> Value {
    checked(serde_json::from_slice(&resp.body))
}

struct ReportUpload<'a> {
    run_id: &'a str,
    source_revision: &'a str,
    attempt_id: &'a str,
    execution: &'a Value,
    kind: &'a str,
    name: &'a str,
    payload: Option<&'a [u8]>,
    unavailable: Option<&'a str>,
}

fn upload_report(engine: &mut Engine, upload: ReportUpload<'_>) -> ApiResponse {
    let (body_base64, blake3) = match upload.payload {
        Some(bytes) => (
            Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
            Some(blake3::hash(bytes).to_hex().to_string()),
        ),
        None => (None, None),
    };
    let req = json!({
        "run_id": upload.run_id,
        "source_revision": upload.source_revision,
        "attempt_id": upload.attempt_id,
        "execution": upload.execution,
        "node_id": "job",
        "kind": upload.kind,
        "name": upload.name,
        "body_base64": body_base64,
        "blake3": blake3,
        "unavailable": upload.unavailable,
    });
    let bytes = checked(serde_json::to_vec(&req));
    checked(engine.api(
        "POST",
        &format!("/api/runs/{}/reports", upload.run_id),
        &bytes,
        Role::Worker,
    ))
}

#[derive(Clone, Debug)]
struct CapturedWebhook {
    headers: BTreeMap<String, String>,
    body: Value,
}

struct WebhookReceiver {
    port: u16,
    captured: Arc<Mutex<Vec<CapturedWebhook>>>,
    stop: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<()>,
}

fn start_tls_webhook_receiver(status_line: &'static str) -> WebhookReceiver {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs: Vec<_> =
        checked(rustls_pemfile::certs(&mut std::io::Cursor::new(CERT_PEM)).collect());
    let key = present(checked(rustls_pemfile::private_key(
        &mut std::io::Cursor::new(KEY_PEM),
    )));
    let tls_cfg = Arc::new(checked(
        rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(certs, key),
    ));
    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    checked(listener.set_nonblocking(true));
    let port = checked(listener.local_addr()).port();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_ref = Arc::clone(&captured);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_ref = Arc::clone(&stop);

    let handle = std::thread::spawn(move || {
        while !stop_ref.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                    let Ok(conn) = rustls::ServerConnection::new(Arc::clone(&tls_cfg)) else {
                        continue;
                    };
                    let mut tls = rustls::StreamOwned::new(conn, stream);
                    let mut reader = BufReader::new(&mut tls);
                    let mut headers = BTreeMap::new();
                    let mut content_length = 0usize;
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        continue;
                    }
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty()
                        {
                            break;
                        }
                        if let Some((k, v)) = line.split_once(':') {
                            let key = k.trim().to_ascii_lowercase();
                            let val = v.trim().to_owned();
                            if key == "content-length" {
                                content_length = val.parse().unwrap_or(0);
                            }
                            headers.insert(key, val);
                        }
                    }
                    let mut body_bytes = vec![0u8; content_length];
                    if reader.read_exact(&mut body_bytes).is_ok()
                        && let Ok(body) = serde_json::from_slice::<Value>(&body_bytes)
                    {
                        captured_ref.lock().push(CapturedWebhook { headers, body });
                    }
                    let resp = format!(
                        "HTTP/1.1 {status_line}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                    );
                    let _ = tls.write_all(resp.as_bytes());
                    let _ = tls.flush();
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    WebhookReceiver {
        port,
        captured,
        stop,
        handle,
    }
}

#[test]
fn rollup_aggregates_dispatches_and_fanouts_with_unknown_receipts_and_prs() {
    let dir = checked(tempfile::tempdir());
    let (p1, sha1) = project_profile(dir.path(), "proj-a", "onixresearch/a", 1);
    let (p2, _sha2) = project_profile(dir.path(), "proj-b", "onixresearch/b", 2);
    let (p3, _sha3) = project_profile(dir.path(), "proj-c", "onixresearch/c", 3);
    let (p4, _sha4) = project_profile(dir.path(), "proj-d", "onixresearch/d", 4);

    let profile = make_service_profile(dir.path(), vec![p1, p2, p3, p4]);
    let backends = Arc::new(Mutex::new(BTreeMap::new()));
    let mut engine = open_engine(profile, Arc::clone(&backends));

    for (idx, pid) in ["proj-a", "proj-b", "proj-c", "proj-d"].iter().enumerate() {
        let resp = post(
            &mut engine,
            "/api/projects",
            json!({ "project_id": pid, "request_id": format!("enroll-{idx}") }),
        );
        assert_eq!(resp.status, 200);
    }

    let registry = make_registry(
        &[
            ("onixresearch/a", "proj-a"),
            ("onixresearch/b", "proj-b"),
            ("onixresearch/c", "proj-c"),
            ("onixresearch/d", "proj-d"),
        ],
        Vec::new(),
    );
    checked(engine.configure_runbooks(registry));

    // 1. Standalone dispatch on proj-a with Patchgate receipt, JUnit, Provenance PR/stack links (and invalid http URL filtered).
    let disp = post(
        &mut engine,
        "/api/runbooks/audit-rollup/dispatch",
        json!({
            "request_id": "disp-1",
            "target": "onixresearch/a@refs/heads/main",
            "params": { "target_tag": "custom/param-path" }
        }),
    );
    assert_eq!(disp.status, 201);
    let disp_val = json_body(&disp);
    let disp_run_id = present(disp_val.pointer("/run_id").and_then(Value::as_str)).to_owned();
    let binding = json_body(&get(
        &mut engine,
        &format!("/api/runs/{disp_run_id}/execution-binding"),
        Role::Operator,
    ));
    let attempt_id =
        present(binding.pointer("/plan/attempt_id").and_then(Value::as_str)).to_owned();
    let execution = present(binding.get("execution")).clone();

    // Upload Patchgate receipt (kiln.patchgate.review.v1)
    let pg_doc = json!({
        "schema": "kiln.patchgate.review.v1",
        "request_identity": "1".repeat(64),
        "observation": {
            "effect_id": "eff-pg-1",
            "candidate_id": "cand-1",
            "candidate_root": "2".repeat(64),
            "pipeline_revision": "rev-1",
            "provider_profile": "patchgate",
            "report_identity": "3".repeat(64),
            "isolated": true,
            "authorizes_acceptance": false,
            "reviews": [
                { "change": { "native_id": "c1" }, "decision": "accepted" },
                { "change": { "native_id": "c2" }, "decision": "unknown" }
            ]
        },
        "native_report": "raw patchgate output"
    });
    assert_eq!(
        upload_report(
            &mut engine,
            ReportUpload {
                run_id: &disp_run_id,
                source_revision: &sha1,
                attempt_id: &attempt_id,
                execution: &execution,
                kind: "review",
                name: "patchgate.json",
                payload: Some(&checked(serde_json::to_vec(&pg_doc))),
                unavailable: None,
            },
        )
        .status,
        201
    );

    // Upload JUnit verification report
    let junit = br#"<testsuite tests="2" failures="0" errors="0" skipped="0"><testcase classname="c" name="t1"/><testcase classname="c" name="t2"/></testsuite>"#;
    assert_eq!(
        upload_report(
            &mut engine,
            ReportUpload {
                run_id: &disp_run_id,
                source_revision: &sha1,
                attempt_id: &attempt_id,
                execution: &execution,
                kind: "junit",
                name: "junit.xml",
                payload: Some(junit),
                unavailable: None,
            },
        )
        .status,
        201
    );

    // Upload Provenance report with valid https:// PR + stack URLs AND invalid http:// / userinfo URLs
    let prov = json!({
        "pull_request_url": "https://github.com/onixresearch/a/pull/42",
        "pull_request_state": "open",
        "stack_url": "https://github.com/onixresearch/a/stacks/7",
        "stack_state": "draft",
        "stacks": [
            { "url": "http://insecure.example/stack/1", "state": "open" },
            { "url": "https://token:secret@github.com/onixresearch/a/pull/99", "state": "open" }
        ]
    });
    assert_eq!(
        upload_report(
            &mut engine,
            ReportUpload {
                run_id: &disp_run_id,
                source_revision: &sha1,
                attempt_id: &attempt_id,
                execution: &execution,
                kind: "provenance",
                name: "publication.json",
                payload: Some(&checked(serde_json::to_vec(&prov))),
                unavailable: None,
            },
        )
        .status,
        201
    );

    // Complete disp-1
    {
        let mut map = backends.lock();
        let b = present(map.get_mut("proj-a"));
        b.outcome = Some(Outcome::Success);
        b.sequence += 1;
    }
    checked(engine.poll());

    // RBAC checks
    assert_eq!(
        get(&mut engine, "/api/runbooks/dispatches", Role::Worker).status,
        403
    );
    assert_eq!(
        get(
            &mut engine,
            &format!("/api/runbooks/dispatches/{disp_run_id}/rollup"),
            Role::Worker
        )
        .status,
        403
    );
    let bad_body_resp = checked(engine.api_as(
        "GET",
        "/api/runbooks/dispatches",
        b"unexpected",
        &operator(),
    ));
    assert_eq!(bad_body_resp.status, 400);
    assert_eq!(
        json_body(&bad_body_resp)
            .pointer("/code")
            .and_then(Value::as_str),
        Some("unexpected_body")
    );

    let dispatches_page = json_body(&get(&mut engine, "/api/runbooks/dispatches", Role::Viewer));
    assert_eq!(
        dispatches_page.pointer("/schema").and_then(Value::as_str),
        Some(RUNBOOK_DISPATCHES_VIEW_SCHEMA)
    );
    assert_eq!(
        dispatches_page
            .pointer("/grants_authority")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        dispatches_page
            .pointer("/dispatches/0/patchgate_receipt_state")
            .and_then(Value::as_str),
        Some("present")
    );
    assert_eq!(
        dispatches_page
            .pointer("/dispatches/0/verification_receipt_state")
            .and_then(Value::as_str),
        Some("present")
    );
    assert_eq!(
        dispatches_page
            .pointer("/dispatches/0/pull_request_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let disp_rollup = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/dispatches/{disp_run_id}/rollup"),
        Role::Viewer,
    ));
    assert_eq!(
        disp_rollup.pointer("/schema").and_then(Value::as_str),
        Some(RUNBOOK_ROLLUP_SCHEMA)
    );
    assert_eq!(
        disp_rollup
            .pointer("/grants_authority")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        disp_rollup
            .pointer("/targets/0/patchgate_receipt/decision_counts/accepted")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        disp_rollup
            .pointer("/targets/0/patchgate_receipt/decision_counts/unknown")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        disp_rollup
            .pointer("/targets/0/patchgate_receipt/authorizes_acceptance")
            .and_then(Value::as_bool),
        Some(false)
    );
    let prs = present(
        disp_rollup
            .pointer("/targets/0/pull_requests")
            .and_then(Value::as_array),
    );
    assert_eq!(
        prs.len(),
        2,
        "invalid http:// and userinfo URLs must be excluded: {prs:?}"
    );

    // 2. Fan-out across 4 targets with budget_cap=3:
    //    proj-a -> Success (with unavailable Patchgate upload)
    //    proj-b -> DomainFailure (with non-Patchgate Review report -> Patchgate must be missing!)
    //    proj-c -> Unknown (fail_unknown)
    //    proj-d -> Skipped (budget exhausted)
    {
        let mut map = backends.lock();
        map.insert(
            "proj-a".into(),
            ProjectBackend {
                outcome: Some(Outcome::Pending),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-b".into(),
            ProjectBackend {
                outcome: Some(Outcome::Pending),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-c".into(),
            ProjectBackend {
                outcome: None,
                fail_unknown: true,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
    }
    let fo = post(
        &mut engine,
        "/api/runbooks/audit-rollup/fanout",
        json!({
            "request_id": "fo-mixed-1",
            "targets": ["proj-a", "proj-b", "proj-c", "proj-d"],
            "max_concurrency": 4,
            "budget_cap": 3
        }),
    );
    assert!(fo.status == 201 || fo.status == 202);
    let fo_val = json_body(&fo);
    let fanout_id = present(fo_val.pointer("/fanout_id").and_then(Value::as_str)).to_owned();
    let children = present(fo_val.pointer("/fanout/children").and_then(Value::as_array)).clone();
    let run_a = present(
        children
            .first()
            .and_then(|c| c.get("run_id"))
            .and_then(Value::as_str),
    )
    .to_owned();
    let run_b = present(
        children
            .get(1)
            .and_then(|c| c.get("run_id"))
            .and_then(Value::as_str),
    )
    .to_owned();

    // Upload unavailable Patchgate receipt on run_a
    let bind_a = json_body(&get(
        &mut engine,
        &format!("/api/runs/{run_a}/execution-binding"),
        Role::Operator,
    ));
    let att_a = present(bind_a.pointer("/plan/attempt_id").and_then(Value::as_str)).to_owned();
    let rev_a = present(
        bind_a
            .pointer("/binding/source/exact_revision/native_id")
            .and_then(Value::as_str),
    )
    .to_owned();
    let exec_a = present(bind_a.get("execution")).clone();
    assert_eq!(
        upload_report(
            &mut engine,
            ReportUpload {
                run_id: &run_a,
                source_revision: &rev_a,
                attempt_id: &att_a,
                execution: &exec_a,
                kind: "review",
                name: "patchgate.json",
                payload: None,
                unavailable: Some("missing"),
            },
        )
        .status,
        201
    );

    // Upload a non-Patchgate Review report on run_b (must NOT satisfy Patchgate receipt!)
    let bind_b = json_body(&get(
        &mut engine,
        &format!("/api/runs/{run_b}/execution-binding"),
        Role::Operator,
    ));
    let att_b = present(bind_b.pointer("/plan/attempt_id").and_then(Value::as_str)).to_owned();
    let rev_b = present(
        bind_b
            .pointer("/binding/source/exact_revision/native_id")
            .and_then(Value::as_str),
    )
    .to_owned();
    let exec_b = present(bind_b.get("execution")).clone();
    let other_review = json!({ "schema": "custom.review.v1", "summary": "ok" });
    assert_eq!(
        upload_report(
            &mut engine,
            ReportUpload {
                run_id: &run_b,
                source_revision: &rev_b,
                attempt_id: &att_b,
                execution: &exec_b,
                kind: "review",
                name: "other-review.json",
                payload: Some(&checked(serde_json::to_vec(&other_review))),
                unavailable: None,
            },
        )
        .status,
        201
    );

    // Advance outcomes: proj-a Success, proj-b DomainFailure (proj-c already Unknown)
    {
        let mut map = backends.lock();
        present(map.get_mut("proj-a")).outcome = Some(Outcome::Success);
        present(map.get_mut("proj-a")).sequence += 1;
        present(map.get_mut("proj-b")).outcome = Some(Outcome::DomainFailure);
        present(map.get_mut("proj-b")).sequence += 1;
    }
    checked(engine.poll());

    let rollup_page1 = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/rollup?limit=2"),
        Role::Viewer,
    ));
    assert_eq!(
        rollup_page1
            .pointer("/has_unknown")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(rollup_page1.pointer("/overall_outcome"), Some(&Value::Null));
    assert_eq!(
        rollup_page1
            .pointer("/counts/total")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/success")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/failure")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/unknown")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/skipped")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/patchgate_unavailable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/patchgate_missing")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rollup_page1
            .pointer("/counts/patchgate_unrun")
            .and_then(Value::as_u64),
        Some(1)
    );

    let page1_targets = present(rollup_page1.pointer("/targets").and_then(Value::as_array));
    assert_eq!(page1_targets.len(), 2);
    assert_eq!(
        page1_targets
            .first()
            .and_then(|t| t.pointer("/patchgate_receipt/state"))
            .and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        page1_targets
            .first()
            .and_then(|t| t.pointer("/patchgate_receipt/unavailable_reason"))
            .and_then(Value::as_str),
        Some("missing")
    );
    assert_eq!(
        page1_targets
            .get(1)
            .and_then(|t| t.pointer("/run_outcome"))
            .and_then(Value::as_str),
        Some("DomainFailure")
    );
    assert_eq!(
        page1_targets
            .get(1)
            .and_then(|t| t.pointer("/patchgate_receipt/state"))
            .and_then(Value::as_str),
        Some("missing")
    );

    let next_cursor = present(rollup_page1.pointer("/next_cursor").and_then(Value::as_str));
    let rollup_page2 = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/rollup?limit=2&cursor={next_cursor}"),
        Role::Viewer,
    ));
    let page2_targets = present(rollup_page2.pointer("/targets").and_then(Value::as_array));
    assert_eq!(page2_targets.len(), 2);
    assert_eq!(rollup_page2.pointer("/next_cursor"), Some(&Value::Null));
    // Child 2 = Unknown
    assert_eq!(
        page2_targets
            .first()
            .and_then(|t| t.pointer("/run_outcome"))
            .and_then(Value::as_str),
        Some("Unknown")
    );
    assert_eq!(
        page2_targets
            .first()
            .and_then(|t| t.pointer("/status"))
            .and_then(Value::as_str),
        Some("unknown")
    );
    // Child 3 = Skipped (unrun)
    assert_eq!(
        page2_targets
            .get(1)
            .and_then(|t| t.pointer("/status"))
            .and_then(Value::as_str),
        Some("skipped")
    );
    assert_eq!(
        page2_targets.get(1).and_then(|t| t.pointer("/run_outcome")),
        Some(&Value::Null)
    );
    assert_eq!(
        page2_targets
            .get(1)
            .and_then(|t| t.pointer("/patchgate_receipt/state"))
            .and_then(Value::as_str),
        Some("unrun")
    );
    assert_eq!(
        page2_targets
            .get(1)
            .and_then(|t| t.pointer("/verification_receipt/state"))
            .and_then(Value::as_str),
        Some("unrun")
    );

    // Tampered cursor refused
    assert_eq!(
        get(
            &mut engine,
            &format!("/api/runbooks/fanouts/{fanout_id}/rollup?cursor=not-valid-base64"),
            Role::Viewer,
        )
        .status,
        400
    );
    // Status filter
    let unk_only = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fanout_id}/rollup?status=unknown"),
        Role::Viewer,
    ));
    assert_eq!(unk_only.pointer("/total").and_then(Value::as_u64), Some(1));
}

#[test]
fn summary_notification_fires_once_per_completion_redacts_secrets_and_recovers_reserved_as_ambiguous()
 {
    let dir = checked(tempfile::tempdir());
    let (p1, _sha1) = project_profile(dir.path(), "proj-a", "onixresearch/a", 1);
    let profile = make_service_profile(dir.path(), vec![p1]);
    let ca_path = dir.path().join("ca.pem");
    let bearer_path = dir.path().join("webhook-secret");
    checked(std::fs::write(&ca_path, CA_PEM));
    checked(std::fs::write(
        &bearer_path,
        b"super-secret-webhook-token-value",
    ));
    checked(std::fs::set_permissions(
        &bearer_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    ));

    let WebhookReceiver {
        port,
        captured,
        stop,
        handle: receiver_thread,
    } = start_tls_webhook_receiver("202 Accepted");
    let notif = RunbookNotificationProfile {
        id: "summary-hook".into(),
        enabled: true,
        runbooks: vec!["audit-rollup".into()],
        on_success: true,
        on_failure: true,
        on_unknown: true,
        host: "localhost".into(),
        port,
        path: "/summary".into(),
        addresses: vec![
            "127.0.0.1"
                .parse()
                .unwrap_or_else(|e| unreachable!("{e:?}")),
        ],
        bearer_secret_file: Some(bearer_path.clone()),
        ca_certificate_file: Some(ca_path.clone()),
        allow_loopback: true,
        allow_private: false,
        timeout_ms: 5000,
        max_request_bytes: 65_536,
        max_response_bytes: 65_536,
        max_resolver_candidates: 4,
    };
    let registry = make_registry(&[("onixresearch/a", "proj-a")], vec![notif]);
    checked(registry.admit());

    let backends = Arc::new(Mutex::new(BTreeMap::new()));
    let mut engine = open_engine(profile.clone(), Arc::clone(&backends));
    assert_eq!(
        post(
            &mut engine,
            "/api/projects",
            json!({ "project_id": "proj-a", "request_id": "enroll-a" }),
        )
        .status,
        200
    );
    checked(engine.configure_runbooks(registry.clone()));

    // Dispatch with a secret-like param value that must NEVER leak into the summary notification payload.
    {
        let mut map = backends.lock();
        map.insert(
            "proj-a".into(),
            ProjectBackend {
                outcome: None,
                fail_unknown: true,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
    }
    let disp = post(
        &mut engine,
        "/api/runbooks/audit-rollup/dispatch",
        json!({
            "request_id": "disp-unk-1",
            "target": "onixresearch/a@refs/heads/main",
            "params": { "target_tag": "sensitive-param-should-not-leak" }
        }),
    );
    assert_eq!(disp.status, 201);
    let run_id = present(json_body(&disp).pointer("/run_id").and_then(Value::as_str)).to_owned();

    // First poll reserves and delivers summary notification for Outcome::Unknown
    let works = checked(engine.poll());
    assert_eq!(works.len(), 1);
    for w in works {
        let completion = checked(w.deliver());
        checked(engine.record_notification(completion));
    }

    // Subsequent polls must NOT resend!
    let works2 = checked(engine.poll());
    assert_eq!(works2.len(), 0);

    let hooks = captured.lock().clone();
    assert_eq!(hooks.len(), 1);
    let hook = present(hooks.first());
    assert_eq!(
        hook.headers.get("authorization").map(String::as_str),
        Some("Bearer super-secret-webhook-token-value")
    );
    assert!(hook.headers.contains_key("idempotency-key"));
    assert_eq!(
        hook.body.pointer("/schema").and_then(Value::as_str),
        Some(RUNBOOK_SUMMARY_NOTIFICATION_SCHEMA)
    );
    assert_eq!(
        hook.body.pointer("/has_unknown").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        hook.body
            .pointer("/grants_authority")
            .and_then(Value::as_bool),
        Some(false)
    );
    let body_text = checked(serde_json::to_string(&hook.body));
    assert!(!body_text.contains("sensitive-param-should-not-leak"));
    assert!(!body_text.contains("super-secret-webhook-token-value"));
    assert!(!body_text.contains("env_bindings"));
    assert!(!body_text.contains("source_env"));

    // Verify notification status shows delivered in rollup view
    let rollup = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/dispatches/{run_id}/rollup"),
        Role::Viewer,
    ));
    assert_eq!(
        rollup
            .pointer("/notification/state")
            .and_then(Value::as_str),
        Some("delivered")
    );

    // Now test crash-after-reservation recovery: start a fanout, poll to reserve, drop WITHOUT recording, restart engine!
    {
        let mut map = backends.lock();
        present(map.get_mut("proj-a")).fail_unknown = false;
        present(map.get_mut("proj-a")).outcome = Some(Outcome::Success);
    }
    let fo = post(
        &mut engine,
        "/api/runbooks/audit-rollup/fanout",
        json!({
            "request_id": "fo-crash-test",
            "targets": ["proj-a"],
            "max_concurrency": 1,
            "budget_cap": 1
        }),
    );
    assert!(fo.status == 201 || fo.status == 202);
    let fanout_id =
        present(json_body(&fo).pointer("/fanout_id").and_then(Value::as_str)).to_owned();
    let reserved_works = checked(engine.poll());
    assert_eq!(reserved_works.len(), 1);
    // Simulate crash by dropping reserved_works and engine WITHOUT calling record_notification!
    drop(reserved_works);
    drop(engine);

    // Re-open engine -> stale "reserved" record must transition to "ambiguous" and NEVER retry!
    let mut reopened = open_engine(profile, Arc::clone(&backends));
    checked(reopened.configure_runbooks(registry));
    let after_restart = checked(reopened.poll());
    assert_eq!(after_restart.len(), 0);

    let fo_rollup = json_body(&get(
        &mut reopened,
        &format!("/api/runbooks/fanouts/{fanout_id}/rollup"),
        Role::Viewer,
    ));
    assert_eq!(
        fo_rollup
            .pointer("/notification/state")
            .and_then(Value::as_str),
        Some("ambiguous")
    );
    assert_eq!(
        fo_rollup
            .pointer("/notification/error_code")
            .and_then(Value::as_str),
        Some("restart_after_reservation")
    );
    assert_eq!(
        captured.lock().len(),
        1,
        "crashed reservation must never be resent"
    );

    stop.store(true, Ordering::Release);
    checked(receiver_thread.join());
}

#[test]
fn served_rollup_enriches_with_read_only_implementation_work_outside_engine_lock() {
    let dir = checked(tempfile::tempdir());
    let (p1, sha1) = project_profile(dir.path(), "kiln", "onixresearch/kiln", 1);
    let service_profile = make_service_profile(dir.path(), vec![p1]);
    let impl_db = dir.path().join("implementation-work.sqlite");

    // Populate read-only ImplementationWork + ImplementationAttempt with verification + PublicationRecord PR!
    {
        let enrollment = Enrollment {
            repositories: BTreeSet::from(["onixresearch/kiln".to_owned()]),
            actors: BTreeSet::from(["operator".to_owned()]),
        };
        let mut implementer = checked(Implementer::open_with_local_operator(
            &impl_db, enrollment, "operator",
        ));
        let work = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.into(),
            revision: 1,
            actor: "operator".into(),
            repository: "onixresearch/kiln".into(),
            project: "kiln".into(),
            base: None,
            title: "change-1: task-1".into(),
            requested_outcome: "implement".into(),
            acceptance: vec![kiln_core::implementation::AcceptanceObligation {
                id: "a1".into(),
                statement: "verified".into(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::from(["write_source".to_owned()]),
            verification_profile: ContentId::from_blake3_hex("9".repeat(64))
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 30,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: checked(ExternalRef::new("cairn", "task", "change-1/task-1")),
                native_id: "change-1/task-1".into(),
                version: ContentId::from_blake3_hex("1".repeat(64))
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            }],
        };
        checked(implementer.admit(&work, None));
        let attempt = present(checked(implementer.claim_next(1, 60_000)));
        let candidate_ref = checked(ExternalRef::new("git", "commit", &sha1));
        let tree_ref = checked(ExternalRef::new("git", "tree", &"d".repeat(40)));
        let base_ref = checked(ExternalRef::new("git", "commit", &"e".repeat(40)));
        let handoff_id =
            ContentId::from_blake3_hex("2".repeat(64)).unwrap_or_else(|e| unreachable!("{e:?}"));
        let verification_id =
            ContentId::from_blake3_hex("3".repeat(64)).unwrap_or_else(|e| unreachable!("{e:?}"));
        let verification = CandidateVerification {
            schema: VERIFICATION_SCHEMA.into(),
            handoff: handoff_id.clone(),
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            candidate: candidate_ref.clone(),
            tree: tree_ref.clone(),
            base: Some(base_ref.clone()),
            verification_profile: work.verification_profile.clone(),
            run: kiln_core::change_review::CiFact {
                project: "kiln".into(),
                repository: "onixresearch/kiln".into(),
                revision: sha1.clone(),
                run_id: "verify-run-77".into(),
                ordinal: 1,
                success: true,
                full_run: true,
                clean: true,
                scope_identity: None,
                terminal: true,
                untrusted: None,
                checks: Vec::new(),
            },
        };
        let pub_rec = PublicationRecord {
            schema: kiln_core::implementation::PUBLICATION_SCHEMA.into(),
            plan: PublicationPlan {
                schema: "kiln.implementation-publication-plan.v1".into(),
                work_identity: attempt.work_identity.clone(),
                work_revision: attempt.work_revision,
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
                handoff: handoff_id,
                verification: verification_id,
                repository: "onixresearch/kiln".into(),
                base: base_ref,
                base_branch: "main".into(),
                candidate: candidate_ref,
                tree: tree_ref,
                branch: "feat/rollup".into(),
                title: "Rollup PR".into(),
                body: "Body".into(),
            },
            phase: PublicationPhase::PullRequestPublished,
            remote_head: None,
            pull_request: Some(checked(ExternalRef::new(
                "github",
                "pull_request",
                "onixresearch/kiln#100",
            ))),
            integration: None,
            source_status: Some(SourceStatusReceipt {
                schema: kiln_core::implementation::SOURCE_STATUS_SCHEMA.into(),
                work_identity: attempt.work_identity.clone(),
                work_revision: attempt.work_revision,
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
                integration: ContentId::from_blake3_hex("4".repeat(64))
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                source: work
                    .aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| unreachable!()),
                state: "open".into(),
                observed_revision: 1,
            }),
        };
        // Write directly into attempts ledger row for clean read-only observation test
        let conn = checked(rusqlite::Connection::open(&impl_db));
        let bytes: Vec<u8> = checked(conn.query_row(
            "SELECT bytes FROM implementation_state WHERE kind='attempts'",
            [],
            |r| r.get(0),
        ));
        let mut ledger: kiln_core::implementation::AttemptLedger =
            checked(serde_json::from_slice(&bytes));
        let mut attempts_val = checked(serde_json::to_value(&ledger));
        let att_obj = present(
            attempts_val
                .pointer_mut(&format!("/attempts/{}", attempt.identity.as_str()))
                .and_then(Value::as_object_mut),
        );
        att_obj.insert(
            "candidate".into(),
            checked(serde_json::to_value(checked(ExternalRef::new(
                "git", "commit", &sha1,
            )))),
        );
        att_obj.insert(
            "verification".into(),
            checked(serde_json::to_value(&verification)),
        );
        att_obj.insert(
            "verification_outcome".into(),
            checked(serde_json::to_value(VerificationOutcome::Unknown)),
        );
        att_obj.insert(
            "publication".into(),
            checked(serde_json::to_value(&pub_rec)),
        );
        ledger = checked(serde_json::from_value(attempts_val));
        checked(conn.execute(
            "UPDATE implementation_state SET bytes=?1 WHERE kind='attempts'",
            rusqlite::params![checked(serde_json::to_vec(&ledger))],
        ));
    }

    // Create dispatch run in durable state directly via Engine, then serve via real `kiln-web-service serve`!
    let backends = Arc::new(Mutex::new(BTreeMap::new()));
    {
        let mut map = backends.lock();
        map.insert(
            "kiln".into(),
            ProjectBackend {
                outcome: Some(Outcome::Success),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
    }
    let mut engine = open_engine(service_profile.clone(), Arc::clone(&backends));
    assert_eq!(
        post(
            &mut engine,
            "/api/projects",
            json!({ "project_id": "kiln", "request_id": "enroll-kiln" }),
        )
        .status,
        200
    );
    let registry = make_registry(&[("onixresearch/kiln", "kiln")], Vec::new());
    checked(engine.configure_runbooks(registry.clone()));
    let disp = post(
        &mut engine,
        "/api/runbooks/audit-rollup/dispatch",
        json!({
            "request_id": "disp-serve-1",
            "target": "onixresearch/kiln@refs/heads/main",
            "params": {}
        }),
    );
    assert_eq!(disp.status, 201);
    let run_id = present(json_body(&disp).pointer("/run_id").and_then(Value::as_str)).to_owned();
    drop(engine);

    let addr = service_profile.listen_address;
    let profile_path = dir.path().join("service.json");
    let access_path = dir.path().join("access.json");
    let runbooks_path = dir.path().join("runbooks.json");
    checked(std::fs::write(
        &profile_path,
        checked(serde_json::to_vec(&service_profile)),
    ));
    checked(std::fs::write(
        &runbooks_path,
        checked(serde_json::to_vec(&registry)),
    ));

    struct ServiceGuard(std::process::Child);
    impl Drop for ServiceGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let notify_path = dir.path().join("notify.sock");
    let receiver = checked(UnixDatagram::bind(&notify_path));
    checked(receiver.set_read_timeout(Some(Duration::from_secs(8))));
    let mut child = ServiceGuard(checked(
        std::process::Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "serve",
                "--profile",
                &profile_path.to_string_lossy(),
                "--access",
                &access_path.to_string_lossy(),
                "--runbooks",
                &runbooks_path.to_string_lossy(),
                "--implementation-work",
                &impl_db.to_string_lossy(),
            ])
            .env("NOTIFY_SOCKET", &notify_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn(),
    ));
    let mut notification = [0; 32];
    match receiver.recv(&mut notification) {
        Ok(n) => assert_eq!(notification.get(..n), Some(b"READY=1".as_slice())),
        Err(err) => {
            let exit = checked(child.0.try_wait());
            let mut stderr = String::new();
            if let Some(mut pipe) = child.0.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            unreachable!("serve readiness failed: {err}; exit={exit:?}; stderr={stderr}");
        }
    }

    let ca_cert = checked(reqwest::Certificate::from_pem(CA_PEM));
    let client = checked(
        reqwest::blocking::Client::builder()
            .add_root_certificate(ca_cert)
            .https_only(true)
            .no_proxy()
            .resolve("localhost", addr)
            .build(),
    );
    let resp = checked(
        client
            .get(format!(
                "https://localhost:{}/api/runbooks/dispatches/{run_id}/rollup",
                addr.port()
            ))
            .header("Authorization", format!("Bearer {}", "c".repeat(64)))
            .send(),
    );
    assert_eq!(resp.status().as_u16(), 200);
    let rollup: Value = checked(resp.json());
    drop(child);

    assert_eq!(
        rollup
            .pointer("/targets/0/verification_receipt/state")
            .and_then(Value::as_str),
        Some("present")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/verification_receipt/implementation_observation_status")
            .and_then(Value::as_str),
        Some("available")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/verification_receipt/implementation_verification/run_id")
            .and_then(Value::as_str),
        Some("verify-run-77")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/verification_receipt/outcome")
            .and_then(Value::as_str),
        Some("Unknown")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/pull_request_state")
            .and_then(Value::as_str),
        Some("observed")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/pull_requests/0/url")
            .and_then(Value::as_str),
        Some("https://github.com/onixresearch/kiln/pull/100")
    );
    assert_eq!(
        rollup
            .pointer("/targets/0/pull_requests/0/state")
            .and_then(Value::as_str),
        Some("open")
    );
}

#[test]
fn skipped_fanout_triggers_failure_notification_not_success_and_preserves_unknown() {
    let dir = checked(tempfile::tempdir());
    let (pa, _) = project_profile(dir.path(), "proj-a", "onixresearch/a", 1);
    let (pb, _) = project_profile(dir.path(), "proj-b", "onixresearch/b", 2);
    let (pc, _) = project_profile(dir.path(), "proj-c", "onixresearch/c", 3);
    let (pd, _) = project_profile(dir.path(), "proj-d", "onixresearch/d", 4);
    let (pe, _) = project_profile(dir.path(), "proj-e", "onixresearch/e", 5);
    let profile = make_service_profile(dir.path(), vec![pa, pb, pc, pd, pe]);
    let ca_path = dir.path().join("ca.pem");
    checked(std::fs::write(&ca_path, CA_PEM));

    let succ_rx = start_tls_webhook_receiver("202 Accepted");
    let fail_rx = start_tls_webhook_receiver("202 Accepted");
    let unk_rx = start_tls_webhook_receiver("202 Accepted");

    let mk_hook = |id: &str, port: u16, on_success: bool, on_failure: bool, on_unknown: bool| {
        RunbookNotificationProfile {
            id: id.into(),
            enabled: true,
            runbooks: vec!["audit-rollup".into()],
            on_success,
            on_failure,
            on_unknown,
            host: "localhost".into(),
            port,
            path: "/summary".into(),
            addresses: vec![
                "127.0.0.1"
                    .parse()
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            ],
            bearer_secret_file: None,
            ca_certificate_file: Some(ca_path.clone()),
            allow_loopback: true,
            allow_private: false,
            timeout_ms: 5000,
            max_request_bytes: 65_536,
            max_response_bytes: 65_536,
            max_resolver_candidates: 4,
        }
    };
    let registry = make_registry(
        &[
            ("onixresearch/a", "proj-a"),
            ("onixresearch/b", "proj-b"),
            ("onixresearch/c", "proj-c"),
            ("onixresearch/d", "proj-d"),
            ("onixresearch/e", "proj-e"),
        ],
        vec![
            mk_hook("on-success-only", succ_rx.port, true, false, false),
            mk_hook("on-failure-only", fail_rx.port, false, true, false),
            mk_hook("on-unknown-only", unk_rx.port, false, false, true),
        ],
    );
    checked(registry.admit());

    let backends = Arc::new(Mutex::new(BTreeMap::new()));
    {
        let mut map = backends.lock();
        map.insert(
            "proj-a".into(),
            ProjectBackend {
                outcome: Some(Outcome::Success),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-b".into(),
            ProjectBackend {
                outcome: Some(Outcome::Success),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-c".into(),
            ProjectBackend {
                outcome: None,
                fail_unknown: true,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-d".into(),
            ProjectBackend {
                outcome: None,
                fail_unknown: true,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
        map.insert(
            "proj-e".into(),
            ProjectBackend {
                outcome: Some(Outcome::Success),
                fail_unknown: false,
                sequence: 0,
                plans: BTreeMap::new(),
            },
        );
    }

    let mut engine = open_engine(profile, Arc::clone(&backends));
    for (pid, req) in [
        ("proj-a", "enroll-a"),
        ("proj-b", "enroll-b"),
        ("proj-c", "enroll-c"),
        ("proj-d", "enroll-d"),
        ("proj-e", "enroll-e"),
    ] {
        assert_eq!(
            post(
                &mut engine,
                "/api/projects",
                json!({ "project_id": pid, "request_id": req }),
            )
            .status,
            200
        );
    }
    checked(engine.configure_runbooks(registry));

    // Step 1: Success + Skipped (budget_cap=1 across 2 targets)
    // Must NOT trigger on-success-only; MUST trigger on-failure-only.
    let fo1 = post(
        &mut engine,
        "/api/runbooks/audit-rollup/fanout",
        json!({
            "request_id": "fo-skip-only-1",
            "targets": ["proj-a", "proj-b"],
            "max_concurrency": 1,
            "budget_cap": 1
        }),
    );
    assert!(fo1.status == 201 || fo1.status == 202);
    let fo1_id = present(
        json_body(&fo1)
            .pointer("/fanout_id")
            .and_then(Value::as_str),
    )
    .to_owned();

    let works1 = checked(engine.poll());
    assert_eq!(works1.len(), 1);
    for w in works1 {
        let completion = checked(w.deliver());
        checked(engine.record_notification(completion));
    }
    // Idempotency: subsequent poll must not resend
    assert_eq!(checked(engine.poll()).len(), 0);

    assert_eq!(succ_rx.captured.lock().len(), 0);
    assert_eq!(unk_rx.captured.lock().len(), 0);
    {
        let fail_hooks = fail_rx.captured.lock();
        assert_eq!(fail_hooks.len(), 1);
        let body = &present(fail_hooks.first()).body;
        assert_eq!(
            body.pointer("/counts/skipped").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.pointer("/counts/success").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.pointer("/counts/failure").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            body.pointer("/counts/unknown").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            body.pointer("/has_unknown").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            body.pointer("/phase").and_then(Value::as_str),
            Some("completed")
        );
    }

    let fo1_rollup = json_body(&get(
        &mut engine,
        &format!("/api/runbooks/fanouts/{fo1_id}/rollup"),
        Role::Viewer,
    ));
    assert_eq!(
        fo1_rollup
            .pointer("/notification/state")
            .and_then(Value::as_str),
        Some("delivered")
    );
    let fo1_deliveries = present(
        fo1_rollup
            .pointer("/notification/deliveries")
            .and_then(Value::as_array),
    );
    assert_eq!(fo1_deliveries.len(), 1);
    assert_eq!(
        fo1_deliveries
            .first()
            .and_then(|d| d.get("registration_id"))
            .and_then(Value::as_str),
        Some("on-failure-only")
    );

    // Step 2: Pure Unknown (1 target, fail_unknown=true, budget_cap=1)
    // Must ONLY trigger on-unknown-only (Unknown must NOT be coerced to failure).
    let fo2 = post(
        &mut engine,
        "/api/runbooks/audit-rollup/fanout",
        json!({
            "request_id": "fo-pure-unknown-2",
            "targets": ["proj-c"],
            "max_concurrency": 1,
            "budget_cap": 1
        }),
    );
    assert!(fo2.status == 201 || fo2.status == 202);
    let works2 = checked(engine.poll());
    assert_eq!(works2.len(), 1);
    for w in works2 {
        let completion = checked(w.deliver());
        checked(engine.record_notification(completion));
    }
    assert_eq!(succ_rx.captured.lock().len(), 0);
    assert_eq!(fail_rx.captured.lock().len(), 1);
    {
        let unk_hooks = unk_rx.captured.lock();
        assert_eq!(unk_hooks.len(), 1);
        let body = &present(unk_hooks.first()).body;
        assert_eq!(
            body.pointer("/has_unknown").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            body.pointer("/counts/unknown").and_then(Value::as_u64),
            Some(1)
        );
    }

    // Step 3: Mixed Unknown + Skipped (proj-d Unknown, proj-e Skipped, budget_cap=1)
    // Must trigger both on-failure-only (for skipped) and on-unknown-only (for unknown), never on-success-only.
    let fo3 = post(
        &mut engine,
        "/api/runbooks/audit-rollup/fanout",
        json!({
            "request_id": "fo-mixed-unk-skip-3",
            "targets": ["proj-d", "proj-e"],
            "max_concurrency": 1,
            "budget_cap": 1
        }),
    );
    assert!(fo3.status == 201 || fo3.status == 202);
    let fo3_val = json_body(&fo3);
    assert_eq!(
        fo3_val.pointer("/fanout/phase").and_then(Value::as_str),
        Some("completed"),
        "unexpected fo3: {fo3_val}"
    );
    assert_eq!(
        fo3_val
            .pointer("/fanout/counts/unknown")
            .and_then(Value::as_u64),
        Some(1),
        "unexpected fo3: {fo3_val}"
    );
    assert_eq!(
        fo3_val
            .pointer("/fanout/counts/skipped")
            .and_then(Value::as_u64),
        Some(1),
        "unexpected fo3: {fo3_val}"
    );
    let works3 = checked(engine.poll());
    assert_eq!(works3.len(), 2, "unexpected works3 len for fo3: {fo3_val}");
    for w in works3 {
        let completion = checked(w.deliver());
        checked(engine.record_notification(completion));
    }
    assert_eq!(succ_rx.captured.lock().len(), 0);
    assert_eq!(fail_rx.captured.lock().len(), 2);
    {
        let unk_hooks = unk_rx.captured.lock();
        assert_eq!(unk_hooks.len(), 2);
        let body = &present(unk_hooks.get(1)).body;
        assert_eq!(
            body.pointer("/has_unknown").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            body.pointer("/counts/unknown").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.pointer("/counts/skipped").and_then(Value::as_u64),
            Some(1)
        );
    }

    for rx in [succ_rx, fail_rx, unk_rx] {
        rx.stop.store(true, Ordering::Release);
        checked(rx.handle.join());
    }
}
