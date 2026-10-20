use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kiln_core::domain::State;
use kiln_observe_core::{
    EventWindow, LogWindow, ObservationRequest, ObservationWindow, ViewRequest, project_observation,
};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use rustls_pemfile::{certs, private_key};

use crate::auth::{AuthRequest, ReplayState, parse_principal, verify_auth_header};
use crate::live::{LiveEventBuffer, LiveFrame, sse_frame};
use crate::profile::HttpObservationProfile;
use crate::request::{HttpRequest, map_live_request, map_view_request, read_request};
use crate::response::{write_error, write_sse_headers, write_view};
use crate::{HttpError, ObserveError, ObserveResult};

#[derive(Clone, Debug)]
pub struct ObservationSnapshot {
    pub state: State,
    pub events: Option<EventWindow>,
    pub log: Option<LogWindow>,
}

impl ObservationSnapshot {
    #[must_use]
    pub fn new(state: State) -> Self {
        Self {
            state,
            events: None,
            log: None,
        }
    }

    #[must_use]
    pub fn with_events(mut self, events: EventWindow) -> Self {
        self.events = Some(events);
        self
    }

    #[must_use]
    pub fn with_log(mut self, log: LogWindow) -> Self {
        self.log = Some(log);
        self
    }
}

pub trait ObservationSource: Send + Sync {
    fn snapshot(&self) -> ObservationSnapshot;
}

#[derive(Debug)]
pub struct MemoryObservationSource {
    snapshot: RwLock<ObservationSnapshot>,
}

impl MemoryObservationSource {
    #[must_use]
    pub fn new(snapshot: ObservationSnapshot) -> Self {
        Self {
            snapshot: RwLock::new(snapshot),
        }
    }

    pub fn update(&self, snapshot: ObservationSnapshot) {
        match self.snapshot.write() {
            Ok(mut guard) => *guard = snapshot,
            Err(poisoned) => *poisoned.into_inner() = snapshot,
        }
    }
}

impl ObservationSource for MemoryObservationSource {
    fn snapshot(&self) -> ObservationSnapshot {
        match self.snapshot.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

pub trait ObservationClock: Send + Sync {
    fn now_unix(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl ObservationClock for SystemClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }
}

// r[impl kiln.observe.http.serve]
pub fn serve(
    profile: HttpObservationProfile,
    source: Arc<dyn ObservationSource>,
    live: Arc<LiveEventBuffer>,
) -> ObserveResult<()> {
    profile.admit()?;
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop))
        .map_err(|_| ObserveError::new("signal", "shutdown signal registration failed"))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop))
        .map_err(|_| ObserveError::new("signal", "shutdown signal registration failed"))?;
    let listener = TcpListener::bind((profile.origin.host.as_str(), profile.origin.port))
        .map_err(|_| ObserveError::new("listen", "observation listener could not bind"))?;
    serve_on(listener, profile, source, live, Arc::new(SystemClock), stop)
}

pub fn serve_on(
    listener: TcpListener,
    profile: HttpObservationProfile,
    source: Arc<dyn ObservationSource>,
    live: Arc<LiveEventBuffer>,
    clock: Arc<dyn ObservationClock>,
    stop: Arc<AtomicBool>,
) -> ObserveResult<()> {
    profile.admit()?;
    ensure_crypto_provider();
    let tls = load_tls_config(&profile)?;
    let max_connections = usize::try_from(profile.bounds.max_connections).unwrap_or(1);
    let replay = Arc::new(Mutex::new(ReplayState::new(
        profile.auth.replay_window_seconds,
        usize::try_from(profile.auth.max_replay_entries).unwrap_or(1),
    )));
    listener
        .set_nonblocking(true)
        .map_err(|_| ObserveError::new("listen", "listener could not enter bounded accept mode"))?;
    let (sender, receiver) = std::sync::mpsc::sync_channel::<TcpStream>(max_connections);
    let receiver = Arc::new(Mutex::new(receiver));
    std::thread::scope(|scope| -> ObserveResult<()> {
        for _ in 0..max_connections {
            let receiver = Arc::clone(&receiver);
            let tls = Arc::clone(&tls);
            let replay = Arc::clone(&replay);
            let source = Arc::clone(&source);
            let live = Arc::clone(&live);
            let clock = Arc::clone(&clock);
            let stop = Arc::clone(&stop);
            let profile = &profile;
            scope.spawn(move || {
                loop {
                    let stream = match receiver.lock() {
                        Ok(receiver) => receiver.recv(),
                        Err(_) => break,
                    };
                    let Ok(stream) = stream else {
                        break;
                    };
                    let context = ConnectionContext {
                        profile,
                        source: source.as_ref(),
                        live: live.as_ref(),
                        replay: replay.as_ref(),
                        clock: clock.as_ref(),
                        stop: stop.as_ref(),
                    };
                    if let Err(error) = handle_tls(stream, Arc::clone(&tls), &context) {
                        eprintln!("http observation: {}: {}", error.code, error.message);
                    }
                }
            });
        }
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _address)) => match sender.try_send(stream) {
                    Ok(()) => {}
                    Err(std::sync::mpsc::TrySendError::Full(_)) => {}
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
                },
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    return Err(ObserveError::new(
                        "accept",
                        "observation listener accept failed",
                    ));
                }
            }
        }
        drop(sender);
        Ok(())
    })
}

pub fn load_tls_config(profile: &HttpObservationProfile) -> ObserveResult<Arc<ServerConfig>> {
    ensure_crypto_provider();
    let cert_file = File::open(&profile.tls.certificate_path).map_err(|_| {
        ObserveError::new(
            "tls_certificate",
            "TLS certificate file could not be opened",
        )
    })?;
    let certificates = certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ObserveError::new("tls_certificate", "TLS certificate file is malformed"))?;
    if certificates.is_empty() {
        return Err(ObserveError::new(
            "tls_certificate",
            "TLS certificate file is empty",
        ));
    }
    let key_file = File::open(&profile.tls.private_key_path)
        .map_err(|_| ObserveError::new("tls_key", "TLS private key file could not be opened"))?;
    let key = private_key(&mut BufReader::new(key_file))
        .map_err(|_| ObserveError::new("tls_key", "TLS private key file is malformed"))?
        .ok_or_else(|| ObserveError::new("tls_key", "TLS private key file is empty"))?;
    let versions: &[&rustls::SupportedProtocolVersion] = if profile.tls.min_version == "tls13" {
        &[&rustls::version::TLS13]
    } else {
        &[&rustls::version::TLS12, &rustls::version::TLS13]
    };
    let config = ServerConfig::builder_with_protocol_versions(versions)
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(|_| ObserveError::new("tls_config", "TLS server profile is invalid"))?;
    Ok(Arc::new(config))
}

fn ensure_crypto_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    });
}

struct ConnectionContext<'a> {
    profile: &'a HttpObservationProfile,
    source: &'a dyn ObservationSource,
    live: &'a LiveEventBuffer,
    replay: &'a Mutex<ReplayState>,
    clock: &'a dyn ObservationClock,
    stop: &'a AtomicBool,
}

fn handle_tls(
    stream: TcpStream,
    tls: Arc<ServerConfig>,
    context: &ConnectionContext<'_>,
) -> ObserveResult<()> {
    let read_timeout = Duration::from_millis(context.profile.bounds.read_timeout_milliseconds);
    let write_timeout = Duration::from_millis(context.profile.bounds.write_timeout_milliseconds);
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|_| ObserveError::new("timeout", "connection read timeout could not be set"))?;
    stream
        .set_write_timeout(Some(write_timeout))
        .map_err(|_| ObserveError::new("timeout", "connection write timeout could not be set"))?;
    let connection = ServerConnection::new(tls)
        .map_err(|_| ObserveError::new("tls_connection", "TLS connection could not start"))?;
    let mut stream = StreamOwned::new(connection, stream);
    handle_request(&mut stream, context)
}

fn handle_request<S: Read + Write>(
    stream: &mut S,
    context: &ConnectionContext<'_>,
) -> ObserveResult<()> {
    let profile = context.profile;
    let source = context.source;
    let live = context.live;
    let replay = context.replay;
    let clock = context.clock;
    let stop = context.stop;
    let max_error = usize::try_from(profile.bounds.max_error_bytes).unwrap_or(usize::MAX);
    let http_request = match read_request(stream, profile) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_error(stream, error, max_error);
            return Ok(());
        }
    };
    let snapshot = source.snapshot();
    let live_request = http_request.path == "/v1/live";
    let route = if live_request {
        match map_live_request(&http_request, profile) {
            Ok((after, limit)) => LiveOrView::Live { after, limit },
            Err(error) => {
                let _ = write_error(stream, error, max_error);
                return Ok(());
            }
        }
    } else {
        match map_view_request(&http_request, snapshot.state.revision, profile) {
            Ok(request) => LiveOrView::View(request),
            Err(error) => {
                let _ = write_error(stream, error, max_error);
                return Ok(());
            }
        }
    };
    if let Err(error) = admit_auth(&http_request, &snapshot, profile, replay, clock) {
        let _ = write_error(stream, error, max_error);
        return Ok(());
    }
    match route {
        LiveOrView::View(request) => {
            if let Err(error) = admit_target(&request.view, &snapshot) {
                let _ = write_error(stream, error, max_error);
                return Ok(());
            }
            let window = match observation_window(&request.view, &snapshot) {
                Ok(window) => window,
                Err(error) => {
                    let _ = write_error(stream, error, max_error);
                    return Ok(());
                }
            };
            let view = match project_observation(&profile.core, &snapshot.state, &request, &window)
            {
                Ok(view) => view,
                Err(error) => {
                    let _ = write_error(stream, projection_error(&error), max_error);
                    return Ok(());
                }
            };
            write_view(stream, profile, &view)
        }
        LiveOrView::Live { after, limit } => {
            serve_live(stream, profile, source, live, after, limit, stop)
        }
    }
}

enum LiveOrView {
    View(ObservationRequest),
    Live { after: Option<u64>, limit: u64 },
}

fn admit_auth(
    http_request: &HttpRequest,
    snapshot: &ObservationSnapshot,
    profile: &HttpObservationProfile,
    replay: &Mutex<ReplayState>,
    clock: &dyn ObservationClock,
) -> Result<(), HttpError> {
    let unauthorized = || HttpError::new(401, "unauthorized", "request authorization failed");
    let header = http_request
        .headers
        .get("authorization")
        .ok_or_else(unauthorized)?;
    let principal = parse_principal(&profile.auth.principal).map_err(|_| unauthorized())?;
    let now = clock.now_unix();
    let max_nonce = usize::try_from(profile.auth.max_nonce_bytes).unwrap_or(128);
    let payload = verify_auth_header(
        header,
        &principal,
        &AuthRequest {
            method: "GET",
            path: &http_request.target,
            body: b"",
            state_revision: snapshot.state.revision,
        },
        now,
        profile.auth.max_clock_skew_seconds,
        max_nonce,
    )
    .map_err(|_| unauthorized())?;
    let mut replay = match replay.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    replay.admit(&payload, now).map_err(|_| unauthorized())?;
    Ok(())
}

fn admit_target(view: &ViewRequest, snapshot: &ObservationSnapshot) -> Result<(), HttpError> {
    match view {
        ViewRequest::RunList {
            after: Some(after), ..
        } => {
            if snapshot.state.runs.contains_key(after) {
                Ok(())
            } else {
                Err(HttpError::new(
                    404,
                    "unknown_target",
                    "run cursor is not in the admitted state",
                ))
            }
        }
        ViewRequest::RunDetail { run_id } => {
            if snapshot.state.runs.contains_key(run_id) {
                Ok(())
            } else {
                Err(HttpError::new(
                    404,
                    "unknown_target",
                    "run is not in the admitted state",
                ))
            }
        }
        ViewRequest::AttemptDetail { attempt_id } => {
            if snapshot.state.attempts.contains_key(attempt_id) {
                Ok(())
            } else {
                Err(HttpError::new(
                    404,
                    "unknown_target",
                    "attempt is not in the admitted state",
                ))
            }
        }
        ViewRequest::LogRange { stream_id, .. } => {
            if snapshot
                .log
                .as_ref()
                .is_some_and(|log| log.meta.stream_id == *stream_id)
            {
                Ok(())
            } else {
                Err(HttpError::new(
                    404,
                    "unknown_target",
                    "log stream is not in the admitted window",
                ))
            }
        }
        ViewRequest::RunList { after: None, .. }
        | ViewRequest::StateSummary
        | ViewRequest::EventTail { .. } => Ok(()),
    }
}

fn observation_window(
    view: &ViewRequest,
    snapshot: &ObservationSnapshot,
) -> Result<ObservationWindow, HttpError> {
    match view {
        ViewRequest::EventTail { .. } => snapshot
            .events
            .as_ref()
            .map(|events| ObservationWindow::events(events.records.clone()))
            .ok_or_else(|| {
                HttpError::new(500, "missing_event_window", "event window is unavailable")
            }),
        ViewRequest::LogRange { .. } => snapshot
            .log
            .as_ref()
            .map(|log| ObservationWindow::log(log.meta.clone(), log.retained_bytes.clone()))
            .ok_or_else(|| HttpError::new(404, "unknown_target", "log stream is unavailable")),
        _ => Ok(ObservationWindow::empty()),
    }
}

// r[impl kiln.observe.http.live.scenario.delivery]
fn serve_live<S: Write>(
    stream: &mut S,
    profile: &HttpObservationProfile,
    source: &dyn ObservationSource,
    live: &LiveEventBuffer,
    after: Option<u64>,
    limit: u64,
    stop: &AtomicBool,
) -> ObserveResult<()> {
    write_sse_headers(stream, profile)?;
    let max_frame = usize::try_from(profile.content.max_view_bytes).unwrap_or(usize::MAX);
    let max_events = profile.live.max_sse_events;
    let poll = Duration::from_millis(profile.live.poll_interval_milliseconds);
    let deadline = Instant::now() + Duration::from_millis(profile.live.max_stream_milliseconds);
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut last = after;
    let mut sent = 0_u64;
    let mut last_lag = None;
    while !stop.load(Ordering::Acquire) && sent < max_events && Instant::now() < deadline {
        let snapshot = source.snapshot();
        let live_snapshot = live.snapshot_after(last, take);
        if live_snapshot.dropped > 0
            && last_lag != Some(live_snapshot.dropped)
            && live_snapshot
                .oldest_revision
                .is_some_and(|oldest| last.is_none_or(|last| last < oldest))
        {
            let frame = LiveFrame::Lag {
                dropped: live_snapshot.dropped,
                oldest_revision: live_snapshot.oldest_revision,
                latest_revision: live_snapshot.latest_revision,
                state_revision: snapshot.state.revision,
            };
            write_frame(stream, &frame, max_frame, None)?;
            last_lag = Some(live_snapshot.dropped);
            sent = sent.saturating_add(1);
            if sent >= max_events {
                break;
            }
        }
        if live_snapshot.records.is_empty() {
            std::thread::sleep(poll);
            continue;
        }
        let window = ObservationWindow::events(live_snapshot.records.clone());
        let request = ObservationRequest {
            state_revision: snapshot.state.revision,
            view: ViewRequest::EventTail { limit },
        };
        match project_observation(&profile.core, &snapshot.state, &request, &window) {
            Ok(view) => {
                let next = live_snapshot
                    .records
                    .last()
                    .map(|record| record.expected_revision.saturating_add(1));
                write_frame(
                    stream,
                    &LiveFrame::View {
                        view: Box::new(view),
                    },
                    max_frame,
                    next,
                )?;
                if let Some(next) = next {
                    last = Some(next);
                }
                sent = sent.saturating_add(1);
            }
            Err(error) if error.code == "event_window_not_tail" => {
                let frame = LiveFrame::Lag {
                    dropped: live_snapshot.dropped,
                    oldest_revision: live_snapshot.oldest_revision,
                    latest_revision: live_snapshot.latest_revision,
                    state_revision: snapshot.state.revision,
                };
                write_frame(stream, &frame, max_frame, None)?;
                if let Some(latest) = live_snapshot.latest_revision {
                    last = Some(latest.saturating_add(1));
                }
                sent = sent.saturating_add(1);
            }
            Err(error) => {
                return Err(ObserveError::new("live_projection", error.message));
            }
        }
    }
    Ok(())
}

fn write_frame<W: Write>(
    stream: &mut W,
    frame: &LiveFrame,
    max_bytes: usize,
    id: Option<u64>,
) -> ObserveResult<()> {
    let bytes = sse_frame(frame, max_bytes, id)?;
    stream
        .write_all(&bytes)
        .and_then(|()| stream.flush())
        .map_err(|_| ObserveError::new("response_write", "SSE frame could not be written"))
}

fn projection_error(error: &kiln_core::types::Diagnostic) -> HttpError {
    match error.code.as_str() {
        "view_not_admitted" => HttpError::new(
            404,
            "unknown_view",
            "view kind is not admitted by the observation profile",
        ),
        "cursor_ahead" | "invalid_max_bytes" | "invalid_page_limit" => HttpError::new(
            400,
            "invalid_request",
            "view request lies outside its admitted bounds",
        ),
        "unknown_state_revision" => HttpError::new(
            409,
            "stale_state",
            "requested state revision is not current",
        ),
        _ => HttpError::new(
            500,
            "projection_error",
            "observation projection could not be completed",
        ),
    }
}
