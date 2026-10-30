use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "git-fixtures")]
use std::path::{Path, PathBuf};
#[cfg(feature = "git-fixtures")]
use std::process::Command;
use std::thread::JoinHandle;
use std::time::Duration;

use kiln_provider_api::PortFailure;
use tempfile::TempDir;

use super::*;
use crate::auth::verify_webhook_signature;
use crate::events::{Admission, admit_notify};
#[cfg(feature = "git-fixtures")]
use crate::identity::ObjectFormat;
use crate::pipeline::{PlanOutcome, plan_pipeline};
use crate::source::{GitCliSource, GitLimits};
use crate::status::{HttpStatusPublisher, StatusDelivery, StatusPort};
use crate::store::{Insert, SqliteStore, StoreLimits, TokenClaim};

const NOW: i64 = 1_800_000_000;
pub const FIXTURE_TIMEOUT_MS: u64 = 5_000;

/// `KILN_GIT_EXE`, else `git` on PATH; only `git-fixtures` tests need it.
#[cfg(feature = "git-fixtures")]
pub fn git_program() -> PathBuf {
    std::env::var_os("KILN_GIT_EXE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("PATH").and_then(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join("git"))
                    .find(|git| git.is_file())
            })
        })
        .unwrap_or_else(|| unreachable!("git-fixtures tests need KILN_GIT_EXE or git on PATH"))
}

#[cfg(feature = "git-fixtures")]
fn git(directory: &Path, args: &[&str]) -> String {
    let output = ok(Command::new(git_program())
        .args([
            "-c",
            "user.name=Kiln Fixture",
            "-c",
            "user.email=fixture@kiln.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output());
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(feature = "git-fixtures")]
/// A knot-like repository: commit B adds the workflows, commit C changes
/// `src/main.go`. Returns (path, B, C).
pub fn fixture_repository(root: &Path, format: ObjectFormat) -> (PathBuf, ObjectId, ObjectId) {
    let path = root.join(format!("knot-{}", format.as_str()));
    ok(std::fs::create_dir_all(path.join(".tangled/workflows")));
    ok(std::fs::create_dir_all(path.join("src")));
    git(
        &path,
        &[
            "init",
            "--quiet",
            "-b",
            "main",
            &format!("--object-format={}", format.as_str()),
        ],
    );
    for file in workflow_files() {
        ok(std::fs::write(path.join(&file.path), &file.bytes));
    }
    ok(std::fs::write(
        path.join(".tangled/workflows/notes.md"),
        "not a workflow",
    ));
    ok(std::fs::write(path.join("src/main.go"), "package main\n"));
    git(&path, &["add", "."]);
    git(&path, &["commit", "--quiet", "-m", "workflows"]);
    let base = some(ObjectId::parse(&git(&path, &["rev-parse", "HEAD"])));
    ok(std::fs::write(
        path.join("src/main.go"),
        "package main\n\nfunc main() {}\n",
    ));
    git(&path, &["commit", "--quiet", "-am", "change"]);
    let head = some(ObjectId::parse(&git(&path, &["rev-parse", "HEAD"])));
    (path, base, head)
}

pub fn git_limits() -> GitLimits {
    GitLimits {
        timeout_ms: 30_000,
        maximum_workflow_files: 16,
        maximum_workflow_bytes: 65_536,
        maximum_changed_paths: 1024,
        maximum_changed_path_bytes: 65_536,
    }
}

pub fn private_root() -> TempDir {
    let root = ok(TempDir::new());
    ok(std::fs::set_permissions(
        root.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    ));
    root
}

// r[verify kiln.tangled.source]
#[cfg(feature = "git-fixtures")]
#[test]
fn git_source_acquires_exact_sha1_and_sha256_commits() {
    let root = private_root();
    for format in [ObjectFormat::Sha256, ObjectFormat::Sha1] {
        let (repository, base, head) = fixture_repository(root.path(), format);
        let url = format!("file://{}", repository.display());
        let mut source = ok(GitCliSource::fixture(
            git_program(),
            root.path().join("stores"),
            git_limits(),
        ));
        let acquired = ok(source.acquire(&AcquireRequest {
            repo_did: REPO,
            clone_url: &url,
            commit: &head,
            base: Some(&base),
        }));
        assert_eq!(acquired.commit, head);
        assert_eq!(head.format(), format);
        let paths: Vec<&str> = acquired
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                ".tangled/workflows/cache.yml",
                ".tangled/workflows/docs.yml",
                ".tangled/workflows/test.yml"
            ]
        );
        assert_eq!(
            acquired.changed,
            ChangedFiles::Exact(["src/main.go".to_string()].into())
        );
        // The older commit is acquired exactly, not the branch tip.
        let older = ok(source.acquire(&AcquireRequest {
            repo_did: REPO,
            clone_url: &url,
            commit: &base,
            base: None,
        }));
        assert_eq!(older.commit, base);
        assert_eq!(
            older.changed,
            ChangedFiles::Fallback(ChangedFallback::NoBase)
        );
        assert_eq!(ok(source.resolve(REPO, &url, "main")), Some(head.clone()));
        assert_eq!(ok(source.resolve(REPO, &url, "HEAD")), Some(head.clone()));
        assert_eq!(ok(source.resolve(REPO, &url, "missing")), None);
        let absent = object('7', head.as_str().len());
        assert!(
            source
                .acquire(&AcquireRequest {
                    repo_did: REPO,
                    clone_url: &url,
                    commit: &absent,
                    base: None
                })
                .is_err()
        );
    }
}

#[test]
fn production_git_source_refuses_non_https_urls() {
    let root = private_root();
    // Refused before any Git process starts, so no executable is needed.
    let mut source = ok(GitCliSource::new(
        "/nonexistent/git".into(),
        root.path().join("stores"),
        git_limits(),
    ));
    let commit = object('a', 40);
    let refused = err(source.acquire(&AcquireRequest {
        repo_did: REPO,
        clone_url: "file:///tmp/x",
        commit: &commit,
        base: None,
    }));
    assert!(matches!(refused, PortFailure::Domain(_)));
    assert!(GitCliSource::new("git".into(), root.path().join("x"), git_limits()).is_err());
}

// r[verify kiln.tangled.replay]
#[test]
fn store_dedupes_replay_keys_and_tokens_durably() {
    let root = ok(TempDir::new());
    let path = root.path().join("tangled.sqlite");
    let limits = StoreLimits {
        maximum_pipelines: 1,
        maximum_tokens: 2,
        maximum_record_bytes: 1_048_576,
    };
    let profile = profile();
    let body = ok(serde_json::to_vec(
        &serde_json::json!({ "repo": REPO, "event": {
            "$type": crate::nsid::PUSH, "repo": REPO, "ref": "refs/heads/main", "oldSha": "a".repeat(40), "newSha": "b".repeat(40), "actor": TRUSTED
        }}),
    ));
    let Admission::Event(event) = ok(admit_notify(&profile, &verified(KNOT, "j"), &body)) else {
        unreachable!()
    };
    let PlanOutcome::Pipeline(record) = ok(plan_pipeline(
        &profile,
        &event,
        &acquired(&event.commit),
        NOW,
    )) else {
        unreachable!()
    };
    {
        let mut store = ok(SqliteStore::open(&path, limits));
        assert_eq!(ok(store.insert(&record)), Insert::Inserted);
        assert_eq!(
            ok(store.insert(&record)),
            Insert::Existing(record.pipeline_id.clone())
        );
        assert_eq!(
            ok(store.claim_token(KNOT, "t1", NOW + 60, NOW)),
            TokenClaim::Fresh
        );
        assert_eq!(
            ok(store.claim_token(KNOT, "t1", NOW + 60, NOW)),
            TokenClaim::Replayed
        );
        assert_eq!(
            ok(store.claim_token(TRUSTED, "t1", NOW + 60, NOW)),
            TokenClaim::Fresh,
            "jti scopes per issuer"
        );
        assert!(
            store.claim_token(KNOT, "t3", NOW + 60, NOW).is_err(),
            "bounded token window"
        );
    }
    let mut store = ok(SqliteStore::open(&path, limits));
    assert_eq!(
        ok(store.find_replay(record.replay_key.as_str())),
        Some(record.pipeline_id.clone())
    );
    let loaded = some(ok(store.load(&record.pipeline_id)));
    assert_eq!(loaded, *record);
    assert_eq!(
        ok(store.claim_token(KNOT, "t1", NOW + 60, NOW + 30)),
        TokenClaim::Replayed,
        "durable across reopen"
    );
    assert_eq!(
        ok(store.claim_token(KNOT, "t1", NOW + 200, NOW + 61)),
        TokenClaim::Fresh,
        "expired ids are forgotten"
    );
    let mut other = (*record).clone();
    other.pipeline_id = "kiln-other".into();
    other.replay_key = some(kiln_core::types::ContentId::from_blake3_hex("e".repeat(64)).ok());
    assert!(store.insert(&other).is_err(), "bounded pipeline count");
    let mut changed = loaded;
    changed.retries = 1;
    ok(store.save(&changed));
    assert_eq!(some(ok(store.load(&record.pipeline_id))).retries, 1);
}

pub struct Captured {
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// Read one HTTP request from `stream`.
pub fn read_http(stream: &mut TcpStream) -> Captured {
    ok(stream.set_read_timeout(Some(Duration::from_millis(FIXTURE_TIMEOUT_MS))));
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let end = loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
        let read = ok(stream.read(&mut chunk));
        assert!(read > 0, "request ended early");
        buffer.extend_from_slice(some(chunk.get(..read)));
    };
    let head = String::from_utf8_lossy(some(buffer.get(..end))).to_string();
    let headers: BTreeMap<String, String> = head
        .split("\r\n")
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = some(buffer.get(end + 4..)).to_vec();
    while body.len() < length {
        let read = ok(stream.read(&mut chunk));
        assert!(read > 0, "body ended early");
        body.extend_from_slice(some(chunk.get(..read)));
    }
    Captured { headers, body }
}

fn endpoint_once(response: Option<&'static str>) -> (String, JoinHandle<Captured>) {
    let listener = ok(TcpListener::bind("127.0.0.1:0"));
    let url = format!(
        "http://127.0.0.1:{}/status",
        ok(listener.local_addr()).port()
    );
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = ok(listener.accept());
        let captured = read_http(&mut stream);
        if let Some(response) = response {
            ok(stream.write_all(response.as_bytes()));
        }
        captured
    });
    (url, handle)
}

fn delivery() -> StatusDelivery {
    StatusDelivery {
        pipeline_id: "kiln-0123".into(),
        request_identity: some(kiln_core::types::ContentId::from_blake3_hex("a".repeat(64)).ok()),
        body: br#"{"id":"kiln-0123","workflows":[]}"#.to_vec(),
    }
}

// r[verify kiln.tangled.status]
#[test]
fn status_publisher_signs_deliveries_and_reports_uncertainty() {
    let secret = b"status-secret".to_vec();
    let (url, handle) = endpoint_once(Some("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"));
    let mut publisher = ok(HttpStatusPublisher::fixture(
        &url,
        secret.clone(),
        FIXTURE_TIMEOUT_MS,
    ));
    ok(publisher.publish(&delivery()));
    let captured = ok(handle.join().map_err(|_| "endpoint thread"));
    assert_eq!(captured.body, delivery().body);
    assert_eq!(
        captured.headers.get("x-tangled-event").map(String::as_str),
        Some(crate::nsid::STATUS_EVENT)
    );
    assert_eq!(
        captured
            .headers
            .get("x-tangled-delivery")
            .map(String::as_str),
        Some("a".repeat(64).as_str())
    );
    let signature = some(captured.headers.get("x-tangled-signature-256"));
    ok(verify_webhook_signature(signature, &captured.body, &secret));
    assert!(verify_webhook_signature(signature, &captured.body, b"wrong").is_err());

    let (url, handle) = endpoint_once(Some(
        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n",
    ));
    let mut publisher = ok(HttpStatusPublisher::fixture(
        &url,
        secret.clone(),
        FIXTURE_TIMEOUT_MS,
    ));
    assert!(matches!(
        err(publisher.publish(&delivery())),
        PortFailure::Domain(_)
    ));
    let _ = handle.join();

    // The endpoint read the delivery and hung up: the outcome is unknown.
    let (url, handle) = endpoint_once(None);
    let mut publisher = ok(HttpStatusPublisher::fixture(
        &url,
        secret.clone(),
        FIXTURE_TIMEOUT_MS,
    ));
    assert!(matches!(
        err(publisher.publish(&delivery())),
        PortFailure::Unknown(_)
    ));
    let _ = handle.join();

    let closed = ok(TcpListener::bind("127.0.0.1:0"));
    let url = format!("http://127.0.0.1:{}/status", ok(closed.local_addr()).port());
    drop(closed);
    let mut publisher = ok(HttpStatusPublisher::fixture(
        &url,
        secret.clone(),
        FIXTURE_TIMEOUT_MS,
    ));
    assert!(matches!(
        err(publisher.publish(&delivery())),
        PortFailure::Retryable(_)
    ));
    assert!(HttpStatusPublisher::new("http://status.example.com", secret, 1000).is_err());
}

#[test]
fn secrets_must_be_private_files() {
    let root = ok(TempDir::new());
    let path = root.path().join("secret");
    ok(std::fs::write(&path, "value\n"));
    ok(std::fs::set_permissions(
        &path,
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    ));
    assert!(crate::status::read_secret(&path).is_err());
    ok(std::fs::set_permissions(
        &path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    ));
    assert_eq!(ok(crate::status::read_secret(&path)), b"value".to_vec());
}
