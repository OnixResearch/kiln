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
if [ "$1" = --machine ]; then state="$HOME/$2"; fi
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
    if [ -f "$state/worktree" ]; then read -r path < "$state/worktree"; read -r branch < "$state/branch"; ok "{\"type\":\"worktree_list\",\"worktrees\":[{\"path\":\"$path\",\"branch\":\"$branch\",\"open_workspace_id\":\"w2\"}]}"
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
    printf '%s\n' "$branch" > "$state/branch"
    ok "{\"type\":\"worktree_created\",\"workspace\":{\"workspace_id\":\"w2\"},\"root_pane\":{\"pane_id\":\"w2:p1\"},\"worktree\":{\"path\":\"$path\",\"branch\":\"$branch\",\"open_workspace_id\":\"w2\"}}" ;;
  agent-start)
    printf '%s\n' "$1" > "$state/agent"
    # A coordinator crash after the agent started: its reply never arrives.
    if [ -f "$state/start-lost" ]; then echo '{"id":"cli","error":{"code":"timeout","message":"reply lost"}}' >&2; exit 1; fi
    ok "{\"type\":\"agent_started\",\"agent\":$(agent_json "$1")}" ;;
  agent-prompt)
    # A coordinator crash before the first prompt was delivered.
    if [ -f "$state/prompt-lost" ]; then echo '{"id":"cli","error":{"code":"timeout","message":"prompt lost"}}' >&2; exit 1; fi
    printf '%s' "$2" > "$state/prompt.txt"
    printf '%s\n' "$2" >> "$state/prompts.log"
    ok "{\"type\":\"agent_prompted\",\"agent\":$(agent_json "$1")}" ;;
  agent-read)
    if [ -f "$state/pane-offline" ]; then echo '{"id":"cli","error":{"code":"server_not_running","message":"herdr offline"}}' >&2; exit 1; fi
    if [ -f "$state/pane.txt" ]; then cat "$state/pane.txt"
    else printf '\033[33mblocked>\033[0m question line 1\nquestion line 2\nquestion line 3\n'; fi ;;
  agent-get)
    if [ -f "$state/agent" ]; then ok "{\"type\":\"agent_info\",\"agent\":$(agent_json "$1")}"
    else echo '{"id":"cli","error":{"code":"agent_not_found","message":"no agent"}}' >&2; exit 1; fi ;;
  agent-send-keys)
    printf '%s\n' "$2" >> "$state/keys.log"
    ok '{"type":"ok"}' ;;
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

fn minimal_search_path() -> TestResult<String> {
    let mut directories = Vec::new();
    for candidate in [
        program("sh")?,
        program("git")?,
        program("rm")?,
        program("tar")?,
        FAKE_HERDR.clone(),
    ] {
        if let Some(parent) = candidate.parent()
            && !directories
                .iter()
                .any(|existing: &PathBuf| existing == parent)
        {
            directories.push(parent.to_path_buf());
        }
    }
    std::env::join_paths(&directories)?
        .into_string()
        .map_err(|_| "minimal PATH is not UTF-8".into())
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
    document: Value,
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
    let authority = if real {
        root.join("authority")
    } else {
        // Explicitly construct a world-writable ancestor so the fixture's
        // expected materialization refusal does not depend on ambient TMPDIR.
        let shared = root.join("world-writable");
        fs::create_dir(&shared)?;
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o777))?;
        shared.join("authority")
    };
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
    fs::create_dir_all(checkout.join("docs"))?;
    fs::write(
        checkout.join(".gitattributes"),
        "docs/ export-ignore\nversion.txt export-subst\n",
    )?;
    fs::write(checkout.join("docs/tracked.txt"), "kept\n")?;
    fs::write(checkout.join("version.txt"), "commit=$Format:%H$\n")?;
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
        "environment": { "home": state, "path": minimal_search_path()? },
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
    let mut store = service.store.open_coordinator()?;
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
        document,
        service,
        dispatcher,
        work,
    })
}

fn move_fixture(
    peer_checkout: bool,
) -> TestResult<(Fleet, kiln_core::fleet::placement::MoveRequest)> {
    let mut fleet = fleet(false)?;
    fleet.dispatcher.tick(now())?;
    let mut store = fleet.service.store.open_coordinator()?;
    let views = store.fleet_attempt_views(now())?;
    let attempt = store
        .attempt(&views.first().ok_or("automatic claim missing")?.attempt)?
        .ok_or("attempt missing")?;
    assert_eq!(attempt.phase, AttemptPhase::Claimed);
    assert!(
        store
            .fleet_state()?
            .sessions
            .for_attempt(&attempt.identity)
            .is_none()
    );
    let inventory_path = fleet.root.join("inventory.json");
    let mut inventory: Value = serde_json::from_slice(&fs::read(&inventory_path)?)?;
    fs::create_dir(fleet.state.join(PEER))?;
    fs::create_dir(fleet.root.join("peer-worktrees"))?;
    let repositories = if peer_checkout {
        json!({ REPOSITORY: fleet.root.join("checkout") })
    } else {
        json!({})
    };
    inventory["machines"]
        .as_array_mut()
        .ok_or("machines array")?
        .push(json!({
            "label": PEER, "herdr_machine": PEER, "ssh_target": "fixture@aspen3",
            "worktree_root": fleet.root.join("peer-worktrees"),
            "repositories": repositories,
            "agents": [{"kind":"omp","slots":1}],
        }));
    fs::write(inventory_path, serde_json::to_vec(&inventory)?)?;
    fleet.dispatcher = Dispatcher::new(Arc::clone(&fleet.service))?;
    Ok((
        fleet,
        kiln_core::fleet::placement::MoveRequest {
            attempt: attempt.identity,
            fence: attempt.fence,
            reservation: attempt
                .dispatch
                .ok_or("automatic reservation missing")?
                .effect_id,
            destination: PEER.into(),
        },
    ))
}

fn move_route(
    fleet: &Fleet,
    machine: &str,
    method: &str,
    path: &str,
    body: &Value,
) -> kiln_fleet::http::Response {
    fleet.service.route(
        machine,
        &Request {
            method: method.into(),
            path: path.into(),
            query: None,
            body: if method == "GET" {
                Vec::new()
            } else {
                body.to_string().into_bytes()
            },
        },
        now(),
    )
}

#[test]
// r[verify kiln.sdlc.queued_workload_move.scenario.operator]
fn queued_move_replaces_automatic_reservation_and_launches_once_on_selected_host() -> TestResult {
    use kiln_core::fleet::placement::MovePlan;
    let (mut fleet, request) = move_fixture(true)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let attempts = move_route(&fleet, COORDINATOR, "GET", "/v1/attempts", &Value::Null);
    assert_eq!(
        attempts.body["attempts"][0]["reservation"],
        json!(request.reservation)
    );
    assert_eq!(attempts.body["attempts"][0]["queued_move_eligible"], true);
    let capabilities = move_route(
        &fleet,
        COORDINATOR,
        "GET",
        "/v1/placement/capabilities",
        &Value::Null,
    );
    assert_eq!(capabilities.body["move"], true);
    assert_eq!(capabilities.body["active_move"], false);
    let body = serde_json::to_value(&request)?;
    assert_eq!(
        move_route(&fleet, PEER, "POST", "/v1/placement/move/plan", &body).status,
        403
    );
    let preview = move_route(
        &fleet,
        COORDINATOR,
        "POST",
        "/v1/placement/move/plan",
        &body,
    );
    assert_eq!(preview.status, 200, "{}", preview.body);
    let plan: MovePlan = serde_json::from_value(preview.body["plan"].clone())?;
    assert_eq!(plan.source, COORDINATOR);
    assert_eq!(plan.request, request);
    let status_path = format!("/v1/placement/move/{}", plan.identity.as_str());
    assert_eq!(
        move_route(&fleet, COORDINATOR, "GET", &status_path, &Value::Null).status,
        404
    );
    let before = store.attempt(&request.attempt)?.ok_or("queued attempt")?;
    assert_eq!(
        before.dispatch.as_ref().ok_or("old reservation")?.effect_id,
        request.reservation
    );
    let mut submission = body;
    submission["plan"] = json!(plan.identity);
    let peer_state = fleet.state.join(PEER);
    fs::write(peer_state.join("agent"), "kiln-000000000000\n")?;
    let unavailable = move_route(
        &fleet,
        COORDINATOR,
        "POST",
        "/v1/placement/move",
        &submission,
    );
    assert_eq!(unavailable.status, 409, "{}", unavailable.body);
    assert_eq!(store.attempt(&request.attempt)?, Some(before.clone()));
    fs::remove_file(peer_state.join("agent"))?;
    let accepted = move_route(
        &fleet,
        COORDINATOR,
        "POST",
        "/v1/placement/move",
        &submission,
    );
    assert_eq!(accepted.status, 202, "{}", accepted.body);
    assert_eq!(accepted.body["placement"]["state"], "requested");
    let after = store.attempt(&request.attempt)?.ok_or("rebound attempt")?;
    let new = after.dispatch.as_ref().ok_or("new reservation")?;
    assert_eq!(new.effect_id, plan.new_reservation);
    assert_eq!(new.plan, plan.dispatch_plan);
    let mut expected = before;
    expected.dispatch = after.dispatch.clone();
    assert_eq!(
        after, expected,
        "only the reservation changes on the attempt"
    );
    assert!(
        store
            .fleet_state()?
            .sessions
            .for_attempt(&request.attempt)
            .is_none()
    );
    let stale_placement = kiln_core::fleet::Placement {
        attempt: request.attempt.clone(),
        fence: request.fence,
        work: after.work_identity,
        work_revision: after.work_revision,
        machine: COORDINATOR.into(),
        kind: "omp".into(),
    };
    assert!(
        store
            .fleet_bind_reserved_session(stale_placement.clone(), &request.reservation, now())
            .is_err()
    );
    assert!(
        store
            .fleet_bind_reserved_session(stale_placement, &plan.new_reservation, now())
            .is_err()
    );

    // The slot may fill after submit; ordinary dispatch must defer, never fall back.
    fs::write(peer_state.join("agent"), "kiln-000000000000\n")?;
    fleet.dispatcher.tick(now())?;
    assert!(
        store
            .fleet_state()?
            .sessions
            .for_attempt(&request.attempt)
            .is_none()
    );
    fs::remove_file(peer_state.join("agent"))?;
    fs::write(peer_state.join("start-lost"), "")?;
    fleet.dispatcher.tick(now())?;
    let reconcile = move_route(&fleet, COORDINATOR, "GET", &status_path, &Value::Null);
    assert_eq!(reconcile.body["placement"]["state"], "reconcile_required");
    assert!(reconcile.body["placement"]["observed_ms"].is_null());
    assert_eq!(
        move_route(
            &fleet,
            COORDINATOR,
            "POST",
            "/v1/placement/move",
            &submission
        )
        .body,
        reconcile.body
    );

    // Listing the name without its exact workspace is not enough to adopt it.
    fleet.dispatcher = fleet.restart(Some(COORDINATOR))?;
    let branch = fs::read(peer_state.join("branch"))?;
    fs::write(peer_state.join("branch"), "other-branch\n")?;
    fleet.dispatcher.tick(now())?;
    assert_eq!(
        move_route(&fleet, COORDINATOR, "GET", &status_path, &Value::Null).body["placement"]["state"],
        "reconcile_required"
    );
    fs::write(peer_state.join("branch"), branch)?;
    fleet.dispatcher.tick(now())?;
    let observed = move_route(&fleet, COORDINATOR, "GET", &status_path, &Value::Null);
    assert_eq!(
        observed.body["placement"]["state"], "observed",
        "{}",
        observed.body
    );
    assert!(observed.body["placement"]["observed_ms"].is_u64());
    assert_eq!(fleet.session()?.machine, PEER);
    let calls = fs::read_to_string(peer_state.join("argv.log"))?;
    assert_eq!(
        calls
            .lines()
            .filter(|line| line.contains("agent start"))
            .count(),
        1
    );
    assert_eq!(
        calls
            .lines()
            .filter(|line| line.contains("worktree create"))
            .count(),
        1
    );
    assert!(!fleet.state.join("agent").exists());
    assert!(!fleet.state.join("worktree").exists());
    Ok(())
}

#[test]
fn queued_move_refuses_stale_and_unadmitted_requests_and_loses_to_reserved_session() -> TestResult {
    use kiln_fleet::dispatch::placement::{MoveSubmit, preview_move, submit_move};
    let (fleet, request) = move_fixture(false)?;
    let mut store = fleet.service.store.open_coordinator()?;
    assert!(
        preview_move(&fleet.service, &mut store, request, now())
            .err()
            .ok_or("move to missing checkout accepted")?
            .starts_with("fleet_placement_unavailable:")
    );

    let (fleet, request) = move_fixture(true)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let mut invalid = request.clone();
    invalid.fence += 1;
    assert!(
        preview_move(&fleet.service, &mut store, invalid, now())
            .err()
            .ok_or("stale fence accepted")?
            .starts_with("fleet_placement_stale:")
    );
    let mut invalid = request.clone();
    invalid.destination = COORDINATOR.into();
    assert!(
        preview_move(&fleet.service, &mut store, invalid, now())
            .err()
            .ok_or("same host accepted")?
            .starts_with("fleet_placement_source:")
    );
    let mut invalid = request.clone();
    invalid.destination = "unenrolled".into();
    assert!(
        preview_move(&fleet.service, &mut store, invalid, now())
            .err()
            .ok_or("unenrolled host accepted")?
            .starts_with("fleet_placement_unavailable:")
    );
    let plan = preview_move(&fleet.service, &mut store, request.clone(), now())?;
    let mut dispatcher_store = fleet.service.store.open_coordinator()?;
    let bound = dispatcher_store.fleet_bind_reserved_session(
        kiln_core::fleet::Placement {
            attempt: request.attempt.clone(),
            fence: request.fence,
            work: plan.work,
            work_revision: plan.work_revision,
            machine: COORDINATOR.into(),
            kind: "omp".into(),
        },
        &request.reservation,
        now(),
    )?;
    assert_eq!(bound.state, SessionState::Placing);
    let submission = MoveSubmit {
        plan: plan.identity.clone(),
        attempt: request.attempt.clone(),
        fence: request.fence,
        reservation: request.reservation.clone(),
        destination: request.destination,
    };
    assert!(
        submit_move(&fleet.service, &mut store, submission, now())
            .err()
            .ok_or("move won after binding")?
            .starts_with("fleet_placement_source:")
    );
    assert!(store.fleet_move_status(&plan.identity, now())?.is_none());
    assert_eq!(
        store
            .attempt(&request.attempt)?
            .and_then(|attempt| attempt.dispatch)
            .ok_or("old reservation")?
            .effect_id,
        request.reservation
    );
    Ok(())
}

#[test]
fn queued_move_round_trip_never_reuses_old_cas_token() -> TestResult {
    use kiln_fleet::dispatch::placement::{MoveSubmit, preview_move, submit_move};
    use kiln_implementer::admission::MoveState;
    let (fleet, request) = move_fixture(true)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let dispatch = store
        .attempt(&request.attempt)?
        .and_then(|attempt| attempt.dispatch)
        .ok_or("old reservation")?;
    let clock = now();
    let first = preview_move(&fleet.service, &mut store, request.clone(), clock)?;
    let submission = MoveSubmit {
        plan: first.identity.clone(),
        attempt: request.attempt.clone(),
        fence: request.fence,
        reservation: request.reservation.clone(),
        destination: request.destination,
    };
    submit_move(&fleet.service, &mut store, submission.clone(), clock)?;
    let return_request = kiln_core::fleet::placement::MoveRequest {
        attempt: request.attempt.clone(),
        fence: request.fence,
        reservation: first.new_reservation,
        destination: COORDINATOR.into(),
    };
    let return_plan = preview_move(&fleet.service, &mut store, return_request.clone(), clock)?;
    assert_eq!(dispatch.plan, return_plan.dispatch_plan);
    assert_ne!(dispatch.effect_id, return_plan.new_reservation);
    submit_move(
        &fleet.service,
        &mut store,
        MoveSubmit {
            plan: return_plan.identity,
            attempt: return_request.attempt,
            fence: return_request.fence,
            reservation: return_request.reservation,
            destination: return_request.destination,
        },
        clock,
    )?;
    assert_eq!(
        submit_move(&fleet.service, &mut store, submission, clock)?.state,
        MoveState::Superseded
    );
    let stale_placement = kiln_core::fleet::Placement {
        attempt: request.attempt,
        fence: request.fence,
        work: return_plan.work,
        work_revision: return_plan.work_revision,
        machine: COORDINATOR.into(),
        kind: "omp".into(),
    };
    assert!(
        store
            .fleet_bind_reserved_session(stale_placement, &dispatch.effect_id, clock)
            .is_err()
    );
    Ok(())
}

#[test]
fn queued_move_eligible_matches_admission_for_re_move_and_refused_states() -> TestResult {
    use kiln_core::fair_admission::AdmissionPolicy;
    use kiln_core::operations::FairnessStrategy;
    use kiln_core::types::{Alias, ContentId};
    use kiln_fleet::dispatch::placement::{
        MoveSubmit, SplitSubmit, preview as preview_split, preview_move, submit as submit_split,
        submit_move,
    };
    use kiln_implementer::admission::AdmissionCommand;
    use std::collections::BTreeMap;

    // 1. Positive: an already-moved unstarted attempt remains eligible for re-moving.
    let (fleet, request) = move_fixture(true)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let clock = now();
    let first = preview_move(&fleet.service, &mut store, request.clone(), clock)?;
    submit_move(
        &fleet.service,
        &mut store,
        MoveSubmit {
            plan: first.identity,
            attempt: request.attempt.clone(),
            fence: request.fence,
            reservation: request.reservation,
            destination: request.destination,
        },
        clock,
    )?;
    let attempts = move_route(&fleet, COORDINATOR, "GET", "/v1/attempts", &Value::Null);
    assert_eq!(attempts.status, 200);
    assert_eq!(attempts.body["attempts"][0]["queued_move_eligible"], true);

    // 2. Negative: split-lineage attempts are not eligible for queued move.
    let (split_fleet, split_request) = split_fixture("requirements", false)?;
    let mut split_store = split_fleet.service.store.open_coordinator()?;
    let split_plan =
        preview_split(&split_fleet.service, &mut split_store, split_request.clone(), clock)?;
    submit_split(
        &split_fleet.service,
        &mut split_store,
        SplitSubmit {
            plan: split_plan.identity,
            units: split_request.units,
        },
        clock,
    )?;
    let split_attempts = move_route(&split_fleet, COORDINATOR, "GET", "/v1/attempts", &Value::Null);
    assert_eq!(split_attempts.status, 200);
    for row in split_attempts.body["attempts"]
        .as_array()
        .ok_or("split attempts array")?
    {
        assert_eq!(row["queued_move_eligible"], false);
    }

    // 3. Negative: fair-admission mode (admission policy configured) is ineligible.
    let (fair_fleet, _) = move_fixture(true)?;
    {
        let mut fair_store = fair_fleet.service.store.open_coordinator()?;
        let policy = AdmissionPolicy {
            version: 1,
            accounting_basis: ContentId::from_blake3_hex("a".repeat(64))
                .map_err(|error| error.message)?,
            max_items: 8,
            max_bytes: 100_000,
            max_wait_ms: 1_000,
            max_observation_age_ms: 100,
            max_retry_after_ms: 100,
            max_children: 4,
            max_active: 2,
            strategy: FairnessStrategy::DominantShare,
            shares: BTreeMap::from([(
                Alias::new("tenant").map_err(|error| error.message)?,
                1,
            )]),
            group_limits: vec![],
            serving: None,
        };
        fair_store.admission_apply(AdmissionCommand::Configure { policy }, clock)?;
    }
    let fair_attempts = move_route(&fair_fleet, COORDINATOR, "GET", "/v1/attempts", &Value::Null);
    assert_eq!(fair_attempts.body["attempts"][0]["queued_move_eligible"], false);

    // 4. Negative: bound session makes attempt ineligible.
    let (sess_fleet, sess_request) = move_fixture(true)?;
    {
        let mut sess_store = sess_fleet.service.store.open_coordinator()?;
        let attempt = sess_store
            .attempt(&sess_request.attempt)?
            .ok_or("attempt missing")?;
        sess_store.fleet_bind_reserved_session(
            kiln_core::fleet::Placement {
                attempt: sess_request.attempt,
                fence: sess_request.fence,
                work: attempt.work_identity,
                work_revision: attempt.work_revision,
                machine: COORDINATOR.into(),
                kind: "omp".into(),
            },
            &sess_request.reservation,
            clock,
        )?;
    }
    let sess_attempts = move_route(&sess_fleet, COORDINATOR, "GET", "/v1/attempts", &Value::Null);
    assert_eq!(sess_attempts.body["attempts"][0]["queued_move_eligible"], false);
    Ok(())
}

fn split_fixture(
    second_path: &str,
    dependent: bool,
) -> TestResult<(Fleet, kiln_core::fleet::placement::SplitRequest)> {
    use kiln_core::fleet::placement::{SplitRequest, SplitUnit};
    let mut fleet = fleet(false)?;
    let inventory_path = fleet.root.join("inventory.json");
    let mut inventory: Value = serde_json::from_slice(&fs::read(&inventory_path)?)?;
    fs::create_dir(fleet.state.join(PEER))?;
    fs::create_dir(fleet.root.join("peer-worktrees"))?;
    inventory["machines"]
        .as_array_mut()
        .ok_or("machines array")?
        .push(json!({
            "label": PEER, "herdr_machine": PEER, "ssh_target": "fixture@aspen3",
            "worktree_root": fleet.root.join("peer-worktrees"),
            "repositories": { REPOSITORY: fleet.root.join("checkout") },
            "agents": [{"kind":"omp","slots":1}],
        }));
    fs::write(inventory_path, serde_json::to_vec(&inventory)?)?;
    fleet.dispatcher = Dispatcher::new(Arc::clone(&fleet.service))?;
    let mut store = fleet.service.store.open_coordinator()?;
    let mut other = fleet.work.clone();
    other.title = "Second split unit".into();
    other.permitted_paths = vec![second_path.into()];
    let alias = other.aliases.first_mut().ok_or("fixture alias")?;
    alias.native_id = "onix/kiln#9".into();
    alias.source = kiln_core::types::ExternalRef::new("github", "issue", "onix/kiln#9")
        .map_err(|error| error.message)?;
    let first_id = fleet.work.identity().map_err(|error| error.message)?;
    if dependent {
        other.dependencies.push(first_id.clone());
    }
    store.admit(&other, None)?;
    let mut units = Vec::new();
    for (id, destination) in [
        (first_id, COORDINATOR),
        (other.identity().map_err(|error| error.message)?, PEER),
    ] {
        let attempt = store
            .claim_next_deferred(
                now(),
                fleet.service.config.policy.attempt_lease_ms,
                &|work| {
                    (work.identity().ok().as_ref() != Some(&id))
                        .then(|| "not the selected fixture unit".into())
                },
            )?
            .attempt
            .ok_or("exact fixture work not claimed")?;
        units.push(SplitUnit {
            attempt: attempt.identity,
            fence: attempt.fence,
            destination: destination.into(),
        });
    }
    Ok((fleet, SplitRequest { units }))
}

#[test]
// r[verify kiln.sdlc.workload_placement.scenario.operator]
fn split_reserves_then_observes_exact_two_host_lineage_and_reconciles_lost_start() -> TestResult {
    use kiln_fleet::dispatch::placement::{SplitSubmit, preview, status, submit};
    use kiln_implementer::admission::UnitState;
    let (mut fleet, request) = split_fixture("requirements", false)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let plan = preview(&fleet.service, &mut store, request.clone(), now())?;
    let reversed = kiln_core::fleet::placement::SplitRequest {
        units: request.units.iter().rev().cloned().collect(),
    };
    assert_eq!(preview(&fleet.service, &mut store, reversed, now())?, plan);
    assert!(store.fleet_state()?.claims.claims.is_empty());
    assert!(store.fleet_split_status(&plan.identity, now())?.is_none());
    for unit in &request.units {
        assert!(store.dispatch_plan(&unit.attempt)?.is_none());
    }

    let submission = SplitSubmit {
        plan: plan.identity.clone(),
        units: request.units.clone(),
    };
    let mut changed = submission.clone();
    let [left, right] = changed.units.as_mut_slice() else {
        return Err("fixture needs two units".into());
    };
    std::mem::swap(&mut left.destination, &mut right.destination);
    assert!(
        submit(&fleet.service, &mut store, changed, now())
            .err()
            .ok_or("changed plan accepted")?
            .starts_with("fleet_placement_changed:")
    );
    assert!(store.fleet_state()?.claims.claims.is_empty());
    let requested = submit(&fleet.service, &mut store, submission.clone(), now())?;
    assert_eq!(requested.plan, plan);
    assert!(
        requested
            .units
            .iter()
            .all(|unit| unit.state == UnitState::Requested && unit.session.is_none())
    );
    assert_eq!(store.fleet_state()?.claims.claims.len(), 2);
    assert!(!fleet.state.join("agent").exists());
    assert!(!fleet.state.join(PEER).join("agent").exists());

    // The remote agent starts but its response is lost. Never count intent
    // alone as placement, never redispatch on replay; observe native evidence.
    fs::write(fleet.state.join(PEER).join("start-lost"), "")?;
    fleet.dispatcher.tick(now())?;
    let placing = status(&fleet.service, &mut store, &plan.identity, now())?;
    let remote = &request.units.last().ok_or("remote unit")?.attempt;
    assert!(placing.units.iter().any(|unit| &unit.attempt == remote
        && unit.state == UnitState::ReconcileRequired
        && unit.observed_ms.is_none()));
    assert!(
        placing
            .units
            .iter()
            .any(|unit| &unit.attempt != remote && unit.state == UnitState::Observed)
    );
    assert!(
        submit(&fleet.service, &mut store, submission, now())?
            .units
            .iter()
            .any(|unit| unit.state == UnitState::ReconcileRequired)
    );

    fleet.dispatcher = fleet.restart(Some(COORDINATOR))?;
    let peer_state = fleet.state.join(PEER);
    let branch = fs::read(peer_state.join("branch"))?;
    fs::write(peer_state.join("branch"), "wrong-branch\n")?;
    fleet.dispatcher.tick(now())?;
    assert!(
        status(&fleet.service, &mut store, &plan.identity, now())?
            .units
            .iter()
            .any(|unit| &unit.attempt == remote && unit.state == UnitState::ReconcileRequired)
    );
    fs::write(peer_state.join("branch"), branch)?;
    fleet.dispatcher.tick(now())?;
    let observed = status(&fleet.service, &mut store, &plan.identity, now())?;
    assert!(
        observed
            .units
            .iter()
            .all(|unit| unit.state == UnitState::Observed && unit.observed_ms.is_some())
    );
    for unit in &plan.units {
        let session = store
            .fleet_state()?
            .sessions
            .for_attempt(&unit.attempt)
            .cloned()
            .ok_or("session")?;
        assert_eq!(
            (session.machine, session.fence, session.work),
            (unit.destination.clone(), unit.fence, unit.work.clone())
        );
    }
    let log = fs::read_to_string(fleet.state.join(PEER).join("argv.log"))?;
    assert_eq!(
        log.lines()
            .filter(|line| line.contains("agent start"))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn split_refuses_overlap_unverified_dependencies_stale_fence_and_unavailable_host_without_reservation()
-> TestResult {
    use kiln_fleet::dispatch::placement::preview;
    for (path, dependent, expected) in [
        ("src/value", false, "fleet_placement_overlap:"),
        ("requirements", true, "fleet_placement_dependencies:"),
    ] {
        let (fleet, request) = split_fixture(path, dependent)?;
        let mut store = fleet.service.store.open_coordinator()?;
        assert!(
            preview(&fleet.service, &mut store, request.clone(), now())
                .err()
                .ok_or("unsafe split accepted")?
                .starts_with(expected)
        );
        for unit in request.units {
            assert!(store.dispatch_plan(&unit.attempt)?.is_none());
        }
        assert!(store.fleet_state()?.claims.claims.is_empty());
    }
    let (fleet, request) = split_fixture("requirements", false)?;
    let mut store = fleet.service.store.open_coordinator()?;
    let mut stale = request.clone();
    stale.units.last_mut().ok_or("remote unit")?.fence += 1;
    assert!(
        preview(&fleet.service, &mut store, stale, now())
            .err()
            .ok_or("stale fence accepted")?
            .starts_with("fleet_placement_stale:")
    );
    let mut unavailable = request.clone();
    unavailable
        .units
        .last_mut()
        .ok_or("remote unit")?
        .destination = "unknown-host".into();
    assert!(
        preview(&fleet.service, &mut store, unavailable, now())
            .err()
            .ok_or("unenrolled destination accepted")?
            .starts_with("fleet_placement_unavailable:")
    );
    fs::write(fleet.state.join(PEER).join("agent"), "kiln-000000000000\n")?;
    assert!(
        preview(&fleet.service, &mut store, request.clone(), now())
            .err()
            .ok_or("full destination accepted")?
            .starts_with("fleet_placement_unavailable:")
    );
    for unit in request.units {
        assert!(store.dispatch_plan(&unit.attempt)?.is_none());
    }
    Ok(())
}

fn now() -> u64 {
    kiln_fleet::log::now_ms()
}

impl Fleet {
    fn session(&self) -> TestResult<kiln_core::fleet::FleetSession> {
        let state = self.service.store.open_coordinator()?.fleet_state()?;
        Ok(state
            .sessions
            .sessions
            .values()
            .next()
            .cloned()
            .ok_or("no session")?)
    }

    /// The coordinator restarted after a crash: a new service and dispatcher
    /// over the same ledger and herdr state, under `authority_operator`, or
    /// under the absent profile when there is none.
    fn restart(&self, authority_operator: Option<&str>) -> TestResult<Dispatcher> {
        let mut document = self.document.clone();
        let object = document
            .as_object_mut()
            .ok_or("configuration is not an object")?;
        match authority_operator {
            Some(operator) => object.insert("authority_operator".into(), json!(operator)),
            None => object.remove("authority_operator"),
        };
        let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
            &document,
        )?)?)?);
        Ok(Dispatcher::new(service)?)
    }

    fn handoff(&self, machine: &str, attempt: &str, fence: u64, commit: &str) -> u16 {
        let request = Request {
            method: "POST".into(),
            path: format!("/v1/attempts/{attempt}/handoff"),
            query: None,
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
        let mut store = self.service.store.open_coordinator()?;
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
    assert_eq!(
        fs::read_to_string(export.join("tree/docs/tracked.txt"))?,
        "kept\n"
    );
    assert_eq!(
        fs::read_to_string(export.join("tree/version.txt"))?,
        "commit=$Format:%H$\n"
    );
    assert!(!export.join("tree/.git").exists());
    let mode = |path: &str| -> TestResult<u32> {
        Ok(fs::metadata(export.join(path))?.permissions().mode() & 0o7777)
    };
    assert_eq!(mode("tree/src/value")?, 0o644);
    assert_eq!(mode("tree/check")?, 0o755);
    let receipt: HerdrCandidateReceipt =
        serde_json::from_slice(&fs::read(export.join("receipt.json"))?)?;
    let mut store = fleet.service.store.open_coordinator()?;
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
    let mut store = fleet.service.store.open_coordinator()?;
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
    let mut store = fleet.service.store.open_coordinator()?;
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
        query: None,
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
            query: None,
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
    let object = document
        .as_object_mut()
        .ok_or("configuration must be an object")?;
    object.insert(
        "dispatch".into(),
        json!({ "enabled": true, "kinds": ["omp"] }),
    );
    object.insert(
        "capture".into(),
        json!({
            "git": "/bin/git", "tar": "/bin/tar", "export_root": root,
            "verification_profiles": root, "repositories": {},
        }),
    );
    let mut service = Service::new(FleetConfig::from_json(&serde_json::to_vec(&document)?)?)?;
    service.clock = || ROSTER_CLOCK.get();
    let service = Arc::new(service);
    let mut dispatcher = Dispatcher::new(Arc::clone(&service))?;
    let read_roster = |now_ms| {
        let response = service.route(
            PEER,
            &Request {
                method: "GET".into(),
                path: "/v1/machines".into(),
                query: None,
                body: Vec::new(),
            },
            now_ms,
        );
        assert_eq!(response.status, 200);
        response.body
    };
    let not_reporting = |machine: &str, placement_configured: bool| {
        let error_code = if placement_configured {
            "not_reporting"
        } else {
            "placement_not_configured"
        };
        let error_detail = if placement_configured {
            "machine has not reported occupancy yet"
        } else {
            "machine is not configured in herdr inventory"
        };
        let inv = if !placement_configured {
            Value::Null
        } else if machine == PEER {
            json!({
                "herdr_machine": "private-route",
                "ssh_target": "private-target",
                "worktree_root": root,
                "repositories": [],
                "agents": [],
            })
        } else {
            json!({
                "herdr_machine": null,
                "ssh_target": null,
                "worktree_root": root,
                "repositories": [],
                "agents": [{ "kind": "omp", "slots": 1 }],
            })
        };
        json!({
            "machine": machine,
            "placement_configured": placement_configured,
            "herdr_windows_configured": placement_configured,
            "state": "not_reporting",
            "observed_at_ms": null,
            "last_seen_at_ms": null,
            "sessions": null,
            "health": "unavailable",
            "error": { "class": "unavailable", "code": error_code, "detail": error_detail },
            "slots": null,
            "active_attempts": [],
            "draining": false,
            "drain_reason": null,
            "drained_at_ms": null,
            "inventory": inv,
        })
    };
    assert_eq!(
        read_roster(1_000),
        json!({ "machines": [
        not_reporting("aspen1", false), not_reporting(PEER, true), not_reporting(COORDINATOR, true),
    ] })
    );
    let log = root.join("roster.log");
    assert!(!log.exists());

    // The foreign agent contributes to the total without filling a Kiln slot.
    ROSTER_CLOCK.set(2_000);
    assert_eq!(dispatcher.tick(2_000)?.free_slots, 1);
    assert_eq!(
        read_roster(2_001),
        json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "herdr_windows_configured": true, "state": "reporting",
          "observed_at_ms": 2_000, "last_seen_at_ms": 2_000, "sessions": 0,
          "health": "available", "error": null, "slots": {},
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": "private-route", "ssh_target": "private-target", "worktree_root": root, "repositories": [], "agents": [] } },
        { "machine": COORDINATOR, "placement_configured": true, "herdr_windows_configured": true, "state": "reporting",
          "observed_at_ms": 2_000, "last_seen_at_ms": 2_000, "sessions": 1,
          "health": "available", "error": null, "slots": { "omp": { "used": 0, "capacity": 1, "free": 1 } },
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": null, "ssh_target": null, "worktree_root": root, "repositories": [], "agents": [{ "kind": "omp", "slots": 1 }] } },
    ] })
    );

    ROSTER_CLOCK.set(3_000);
    fs::write(root.join("unreachable"), "")?;
    dispatcher.tick(3_000)?;
    let failure = read_roster(3_001);
    assert_eq!(
        failure,
        json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "herdr_windows_configured": true, "state": "unreachable",
          "observed_at_ms": 3_000, "last_seen_at_ms": 2_000, "sessions": null,
          "health": "unavailable", "error": { "class": "unavailable", "code": null, "detail": "private remote failure details" }, "slots": null,
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": "private-route", "ssh_target": "private-target", "worktree_root": root, "repositories": [], "agents": [] } },
        { "machine": COORDINATOR, "placement_configured": true, "herdr_windows_configured": true, "state": "reporting",
          "observed_at_ms": 3_000, "last_seen_at_ms": 3_000, "sessions": 1,
          "health": "available", "error": null, "slots": { "omp": { "used": 0, "capacity": 1, "free": 1 } },
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": null, "ssh_target": null, "worktree_root": root, "repositories": [], "agents": [{ "kind": "omp", "slots": 1 }] } },
    ] })
    );

    let stale = read_roster(3_000 + service.config.policy.tick_ms * 3);
    assert_eq!(
        stale,
        json!({ "machines": [
        not_reporting("aspen1", false),
        { "machine": PEER, "placement_configured": true, "herdr_windows_configured": true, "state": "stale",
          "observed_at_ms": 3_000, "last_seen_at_ms": 2_000, "sessions": null,
          "health": "unavailable", "error": { "class": "unavailable", "code": "observation_stale", "detail": "machine occupancy observation exceeded its stale window" }, "slots": null,
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": "private-route", "ssh_target": "private-target", "worktree_root": root, "repositories": [], "agents": [] } },
        { "machine": COORDINATOR, "placement_configured": true, "herdr_windows_configured": true, "state": "stale",
          "observed_at_ms": 3_000, "last_seen_at_ms": 3_000, "sessions": null,
          "health": "unavailable", "error": { "class": "unavailable", "code": "observation_stale", "detail": "machine occupancy observation exceeded its stale window" }, "slots": null,
          "active_attempts": [], "draining": false, "drain_reason": null, "drained_at_ms": null,
          "inventory": { "herdr_machine": null, "ssh_target": null, "worktree_root": root, "repositories": [], "agents": [{ "kind": "omp", "slots": 1 }] } },
    ] })
    );
    assert_eq!(
        fs::read_to_string(log)?,
        concat!(
            "--session kiln-test agent list\n--machine private-route agent list\n",
            "--session kiln-test agent list\n--machine private-route agent list\n",
        )
    );
    Ok(())
}

/// Where a coordinator crash loses the outcome of an agent's launch.
#[derive(Clone, Copy, Debug)]
enum Lost {
    /// After `start_agent` created the agent, before its reply was recorded:
    /// the session stays `Placing` with an agent already running.
    StartReply,
    /// After the start was recorded, before the first prompt was delivered:
    /// the session stays `Starting`.
    FirstPrompt,
}

impl Lost {
    const ALL: [Self; 2] = [Self::StartReply, Self::FirstPrompt];
}

/// Claim and place the interactive work, losing the launch at `lost`.
fn interrupted(lost: Lost) -> TestResult<Fleet> {
    let mut fleet = fleet(false)?;
    let (flag, state) = match lost {
        Lost::StartReply => ("start-lost", SessionState::Placing),
        Lost::FirstPrompt => ("prompt-lost", SessionState::Starting),
    };
    fs::write(fleet.state.join(flag), "")?;
    fleet.dispatcher.tick(now())?;
    fleet.dispatcher.tick(now())?;
    fs::remove_file(fleet.state.join(flag))?;
    let session = fleet.session()?;
    assert_eq!(session.state, state, "{lost:?}: {session:?}");
    assert!(
        fleet.state.join("agent").is_file(),
        "{lost:?}: agent started"
    );
    assert!(!fleet.state.join("prompt.txt").exists(), "{lost:?}");
    Ok(fleet)
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.denied]
fn a_recovered_agent_is_never_prompted_once_its_dispatch_is_refused() -> TestResult {
    for lost in Lost::ALL {
        let fleet = interrupted(lost)?;
        let before = fleet.session()?.state;
        // The restarted coordinator selects a profile that refuses Dispatch,
        // as a revoked grant does.
        let mut recovered = fleet.restart(None)?;
        drop(recovered.tick(now()));
        assert!(
            !fleet.state.join("prompt.txt").exists(),
            "{lost:?}: a refused launch must not prompt the agent"
        );
        assert_eq!(fleet.session()?.state, before, "{lost:?}");
    }
    Ok(())
}

#[test]
// r[verify kiln.implementation.authority.dispatch.scenario.admitted]
fn a_recovered_agent_is_prompted_while_its_dispatch_is_admitted() -> TestResult {
    for lost in Lost::ALL {
        let fleet = interrupted(lost)?;
        let mut recovered = fleet.restart(Some(COORDINATOR))?;
        recovered.tick(now())?;
        let session = fleet.session()?;
        assert_eq!(
            session.state,
            SessionState::Prompted,
            "{lost:?}: {session:?}"
        );
        let prompt = fs::read_to_string(fleet.state.join("prompt.txt"))?;
        assert!(prompt.contains(session.attempt.as_str()), "{lost:?}");
    }
    Ok(())
}

#[test]
// r[verify kiln.fleet.live_pane_and_machine_census]
// r[verify kiln.fleet.live_pane_and_machine_census.scenario.pane_read]
fn drain_and_dispatch_pause_prevent_new_dispatches_and_pane_steering_works() -> TestResult {
    let mut fleet = fleet(false)?;
    let call =
        |machine: &str, method: &str, path: &str, query: Option<&str>, body: Option<Value>| {
            fleet.service.route(
                machine,
                &Request {
                    method: method.to_owned(),
                    path: path.to_owned(),
                    query: query.map(str::to_owned),
                    body: body
                        .map(|value| value.to_string().into_bytes())
                        .unwrap_or_default(),
                },
                now(),
            )
        };

    // 1. Pause dispatch globally: tick runs observations/sweeps, launches no new attempt.
    let paused = call(
        COORDINATOR,
        "POST",
        "/v1/dispatch/pause",
        None,
        Some(json!({ "reason": "maintenance window", "request_id": "pause-1" })),
    );
    assert_eq!(paused.status, 200);
    assert_eq!(paused.body["state"], "paused");
    assert_eq!(paused.body["paused_reason"], "maintenance window");

    let report = fleet.dispatcher.tick(now())?;
    assert_eq!(report.free_slots, 0);
    assert!(report.performed.is_empty());

    let status = call(PEER, "GET", "/v1/dispatch/status", None, None);
    assert_eq!(status.status, 200);
    assert_eq!(status.body["state"], "paused");
    assert_eq!(
        status
            .body
            .pointer("/last_tick/deferral_counts/dispatch is paused: maintenance window"),
        Some(&json!(1))
    );

    // Non-operator cannot resume or drain.
    assert_eq!(
        call(PEER, "POST", "/v1/dispatch/resume", None, None).status,
        403
    );
    assert_eq!(
        call(
            PEER,
            "POST",
            &format!("/v1/machines/{COORDINATOR}/drain"),
            None,
            Some(json!({ "draining": true, "reason": "nope" }))
        )
        .status,
        403
    );

    // 2. Resume dispatch, then drain the only configured machine: tick still defers new dispatch.
    let resumed = call(
        COORDINATOR,
        "POST",
        "/v1/dispatch/resume",
        None,
        Some(json!({ "request_id": "resume-1" })),
    );
    assert_eq!(resumed.status, 200);
    assert_eq!(resumed.body["state"], "running");

    let drained = call(
        COORDINATOR,
        "POST",
        &format!("/v1/machines/{COORDINATOR}/drain"),
        None,
        Some(json!({ "draining": true, "reason": "rebooting host", "request_id": "drain-1" })),
    );
    assert_eq!(drained.status, 200);
    assert_eq!(drained.body["draining"], true);
    assert_eq!(drained.body["drain_reason"], "rebooting host");

    let report = fleet.dispatcher.tick(now())?;
    assert_eq!(report.free_slots, 0);
    assert!(report.performed.is_empty());

    let status = call(COORDINATOR, "GET", "/v1/dispatch/status", None, None);
    assert_eq!(status.body["draining_machines"], json!([COORDINATOR]));
    assert_eq!(
        status
            .body
            .pointer("/last_tick/deferral_counts/all available machines are draining"),
        Some(&json!(1))
    );

    // 3. Un-drain machine and run two ticks to claim + place + start + prompt session.
    let undrained = call(
        COORDINATOR,
        "POST",
        &format!("/v1/machines/{COORDINATOR}/drain"),
        None,
        Some(json!({ "draining": false, "request_id": "drain-2" })),
    );
    assert_eq!(undrained.status, 200);
    assert_eq!(undrained.body["draining"], false);

    fleet.dispatcher.tick(now())?;
    fleet.dispatcher.tick(now())?;
    let session = fleet.session()?;
    let attempt_id = session.attempt.as_str().to_owned();
    assert_eq!(session.state, SessionState::Prompted);

    // 4. Third tick observes live herdr occupancy: 1/1 omp slot used + active attempt.
    fleet.dispatcher.tick(now())?;
    let machines = call(PEER, "GET", "/v1/machines", None, None);
    assert_eq!(machines.status, 200);
    let coord_row = machines.body["machines"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["machine"] == COORDINATOR))
        .ok_or("missing coordinator machine row")?;
    assert_eq!(coord_row["health"], "available");
    assert_eq!(
        coord_row.pointer("/slots/omp"),
        Some(&json!({ "used": 1, "capacity": 1, "free": 0 }))
    );
    assert_eq!(coord_row["active_attempts"][0]["attempt"], attempt_id);

    // 5. Read bounded, ANSI-stripped live pane lines; reject out-of-range lines queries.
    let pane = call(
        PEER,
        "GET",
        &format!("/v1/attempts/{attempt_id}/pane"),
        Some("lines=2"),
        None,
    );
    assert_eq!(pane.status, 200);
    assert_eq!(pane.body["machine"], COORDINATOR);
    assert_eq!(pane.body["truncated"], true);
    assert_eq!(
        pane.body["lines"],
        json!(["question line 2", "question line 3"])
    );
    assert_eq!(pane.body["text"], json!("question line 2\nquestion line 3"));
    let full_pane = call(
        PEER,
        "GET",
        &format!("/v1/attempts/{attempt_id}/pane"),
        None,
        None,
    );
    assert_eq!(full_pane.status, 200);
    assert_eq!(full_pane.body["truncated"], false);
    assert_eq!(full_pane.body["lines"][0], "blocked> question line 1");

    for bad_query in ["lines=0", "lines=1001", "lines=abc", "unknown=1"] {
        assert_eq!(
            call(
                PEER,
                "GET",
                &format!("/v1/attempts/{attempt_id}/pane"),
                Some(bad_query),
                None,
            )
            .status,
            400,
            "{bad_query}"
        );
    }

    // Offline herdr failure returns explicit 503 with failure metadata.
    fs::write(fleet.state.join("pane-offline"), "")?;
    let offline = call(
        PEER,
        "GET",
        &format!("/v1/attempts/{attempt_id}/pane"),
        None,
        None,
    );
    assert_eq!(offline.status, 503);
    assert_eq!(offline.body["code"], "fleet_herdr_unavailable");
    assert_eq!(
        offline.body.pointer("/failure/code"),
        Some(&json!("server_not_running"))
    );
    fs::remove_file(fleet.state.join("pane-offline"))?;

    // 6. Prompt & interrupt steering + request_id deduplication and conflict check.
    let prompt_resp = call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt_id}/prompt"),
        None,
        Some(json!({ "text": "Use BLAKE3 for ids", "request_id": "steer-1" })),
    );
    assert_eq!(prompt_resp.status, 200);
    assert_eq!(prompt_resp.body["prompted"], true);
    assert_eq!(
        fs::read_to_string(fleet.state.join("prompt.txt"))?,
        "Use BLAKE3 for ids"
    );

    let prompt_count_before = fs::read_to_string(fleet.state.join("prompts.log"))?
        .lines()
        .count();
    let replay = call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt_id}/prompt"),
        None,
        Some(json!({ "text": "Use BLAKE3 for ids", "request_id": "steer-1" })),
    );
    assert_eq!(replay.status, 200);
    assert_eq!(
        fs::read_to_string(fleet.state.join("prompts.log"))?
            .lines()
            .count(),
        prompt_count_before
    );
    let conflict = call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt_id}/prompt"),
        None,
        Some(json!({ "text": "Different text", "request_id": "steer-1" })),
    );
    assert_eq!(conflict.status, 409);

    let interrupt_resp = call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt_id}/interrupt"),
        None,
        Some(json!({ "reason": "stop runaway loop", "request_id": "int-1" })),
    );
    assert_eq!(interrupt_resp.status, 200);
    assert_eq!(interrupt_resp.body["interrupted"], true);
    assert_eq!(
        fs::read_to_string(fleet.state.join("keys.log"))?.trim(),
        "ctrl+c"
    );

    // 7. Ended/cancelled attempt refuses pane/prompt/interrupt cleanly with 409.
    let cancelled = call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt_id}/cancel"),
        None,
        Some(json!({ "reason": "operator cancel" })),
    );
    assert_eq!(cancelled.status, 200);
    for (method, suffix, body) in [
        ("GET", "pane", None),
        ("POST", "prompt", Some(json!({ "text": "hello" }))),
        ("POST", "interrupt", Some(json!({ "reason": "stop" }))),
    ] {
        let resp = call(
            COORDINATOR,
            method,
            &format!("/v1/attempts/{attempt_id}/{suffix}"),
            None,
            body,
        );
        assert_eq!(resp.status, 409, "{method} {suffix}");
    }

    Ok(())
}
