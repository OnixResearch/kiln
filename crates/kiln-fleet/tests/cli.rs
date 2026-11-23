#![cfg(unix)]
//! Exit codes of the `kiln-fleet` binary: 0 with JSON on stdout, 1 for
//! refusals and HTTP errors with JSON on stderr, 2 for usage.

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

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
