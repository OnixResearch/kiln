#![cfg(unix)]
//! Exit codes of the `kiln-fleet` binary: 0 with JSON on stdout, 1 for
//! refusals and HTTP errors with JSON on stderr, 2 for usage.

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{TestResult, config, write_enrollment};
use kiln_fleet::api::Service;
use kiln_fleet::config::FleetConfig;
use serde_json::Value;

fn kiln_fleet(arguments: &[&str]) -> TestResult<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args(arguments)
        .env_remove("KILN_FLEET_SERVER")
        .output()?)
}

fn json(bytes: &[u8]) -> TestResult<Value> {
    Ok(serde_json::from_slice(bytes)?)
}

#[test]
fn usage_errors_exit_two() -> TestResult {
    for arguments in [
        &[][..],
        &["frobnicate"][..],
        &["claims"][..],
        &["claims", "--server"][..],
        &["claims", "--server", "ftp://x:1"][..],
        &[
            "claim",
            "--scope",
            "a/b c",
            "--server",
            "http://127.0.0.1:1",
        ][..],
        &[
            "renew",
            "--claim",
            "short",
            "--fence",
            "1",
            "--server",
            "http://127.0.0.1:1",
        ][..],
        &[
            "handoff",
            "--attempt",
            &"a".repeat(64),
            "--fence",
            "x",
            "--commit",
            "c",
            "--server",
            "http://127.0.0.1:1",
        ][..],
        &["check-config"][..],
        &["claims", "--bogus", "1"][..],
    ] {
        let output = kiln_fleet(arguments)?;
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(json(&output.stderr)?.get("error").is_some());
    }
    assert_eq!(kiln_fleet(&["--help"])?.status.code(), Some(0));
    Ok(())
}

#[test]
fn check_config_accepts_the_positive_export_and_refuses_others() -> TestResult {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fleet.json");
    let fixture = fixture.to_string_lossy().into_owned();
    let accepted = kiln_fleet(&["check-config", "--config", &fixture])?;
    assert_eq!(accepted.status.code(), Some(0));
    assert_eq!(
        json(&accepted.stdout)?.get("valid"),
        Some(&Value::Bool(true))
    );
    let directory = tempfile::tempdir()?;
    let bad = directory.path().join("bad.json");
    std::fs::write(&bad, br#"{"schema":"kiln.fleet.v1"}"#)?;
    let refused = kiln_fleet(&["check-config", "--config", &bad.to_string_lossy()])?;
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(
        json(&refused.stderr)?.get("valid"),
        Some(&Value::Bool(false))
    );
    Ok(())
}

#[test]
fn client_commands_report_http_outcomes() -> TestResult {
    let directory = tempfile::tempdir()?;
    write_enrollment(directory.path())?;
    let document = config(directory.path(), Path::new("/nonexistent/tailscale"));
    let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
        &document,
    )?)?)?);
    let listeners = kiln_fleet::server::bind(&["127.0.0.1:0".parse()?])?;
    let address = listeners.first().ok_or("no listener")?.local_addr()?;
    kiln_fleet::server::spawn(listeners, &service)?;
    let server = format!("http://{address}");
    let claim = |session: &str| {
        kiln_fleet(&[
            "claim",
            "--scope",
            "OnixResearch/kiln src",
            "--tool",
            "codex",
            "--session",
            session,
            "--purpose",
            "fix",
            "--lease-minutes",
            "30",
            "--server",
            &server,
        ])
    };
    let granted = claim("one")?;
    assert_eq!(
        granted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&granted.stderr)
    );
    let claim_body = json(&granted.stdout)?;
    assert_eq!(
        claim_body
            .pointer("/claim/holder/host")
            .and_then(Value::as_str),
        Some("britton-desktop")
    );
    let conflict = claim("two")?;
    assert_eq!(conflict.status.code(), Some(1));
    assert_eq!(
        json(&conflict.stderr)?.get("status"),
        Some(&Value::from(409))
    );
    let listed = kiln_fleet(&["claims", "--server", &server])?;
    assert_eq!(listed.status.code(), Some(0));
    let identity = claim_body
        .pointer("/claim/identity")
        .and_then(Value::as_str)
        .ok_or("identity")?;
    let status = kiln_fleet(&["status", "--server", &server])?;
    assert_eq!(status.status.code(), Some(0));
    assert_eq!(json(&status.stdout)?.get("claims"), Some(&Value::from(1)));
    let waybar = kiln_fleet(&["status", "--format", "waybar", "--server", &server])?;
    assert_eq!(waybar.status.code(), Some(0));
    assert!(
        json(&waybar.stdout)?
            .get("text")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("C1"))
    );
    let tmux = kiln_fleet(&["status", "--format", "tmux", "--server", &server])?;
    assert_eq!(tmux.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&tmux.stdout).contains("Fleet C:1"));
    let watch = kiln_fleet(&[
        "watch",
        "--iterations",
        "1",
        "--interval",
        "1",
        "--server",
        &server,
    ])?;
    assert_eq!(watch.status.code(), Some(0));
    let display = String::from_utf8_lossy(&watch.stdout);
    assert!(display.contains("Fleet britton-desktop"));
    assert!(display.contains("Claims"));
    let mut interactive = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args(["watch", "--interval", "1", "--server", &server])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    interactive
        .stdin
        .as_mut()
        .ok_or("stdin")?
        .write_all(b"help\nquit\n")?;
    let interactive = interactive.wait_with_output()?;
    assert_eq!(interactive.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&interactive.stdout).contains("renew"));
    let released = kiln_fleet(&[
        "release", "--claim", identity, "--fence", "1", "--note", "done", "--server", &server,
    ])?;
    assert_eq!(
        released.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&released.stderr)
    );
    let stale = kiln_fleet(&[
        "release", "--claim", identity, "--fence", "1", "--server", &server,
    ])?;
    assert_eq!(stale.status.code(), Some(1));
    let unreachable = kiln_fleet(&["work", "--server", "http://127.0.0.1:1"])?;
    assert_eq!(unreachable.status.code(), Some(1));
    assert!(json(&unreachable.stderr)?.get("error").is_some());
    let from_environment = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .arg("decisions")
        .env("KILN_FLEET_SERVER", &server)
        .output()?;
    assert_eq!(from_environment.status.code(), Some(0));
    Ok(())
}

#[test]
// r[verify kiln.fleet.swarm.surface]
fn swarm_cli_subcommands_round_trip() -> TestResult {
    let directory = tempfile::tempdir()?;
    write_enrollment(directory.path())?;
    let document = config(directory.path(), Path::new("/nonexistent/tailscale"));
    let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
        &document,
    )?)?)?);
    let listeners = kiln_fleet::server::bind(&["127.0.0.1:0".parse()?])?;
    let address = listeners.first().ok_or("no listener")?.local_addr()?;
    kiln_fleet::server::spawn(listeners, &service)?;
    let server = format!("http://{address}");

    let created = kiln_fleet(&[
        "swarm",
        "create",
        "--title",
        "CLI Swarm",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--max-concurrency",
        "3",
        "--server",
        &server,
    ])?;
    assert_eq!(
        created.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created_json = json(&created.stdout)?;
    let swarm_id = created_json
        .pointer("/swarm/identity")
        .and_then(Value::as_str)
        .ok_or("swarm id")?;

    let spawned = kiln_fleet(&[
        "swarm",
        "spawn",
        "--swarm",
        swarm_id,
        "--name",
        "author-cli",
        "--role",
        "author",
        "--harness",
        "codex",
        "--model",
        "gpt-5",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet/src",
        "--brief",
        "Implement CLI",
        "--server",
        &server,
    ])?;
    assert_eq!(spawned.status.code(), Some(0));

    let posted = kiln_fleet(&[
        "swarm",
        "post",
        "--swarm",
        swarm_id,
        "--sender",
        "author-cli",
        "--kind",
        "status",
        "--mentions",
        "@all",
        "--body",
        "Working cleanly",
        "--server",
        &server,
    ])?;
    assert_eq!(posted.status.code(), Some(0));

    let read = kiln_fleet(&[
        "swarm", "read", "--swarm", swarm_id, "--since", "0", "--server", &server,
    ])?;
    assert_eq!(read.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&read.stdout).contains("Working cleanly"));

    // Query-smuggling attempts in --member or --seqs exit with usage error (2).
    let bad_member = kiln_fleet(&[
        "swarm",
        "read",
        "--swarm",
        swarm_id,
        "--member",
        "author-cli&limit=1",
        "--server",
        &server,
    ])?;
    assert_eq!(bad_member.status.code(), Some(2));
    let bad_seqs = kiln_fleet(&[
        "swarm",
        "read",
        "--swarm",
        swarm_id,
        "--seqs",
        "1,2&member=x",
        "--server",
        &server,
    ])?;
    assert_eq!(bad_seqs.status.code(), Some(2));

    let graph = kiln_fleet(&[
        "swarm",
        "graph",
        "--swarm",
        swarm_id,
        "--up-to-seq",
        "2",
        "--server",
        &server,
    ])?;
    assert_eq!(graph.status.code(), Some(0));
    assert_eq!(
        json(&graph.stdout)?.pointer("/graph/up_to_seq"),
        Some(&serde_json::json!(2))
    );

    let status = kiln_fleet(&["swarm", "status", "--swarm", swarm_id, "--server", &server])?;
    assert_eq!(status.status.code(), Some(0));
    let list = kiln_fleet(&["swarm", "list", "--server", &server])?;
    assert_eq!(list.status.code(), Some(0));
    Ok(())
}

fn spawn_loopback_service(
    root: &Path,
) -> TestResult<(Arc<Service>, String, Vec<std::thread::JoinHandle<()>>)> {
    write_enrollment(root)?;
    let document = config(root, Path::new("/nonexistent/tailscale"));
    let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
        &document,
    )?)?)?);
    let listeners = kiln_fleet::server::bind(&["127.0.0.1:0".parse()?])?;
    let address = listeners.first().ok_or("no listener")?.local_addr()?;
    let handles = kiln_fleet::server::spawn(listeners, &service)?;
    Ok((service, format!("http://{address}"), handles))
}

fn wait_for_file(path: &Path, timeout: Duration) -> TestResult {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Err(format!("timed out waiting for {}", path.display()).into())
}

fn wait_for_nonempty_file(path: &Path, timeout: Duration) -> TestResult<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Ok(text);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Err(format!("timed out waiting for content in {}", path.display()).into())
}

fn wait_for_child(
    child: &mut std::process::Child,
    timeout: Duration,
) -> TestResult<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("timed out waiting for child process exit".into())
}

#[test]
fn hold_usage_errors_exit_two_and_pass_child_flags_after_separator() -> TestResult {
    for arguments in [
        &["hold"][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--server",
            "http://127.0.0.1:1",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--server",
            "http://127.0.0.1:1",
            "--",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--server",
            "http://127.0.0.1:1",
            "--",
            "true",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--lease-minutes",
            "0",
            "--server",
            "http://127.0.0.1:1",
            "--",
            "true",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--lease-minutes",
            "1441",
            "--server",
            "http://127.0.0.1:1",
            "--",
            "true",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--lease-minutes",
            "abc",
            "--server",
            "http://127.0.0.1:1",
            "--",
            "true",
        ][..],
        &[
            "hold",
            "--scope",
            "OnixResearch/kiln docs",
            "--tool",
            "codex",
            "--session",
            "s1",
            "--purpose",
            "p1",
            "--bogus",
            "1",
            "--server",
            "http://127.0.0.1:1",
            "--",
            "true",
        ][..],
    ] {
        let output = kiln_fleet(arguments)?;
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        let err = json(&output.stderr)?;
        assert!(err.get("error").is_some(), "{arguments:?}");
        assert!(err.get("usage").is_some(), "{arguments:?}");
    }

    // `--help` after `--` passes verbatim to the wrapped command.
    let directory = tempfile::tempdir()?;
    let (_service, server, _handles) = spawn_loopback_service(directory.path())?;
    let passthrough = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln docs",
        "--tool",
        "codex",
        "--session",
        "help-pass",
        "--purpose",
        "pass --help to child",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "printf '%s\\n' \"$1\"",
        "_",
        "--help",
    ])?;
    assert_eq!(passthrough.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&passthrough.stdout), "--help\n");
    Ok(())
}

#[test]
fn hold_exit_zero_nonzero_and_conflict_against_loopback_service() -> TestResult {
    use kiln_core::fleet::{ClaimEffect, ClaimState, MirrorStatus};

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;

    // AC 1: exit 0 releases claim once, hold exits 0, mirror queues one PostRelease,
    // default lease is 15 minutes, attempt is None.
    let zero = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "codex",
        "--session",
        "hold-zero",
        "--purpose",
        "verify zero exit",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "printf 'child-stdout\\n'",
    ])?;
    assert_eq!(
        zero.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&zero.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&zero.stdout), "child-stdout\n");

    let mut store = service.store.open_coordinator()?;
    let state = store.fleet_state()?;
    assert_eq!(state.claims.claims.len(), 1);
    let zero_claim = state
        .claims
        .claims
        .values()
        .next()
        .ok_or("missing zero claim")?
        .clone();
    assert_eq!(zero_claim.attempt, None);
    assert_eq!(zero_claim.expires_ms - zero_claim.granted_ms, 15 * 60_000);
    assert!(matches!(
        &zero_claim.state,
        ClaimState::Released { note, .. } if note == "exit 0"
    ));
    assert_eq!(zero_claim.release_mirror, MirrorStatus::Pending);
    let pending = store.fleet_pending_mirror(common::COORDINATOR)?;
    let release_effects: Vec<_> = pending
        .iter()
        .filter(|effect| matches!(effect, ClaimEffect::PostRelease { identity, .. } if identity == &zero_claim.identity))
        .collect();
    assert!(
        release_effects.is_empty(),
        "release waits for the claim receipt"
    );
    drop(store);

    // AC 2: non-zero exit releases claim with exit code in note, hold exits with same code.
    let nonzero = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "codex",
        "--session",
        "hold-nonzero",
        "--purpose",
        "verify nonzero exit",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "exit 7",
    ])?;
    assert_eq!(nonzero.status.code(), Some(7));
    let state = service.store.open_coordinator()?.fleet_state()?;
    let nonzero_claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-nonzero")
        .ok_or("missing nonzero claim")?;
    assert!(matches!(
        &nonzero_claim.state,
        ClaimState::Released { note, .. } if note == "exit 7"
    ));

    // Spawn failure releases claim cleanly and exits 1.
    let spawn_fail = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "codex",
        "--session",
        "hold-spawn-fail",
        "--purpose",
        "verify spawn fail release",
        "--server",
        &server,
        "--",
        "/nonexistent-kiln-fleet-hold-binary-xyz",
    ])?;
    assert_eq!(spawn_fail.status.code(), Some(1));
    let state = service.store.open_coordinator()?.fleet_state()?;
    let spawn_fail_claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-spawn-fail")
        .ok_or("missing spawn-fail claim")?;
    assert!(matches!(
        &spawn_fail_claim.state,
        ClaimState::Released { note, .. } if note == "spawn failed"
    ));

    // AC 3: claim conflict at start prints conflict JSON on stderr, starts no command, exits 1.
    let pre_claim = kiln_fleet(&[
        "claim",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "omp",
        "--session",
        "holder-one",
        "--purpose",
        "occupy scope",
        "--server",
        &server,
    ])?;
    assert_eq!(pre_claim.status.code(), Some(0));

    let sentinel = directory.path().join("should-not-exist");
    let sentinel_str = sentinel.to_string_lossy().into_owned();
    let conflict = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet/src",
        "--tool",
        "codex",
        "--session",
        "holder-two",
        "--purpose",
        "blocked hold",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "touch \"$1\"",
        "_",
        &sentinel_str,
    ])?;
    assert_eq!(conflict.status.code(), Some(1));
    assert!(conflict.stdout.is_empty());
    assert!(
        !sentinel.exists(),
        "command must not start on claim conflict"
    );
    let conflict_json = json(&conflict.stderr)?;
    assert_eq!(conflict_json.get("status"), Some(&Value::from(409)));
    assert_eq!(
        conflict_json.pointer("/response/code"),
        Some(&Value::from("fleet_claim_conflict"))
    );

    // Unreachable coordinator at acquire starts no command and exits 1.
    let unreachable_sentinel = directory.path().join("unreachable-should-not-exist");
    let unreachable_str = unreachable_sentinel.to_string_lossy().into_owned();
    let unreachable = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln docs",
        "--tool",
        "codex",
        "--session",
        "unreachable-acquire",
        "--purpose",
        "unreachable",
        "--server",
        "http://127.0.0.1:1",
        "--",
        "/bin/sh",
        "-c",
        "touch \"$1\"",
        "_",
        &unreachable_str,
    ])?;
    assert_eq!(unreachable.status.code(), Some(1));
    assert!(!unreachable_sentinel.exists());
    assert!(json(&unreachable.stderr)?.get("error").is_some());
    Ok(())
}

#[test]
fn hold_sigint_and_sigterm_release_claim_kill_group_and_exit_130_143() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;

    for (index, (signal, expected_code, expected_note)) in [
        (Signal::SIGINT, 130, "signal SIGINT"),
        (Signal::SIGTERM, 143, "signal SIGTERM"),
    ]
    .into_iter()
    .enumerate()
    {
        let marker = directory.path().join(format!("ready-{index}"));
        let grandchild_pid_file = directory.path().join(format!("grandchild-{index}.pid"));
        let session = format!("hold-sig-{index}");
        let mut child = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
            .args([
                "hold",
                "--scope",
                "OnixResearch/kiln crates/kiln-fleet",
                "--tool",
                "codex",
                "--session",
                &session,
                "--purpose",
                "signal test",
                "--server",
                &server,
                "--",
                "/bin/sh",
                "-c",
                "sleep 60 & echo $! > \"$1\"; touch \"$2\"; wait",
                "_",
                &grandchild_pid_file.to_string_lossy(),
                &marker.to_string_lossy(),
            ])
            .env_remove("KILN_FLEET_SERVER")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        wait_for_file(&marker, Duration::from_secs(5))?;
        let grandchild_raw: i32 =
            wait_for_nonempty_file(&grandchild_pid_file, Duration::from_secs(5))?
                .trim()
                .parse()?;

        let hold_pid = Pid::from_raw(i32::try_from(child.id())?);
        kill(hold_pid, Some(signal))?;

        let status = wait_for_child(&mut child, Duration::from_secs(5))?;
        assert_eq!(status.code(), Some(expected_code), "{session}");

        let state = service.store.open_coordinator()?.fleet_state()?;
        let claim = state
            .claims
            .claims
            .values()
            .find(|claim| claim.holder.session == session)
            .ok_or("missing signal claim")?;
        assert!(matches!(
            &claim.state,
            ClaimState::Released { note, .. } if note == expected_note
        ));
        assert!(
            state
                .claims
                .live(kiln_fleet::log::now_ms())
                .next()
                .is_none()
        );

        let grandchild_pid = Pid::from_raw(grandchild_raw);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if matches!(kill(grandchild_pid, None), Err(Errno::ESRCH)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            matches!(kill(grandchild_pid, None), Err(Errno::ESRCH)),
            "grandchild {grandchild_raw} survived {expected_note}"
        );
    }

    // Trapped SIGTERM escalates to SIGKILL after bounded grace and still releases + exits 143.
    let trap_marker = directory.path().join("ready-trap");
    let mut trapped = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args([
            "hold",
            "--scope",
            "OnixResearch/kiln crates/kiln-fleet",
            "--tool",
            "codex",
            "--session",
            "hold-trap-term",
            "--purpose",
            "trapped term test",
            "--server",
            &server,
            "--",
            "/bin/sh",
            "-c",
            "trap '' TERM; touch \"$1\"; while true; do sleep 1; done",
            "_",
            &trap_marker.to_string_lossy(),
        ])
        .env_remove("KILN_FLEET_SERVER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    wait_for_file(&trap_marker, Duration::from_secs(5))?;
    kill(
        Pid::from_raw(i32::try_from(trapped.id())?),
        Some(Signal::SIGTERM),
    )?;
    let trapped_status = wait_for_child(&mut trapped, Duration::from_secs(6))?;
    assert_eq!(trapped_status.code(), Some(143));
    let state = service.store.open_coordinator()?.fleet_state()?;
    let trapped_claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-trap-term")
        .ok_or("missing trapped claim")?;
    assert!(matches!(
        &trapped_claim.state,
        ClaimState::Released { note, .. } if note == "signal SIGTERM"
    ));
    Ok(())
}

#[test]
fn hold_sigkill_leaves_claim_open_until_sweep_queues_release_mirror() -> TestResult {
    use kiln_core::fleet::{ClaimEffect, ClaimState, MirrorStatus};
    use kiln_fleet::dispatch::Dispatcher;
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;
    let child_pid_file = directory.path().join("sigkill-child.pid");

    let mut hold_proc = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args([
            "hold",
            "--scope",
            "OnixResearch/kiln crates/kiln-fleet",
            "--tool",
            "codex",
            "--session",
            "hold-sigkill",
            "--purpose",
            "sigkill expiry test",
            "--lease-minutes",
            "1",
            "--server",
            &server,
            "--",
            "/bin/sh",
            "-c",
            "echo $$ > \"$1\"; sleep 60",
            "_",
            &child_pid_file.to_string_lossy(),
        ])
        .env_remove("KILN_FLEET_SERVER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let child_raw: i32 = wait_for_nonempty_file(&child_pid_file, Duration::from_secs(5))?
        .trim()
        .parse()?;

    kill(
        Pid::from_raw(i32::try_from(hold_proc.id())?),
        Some(Signal::SIGKILL),
    )?;
    let _ = wait_for_child(&mut hold_proc, Duration::from_secs(5))?;
    // Clean up the wrapped child that survived SIGKILL of hold.
    let _ = kill(Pid::from_raw(child_raw), Some(Signal::SIGKILL));

    let state_before = service.store.open_coordinator()?.fleet_state()?;
    let claim = state_before
        .claims
        .claims
        .values()
        .next()
        .ok_or("missing claim after sigkill")?
        .clone();
    assert_eq!(claim.state, ClaimState::Open);
    assert_eq!(claim.expires_ms - claim.granted_ms, 60_000);

    let mut dispatcher = Dispatcher::new(Arc::clone(&service))?;
    dispatcher.tick(claim.expires_ms)?;

    let mut store = service.store.open_coordinator()?;
    let state_after = store.fleet_state()?;
    let expired_claim = state_after
        .claims
        .claims
        .get(&claim.identity)
        .ok_or("missing expired claim")?;
    assert!(matches!(expired_claim.state, ClaimState::Expired { .. }));
    assert_eq!(expired_claim.release_mirror, MirrorStatus::Pending);

    let pending = store.fleet_pending_mirror(common::COORDINATOR)?;
    assert!(!pending.iter().any(|effect| matches!(
        effect,
        ClaimEffect::PostRelease { identity, .. } if identity == &claim.identity
    )));
    Ok(())
}

#[test]
fn hold_refuses_existing_same_holder_claim_on_cli() -> TestResult {
    use kiln_core::fleet::ClaimState;

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;
    let ready = directory.path().join("ready");
    let stop_file = directory.path().join("stop");
    let sentinel = directory.path().join("duplicate-cli-should-not-exist");

    let mut first = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args([
            "hold",
            "--scope",
            "OnixResearch/kiln crates/kiln-fleet",
            "--tool",
            "codex",
            "--session",
            "same-cli-session",
            "--purpose",
            "first cli hold",
            "--server",
            &server,
            "--",
            "/bin/sh",
            "-c",
            "touch \"$1\"; i=0; while [ ! -f \"$2\" ] && [ \"$i\" -lt 500 ]; do sleep 0.02; i=$((i + 1)); done",
            "_",
            &ready.to_string_lossy(),
            &stop_file.to_string_lossy(),
        ])
        .env_remove("KILN_FLEET_SERVER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    wait_for_file(&ready, Duration::from_secs(5))?;

    let second = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "codex",
        "--session",
        "same-cli-session",
        "--purpose",
        "duplicate cli hold",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "touch \"$1\"",
        "_",
        &sentinel.to_string_lossy(),
    ])?;

    let state_during = service.store.open_coordinator()?.fleet_state()?;
    std::fs::write(&stop_file, b"done")?;
    let first_status = wait_for_child(&mut first, Duration::from_secs(5))?;

    assert_eq!(second.status.code(), Some(1));
    assert!(second.stdout.is_empty());
    assert!(
        !sentinel.exists(),
        "command must not start on existing same-holder claim"
    );
    let err = json(&second.stderr)?;
    assert_eq!(err.get("status"), Some(&Value::from(200)));
    assert_eq!(
        err.pointer("/response/outcome"),
        Some(&Value::from("existing"))
    );

    assert_eq!(state_during.claims.claims.len(), 1);
    let during_claim = state_during
        .claims
        .claims
        .values()
        .next()
        .ok_or("missing claim during duplicate refusal")?;
    assert_eq!(during_claim.state, ClaimState::Open);

    assert_eq!(first_status.code(), Some(0));
    let state_after = service.store.open_coordinator()?.fleet_state()?;
    let final_claim = state_after
        .claims
        .claims
        .get(&during_claim.identity)
        .ok_or("missing claim after first exit")?;
    assert!(matches!(
        &final_claim.state,
        ClaimState::Released { note, .. } if note == "exit 0"
    ));
    Ok(())
}

#[test]
fn hold_child_killed_by_signal_exits_128_plus_sig_and_releases_claim() -> TestResult {
    use kiln_core::fleet::{ClaimEffect, ClaimState, MirrorStatus};
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;

    // Case 1: direct child killed by SIGTERM (self-signal) -> 128 + 15 = 143, note "signal 15".
    let sigterm = kiln_fleet(&[
        "hold",
        "--scope",
        "OnixResearch/kiln crates/kiln-fleet",
        "--tool",
        "codex",
        "--session",
        "hold-child-sigterm",
        "--purpose",
        "verify child sigterm exit",
        "--server",
        &server,
        "--",
        "/bin/sh",
        "-c",
        "kill -TERM $$",
    ])?;
    assert_eq!(
        sigterm.status.code(),
        Some(143),
        "{}",
        String::from_utf8_lossy(&sigterm.stderr)
    );
    let mut store = service.store.open_coordinator()?;
    let state = store.fleet_state()?;
    let term_claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-child-sigterm")
        .ok_or("missing child-sigterm claim")?
        .clone();
    assert!(matches!(
        &term_claim.state,
        ClaimState::Released { note, .. } if note == "signal 15"
    ));
    assert_eq!(term_claim.release_mirror, MirrorStatus::Pending);
    let pending = store.fleet_pending_mirror(common::COORDINATOR)?;
    assert!(!pending.iter().any(|effect| matches!(
        effect,
        ClaimEffect::PostRelease { identity, .. } if identity == &term_claim.identity
    )));
    drop(store);

    // Case 2: direct child killed externally by SIGKILL -> 128 + 9 = 137, note "signal 9".
    // `exec sleep 60` replaces `sh` so SIGKILL terminates the direct child itself.
    let marker = directory.path().join("ready-child-sigkill");
    let pid_file = directory.path().join("child-sigkill.pid");
    let mut hold_proc = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args([
            "hold",
            "--scope",
            "OnixResearch/kiln crates/kiln-fleet",
            "--tool",
            "codex",
            "--session",
            "hold-child-sigkill",
            "--purpose",
            "verify child sigkill exit",
            "--server",
            &server,
            "--",
            "/bin/sh",
            "-c",
            "echo $$ > \"$1\"; touch \"$2\"; exec sleep 60",
            "_",
            &pid_file.to_string_lossy(),
            &marker.to_string_lossy(),
        ])
        .env_remove("KILN_FLEET_SERVER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    wait_for_file(&marker, Duration::from_secs(5))?;
    let child_raw: i32 = wait_for_nonempty_file(&pid_file, Duration::from_secs(5))?
        .trim()
        .parse()?;
    kill(Pid::from_raw(child_raw), Some(Signal::SIGKILL))?;

    let status = wait_for_child(&mut hold_proc, Duration::from_secs(5))?;
    assert_eq!(status.code(), Some(137));

    let mut store = service.store.open_coordinator()?;
    let state = store.fleet_state()?;
    let kill_claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-child-sigkill")
        .ok_or("missing child-sigkill claim")?
        .clone();
    assert!(matches!(
        &kill_claim.state,
        ClaimState::Released { note, .. } if note == "signal 9"
    ));
    assert_eq!(kill_claim.release_mirror, MirrorStatus::Pending);
    let pending = store.fleet_pending_mirror(common::COORDINATOR)?;
    assert!(!pending.iter().any(|effect| matches!(
        effect,
        ClaimEffect::PostRelease { identity, .. } if identity == &kill_claim.identity
    )));
    Ok(())
}

#[test]
fn hold_sigterm_kills_term_ignoring_grandchild_when_direct_child_exits() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let directory = tempfile::tempdir()?;
    let (service, server, _handles) = spawn_loopback_service(directory.path())?;
    let marker = directory.path().join("ready-grandchild");
    let pid_file = directory.path().join("grandchild-ignore-term.pid");

    let mut hold_proc = Command::new(env!("CARGO_BIN_EXE_kiln-fleet"))
        .args([
            "hold",
            "--scope",
            "OnixResearch/kiln crates/kiln-fleet",
            "--tool",
            "codex",
            "--session",
            "hold-grandchild-ignore-term",
            "--purpose",
            "verify SIGTERM-ignoring grandchild cleanup",
            "--server",
            &server,
            "--",
            "/bin/sh",
            "-c",
            "sh -c 'trap \"\" TERM; touch \"$1\"; exec sleep 60' _ \"$2\" & echo $! > \"$1\"; wait",
            "_",
            &pid_file.to_string_lossy(),
            &marker.to_string_lossy(),
        ])
        .env_remove("KILN_FLEET_SERVER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    wait_for_file(&marker, Duration::from_secs(5))?;
    let grandchild_raw: i32 = wait_for_nonempty_file(&pid_file, Duration::from_secs(5))?
        .trim()
        .parse()?;

    let started = Instant::now();
    kill(
        Pid::from_raw(i32::try_from(hold_proc.id())?),
        Some(Signal::SIGTERM),
    )?;
    let status = wait_for_child(&mut hold_proc, Duration::from_secs(5))?;
    assert_eq!(status.code(), Some(143));
    assert!(
        started.elapsed() < Duration::from_millis(1_500),
        "expected immediate grandchild SIGKILL without waiting full STOP_GRACE_MS"
    );

    let grandchild_pid = Pid::from_raw(grandchild_raw);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if matches!(kill(grandchild_pid, None), Err(Errno::ESRCH)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        matches!(kill(grandchild_pid, None), Err(Errno::ESRCH)),
        "SIGTERM-ignoring grandchild {grandchild_raw} survived direct child exit"
    );

    let state = service.store.open_coordinator()?.fleet_state()?;
    let claim = state
        .claims
        .claims
        .values()
        .find(|claim| claim.holder.session == "hold-grandchild-ignore-term")
        .ok_or("missing grandchild-ignore-term claim")?;
    assert!(matches!(
        &claim.state,
        ClaimState::Released { note, .. } if note == "signal SIGTERM"
    ));
    Ok(())
}
