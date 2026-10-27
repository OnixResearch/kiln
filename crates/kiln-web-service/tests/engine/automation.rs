use super::*;
use hmac::{Hmac, Mac};
use kiln_adapter_github::events::DeliveryHeaders;
use kiln_web_service::automation::AutomationProfile;
use sha2::Sha256;
use std::os::unix::fs::PermissionsExt;

fn tick(engine: &mut Engine) {
    let notifications = checked(engine.tick_automation());
    finish_notifications(engine, notifications);
}

fn policy(root: &std::path::Path) -> AutomationProfile {
    let secret = root.join("github.secret");
    checked(std::fs::write(&secret, b"private-github-test-secret"));
    checked(std::fs::set_permissions(
        &secret,
        std::fs::Permissions::from_mode(0o600),
    ));
    checked(serde_json::from_value(
        json!({"schema":"kiln.web-automation.v1","max_records":100,"status_limit":10,"max_event_bytes":65536,"max_tick_effects":2,
        "triggers":[{"id":"github","project_id":"fixture","reference":"refs/heads/main","repository":{"owner":"fixture","repository":"source","repository_id":42,"allow_fork_pull_requests":false},"secret_file":secret,"enabled":true,"push":true,"pull_request":true,"pull_request_base":"main"}],
        "schedules":[{"id":"periodic","project_id":"fixture","reference":"refs/heads/main","enabled":false,"interval_ms":60000,"min_interval_ms":1000,"max_interval_ms":3600000,"next_due_ms":null}],"notifications":[]}),
    ))
}
fn repository() -> Value {
    json!({"id":42,"name":"source","full_name":"fixture/source","owner":{"login":"fixture"}})
}
fn push(revision: &str) -> Vec<u8> {
    checked(serde_json::to_vec(
        &json!({"repository":repository(),"ref":"refs/heads/main","after":revision,"deleted":false}),
    ))
}
fn signed(delivery: &str, event: &str, body: &[u8]) -> DeliveryHeaders {
    let mut mac = checked(Hmac::<Sha256>::new_from_slice(
        b"private-github-test-secret",
    ));
    mac.update(body);
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    DeliveryHeaders {
        delivery: delivery.into(),
        event: event.into(),
        signature: format!("sha256={signature}"),
    }
}
#[test]
fn signed_push_is_exact_once_across_restart_and_bad_signature_never_dispatches() {
    let (root, profile, revision) = fixture();
    let mut automation = policy(root.path());
    let mut second = checked(automation.triggers.first().ok_or("trigger")).clone();
    second.id = "other-github".into();
    automation.triggers.push(second);
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation.clone()));
    let body = push(&revision);
    let headers = signed("00000000-0000-4000-8000-000000000001", "push", &body);
    let mut bad = headers.clone();
    bad.signature = format!("sha256={}", "0".repeat(64));
    assert_eq!(
        checked(engine.automation_trigger("github", &bad, &body)).status,
        403
    );
    assert_eq!(backend.lock().dispatches, 0);
    let response = checked(engine.automation_trigger("github", &headers, &body));
    assert_eq!(response.status, 202);
    let run = value(&response)
        .pointer("/run_id")
        .unwrap_or(&serde_json::Value::Null)
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| unreachable!("run identity"));
    assert_eq!(
        value(&get(
            &mut engine,
            &format!("/api/runs/{run}"),
            Role::Operator
        ))
        .pointer("/source_revision")
        .unwrap_or(&serde_json::Value::Null),
        &revision
    );
    assert_eq!(backend.lock().dispatches, 1);
    drop(engine);
    let mut engine = open(profile, backend.clone());
    checked(engine.configure_automation(automation));
    assert_eq!(
        checked(engine.automation_trigger("github", &headers, &body)).status,
        409
    );
    let another = signed("00000000-0000-4000-8000-000000000002", "push", &body);
    assert_eq!(
        checked(engine.automation_trigger("github", &another, &body)).status,
        409
    );
    assert_eq!(
        checked(engine.automation_trigger("other-github", &headers, &body)).status,
        409
    );
    assert_eq!(
        checked(engine.automation_trigger("other-github", &another, &body)).status,
        409
    );
    assert_eq!(backend.lock().dispatches, 1);
}
#[test]
fn signed_pull_request_and_stale_or_unregistered_sources_have_opposite_effects() {
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(policy(root.path())));
    let mut body = json!({"action":"synchronize","repository":repository(),"number":7,"pull_request":{"number":7,"state":"open",
        "head":{"ref":"main","sha":revision,"repo":repository()},"base":{"ref":"main","sha":revision,"repo":repository()}}});
    *checked(body.pointer_mut("/pull_request/head/sha").ok_or("PR head")) = json!("a".repeat(40));
    let stale = checked(serde_json::to_vec(&body));
    assert_eq!(
        checked(engine.automation_trigger(
            "github",
            &signed(
                "00000000-0000-4000-8000-000000000003",
                "pull_request",
                &stale
            ),
            &stale
        ))
        .status,
        409
    );
    *checked(body.pointer_mut("/pull_request/head/sha").ok_or("PR head")) = json!(revision);
    *checked(body.pointer_mut("/pull_request/base/ref").ok_or("PR base")) = json!("unregistered");
    let wrong = checked(serde_json::to_vec(&body));
    assert_eq!(
        checked(engine.automation_trigger(
            "github",
            &signed(
                "00000000-0000-4000-8000-000000000004",
                "pull_request",
                &wrong
            ),
            &wrong
        ))
        .status,
        403
    );
    assert_eq!(backend.lock().dispatches, 0);
    *checked(body.pointer_mut("/pull_request/base/ref").ok_or("PR base")) = json!("main");
    let valid = checked(serde_json::to_vec(&body));
    assert_eq!(
        checked(engine.automation_trigger(
            "github",
            &signed(
                "00000000-0000-4000-8000-000000000005",
                "pull_request",
                &valid
            ),
            &valid
        ))
        .status,
        202
    );
    assert_eq!(backend.lock().dispatches, 1);
}
#[test]
fn due_schedule_reserves_once_and_operator_configuration_cannot_add_authority() {
    let (root, profile, _) = fixture();
    let mut automation = policy(root.path());
    let schedule = checked(automation.schedules.first_mut().ok_or("schedule"));
    schedule.enabled = true;
    schedule.next_due_ms = Some(1);
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation.clone()));
    tick(&mut engine);
    assert_eq!(backend.lock().dispatches, 1);
    let first = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        first
            .pointer("/sources/0/kind")
            .unwrap_or(&serde_json::Value::Null),
        "schedule"
    );
    assert_eq!(
        first
            .pointer("/sources/0/status")
            .unwrap_or(&serde_json::Value::Null),
        "queued"
    );
    drop(engine);
    let mut engine = open(profile, backend.clone());
    checked(engine.configure_automation(automation));
    tick(&mut engine);
    let after = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        first
            .pointer("/sources")
            .unwrap_or(&serde_json::Value::Null),
        after
            .pointer("/sources")
            .unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(backend.lock().dispatches, 1);
    let mutation = json!({"request_id":"schedule-edit","kind":"schedule","id":"periodic","enabled":false,"interval_ms":30000});
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/automation",
            mutation.clone(),
            Role::Viewer
        )
        .status,
        403
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/automation",
            mutation.clone(),
            Role::Operator
        )
        .status,
        200
    );
    let mut illegal = mutation;
    *checked(illegal.get_mut("request_id").ok_or("request ID")) = json!("illegal");
    *checked(illegal.get_mut("interval_ms").ok_or("interval")) = json!(1);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/automation",
            illegal,
            Role::Operator
        )
        .status,
        400
    );
    let arbitrary = json!({"request_id":"arbitrary","kind":"notification","id":"new","enabled":true,"url":"https://attacker.invalid"});
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/automation",
            arbitrary,
            Role::Operator
        )
        .status,
        400
    );
}
#[test]
fn definitely_rejected_source_never_dispatches_after_enrollment_or_polling() {
    // A definite failed admission must not become a provider effect later.
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    checked(engine.configure_automation(policy(root.path())));
    let body = push(&revision);
    let headers = signed("00000000-0000-4000-8000-000000000006", "push", &body);
    assert_eq!(
        checked(engine.automation_trigger("github", &headers, &body)).status,
        202
    );
    let response = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        response
            .pointer("/sources/0/status")
            .unwrap_or(&serde_json::Value::Null),
        "rejected"
    );
    enroll(&mut engine);
    tick(&mut engine);
    assert_eq!(backend.lock().dispatches, 0);
    assert_eq!(
        checked(engine.automation_trigger("github", &headers, &body)).status,
        409
    );
}

fn receiver(response_status: Option<u16>) -> (u16, std::thread::JoinHandle<Value>) {
    receiver_gated(response_status, None)
}

fn receiver_gated(
    response_status: Option<u16>,
    gate: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
) -> (u16, std::thread::JoinHandle<Value>) {
    use std::io::{BufRead, Read, Write};
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certificates = checked(
        rustls_pemfile::certs(&mut std::io::Cursor::new(include_bytes!(
            "../../../../fixtures/http-observation/tls/cert.pem"
        )))
        .collect::<Result<Vec<_>, _>>(),
    );
    let private_key = checked(rustls_pemfile::private_key(&mut std::io::Cursor::new(
        include_bytes!("../../../../fixtures/http-observation/tls/key.pem"),
    )))
    .unwrap_or_else(|| unreachable!("TLS key"));
    let tls = Arc::new(checked(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certificates, private_key),
    ));
    let listener = checked(std::net::TcpListener::bind("127.0.0.1:0"));
    checked(listener.set_nonblocking(true));
    let port = checked(listener.local_addr()).port();
    let thread = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                Err(error) => unreachable!("receiver accept: {error}"),
            }
        };
        checked(socket.set_read_timeout(Some(std::time::Duration::from_secs(5))));
        checked(socket.set_write_timeout(Some(std::time::Duration::from_secs(5))));
        let mut stream =
            rustls::StreamOwned::new(checked(rustls::ServerConnection::new(tls)), socket);
        let mut reader = std::io::BufReader::new(&mut stream);
        let mut line = String::new();
        checked(reader.read_line(&mut line));
        assert_eq!(line, "POST /completion HTTP/1.1\r\n");
        let mut length = None;
        let mut idempotency_key = None;
        loop {
            line.clear();
            assert!(
                checked(reader.read_line(&mut line)) > 0,
                "HTTP headers ended early"
            );
            if line == "\r\n" {
                break;
            }
            let (name, value) = line
                .split_once(':')
                .unwrap_or_else(|| unreachable!("HTTP header"));
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(checked(value.trim().parse::<usize>()));
            }
            if name.eq_ignore_ascii_case("idempotency-key") {
                idempotency_key = Some(value.trim().to_string());
            }
        }
        let size = length.unwrap_or_else(|| unreachable!("HTTP content length"));
        assert!(size <= 65536);
        let mut body = vec![0; size];
        checked(reader.read_exact(&mut body));
        let value: Value = checked(serde_json::from_slice(&body));
        assert_eq!(
            value
                .pointer("/idempotency_key/native_id")
                .unwrap_or(&serde_json::Value::Null)
                .as_str(),
            idempotency_key.as_deref()
        );
        drop(reader);
        if let Some((arrived, release)) = gate {
            checked(arrived.send(()));
            checked(release.recv_timeout(std::time::Duration::from_secs(5)));
        }
        if let Some(status) = response_status {
            checked(stream.write_all(format!("HTTP/1.1 {status} Receiver result\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()));
            checked(stream.flush());
        }
        value
    });
    (port, thread)
}

#[test]
fn real_https_acceptance_and_disconnect_are_durable_and_never_blindly_retried() {
    for (response_status, failed) in [
        (Some(202), false),
        (Some(202), true),
        (Some(400), true),
        (None, true),
    ] {
        let (root, profile, _) = fixture();
        let (port, receiver) = receiver(response_status);
        let ca = root.path().join("receiver-ca.pem");
        checked(std::fs::write(
            &ca,
            include_bytes!("../../../../fixtures/http-observation/tls/ca-cert.pem"),
        ));
        let mut automation = policy(root.path());
        automation.notifications.push(checked(serde_json::from_value(json!({"id":"completion","project_id":"fixture","enabled":true,"on_success":true,"on_failure":true,
            "host":"localhost","port":port,"path":"/completion","addresses":["127.0.0.1"],"bearer_secret_file":null,"ca_certificate_file":ca,"allow_loopback":true,"allow_private":false,
            "timeout_ms":5000,"max_request_bytes":65536,"max_response_bytes":65536,"max_resolver_candidates":4}))));
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = open(profile.clone(), backend.clone());
        enroll(&mut engine);
        checked(engine.configure_automation(automation.clone()));
        let start = start(&mut engine, "notified-run");
        assert_eq!(start.status, 201);
        let run_id = value(&start)
            .pointer("/id")
            .unwrap_or(&serde_json::Value::Null)
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| unreachable!("run id"));
        {
            let mut state = backend.lock();
            state.terminal = true;
            state.sequence += 1;
            if failed {
                state.failed_nodes.insert(
                    checked(
                        checked(profile.projects.first().ok_or("project"))
                            .workflow
                            .nodes
                            .first()
                            .ok_or("node"),
                    )
                    .id
                    .clone(),
                );
            }
        }
        poll(&mut engine);
        let delivered = checked(receiver.join());
        assert_eq!(
            delivered
                .pointer("/run_id")
                .unwrap_or(&serde_json::Value::Null),
            &run_id
        );
        assert_eq!(
            delivered
                .pointer("/outcome")
                .unwrap_or(&serde_json::Value::Null),
            if failed { "domain_failure" } else { "success" }
        );
        let status = value(&get(&mut engine, "/api/automation", Role::Operator));
        assert_eq!(
            status
                .pointer("/deliveries/0/status")
                .unwrap_or(&serde_json::Value::Null),
            match response_status {
                Some(202) => "delivered",
                Some(_) => "rejected",
                None => "ambiguous",
            }
        );
        assert_eq!(
            status
                .pointer("/deliveries/0/http_status")
                .unwrap_or(&serde_json::Value::Null),
            &json!(response_status)
        );
        drop(engine);
        let mut engine = open(profile, backend.clone());
        checked(engine.configure_automation(automation));
        tick(&mut engine);
        assert_eq!(
            value(&get(&mut engine, "/api/automation", Role::Operator))
                .pointer("/deliveries")
                .unwrap_or(&serde_json::Value::Null),
            status
                .pointer("/deliveries")
                .unwrap_or(&serde_json::Value::Null)
        );
        assert_eq!(backend.lock().dispatches, 1);
    }
}

#[test]
fn notification_authority_rejects_url_confusion_and_unadmitted_networks() {
    let (root, _, _) = fixture();
    let mut automation = policy(root.path());
    let receiver = json!({"id":"completion","project_id":"fixture","enabled":false,"on_success":true,"on_failure":true,
        "host":"localhost","port":443,"path":"/completion","addresses":["127.0.0.1"],"bearer_secret_file":null,"ca_certificate_file":null,"allow_loopback":true,"allow_private":false,
        "timeout_ms":1000,"max_request_bytes":4096,"max_response_bytes":4096,"max_resolver_candidates":4});
    automation
        .notifications
        .push(checked(serde_json::from_value(receiver.clone())));
    checked(automation.admit());
    for (field, replacement) in [
        ("host", json!("localhost@attacker.invalid")),
        ("path", json!("//attacker.invalid")),
        ("host", json!("0x7f000001")),
        ("allow_loopback", json!(false)),
        ("addresses", json!(["::ffff:127.0.0.1"])),
    ] {
        let mut invalid = receiver.clone();
        checked(invalid.as_object_mut().ok_or("receiver")).insert(field.to_string(), replacement);
        // Mapped loopback must not evade the loopback flag.
        if field == "addresses" {
            checked(invalid.as_object_mut().ok_or("receiver"))
                .insert("allow_loopback".into(), json!(false));
        }
        *checked(automation.notifications.first_mut().ok_or("notification")) =
            checked(serde_json::from_value(invalid));
        assert!(automation.admit().is_err(), "accepted invalid {field}");
    }
}

fn notification(
    root: &std::path::Path,
    port: u16,
) -> kiln_web_service::automation::NotificationProfile {
    let ca = root.join("receiver-ca.pem");
    checked(std::fs::write(
        &ca,
        include_bytes!("../../../../fixtures/http-observation/tls/ca-cert.pem"),
    ));
    checked(serde_json::from_value(
        json!({"id":"completion","project_id":"fixture","enabled":true,"on_success":true,"on_failure":true,
        "host":"localhost","port":port,"path":"/completion","addresses":["127.0.0.1"],"bearer_secret_file":null,"ca_certificate_file":ca,"allow_loopback":true,"allow_private":false,
        "timeout_ms":5000,"max_request_bytes":65536,"max_response_bytes":65536,"max_resolver_candidates":4}),
    ))
}

#[test]
fn receiver_blocked_notification_does_not_block_worker_or_api_ingress() {
    let (root, profile, revision) = fixture();
    let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (port, receiver) = receiver_gated(Some(202), Some((arrived_tx, release_rx)));
    let mut automation = policy(root.path());
    automation
        .notifications
        .push(notification(root.path(), port));
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation));
    assert_eq!(start(&mut engine, "blocked-receiver").status, 201);
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    let engine = Arc::new(Mutex::new(engine));
    let polling_engine = engine.clone();
    let polling = std::thread::spawn(move || {
        let notifications = checked(polling_engine.lock().poll());
        for notification in notifications {
            let completion = checked(notification.deliver());
            checked(polling_engine.lock().record_notification(completion));
        }
    });
    checked(arrived_rx.recv_timeout(std::time::Duration::from_secs(5)));
    let ingress_engine = engine.clone();
    let (ingress_tx, ingress_rx) = std::sync::mpsc::channel();
    let ingress = std::thread::spawn(move || {
        let mut engine = ingress_engine.lock();
        let worker = get(
            &mut engine,
            &format!("/api/runs?project_id=fixture&source_revision={revision}"),
            Role::Worker,
        )
        .status;
        let api = get(&mut engine, "/api/automation", Role::Viewer).status;
        checked(ingress_tx.send((worker, api)));
    });
    let available_while_receiver_blocked =
        ingress_rx.recv_timeout(std::time::Duration::from_millis(500));
    checked(release_tx.send(()));
    checked(polling.join());
    checked(ingress.join());
    checked(receiver.join());
    assert_eq!(checked(available_while_receiver_blocked), (200, 200));
}

#[test]
fn single_effect_budget_cannot_starve_later_schedule_or_pending_notification() {
    let (root, profile, _) = fixture();
    let mut automation = policy(root.path());
    automation.max_tick_effects = 1;
    let schedule = checked(automation.schedules.first_mut().ok_or("schedule"));
    schedule.enabled = true;
    schedule.interval_ms = 1000;
    schedule.next_due_ms = Some(1);
    let mut later = schedule.clone();
    later.id = "later".into();
    automation.schedules.push(later);
    // A missing local CA produces deterministic preflight rejection, no receiver.
    let mut completion = notification(root.path(), 443);
    completion.ca_certificate_file = Some(root.path().join("missing-ca.pem"));
    automation.notifications.push(completion);
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation.clone()));
    assert_eq!(start(&mut engine, "pending-notification").status, 201);
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    for tick in 0..3 {
        poll(&mut engine);
        if tick < 2 {
            // Keep the first registration ready, including across service restart.
            std::thread::sleep(std::time::Duration::from_millis(1100));
            drop(engine);
            engine = open(profile.clone(), backend.clone());
            checked(engine.configure_automation(automation.clone()));
        }
    }
    let status = value(&get(&mut engine, "/api/automation", Role::Operator));
    let sources = status
        .pointer("/sources")
        .unwrap_or(&serde_json::Value::Null)
        .as_array()
        .unwrap_or_else(|| unreachable!("sources"));
    assert!(
        sources.iter().any(|source| source
            .pointer("/registration_id")
            .unwrap_or(&serde_json::Value::Null)
            == "later"),
        "later due schedule starved: {status}"
    );
    assert_eq!(
        status
            .pointer("/deliveries/0/registration_id")
            .unwrap_or(&serde_json::Value::Null),
        "completion",
        "pending notification starved: {status}"
    );
    assert!(
        sources.len()
            + status
                .pointer("/deliveries")
                .unwrap_or(&serde_json::Value::Null)
                .as_array()
                .unwrap_or_else(|| unreachable!("deliveries"))
                .len()
            <= 3,
        "tick budget exceeded"
    );
}

#[test]
fn restart_never_resends_reserved_notification_and_rejects_stale_completion() {
    let (root, profile, _) = fixture();
    let (port, receiver) = receiver(Some(202));
    let mut automation = policy(root.path());
    automation
        .notifications
        .push(notification(root.path(), port));
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation.clone()));
    let run = value(&start(&mut engine, "reserved-before-restart"));
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    let mut work = checked(engine.poll());
    assert_eq!(work.len(), 1);
    let completion = checked(
        work.pop()
            .unwrap_or_else(|| unreachable!("reserved notification"))
            .deliver(),
    );
    assert_eq!(
        checked(receiver.join())
            .pointer("/run_id")
            .unwrap_or(&serde_json::Value::Null),
        run.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    // Simulate losing the service after the receiver accepts, before observation save.
    drop(engine);
    let mut engine = open(profile, backend);
    checked(engine.configure_automation(automation));
    assert!(engine.record_notification(completion).is_err());
    assert!(checked(engine.tick_automation()).is_empty());
    let state = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        state
            .pointer("/deliveries/0/run_id")
            .unwrap_or(&serde_json::Value::Null),
        run.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        state
            .pointer("/deliveries/0/status")
            .unwrap_or(&serde_json::Value::Null),
        "ambiguous"
    );
    assert_eq!(
        state
            .pointer("/deliveries/0/error_code")
            .unwrap_or(&serde_json::Value::Null),
        "restart_after_reservation"
    );
}
