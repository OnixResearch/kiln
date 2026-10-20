use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Cursor, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use kiln_core::domain::{AttemptState, Event, Outcome, RunState, State};
use kiln_core::operations::JournalRecord;
use kiln_core::task_ops::LogStreamMeta;
use kiln_core::types::{AttemptId, CommandId, ExternalRef, ProviderProfileId, RunId, StreamId};
use kiln_observe_core::{LogWindow, ObservationView, ViewBody, ViewRequest};
use rustls::pki_types::ServerName;
use serde::Deserialize;

use crate::auth::AUTH_HEADER_SCHEME;
use crate::*;

const NOW: u64 = 1_700_000_000;
const TEST_SEED: [u8; 32] = [7; 32];
const OTHER_SEED: [u8; 32] = [9; 32];

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&TEST_SEED)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn run_id(value: &str) -> RunId {
    RunId::new(value).unwrap_or_else(|_| unreachable!())
}

fn attempt_id(value: &str) -> AttemptId {
    AttemptId::new(value).unwrap_or_else(|_| unreachable!())
}

fn stream_id(value: &str) -> StreamId {
    StreamId::new(value).unwrap_or_else(|_| unreachable!())
}

fn provider_profile(value: &str) -> ProviderProfileId {
    ProviderProfileId::new(value).unwrap_or_else(|_| unreachable!())
}

fn command_id(value: &str) -> CommandId {
    CommandId::new(value).unwrap_or_else(|_| unreachable!())
}

fn external_ref(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
}

#[derive(Deserialize)]
struct ProfileDocument {
    profile: kiln_observe_core::ObservationProfile,
}

#[derive(Deserialize)]
struct HttpProfileDocument {
    profile: HttpObservationProfile,
}

fn core_profile() -> kiln_observe_core::ObservationProfile {
    let document: ProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile
}

fn tls_path(name: &str) -> String {
    format!(
        "{}/../../fixtures/http-observation/tls/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn http_non_claims() -> Vec<String> {
    HTTP_OBSERVATION_NON_CLAIMS
        .iter()
        .map(|value| String::from(*value))
        .collect()
}

fn profile() -> HttpObservationProfile {
    HttpObservationProfile {
        schema_version: crate::profile::HTTP_OBSERVATION_PROFILE_SCHEMA_VERSION,
        profile_id: String::from("http-observation-v1"),
        origin: OriginProfile {
            scheme: String::from("https"),
            host: String::from("localhost"),
            port: 443,
        },
        tls: TlsProfile {
            certificate_path: tls_path("chain.pem"),
            private_key_path: tls_path("key.pem"),
            min_version: String::from("tls12"),
        },
        auth: AuthProfile {
            kind: String::from("ed25519_request_v1"),
            principal: hex(&signing_key().verifying_key().to_bytes()),
            max_clock_skew_seconds: 60,
            replay_window_seconds: 300,
            max_replay_entries: 64,
            max_nonce_bytes: 64,
        },
        content: ContentProfile {
            view_content_type: String::from("application/json"),
            event_content_type: String::from("text/event-stream"),
            canonical_json: true,
            max_view_bytes: 4_194_304,
            default_page_limit: 2,
            default_log_bytes: 4,
        },
        live: LiveProfile {
            max_records: 4,
            max_dropped: 16,
            poll_interval_milliseconds: 10,
            max_stream_milliseconds: 500,
            max_sse_events: 4,
        },
        bounds: HttpBounds {
            max_request_bytes: 16_384,
            max_header_bytes: 8_192,
            max_header_fields: 64,
            max_connections: 4,
            read_timeout_milliseconds: 2_000,
            write_timeout_milliseconds: 2_000,
            max_error_bytes: 4_096,
        },
        core: core_profile(),
        non_claims: http_non_claims(),
    }
}

fn fixture_state() -> State {
    let run = run_id("run-a");
    let attempt = attempt_id("attempt-a1");
    let mut runs = BTreeMap::new();
    runs.insert(
        run.clone(),
        RunState {
            active_attempt: attempt.clone(),
            outcome: Outcome::Success,
        },
    );
    let mut attempts = BTreeMap::new();
    attempts.insert(
        attempt.clone(),
        AttemptState {
            run_id: run,
            provider_profile: provider_profile("provider-one"),
            outcome: Outcome::Success,
            external_reference: Some(external_ref("provider", "attempt", "attempt-a1")),
            products: Vec::new(),
            metrics: Vec::new(),
        },
    );
    State {
        revision: 1,
        runs,
        attempts,
        holds: BTreeSet::new(),
        seen_commands: BTreeSet::new(),
    }
}

fn state_with_revision(revision: u64) -> State {
    let mut state = fixture_state();
    state.revision = revision;
    state
}

fn journal_record(revision: u64) -> JournalRecord {
    JournalRecord {
        schema_version: 1,
        stream: external_ref("kiln", "journal", "journal-one"),
        expected_revision: revision,
        event_identity: external_ref("kiln", "event", &format!("event-{revision}")),
        event: Event::RunStarted {
            command_id: command_id(&format!("command-{revision}")),
            run_id: run_id("run-a"),
            attempt_id: attempt_id("attempt-a1"),
            provider_profile: provider_profile("provider-one"),
        },
    }
}

fn event_window(state_revision: u64) -> kiln_observe_core::EventWindow {
    kiln_observe_core::EventWindow {
        records: (0..state_revision).map(journal_record).collect(),
    }
}

fn log_window() -> LogWindow {
    LogWindow {
        meta: LogStreamMeta {
            stream_id: stream_id("stream-one"),
            attempt_id: attempt_id("attempt-a1"),
            segments: Vec::new(),
            retained_start: 0,
            retained_end: 3,
            complete: false,
            truncated: false,
        },
        retained_bytes: b"abc".to_vec(),
    }
}

fn snapshot() -> ObservationSnapshot {
    ObservationSnapshot::new(fixture_state())
        .with_events(event_window(1))
        .with_log(log_window())
}

fn auth_header_with_key(
    key: &SigningKey,
    target: &str,
    state_revision: u64,
    nonce: &str,
) -> String {
    let payload = AuthPayload {
        schema: String::from(AUTH_SCHEMA),
        method: String::from("GET"),
        path: target.to_string(),
        body_blake3: body_digest(b""),
        nonce: nonce.to_string(),
        issued_at: NOW,
        state_revision,
    };
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| unreachable!());
    let signature = key.sign(&payload_bytes);
    format!(
        "{AUTH_HEADER_SCHEME} {}.{}",
        URL_SAFE_NO_PAD.encode(&payload_bytes),
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    )
}

fn auth_header(target: &str, state_revision: u64, nonce: &str) -> String {
    auth_header_with_key(&signing_key(), target, state_revision, nonce)
}

fn request_bytes(target: &str, authorization: Option<&str>) -> Vec<u8> {
    let mut request = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n");
    if let Some(authorization) = authorization {
        request.push_str(&format!("Authorization: {authorization}\r\n"));
    }
    request.push_str("\r\n");
    request.into_bytes()
}

fn ensure_provider() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
}

fn tls_exchange(address: SocketAddr, request: &[u8]) -> Vec<u8> {
    ensure_provider();
    let cert_file = File::open(tls_path("ca-cert.pem")).unwrap_or_else(|_| unreachable!());
    let mut roots = rustls::RootCertStore::empty();
    for certificate in rustls_pemfile::certs(&mut BufReader::new(cert_file)) {
        let certificate = certificate.unwrap_or_else(|_| unreachable!());
        roots.add(certificate).unwrap_or_else(|_| unreachable!());
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from("localhost").unwrap_or_else(|_| unreachable!());
    let connection = rustls::ClientConnection::new(Arc::new(config), server_name)
        .unwrap_or_else(|_| unreachable!());
    let tcp = TcpStream::connect(address).unwrap_or_else(|_| unreachable!());
    let mut stream = rustls::StreamOwned::new(connection, tcp);
    stream
        .write_all(request)
        .unwrap_or_else(|error| unreachable!("TLS write failed: {error:?}"));
    stream
        .flush()
        .unwrap_or_else(|error| unreachable!("TLS flush failed: {error:?}"));
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    response
}

struct TestServer {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<ObserveResult<()>>>,
}

impl TestServer {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

struct FixedClock(u64);

impl ObservationClock for FixedClock {
    fn now_unix(&self) -> u64 {
        self.0
    }
}

fn start_server(
    profile: HttpObservationProfile,
    source: Arc<dyn ObservationSource>,
    live: Arc<LiveEventBuffer>,
) -> TestServer {
    ensure_provider();
    profile.admit().unwrap_or_else(|_| unreachable!());
    let _ = load_tls_config(&profile).unwrap_or_else(|_| unreachable!());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
    let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        serve_on(
            listener,
            profile,
            source,
            live,
            Arc::new(FixedClock(NOW)),
            thread_stop,
        )
    });
    TestServer {
        address,
        stop,
        handle: Some(handle),
    }
}

fn response_body(response: &[u8]) -> &[u8] {
    response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .and_then(|position| response.get(position.saturating_add(4)..))
        .unwrap_or_default()
}

fn status(response: &[u8]) -> u16 {
    let text = std::str::from_utf8(response).unwrap_or_default();
    text.lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn header_present(response: &[u8], name: &[u8]) -> bool {
    response.windows(name.len()).any(|window| window == name)
}

// r[verify kiln.observe.http.nonclaims.scenario.scope]
#[test]
fn profile_admission_accepts_complete_profile() {
    profile().admit().unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.observe.http.profile.scenario.match]
#[test]
fn nickel_http_profile_fixture_is_admitted() {
    let document: HttpProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/http-observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile.admit().unwrap_or_else(|_| unreachable!());
    assert_eq!(document.profile.profile_id, "http-observation-v1");
}

// r[verify kiln.observe.http.profile.scenario.drift]
#[test]
fn profile_admission_rejects_drift() {
    let mut invalid = profile();
    invalid.origin.scheme = String::from("http");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("non_https_origin")
    );
    let mut invalid = profile();
    invalid.bounds.max_connections = 0;
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("zero_http_bound")
    );
    let mut invalid = profile();
    invalid.auth.kind = String::from("bearer");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("unsupported_auth_kind")
    );
    let mut invalid = profile();
    let _ = invalid.non_claims.pop();
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("missing_required_http_non_claim")
    );
    let mut invalid = profile();
    invalid.tls.certificate_path = String::from("cert.pem");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("relative_tls_path")
    );
    let mut invalid = profile();
    invalid.content.view_content_type = String::from("text/plain");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_content_contract")
    );
}

// r[verify kiln.observe.http.serve.scenario.unknown_kind]
#[test]
fn request_mapping_admits_views_and_rejects_unknowns() {
    let profile = profile();
    let request = parse_request(
        b"GET /v1/views/state-summary HTTP/1.1\r\nHost: localhost",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        map_view_request(&request, 1, &profile)
            .unwrap_or_else(|_| unreachable!())
            .view,
        ViewRequest::StateSummary
    ));
    let request = parse_request(
        b"GET /v1/views/runs?limit=2&after=run-a HTTP/1.1\r\nHost: localhost",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        map_view_request(&request, 1, &profile)
            .unwrap_or_else(|_| unreachable!())
            .view,
        ViewRequest::RunList { .. }
    ));
    let request = parse_request(b"GET /v1/views/nope HTTP/1.1\r\nHost: localhost", &profile)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        map_view_request(&request, 1, &profile)
            .err()
            .map(|error| error.code),
        Some("unknown_view")
    );
    let request = parse_request(
        b"GET /v1/views/state-summary HTTP/1.1\r\nHost: evil",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        map_view_request(&request, 1, &profile)
            .err()
            .map(|error| error.code),
        Some("origin_mismatch")
    );
    assert_eq!(
        parse_request(b"POST /v1/views/state-summary HTTP/1.1", &profile)
            .err()
            .map(|error| error.code),
        Some("method_not_allowed")
    );
    let request = parse_request(
        b"GET /v1/views/runs?limit=1&limit=2 HTTP/1.1\r\nHost: localhost",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        map_view_request(&request, 1, &profile)
            .err()
            .map(|error| error.code),
        Some("duplicate_query_field")
    );
    let request = parse_request(
        b"GET /v1/views/runs?foo=1 HTTP/1.1\r\nHost: localhost",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        map_view_request(&request, 1, &profile)
            .err()
            .map(|error| error.code),
        Some("unexpected_query_field")
    );
    let request = parse_request(
        b"GET /v1/views/runs?limit=abc HTTP/1.1\r\nHost: localhost",
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        map_view_request(&request, 1, &profile)
            .err()
            .map(|error| error.code),
        Some("invalid_query_value")
    );
}

#[test]
fn request_reader_enforces_bounds_and_timeouts() {
    let profile = profile();
    let mut input = Cursor::new(b"GET /v1/views/state-summary HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(read_request(&mut input, &profile).is_ok());
    let mut invalid = profile.clone();
    invalid.bounds.max_request_bytes = 4;
    let mut input = Cursor::new(b"GET /v1/views/state-summary HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert_eq!(
        read_request(&mut input, &invalid)
            .err()
            .map(|error| error.status),
        Some(413)
    );
    let mut input = Cursor::new(b"BAD\r\n\r\n");
    assert_eq!(
        read_request(&mut input, &profile)
            .err()
            .map(|error| error.status),
        Some(400)
    );
    struct TimeoutReader;
    impl Read for TimeoutReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(ErrorKind::WouldBlock, "timeout"))
        }
    }
    assert_eq!(
        read_request(&mut TimeoutReader, &profile)
            .err()
            .map(|error| error.status),
        Some(408)
    );
}

// r[verify kiln.observe.http.auth.scenario.authorized]
// r[verify kiln.observe.http.auth.scenario.unauthorized]
#[test]
fn auth_verifies_and_rejects() {
    let profile = profile();
    let principal = parse_principal(&profile.auth.principal).unwrap_or_else(|_| unreachable!());
    let target = "/v1/views/state-summary";
    let header = auth_header(target, 1, "nonce-auth");
    let request = AuthRequest {
        method: "GET",
        path: target,
        body: b"",
        state_revision: 1,
    };
    let payload = verify_auth_header(&header, &principal, &request, NOW, 60, 64)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        verify_auth_header(&header, &principal, &request, NOW + 61, 60, 64)
            .err()
            .map(|error| error.code),
        Some("auth_clock")
    );
    let wrong_target = AuthRequest {
        path: "/v1/views/runs",
        ..request
    };
    assert_eq!(
        verify_auth_header(&header, &principal, &wrong_target, NOW, 60, 64)
            .err()
            .map(|error| error.code),
        Some("auth_binding")
    );
    let wrong_key = auth_header_with_key(
        &SigningKey::from_bytes(&OTHER_SEED),
        target,
        1,
        "nonce-other",
    );
    assert_eq!(
        verify_auth_header(&wrong_key, &principal, &request, NOW, 60, 64)
            .err()
            .map(|error| error.code),
        Some("auth_signature")
    );
    let mut replay = ReplayState::new(300, 4);
    replay
        .admit(&payload, NOW)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        replay.admit(&payload, NOW).err().map(|error| error.code),
        Some("auth_replay")
    );
}

// r[verify kiln.observe.http.live.scenario.loss]
#[test]
fn live_buffer_drops_oldest_and_reports_loss() {
    let live = LiveEventBuffer::new(2);
    live.push(journal_record(0))
        .unwrap_or_else(|_| unreachable!());
    live.push(journal_record(1))
        .unwrap_or_else(|_| unreachable!());
    live.push(journal_record(2))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(live.dropped(), 1);
    let snapshot = live.snapshot_after(None, 2);
    assert_eq!(snapshot.dropped, 1);
    assert_eq!(snapshot.oldest_revision, Some(1));
    assert_eq!(snapshot.latest_revision, Some(2));
    assert_eq!(snapshot.records.len(), 2);
    assert_eq!(
        live.push(journal_record(5)).err().map(|error| error.code),
        Some("live_revision_gap")
    );
}

#[test]
fn sse_frame_is_bounded_and_identified() {
    let frame = LiveFrame::Lag {
        dropped: 2,
        oldest_revision: Some(3),
        latest_revision: Some(4),
        state_revision: 5,
    };
    let bytes = sse_frame(&frame, 1_024, Some(5)).unwrap_or_else(|_| unreachable!());
    assert!(bytes.windows(5).any(|window| window == b"id: 5"));
    assert!(bytes.windows(10).any(|window| window == b"event: lag"));
    assert!(bytes.windows(6).any(|window| window == b"data: "));
    assert_eq!(
        sse_frame(&frame, 4, None).err().map(|error| error.code),
        Some("live_encoding_bound")
    );
}

// r[verify kiln.observe.http.serve.scenario.view]
#[test]
fn server_serves_identity_bearing_view() {
    let profile = profile();
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(4));
    let mut server = start_server(profile.clone(), source, live);
    let target = "/v1/views/state-summary";
    let authorization = auth_header(target, 1, "nonce-view");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 200);
    assert!(header_present(&response, b"X-Kiln-View-Identity"));
    let view: ObservationView =
        serde_json::from_slice(response_body(&response)).unwrap_or_else(|_| unreachable!());
    assert!(matches!(view.body, ViewBody::StateSummary(_)));
    server.stop();
}

#[test]
fn server_rejects_unauthorized_unknown_and_malformed() {
    let profile = profile();
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(4));
    let mut server = start_server(profile.clone(), source, live);
    let response = tls_exchange(
        server.address,
        &request_bytes("/v1/views/state-summary", None),
    );
    assert_eq!(status(&response), 401);
    let target = "/v1/views/nope";
    let authorization = auth_header(target, 1, "nonce-unknown");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 404);
    let target = "/v1/views/runs/run-missing";
    let authorization = auth_header(target, 1, "nonce-target");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 404);
    let response = tls_exchange(server.address, b"BAD\r\n\r\n");
    assert_eq!(status(&response), 400);
    server.stop();
}

#[test]
fn server_live_delivers_and_reports_loss() {
    let mut profile = profile();
    profile.live.max_sse_events = 4;
    profile.live.max_stream_milliseconds = 500;
    let source = Arc::new(MemoryObservationSource::new(ObservationSnapshot::new(
        state_with_revision(4),
    )));
    let live = Arc::new(LiveEventBuffer::new(2));
    for revision in 0..4 {
        live.push(journal_record(revision))
            .unwrap_or_else(|_| unreachable!());
    }
    let mut server = start_server(profile.clone(), source, live);
    let target = "/v1/live?after=0&limit=2";
    let authorization = auth_header(target, 4, "nonce-live");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 200);
    assert!(header_present(&response, b"event: lag"));
    assert!(header_present(&response, b"\"dropped\":2"));
    assert!(header_present(&response, b"event: observe"));
    server.stop();
}

// r[verify kiln.observe.http.live.scenario.delivery]
#[test]
fn server_live_delivers_new_committed_event() {
    let profile = profile();
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(1));
    let mut server = start_server(profile.clone(), source, Arc::clone(&live));
    let pusher = {
        let live = Arc::clone(&live);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            live.push(journal_record(0))
                .unwrap_or_else(|_| unreachable!());
        })
    };
    let target = "/v1/live?limit=1";
    let authorization = auth_header(target, 1, "nonce-live-new");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 200);
    assert!(header_present(&response, b"event: observe"));
    assert!(header_present(response_body(&response), b"\"identity\""));
    pusher.join().unwrap_or_else(|_| unreachable!());
    server.stop();
}

#[test]
fn server_tls_failure_does_not_break_service() {
    let profile = profile();
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(4));
    let mut server = start_server(profile.clone(), source, live);
    if let Ok(mut plain) = TcpStream::connect(server.address) {
        let _ = plain.write_all(b"not tls");
    }
    let target = "/v1/views/state-summary";
    let authorization = auth_header(target, 1, "nonce-after-tls");
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 200);
    server.stop();
}

// r[verify kiln.observe.http.conformance.scenario.fixture_missing]
#[test]
fn conformance_manifest_requires_all_cases() {
    let complete = HttpConformanceManifest {
        positive: REQUIRED_POSITIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
        negative: REQUIRED_NEGATIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
    };
    admit_http_conformance(&complete).unwrap_or_else(|_| unreachable!());
    let mut missing = complete.clone();
    missing
        .negative
        .retain(|case| case != "missing_negative_case");
    assert_eq!(
        admit_http_conformance(&missing)
            .err()
            .map(|error| error.code),
        Some("missing_negative_case")
    );
    let mut duplicate = complete;
    duplicate.positive.push(String::from("state_summary"));
    assert_eq!(
        admit_http_conformance(&duplicate)
            .err()
            .map(|error| error.code),
        Some("duplicate_case")
    );
}
