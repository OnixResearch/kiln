use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

use bounded_exec_core::{CaptureAccumulator, CapturedOutput};

const READ_BYTES: usize = 8192;
pub const MAX_CHUNK_BYTES: usize = 65_536;
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const READ_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    pub fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

pub struct Chunk {
    pub stream: Stream,
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

pub struct Endpoints {
    // Private directory owns both listeners until all readers have joined.
    _root: tempfile::TempDir,
    _metrics: tempfile::TempDir,
    pub stdout: UnixListener,
    pub stderr: UnixListener,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub usage_path: PathBuf,
}

impl Endpoints {
    pub fn new() -> Result<Self, String> {
        // Keep AF_UNIX paths short even when the admitted checkout path is long.
        // tempfile creates the directory mode 0700 atomically.
        let root = tempfile::Builder::new()
            .prefix("kiln-stream-")
            .tempdir_in("/tmp")
            .map_err(|_| "worker stream directory failed")?;
        let stdout_path = root.path().join("out");
        let stderr_path = root.path().join("err");
        let metrics = tempfile::Builder::new()
            .prefix("kiln-metrics-")
            .tempdir_in("/tmp")
            .map_err(|_| "worker metrics directory failed")?;
        let usage_path = metrics.path().join("usage");
        let stdout =
            UnixListener::bind(&stdout_path).map_err(|_| "worker stdout endpoint failed")?;
        let stderr =
            UnixListener::bind(&stderr_path).map_err(|_| "worker stderr endpoint failed")?;
        for listener in [&stdout, &stderr] {
            listener
                .set_nonblocking(true)
                .map_err(|_| "worker endpoint mode failed")?;
        }
        Ok(Self {
            _root: root,
            _metrics: metrics,
            stdout,
            stderr,
            stdout_path,
            stderr_path,
            usage_path,
        })
    }
}

/// Bounded Exec owns this adapter and its child process group. Each newly exec'd
/// adapter spawns exactly one child; its child rusage cannot include other jobs.
pub fn child_main(mut arguments: impl Iterator<Item = OsString>) -> i32 {
    let selector = arguments.next();
    if selector.as_deref() == Some(std::ffi::OsStr::new("--limit-child")) {
        return limit_child_main(arguments);
    }
    if selector.as_deref() == Some(std::ffi::OsStr::new("--measure-child")) {
        return measure_child_main(arguments);
    }
    let parsed = (|| {
        if selector.as_deref() != Some(std::ffi::OsStr::new("--stdout")) {
            return None;
        }
        let stdout = PathBuf::from(arguments.next()?);
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--stderr")) {
            return None;
        }
        let stderr = PathBuf::from(arguments.next()?);
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--usage")) {
            return None;
        }
        let usage = PathBuf::from(arguments.next()?);
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--limits")) {
            return None;
        }
        let limits = arguments.next()?.into_string().ok()?;
        let limits: Option<crate::execution::IsolationPolicy> =
            serde_json::from_str(&limits).ok()?;
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
            return None;
        }
        let program = PathBuf::from(arguments.next()?);
        if [&stdout, &stderr, &program, &usage]
            .iter()
            .any(|p| !p.is_absolute())
        {
            return None;
        }
        Some((stdout, stderr, program, usage, limits))
    })();
    let Some((stdout, stderr, program, usage, limits)) = parsed else {
        return 2;
    };
    let connected = UnixStream::connect(stdout)
        .and_then(|stdout| UnixStream::connect(stderr).map(|stderr| (stdout, stderr)));
    let Ok((stdout, stderr)) = connected else {
        return 126;
    };
    let stdout: OwnedFd = stdout.into();
    let stderr: OwnedFd = stderr.into();
    if limits
        .as_ref()
        .is_some_and(|policy| policy.admit().is_err())
    {
        return 126;
    }
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    observe(command, limits.is_none().then_some(usage.as_path()))
}

/// Called inside the namespace for isolated jobs, outside it for legacy jobs.
/// A single child is waited here, so kernel accounting includes actual job CPU.
fn observe(mut command: Command, usage: Option<&Path>) -> i32 {
    let Ok(mut child) = command.spawn() else {
        return 126;
    };
    let status = loop {
        match child.wait() {
            Ok(status) => break status,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(_) => return 126,
        }
    };
    let code = status
        .code()
        .unwrap_or_else(|| status.signal().map_or(126, |signal| 128 + signal));
    let Some(usage) = usage else {
        return code;
    };
    // There are no prior child processes in this freshly exec'd adapter. Unlike
    // RUSAGE_CHILDREN in a worker daemon, this is one step's exact child cohort.
    use nix::sys::resource::{UsageWho, getrusage};
    use nix::sys::time::TimeValLike;
    let Ok(resources) = getrusage(UsageWho::RUSAGE_CHILDREN) else {
        return 126;
    };
    let record = ResourceRecord {
        exit_code: code,
        cpu_time_us: (resources.user_time().num_microseconds() as u64)
            .saturating_add(resources.system_time().num_microseconds() as u64),
        peak_memory_bytes: (resources.max_rss() as u64).saturating_mul(1024),
    };
    let written = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(usage)?;
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        file.write_all(&bytes)
    })();
    if written.is_err() {
        return 126;
    }
    code
}

fn measure_child_main(mut arguments: impl Iterator<Item = OsString>) -> i32 {
    let Some(usage) = arguments.next().map(PathBuf::from) else {
        return 2;
    };
    let Some(encoded) = arguments.next() else {
        return 2;
    };
    if !usage.is_absolute() || arguments.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return 2;
    }
    let Some(program) = arguments.next().map(PathBuf::from) else {
        return 2;
    };
    if !program.is_absolute() {
        return 2;
    }
    let Ok((executable, mut args)) = entry_command() else {
        return 126;
    };
    args.extend([
        OsString::from("--limit-child"),
        encoded,
        OsString::from("--"),
        program.into_os_string(),
    ]);
    let mut command = Command::new(executable);
    command.args(args).args(arguments);
    observe(command, Some(&usage))
}

/// Resource limits belong to the command, not to the process which must retain
/// its resource report. exec preserves the owned PID and Bounded Exec group.
fn limit_child_main(mut arguments: impl Iterator<Item = OsString>) -> i32 {
    use nix::sys::resource::{Resource, setrlimit};
    use std::os::unix::process::CommandExt;
    let Some(encoded) = arguments.next().and_then(|v| v.into_string().ok()) else {
        return 2;
    };
    let Ok(policy) = serde_json::from_str::<crate::execution::IsolationPolicy>(&encoded) else {
        return 2;
    };
    if policy.admit().is_err() || arguments.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return 2;
    }
    let Some(program) = arguments.next().map(PathBuf::from) else {
        return 2;
    };
    if !program.is_absolute() {
        return 2;
    }
    let mut command = Command::new(program);
    command.args(arguments);
    for (resource, bound) in [
        (Resource::RLIMIT_CPU, policy.cpu_seconds),
        (Resource::RLIMIT_AS, policy.address_space_bytes),
        (Resource::RLIMIT_FSIZE, policy.maximum_file_bytes),
        (Resource::RLIMIT_NOFILE, policy.maximum_open_files),
    ] {
        if setrlimit(resource, bound, bound).is_err() {
            return 126;
        }
    }
    let _error = command.exec();
    126
}

pub(super) fn entry_command() -> Result<(PathBuf, Vec<OsString>), String> {
    let executable = std::env::current_exe().map_err(|_| "worker executable unavailable")?;
    #[cfg(not(test))]
    let args = vec![OsString::from("stream-child")];
    // Unit-test executables are libtest, not the deployed CLI. Both accounting
    // and quota launchers use this same real helper entry, never a fake runner.
    #[cfg(test)]
    let args = vec![
        OsString::from("--exact"),
        OsString::from("worker::streaming::tests::stream_child_entry"),
        OsString::from("--ignored"),
        OsString::from("--nocapture"),
        OsString::from("--"),
    ];
    Ok((executable, args))
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRecord {
    pub exit_code: i32,
    pub cpu_time_us: u64,
    pub peak_memory_bytes: u64,
}

pub fn helper_command(
    stdout: &Path,
    stderr: &Path,
    program: &Path,
    argv: &[String],
    usage: &Path,
    limits: Option<&crate::execution::IsolationPolicy>,
) -> Result<(PathBuf, Vec<OsString>), String> {
    let (executable, mut args) = entry_command()?;
    args.extend([
        OsString::from("--stdout"),
        stdout.as_os_str().to_owned(),
        OsString::from("--stderr"),
        stderr.as_os_str().to_owned(),
        OsString::from("--usage"),
        usage.as_os_str().to_owned(),
        OsString::from("--limits"),
        OsString::from(
            serde_json::to_string(&limits).map_err(|_| "worker resource policy encoding failed")?,
        ),
        OsString::from("--"),
        program.as_os_str().to_owned(),
    ]);
    args.extend(argv.iter().map(OsString::from));
    Ok((executable, args))
}

pub fn read_stream(
    listener: &UnixListener,
    stream: Stream,
    limit: usize,
    chunk_bytes: usize,
    teardown: Duration,
    done: &AtomicBool,
    sender: SyncSender<Chunk>,
) -> Result<CapturedOutput, String> {
    if chunk_bytes == 0 {
        return Err("worker log chunk bound invalid".into());
    }
    let mut capture = CaptureAccumulator::new(limit);
    let socket = loop {
        match listener.accept() {
            Ok((socket, _)) => break socket,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if done.load(Ordering::Acquire) {
                    return Err("worker stream helper did not connect".into());
                }
                std::thread::sleep(READ_INTERVAL);
            }
            Err(_) => return Err("worker stream accept failed".into()),
        }
    };
    socket
        .set_read_timeout(Some(READ_INTERVAL))
        .map_err(|_| "worker stream timeout failed")?;
    let mut socket = socket;
    let mut buffer = [0u8; READ_BYTES];
    let mut retained = 0usize;
    let mut pending = Vec::with_capacity(chunk_bytes);
    let mut last_flush = Instant::now();
    let mut teardown_started = None;
    let mut sent = false;
    loop {
        if done.load(Ordering::Acquire) {
            let started = teardown_started.get_or_insert_with(Instant::now);
            if started.elapsed() > teardown {
                return Err("worker stream did not close after process teardown".into());
            }
        }
        let count = match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                0
            }
            Err(_) => return Err("worker stream read failed".into()),
        };
        let observed = buffer
            .get(..count)
            .ok_or("worker stream read count exceeds buffer")?;
        capture
            .observe(observed)
            .map_err(|_| "worker stream counter overflow")?;
        let keep = count.min(limit.saturating_sub(retained));
        retained += keep;
        let mut bytes = observed
            .get(..keep)
            .ok_or("worker stream retained prefix exceeds read")?;
        while !bytes.is_empty() {
            let available = chunk_bytes
                .checked_sub(pending.len())
                .filter(|available| *available > 0)
                .ok_or("worker stream pending chunk bound invalid")?;
            let take = bytes.len().min(available);
            let (next, remaining) = bytes.split_at(take);
            pending.extend_from_slice(next);
            bytes = remaining;
            if pending.len() == chunk_bytes {
                let blocked = send(&sender, stream, &mut pending, false)?;
                // Waiting for bounded ingress is not a leaked-writer timeout.
                if let Some(started) = &mut teardown_started {
                    *started += blocked;
                }
                sent = true;
                last_flush = Instant::now();
            }
        }
        if !pending.is_empty() && last_flush.elapsed() >= FLUSH_INTERVAL {
            let blocked = send(&sender, stream, &mut pending, false)?;
            if let Some(started) = &mut teardown_started {
                *started += blocked;
            }
            sent = true;
            last_flush = Instant::now();
        }
    }
    let output = capture.finish();
    // A stream needs an explicit empty record if no bytes were sent, and an
    // explicit truncation marker even if the retained prefix already flushed.
    if !pending.is_empty() || !sent || output.truncated {
        send(&sender, stream, &mut pending, output.truncated)?;
    }
    Ok(output)
}

fn send(
    sender: &SyncSender<Chunk>,
    stream: Stream,
    pending: &mut Vec<u8>,
    truncated: bool,
) -> Result<Duration, String> {
    let bytes = std::mem::take(pending);
    let started = Instant::now();
    sender
        .send(Chunk {
            stream,
            bytes,
            truncated,
        })
        .map_err(|_| "worker log uploader stopped".to_owned())?;
    Ok(started.elapsed())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "exec adapter launched only by worker process tests"]
    fn stream_child_entry() {
        let mut arguments = std::env::args_os();
        while arguments.next().is_some_and(|argument| argument != "--") {}
        std::process::exit(super::child_main(arguments));
    }
}
