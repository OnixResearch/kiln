//! Guest-side shell inside the pinned Lattice worker guest (Kiln #51, M1).
//!
//! `serve` runs after the initramfs init. It connects to the host on the worker
//! vsock port, announces its baked environment identity and accepts at most one
//! admitted dispatch per incarnation. For that attempt it formats only a never
//! used workspace image, writes the per-attempt guest identity, Lattice config
//! and effect-authority profile, and runs the pinned Lattice headless executor
//! as an unprivileged user under the original attempt deadline. Its report is
//! written to the workspace and returned. Later connections, including after a
//! snapshot restore, only report the attempt this incarnation is running.
//!
//! `environment-identity FILE` prints the identity of an environment record; the
//! image build uses it so build and admission compute one identity.
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use kiln_core::types::ContentId;
use kiln_microvm::lattice_worker::{
    self, AdmittedWorkflow, AttemptRequest, EffectRequirements, GuestEnvironment, GuestMessage,
    HostMessage, LATTICE_WORKER_PORT, Report, ReportOutcome, RunSummary, WORKSPACE_PROBE_BYTES,
};
use kiln_microvm::{GUEST_GID, GUEST_UID, HOST_CID, protocol};
use nix::sys::signal::{Signal, killpg};
use nix::sys::socket::{AddressFamily, SockFlag, SockType, VsockAddr, connect, socket};
use nix::unistd::Pid;

const ENVIRONMENT_PATH: &str = "/etc/kiln/lattice-worker-environment.json";
const WORKFLOW_DIRECTORY: &str = "/etc/lattice/workflows";
const LATTICE: &str = "/bin/lattice";
const BUSYBOX: &str = "/bin/busybox";
const WORKSPACE_DEVICE: &str = "/dev/vda";
const WORKSPACE: &str = "/workspace";
const KILN_DIRECTORY: &str = "/workspace/kiln";
const CONFIG_PATH: &str = "/workspace/kiln/lattice-config.ncl";
const AUTHORITY_PATH: &str = "/workspace/kiln/lattice-authority.ncl";
const LABEL_PATH: &str = "/workspace/kiln/workspace-label.json";
const REPORT_PATH: &str = "/workspace/kiln/report.json";
const STATE_DIRECTORY: &str = "/workspace/lattice";
const CHECKOUT_DIRECTORY: &str = "/workspace/checkout";
const HOME_DIRECTORY: &str = "/workspace/home";
const SCRATCH_DIRECTORY: &str = "/tmp/kiln-lattice";
const GUEST_PATH: &str = "/bin";
/// Pause between connection attempts while the host listener is absent.
const RECONNECT_INTERVAL: Duration = Duration::from_millis(5);
const WAIT_INTERVAL: Duration = Duration::from_millis(5);
/// Bound for administrative helpers (format, mount, sync).
const HELPER_TIMEOUT: Duration = Duration::from_secs(60);
/// Per-node Lattice timeout written into the application config.
const NODE_TIMEOUT_SECS: u64 = 60;
/// Captured stdout of one Lattice command.
const MAX_OUTPUT_BYTES: u64 = 1_048_576;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["serve"] => serve(),
        ["environment-identity", path] => match load_environment(Path::new(path)) {
            Ok((_, identity)) => {
                println!("{}", identity.as_str());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: kiln-lattice-worker-guest serve | environment-identity FILE");
            ExitCode::from(2)
        }
    }
}

fn load_environment(path: &Path) -> Result<(GuestEnvironment, ContentId), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("environment unreadable: {e}"))?;
    let environment: GuestEnvironment =
        serde_json::from_slice(&bytes).map_err(|e| format!("environment invalid: {e}"))?;
    let identity = environment
        .identity()
        .map_err(|e| format!("environment refused: {}", e.code))?;
    Ok((environment, identity))
}

fn vsock_stream(cid: u32, port: u32) -> Result<UnixStream, String> {
    let fd = socket(
        AddressFamily::Vsock,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None,
    )
    .map_err(|e| e.to_string())?;
    connect(fd.as_raw_fd(), &VsockAddr::new(cid, port)).map_err(|e| e.to_string())?;
    Ok(UnixStream::from(fd))
}

fn send(stream: &mut UnixStream, message: GuestMessage) -> Result<(), String> {
    stream
        .write_all(&lattice_worker::encode(message)?)
        .map_err(|e| e.to_string())
}

fn read_frame(stream: &mut UnixStream) -> Result<HostMessage, String> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).map_err(|e| e.to_string())?;
    let mut body = vec![0_u8; lattice_worker::frame_length(prefix)?];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    lattice_worker::decode(&body)
}

struct Incarnation {
    environment: GuestEnvironment,
    identity: ContentId,
    /// The single admitted attempt of this incarnation and, once finished,
    /// its report. Survives snapshot/restore with the rest of guest memory.
    dispatched: Option<(AttemptRequest, Option<Report>)>,
}

fn serve() -> ExitCode {
    let (environment, identity) = match load_environment(Path::new(ENVIRONMENT_PATH)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("kiln lattice worker: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = nix::sys::prctl::set_no_new_privs() {
        eprintln!("no_new_privs failed: {error}");
        return ExitCode::FAILURE;
    }
    let mut incarnation = Incarnation {
        environment,
        identity,
        dispatched: None,
    };
    loop {
        match vsock_stream(HOST_CID, LATTICE_WORKER_PORT) {
            Ok(stream) => {
                if let Err(error) = session(stream, &mut incarnation) {
                    eprintln!("kiln lattice worker session ended: {error}");
                }
            }
            Err(_) => std::thread::sleep(RECONNECT_INTERVAL),
        }
    }
}

fn session(mut stream: UnixStream, incarnation: &mut Incarnation) -> Result<(), String> {
    send(
        &mut stream,
        GuestMessage::Hello {
            environment: incarnation.identity.clone(),
            attempt: incarnation
                .dispatched
                .as_ref()
                .map(|(request, _)| request.attempt.clone()),
        },
    )?;
    loop {
        match read_frame(&mut stream)? {
            HostMessage::Ping { nonce } => send(
                &mut stream,
                GuestMessage::Pong {
                    nonce,
                    attempt: incarnation
                        .dispatched
                        .as_ref()
                        .map(|(request, _)| request.attempt.clone()),
                },
            )?,
            HostMessage::Dispatch {
                request,
                clock_unix_ms,
                entropy_hex,
            } => {
                let report = dispatch(incarnation, *request, clock_unix_ms, &entropy_hex);
                send(&mut stream, GuestMessage::Report(report))?;
            }
        }
    }
}

fn refusal(request: &AttemptRequest, code: &str, field: &str) -> Report {
    Report {
        attempt: request.attempt.clone(),
        guest_identity: request.guest_identity.clone(),
        outcome: ReportOutcome::Refused {
            code: code.into(),
            field: field.into(),
        },
    }
}

fn dispatch(
    incarnation: &mut Incarnation,
    request: AttemptRequest,
    clock_unix_ms: u64,
    entropy_hex: &str,
) -> Report {
    if let Some((admitted, report)) = &incarnation.dispatched {
        // One attempt per incarnation; an exact repeat observes, never reruns.
        return match report {
            Some(report) if *admitted == request => report.clone(),
            _ => refusal(&request, "lattice_worker_already_dispatched", "attempt"),
        };
    }
    // The host is the deadline authority: the guest clock is set from it.
    let workflow =
        match lattice_worker::admit_dispatch(&incarnation.environment, &request, clock_unix_ms) {
            Ok(workflow) => workflow.clone(),
            Err(diagnostic) => return refusal(&request, &diagnostic.code, &diagnostic.field),
        };
    if entropy_hex.len() != lattice_worker::ENTROPY_BYTES * 2
        || protocol::hex_bytes(entropy_hex).is_none()
    {
        return refusal(&request, "lattice_worker_entropy", "entropy_hex");
    }
    match workspace_prefix() {
        Ok(prefix) if lattice_worker::workspace_blank(&prefix) => (),
        Ok(_) => return refusal(&request, "lattice_worker_workspace_used", "workspace"),
        Err(_) => {
            return refusal(
                &request,
                "lattice_worker_workspace_unavailable",
                "workspace",
            );
        }
    }
    incarnation.dispatched = Some((request.clone(), None));
    let outcome = match run_attempt(&request, &workflow, clock_unix_ms, entropy_hex) {
        Ok(outcome) => outcome,
        Err((stage, detail)) => ReportOutcome::PreparationFailed {
            stage: stage.into(),
            detail: lattice_worker::bounded_tail(&detail),
        },
    };
    let report = Report {
        attempt: request.attempt.clone(),
        guest_identity: request.guest_identity.clone(),
        outcome,
    };
    if let Err(error) = persist_report(&report) {
        eprintln!("kiln lattice worker report not persisted: {error}");
    }
    incarnation.dispatched = Some((request, Some(report.clone())));
    report
}

fn workspace_prefix() -> Result<Vec<u8>, String> {
    let mut prefix = vec![0_u8; WORKSPACE_PROBE_BYTES];
    std::fs::File::open(WORKSPACE_DEVICE)
        .and_then(|mut device| device.read_exact(&mut prefix))
        .map_err(|e| e.to_string())?;
    Ok(prefix)
}

type Stage = (&'static str, String);

fn helper(arguments: &[&str]) -> Result<(), String> {
    let mut child = Command::new(BUSYBOX)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + HELPER_TIMEOUT;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => return Err(format!("{} failed", arguments.join(" "))),
            None if Instant::now() < deadline => std::thread::sleep(WAIT_INTERVAL),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{} timed out", arguments.join(" ")));
            }
        }
    }
}

fn write_file(path: &str, bytes: &[u8], mode: u32) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("{path}: {e}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{path}: {e}"))
}

fn directory(path: &str, owner: Option<u32>) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|e| format!("{path}: {e}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{path}: {e}"))?;
    if let Some(uid) = owner {
        std::os::unix::fs::chown(path, Some(uid), Some(GUEST_GID))
            .map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}

fn prepare(
    request: &AttemptRequest,
    workflow: &AdmittedWorkflow,
    clock_unix_ms: u64,
    entropy_hex: &str,
) -> Result<(), Stage> {
    use nix::sys::time::TimeSpec;
    use nix::time::{ClockId, clock_settime};
    clock_settime(
        ClockId::CLOCK_REALTIME,
        TimeSpec::from_duration(Duration::from_millis(clock_unix_ms)),
    )
    .map_err(|e| ("clock", e.to_string()))?;
    let seed = protocol::hex_bytes(entropy_hex).ok_or(("entropy", "seed".to_string()))?;
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/urandom")
        .and_then(|mut pool| pool.write_all(&seed))
        .map_err(|e| ("entropy", e.to_string()))?;
    let definition_path = format!("{WORKFLOW_DIRECTORY}/{}.ncl", request.workflow);
    let definition = std::fs::read(&definition_path).map_err(|e| ("definition", e.to_string()))?;
    if blake3::hash(&definition).to_hex().as_str() != workflow.definition.as_str() {
        return Err(("definition", "baked workflow bytes changed".into()));
    }
    let identity = format!("{}\n", request.guest_identity);
    std::fs::write("/etc/machine-id", identity.as_bytes())
        .map_err(|e| ("identity", e.to_string()))?;
    let hostname = format!(
        "kiln-{}",
        request.guest_identity.get(..12).unwrap_or_default()
    );
    std::fs::write("/proc/sys/kernel/hostname", hostname.as_bytes())
        .map_err(|e| ("identity", e.to_string()))?;
    helper(&["mke2fs", "-q", "-L", "kiln-workspace", WORKSPACE_DEVICE])
        .or_else(|_| helper(&["mke2fs", "-L", "kiln-workspace", WORKSPACE_DEVICE]))
        .map_err(|e| ("format", e))?;
    std::fs::create_dir_all(WORKSPACE).map_err(|e| ("mount", e.to_string()))?;
    helper(&[
        "mount",
        "-t",
        "ext4",
        "-o",
        "nosuid,nodev",
        WORKSPACE_DEVICE,
        WORKSPACE,
    ])
    .map_err(|e| ("mount", e))?;
    // Root-owned attempt metadata; the workload cannot rewrite its authority.
    directory(KILN_DIRECTORY, None).map_err(|e| ("layout", e))?;
    for owned in [STATE_DIRECTORY, CHECKOUT_DIRECTORY, HOME_DIRECTORY] {
        directory(owned, Some(GUEST_UID)).map_err(|e| ("layout", e))?;
    }
    let label = serde_json::to_vec(&lattice_worker::workspace_label(request))
        .map_err(|e| ("label", e.to_string()))?;
    write_file(LABEL_PATH, &label, 0o644).map_err(|e| ("label", e))?;
    let config = lattice_worker::lattice_config(STATE_DIRECTORY, AUTHORITY_PATH, NODE_TIMEOUT_SECS)
        .map_err(|e| ("config", e.code))?;
    write_file(CONFIG_PATH, config.as_bytes(), 0o644).map_err(|e| ("config", e))?;
    let authority =
        lattice_worker::authority_profile(&workflow.requirements, request.deadline_unix_ms)
            .map_err(|e| ("authority", e.code))?;
    write_file(AUTHORITY_PATH, authority.as_bytes(), 0o644).map_err(|e| ("authority", e))?;
    std::fs::create_dir_all(SCRATCH_DIRECTORY).map_err(|e| ("layout", e.to_string()))?;
    std::os::unix::fs::chown(SCRATCH_DIRECTORY, Some(GUEST_UID), Some(GUEST_GID))
        .map_err(|e| ("layout", e.to_string()))?;
    helper(&["sync"]).map_err(|e| ("sync", e))
}

struct Captured {
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

/// Run one Lattice command as the unprivileged guest user in its own process
/// group, with output redirected to bounded scratch files.
fn lattice(name: &str, arguments: &[&str], deadline: Instant) -> Result<Captured, String> {
    let stdout_path = format!("{SCRATCH_DIRECTORY}/{name}.stdout");
    let stderr_path = format!("{SCRATCH_DIRECTORY}/{name}.stderr");
    let stdout = std::fs::File::create(&stdout_path).map_err(|e| e.to_string())?;
    let stderr = std::fs::File::create(&stderr_path).map_err(|e| e.to_string())?;
    let mut child = Command::new(LATTICE)
        .arg("--config")
        .arg(CONFIG_PATH)
        .args(arguments)
        .env_clear()
        .env("PATH", GUEST_PATH)
        .env("HOME", HOME_DIRECTORY)
        .env("TMPDIR", SCRATCH_DIRECTORY)
        .current_dir(CHECKOUT_DIRECTORY)
        .uid(GUEST_UID)
        .gid(GUEST_GID)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| e.to_string())?;
    let group = i32::try_from(child.id()).map(Pid::from_raw).ok();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break Some(status),
            None if Instant::now() < deadline => std::thread::sleep(WAIT_INTERVAL),
            None => {
                if let Some(group) = group {
                    let _ = killpg(group, Signal::SIGKILL);
                }
                let _ = child.wait();
                break None;
            }
        }
    };
    if let Some(group) = group {
        let _ = killpg(group, Signal::SIGKILL);
    }
    let read = |path: &str| -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(MAX_OUTPUT_BYTES).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        Ok(bytes)
    };
    Ok(Captured {
        exit_code: status.and_then(|status| status.code()),
        stdout: read(&stdout_path)?,
        stderr: String::from_utf8_lossy(&read(&stderr_path)?).into_owned(),
    })
}

fn run_attempt(
    request: &AttemptRequest,
    workflow: &AdmittedWorkflow,
    clock_unix_ms: u64,
    entropy_hex: &str,
) -> Result<ReportOutcome, Stage> {
    let remaining = request.deadline_unix_ms.saturating_sub(clock_unix_ms);
    let deadline = Instant::now() + Duration::from_millis(remaining);
    prepare(request, workflow, clock_unix_ms, entropy_hex)?;
    let definition = format!("{WORKFLOW_DIRECTORY}/{}.ncl", request.workflow);
    let imported =
        lattice("import", &["import", &definition], deadline).map_err(|e| ("import", e))?;
    if imported.exit_code != Some(0) {
        return Err(("import", imported.stderr));
    }
    let inspected = lattice(
        "inspect",
        &["authority", "inspect", &request.workflow],
        deadline,
    )
    .map_err(|e| ("authority", e))?;
    let observed: EffectRequirements = serde_json::from_slice(&inspected.stdout)
        .map_err(|_| ("authority", inspected.stderr.clone()))?;
    if inspected.exit_code != Some(0) || observed != workflow.requirements {
        return Err((
            "authority",
            "pinned Lattice reported effect requirements other than the baked admission".into(),
        ));
    }
    let ran = lattice(
        "run",
        &["run", &request.workflow, "--output", "json"],
        deadline,
    )
    .map_err(|e| ("run", e))?;
    let summary = serde_json::from_slice::<RunSummary>(&ran.stdout).ok();
    let _ = helper(&["sync"]);
    Ok(ReportOutcome::Ran {
        exit_code: ran.exit_code,
        summary,
        stderr_tail: lattice_worker::bounded_tail(&ran.stderr),
    })
}

fn persist_report(report: &Report) -> Result<(), String> {
    if !Path::new(KILN_DIRECTORY).is_dir() {
        return Err("workspace was not prepared".into());
    }
    let bytes = serde_json::to_vec(report).map_err(|e| e.to_string())?;
    write_file(REPORT_PATH, &bytes, 0o644)?;
    helper(&["sync"])
}
