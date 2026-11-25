#![cfg(feature = "protocol-fixtures")]

use hmac::{Hmac, Mac};
use kiln_adapter_github::events::EventPolicy;
use kiln_adapter_github::implementation::ImplementationIssuePolicy;
use kiln_adapter_github::runbook_routing::{
    CommandRunbookRoute, LabelRunbookRoute, RoutingParamValue, RunbookRoutingMode,
    RunbookRoutingPolicy,
};
use kiln_core::implementation::{AcceptanceObligation, Enrollment, TerminalPolicy, WorkBudget};
use kiln_core::types::ContentId;
use kiln_implementer::{DeliveryQueue, RunbookRoutingStore};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SECRET: &[u8] = b"cli-routing-secret";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sign(body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).unwrap_or_else(|_| unreachable!());
    mac.update(body);
    format!("sha256={}", hex(&mac.finalize().into_bytes()))
}

fn routing_policy() -> ImplementationIssuePolicy {
    ImplementationIssuePolicy {
        repository: EventPolicy {
            owner: "octocat".to_string(),
            repository: "hello-world".to_string(),
            repository_id: 1296269,
            allow_fork_pull_requests: false,
            untrusted_pull_requests: Default::default(),
        },
        project: "kiln".to_string(),
        base: None,
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: ContentId::from_blake3_hex("a".repeat(64))
            .unwrap_or_else(|_| unreachable!()),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        selection: None,
        runbook_routing: Some(RunbookRoutingPolicy {
            allowed_actors: vec!["operator".to_string()],
            allow_issue_comments: true,
            allow_pull_request_comments: true,
            require_push_association: true,
            console_base_url: Some("https://kiln.example.invalid".to_string()),
            labels: vec![LabelRunbookRoute {
                label: "runbook:flake-update".to_string(),
                runbook_id: "flake-update".to_string(),
                target: None,
                target_ref: Some("refs/heads/main".to_string()),
                params: BTreeMap::new(),
                mode: RunbookRoutingMode::Dispatch,
                fanout: None,
            }],
            commands: vec![CommandRunbookRoute {
                command: "flake-update".to_string(),
                runbook_id: "flake-update".to_string(),
                target: Some("octocat/hello-world@refs/heads/main".to_string()),
                target_ref: None,
                params: BTreeMap::from([("dry_run".to_string(), RoutingParamValue::Bool(true))]),
                mode: RunbookRoutingMode::Dispatch,
                fanout: None,
            }],
        }),
    }
}

fn read_http_request(stream: &mut TcpStream) -> String {
    const MAX_HEADER_BYTES: usize = 65_536;
    const MAX_BODY_BYTES: usize = 1_048_576;
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    let deadline = Instant::now() + READ_TIMEOUT;
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 4096];

    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() >= MAX_HEADER_BYTES {
            unreachable!("mock HTTP request headers exceeded bound");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            unreachable!("mock HTTP request header read timed out");
        }
        stream
            .set_read_timeout(Some(remaining))
            .unwrap_or_else(|_| unreachable!());
        let max_read = chunk.len().min(MAX_HEADER_BYTES.saturating_sub(buf.len()));
        let slice = chunk.get_mut(..max_read).unwrap_or_else(|| unreachable!());
        let read = match stream.read(slice) {
            Ok(n) => n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => unreachable!("mock HTTP request header read failed: {err}"),
        };
        if read == 0 {
            unreachable!("mock HTTP request closed before header terminator");
        }
        buf.extend_from_slice(chunk.get(..read).unwrap_or_else(|| unreachable!()));
    };

    let headers_str = std::str::from_utf8(buf.get(..header_end).unwrap_or_else(|| unreachable!()))
        .unwrap_or_else(|_| unreachable!("mock HTTP request headers are not UTF-8"));

    let mut content_length: Option<usize> = None;
    for line in headers_str.split("\r\n").skip(1) {
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            unreachable!("mock HTTP request does not support Transfer-Encoding: {value}");
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .unwrap_or_else(|_| unreachable!("invalid Content-Length: {value}"));
            if parsed > MAX_BODY_BYTES {
                unreachable!("mock HTTP request body exceeded bound: {parsed}");
            }
            if let Some(prev) = content_length
                && prev != parsed
            {
                unreachable!("conflicting Content-Length headers: {prev} vs {parsed}");
            }
            content_length = Some(parsed);
        }
    }

    let body_len = content_length.unwrap_or(0);
    let total_len = header_end
        .checked_add(body_len)
        .unwrap_or_else(|| unreachable!());

    while buf.len() < total_len {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            unreachable!("mock HTTP request body read timed out");
        }
        stream
            .set_read_timeout(Some(remaining))
            .unwrap_or_else(|_| unreachable!());
        let want = chunk.len().min(total_len.saturating_sub(buf.len()));
        let slice = chunk.get_mut(..want).unwrap_or_else(|| unreachable!());
        let read = match stream.read(slice) {
            Ok(n) => n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => unreachable!("mock HTTP request body read failed: {err}"),
        };
        if read == 0 {
            unreachable!("mock HTTP request closed before Content-Length body arrived");
        }
        buf.extend_from_slice(chunk.get(..read).unwrap_or_else(|| unreachable!()));
    }

    String::from_utf8_lossy(buf.get(..total_len).unwrap_or_default()).into_owned()
}

fn spawn_mock_server(
    responses: Vec<(&'static str, &'static str)>,
) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
    let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
    let handle = std::thread::spawn(move || {
        let mut recorded = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
            recorded.push(read_http_request(&mut stream));
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(reply.as_bytes())
                .unwrap_or_else(|_| unreachable!());
        }
        recorded
    });
    (format!("http://{addr}"), handle)
}

struct ConcurrentMockHandle {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<Vec<String>>,
}

impl ConcurrentMockHandle {
    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn stop(self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.addr);
        self.handle.join().unwrap_or_else(|_| unreachable!())
    }
}

fn spawn_concurrent_mock_server<F>(handler: F) -> ConcurrentMockHandle
where
    F: Fn(&str) -> (&'static str, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
    let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
    let stop = Arc::new(AtomicBool::new(false));
    let stop_accept = Arc::clone(&stop);
    let handler = Arc::new(handler);

    let handle = std::thread::spawn(move || {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let mut workers = Vec::new();
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => break,
            };
            if stop_accept.load(Ordering::Acquire) {
                break;
            }
            let recorded_worker = Arc::clone(&recorded);
            let handler_worker = Arc::clone(&handler);
            workers.push(std::thread::spawn(move || {
                let req = read_http_request(&mut stream);
                let (status, body) = handler_worker(&req);
                recorded_worker
                    .lock()
                    .unwrap_or_else(|_| unreachable!())
                    .push(req);
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }));
        }
        for worker in workers {
            worker.join().unwrap_or_else(|_| unreachable!());
        }
        let guard = recorded.lock().unwrap_or_else(|_| unreachable!());
        guard.clone()
    });

    ConcurrentMockHandle { addr, stop, handle }
}

#[test]
fn drain_runbooks_and_poll_runbooks_cli_route_and_audit() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let db_path = dir.path().join("routing.sqlite");
    let queue_path = dir.path().join("queue.sqlite");
    let secret_file = dir.path().join("webhook.secret");
    let gh_token_file = dir.path().join("gh.token");
    let svc_token_file = dir.path().join("svc.token");
    let policy_file = dir.path().join("policy.json");
    let enrollment_file = dir.path().join("enrollment.json");

    std::fs::write(&secret_file, SECRET).unwrap_or_else(|_| unreachable!());
    std::fs::write(&gh_token_file, b"gh-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(&svc_token_file, b"svc-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &policy_file,
        serde_json::to_vec(&routing_policy()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    std::fs::write(
        &enrollment_file,
        serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());

    let comment_body = serde_json::to_vec(&serde_json::json!({
        "action": "created",
        "repository": {"id": 1296269, "name": "hello-world", "full_name": "octocat/hello-world", "owner": {"login": "octocat"}},
        "sender": {"login": "operator", "type": "User"},
        "issue": {"number": 8, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
        "comment": {"id": 801, "body": "/runbook flake-update", "user": {"login": "operator", "type": "User"}, "author_association": "OWNER"}
    }))
    .unwrap_or_else(|_| unreachable!());

    let mut queue = DeliveryQueue::open(&queue_path, 8).unwrap_or_else(|_| unreachable!());
    queue
        .record(
            "00000000-0000-0000-0000-000000000051",
            "issue_comment",
            &sign(&comment_body),
            &comment_body,
            10,
        )
        .unwrap_or_else(|_| unreachable!());

    let (gh_origin, gh_handle) = spawn_mock_server(vec![
        ("201 Created", r#"{"id":9001}"#),
        // poll-runbooks: list_issue_comments + list_issues_for_routing
        (
            "200 OK",
            r#"[{"id":801,"body":"/runbook flake-update","user":{"login":"operator","type":"User"},"author_association":"OWNER","created_at":"2026-11-11T10:00:00Z","updated_at":"2026-11-11T10:00:00Z","issue_url":"https://api.github.com/repos/octocat/hello-world/issues/8","html_url":"https://github.com/octocat/hello-world/issues/8#issuecomment-801"}]"#,
        ),
        ("200 OK", r#"[]"#),
    ]);
    let (svc_origin, svc_handle) = spawn_mock_server(vec![(
        "201 Created",
        r#"{"request_id":"gh-rb-1","replayed":false,"runbook_id":"flake-update","target":"octocat/hello-world@refs/heads/main","source_revision":"1111111111111111111111111111111111111111","run_id":"web-run-42"}"#,
    )]);

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["drain-runbooks", "--database"])
        .arg(&db_path)
        .arg("--queue")
        .arg(&queue_path)
        .arg("--secret-file")
        .arg(&secret_file)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args([
            "--maximum-deliveries",
            "8",
            "--limit",
            "8",
            "--service-origin",
        ])
        .arg(&svc_origin)
        .arg("--service-token-file")
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", &gh_origin)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "drain-runbooks failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(summary.get("dispatched").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        summary.get("comments_posted").and_then(|v| v.as_u64()),
        Some(1)
    );

    // poll-runbooks overlapping the same comment_id=801 must replay without dispatching or reposting.
    let poll_out = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["poll-runbooks", "--database"])
        .arg(&db_path)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args(["--max-pages", "1", "--service-origin"])
        .arg(&svc_origin)
        .arg("--service-token-file")
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", &gh_origin)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        poll_out.status.success(),
        "poll-runbooks failed: {}",
        String::from_utf8_lossy(&poll_out.stderr)
    );
    let poll_sum: serde_json::Value =
        serde_json::from_slice(&poll_out.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(poll_sum.get("dispatched").and_then(|v| v.as_u64()), Some(0));
    assert_eq!(poll_sum.get("replayed").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        poll_sum.get("comments_posted").and_then(|v| v.as_u64()),
        Some(0)
    );

    let gh_reqs = gh_handle.join().unwrap_or_else(|_| unreachable!());
    assert_eq!(gh_reqs.len(), 3);
    assert!(
        gh_reqs
            .first()
            .unwrap_or_else(|| unreachable!())
            .contains("POST /repos/octocat/hello-world/issues/8/comments")
    );
    let svc_reqs = svc_handle.join().unwrap_or_else(|_| unreachable!());
    assert_eq!(svc_reqs.len(), 1);

    let store = RunbookRoutingStore::open(&db_path, 16).unwrap_or_else(|_| unreachable!());
    let audit = store.list_audit(10).unwrap_or_else(|_| unreachable!());
    assert_eq!(audit.len(), 1);
    assert_eq!(
        audit.first().map(|e| e.decision_code.as_str()),
        Some("dispatched")
    );
}

#[test]
fn drain_runbooks_and_poll_runbooks_cli_reject_unenrolled_repository() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let db_path = dir.path().join("routing.sqlite");
    let queue_path = dir.path().join("queue.sqlite");
    let secret_file = dir.path().join("webhook.secret");
    let gh_token_file = dir.path().join("gh.token");
    let svc_token_file = dir.path().join("svc.token");
    let policy_file = dir.path().join("policy.json");
    let enrollment_file = dir.path().join("enrollment.json");

    std::fs::write(&secret_file, SECRET).unwrap_or_else(|_| unreachable!());
    std::fs::write(&gh_token_file, b"gh-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(&svc_token_file, b"svc-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &policy_file,
        serde_json::to_vec(&routing_policy()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let unenrolled = Enrollment {
        repositories: BTreeSet::from(["octocat/other-repo".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    std::fs::write(
        &enrollment_file,
        serde_json::to_vec(&unenrolled).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());

    let drain_out = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["drain-runbooks", "--database"])
        .arg(&db_path)
        .arg("--queue")
        .arg(&queue_path)
        .arg("--secret-file")
        .arg(&secret_file)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args([
            "--maximum-deliveries",
            "8",
            "--limit",
            "8",
            "--service-origin",
            "http://127.0.0.1:9",
            "--service-token-file",
        ])
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", "http://127.0.0.1:9")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!drain_out.status.success());
    let drain_err = String::from_utf8_lossy(&drain_out.stderr);
    assert!(
        drain_err.contains("implementation_unenrolled"),
        "expected implementation_unenrolled, got: {drain_err}"
    );
    assert!(!db_path.exists());

    let poll_out = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["poll-runbooks", "--database"])
        .arg(&db_path)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args([
            "--max-pages",
            "1",
            "--service-origin",
            "http://127.0.0.1:9",
            "--service-token-file",
        ])
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", "http://127.0.0.1:9")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!poll_out.status.success());
    let poll_err = String::from_utf8_lossy(&poll_out.stderr);
    assert!(
        poll_err.contains("implementation_unenrolled"),
        "expected implementation_unenrolled, got: {poll_err}"
    );
    assert!(!db_path.exists());
}

#[test]
fn concurrent_drain_runbooks_dispatches_and_comments_once() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let db_path = dir.path().join("routing.sqlite");
    let queue_path = dir.path().join("queue.sqlite");
    let secret_file = dir.path().join("webhook.secret");
    let gh_token_file = dir.path().join("gh.token");
    let svc_token_file = dir.path().join("svc.token");
    let policy_file = dir.path().join("policy.json");
    let enrollment_file = dir.path().join("enrollment.json");

    std::fs::write(&secret_file, SECRET).unwrap_or_else(|_| unreachable!());
    std::fs::write(&gh_token_file, b"gh-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(&svc_token_file, b"svc-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &policy_file,
        serde_json::to_vec(&routing_policy()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    std::fs::write(
        &enrollment_file,
        serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());

    let comment_body = serde_json::to_vec(&serde_json::json!({
        "action": "created",
        "repository": {"id": 1296269, "name": "hello-world", "full_name": "octocat/hello-world", "owner": {"login": "octocat"}},
        "sender": {"login": "operator", "type": "User"},
        "issue": {"number": 8, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
        "comment": {"id": 801, "body": "/runbook flake-update", "user": {"login": "operator", "type": "User"}, "author_association": "OWNER"}
    }))
    .unwrap_or_else(|_| unreachable!());

    let mut queue = DeliveryQueue::open(&queue_path, 8).unwrap_or_else(|_| unreachable!());
    queue
        .record(
            "00000000-0000-0000-0000-000000000061",
            "issue_comment",
            &sign(&comment_body),
            &comment_body,
            10,
        )
        .unwrap_or_else(|_| unreachable!());
    drop(queue);

    let gh_server = spawn_concurrent_mock_server(|req| {
        if req.starts_with("POST /repos/octocat/hello-world/issues/8/comments ") {
            std::thread::sleep(Duration::from_millis(350));
            ("201 Created", r#"{"id":9001}"#.to_string())
        } else {
            ("200 OK", "[]".to_string())
        }
    });
    let svc_server = spawn_concurrent_mock_server(|req| {
        std::thread::sleep(Duration::from_millis(100));
        let req_id = req
            .split_once("\r\n\r\n")
            .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
            .and_then(|val| {
                val.get("request_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "gh-rb-1".to_string());
        (
            "201 Created",
            serde_json::json!({
                "request_id": req_id,
                "replayed": false,
                "runbook_id": "flake-update",
                "target": "octocat/hello-world@refs/heads/main",
                "source_revision": "1111111111111111111111111111111111111111",
                "run_id": "web-run-42"
            })
            .to_string(),
        )
    });

    let gh_origin = gh_server.origin();
    let svc_origin = svc_server.origin();
    let spawn_drain = || {
        Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
            .args(["drain-runbooks", "--database"])
            .arg(&db_path)
            .arg("--queue")
            .arg(&queue_path)
            .arg("--secret-file")
            .arg(&secret_file)
            .arg("--policy")
            .arg(&policy_file)
            .arg("--enrollment")
            .arg(&enrollment_file)
            .args([
                "--maximum-deliveries",
                "8",
                "--limit",
                "8",
                "--service-origin",
            ])
            .arg(&svc_origin)
            .arg("--service-token-file")
            .arg(&svc_token_file)
            .arg("--github-token-file")
            .arg(&gh_token_file)
            .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", &gh_origin)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|_| unreachable!())
    };

    let child1 = spawn_drain();
    let child2 = spawn_drain();
    let out1 = child1.wait_with_output().unwrap_or_else(|_| unreachable!());
    let out2 = child2.wait_with_output().unwrap_or_else(|_| unreachable!());

    assert!(
        out1.status.success(),
        "drain 1 failed: {}",
        String::from_utf8_lossy(&out1.stderr)
    );
    assert!(
        out2.status.success(),
        "drain 2 failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    let sum1: serde_json::Value =
        serde_json::from_slice(&out1.stdout).unwrap_or_else(|_| unreachable!());
    let sum2: serde_json::Value =
        serde_json::from_slice(&out2.stdout).unwrap_or_else(|_| unreachable!());
    let count = |val: &serde_json::Value, key: &str| -> u64 {
        val.get(key)
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| unreachable!())
    };
    assert_eq!(count(&sum1, "dispatched") + count(&sum2, "dispatched"), 1);
    assert_eq!(
        count(&sum1, "comments_posted") + count(&sum2, "comments_posted"),
        1
    );
    assert_eq!(count(&sum1, "dispatch_errors"), 0);
    assert_eq!(count(&sum2, "dispatch_errors"), 0);
    assert_eq!(count(&sum1, "comment_errors"), 0);
    assert_eq!(count(&sum2, "comment_errors"), 0);

    let gh_reqs = gh_server.stop();
    let svc_reqs = svc_server.stop();
    assert_eq!(
        svc_reqs
            .iter()
            .filter(|req| req.starts_with("POST /api/runbooks/flake-update/dispatch "))
            .count(),
        1
    );
    assert_eq!(
        gh_reqs
            .iter()
            .filter(|req| req.starts_with("POST /repos/octocat/hello-world/issues/8/comments "))
            .count(),
        1
    );

    let store = RunbookRoutingStore::open(&db_path, 16).unwrap_or_else(|_| unreachable!());
    let audit = store.list_audit(20).unwrap_or_else(|_| unreachable!());
    assert_eq!(audit.len(), 1);
    assert_eq!(
        audit.first().map(|e| e.decision_code.as_str()),
        Some("dispatched")
    );
    let queue_check = DeliveryQueue::open(&queue_path, 8).unwrap_or_else(|_| unreachable!());
    assert_eq!(queue_check.is_empty(), Ok(true));
}

#[test]
fn concurrent_drain_and_poll_runbooks_for_same_comment_dispatch_and_comment_once() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let db_path = dir.path().join("routing.sqlite");
    let queue_path = dir.path().join("queue.sqlite");
    let secret_file = dir.path().join("webhook.secret");
    let gh_token_file = dir.path().join("gh.token");
    let svc_token_file = dir.path().join("svc.token");
    let policy_file = dir.path().join("policy.json");
    let enrollment_file = dir.path().join("enrollment.json");

    std::fs::write(&secret_file, SECRET).unwrap_or_else(|_| unreachable!());
    std::fs::write(&gh_token_file, b"gh-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(&svc_token_file, b"svc-token-fixture").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &policy_file,
        serde_json::to_vec(&routing_policy()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    std::fs::write(
        &enrollment_file,
        serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());

    let comment_body = serde_json::to_vec(&serde_json::json!({
        "action": "created",
        "repository": {"id": 1296269, "name": "hello-world", "full_name": "octocat/hello-world", "owner": {"login": "octocat"}},
        "sender": {"login": "operator", "type": "User"},
        "issue": {"number": 9, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
        "comment": {"id": 802, "body": "/runbook flake-update", "user": {"login": "operator", "type": "User"}, "author_association": "OWNER"}
    }))
    .unwrap_or_else(|_| unreachable!());

    let mut queue = DeliveryQueue::open(&queue_path, 8).unwrap_or_else(|_| unreachable!());
    queue
        .record(
            "00000000-0000-0000-0000-000000000062",
            "issue_comment",
            &sign(&comment_body),
            &comment_body,
            10,
        )
        .unwrap_or_else(|_| unreachable!());
    drop(queue);

    let posted_comments = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let posted_for_gh = Arc::clone(&posted_comments);
    let gh_server = spawn_concurrent_mock_server(move |req| {
        if req.starts_with("POST /repos/octocat/hello-world/issues/9/comments ") {
            std::thread::sleep(Duration::from_millis(350));
            let body_text = req
                .split_once("\r\n\r\n")
                .and_then(|(_, b)| serde_json::from_str::<serde_json::Value>(b).ok())
                .and_then(|v| v.get("body").and_then(|b| b.as_str()).map(str::to_string))
                .unwrap_or_default();
            posted_for_gh
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .push(serde_json::json!({
                    "id": 9002,
                    "body": body_text,
                    "user": {"login": "kiln-bot", "type": "Bot"}
                }));
            ("201 Created", r#"{"id":9002}"#.to_string())
        } else if req.starts_with("GET /repos/octocat/hello-world/issues/9/comments") {
            let guard = posted_for_gh.lock().unwrap_or_else(|_| unreachable!());
            (
                "200 OK",
                serde_json::to_string(&*guard).unwrap_or_else(|_| unreachable!()),
            )
        } else if req.starts_with("GET /repos/octocat/hello-world/issues/comments") {
            (
                "200 OK",
                r#"[{"id":802,"body":"/runbook flake-update","user":{"login":"operator","type":"User"},"author_association":"OWNER","created_at":"2026-11-11T10:00:00Z","updated_at":"2026-11-11T10:00:00Z","issue_url":"https://api.github.com/repos/octocat/hello-world/issues/9","html_url":"https://github.com/octocat/hello-world/issues/9#issuecomment-802"}]"#.to_string(),
            )
        } else {
            ("200 OK", "[]".to_string())
        }
    });
    let svc_server = spawn_concurrent_mock_server(|req| {
        std::thread::sleep(Duration::from_millis(100));
        let req_id = req
            .split_once("\r\n\r\n")
            .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
            .and_then(|val| {
                val.get("request_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "gh-rb-2".to_string());
        (
            "201 Created",
            serde_json::json!({
                "request_id": req_id,
                "replayed": false,
                "runbook_id": "flake-update",
                "target": "octocat/hello-world@refs/heads/main",
                "source_revision": "1111111111111111111111111111111111111111",
                "run_id": "web-run-43"
            })
            .to_string(),
        )
    });

    let gh_origin = gh_server.origin();
    let svc_origin = svc_server.origin();

    let drain_cmd = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["drain-runbooks", "--database"])
        .arg(&db_path)
        .arg("--queue")
        .arg(&queue_path)
        .arg("--secret-file")
        .arg(&secret_file)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args([
            "--maximum-deliveries",
            "8",
            "--limit",
            "8",
            "--service-origin",
        ])
        .arg(&svc_origin)
        .arg("--service-token-file")
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", &gh_origin)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| unreachable!());

    let poll_cmd = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["poll-runbooks", "--database"])
        .arg(&db_path)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .args(["--max-pages", "1", "--service-origin"])
        .arg(&svc_origin)
        .arg("--service-token-file")
        .arg(&svc_token_file)
        .arg("--github-token-file")
        .arg(&gh_token_file)
        .env("KILN_GITHUB_FIXTURE_API_ENDPOINT", &gh_origin)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| unreachable!());

    let drain_out = drain_cmd
        .wait_with_output()
        .unwrap_or_else(|_| unreachable!());
    let poll_out = poll_cmd
        .wait_with_output()
        .unwrap_or_else(|_| unreachable!());

    assert!(
        drain_out.status.success(),
        "drain failed: {}",
        String::from_utf8_lossy(&drain_out.stderr)
    );
    assert!(
        poll_out.status.success(),
        "poll failed: {}",
        String::from_utf8_lossy(&poll_out.stderr)
    );

    let drain_sum: serde_json::Value =
        serde_json::from_slice(&drain_out.stdout).unwrap_or_else(|_| unreachable!());
    let poll_sum: serde_json::Value =
        serde_json::from_slice(&poll_out.stdout).unwrap_or_else(|_| unreachable!());
    let count = |val: &serde_json::Value, key: &str| -> u64 {
        val.get(key)
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| unreachable!())
    };
    assert_eq!(
        count(&drain_sum, "dispatched") + count(&poll_sum, "dispatched"),
        1
    );
    assert_eq!(
        count(&drain_sum, "replayed") + count(&poll_sum, "replayed"),
        1
    );
    assert_eq!(
        count(&drain_sum, "comments_posted") + count(&poll_sum, "comments_posted"),
        1
    );
    assert_eq!(count(&drain_sum, "dispatch_errors"), 0);
    assert_eq!(count(&poll_sum, "dispatch_errors"), 0);
    assert_eq!(count(&drain_sum, "comment_errors"), 0);
    assert_eq!(count(&poll_sum, "comment_errors"), 0);

    let gh_reqs = gh_server.stop();
    let svc_reqs = svc_server.stop();
    assert_eq!(
        svc_reqs
            .iter()
            .filter(|req| req.starts_with("POST /api/runbooks/flake-update/dispatch "))
            .count(),
        1
    );
    assert_eq!(
        gh_reqs
            .iter()
            .filter(|req| req.starts_with("POST /repos/octocat/hello-world/issues/9/comments "))
            .count(),
        1
    );

    let store = RunbookRoutingStore::open(&db_path, 16).unwrap_or_else(|_| unreachable!());
    let audit = store.list_audit(20).unwrap_or_else(|_| unreachable!());
    assert_eq!(audit.len(), 1);
    assert_eq!(
        audit.first().map(|e| e.decision_code.as_str()),
        Some("dispatched")
    );
}
