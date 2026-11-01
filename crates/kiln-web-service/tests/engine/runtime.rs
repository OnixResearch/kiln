use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CERTIFICATE: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/cert.pem");
const KEY: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/key.pem");
const CA: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/ca-cert.pem");

fn runtime_fixture() -> (tempfile::TempDir, ServiceProfile, std::path::PathBuf) {
    let (root, mut profile, _) = fixture();
    // Reserve an otherwise free port only for the fixture, not during preflight.
    let listener = checked(std::net::TcpListener::bind("127.0.0.1:0"));
    profile.listen_address = checked(listener.local_addr());
    profile.origin = format!("https://localhost:{}", profile.listen_address.port());
    drop(listener);
    checked(fs::write(&profile.tls.certificate_path, CERTIFICATE));
    checked(fs::write(&profile.tls.private_key_path, KEY));
    checked(fs::write(&profile.operator_token_file, [b'a'; 64]));
    checked(fs::write(&profile.worker_token_file, [b'b'; 64]));
    for path in [
        Path::new(&profile.tls.private_key_path),
        &profile.operator_token_file,
        &profile.worker_token_file,
    ] {
        checked(fs::set_permissions(path, fs::Permissions::from_mode(0o400)));
    }
    let path = root.path().join("service.json");
    checked(fs::write(&path, checked(serde_json::to_vec(&profile))));
    (root, profile, path)
}

fn snapshot(path: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    if path.exists() {
        result.insert(path.to_path_buf(), Vec::new());
        for entry in checked(fs::read_dir(path)) {
            let entry = checked(entry);
            if checked(entry.file_type()).is_dir() {
                result.extend(snapshot(&entry.path()));
            } else {
                result.insert(entry.path(), checked(fs::read(entry.path())));
            }
        }
    }
    result
}

#[test]
fn preflight_accepts_readonly_credentials_without_binding_creating_or_reconciling() {
    let (_root, profile, path) = runtime_fixture();
    // A provider socket need not be live, and preflight must not bind the HTTPS port.
    let _reserved = checked(std::net::TcpListener::bind(profile.listen_address));
    checked(kiln_web_service::server::check(&path));
    assert!(!profile.state_directory.exists());
    let output = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["check", "--profile"])
            .arg(&path)
            .env("NOTIFY_SOCKET", "/missing/supervisor")
            .output(),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!profile.state_directory.exists());
    for secret in [
        Path::new(&profile.tls.private_key_path),
        &profile.operator_token_file,
        &profile.worker_token_file,
    ] {
        checked(fs::set_permissions(
            secret,
            fs::Permissions::from_mode(0o600),
        ));
    }
    let project = profile.projects.first().unwrap_or_else(|| unreachable!());
    let proxy = checked(std::os::unix::net::UnixListener::bind(
        &project.workflow.socket_path,
    ));
    checked(proxy.set_nonblocking(true));
    let backend = Arc::new(Mutex::new(Backend::default()));
    let engine = open(profile.clone(), backend.clone());
    drop(engine);
    let before = snapshot(&profile.state_directory);
    checked(kiln_web_service::server::check(&path));
    assert_eq!(snapshot(&profile.state_directory), before);
    assert!(matches!(proxy.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
}

// r[verify kiln.web.deploy.runtime.scenario.denied]
#[test]
fn preflight_rejects_unsafe_secrets_tls_and_filesystem_inputs_without_state() {
    for case in [
        "public-token",
        "executable-token",
        "linked-token",
        "hardlinked-token",
        "missing-token",
        "short-token",
        "nonhex-token",
        "equal-tokens",
        "fifo-key",
        "public-key",
        "linked-key",
        "oversize-key",
        "malformed-key",
        "mismatched-certificate",
        "state-symlink",
        "public-state",
        "repository-symlink",
        "socket-symlink",
        "socket-file",
        "unknown-profile",
    ] {
        let (root, profile, path) = runtime_fixture();
        for secret in [
            Path::new(&profile.tls.private_key_path),
            &profile.operator_token_file,
            &profile.worker_token_file,
        ] {
            checked(fs::set_permissions(
                secret,
                fs::Permissions::from_mode(0o600),
            ));
        }
        let project = profile.projects.first().unwrap_or_else(|| unreachable!());
        match case {
            "public-token" => checked(fs::set_permissions(
                &profile.operator_token_file,
                fs::Permissions::from_mode(0o640),
            )),
            "executable-token" => checked(fs::set_permissions(
                &profile.operator_token_file,
                fs::Permissions::from_mode(0o500),
            )),
            "linked-token" => {
                checked(fs::remove_file(&profile.operator_token_file));
                checked(symlink(
                    &profile.worker_token_file,
                    &profile.operator_token_file,
                ));
            }
            "hardlinked-token" => checked(fs::hard_link(
                &profile.operator_token_file,
                root.path().join("alias"),
            )),
            "missing-token" => checked(fs::remove_file(&profile.operator_token_file)),
            "short-token" => checked(fs::write(&profile.operator_token_file, [b'a'; 63])),
            "nonhex-token" => checked(fs::write(&profile.operator_token_file, [b'g'; 64])),
            "equal-tokens" => checked(fs::write(&profile.worker_token_file, [b'a'; 64])),
            "fifo-key" => {
                checked(fs::remove_file(&profile.tls.private_key_path));
                checked(nix::unistd::mkfifo(
                    Path::new(&profile.tls.private_key_path),
                    nix::sys::stat::Mode::S_IRUSR,
                ));
            }
            "public-key" => checked(fs::set_permissions(
                &profile.tls.private_key_path,
                fs::Permissions::from_mode(0o644),
            )),
            "linked-key" => {
                checked(fs::rename(
                    &profile.tls.private_key_path,
                    root.path().join("original-key"),
                ));
                checked(symlink(
                    root.path().join("original-key"),
                    &profile.tls.private_key_path,
                ));
            }
            "oversize-key" => checked(fs::write(
                &profile.tls.private_key_path,
                vec![b'x'; 1_048_577],
            )),
            "malformed-key" => checked(fs::write(&profile.tls.private_key_path, b"not a key")),
            "mismatched-certificate" => checked(fs::write(&profile.tls.certificate_path, CA)),
            "state-symlink" => checked(symlink(
                root.path().join("repository"),
                &profile.state_directory,
            )),
            "public-state" => {
                checked(fs::create_dir(&profile.state_directory));
                checked(fs::set_permissions(
                    &profile.state_directory,
                    fs::Permissions::from_mode(0o755),
                ));
            }
            "repository-symlink" => {
                let repository = &project.repository_path;
                checked(fs::rename(
                    repository,
                    root.path().join("original-repository"),
                ));
                checked(symlink(root.path().join("original-repository"), repository));
            }
            "socket-symlink" => checked(symlink(
                root.path().join("missing"),
                &project.workflow.socket_path,
            )),
            "socket-file" => checked(fs::write(&project.workflow.socket_path, b"not a socket")),
            "unknown-profile" => {
                let mut value = checked(serde_json::to_value(&profile));
                checked(value.as_object_mut().ok_or("profile object"))
                    .insert("deployment_override".into(), json!(true));
                checked(fs::write(&path, checked(serde_json::to_vec(&value))));
            }
            _ => unreachable!(),
        }
        let before = if case == "state-symlink" {
            BTreeMap::new()
        } else {
            snapshot(&profile.state_directory)
        };
        assert!(kiln_web_service::server::check(&path).is_err(), "{case}");
        if case != "state-symlink" {
            assert_eq!(snapshot(&profile.state_directory), before, "{case}");
        }
    }
}

struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// r[verify kiln.web.deploy.runtime.scenario.ready]
#[test]
fn initialized_https_health_is_minimal_same_origin_and_survives_sigterm_restart() {
    use std::os::unix::net::UnixDatagram;
    let (root, profile, path) = runtime_fixture();
    let socket_path = root.path().join("notify.sock");
    let receiver = checked(UnixDatagram::bind(&socket_path));
    checked(receiver.set_read_timeout(Some(Duration::from_secs(10))));
    let client = checked(
        reqwest::blocking::Client::builder()
            .add_root_certificate(checked(reqwest::Certificate::from_pem(CA)))
            .timeout(Duration::from_secs(2))
            .build(),
    );
    for _ in 0..2 {
        let mut service = Service(checked(
            Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
                .args(["serve", "--profile"])
                .arg(&path)
                .env("NOTIFY_SOCKET", &socket_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn(),
        ));
        let mut notification = [0; 32];
        let count = checked(receiver.recv(&mut notification));
        assert_eq!(notification.get(..count), Some(b"READY=1".as_slice()));
        assert!(profile.state_directory.join("service.sqlite").is_file());
        let health = format!("{}/healthz", profile.origin);
        let response = checked(client.get(&health).send());
        assert_eq!(response.status(), 200);
        assert_eq!(checked(response.text()), "{\"status\":\"ok\"}");
        for navigation in [
            "/?page=runs&run=retained-run&view=logs&node=build",
            "/?page=settings&theme=dark&density=compact",
        ] {
            let response = checked(client.get(format!("{}{navigation}", profile.origin)).send());
            assert_eq!(response.status(), 200);
            assert_eq!(
                response
                    .headers()
                    .get("content-type")
                    .and_then(|value| value.to_str().ok()),
                Some("text/html; charset=utf-8")
            );
        }
        assert_eq!(
            checked(
                client
                    .get(format!("{}/missing?page=runs", profile.origin))
                    .send()
            )
            .status(),
            404
        );
        assert_eq!(
            checked(
                client
                    .get(format!("{}/api/runs?limit=1", profile.origin))
                    .send()
            )
            .status(),
            401
        );
        let feeds = checked(
            client
                .get(format!("{}/api/feeds", profile.origin))
                .bearer_auth("a".repeat(64))
                .send(),
        );
        assert_eq!(feeds.status(), 200);
        let feeds: serde_json::Value = checked(feeds.json());
        assert_eq!(
            feeds.get("configured"),
            Some(&serde_json::Value::Bool(false))
        );
        assert_eq!(
            checked(
                client
                    .get(&health)
                    .header("Origin", "https://elsewhere.invalid")
                    .send()
            )
            .status(),
            403
        );
        assert_eq!(
            checked(
                client
                    .get(&health)
                    .header("Host", "elsewhere.invalid")
                    .send()
            )
            .status(),
            403
        );
        assert_eq!(
            checked(client.get(format!("{}/api/runs", profile.origin)).send()).status(),
            401
        );
        assert_eq!(checked(client.post(&health).body("").send()).status(), 405);
        checked(nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(service.0.id() as i32),
            nix::sys::signal::Signal::SIGTERM,
        ));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = checked(service.0.try_wait()) {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "SIGTERM exceeded stop deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// r[verify kiln.web.feeds.publish.scenario.rendered]
// r[verify kiln.web.feeds.subscriptions.scenario.add]
// r[verify kiln.web.feeds.subscriptions.scenario.refresh]
// r[verify kiln.web.feeds.token.scenario.required]
// r[verify kiln.web.feeds.config.scenario.console]
#[test]
fn repository_feeds_and_external_subscription_round_trip() {
    use std::io::{Read, Write};
    let (root, mut profile, path) = runtime_fixture();
    let feed_token = "c".repeat(64);
    let feed_token_path = root.path().join("feed.token");
    checked(fs::write(&feed_token_path, &feed_token));
    checked(fs::set_permissions(
        &feed_token_path,
        fs::Permissions::from_mode(0o400),
    ));
    profile.feeds = Some(kiln_web_service::profile::FeedsProfile {
        token_file: feed_token_path,
        max_subscriptions: 4,
        max_entries_per_feed: 10,
        max_feed_bytes: 65_536,
        max_redirects: 1,
        timeout_seconds: 5,
        refresh_interval_ms: 0,
        allow_private_destinations: true,
    });
    checked(fs::write(&path, checked(serde_json::to_vec(&profile))));
    let viewer_token = "d".repeat(64);
    let viewer_token_path = root.path().join("viewer.token");
    checked(fs::write(&viewer_token_path, &viewer_token));
    checked(fs::set_permissions(
        &viewer_token_path,
        fs::Permissions::from_mode(0o400),
    ));
    let access_path = root.path().join("access.json");
    checked(fs::write(
        &access_path,
        checked(serde_json::to_vec(&serde_json::json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{
                "id": "reader",
                "label": "Read-only fixture",
                "token_file": viewer_token_path,
            }],
        }))),
    ));

    let body = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>External repository</title><item><title>External release</title><link>https://example.invalid/release</link><guid>release-1</guid></item></channel></rss>"#;
    let listener = checked(std::net::TcpListener::bind("127.0.0.1:0"));
    let feed_port = checked(listener.local_addr()).port();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });

    let socket_path = root.path().join("notify.sock");
    let receiver = checked(std::os::unix::net::UnixDatagram::bind(&socket_path));
    checked(receiver.set_read_timeout(Some(Duration::from_secs(10))));
    let _service = Service(checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(&path)
            .arg("--access")
            .arg(&access_path)
            .env("NOTIFY_SOCKET", &socket_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn(),
    ));
    let mut notification = [0; 32];
    let count = checked(receiver.recv(&mut notification));
    assert_eq!(notification.get(..count), Some(b"READY=1".as_slice()));

    let client = checked(
        reqwest::blocking::Client::builder()
            .add_root_certificate(checked(reqwest::Certificate::from_pem(CA)))
            .timeout(Duration::from_secs(10))
            .build(),
    );
    let operator = "a".repeat(64);
    let projects = checked(
        client
            .get(format!("{}/api/projects", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    assert_eq!(projects.status(), 200);
    let projects: serde_json::Value = checked(projects.json());
    let key = projects
        .get("available")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("repository_key"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| unreachable!("fixture repository key"))
        .to_string();

    let feed = checked(
        client
            .get(format!(
                "{}/feeds/{key}.xml?token={feed_token}",
                profile.origin
            ))
            .send(),
    );
    assert_eq!(feed.status(), 200);
    assert!(
        feed.headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/atom+xml"))
    );
    let xml = checked(feed.text());
    assert!(xml.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));

    assert_eq!(
        checked(
            client
                .get(format!(
                    "{}/feeds/{key}.xml?token={}",
                    profile.origin,
                    "d".repeat(64)
                ))
                .send()
        )
        .status(),
        403
    );
    assert_eq!(
        checked(
            client
                .get(format!("{}/feeds/{key}.xml", profile.origin))
                .send()
        )
        .status(),
        400
    );
    assert_eq!(
        checked(
            client
                .get(format!("{}/feeds.xml?token={feed_token}", profile.origin))
                .send()
        )
        .status(),
        200
    );
    assert_eq!(
        checked(
            client
                .get(format!(
                    "{}/feeds/{}.xml?token={feed_token}",
                    profile.origin,
                    "f".repeat(64)
                ))
                .send()
        )
        .status(),
        404
    );
    let opml = checked(
        client
            .get(format!("{}/feeds.opml?token={feed_token}", profile.origin))
            .send(),
    );
    assert_eq!(opml.status(), 200);
    assert!(
        opml.headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/x-opml"))
    );
    let opml = checked(opml.text());
    assert!(opml.contains("<opml version=\"2.0\">"));
    assert!(opml.contains("xmlUrl=\""));
    assert!(opml.contains("token="));

    let feed_url = format!("http://127.0.0.1:{feed_port}/feed.xml");
    let added = checked(
        client
            .post(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&operator)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({
                "url": feed_url,
                "label": "External repository"
            }))
            .send(),
    );
    assert_eq!(added.status(), 201);
    let feeds = checked(
        client
            .get(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    assert_eq!(feeds.status(), 200);
    let feeds: serde_json::Value = checked(feeds.json());
    assert_eq!(
        feeds.pointer("/bounds/max_import_bytes"),
        Some(&serde_json::Value::from(profile.bounds.max_request_bytes))
    );
    let subscriptions = feeds
        .get("subscriptions")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture subscriptions"));
    assert_eq!(subscriptions.len(), 1);
    let first = subscriptions
        .first()
        .unwrap_or_else(|| unreachable!("fixture subscription"));
    assert_eq!(
        first
            .get("entries")
            .and_then(|value| value.get(0))
            .and_then(|value| value.get("title")),
        Some(&serde_json::Value::String("External release".into()))
    );
    let id = first
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| unreachable!("fixture subscription id"))
        .to_string();

    let searched = checked(
        client
            .get(format!("{}/api/feeds?q=release", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    assert_eq!(searched.status(), 200);
    let searched: serde_json::Value = checked(searched.json());
    assert_eq!(
        searched
            .get("subscriptions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        searched
            .pointer("/subscriptions/0/entries/0/title")
            .and_then(serde_json::Value::as_str),
        Some("External release")
    );
    let no_match = checked(
        client
            .get(format!("{}/api/feeds?q=missing", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    let no_match: serde_json::Value = checked(no_match.json());
    assert_eq!(
        no_match
            .get("subscriptions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let viewer_feeds = checked(
        client
            .get(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&viewer_token)
            .send(),
    );
    assert_eq!(viewer_feeds.status(), 200);
    let viewer_feeds: serde_json::Value = checked(viewer_feeds.json());
    let viewer_subscription = viewer_feeds
        .get("subscriptions")
        .and_then(|value| value.get(0))
        .unwrap_or_else(|| unreachable!("fixture viewer subscription"));
    assert!(viewer_subscription.get("url").is_none());
    assert!(
        viewer_feeds
            .pointer("/publication/repositories/0/subscribe_url")
            .is_none()
    );
    let viewer_write = checked(
        client
            .post(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&viewer_token)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({"url": "https://example.invalid/feed"}))
            .send(),
    );
    assert_eq!(viewer_write.status(), 403);

    let imported = checked(
        client
            .post(format!("{}/api/feeds/import", profile.origin))
            .bearer_auth(&operator)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({
                "opml": format!(
                    "<?xml version=\"1.0\"?><opml version=\"2.0\"><body><outline text=\"Imported\" xmlUrl=\"https://example.invalid/imported.xml\"/><outline text=\"Invalid\" xmlUrl=\"javascript:alert(1)\"/><outline text=\"Existing\" xmlUrl=\"{feed_url}\"/></body></opml>"
                )
            }))
            .send(),
    );
    assert_eq!(imported.status(), 201);
    let imported: serde_json::Value = checked(imported.json());
    assert_eq!(imported.get("added"), Some(&serde_json::Value::from(1)));
    assert_eq!(imported.get("skipped"), Some(&serde_json::Value::from(2)));
    let after_import = checked(
        client
            .get(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    let after_import: serde_json::Value = checked(after_import.json());
    let imported_rows = after_import
        .get("subscriptions")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture subscriptions"));
    assert_eq!(imported_rows.len(), 2);
    let pending = imported_rows
        .iter()
        .find(|row| row.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str()))
        .unwrap_or_else(|| unreachable!("fixture imported subscription"));
    assert!(pending.get("fetched_at_ms").is_none());
    assert!(
        pending
            .get("entries")
            .is_none_or(|value| value.as_array().is_some_and(Vec::is_empty))
    );
    let viewer_import = checked(
        client
            .post(format!("{}/api/feeds/import", profile.origin))
            .bearer_auth(&viewer_token)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({"opml": "<opml/>"}))
            .send(),
    );
    assert_eq!(viewer_import.status(), 403);

    let refreshed = checked(
        client
            .post(format!("{}/api/feeds/{id}/refresh", profile.origin))
            .bearer_auth(&operator)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({}))
            .send(),
    );
    assert_eq!(refreshed.status(), 502);
    let retained = checked(
        client
            .get(format!("{}/api/feeds", profile.origin))
            .bearer_auth(&operator)
            .send(),
    );
    let retained: serde_json::Value = checked(retained.json());
    let retained_first = retained
        .get("subscriptions")
        .and_then(|value| value.get(0))
        .unwrap_or_else(|| unreachable!("fixture subscription"));
    assert!(
        retained_first
            .get("error")
            .is_some_and(serde_json::Value::is_string)
    );
    assert_eq!(
        retained_first
            .get("entries")
            .and_then(|value| value.get(0))
            .and_then(|value| value.get("title")),
        Some(&serde_json::Value::String("External release".into()))
    );

    let removed = checked(
        client
            .post(format!("{}/api/feeds/{id}/remove", profile.origin))
            .bearer_auth(&operator)
            .header("Origin", &profile.origin)
            .json(&serde_json::json!({}))
            .send(),
    );
    assert_eq!(removed.status(), 200);
}

// r[verify kiln.web.deploy.runtime.scenario.denied]
#[test]
fn configured_notification_failure_prevents_running_service_claim() {
    let (root, _profile, path) = runtime_fixture();
    let output = checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(path)
            .env("NOTIFY_SOCKET", root.path().join("missing.sock"))
            .output(),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("supervisor notification failed"));
}
