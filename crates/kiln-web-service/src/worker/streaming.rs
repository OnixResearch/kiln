use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
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
    pub stdout: UnixListener,
    pub stderr: UnixListener,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
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
            stdout,
            stderr,
            stdout_path,
            stderr_path,
        })
    }
}

/// This is an exec adapter, not a supervisor. Bounded Exec owns this PID and
/// process group before the exact reviewed command replaces the adapter.
pub fn child_main(mut arguments: impl Iterator<Item = OsString>) -> i32 {
    let parsed = (|| {
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--stdout")) {
            return None;
        }
        let stdout = PathBuf::from(arguments.next()?);
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--stderr")) {
            return None;
        }
        let stderr = PathBuf::from(arguments.next()?);
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
            return None;
        }
        let program = PathBuf::from(arguments.next()?);
        if !stdout.is_absolute() || !stderr.is_absolute() || !program.is_absolute() {
            return None;
        }
        Some((stdout, stderr, program))
    })();
    let Some((stdout, stderr, program)) = parsed else {
        return 2;
    };
    let connected = UnixStream::connect(stdout)
        .and_then(|stdout| UnixStream::connect(stderr).map(|stderr| (stdout, stderr)));
    let Ok((stdout, stderr)) = connected else {
        return 126;
    };
    let stdout: OwnedFd = stdout.into();
    let stderr: OwnedFd = stderr.into();
    let _error = Command::new(program)
        .args(arguments)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .exec();
    // Do not expose executable names, arguments, or OS diagnostics.
    126
}

pub fn helper_command(
    stdout: &Path,
    stderr: &Path,
    program: &Path,
    argv: &[String],
) -> Result<(PathBuf, Vec<OsString>), String> {
    let executable = std::env::current_exe().map_err(|_| "worker executable unavailable")?;
    #[cfg(not(test))]
    let mut args = vec![OsString::from("stream-child")];
    // A unit-test executable is libtest, not the deployed CLI. Invoke one real
    // helper test, with payload after libtest's `--`; no alternate supervisor.
    #[cfg(test)]
    let mut args = vec![
        OsString::from("--exact"),
        OsString::from("worker::streaming::tests::stream_child_entry"),
        OsString::from("--ignored"),
        OsString::from("--nocapture"),
        OsString::from("--"),
    ];
    args.extend([
        OsString::from("--stdout"),
        stdout.as_os_str().to_owned(),
        OsString::from("--stderr"),
        stderr.as_os_str().to_owned(),
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
