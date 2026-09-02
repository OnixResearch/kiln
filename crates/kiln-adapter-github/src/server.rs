use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_provider_api::PortResult;

use crate::events::{DeliveryHeaders, admit_event};
use crate::profile::{AdmittedGithubConfig, GithubConfig};
use crate::replay::{SqliteStore, StoreLimits};
use crate::secret::{ResolvedSecret, resolve_secret};
use crate::service::{GithubService, error_code, plan_event};
use crate::status::{GithubStatusClient, StatusPort};
use crate::{domain, infrastructure};

const HEADER_DELIMITER: &[u8] = b"\r\n\r\n";
const READ_CHUNK_BYTES: usize = 1_024;
const MAXIMUM_HEADER_FIELDS: usize = 64;
const ACCEPT_POLL_MS: u64 = 25;

#[derive(Debug)]
struct Request {
    headers: DeliveryHeaders,
    bytes: Vec<u8>,
    body_offset: usize,
}

impl Request {
    fn body(&self) -> &[u8] {
        self.bytes.get(self.body_offset..).unwrap_or_default()
    }
}

pub fn open_store(admitted: &AdmittedGithubConfig) -> PortResult<SqliteStore> {
    SqliteStore::open(
        &admitted.config.replay_database_path,
        admitted.identity.clone(),
        StoreLimits {
            maximum_runs: admitted.config.maximum_run_records,
            maximum_deliveries: admitted.config.maximum_delivery_records,
            maximum_record_bytes: admitted.config.maximum_record_bytes,
        },
    )
}

pub fn runtime(
    admitted: &AdmittedGithubConfig,
) -> PortResult<GithubService<LatticeAdapter<UnixWorkflowExchange>, GithubStatusClient>> {
    let token = resolve_secret(
        &admitted.config.status_secret,
        "publish exact Kiln GitHub CI status",
    )?;
    let status = GithubStatusClient::new(admitted.config.status.clone(), token)?;
    let transport = UnixWorkflowExchange::new(
        admitted.config.workflow.socket_path.clone(),
        admitted.transport.clone(),
        admitted.lattice.contract.clone(),
        admitted.config.maximum_run_records,
    )?;
    let workflow = LatticeAdapter::new(admitted.lattice.clone(), transport)?;
    Ok(GithubService::new(
        workflow,
        status,
        open_store(admitted)?,
        admitted.config.clone(),
    ))
}

// r[impl kiln.github.shell]
pub fn serve(admitted: AdmittedGithubConfig) -> PortResult<()> {
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop)).map_err(|_| {
        infrastructure(
            "github_signal",
            "GitHub shutdown signal registration failed",
        )
    })?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop)).map_err(|_| {
        infrastructure(
            "github_signal",
            "GitHub shutdown signal registration failed",
        )
    })?;
    let webhook_secret = resolve_secret(
        &admitted.config.webhook_secret,
        "authenticate Kiln GitHub webhook deliveries",
    )?;
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let service = runtime(&admitted)?;
    serve_with(admitted, webhook_secret, service, stop)
}

pub fn serve_with<W, P>(
    admitted: AdmittedGithubConfig,
    webhook_secret: ResolvedSecret,
    service: GithubService<W, P>,
    stop: Arc<AtomicBool>,
) -> PortResult<()>
where
    W: kiln_provider_api::WorkflowPort + Send + 'static,
    P: StatusPort + Send + 'static,
{
    let listener = TcpListener::bind(admitted.config.listen_address)
        .map_err(|_| infrastructure("github_listen", "GitHub webhook listener could not bind"))?;
    serve_on(admitted, webhook_secret, service, stop, listener)
}

pub(crate) fn serve_on<W, P>(
    admitted: AdmittedGithubConfig,
    webhook_secret: ResolvedSecret,
    mut service: GithubService<W, P>,
    stop: Arc<AtomicBool>,
    listener: TcpListener,
) -> PortResult<()>
where
    W: kiln_provider_api::WorkflowPort + Send + 'static,
    P: StatusPort + Send + 'static,
{
    if listener.local_addr().ok() != Some(admitted.config.listen_address) {
        return Err(domain(
            "github_listener_address",
            "github.http",
            "GitHub listener differs from the admitted address",
        ));
    }
    let intake_stores = (0..admitted.config.maximum_connections)
        .map(|_| open_store(&admitted))
        .collect::<PortResult<Vec<_>>>()?;
    listener.set_nonblocking(true).map_err(|_| {
        infrastructure(
            "github_listen",
            "GitHub webhook listener could not enter bounded accept mode",
        )
    })?;
    let interval = Duration::from_millis(admitted.config.worker_poll_interval_ms);
    let worker_stop = Arc::clone(&stop);
    let worker = std::thread::Builder::new()
        .name("kiln-github-worker".to_string())
        .spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                if let Err(error) = service.tick() {
                    eprintln!("GitHub worker: {}", error_code(&error));
                }
                std::thread::park_timeout(interval);
            }
        })
        .map_err(|_| infrastructure("github_worker", "GitHub worker could not start"))?;
    let active = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) =
        std::sync::mpsc::sync_channel::<(TcpStream, Instant)>(admitted.config.maximum_connections);
    let receiver = Arc::new(Mutex::new(receiver));
    let result = std::thread::scope(|scope| {
        let mut handlers = Vec::with_capacity(admitted.config.maximum_connections);
        for mut intake in intake_stores {
            let receiver = Arc::clone(&receiver);
            let active = Arc::clone(&active);
            let stop = Arc::clone(&stop);
            let admitted = &admitted;
            let secret = webhook_secret.as_bytes();
            let handler = std::thread::Builder::new()
                .name("kiln-github-http".to_string())
                .spawn_scoped(scope, move || {
                    loop {
                        let job = match receiver.lock() {
                            Ok(receiver) => receiver.recv(),
                            Err(_) => {
                                stop.store(true, Ordering::Release);
                                break;
                            }
                        };
                        let Ok((mut stream, accepted_at)) = job else {
                            break;
                        };
                        if let Err(error) = handle_connection(
                            &mut stream,
                            admitted,
                            secret,
                            &mut intake,
                            accepted_at,
                        ) {
                            eprintln!("GitHub intake: {}", error_code(&error));
                        }
                        drop(stream);
                        active.fetch_sub(1, Ordering::AcqRel);
                    }
                });
            match handler {
                Ok(handler) => handlers.push(handler),
                Err(_) => {
                    drop(sender);
                    for handler in handlers {
                        let _ = handler.join();
                    }
                    return Err(infrastructure(
                        "github_http_worker",
                        "GitHub HTTP worker could not start",
                    ));
                }
            }
        }
        eprintln!(
            "GitHub webhook listener ready at {}",
            admitted.config.listen_address
        );
        let result = (|| {
            while !stop.load(Ordering::Acquire) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(ACCEPT_POLL_MS));
                        continue;
                    }
                    Err(_) => {
                        return Err(infrastructure(
                            "github_accept",
                            "GitHub webhook connection could not be accepted",
                        ));
                    }
                };
                let accepted_at = Instant::now();
                if active.load(Ordering::Acquire) >= admitted.config.maximum_connections {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(
                        admitted.config.write_timeout_ms,
                    )));
                    let _ = respond(
                        &mut stream,
                        503,
                        "Service Unavailable",
                        &serde_json::json!({"accepted":false,"error":"github_http_capacity"}),
                    );
                    continue;
                }
                active.fetch_add(1, Ordering::AcqRel);
                if let Err(error) = sender.try_send((stream, accepted_at)) {
                    active.fetch_sub(1, Ordering::AcqRel);
                    let (mut stream, _) = match error {
                        std::sync::mpsc::TrySendError::Full(job)
                        | std::sync::mpsc::TrySendError::Disconnected(job) => job,
                    };
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(
                        admitted.config.write_timeout_ms,
                    )));
                    let _ = respond(
                        &mut stream,
                        503,
                        "Service Unavailable",
                        &serde_json::json!({"accepted":false,"error":"github_http_capacity"}),
                    );
                }
            }
            Ok(())
        })();
        drop(sender);
        for handler in handlers {
            if handler.join().is_err() {
                return Err(infrastructure(
                    "github_http_worker",
                    "GitHub HTTP worker stopped unexpectedly",
                ));
            }
        }
        result
    });
    stop.store(true, Ordering::Release);
    worker.thread().unpark();
    worker
        .join()
        .map_err(|_| infrastructure("github_worker", "GitHub worker stopped unexpectedly"))?;
    result
}

// r[impl kiln.github.authentication]
// r[impl kiln.github.shell]
fn handle_connection(
    stream: &mut TcpStream,
    admitted: &AdmittedGithubConfig,
    secret: &[u8],
    store: &mut SqliteStore,
    accepted_at: Instant,
) -> PortResult<()> {
    stream
        .set_write_timeout(Some(Duration::from_millis(
            admitted.config.write_timeout_ms,
        )))
        .map_err(|_| {
            infrastructure(
                "github_http_timeout",
                "GitHub response timeout could not be set",
            )
        })?;
    let result = read_request(stream, &admitted.config, accepted_at).and_then(|request| {
        match admit_event(&admitted.config.event_policy, &request.headers, request.body(), secret, admitted.config.maximum_body_bytes)? {
            None => Ok(serde_json::json!({"accepted": true, "ignored": true})),
            Some(event) => {
                let record = plan_event(admitted, &event)?;
                let reserved = store.reserve(&event, record)?;
                Ok(serde_json::json!({"accepted":true,"run_identity":reserved.run_identity.as_str(),"duplicate":!reserved.inserted}))
            }
        }
    });
    match result {
        Ok(value) => respond(stream, 202, "Accepted", &value),
        Err(error) => {
            let code = error_code(&error);
            let status = if code == "github_http_body_bound" {
                (413, "Content Too Large")
            } else if code == "github_http_method" {
                (405, "Method Not Allowed")
            } else if code == "github_http_deadline" {
                (408, "Request Timeout")
            } else if code == "github_replay_capacity"
                || matches!(
                    error,
                    kiln_provider_api::PortFailure::Infrastructure(_)
                        | kiln_provider_api::PortFailure::Retryable(_)
                )
            {
                (503, "Service Unavailable")
            } else {
                (400, "Bad Request")
            };
            respond(
                stream,
                status.0,
                status.1,
                &serde_json::json!({"accepted":false,"error":code}),
            )?;
            Err(error)
        }
    }
}

fn read_request(
    stream: &mut TcpStream,
    config: &GithubConfig,
    accepted_at: Instant,
) -> PortResult<Request> {
    let deadline = accepted_at
        .checked_add(Duration::from_millis(config.read_timeout_ms))
        .ok_or_else(http_deadline)?;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    let body_offset = loop {
        let start = bytes.len().saturating_sub(HEADER_DELIMITER.len() - 1);
        let remaining = config
            .maximum_header_bytes
            .saturating_add(1)
            .saturating_sub(bytes.len());
        if remaining == 0 {
            return Err(http_error(
                "github_http_header_bound",
                "GitHub HTTP headers exceed their byte bound",
            ));
        }
        let count = READ_CHUNK_BYTES.min(remaining);
        let count = read_deadline(
            stream,
            chunk.get_mut(..count).ok_or_else(http_deadline)?,
            deadline,
        )?;
        if count == 0 {
            return Err(http_error(
                "github_http_incomplete",
                "GitHub HTTP request ended before its headers",
            ));
        }
        bytes.extend_from_slice(chunk.get(..count).ok_or_else(http_deadline)?);
        if let Some(position) = bytes.get(start..).and_then(|tail| {
            tail.windows(HEADER_DELIMITER.len())
                .position(|window| window == HEADER_DELIMITER)
        }) {
            let end = start + position + HEADER_DELIMITER.len();
            if end > config.maximum_header_bytes {
                return Err(http_error(
                    "github_http_header_bound",
                    "GitHub HTTP headers exceed their byte bound",
                ));
            }
            break end;
        }
    };
    let head = std::str::from_utf8(
        bytes
            .get(..body_offset)
            .ok_or_else(|| http_error("github_http_headers", "GitHub HTTP headers are invalid"))?,
    )
    .map_err(|_| http_error("github_http_headers", "GitHub HTTP headers must be ASCII"))?;
    if !head.is_ascii() {
        return Err(http_error(
            "github_http_headers",
            "GitHub HTTP headers must be ASCII",
        ));
    }
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split(' ');
    if request_line.next() != Some("POST") {
        return Err(http_error(
            "github_http_method",
            "GitHub webhook accepts only POST",
        ));
    }
    if request_line.next() != Some(config.webhook_path.as_str())
        || request_line.next() != Some("HTTP/1.1")
        || request_line.next().is_some()
    {
        return Err(http_error(
            "github_http_target",
            "GitHub HTTP target or protocol is unsupported",
        ));
    }
    let mut delivery = None;
    let mut event = None;
    let mut signature = None;
    let mut length = None;
    let mut content_type = None;
    let mut host = None;
    let mut fields = 0;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        fields += 1;
        if fields > MAXIMUM_HEADER_FIELDS || line.starts_with([' ', '\t']) {
            return Err(http_error(
                "github_http_headers",
                "GitHub HTTP header count or folding is unsupported",
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| http_error("github_http_headers", "GitHub HTTP header is malformed"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(http_error(
                "github_http_headers",
                "GitHub HTTP header is malformed",
            ));
        }
        let value = value.trim_matches([' ', '\t']);
        let slot = if name.eq_ignore_ascii_case("x-github-delivery") {
            Some(&mut delivery)
        } else if name.eq_ignore_ascii_case("x-github-event") {
            Some(&mut event)
        } else if name.eq_ignore_ascii_case("x-hub-signature-256") {
            Some(&mut signature)
        } else if name.eq_ignore_ascii_case("content-length") {
            Some(&mut length)
        } else if name.eq_ignore_ascii_case("content-type") {
            Some(&mut content_type)
        } else if name.eq_ignore_ascii_case("host") {
            Some(&mut host)
        } else {
            None
        };
        if let Some(slot) = slot {
            if slot.replace(value).is_some() {
                return Err(http_error(
                    "github_http_duplicate_header",
                    "GitHub HTTP critical header occurs more than once",
                ));
            }
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("expect")
            || name.eq_ignore_ascii_case("content-encoding")
        {
            return Err(http_error(
                "github_http_framing",
                "GitHub HTTP transfer, content encoding or expectation is unsupported",
            ));
        }
    }
    if host.is_none_or(str::is_empty)
        || !matches!(
            content_type,
            Some("application/json" | "application/json; charset=utf-8")
        )
    {
        return Err(http_error(
            "github_http_content_type",
            "GitHub HTTP Host and JSON content type are required",
        ));
    }
    let length = length
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            http_error(
                "github_http_length",
                "GitHub HTTP requires one bounded decimal Content-Length",
            )
        })?;
    if length == 0 || length > config.maximum_body_bytes {
        return Err(http_error(
            "github_http_body_bound",
            "GitHub HTTP body exceeds its byte bound",
        ));
    }
    let headers = DeliveryHeaders {
        delivery: delivery
            .ok_or_else(|| {
                http_error("github_http_delivery", "GitHub delivery header is required")
            })?
            .to_string(),
        event: event
            .ok_or_else(|| http_error("github_http_event", "GitHub event header is required"))?
            .to_string(),
        signature: signature
            .ok_or_else(|| {
                http_error(
                    "github_http_signature",
                    "GitHub signature header is required",
                )
            })?
            .to_string(),
    };
    let total = body_offset.checked_add(length).ok_or_else(|| {
        http_error(
            "github_http_length",
            "GitHub HTTP length overflows the input bound",
        )
    })?;
    if bytes.len() > total {
        return Err(http_error(
            "github_http_framing",
            "GitHub HTTP pipelining or trailing bytes are unsupported",
        ));
    }
    while bytes.len() < total {
        let count = READ_CHUNK_BYTES.min(total - bytes.len());
        let count = read_deadline(
            stream,
            chunk.get_mut(..count).ok_or_else(http_deadline)?,
            deadline,
        )?;
        if count == 0 {
            return Err(http_error(
                "github_http_incomplete",
                "GitHub HTTP body ended before Content-Length",
            ));
        }
        bytes.extend_from_slice(chunk.get(..count).ok_or_else(http_deadline)?);
    }
    Ok(Request {
        headers,
        bytes,
        body_offset,
    })
}

fn read_deadline(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> PortResult<usize> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(http_deadline());
    }
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|_| http_deadline())?;
    stream.read(buffer).map_err(|error| {
        if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            http_deadline()
        } else {
            http_error("github_http_read", "GitHub HTTP input could not be read")
        }
    })
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    value: &serde_json::Value,
) -> PortResult<()> {
    let body = serde_json::to_vec(value).map_err(|_| {
        infrastructure(
            "github_http_encoding",
            "GitHub HTTP response encoding failed",
        )
    })?;
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .map_err(|_| {
            infrastructure(
                "github_http_write",
                "GitHub HTTP response could not be written",
            )
        })
}

fn http_error(code: &str, message: &str) -> kiln_provider_api::PortFailure {
    domain(code, "github.http", message)
}
fn http_deadline() -> kiln_provider_api::PortFailure {
    http_error(
        "github_http_deadline",
        "GitHub HTTP request exceeded its total read deadline",
    )
}
