//! Single-host KVM qualification of the pinned Lattice worker guest (Kiln #51
//! and #52, milestone M1) through the existing retained Cloud Hypervisor shell.
//!
//! Prerequisites: `/dev/kvm`, a systemd user manager with cgroup delegation,
//! Nix-store `systemd-run`/`systemctl`/`env`/`debugfs` on `PATH`, and
//! `KILN_LATTICE_WORKER_GUEST` pointing at `nix build .#lattice-worker-guest`.
//! Without them the test prints an explicit `SKIP` reason and runs no VM; set
//! `KILN_MICROVM_REQUIRED=1` to turn a missing prerequisite into a failure.
//!
//! The retained shell needs a `RetainedAuthority`. The production selected
//! Aspen authority refuses, so this test supplies `SingleHostTestReservation`,
//! which only checks that every placement names this one test host. It is a
//! test fixture: it is not placement ownership, distributed authority, or
//! volume/effect fencing, and the evidence printed here does not claim them.
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use kiln_core::types::ContentId;
use kiln_hosted::execution::{Backend, Config};
use kiln_hosted::microvm::lattice_worker::{self, DispatchOutcome};
use kiln_hosted::microvm::retained::{
    self, Action, Binding, Operation, Outcome, RetainedAuthority,
};
use kiln_microvm::lattice_worker::{
    self as core, AttemptRequest, GuestManifest, GuestMessage, HostMessage, LATTICE_WORKER_PORT,
    Report, ReportOutcome,
};
use kiln_microvm::retained::{
    Placement, RetainedResources, RetainedSession, RetainedState, SessionIdentity,
};
use rusqlite::Connection;
use serde_json::{Value, json};

const HOST: &str = "kvm-single-host";
const BOOT_TIMEOUT_MS: u64 = 60_000;
const ATTEMPT_LIFETIME_MS: u64 = 600_000;
const GUEST_WAIT: Duration = Duration::from_secs(60);

fn fail(context: &str, error: impl std::fmt::Debug) -> ! {
    unreachable!("{context}: {error:?}")
}

/// Test-only single-host reservation; see the module documentation.
#[derive(Debug)]
struct SingleHostTestReservation;

impl RetainedAuthority for SingleHostTestReservation {
    fn with_reservation<T>(
        &mut self,
        operation: &Operation,
        effect: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if operation.session.placement.host != HOST {
            return Err("test reservation admits only its own single host".into());
        }
        effect()
    }
}

fn store_executable(path: &Path) -> Result<Value, String> {
    let mut current = path.to_path_buf();
    for _ in 0..16 {
        if current.starts_with("/nix/store") {
            let bytes = fs::read(&current).map_err(|e| format!("{}: {e}", current.display()))?;
            return Ok(
                json!({"path": current, "blake3": blake3::hash(&bytes).to_hex().to_string()}),
            );
        }
        let target = fs::read_link(&current)
            .map_err(|_| format!("{} is not a Nix-store executable", path.display()))?;
        current = current
            .parent()
            .map(|parent| parent.join(&target))
            .unwrap_or(target);
    }
    Err(format!("{} symlink chain too long", path.display()))
}

fn tool(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").ok_or("PATH unset")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name} not on PATH"))
}

struct Host {
    manifest: GuestManifest,
    image: ContentId,
    config: Config,
    backend: Backend,
    debugfs: PathBuf,
    _work: tempfile::TempDir,
}

// r[impl kiln.lattice.worker.session.scenario.unavailable]
fn prerequisites() -> Result<Host, String> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .map_err(|e| format!("/dev/kvm unavailable: {e}"))?;
    let guest = std::env::var_os("KILN_LATTICE_WORKER_GUEST").ok_or(
        "KILN_LATTICE_WORKER_GUEST unset; build it with `nix build .#lattice-worker-guest`",
    )?;
    let (manifest, image) =
        lattice_worker::load_manifest(&Path::new(&guest).join("manifest.json"))?;
    let uid = fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .map_err(|e| e.to_string())?;
    let runtime = PathBuf::from(format!("/run/user/{uid}"));
    if !runtime.join("systemd").is_dir() {
        return Err(format!("no systemd user manager at {}", runtime.display()));
    }
    let debugfs = tool("debugfs")?;
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    fs::set_permissions(work.path(), fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let backend = lattice_worker::backend(&manifest, BOOT_TIMEOUT_MS);
    let config: Config = serde_json::from_value(json!({
        "systemd_run": store_executable(&tool("systemd-run")?)?,
        "systemctl": store_executable(&tool("systemctl")?)?,
        "env": store_executable(&tool("env")?)?,
        "manager_runtime_directory": runtime, "work_root": work.path(),
        "runners": {"lattice-worker": backend},
        "maximum_concurrent": 2, "memory_bytes": 2_147_483_648_u64, "processes": 256,
        "cpu_percent": 200, "maximum_timeout_ms": 900_000, "maximum_output_bytes": 262_144,
    }))
    .map_err(|e| format!("execution profile: {e}"))?;
    Ok(Host {
        manifest,
        image,
        config,
        backend,
        debugfs,
        _work: work,
    })
}

fn content(label: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(label.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|e| fail("content id", e))
}

fn now_ms() -> u64 {
    kiln_hosted::now_ms().unwrap_or_else(|e| fail("clock", e))
}

/// One retained Lattice worker session and its exact current record.
struct Session {
    label: String,
    current: RetainedSession,
    binding: Binding,
    request: AttemptRequest,
    operations: u64,
}

impl Session {
    fn new(host: &Host, run: &str, label: &str, slot: u16, workflow: &str) -> Self {
        let directory = PathBuf::from(format!("/tmp/klw-{run}-{label}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap_or_else(|e| fail("vm directory", e));
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|e| fail("vm directory", e));
        let attempt = content(&format!("{run}/{label}/attempt"));
        let execution = content(&format!("{run}/{label}/execution"));
        let deadline = now_ms() + ATTEMPT_LIFETIME_MS;
        let request = core::request(
            &host.manifest,
            attempt.clone(),
            execution.clone(),
            workflow,
            1,
            deadline,
        )
        .unwrap_or_else(|e| fail("attempt request", e));
        let current = RetainedSession::admit(
            SessionIdentity {
                session: content(&format!("{run}/{label}/session")),
                execution,
                vm: content(&format!("{run}/{label}/vm")),
                work: content(&format!("{run}/{label}/work")),
                work_revision: 1,
                attempt,
                fence: 1,
            },
            Placement {
                host: HOST.into(),
                generation: 1,
            },
            RetainedResources {
                vcpus: u16::from(host.manifest.resources.vcpus),
                memory_mib: host.manifest.resources.memory_mib,
                slot,
            },
        )
        .unwrap_or_else(|e| fail("retained admission", e));
        let profile = retained::observed_profile(&host.config, &host.backend)
            .unwrap_or_else(|e| fail("observed CH profile", e))
            .identity;
        Self {
            label: label.into(),
            current,
            binding: Binding {
                workspace: directory.join("workspace.img"),
                directory,
                unit: format!("kiln-retained-lw-{run}-{label}"),
                disk_generation: 1,
                deadline_unix_ms: deadline,
                profile,
            },
            request,
            operations: 0,
        }
    }

    fn operation(&mut self, action: Action) -> Operation {
        self.operations += 1;
        Operation {
            operation: content(&format!(
                "{}/{}/operation/{}",
                self.binding.unit, self.label, self.operations
            )),
            session: self.current.clone(),
            binding: self.binding.clone(),
            action,
        }
    }

    fn observe(&mut self, outcome: Result<Outcome, String>, expected: RetainedState) {
        match outcome {
            Ok(Outcome::Observed(session)) if session.state == expected => {
                self.current = *session;
            }
            other => fail(&format!("{} expected {expected:?}", self.label), other),
        }
    }
}

fn execute(
    db: &mut Connection,
    host: &Host,
    session: &mut Session,
    action: Action,
    expected: RetainedState,
) -> Operation {
    let operation = session.operation(action);
    let outcome = retained::execute(
        db,
        &host.config,
        &host.backend,
        &mut SingleHostTestReservation,
        &operation,
    );
    session.observe(outcome, expected);
    operation
}

fn database(path: &Path) -> Connection {
    let db = Connection::open(path).unwrap_or_else(|e| fail("database", e));
    db.pragma_update(None, "synchronous", "FULL")
        .unwrap_or_else(|e| fail("pragma", e));
    retained::init(&db).unwrap_or_else(|e| fail("retained init", e));
    lattice_worker::init(&db).unwrap_or_else(|e| fail("dispatch init", e));
    db
}

fn boot(db: &mut Connection, host: &Host, session: &mut Session) {
    lattice_worker::create_workspace(&session.binding.workspace, &host.manifest)
        .unwrap_or_else(|e| fail("workspace", e));
    let registration = session.operation(Action::ColdStart);
    retained::register(db, &mut SingleHostTestReservation, &registration)
        .unwrap_or_else(|e| fail("register", e));
    let started = Instant::now();
    session.observe(
        retained::execute(
            db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &registration,
        ),
        RetainedState::Paused,
    );
    let cold = started.elapsed();
    execute(db, host, session, Action::Resume, RetainedState::Running);
    println!(
        "KILN_LATTICE_WORKER_EVIDENCE {}",
        json!({"event":"cold-start-paused-then-running","session":session.label,
            "cold_create_boot_pause_ms":cold.as_millis(),"disk_image_type":"Raw"})
    );
}

fn dispatch(db: &mut Connection, host: &Host, session: &mut Session) -> Report {
    let operation = session.operation(Action::Resume);
    match lattice_worker::dispatch(
        db,
        &host.config,
        &host.backend,
        &operation,
        &host.manifest,
        &session.request,
    ) {
        Ok(DispatchOutcome::Reported(report)) => *report,
        other => fail("dispatch", other),
    }
}

fn assert_succeeded(report: &Report, workflow: &str) {
    match &report.outcome {
        ReportOutcome::Ran {
            exit_code: Some(0),
            summary: Some(summary),
            ..
        } => {
            assert_eq!(summary.workflow_name, workflow);
            assert_eq!(summary.status, "succeeded");
            assert_eq!(summary.node_count, 3);
            assert_eq!(summary.completed, 3);
            assert_eq!(summary.failures, 0);
        }
        other => fail("Lattice run did not succeed", other),
    }
}

/// Send one dispatch frame directly, bypassing host admission, to observe the
/// guest's own admission. Returns the guest's report.
fn raw_dispatch(session: &Session, request: AttemptRequest) -> Report {
    let path = session
        .binding
        .directory
        .join(format!("vsock.sock_{LATTICE_WORKER_PORT}"));
    let listener = UnixListener::bind(&path).unwrap_or_else(|e| fail("listener", e));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|e| fail("listener", e));
    let deadline = Instant::now() + GUEST_WAIT;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => fail("guest did not connect", error),
        }
    };
    stream
        .set_nonblocking(false)
        .unwrap_or_else(|e| fail("stream", e));
    stream
        .set_read_timeout(Some(GUEST_WAIT))
        .unwrap_or_else(|e| fail("timeout", e));
    let read = |stream: &mut std::os::unix::net::UnixStream| -> GuestMessage {
        let mut prefix = [0_u8; 4];
        stream
            .read_exact(&mut prefix)
            .unwrap_or_else(|e| fail("frame", e));
        let mut body = vec![0_u8; core::frame_length(prefix).unwrap_or_else(|e| fail("len", e))];
        stream
            .read_exact(&mut body)
            .unwrap_or_else(|e| fail("frame", e));
        core::decode(&body).unwrap_or_else(|e| fail("decode", e))
    };
    let hello = read(&mut stream);
    assert!(matches!(hello, GuestMessage::Hello { attempt: None, .. }));
    let frame = core::encode(HostMessage::Dispatch {
        request: Box::new(request),
        clock_unix_ms: now_ms(),
        entropy_hex: "00".repeat(core::ENTROPY_BYTES),
    })
    .unwrap_or_else(|e| fail("encode", e));
    stream
        .write_all(&frame)
        .unwrap_or_else(|e| fail("write", e));
    let reply = read(&mut stream);
    drop(stream);
    drop(listener);
    let _ = fs::remove_file(&path);
    match reply {
        GuestMessage::Report(report) => report,
        other => fail("guest reply", other),
    }
}

fn debugfs_cat(host: &Host, image: &Path, path: &str) -> String {
    let output = Command::new(&host.debugfs)
        .args(["-R", &format!("cat {path}")])
        .arg(image)
        .output()
        .unwrap_or_else(|e| fail("debugfs", e));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn retire(db: &mut Connection, host: &Host, session: &mut Session, checkpoint: Option<&Path>) {
    execute(db, host, session, Action::Pause, RetainedState::Paused);
    if let Some(checkpoint) = checkpoint {
        fs::create_dir(checkpoint).unwrap_or_else(|e| fail("checkpoint dir", e));
        fs::set_permissions(checkpoint, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|e| fail("checkpoint dir", e));
        execute(
            db,
            host,
            session,
            Action::Snapshot {
                checkpoint: checkpoint.to_path_buf(),
            },
            RetainedState::Paused,
        );
    }
    execute(db, host, session, Action::Retire, RetainedState::Retired);
}

// r[verify kiln.lattice.worker.session.scenario.lifecycle]
// r[verify kiln.lattice.worker.session.scenario.stale]
// r[verify kiln.lattice.worker.session.scenario.unavailable]
// r[verify kiln.lattice.worker.guest.scenario.executed]
// r[verify kiln.lattice.worker.guest.scenario.isolated]
// r[verify kiln.lattice.worker.guest.scenario.refused]
#[test]
fn lattice_worker_guest_runs_retained_single_host_lifecycle_on_kvm() {
    let host = match prerequisites() {
        Ok(host) => host,
        Err(error) => {
            assert!(
                std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
                "required Lattice worker KVM qualification prerequisites unavailable: {error}"
            );
            eprintln!("SKIP Lattice worker KVM qualification: {error}; no VM was started");
            return;
        }
    };
    let run = std::process::id().to_string();
    let state = tempfile::tempdir().unwrap_or_else(|e| fail("state", e));
    let db_path = state.path().join("retained.sqlite");
    let mut db = database(&db_path);

    // Host admission refuses a crossed attempt before any VM or workspace effect.
    let mut a = Session::new(&host, &run, "a", 1, "log-triage");
    let mut b = Session::new(&host, &run, "b", 2, "workspace-journal");
    let mut crossed = a.request.clone();
    crossed.environment = content("another environment");
    assert!(core::admit_attempt(&host.manifest, &crossed, now_ms()).is_err());
    let mut wrong_image = b.request.clone();
    wrong_image.image = content("another image");
    assert!(core::admit_attempt(&host.manifest, &wrong_image, now_ms()).is_err());
    assert!(!a.binding.workspace.exists());

    // Two concurrent attempts on the same immutable artifacts.
    boot(&mut db, &host, &mut a);
    boot(&mut db, &host, &mut b);

    // Guest-side admission refuses a crossed dispatch and stays undispatched.
    let mut swapped = b.request.clone();
    swapped.definition = a.request.definition.clone();
    let refused = raw_dispatch(&b, swapped);
    assert!(
        matches!(&refused.outcome, ReportOutcome::Refused { code, .. } if code == "lattice_worker_workflow_mismatch"),
        "{refused:?}"
    );

    let report_a = dispatch(&mut db, &host, &mut a);
    assert_succeeded(&report_a, "log-triage");
    let report_b = dispatch(&mut db, &host, &mut b);
    assert_succeeded(&report_b, "workspace-journal");
    assert_ne!(report_a.guest_identity, report_b.guest_identity);
    assert_eq!(a.request.image, b.request.image);
    assert_eq!(a.request.image, host.image);

    // Stale owner and changed binding are refused before any host effect.
    let mut stale = a.operation(Action::Pause);
    stale.session.placement.generation += 1;
    assert!(
        retained::execute(
            &mut db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &stale
        )
        .is_err()
    );
    let mut foreign = a.operation(Action::Pause);
    foreign.session.placement.host = "another-host".into();
    assert!(
        retained::execute(
            &mut db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &foreign
        )
        .is_err()
    );

    // Controller restart: a new connection over the same journal returns the
    // recorded dispatch and control observations without redispatch.
    drop(db);
    let mut db = database(&db_path);
    let listener_path = a
        .binding
        .directory
        .join(format!("vsock.sock_{LATTICE_WORKER_PORT}"));
    let replayed = dispatch(&mut db, &host, &mut a);
    assert_eq!(replayed, report_a);
    assert!(
        !listener_path.exists(),
        "replayed dispatch opened a listener"
    );

    // A: pause, paired snapshot, observed retirement and capacity release.
    let checkpoint = a.binding.directory.join("checkpoint");
    retire(&mut db, &host, &mut a, Some(&checkpoint));
    let retired: i64 = db
        .query_row(
            "SELECT occupied FROM hosted_retained_session WHERE session=?1",
            [a.current.identity.session.as_str()],
            |row| row.get(0),
        )
        .unwrap_or(-1);
    assert_eq!(retired, 0);

    // A retired execution cannot cold start again.
    let restart = a.operation(Action::ColdStart);
    assert!(
        retained::execute(
            &mut db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &restart
        )
        .is_err()
    );

    // B's mutable workspace is observed independently of the guest.
    retire(&mut db, &host, &mut b, None);
    let journal = debugfs_cat(&host, &b.binding.workspace, "/checkout/out/journal.txt");
    assert!(journal.contains(&report_b.guest_identity), "{journal}");
    assert!(!journal.contains(&report_a.guest_identity));
    let persisted = debugfs_cat(&host, &a.binding.workspace, "/kiln/report.json");
    let persisted: Report =
        serde_json::from_str(&persisted).unwrap_or_else(|e| fail("persisted report", e));
    assert_eq!(persisted, report_a);

    // A: reopen the paired checkpoint, restore paused, check confinement on
    // resume, and observe the continuation of the same attempt in memory.
    let reopened = RetainedSession::admit(
        a.current.identity.clone(),
        a.current.placement.clone(),
        a.current.resources,
    )
    .unwrap_or_else(|e| fail("reopen admission", e));
    a.current = reopened;
    let restore = a.operation(Action::Restore {
        checkpoint: checkpoint.clone(),
    });
    retained::reopen_retired_checkpoint(&mut db, &mut SingleHostTestReservation, &restore)
        .unwrap_or_else(|e| fail("reopen", e));
    let started = Instant::now();
    a.observe(
        retained::execute(
            &mut db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &restore,
        ),
        RetainedState::Paused,
    );
    let restored = started.elapsed();
    execute(
        &mut db,
        &host,
        &mut a,
        Action::Resume,
        RetainedState::Running,
    );
    let observed = lattice_worker::observe(
        &db,
        &host.config,
        &host.backend,
        &a.operation(Action::Resume),
        &host.manifest,
        GUEST_WAIT,
    )
    .unwrap_or_else(|e| fail("observe restored guest", e));
    assert_eq!(observed.attempt, Some(a.request.attempt.clone()));
    retire(&mut db, &host, &mut a, None);

    // A workspace already used by attempt A is refused by a fresh guest.
    let mut c = Session::new(&host, &run, "c", 1, "log-triage");
    lattice_worker::create_workspace(&c.binding.workspace, &host.manifest)
        .unwrap_or_else(|e| fail("workspace", e));
    let mut used = fs::OpenOptions::new()
        .write(true)
        .open(&c.binding.workspace)
        .unwrap_or_else(|e| fail("open", e));
    let mut source = fs::File::open(&a.binding.workspace).unwrap_or_else(|e| fail("open", e));
    std::io::copy(&mut source, &mut used).unwrap_or_else(|e| fail("copy", e));
    used.sync_all().unwrap_or_else(|e| fail("sync", e));
    let registration = c.operation(Action::ColdStart);
    retained::register(&mut db, &mut SingleHostTestReservation, &registration)
        .unwrap_or_else(|e| fail("register", e));
    c.observe(
        retained::execute(
            &mut db,
            &host.config,
            &host.backend,
            &mut SingleHostTestReservation,
            &registration,
        ),
        RetainedState::Paused,
    );
    execute(
        &mut db,
        &host,
        &mut c,
        Action::Resume,
        RetainedState::Running,
    );
    let reused = dispatch(&mut db, &host, &mut c);
    assert!(
        matches!(&reused.outcome, ReportOutcome::Refused { code, .. } if code == "lattice_worker_workspace_used"),
        "{reused:?}"
    );
    retire(&mut db, &host, &mut c, None);

    println!(
        "KILN_LATTICE_WORKER_EVIDENCE {}",
        json!({"event":"single-host-qualification","host":"single KVM host",
            "image":host.image.as_str(),"environment":host.manifest.environment_identity.as_str(),
            "vmm":host.manifest.vmm.version,"lattice":host.manifest.environment.lattice.revision,
            "attempt_a":{"workflow":"log-triage","guest_identity":report_a.guest_identity,"outcome":report_a.outcome},
            "attempt_b":{"workflow":"workspace-journal","guest_identity":report_b.guest_identity,"outcome":report_b.outcome},
            "guest_refusal":"lattice_worker_workflow_mismatch","workspace_reuse_refusal":"lattice_worker_workspace_used",
            "restore_ms":restored.as_millis(),"restored_attempt_continued":true,
            "authority":"test-only single-host reservation; no placement ownership or fencing claimed",
            "not_run":["cross-host relocation","Iroh/Aspen channels","live migration","host draining"]})
    );
    for session in [&a, &b, &c] {
        let _ = fs::remove_dir_all(&session.binding.directory);
    }
}
