#![cfg(target_os = "linux")]

use std::collections::BTreeSet;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use iroh::{Endpoint, EndpointAddr, SecretKey};
use kiln_adapter_iroh::{
    ALPN, IROH_VERSION, IrohClient, PROFILE_SCHEMA, Profile, bind_endpoint, serve_proxy,
    serve_worker,
};
use kiln_adapter_lattice::UnixWorkflowExchange;
use kiln_provider_api::{
    PortFailure, WorkflowExchangeOperation, WorkflowExchangePort, WorkflowExchangeRequest,
};
use lattice_workflow_contract::{
    CapabilityProfile, ControlAction, ControlRequest, ExecutionObservation, ExecutionState,
    LogReadRequest, LogStream, NodeState, WorkflowAccepted, WorkflowRequest, observation_identity,
};
use lattice_workflow_transport::{
    ExchangeCommand, ExchangeRequest, ExchangeResponse, ExchangeResult, ExchangeSuccess,
    HandlerError, PeerPolicy, TransportLimits, TransportProfile, WorkflowExchangeHandler,
    WorkflowExchangeService, encode_frame, new_request,
};
use parking_lot::Mutex;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const BOUND: Duration = Duration::from_secs(5);
const LOG: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

#[derive(Deserialize)]
struct Producer {
    profile: CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
    observation: ExecutionObservation,
}

fn producer() -> Producer {
    serde_json::from_str(include_str!("../../../fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/producer-fixtures-v1.json")).unwrap_or_else(|_| unreachable!())
}

#[derive(Deserialize)]
struct Recovery {
    request: ExchangeRequest,
    accepted_response: ExchangeResponse,
    not_found_response: ExchangeResponse,
}

fn recovery() -> Recovery {
    serde_json::from_str(include_str!("../../../fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/recovery-fixtures-v1.json")).unwrap_or_else(|_| unreachable!())
}

fn key(value: u8) -> SecretKey {
    SecretKey::from_bytes(&[value; 32])
}

fn profile(socket: &Path, peer: u8) -> Profile {
    Profile {
        schema: PROFILE_SCHEMA.to_owned(),
        iroh_version: IROH_VERSION.to_owned(),
        alpn: String::from_utf8(ALPN.to_vec()).unwrap_or_else(|_| unreachable!()),
        bind_address: "127.0.0.1:0".parse().unwrap_or_else(|_| unreachable!()),
        relay_urls: Vec::new(),
        allowed_peers: BTreeSet::from([key(peer).public()]),
        socket_path: socket.to_path_buf(),
        transport: TransportProfile {
            schema: lattice_workflow_transport::TRANSPORT_PROFILE_SCHEMA.to_owned(),
            limits: TransportLimits {
                maximum_frame_bytes: 65_536,
                maximum_log_bytes: 4096,
                maximum_connections: 8,
                read_deadline_milliseconds: 1000,
                write_deadline_milliseconds: 1000,
            },
            peers: PeerPolicy {
                allowed_user_ids: vec![
                    std::fs::metadata(socket.parent().unwrap_or_else(|| unreachable!()))
                        .unwrap_or_else(|_| unreachable!())
                        .uid(),
                ],
            },
            non_claims: lattice_workflow_transport::REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        contract: producer().profile,
        exchange_timeout_ms: 1000,
    }
}

// Every spawned service is owned even if an assertion unwinds. Normal teardown also joins it.
struct Task<T>(Option<JoinHandle<T>>);
impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}
impl<T> Task<T> {
    async fn join(mut self) -> T {
        let result = timeout(BOUND, self.0.as_mut().unwrap_or_else(|| unreachable!()))
            .await
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|_| unreachable!());
        let _finished = self.0.take();
        result
    }
}
fn spawn<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Task<T> {
    Task(Some(tokio::spawn(future)))
}

fn address(endpoint: &Endpoint) -> EndpointAddr {
    let sockets = endpoint.bound_sockets();
    assert!(sockets.iter().all(|socket| socket.ip().is_loopback()));
    EndpointAddr::new(endpoint.id()).with_ip_addr(
        *sockets
            .iter()
            .find(|socket| socket.is_ipv4())
            .unwrap_or_else(|| unreachable!()),
    )
}

struct Worker {
    endpoint: Endpoint,
    task: Task<Result<(), kiln_adapter_iroh::Error>>,
    shutdown: oneshot::Sender<()>,
}
impl Worker {
    async fn start(profile: Profile) -> Self {
        let endpoint = bind_endpoint(&profile, key(2))
            .await
            .unwrap_or_else(|_| unreachable!());
        let (shutdown, receive) = oneshot::channel();
        let owned = endpoint.clone();
        let task = spawn(serve_worker(owned, profile, async {
            let _ = receive.await;
        }));
        Self {
            endpoint,
            task,
            shutdown,
        }
    }
    async fn stop(self) {
        let _ = self.shutdown.send(());
        assert_eq!(self.task.join().await, Ok(()));
        self.endpoint.close().await;
    }
}

#[derive(Default)]
struct Effects {
    requests: Vec<ExchangeRequest>,
    executions: usize,
    accepted: bool,
}
struct Handler {
    effects: Arc<Mutex<Effects>>,
    state: ExecutionState,
    sequence: u64,
}
impl Handler {
    fn observation(&mut self) -> ExecutionObservation {
        self.sequence += 1;
        let mut observation = producer().observation;
        observation.sequence = self.sequence;
        observation.state = self.state;
        observation.terminal = self.state.is_terminal();
        observation.successful = self.state == ExecutionState::Completed;
        observation.receipt_identity = if observation.terminal {
            observation.receipt_identity
        } else {
            None
        };
        for node in &mut observation.nodes {
            node.state = match self.state {
                ExecutionState::Paused => NodeState::Paused,
                ExecutionState::Cancelled => NodeState::Cancelled,
                ExecutionState::Completed => NodeState::Completed,
                _ => NodeState::Running,
            };
            node.terminal = observation.terminal;
            node.successful = observation.successful;
            if !node.terminal {
                node.receipt_identity = None;
            }
        }
        observation.identity = observation_identity(&observation);
        observation
    }
}
impl WorkflowExchangeHandler for Handler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        match command {
            ExchangeCommand::Dispatch { request, .. } => {
                assert_eq!(request, &producer().request);
                let mut effects = self.effects.lock();
                if !effects.accepted {
                    effects.accepted = true;
                    effects.executions += 1;
                }
                Ok(ExchangeSuccess::Accepted(producer().accepted))
            }
            ExchangeCommand::RecoverDispatch { request, .. } => {
                assert_eq!(request, &producer().request);
                if !self.effects.lock().accepted {
                    return Err(HandlerError::NotFound);
                }
                Ok(ExchangeSuccess::Accepted(producer().accepted))
            }
            ExchangeCommand::Observe { .. } => Ok(ExchangeSuccess::Observation(self.observation())),
            ExchangeCommand::Control { control, .. } => {
                self.state = match control.action {
                    ControlAction::Pause => ExecutionState::Paused,
                    ControlAction::Resume => ExecutionState::Running,
                    ControlAction::Cancel => ExecutionState::Cancelled,
                };
                Ok(ExchangeSuccess::Observation(self.observation()))
            }
            ExchangeCommand::ReadLog { .. } => Ok(ExchangeSuccess::Log {
                observation: self.observation(),
                bytes: LOG.to_vec(),
            }),
            ExchangeCommand::Reconcile { .. } => {
                self.state = ExecutionState::Completed;
                Ok(ExchangeSuccess::Observation(self.observation()))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Reply {
    Normal,
    LoseDispatch,
    WithholdDispatch,
    Unlinked,
}
struct Provider {
    task: Task<()>,
    shutdown: oneshot::Sender<()>,
    effects: Arc<Mutex<Effects>>,
}
impl Provider {
    fn start(profile: &Profile, reply: Reply) -> Self {
        let listener = UnixListener::bind(&profile.socket_path).unwrap_or_else(|_| unreachable!());
        let effects = Arc::new(Mutex::new(Effects::default()));
        let shared = effects.clone();
        let maximum = profile.transport.limits.maximum_frame_bytes;
        let (shutdown, mut receive) = oneshot::channel();
        let task = spawn(async move {
            let mut service = WorkflowExchangeService::new(Handler {
                effects: shared.clone(),
                state: ExecutionState::Running,
                sequence: 7,
            });
            let mut withheld = Vec::new();
            loop {
                let accepted = tokio::select! {
                    _ = &mut receive => break,
                    result = listener.accept() => result.unwrap_or_else(|_| unreachable!()),
                };
                let (mut stream, _) = accepted;
                let request: ExchangeRequest =
                    serde_json::from_slice(&read_frame(&mut stream, maximum).await)
                        .unwrap_or_else(|_| unreachable!());
                shared.lock().requests.push(request.clone());
                let mut response = service
                    .exchange(&request)
                    .unwrap_or_else(|_| unreachable!());
                if matches!(request.command, ExchangeCommand::Dispatch { .. }) {
                    match reply {
                        Reply::LoseDispatch => continue,
                        Reply::WithholdDispatch => {
                            // Hold the reply open without blocking the independent recovery connection.
                            withheld.push(stream);
                            continue;
                        }
                        Reply::Unlinked => response = recovery().accepted_response,
                        Reply::Normal => {}
                    }
                }
                let frame = encode_frame(&response, maximum).unwrap_or_else(|_| unreachable!());
                timeout(BOUND, stream.write_all(&frame))
                    .await
                    .unwrap_or_else(|_| unreachable!())
                    .unwrap_or_else(|_| unreachable!());
            }
            drop(withheld);
        });
        Self {
            task,
            shutdown,
            effects,
        }
    }
    async fn stop(self) -> Arc<Mutex<Effects>> {
        let _ = self.shutdown.send(());
        self.task.join().await;
        self.effects
    }
}

async fn read_frame(stream: &mut UnixStream, maximum: u32) -> Vec<u8> {
    timeout(BOUND, async {
        let length = stream.read_u32().await.unwrap_or_else(|_| unreachable!());
        assert!(length <= maximum);
        let mut body = vec![0; usize::try_from(length).unwrap_or_else(|_| unreachable!())];
        stream
            .read_exact(&mut body)
            .await
            .unwrap_or_else(|_| unreachable!());
        body
    })
    .await
    .unwrap_or_else(|_| unreachable!())
}

fn dispatch() -> ExchangeRequest {
    let fixture = producer();
    new_request(ExchangeCommand::Dispatch {
        profile: fixture.profile,
        request: fixture.request,
    })
    .unwrap_or_else(|_| unreachable!())
}
fn command(operation: &str) -> ExchangeRequest {
    let fixture = producer();
    let request = fixture.request;
    let profile = fixture.profile;
    let accepted = fixture.accepted;
    let command = match operation {
        "recover" => return recovery().request,
        "observe" => ExchangeCommand::Observe {
            profile,
            request,
            accepted,
        },
        "reconcile" => ExchangeCommand::Reconcile {
            profile,
            request,
            accepted,
        },
        "log" => {
            let log_read = LogReadRequest {
                schema: "lattice.workflow-log-read.v1".to_owned(),
                request_identity: request.identity.clone(),
                workflow_identity: accepted.workflow_identity.clone(),
                execution_identity: accepted.execution_identity.clone(),
                node_id: fixture
                    .observation
                    .logs
                    .first()
                    .unwrap_or_else(|| unreachable!())
                    .node_id
                    .clone(),
                stream: LogStream::Stdout,
                cursor: 0,
                maximum_bytes: 32,
            };
            ExchangeCommand::ReadLog {
                profile,
                request,
                accepted,
                log_read,
            }
        }
        action => {
            let control = ControlRequest {
                schema: "lattice.workflow-control.v1".to_owned(),
                request_identity: request.identity.clone(),
                workflow_identity: accepted.workflow_identity.clone(),
                execution_identity: accepted.execution_identity.clone(),
                expected_sequence: 8,
                action: match action {
                    "pause" => ControlAction::Pause,
                    "resume" => ControlAction::Resume,
                    "cancel" => ControlAction::Cancel,
                    _ => unreachable!("unknown fixture control"),
                },
            };
            ExchangeCommand::Control {
                profile,
                request,
                accepted,
                current_sequence: 8,
                control,
            }
        }
    };
    new_request(command).unwrap_or_else(|_| unreachable!())
}

async fn raw(endpoint: &Endpoint, remote: EndpointAddr, bytes: &[u8]) -> Option<Vec<u8>> {
    timeout(BOUND, async {
        let connection = endpoint.connect(remote, ALPN).await.ok()?;
        let streams = connection.open_bi().await;
        let result = if let Ok((mut send, mut receive)) = streams {
            let _ = send.write_all(bytes).await;
            let _ = send.finish();
            receive
                .read_to_end(65_540)
                .await
                .ok()
                .filter(|body| !body.is_empty())
        } else {
            None
        };
        connection.close(0u32.into(), b"test complete");
        result
    })
    .await
    .unwrap_or_else(|_| unreachable!())
}

// r[verify kiln.iroh.workflow.transport]
// r[verify kiln.iroh.workflow.peers.scenario.allowed]
// r[verify kiln.iroh.workflow.conformance]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_native_operations_preserve_acceptance_controls_logs_and_recovery() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let worker_profile = profile(&directory.path().join("lattice.sock"), 1);
    let provider = Provider::start(&worker_profile, Reply::Normal);
    let worker = Worker::start(worker_profile.clone()).await;
    let controller_profile = profile(&directory.path().join("proxy.sock"), 2);
    let endpoint = bind_endpoint(&controller_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let client = IrohClient::new(
        endpoint.clone(),
        address(&worker.endpoint),
        controller_profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        client
            .exchange(&command("recover"))
            .await
            .unwrap_or_else(|_| unreachable!()),
        recovery().not_found_response
    );
    assert_eq!(provider.effects.lock().executions, 0);
    let request = dispatch();
    let response = timeout(BOUND, client.exchange(&request))
        .await
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(response.request_identity, request.identity);
    assert_eq!(
        response.result,
        ExchangeResult::Success(Box::new(ExchangeSuccess::Accepted(producer().accepted)))
    );
    assert_eq!(
        client
            .exchange(&command("recover"))
            .await
            .unwrap_or_else(|_| unreachable!()),
        recovery().accepted_response
    );
    for (operation, expected) in [
        ("observe", ExecutionState::Running),
        ("pause", ExecutionState::Paused),
        ("resume", ExecutionState::Running),
        ("reconcile", ExecutionState::Completed),
    ] {
        let response = client
            .exchange(&command(operation))
            .await
            .unwrap_or_else(|_| unreachable!());
        let ExchangeResult::Success(success) = response.result else {
            unreachable!("operation rejected: {operation}")
        };
        let ExchangeSuccess::Observation(observation) = *success else {
            unreachable!("not an observation")
        };
        assert_eq!(observation.state, expected);
        assert_eq!(
            observation.successful,
            expected == ExecutionState::Completed
        );
        assert_eq!(
            observation.execution_identity,
            producer().accepted.execution_identity
        );
    }
    let response = client
        .exchange(&command("log"))
        .await
        .unwrap_or_else(|_| unreachable!());
    let ExchangeResult::Success(success) = response.result else {
        unreachable!("log rejected")
    };
    let ExchangeSuccess::Log { bytes, observation } = *success else {
        unreachable!("not a log")
    };
    assert_eq!(bytes, LOG);
    assert!(observation.successful);
    let response = client
        .exchange(&command("cancel"))
        .await
        .unwrap_or_else(|_| unreachable!());
    let ExchangeResult::Success(success) = response.result else {
        unreachable!("cancel rejected")
    };
    let ExchangeSuccess::Observation(observation) = *success else {
        unreachable!("not a cancel observation")
    };
    assert_eq!(observation.state, ExecutionState::Cancelled);
    assert!(!observation.successful);
    endpoint.close().await;
    worker.stop().await;
    let effects = provider.stop().await;
    let effects = effects.lock();
    assert_eq!(effects.executions, 1);
    assert_eq!(effects.requests.first(), Some(&recovery().request));
    assert_eq!(effects.requests.get(1), Some(&request));
    assert_eq!(effects.requests.get(2), Some(&recovery().request));
}

// r[verify kiln.iroh.workflow.peers.scenario.denied]
// r[verify kiln.iroh.workflow.transport.scenario.invalid]
// r[verify kiln.iroh.workflow.profile]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_peer_profile_drift_stale_identity_and_bad_frames_never_open_provider_socket() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let worker_profile = profile(&directory.path().join("lattice.sock"), 1);
    let listener =
        UnixListener::bind(&worker_profile.socket_path).unwrap_or_else(|_| unreachable!());
    let worker = Worker::start(worker_profile.clone()).await;
    let client_profile = profile(&directory.path().join("proxy.sock"), 2);
    let endpoint = bind_endpoint(&client_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let intruder = bind_endpoint(&client_profile, key(3))
        .await
        .unwrap_or_else(|_| unreachable!());
    let frame = encode_frame(&dispatch(), 65_536).unwrap_or_else(|_| unreachable!());
    assert!(
        raw(&intruder, address(&worker.endpoint), &frame)
            .await
            .is_none()
    );
    let mut wrong_pin_profile = client_profile.clone();
    wrong_pin_profile.allowed_peers = BTreeSet::from([key(4).public()]);
    let wrong_worker = EndpointAddr::new(key(4).public()).with_ip_addr(
        worker
            .endpoint
            .bound_sockets()
            .into_iter()
            .find(|socket| socket.is_ipv4())
            .unwrap_or_else(|| unreachable!()),
    );
    let wrong_pin = IrohClient::new(endpoint.clone(), wrong_worker, wrong_pin_profile)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        timeout(BOUND, wrong_pin.exchange(&dispatch()))
            .await
            .unwrap_or_else(|_| unreachable!())
            .is_err()
    );
    let mut stale = dispatch();
    stale.identity = recovery().request.identity;
    let mut drift = dispatch();
    if let ExchangeCommand::Dispatch { profile, .. } = &mut drift.command {
        profile.limits.maximum_nodes += 1;
    }
    // Recompute both native identities so this is a valid but unselected contract, not corrupt JSON.
    if let ExchangeCommand::Dispatch { profile, request } = &mut drift.command {
        request.profile_identity = lattice_workflow_contract::profile_identity(profile);
        request.identity = lattice_workflow_contract::request_identity(request);
    }
    drift.identity = lattice_workflow_transport::request_identity(&drift.command)
        .unwrap_or_else(|_| unreachable!());
    lattice_workflow_transport::validate_exchange_request(&drift)
        .unwrap_or_else(|_| unreachable!());
    for bytes in [
        encode_frame(&stale, 65_536).unwrap_or_else(|_| unreachable!()),
        encode_frame(&drift, 65_536).unwrap_or_else(|_| unreachable!()),
        [4u32.to_be_bytes().as_slice(), b"nope"].concat(),
        [8u32.to_be_bytes().as_slice(), b"nope"].concat(),
        [frame.as_slice(), frame.as_slice()].concat(),
        65_537u32.to_be_bytes().to_vec(),
    ] {
        assert!(
            raw(&endpoint, address(&worker.endpoint), &bytes)
                .await
                .is_none()
        );
    }
    assert!(
        timeout(Duration::from_millis(150), listener.accept())
            .await
            .is_err()
    );
    endpoint.close().await;
    intruder.close().await;
    worker.stop().await;
}

// r[verify kiln.iroh.workflow.transport.scenario.invalid]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_valid_but_unlinked_provider_response_is_not_success() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let worker_profile = profile(&directory.path().join("lattice.sock"), 1);
    let provider = Provider::start(&worker_profile, Reply::Unlinked);
    let worker = Worker::start(worker_profile).await;
    let client_profile = profile(&directory.path().join("proxy.sock"), 2);
    let endpoint = bind_endpoint(&client_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let client = IrohClient::new(endpoint.clone(), address(&worker.endpoint), client_profile)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        timeout(BOUND, client.exchange(&dispatch()))
            .await
            .unwrap_or_else(|_| unreachable!())
            .is_err()
    );
    endpoint.close().await;
    worker.stop().await;
    assert_eq!(provider.stop().await.lock().executions, 1);
}

async fn proxy_ready(path: &Path) {
    timeout(BOUND, async {
        loop {
            if std::fs::symlink_metadata(path)
                .is_ok_and(|metadata| metadata.permissions().mode() & 0o777 == 0o600)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| unreachable!());
}
fn consumer_request(operation: WorkflowExchangeOperation) -> WorkflowExchangeRequest {
    let body = serde_json::to_vec(&producer().request).unwrap_or_else(|_| unreachable!());
    WorkflowExchangeRequest {
        operation,
        body_identity: kiln_codec::identity_from_bytes(&body).unwrap_or_else(|_| unreachable!()),
        body,
        max_response_bytes: 65_536,
    }
}

async fn through_proxy(reply: Option<Reply>) {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let worker_profile = profile(&directory.path().join("lattice.sock"), 1);
    let provider = reply.map(|reply| Provider::start(&worker_profile, reply));
    let worker = Worker::start(worker_profile).await;
    let controller_profile = profile(&directory.path().join("proxy.sock"), 2);
    let endpoint = bind_endpoint(&controller_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let (shutdown, receive) = oneshot::channel();
    let owned = endpoint.clone();
    let remote = address(&worker.endpoint);
    let owned_profile = controller_profile.clone();
    let proxy = spawn(serve_proxy(owned, remote, owned_profile, async {
        let _ = receive.await;
    }));
    proxy_ready(&controller_profile.socket_path).await;
    assert_eq!(
        std::fs::metadata(&controller_profile.socket_path)
            .unwrap_or_else(|_| unreachable!())
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let recoverable = matches!(reply, Some(Reply::LoseDispatch | Reply::WithholdDispatch));
    let consumer = spawn(async move {
        // The consumer is blocking; never block an Iroh executor thread on its Unix exchange.
        tokio::task::spawn_blocking(move || {
            let mut consumer = UnixWorkflowExchange::new(
                controller_profile.socket_path,
                controller_profile.transport,
                controller_profile.contract,
                8,
            )
            .unwrap_or_else(|_| unreachable!());
            let Err(failure) =
                consumer.exchange(consumer_request(WorkflowExchangeOperation::Dispatch))
            else {
                unreachable!("ambiguous dispatch became success")
            };
            assert!(
                matches!(failure, PortFailure::Unknown(_)),
                "must not become success or retryable: {failure:?}"
            );
            let result =
                consumer.exchange(consumer_request(WorkflowExchangeOperation::RecoverDispatch));
            if recoverable {
                let recovered: WorkflowAccepted =
                    serde_json::from_slice(&result.unwrap_or_else(|_| unreachable!()).body)
                        .unwrap_or_else(|_| unreachable!());
                assert_eq!(recovered, producer().accepted);
            } else {
                assert!(matches!(result, Err(PortFailure::Unknown(_))));
            }
        })
        .await
        .unwrap_or_else(|_| unreachable!());
    });
    consumer.join().await;
    let _ = shutdown.send(());
    proxy.join().await.unwrap_or_else(|_| unreachable!());
    endpoint.close().await;
    worker.stop().await;
    if let Some(provider) = provider {
        let effects = provider.stop().await;
        let effects = effects.lock();
        assert_eq!(effects.executions, 1);
        assert_eq!(effects.requests.len(), 2);
        let ExchangeCommand::Dispatch {
            request: original, ..
        } = &effects
            .requests
            .first()
            .unwrap_or_else(|| unreachable!())
            .command
        else {
            unreachable!("first exchange was not dispatch")
        };
        let ExchangeCommand::RecoverDispatch {
            request: recovered, ..
        } = &effects
            .requests
            .get(1)
            .unwrap_or_else(|| unreachable!())
            .command
        else {
            unreachable!("ambiguous dispatch was blindly repeated")
        };
        assert_eq!(original, recovered);
        assert_eq!(effects.requests.get(1), Some(&recovery().request));
    }
}

// r[verify kiln.iroh.workflow.recovery.scenario.unknown]
// r[verify kiln.iroh.workflow.conformance]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_response_stays_unknown_to_unix_consumer_and_exact_recovery_does_not_redispatch() {
    through_proxy(Some(Reply::LoseDispatch)).await;
}

// r[verify kiln.iroh.workflow.recovery]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadline_after_provider_acceptance_stays_unknown_and_recovers_original_request() {
    through_proxy(Some(Reply::WithholdDispatch)).await;
}

// r[verify kiln.iroh.workflow.recovery]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unavailable_provider_is_unknown_not_safe_retry_through_proxy() {
    through_proxy(None).await;
}

// r[verify kiln.iroh.workflow.peers]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxy_denies_unlisted_local_user_without_opening_worker_provider_socket() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let worker_profile = profile(&directory.path().join("lattice.sock"), 1);
    let listener =
        UnixListener::bind(&worker_profile.socket_path).unwrap_or_else(|_| unreachable!());
    let worker = Worker::start(worker_profile).await;
    let mut controller_profile = profile(&directory.path().join("proxy.sock"), 2);
    *controller_profile
        .transport
        .peers
        .allowed_user_ids
        .first_mut()
        .unwrap_or_else(|| unreachable!()) ^= 1;
    let endpoint = bind_endpoint(&controller_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let (shutdown, receive) = oneshot::channel();
    let owned = endpoint.clone();
    let remote = address(&worker.endpoint);
    let path = controller_profile.socket_path.clone();
    let proxy = spawn(serve_proxy(owned, remote, controller_profile, async {
        let _ = receive.await;
    }));
    proxy_ready(&path).await;
    let mut local = UnixStream::connect(&path)
        .await
        .unwrap_or_else(|_| unreachable!());
    let frame = encode_frame(&dispatch(), 65_536).unwrap_or_else(|_| unreachable!());
    let _ = timeout(BOUND, local.write_all(&frame))
        .await
        .unwrap_or_else(|_| unreachable!());
    let _ = local.shutdown().await;
    let mut reply = Vec::new();
    let result = timeout(BOUND, local.read_to_end(&mut reply))
        .await
        .unwrap_or_else(|_| unreachable!());
    assert!(result.is_err() || reply.is_empty());
    assert!(
        timeout(Duration::from_millis(150), listener.accept())
            .await
            .is_err()
    );
    let _ = shutdown.send(());
    proxy.join().await.unwrap_or_else(|_| unreachable!());
    endpoint.close().await;
    worker.stop().await;
}

// r[verify kiln.iroh.workflow.profile.scenario.valid]
// r[verify kiln.iroh.workflow.profile.scenario.invalid]
#[test]
fn profile_admission_rejects_drift_empty_authority_and_invalid_bounds() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let admitted = profile(&directory.path().join("lattice.sock"), 1);
    assert_eq!(admitted.admit(), Ok(()));
    let mut wrong_cohort = admitted.clone();
    wrong_cohort.iroh_version = "0.96.0".to_owned();
    let mut no_peers = admitted.clone();
    no_peers.allowed_peers.clear();
    let mut deadline = admitted.clone();
    deadline.exchange_timeout_ms = 0;
    let mut frames = admitted.clone();
    frames.transport.limits.maximum_frame_bytes = 0;
    let mut concurrency = admitted.clone();
    concurrency.transport.limits.maximum_connections = 65;
    let mut socket = admitted.clone();
    socket.socket_path = directory.path().join("./lattice.sock");
    let mut relay = admitted;
    relay.relay_urls = vec!["http://relay.invalid".to_owned()];
    for candidate in [
        wrong_cohort,
        no_peers,
        deadline,
        frames,
        concurrency,
        socket,
        relay,
    ] {
        assert_eq!(
            candidate.admit(),
            Err(kiln_adapter_iroh::Error::InvalidProfile)
        );
    }
}
