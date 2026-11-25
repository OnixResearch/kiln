// r[verify kiln.web.developer_loop.cli]
use kiln_web_service::developer::{DeveloperProfile, load_profile, outcome_exit};
use kiln_web_service::runbooks::{
    ParamBinding, ParamDeclaration, ParamType, RUNBOOK_DISPATCH_RESULT_SCHEMA,
    RUNBOOKS_VIEW_SCHEMA, resolve_params,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CA_CERT: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/cert.pem");
const KEY: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/key.pem");

fn profile() -> DeveloperProfile {
    DeveloperProfile {
        schema: "kiln.web-developer.v1".into(),
        service_origin: "https://kiln.internal:8443".into(),
        token_file: "/run/credentials/kiln-developer/token".into(),
        tls_ca_path: "/etc/kiln/ca.pem".into(),
        http_timeout_ms: 1000,
        poll_interval_ms: 10,
        watch_timeout_ms: 10000,
        maximum_request_bytes: 1048576,
        maximum_response_bytes: 1048576,
    }
}
#[test]
fn reviewed_client_identity_admits_only_exact_original_profile_bytes() {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile directory", error));
    let path = root.path().join("developer.json");
    let bytes = serde_json::to_vec_pretty(&profile())
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile JSON", error));
    std::fs::write(&path, &bytes)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile write", error));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile permissions", error));
    let identity = blake3::hash(&bytes).to_hex().to_string();
    let admitted = load_profile(&path, &identity)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "exact reviewed profile", error));
    assert_eq!(admitted.service_origin, "https://kiln.internal:8443");
    let mut changed = bytes.clone();
    changed.push(b'\n');
    std::fs::write(&path, &changed)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "changed profile", error));
    assert!(load_profile(&path, &identity).is_err());
    std::fs::write(&path, &bytes)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "restored profile", error));
    let alias = root.path().join("alias.json");
    std::os::unix::fs::symlink(&path, &alias)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile symlink", error));
    assert!(load_profile(&alias, &identity).is_err());
}
#[test]
fn developer_transport_rejects_credentials_in_origin_insecure_schemes_and_unbounded_waits() {
    assert!(profile().admit().is_ok());
    for origin in [
        "http://kiln.internal",
        "https://user:secret@kiln.internal",
        "https://kiln.internal/path",
        "https://kiln.internal?token=secret",
        "https://kiln.internal/#fragment",
    ] {
        let mut rejected = profile();
        rejected.service_origin = origin.into();
        assert!(rejected.admit().is_err());
    }
    let mut rejected = profile();
    rejected.maximum_response_bytes = 0;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.watch_timeout_ms = 86_400_001;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.poll_interval_ms = rejected.watch_timeout_ms + 1;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.token_file = "token".into();
    assert!(rejected.admit().is_err());
}
#[test]
fn run_outcomes_preserve_failure_cancel_and_unknown_exit_semantics() {
    assert_eq!(outcome_exit("Pending"), Ok(None));
    assert_eq!(outcome_exit("Success"), Ok(Some(0)));
    for failed in ["DomainFailure", "Denied", "TimedOut", "ProviderError"] {
        assert_eq!(outcome_exit(failed), Ok(Some(1)));
    }
    assert_eq!(outcome_exit("Cancelled"), Ok(Some(130)));
    assert_eq!(outcome_exit("Unknown"), Ok(Some(3)));
    assert!(outcome_exit("future-unknown-value").is_err());
}

#[derive(Clone)]
struct StubResponse {
    status: u16,
    body: Value,
}

type CapturedRequests = Arc<Mutex<Vec<(String, String, Option<Value>)>>>;

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("test failure: {error:?}"))
}

fn present<T>(option: Option<T>) -> T {
    option.unwrap_or_else(|| unreachable!("expected value missing"))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

fn parse_content_length(header: &str) -> usize {
    for line in header.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            return len;
        }
    }
    0
}

fn spawn_tls_stub<F>(
    handler: F,
) -> (
    SocketAddr,
    CapturedRequests,
    Arc<AtomicBool>,
    std::thread::JoinHandle<()>,
)
where
    F: Fn(&str, &str, Option<&Value>) -> StubResponse + Send + Sync + 'static,
{
    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    checked(listener.set_nonblocking(true));
    let captured: CapturedRequests = Arc::new(Mutex::new(Vec::new()));
    let captured_thread = Arc::clone(&captured);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let handle = std::thread::spawn(move || {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let certs = checked(
            rustls_pemfile::certs(&mut std::io::Cursor::new(CERT)).collect::<Result<Vec<_>, _>>(),
        );
        let key = present(checked(rustls_pemfile::private_key(
            &mut std::io::Cursor::new(KEY),
        )));
        let tls_config = Arc::new(checked(
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(certs, key),
        ));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !stop_thread.load(Ordering::Acquire) && Instant::now() < deadline {
            match listener.accept() {
                Ok((socket, _)) => {
                    let _ = socket.set_nonblocking(false);
                    let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = socket.set_write_timeout(Some(Duration::from_secs(2)));
                    let conn = match rustls::ServerConnection::new(Arc::clone(&tls_config)) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    let mut stream = rustls::StreamOwned::new(conn, socket);
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let header_end = loop {
                        if let Some(end) = find_header_end(&buf) {
                            break Some(end);
                        }
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => break None,
                            Ok(n) => {
                                if let Some(slice) = chunk.get(..n) {
                                    buf.extend_from_slice(slice);
                                }
                            }
                        }
                    };
                    let Some(header_end) = header_end else {
                        continue;
                    };
                    let header_bytes = buf.get(..header_end).unwrap_or(&[]);
                    let header_text = String::from_utf8_lossy(header_bytes).into_owned();
                    let content_length = parse_content_length(&header_text);
                    while buf.len() < header_end + content_length {
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if let Some(slice) = chunk.get(..n) {
                                    buf.extend_from_slice(slice);
                                }
                            }
                        }
                    }
                    let request_line = header_text.lines().next().unwrap_or("");
                    let mut parts = request_line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_owned();
                    let path = parts.next().unwrap_or("").to_owned();
                    let body_json = if content_length > 0 {
                        buf.get(header_end..header_end + content_length)
                            .and_then(|raw| serde_json::from_slice::<Value>(raw).ok())
                    } else {
                        None
                    };
                    if let Ok(mut guard) = captured_thread.lock() {
                        guard.push((method.clone(), path.clone(), body_json.clone()));
                    }
                    let resp = handler(&method, &path, body_json.as_ref());
                    let resp_bytes = checked(serde_json::to_vec(&resp.body));
                    let status_text = match resp.status {
                        200 => "OK",
                        201 => "Created",
                        403 => "Forbidden",
                        404 => "Not Found",
                        _ => "Error",
                    };
                    let head = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        resp.status,
                        status_text,
                        resp_bytes.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&resp_bytes);
                    let _ = stream.flush();
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    (addr, captured, stop, handle)
}

fn write_cli_profile(root: &std::path::Path, port: u16) -> (std::path::PathBuf, String) {
    let ca_path = root.join("ca-cert.pem");
    checked(std::fs::write(&ca_path, CA_CERT));
    let token_path = root.join("developer.token");
    checked(std::fs::write(&token_path, "a".repeat(64)));
    checked(std::fs::set_permissions(
        &token_path,
        std::fs::Permissions::from_mode(0o600),
    ));

    let dev_profile = DeveloperProfile {
        schema: "kiln.web-developer.v1".into(),
        service_origin: format!("https://localhost:{port}"),
        token_file: token_path,
        tls_ca_path: ca_path,
        http_timeout_ms: 5000,
        poll_interval_ms: 10,
        watch_timeout_ms: 10000,
        maximum_request_bytes: 1048576,
        maximum_response_bytes: 1048576,
    };
    let profile_path = root.join("developer.json");
    let bytes = checked(serde_json::to_vec_pretty(&dev_profile));
    checked(std::fs::write(&profile_path, &bytes));
    checked(std::fs::set_permissions(
        &profile_path,
        std::fs::Permissions::from_mode(0o600),
    ));
    let identity = blake3::hash(&bytes).to_hex().to_string();
    (profile_path, identity)
}

#[test]
fn ci_dispatch_queries_runbook_schema_and_preserves_enum_and_path_strings() {
    let root = checked(tempfile::tempdir());
    let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, body| {
        if method == "GET" && path == "/api/runbooks" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": RUNBOOKS_VIEW_SCHEMA,
                    "configured": true,
                    "principal": "local:operator",
                    "runbooks": [{
                        "id": "edge-test",
                        "display_name": "Edge Test",
                        "description": "Tests typed coercion",
                        "mode": "read_only",
                        "lattice_bundle_id": "edge-test",
                        "lattice_bundle_identity": "b".repeat(64),
                        "lattice_workflow_name": "edge-test",
                        "lattice_workflow_revision": "c".repeat(64),
                        "params": [
                            { "name": "path", "kind": "relative_path", "required": true, "bindings": [] },
                            { "name": "mode_num", "kind": "enum", "required": true, "enum_values": ["123", "true", "fast"], "bindings": [] },
                            { "name": "flag", "kind": "bool", "required": true, "bindings": [] },
                            { "name": "limit", "kind": "integer", "required": true, "integer_min": -10, "integer_max": 10, "bindings": [] }
                        ],
                        "allowed_targets": [],
                        "can_dispatch": true,
                        "dispatch_refusal": null
                    }]
                }),
            };
        }
        if method == "POST" && path == "/api/runbooks/edge-test/dispatch" {
            let req_id = body
                .and_then(|v| v.get("request_id"))
                .and_then(Value::as_str)
                .unwrap_or("req");
            let target = body
                .and_then(|v| v.get("target"))
                .and_then(Value::as_str)
                .unwrap_or("repo@main");
            return StubResponse {
                status: 201,
                body: json!({
                    "schema": RUNBOOK_DISPATCH_RESULT_SCHEMA,
                    "request_id": req_id,
                    "replayed": false,
                    "run_id": "web-run-edge-1",
                    "runbook_id": "edge-test",
                    "target": target,
                    "source_revision": "d".repeat(40)
                }),
            };
        }
        StubResponse {
            status: 404,
            body: json!({"code": "not_found", "message": "unexpected"}),
        }
    });

    let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
    let output = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "dispatch",
                "--runbook",
                "edge-test",
                "--target",
                "onixresearch/kiln@refs/heads/main",
                "--request-id",
                "req-cli-schema",
                "--param",
                "path=123",
                "--param",
                "mode_num=true",
                "--param",
                "flag=true",
                "--param",
                "limit=-5",
            ])
            .output(),
    );
    stop.store(true, Ordering::Release);
    let _ = handle.join();

    assert!(
        output.status.success(),
        "cli failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let requests = checked(captured.lock()).clone();
    assert_eq!(requests.len(), 2);
    let post = present(
        requests
            .iter()
            .find(|(m, p, _)| m == "POST" && p == "/api/runbooks/edge-test/dispatch"),
    );
    let post_body = present(post.2.as_ref());
    let params_val = present(post_body.get("params"));
    assert_eq!(
        params_val,
        &json!({
            "path": "123",
            "mode_num": "true",
            "flag": true,
            "limit": -5
        })
    );

    let declared = vec![
        ParamDeclaration {
            name: "path".into(),
            param_type: ParamType::RelativePath { max_bytes: 64 },
            required: true,
            default: None,
            bindings: vec![ParamBinding {
                node: "job".into(),
                shell_env: "LATTICE_PARAM_PATH".into(),
            }],
        },
        ParamDeclaration {
            name: "mode_num".into(),
            param_type: ParamType::Enum {
                values: vec!["123".into(), "true".into(), "fast".into()],
            },
            required: true,
            default: None,
            bindings: vec![ParamBinding {
                node: "job".into(),
                shell_env: "LATTICE_PARAM_MODE_NUM".into(),
            }],
        },
        ParamDeclaration {
            name: "flag".into(),
            param_type: ParamType::Bool {},
            required: true,
            default: None,
            bindings: vec![ParamBinding {
                node: "job".into(),
                shell_env: "LATTICE_PARAM_FLAG".into(),
            }],
        },
        ParamDeclaration {
            name: "limit".into(),
            param_type: ParamType::Integer { min: -10, max: 10 },
            required: true,
            default: None,
            bindings: vec![ParamBinding {
                node: "job".into(),
                shell_env: "LATTICE_PARAM_LIMIT".into(),
            }],
        },
    ];
    let supplied: BTreeMap<String, Value> = present(params_val.as_object())
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    assert!(resolve_params(&declared, &supplied).is_ok());
}

#[test]
fn ci_dispatch_falls_back_gracefully_when_runbook_listing_fails_or_omits_runbook() {
    // Subcase A: GET /api/runbooks returns 403 -> POST still runs and surfaces audited 403.
    {
        let root = checked(tempfile::tempdir());
        let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, _| {
            if method == "GET" && path == "/api/runbooks" {
                return StubResponse {
                    status: 403,
                    body: json!({
                        "code": "worker_scope",
                        "message": "Role is not authorized for this operation"
                    }),
                };
            }
            if method == "POST" && path == "/api/runbooks/edge-test/dispatch" {
                return StubResponse {
                    status: 403,
                    body: json!({
                        "code": "worker_scope",
                        "message": "Role is not authorized for this operation"
                    }),
                };
            }
            StubResponse {
                status: 404,
                body: json!({"code": "not_found", "message": "unexpected"}),
            }
        });
        let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
        let output = checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args([
                    "ci",
                    "--profile",
                    &profile_path.to_string_lossy(),
                    "--identity",
                    &identity,
                    "dispatch",
                    "--runbook",
                    "edge-test",
                    "--target",
                    "onixresearch/kiln@refs/heads/main",
                    "--request-id",
                    "req-cli-403",
                    "--param",
                    "path=123",
                ])
                .output(),
        );
        stop.store(true, Ordering::Release);
        let _ = handle.join();

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("HTTP 403"), "stderr: {stderr}");
        assert!(stderr.contains("worker_scope"), "stderr: {stderr}");
        let requests = checked(captured.lock()).clone();
        assert!(
            requests
                .iter()
                .any(|(m, p, _)| m == "GET" && p == "/api/runbooks")
        );
        assert!(
            requests
                .iter()
                .any(|(m, p, _)| m == "POST" && p == "/api/runbooks/edge-test/dispatch")
        );
    }

    // Subcase B: GET /api/runbooks returns 200 with runbooks: [] -> POST still runs and surfaces audited 404.
    {
        let root = checked(tempfile::tempdir());
        let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, _| {
            if method == "GET" && path == "/api/runbooks" {
                return StubResponse {
                    status: 200,
                    body: json!({
                        "schema": RUNBOOKS_VIEW_SCHEMA,
                        "configured": true,
                        "principal": "local:operator",
                        "runbooks": []
                    }),
                };
            }
            if method == "POST" && path == "/api/runbooks/missing-rb/dispatch" {
                return StubResponse {
                    status: 404,
                    body: json!({
                        "code": "runbook_not_found",
                        "message": "Runbook was not found"
                    }),
                };
            }
            StubResponse {
                status: 404,
                body: json!({"code": "not_found", "message": "unexpected"}),
            }
        });
        let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
        let output = checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args([
                    "ci",
                    "--profile",
                    &profile_path.to_string_lossy(),
                    "--identity",
                    &identity,
                    "dispatch",
                    "--runbook",
                    "missing-rb",
                    "--target",
                    "onixresearch/kiln@refs/heads/main",
                    "--request-id",
                    "req-cli-404",
                    "--param",
                    "path=123",
                ])
                .output(),
        );
        stop.store(true, Ordering::Release);
        let _ = handle.join();

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("HTTP 404"), "stderr: {stderr}");
        assert!(stderr.contains("runbook_not_found"), "stderr: {stderr}");
        let requests = checked(captured.lock()).clone();
        assert!(
            requests
                .iter()
                .any(|(m, p, _)| m == "GET" && p == "/api/runbooks")
        );
        assert!(
            requests
                .iter()
                .any(|(m, p, _)| m == "POST" && p == "/api/runbooks/missing-rb/dispatch")
        );
    }
}

#[test]
fn ci_retry_surfaces_server_rejection_codes() {
    // Subcase A: 201 prints run JSON and sends {"request_id": ..., "mode": "exact"}.
    {
        let root = checked(tempfile::tempdir());
        let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, body| {
            if method == "POST" && path == "/api/runs/run-1/rerun" {
                let req_id = body
                    .and_then(|v| v.get("request_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("req");
                return StubResponse {
                    status: 201,
                    body: json!({
                        "id": "run-2",
                        "project_id": "fixture",
                        "request_id": req_id,
                        "outcome": "Pending",
                        "phase": "active"
                    }),
                };
            }
            StubResponse {
                status: 404,
                body: json!({"code": "not_found", "message": "unexpected"}),
            }
        });
        let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
        let output = checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args([
                    "ci",
                    "--profile",
                    &profile_path.to_string_lossy(),
                    "--identity",
                    &identity,
                    "retry",
                    "--run",
                    "run-1",
                    "--request-id",
                    "cli-retry-ok",
                ])
                .output(),
        );
        stop.store(true, Ordering::Release);
        let _ = handle.join();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout: Value = checked(serde_json::from_slice(&output.stdout));
        assert_eq!(stdout.get("id"), Some(&json!("run-2")));
        let requests = checked(captured.lock()).clone();
        assert_eq!(
            requests.as_slice(),
            &[(
                "POST".into(),
                "/api/runs/run-1/rerun".into(),
                Some(json!({"request_id": "cli-retry-ok", "mode": "exact"}))
            )]
        );
    }

    // Subcase B: 403 runbook_grant_required fails non-zero and prints exact server error.
    {
        let root = checked(tempfile::tempdir());
        let (addr, _captured, stop, handle) = spawn_tls_stub(|method, path, _| {
            if method == "POST" && path == "/api/runs/run-1/rerun" {
                return StubResponse {
                    status: 403,
                    body: json!({
                        "code": "runbook_grant_required",
                        "message": "Role is not authorized for this operation"
                    }),
                };
            }
            StubResponse {
                status: 404,
                body: json!({"code": "not_found", "message": "unexpected"}),
            }
        });
        let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
        let output = checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args([
                    "ci",
                    "--profile",
                    &profile_path.to_string_lossy(),
                    "--identity",
                    &identity,
                    "retry",
                    "--run",
                    "run-1",
                    "--request-id",
                    "cli-retry-revoked",
                ])
                .output(),
        );
        stop.store(true, Ordering::Release);
        let _ = handle.join();

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("HTTP 403"), "stderr: {stderr}");
        assert!(
            stderr.contains("runbook_grant_required"),
            "stderr: {stderr}"
        );
    }

    // Subcase C: 409 runbook_mode_changed fails non-zero and prints exact server error.
    {
        let root = checked(tempfile::tempdir());
        let (addr, _captured, stop, handle) = spawn_tls_stub(|method, path, _| {
            if method == "POST" && path == "/api/runs/run-1/rerun" {
                return StubResponse {
                    status: 409,
                    body: json!({
                        "code": "runbook_mode_changed",
                        "message": "Request conflicts with current identity or retained state"
                    }),
                };
            }
            StubResponse {
                status: 404,
                body: json!({"code": "not_found", "message": "unexpected"}),
            }
        });
        let (profile_path, identity) = write_cli_profile(root.path(), addr.port());
        let output = checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args([
                    "ci",
                    "--profile",
                    &profile_path.to_string_lossy(),
                    "--identity",
                    &identity,
                    "retry",
                    "--run",
                    "run-1",
                    "--request-id",
                    "cli-retry-drift",
                ])
                .output(),
        );
        stop.store(true, Ordering::Release);
        let _ = handle.join();

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("HTTP 409"), "stderr: {stderr}");
        assert!(stderr.contains("runbook_mode_changed"), "stderr: {stderr}");
    }
}

#[test]
fn ci_fanout_and_controls_wire_http_routes_and_watch_exit_codes() {
    let root = checked(tempfile::tempdir());
    let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, body| {
        if method == "GET" && path == "/api/runbooks" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.web-runbooks-view.v1",
                    "configured": true,
                    "principal": "local:operator",
                    "runbooks": [{
                        "id": "security-audit",
                        "params": [
                            { "name": "profile", "kind": "enum", "required": true, "bindings": [] }
                        ]
                    }]
                }),
            };
        }
        if method == "POST" && path == "/api/runbooks/security-audit/fanout" {
            let req_id = body
                .and_then(|v| v.get("request_id"))
                .and_then(Value::as_str)
                .unwrap_or("req");
            return StubResponse {
                status: 202,
                body: json!({
                    "schema": "kiln.runbook-fanout-result.v1",
                    "request_id": req_id,
                    "replayed": false,
                    "fanout_id": "rb-fanout-1",
                    "fanout": {
                        "id": "rb-fanout-1",
                        "runbook_id": "security-audit",
                        "admission": "open",
                        "phase": "active",
                        "max_concurrency": 2,
                        "budget_drawn": 1,
                        "budget_cap": 3,
                        "counts": { "total": 2, "pending": 1, "active": 1, "success": 0, "failure": 0, "unknown": 0, "skipped": 0 }
                    }
                }),
            };
        }
        if method == "GET" && path == "/api/runbooks/fanouts?runbook_id=security-audit&limit=10" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-fanouts-view.v1",
                    "fanouts": [],
                    "next_cursor": null,
                    "total": 0,
                    "bounded": true
                }),
            };
        }
        if method == "GET" && path == "/api/runbooks/fanouts/rb-fanout-1" {
            return StubResponse {
                status: 200,
                body: json!({
                    "id": "rb-fanout-1",
                    "runbook_id": "security-audit",
                    "admission": "open",
                    "phase": "completed",
                    "max_concurrency": 2,
                    "budget_drawn": 2,
                    "budget_cap": 3,
                    "counts": { "total": 2, "pending": 0, "active": 0, "success": 1, "failure": 0, "unknown": 1, "skipped": 0 }
                }),
            };
        }
        if method == "GET" && path.starts_with("/api/runbooks/dispatches") {
            if path.contains("/rollup") {
                let counts = json!({
                    "total": 1, "pending": 0, "active": 0, "success": 0, "failure": 0, "unknown": 1, "skipped": 0,
                });
                let targets = json!([{
                    "target": "onixresearch/kiln@refs/heads/main",
                    "source_revision": "d".repeat(40),
                    "status": "unknown",
                    "patchgate_receipt": { "state": "missing" },
                    "verification_receipt": { "state": "missing", "grants_authority": false },
                    "pull_requests": [],
                }]);
                return StubResponse {
                    status: 200,
                    body: json!({
                        "schema": "kiln.runbook-rollup.v1",
                        "subject_kind": "dispatch",
                        "subject_id": "web-run-1",
                        "runbook_id": "security-audit",
                        "phase": "unknown",
                        "counts": counts,
                        "notification": { "state": "not_configured" },
                        "targets": targets,
                        "next_cursor": null,
                        "total": 1,
                        "bounded": true,
                        "grants_authority": false,
                    }),
                };
            }
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-dispatches-view.v1",
                    "dispatches": [],
                    "next_cursor": null,
                    "total": 0,
                    "bounded": true,
                    "grants_authority": false,
                    "non_claims": []
                }),
            };
        }
        if method == "GET" && path == "/api/runbooks/fanouts/rb-fanout-1/rollup" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-rollup.v1",
                    "subject_kind": "fanout",
                    "subject_id": "rb-fanout-1",
                    "runbook_id": "security-audit",
                    "mode": "read_only",
                    "run_id": null,
                    "fanout_id": "rb-fanout-1",
                    "principal": "local:operator",
                    "phase": "completed",
                    "overall_outcome": null,
                    "has_unknown": false,
                    "admission": "open",
                    "stop_reason": null,
                    "lattice_bundle_id": null,
                    "lattice_bundle_identity": "a".repeat(64),
                    "lattice_workflow_name": "security-audit",
                    "lattice_workflow_revision": "b".repeat(64),
                    "params_identity": "c".repeat(64),
                    "counts": { "total": 1, "pending": 0, "active": 0, "success": 1, "failure": 0, "unknown": 0, "skipped": 0 },
                    "notification": { "state": "delivered" },
                    "targets": [{
                        "target": "onixresearch/kiln@refs/heads/main",
                        "source_revision": "d".repeat(40),
                        "status": "success",
                        "patchgate_receipt": { "state": "present" },
                        "verification_receipt": { "state": "present" },
                        "pull_requests": [{ "url": "https://github.com/onixresearch/kiln/pull/100", "state": "open" }]
                    }],
                    "next_cursor": null, "total": 1, "bounded": true, "grants_authority": false, "non_claims": []
                }),
            };
        }
        if method == "POST" && path == "/api/runbooks/security-audit/preview" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-preview.v1",
                    "dry_run": true,
                    "grants_authority": false,
                    "runbook_id": "security-audit",
                    "mode": "read_only",
                    "approval_gate": "none",
                    "can_dispatch": true,
                    "dispatch_refusal": null,
                    "bundle_pin_verified": true,
                    "bundle_id": "security-audit",
                    "bundle_identity": "a".repeat(64),
                    "workflow_name": "security-audit",
                    "workflow_revision": "b".repeat(64),
                    "nodes": [],
                    "edges": [],
                    "params_identity": "c".repeat(64),
                    "params": {},
                    "env_bindings": {},
                    "targets": [{
                        "project_id": "kiln-audit",
                        "repository": "onixresearch/kiln",
                        "target": "onixresearch/kiln@refs/heads/main",
                        "resolved_reference": "refs/heads/main",
                        "source_revision": "d".repeat(40),
                        "write_scopes": [],
                        "claim_conflict": null
                    }],
                    "budget": {
                        "max_concurrency": 1,
                        "budget_cap": 1,
                        "unit_cost": 1,
                        "estimated_units": 1,
                        "within_budget": true,
                        "max_runs_available": 64
                    },
                    "non_claims": ["Dry-run preview performs zero state mutation."]
                }),
            };
        }
        if method == "POST" && path == "/api/runbooks/fanouts/rb-fanout-1/pause" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-fanout-control-result.v1",
                    "request_id": "pause-cli",
                    "replayed": false,
                    "action": "pause",
                    "fanout_id": "rb-fanout-1",
                    "fanout": { "id": "rb-fanout-1", "admission": "paused", "phase": "paused" }
                }),
            };
        }
        StubResponse {
            status: 404,
            body: json!({"code": "not_found", "message": "unexpected"}),
        }
    });
    let (profile_path, identity) = write_cli_profile(root.path(), addr.port());

    let fo_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout",
                "--runbook",
                "security-audit",
                "--request-id",
                "cli-fo-1",
                "--target-glob",
                "onixresearch/*@refs/heads/main",
                "--param",
                "profile=123",
                "--max-concurrency",
                "2",
                "--budget-cap",
                "3",
                "--claim-policy",
                "skip",
                "--output",
                "text",
            ])
            .output(),
    );
    assert!(
        fo_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&fo_out.stderr)
    );
    let stdout_text = String::from_utf8_lossy(&fo_out.stdout);
    assert!(stdout_text.contains("fanout_id: rb-fanout-1"));

    let list_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout-list",
                "--runbook",
                "security-audit",
                "--limit",
                "10",
            ])
            .output(),
    );
    assert!(list_out.status.success());

    // fanout-watch exits 3 when unknown > 0 && failure == 0.
    let watch_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout-watch",
                "--fanout",
                "rb-fanout-1",
            ])
            .output(),
    );
    assert_eq!(watch_out.status.code(), Some(3));

    let pause_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout-pause",
                "--fanout",
                "rb-fanout-1",
                "--request-id",
                "pause-cli",
            ])
            .output(),
    );
    assert!(pause_out.status.success());

    let disp_list_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "dispatch-list",
                "--runbook",
                "security-audit",
                "--limit",
                "10",
            ])
            .output(),
    );
    assert!(disp_list_out.status.success());

    let disp_rollup_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "dispatch-rollup",
                "--run",
                "web-run-1",
                "--output",
                "text",
            ])
            .output(),
    );
    assert!(disp_rollup_out.status.success());
    let disp_rollup_text = String::from_utf8_lossy(&disp_rollup_out.stdout);
    assert!(disp_rollup_text.contains("subject: dispatch web-run-1"));
    assert!(disp_rollup_text.contains("patchgate=missing"));
    assert!(disp_rollup_text.contains("verification=missing"));

    let fo_rollup_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout-rollup",
                "--fanout",
                "rb-fanout-1",
                "--output",
                "text",
            ])
            .output(),
    );
    assert!(fo_rollup_out.status.success());
    let fo_rollup_text = String::from_utf8_lossy(&fo_rollup_out.stdout);
    assert!(fo_rollup_text.contains("subject: fanout rb-fanout-1"));
    assert!(fo_rollup_text.contains("https://github.com/onixresearch/kiln/pull/100 (open)"));

    let prev_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "preview",
                "--runbook",
                "security-audit",
                "--target",
                "onixresearch/kiln@refs/heads/main",
                "--output",
                "text",
            ])
            .output(),
    );
    assert!(
        prev_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&prev_out.stderr)
    );
    let prev_text = String::from_utf8_lossy(&prev_out.stdout);
    assert!(prev_text.contains("dry_run: true"));
    assert!(prev_text.contains("grants_authority: false"));
    assert!(prev_text.contains("bundle_pin_verified: true"));

    let dry_disp_out = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "dispatch",
                "--runbook",
                "security-audit",
                "--target",
                "onixresearch/kiln@refs/heads/main",
                "--dry-run",
            ])
            .output(),
    );
    assert!(
        dry_disp_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&dry_disp_out.stderr)
    );
    let dry_disp_json: Value = checked(serde_json::from_slice(&dry_disp_out.stdout));
    assert_eq!(dry_disp_json.get("dry_run"), Some(&json!(true)));

    stop.store(true, Ordering::Release);
    let _ = handle.join();

    let requests = checked(captured.lock()).clone();
    assert!(requests.iter().any(|(m, p, b)| {
        m == "POST"
            && p == "/api/runbooks/security-audit/fanout"
            && b.as_ref().and_then(|v| v.pointer("/params/profile")) == Some(&json!("123"))
    }));
    assert_eq!(
        requests
            .iter()
            .filter(|(m, p, _)| m == "POST" && p == "/api/runbooks/security-audit/preview")
            .count(),
        2
    );
}

#[test]
fn ci_fanout_dry_run_single_target_uses_fanout_targets_body() {
    let root = checked(tempfile::tempdir());
    let (addr, captured, stop, handle) = spawn_tls_stub(|method, path, _| {
        if method == "GET" && path == "/api/runbooks" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": RUNBOOKS_VIEW_SCHEMA,
                    "configured": true,
                    "principal": "local:operator",
                    "runbooks": [{ "id": "security-audit", "params": [] }]
                }),
            };
        }
        if method == "POST" && path == "/api/runbooks/security-audit/preview" {
            return StubResponse {
                status: 200,
                body: json!({
                    "schema": "kiln.runbook-preview.v1",
                    "dry_run": true,
                    "grants_authority": false,
                    "runbook_id": "security-audit",
                    "mode": "read_only",
                    "approval_gate": "none",
                    "can_dispatch": true,
                    "dispatch_refusal": null,
                    "bundle_pin_verified": true,
                    "bundle_id": "security-audit",
                    "bundle_identity": "a".repeat(64),
                    "workflow_name": "security-audit",
                    "workflow_revision": "b".repeat(64),
                    "nodes": [],
                    "edges": [],
                    "params_identity": "c".repeat(64),
                    "params": {},
                    "env_bindings": {},
                    "targets": [{
                        "project_id": "kiln-audit",
                        "repository": "onixresearch/kiln",
                        "target": "onixresearch/kiln@refs/heads/main",
                        "resolved_reference": "refs/heads/main",
                        "source_revision": "d".repeat(40),
                        "write_scopes": [],
                        "claim_conflict": null
                    }],
                    "budget": {
                        "max_concurrency": 2,
                        "budget_cap": 16,
                        "unit_cost": 1,
                        "estimated_units": 1,
                        "within_budget": true,
                        "max_runs_available": 64
                    },
                    "non_claims": ["Dry-run preview performs zero state mutation."]
                }),
            };
        }
        StubResponse {
            status: 404,
            body: json!({"code": "not_found", "message": "unexpected"}),
        }
    });
    let (profile_path, identity) = write_cli_profile(root.path(), addr.port());

    let out_repo_ref = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout",
                "--runbook",
                "security-audit",
                "--target",
                "onixresearch/kiln@refs/heads/main",
                "--dry-run",
            ])
            .output(),
    );
    assert!(
        out_repo_ref.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_repo_ref.stderr)
    );

    let out_project = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "fanout",
                "--runbook",
                "security-audit",
                "--target",
                "kiln-audit",
                "--dry-run",
            ])
            .output(),
    );
    assert!(
        out_project.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_project.stderr)
    );

    let out_dispatch = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "ci",
                "--profile",
                &profile_path.to_string_lossy(),
                "--identity",
                &identity,
                "dispatch",
                "--runbook",
                "security-audit",
                "--target",
                "onixresearch/kiln@refs/heads/main",
                "--dry-run",
            ])
            .output(),
    );
    assert!(
        out_dispatch.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_dispatch.stderr)
    );

    stop.store(true, Ordering::Release);
    let _ = handle.join();

    let posts: Vec<Value> = checked(captured.lock())
        .iter()
        .filter(|(m, p, _)| m == "POST" && p == "/api/runbooks/security-audit/preview")
        .filter_map(|(_, _, body)| body.clone())
        .collect();
    let [p0, p1, p2] = posts.as_slice() else {
        unreachable!("expected 3 preview posts, got {}", posts.len());
    };
    assert_eq!(p0.get("target"), None);
    assert_eq!(
        p0.get("targets"),
        Some(&json!(["onixresearch/kiln@refs/heads/main"]))
    );
    assert_eq!(p1.get("target"), None);
    assert_eq!(p1.get("targets"), Some(&json!(["kiln-audit"])));
    assert_eq!(
        p2.get("target"),
        Some(&json!("onixresearch/kiln@refs/heads/main"))
    );
    assert_eq!(p2.get("targets"), None);
}
