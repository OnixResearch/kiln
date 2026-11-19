#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

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
    read|post|groups) cmd="$a" ;;
  esac
done
printf '%s\n' "$@" > "$state/argv-$cmd"
printf '%s\n' "${IROH_NEWS_FROM-unset}" > "$state/from-$cmd"
printf '%s\n' "${HOME-unset}" > "$state/home-$cmd"
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
if [ "$cmd" = "groups" ]; then
  if [ -f "$state/groups-fail" ]; then
    echo "503 service unavailable" >&2
    exit 1
  fi
  while IFS= read -r line; do printf '%s\n' "$line"; done < "$state/groups.json"
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

/// One fake executable per test process, written before any test spawns it:
/// writing an executable while another test thread forks races into
/// `ETXTBSY` when that executable is then started. The script finds each
/// test's state through `--state-dir`.
static EXECUTABLE: LazyLock<PathBuf> = LazyLock::new(|| {
    let executable = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-adapter-swarm-fake-{}", std::process::id()));
    fs::write(&executable, SCRIPT).unwrap_or_else(|_| unreachable!());
    let mut permissions = fs::metadata(&executable)
        .unwrap_or_else(|_| unreachable!())
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap_or_else(|_| unreachable!());
    executable
});

fn client(dir: &Path) -> SwarmClient {
    SwarmClient::new(config(EXECUTABLE.clone(), dir.to_path_buf()))
        .unwrap_or_else(|_| unreachable!())
}

fn config(executable: PathBuf, state_dir: PathBuf) -> SwarmConfig {
    SwarmConfig {
        enabled: true,
        executable,
        state_dir,
        config_file: None,
        server: Some("swarm.blr.dev".to_owned()),
        group: "bots.claims".to_owned(),
        from: FROM.to_owned(),
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
    let mut configured = config(EXECUTABLE.clone(), state.to_path_buf());
    configured.max_output_bytes = 1_024;
    let client = SwarmClient::new(configured).unwrap_or_else(|_| unreachable!());
    std::fs::write(state.join("big"), "1").unwrap_or_else(|_| unreachable!());
    assert!(client.read(None, 10).is_err());
}

const FROM: &str = "kiln-aspen1 <kiln-aspen1@swarm.local>";

fn groups(state: &Path, high: u64) {
    fs::write(
        state.join("groups.json"),
        format!(
            r#"[{{"name":"bots.general","high":900,"low":1,"posting":"y"}},{{"name":"bots.claims","high":{high},"low":1,"posting":"y","description":"claims","creator":"britton"}}]
"#
        ),
    )
    .unwrap_or_else(|_| unreachable!());
    fs::write(state.join("read.json"), "[]\n").unwrap_or_else(|_| unreachable!());
}

fn recorded(state: &Path, name: &str) -> Vec<String> {
    fs::read_to_string(state.join(name))
        .unwrap_or_else(|_| unreachable!("{name} must be recorded"))
        .lines()
        .map(str::to_owned)
        .collect()
}

fn flag(argv: &[String], name: &str) -> Option<String> {
    argv.iter()
        .position(|value| value == name)
        .and_then(|index| argv.get(index.saturating_add(1)))
        .cloned()
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn read_recent_starts_at_high_water_mark_minus_limit() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    groups(state, 120);
    let articles = client
        .read_recent(20)
        .unwrap_or_else(|failure| unreachable!("{failure:?}"));
    assert!(articles.is_empty());
    let argv = recorded(state, "argv-read");
    assert!(argv.iter().any(|value| value == "bots.claims"));
    assert_eq!(flag(&argv, "--since").as_deref(), Some("100"));
    assert_eq!(flag(&argv, "--limit").as_deref(), Some("20"));
    assert_eq!(
        recorded(state, "argv-groups").last().map(String::as_str),
        Some("groups")
    );
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn read_recent_below_limit_starts_at_zero_and_clamps_limit() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    groups(state, 5);
    client
        .read_recent(10)
        .unwrap_or_else(|failure| unreachable!("{failure:?}"));
    let argv = recorded(state, "argv-read");
    assert_eq!(flag(&argv, "--since").as_deref(), Some("0"));
    assert_eq!(flag(&argv, "--limit").as_deref(), Some("10"));

    groups(state, 500);
    client
        .read_recent(10_000)
        .unwrap_or_else(|failure| unreachable!("{failure:?}"));
    let argv = recorded(state, "argv-read");
    assert_eq!(flag(&argv, "--since").as_deref(), Some("450"));
    assert_eq!(flag(&argv, "--limit").as_deref(), Some("50"));
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn read_recent_groups_failure_is_a_read_failure() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    groups(state, 120);
    fs::write(state.join("groups-fail"), "1").unwrap_or_else(|_| unreachable!());
    let error = client
        .read_recent(20)
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert!(error.detail.contains("503"));
    assert!(!state.join("argv-read").exists());

    fs::remove_file(state.join("groups-fail")).unwrap_or_else(|_| unreachable!());
    fs::write(state.join("groups.json"), "not json\n").unwrap_or_else(|_| unreachable!());
    let malformed = client
        .read_recent(20)
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert!(malformed.detail.contains("group array"));
    assert!(!state.join("argv-read").exists());
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn read_recent_missing_group_is_a_read_failure() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    fs::write(
        state.join("groups.json"),
        r#"[{"name":"bots.general","high":900,"low":1,"posting":"y"}]
"#,
    )
    .unwrap_or_else(|_| unreachable!());
    let error = client
        .read_recent(20)
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert_eq!(error.detail, "swarm group is not listed by the server");
    assert!(!state.join("argv-read").exists());
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn every_child_runs_with_the_configured_from() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let client = client(state);
    let receipt = client.post(
        "CLAIM OnixResearch/kiln crates/kiln-core",
        None,
        "Claiming OnixResearch/kiln crates/kiln-core.\n",
    );
    assert_eq!(receipt.outcome, PostOutcome::Accepted);
    assert_eq!(recorded(state, "from-post"), vec![FROM.to_owned()]);
    assert_eq!(recorded(state, "home-post"), vec!["unset".to_owned()]);

    groups(state, 3);
    client
        .read_recent(5)
        .unwrap_or_else(|failure| unreachable!("{failure:?}"));
    assert_eq!(recorded(state, "from-groups"), vec![FROM.to_owned()]);
    assert_eq!(recorded(state, "from-read"), vec![FROM.to_owned()]);
}

#[test]
// r[verify kiln.implementation.swarm.transport.scenario.window]
fn conflicting_from_environment_is_refused() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let state = directory.path();
    let mut conflicting = config(state.join("iroh-news"), state.to_path_buf());
    conflicting.environment.insert(
        "IROH_NEWS_FROM".to_owned(),
        "forged <forged@swarm.local>".to_owned(),
    );
    assert!(SwarmClient::new(conflicting).is_err());
}
