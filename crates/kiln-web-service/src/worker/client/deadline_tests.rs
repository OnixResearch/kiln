use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::source::SourceResolution;
use kiln_core::types::{ExternalRef, ProviderProfileId};
use serde_json::{Value, json};

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
    };
    checked(super::admit_binding(&profile, &source, &binding));
    (root, profile, source, binding)
}

// Send headers immediately but delay the body: the lookup budget must cover
// receiving and admitting the entire binding, not only establishing TLS.
fn receiver(
    replies: Vec<(Duration, Value)>,
) -> (String, Arc<AtomicBool>, std::thread::JoinHandle<usize>) {
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
            loop {
                line.clear();
                assert!(
                    checked(reader.read_line(&mut line)) > 0,
                    "request ended early"
                );
                if line == "\r\n" {
                    break;
                }
            }
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
    (origin, stop, thread)
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
    profile.lookup_timeout_ms = 300;
    profile.http_timeout_ms = 3000;
    let (origin, stop, server) = receiver(vec![(
        Duration::from_millis(1500),
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
        elapsed < Duration::from_millis(900),
        "lookup exceeded its 300ms budget: {elapsed:?}"
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
