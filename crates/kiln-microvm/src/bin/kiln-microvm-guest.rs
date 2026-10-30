//! Guest-side shell: PID 1 agent inside the per-job Firecracker microVM.
//!
//! `serve` connects to the single per-session host listener over vsock,
//! answers `ping`, and runs at most one job per boot: it sets the clock, mixes
//! the host seed into the entropy pool, extracts the read-only source drive into
//! tmpfs and runs each step as an unprivileged user with no new privileges in
//! its own process group, under a deadline and output bounds.
//!
//! `probe` subcommands exist only for the guest-escape suite: they exit 0 when
//! the attempted connection is denied and 1 when it succeeds.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use kiln_microvm::protocol::{
    self, GuestMessage, GuestStep, HostMessage, StepReport, decode_host, encode, frame_length,
    guest_frame,
};
use kiln_microvm::{AGENT_PORT, GUEST_GID, GUEST_UID, GUEST_WORKSPACE, HOST_CID};
use nix::sys::signal::{Signal, killpg};
use nix::sys::socket::{AddressFamily, SockFlag, SockType, VsockAddr, connect, socket};
use nix::unistd::Pid;

/// Pause between connection attempts while the host listener is absent
/// (before restore or after the session ends).
const RECONNECT_INTERVAL: Duration = Duration::from_millis(1);
/// Poll interval while waiting for a step's process.
const WAIT_INTERVAL: Duration = Duration::from_millis(2);
/// Time allowed for output pipes to close after the process group is killed.
const DRAIN_GRACE: Duration = Duration::from_secs(1);
/// Deadline for source extraction helpers.
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(120);
/// Probe connection deadline.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let argument = |index: usize| arguments.get(index).map(String::as_str);
    match (argument(0), argument(1), argument(2), argument(3)) {
        (Some("serve"), Some(source), Some(busybox), None) => serve(source, busybox),
        (Some("probe"), Some("vsock"), Some(cid), Some(port)) => {
            match (cid.parse::<u32>(), port.parse::<u32>()) {
                (Ok(cid), Ok(port)) => probe_vsock(cid, port),
                _ => usage(),
            }
        }
        (Some("probe"), Some("tcp"), Some(address), None) => probe_tcp(address),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: kiln-microvm-guest serve SOURCE_DEVICE BUSYBOX | probe vsock CID PORT | probe tcp ADDRESS:PORT"
    );
    ExitCode::from(2)
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
    // A connected stream socket; only byte-stream read/write is used.
    Ok(UnixStream::from(fd))
}

fn probe_vsock(cid: u32, port: u32) -> ExitCode {
    match vsock_stream(cid, port) {
        Ok(_) => {
            println!("connected vsock {cid}:{port}");
            ExitCode::from(1)
        }
        Err(error) => {
            println!("denied vsock {cid}:{port}: {error}");
            ExitCode::SUCCESS
        }
    }
}

fn probe_tcp(address: &str) -> ExitCode {
    let Ok(address) = address.parse::<std::net::SocketAddr>() else {
        return usage();
    };
    match std::net::TcpStream::connect_timeout(&address, PROBE_TIMEOUT) {
        Ok(_) => {
            println!("connected tcp {address}");
            ExitCode::from(1)
        }
        Err(error) => {
            println!("denied tcp {address}: {error}");
            ExitCode::SUCCESS
        }
    }
}

fn serve(source: &str, busybox: &str) -> ExitCode {
    if let Err(error) = nix::sys::prctl::set_no_new_privs() {
        eprintln!("no_new_privs failed: {error}");
        return ExitCode::FAILURE;
    }
    let mut ran = false;
    loop {
        match vsock_stream(HOST_CID, AGENT_PORT) {
            Ok(stream) => {
                if let Err(error) = session(stream, source, busybox, &mut ran) {
                    eprintln!("kiln guest session ended: {error}");
                }
            }
            Err(_) => std::thread::sleep(RECONNECT_INTERVAL),
        }
    }
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, String> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).map_err(|e| e.to_string())?;
    let mut body = vec![0_u8; frame_length(prefix)?];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

fn send(stream: &mut UnixStream, message: GuestMessage) -> Result<(), String> {
    stream
        .write_all(&encode(&guest_frame(message))?)
        .map_err(|e| e.to_string())
}

fn session(
    mut stream: UnixStream,
    source: &str,
    busybox: &str,
    ran: &mut bool,
) -> Result<(), String> {
    loop {
        let frame = decode_host(&read_frame(&mut stream)?)?;
        match frame.message {
            HostMessage::Ping { nonce } => send(&mut stream, GuestMessage::Pong { nonce })?,
            HostMessage::Run {
                nonce,
                clock_unix_ms,
                entropy_hex,
                deadline_ms,
                steps,
            } => {
                let received = Instant::now();
                if *ran {
                    return send(
                        &mut stream,
                        GuestMessage::Done {
                            nonce,
                            admission_error: Some("this microVM already ran a job".into()),
                        },
                    );
                }
                *ran = true;
                if let Err(error) = prepare(clock_unix_ms, &entropy_hex, source, busybox) {
                    return send(
                        &mut stream,
                        GuestMessage::Done {
                            nonce,
                            admission_error: Some(error),
                        },
                    );
                }
                let deadline = received + Duration::from_millis(deadline_ms);
                for (index, step) in steps.iter().enumerate() {
                    let report = run_step(index, step, deadline);
                    let success = report.exit_code == Some(0)
                        && !report.timed_out
                        && !report.stdout_truncated
                        && !report.stderr_truncated;
                    send(&mut stream, GuestMessage::Step(report))?;
                    if !success {
                        break;
                    }
                }
                return send(
                    &mut stream,
                    GuestMessage::Done {
                        nonce,
                        admission_error: None,
                    },
                );
            }
        }
    }
}

fn prepare(
    clock_unix_ms: u64,
    entropy_hex: &str,
    source: &str,
    busybox: &str,
) -> Result<(), String> {
    use nix::sys::time::TimeSpec;
    use nix::time::{ClockId, clock_settime};
    clock_settime(
        ClockId::CLOCK_REALTIME,
        TimeSpec::from_duration(Duration::from_millis(clock_unix_ms)),
    )
    .map_err(|e| format!("guest clock: {e}"))?;
    let seed = protocol::hex_bytes(entropy_hex).ok_or("guest entropy seed")?;
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/urandom")
        .and_then(|mut pool| pool.write_all(&seed))
        .map_err(|e| format!("guest entropy: {e}"))?;
    // Restored snapshots may cache blocks read from the blank template image.
    helper(busybox, &["blockdev", "--flushbufs", source])?;
    helper(busybox, &["tar", "-x", "-f", source, "-C", GUEST_WORKSPACE])?;
    let owner = format!("{GUEST_UID}:{GUEST_GID}");
    helper(busybox, &["chown", "-R", &owner, GUEST_WORKSPACE])
}

fn helper(busybox: &str, arguments: &[&str]) -> Result<(), String> {
    let mut child = Command::new(busybox)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("source snapshot helper: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                return Err(format!(
                    "source snapshot helper failed: {}",
                    arguments.first().copied().unwrap_or_default()
                ));
            }
            Ok(None) if started.elapsed() < EXTRACT_TIMEOUT => std::thread::sleep(WAIT_INTERVAL),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("source snapshot helper timed out".into());
            }
        }
    }
}

/// Read at most `limit` bytes, then keep draining so the writer never blocks.
fn bounded_reader(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut truncated = false;
        let mut buffer = [0_u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    let chunk = buffer.get(..count).unwrap_or_default();
                    let room = limit.saturating_sub(kept.len());
                    if chunk.len() > room {
                        truncated = true;
                    }
                    kept.extend_from_slice(chunk.get(..room.min(chunk.len())).unwrap_or_default());
                }
            }
        }
        let _ = sender.send((kept, truncated));
    });
    receiver
}

fn run_step(index: usize, step: &GuestStep, deadline: Instant) -> StepReport {
    let started = Instant::now();
    let mut report = StepReport {
        index,
        exit_code: None,
        signal: None,
        timed_out: false,
        stdout: String::new(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        elapsed_ms: 0,
    };
    let environment: &BTreeMap<String, String> = &step.environment;
    let spawned = Command::new(&step.program)
        .args(&step.arguments)
        .env_clear()
        .envs(environment)
        .current_dir(&step.working_directory)
        .uid(GUEST_UID)
        .gid(GUEST_GID)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            report.stderr = format!("guest step spawn failed: {error}");
            report.elapsed_ms = elapsed(started);
            return report;
        }
    };
    let group = i32::try_from(child.id()).map(Pid::from_raw).ok();
    let stdout = child
        .stdout
        .take()
        .map(|p| bounded_reader(p, step.output_limit));
    let stderr = child
        .stderr
        .take()
        .map(|p| bounded_reader(p, step.output_limit));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(WAIT_INTERVAL),
            Ok(None) => {
                report.timed_out = true;
                if let Some(group) = group {
                    let _ = killpg(group, Signal::SIGKILL);
                }
                break child.wait().ok();
            }
            Err(_) => break None,
        }
    };
    // Like a stopped unit, nothing from the step's process group survives it.
    if let Some(group) = group {
        let _ = killpg(group, Signal::SIGKILL);
    }
    if let Some(status) = status {
        report.exit_code = status.code();
        report.signal = status.signal();
    }
    for (receiver, text, truncated) in [
        (stdout, &mut report.stdout, &mut report.stdout_truncated),
        (stderr, &mut report.stderr, &mut report.stderr_truncated),
    ] {
        match receiver.map(|r| r.recv_timeout(DRAIN_GRACE)) {
            Some(Ok((bytes, cut))) => {
                *text = String::from_utf8_lossy(&bytes).into_owned();
                *truncated = cut;
            }
            // An escaped descendant still holds the pipe: output is incomplete.
            _ => *truncated = true,
        }
    }
    report.elapsed_ms = elapsed(started);
    report
}

fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
