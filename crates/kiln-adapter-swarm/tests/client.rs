#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use kiln_adapter_swarm::{PostOutcome, SwarmClient, SwarmConfig};

const SCRIPT: &str = r#"#!/bin/sh
state=""
prev=""
for a in "$@"; do
  if [ "$prev" = "--state-dir" ]; then state="$a"; fi
  prev="$a"
done
cmd=""
for a in "$@"; do
  case "$a" in
    read|post) cmd="$a" ;;
  esac
done
if [ -f "$state/spin" ]; then
  while :; do :; done
fi
if [ "$cmd" = "post" ]; then
  while IFS= read -r line; do :; done
  if [ -f "$state/reject" ]; then
    echo "441 posting rate limit exceeded" >&2
    exit 3
  fi
  exit 0
fi
if [ "$cmd" = "read" ]; then
  if [ -f "$state/read-fail" ]; then
    echo "503 service unavailable" >&2
    exit 1
  fi
  if [ -f "$state/big" ]; then
    i=0
    while [ $i -lt 2000 ]; do printf 'x'; i=$((i+1)); done
    exit 0
  fi
  while IFS= read -r line; do printf '%s\n' "$line"; done < "$state/read.json"
  exit 0
fi
exit 2
"#;

fn client(dir: &Path) -> SwarmClient {
    let executable = dir.join("iroh-news");
    fs::write(&executable, SCRIPT).unwrap_or_else(|_| unreachable!());
    let mut permissions = fs::metadata(&executable)
        .unwrap_or_else(|_| unreachable!())
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap_or_else(|_| unreachable!());
    SwarmClient::new(config(executable, dir.to_path_buf())).unwrap_or_else(|_| unreachable!())
}

fn config(executable: PathBuf, state_dir: PathBuf) -> SwarmConfig {
    SwarmConfig {
        enabled: true,
        executable,
        state_dir,
        config_file: None,
        server: Some("swarm.blr.dev".to_owned()),
        group: "bots.claims".to_owned(),
        from: "kiln-aspen1 <kiln-aspen1@swarm.local>".to_owned(),
        timeout_ms: 150,
        read_limit: 50,
        max_output_bytes: 65_536,
        no_relays: true,
        environment: std::collections::BTreeMap::new(),
    }
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.bounded]
fn read_returns_bounded_articles() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    fs::write(
        state.join("read.json"),
        r#"[
          {"number":1,"message_id":"<claim@aspen1>","from":"omp <aspen3@swarm.local>","subject":"CLAIM OnixResearch/kiln crates/kiln-core","date":"Wed, 18 Nov 2026 23:01:00 +0000","references":"","body":"ignored"},
          {"number":2,"message_id":"<release@aspen1>","from":"omp <aspen3@swarm.local>","subject":"RELEASE OnixResearch/kiln crates/kiln-core","date":"Wed, 18 Nov 2026 23:10:00 +0000","references":"<claim@aspen1>","body":"ignored"}
        ]
"#,
    )
    .unwrap_or_else(|_| unreachable!());
    let articles = client
        .read(None, 10)
        .unwrap_or_else(|failure| unreachable!("{failure:?}"));
    assert_eq!(articles.len(), 2);
    let view = articles
        .first()
        .and_then(|article| article.view())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(view.date_unix_ms, 1_795_042_860_000);
    assert!(articles.get(1).is_some_and(|article| {
        article
            .view()
            .is_some_and(|view| view.references == vec!["<claim@aspen1>".to_owned()])
    }));
}

#[test]
// r[verify kiln.implementation.swarm.outcomes.scenario.unavailable]
fn read_failure_is_unavailable_not_empty() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    fs::write(state.join("read-fail"), "1").unwrap_or_else(|_| unreachable!());
    let error = client
        .read(None, 10)
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert!(error.detail.contains("503"));
}

#[test]
// r[verify kiln.implementation.swarm.outcomes.scenario.classify]
fn post_classifies_accepted_rejected_and_unknown() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    let subject = "CLAIM OnixResearch/kiln crates/kiln-core";
    let body =
        "Claiming OnixResearch/kiln crates/kiln-core.\n\n— kiln on aspen1, attempt attempt-a\n";
    let accepted = client.post(subject, None, body);
    assert_eq!(accepted.outcome, PostOutcome::Accepted);
    assert_eq!(accepted.detail, None);

    fs::write(state.join("reject"), "1").unwrap_or_else(|_| unreachable!());
    let rejected = client.post(subject, None, body);
    assert_eq!(rejected.outcome, PostOutcome::Rejected);
    assert!(
        rejected
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("441"))
    );

    fs::remove_file(state.join("reject")).unwrap_or_else(|_| unreachable!());
    fs::write(state.join("spin"), "1").unwrap_or_else(|_| unreachable!());
    let unknown = client.post(subject, None, body);
    assert_eq!(unknown.outcome, PostOutcome::Unknown);
}

#[test]
fn post_refuses_invalid_body_and_spawn_failure() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    let refused = client.post(
        "CLAIM OnixResearch/kiln crates/kiln-core",
        None,
        "token SECRETSPEC_GITHUB\n",
    );
    assert_eq!(refused.outcome, PostOutcome::Rejected);
    assert_eq!(
        refused.detail.as_deref(),
        Some("swarm body contains a secret reference")
    );

    let missing = SwarmClient::new(config(state.join("missing"), state.to_path_buf()))
        .unwrap_or_else(|_| unreachable!());
    let unavailable = missing.post(
        "CLAIM OnixResearch/kiln crates/kiln-core",
        None,
        "Claiming OnixResearch/kiln crates/kiln-core.\n",
    );
    assert_eq!(unavailable.outcome, PostOutcome::Unavailable);
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.bounded]
fn oversized_read_fails_closed() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let mut configured = config(state.join("iroh-news"), state.to_path_buf());
    configured.max_output_bytes = 1_024;
    let executable = state.join("iroh-news");
    std::fs::write(&executable, SCRIPT).unwrap_or_else(|_| unreachable!());
    let mut permissions = std::fs::metadata(&executable)
        .unwrap_or_else(|_| unreachable!())
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap_or_else(|_| unreachable!());
    let client = SwarmClient::new(configured).unwrap_or_else(|_| unreachable!());
    std::fs::write(state.join("big"), "1").unwrap_or_else(|_| unreachable!());
    assert!(client.read(None, 10).is_err());
}
