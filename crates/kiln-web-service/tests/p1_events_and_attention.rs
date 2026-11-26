//! End-to-end served HTTPS tests for P1 multiplexed `/api/events` SSE fan-out,
//! prompt disconnect cleanup, `/api/implementation/attention`, campaign wake,
//! termination reconciliation, and inline plan review.
// r[verify kiln.web.multiplexed_sse_events]
// r[verify kiln.web.multiplexed_sse_events.scenario.fanout]
// r[verify kiln.implementation.campaign.unified_attention_and_recovery]
// r[verify kiln.implementation.campaign.unified_attention_and_recovery.scenario.resolve]

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, MemberBinding, MemberSelector,
    WorkflowProfile,
};
use kiln_core::implementation::{
    AcceptanceObligation, Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA, ImplementationPlan,
    ImplementationWork, PLAN_REVIEW_POLICY_SCHEMA, PLAN_SCHEMA, PlanReviewCommand,
    PlanReviewPolicy, PlanStep, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use kiln_implementer::campaign::CampaignStore;
use serde_json::{Value, json};

const CA: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/cert.pem");
const KEY: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/key.pem");
const TOKEN_LENGTH: usize = 64;
const READY_DEADLINE: Duration = Duration::from_secs(10);

struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct MockFleet {
    addr: SocketAddr,
    snapshot_calls: Arc<AtomicUsize>,
    version: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MockFleet {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl MockFleet {
    fn start(decision_attempt: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let snapshot_calls = Arc::new(AtomicUsize::new(0));
        let version = Arc::new(Mutex::new("a".repeat(64)));
        let stop = Arc::new(AtomicBool::new(false));
        let sc = Arc::clone(&snapshot_calls);
        let fv = Arc::clone(&version);
        let fs = Arc::clone(&stop);
        let attempt_id = decision_attempt.to_owned();
        let decision_id = "e".repeat(64);
        let session_id = "f".repeat(64);

        let handle = std::thread::spawn(move || {
            while !fs.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        let _ = sock.set_nonblocking(false);
                        let mut buf = [0u8; 2048];
                        let n = sock.read(&mut buf).unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let path = req
                            .lines()
                            .next()
                            .and_then(|l| l.split_whitespace().nth(1))
                            .unwrap_or("/");
                        let body = match path {
                            "/v1/version" => {
                                let v = fv.lock().unwrap().clone();
                                format!("{{\"version\":\"{v}\"}}")
                            }
                            "/v1/health" => {
                                sc.fetch_add(1, Ordering::Relaxed);
                                "{\"api\":\"v1\",\"machine\":\"local\",\"version\":\"0.1.0\"}"
                                    .to_owned()
                            }
                            "/v1/claims?include=foreign,expired" | "/v1/claims" => {
                                "{\"claims\":[],\"foreign_claims\":[]}".to_owned()
                            }
                            "/v1/work" => {
                                "{\"ready_unclaimed\":0,\"ranked\":[],\"blocked\":[]}".to_owned()
                            }
                            "/v1/attempts" => "{\"attempts\":[]}".to_owned(),
                            "/v1/decisions" => json!({
                                "decisions": [{
                                    "identity": decision_id,
                                    "session": session_id,
                                    "attempt": attempt_id,
                                    "question": "Approve workspace edit?",
                                    "raised_ms": 1_000,
                                    "answer": null,
                                    "closed_ms": null
                                }]
                            })
                            .to_string(),
                            "/v1/machines" => "{\"machines\":[]}".to_owned(),
                            _ => "{\"error\":\"not_found\"}".to_owned(),
                        };
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = sock.write_all(resp.as_bytes());
                        let _ = sock.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            addr,
            snapshot_calls,
            version,
            stop,
            handle: Some(handle),
        })
    }
}

fn tls_connect(
    addr: SocketAddr,
    timeout: Duration,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, Box<dyn std::error::Error>> {
    let mut roots = rustls::RootCertStore::empty();
    let mut pem = std::io::Cursor::new(CA);
    let certs = rustls_pemfile::certs(&mut pem).collect::<Result<Vec<_>, _>>()?;
    roots.add_parsable_certificates(certs);
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from("localhost")?;
    let conn = rustls::ClientConnection::new(Arc::new(tls), name)?;
    let sock = TcpStream::connect_timeout(&addr, timeout)?;
    sock.set_read_timeout(Some(timeout))?;
    sock.set_write_timeout(Some(timeout))?;
    let mut stream = rustls::StreamOwned::new(conn, sock);
    stream.conn.complete_io(&mut stream.sock)?;
    Ok(stream)
}

fn http_json(
    addr: SocketAddr,
    method: &str,
    target: &str,
    token: &str,
    payload: Option<&Value>,
) -> Result<(u16, Value), Box<dyn std::error::Error>> {
    let mut stream = tls_connect(addr, Duration::from_secs(4))?;
    let body = payload.map(serde_json::to_vec).transpose()?.unwrap_or_default();
    let req = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {token}\r\nOrigin: https://localhost:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr.port(),
        addr.port(),
        body.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
    }
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("missing header terminator")?;
    let head = std::str::from_utf8(&raw[..split])?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .ok_or("missing status")?
        .parse()?;
    let json: Value = serde_json::from_slice(&raw[split + 4..])?;
    Ok((status, json))
}

fn read_sse_until(
    reader: &mut BufReader<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>,
    wanted_events: &[&str],
    deadline: Instant,
) -> Result<Vec<(String, Value)>, Box<dyn std::error::Error>> {
    let mut seen = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();
    while Instant::now() < deadline {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            if !current_event.is_empty() && !current_data.is_empty() {
                let parsed: Value = serde_json::from_str(&current_data)?;
                seen.push((current_event.clone(), parsed));
                if wanted_events
                    .iter()
                    .all(|wanted| seen.iter().any(|(ev, _)| ev == wanted))
                {
                    return Ok(seen);
                }
            }
            current_event.clear();
            current_data.clear();
        } else if let Some(ev) = trimmed.strip_prefix("event:") {
            current_event = ev.trim().to_owned();
        } else if let Some(data) = trimmed.strip_prefix("data:") {
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(data.trim_start());
        }
    }
    Err(format!("timed out waiting for SSE events {wanted_events:?}; got {seen:?}").into())
}

#[test]
fn served_multiplexed_sse_fanout_disconnect_cleanup_and_attention_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo_dir = root.path().join("repository");
    let repo = git2::Repository::init(&repo_dir)?;
    std::fs::write(repo_dir.join("lib.rs"), b"pub fn ok() {}\n")?;
    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("lib.rs"))?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = git2::Signature::now("test", "test@example.invalid")?;
    repo.commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])?;

    let db_path = root.path().join("impl.sqlite");
    let hash = |ch: &str| ContentId::from_blake3_hex(ch.repeat(64)).unwrap();
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_owned(), "onix/other".to_owned()]),
        actors: BTreeSet::from(["local:operator".to_owned()]),
    };
    let mut implementer =
        Implementer::open_with_local_operator(&db_path, enrollment, "local:operator")?;
    let work_plan_item = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: "local:operator".to_owned(),
        repository: "onix/kiln".to_owned(),
        project: "kiln".to_owned(),
        base: None,
        title: "Plan blocked unit".to_owned(),
        requested_outcome: "deliver".to_owned(),
        acceptance: vec![AcceptanceObligation {
            id: "a1".into(),
            statement: "verified".into(),
        }],
        dependencies: Vec::new(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-plan/t1").unwrap(),
            native_id: "chg-plan/t1".into(),
            version: hash("1"),
        }],
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::new(),
        verification_profile: hash("9"),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
    };
    let work_plan_id = work_plan_item
        .identity()
        .map_err(|e| std::io::Error::other(e.message))?;
    implementer.admit(&work_plan_item, None)?;

    let work_unknown = ImplementationWork {
        repository: "onix/other".to_owned(),
        title: "Unknown termination unit".to_owned(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-unk/t2").unwrap(),
            native_id: "chg-unk/t2".into(),
            version: hash("2"),
        }],
        ..work_plan_item
    };
    let work_unknown_id = work_unknown
        .identity()
        .map_err(|e| std::io::Error::other(e.message))?;
    implementer.admit(&work_unknown, None)?;
    let claim = implementer.claim_next(1_000, 60_000)?.expect("claim");
    let attempt_id = claim.identity.clone();
    let fence = claim.fence;
    implementer.fail(&attempt_id, fence, 2_000, FailureClass::Unknown, "ambiguous")?;

    implementer.plan_review(&PlanReviewCommand::Configure {
        policy: Some(PlanReviewPolicy {
            schema: PLAN_REVIEW_POLICY_SCHEMA.to_owned(),
            repositories: BTreeSet::from(["onix/kiln".to_owned()]),
            reviewers: BTreeSet::from(["local:operator".to_owned()]),
        }),
    })?;
    let plan = ImplementationPlan {
        schema: PLAN_SCHEMA.to_owned(),
        work_identity: work_plan_id.clone(),
        revision: 1,
        previous: None,
        author: "agent:coder".into(),
        steps: vec![PlanStep {
            id: "s1".into(),
            text: "Build core".into(),
        }],
    };
    let plan_id = plan
        .identity()
        .map_err(|e| std::io::Error::other(e.message))?;
    implementer.plan_review(&PlanReviewCommand::Propose { plan })?;

    let mut store = CampaignStore::open(&db_path)?;
    store.start(CampaignContract {
        schema: CAMPAIGN_SCHEMA.to_owned(),
        campaign_id: "camp-p1".to_owned(),
        revision: 1,
        repository: "onix/other".to_owned(),
        members: vec![MemberBinding {
            selector: MemberSelector::WholeChange {
                change: "chg-unk".to_owned(),
            },
            prerequisite_inclusion: false,
            profile: WorkflowProfile::SpecDriven,
            profile_version: hash("a"),
            policy: hash("a"),
            repository_instructions: hash("a"),
            integration_policy: hash("a"),
            goal: CompletionGoal::VerifiedCandidate,
            required_checks: Vec::new(),
            required_reviews: Vec::new(),
            context: Vec::new(),
        }],
        max_concurrency: 1,
        max_outstanding_decisions: 4,
        parent_budget_id: hash("a"),
        agent_definition: hash("a"),
        require_one_writer_per_repository: true,
    })?;

    let fleet = MockFleet::start(attempt_id.as_str())?;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);

    let cert_path = root.path().join("cert.pem");
    let key_path = root.path().join("key.pem");
    std::fs::write(&cert_path, CERT)?;
    std::fs::write(&key_path, KEY)?;
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o400))?;

    let op_tok_path = root.path().join("op.token");
    let wk_tok_path = root.path().join("wk.token");
    let vw_tok_path = root.path().join("vw.token");
    for (p, ch) in [
        (&op_tok_path, b'a'),
        (&wk_tok_path, b'b'),
        (&vw_tok_path, b'c'),
    ] {
        std::fs::write(p, vec![ch; TOKEN_LENGTH])?;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o400))?;
    }
    let op_tok = "a".repeat(TOKEN_LENGTH);
    let wk_tok = "b".repeat(TOKEN_LENGTH);
    let vw_tok = "c".repeat(TOKEN_LENGTH);

    let profile_path = root.path().join("service.json");
    // Bound max_connections = 6 so 5 concurrent SSE clients leave 1 spare slot;
    // releasing 4 clients MUST free their threads so subsequent requests succeed!
    std::fs::write(
        &profile_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-service.v1",
            "service_id": "p1-served-test",
            "listen_address": addr.to_string(),
            "origin": format!("https://localhost:{}", addr.port()),
            "tls": {"certificate_path": cert_path, "private_key_path": key_path, "min_version": "TLS1.3"},
            "operator_token_file": op_tok_path,
            "worker_token_file": wk_tok_path,
            "state_directory": root.path().join("state"),
            "projects": [{
                "id": "fixture",
                "label": "Fixture",
                "repository_path": repo_dir,
                "reference": "refs/heads/main",
                "source_limits": {"max_entries": 32, "max_path_bytes": 1024, "max_file_bytes": 4096, "max_total_bytes": 16384},
                "workflow": {
                    "socket_path": root.path().join("lattice.sock"),
                    "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                    "transport_limits": {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 65536, "maximum_connections": 4, "read_deadline_milliseconds": 1000, "write_deadline_milliseconds": 1000},
                    "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576},
                    "policy_id": "reviewed-policy",
                    "provider_profile_id": "reviewed-lattice",
                    "platform": "linux-x86_64",
                    "nodes": [{"id": "job", "kind": "test", "effect_class": "test", "adapter_kind": "local", "operation_identity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "edges": [],
                    "controls": {"cancel": true, "pause": false, "resume": false},
                    "bounds": {"max_nodes": 8, "max_edges": 16, "max_log_references": 32, "max_log_read_bytes": 4096, "max_total_log_bytes": 65536}
                }
            }],
            "bounds": {
                "max_request_bytes": 16384, "max_response_bytes": 1048576, "max_header_bytes": 4096,
                "max_connections": 6, "timeout_seconds": 3, "poll_interval_ms": 50, "max_projects": 2,
                "max_runs": 4, "max_artifact_bytes": 1024, "max_artifacts": 2, "max_run_artifact_bytes": 2048,
                "max_total_artifact_bytes": 4096, "max_log_read_bytes": 4096, "max_requests": 32,
                "max_events_per_run": 8, "max_state_bytes": 1048576
            }
        }))?,
    )?;
    let access_path = root.path().join("access.json");
    std::fs::write(
        &access_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{"id": "reader", "label": "Reader", "token_file": vw_tok_path}]
        }))?,
    )?;
    let fleet_path = root.path().join("fleet.json");
    std::fs::write(
        &fleet_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-fleet.v1",
            "address": fleet.addr.to_string(),
            "timeout_ms": 2000,
            "max_response_bytes": 1048576
        }))?,
    )?;

    let notify_path = root.path().join("notify.sock");
    let receiver = UnixDatagram::bind(&notify_path)?;
    receiver.set_read_timeout(Some(READY_DEADLINE))?;
    let _service = Service(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(&profile_path)
            .arg("--access")
            .arg(&access_path)
            .arg("--fleet")
            .arg(&fleet_path)
            .arg("--implementation-work")
            .arg(&db_path)
            .env("NOTIFY_SOCKET", &notify_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let mut ready_buf = [0u8; 32];
    let n = receiver.recv(&mut ready_buf)?;
    assert_eq!(&ready_buf[..n], b"READY=1");

    // 1. Connect 5 concurrent SSE clients to /api/events.
    let mut readers = Vec::new();
    for _ in 0..5 {
        let mut s = tls_connect(addr, Duration::from_secs(4))?;
        let req = format!(
            "GET /api/events HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {op_tok}\r\nConnection: close\r\n\r\n",
            addr.port()
        );
        s.write_all(req.as_bytes())?;
        s.flush()?;
        let mut r = BufReader::new(s);
        let mut header_line = String::new();
        r.read_line(&mut header_line)?;
        assert!(header_line.starts_with("HTTP/1.1 200 OK"), "{header_line}");
        loop {
            header_line.clear();
            r.read_line(&mut header_line)?;
            if header_line == "\r\n" || header_line.is_empty() {
                break;
            }
        }
        let events = read_sse_until(
            &mut r,
            &["ready", "ci", "fleet", "cairn", "attention"],
            Instant::now() + Duration::from_secs(4),
        )?;
        assert!(events.iter().any(|(ev, v)| ev == "ready" && v["schema"] == "kiln.events.v1"));
        assert!(events.iter().any(|(ev, v)| ev == "attention" && v["total"].as_u64() == Some(3)));
        readers.push(r);
    }

    // Only 1 upstream snapshot fetch occurred despite 5 concurrent SSE clients!
    assert_eq!(fleet.snapshot_calls.load(Ordering::Relaxed), 1);

    // 2. Advance fleet version once -> all 5 tabs receive the new fleet frame from 1 upstream fetch!
    *fleet.version.lock().unwrap() = "b".repeat(64);
    for r in &mut readers {
        let events = read_sse_until(r, &["fleet"], Instant::now() + Duration::from_secs(4))?;
        assert!(events.iter().any(|(ev, v)| ev == "fleet" && v["version"] == "b".repeat(64)));
    }
    assert_eq!(fleet.snapshot_calls.load(Ordering::Relaxed), 2);

    // 3. Close 4 SSE tabs -> server must promptly release their threads (within max_connections=6).
    let keeper = readers.pop().unwrap();
    drop(readers);
    std::thread::sleep(Duration::from_millis(500));

    // Verify 5 sequential/concurrent requests succeed cleanly (would starve if those 4 SSE threads leaked).
    let (wk_status, _) = http_json(addr, "GET", "/api/implementation/attention", &wk_tok, None)?;
    assert_eq!(wk_status, 403);
    let (vw_status, att) = http_json(addr, "GET", "/api/implementation/attention", &vw_tok, None)?;
    assert_eq!(vw_status, 200);
    assert_eq!(att["schema"], "kiln.implementation-attention.v1");
    assert_eq!(att["counts"]["PlanReviewGate"], 1);
    assert_eq!(att["counts"]["UnknownTerminationOrDispatch"], 1);
    assert_eq!(att["counts"]["FleetSessionDecision"], 1);

    // 4. Resolve PlanReviewGate + UnknownTerminationOrDispatch + Wake campaign via HTTP.
    let (pr_status, pr_json) = http_json(
        addr,
        "POST",
        "/api/implementation/plan-review",
        &op_tok,
        Some(&json!({
            "work_id": work_plan_id,
            "plan_id": plan_id,
            "verdict": "accepted",
            "request_id": "req-e2e-plan-1"
        })),
    )?;
    assert_eq!(pr_status, 200);
    assert_eq!(pr_json["gate"]["gate"], "accepted");

    let (rt_status, rt_json) = http_json(
        addr,
        "POST",
        "/api/implementation/reconcile-termination",
        &op_tok,
        Some(&json!({
            "campaign_id": "camp-p1",
            "work_id": work_unknown_id,
            "attempt_id": attempt_id,
            "fence": fence,
            "request_id": "req-e2e-rt-1"
        })),
    )?;
    assert_eq!(rt_status, 200);
    assert_eq!(rt_json["reconciled"], true);
    assert_eq!(rt_json["campaign_woken"], true);

    let (wk_op_status, wk_op_json) = http_json(
        addr,
        "POST",
        "/api/implementation/campaigns/camp-p1/wake",
        &op_tok,
        Some(&json!({
            "kind": "operator",
            "reason": "resume after termination",
            "request_id": "req-e2e-wake-1"
        })),
    )?;
    assert_eq!(wk_op_status, 200);
    assert_eq!(wk_op_json["campaign_id"], "camp-p1");

    // 5. Verify attention total dropped from 3 to 1 (only the open FleetSessionDecision remains)
    // and the surviving SSE tab received the updated attention frame!
    let (att2_status, att2) = http_json(addr, "GET", "/api/implementation/attention", &op_tok, None)?;
    assert_eq!(att2_status, 200);
    assert_eq!(att2["total"], 1);
    assert_eq!(att2["counts"]["PlanReviewGate"], 0);
    assert_eq!(att2["counts"]["UnknownTerminationOrDispatch"], 0);

    let mut keeper = keeper;
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut saw_total_one = false;
    while Instant::now() < deadline && !saw_total_one {
        let updated = read_sse_until(&mut keeper, &["attention"], deadline)?;
        saw_total_one = updated
            .iter()
            .any(|(ev, v)| ev == "attention" && v["total"] == 1);
    }
    assert!(saw_total_one);

    // Negative /api/events checks
    let (bad_ev, _) = http_json(addr, "GET", "/api/events?channels=bogus", &op_tok, None)?;
    assert_eq!(bad_ev, 400);
    let (wk_ev, _) = http_json(addr, "GET", "/api/events", &wk_tok, None)?;
    assert_eq!(wk_ev, 403);
    let (post_ev, _) = http_json(addr, "POST", "/api/events", &op_tok, Some(&json!({})))?;
    assert_eq!(post_ev, 405);

    Ok(())
}
