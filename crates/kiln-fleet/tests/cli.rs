#![cfg(unix)]
//! Exit codes of the `kiln-fleet` binary: 0 with JSON on stdout, 1 for
//! refusals and HTTP errors with JSON on stderr, 2 for usage.

mod common;

use std::path::Path;
use std::process::{Command, Output};
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
