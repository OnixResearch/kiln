// r[verify kiln.github.local_e2e]
// r[verify kiln.github.local_e2e.nonclaims]
#![cfg(feature = "protocol-fixtures")]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use git2::{Repository, Signature};
use hmac::{Hmac, Mac};
use kiln_adapter_github::events::{DeliveryHeaders, EventPolicy};
use kiln_adapter_github::job::{JOB_SCHEMA_VERSION, JobProfile};
use kiln_adapter_github::profile::{
    AdapterLimits, ConfigEffectClass, ConfigNode, GITHUB_PROFILE_SCHEMA, GithubConfig,
    REQUIRED_NON_CLAIMS, WorkflowConfig, admit_config,
};
use kiln_adapter_github::secret::{ResolvedSecret, SecretReference};
use kiln_adapter_github::server::{open_store, serve_with};
use kiln_adapter_github::service::{GithubService, PublicationPhase, RunPhase, RunRecord};
use kiln_adapter_github::status::{
    Publication, StatusConfig, StatusPlan, StatusPort, StatusState, request_identity,
};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_core::domain::Outcome;
use kiln_core::source::GitSnapshotLimits;
use kiln_core::types::ContentId;
use kiln_core::workflow::{WorkflowBounds, WorkflowControls};
use kiln_provider_api::PortResult;
use lattice_workflow_contract::{
    ACCEPTED_SCHEMA, BLAKE3_PREFIX, ExecutionObservation, ExecutionState, Identity,
    NodeObservation, NodeState, OBSERVATION_SCHEMA, SourceContext, WorkflowAccepted,
    WorkflowRequest, observation_identity,
};
use lattice_workflow_transport::{
    ExchangeCommand, ExchangeSuccess, HandlerError, WorkflowExchangeHandler,
    WorkflowExchangeService, serve_listener,
};
use sha2::Sha256;
use tempfile::TempDir;

const WEBHOOK_SECRET: &[u8] = b"webhook-secret";
const STATUS_TOKEN: &str = "status-token";
const REPOSITORY_ID: u64 = 42;
const OWNER: &str = "octocat";
const REPOSITORY: &str = "hello-world";
const DELIVERY_BASE: u64 = 1;
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
const TEST_POLL: Duration = Duration::from_millis(20);
const LATTICE_CONNECTIONS: u32 = 64;
const TEST_OPERATION_IDENTITY: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PASS_COMMAND: &str = "IFS= read -r value < source.txt; test \"$value\" = 'selected exact commit' && test ! -e .git && test -z \"${GITHUB_TOKEN+x}\" && test -z \"${HOME+x}\"";
const BASIC_PASS_COMMAND: &str =
    "IFS= read -r value < source.txt; test \"$value\" = 'selected exact commit'";

#[derive(Clone, Debug)]
struct JobFixture {
    profile_path: PathBuf,
    identity: String,
    source_path: PathBuf,
}

#[derive(Debug)]
struct JobState {
    dispatches: usize,
    recoveries: usize,
    observations: usize,
    job_runs: usize,
    last_exit: Option<i32>,
    allow_terminal: bool,
    accepted: Option<WorkflowAccepted>,
}

impl Default for JobState {
    fn default() -> Self {
        Self {
            dispatches: 0,
            recoveries: 0,
            observations: 0,
            job_runs: 0,
            last_exit: None,
            allow_terminal: true,
            accepted: None,
        }
    }
}

#[derive(Debug)]
struct JobHandler {
    job: JobFixture,
    state: Arc<Mutex<JobState>>,
}

impl WorkflowExchangeHandler for JobHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        match command {
            ExchangeCommand::Dispatch { request, .. } => {
                let exit = self.run_job(request.source.as_ref())?;
                let accepted = accepted(request)?;
                let mut state = lock(&self.state);
                state.dispatches = state.dispatches.saturating_add(1);
                state.job_runs = state.job_runs.saturating_add(1);
                state.last_exit = Some(exit);
                state.accepted = Some(accepted.clone());
                Ok(ExchangeSuccess::Accepted(accepted))
            }
            ExchangeCommand::RecoverDispatch { request, .. } => {
                let mut state = lock(&self.state);
                state.recoveries = state.recoveries.saturating_add(1);
                let accepted = match state.accepted.clone() {
                    Some(accepted) => accepted,
                    None => accepted(request)?,
                };
                Ok(ExchangeSuccess::Accepted(accepted))
            }
            ExchangeCommand::Observe {
                request, accepted, ..
            } => {
                let mut state = lock(&self.state);
                state.observations = state.observations.saturating_add(1);
                if !state.allow_terminal {
                    return Err(HandlerError::Unavailable);
                }
                let successful = state.last_exit == Some(0);
                Ok(ExchangeSuccess::Observation(terminal_observation(
                    request, accepted, successful,
                )?))
            }
            ExchangeCommand::Reconcile {
                request, accepted, ..
            } => {
                let allow_terminal = lock(&self.state).allow_terminal;
                if !allow_terminal {
                    return Err(HandlerError::Unavailable);
                }
                let successful = lock(&self.state).last_exit == Some(0);
                Ok(ExchangeSuccess::Observation(terminal_observation(
                    request, accepted, successful,
                )?))
            }
            ExchangeCommand::Control { .. } | ExchangeCommand::ReadLog { .. } => {
                Err(HandlerError::Rejected)
            }
        }
    }
}

impl JobHandler {
    fn run_job(&self, source: Option<&SourceContext>) -> Result<i32, HandlerError> {
        let source = source.ok_or(HandlerError::Rejected)?;
        let output = Command::new(env!("CARGO_BIN_EXE_kiln-github-job"))
            .arg("--profile")
            .arg(&self.job.profile_path)
            .arg("--identity")
            .arg(&self.job.identity)
            .arg("--fixture-source")
            .arg(&self.job.source_path)
            .env("LATTICE_SOURCE_PROVIDER", source.provider.as_str())
            .env("LATTICE_SOURCE_REPOSITORY", source.repository.as_str())
            .env("LATTICE_SOURCE_REVISION", source.revision.as_str())
            .env("GITHUB_TOKEN", "must-not-reach-command")
            .output()
            .map_err(|_| HandlerError::Unavailable)?;
        Ok(output.status.code().unwrap_or(1))
    }
}

fn accepted(request: &WorkflowRequest) -> Result<WorkflowAccepted, HandlerError> {
    Ok(WorkflowAccepted {
        schema: ACCEPTED_SCHEMA.to_string(),
        request_identity: request.identity.clone(),
        idempotency_key: request.idempotency_key.clone(),
        workflow_identity: identity('8')?,
        execution_identity: identity('9')?,
    })
}

fn terminal_observation(
    request: &WorkflowRequest,
    accepted: &WorkflowAccepted,
    successful: bool,
) -> Result<ExecutionObservation, HandlerError> {
    let node = request.graph.nodes.first().ok_or(HandlerError::Rejected)?;
    let (state, node_state) = if successful {
        (ExecutionState::Completed, NodeState::Completed)
    } else {
        (ExecutionState::Failed, NodeState::Failed)
    };
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('1')?,
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: 2,
        state,
        terminal: true,
        successful,
        nodes: vec![NodeObservation {
            node_id: node.id.clone(),
            state: node_state,
            terminal: true,
            successful,
            adapter_identity: identity('a')?,
            receipt_identity: Some(identity('b')?),
        }],
        logs: Vec::new(),
        receipt_identity: Some(identity('c')?),
    };
    observation.identity = observation_identity(&observation);
    Ok(observation)
}

fn identity(digit: char) -> Result<Identity, HandlerError> {
    Identity::new(format!("{BLAKE3_PREFIX}{}", digit.to_string().repeat(64)))
        .map_err(|_| HandlerError::Rejected)
}

#[derive(Debug, Default)]
struct StatusRecorderState {
    plans: Vec<StatusPlan>,
    publications: Vec<Publication>,
}

#[derive(Clone, Debug)]
struct RecordingStatus(Arc<Mutex<StatusRecorderState>>);

impl RecordingStatus {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(StatusRecorderState::default())))
    }

    fn plans(&self) -> Vec<StatusPlan> {
        lock(&self.0).plans.clone()
    }

    fn publications(&self) -> Vec<Publication> {
        lock(&self.0).publications.clone()
    }
}

impl StatusPort for RecordingStatus {
    fn publish(&mut self, plan: &StatusPlan) -> PortResult<Publication> {
        let mut state = lock(&self.0);
        let status_id = u64::try_from(state.plans.len())
            .unwrap_or_else(|_| unreachable!())
            .saturating_add(1);
        let publication = Publication {
            status_id,
            request_identity: request_identity(plan)?,
        };
        state.plans.push(plan.clone());
        state.publications.push(publication.clone());
        Ok(publication)
    }

    fn reconcile(&mut self, plan: &StatusPlan) -> PortResult<Option<Publication>> {
        let state = lock(&self.0);
        Ok(state
            .plans
            .iter()
            .position(|written| written == plan)
            .and_then(|index| state.publications.get(index).cloned()))
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct RecordedStatus {
    id: u64,
    state: String,
    context: String,
    description: String,
    url: String,
}

#[derive(Debug, Default)]
struct StatusServerState {
    records: Vec<RecordedStatus>,
    gets: usize,
    drop_post_responses: bool,
    conflict_terminal: bool,
}

#[derive(Debug)]
struct StatusServer {
    address: SocketAddr,
    state: Arc<Mutex<StatusServerState>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl StatusServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|_| unreachable!("fixture status listener binds"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!("fixture status address is readable"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|_| unreachable!("fixture status listener is nonblocking"));
        let state = Arc::new(Mutex::new(StatusServerState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_state = Arc::clone(&state);
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => handle_status_connection(stream, &thread_state),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            state,
            stop,
            handle: Some(handle),
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    fn records(&self) -> Vec<RecordedStatus> {
        lock(&self.state).records.clone()
    }

    fn gets(&self) -> usize {
        lock(&self.state).gets
    }

    fn drop_post_responses(&self) {
        lock(&self.state).drop_post_responses = true;
    }

    fn conflict_terminal(&self) {
        lock(&self.state).conflict_terminal = true;
    }
}

impl Drop for StatusServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_status_connection(mut stream: TcpStream, state: &Mutex<StatusServerState>) {
    let _ = stream.set_read_timeout(Some(TEST_TIMEOUT));
    let _ = stream.set_write_timeout(Some(TEST_TIMEOUT));
    let Some((method, path, body)) = read_http_request(&mut stream) else {
        return;
    };
    if method == "POST" && path.starts_with("/repos/octocat/hello-world/statuses/") {
        let revision = path.rsplit('/').next().unwrap_or_default();
        let value: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => {
                respond_http(&mut stream, 400, "Bad Request", b"{}");
                return;
            }
        };
        let status_state = value
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let context = value
            .get("context")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let description = value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let (record, drop_response) = {
            let mut state = lock(state);
            let id = u64::try_from(state.records.len())
                .unwrap_or_else(|_| unreachable!())
                .saturating_add(1);
            let record = RecordedStatus {
                id,
                state: status_state,
                context,
                description,
                url: format!(
                    "https://api.github.com/repos/octocat/hello-world/statuses/{revision}"
                ),
            };
            state.records.push(record.clone());
            let drop_response = state.drop_post_responses;
            (record, drop_response)
        };
        if drop_response {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
        let body = serde_json::to_vec(&record)
            .unwrap_or_else(|_| unreachable!("fixture status response encodes"));
        respond_http(&mut stream, 201, "Created", &body);
    } else if method == "GET" && path.starts_with("/repos/octocat/hello-world/commits/") {
        let body = {
            let mut state = lock(state);
            state.gets = state.gets.saturating_add(1);
            let mut records = state.records.clone();
            if state.conflict_terminal {
                for record in &mut records {
                    if record.state == "success" {
                        record.state = "failure".to_string();
                    }
                }
            }
            serde_json::to_vec(&records)
                .unwrap_or_else(|_| unreachable!("fixture status page encodes"))
        };
        respond_http(&mut stream, 200, "OK", &body);
    } else {
        respond_http(&mut stream, 404, "Not Found", b"{}");
    }
}

fn read_http_request(stream: &mut TcpStream) -> Option<(String, String, Vec<u8>)> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 1024];
        let count = stream.read(&mut chunk).ok()?;
        if count == 0 {
            return None;
        }
        buffer.extend_from_slice(chunk.get(..count)?);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position.checked_add(4)?;
        }
        if buffer.len() > 65_536 {
            return None;
        }
    };
    let head = std::str::from_utf8(buffer.get(..header_end)?).ok()?;
    let mut lines = head.split("\r\n");
    let mut request = lines.next()?.split(' ');
    let method = request.next()?.to_string();
    let path = request.next()?.to_string();
    let mut content_length = 0_usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().ok()?;
        }
    }
    let body_end = header_end.checked_add(content_length)?;
    while buffer.len() < body_end {
        let mut chunk = [0_u8; 1024];
        let count = stream.read(&mut chunk).ok()?;
        if count == 0 {
            return None;
        }
        buffer.extend_from_slice(chunk.get(..count)?);
    }
    let body = buffer.get(header_end..body_end)?.to_vec();
    Some((method, path, body))
}

fn respond_http(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

struct Fixture {
    _root: TempDir,
    config: GithubConfig,
    status: RecordingStatus,
    state: Arc<Mutex<JobState>>,
    source_revision: String,
}

impl Fixture {
    fn new(command: &str, allow_terminal: bool) -> Self {
        let root = tempfile::tempdir()
            .unwrap_or_else(|_| unreachable!("fixture temporary directory is available"));
        let source_path = root.path().join("source.git");
        let repository = Repository::init_bare(&source_path)
            .unwrap_or_else(|_| unreachable!("fixture bare repository is created"));
        let selected = commit(&repository, b"selected exact commit\n", None);
        let _tip = commit(&repository, b"unselected branch tip\n", Some(selected));
        let revision = selected.to_string();

        let profile_path = root.path().join("job.json");
        let profile = JobProfile {
            schema_version: JOB_SCHEMA_VERSION,
            owner: OWNER.to_string(),
            repository: REPOSITORY.to_string(),
            program_path: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), command.to_string()],
            environment: BTreeMap::new(),
            source_secret: None,
            snapshot_limits: GitSnapshotLimits {
                max_entries: 64,
                max_path_bytes: 256,
                max_file_bytes: 4096,
                max_total_bytes: 16384,
            },
            maximum_transfer_bytes: 1_048_576,
            acquisition_timeout_ms: 5_000,
            command_timeout_ms: 5_000,
            maximum_output_bytes: 4096,
            teardown_grace_ms: 1_000,
        };
        let profile_bytes = serde_json::to_vec(&profile)
            .unwrap_or_else(|_| unreachable!("fixture job profile encodes"));
        std::fs::write(&profile_path, &profile_bytes)
            .unwrap_or_else(|_| unreachable!("fixture job profile is written"));
        let identity = blake3::hash(&profile_bytes).to_hex().to_string();

        let resolver = root.path().join("get");
        std::fs::write(
            &resolver,
            format!(
                "#!/bin/sh\ncase \"$0 $*\" in\n\
                 'get WEBHOOK_SECRET --profile ci --provider fixture --reason authenticate Kiln GitHub webhook deliveries') printf '{}\\n';;\n\
                 'get STATUS_TOKEN --profile ci --provider fixture --reason publish exact Kiln GitHub CI status') printf '{}\\n';;\n\
                 *) exit 1;;\nesac\n",
                String::from_utf8_lossy(WEBHOOK_SECRET),
                STATUS_TOKEN,
            ),
        )
        .unwrap_or_else(|_| unreachable!("fixture resolver script is written"));
        let resolver_bytes = std::fs::read("/bin/sh")
            .unwrap_or_else(|_| unreachable!("fixture shell interpreter is readable"));
        let secret = SecretReference {
            program_path: PathBuf::from("/bin/sh"),
            program_blake3: blake3::hash(&resolver_bytes).to_hex().to_string(),
            project_root: root.path().to_path_buf(),
            profile: "ci".to_string(),
            provider: Some("fixture".to_string()),
            secret_key: "WEBHOOK_SECRET".to_string(),
        };
        let status_secret = SecretReference {
            secret_key: "STATUS_TOKEN".to_string(),
            ..secret.clone()
        };
        let uid = std::fs::metadata(root.path())
            .unwrap_or_else(|_| unreachable!("fixture root metadata is readable"))
            .uid();
        let listen_address = free_loopback_address();

        let config = GithubConfig {
            schema: GITHUB_PROFILE_SCHEMA.to_string(),
            event_policy: EventPolicy {
                owner: OWNER.to_string(),
                repository: REPOSITORY.to_string(),
                repository_id: REPOSITORY_ID,
                allow_fork_pull_requests: false,
            },
            listen_address,
            webhook_path: "/github/webhook".to_string(),
            replay_database_path: root.path().join("github.sqlite"),
            maximum_run_records: 8,
            maximum_delivery_records: 8,
            maximum_record_bytes: 65_536,
            maximum_body_bytes: 65_536,
            maximum_header_bytes: 8_192,
            read_timeout_ms: 1_000,
            write_timeout_ms: 1_000,
            maximum_connections: 4,
            worker_poll_interval_ms: 10,
            maximum_workflow_polls: 16,
            maximum_active_runs: 1,
            webhook_secret: secret,
            status_secret,
            status: StatusConfig {
                owner: OWNER.to_string(),
                repository: REPOSITORY.to_string(),
                context: "kiln/ci".to_string(),
                maximum_response_bytes: 65_536,
                maximum_pages: 2,
                timeout_ms: 5_000,
            },
            workflow: WorkflowConfig {
                socket_path: root.path().join("lattice.sock"),
                allowed_user_ids: vec![uid],
                transport_limits: lattice_workflow_transport::TransportLimits {
                    maximum_frame_bytes: 65_536,
                    maximum_log_bytes: 4_096,
                    maximum_connections: LATTICE_CONNECTIONS,
                    read_deadline_milliseconds: 5_000,
                    write_deadline_milliseconds: 5_000,
                },
                adapter_limits: AdapterLimits {
                    maximum_request_bytes: 65_536,
                    maximum_response_bytes: 65_536,
                },
                policy_id: "github-ci-v1".to_string(),
                provider_profile_id: "lattice-v1".to_string(),
                platform: "linux-x86_64".to_string(),
                nodes: vec![ConfigNode {
                    id: "test".to_string(),
                    kind: "test".to_string(),
                    effect_class: ConfigEffectClass::Test,
                    adapter_kind: "local".to_string(),
                    operation_identity: TEST_OPERATION_IDENTITY.to_string(),
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
                    max_log_read_bytes: 1_024,
                    max_total_log_bytes: 4_096,
                },
            },
            non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        };

        let admitted = admit_config(config.clone())
            .unwrap_or_else(|_| unreachable!("fixture GitHub profile is admitted"));
        let transport = admitted.transport.clone();
        let socket_path = config.workflow.socket_path.clone();
        let job = JobFixture {
            profile_path,
            identity,
            source_path,
        };
        let state = Arc::new(Mutex::new(JobState {
            allow_terminal,
            ..JobState::default()
        }));
        let handler = JobHandler {
            job: job.clone(),
            state: Arc::clone(&state),
        };
        let listener = UnixListener::bind(&socket_path)
            .unwrap_or_else(|_| unreachable!("fixture Lattice socket binds"));
        std::thread::spawn(move || {
            let mut service = WorkflowExchangeService::new(handler);
            let _ = serve_listener(&listener, &transport, &mut service);
        });

        Self {
            _root: root,
            config,
            status: RecordingStatus::new(),
            state,
            source_revision: revision,
        }
    }

    fn start_service(&self, stop: Arc<AtomicBool>) -> JoinHandle<PortResult<()>> {
        let admitted = admit_config(self.config.clone())
            .unwrap_or_else(|_| unreachable!("fixture GitHub profile is re-admitted"));
        let transport = UnixWorkflowExchange::new(
            admitted.config.workflow.socket_path.clone(),
            admitted.transport.clone(),
            admitted.lattice.contract.clone(),
            admitted.config.maximum_run_records,
        )
        .unwrap_or_else(|_| unreachable!("fixture Lattice transport is admitted"));
        let workflow = LatticeAdapter::new(admitted.lattice.clone(), transport)
            .unwrap_or_else(|_| unreachable!("fixture Lattice adapter is admitted"));
        let store =
            open_store(&admitted).unwrap_or_else(|_| unreachable!("fixture SQLite store opens"));
        let service = GithubService::new(
            workflow,
            self.status.clone(),
            store,
            admitted.config.clone(),
        );
        let secret = ResolvedSecret::from_bytes(WEBHOOK_SECRET.to_vec())
            .unwrap_or_else(|_| unreachable!("fixture webhook secret is admitted"));
        std::thread::spawn(move || serve_with(admitted, secret, service, stop))
    }

    fn stop_service(&self, stop: Arc<AtomicBool>, handle: JoinHandle<PortResult<()>>) {
        stop.store(true, Ordering::Release);
        handle
            .join()
            .unwrap_or_else(|_| unreachable!("fixture service thread joins"))
            .unwrap_or_else(|_| unreachable!("fixture service stops cleanly"));
    }

    fn send(&self, delivery: u64) -> String {
        let body = body(&self.source_revision);
        let headers = headers(&body, delivery);
        http_request(self.config.listen_address, &body, &headers)
    }

    fn send_event(&self, event: &str, body: &[u8], delivery: u64) -> String {
        let headers = headers_for(event, body, delivery);
        http_request(self.config.listen_address, body, &headers)
    }

    fn send_invalid_signature(&self, delivery: u64) -> String {
        let body = body(&self.source_revision);
        let mut headers = headers(&body, delivery);
        headers.signature = format!("sha256={}", "0".repeat(64));
        http_request(self.config.listen_address, &body, &headers)
    }

    fn load(&self, run: &ContentId) -> Option<RunRecord> {
        let admitted = admit_config(self.config.clone()).ok()?;
        let store = open_store(&admitted).ok()?;
        store.load(run).ok().flatten()
    }

    fn wait_for_terminal(&self, run: &ContentId) -> RunRecord {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            if let Some(record) = self.load(run) {
                assert!(
                    record.phase != RunPhase::Unresolved,
                    "fixture run became unresolved"
                );
                if record.phase == RunPhase::Terminal && !record.has_work() {
                    return record;
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for a terminal fixture run"
            );
            std::thread::sleep(TEST_POLL);
        }
    }

    fn wait_for_unknown(&self, run: &ContentId) -> RunRecord {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            if let Some(record) = self.load(run) {
                if record.phase == RunPhase::Unknown {
                    return record;
                }
                assert!(
                    record.phase != RunPhase::Terminal,
                    "fixture run completed before restart recovery was exercised"
                );
                assert!(
                    record.phase != RunPhase::Unresolved,
                    "fixture run became unresolved"
                );
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for a nonterminal observation"
            );
            std::thread::sleep(TEST_POLL);
        }
    }

    fn job_state(&self) -> std::sync::MutexGuard<'_, JobState> {
        lock(&self.state)
    }

    fn allow_terminal(&self) {
        let mut state = lock(&self.state);
        state.allow_terminal = true;
    }

    fn profile_path(&self) -> PathBuf {
        self.profile_path_for("service.json", &self.config)
    }

    fn profile_path_for(&self, name: &str, config: &GithubConfig) -> PathBuf {
        let path = self._root.path().join(name);
        let bytes = serde_json::to_vec(config)
            .unwrap_or_else(|_| unreachable!("fixture service profile encodes"));
        std::fs::write(&path, bytes)
            .unwrap_or_else(|_| unreachable!("fixture service profile is written"));
        path
    }

    fn spawn_binary(&self, status_endpoint: &str) -> std::process::Child {
        self.spawn_binary_with_profile(&self.profile_path(), status_endpoint)
    }

    fn spawn_binary_with_profile(
        &self,
        profile_path: &Path,
        status_endpoint: &str,
    ) -> std::process::Child {
        Command::new(env!("CARGO_BIN_EXE_kiln-adapter-github"))
            .arg("--profile")
            .arg(profile_path)
            .env("KILN_GITHUB_FIXTURE_STATUS_ENDPOINT", status_endpoint)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap_or_else(|_| unreachable!("fixture GitHub binary starts"))
    }

    fn stop_binary(&self, child: &mut std::process::Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    fn run_identity(&self, response: &str) -> ContentId {
        let body = response_body(response);
        let value: serde_json::Value =
            serde_json::from_str(body).unwrap_or_else(|_| unreachable!("fixture response is JSON"));
        let identity = value
            .get("run_identity")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| unreachable!("fixture response names a run identity"));
        ContentId::from_blake3_hex(identity.to_string())
            .unwrap_or_else(|_| unreachable!("fixture run identity is canonical"))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|_| unreachable!("fixture lock is not poisoned"))
}

fn commit(repository: &Repository, content: &[u8], parent: Option<git2::Oid>) -> git2::Oid {
    let blob = repository
        .blob(content)
        .unwrap_or_else(|_| unreachable!("fixture blob is written"));
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|_| unreachable!("fixture tree builder is available"));
    builder
        .insert("source.txt", blob, 0o100_644)
        .unwrap_or_else(|_| unreachable!("fixture tree entry is inserted"));
    let tree_id = builder
        .write()
        .unwrap_or_else(|_| unreachable!("fixture tree is written"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|_| unreachable!("fixture tree is found"));
    let signature = Signature::now("fixture", "fixture@example.invalid")
        .unwrap_or_else(|_| unreachable!("fixture signature is created"));
    let parent = parent.map(|id| {
        repository
            .find_commit(id)
            .unwrap_or_else(|_| unreachable!("fixture parent commit is found"))
    });
    let parents: Vec<_> = parent.iter().collect();
    repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "fixture",
            &tree,
            &parents,
        )
        .unwrap_or_else(|_| unreachable!("fixture commit is written"))
}

fn free_loopback_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|_| unreachable!("fixture loopback port is available"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|_| unreachable!("fixture loopback address is readable"));
    drop(listener);
    address
}

fn body(revision: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "repository": {
            "id": REPOSITORY_ID,
            "name": "Hello-World",
            "full_name": "octocat/Hello-World",
            "owner": {"login": "octocat"}
        },
        "ref": "refs/heads/main",
        "after": revision,
        "deleted": false
    }))
    .unwrap_or_else(|_| unreachable!("fixture webhook body encodes"))
}

fn headers(bytes: &[u8], delivery: u64) -> DeliveryHeaders {
    headers_for("push", bytes, delivery)
}

fn headers_for(event: &str, bytes: &[u8], delivery: u64) -> DeliveryHeaders {
    let mut mac = Hmac::<Sha256>::new_from_slice(WEBHOOK_SECRET)
        .unwrap_or_else(|_| unreachable!("fixture HMAC key is accepted"));
    mac.update(bytes);
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    DeliveryHeaders {
        delivery: format!("00000000-0000-4000-8000-{delivery:012x}"),
        event: event.to_string(),
        signature: format!("sha256={signature}"),
    }
}

fn pull_request_body(head: &str, base: &str, fork: bool, action: &str, state: &str) -> Vec<u8> {
    let base_repository = serde_json::json!({
        "id": REPOSITORY_ID,
        "name": "Hello-World",
        "full_name": "octocat/Hello-World",
        "owner": {"login": "octocat"}
    });
    let head_repository = if fork {
        serde_json::json!({
            "id": 99,
            "name": "fork",
            "full_name": "other/fork",
            "owner": {"login": "other"}
        })
    } else {
        base_repository.clone()
    };
    serde_json::to_vec(&serde_json::json!({
        "action": action,
        "repository": base_repository.clone(),
        "number": 7,
        "pull_request": {
            "number": 7,
            "state": state,
            "head": {"sha": head, "ref": "feature", "repo": head_repository},
            "base": {"sha": base, "ref": "main", "repo": base_repository}
        }
    }))
    .unwrap_or_else(|_| unreachable!("fixture pull request body encodes"))
}

fn connect(address: SocketAddr) -> TcpStream {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        if let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_millis(50)) {
            return stream;
        }
        assert!(
            Instant::now() < deadline,
            "timed out connecting to the fixture webhook listener"
        );
        std::thread::sleep(TEST_POLL);
    }
}

fn raw_request(address: SocketAddr, bytes: &[u8]) -> String {
    let mut stream = connect(address);
    let _ = stream.set_read_timeout(Some(TEST_TIMEOUT));
    let _ = stream.set_write_timeout(Some(TEST_TIMEOUT));
    let _ = stream.write_all(bytes);
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

fn duplicate_header_request(revision: &str) -> Vec<u8> {
    let body = body(revision);
    let headers = headers(&body, DELIVERY_BASE);
    let mut request = format!(
        "POST /github/webhook HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-GitHub-Delivery: {}\r\nX-GitHub-Delivery: {}\r\nX-GitHub-Event: {}\r\nX-Hub-Signature-256: {}\r\n\r\n",
        body.len(),
        headers.delivery,
        headers.delivery,
        headers.event,
        headers.signature
    )
    .into_bytes();
    request.extend_from_slice(&body);
    request
}

fn oversized_body_request() -> Vec<u8> {
    let body = vec![b'0'; 65_537];
    let headers = headers(&body, DELIVERY_BASE);
    let mut request = format!(
        "POST /github/webhook HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-GitHub-Delivery: {}\r\nX-GitHub-Event: {}\r\nX-Hub-Signature-256: {}\r\n\r\n",
        body.len(),
        headers.delivery,
        headers.event,
        headers.signature
    )
    .into_bytes();
    request.extend_from_slice(&body);
    request
}

fn signed_request(bytes: &[u8], headers: &DeliveryHeaders) -> Vec<u8> {
    let mut request = format!(
        "POST /github/webhook HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-GitHub-Delivery: {}\r\nX-GitHub-Event: {}\r\nX-Hub-Signature-256: {}\r\n\r\n",
        bytes.len(),
        headers.delivery,
        headers.event,
        headers.signature
    )
    .into_bytes();
    request.extend_from_slice(bytes);
    request
}

fn http_request(address: SocketAddr, bytes: &[u8], headers: &DeliveryHeaders) -> String {
    let request = signed_request(bytes, headers);
    let mut stream = connect(address);
    stream
        .set_read_timeout(Some(TEST_TIMEOUT))
        .unwrap_or_else(|_| unreachable!("fixture webhook read timeout is set"));
    stream
        .set_write_timeout(Some(TEST_TIMEOUT))
        .unwrap_or_else(|_| unreachable!("fixture webhook write timeout is set"));
    stream
        .write_all(&request)
        .unwrap_or_else(|_| unreachable!("fixture webhook request is written"));
    stream
        .shutdown(std::net::Shutdown::Write)
        .unwrap_or_else(|_| unreachable!("fixture webhook write side is closed"));
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .unwrap_or_else(|_| unreachable!("fixture webhook response is read"));
    response
}

fn response_body(response: &str) -> &str {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_else(|| unreachable!("fixture webhook response has a body"))
}

fn response_duplicate(response: &str) -> bool {
    let value: serde_json::Value = serde_json::from_str(response_body(response))
        .unwrap_or_else(|_| unreachable!("fixture webhook response is JSON"));
    value
        .get("duplicate")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn status_states(record: &RunRecord) -> Vec<StatusState> {
    record
        .statuses
        .iter()
        .map(|status| status.plan.state)
        .collect()
}

// r[verify kiln.github.local_e2e.outcomes] r[verify kiln.github.source] r[verify kiln.github.workflow]
#[test]
fn passing_composition_executes_exact_source_and_publishes_success() {
    let fixture = Fixture::new(PASS_COMMAND, true);
    let stop = Arc::new(AtomicBool::new(false));
    let handle = fixture.start_service(Arc::clone(&stop));

    let response = fixture.send(DELIVERY_BASE);
    assert!(!response_duplicate(&response));
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);

    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Success]
    );
    assert_eq!(fixture.status.plans().len(), 2);
    assert_eq!(fixture.status.publications().len(), 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.recoveries, 0);
    assert_eq!(state.observations, 1);
    assert_eq!(state.job_runs, 1);
    assert_eq!(state.last_exit, Some(0));
    drop(state);

    fixture.stop_service(stop, handle);
}

// r[verify kiln.github.local_e2e.outcomes] r[verify kiln.github.source] r[verify kiln.github.workflow]
#[test]
fn failing_command_publishes_domain_failure() {
    let fixture = Fixture::new("exit 23", true);
    let stop = Arc::new(AtomicBool::new(false));
    let handle = fixture.start_service(Arc::clone(&stop));

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);

    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::DomainFailure
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Failure]
    );
    assert_eq!(fixture.status.plans().len(), 2);
    assert_eq!(fixture.status.publications().len(), 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.observations, 1);
    assert_eq!(state.job_runs, 1);
    assert_eq!(state.last_exit, Some(23));
    drop(state);

    fixture.stop_service(stop, handle);
}

// r[verify kiln.github.local_e2e.recovery] r[verify kiln.github.replay]
#[test]
fn duplicate_delivery_reuses_run_without_second_job() {
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let stop = Arc::new(AtomicBool::new(false));
    let handle = fixture.start_service(Arc::clone(&stop));

    let first = fixture.send(DELIVERY_BASE);
    let second = fixture.send(DELIVERY_BASE);
    assert!(!response_duplicate(&first));
    assert!(response_duplicate(&second));
    let first_run = fixture.run_identity(&first);
    let second_run = fixture.run_identity(&second);
    assert_eq!(first_run, second_run);

    let record = fixture.wait_for_terminal(&first_run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(fixture.status.plans().len(), 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    fixture.stop_service(stop, handle);
}

// r[verify kiln.github.local_e2e.recovery] r[verify kiln.github.replay] r[verify kiln.github.workflow]
#[test]
fn restart_recovers_original_execution_without_redispatch() {
    let fixture = Fixture::new(BASIC_PASS_COMMAND, false);
    let stop = Arc::new(AtomicBool::new(false));
    let handle = fixture.start_service(Arc::clone(&stop));

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_unknown(&run);
    assert_eq!(record.phase, RunPhase::Unknown);

    {
        let state = fixture.job_state();
        assert_eq!(state.dispatches, 1);
        assert_eq!(state.recoveries, 0);
        assert_eq!(state.job_runs, 1);
    }
    fixture.stop_service(stop, handle);

    fixture.allow_terminal();
    let stop = Arc::new(AtomicBool::new(false));
    let handle = fixture.start_service(Arc::clone(&stop));
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Success]
    );
    assert_eq!(fixture.status.plans().len(), 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.recoveries, 1);
    assert_eq!(state.job_runs, 1);
    assert!(state.observations >= 2);
    drop(state);

    fixture.stop_service(stop, handle);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.outcomes] r[verify kiln.github.local_e2e.process]
#[test]
fn real_binary_publishes_through_fixture_status() {
    let status = StatusServer::start();
    let fixture = Fixture::new(PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send(DELIVERY_BASE);
    assert!(!response_duplicate(&response));
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Success]
    );

    let posts = status.records();
    assert_eq!(posts.len(), 2);
    assert_eq!(
        posts.first().map(|post| post.state.as_str()),
        Some("pending")
    );
    assert_eq!(
        posts.get(1).map(|post| post.state.as_str()),
        Some("success")
    );
    assert!(posts.iter().all(|post| post.context == "kiln/ci"));
    assert!(posts.iter().all(|post| {
        post.url
            .starts_with("https://api.github.com/repos/octocat/hello-world/statuses/")
    }));
    assert!(
        posts
            .get(1)
            .is_some_and(|post| post.description.contains(run.as_str()))
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.recoveries, 0);
    assert_eq!(state.job_runs, 1);
    drop(state);

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.recovery] r[verify kiln.github.local_e2e.process]
#[test]
fn real_binary_restart_recovers_without_redispatch() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, false);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_unknown(&run);
    assert_eq!(record.phase, RunPhase::Unknown);
    {
        let state = fixture.job_state();
        assert_eq!(state.dispatches, 1);
        assert_eq!(state.recoveries, 0);
        assert_eq!(state.job_runs, 1);
    }
    fixture.stop_binary(&mut child);

    fixture.allow_terminal();
    let mut child = fixture.spawn_binary(&status.origin());
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Success]
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.recoveries, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    let posts = status.records();
    assert_eq!(posts.len(), 2);
    assert_eq!(
        posts.first().map(|post| post.state.as_str()),
        Some("pending")
    );
    assert_eq!(
        posts.get(1).map(|post| post.state.as_str()),
        Some("success")
    );

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.outcomes] r[verify kiln.github.local_e2e.process]
#[test]
fn real_binary_failing_command_publishes_failure() {
    let status = StatusServer::start();
    let fixture = Fixture::new("exit 23", true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::DomainFailure
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Failure]
    );

    let posts = status.records();
    assert_eq!(posts.len(), 2);
    assert_eq!(
        posts.first().map(|post| post.state.as_str()),
        Some("pending")
    );
    assert_eq!(
        posts.get(1).map(|post| post.state.as_str()),
        Some("failure")
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    assert_eq!(state.last_exit, Some(23));
    drop(state);

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.recovery] r[verify kiln.github.local_e2e.process]
#[test]
fn real_binary_duplicate_delivery_reuses_run() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let first = fixture.send(DELIVERY_BASE);
    let second = fixture.send(DELIVERY_BASE);
    assert!(!response_duplicate(&first));
    assert!(response_duplicate(&second));
    let first_run = fixture.run_identity(&first);
    let second_run = fixture.run_identity(&second);
    assert_eq!(first_run, second_run);

    let record = fixture.wait_for_terminal(&first_run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    assert_eq!(status.records().len(), 2);
    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.authentication]
#[test]
fn real_binary_rejects_invalid_signature_before_effects() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send_invalid_signature(DELIVERY_BASE);
    assert!(response.starts_with("HTTP/1.1 400"));
    let value: serde_json::Value = serde_json::from_str(response_body(&response))
        .unwrap_or_else(|_| unreachable!("fixture rejection response is JSON"));
    assert_eq!(
        value.get("accepted").and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 0);
    assert_eq!(state.job_runs, 0);
    drop(state);
    assert!(status.records().is_empty());

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.status_recovery] r[verify kiln.github.status]
#[test]
fn real_binary_reconciles_ambiguous_status_without_repost() {
    let status = StatusServer::start();
    status.drop_post_responses();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    assert_eq!(
        status_states(&record),
        vec![StatusState::Pending, StatusState::Success]
    );
    assert!(
        record
            .statuses
            .iter()
            .all(|status| status.phase == PublicationPhase::Published)
    );

    assert_eq!(status.records().len(), 2);
    assert!(status.gets() >= 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.framing] r[verify kiln.github.authentication] r[verify kiln.github.shell]
#[test]
fn real_binary_rejects_malformed_webhook_framing() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = raw_request(
        fixture.config.listen_address,
        &duplicate_header_request(&fixture.source_revision),
    );
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");

    let response = raw_request(fixture.config.listen_address, &oversized_body_request());
    assert!(response.starts_with("HTTP/1.1 413"), "{response}");

    let response = raw_request(
        fixture.config.listen_address,
        b"GET /github/webhook HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 405"), "{response}");

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 0);
    assert_eq!(state.job_runs, 0);
    drop(state);
    assert!(status.records().is_empty());

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.profile_drift] r[verify kiln.github.replay]
#[test]
fn real_binary_rejects_changed_profile_against_existing_replay_state() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());
    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    fixture.stop_binary(&mut child);

    let mut changed = fixture.config.clone();
    changed.worker_poll_interval_ms = changed.worker_poll_interval_ms.saturating_add(1);
    let changed_profile = fixture.profile_path_for("changed-service.json", &changed);
    let mut child = fixture.spawn_binary_with_profile(&changed_profile, &status.origin());
    let exit = child
        .wait()
        .unwrap_or_else(|_| unreachable!("changed-profile binary is reaped"));
    assert!(!exit.success());
    assert_eq!(exit.code(), Some(1));

    let record = fixture
        .load(&run)
        .unwrap_or_else(|| unreachable!("original replay record survives profile drift"));
    assert_eq!(record.phase, RunPhase::Terminal);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("original record still has an outcome")),
        Outcome::Success
    );
    assert!(
        record
            .statuses
            .iter()
            .all(|status| status.phase == PublicationPhase::Published)
    );
    assert_eq!(status.records().len(), 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.status_recovery] r[verify kiln.github.status]
#[test]
fn real_binary_keeps_terminal_status_unknown_on_conflict() {
    let status = StatusServer::start();
    status.drop_post_responses();
    status.conflict_terminal();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let response = fixture.send(DELIVERY_BASE);
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    let terminal = record
        .statuses
        .iter()
        .find(|status| status.plan.state == StatusState::Success)
        .unwrap_or_else(|| unreachable!("fixture terminal status exists"));
    assert_eq!(terminal.phase, PublicationPhase::Unknown);
    assert_eq!(
        terminal.error_code.as_deref(),
        Some("github_status_conflict")
    );

    assert_eq!(status.records().len(), 2);
    assert!(status.gets() >= 2);

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.events] r[verify kiln.github.events]
#[test]
fn real_binary_admits_same_repository_pull_request() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let body = pull_request_body(
        &fixture.source_revision,
        &"a".repeat(40),
        false,
        "opened",
        "open",
    );
    let response = fixture.send_event("pull_request", &body, DELIVERY_BASE);
    assert!(!response_duplicate(&response));
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );
    let source = record
        .plan
        .source
        .as_ref()
        .unwrap_or_else(|| unreachable!("pull request plan retains source context"));
    assert_eq!(source.revision.native_id.as_str(), fixture.source_revision);
    assert_eq!(source.repository.native_id.as_str(), "octocat/hello-world");

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.events] r[verify kiln.github.events]
#[test]
fn real_binary_denies_fork_pull_request() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let body = pull_request_body(
        &fixture.source_revision,
        &"a".repeat(40),
        true,
        "opened",
        "open",
    );
    let response = fixture.send_event("pull_request", &body, DELIVERY_BASE);
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
    let value: serde_json::Value = serde_json::from_str(response_body(&response))
        .unwrap_or_else(|_| unreachable!("fork rejection response is JSON"));
    assert_eq!(
        value.get("error").and_then(serde_json::Value::as_str),
        Some("github_fork_denied")
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 0);
    assert_eq!(state.job_runs, 0);
    drop(state);
    assert!(status.records().is_empty());

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.events] r[verify kiln.github.events]
#[test]
fn real_binary_ignores_closed_pull_request() {
    let status = StatusServer::start();
    let fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    let mut child = fixture.spawn_binary(&status.origin());

    let body = pull_request_body(
        &fixture.source_revision,
        &"a".repeat(40),
        false,
        "opened",
        "closed",
    );
    let response = fixture.send_event("pull_request", &body, DELIVERY_BASE);
    assert!(response.starts_with("HTTP/1.1 202"), "{response}");
    let value: serde_json::Value = serde_json::from_str(response_body(&response))
        .unwrap_or_else(|_| unreachable!("ignored event response is JSON"));
    assert_eq!(
        value.get("ignored").and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 0);
    assert_eq!(state.job_runs, 0);
    drop(state);
    assert!(status.records().is_empty());

    fixture.stop_binary(&mut child);
}

// r[verify kiln.github.local_e2e] r[verify kiln.github.local_e2e.process] r[verify kiln.github.local_e2e.shell_bounds] r[verify kiln.github.shell]
#[test]
fn real_binary_survives_partial_request_and_overload() {
    let status = StatusServer::start();
    let mut fixture = Fixture::new(BASIC_PASS_COMMAND, true);
    fixture.config.read_timeout_ms = 200;
    fixture.config.write_timeout_ms = 200;
    fixture.config.maximum_connections = 2;
    let mut child = fixture.spawn_binary(&status.origin());

    let mut stalled = Vec::new();
    for _ in 0..2 {
        let mut stream = connect(fixture.config.listen_address);
        stream
            .write_all(b"POST /github/webhook HTTP/1.1\r\nHost: localhost\r\n")
            .unwrap_or_else(|_| unreachable!("fixture partial request is written"));
        stalled.push(stream);
    }

    let body = body(&fixture.source_revision);
    let headers = headers(&body, DELIVERY_BASE);
    let response = raw_request(
        fixture.config.listen_address,
        &signed_request(&body, &headers),
    );
    assert!(response.starts_with("HTTP/1.1 503"), "{response}");

    let mut timed_out = stalled
        .pop()
        .unwrap_or_else(|| unreachable!("fixture stalled stream exists"));
    timed_out
        .set_read_timeout(Some(TEST_TIMEOUT))
        .unwrap_or_else(|_| unreachable!("fixture stalled read timeout is set"));
    let mut response = String::new();
    timed_out
        .read_to_string(&mut response)
        .unwrap_or_else(|_| unreachable!("fixture stalled response is read"));
    assert!(response.starts_with("HTTP/1.1 408"), "{response}");
    drop(timed_out);
    drop(stalled);
    std::thread::sleep(Duration::from_millis(100));

    let response = fixture.send(DELIVERY_BASE);
    assert!(!response_duplicate(&response));
    let run = fixture.run_identity(&response);
    let record = fixture.wait_for_terminal(&run);
    assert_eq!(
        record
            .outcome()
            .unwrap_or_else(|_| unreachable!("fixture terminal record has an outcome")),
        Outcome::Success
    );

    let state = fixture.job_state();
    assert_eq!(state.dispatches, 1);
    assert_eq!(state.job_runs, 1);
    drop(state);
    assert_eq!(status.records().len(), 2);

    fixture.stop_binary(&mut child);
}
