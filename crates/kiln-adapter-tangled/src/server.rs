//! Bounded HTTP/1.1 shell for the spindle XRPC surface.
//!
//! One request per connection. Headers, bodies and socket time are bounded;
//! POST bodies need an explicit `Content-Length` and chunked bodies are
//! refused. A worker thread advances runs and status deliveries.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kiln_provider_api::{PortResult, WorkflowPort};

use crate::error::infrastructure;
use crate::profile::AdmittedProfile;
use crate::resolver::{HttpDidResolver, KeyResolver};
use crate::service::TangledService;
use crate::source::{GitCliSource, GitLimits, SourcePort};
use crate::status::{HttpStatusPublisher, StatusPort, read_secret};
use crate::store::{SqliteStore, StoreLimits};
use crate::xrpc::{Method, XrpcRequest, XrpcResponse};

const HEADER_DELIMITER: &[u8] = b"\r\n\r\n";
const READ_CHUNK_BYTES: usize = 4096;
const MAXIMUM_HEADER_FIELDS: usize = 64;
const ACCEPT_POLL_MS: u64 = 25;
const XRPC_PREFIX: &str = "/xrpc/";
const HEALTH_PATH: &str = "/_health";

#[derive(Clone, Copy, Debug)]
pub struct ServerLimits {
    pub maximum_header_bytes: usize,
    pub maximum_body_bytes: usize,
    pub read_timeout_ms: u64,
    pub write_timeout_ms: u64,
    pub worker_poll_interval_ms: u64,
}

impl ServerLimits {
    #[must_use]
    pub fn from_profile(admitted: &AdmittedProfile) -> Self {
        let limits = admitted.profile.limits;
        Self {
            maximum_header_bytes: limits.maximum_header_bytes,
            maximum_body_bytes: limits.maximum_body_bytes,
            read_timeout_ms: limits.read_timeout_ms,
            write_timeout_ms: limits.write_timeout_ms,
            worker_poll_interval_ms: limits.worker_poll_interval_ms,
        }
    }
}

pub type Shared<S, W, P, K> = Arc<Mutex<TangledService<S, W, P, K>>>;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

/// Production ports for an admitted profile; the caller supplies the Kiln
/// `WorkflowPort` that executes runs.
pub fn runtime<W: WorkflowPort>(
    admitted: &AdmittedProfile,
    workflow: W,
) -> PortResult<TangledService<GitCliSource, W, HttpStatusPublisher, HttpDidResolver>> {
    let profile = &admitted.profile;
    let limits = profile.limits;
    let store = SqliteStore::open(
        &profile.database_path,
        StoreLimits {
            maximum_pipelines: limits.maximum_pipelines,
            maximum_tokens: limits.maximum_tokens,
            maximum_record_bytes: limits.maximum_record_bytes,
        },
    )?;
    let source = GitCliSource::new(
        profile.git_program.clone(),
        profile.source_directory.clone(),
        GitLimits {
            timeout_ms: limits.acquisition_timeout_ms,
            maximum_workflow_files: limits.maximum_workflow_files,
            maximum_workflow_bytes: limits.maximum_workflow_bytes,
            maximum_changed_paths: limits.maximum_changed_paths,
            maximum_changed_path_bytes: limits.maximum_changed_path_bytes,
        },
    )?;
    let status = HttpStatusPublisher::new(
        &profile.status.endpoint,
        read_secret(&profile.status.secret_path)?,
        profile.status.timeout_ms,
    )?;
    let keys = HttpDidResolver::new(
        &profile.plc_directory_url,
        limits.resolver_timeout_ms,
        limits.maximum_did_document_bytes,
        limits.key_cache_seconds,
    );
    Ok(TangledService::new(
        admitted.clone(),
        store,
        source,
        workflow,
        status,
        keys,
    ))
}

// r[impl kiln.tangled.xrpc]
/// Bind the profile's listen address and serve until `stop` is raised.
pub fn serve<W>(admitted: &AdmittedProfile, workflow: W, stop: Arc<AtomicBool>) -> PortResult<()>
where
    W: WorkflowPort + Send + 'static,
{
    let service = runtime(admitted, workflow)?;
    let listener = TcpListener::bind(admitted.profile.listen_address)
        .map_err(|_| infrastructure("tangled_listen", "Spindle listener could not bind"))?;
    serve_on(
        listener,
        Arc::new(Mutex::new(service)),
        ServerLimits::from_profile(admitted),
        stop,
    )
}

pub fn serve_on<S, W, P, K>(
    listener: TcpListener,
    service: Shared<S, W, P, K>,
    limits: ServerLimits,
    stop: Arc<AtomicBool>,
) -> PortResult<()>
where
    S: SourcePort + Send + 'static,
    W: WorkflowPort + Send + 'static,
    P: StatusPort + Send + 'static,
    K: KeyResolver + Send + 'static,
{
    listener
        .set_nonblocking(true)
        .map_err(|_| infrastructure("tangled_listen", "Listener could not be configured"))?;
    let worker = {
        let service = Arc::clone(&service);
        let stop = Arc::clone(&stop);
        thread::spawn(move || work(&service, limits, &stop))
    };
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                // A failed connection affects only that client.
                let _ = handle_connection(stream, &service, limits);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(ACCEPT_POLL_MS));
            }
            Err(_) => thread::sleep(Duration::from_millis(ACCEPT_POLL_MS)),
        }
    }
    worker
        .join()
        .map_err(|_| infrastructure("tangled_worker", "Spindle worker stopped abnormally"))
}

fn work<S, W, P, K>(service: &Shared<S, W, P, K>, limits: ServerLimits, stop: &AtomicBool)
where
    S: SourcePort,
    W: WorkflowPort,
    P: StatusPort,
    K: KeyResolver,
{
    while !stop.load(Ordering::Acquire) {
        let mut progressed = true;
        while progressed && !stop.load(Ordering::Acquire) {
            progressed = match service.lock() {
                // A failed tick is retried on the next pass; state is durable.
                Ok(mut service) => service.tick(now_unix()).unwrap_or(false),
                Err(_) => return,
            };
        }
        thread::sleep(Duration::from_millis(limits.worker_poll_interval_ms));
    }
}

fn respond(stream: &mut TcpStream, response: &XrpcResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn handle_connection<S, W, P, K>(
    mut stream: TcpStream,
    service: &Shared<S, W, P, K>,
    limits: ServerLimits,
) -> std::io::Result<()>
where
    S: SourcePort,
    W: WorkflowPort,
    P: StatusPort,
    K: KeyResolver,
{
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(limits.read_timeout_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(limits.write_timeout_ms)))?;
    let response = match read_request(&mut stream, limits) {
        Ok(Parsed::Health) => XrpcResponse::ok(&serde_json::json!({ "status": "ok" })),
        Ok(Parsed::Xrpc(request)) => match service.lock() {
            Ok(mut service) => service.handle(&request, now_unix()),
            Err(_) => {
                XrpcResponse::named(503, "ServiceUnavailable", "Service state is unavailable")
            }
        },
        Err((status, message)) => XrpcResponse::named(status, "InvalidRequest", message),
    };
    respond(&mut stream, &response)
}

enum Parsed {
    Health,
    Xrpc(XrpcRequest),
}

type HttpError = (u16, &'static str);

fn read_request(stream: &mut TcpStream, limits: ServerLimits) -> Result<Parsed, HttpError> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    let header_end = loop {
        if let Some(position) = buffer
            .windows(HEADER_DELIMITER.len())
            .position(|window| window == HEADER_DELIMITER)
        {
            break position;
        }
        if buffer.len() > limits.maximum_header_bytes {
            return Err((400, "Request headers exceed their bound"));
        }
        let read = stream
            .read(&mut chunk)
            .map_err(|_| (400, "Request could not be read in time"))?;
        if read == 0 {
            return Err((400, "Request ended before its headers"));
        }
        buffer.extend_from_slice(chunk.get(..read).unwrap_or_default());
    };
    if header_end > limits.maximum_header_bytes {
        return Err((400, "Request headers exceed their bound"));
    }
    let head = std::str::from_utf8(buffer.get(..header_end).unwrap_or_default())
        .map_err(|_| (400, "Headers are not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some("HTTP/1.1"), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err((400, "Malformed request line"));
    };
    let method = match method {
        "GET" => Method::Get,
        "POST" => Method::Post,
        _ => return Err((405, "Only GET and POST are served")),
    };
    let mut content_length = None;
    let mut authorization = None;
    for (count, line) in lines.enumerate() {
        if count >= MAXIMUM_HEADER_FIELDS {
            return Err((400, "Too many header fields"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or((400, "Malformed header field"))?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| (400, "Invalid Content-Length"))?;
                if content_length.replace(parsed).is_some() {
                    return Err((400, "Repeated Content-Length"));
                }
            }
            "transfer-encoding" => {
                return Err((411, "Chunked bodies are refused; send Content-Length"));
            }
            "authorization" => {
                if authorization.replace(value.to_string()).is_some() {
                    return Err((400, "Repeated Authorization"));
                }
            }
            _ => {}
        }
    }
    let length = match (method, content_length) {
        (Method::Post, None) => return Err((411, "POST requires Content-Length")),
        (_, Some(length)) if length > limits.maximum_body_bytes => {
            return Err((413, "Body exceeds its bound"));
        }
        (_, length) => length.unwrap_or(0),
    };
    let mut body = buffer
        .get(header_end + HEADER_DELIMITER.len()..)
        .unwrap_or_default()
        .to_vec();
    if body.len() > length {
        return Err((400, "Body is longer than Content-Length"));
    }
    while body.len() < length {
        let read = stream
            .read(&mut chunk)
            .map_err(|_| (400, "Body could not be read in time"))?;
        if read == 0 {
            return Err((400, "Body ended early"));
        }
        body.extend_from_slice(chunk.get(..read).unwrap_or_default());
        if body.len() > length {
            return Err((400, "Body is longer than Content-Length"));
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == HEALTH_PATH && method == Method::Get {
        return Ok(Parsed::Health);
    }
    let nsid = path
        .strip_prefix(XRPC_PREFIX)
        .ok_or((404, "Not an XRPC path"))?;
    if nsid.is_empty()
        || !nsid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.')
    {
        return Err((400, "Malformed method NSID"));
    }
    let query = parse_query(query).ok_or((400, "Malformed query string"))?;
    Ok(Parsed::Xrpc(XrpcRequest {
        method,
        nsid: nsid.to_string(),
        query,
        authorization,
        body,
    }))
}

/// `application/x-www-form-urlencoded` query decoding.
#[must_use]
pub fn parse_query(query: &str) -> Option<Vec<(String, String)>> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((decode_component(name)?, decode_component(value)?))
        })
        .collect()
}

fn decode_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'%' => {
                let high = hex(*bytes.get(index + 1)?)?;
                let low = hex(*bytes.get(index + 2)?)?;
                out.push(high * 16 + low);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            other => {
                out.push(*other);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
