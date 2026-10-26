mod export;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{ContentId, EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{PortResult, WorkflowPort};
use sha2::Sha256;
use tempfile::TempDir;

use crate::events::{DeliveryHeaders, EventPolicy, VerifiedEvent, admit_event};
use crate::profile::*;
use crate::secret::{ResolvedSecret, SecretReference};
use crate::server::open_store;
use crate::service::{GithubService, RunPhase, plan_event};
use crate::status::*;

const SECRET: &[u8] = b"webhook-secret";

fn hash(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn config(root: &TempDir) -> GithubConfig {
    let secret = SecretReference {
        program_path: "/bin/sh".into(),
        program_blake3: "a".repeat(64),
        project_root: root.path().to_path_buf(),
        profile: "tests".to_string(),
        provider: None,
        secret_key: "WEBHOOK_SECRET".to_string(),
    };
    GithubConfig {
        schema: GITHUB_PROFILE_SCHEMA.to_string(),
        event_policy: EventPolicy {
            owner: "octocat".into(),
            repository: "hello-world".into(),
            repository_id: 1296269,
            allow_fork_pull_requests: false,
        },
        listen_address: "127.0.0.1:19091".parse().unwrap_or_else(|_| unreachable!()),
        webhook_path: "/github/webhook".into(),
        replay_database_path: root.path().join("github.sqlite"),
        maximum_run_records: 4,
        maximum_delivery_records: 8,
        maximum_record_bytes: 65536,
        maximum_body_bytes: 65536,
        maximum_header_bytes: 8192,
        read_timeout_ms: 500,
        write_timeout_ms: 500,
        maximum_connections: 2,
        worker_poll_interval_ms: 10,
        maximum_workflow_polls: 2,
        maximum_active_runs: 1,
        webhook_secret: secret.clone(),
        status_secret: secret,
        status: StatusConfig {
            owner: "octocat".into(),
            repository: "hello-world".into(),
            context: "kiln/ci".into(),
            maximum_response_bytes: 65536,
            maximum_pages: 2,
            timeout_ms: 500,
        },
        workflow: WorkflowConfig {
            socket_path: root.path().join("lattice.sock"),
            allowed_user_ids: vec![1000],
            transport_limits: lattice_workflow_transport::TransportLimits {
                maximum_frame_bytes: 65536,
                maximum_log_bytes: 4096,
                maximum_connections: 64,
                read_deadline_milliseconds: 500,
                write_deadline_milliseconds: 500,
            },
            adapter_limits: AdapterLimits {
                maximum_request_bytes: 65536,
                maximum_response_bytes: 65536,
            },
            policy_id: "github-ci-v1".into(),
            provider_profile_id: "lattice-v1".into(),
            platform: "linux-x86_64".into(),
            nodes: vec![ConfigNode {
                id: "test".into(),
                kind: "test".into(),
                effect_class: ConfigEffectClass::Test,
                adapter_kind: "local".into(),
                operation_identity: hash('b').as_str().into(),
            }],
            edges: Vec::new(),
            controls: WorkflowControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            bounds: WorkflowBounds {
                max_nodes: 4,
                max_edges: 4,
                max_log_references: 4,
                max_log_read_bytes: 1024,
                max_total_log_bytes: 4096,
            },
        },
        non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
    }
}

fn body(revision: char) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"repository":{"id":1296269,"name":"Hello-World","full_name":"octocat/Hello-World","owner":{"login":"octocat"}},"ref":"refs/heads/main","after":revision.to_string().repeat(40),"deleted":false})).unwrap_or_else(|_| unreachable!())
}

fn headers(bytes: &[u8], delivery: u64) -> DeliveryHeaders {
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).unwrap_or_else(|_| unreachable!());
    mac.update(bytes);
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    DeliveryHeaders {
        delivery: format!("00000000-0000-4000-8000-{delivery:012x}"),
        event: "push".into(),
        signature: format!("sha256={signature}"),
    }
}

fn event(admitted: &AdmittedGithubConfig, revision: char, delivery: u64) -> VerifiedEvent {
    let bytes = body(revision);
    match admit_event(
        &admitted.config.event_policy,
        &headers(&bytes, delivery),
        &bytes,
        SECRET,
        admitted.config.maximum_body_bytes,
    )
    .unwrap_or_else(|_| unreachable!())
    {
        Some(event) => event,
        _ => unreachable!(),
    }
}

#[derive(Debug, Default)]
struct Remote {
    dispatches: usize,
    recoveries: usize,
    plan: Option<WorkflowEffectPlan>,
}

#[derive(Debug)]
struct Workflow {
    profile: CapabilityProfile,
    remote: Arc<Mutex<Remote>>,
    ambiguous: bool,
    stale: bool,
    unavailable: bool,
}

fn observation(plan: &WorkflowEffectPlan, completed: bool, sequence: u64) -> WorkflowObservation {
    let reference = |kind: &str| {
        ExternalRef::new("lattice", kind, "fixture").unwrap_or_else(|_| unreachable!())
    };
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: reference("workflow"),
        execution: reference("execution"),
        sequence,
        state: if completed {
            WorkflowState::Completed
        } else {
            WorkflowState::Accepted
        },
        terminal: completed,
        successful: completed,
        outcome: if completed {
            Outcome::Success
        } else {
            Outcome::Pending
        },
        nodes: plan
            .nodes
            .iter()
            .map(|node| WorkflowNodeObservation {
                node_id: node.id.clone(),
                state: if completed {
                    WorkflowNodeState::Completed
                } else {
                    WorkflowNodeState::Pending
                },
                terminal: completed,
                successful: completed,
                adapter: reference("adapter"),
                receipt: completed.then(|| reference("receipt")),
            })
            .collect(),
        logs: Vec::new(),
        receipt: completed.then(|| reference("receipt")),
    }
}

impl WorkflowPort for Workflow {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut remote = self.remote.lock().unwrap_or_else(|_| unreachable!());
        remote.dispatches += 1;
        remote.plan = Some(plan.clone());
        if self.ambiguous {
            return Err(crate::infrastructure(
                "fixture_recovery_unavailable",
                "Immediate recovery failed after an accepted dispatch",
            ));
        }
        Ok(observation(plan, false, if self.stale { 10 } else { 0 }))
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut remote = self.remote.lock().unwrap_or_else(|_| unreachable!());
        remote.recoveries += 1;
        if self.unavailable || remote.plan.is_none() {
            return Err(crate::infrastructure(
                "fixture_recovery_unavailable",
                "Recovery is unavailable",
            ));
        }
        Ok(observation(plan, false, 0))
    }
    fn observe(&mut self, _: &EffectId) -> PortResult<WorkflowObservation> {
        let remote = self.remote.lock().unwrap_or_else(|_| unreachable!());
        let plan = remote.plan.as_ref().unwrap_or_else(|| unreachable!());
        Ok(observation(plan, true, if self.stale { 9 } else { 1 }))
    }
    fn reconcile(&mut self, effect: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(effect)
    }
    fn control(&mut self, _: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        Err(crate::domain(
            "fixture_control",
            "test",
            "Not selected by this fixture",
        ))
    }
    fn read_log(&mut self, _: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        Err(crate::domain(
            "fixture_log",
            "test",
            "Not selected by this fixture",
        ))
    }
}

#[derive(Debug, Default)]
struct Publisher {
    plans: Vec<StatusPlan>,
    lose_response: bool,
    posts: usize,
    reads: usize,
}
impl StatusPort for Publisher {
    fn publish(&mut self, plan: &StatusPlan) -> PortResult<Publication> {
        self.posts += 1;
        self.plans.push(plan.clone());
        if self.lose_response {
            return Err(kiln_provider_api::PortFailure::Unknown(
                kiln_provider_api::InfrastructureFailure {
                    code: "fixture_lost_response".into(),
                    message: "Status response was lost".into(),
                },
            ));
        }
        Ok(Publication {
            status_id: self.posts as u64,
            request_identity: request_identity(plan)?,
        })
    }
    fn reconcile(&mut self, plan: &StatusPlan) -> PortResult<Option<Publication>> {
        self.reads += 1;
        Ok(self
            .plans
            .iter()
            .position(|written| written == plan)
            .map(|index| Publication {
                status_id: (index + 1) as u64,
                request_identity: request_identity(plan).unwrap_or_else(|_| unreachable!()),
            }))
    }
}

fn service(
    admitted: &AdmittedGithubConfig,
    remote: Arc<Mutex<Remote>>,
    ambiguous: bool,
    stale: bool,
    unavailable: bool,
) -> GithubService<Workflow, Publisher> {
    let workflow = Workflow {
        profile: admitted.lattice.provider.clone(),
        remote,
        ambiguous,
        stale,
        unavailable,
    };
    GithubService::new(
        workflow,
        Publisher::default(),
        open_store(admitted).unwrap_or_else(|_| unreachable!()),
        admitted.config.clone(),
    )
}

#[test]
// r[verify kiln.github.replay]
fn durable_delivery_and_candidate_dedup_reject_conflict_and_stale_cas() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(config(&root)).unwrap_or_else(|_| unreachable!());
    let first = event(&admitted, 'a', 1);
    let record = plan_event(&admitted, &first).unwrap_or_else(|_| unreachable!());
    let run = record.run_identity.clone();
    let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
    assert!(
        store
            .reserve(&first, record.clone())
            .unwrap_or_else(|_| unreachable!())
            .inserted
    );
    assert!(
        !store
            .reserve(&first, record)
            .unwrap_or_else(|_| unreachable!())
            .inserted
    );
    let duplicate = event(&admitted, 'a', 2);
    assert!(
        !store
            .reserve(
                &duplicate,
                plan_event(&admitted, &duplicate).unwrap_or_else(|_| unreachable!())
            )
            .unwrap_or_else(|_| unreachable!())
            .inserted
    );
    let conflict = event(&admitted, 'b', 1);
    assert!(
        store
            .reserve(
                &conflict,
                plan_event(&admitted, &conflict).unwrap_or_else(|_| unreachable!())
            )
            .is_err()
    );
    let mut current = store
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let mut stale = current.clone();
    current.phase = RunPhase::DispatchReserved;
    store.save(&mut current).unwrap_or_else(|_| unreachable!());
    stale.phase = RunPhase::DispatchReserved;
    assert!(store.save(&mut stale).is_err());
    drop(store);
    let restored = open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(restored.phase, RunPhase::DispatchReserved);
    assert_eq!(
        restored.outcome().unwrap_or_else(|_| unreachable!()),
        Outcome::Pending
    );
}

#[test]
// r[verify kiln.github.replay]
fn retention_and_changed_profile_never_erase_prior_replay_state() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut cfg = config(&root);
    cfg.maximum_run_records = 1;
    let admitted = admit_config(cfg.clone()).unwrap_or_else(|_| unreachable!());
    let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
    let first = event(&admitted, 'a', 1);
    let record = plan_event(&admitted, &first).unwrap_or_else(|_| unreachable!());
    let run = record.run_identity.clone();
    store
        .reserve(&first, record)
        .unwrap_or_else(|_| unreachable!());
    let second = event(&admitted, 'b', 2);
    assert!(
        store
            .reserve(
                &second,
                plan_event(&admitted, &second).unwrap_or_else(|_| unreachable!())
            )
            .is_err()
    );
    assert_eq!(
        store
            .load(&run)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .phase,
        RunPhase::Queued
    );
    drop(store);
    cfg.status.context = "kiln/other".into();
    let changed = admit_config(cfg).unwrap_or_else(|_| unreachable!());
    assert!(open_store(&changed).is_err());
}

#[test]
// r[verify kiln.github.workflow]
fn accepted_dispatch_with_failed_recovery_stays_unknown_and_recovers_without_redispatch() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(config(&root)).unwrap_or_else(|_| unreachable!());
    let event = event(&admitted, 'a', 1);
    let record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
    let run = record.run_identity.clone();
    open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .reserve(&event, record)
        .unwrap_or_else(|_| unreachable!());
    let remote = Arc::new(Mutex::new(Remote::default()));
    let mut first = service(&admitted, Arc::clone(&remote), true, false, false);
    first.tick().unwrap_or_else(|_| unreachable!());
    let unknown = open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        unknown.outcome().unwrap_or_else(|_| unreachable!()),
        Outcome::Unknown
    );
    assert_eq!(unknown.phase, RunPhase::Unknown);
    drop(first);
    let mut restarted = service(&admitted, Arc::clone(&remote), false, false, false);
    restarted.tick().unwrap_or_else(|_| unreachable!());
    let completed = open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        completed.outcome().unwrap_or_else(|_| unreachable!()),
        Outcome::Success
    );
    assert_eq!(completed.phase, RunPhase::Terminal);
    let remote = remote.lock().unwrap_or_else(|_| unreachable!());
    assert_eq!(remote.dispatches, 1);
    assert_eq!(remote.recoveries, 1);
}

#[test]
// r[verify kiln.github.status]
fn terminal_intent_does_not_hide_an_ambiguous_pending_status() {
    for lose_response in [false, true] {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let admitted = admit_config(config(&root)).unwrap_or_else(|_| unreachable!());
        let event = event(&admitted, 'a', 1);
        let record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
        let run = record.run_identity.clone();
        let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
        store
            .reserve(&event, record)
            .unwrap_or_else(|_| unreachable!());
        let remote = Arc::new(Mutex::new(Remote::default()));
        let workflow = Workflow {
            profile: admitted.lattice.provider.clone(),
            remote: Arc::clone(&remote),
            ambiguous: false,
            stale: false,
            unavailable: false,
        };
        let publisher = Publisher {
            lose_response,
            ..Publisher::default()
        };
        let mut service = GithubService::new(
            workflow,
            publisher,
            open_store(&admitted).unwrap_or_else(|_| unreachable!()),
            admitted.config.clone(),
        );
        for _ in 0..4 {
            service.tick().unwrap_or_else(|_| unreachable!());
        }
        let record = store
            .load(&run)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            record.outcome().unwrap_or_else(|_| unreachable!()),
            Outcome::Success
        );
        assert_eq!(
            record
                .statuses
                .iter()
                .map(|status| status.plan.state)
                .collect::<Vec<_>>(),
            vec![StatusState::Pending, StatusState::Success]
        );
        assert_eq!(
            record
                .statuses
                .iter()
                .map(|status| status.phase)
                .collect::<Vec<_>>(),
            vec![crate::service::PublicationPhase::Published; 2]
        );
        assert_eq!(
            record
                .statuses
                .iter()
                .map(|status| status
                    .publication
                    .as_ref()
                    .map(|publication| publication.status_id))
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert!(!record.has_work());
        assert!(!service.tick().unwrap_or_else(|_| unreachable!()));
        assert_eq!(
            remote.lock().unwrap_or_else(|_| unreachable!()).dispatches,
            1
        );
    }
}

#[test]
// r[verify kiln.github.workflow]
fn stale_observations_consume_durable_budget_and_never_publish_success() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(config(&root)).unwrap_or_else(|_| unreachable!());
    let event = event(&admitted, 'a', 1);
    let record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
    let run = record.run_identity.clone();
    open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .reserve(&event, record)
        .unwrap_or_else(|_| unreachable!());
    let remote = Arc::new(Mutex::new(Remote::default()));
    let mut service = service(&admitted, remote, false, true, false);
    service.tick().unwrap_or_else(|_| unreachable!());
    assert!(service.tick().is_err());
    assert!(service.tick().is_err());
    service.tick().unwrap_or_else(|_| unreachable!());
    let record = open_store(&admitted)
        .unwrap_or_else(|_| unreachable!())
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(record.phase, RunPhase::Unresolved);
    assert_eq!(record.workflow_polls, 2);
    assert_eq!(
        record.outcome().unwrap_or_else(|_| unreachable!()),
        Outcome::Unknown
    );
    assert_eq!(
        record
            .observation
            .as_ref()
            .map(|observation| observation.sequence),
        Some(10)
    );
    assert!(
        record
            .statuses
            .iter()
            .all(|status| status.plan.state != StatusState::Success)
    );
}

#[test]
// r[verify kiln.github.workflow]
fn interrupted_reservation_never_becomes_permission_to_dispatch() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(config(&root)).unwrap_or_else(|_| unreachable!());
    let event = event(&admitted, 'a', 1);
    let record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
    let run = record.run_identity.clone();
    let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
    store
        .reserve(&event, record)
        .unwrap_or_else(|_| unreachable!());
    let mut record = store
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    record.phase = RunPhase::DispatchReserved;
    store.save(&mut record).unwrap_or_else(|_| unreachable!());
    let remote = Arc::new(Mutex::new(Remote::default()));
    let mut service = service(&admitted, Arc::clone(&remote), false, false, true);
    for _ in 0..3 {
        service.tick().unwrap_or_else(|_| unreachable!());
    }
    let record = store
        .load(&run)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(record.phase, RunPhase::Unresolved);
    assert_eq!(
        record.outcome().unwrap_or_else(|_| unreachable!()),
        Outcome::Unknown
    );
    assert_eq!(
        remote.lock().unwrap_or_else(|_| unreachable!()).dispatches,
        0
    );
}

fn request(address: std::net::SocketAddr, bytes: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).unwrap_or_else(|_| unreachable!());
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap_or_else(|_| unreachable!());
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap_or_else(|_| unreachable!());
    stream.write_all(bytes).unwrap_or_else(|_| unreachable!());
    stream
        .shutdown(std::net::Shutdown::Write)
        .unwrap_or_else(|_| unreachable!());
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .unwrap_or_else(|_| unreachable!());
    response
}

fn http_event(delivery: u64, duplicate_header: bool) -> Vec<u8> {
    let bytes = body('a');
    let headers = headers(&bytes, delivery);
    let duplicate = if duplicate_header {
        "X-GitHub-Event: push\r\n"
    } else {
        ""
    };
    let mut request = format!("POST /github/webhook HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-GitHub-Delivery: {}\r\nX-GitHub-Event: {}\r\nX-Hub-Signature-256: {}\r\n{duplicate}\r\n", bytes.len(), headers.delivery, headers.event, headers.signature).into_bytes();
    request.extend_from_slice(&bytes);
    request
}

#[test]
// r[verify kiln.github.shell]
// r[verify kiln.github.authentication]
fn http_daemon_survives_more_than_concurrency_limit_and_rejects_ambiguous_headers() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
    let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
    let mut cfg = config(&root);
    cfg.listen_address = address;
    let admitted = admit_config(cfg.clone()).unwrap_or_else(|_| unreachable!());
    let remote = Arc::new(Mutex::new(Remote::default()));
    let service = service(&admitted, Arc::clone(&remote), false, false, false);
    let stop = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::clone(&stop);
    let server = std::thread::spawn(move || {
        crate::server::serve_on(
            admitted,
            ResolvedSecret::from_bytes(SECRET.to_vec()).unwrap_or_else(|_| unreachable!()),
            service,
            shutdown,
            listener,
        )
    });
    for _ in 0..6 {
        assert!(
            request(
                address,
                b"GET /github/webhook HTTP/1.1\r\nHost: localhost\r\n\r\n"
            )
            .starts_with("HTTP/1.1 405")
        );
    }
    assert!(request(address, &http_event(1, true)).starts_with("HTTP/1.1 400"));
    let accepted = request(address, &http_event(1, false));
    assert!(accepted.starts_with("HTTP/1.1 202"));
    let duplicate = request(address, &http_event(2, false));
    assert!(duplicate.starts_with("HTTP/1.1 202"));
    assert!(duplicate.contains("\"duplicate\":true"));
    let accepted_json = accepted
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_else(|| unreachable!());
    let accepted_json: serde_json::Value =
        serde_json::from_str(accepted_json).unwrap_or_else(|_| unreachable!());
    let run = accepted_json
        .get("run_identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| unreachable!());
    let run = ContentId::from_blake3_hex(run.to_string()).unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(cfg).unwrap_or_else(|_| unreachable!());
    assert!(
        open_store(&admitted)
            .unwrap_or_else(|_| unreachable!())
            .load(&run)
            .unwrap_or_else(|_| unreachable!())
            .is_some()
    );
    stop.store(true, Ordering::Release);
    server
        .join()
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        remote.lock().unwrap_or_else(|_| unreachable!()).dispatches,
        1
    );
}

#[test]
// r[verify kiln.github.shell]
fn incomplete_connection_does_not_block_another_webhook_and_overload_does_not_stop_daemon() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
    let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
    let mut cfg = config(&root);
    cfg.listen_address = address;
    cfg.read_timeout_ms = 5000;
    let admitted = admit_config(cfg).unwrap_or_else(|_| unreachable!());
    let service = service(
        &admitted,
        Arc::new(Mutex::new(Remote::default())),
        false,
        false,
        false,
    );
    let stop = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::clone(&stop);
    let server = std::thread::spawn(move || {
        crate::server::serve_on(
            admitted,
            ResolvedSecret::from_bytes(SECRET.to_vec()).unwrap_or_else(|_| unreachable!()),
            service,
            shutdown,
            listener,
        )
    });
    let mut slow = TcpStream::connect(address).unwrap_or_else(|_| unreachable!());
    slow.write_all(b"POST /github/webhook HTTP/1.1\r\n")
        .unwrap_or_else(|_| unreachable!());
    let before = Instant::now();
    assert!(request(address, &http_event(1, false)).starts_with("HTTP/1.1 202"));
    assert!(before.elapsed() < Duration::from_secs(2));
    drop(slow);
    stop.store(true, Ordering::Release);
    server
        .join()
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|_| unreachable!());
}

#[test]
// r[verify kiln.github.profile]
fn unsupported_graph_controls_and_unbounded_acknowledgement_fail_before_resources() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let cfg = config(&root);
    let mut invalid = cfg.clone();
    invalid.workflow.controls.pause = true;
    assert!(admit_config(invalid).is_err());
    let mut invalid = cfg.clone();
    invalid
        .workflow
        .nodes
        .get_mut(0)
        .unwrap_or_else(|| unreachable!())
        .operation_identity = "not-a-content-identity".into();
    assert!(admit_config(invalid).is_err());
    let mut invalid = cfg.clone();
    invalid.read_timeout_ms = MAXIMUM_ACKNOWLEDGEMENT_MS;
    assert!(admit_config(invalid).is_err());
    let mut invalid = cfg;
    invalid.maximum_connections = MAXIMUM_HTTP_CONNECTIONS + 1;
    assert!(admit_config(invalid).is_err());
    assert!(!root.path().join("github.sqlite").exists());
}
