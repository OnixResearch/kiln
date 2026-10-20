use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use iroh::{Endpoint, SecretKey};
use kiln_adapter_http_observe::{
    MemoryObservationSource, ObservationSnapshot, ObservationSource, encode_view,
};
use kiln_core::domain::{AttemptState, Event, Outcome, RunState, State};
use kiln_core::operations::JournalRecord;
use kiln_core::task_ops::LogStreamMeta;
use kiln_core::types::{AttemptId, CommandId, ExternalRef, ProviderProfileId, RunId, StreamId};
use kiln_observe_core::{
    ObservationProfile, ObservationRequest, ObservationWindow, ViewBody, ViewRequest,
    project_observation,
};
use serde::Deserialize;
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::*;

const BOUND: Duration = Duration::from_secs(5);

fn key(value: u8) -> SecretKey {
    SecretKey::from_bytes(&[value; 32])
}

#[derive(Deserialize)]
struct ProfileDocument {
    profile: ObservationProfile,
}

#[derive(Deserialize)]
struct IrohProfileDocument {
    profile: IrohObservationProfile,
}

fn core_profile() -> ObservationProfile {
    let document: ProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile
}

fn non_claims() -> Vec<String> {
    IROH_OBSERVATION_NON_CLAIMS
        .iter()
        .map(|value| String::from(*value))
        .collect()
}

fn profile(peer: SecretKey, revoked: bool) -> IrohObservationProfile {
    let mut revoked_peers = BTreeSet::new();
    if revoked {
        revoked_peers.insert(peer.public());
    }
    IrohObservationProfile {
        schema_version: 1,
        profile_id: String::from("iroh-observation-v1"),
        cohort: CohortProfile {
            iroh_version: String::from(IROH_VERSION),
            iroh_blobs: String::from(IROH_BLOBS_COHORT),
            bao_tree: String::from(BAO_TREE_COHORT),
            alpn: String::from_utf8(IROH_OBSERVATION_ALPN.to_vec())
                .unwrap_or_else(|_| unreachable!()),
        },
        endpoint: EndpointProfile {
            bind_address: "127.0.0.1:0".parse().unwrap_or_else(|_| unreachable!()),
            relay_urls: Vec::new(),
            max_connections: 4,
            max_frame_bytes: 4_194_304,
            max_payload_bytes: 4_194_304,
            request_timeout_milliseconds: 1_000,
            write_timeout_milliseconds: 1_000,
        },
        peers: PeerProfile {
            allowed: BTreeSet::from([peer.public()]),
            revoked: revoked_peers,
            max_peers: 8,
        },
        core: core_profile(),
        non_claims: non_claims(),
    }
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
        attempt,
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

fn snapshot() -> ObservationSnapshot {
    ObservationSnapshot::new(fixture_state())
        .with_events(kiln_observe_core::EventWindow {
            records: vec![journal_record(0)],
        })
        .with_log(kiln_observe_core::LogWindow {
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
        })
}

struct Server {
    endpoint: Endpoint,
    task: Option<tokio::task::JoinHandle<ObserveResult<()>>>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl Server {
    async fn start(profile: IrohObservationProfile) -> Self {
        let endpoint = bind_endpoint(&profile, key(1))
            .await
            .unwrap_or_else(|_| unreachable!());
        let source: Arc<dyn ObservationSource> = Arc::new(MemoryObservationSource::new(snapshot()));
        let (shutdown, receive) = oneshot::channel();
        let owned = endpoint.clone();
        let task = tokio::spawn(serve(owned, profile, source, async {
            let _ = receive.await;
        }));
        Self {
            endpoint,
            task: Some(task),
            shutdown: Some(shutdown),
        }
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = timeout(BOUND, task).await;
        }
        self.endpoint.close().await;
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.as_ref() {
            task.abort();
        }
    }
}

async fn client(profile: &IrohObservationProfile) -> Endpoint {
    bind_endpoint(profile, key(2))
        .await
        .unwrap_or_else(|_| unreachable!())
}

fn state_request() -> ObservationRequest {
    ObservationRequest {
        state_revision: 1,
        view: ViewRequest::StateSummary,
    }
}

fn log_request() -> ObservationRequest {
    ObservationRequest {
        state_revision: 1,
        view: ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 0,
            max_bytes: 4,
        },
    }
}

fn event_request() -> ObservationRequest {
    ObservationRequest {
        state_revision: 1,
        view: ViewRequest::EventTail { limit: 1 },
    }
}

// r[verify kiln.observe.iroh.nonclaims.scenario.scope]
#[test]
fn profile_admission_accepts_complete_profile() {
    profile(key(2), false)
        .admit()
        .unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.observe.iroh.profile.scenario.match]
#[test]
fn nickel_iroh_profile_fixture_is_admitted() {
    let document: IrohProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/iroh-observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile.admit().unwrap_or_else(|_| unreachable!());
    assert_eq!(document.profile.profile_id, "iroh-observation-v1");
}

// r[verify kiln.observe.iroh.profile.scenario.drift]
#[test]
fn profile_admission_rejects_drift_and_missing_bounds() {
    let mut invalid = profile(key(2), false);
    invalid.cohort.iroh_version = String::from("0.95.0");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("iroh_cohort_drift")
    );
    let mut invalid = profile(key(2), false);
    invalid.cohort.alpn = String::from("bad");
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("iroh_cohort_drift")
    );
    let mut invalid = profile(key(2), false);
    invalid.endpoint.max_frame_bytes = 1;
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_iroh_bounds")
    );
    let mut invalid = profile(key(2), false);
    let _ = invalid.non_claims.pop();
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("missing_required_iroh_non_claim")
    );
    let mut invalid = profile(key(2), false);
    invalid.endpoint.relay_urls = vec![String::from("http://relay.example")];
    assert_eq!(
        invalid.admit().err().map(|error| error.code),
        Some("invalid_relay")
    );
}

#[test]
fn did_key_is_stable_and_aligned_with_endpoint_identity() {
    let first = did_key(key(2).public());
    let second = did_key(key(2).public());
    assert_eq!(first, second);
    assert!(first.starts_with("did:key:z"));
}

#[test]
fn payload_identity_binds_log_bytes_and_event_records() {
    let profile = profile(key(2), false);
    let source = MemoryObservationSource::new(snapshot());
    let snapshot = source.snapshot();
    let log_view = project_observation(
        &profile.core,
        &snapshot.state,
        &log_request(),
        &ObservationWindow::log(
            snapshot
                .log
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .meta
                .clone(),
            snapshot
                .log
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .retained_bytes
                .clone(),
        ),
    )
    .unwrap_or_else(|_| unreachable!());
    let payload = payload_identity(&log_view)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(payload.kind, PAYLOAD_LOG_BYTES);
    verify_payload(&log_view, Some(&payload)).unwrap_or_else(|_| unreachable!());
    let mut corrupted = payload.clone();
    corrupted.blake3 = "00".repeat(32);
    assert_eq!(
        verify_payload(&log_view, Some(&corrupted))
            .err()
            .map(|error| error.code),
        Some("iroh_payload_identity")
    );

    let event_view = project_observation(
        &profile.core,
        &snapshot.state,
        &event_request(),
        &ObservationWindow::events(
            snapshot
                .events
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .records
                .clone(),
        ),
    )
    .unwrap_or_else(|_| unreachable!());
    let payload = payload_identity(&event_view)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(payload.kind, PAYLOAD_EVENT_RECORDS);
    verify_payload(&event_view, Some(&payload)).unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.observe.iroh.equiv.scenario.divergence]
#[test]
fn equivalence_accepts_equal_views_and_rejects_divergence() {
    let profile = profile(key(2), false);
    let state = fixture_state();
    let request = state_request();
    let first = project_observation(&profile.core, &state, &request, &ObservationWindow::empty())
        .unwrap_or_else(|_| unreachable!());
    let second = project_observation(&profile.core, &state, &request, &ObservationWindow::empty())
        .unwrap_or_else(|_| unreachable!());
    admit_equivalence(&first, &second).unwrap_or_else(|_| unreachable!());
    let mut divergent = second;
    divergent.state_revision = 2;
    assert_eq!(
        admit_equivalence(&first, &divergent)
            .err()
            .map(|error| error.code),
        Some("iroh_view_divergence")
    );
}

// r[verify kiln.observe.iroh.conformance.scenario.fixture_missing]
#[test]
fn conformance_manifest_requires_all_cases() {
    let complete = IrohConformanceManifest {
        positive: REQUIRED_POSITIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
        negative: REQUIRED_NEGATIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
    };
    admit_iroh_conformance(&complete).unwrap_or_else(|_| unreachable!());
    let mut missing = complete.clone();
    missing
        .negative
        .retain(|case| case != "missing_negative_case");
    assert_eq!(
        admit_iroh_conformance(&missing)
            .err()
            .map(|error| error.code),
        Some("missing_negative_case")
    );
}

// r[verify kiln.observe.iroh.verify.scenario.corrupted]
#[test]
fn corrupted_payload_identity_is_rejected() {
    let profile = profile(key(2), false);
    let source = MemoryObservationSource::new(snapshot());
    let snapshot = source.snapshot();
    let view = project_observation(
        &profile.core,
        &snapshot.state,
        &log_request(),
        &ObservationWindow::log(
            snapshot
                .log
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .meta
                .clone(),
            snapshot
                .log
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .retained_bytes
                .clone(),
        ),
    )
    .unwrap_or_else(|_| unreachable!());
    let response = IrohObserveResponse::ok(view).unwrap_or_else(|_| unreachable!());
    let mut corrupted = response;
    if let IrohResult::Ok { view, .. } = &mut corrupted.result
        && let ViewBody::LogRange(range) = &mut view.body
    {
        range.bytes = Some(b"abd".to_vec());
    }
    let encoded = encode_response(&corrupted, 4_194_304).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        decode_response(&encoded, 4_194_304)
            .err()
            .map(|error| error.code),
        Some("iroh_payload_identity")
    );
}

// r[verify kiln.observe.iroh.serve.scenario.target]
// r[verify kiln.observe.iroh.peer.scenario.trusted]
// r[verify kiln.observe.iroh.equiv.scenario.equal_views]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_peer_serves_equal_http_and_iroh_views() {
    let server_profile = profile(key(2), false);
    let server = Server::start(server_profile).await;
    let client_profile = profile(key(1), false);
    let client = client(&client_profile).await;
    let response = timeout(
        BOUND,
        client_exchange(
            &client,
            remote_address(&server.endpoint),
            &client_profile,
            &state_request(),
        ),
    )
    .await
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|_| unreachable!());
    let iroh_view = logical_view(&response).unwrap_or_else(|| unreachable!());
    let http_view = project_observation(
        &client_profile.core,
        &fixture_state(),
        &state_request(),
        &ObservationWindow::empty(),
    )
    .unwrap_or_else(|_| unreachable!());
    admit_equivalence(&http_view, iroh_view).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        encode_view(&http_view, 4_194_304).unwrap_or_else(|_| unreachable!()),
        encode_view(iroh_view, 4_194_304).unwrap_or_else(|_| unreachable!())
    );
    client.close().await;
    server.stop().await;
}

// r[verify kiln.observe.iroh.verify.scenario.verified]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_range_delivers_verified_payload() {
    let server_profile = profile(key(2), false);
    let server = Server::start(server_profile).await;
    let client_profile = profile(key(1), false);
    let client = client(&client_profile).await;
    let response = timeout(
        BOUND,
        client_exchange(
            &client,
            remote_address(&server.endpoint),
            &client_profile,
            &log_request(),
        ),
    )
    .await
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|_| unreachable!());
    let view = logical_view(&response).unwrap_or_else(|| unreachable!());
    match &view.body {
        ViewBody::LogRange(range) => {
            assert_eq!(range.bytes.as_deref(), Some(b"abc".as_slice()));
        }
        _ => unreachable!(),
    }
    let IrohResult::Ok { payload, .. } = &response.result else {
        unreachable!()
    };
    verify_payload(view, payload.as_ref()).unwrap_or_else(|_| unreachable!());
    client.close().await;
    server.stop().await;
}

// r[verify kiln.observe.iroh.peer.scenario.denied]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_and_revoked_peers_are_denied() {
    for revoked in [false, true] {
        let server_profile = if revoked {
            profile(key(2), true)
        } else {
            profile(key(3), false)
        };
        let server = Server::start(server_profile).await;
        let client_profile = profile(key(1), false);
        let client = client(&client_profile).await;
        let result = timeout(
            BOUND,
            client_exchange(
                &client,
                remote_address(&server.endpoint),
                &client_profile,
                &state_request(),
            ),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        assert!(result.is_err());
        client.close().await;
        server.stop().await;
    }
}

// r[verify kiln.observe.iroh.serve.scenario.transport_failure]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_failure_is_bounded_and_never_claims_a_view() {
    let server_profile = profile(key(1), false);
    let server_endpoint = bind_endpoint(&server_profile, key(1))
        .await
        .unwrap_or_else(|_| unreachable!());
    let address = remote_address(&server_endpoint);
    server_endpoint.close().await;
    let client_profile = profile(key(1), false);
    let client = client(&client_profile).await;
    let result = timeout(
        BOUND,
        client_exchange(&client, address, &client_profile, &state_request()),
    )
    .await
    .unwrap_or_else(|_| unreachable!());
    assert!(result.is_err());
    client.close().await;
}
