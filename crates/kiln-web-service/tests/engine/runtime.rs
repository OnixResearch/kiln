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
