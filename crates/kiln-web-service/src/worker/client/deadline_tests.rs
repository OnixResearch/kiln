use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_adapter_git::{resolve_and_acquire, GitSourceProfile, LocalGitAdapter};
use kiln_core::source::SourceResolution;
use kiln_core::types::{ExternalRef, ProviderProfileId};
use serde_json::{json, Value};

use super::{LogBounds, Node, RunBinding, ServiceClient, WorkerProfile};

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("deadline fixture: {error:?}"))
}

fn fixture() -> (
    tempfile::TempDir,
    WorkerProfile,
    SourceResolution,
    RunBinding,
) {
    let root = checked(tempfile::tempdir());
    let mut profile: WorkerProfile = checked(serde_json::from_slice(include_bytes!(
        "../../../../../fixtures/web-worker/profile.json"
    )));
    profile.repository_path = root.path().join("repository");
    profile.work_root = root.path().join("work");
    profile.token_file = root.path().join("worker.token");
    profile.tls_ca_path = root.path().join("ca.pem");
    checked(std::fs::write(&profile.token_file, "ab".repeat(32)));
    checked(std::fs::set_permissions(
        &profile.token_file,
        std::fs::Permissions::from_mode(0o600),
    ));
    checked(std::fs::write(
        &profile.tls_ca_path,
        include_bytes!("../../../../../fixtures/http-observation/tls/ca-cert.pem"),
    ));
    let repository = checked(git2::Repository::init(&profile.repository_path));
    let tree_id = checked(checked(repository.index()).write_tree());
    let tree = checked(repository.find_tree(tree_id));
    let signature = checked(git2::Signature::now(
        "Deadline fixture",
        "deadline@example.invalid",
    ));
    let commit =
        checked(repository.commit(Some("HEAD"), &signature, &signature, "source", &tree, &[]));
    let provider = checked(ProviderProfileId::new("deadline-git"));
    let adapter = checked(LocalGitAdapter::new(
        profile.repository_path.clone(),
        commit.to_string(),
        GitSourceProfile::pinned_local(provider.clone(), profile.snapshot_limits.clone()),
    ));
    let source = checked(resolve_and_acquire(&adapter, provider));
    profile.source_repository = source.repository.native_id.as_str().into();
    let binding = RunBinding {
        id: "deadline-run".into(),
        project_id: profile.project_id.clone(),
        repository: checked(serde_json::to_value(&source.repository)),
        source_revision: source.exact_revision.native_id.as_str().into(),
        canonical_tree: source.canonical_tree.as_str().into(),
        attempt_id: "deadline-attempt".into(),
        execution: checked(serde_json::to_value(checked(ExternalRef::new(
            "lattice",
            "execution",
            "deadline-execution",
        )))),
        outcome: "Pending".into(),
        nodes: vec![Node { id: "build".into() }],
        log_bounds: LogBounds {
            maximum_chunk_bytes: 1024,
            maximum_total_bytes: 4096,
            maximum_chunks: 4,
        },
        project_root: None,
        untrusted: None,
    };
    checked(super::admit_binding(&profile, &source, &binding));
    (root, profile, source, binding)
}

// Send headers immediately but delay the body: the lookup budget must cover
// receiving and admitting the entire binding, not only establishing TLS.
fn receiver(
    replies: Vec<(Duration, Value)>,
) -> (String, Arc<AtomicBool>, std::thread::JoinHandle<usize>) {
    let (origin, stop, thread, _) = recording_receiver(replies);
    (origin, stop, thread)
}

type RecordedRequests = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

fn recording_receiver(
    replies: Vec<(Duration, Value)>,
) -> (
    String,
    Arc<AtomicBool>,
    std::thread::JoinHandle<usize>,
    RecordedRequests,
) {
    let recorded = RecordedRequests::default();
    let captured = recorded.clone();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certificates = checked(
        rustls_pemfile::certs(&mut std::io::Cursor::new(include_bytes!(
            "../../../../../fixtures/http-observation/tls/cert.pem"
        )))
        .collect::<Result<Vec<_>, _>>(),
    );
    let key = checked(rustls_pemfile::private_key(&mut std::io::Cursor::new(
        include_bytes!("../../../../../fixtures/http-observation/tls/key.pem"),
    )))
    .unwrap_or_else(|| unreachable!("TLS key"));
    let tls = Arc::new(checked(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certificates, key),
    ));
    let listener = checked(std::net::TcpListener::bind("127.0.0.1:0"));
    checked(listener.set_nonblocking(true));
    let origin = format!(
        "https://localhost:{}",
        checked(listener.local_addr()).port()
    );
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut requests = 0;
        for (delay, body) in replies {
            let socket = loop {
                if stopped.load(Ordering::SeqCst) {
                    return requests;
                }
                assert!(Instant::now() < deadline, "fixture receiver timed out");
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1))
                    }
                    Err(error) => unreachable!("TLS accept: {error:?}"),
                }
            };
            checked(socket.set_read_timeout(Some(Duration::from_secs(3))));
            checked(socket.set_write_timeout(Some(Duration::from_secs(3))));
            let mut stream = rustls::StreamOwned::new(
                checked(rustls::ServerConnection::new(tls.clone())),
                socket,
            );
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            let mut request_line = String::new();
            checked(reader.read_line(&mut request_line));
            let mut length = 0;
            loop {
                line.clear();
                assert!(
                    checked(reader.read_line(&mut line)) > 0,
                    "request ended early"
                );
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = checked(value.trim().parse::<usize>());
                }
            }
            let mut request_body = vec![0; length];
            checked(std::io::Read::read_exact(&mut reader, &mut request_body));
            checked(captured.lock()).push((request_line, request_body));
            drop(reader);
            requests += 1;
            let bytes = checked(serde_json::to_vec(&body));
            checked(write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            ));
            checked(stream.flush());
            std::thread::sleep(delay);
            // A deadline correctly closes this socket before the late body.
            let _ = stream.write_all(&bytes).and_then(|()| stream.flush());
        }
        requests
    });
    (origin, stop, thread, recorded)
}

// r[verify kiln.web.remote_workers.source]
// r[verify kiln.web.remote_workers.source.scenario.denied]
#[test]
fn independent_clone_rejects_wrong_callback_source_before_commands_or_uploads() {
    for (field, replacement) in [
        ("native_id", json!("f".repeat(64))),
        ("provider_namespace", json!("pijul")),
        ("native_kind", json!("commit")),
        ("contract_version", json!(2)),
        ("extra_repository_field", json!(true)),
        ("repository", Value::Null),
        ("source_revision", json!("f".repeat(40))),
        ("canonical_tree", json!("f".repeat(64))),
    ] {
        let (root, mut profile, source, binding) = fixture();
        let clone = root.path().join("worker-clone");
        checked(git2::Repository::clone(
            profile
                .repository_path
                .to_str()
                .unwrap_or_else(|| unreachable!()),
            &clone,
        ));
        profile.repository_path = clone;
        checked(std::fs::create_dir(&profile.work_root));
        checked(std::fs::set_permissions(
            &profile.work_root,
            std::fs::Permissions::from_mode(0o700),
        ));
        let marker = root.path().join("command-executed");
        let step = profile.steps.first_mut().unwrap_or_else(|| unreachable!());
        step.program = "/bin/sh".into();
        step.argv = vec![
            "-c".into(),
            ": > \"$1\"".into(),
            "denial-probe".into(),
            marker.to_str().unwrap_or_else(|| unreachable!()).into(),
        ];
        let context = crate::worker::SourceContext {
            provider: "git".into(),
            repository: profile.source_repository.clone(),
            revision: source.exact_revision.native_id.as_str().into(),
        };
        let mut wrong = checked(serde_json::to_value(binding));
        if [
            "native_id",
            "provider_namespace",
            "native_kind",
            "contract_version",
            "extra_repository_field",
        ]
        .contains(&field)
        {
            wrong
                .get_mut("repository")
                .and_then(Value::as_object_mut)
                .unwrap_or_else(|| unreachable!())
                .insert(field.into(), replacement);
        } else {
            *wrong.get_mut(field).unwrap_or_else(|| unreachable!()) = replacement;
        }
        let (origin, stop, server) = receiver(vec![(Duration::ZERO, json!({"runs": [wrong]}))]);
        profile.service_origin = origin;
        let result = crate::worker::run_job(&profile, &context, &AtomicBool::new(false));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(
            checked(server.join()),
            1,
            "unexpected callback/upload request for {field}"
        );
        assert!(result.is_err(), "wrong callback {field} was admitted");
        assert!(!marker.exists(), "command executed for rejected {field}");
        assert_eq!(checked(std::fs::read_dir(&profile.work_root)).count(), 0);
    }
}

#[test]
fn binding_lookup_budget_bounds_slow_valid_https_response() {
    let (_root, mut profile, source, binding) = fixture();
    // Keep generous wall-clock slack so a heavily loaded test runner cannot
    // turn a deterministic budget failure into a scheduling failure.
    profile.lookup_timeout_ms = 1000;
    profile.http_timeout_ms = 5000;
    let (origin, stop, server) = receiver(vec![(
        Duration::from_millis(3500),
        json!({"runs": [binding]}),
    )]);
    profile.service_origin = origin;
    checked(profile.admit());
    let client = checked(ServiceClient::new(&profile));
    let started = Instant::now();
    let result = client.await_binding(&profile, &source, &AtomicBool::new(false));
    let elapsed = started.elapsed();
    stop.store(true, Ordering::SeqCst);
    assert_eq!(checked(server.join()), 1);
    assert!(
        result.is_err(),
        "late valid binding was accepted after {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(3000),
        "lookup exceeded its 1000ms budget: {elapsed:?}"
    );
}

#[test]
fn binding_lookup_does_not_start_request_after_poll_sleep_exhausts_budget() {
    let (_root, mut profile, source, binding) = fixture();
    profile.lookup_timeout_ms = 300;
    profile.poll_interval_ms = 300;
    profile.http_timeout_ms = 3000;
    let (origin, stop, server) = receiver(vec![
        (Duration::ZERO, json!({"runs": []})),
        (Duration::ZERO, json!({"runs": [binding]})),
    ]);
    profile.service_origin = origin;
    checked(profile.admit());
    let client = checked(ServiceClient::new(&profile));
    let result = client.await_binding(&profile, &source, &AtomicBool::new(false));
    stop.store(true, Ordering::SeqCst);
    assert_eq!(
        checked(server.join()),
        1,
        "expired poll issued another request"
    );
    assert!(result.is_err(), "expired poll accepted a binding");
}

#[test]
fn binding_lookup_admits_exact_source_within_budget_after_pending_dispatch() {
    let (_root, mut profile, source, binding) = fixture();
    profile.lookup_timeout_ms = 2000;
    profile.http_timeout_ms = 3000;
    let (origin, stop, server) = receiver(vec![
        (Duration::ZERO, json!({"runs": []})),
        (
            Duration::from_millis(20),
            json!({"runs": [binding.clone()]}),
        ),
    ]);
    profile.service_origin = origin;
    checked(profile.admit());
    let client = checked(ServiceClient::new(&profile));
    let result = client.await_binding(&profile, &source, &AtomicBool::new(false));
    stop.store(true, Ordering::SeqCst);
    assert_eq!(checked(server.join()), 2);
    let admitted = checked(result);
    checked(super::exact_binding(&binding, &admitted));
    let mut stale = admitted;
    stale.attempt_id = "different-attempt".into();
    assert!(super::exact_binding(&binding, &stale).is_err());
}

#[test]
fn binding_lookup_keeps_shorter_http_timeout_and_never_retries_failure() {
    let (_root, mut profile, source, binding) = fixture();
    profile.lookup_timeout_ms = 2000;
    profile.http_timeout_ms = 300;
    let (origin, stop, server) = receiver(vec![
        (Duration::from_millis(1500), json!({"runs": [binding]})),
        (Duration::ZERO, json!({"runs": []})),
    ]);
    profile.service_origin = origin;
    checked(profile.admit());
    let client = checked(ServiceClient::new(&profile));
    let started = Instant::now();
    let result = client.await_binding(&profile, &source, &AtomicBool::new(false));
    let elapsed = started.elapsed();
    stop.store(true, Ordering::SeqCst);
    assert_eq!(checked(server.join()), 1, "failed HTTP request was retried");
    assert!(result.is_err(), "binding ignored the shorter HTTP timeout");
    assert!(
        elapsed < Duration::from_millis(900),
        "lookup overrode its 300ms HTTP timeout: {elapsed:?}"
    );
}

fn execution_fixture(source: &SourceResolution, run: &RunBinding) -> super::ExecutionBinding {
    let plan = checked(serde_json::from_value(json!({
        "effect_id":"effect", "attempt_id":run.attempt_id, "candidate_id":"candidate",
        "pipeline_revision_id":"pipeline", "workflow_policy_id":"policy",
        "provider_profile":"lattice", "idempotency_key":"a".repeat(64), "platform":"linux",
        "source":{"provider":"git", "repository":source.repository,
            "revision":checked(ExternalRef::new("git", "revision", source.exact_revision.native_id.as_str()))},
        "nodes":[{"id":"build","kind":"build","effect_class":"Build","adapter_kind":"lattice",
            "operation_identity":"b".repeat(64)}], "edges":[],
        "controls":{"cancel":true,"pause":false,"resume":false},
        "bounds":{"max_nodes":1,"max_edges":1,"max_log_references":1,
            "max_log_read_bytes":4096,"max_total_log_bytes":4096}
    })));
    super::ExecutionBinding {
        plan,
        binding: kiln_provider_api::WorkflowDispatchBinding {
            run_id: run.id.clone(),
            project_id: run.project_id.clone(),
            source: source.clone(),
            untrusted: None,
        },
        execution: Some(checked(serde_json::from_value(run.execution.clone()))),
        phase: "active".into(),
        outcome: kiln_core::domain::Outcome::Pending,
        cancel_requested: false,
        log_bounds: run.log_bounds.clone(),
    }
}

#[test]
fn admit_refuses_a_cancellation_requested_binding() {
    let (_root, _profile, source, run) = fixture();
    let mut bound = execution_fixture(&source, &run);
    super::admit_execution_binding(&bound, "build").unwrap_or_else(|e| unreachable!("{e}"));
    bound.cancel_requested = true;
    assert!(super::admit_execution_binding(&bound, "build").is_err());
}

#[test]
fn execution_lookup_rejects_ambiguous_stale_and_wrong_source_authority() {
    for case in [
        "valid",
        "ambiguous",
        "source",
        "repository",
        "node",
        "execution",
        "terminal",
        "approval-source",
    ] {
        let (_root, mut profile, source, run) = fixture();
        let context = crate::worker::SourceContext {
            provider: "git".into(),
            repository: profile.source_repository.clone(),
            revision: source.exact_revision.native_id.as_str().into(),
        };
        let mut bound = execution_fixture(&source, &run);
        match case {
            "source" => {
                bound
                    .plan
                    .source
                    .as_mut()
                    .unwrap_or_else(|| unreachable!())
                    .revision = checked(ExternalRef::new("git", "revision", &"f".repeat(40)))
            }
            "repository" => {
                bound
                    .plan
                    .source
                    .as_mut()
                    .unwrap_or_else(|| unreachable!())
                    .repository = checked(ExternalRef::new("git", "repository", &"f".repeat(64)))
            }
            "execution" => {
                bound.execution = Some(checked(ExternalRef::new("lattice", "execution", "other")))
            }
            "terminal" => bound.outcome = kiln_core::domain::Outcome::Success,
            "approval-source" => {
                bound.binding.source.exact_revision =
                    checked(ExternalRef::new("git", "commit", &"f".repeat(40)))
            }
            _ => {}
        }
        let runs = if case == "ambiguous" {
            json!([run, run])
        } else {
            json!([run])
        };
        let (origin, stop, server) = receiver(vec![
            (Duration::ZERO, json!({"runs":runs})),
            (Duration::ZERO, checked(serde_json::to_value(bound))),
        ]);
        profile.service_origin = origin;
        let client = checked(ServiceClient::new(&profile));
        let result = client.lookup_execution_binding(
            &profile.project_id,
            &context,
            if case == "node" { "other" } else { "build" },
            Duration::from_secs(2),
        );
        stop.store(true, Ordering::SeqCst);
        assert_eq!(
            checked(server.join()),
            if case == "ambiguous" { 1 } else { 2 }
        );
        if case == "valid" {
            assert_eq!(
                checked(result)
                    .unwrap_or_else(|| unreachable!())
                    .binding
                    .source,
                source
            );
        } else {
            assert!(result.is_err(), "admitted {case}");
        }
    }
}

#[test]
fn execution_lookup_deadline_covers_both_response_bodies_and_keeps_http_timeout() {
    for (first, second, budget, http_ms, succeeds, requests) in [
        (0, 0, 1000, 2000, true, 2),
        (500, 0, 100, 2000, false, 1),
        (0, 500, 100, 2000, false, 2),
        (80, 500, 150, 2000, false, 2),
        (500, 0, 2000, 100, false, 1),
        (0, 0, 0, 2000, false, 0),
    ] {
        let (_root, mut profile, source, run) = fixture();
        let context = crate::worker::SourceContext {
            provider: "git".into(),
            repository: profile.source_repository.clone(),
            revision: source.exact_revision.native_id.as_str().into(),
        };
        let bound = execution_fixture(&source, &run);
        let (origin, stop, server) = receiver(vec![
            (Duration::from_millis(first), json!({"runs":[run]})),
            (
                Duration::from_millis(second),
                checked(serde_json::to_value(bound)),
            ),
        ]);
        profile.service_origin = origin;
        profile.http_timeout_ms = http_ms;
        let client = checked(ServiceClient::new(&profile));
        let started = Instant::now();
        let result = client.lookup_execution_binding(
            &profile.project_id,
            &context,
            "build",
            Duration::from_millis(budget),
        );
        let elapsed = started.elapsed();
        stop.store(true, Ordering::SeqCst);
        assert_eq!(checked(server.join()), requests);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert!(checked(result).is_some());
        } else {
            assert!(
                elapsed < Duration::from_millis(400),
                "lookup exceeded budget: {elapsed:?}"
            );
        }
    }
}

#[test]
fn execution_log_https_consumer_fits_minimum_http_envelope_across_chunks() {
    use base64::Engine as _;
    let (_root, mut profile, source, run) = fixture();
    let mut bound = execution_fixture(&source, &run);
    bound.log_bounds.maximum_chunk_bytes = checked(bound.maximum_upload_chunk_bytes(4097));
    bound.log_bounds.maximum_chunks = 2;
    bound.log_bounds.maximum_total_bytes = 8192;
    let maximum = bound.log_bounds.maximum_chunk_bytes;
    let bytes = vec![b'x'; maximum + 1];
    let (origin, stop, server, requests) = recording_receiver(vec![
        (Duration::ZERO, checked(serde_json::to_value(&bound))),
        (Duration::ZERO, json!({"next_cursor":maximum})),
        (Duration::ZERO, json!({"next_cursor":bytes.len()})),
    ]);
    profile.service_origin = origin;
    let client = checked(ServiceClient::new(&profile));
    checked(client.upload_execution_logs(&bound, "build", &bytes, b"", true, false));
    // Reference exhaustion must reject before another binding lookup or upload.
    assert!(client
        .upload_execution_logs(
            &bound,
            "build",
            &vec![b'x'; maximum * 2 + 1],
            b"",
            false,
            false
        )
        .is_err());
    stop.store(true, Ordering::SeqCst);
    assert_eq!(checked(server.join()), 3);
    let requests = checked(requests.lock());
    let mut retained = Vec::new();
    for (index, (_, body)) in requests.iter().skip(1).enumerate() {
        assert!(
            body.len() <= 4097,
            "HTTPS upload exceeded HTTP request bound"
        );
        let chunk: Value = checked(serde_json::from_slice(body));
        assert_eq!(
            chunk.get("cursor"),
            Some(&serde_json::json!(retained.len()))
        );
        assert_eq!(chunk.get("stream").and_then(Value::as_str), Some("stdout"));
        assert_eq!(
            chunk.get("truncated").and_then(Value::as_bool),
            Some(index == 1)
        );
        retained.extend(checked(
            base64::engine::general_purpose::STANDARD.decode(checked(
                chunk
                    .get("body_base64")
                    .and_then(Value::as_str)
                    .ok_or("base64 body"),
            )),
        ));
    }
    assert_eq!(retained, bytes);
}

#[test]
fn execution_log_single_reference_preserves_nonempty_and_truncated_streams() {
    for (stdout, stderr, out_truncated, err_truncated, expected_stream) in [
        (
            b"out".as_slice(),
            b"".as_slice(),
            false,
            false,
            Some("stdout"),
        ),
        (
            b"".as_slice(),
            b"err".as_slice(),
            false,
            false,
            Some("stderr"),
        ),
        (b"".as_slice(), b"".as_slice(), false, false, None),
        (b"".as_slice(), b"".as_slice(), true, false, Some("stdout")),
    ] {
        let (_root, mut profile, source, run) = fixture();
        let mut bound = execution_fixture(&source, &run);
        bound.log_bounds.maximum_chunks = 1;
        let bytes = if expected_stream == Some("stderr") {
            stderr
        } else {
            stdout
        };
        let mut replies = vec![(Duration::ZERO, checked(serde_json::to_value(&bound)))];
        if expected_stream.is_some() {
            replies.push((Duration::ZERO, json!({"next_cursor":bytes.len()})));
        }
        let (origin, stop, server, requests) = recording_receiver(replies);
        profile.service_origin = origin;
        checked(checked(ServiceClient::new(&profile)).upload_execution_logs(
            &bound,
            "build",
            stdout,
            stderr,
            out_truncated,
            err_truncated,
        ));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(
            checked(server.join()),
            1 + usize::from(expected_stream.is_some())
        );
        let requests = checked(requests.lock());
        if let Some(stream) = expected_stream {
            let (_, body) = checked(requests.get(1).ok_or("upload request"));
            let chunk: Value = checked(serde_json::from_slice(body));
            assert_eq!(chunk.get("stream").and_then(Value::as_str), Some(stream));
            assert_eq!(
                chunk.get("body_base64"),
                Some(&serde_json::json!(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes
                )))
            );
            assert_eq!(
                chunk.get("truncated").and_then(Value::as_bool),
                Some(out_truncated || err_truncated)
            );
        }
        assert!(super::execution_log_chunks(&bound.log_bounds, b"a", b"b", false, false).is_err());
        assert!(super::execution_log_chunks(&bound.log_bounds, b"", b"", true, true).is_err());
    }
}

#[test]
fn execution_log_capture_is_chunked_hashed_and_marks_only_final_chunk_truncated() {
    use base64::Engine as _;
    let (_root, mut profile, source, run) = fixture();
    let mut bound = execution_fixture(&source, &run);
    bound.log_bounds = LogBounds {
        maximum_chunk_bytes: 3,
        maximum_total_bytes: 8,
        maximum_chunks: 3,
    };
    let (origin, stop, server, requests) = recording_receiver(vec![
        (Duration::ZERO, checked(serde_json::to_value(&bound))),
        (Duration::ZERO, json!({"next_cursor":3})),
        (Duration::ZERO, json!({"next_cursor":5})),
        (Duration::ZERO, json!({"next_cursor":2})),
    ]);
    profile.service_origin = origin;
    let client = checked(ServiceClient::new(&profile));
    checked(client.upload_execution_logs(&bound, "build", b"hello", b"no", true, false));
    stop.store(true, Ordering::SeqCst);
    assert_eq!(checked(server.join()), 4);
    let requests = checked(requests.lock());
    let chunks: Vec<Value> = requests
        .iter()
        .skip(1)
        .map(|(line, body)| {
            assert!(line.starts_with("POST /api/runs/deadline-run/logs "));
            checked(serde_json::from_slice(body))
        })
        .collect();
    for (chunk, (stream, cursor, bytes, truncated)) in chunks.iter().zip([
        ("stdout", 0, b"hel".as_slice(), false),
        ("stdout", 3, b"lo".as_slice(), true),
        ("stderr", 0, b"no".as_slice(), false),
    ]) {
        assert_eq!(chunk.get("stream").and_then(Value::as_str), Some(stream));
        assert_eq!(chunk.get("cursor").and_then(Value::as_u64), Some(cursor));
        assert_eq!(
            chunk.get("truncated").and_then(Value::as_bool),
            Some(truncated)
        );
        assert_eq!(
            checked(
                base64::engine::general_purpose::STANDARD.decode(
                    chunk
                        .get("body_base64")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| unreachable!())
                )
            ),
            bytes
        );
        assert_eq!(
            chunk.get("blake3").and_then(Value::as_str),
            Some(blake3::hash(bytes).to_hex().as_str())
        );
    }
}

#[test]
fn execution_log_limits_reserve_both_streams_and_reject_before_upload() {
    let bounds = LogBounds {
        maximum_chunk_bytes: 3,
        maximum_total_bytes: 8,
        maximum_chunks: 3,
    };
    assert_eq!(
        checked(super::execution_log_chunks(
            &bounds, b"12345", b"67", false, false
        )),
        [2, 1]
    );
    assert!(super::execution_log_chunks(&bounds, b"1234567", b"89", false, false).is_err());
    assert!(super::execution_log_chunks(&bounds, b"1234567", b"8", false, false).is_err());
    assert_eq!(
        checked(super::execution_log_chunks(&bounds, b"", b"", false, false)),
        [0, 0]
    );
    let (_root, mut profile, source, run) = fixture();
    let bound = execution_fixture(&source, &run);
    let mut stale = bound.clone();
    stale.execution = Some(checked(ExternalRef::new(
        "lattice",
        "execution",
        "replacement",
    )));
    let (origin, stop, server) =
        receiver(vec![(Duration::ZERO, checked(serde_json::to_value(stale)))]);
    profile.service_origin = origin;
    assert!(checked(ServiceClient::new(&profile))
        .upload_execution_logs(&bound, "build", b"captured", b"", false, false)
        .is_err());
    stop.store(true, Ordering::SeqCst);
    assert_eq!(
        checked(server.join()),
        1,
        "stale execution received a log upload"
    );
}
