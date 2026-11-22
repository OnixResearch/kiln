//! Shared assertions for binaries that serve the reserved GitHub DNS helper
//! entry. Included by each crate's integration test with `#[path]`.

use kiln_adapter_github::status::{
    DNS_CHILD_ARGUMENT, DNS_CHILD_FAILURE, DNS_CHILD_FAILURE_EXIT, DNS_CHILD_USAGE_EXIT,
};
use std::net::SocketAddr;
use std::process::{Command, Output};

const HTTPS_PORT: u16 = 443;

fn helper(program: &str, arguments: &[&str]) -> Output {
    Command::new(program)
        .args(arguments)
        .env_clear()
        .output()
        .unwrap_or_else(|_| unreachable!("binary under test starts"))
}

/// The helper either resolved (addresses on stdout) or reported its own
/// resolution failure. Offline hosts take the second branch.
pub fn assert_served(program: &str) {
    let parent = std::process::id().to_string();
    let output = helper(program, &[DNS_CHILD_ARGUMENT, &parent]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match output.status.code() {
        Some(0) => {
            assert!(stderr.is_empty(), "{program}: {stderr}");
            assert!(!stdout.trim().is_empty(), "{program}: no addresses");
            for line in stdout.lines() {
                let address: SocketAddr = line
                    .parse()
                    .unwrap_or_else(|_| unreachable!("{program}: invalid address {line}"));
                assert_eq!(address.port(), HTTPS_PORT, "{program}: {line}");
            }
        }
        Some(DNS_CHILD_FAILURE_EXIT) => {
            assert!(stdout.is_empty(), "{program}: {stdout}");
            assert!(
                stderr.starts_with(DNS_CHILD_FAILURE) && stderr.contains("api.github.com"),
                "{program}: {stderr}"
            );
        }
        other => unreachable!("{program}: entry not served ({other:?}): {stderr}"),
    }
}

pub fn assert_malformed_entry_refused(program: &str) {
    let parent = std::process::id().to_string();
    for arguments in [
        vec![DNS_CHILD_ARGUMENT],
        vec![DNS_CHILD_ARGUMENT, "not-a-pid"],
        vec![DNS_CHILD_ARGUMENT, parent.as_str(), "extra"],
    ] {
        let output = helper(program, &arguments);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(DNS_CHILD_USAGE_EXIT),
            "{program} {arguments:?}: {stderr}"
        );
        assert!(output.stdout.is_empty());
        assert!(stderr.starts_with(DNS_CHILD_FAILURE), "{program}: {stderr}");
    }
    // A PID that is not the helper's parent is refused before any lookup.
    let output = helper(program, &[DNS_CHILD_ARGUMENT, "1"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(DNS_CHILD_FAILURE_EXIT));
    assert!(output.stdout.is_empty());
    assert!(
        stderr.contains("not this helper's parent"),
        "{program}: {stderr}"
    );
}
