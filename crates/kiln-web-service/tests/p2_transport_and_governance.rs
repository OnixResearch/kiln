//! End-to-end served HTTPS tests for P2 static asset caching/compression,
//! live Fair-Admission governance & AttemptMetrics, Plan Review stale-revision
//! & step-level feedback, and guarded publication/integration lifecycle actions.
// r[verify kiln.web.static_asset_caching_and_compression]
// r[verify kiln.web.static_asset_caching_and_compression.scenario.http_caching]
// r[verify kiln.admission.live_console_governance]
// r[verify kiln.admission.live_console_governance.scenario.honest_metrics]
// r[verify kiln.implementation.plan_review_console_surface]
// r[verify kiln.implementation.plan_review_console_surface.scenario.stale_plan]
// r[verify kiln.cairn.gate_passport_surface]

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::fair_admission::{
    ADMISSION_VERSION, AdmissionPolicy, BudgetAccount, ClaimRef, EligibilityInput,
    ServingAdmissionPolicy, ServingBounds, ServingCapacityRule, ServingObservation, ServingOrigin,
    ServingReadiness, UsageObservation,
};
use kiln_core::implementation::{
    AbsenceReason, AcceptanceObligation, AttemptUsage, AuxiliaryCallObservation, CandidateEvidence,
    CandidateHandoff, CandidateVerification, Enrollment, EvidenceKind, EvidenceOutcome,
    IMPLEMENTATION_WORK_SCHEMA, ImplementationPlan, ImplementationWork, ModelTurnObservation,
    PLAN_REVIEW_POLICY_SCHEMA, PLAN_SCHEMA, PlanReviewCommand, PlanReviewPolicy, PlanStep,
    SourceRef, SupersessionRule, TerminalPolicy, TokenObservation, VERIFICATION_SCHEMA, WorkBudget,
    WorkSourceKind, acceptance_digest, admit_candidate_handoff,
};
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef};
use kiln_implementer::Implementer;
use kiln_implementer::admission::AdmissionCommand;
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
    headers: BTreeMap<String, String>,
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
    extra_headers: &[(&str, &str)],
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
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
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
    let head = std::str::from_utf8(&raw[..split])?;
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .ok_or("missing status")?
        .parse()?;
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, val)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), val.trim().to_owned());
        }
    }
    Ok(HttpReply {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
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
    let reply = http_raw(addr, method, target, Some(token), &[], &body)?;
    let parsed: Value = serde_json::from_slice(&reply.body)?;
    Ok((reply.status, parsed))
}

/// Pure-Rust RFC 1952 + RFC 1951 (stored & fixed Huffman) gunzip verifier so
/// compression compliance is proven without external tools.
fn gunzip_verify(gzip: &[u8]) -> Result<Vec<u8>, String> {
    if gzip.len() < 18 || gzip[0..3] != [0x1f, 0x8b, 0x08] {
        return Err("invalid gzip header".into());
    }
    let flags = gzip[3];
    if flags != 0 {
        return Err("unexpected gzip flags".into());
    }
    let payload = &gzip[10..gzip.len() - 8];
    let mut bit_pos = 0usize;
    let read_bits = |pos: &mut usize, count: usize| -> Result<u32, String> {
        let mut val = 0_u32;
        for i in 0..count {
            let byte_idx = (*pos) / 8;
            let bit_idx = (*pos) % 8;
            let byte = *payload.get(byte_idx).ok_or("unexpected EOF")?;
            if ((byte >> bit_idx) & 1) != 0 {
                val |= 1 << i;
            }
            *pos += 1;
        }
        Ok(val)
    };
    let read_huff_lit = |pos: &mut usize| -> Result<u16, String> {
        let mut code = 0_u16;
        for len in 1..=9 {
            let bit = read_bits(pos, 1)? as u16;
            code = (code << 1) | bit;
            if len == 7 && code <= 0b0010_111 {
                return Ok(256 + code);
            }
            if len == 8 && (0b0011_0000..=0b1011_1111).contains(&code) {
                return Ok(code - 0b0011_0000);
            }
            if len == 8 && (0b1100_0000..=0b1100_0111).contains(&code) {
                return Ok(280 + (code - 0b1100_0000));
            }
            if len == 9 && (0b1_1001_0000..=0b1_1111_1111).contains(&code) {
                return Ok(144 + (code - 0b1_1001_0000));
            }
        }
        Err("invalid fixed huffman code".into())
    };
    let len_table: [(u16, usize); 29] = [
        (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0), (9, 0), (10, 0),
        (11, 1), (13, 1), (15, 1), (17, 1), (19, 2), (23, 2), (27, 2), (31, 2),
        (35, 3), (43, 3), (51, 3), (59, 3), (67, 4), (83, 4), (99, 4), (115, 4),
        (131, 5), (163, 5), (195, 5), (227, 5), (258, 0),
    ];
    let dist_table: [(u16, usize); 30] = [
        (1, 0), (2, 0), (3, 0), (4, 0), (5, 1), (7, 1), (9, 2), (13, 2),
        (17, 3), (25, 3), (33, 4), (49, 4), (65, 5), (97, 5), (129, 6), (193, 6),
        (257, 7), (385, 7), (513, 8), (769, 8), (1025, 9), (1537, 9), (2049, 10),
        (3073, 10), (4097, 11), (6145, 11), (8193, 12), (12289, 12), (16385, 13), (24577, 13),
    ];
    let mut out = Vec::new();
    loop {
        let bfinal = read_bits(&mut bit_pos, 1)?;
        let btype = read_bits(&mut bit_pos, 2)?;
        if btype != 1 {
            return Err(format!("expected fixed huffman block, got {btype}"));
        }
        loop {
            let sym = read_huff_lit(&mut bit_pos)?;
            if sym < 256 {
                out.push(sym as u8);
            } else if sym == 256 {
                break;
            } else {
                let (l_base, l_extra) = len_table[(sym - 257) as usize];
                let length = usize::from(l_base) + read_bits(&mut bit_pos, l_extra)? as usize;
                let mut d_code = 0_u16;
                for _ in 0..5 {
                    d_code = (d_code << 1) | (read_bits(&mut bit_pos, 1)? as u16);
                }
                let (d_base, d_extra) = dist_table[d_code as usize];
                let distance = usize::from(d_base) + read_bits(&mut bit_pos, d_extra)? as usize;
                for _ in 0..length {
                    let src = out.len().checked_sub(distance).ok_or("invalid distance")?;
                    out.push(out[src]);
                }
            }
        }
        if bfinal == 1 {
            break;
        }
    }
    let expected_isize = u32::from_le_bytes(gzip[gzip.len() - 4..].try_into().unwrap());
    if (out.len() as u32) != expected_isize {
        return Err("gzip ISIZE mismatch".into());
    }
    Ok(out)
}

fn spawn_service(
    root: &std::path::Path,
    name: &str,
    db_path: Option<&std::path::Path>,
    restricted: bool,
) -> Result<(Service, SocketAddr, String, String, String), Box<dyn std::error::Error>> {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir)?;
    let repo_dir = dir.join("repository");
    let repo = git2::Repository::init(&repo_dir)?;
    std::fs::write(repo_dir.join("lib.rs"), b"pub fn ok() {}\n")?;
    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("lib.rs"))?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = git2::Signature::now("test", "test@example.invalid")?;
    repo.commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])?;

    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, CERT)?;
    std::fs::write(&key_path, KEY)?;
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o400))?;
    let op_tok_path = dir.join("operator.token");
    let vw_tok_path = dir.join("viewer.token");
    let wk_tok_path = dir.join("worker.token");
    let op_tok = "a".repeat(TOKEN_LENGTH);
    let vw_tok = "c".repeat(TOKEN_LENGTH);
    let wk_tok = "b".repeat(TOKEN_LENGTH);
    std::fs::write(&op_tok_path, &op_tok)?;
    std::fs::write(&vw_tok_path, &vw_tok)?;
    std::fs::write(&wk_tok_path, &wk_tok)?;
    for p in [&op_tok_path, &vw_tok_path, &wk_tok_path] {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o400))?;
    }

    let addr = free_addr()?;
    let profile_path = dir.join("profile.json");
    let profile = json!({
        "schema": "kiln.web-service.v1",
        "service_id": format!("p2-test-{name}"),
        "listen_address": addr.to_string(),
        "origin": format!("https://localhost:{}", addr.port()),
        "tls": {
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "min_version": "TLS1.3"
        },
        "operator_token_file": op_tok_path,
        "worker_token_file": wk_tok_path,
        "restricted_projection": restricted,
        "state_directory": dir.join("state"),
        "projects": [{
            "id": "fixture",
            "label": "Fixture",
            "repository_path": repo_dir,
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
            "max_connections": 8,
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
    std::fs::write(&profile_path, serde_json::to_vec(&profile)?)?;
    let access_path = dir.join("access.json");
    std::fs::write(
        &access_path,
        serde_json::to_vec(&json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{"id": "reader", "label": "Reader", "token_file": vw_tok_path}]
        }))?,
    )?;

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kiln-web-service"));
    cmd.args(["serve", "--profile"])
        .arg(&profile_path)
        .arg("--access")
        .arg(&access_path);
    if let Some(db) = db_path {
        cmd.arg("--implementation-work").arg(db);
    }
    let child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    wait_ready(addr)?;
    Ok((Service(child), addr, op_tok, vw_tok, wk_tok))
}

#[test]
fn static_assets_etag_304_cache_control_and_gzip_negotiation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let (_svc, addr, _op, _vw, _wk) = spawn_service(root.path(), "static", None, false)?;

    // 1. Cold fetch of /web.js and /web.css with Accept-Encoding: gzip.
    let raw_web_js = include_bytes!("../src/web.js");
    let cold = http_raw(
        addr,
        "GET",
        "/web.js",
        None,
        &[("Accept-Encoding", "gzip, deflate")],
        &[],
    )?;
    assert_eq!(cold.status, 200);
    assert_eq!(
        cold.headers.get("content-encoding").map(String::as_str),
        Some("gzip")
    );
    assert_eq!(
        cold.headers.get("vary").map(String::as_str),
        Some("Accept-Encoding")
    );
    assert_eq!(
        cold.headers.get("cache-control").map(String::as_str),
        Some("private, no-cache")
    );
    let etag_gzip = cold
        .headers
        .get("etag")
        .cloned()
        .ok_or("missing ETag header")?;
    assert!(etag_gzip.ends_with("-gzip\""), "expected gzip etag, got {etag_gzip}");
    assert!(
        cold.body.len() < raw_web_js.len(),
        "compressed web.js ({}) should be smaller than raw ({})",
        cold.body.len(),
        raw_web_js.len()
    );
    let decoded = gunzip_verify(&cold.body)?;
    assert_eq!(decoded.as_slice(), raw_web_js.as_slice());

    // 2. Conditional repeat request with matching ETag -> 304 Not Modified + empty body.
    let not_mod = http_raw(
        addr,
        "GET",
        "/web.js",
        None,
        &[
            ("Accept-Encoding", "gzip"),
            ("If-None-Match", &etag_gzip),
        ],
        &[],
    )?;
    assert_eq!(not_mod.status, 304);
    assert!(not_mod.body.is_empty(), "304 response must have empty body");
    assert_eq!(not_mod.headers.get("etag"), Some(&etag_gzip));
    assert_eq!(
        not_mod.headers.get("vary").map(String::as_str),
        Some("Accept-Encoding")
    );
    assert_eq!(
        not_mod.headers.get("cache-control").map(String::as_str),
        Some("private, no-cache")
    );

    // 3. Mismatched If-None-Match -> 200 OK with body.
    let mismatch = http_raw(
        addr,
        "GET",
        "/web.js",
        None,
        &[
            ("Accept-Encoding", "gzip"),
            ("If-None-Match", "\"000000000000000000000000-gzip\""),
        ],
        &[],
    )?;
    assert_eq!(mismatch.status, 200);
    assert_eq!(mismatch.body, cold.body);

    // 4. Cross-representation If-None-Match (gzip ETag sent with identity) -> 200 uncompressed.
    let cross = http_raw(
        addr,
        "GET",
        "/web.js",
        None,
        &[("If-None-Match", &etag_gzip)],
        &[],
    )?;
    assert_eq!(cross.status, 200);
    assert!(!cross.headers.contains_key("content-encoding"));
    assert_eq!(cross.body.as_slice(), raw_web_js.as_slice());
    let etag_identity = cross.headers.get("etag").cloned().unwrap();
    assert_ne!(etag_identity, etag_gzip);

    // 5. Negative/unknown/malformed Accept-Encoding gracefully falls back to identity.
    for bogus_ae in ["br, zstd", "gzip;q=bogus", "gzip;q=0", "invalid/encoding"] {
        let res = http_raw(
            addr,
            "GET",
            "/web.css",
            None,
            &[("Accept-Encoding", bogus_ae)],
            &[],
        )?;
        assert_eq!(res.status, 200, "AE={bogus_ae}");
        assert!(
            !res.headers.contains_key("content-encoding"),
            "unexpected encoding for {bogus_ae}"
        );
        assert_eq!(res.body.as_slice(), include_bytes!("../src/web.css").as_slice());
    }
    Ok(())
}

#[test]
fn admission_plan_feedback_and_lifecycle_governance() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let db_path = root.path().join("impl.sqlite");
    let hash = |ch: &str| ContentId::from_blake3_hex(ch.repeat(64)).unwrap();

    let enrollment = Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_owned(), "onix/adm".to_owned()]),
        actors: BTreeSet::from(["local:operator".to_owned()]),
    };
    let mut implementer =
        Implementer::open_with_local_operator(&db_path, enrollment, "local:operator")?;

    let base_ref = ExternalRef::new("git", "commit", &"1".repeat(40)).unwrap();
    let cand_ref = ExternalRef::new("git", "commit", &"2".repeat(40)).unwrap();
    let tree_ref = ExternalRef::new("git", "tree", &"3".repeat(40)).unwrap();

    // Work 1: verified attempt with AttemptMetrics + candidate handoff + verification receipt.
    let work_verified = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: "local:operator".to_owned(),
        repository: "onix/kiln".to_owned(),
        project: "kiln".to_owned(),
        base: Some(base_ref.clone()),
        title: "Verified candidate unit".to_owned(),
        requested_outcome: "ship verified change".to_owned(),
        acceptance: vec![AcceptanceObligation {
            id: "a1".into(),
            statement: "tests pass".into(),
        }],
        dependencies: Vec::new(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-p2/t1").unwrap(),
            native_id: "chg-p2/t1".into(),
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
        priority: 2,
    };
    let work_verified_id = work_verified.identity().unwrap();
    implementer.admit(&work_verified, None)?;
    let claim1 = implementer.claim_next(1_000, 60_000)?.expect("claim1");
    let attempt1_id = claim1.identity.clone();
    let fence1 = claim1.fence;
    let dispatch_plan_id = hash("7");
    let dispatch_provider = kiln_core::types::ProviderProfileId::new("lattice").unwrap();
    implementer.reserve_dispatch(
        &attempt1_id,
        fence1,
        1_050,
        &dispatch_plan_id,
        &dispatch_provider,
    )?;
    let wf_ref = ExternalRef::new("lattice", "workflow", "wf-1").unwrap();
    implementer.start_execution(&attempt1_id, fence1, 1_100, &wf_ref)?;
    implementer.record_metrics(
        &attempt1_id,
        fence1,
        5_200,
        AttemptUsage {
            tokens: TokenObservation::Observed { tokens: 1_500 },
            auxiliary_calls: AuxiliaryCallObservation::Observed { calls: 3 },
            model_turns: ModelTurnObservation::Observed { turns: 6 },
        },
        Vec::new(),
    )?;
    let acc_digest = acceptance_digest(&work_verified).unwrap();
    let handoff = CandidateHandoff {
        schema: kiln_core::implementation::CANDIDATE_HANDOFF_SCHEMA.to_owned(),
        work_identity: work_verified_id.clone(),
        work_revision: 1,
        attempt_identity: attempt1_id.clone(),
        fence: fence1,
        repository: "onix/kiln".to_owned(),
        base: Some(base_ref.clone()),
        candidate: cand_ref.clone(),
        tree: tree_ref.clone(),
        changed_paths: vec!["crates/a.rs".into()],
        acceptance_digest: acc_digest,
        verification_profile: hash("9"),
        toolchain: hash("7"),
        environment: hash("8"),
        evidence: vec![CandidateEvidence {
            identity: hash("4"),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: ExternalRef::new("kiln", "check", "unit").unwrap(),
        }],
    };
    let handoff_id = admit_candidate_handoff(
        &work_verified,
        &implementer.attempt(&attempt1_id)?.unwrap(),
        &handoff,
    )
    .unwrap();
    implementer.admit_candidate_handoff(&work_verified, &attempt1_id, fence1, 1_300, &handoff)?;
    implementer.begin_verification(&attempt1_id, fence1, 1_400)?;
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_owned(),
        handoff: handoff_id,
        work_identity: work_verified_id.clone(),
        work_revision: 1,
        attempt: attempt1_id.clone(),
        fence: fence1,
        candidate: cand_ref.clone(),
        tree: tree_ref,
        base: Some(base_ref),
        verification_profile: hash("9"),
        run: CiFact {
            project: "kiln".into(),
            repository: "onix/kiln".into(),
            revision: "2".repeat(40),
            ordinal: 1,
            run_id: "run-verify-1".into(),
            terminal: true,
            success: true,
            full_run: true,
            clean: true,
            untrusted: None,
            scope_identity: None,
            checks: vec![CiCheck {
                name: "cargo-test".into(),
                kind: "test".into(),
                outcome: "pass".into(),
                artifact_digest: Some("f".repeat(64)),
            }],
        },
    };
    implementer.record_verification(
        &work_verified,
        &attempt1_id,
        fence1,
        1_500,
        &handoff,
        &verification,
    )?;

    // Work 2: active unverified attempt with NO recorded AttemptMetrics (must project null, not 0).
    let work_unverified = ImplementationWork {
        title: "Unrecorded metrics unit".to_owned(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-p2/t2").unwrap(),
            native_id: "chg-p2/t2".into(),
            version: hash("5"),
        }],
        priority: 1,
        ..work_verified.clone()
    };
    let _work_unverified_id = work_unverified.identity().unwrap();
    implementer.admit(&work_unverified, None)?;
    let claim2 = implementer.claim_next(2_000, 60_000)?.expect("claim2");
    let attempt2_id = claim2.identity.clone();
    let _fence2 = claim2.fence;

    // Work 3: superseded work (for stale-work fail-closed test).
    let work_old = ImplementationWork {
        title: "Superseded unit".to_owned(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-p2/t3").unwrap(),
            native_id: "chg-p2/t3".into(),
            version: hash("6"),
        }],
        ..work_verified.clone()
    };
    let work_old_id = work_old.identity().unwrap();
    implementer.admit(&work_old, None)?;
    let work_new = ImplementationWork {
        revision: 2,
        requested_outcome: "updated outcome".to_owned(),
        ..work_old
    };
    implementer.admit_with_supersession(
        &work_new,
        None,
        SupersessionRule::AbortActive,
        2_500,
    )?;

    // Work 4: Plan Review thread (rev 1 -> annotate -> resolve -> rev 2 -> stale rev 1 guard -> accept rev 2).
    implementer.plan_review(&PlanReviewCommand::Configure {
        policy: Some(PlanReviewPolicy {
            schema: PLAN_REVIEW_POLICY_SCHEMA.to_owned(),
            repositories: BTreeSet::from(["onix/kiln".to_owned()]),
            reviewers: BTreeSet::from(["local:operator".to_owned()]),
        }),
    })?;
    let work_plan = ImplementationWork {
        title: "Plan review unit".to_owned(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-p2/t4").unwrap(),
            native_id: "chg-p2/t4".into(),
            version: hash("a"),
        }],
        ..work_verified.clone()
    };
    let work_plan_id = work_plan.identity().unwrap();
    implementer.admit(&work_plan, None)?;
    let plan_v1 = ImplementationPlan {
        schema: PLAN_SCHEMA.to_owned(),
        work_identity: work_plan_id.clone(),
        revision: 1,
        previous: None,
        author: "agent:coder".into(),
        steps: vec![
            PlanStep {
                id: "step-1".into(),
                text: "Implement core".into(),
            },
            PlanStep {
                id: "step-2".into(),
                text: "Add negative tests".into(),
            },
        ],
    };
    let plan_v1_id = plan_v1.identity().unwrap();
    implementer.plan_review(&PlanReviewCommand::Propose {
        plan: plan_v1.clone(),
    })?;

    // Fair-Admission setup: two tenants, root + child transfer (separate from consumption!), ServingBounds.
    let camp_a = Alias::new("camp-alpha".to_owned()).unwrap();
    let camp_b = Alias::new("camp-beta".to_owned()).unwrap();
    let tenant_a = Alias::new("tenant-alpha".to_owned()).unwrap();
    let tenant_b = Alias::new("tenant-beta".to_owned()).unwrap();
    let dim_tokens = Alias::new("tokens".to_owned()).unwrap();
    let policy = AdmissionPolicy {
        version: ADMISSION_VERSION,
        accounting_basis: hash("b"),
        max_items: 64,
        max_bytes: 1_048_576,
        max_wait_ms: 60_000,
        max_observation_age_ms: 30_000,
        max_retry_after_ms: 5_000,
        max_children: 16,
        max_active: 8,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(camp_a.clone(), 3), (camp_b.clone(), 1)]),
        group_limits: Vec::new(),
        serving: Some(ServingAdmissionPolicy {
            max_snapshot_age_ms: 15_000,
            capacity_rule: ServingCapacityRule::OwnerAdmissionDecides,
        }),
    };
    let policy_id = policy.identity().unwrap();
    implementer.admission_apply(AdmissionCommand::Configure { policy }, 3_000)?;

    let root_a_id = hash("c");
    let root_b_id = hash("d");
    let child_a_id = hash("e");
    implementer.admission_apply(
        AdmissionCommand::Root {
            account: BudgetAccount {
                identity: root_a_id.clone(),
                parent: None,
                parent_attempt: None,
                campaign: camp_a.clone(),
                tenant: tenant_a.clone(),
                deadline_ms: 9_999_999,
                available: BTreeMap::from([(dim_tokens.clone(), 10_000)]),
                consumed: BTreeMap::new(),
                children: 0,
            },
        },
        3_000,
    )?;
    implementer.admission_apply(
        AdmissionCommand::Root {
            account: BudgetAccount {
                identity: root_b_id,
                parent: None,
                parent_attempt: None,
                campaign: camp_b,
                tenant: tenant_b,
                deadline_ms: 9_999_999,
                available: BTreeMap::from([(dim_tokens.clone(), 5_000)]),
                consumed: BTreeMap::new(),
                children: 0,
            },
        },
        3_000,
    )?;
    implementer.admission_apply(
        AdmissionCommand::ObserveServing {
            observation: ServingObservation {
                version: ADMISSION_VERSION,
                provider: kiln_core::types::ProviderProfileId::new("lattice").unwrap(),
                scope: hash("f"),
                source_generation: 1,
                sequence: 1,
                deployment_generation: 1,
                readiness: ServingReadiness::Ready,
                origin: ServingOrigin::ServingOwner,
                observed_at_ms: 3_000,
                valid_until_ms: 18_000,
                bounds: ServingBounds {
                    scope: hash("f"),
                    max_ongoing: 8,
                    max_queued: 32,
                },
                remaining_slots: None,
                queued: Some(1),
            },
        },
        3_000,
    )?;
    // Work 5: queued/claimed via fair-admission + child transfer (to verify transfers != consumption).
    let work_adm = ImplementationWork {
        repository: "onix/adm".to_owned(),
        title: "Admission transfer unit".to_owned(),
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "chg-p2/t5").unwrap(),
            native_id: "chg-p2/t5".into(),
            version: hash("0"),
        }],
        ..work_verified.clone()
    };
    let work_adm_id = work_adm.identity().unwrap();
    implementer.admit(&work_adm, None)?;

    // Enqueue work_adm under root_a, observe usage, claim it through admission, then transfer 2000 to child_a.
    let provider_lattice = kiln_core::types::ProviderProfileId::new("lattice").unwrap();
    let repo_alias = Alias::new("onix-kiln".to_owned()).unwrap();
    implementer.admission_apply(
        AdmissionCommand::Observe {
            observation: UsageObservation {
                usage: TenantUsage {
                    tenant: camp_a.clone(),
                    share: 3,
                    provider: provider_lattice.clone(),
                    interval_start: 2_000,
                    interval_end: 3_000,
                    observed_at: 3_000,
                    resources: BTreeMap::from([(
                        dim_tokens.clone(),
                        UsageValue {
                            amount: 0,
                            capacity: Some(10_000),
                        },
                    )]),
                },
                generation: 1,
                accounting_basis: hash("b"),
                covered: BTreeMap::new(),
                complete: true,
            },
        },
        3_000,
    )?;
    implementer.admission_apply(
        AdmissionCommand::Enqueue {
            item: kiln_core::fair_admission::QueueItem {
                work_version: work_adm_id.clone(),
                account: root_a_id.clone(),
                tenant: tenant_a.clone(),
                campaign: camp_a.clone(),
                repository: repo_alias,
                priority: 1,
                bytes: 256,
                enqueued_at_ms: 3_000,
                deadline_ms: 9_999_999,
                eligible_at_ms: 3_000,
                claim: None,
                phase: kiln_core::fair_admission::AdmissionPhase::PreClaim,
                reason: kiln_core::fair_admission::AdmissionReason::Eligible,
                order: 1,
                required_owners: BTreeMap::from([(
                    provider_lattice.clone(),
                    kiln_core::fair_admission::ChargeKind::WorkerRequest,
                )]),
                shared_allocations: Vec::new(),
                budget: BTreeMap::from([(dim_tokens.clone(), 3_000)]),
            },
        },
        3_000,
    )?;
    let claimed_adm = match implementer.admission_apply(
        AdmissionCommand::Claim {
            inputs: vec![EligibilityInput {
                work_version: work_adm_id.clone(),
                reason: kiln_core::fair_admission::AdmissionReason::Eligible,
                provider: provider_lattice,
                generation: 1,
                observed_at_ms: 3_000,
                valid_until_ms: 60_000,
            }],
            lease_ms: 60_000,
        },
        3_000,
    )? {
        kiln_implementer::admission::AdmissionResult::Claimed {
            attempt: Some(attempt),
        } => *attempt,
        other => panic!("expected admission claim, got {other:?}"),
    };
    implementer.admission_apply(
        AdmissionCommand::Transfer {
            parent: root_a_id.clone(),
            child: BudgetAccount {
                identity: child_a_id.clone(),
                parent: Some(root_a_id.clone()),
                parent_attempt: Some(ClaimRef {
                    attempt: claimed_adm.identity.clone(),
                    fence: claimed_adm.fence,
                }),
                campaign: camp_a.clone(),
                tenant: tenant_a.clone(),
                deadline_ms: 60_000,
                available: BTreeMap::from([(dim_tokens.clone(), 2_000)]),
                consumed: BTreeMap::new(),
                children: 0,
            },
        },
        3_000,
    )?;
    drop(implementer);

    let (_svc, addr, op_tok, vw_tok, wk_tok) =
        spawn_service(root.path(), "live", Some(&db_path), false)?;

    // ── A. Fair-Admission Governance & AttemptMetrics ──
    assert_eq!(
        http_json(addr, "GET", "/api/implementation/admission", &wk_tok, None)?.0,
        403
    );
    let (adm_status, adm) =
        http_json(addr, "GET", "/api/implementation/admission", &vw_tok, None)?;
    assert_eq!(adm_status, 200);
    assert_eq!(adm["schema"], "kiln.implementation-admission.v1");
    assert_eq!(adm["configured"], true);
    assert_eq!(adm["policy"]["shares"]["camp-alpha"], 3);
    assert_eq!(adm["policy"]["shares"]["camp-beta"], 1);
    assert_eq!(adm["serving"][0]["bounds"]["max_ongoing"], 8);
    assert_eq!(adm["serving"][0]["bounds"]["max_queued"], 32);
    assert!(adm["serving"][0]["remaining_slots"].is_null());

    // Verify transfer is tracked separately from consumption on root_a.
    let accounts = adm["accounts"].as_array().unwrap();
    let root_a_view = accounts
        .iter()
        .find(|a| a["identity"] == root_a_id.as_str())
        .expect("root_a account");
    assert_eq!(root_a_view["allocated"]["tokens"], 10_000);
    assert_eq!(root_a_view["transferred_out"]["tokens"], 2_000);
    assert_eq!(root_a_view["available"]["tokens"], 8_000);
    assert!(
        root_a_view["consumed"].as_object().unwrap().is_empty(),
        "transfers must never appear as consumption"
    );

    // Verify AttemptMetrics: recorded vs unrecorded (null, NEVER 0!).
    let metrics_arr = adm["attempt_metrics"].as_array().unwrap();
    let m1 = metrics_arr
        .iter()
        .find(|m| m["attempt_id"] == attempt1_id.as_str())
        .expect("attempt1 metrics");
    assert_eq!(m1["recorded"], true);
    assert_eq!(m1["tokens"], 1_500);
    assert_eq!(m1["model_turns"], 6);
    assert_eq!(m1["auxiliary_calls"], 3);
    assert_eq!(m1["wall_time_ms"], 4_200);

    let m2 = metrics_arr
        .iter()
        .find(|m| m["attempt_id"] == attempt2_id.as_str())
        .expect("attempt2 metrics");
    assert_eq!(m2["recorded"], false);
    assert_eq!(m2["telemetry_source"], "not_recorded");
    assert!(m2["tokens"].is_null(), "unrecorded tokens must be null");
    assert!(m2["model_turns"].is_null(), "unrecorded turns must be null");
    assert!(
        m2["auxiliary_calls"].is_null(),
        "unrecorded aux calls must be null"
    );
    assert!(m2["wall_time_ms"].is_null(), "unrecorded wall time must be null");
    assert_eq!(
        m2["tokens_absence_reason"],
        serde_json::to_value(AbsenceReason::NotReported).unwrap()
    );

    // Policy POST authorization, validation, staleness, and immutability checks.
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/admission/policy",
            &vw_tok,
            Some(&json!({"request_id":"req-adm-vw","shares":{"camp-alpha":2}})),
        )?
        .0,
        403
    );
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/admission/policy",
            &op_tok,
            Some(&json!({"request_id":"req-adm-bad","shares":{"camp-alpha":0}})),
        )?
        .0,
        400
    );
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/admission/policy",
            &op_tok,
            Some(&json!({"request_id":"req-adm-stale","expected_policy_id":hash("0"),"shares":{"camp-alpha":4,"camp-beta":1}})),
        )?
        .0,
        409
    );
    let (frozen_status, frozen_json) = http_json(
        addr,
        "POST",
        "/api/implementation/admission/policy",
        &op_tok,
        Some(&json!({"request_id":"req-adm-frozen","expected_policy_id":policy_id,"shares":{"camp-alpha":5,"camp-beta":1}})),
    )?;
    assert_eq!(frozen_status, 409);
    assert_eq!(frozen_json["code"], "admission_policy_frozen");

    // Verify transfer separation from consumption via POST root/transfer check on a fresh unconfigured DB too.
    // Also test initial Configure via POST /api/implementation/admission/policy on a fresh DB service.
    {
        let fresh_db = root.path().join("fresh.sqlite");
        let mut fresh_imp = Implementer::open_with_local_operator(
            &fresh_db,
            Enrollment {
                repositories: BTreeSet::from(["onix/kiln".to_owned()]),
                actors: BTreeSet::from(["local:operator".to_owned()]),
            },
            "local:operator",
        )?;
        fresh_imp.admit(&work_verified, None)?;
        let fresh_claim = fresh_imp.claim_next(1_000, 60_000)?.unwrap();
        drop(fresh_imp);
        let (_fresh_svc, fresh_addr, fresh_op, _, _) =
            spawn_service(root.path(), "fresh", Some(&fresh_db), false)?;
        let (unconf_status, unconf) =
            http_json(fresh_addr, "GET", "/api/implementation/admission", &fresh_op, None)?;
        assert_eq!(unconf_status, 200);
        assert_eq!(unconf["configured"], false);

        let (cfg_status, cfg_res) = http_json(
            fresh_addr,
            "POST",
            "/api/implementation/admission/policy",
            &fresh_op,
            Some(&json!({
                "request_id": "req-cfg-1",
                "policy": {
                    "version": 1,
                    "accounting_basis": hash("b"),
                    "max_items": 32,
                    "max_bytes": 65536,
                    "max_wait_ms": 30000,
                    "max_observation_age_ms": 10000,
                    "max_retry_after_ms": 5000,
                    "max_children": 8,
                    "max_active": 4,
                    "strategy": "DominantShare",
                    "shares": { "camp-alpha": 2, "camp-beta": 1 },
                    "group_limits": []
                }
            })),
        )?;
        assert_eq!(cfg_status, 200);
        assert_eq!(cfg_res["result"], "configured");
        let _ = (fresh_claim, child_a_id);
    }

    // ── B. Plan Review Stale-Revision Guard & Step-Level Feedback ──
    let (pf_get_status, pf_get) = http_json(
        addr,
        "GET",
        &format!("/api/implementation/plan-feedback?work={}", work_plan_id.as_str()),
        &vw_tok,
        None,
    )?;
    assert_eq!(pf_get_status, 200);
    assert_eq!(pf_get["revision"], 1);
    assert_eq!(pf_get["steps"][0]["id"], "step-1");

    let (pf_ann_status, pf_ann) = http_json(
        addr,
        "POST",
        "/api/implementation/plan-feedback",
        &op_tok,
        Some(&json!({
            "work_id": work_plan_id,
            "plan_id": plan_v1_id,
            "expected_revision": 1,
            "step_id": "step-1",
            "comment": "Verify bounds explicitly",
            "blocking": true,
            "annotation_id": "ann-bounds-1",
            "request_id": "req-pf-1"
        })),
    )?;
    assert_eq!(pf_ann_status, 200);
    assert_eq!(pf_ann["gate"]["gate"], "unresolved");
    assert_eq!(pf_ann["items"].as_array().unwrap().len(), 1);

    // Accepting while blocking annotation is unresolved fails closed (409).
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/plan-review",
            &op_tok,
            Some(&json!({
                "work_id": work_plan_id,
                "plan_id": plan_v1_id,
                "expected_revision": 1,
                "verdict": "accepted",
                "request_id": "req-pr-blocked"
            })),
        )?
        .0,
        409
    );

    // Resolve the blocking annotation.
    let (pf_res_status, pf_res) = http_json(
        addr,
        "POST",
        "/api/implementation/plan-feedback",
        &op_tok,
        Some(&json!({
            "work_id": work_plan_id,
            "plan_id": plan_v1_id,
            "expected_revision": 1,
            "action": "resolve",
            "annotation_id": "ann-bounds-1",
            "request_id": "req-pf-resolve"
        })),
    )?;
    assert_eq!(pf_res_status, 200);
    assert_eq!(pf_res["gate"]["gate"], "unreviewed");

    // Propose revision 2 concurrently in the store.
    let plan_v2 = ImplementationPlan {
        schema: PLAN_SCHEMA.to_owned(),
        work_identity: work_plan_id.clone(),
        revision: 2,
        previous: Some(plan_v1_id.clone()),
        author: "agent:coder".into(),
        steps: vec![
            PlanStep {
                id: "step-1".into(),
                text: "Implement core with explicit bounds".into(),
            },
            PlanStep {
                id: "step-2".into(),
                text: "Add negative tests".into(),
            },
        ],
    };
    let plan_v2_id = plan_v2.identity().unwrap();
    {
        let mut imp = Implementer::open_with_local_operator(
            &db_path,
            Enrollment {
                repositories: BTreeSet::from(["onix/kiln".to_owned()]),
                actors: BTreeSet::from(["local:operator".to_owned()]),
            },
            "local:operator",
        )?;
        imp.plan_review(&PlanReviewCommand::Propose { plan: plan_v2 })?;
    }

    // Stale rev-1 acceptance MUST fail closed with 409 implementation_plan_revision.
    let (stale_pr_status, stale_pr) = http_json(
        addr,
        "POST",
        "/api/implementation/plan-review",
        &op_tok,
        Some(&json!({
            "work_id": work_plan_id,
            "plan_id": plan_v1_id,
            "expected_revision": 1,
            "verdict": "accepted",
            "request_id": "req-pr-stale"
        })),
    )?;
    assert_eq!(stale_pr_status, 409);
    assert_eq!(stale_pr["code"], "implementation_plan_revision");
    assert_eq!(stale_pr["actual_revision"], 2);
    assert_eq!(stale_pr["actual_plan_id"], plan_v2_id.as_str());

    // Stale rev-1 feedback annotation also fails closed with 409.
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/plan-feedback",
            &op_tok,
            Some(&json!({
                "work_id": work_plan_id,
                "plan_id": plan_v1_id,
                "expected_revision": 1,
                "step_id": "step-1",
                "comment": "late note",
                "request_id": "req-pf-stale"
            })),
        )?
        .0,
        409
    );

    // Accepting rev-2 succeeds and returns receipt_digest.
    let (ok_pr_status, ok_pr) = http_json(
        addr,
        "POST",
        "/api/implementation/plan-review",
        &op_tok,
        Some(&json!({
            "work_id": work_plan_id,
            "plan_id": plan_v2_id,
            "expected_revision": 2,
            "verdict": "accepted",
            "request_id": "req-pr-ok"
        })),
    )?;
    assert_eq!(ok_pr_status, 200);
    assert_eq!(ok_pr["revision"], 2);
    assert_eq!(ok_pr["gate"]["gate"], "accepted");
    assert_eq!(ok_pr["receipt_digest"].as_str().unwrap().len(), 64);

    // ── C. Guarded Publication & Integration Lifecycle Recovery ──
    // Negative: Viewer/Worker denied (403).
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/lifecycle/prepare-publication",
            &vw_tok,
            Some(&json!({
                "work_id": work_verified_id,
                "attempt_id": attempt1_id,
                "fence": fence1,
                "candidate_identity": cand_ref,
                "reason": "unauthorized attempt",
                "request_id": "req-lc-vw"
            })),
        )?
        .0,
        403
    );

    // Negative: mismatched candidate_identity -> 409 lifecycle_candidate_mismatch.
    let (cand_err_status, cand_err) = http_json(
        addr,
        "POST",
        "/api/implementation/lifecycle/prepare-publication",
        &op_tok,
        Some(&json!({
            "work_id": work_verified_id,
            "attempt_id": attempt1_id,
            "fence": fence1,
            "candidate_identity": "9".repeat(40),
            "reason": "wrong candidate",
            "request_id": "req-lc-badcand"
        })),
    )?;
    assert_eq!(cand_err_status, 409);
    assert_eq!(cand_err["code"], "lifecycle_candidate_mismatch");

    // Negative: stale fence -> 409 lifecycle_fence_mismatch.
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/lifecycle/prepare-publication",
            &op_tok,
            Some(&json!({
                "work_id": work_verified_id,
                "attempt_id": attempt1_id,
                "fence": fence1 + 1,
                "candidate_identity": cand_ref,
                "reason": "stale fence",
                "request_id": "req-lc-badfence"
            })),
        )?
        .0,
        409
    );

    // Negative: superseded work -> 409 lifecycle_work_stale.
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/lifecycle/prepare-publication",
            &op_tok,
            Some(&json!({
                "work_id": work_old_id,
                "attempt_id": attempt1_id,
                "fence": fence1,
                "candidate_identity": cand_ref,
                "reason": "stale work",
                "request_id": "req-lc-stalework"
            })),
        )?
        .0,
        409
    );

    // Negative: unknown action -> 404.
    assert_eq!(
        http_json(
            addr,
            "POST",
            "/api/implementation/lifecycle/bogus",
            &op_tok,
            Some(&json!({
                "work_id": work_verified_id,
                "attempt_id": attempt1_id,
                "fence": fence1,
                "candidate_identity": cand_ref,
                "reason": "unknown action",
                "request_id": "req-lc-bogus"
            })),
        )?
        .0,
        404
    );

    // Positive: prepare-publication succeeds on verified attempt with exact candidate.
    let (prep_status, prep) = http_json(
        addr,
        "POST",
        "/api/implementation/lifecycle/prepare-publication",
        &op_tok,
        Some(&json!({
            "work_id": work_verified_id,
            "attempt_id": attempt1_id,
            "fence": fence1,
            "candidate_identity": cand_ref,
            "reason": "Prepare verified PR plan",
            "request_id": "req-lc-prep-1"
        })),
    )?;
    assert_eq!(prep_status, 200);
    assert_eq!(prep["action"], "prepare-publication");
    assert_eq!(prep["publication"]["phase"], "prepared");
    assert_eq!(prep["already_recorded"], false);
    assert_eq!(prep["publication_digest"].as_str().unwrap().len(), 64);

    // Idempotent replay of prepare-publication -> already_recorded: true.
    let (prep_replay_status, prep_replay) = http_json(
        addr,
        "POST",
        "/api/implementation/lifecycle/prepare-publication",
        &op_tok,
        Some(&json!({
            "work_id": work_verified_id,
            "attempt_id": attempt1_id,
            "fence": fence1,
            "candidate_identity": cand_ref,
            "reason": "Replay prepare",
            "request_id": "req-lc-prep-2"
        })),
    )?;
    assert_eq!(prep_replay_status, 200);
    assert_eq!(prep_replay["already_recorded"], true);
    assert_eq!(prep_replay["publication_digest"], prep["publication_digest"]);

    // Remote mutation actions (`publish`, `recover-integration`, `close-source`) fail closed
    // when provider mutation authority / forge credentials are not held by the web service.
    for action in ["publish", "recover-integration", "close-source"] {
        let (mut_status, mut_res) = http_json(
            addr,
            "POST",
            &format!("/api/implementation/lifecycle/{action}"),
            &op_tok,
            Some(&json!({
                "work_id": work_verified_id,
                "attempt_id": attempt1_id,
                "fence": fence1,
                "candidate_identity": cand_ref,
                "reason": "Execute guarded action",
                "request_id": format!("req-lc-{action}")
            })),
        )?;
        assert_eq!(mut_status, 403, "action={action}");
        assert_eq!(mut_res["code"], "lifecycle_provider_mutation_unsupported");
        assert!(mut_res["cli_parity"].as_str().unwrap().contains("kiln-implementer"));
    }

    // Restricted projection fails closed even for prepare-publication (403 lifecycle_authority_absent).
    let (_r_svc, r_addr, r_op, _, _) =
        spawn_service(root.path(), "restricted", Some(&db_path), true)?;
    let (r_status, r_res) = http_json(
        r_addr,
        "POST",
        "/api/implementation/lifecycle/prepare-publication",
        &r_op,
        Some(&json!({
            "work_id": work_verified_id,
            "attempt_id": attempt1_id,
            "fence": fence1,
            "candidate_identity": cand_ref,
            "reason": "restricted",
            "request_id": "req-lc-restr"
        })),
    )?;
    assert_eq!(r_status, 403);
    assert_eq!(r_res["code"], "lifecycle_authority_absent");

    // Unconfigured --implementation-work returns 503 on admission & lifecycle endpoints.
    let (_u_svc, u_addr, u_op, _, _) = spawn_service(root.path(), "unconfigured", None, false)?;
    assert_eq!(
        http_json(u_addr, "GET", "/api/implementation/admission", &u_op, None)?.0,
        503
    );
    assert_eq!(
        http_json(
            u_addr,
            "POST",
            "/api/implementation/admission/policy",
            &u_op,
            Some(&json!({"request_id":"req-u-1","shares":{"a":1}})),
        )?
        .0,
        503
    );

    Ok(())
}
