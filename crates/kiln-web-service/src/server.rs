//! Bounded HTTPS shell. Authorization precedes every application call.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use rustls::{ServerConfig, ServerConnection, StreamOwned};
use rustls_pemfile::{certs, private_key};

use crate::access::{AccessProfile, IdentityResolver, Principal};
use crate::auth::Credentials;
use crate::engine::{ApiResponse, Engine};
use crate::profile::ServiceProfile;
use zeroize::Zeroizing;

/// Explicit runtime policies, never part of the canonical ServiceProfile identity.
#[derive(Debug, Default)]
pub struct Options {
    pub access: Option<PathBuf>,
    pub automation: Option<PathBuf>,
    pub cobs: Option<PathBuf>,
    pub sources: Option<PathBuf>,
    pub reviews: Option<PathBuf>,
    pub cache: Option<PathBuf>,
    pub workers: Option<PathBuf>,
    pub work: Option<PathBuf>,
    pub code_review: Option<PathBuf>,
}

impl Options {
    fn access_profile(&self) -> Result<AccessProfile, String> {
        self.access
            .as_deref()
            .map(AccessProfile::load)
            .unwrap_or_else(|| Ok(AccessProfile::default()))
    }
    fn automation_profile(&self) -> Result<Option<crate::automation::AutomationProfile>, String> {
        self.automation
            .as_deref()
            .map(crate::automation::AutomationProfile::load)
            .transpose()
    }
}

#[path = "server/admission.rs"]
mod admission;
#[path = "server/notify.rs"]
mod notify;

// r[impl kiln.web.deploy.runtime]
/// Admit startup inputs without creating state, binding listeners or contacting providers.
pub fn check(path: &Path) -> Result<(), String> {
    check_with_options(path, &Options::default())
}

pub fn check_with_options(path: &Path, options: &Options) -> Result<(), String> {
    let profile = ServiceProfile::load(path)?;
    let _credentials = Credentials::load(&profile, options.access_profile()?)?;
    let _automation = options.automation_profile()?;
    let _cobs = options
        .cobs
        .as_deref()
        .map(crate::cobs::Profile::load)
        .transpose()?;
    let _sources = options
        .sources
        .as_deref()
        .map(crate::sources::Profile::load)
        .transpose()?;
    let _reviews = options
        .reviews
        .as_deref()
        .map(crate::reviews::Profile::load)
        .transpose()?;
    let _cache = options
        .cache
        .as_deref()
        .map(crate::cache::CacheProfile::load)
        .transpose()?;
    let _workers = options
        .workers
        .as_deref()
        .map(crate::engine::WorkerStatusProfile::load)
        .transpose()?;
    let _work = crate::work::Workspace::new(
        options
            .work
            .as_deref()
            .map(crate::work::Profile::load)
            .transpose()?,
    )?;
    let review_profile = options
        .code_review
        .as_deref()
        .map(crate::code_review::Profile::load)
        .transpose()?;
    if let Some(review_profile) = &review_profile {
        review_profile.admit_service(&profile)?;
    }
    let _code_review = crate::code_review::Workspace::new(review_profile)?;
    if let Some(cache) = &_cache {
        cache.preflight_for(&profile)?;
    }
    if let Some(workers) = &_workers {
        workers.preflight_for(&profile)?;
    }
    let _tls = tls_config(&profile)?;
    admission::runtime_inputs(&profile)
}

const ACCEPT_INTERVAL: Duration = Duration::from_millis(25);
const MAX_HEADER_FIELDS: usize = 64;
const SECURITY_HEADERS: &str = "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; frame-ancestors 'none'; form-action 'none'\r\nStrict-Transport-Security: max-age=31536000\r\n";

pub fn run(path: &Path) -> Result<(), String> {
    run_with_options(path, &Options::default())
}

pub fn run_with_options(path: &Path, options: &Options) -> Result<(), String> {
    let profile = ServiceProfile::load(path)?;
    let access = options.access_profile()?;
    let access_identity = access.identity()?;
    let credentials = Credentials::load(&profile, access)?;
    let automation = options.automation_profile()?;
    let cache = options
        .cache
        .as_deref()
        .map(crate::cache::CacheProfile::load)
        .transpose()?;
    let workers = options
        .workers
        .as_deref()
        .map(crate::engine::WorkerStatusProfile::load)
        .transpose()?;
    let cobs = crate::cobs::Browser::new(
        options
            .cobs
            .as_deref()
            .map(crate::cobs::Profile::load)
            .transpose()?,
    )?;
    let sources = crate::sources::Browser::new(
        options
            .sources
            .as_deref()
            .map(crate::sources::Profile::load)
            .transpose()?,
    )?;
    let reviews = crate::reviews::Browser::new(
        options
            .reviews
            .as_deref()
            .map(crate::reviews::Profile::load)
            .transpose()?,
    )?;
    let work = crate::work::Workspace::new(
        options
            .work
            .as_deref()
            .map(crate::work::Profile::load)
            .transpose()?,
    )?;
    let review_profile = options
        .code_review
        .as_deref()
        .map(crate::code_review::Profile::load)
        .transpose()?;
    if let Some(review_profile) = &review_profile {
        review_profile.admit_service(&profile)?;
    }
    let code_review = crate::code_review::Workspace::new(review_profile)?;
    let tls = tls_config(&profile)?;
    admission::runtime_inputs(&profile)?;
    let listener = TcpListener::bind(profile.listen_address)
        .map_err(|_| "listener could not bind".to_owned())?;
    let mut engine = Engine::open(profile.clone())?;
    engine.configure_access_identity(&access_identity)?;
    if let Some(automation) = automation {
        engine.configure_automation(automation)?;
    }
    if let Some(cache) = cache {
        engine.install_cache(cache)?;
    }
    if let Some(workers) = workers {
        engine.configure_worker_status(workers)?;
    }
    let engine = Mutex::new(engine);
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&stop))
            .map_err(|_| "shutdown handler could not register".to_owned())?;
    }
    listener
        .set_nonblocking(true)
        .map_err(|_| "listener mode failed".to_owned())?;
    let active = AtomicUsize::new(0);
    let poll_failed = AtomicBool::new(false);
    let context = Context {
        profile: &profile,
        credentials: &credentials,
        engine: &engine,
        cobs: &cobs,
        sources: &sources,
        reviews: &reviews,
        work: &work,
        code_review: &code_review,
    };
    // r[impl kiln.web.deploy.runtime] Readiness follows TLS, listener and canonical state initialization.
    notify::ready()?;
    std::thread::scope(|scope| -> Result<(), String> {
        let stop_ref = &stop;
        let failed_ref = &poll_failed;
        let engine_ref = &engine;
        let review_ref = &code_review;
        let polling = scope.spawn(move || {
            while !stop_ref.load(Ordering::Acquire) {
                let result = (|| -> Result<(), ()> {
                    let now = crate::engine::host_time_ms().map_err(|_| ())?;
                    let now = i64::try_from(now).map_err(|_| ())?;
                    let notifications = {
                        let mut engine = engine_ref.lock().map_err(|_| ())?;
                        let notifications = engine.poll().map_err(|_| ())?;
                        review_ref.advance_queue(&mut engine, now).map_err(|_| ())?;
                        notifications
                    };
                    review_ref.deliver_notifications(now).map_err(|_| ())?;
                    // One bounded serial executor: no HTTPS runs under Engine's
                    // mutex and no detached deliveries accumulate across polls.
                    for notification in notifications {
                        let observation = notification.deliver().map_err(|_| ())?;
                        engine_ref
                            .lock()
                            .map_err(|_| ())?
                            .record_notification(observation)
                            .map_err(|_| ())?;
                    }
                    Ok(())
                })();
                if result.is_err() {
                    failed_ref.store(true, Ordering::Release);
                    stop_ref.store(true, Ordering::Release);
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(profile.bounds.poll_interval_ms));
            }
        });
        println!("kiln-web-service: listening {}", profile.origin);
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if active
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                            (count < profile.bounds.max_connections).then_some(count + 1)
                        })
                        .is_err()
                    {
                        continue;
                    }
                    let permit = Permit(&active);
                    let tls = Arc::clone(&tls);
                    let context = &context;
                    if std::thread::Builder::new()
                        .spawn_scoped(scope, move || {
                            let _permit = permit;
                            // Malformed/closed/expired client connections have no application effect.
                            let _ = connection(stream, tls, context);
                        })
                        .is_err()
                    {
                        stop.store(true, Ordering::Release);
                        polling.thread().unpark();
                        return Err("connection worker could not start".to_owned());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_INTERVAL)
                }
                Err(_) => {
                    stop.store(true, Ordering::Release);
                    polling.thread().unpark();
                    return Err("listener failed".to_owned());
                }
            }
        }
        polling.thread().unpark();
        Ok(())
    })?;
    if poll_failed.load(Ordering::Acquire) {
        Err("canonical workflow reconciliation failed; service stopped".to_owned())
    } else {
        Ok(())
    }
}

// r[impl kiln.web.network.boundaries]
/// Private nginx auth_request endpoint. Its credential is never returned to a browser.
/// The trusted nginx caller must overwrite X-Kiln-Peer from the TCP peer socket.
pub fn proxy_auth(path: &Path) -> Result<(), String> {
    let access = AccessProfile::load(path)?;
    let profile = access
        .proxy
        .as_ref()
        .ok_or("proxy identity policy is not configured")?;
    let secret = Zeroizing::new(crate::profile::read_token(&profile.credential_file)?);
    let verifier = crate::auth::TokenVerifier::new(&secret)?;
    let resolver = IdentityResolver::new(profile)?;
    let listener = TcpListener::bind(profile.listen_address)
        .map_err(|_| "private proxy listener unavailable")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "private proxy listener mode failed")?;
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&stop))
            .map_err(|_| "proxy shutdown handler unavailable")?;
    }
    notify::ready()?;
    while !stop.load(Ordering::Acquire) {
        let stream = match listener.accept() {
            Ok((stream, peer)) if peer.ip().is_loopback() => stream,
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_INTERVAL);
                continue;
            }
            Err(_) => return Err("private proxy listener failed".into()),
        };
        let mut stream = DeadlineStream {
            stream,
            deadline: Instant::now() + Duration::from_secs(5),
        };
        let request = read_request(&mut BufReader::new(&mut stream), 8192, 0);
        let principal = request
            .ok()
            .filter(|request| {
                request.method == "GET"
                    && request.target == "/verify"
                    && header(request, "host") == Some("kiln-proxy-auth")
                    && header(request, "authorization").is_none()
                    && header(request, "origin").is_none()
                    && header(request, "x-kiln-principal").is_none()
                    && header(request, "x-kiln-proxy-key")
                        .is_some_and(|key| key.len() == 64 && verifier.matches(key.as_bytes()))
            })
            .and_then(|request| {
                header(&request, "x-kiln-peer")
                    .and_then(|peer| resolver.resolve(profile, peer).ok())
            });
        // Never put labels, roles, credentials or error details in the auth response.
        if let Some(principal) = principal {
            let _ = write!(
                stream,
                "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\nCache-Control: no-store\r\nX-Kiln-Principal: {}\r\n\r\n",
                principal.id
            );
            let _ = stream.flush();
        } else {
            let _ = write_response(&mut stream, error_response(403, "peer_denied"));
        }
    }
    Ok(())
}

struct Permit<'a>(&'a AtomicUsize);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Context<'a> {
    profile: &'a ServiceProfile,
    credentials: &'a Credentials,
    engine: &'a Mutex<Engine>,
    cobs: &'a crate::cobs::Browser,
    sources: &'a crate::sources::Browser,
    reviews: &'a crate::reviews::Browser,
    work: &'a crate::work::Workspace,
    code_review: &'a crate::code_review::Workspace,
}

fn tls_config(profile: &ServiceProfile) -> Result<Arc<ServerConfig>, String> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    const MAX_TLS_BYTES: usize = 1_048_576;
    let certificate = crate::worker::files::regular(
        Path::new(&profile.tls.certificate_path),
        MAX_TLS_BYTES,
        false,
    )
    .map_err(|_| "TLS certificate unavailable or unsafe".to_owned())?;
    let certificates = certs(&mut BufReader::new(certificate.as_slice()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "TLS certificate malformed".to_owned())?;
    if certificates.is_empty() {
        return Err("TLS certificate empty".to_owned());
    }
    let key_bytes = Zeroizing::new(
        crate::worker::files::regular(
            Path::new(&profile.tls.private_key_path),
            MAX_TLS_BYTES,
            true,
        )
        .map_err(|_| "TLS key unavailable or unsafe".to_owned())?,
    );
    let key = private_key(&mut BufReader::new(key_bytes.as_slice()))
        .map_err(|_| "TLS key malformed".to_owned())?
        .ok_or_else(|| "TLS key empty".to_owned())?;
    let config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(|_| "TLS configuration invalid".to_owned())?;
    Ok(Arc::new(config))
}

fn connection(
    stream: TcpStream,
    tls: Arc<ServerConfig>,
    context: &Context<'_>,
) -> std::io::Result<()> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(context.profile.bounds.timeout_seconds))
        .ok_or_else(|| std::io::Error::other("connection deadline invalid"))?;
    let stream = DeadlineStream { stream, deadline };
    let connection = ServerConnection::new(tls).map_err(std::io::Error::other)?;
    let mut stream = StreamOwned::new(connection, stream);
    let response = match read_request(
        &mut BufReader::new(&mut stream),
        context.profile.bounds.max_header_bytes,
        context.profile.bounds.max_request_bytes,
    ) {
        Ok(request) => route(request, context),
        Err(status) => error_response(status, "request_rejected"),
    };
    write_response(&mut stream, response)
}

struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
}

impl DeadlineStream {
    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "connection deadline elapsed")
            })
    }
}

impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(bytes)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.flush()
    }
}

// Headers remain private, are bounded by the framing reader, and are never logged.
struct Request {
    method: String,
    target: String,
    body: Vec<u8>,
    headers: BTreeMap<String, String>,
}

fn line<R: BufRead>(reader: &mut R, remaining: &mut usize) -> Result<Vec<u8>, u16> {
    let mut bytes = Vec::new();
    let count = reader
        .take((*remaining as u64).saturating_add(1))
        .read_until(b'\n', &mut bytes)
        .map_err(|_| 400_u16)?;
    if count > *remaining {
        return Err(431);
    }
    *remaining -= count;
    if !bytes.ends_with(b"\r\n") {
        return Err(400);
    }
    bytes.truncate(bytes.len() - 2);
    if !bytes.iter().all(|byte| (b' '..=b'~').contains(byte)) {
        return Err(400);
    }
    Ok(bytes)
}

fn read_request<R: BufRead>(
    reader: &mut R,
    max_header: usize,
    max_body: usize,
) -> Result<Request, u16> {
    let mut remaining = max_header;
    let first = line(reader, &mut remaining)?;
    let first = std::str::from_utf8(&first).map_err(|_| 400_u16)?;
    let mut parts = first.split(' ');
    let method = parts.next().ok_or(400_u16)?;
    let target = parts.next().ok_or(400_u16)?;
    if parts.next() != Some("HTTP/1.1")
        || parts.next().is_some()
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.contains('#')
    {
        return Err(400);
    }
    if !matches!(method, "GET" | "POST") {
        return Err(405);
    }
    let mut request = Request {
        method: method.to_owned(),
        target: target.to_owned(),
        body: Vec::new(),
        headers: BTreeMap::new(),
    };
    let mut length = None;
    let mut ended = false;
    for _ in 0..MAX_HEADER_FIELDS {
        let bytes = line(reader, &mut remaining)?;
        if bytes.is_empty() {
            ended = true;
            break;
        }
        let header = std::str::from_utf8(&bytes).map_err(|_| 400_u16)?;
        let (name, value) = header.split_once(':').ok_or(400_u16)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        {
            return Err(400);
        }
        let value = value.trim_matches(' ');
        let name = name.to_ascii_lowercase();
        if request.headers.contains_key(&name) {
            return Err(400);
        }
        match name.as_str() {
            "content-length" if length.is_none() => {
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(400);
                }
                let size = value.parse::<usize>().map_err(|_| 413_u16)?;
                if size > max_body {
                    return Err(413);
                }
                length = Some(size);
            }
            "transfer-encoding" | "expect" => return Err(400),
            _ => {}
        }
        request.headers.insert(name, value.to_owned());
    }
    if !ended {
        return Err(431);
    }
    if header(&request, "host").is_none_or(str::is_empty) {
        return Err(400);
    }
    if request.method == "GET" && length.unwrap_or(0) != 0 {
        return Err(400);
    }
    if request.method == "POST" && length.is_none() {
        return Err(411);
    }
    request.body.resize(length.unwrap_or(0), 0);
    reader.read_exact(&mut request.body).map_err(|_| 400_u16)?;
    Ok(request)
}

// r[impl kiln.web.authentication]
fn route(request: Request, context: &Context<'_>) -> ApiResponse {
    let principal = context.credentials.authenticate_request(
        header(&request, "authorization"),
        header(&request, "x-kiln-proxy-key"),
        header(&request, "x-kiln-principal"),
    );
    let host = context
        .profile
        .origin
        .strip_prefix("https://")
        .unwrap_or("");
    if header(&request, "host") != Some(host)
        || header(&request, "origin").is_some_and(|origin| origin != context.profile.origin)
    {
        return denied(&request, context, principal.as_ref(), 403, "origin_denied");
    }
    if forbidden_identity(&request) {
        return denied(&request, context, None, 403, "identity_header_denied");
    }
    if principal.is_none()
        && (header(&request, "x-kiln-proxy-key").is_some()
            || header(&request, "x-kiln-principal").is_some())
    {
        return denied(&request, context, None, 401, "proxy_identity_denied");
    }
    if let Some(id) = trigger_id(&request) {
        // Only this exact signed intake has an Origin/bearer exception. Authentication,
        // repository/ref binding and replay admission belong to the registered trigger.
        if header(&request, "authorization").is_some()
            || header(&request, "x-kiln-proxy-key").is_some()
            || header(&request, "x-kiln-principal").is_some()
        {
            return denied(
                &request,
                context,
                principal.as_ref(),
                403,
                "trigger_identity_denied",
            );
        }
        if !json_content(&request) {
            return denied(&request, context, None, 415, "json_required");
        }
        let (Some(delivery), Some(event), Some(signature)) = (
            header(&request, "x-github-delivery"),
            header(&request, "x-github-event"),
            header(&request, "x-hub-signature-256"),
        ) else {
            return denied(&request, context, None, 401, "signature_required");
        };
        let headers = kiln_adapter_github::events::DeliveryHeaders {
            delivery: delivery.to_owned(),
            event: event.to_owned(),
            signature: signature.to_owned(),
        };
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .automation_trigger(id, &headers, &request.body)
                .unwrap_or_else(|_| error_response(503, "service_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.target.starts_with("/api/") {
        if let Err((status, code)) =
            api_admission(&request, &context.profile.origin, principal.as_ref())
        {
            return denied(&request, context, principal.as_ref(), status, code);
        }
        let Some(principal) = principal else {
            return error_response(401, "unauthorized");
        };
        let path = request
            .target
            .split_once('?')
            .map_or(request.target.as_str(), |(path, _)| path);
        // r[impl kiln.review.authority]
        if path == "/api/code-review" || path.starts_with("/api/code-review/") {
            let now = match crate::engine::host_time_ms().and_then(|value| {
                i64::try_from(value).map_err(|_| "review clock exceeds bound".to_owned())
            }) {
                Ok(value) => value,
                Err(_) => return error_response(503, "clock_unavailable"),
            };
            // Hold canonical CI state stable through admission and the provider CAS.
            // The review workspace never locks Engine, so this lock order is one-way.
            let engine = match context.engine.lock() {
                Ok(engine) => engine,
                Err(_) => return error_response(503, "service_unavailable"),
            };
            let ci = engine.code_review_ci();
            return context.code_review.response(
                &request.method,
                &request.target,
                &request.body,
                crate::code_review::RequestContext {
                    role: principal.role,
                    principal: &principal.id,
                    now_ms: now,
                    ci: &ci,
                },
            );
        }
        // r[impl kiln.work.authority]
        if path == "/api/work" || path.starts_with("/api/work/") {
            let now = match crate::engine::host_time_ms().and_then(|value| {
                i64::try_from(value).map_err(|_| "work clock exceeds bound".to_owned())
            }) {
                Ok(value) => value,
                Err(_) => return error_response(503, "clock_unavailable"),
            };
            return context.work.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
                &principal.id,
                now,
            );
        }
        if path == "/api/reviews" || path.starts_with("/api/reviews/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(
                    &request,
                    context,
                    Some(&principal),
                    403,
                    "reviews_read_only",
                );
            }
            return context.reviews.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        if path == "/api/sources" || path.starts_with("/api/sources/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(
                    &request,
                    context,
                    Some(&principal),
                    403,
                    "sources_read_only",
                );
            }
            return context.sources.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        if path == "/api/cobs" || path.starts_with("/api/cobs/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(&request, context, Some(&principal), 403, "cobs_read_only");
            }
            return context.cobs.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        match context.engine.lock() {
            Ok(mut engine) => engine
                .api_as(&request.method, &request.target, &request.body, &principal)
                .unwrap_or_else(|_| error_response(503, "service_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        }
    } else if request.method == "GET" {
        let path = request
            .target
            .split_once('?')
            .map_or(request.target.as_str(), |(path, _)| path);
        // Compiled UI entrypoint; interaction logic is embedded from web.js.
        // r[impl kiln.dashboard.cobs.view]
        // r[impl kiln.dashboard.cobs.view.scenario.restore]
        // r[impl kiln.dashboard.cobs.view.scenario.cleared]
        let (content_type, bytes): (&'static str, &[u8]) = match path {
            "/" => ("text/html; charset=utf-8", include_bytes!("web.html")),
            "/web.js" => ("text/javascript; charset=utf-8", include_bytes!("web.js")),
            "/web.css" => ("text/css; charset=utf-8", include_bytes!("web.css")),
            "/work.js" => ("text/javascript; charset=utf-8", include_bytes!("work.js")),
            "/work.css" => ("text/css; charset=utf-8", include_bytes!("work.css")),
            "/code-review.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("code-review.js"),
            ),
            "/code-review.css" => ("text/css; charset=utf-8", include_bytes!("code-review.css")),
            "/work-interaction.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("work-interaction.js"),
            ),
            "/work-interaction.css" => (
                "text/css; charset=utf-8",
                include_bytes!("work-interaction.css"),
            ),
            "/work-evidence.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("work-evidence.js"),
            ),
            // r[impl kiln.web.deploy.runtime] Minimal health exposes no private state.
            "/healthz" => ("application/json", b"{\"status\":\"ok\"}"),
            "/catalog.css" => (
                "text/css; charset=utf-8",
                include_bytes!("../../kiln-dashboard/src/render/catalog.css"),
            ),
            _ => return error_response(404, "not_found"),
        };
        ApiResponse {
            status: 200,
            content_type,
            body: bytes.to_vec(),
            download_name: None,
        }
    } else {
        error_response(405, "method_denied")
    }
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers.get(name).map(String::as_str)
}

fn forbidden_identity(request: &Request) -> bool {
    request.headers.keys().any(|name| {
        (name.starts_with("x-kiln-")
            && !matches!(name.as_str(), "x-kiln-proxy-key" | "x-kiln-principal"))
            || name.starts_with("tailscale-")
            || matches!(
                name.as_str(),
                "x-forwarded-user" | "x-auth-request-user" | "remote-user"
            )
    })
}

fn trigger_id(request: &Request) -> Option<&str> {
    if request.method != "POST" {
        return None;
    }
    request
        .target
        .strip_prefix("/api/triggers/")
        .filter(|id| crate::profile::identifier(id))
}

fn json_content(request: &Request) -> bool {
    matches!(
        header(request, "content-type"),
        Some("application/json" | "application/json; charset=utf-8")
    )
}

// r[impl kiln.web.authentication] r[impl kiln.web.network.boundaries]
fn api_admission(
    request: &Request,
    origin: &str,
    principal: Option<&Principal>,
) -> Result<(), (u16, &'static str)> {
    if principal.is_none() {
        return Err((401, "unauthorized"));
    }
    if request.method == "POST" {
        if header(request, "origin") != Some(origin) {
            return Err((403, "origin_required"));
        }
        if !json_content(request) {
            return Err((415, "json_required"));
        }
        if principal.is_some_and(|principal| principal.role == crate::engine::Role::Viewer)
            && request.target != "/api/work/commands"
            && request.target != "/api/code-review/commands"
        {
            return Err((403, "operator_required"));
        }
    }
    Ok(())
}

fn denied(
    request: &Request,
    context: &Context<'_>,
    principal: Option<&Principal>,
    status: u16,
    code: &str,
) -> ApiResponse {
    match context.engine.lock() {
        Ok(mut engine) => {
            match engine.record_denial(&request.method, &request.target, principal, code) {
                Ok(()) => error_response(status, code),
                Err(_) => error_response(503, "audit_unavailable"),
            }
        }
        Err(_) => error_response(503, "service_unavailable"),
    }
}

fn error_response(status: u16, code: &str) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: format!("{{\"code\":\"{code}\",\"message\":\"Request could not be admitted\"}}")
            .into_bytes(),
        download_name: None,
    }
}

fn write_response<W: Write>(writer: &mut W, response: ApiResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Response",
    };
    write!(
        writer,
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{SECURITY_HEADERS}",
        response.status,
        response.content_type,
        response.body.len()
    )?;
    if let Some(name) = response.download_name {
        // The engine supplies a display name, never an HTTP header fragment.
        let safe: String = name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        write!(
            writer,
            "Content-Disposition: attachment; filename=\"{safe}\"\r\n"
        )?;
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(&response.body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> Result<Request, u16> {
        read_request(&mut std::io::Cursor::new(bytes), 1024, 16)
    }

    // r[verify kiln.web.authentication.scenario.denied]
    #[test]
    fn framing_accepts_one_bounded_json_request_and_rejects_ambiguous_bodies() {
        let request = parse(b"POST /api/projects HTTP/1.1\r\nHost: localhost:8443\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}").unwrap_or_else(|_| unreachable!());
        assert_eq!(request.body, b"{}");
        for bytes in [
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"GET /api/runs HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer a\r\nAuthorization: Bearer b\r\n\r\n",
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\nx",
            b"GET / HTTP/1.1\r\nHost: localhost\r\n folded: value\r\n\r\n",
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: +2\r\n\r\n{}",
            b"GET /api/session HTTP/1.1\r\nHost: localhost\r\nX-Kiln-Principal: tailscale:1\r\nx-kiln-principal: tailscale:2\r\n\r\n",
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nX-GitHub-Delivery: first\r\nX-GitHub-Delivery: second\r\nContent-Length: 2\r\n\r\n{}",
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nX-Hub-Signature-256: first\r\nx-hub-signature-256: second\r\nContent-Length: 2\r\n\r\n{}",
        ] { assert!(parse(bytes).is_err()); }
        assert_eq!(
            parse(b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: 17\r\n\r\n")
                .err(),
            Some(413)
        );
        assert_eq!(
            read_request(
                &mut std::io::Cursor::new(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                16,
                16
            )
            .err(),
            Some(431)
        );
    }

    // r[verify kiln.web.authentication.scenario.operator] r[verify kiln.web.authentication.scenario.denied]
    // r[verify kiln.web.network.boundaries]
    #[test]
    fn viewer_reads_but_never_mutates_and_writes_require_exact_origin() {
        let viewer = Principal {
            id: "viewer:reader".into(),
            label: "Reader".into(),
            role: crate::engine::Role::Viewer,
        };
        let operator = Principal {
            role: crate::engine::Role::Operator,
            ..viewer.clone()
        };
        let read = parse(b"GET /api/runs HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&read, "https://localhost", Some(&viewer)),
            Ok(())
        );
        assert_eq!(
            api_admission(&read, "https://localhost", None),
            Err((401, "unauthorized"))
        );
        let mut write = parse(b"POST /api/runs HTTP/1.1\r\nHost: localhost\r\nOrigin: https://localhost\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}").unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Ok(())
        );
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&viewer)),
            Err((403, "operator_required"))
        );
        write.headers.remove("origin");
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Err((403, "origin_required"))
        );
        write
            .headers
            .insert("origin".into(), "https://foreign.example".into());
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Err((403, "origin_required"))
        );
    }

    #[test]
    fn review_collaboration_does_not_grant_ci_operator_or_cross_origin_access() {
        let reviewer = Principal {
            id: "viewer:reviewer".into(),
            label: "Reviewer".into(),
            role: crate::engine::Role::Viewer,
        };
        let mut request = parse(b"POST /api/code-review/commands HTTP/1.1\r\nHost: localhost\r\nOrigin: https://localhost\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&request, "https://localhost", Some(&reviewer)),
            Ok(())
        );
        for target in [
            "/api/runs",
            "/api/projects",
            "/api/code-review/commands?bypass=1",
            "/api/code-review/commands/extra",
        ] {
            request.target = target.into();
            assert_eq!(
                api_admission(&request, "https://localhost", Some(&reviewer)),
                Err((403, "operator_required"))
            );
        }
        request.target = "/api/code-review/commands".into();
        request
            .headers
            .insert("origin".into(), "https://foreign.example".into());
        assert_eq!(
            api_admission(&request, "https://localhost", Some(&reviewer)),
            Err((403, "origin_required"))
        );
        assert_eq!(
            api_admission(&request, "https://localhost", None),
            Err((401, "unauthorized"))
        );
    }

    #[test]
    fn only_exact_registered_trigger_path_can_enter_signed_ingress() {
        let raw =
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}";
        let mut request = parse(raw).unwrap_or_else(|_| unreachable!());
        assert_eq!(trigger_id(&request), Some("github"));
        for target in [
            "/api/triggers/",
            "/api/triggers/github/extra",
            "/api/triggers/github?bypass=1",
            "/api/runs",
        ] {
            request.target = target.into();
            assert_eq!(trigger_id(&request), None);
        }
        request.target = "/api/triggers/github".into();
        request.method = "GET".into();
        assert_eq!(trigger_id(&request), None);
        request
            .headers
            .insert("x-kiln-role".into(), "operator".into());
        assert!(forbidden_identity(&request));
    }
}
