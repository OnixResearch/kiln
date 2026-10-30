//! A fixture knot drives the real HTTP shell, Git acquisition and signed status
//! delivery over loopback.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::shell::{FIXTURE_TIMEOUT_MS, private_root, read_http};
use super::*;
use crate::auth::verify_webhook_signature;
use crate::nsid;
use crate::resolver::StaticKeyResolver;
use crate::server::{ServerLimits, serve_on};
use crate::service::TangledService;
use crate::status::HttpStatusPublisher;
use crate::store::{SqliteStore, StoreLimits};

const POLL_MS: u64 = 20;
const STATUS_SECRET: &[u8] = b"fixture-status-secret";

struct Delivery {
    signed: bool,
    view: Value,
}

fn status_endpoint(stop: Arc<AtomicBool>) -> (String, Arc<Mutex<Vec<Delivery>>>) {
    let listener = ok(TcpListener::bind("127.0.0.1:0"));
    ok(listener.set_nonblocking(true));
    let url = format!(
        "http://127.0.0.1:{}/kiln-status",
        ok(listener.local_addr()).port()
    );
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(POLL_MS));
                continue;
            };
            let _ = stream.set_nonblocking(false);
            let captured = read_http(&mut stream);
            let signed = captured
                .headers
                .get("x-tangled-signature-256")
                .is_some_and(|signature| {
                    verify_webhook_signature(signature, &captured.body, STATUS_SECRET).is_ok()
                });
            let view = serde_json::from_slice(&captured.body).unwrap_or(Value::Null);
            if let Ok(mut deliveries) = sink.lock() {
                deliveries.push(Delivery { signed, view });
            }
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (url, received)
}

fn http(address: SocketAddr, request: &str, body: &[u8]) -> (u16, Value) {
    let mut stream = ok(TcpStream::connect(address));
    ok(stream.set_read_timeout(Some(Duration::from_millis(FIXTURE_TIMEOUT_MS))));
    ok(stream.write_all(request.as_bytes()));
    ok(stream.write_all(body));
    let mut response = Vec::new();
    ok(stream.read_to_end(&mut response));
    let text = String::from_utf8_lossy(&response).to_string();
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

fn post(address: SocketAddr, method: &str, token: &str, body: &Value) -> (u16, Value) {
    let bytes = ok(serde_json::to_vec(body));
    let request = format!(
        "POST /xrpc/{method} HTTP/1.1\r\nHost: kiln\r\nContent-Type: application/json\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n",
        bytes.len()
    );
    http(address, &request, &bytes)
}

fn get(address: SocketAddr, path: &str) -> (u16, Value) {
    http(
        address,
        &format!("GET {path} HTTP/1.1\r\nHost: kiln\r\n\r\n"),
        b"",
    )
}

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(FIXTURE_TIMEOUT_MS * 2);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(POLL_MS));
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

fn statuses_of(view: &Value) -> Vec<String> {
    field(view, "/workflows")
        .as_array()
        .map(|runs| {
            runs.iter()
                .filter_map(|run| field(run, "/status").as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn delivered(received: &Arc<Mutex<Vec<Delivery>>>, pipeline: &str, statuses: &[&str]) -> bool {
    received.lock().is_ok_and(|deliveries| {
        deliveries.iter().any(|delivery| {
            delivery.signed
                && field(&delivery.view, "/id") == pipeline
                && statuses_of(&delivery.view) == statuses
        })
    })
}

// r[verify kiln.tangled.source.scenario.exact] r[verify kiln.tangled.status.scenario.published]
// r[verify kiln.tangled.replay.scenario.push] r[verify kiln.tangled.untrusted.scenario.held]
// r[verify kiln.tangled.events.scenario.malformed] r[verify kiln.tangled.identity.scenario.unknown-repo]
#[test]
fn fixture_knot_push_over_http_publishes_status_back() {
    let root = private_root();
    let head = object('b', 64);
    let source = FixtureSource {
        head: Some(head.clone()),
        ..FixtureSource::default()
    };
    exercise(&root, profile(), source, object('a', 64), head);
}

/// The same flow with exact Git acquisition from a local SHA-256 knot repository.
#[cfg(feature = "git-fixtures")]
#[test]
fn fixture_knot_push_with_git_acquisition_runs_exact_revision() {
    use super::shell::{fixture_repository, git_limits, git_program};
    use crate::identity::ObjectFormat;
    use crate::source::GitCliSource;

    let root = private_root();
    let (repository, base, head) = fixture_repository(root.path(), ObjectFormat::Sha256);
    let mut admitted = profile();
    if let Some(pin) = admitted.profile.repositories.first_mut() {
        // The fixture knot serves the local repository.
        pin.clone_url = format!("file://{}", repository.display());
    }
    let source = ok(GitCliSource::fixture(
        git_program(),
        root.path().join("stores"),
        git_limits(),
    ));
    exercise(&root, admitted, source, base, head);
}

fn exercise<S: SourcePort + Send + 'static>(
    root: &tempfile::TempDir,
    admitted: AdmittedProfile,
    source: S,
    base: ObjectId,
    head: ObjectId,
) {
    let stop = Arc::new(AtomicBool::new(false));
    let (endpoint, received) = status_endpoint(Arc::clone(&stop));
    let store = ok(SqliteStore::open(
        &root.path().join("tangled.sqlite"),
        StoreLimits {
            maximum_pipelines: 16,
            maximum_tokens: 64,
            maximum_record_bytes: 1_048_576,
        },
    ));
    let status = ok(HttpStatusPublisher::fixture(
        &endpoint,
        STATUS_SECRET.to_vec(),
        FIXTURE_TIMEOUT_MS,
    ));
    let workflow = FixtureWorkflow::new();
    let service = Arc::new(Mutex::new(TangledService::new(
        admitted.clone(),
        store,
        source,
        workflow.clone(),
        status,
        StaticKeyResolver::new(issuer_keys()),
    )));
    let listener = ok(TcpListener::bind("127.0.0.1:0"));
    let address = ok(listener.local_addr());
    let server = {
        let (service, stop) = (Arc::clone(&service), Arc::clone(&stop));
        let limits = ServerLimits::from_profile(&admitted);
        std::thread::spawn(move || serve_on(listener, service, limits, stop))
    };

    let mut nonce = 0;
    let mut token = |issuer: &str, method: &str| {
        nonce += 1;
        let signer = if issuer == KNOT {
            Signer::K256(knot_key())
        } else {
            Signer::P256(actor_key())
        };
        sign_token(
            &signer,
            &claims(issuer, method, now(), &format!("e2e-{nonce}")),
        )
    };
    let push = |repo: &str| {
        json!({ "repo": repo, "event": {
            "$type": nsid::PUSH, "repo": repo, "ref": "refs/heads/main",
            "oldSha": base.as_str(), "newSha": head.as_str(), "actor": TRUSTED
        }})
    };

    let (status, health) = get(address, "/_health");
    assert_eq!((status, field(&health, "/status")), (200, &json!("ok")));

    // The knot notifies a push; Kiln acquires the exact commit and runs it.
    let first_token = token(KNOT, nsid::NOTIFY_EVENT);
    let (status, body) = post(address, nsid::NOTIFY_EVENT, &first_token, &push(REPO));
    assert_eq!(status, 200, "{body}");
    let pipeline = some(field(&body, "/ci/pipelines/0").as_str()).to_string();
    wait_for("signed success status", || {
        delivered(&received, &pipeline, &["success"])
    });
    let plans = workflow.dispatched();
    assert_eq!(plans.len(), 1);
    assert_eq!(
        some(some(plans.first()).source.as_ref())
            .revision
            .native_id
            .as_str(),
        head.as_str()
    );
    let (status, view) = get(
        address,
        &format!(
            "/xrpc/{}?repo={REPO}&pipeline={pipeline}",
            nsid::GET_PIPELINE
        ),
    );
    assert_eq!(
        (status, statuses_of(&view)),
        (200, vec!["success".to_string()])
    );
    assert_eq!(field(&view, "/commit"), head.as_str());

    // Redelivery under a fresh token: same pipeline, no new run.
    let (status, again) = post(
        address,
        nsid::NOTIFY_EVENT,
        &token(KNOT, nsid::NOTIFY_EVENT),
        &push(REPO),
    );
    assert_eq!(
        (status, field(&again, "/ci/pipelines/0").as_str()),
        (200, Some(pipeline.as_str()))
    );
    // The same token twice is refused.
    let (status, _) = post(address, nsid::NOTIFY_EVENT, &first_token, &push(REPO));
    assert_eq!(status, 401);
    let rogue = sign_token(
        &Signer::K256(rogue_key()),
        &claims(KNOT, nsid::NOTIFY_EVENT, now(), "rogue"),
    );
    assert_eq!(
        post(address, nsid::NOTIFY_EVENT, &rogue, &push(REPO)).0,
        401
    );
    assert_eq!(
        post(
            address,
            nsid::NOTIFY_EVENT,
            &token(KNOT, nsid::NOTIFY_EVENT),
            &push(OTHER_REPO)
        )
        .0,
        404
    );
    assert_eq!(
        post(
            address,
            nsid::NOTIFY_EVENT,
            &token(KNOT, nsid::NOTIFY_EVENT),
            &json!({ "repo": REPO })
        )
        .0,
        400
    );
    let chunked = format!(
        "POST /xrpc/{} HTTP/1.1\r\nHost: kiln\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        nsid::NOTIFY_EVENT
    );
    assert_eq!(http(address, &chunked, b"").0, 411);
    assert_eq!(
        http(
            address,
            &format!(
                "POST /xrpc/{} HTTP/1.1\r\nHost: kiln\r\n\r\n",
                nsid::NOTIFY_EVENT
            ),
            b""
        )
        .0,
        411
    );
    assert_eq!(
        get(address, &format!("/xrpc/{}?pipeline=x", nsid::GET_PIPELINE)).0,
        400
    );
    {
        let service = ok(service.lock().map_err(|_| "poisoned"));
        assert_eq!(ok(service.store().pipeline_count()), 1);
        assert_eq!(ok(service.store().run_count()), 1);
    }
    assert_eq!(workflow.dispatched().len(), 1);

    // Workflow definitions at the exact branch tip.
    let (status, listed) = get(
        address,
        &format!("/xrpc/{}?repo={REPO}&ref=main", nsid::LIST_WORKFLOWS),
    );
    assert_eq!(status, 200, "{listed}");
    let mut ids: Vec<&str> = field(&listed, "/workflows")
        .as_array()
        .map(|w| w.iter().filter_map(|w| field(w, "/id").as_str()).collect())
        .unwrap_or_default();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec!["docs.yml", "test.yml"],
        "unreviewable definitions are not listed"
    );

    // An untrusted fork pull request is held until an approver approves it.
    let pull = json!({ "repo": REPO, "trigger": {
        "$type": nsid::PULL_REQUEST, "action": "opened", "pull": format!("at://{REPO}/org.tangled.track.ticket/3lzq"),
        "targetBranch": "main", "sourceBranch": "feature", "sourceRepo": OTHER_REPO, "sourceSha": base.as_str()
    }});
    let (status, body) = post(
        address,
        nsid::TRIGGER_PIPELINE,
        &token(TRUSTED, nsid::TRIGGER_PIPELINE),
        &pull,
    );
    assert_eq!(status, 200, "{body}");
    let held = some(field(&body, "/ci/pipelines/0").as_str()).to_string();
    wait_for("blocked status", || {
        delivered(&received, &held, &["blocked"])
    });
    assert_eq!(
        workflow.dispatched().len(),
        1,
        "held pipelines dispatch nothing"
    );
    let approve = json!({ "repo": REPO, "pipeline": held });
    assert_eq!(
        post(
            address,
            nsid::APPROVE_PIPELINE,
            &token(TRUSTED, nsid::APPROVE_PIPELINE),
            &approve
        )
        .0,
        403
    );
    assert_eq!(
        post(
            address,
            nsid::APPROVE_PIPELINE,
            &token(APPROVER, nsid::APPROVE_PIPELINE),
            &approve
        )
        .0,
        200
    );
    wait_for("approved run status", || {
        delivered(&received, &held, &["success"])
    });
    let plans = workflow.dispatched();
    assert_eq!(plans.len(), 2);
    let approved = some(plans.last());
    assert_eq!(approved.workflow_policy_id.as_str(), "tangled-untrusted-v1");
    assert_eq!(
        some(approved.source.as_ref()).revision.native_id.as_str(),
        base.as_str()
    );

    stop.store(true, Ordering::Release);
    assert!(ok(server.join().map_err(|_| "server thread")).is_ok());
}
