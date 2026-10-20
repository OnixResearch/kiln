use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Cursor, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
use ucan::{
    AudienceDid, CapabilityDocument, CapabilitySet, CaveatDocument, CompactToken,
    Ed25519InMemorySigner, HolderInvocationLimits, HolderInvocationNonce,
    HolderInvocationSigningRequest, OperationBinding, OperationProfileId, ProofReferences,
    TokenSigner, TokenTimeBounds, VerificationLimits, issue_token_with_signer, proof_reference,
    sign_holder_invocation,
};

use crate::auth::AUTH_HEADER_SCHEME;
use crate::ucan_auth::{ucan_operation_profile_id, ucan_request_binding};
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
            issuer: String::new(),
            resource: String::new(),
            ability: String::new(),
            operation_profile: String::new(),
            holder_keys: Vec::new(),
            proof_tokens: Vec::new(),
            revoked_proof_references: Vec::new(),
            max_proof_depth: 0,
            max_proof_count: 0,
            max_token_bytes: 0,
            max_holder_audience_bytes: 0,
            max_holder_resource_bytes: 0,
            max_holder_ability_bytes: 0,
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

// r[verify kiln.observe.http.ucan_profile]
// r[verify kiln.observe.http.ucan_profile.scenario.match]
#[test]
fn nickel_ucan_profile_fixture_is_admitted() {
    let document: HttpProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/http-observation/ucan-profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile.admit().unwrap_or_else(|_| unreachable!());
    assert_eq!(document.profile.auth.kind, UCAN_AUTH_KIND);
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

fn ucan_profile() -> HttpObservationProfile {
    let document: HttpProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/http-observation/ucan-profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile
}

fn ucan_root() -> Ed25519InMemorySigner {
    Ed25519InMemorySigner::from_seed_bytes([1; 32])
}

fn ucan_holder() -> Ed25519InMemorySigner {
    Ed25519InMemorySigner::from_seed_bytes([2; 32])
}

fn ucan_audience(signer: &Ed25519InMemorySigner) -> AudienceDid {
    AudienceDid::from(signer.issuer().unwrap_or_else(|_| unreachable!()))
}

fn ucan_token(
    issuer: &Ed25519InMemorySigner,
    audience: &AudienceDid,
    resource: &str,
    ability: &str,
    proofs: &ProofReferences,
    now: u64,
) -> String {
    let capabilities = CapabilitySet::single(resource, ability).unwrap_or_else(|_| unreachable!());
    let bounds = TokenTimeBounds::new(now.saturating_sub(10), now.saturating_add(3_600))
        .unwrap_or_else(|_| unreachable!());
    issue_token_with_signer(issuer, audience, &capabilities, proofs, bounds)
        .unwrap_or_else(|_| unreachable!())
        .as_str()
        .to_string()
}

fn ucan_header(
    profile: &HttpObservationProfile,
    token: &str,
    target: &str,
    nonce: [u8; 32],
) -> String {
    ucan_header_with(
        profile,
        token,
        &profile.auth.resource,
        &profile.auth.ability,
        target,
        ucan_operation_profile_id(),
        nonce,
    )
}

fn ucan_header_with(
    profile: &HttpObservationProfile,
    token: &str,
    resource: &str,
    ability: &str,
    target: &str,
    operation: OperationProfileId,
    nonce: [u8; 32],
) -> String {
    let holder = ucan_holder();
    let audience = ucan_audience(&holder);
    let binding = ucan_request_binding("GET", target, b"", 1).unwrap_or_else(|_| unreachable!());
    let operation = OperationBinding::new(operation, binding);
    let nonce = HolderInvocationNonce::from_bytes(nonce);
    let signing =
        HolderInvocationSigningRequest::new(token, &audience, resource, ability, operation, nonce);
    let limits = VerificationLimits::new(
        usize::try_from(profile.auth.max_proof_depth).unwrap_or(usize::MAX),
        usize::try_from(profile.auth.max_proof_count).unwrap_or(usize::MAX),
        usize::try_from(profile.auth.max_token_bytes).unwrap_or(usize::MAX),
    )
    .with_clock_skew_tolerance_seconds(profile.auth.max_clock_skew_seconds);
    let holder_limits = HolderInvocationLimits::try_new(
        usize::try_from(profile.auth.max_holder_audience_bytes).unwrap_or(usize::MAX),
        usize::try_from(profile.auth.max_holder_resource_bytes).unwrap_or(usize::MAX),
        usize::try_from(profile.auth.max_holder_ability_bytes).unwrap_or(usize::MAX),
    )
    .unwrap_or_else(|_| unreachable!());
    let signature = sign_holder_invocation(&signing, &holder, limits, holder_limits)
        .unwrap_or_else(|_| unreachable!());
    format!(
        "UCAN {}.{}.{}",
        URL_SAFE_NO_PAD.encode(token.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature.as_bytes()),
        URL_SAFE_NO_PAD.encode(nonce.as_bytes())
    )
}

fn ucan_verify(
    profile: &HttpObservationProfile,
    header: &str,
    target: &str,
    state_revision: u64,
    now: u64,
    replay: &Mutex<ReplayState>,
) -> ObserveResult<()> {
    let request = AuthRequest {
        method: "GET",
        path: target,
        body: b"",
        state_revision,
    };
    verify_ucan_header(header, &profile.auth, &request, now, replay)
}

// r[verify kiln.observe.http.ucan_holder.scenario.authorized]
// r[verify kiln.observe.http.ucan_replay]
// r[verify kiln.observe.http.ucan_replay.scenario.replay]
#[test]
fn ucan_holder_invocation_is_admitted_and_replayed_nonce_denied() {
    let profile = ucan_profile();
    let root = ucan_root();
    let holder = ucan_holder();
    let token = ucan_token(
        &root,
        &ucan_audience(&holder),
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let target = "/v1/views/state-summary";
    let header = ucan_header(&profile, &token, target, [3; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    ucan_verify(&profile, &header, target, 1, NOW, &replay).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
}

// r[verify kiln.observe.http.ucan_holder.scenario.denied]
#[test]
fn ucan_rejects_wrong_target_capability_operation_and_expired_token() {
    let profile = ucan_profile();
    let root = ucan_root();
    let holder = ucan_holder();
    let audience = ucan_audience(&holder);
    let token = ucan_token(
        &root,
        &audience,
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let target = "/v1/views/state-summary";
    let header = ucan_header(&profile, &token, target, [4; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    assert_eq!(
        ucan_verify(&profile, &header, "/v1/views/runs", 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
    let wrong_capability = ucan_token(
        &root,
        &audience,
        "kiln:other",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let header = ucan_header_with(
        &profile,
        &wrong_capability,
        "kiln:other",
        "observe",
        target,
        ucan_operation_profile_id(),
        [5; 32],
    );
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
    let wrong_audience = ucan_token(
        &root,
        &ucan_audience(&root),
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let header = ucan_header(&profile, &wrong_audience, target, [14; 32]);
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
    let wrong_operation = OperationProfileId::from_bytes(*blake3::hash(b"wrong").as_bytes());
    let header = ucan_header_with(
        &profile,
        &token,
        "kiln:observe",
        "observe",
        target,
        wrong_operation,
        [6; 32],
    );
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
    let expired = ucan_token(
        &root,
        &audience,
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW.saturating_sub(7_200),
    );
    let header = ucan_header(&profile, &expired, target, [7; 32]);
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
    assert_eq!(
        ucan_verify(&profile, "Bearer token", target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_shape")
    );
}

// r[verify kiln.observe.http.ucan_holder]
#[test]
fn ucan_proof_chain_is_admitted_and_revocation_is_denied() {
    let profile = ucan_profile();
    let root = ucan_root();
    let intermediate = Ed25519InMemorySigner::from_seed_bytes([3; 32]);
    let holder = ucan_holder();
    let root_token = ucan_token(
        &root,
        &ucan_audience(&intermediate),
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let root_compact = CompactToken::new(&root_token).unwrap_or_else(|_| unreachable!());
    let reference = proof_reference(&root_compact);
    let proofs = ProofReferences::from_references(vec![reference.clone()]);
    let child_token = ucan_token(
        &intermediate,
        &ucan_audience(&holder),
        "kiln:observe",
        "observe",
        &proofs,
        NOW,
    );
    let mut proof_profile = profile;
    proof_profile.auth.proof_tokens = vec![root_token];
    proof_profile.auth.holder_keys = vec![
        ucan_audience(&holder).as_str().to_string(),
        intermediate
            .issuer()
            .unwrap_or_else(|_| unreachable!())
            .as_str()
            .to_string(),
    ];
    proof_profile.admit().unwrap_or_else(|_| unreachable!());
    let target = "/v1/views/state-summary";
    let header = ucan_header(&proof_profile, &child_token, target, [8; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    ucan_verify(&proof_profile, &header, target, 1, NOW, &replay)
        .unwrap_or_else(|_| unreachable!());
    proof_profile.auth.revoked_proof_references = vec![hex(reference.as_bytes())];
    proof_profile.admit().unwrap_or_else(|_| unreachable!());
    let header = ucan_header(&proof_profile, &child_token, target, [9; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    assert_eq!(
        ucan_verify(&proof_profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
}

// r[verify kiln.observe.http.ucan_holder.scenario.denied]
#[test]
fn ucan_rejects_caveated_token() {
    let profile = ucan_profile();
    let root = ucan_root();
    let holder = ucan_holder();
    let audience = ucan_audience(&holder);
    let caveat = CaveatDocument::new(
        "kiln.observe".to_owned(),
        "test".to_owned(),
        "key".to_owned(),
        vec![1],
    )
    .unwrap_or_else(|_| unreachable!());
    let capability = CapabilityDocument::with_caveats(
        "kiln:observe".to_owned(),
        "observe".to_owned(),
        vec![caveat],
    )
    .unwrap_or_else(|_| unreachable!());
    let capabilities = CapabilitySet::new(vec![capability]).unwrap_or_else(|_| unreachable!());
    let bounds = TokenTimeBounds::new(NOW.saturating_sub(10), NOW.saturating_add(3_600))
        .unwrap_or_else(|_| unreachable!());
    let token = issue_token_with_signer(
        &root,
        &audience,
        &capabilities,
        &ProofReferences::empty(),
        bounds,
    )
    .unwrap_or_else(|_| unreachable!());
    let target = "/v1/views/state-summary";
    let header = ucan_header(&profile, token.as_str(), target, [13; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
}

// r[verify kiln.observe.http.ucan_profile.scenario.anchor]
#[test]
fn ucan_profile_rejects_unanchored_proof_token() {
    let holder = ucan_holder();
    let unanchored = ucan_token(
        &holder,
        &ucan_audience(&holder),
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let mut invalid = ucan_profile();
    invalid.auth.proof_tokens = vec![unanchored];
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_ucan_anchor")
    );
}

// r[verify kiln.observe.http.ucan_holder.scenario.root]
#[test]
fn ucan_rejects_self_issued_holder_token() {
    let profile = ucan_profile();
    let holder = ucan_holder();
    let audience = ucan_audience(&holder);
    let token = ucan_token(
        &holder,
        &audience,
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let target = "/v1/views/state-summary";
    let header = ucan_header(&profile, &token, target, [12; 32]);
    let replay = Mutex::new(ReplayState::new(300, 64));
    assert_eq!(
        ucan_verify(&profile, &header, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
    );
}

// r[verify kiln.observe.http.ucan_profile.scenario.drift]
// r[verify kiln.observe.http.ucan_nonclaims]
// r[verify kiln.observe.http.ucan_nonclaims.scenario.scope]
#[test]
fn ucan_profile_admission_rejects_drift_and_missing_nonclaims() {
    let mut invalid = ucan_profile();
    invalid.auth.kind = String::from("bearer");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("unsupported_auth_kind")
    );
    let mut invalid = ucan_profile();
    invalid.auth.issuer = ucan_audience(&ucan_holder()).as_str().to_string();
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_auth_principal")
    );
    let mut invalid = ucan_profile();
    invalid.auth.operation_profile = String::from("kiln.observe.http.v2");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("unsupported_operation_profile")
    );
    let mut invalid = ucan_profile();
    invalid.auth.max_token_bytes = 0;
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_ucan_bounds")
    );
    let mut invalid = ucan_profile();
    invalid.auth.holder_keys.clear();
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_holder_keys")
    );
    let mut invalid = ucan_profile();
    invalid.auth.revoked_proof_references = vec![String::from("not-a-digest")];
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_revocation_reference")
    );
    let mut invalid = ucan_profile();
    let _ = invalid.non_claims.pop();
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("missing_required_http_non_claim")
    );
}

// r[verify kiln.observe.http.ucan_replay.scenario.bound]
#[test]
fn ucan_replay_cache_bound_denies_new_nonce() {
    let profile = ucan_profile();
    let root = ucan_root();
    let holder = ucan_holder();
    let token = ucan_token(
        &root,
        &ucan_audience(&holder),
        "kiln:observe",
        "observe",
        &ProofReferences::empty(),
        NOW,
    );
    let target = "/v1/views/state-summary";
    let first = ucan_header(&profile, &token, target, [10; 32]);
    let second = ucan_header(&profile, &token, target, [11; 32]);
    let replay = Mutex::new(ReplayState::new(300, 1));
    ucan_verify(&profile, &first, target, 1, NOW, &replay).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        ucan_verify(&profile, &second, target, 1, NOW, &replay)
            .err()
            .map(|error| error.code),
        Some("ucan_denied")
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

// r[verify kiln.observe.http.ucan_holder.scenario.authorized]
#[test]
fn server_serves_ucan_holder_invocation() {
    let mut profile = ucan_profile();
    profile.tls.certificate_path = tls_path("chain.pem");
    profile.tls.private_key_path = tls_path("key.pem");
    profile.origin.host = String::from("localhost");
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(4));
    let mut server = start_server(profile.clone(), source, live);
    let target = "/v1/views/state-summary";
    let holder = ucan_holder();
    let token = ucan_token(
        &ucan_root(),
        &ucan_audience(&holder),
        &profile.auth.resource,
        &profile.auth.ability,
        &ProofReferences::empty(),
        NOW,
    );
    let authorization = ucan_header(&profile, &token, target, [21; 32]);
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 200);
    assert!(header_present(&response, b"X-Kiln-View-Identity"));
    server.stop();
}

// r[verify kiln.observe.http.ucan_holder.scenario.root]
#[test]
fn server_rejects_unanchored_ucan_invocation() {
    let mut profile = ucan_profile();
    profile.tls.certificate_path = tls_path("chain.pem");
    profile.tls.private_key_path = tls_path("key.pem");
    profile.origin.host = String::from("localhost");
    let source = Arc::new(MemoryObservationSource::new(snapshot()));
    let live = Arc::new(LiveEventBuffer::new(4));
    let mut server = start_server(profile.clone(), source, live);
    let target = "/v1/views/state-summary";
    let holder = ucan_holder();
    let token = ucan_token(
        &holder,
        &ucan_audience(&holder),
        &profile.auth.resource,
        &profile.auth.ability,
        &ProofReferences::empty(),
        NOW,
    );
    let authorization = ucan_header(&profile, &token, target, [22; 32]);
    let response = tls_exchange(server.address, &request_bytes(target, Some(&authorization)));
    assert_eq!(status(&response), 401);
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
