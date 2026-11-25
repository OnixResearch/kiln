#![cfg(unix)]
//! End-to-end dispatch: admitted interactive work is claimed, placed in a
//! scripted herdr that creates a real git worktree, prompted, handed off
//! through the API and captured from a local git repository.

mod common;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock};

use common::{COORDINATOR, PEER, REPOSITORY, TestResult, config, executable, write_enrollment};
use kiln_core::fleet::{ClaimState, SessionState};
use kiln_core::implementation::{AttemptPhase, ImplementationWork, SupersessionRule};
use kiln_fleet::api::Service;
use kiln_fleet::config::FleetConfig;
use kiln_fleet::dispatch::Dispatcher;
use kiln_fleet::http::Request;
use kiln_implementer::verification::{
    FrozenProgram, HerdrCandidateDisposition, HerdrCandidateReceipt, VerificationProfile,
    VerifierAuthority,
};
use serde_json::{Value, json};

const HERDR: &str = r#"#!/bin/sh
state="$HOME"
printf '%s\n' "$*" >> "$state/argv.log"
shift 2
group="$1"; op="$2"; shift 2
status=idle
if [ -f "$state/status" ]; then read -r status < "$state/status"; fi
agent_json() { printf '{"agent":"omp","agent_status":"%s","name":"%s","pane_id":"w2:p1","workspace_id":"w2"}' "$status" "$1"; }
ok() { printf '{"id":"cli","result":%s}\n' "$1"; }
case "$group-$op" in
  agent-list)
    if [ -f "$state/agent" ]; then read -r name < "$state/agent"; ok "{\"type\":\"agent_list\",\"agents\":[$(agent_json "$name")]}"
    else ok '{"type":"agent_list","agents":[]}'; fi ;;
  worktree-list)
    if [ -f "$state/worktree" ]; then read -r path < "$state/worktree"; ok "{\"type\":\"worktree_list\",\"worktrees\":[{\"path\":\"$path\"}]}"
    else ok '{"type":"worktree_list","worktrees":[]}'; fi ;;
  worktree-create)
    while [ $# -gt 0 ]; do
      case "$1" in
        --cwd) cwd="$2"; shift 2 ;;
        --branch) branch="$2"; shift 2 ;;
        --base) base="$2"; shift 2 ;;
        --path) path="$2"; shift 2 ;;
        --label) shift 2 ;;
        *) shift ;;
      esac
    done
    if ! git -C "$cwd" worktree add -q -b "$branch" "$path" "$base" >/dev/null 2>&1; then
      echo '{"id":"cli","error":{"code":"worktree_failed","message":"git worktree add failed"}}' >&2; exit 1
    fi
    printf '%s\n' "$path" > "$state/worktree"
    ok "{\"type\":\"worktree_created\",\"workspace\":{\"workspace_id\":\"w2\"},\"root_pane\":{\"pane_id\":\"w2:p1\"},\"worktree\":{\"path\":\"$path\",\"branch\":\"$branch\",\"open_workspace_id\":\"w2\"}}" ;;
  agent-start)
    printf '%s\n' "$1" > "$state/agent"
    ok "{\"type\":\"agent_started\",\"agent\":$(agent_json "$1")}" ;;
  agent-prompt)
    printf '%s' "$2" > "$state/prompt.txt"
    ok "{\"type\":\"agent_prompted\",\"agent\":$(agent_json "$1")}" ;;
  agent-get)
    if [ -f "$state/agent" ]; then ok "{\"type\":\"agent_info\",\"agent\":$(agent_json "$1")}"
    else echo '{"id":"cli","error":{"code":"agent_not_found","message":"no agent"}}' >&2; exit 1; fi ;;
  agent-send-keys) ok '{"type":"ok"}' ;;
  workspace-close) rm -f "$state/agent"; ok '{"type":"ok"}' ;;
  *) echo "{\"id\":\"cli\",\"error\":{\"code\":\"unscripted\",\"message\":\"$group $op\"}}" >&2; exit 1 ;;
esac
"#;

static FAKE_HERDR: LazyLock<PathBuf> = LazyLock::new(|| executable("herdr", HERDR));

/// The real tar under the service's umask, 077: extraction must still
/// produce the exact 0644 and 0755 modes materialization admits.
static RESTRICTIVE_TAR: LazyLock<PathBuf> = LazyLock::new(|| {
    let tar = program("tar").unwrap_or_else(|_| unreachable!());
    executable(
        "tar",
        &format!("#!/bin/sh\numask 077\nexec '{}' \"$@\"\n", tar.display()),
    )
});

fn program(name: &str) -> TestResult<PathBuf> {
    let path = std::env::var_os("PATH").ok_or("PATH is unset")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name} is not on PATH").into())
}

fn git(directory: &Path, args: &[&str]) -> TestResult<String> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-C",
        ])
        .arg(directory)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

struct Fleet {
    _directory: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    authority: PathBuf,
    service: Arc<Service>,
    dispatcher: Dispatcher,
    work: ImplementationWork,
}

/// `real` uses the private verification root and git from
/// `KILN_VERIFICATION_TEST_ROOT`/`KILN_VERIFICATION_TEST_GIT`, which
/// `materialize_candidate` admits.
fn fleet(real: bool) -> TestResult<Fleet> {
    let directory = if real {
        tempfile::Builder::new().prefix("kiln-fleet-").tempdir_in(
            std::env::var_os("KILN_VERIFICATION_TEST_ROOT")
                .ok_or("set KILN_VERIFICATION_TEST_ROOT")?,
        )?
    } else {
        tempfile::tempdir()?
    };
    let root = fs::canonicalize(directory.path())?;
    write_enrollment(&root)?;
    let state = root.join("herdr-home");
    let checkout = root.join("checkout");
    let authority = root.join("authority");
    let evidence = root.join("evidence");
    for path in [
        &state,
        &checkout,
        &evidence,
        &root.join("profiles"),
        &root.join("worktrees"),
    ] {
        fs::create_dir_all(path)?;
    }
    git(&checkout, &["init", "-q", "-b", "main"])?;
    fs::create_dir_all(checkout.join("src"))?;
    fs::write(checkout.join("src/value"), "broken\n")?;
    fs::write(checkout.join("requirements"), "frozen\n")?;
    fs::write(checkout.join("check"), "#!/bin/sh\n")?;
    fs::set_permissions(checkout.join("check"), fs::Permissions::from_mode(0o755))?;
    git(&checkout, &["add", "."])?;
    git(&checkout, &["commit", "-q", "-m", "baseline"])?;
    let base = git(&checkout, &["rev-parse", "HEAD"])?;
    let clone = Command::new("git")
        .arg("clone")
        .arg("-q")
        .arg(&checkout)
        .arg(&authority)
        .status()?;
    assert!(clone.success());

    let git_program = if real {
        fs::canonicalize(
            std::env::var_os("KILN_VERIFICATION_TEST_GIT")
                .ok_or("set KILN_VERIFICATION_TEST_GIT")?,
        )?
    } else {
        program("git")?
    };
    let mut profile = VerificationProfile {
        schema: "kiln.verification-profile.v1".into(),
        repository: REPOSITORY.into(),
        toolchain: common::content('b'),
        environment_identity: common::content('c'),
        environment: BTreeMap::new(),
        checks: Vec::new(),
        reproducer: None,
        protected_paths: vec!["requirements".into()],
        allow_already_satisfied: false,
        cairn: None,
        proof: None,
    };
    profile.toolchain = profile.derived_toolchain_identity()?;
    profile.environment_identity = profile.derived_environment_identity()?;
    let profile_identity = profile.identity()?;
    fs::write(
        root.join("profiles")
            .join(format!("{}.json", profile_identity.as_str())),
        serde_json::to_vec(&profile)?,
    )?;
    let verifier = VerifierAuthority {
        repository: REPOSITORY.into(),
        repository_root: authority.clone(),
        scratch_root: evidence,
        git: FrozenProgram {
            blake3: blake3::hash(&fs::read(&git_program)?).to_hex().to_string(),
            path: git_program.clone(),
        },
        worker_uid: u32::MAX,
    };
    fs::write(root.join("authority.json"), serde_json::to_vec(&verifier)?)?;
    let inventory = json!({
        "schema": "kiln.fleet-inventory.v1",
        "executable": *FAKE_HERDR,
        "session": "kiln-test",
        "environment": { "home": state, "path": std::env::var("PATH")? },
        "timeout_ms": 20_000,
        "start_timeout_ms": 1_000,
        "machines": [{
            "label": COORDINATOR,
            "worktree_root": root.join("worktrees"),
            "repositories": { REPOSITORY: checkout },
            "agents": [{ "kind": "omp", "slots": 1 }],
        }],
    });
    fs::write(root.join("inventory.json"), serde_json::to_vec(&inventory)?)?;
    let mut document = config(&root, Path::new("/nonexistent/tailscale"));
    let object = document
        .as_object_mut()
        .ok_or("configuration is not an object")?;
    object.insert(
        "capture".into(),
        json!({
            "git": program("git")?,
            "tar": *RESTRICTIVE_TAR,
            "export_root": root.join("exports"),
            "verification_profiles": root.join("profiles"),
            "repositories": { REPOSITORY: { "authority": root.join("authority.json") } },
        }),
    );
    object.insert(
        "dispatch".into(),
        json!({ "enabled": true, "kinds": ["omp"] }),
    );
    let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
        &document,
    )?)?)?);
    let work = common::work(&base, &profile_identity, &["src"])?;
    let mut store = service.store.open()?;
    store.admit(&work, None)?;
    // Lattice work without interactive_agent stays queued for Lattice.
    let mut lattice = common::work(&base, &profile_identity, &["docs"])?;
    lattice.title = "Lattice-only work".into();
    lattice
        .permitted_effects
        .remove(kiln_core::implementation::INTERACTIVE_AGENT_EFFECT);
    for alias in &mut lattice.aliases {
        alias.native_id = "onix/kiln#8".into();
        alias.source = kiln_core::types::ExternalRef::new("github", "issue", "onix/kiln#8")
            .map_err(|error| error.message)?;
    }
    lattice.priority = 9;
    store.admit(&lattice, None)?;
    let dispatcher = Dispatcher::new(Arc::clone(&service))?;
    Ok(Fleet {
        _directory: directory,
        root,
        state,
        authority,
        service,
        dispatcher,
        work,
    })
}

fn now() -> u64 {
    kiln_fleet::log::now_ms()
}

impl Fleet {
    fn session(&self) -> TestResult<kiln_core::fleet::FleetSession> {
        let state = self.service.store.open()?.fleet_state()?;
        Ok(state
            .sessions
            .sessions
            .values()
            .next()
            .cloned()
            .ok_or("no session")?)
    }

    fn handoff(&self, machine: &str, attempt: &str, fence: u64, commit: &str) -> u16 {
        let request = Request {
            method: "POST".into(),
            path: format!("/v1/attempts/{attempt}/handoff"),
            body: json!({ "fence": fence, "commit": commit })
                .to_string()
                .into_bytes(),
        };
        self.service.route(machine, &request, now()).status
    }

    /// Claim, place, work and hand off; returns the attempt, fence and commit.
    fn drive_to_handoff(&mut self) -> TestResult<(String, u64, String)> {
        let first = self.dispatcher.tick(now())?;
        assert_eq!(first.free_slots, 1);
        assert_eq!(
            first.performed,
            vec![kiln_core::fleet::FleetEffect::ClaimNext]
        );
        let mut store = self.service.store.open()?;
        let views = store.fleet_attempt_views(now())?;
        let view = views.first().ok_or("no attempt")?.clone();
        assert!(view.held_by_fleet);
        let attempt = store.attempt(&view.attempt)?.ok_or("attempt")?;
        assert_eq!(
            attempt.work_identity,
            self.work.identity().map_err(|error| error.message)?
        );
        let plan = store.dispatch_plan(&attempt.identity)?.ok_or("plan")?;
        assert_eq!(plan.provider_profile.as_str(), "herdr-interactive");
        assert!(plan.workspace.network && plan.workspace.credentials);
        let claims = store.fleet_state()?.claims;
        let claim = claims.claims.values().next().ok_or("no attempt claim")?;
        assert_eq!(claim.attempt.as_ref(), Some(&attempt.identity));
        assert_eq!(claim.holder.tool, "kiln");
        assert_eq!(claim.scope.text(), "onix/kiln src");

        let second = self.dispatcher.tick(now())?;
        assert!(matches!(
            second.performed.as_slice(),
            [kiln_core::fleet::FleetEffect::Place { .. }]
        ));
        let session = self.session()?;
        assert_eq!(session.state, SessionState::Prompted, "{session:?}");
        let workspace = session.workspace.clone().ok_or("workspace")?;
        let prompt = fs::read_to_string(self.state.join("prompt.txt"))?;
        assert!(prompt.contains(attempt.identity.as_str()));
        assert!(prompt.contains(&workspace.branch));
        let store_attempt = store.attempt(&attempt.identity)?.ok_or("attempt")?;
        assert_eq!(store_attempt.phase, AttemptPhase::Executing);
        // The plan's workspace identity names this machine and worktree.
        let inventory = kiln_adapter_herdr::HerdrInventory::from_json(&fs::read_to_string(
            self.root.join("inventory.json"),
        )?)?;
        let planned =
            kiln_fleet::dispatch::planned_machine(&inventory, &plan, &store_attempt, &self.work)
                .ok_or("plan names no inventory machine")?;
        assert_eq!(planned.label, COORDINATOR);
        assert!(workspace.path.starts_with(&planned.worktree_root));

        // The agent works and commits on its branch.
        fs::write(self.state.join("status"), "working\n")?;
        self.dispatcher.tick(now())?;
        assert_eq!(self.session()?.state, SessionState::Working);
        let worktree = PathBuf::from(&workspace.path);
        fs::write(worktree.join("src/value"), "accepted\n")?;
        git(&worktree, &["commit", "-q", "-am", "fix the value"])?;
        let commit = git(&worktree, &["rev-parse", "HEAD"])?;
        assert_eq!(
            git(&worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?,
            workspace.branch
        );

        let attempt_id = attempt.identity.as_str().to_owned();
        assert_eq!(self.handoff(PEER, &attempt_id, attempt.fence, &commit), 403);
        assert_eq!(
            self.handoff(COORDINATOR, &attempt_id, attempt.fence + 1, &commit),
            404
        );
        assert_eq!(
            self.handoff(COORDINATOR, &attempt_id, attempt.fence, &commit),
            202
        );
        assert!(matches!(
            self.session()?.state,
            SessionState::HandoffReceived { .. }
        ));
        Ok((attempt_id, attempt.fence, commit))
    }
}

fn identity(text: &str) -> TestResult<kiln_core::types::ContentId> {
    Ok(kiln_core::types::ContentId::from_blake3_hex(text).map_err(|error| error.message)?)
}

#[test]
// r[verify kiln.fleet.dispatch] r[verify kiln.fleet.transport] r[verify kiln.fleet.capture]
fn interactive_work_is_dispatched_handed_off_and_exported() -> TestResult {
    let mut fleet = fleet(false)?;
    let (attempt, fence, commit) = fleet.drive_to_handoff()?;

    // Capture fetches and exports the exact commit, then materialization
    // refuses this fixture's unprivileged verifier authority.
    let capture = fleet.dispatcher.tick(now())?;
    assert!(matches!(
        capture.performed.as_slice(),
        [kiln_core::fleet::FleetEffect::Capture { .. }]
    ));
    let fetched = git(
        &fleet.authority,
        &["rev-parse", &format!("refs/kiln/fleet-handoffs/{attempt}")],
    )?;
    assert_eq!(fetched, commit);
    let export = fleet
        .root
        .join("exports")
        .join(format!("{attempt}-{fence}"));
    assert_eq!(
        fs::read_to_string(export.join("tree/src/value"))?,
        "accepted\n"
    );
    assert_eq!(
        fs::read_to_string(export.join("tree/requirements"))?,
        "frozen\n"
    );
    assert!(!export.join("tree/.git").exists());
    let mode = |path: &str| -> TestResult<u32> {
        Ok(fs::metadata(export.join(path))?.permissions().mode() & 0o7777)
    };
    assert_eq!(mode("tree/src/value")?, 0o644);
    assert_eq!(mode("tree/check")?, 0o755);
    let receipt: HerdrCandidateReceipt =
        serde_json::from_slice(&fs::read(export.join("receipt.json"))?)?;
    let mut store = fleet.service.store.open()?;
    let plan = store.dispatch_plan(&identity(&attempt)?)?.ok_or("plan")?;
    assert_eq!(receipt.candidate_commit, commit);
    assert_eq!(receipt.attempt_id, attempt);
    assert_eq!(receipt.fence, fence);
    assert_eq!(receipt.workspace_id, plan.workspace.identity.as_str());
    assert_eq!(receipt.disposition, HerdrCandidateDisposition::Produced);
    let session = fleet.session()?;
    assert_eq!(receipt.session, session.identity);
    let SessionState::Failed { cause } = &session.state else {
        return Err(format!("capture should fail materialization here: {session:?}").into());
    };
    assert!(cause.contains("materialization"), "{cause}");

    // A failed capture cancels the attempt and stops the agent.
    let stop = fleet.dispatcher.tick(now())?;
    assert!(
        stop.performed
            .iter()
            .any(|effect| matches!(effect, kiln_core::fleet::FleetEffect::Cancel { .. }))
    );
    assert!(
        stop.performed
            .iter()
            .any(|effect| matches!(effect, kiln_core::fleet::FleetEffect::Stop { .. }))
    );
    let attempt_id = identity(&attempt)?;
    assert_eq!(
        store.attempt(&attempt_id)?.ok_or("attempt")?.phase,
        AttemptPhase::Cancelled
    );
    let claims = store.fleet_state()?.claims;
    assert!(
        claims
            .claims
            .values()
            .all(|claim| claim.state != ClaimState::Open)
    );
    let argv = fs::read_to_string(fleet.state.join("argv.log"))?;
    assert!(argv.contains("agent send-keys kiln-"));
    assert!(argv.contains("workspace close w2"));
    // Nothing repeats once observed.
    let quiet = fleet.dispatcher.tick(now())?;
    assert!(
        quiet
            .performed
            .iter()
            .all(|effect| matches!(effect, kiln_core::fleet::FleetEffect::ClaimNext)),
        "{:?}",
        quiet.performed
    );
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
// r[verify kiln.fleet.capture] r[verify kiln.fleet.capture.scenario.receipt]
fn handed_off_commits_become_candidate_handoffs() -> TestResult {
    let mut fleet = fleet(true)?;
    let (attempt, fence, commit) = fleet.drive_to_handoff()?;
    fleet.dispatcher.tick(now())?;
    assert_eq!(fleet.session()?.state, SessionState::Captured);
    let mut store = fleet.service.store.open()?;
    let attempt_id = identity(&attempt)?;
    let record = store.attempt(&attempt_id)?.ok_or("attempt")?;
    assert_eq!(record.phase, AttemptPhase::CandidateProduced);
    let handoff = store.candidate_handoff(&attempt_id)?.ok_or("handoff")?;
    let tree = git(
        &fleet.authority,
        &["rev-parse", &format!("{commit}^{{tree}}")],
    )?;
    assert_eq!(handoff.tree.native_id.as_str(), tree);
    assert_eq!(handoff.changed_paths, vec!["src/value".to_owned()]);
    let written = fleet
        .root
        .join("exports")
        .join(format!("{attempt}-{fence}/handoff.json"));
    assert!(written.is_file());
    Ok(())
}

#[test]
fn work_without_interactive_agent_is_left_for_lattice() -> TestResult {
    let mut fleet = fleet(false)?;
    fleet.dispatcher.tick(now())?;
    let mut store = fleet.service.store.open()?;
    let attempts = store.attempts()?;
    let claimed: Vec<_> = attempts.attempts().collect();
    assert_eq!(claimed.len(), 1);
    let work_identity = fleet.work.identity().map_err(|error| error.message)?;
    assert_eq!(
        claimed.first().map(|attempt| &attempt.work_identity),
        Some(&work_identity)
    );
    let request = Request {
        method: "GET".into(),
        path: "/v1/work".into(),
        body: Vec::new(),
    };
    let listed = fleet.service.route(COORDINATOR, &request, now());
    assert_eq!(listed.status, 200);
    let ranked = listed
        .body
        .pointer("/ranked")
        .and_then(Value::as_array)
        .ok_or("ranked")?;
    assert!(
        ranked
            .iter()
            .any(|row| row.get("interactive") == Some(&Value::Bool(false)))
    );
    let mut newer = fleet.work.clone();
    newer.revision += 1;
    newer.aliases.first_mut().ok_or("alias")?.version = common::content('d');
    store.admit_with_supersession(&newer, None, SupersessionRule::FinishActive, now())?;
    let attempts = fleet.service.route(
        COORDINATOR,
        &Request {
            method: "GET".into(),
            path: "/v1/attempts".into(),
            body: Vec::new(),
        },
        now(),
    );
    assert_eq!(attempts.status, 200);
    assert_eq!(
        attempts.body.pointer("/attempts/0/held_by_fleet"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        attempts.body.pointer("/attempts/0/sources"),
        Some(&serde_json::to_value(&fleet.work.aliases)?)
    );
    Ok(())
}

const ROSTER_HERDR: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/roster.log"
case "$*" in
  '--session kiln-test agent list')
    printf '%s\n' '{"id":"cli","result":{"type":"agent_list","agents":[{"agent":"reviewer","agent_status":"working","name":"private-review","pane_id":"w1:p1","workspace_id":"w1"}]}}' ;;
  '--machine private-route agent list')
    if [ -f "$HOME/unreachable" ]; then
      echo 'private remote failure details' >&2
      exit 1
    fi
    printf '%s\n' '{"id":"cli","result":{"type":"agent_list","agents":[]}}' ;;
  *) exit 1 ;;
esac
"#;

thread_local! {
    static ROSTER_CLOCK: Cell<u64> = const { Cell::new(1_000) };
}

#[test]
// r[verify kiln.fleet.aggregate_workspace]
fn enrolled_roster_reports_only_fresh_successful_herdr_counts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_enrollment(root)?;
    let herdr = executable("roster-herdr", ROSTER_HERDR);
    let inventory = json!({
        "schema": "kiln.fleet-inventory.v1",
        "executable": herdr,
        "session": "kiln-test",
        "environment": { "home": root, "path": "/bin" },
        "machines": [
            { "label": COORDINATOR, "worktree_root": root, "agents": [{ "kind": "omp", "slots": 1 }] },
            { "label": PEER, "worktree_root": root, "herdr_machine": "private-route", "ssh_target": "private-target" },
        ],
    });
    fs::write(root.join("inventory.json"), serde_json::to_vec(&inventory)?)?;
    let mut document = config(root, Path::new("/nonexistent/tailscale"));
    let object = document.as_object_mut().ok_or("configuration must be an object")?;
    object.insert("dispatch".into(), json!({ "enabled": true, "kinds": ["omp"] }));
    object.insert("capture".into(), json!({
        "git": "/bin/git", "tar": "/bin/tar", "export_root": root,
        "verification_profiles": root, "repositories": {},
    }));
    let mut service = Service::new(FleetConfig::from_json(&serde_json::to_vec(&document)?)?)?;
    service.clock = || ROSTER_CLOCK.get();
    let service = Arc::new(service);
    let mut dispatcher = Dispatcher::new(Arc::clone(&service))?;
    let read_roster = |now_ms| {
        let response = service.route(
            PEER,
            &Request { method: "GET".into(), path: "/v1/machines".into(), body: Vec::new() },
            now_ms,
        );
        assert_eq!(response.status, 200);
        response.body
    };
    let not_reporting = |machine, placement_configured| json!({
        "machine": machine, "placement_configured": placement_configured, "state": "not_reporting",
        "observed_at_ms": null, "last_seen_at_ms": null, "sessions": null,
    });
    assert_eq!(read_roster(1_000), json!({ "machines": [
        not_reporting("aspen1", false), not_reporting(PEER, true), not_reporting(COORDINATOR, true),
    ] }));
    let log = root.join("roster.log");
    assert!(!log.exists());

    // The foreign agent contributes to the total without filling a Kiln slot.
    ROSTER_CLOCK.set(2_000);
    assert_eq!(dispatcher.tick(2_000)?.free_slots, 1);
    assert_eq!(read_roster(2_001), json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "state": "reporting",
          "observed_at_ms": 2_000, "last_seen_at_ms": 2_000, "sessions": 0 },
        { "machine": COORDINATOR, "placement_configured": true, "state": "reporting",
          "observed_at_ms": 2_000, "last_seen_at_ms": 2_000, "sessions": 1 },
    ] }));

    ROSTER_CLOCK.set(3_000);
    fs::write(root.join("unreachable"), "")?;
    dispatcher.tick(3_000)?;
    let failure = read_roster(3_001);
    assert_eq!(failure, json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "state": "unreachable",
          "observed_at_ms": 3_000, "last_seen_at_ms": 2_000, "sessions": null },
        { "machine": COORDINATOR, "placement_configured": true, "state": "reporting",
          "observed_at_ms": 3_000, "last_seen_at_ms": 3_000, "sessions": 1 },
    ] }));

    let stale = read_roster(3_000 + service.config.policy.tick_ms * 3);
    assert_eq!(stale, json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "state": "stale",
          "observed_at_ms": 3_000, "last_seen_at_ms": 2_000, "sessions": null },
        { "machine": COORDINATOR, "placement_configured": true, "state": "stale",
          "observed_at_ms": 3_000, "last_seen_at_ms": 3_000, "sessions": null },
    ] }));
    assert_eq!(fs::read_to_string(log)?, concat!(
        "--session kiln-test agent list\n--machine private-route agent list\n",
        "--session kiln-test agent list\n--machine private-route agent list\n",
    ));
    Ok(())
}
