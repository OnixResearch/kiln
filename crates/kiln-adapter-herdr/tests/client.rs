#![cfg(unix)]
//! Conformance tests against a scripted `herdr`.
//!
//! The fake reads its script from `$HOME`, which the client sets from the
//! inventory: `<target>.<group>-<command>.out` is printed on stdout with exit
//! 0, `.err` on stderr with exit 1, `.usage` on stderr with exit 2, and
//! `.spin` never returns. `<target>.down` and `<target>.unknown` make the
//! whole target unavailable. Every argv is appended to `argv.log`.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::thread;
use std::time::Duration;

use kiln_adapter_herdr::{
    AgentKind, AgentObservation, AgentSlots, AgentStatus, FailureClass, HerdrClient,
    HerdrEnvironment, HerdrFailure, HerdrInventory, HerdrMachine, INVENTORY_SCHEMA, MachineLoad,
    Placement, place,
};
use kiln_core::ContentId;

const SCRIPT: &str = r#"#!/bin/sh
state="$HOME"
printf '%s\n' "$*" >> "$state/argv.log"
printf 'HOME=%s PATH=%s XDG_RUNTIME_DIR=%s CARGO=%s\n' "$HOME" "$PATH" "${XDG_RUNTIME_DIR-unset}" "${CARGO-unset}" > "$state/env.log"
emit() { while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$1"; }
case "$1" in
  --machine) target="machine-$2" ;;
  --session) target="session-$2" ;;
  *) echo "missing selector" >&2; exit 2 ;;
esac
shift 2
if [ -f "$state/$target.unknown" ]; then
  echo "error: unknown machine '${target#machine-}'; use \`herdr machine list\`" >&2
  exit 2
fi
if [ -f "$state/$target.down" ]; then
  echo '{"id":"cli","error":{"code":"server_not_running","message":"no herdr server is running"}}' >&2
  exit 1
fi
key="$target.$1-$2"
if [ -f "$state/$key.spin" ]; then while :; do :; done; fi
if [ -f "$state/$key.usage" ]; then emit "$state/$key.usage" >&2; exit 2; fi
if [ -f "$state/$key.err" ]; then emit "$state/$key.err" >&2; exit 1; fi
if [ -f "$state/$key.out" ]; then emit "$state/$key.out"; exit 0; fi
echo "{\"id\":\"cli\",\"error\":{\"code\":\"unscripted\",\"message\":\"no script for $key\"}}" >&2
exit 1
"#;

const LOCAL: &str = "session-kiln-test";
const REMOTE: &str = "machine-aspen3-saved";
const NAME: &str = "kiln-0123456789ab";
const PANE: &str = "w2:p1";
const BASE: &str = "96274e6fc6080feee886ff797565ffafdab20578";
const BRANCH: &str = "kiln/impl-aaaaaaaaaaaa-0123456789ab";
const AGENT_PATH: &str = "/kiln-test/bin";

struct Fake {
    _directory: tempfile::TempDir,
    home: PathBuf,
}

/// One fake executable per test process, written before any test spawns it:
/// writing an executable while another test thread forks races into
/// `ETXTBSY` when that executable is then started.
static EXECUTABLE: LazyLock<PathBuf> = LazyLock::new(|| {
    let executable = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-adapter-herdr-fake-{}", std::process::id()));
    fs::write(&executable, SCRIPT).unwrap_or_else(|_| unreachable!());
    let mut permissions = fs::metadata(&executable)
        .unwrap_or_else(|_| unreachable!())
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap_or_else(|_| unreachable!());
    executable
});

impl Fake {
    fn new() -> Self {
        LazyLock::force(&EXECUTABLE);
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let home = directory.path().to_path_buf();
        Self {
            _directory: directory,
            home,
        }
    }

    fn script(&self, target: &str, operation: &str, suffix: &str, content: &str) {
        fs::write(
            self.home.join(format!("{target}.{operation}.{suffix}")),
            content,
        )
        .unwrap_or_else(|_| unreachable!());
    }

    fn mark(&self, file: &str) {
        fs::write(self.home.join(file), "1").unwrap_or_else(|_| unreachable!());
    }

    fn clear(&self, target: &str, operation: &str, suffix: &str) {
        fs::remove_file(self.home.join(format!("{target}.{operation}.{suffix}")))
            .unwrap_or_else(|_| unreachable!());
    }

    fn argv(&self) -> Vec<String> {
        fs::read_to_string(self.home.join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn inventory(&self) -> HerdrInventory {
        inventory(&self.home)
    }

    fn client(&self) -> HerdrClient {
        HerdrClient::new(self.inventory()).unwrap_or_else(|error| unreachable!("{error}"))
    }
}

fn machine(label: &str, saved: Option<&str>, agents: Vec<AgentSlots>) -> HerdrMachine {
    HerdrMachine {
        label: label.to_owned(),
        herdr_machine: saved.map(str::to_owned),
        ssh_target: Some(format!("brittonr@{label}")),
        worktree_root: "/srv/kiln/worktrees/".to_owned(),
        repositories: BTreeMap::from([(
            "OnixResearch/kiln".to_owned(),
            "/srv/kiln/src/kiln".to_owned(),
        )]),
        agents,
    }
}

fn slots(kind: AgentKind, slots: u32, args: &[&str]) -> AgentSlots {
    AgentSlots {
        kind,
        slots,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    }
}

fn inventory(home: &Path) -> HerdrInventory {
    HerdrInventory {
        schema: INVENTORY_SCHEMA.to_owned(),
        executable: EXECUTABLE.clone(),
        session: "kiln-test".to_owned(),
        environment: HerdrEnvironment {
            home: home.to_string_lossy().into_owned(),
            path: AGENT_PATH.to_owned(),
            xdg_runtime_dir: Some("/run/user/1000".to_owned()),
            xdg_config_home: None,
            ssh_auth_sock: None,
        },
        timeout_ms: 400,
        start_timeout_ms: 100,
        max_output_bytes: 4_096,
        machines: vec![
            machine(
                "aspen1",
                None,
                vec![
                    slots(AgentKind::Omp, 3, &["--model", "fast"]),
                    slots(AgentKind::Claude, 1, &[]),
                ],
            ),
            machine(
                "aspen3",
                Some("aspen3-saved"),
                vec![slots(AgentKind::Omp, 1, &[])],
            ),
            machine(
                "aspen4",
                Some("aspen4-saved"),
                vec![slots(AgentKind::Omp, 1, &[])],
            ),
        ],
    }
}

fn attempt() -> ContentId {
    ContentId::from_blake3_hex("0123456789abcdef".repeat(4)).unwrap_or_else(|_| unreachable!())
}

fn agent_json(name: &str, kind: Option<&str>, status: &str, pane: &str) -> String {
    let kind = kind.map_or(String::new(), |kind| format!(r#""agent":"{kind}","#));
    let workspace = pane.split(':').next().unwrap_or("w0");
    format!(
        r#"{{{kind}"agent_status":"{status}","cwd":"/srv","focused":false,"interactive_ready":true,"name":"{name}","pane_id":"{pane}","revision":0,"state_change_seq":1,"tab_id":"{workspace}:t1","terminal_id":"term_1","workspace_id":"{workspace}"}}"#
    )
}

fn result(kind: &str, body: &str) -> String {
    format!(r#"{{"id":"cli:x","result":{{{body}"type":"{kind}"}}}}"#)
}

fn error(code: &str, message: &str) -> String {
    format!(r#"{{"error":{{"code":"{code}","message":"{message}"}},"id":"cli:x"}}"#)
}

fn failure<T: std::fmt::Debug>(outcome: Result<T, HerdrFailure>) -> HerdrFailure {
    outcome.err().unwrap_or_else(|| unreachable!("must fail"))
}

fn worktree_created(path: &str, branch: &str) -> String {
    let body = format!(
        r#""root_pane":{{"agent_status":"unknown","focused":false,"pane_id":"w2:p1","revision":0,"tab_id":"w2:t1","terminal_id":"term_2","workspace_id":"w2"}},"tab":{{"tab_id":"w2:t1"}},"workspace":{{"active_tab_id":"w2:t1","agent_status":"unknown","focused":false,"label":"kiln-0123456789ab","number":2,"pane_count":1,"tab_count":1,"workspace_id":"w2"}},"worktree":{{"branch":"{branch}","is_bare":false,"is_detached":false,"is_linked_worktree":true,"is_prunable":false,"label":"kiln","open_workspace_id":"w2","path":"{path}"}},"#
    );
    result("worktree_created", &body)
}

#[test]
// r[verify kiln.fleet.transport]
fn valid_operations_use_exact_argv_and_a_cleared_environment() {
    let fake = Fake::new();
    let client = fake.client();
    let path = "/srv/kiln/worktrees/0123456789ab";
    fake.script(
        LOCAL,
        "worktree-create",
        "out",
        &worktree_created(path, BRANCH),
    );
    fake.script(
        LOCAL,
        "agent-start",
        "out",
        &result(
            "agent_started",
            &format!(
                r#""agent":{},"argv":["omp","--model","fast"],"#,
                agent_json(NAME, Some("omp"), "idle", PANE)
            ),
        ),
    );
    fake.script(
        LOCAL,
        "agent-prompt",
        "out",
        &result(
            "agent_prompted",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("omp"), "idle", PANE)
            ),
        ),
    );
    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("omp"), "working", PANE)
            ),
        ),
    );
    fake.script(LOCAL, "agent-read", "out", "fake omp\nworking on it\n");
    fake.script(LOCAL, "agent-send-keys", "out", &result("ok", ""));
    fake.script(LOCAL, "workspace-close", "out", &result("ok", ""));
    fake.script(
        LOCAL,
        "agent-list",
        "out",
        &result(
            "agent_list",
            &format!(
                r#""agents":[{}],"#,
                agent_json(NAME, Some("omp"), "working", PANE)
            ),
        ),
    );
    fake.script(
        LOCAL,
        "worktree-list",
        "out",
        &result(
            "worktree_list",
            &format!(
                r#""source":{{"repo_root":"/srv/kiln/src/kiln"}},"worktrees":[{{"branch":"main","is_bare":false,"is_detached":false,"is_linked_worktree":false,"is_prunable":false,"label":"kiln","path":"/srv/kiln/src/kiln"}},{{"branch":"{BRANCH}","is_bare":false,"is_detached":false,"is_linked_worktree":true,"is_prunable":false,"label":"kiln","open_workspace_id":"w2","path":"{path}"}}],"#
            ),
        ),
    );

    let created = client
        .create_worktree("aspen1", "onixresearch/kiln", BRANCH, BASE, &attempt())
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    assert_eq!(created.workspace_id, "w2");
    assert_eq!(created.pane_id, PANE);
    assert_eq!(created.path, path);
    let started = client
        .start_agent("aspen1", PANE, NAME, AgentKind::Omp)
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    assert!(!started.replayed);
    assert_eq!(started.agent.admitted_kind(), Some(AgentKind::Omp));
    client
        .prompt("aspen1", NAME, "Implement the attempt.\nThen hand off.")
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    assert_eq!(
        client.status("aspen1", NAME),
        Ok(AgentObservation::Status(AgentStatus::Working))
    );
    assert_eq!(
        client.read("aspen1", NAME, 120).as_deref(),
        Ok("fake omp\nworking on it\n")
    );
    client
        .interrupt("aspen1", NAME)
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    client
        .close_workspace("aspen1", "w2")
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    client
        .probe("aspen1")
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    let worktrees = client
        .list_worktrees("aspen1", "OnixResearch/kiln")
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    assert_eq!(worktrees.len(), 2);
    assert!(worktrees.iter().any(|worktree| {
        worktree.path == path && worktree.open_workspace_id.as_deref() == Some("w2")
    }));

    assert_eq!(
        fake.argv(),
        vec![
            format!(
                "--session kiln-test worktree create --cwd /srv/kiln/src/kiln --branch {BRANCH} --base {BASE} --path {path} --label kiln-0123456789ab --no-focus"
            ),
            format!(
                "--session kiln-test agent start {NAME} --kind omp --pane {PANE} --timeout 100 -- --model fast"
            ),
            format!("--session kiln-test agent prompt {NAME} Implement the attempt."),
            "Then hand off.".to_owned(),
            format!("--session kiln-test agent get {NAME}"),
            format!("--session kiln-test agent read {NAME} --source recent-unwrapped --lines 120"),
            format!("--session kiln-test agent send-keys {NAME} ctrl+c"),
            "--session kiln-test workspace close w2".to_owned(),
            "--session kiln-test agent list".to_owned(),
            "--session kiln-test worktree list --cwd /srv/kiln/src/kiln".to_owned(),
        ]
    );
    let environment =
        fs::read_to_string(fake.home.join("env.log")).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        environment.trim_end(),
        format!(
            "HOME={} PATH={AGENT_PATH} XDG_RUNTIME_DIR=/run/user/1000 CARGO=unset",
            fake.home.display()
        )
    );
}

#[test]
// r[verify kiln.fleet.transport]
fn remote_machines_use_the_saved_machine_selector() {
    let fake = Fake::new();
    let client = fake.client();
    fake.script(
        REMOTE,
        "agent-list",
        "out",
        &result("agent_list", r#""agents":[],"#),
    );
    assert_eq!(client.list_agents("aspen3"), Ok(Vec::new()));
    assert_eq!(
        fake.argv(),
        vec!["--machine aspen3-saved agent list".to_owned()]
    );
}

#[test]
// r[verify kiln.fleet.transport]
fn occupancy_feeds_placement_and_skips_unavailable_machines() {
    let fake = Fake::new();
    let client = fake.client();
    let agents = [
        agent_json("kiln-aaaaaaaaaaaa", Some("omp"), "working", "w1:p1"),
        agent_json("kiln-bbbbbbbbbbbb", Some("omp"), "idle", "w2:p1"),
        agent_json("reviewer", Some("omp"), "idle", "w3:p1"),
    ]
    .join(",");
    fake.script(
        LOCAL,
        "agent-list",
        "out",
        &result("agent_list", &format!(r#""agents":[{agents}],"#)),
    );
    fake.script(
        REMOTE,
        "agent-list",
        "out",
        &result("agent_list", r#""agents":[],"#),
    );
    fake.mark("machine-aspen4-saved.down");

    let occupancy = client.occupancy();
    assert_eq!(
        occupancy.machines.get("aspen1"),
        Some(&MachineLoad::Available(BTreeMap::from([(
            AgentKind::Omp,
            2
        )])))
    );
    assert_eq!(
        occupancy.machines.get("aspen3"),
        Some(&MachineLoad::Available(BTreeMap::new()))
    );
    assert!(matches!(
        occupancy.machines.get("aspen4"),
        Some(MachineLoad::Unavailable(HerdrFailure {
            class: FailureClass::Unavailable,
            ..
        }))
    ));
    assert_eq!(
        place(client.inventory(), &occupancy, &[AgentKind::Omp]),
        Some(Placement {
            machine: "aspen3".to_owned(),
            kind: AgentKind::Omp,
            slot: 0,
        })
    );
    assert_eq!(
        place(client.inventory(), &occupancy, &[AgentKind::Claude]),
        Some(Placement {
            machine: "aspen1".to_owned(),
            kind: AgentKind::Claude,
            slot: 0,
        })
    );
    assert_eq!(
        place(client.inventory(), &occupancy, &[AgentKind::Codex]),
        None
    );
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn malformed_output_is_rejected_for_reads_and_unknown_for_mutations() {
    let fake = Fake::new();
    let client = fake.client();
    fake.script(LOCAL, "agent-list", "out", "not json");
    assert_eq!(
        failure(client.list_agents("aspen1")).class,
        FailureClass::Rejected
    );
    fake.script(
        LOCAL,
        "agent-list",
        "out",
        &result(
            "agent_list",
            &format!(
                r#""agents":[{}],"#,
                agent_json(NAME, Some("omp"), "sleeping", PANE)
            ),
        ),
    );
    assert_eq!(
        failure(client.list_agents("aspen1")).class,
        FailureClass::Rejected
    );
    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result("agent_list", r#""agents":[],"#),
    );
    assert_eq!(
        failure(client.status("aspen1", NAME)).class,
        FailureClass::Rejected
    );
    fake.script(LOCAL, "agent-prompt", "out", "{\"id\":\"cli\",\"result\":");
    assert_eq!(
        failure(client.prompt("aspen1", NAME, "go")).class,
        FailureClass::Unknown
    );
    fake.script(LOCAL, "workspace-close", "out", &result("pane_info", ""));
    assert_eq!(
        failure(client.close_workspace("aspen1", "w2")).class,
        FailureClass::Unknown
    );
    let big = format!(
        r#"{{"id":"cli","result":{{"type":"agent_list","agents":[],"pad":"{}"}}}}"#,
        "x".repeat(5_000)
    );
    fake.script(LOCAL, "agent-list", "out", &big);
    let oversized = failure(client.list_agents("aspen1"));
    assert_eq!(oversized.class, FailureClass::Rejected);
    assert!(oversized.detail.contains("bound"));
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn unsupported_kinds_and_invalid_requests_fail_before_herdr_runs() {
    let fake = Fake::new();
    let client = fake.client();
    let unsupported = failure(client.start_agent("aspen3", PANE, NAME, AgentKind::Codex));
    assert_eq!(unsupported.class, FailureClass::Rejected);
    assert!(unsupported.detail.contains("codex"));
    let refusals = [
        failure(client.start_agent("aspen1", PANE, "reviewer", AgentKind::Omp)),
        failure(client.start_agent("aspen1", PANE, "kiln-UPPER", AgentKind::Omp)),
        failure(client.start_agent("aspen1", "w1 p1", NAME, AgentKind::Omp)),
        failure(client.start_agent("aspen9", PANE, NAME, AgentKind::Omp)),
        failure(client.create_worktree("aspen1", "other/repo", BRANCH, BASE, &attempt())),
        failure(client.create_worktree("aspen1", "OnixResearch/kiln", BRANCH, "main", &attempt())),
        failure(client.create_worktree("aspen1", "OnixResearch/kiln", "-x", BASE, &attempt())),
        failure(client.prompt("aspen1", NAME, "")),
        failure(client.prompt("aspen1", NAME, "bell\u{7}")),
        failure(client.read("aspen1", NAME, 0)),
        failure(client.close_workspace("aspen1", "--group")),
    ];
    assert!(
        refusals
            .iter()
            .all(|failure| failure.class == FailureClass::Rejected)
    );
    assert!(fake.argv().is_empty(), "no herdr process may run");

    fake.script(
        LOCAL,
        "agent-start",
        "usage",
        "unsupported interactive agent kind: omp\n",
    );
    let usage = failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp));
    assert_eq!(usage.class, FailureClass::Rejected);
    assert!(usage.detail.contains("unsupported interactive agent kind"));
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn unavailable_machines_are_never_empty() {
    let fake = Fake::new();
    let client = fake.client();
    fake.mark(&format!("{LOCAL}.down"));
    let down = failure(client.list_agents("aspen1"));
    assert_eq!(down.class, FailureClass::Unavailable);
    assert_eq!(down.code.as_deref(), Some("server_not_running"));
    assert_eq!(
        failure(client.status("aspen1", NAME)).class,
        FailureClass::Unavailable
    );
    assert_eq!(
        failure(client.prompt("aspen1", NAME, "go")).class,
        FailureClass::Unavailable
    );

    fake.mark("machine-aspen4-saved.unknown");
    let unknown_machine = failure(client.probe("aspen4"));
    assert_eq!(unknown_machine.class, FailureClass::Unavailable);
    assert!(unknown_machine.detail.contains("unknown machine"));

    let mut missing = fake.inventory();
    missing.executable = fake.home.join("missing-herdr");
    let missing = HerdrClient::new(missing).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        failure(missing.start_agent("aspen1", PANE, NAME, AgentKind::Omp)).class,
        FailureClass::Unavailable
    );
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn deadlines_are_unknown_for_mutations_and_unavailable_for_reads() {
    let fake = Fake::new();
    let client = fake.client();
    fake.script(LOCAL, "agent-prompt", "spin", "");
    let prompt = failure(client.prompt("aspen1", NAME, "go"));
    assert_eq!(prompt.class, FailureClass::Unknown);
    assert!(prompt.detail.contains("deadline"));
    fake.script(LOCAL, "agent-start", "spin", "");
    assert_eq!(
        failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp)).class,
        FailureClass::Unknown
    );
    fake.script(LOCAL, "agent-get", "spin", "");
    assert_eq!(
        failure(client.status("aspen1", NAME)).class,
        FailureClass::Unavailable
    );
    fake.script(LOCAL, "agent-prompt", "err", &error("timeout", "timed out"));
    fake.clear(LOCAL, "agent-prompt", "spin");
    assert_eq!(
        failure(client.prompt("aspen1", NAME, "go")).class,
        FailureClass::Unknown
    );
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn cancellation_tears_down_and_reports_unknown() {
    let fake = Fake::new();
    let mut configured = fake.inventory();
    configured.timeout_ms = 30_000;
    let flag = Arc::new(AtomicBool::new(false));
    let client = HerdrClient::new(configured)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .with_cancellation(Arc::clone(&flag));
    fake.script(LOCAL, "workspace-close", "spin", "");
    let raiser = {
        let flag = Arc::clone(&flag);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            flag.store(true, Ordering::SeqCst);
        })
    };
    let cancelled = failure(client.close_workspace("aspen1", "w2"));
    raiser.join().unwrap_or_else(|_| unreachable!());
    assert_eq!(cancelled.class, FailureClass::Unknown);
    assert!(cancelled.detail.contains("cancelled"));
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn stale_agent_names_are_gone_or_rejected() {
    let fake = Fake::new();
    let client = fake.client();
    let not_found = error(
        "agent_not_found",
        "agent target kiln-0123456789ab not found",
    );
    fake.script(LOCAL, "agent-get", "err", &not_found);
    assert_eq!(client.status("aspen1", NAME), Ok(AgentObservation::Gone));
    for operation in ["agent-prompt", "agent-send-keys", "agent-read"] {
        fake.script(LOCAL, operation, "err", &not_found);
    }
    let prompt = failure(client.prompt("aspen1", NAME, "go"));
    assert_eq!(prompt.class, FailureClass::Rejected);
    assert_eq!(prompt.code.as_deref(), Some("agent_not_found"));
    assert_eq!(
        failure(client.interrupt("aspen1", NAME)).code.as_deref(),
        Some("agent_not_found")
    );
    assert_eq!(
        failure(client.read("aspen1", NAME, 10)).class,
        FailureClass::Rejected
    );

    // The name outlives the agent briefly: no detected kind means it exited.
    fake.clear(LOCAL, "agent-get", "err");
    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(r#""agent":{},"#, agent_json(NAME, None, "idle", PANE)),
        ),
    );
    assert_eq!(client.status("aspen1", NAME), Ok(AgentObservation::Gone));

    // herdr's own `unknown` status is not invented liveness or absence.
    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("omp"), "unknown", PANE)
            ),
        ),
    );
    assert_eq!(
        client.status("aspen1", NAME),
        Ok(AgentObservation::Status(AgentStatus::Unknown))
    );

    fake.script(
        LOCAL,
        "workspace-close",
        "err",
        &error("workspace_not_found", "workspace w2 not found"),
    );
    let closed = failure(client.close_workspace("aspen1", "w2"));
    assert_eq!(closed.class, FailureClass::Rejected);
    assert_eq!(closed.code.as_deref(), Some("workspace_not_found"));
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn replayed_start_is_reconciled_by_reading_the_holder() {
    let fake = Fake::new();
    let client = fake.client();
    fake.script(
        LOCAL,
        "agent-start",
        "err",
        &error(
            "agent_name_taken",
            "agent name kiln-0123456789ab is already used",
        ),
    );
    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("omp"), "idle", PANE)
            ),
        ),
    );
    let replayed = client
        .start_agent("aspen1", PANE, NAME, AgentKind::Omp)
        .unwrap_or_else(|failure| unreachable!("{failure}"));
    assert!(replayed.replayed);
    assert_eq!(replayed.agent.pane_id, PANE);

    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("omp"), "idle", "w7:p1")
            ),
        ),
    );
    let elsewhere = failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp));
    assert_eq!(elsewhere.class, FailureClass::Rejected);
    assert_eq!(elsewhere.code.as_deref(), Some("agent_name_taken"));

    fake.script(
        LOCAL,
        "agent-get",
        "out",
        &result(
            "agent_info",
            &format!(
                r#""agent":{},"#,
                agent_json(NAME, Some("claude"), "idle", PANE)
            ),
        ),
    );
    assert_eq!(
        failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp)).class,
        FailureClass::Rejected
    );

    fake.clear(LOCAL, "agent-get", "out");
    fake.script(LOCAL, "agent-get", "spin", "");
    assert_eq!(
        failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp)).class,
        FailureClass::Unknown
    );
}

#[test]
// r[verify kiln.fleet.transport.scenario.failures]
fn ambiguous_outcomes_are_unknown() {
    let fake = Fake::new();
    let client = fake.client();
    fake.script(
        LOCAL,
        "agent-start",
        "err",
        &error(
            "agent_not_ready",
            "agent kiln-0123456789ab is blocked during startup",
        ),
    );
    let not_ready = failure(client.start_agent("aspen1", PANE, NAME, AgentKind::Omp));
    assert_eq!(not_ready.class, FailureClass::Unknown);
    assert_eq!(not_ready.code.as_deref(), Some("agent_not_ready"));

    fake.script(
        LOCAL,
        "agent-prompt",
        "err",
        &error("agent_prompt_stalled", "no activity observed"),
    );
    assert_eq!(
        failure(client.prompt("aspen1", NAME, "go")).class,
        FailureClass::Unknown
    );

    // A remote transport failure without a herdr error body may have applied.
    fake.script(
        REMOTE,
        "agent-send-keys",
        "err",
        "ssh: connection reset by peer\n",
    );
    let lost = failure(client.interrupt("aspen3", NAME));
    assert_eq!(lost.class, FailureClass::Unknown);
    assert!(lost.detail.contains("connection reset"));

    fake.script(
        LOCAL,
        "worktree-create",
        "out",
        &worktree_created("/srv/elsewhere", BRANCH),
    );
    assert_eq!(
        failure(client.create_worktree("aspen1", "OnixResearch/kiln", BRANCH, BASE, &attempt()))
            .class,
        FailureClass::Unknown
    );

    // An explicit herdr refusal is authoritative, not ambiguous.
    fake.script(
        LOCAL,
        "agent-prompt",
        "err",
        &error("agent_blocked", "agent is waiting at an approval dialog"),
    );
    assert_eq!(
        failure(client.prompt("aspen1", NAME, "go")).class,
        FailureClass::Rejected
    );
}

#[test]
fn nickel_export_of_the_positive_fixture_is_admitted() {
    let text = include_str!("fixtures/fleet-inventory.json");
    let inventory = HerdrInventory::from_json(text).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(inventory.session, "kiln");
    assert_eq!(inventory.machines.len(), 2);
    let aspen3 = inventory
        .machine("aspen3")
        .unwrap_or_else(|| unreachable!());
    assert_eq!(aspen3.herdr_machine.as_deref(), Some("aspen3"));
    assert_eq!(aspen3.slots(AgentKind::Agy), 1);
    assert!(HerdrClient::new(inventory).is_ok());
}
