//! End-to-end TLS HTTPS integration tests for repository import, mirror
//! attachment, read-only local/Fleet census reconciliation, `request_id`
//! idempotency, offline Fleet graceful degradation, and non-destructive removal.
// r[verify kiln.web.repository_import_api]
// r[verify kiln.web.repository_import_api.scenario.import_and_mirror]

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

struct HttpReply {
    status: u16,
    body: Vec<u8>,
}

fn free_addr() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?)
}

fn wait_ready(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    while start.elapsed() < READY_DEADLINE {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!("service at {addr} did not become ready").into())
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

fn http_raw(
    addr: SocketAddr,
    method: &str,
    target: &str,
    token: Option<&str>,
    body: &[u8],
) -> Result<HttpReply, Box<dyn std::error::Error>> {
    let mut stream = tls_connect(addr, Duration::from_secs(4))?;
    let mut req = format!("{method} {target} HTTP/1.1\r\nHost: localhost:{}\r\n", addr.port());
    if let Some(tok) = token {
        req.push_str(&format!("Authorization: Bearer {tok}\r\n"));
    }
    if method == "POST" {
        req.push_str(&format!(
            "Origin: https://localhost:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            addr.port(),
            body.len()
        ));
    }
    req.push_str("Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes())?;
    stream.write_all(body)?;
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
    let head = std::str::from_utf8(raw.get(..split).ok_or("header slice invalid")?)?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .ok_or("missing status")?
        .parse()?;
    Ok(HttpReply {
        status,
        body: raw.get(split + 4..).ok_or("body slice invalid")?.to_vec(),
    })
}

fn http_json(
    addr: SocketAddr,
    method: &str,
    target: &str,
    token: &str,
    payload: Option<&Value>,
) -> Result<(u16, Value), Box<dyn std::error::Error>> {
    let body = payload.map(serde_json::to_vec).transpose()?.unwrap_or_default();
    let reply = http_raw(addr, method, target, Some(token), &body)?;
    let parsed: Value = serde_json::from_slice(&reply.body)?;
    Ok((reply.status, parsed))
}

fn init_git_repo(
    path: &Path,
    remote_url: Option<&str>,
    dirty: bool,
    cairn_changes: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let repo = git2::Repository::init(path)?;
    fs::write(path.join("README.md"), b"kiln repo fixture\n")?;
    let mut index = repo.index()?;
    index.add_path(Path::new("README.md"))?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = git2::Signature::now("test", "test@example.invalid")?;
    let commit_id = repo.commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])?;
    repo.set_head("refs/heads/main")?;
    if let Some(url) = remote_url {
        repo.remote("origin", url)?;
    }
    if dirty {
        fs::write(path.join("untracked.txt"), b"dirty\n")?;
    }
    for change in cairn_changes {
        fs::create_dir_all(path.join(".cairn/changes").join(change))?;
    }
    Ok(commit_id.to_string())
}

fn start_mock_fleet(
    commit_a: String,
    commit_b: String,
) -> Result<(SocketAddr, Arc<AtomicBool>, std::thread::JoinHandle<()>), Box<dyn std::error::Error>>
{
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);

    let handle = std::thread::spawn(move || {
        let version_hex = "a".repeat(64);
        while !stop_flag.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    let _ = sock.set_nonblocking(false);
                    let mut buf = [0u8; 2048];
                    let n = sock.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(buf.get(..n).unwrap_or(&[]));
                    let path = req
                        .lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let body = match path {
                        "/v1/version" => json!({"version": version_hex}).to_string(),
                        "/v1/health" => {
                            json!({"api":"v1","machine":"britton-desktop","version":"0.1.0"})
                                .to_string()
                        }
                        "/v1/claims?include=foreign,expired" | "/v1/claims" => {
                            json!({"claims":[],"foreign_claims":[]}).to_string()
                        }
                        "/v1/work" => {
                            json!({"ready_unclaimed":0,"ranked":[],"blocked":[]}).to_string()
                        }
                        "/v1/attempts" => json!({"attempts":[]}).to_string(),
                        "/v1/decisions" => json!({"decisions":[]}).to_string(),
                        "/v1/machines" => json!({"machines":[]}).to_string(),
                        "/v1/repositories" => json!({
                            "schema": "kiln.fleet-repositories.v1",
                            "observed_at_ms": 1_700_000_000_000_u64,
                            "hosts": [
                                {"machine": "britton-desktop", "health": "available", "error": null, "copy_count": 1},
                                {"machine": "aspen3", "health": "available", "error": null, "copy_count": 1},
                                {"machine": "aspen1", "health": "unavailable", "error": "host unreachable", "copy_count": 0}
                            ],
                            "copies": [
                                {
                                    "machine": "britton-desktop",
                                    "path": "/home/brittonr/git/OnixResearch/kiln",
                                    "vcs": "jj",
                                    "head_commit": commit_a,
                                    "branch_or_bookmark": "main",
                                    "dirty": false,
                                    "ahead": 0,
                                    "behind": 0,
                                    "remotes": ["https://github.com/OnixResearch/kiln.git"],
                                    "cairn_count": 2,
                                    "observed_at_ms": 1_700_000_000_000_u64,
                                    "health": "available",
                                    "error": null
                                },
                                {
                                    "machine": "aspen3",
                                    "path": "/srv/git/kiln",
                                    "vcs": "git",
                                    "head_commit": commit_b,
                                    "branch_or_bookmark": "main",
                                    "dirty": true,
                                    "ahead": 1,
                                    "behind": 0,
                                    "remotes": ["rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5"],
                                    "cairn_count": 1,
                                    "observed_at_ms": 1_700_000_000_000_u64,
                                    "health": "available",
                                    "error": null
                                }
                            ],
                            "bounded": true,
                            "truncated": false
                        })
                        .to_string(),
                        _ => json!({"error":"not_found"}).to_string(),
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

    Ok((addr, stop, handle))
}

fn read_sse_event(
    stream: &mut rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
    wanted_event: &str,
    deadline: Instant,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(stream);
    let mut current_event = String::new();
    let mut data_buf = String::new();
    while Instant::now() < deadline {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
                if trimmed.is_empty() {
                    if current_event == wanted_event && !data_buf.is_empty() {
                        return Ok(serde_json::from_str(&data_buf)?);
                    }
                    current_event.clear();
                    data_buf.clear();
                } else if let Some(ev) = trimmed.strip_prefix("event:") {
                    current_event = ev.trim().to_owned();
                } else if let Some(data) = trimmed.strip_prefix("data:") {
                    if !data_buf.is_empty() {
                        data_buf.push('\n');
                    }
                    data_buf.push_str(data.trim());
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(format!("timed out waiting for SSE event `{wanted_event}`").into())
}

#[test]
fn repository_import_mirrors_fleet_census_and_idempotent_lifecycle()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let dir = root.path().join("svc");
    fs::create_dir_all(&dir)?;

    let ci_repo = dir.join("ci-kiln");
    let commit_a = init_git_repo(
        &ci_repo,
        Some("https://github.com/OnixResearch/kiln.git"),
        false,
        &["c1", "c2"],
    )?;

    let source_repo = dir.join("docs-source");
    let _commit_docs = init_git_repo(
        &source_repo,
        Some("https://gitea.example.org/onix/docs.git"),
        false,
        &[],
    )?;

    let scan_root = dir.join("scan-root");
    let scanned_repo = scan_root.join("org").join("extra-local");
    fs::create_dir_all(&scanned_repo)?;
    let _commit_scan = init_git_repo(&scanned_repo, None, true, &["sc1"])?;
    // Simulate colocated jj directory alongside .git
    fs::create_dir_all(scanned_repo.join(".jj"))?;

    let clean_local_repo = scan_root.join("clean-local");
    fs::create_dir_all(&clean_local_repo)?;
    let _commit_clean = init_git_repo(&clean_local_repo, None, false, &[])?;

    let commit_b = "b".repeat(40);
    let (fleet_addr, fleet_stop, fleet_handle) =
        start_mock_fleet(commit_a.clone(), commit_b)?;

    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    fs::write(&cert_path, CERT)?;
    fs::write(&key_path, KEY)?;
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o400))?;

    let op_tok_path = dir.join("operator.token");
    let vw_tok_path = dir.join("viewer.token");
    let wk_tok_path = dir.join("worker.token");
    let op_tok = "a".repeat(TOKEN_LENGTH);
    let vw_tok = "c".repeat(TOKEN_LENGTH);
    let wk_tok = "b".repeat(TOKEN_LENGTH);
    fs::write(&op_tok_path, &op_tok)?;
    fs::write(&vw_tok_path, &vw_tok)?;
    fs::write(&wk_tok_path, &wk_tok)?;
    for p in [&op_tok_path, &vw_tok_path, &wk_tok_path] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o400))?;
    }

    let addr = free_addr()?;
    let state_dir = dir.join("state");
    let profile_path = dir.join("profile.json");
    let profile = json!({
        "schema": "kiln.web-service.v1",
        "service_id": "repo-import-test",
        "listen_address": addr.to_string(),
        "origin": format!("https://localhost:{}", addr.port()),
        "tls": {
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "min_version": "TLS1.3"
        },
        "operator_token_file": op_tok_path,
        "worker_token_file": wk_tok_path,
        "state_directory": state_dir,
        "projects": [{
            "id": "kiln",
            "label": "Kiln",
            "repository_path": ci_repo,
            "reference": "refs/heads/main",
            "source_limits": {
                "max_entries": 32,
                "max_path_bytes": 1024,
                "max_file_bytes": 4096,
                "max_total_bytes": 16384
            },
            "workflow": {
                "socket_path": dir.join("lattice.sock"),
                "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                "transport_limits": {
                    "maximum_frame_bytes": 1048576,
                    "maximum_log_bytes": 65536,
                    "maximum_connections": 4,
                    "read_deadline_milliseconds": 1000,
                    "write_deadline_milliseconds": 1000
                },
                "adapter_limits": {
                    "maximum_request_bytes": 1048576,
                    "maximum_response_bytes": 1048576
                },
                "policy_id": "reviewed-policy",
                "provider_profile_id": "reviewed-lattice",
                "platform": "linux-x86_64",
                "nodes": [{
                    "id": "job",
                    "kind": "test",
                    "effect_class": "test",
                    "adapter_kind": "local",
                    "operation_identity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }],
                "edges": [],
                "controls": {"cancel": true, "pause": false, "resume": false},
                "bounds": {
                    "max_nodes": 8,
                    "max_edges": 16,
                    "max_log_references": 32,
                    "max_log_read_bytes": 4096,
                    "max_total_log_bytes": 65536
                }
            }
        }],
        "bounds": {
            "max_request_bytes": 65536,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 16,
            "timeout_seconds": 5,
            "poll_interval_ms": 50,
            "max_projects": 2,
            "max_runs": 8,
            "max_artifact_bytes": 1024,
            "max_artifacts": 2,
            "max_run_artifact_bytes": 2048,
            "max_total_artifact_bytes": 4096,
            "max_log_read_bytes": 4096,
            "max_requests": 32,
            "max_events_per_run": 8,
            "max_state_bytes": 1048576
        }
    });
    fs::write(&profile_path, serde_json::to_vec(&profile)?)?;

    let access_path = dir.join("access.json");
    fs::write(
        &access_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{"id": "reader", "label": "Reader", "token_file": vw_tok_path}]
        }))?,
    )?;

    let sources_path = dir.join("sources.json");
    fs::write(
        &sources_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-sources.v1",
            "sources": [{
                "id": "docs",
                "label": "Docs",
                "kind": "git",
                "path": source_repo
            }]
        }))?,
    )?;

    let fleet_path = dir.join("fleet.json");
    fs::write(
        &fleet_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-fleet.v1",
            "address": fleet_addr.to_string(),
            "timeout_ms": 2000,
            "max_response_bytes": 1048576
        }))?,
    )?;

    let spawn = || -> Result<Service, Box<dyn std::error::Error>> {
        let child = Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(&profile_path)
            .arg("--access")
            .arg(&access_path)
            .arg("--sources")
            .arg(&sources_path)
            .arg("--fleet")
            .arg(&fleet_path)
            .arg("--repository-root")
            .arg(&scan_root)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        wait_ready(addr)?;
        Ok(Service(child))
    };

    let svc = spawn()?;

    // 1. Role checks: Worker forbidden on GET & POST; Viewer allowed on GET, forbidden on POST.
    let (wk_get, _) = http_json(addr, "GET", "/api/repositories", &wk_tok, None)?;
    assert_eq!(wk_get, 403);
    let (wk_post, _) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &wk_tok,
        Some(&json!({"provider":"github","role":"mirror","url":"OnixResearch/kiln","request_id":"wk-1"})),
    )?;
    assert_eq!(wk_post, 403);
    let (vw_get, _) = http_json(addr, "GET", "/api/repositories", &vw_tok, None)?;
    assert_eq!(vw_get, 200);
    let (vw_post, _) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &vw_tok,
        Some(&json!({"provider":"github","role":"mirror","url":"OnixResearch/kiln","request_id":"vw-1"})),
    )?;
    assert_eq!(vw_post, 403);
    let (vw_refresh, _) = http_json(addr, "POST", "/api/repositories/refresh", &vw_tok, None)?;
    assert_eq!(vw_refresh, 403);

    // 2. Negative input validation: embedded credentials, malformed RID, SSRF IP, unknown field.
    let (cred_status, cred_body) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "gitea",
            "role": "mirror",
            "url": "https://user:secret@gitea.example.org/org/repo",
            "request_id": "bad-cred"
        })),
    )?;
    assert_eq!(cred_status, 400);
    assert_eq!(
        cred_body.get("code").and_then(Value::as_str),
        Some("embedded_credentials")
    );

    let (rid_status, _) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "radicle",
            "role": "canonical",
            "url": "rad:not-valid",
            "request_id": "bad-rid"
        })),
    )?;
    assert_eq!(rid_status, 400);

    let (ssrf_status, ssrf_body) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "gitea",
            "role": "mirror",
            "url": "https://127.0.0.1/org/repo",
            "request_id": "bad-ssrf"
        })),
    )?;
    assert_eq!(ssrf_status, 400);
    assert_eq!(
        ssrf_body.get("code").and_then(Value::as_str),
        Some("ssrf_ip_literal_rejected")
    );

    let (unknown_status, _) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "github",
            "role": "mirror",
            "url": "OnixResearch/kiln",
            "unexpected": true,
            "request_id": "bad-field"
        })),
    )?;
    assert_eq!(unknown_status, 400);

    // 3. Import canonical Radicle RID + attach GitHub & Gitea mirrors + request_id idempotency.
    let rad_import = json!({
        "provider": "radicle",
        "role": "canonical",
        "url": "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5",
        "label": "Kiln Core",
        "request_id": "req-import-rad-1"
    });
    let (imp_status, imp_body) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&rad_import),
    )?;
    assert_eq!(imp_status, 201);
    assert_eq!(
        imp_body.get("action").and_then(Value::as_str),
        Some("imported")
    );
    assert_eq!(
        imp_body.get("already_recorded").and_then(Value::as_bool),
        Some(false)
    );
    let repo_id = imp_body
        .get("repository")
        .and_then(|r| r.get("identity"))
        .and_then(Value::as_str)
        .ok_or("missing repository identity")?
        .to_owned();
    assert_eq!(repo_id.len(), 64);

    // Verify private 0600 permissions on state_directory/repositories.json
    let store_meta = fs::symlink_metadata(state_dir.join("repositories.json"))?;
    assert_eq!(store_meta.permissions().mode() & 0o7777, 0o600);

    // Radicle non-mutation guarantee: no rad remote created on local checkout
    assert!(!ci_repo.join(".git/refs/remotes/rad").exists());

    // Replay identical request_id -> 200 already_recorded: true
    let (rep_status, rep_body) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&rad_import),
    )?;
    assert_eq!(rep_status, 200);
    assert_eq!(
        rep_body.get("already_recorded").and_then(Value::as_bool),
        Some(true)
    );

    // Conflicting request_id reuse -> 409
    let (conf_status, conf_body) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "github",
            "role": "mirror",
            "url": "OnixResearch/kiln",
            "request_id": "req-import-rad-1"
        })),
    )?;
    assert_eq!(conf_status, 409);
    assert_eq!(
        conf_body.get("code").and_then(Value::as_str),
        Some("request_id_conflict")
    );

    // Attach GitHub mirror via POST /api/repositories/{id}/mirrors
    let (gh_status, gh_body) = http_json(
        addr,
        "POST",
        &format!("/api/repositories/{repo_id}/mirrors"),
        &op_tok,
        Some(&json!({
            "provider": "github",
            "role": "mirror",
            "url": "OnixResearch/kiln",
            "default_ref": "main",
            "request_id": "req-mirror-gh-1"
        })),
    )?;
    assert_eq!(gh_status, 200);
    assert_eq!(
        gh_body.get("action").and_then(Value::as_str),
        Some("mirror_attached")
    );

    // Attach Gitea mirror via POST /api/repositories/import with attach_to
    let (gt_status, _) = http_json(
        addr,
        "POST",
        "/api/repositories/import",
        &op_tok,
        Some(&json!({
            "provider": "gitea",
            "role": "mirror",
            "url": "https://gitea.example.org/onix/kiln",
            "attach_to": repo_id,
            "request_id": "req-mirror-gt-1"
        })),
    )?;
    assert_eq!(gt_status, 200);

    // Duplicate canonical on same repository -> 409
    let (dup_can_status, dup_can_body) = http_json(
        addr,
        "POST",
        &format!("/api/repositories/{repo_id}/mirrors"),
        &op_tok,
        Some(&json!({
            "provider": "github",
            "role": "canonical",
            "url": "OnixResearch/kiln-other",
            "request_id": "req-dup-canon"
        })),
    )?;
    assert_eq!(dup_can_status, 409);
    assert_eq!(
        dup_can_body.get("code").and_then(Value::as_str),
        Some("duplicate_canonical_remote")
    );

    // 4. Query GET /api/repositories and inspect reconciled catalog, Fleet copies, and offline host.
    let (list_status, list_body) = http_json(addr, "GET", "/api/repositories", &op_tok, None)?;
    assert_eq!(list_status, 200);
    assert_eq!(
        list_body.get("schema").and_then(Value::as_str),
        Some("kiln.repositories.v1")
    );
    assert_eq!(
        list_body
            .get("sources")
            .and_then(|s| s.get("fleet"))
            .and_then(|f| f.get("available"))
            .and_then(Value::as_bool),
        Some(true)
    );
    let repos = list_body
        .get("repositories")
        .and_then(Value::as_array)
        .ok_or("missing repositories array")?;

    let kiln_rec = repos
        .iter()
        .find(|r| r.get("identity").and_then(Value::as_str) == Some(repo_id.as_str()))
        .ok_or("imported kiln repo not found in GET /api/repositories")?;
    assert_eq!(
        kiln_rec.get("remotes").and_then(Value::as_array).map(Vec::len),
        Some(3)
    );
    assert_eq!(
        kiln_rec.get("sync_status").and_then(Value::as_str),
        Some("diverged")
    );
    assert_eq!(
        kiln_rec
            .get("admission")
            .and_then(|a| a.get("catalog_imported"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        kiln_rec
            .get("admission")
            .and_then(|a| a.get("ci_configured"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        kiln_rec
            .get("admission")
            .and_then(|a| a.get("fleet_enrolled"))
            .and_then(Value::as_bool),
        Some(true)
    );

    // Verify scanned dirty jj repo (no remotes, dirty=true, cairn_count=1)
    let dirty_local_rec = repos
        .iter()
        .find(|r| r.get("slug").and_then(Value::as_str) == Some("extra-local"))
        .ok_or("scanned extra-local repo missing")?;
    assert_eq!(
        dirty_local_rec.get("sync_status").and_then(Value::as_str),
        Some("dirty")
    );
    assert_eq!(
        dirty_local_rec
            .get("remotes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        dirty_local_rec.get("total_cairns").and_then(Value::as_u64),
        Some(1)
    );

    // Verify clean scanned repo with no remotes reports local_only
    let local_only_rec = repos
        .iter()
        .find(|r| r.get("slug").and_then(Value::as_str) == Some("clean-local"))
        .ok_or("scanned clean-local repo missing")?;
    assert_eq!(
        local_only_rec.get("sync_status").and_then(Value::as_str),
        Some("local_only")
    );

    // Verify unavailable host aspen1 is reported explicitly in hosts
    let hosts = list_body
        .get("hosts")
        .and_then(Value::as_array)
        .ok_or("missing hosts array")?;
    assert!(hosts.iter().any(|h| {
        h.get("machine").and_then(Value::as_str) == Some("aspen1")
            && h.get("health").and_then(Value::as_str) == Some("unavailable")
            && h.get("error").and_then(Value::as_str) == Some("host unreachable")
    }));

    // 5. SSE EventHub notification on POST /api/repositories/refresh
    let mut sse_stream = tls_connect(addr, Duration::from_secs(4))?;
    let sse_req = format!(
        "GET /api/events HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {op_tok}\r\nConnection: close\r\n\r\n",
        addr.port()
    );
    sse_stream.write_all(sse_req.as_bytes())?;
    sse_stream.flush()?;
    let (ref_status, ref_body) =
        http_json(addr, "POST", "/api/repositories/refresh", &op_tok, Some(&json!({})))?;
    assert_eq!(ref_status, 200);
    assert_eq!(
        ref_body.get("schema").and_then(Value::as_str),
        Some("kiln.repositories.v1")
    );
    let sse_ev = read_sse_event(
        &mut sse_stream,
        "repositories",
        Instant::now() + Duration::from_secs(4),
    )?;
    assert_eq!(sse_ev.get("invalidated").and_then(Value::as_bool), Some(true));
    drop(sse_stream);

    // 6. Offline Fleet graceful degradation: stop mock Fleet server and verify catalog stays up.
    fleet_stop.store(true, Ordering::Release);
    let _ = fleet_handle.join();
    let (deg_status, deg_body) = http_json(addr, "GET", "/api/repositories", &op_tok, None)?;
    assert_eq!(deg_status, 200);
    assert_eq!(
        deg_body
            .get("sources")
            .and_then(|s| s.get("fleet"))
            .and_then(|f| f.get("configured"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        deg_body
            .get("sources")
            .and_then(|s| s.get("fleet"))
            .and_then(|f| f.get("available"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let deg_repos = deg_body
        .get("repositories")
        .and_then(Value::as_array)
        .ok_or("missing degraded repositories array")?;
    assert!(deg_repos.iter().any(|r| {
        r.get("identity").and_then(Value::as_str) == Some(repo_id.as_str())
    }));

    // 7. Persistence across process restart + non-destructive removal.
    drop(svc);
    let _svc2 = spawn()?;
    let (after_restart_status, after_restart_body) =
        http_json(addr, "GET", "/api/repositories", &op_tok, None)?;
    assert_eq!(after_restart_status, 200);
    assert!(
        after_restart_body
            .get("repositories")
            .and_then(Value::as_array)
            .is_some_and(|arr| arr.iter().any(|r| {
                r.get("identity").and_then(Value::as_str) == Some(repo_id.as_str())
            }))
    );

    let (rem_status, rem_body) = http_json(
        addr,
        "POST",
        &format!("/api/repositories/{repo_id}/remove"),
        &op_tok,
        Some(&json!({
            "reason": "retire catalog entry",
            "request_id": "req-remove-1"
        })),
    )?;
    assert_eq!(rem_status, 200);
    assert_eq!(
        rem_body.get("action").and_then(Value::as_str),
        Some("removed")
    );
    // Disk checkout files MUST remain untouched!
    assert!(ci_repo.join("README.md").exists());
    assert!(source_repo.join("README.md").exists());
    assert!(scanned_repo.join("README.md").exists());

    // Confirm removed entry no longer appears as catalog_imported
    let (final_status, final_body) = http_json(addr, "GET", "/api/repositories", &op_tok, None)?;
    assert_eq!(final_status, 200);
    let final_repos = final_body
        .get("repositories")
        .and_then(Value::as_array)
        .ok_or("missing final repositories")?;
    assert!(!final_repos.iter().any(|r| {
        r.get("identity").and_then(Value::as_str) == Some(repo_id.as_str())
    }));

    // Re-check GET /api/fleet/repositories proxy route while fleet is down -> 503
    let (fleet_proxy_status, _) =
        http_json(addr, "GET", "/api/fleet/repositories", &op_tok, None)?;
    assert_eq!(fleet_proxy_status, 503);

    let _ = BTreeMap::<String, String>::new();
    Ok(())
}
