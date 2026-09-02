//! Bounded child-process execution imperative shell.
//!
//! Consumers supply explicit argv, environment behavior, input, deadlines,
//! capture limits, termination scope, and outcome policy. This crate owns only
//! process mechanics. It does not own sandboxing, executable authorization,
//! remote transport, output interpretation, evidence, or lifecycle policy.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use bounded_exec_core::CaptureAccumulator;
pub use bounded_exec_core::CapturedOutput;
pub use bounded_exec_core::Completion;
pub use bounded_exec_core::Disposition;
pub use bounded_exec_core::ExecutionLimits;
pub use bounded_exec_core::LimitError;
use bounded_exec_core::OutcomeFacts;
pub use bounded_exec_core::OutcomePolicy;
pub use bounded_exec_core::OverflowedStreams;
pub use bounded_exec_core::PolicyError;

const CAPTURE_BUFFER_BYTES: usize = 8_192;
const WORKER_CHANNEL_CAPACITY: usize = 1;

/// Whether the child inherits ambient environment variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentMode {
    /// Remove the ambient environment before applying explicit entries.
    Clear,
    /// Inherit the ambient environment and apply explicit overrides.
    Inherit,
}

/// Standard-input source for the child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    /// Connect standard input to the null device.
    Null,
    /// Write these exact bytes through a pipe, then close it.
    Bytes(Vec<u8>),
}

/// Processes owned by one execution request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationScope {
    /// Terminate only the direct child.
    Child,
    /// Place the child in a fresh process group and terminate the whole group.
    ///
    /// Unix uses a real process group. Other platforms currently fall back to
    /// direct-child termination without claiming descendant teardown.
    ProcessGroup,
}

/// Fully explicit command description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// Absolute executable path passed directly to the operating system.
    pub program: PathBuf,
    /// Exact argument vector excluding argv zero.
    pub args: Vec<OsString>,
    /// Absolute child working directory selected by the caller.
    pub current_dir: PathBuf,
    /// Ambient environment behavior.
    pub environment_mode: EnvironmentMode,
    /// Explicit environment entries or overrides.
    pub environment: Vec<(OsString, OsString)>,
    /// Standard-input source.
    pub input: Input,
}

/// Complete bounded execution request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunRequest {
    /// Command and process-context description.
    pub command: CommandSpec,
    /// Resource and timing bounds.
    pub limits: ExecutionLimits,
    /// Owned-process termination behavior.
    pub termination_scope: TerminationScope,
    /// Consumer-owned result classification policy.
    pub outcome_policy: OutcomePolicy,
}

/// Result of one completed bounded execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOutput {
    /// Why execution stopped.
    pub completion: Completion,
    /// Portable exit code, if one was available.
    pub exit_code: Option<i32>,
    /// Unix termination signal, if one was available.
    pub signal: Option<i32>,
    /// Bounded stdout capture.
    pub stdout: CapturedOutput,
    /// Bounded stderr capture.
    pub stderr: CapturedOutput,
    /// Product-neutral classification under the supplied outcome policy.
    pub disposition: Disposition,
}

/// Worker associated with a runner error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Worker {
    /// Standard-input writer.
    Stdin,
    /// Standard-output reader.
    Stdout,
    /// Standard-error reader.
    Stderr,
}

/// Operating-system operation associated with a runner error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// Spawn the child.
    Spawn,
    /// Poll child status.
    Wait,
    /// Terminate an owned child or process group.
    Kill,
    /// Reap the terminated direct child.
    Reap,
    /// Write standard input.
    WriteStdin,
    /// Read standard input. Reserved for future duplex execution support.
    ReadStdin,
    /// Read standard output.
    ReadStdout,
    /// Read standard error.
    ReadStderr,
}

/// Bounded runner failure.
#[derive(Debug)]
pub enum RunError {
    /// Invalid resource or timing limits.
    InvalidLimits(LimitError),
    /// The executable path is empty.
    EmptyProgram,
    /// The executable path is not absolute.
    ProgramNotAbsolute,
    /// The working directory is empty.
    EmptyCurrentDirectory,
    /// The working directory is not absolute.
    CurrentDirectoryNotAbsolute,
    /// An explicit environment variable appears more than once.
    DuplicateEnvironmentName,
    /// Supplied stdin exceeds its configured bound.
    InputLimitExceeded,
    /// A configured pipe was unexpectedly unavailable.
    MissingPipe(Worker),
    /// An operating-system operation failed.
    Io {
        /// Failed operation.
        operation: Operation,
        /// Operating-system error.
        error: io::Error,
    },
    /// A stream byte counter overflowed.
    CaptureCountOverflow(Worker),
    /// A worker exited without delivering a result.
    WorkerDisconnected(Worker),
    /// A worker panicked.
    WorkerPanicked(Worker),
    /// A worker did not finish within the teardown deadline.
    WorkerTeardownTimedOut(Worker),
    /// The direct child did not reap within the teardown deadline.
    ChildTeardownTimedOut,
    /// The platform process identifier cannot be represented safely.
    ProcessIdentifierOverflow,
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits(error) => write!(formatter, "invalid execution limits: {error:?}"),
            Self::EmptyProgram => formatter.write_str("executable path is empty"),
            Self::ProgramNotAbsolute => formatter.write_str("executable path is not absolute"),
            Self::EmptyCurrentDirectory => formatter.write_str("working directory is empty"),
            Self::CurrentDirectoryNotAbsolute => {
                formatter.write_str("working directory is not absolute")
            }
            Self::DuplicateEnvironmentName => {
                formatter.write_str("explicit environment contains a duplicate name")
            }
            Self::InputLimitExceeded => {
                formatter.write_str("stdin exceeds its configured byte bound")
            }
            Self::MissingPipe(worker) => write!(formatter, "{worker:?} pipe is unavailable"),
            Self::Io { operation, error } => write!(formatter, "{operation:?} failed: {error}"),
            Self::CaptureCountOverflow(worker) => {
                write!(formatter, "{worker:?} byte count overflowed")
            }
            Self::WorkerDisconnected(worker) => write!(formatter, "{worker:?} worker disconnected"),
            Self::WorkerPanicked(worker) => write!(formatter, "{worker:?} worker panicked"),
            Self::WorkerTeardownTimedOut(worker) => {
                write!(formatter, "{worker:?} worker teardown timed out")
            }
            Self::ChildTeardownTimedOut => formatter.write_str("direct child teardown timed out"),
            Self::ProcessIdentifierOverflow => {
                formatter.write_str("child process identifier does not fit the platform PID type")
            }
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Execute a request without external cancellation.
///
/// # Errors
///
/// Returns an error when request validation, process operations, capture, or
/// teardown fails. Timeouts and output-limit dispositions are successful
/// observations and are returned in [`ExecutionOutput`].
pub fn run(request: RunRequest) -> Result<ExecutionOutput, RunError> {
    run_inner(request, None)
}

/// Execute a request while observing an explicit cancellation flag.
///
/// # Errors
///
/// Returns the same infrastructure errors as [`run`]. A raised cancellation
/// flag produces [`Completion::Cancelled`] after owned-process teardown.
pub fn run_with_cancellation(
    request: RunRequest,
    cancellation: &AtomicBool,
) -> Result<ExecutionOutput, RunError> {
    run_inner(request, Some(cancellation))
}

fn run_inner(
    request: RunRequest,
    cancellation: Option<&AtomicBool>,
) -> Result<ExecutionOutput, RunError> {
    validate_request(&request)?;
    let RunRequest {
        command: command_spec,
        limits,
        termination_scope,
        outcome_policy,
    } = request;
    let mut command = prepare_command(&command_spec, termination_scope);
    let mut child = command.spawn().map_err(|error| RunError::Io {
        operation: Operation::Spawn,
        error,
    })?;

    let stdin = if matches!(&command_spec.input, Input::Bytes(_)) {
        Some(take_required_stdin(&mut child, termination_scope, limits)?)
    } else {
        None
    };
    let stdout = take_required_stdout(&mut child, termination_scope, limits)?;
    let stderr = take_required_stderr(&mut child, termination_scope, limits)?;
    let stdin_worker = start_stdin_worker(stdin, command_spec.input);
    let stdout_worker = start_capture_worker(stdout, limits.stdout_max_bytes, Worker::Stdout);
    let stderr_worker = start_capture_worker(stderr, limits.stderr_max_bytes, Worker::Stderr);

    let (completion, exit_status) =
        wait_for_completion(&mut child, termination_scope, limits, cancellation)?;
    if completion == Completion::Exited && termination_scope == TerminationScope::ProcessGroup {
        terminate_remaining_process_group(child.id())?;
    }

    let teardown_budget = TeardownBudget::new(limits.teardown_timeout_ms);
    let stdin_result = finish_stdin_worker(stdin_worker, completion, teardown_budget);
    let stdout_result = finish_worker(stdout_worker, teardown_budget);
    let stderr_result = finish_worker(stderr_worker, teardown_budget);
    stdin_result?;
    let stdout = stdout_result?;
    let stderr = stderr_result?;
    let exit_code = exit_status.and_then(|status| status.code());
    let signal = exit_status.and_then(exit_signal);
    let facts = OutcomeFacts {
        completion,
        exit_code,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
    };
    let disposition = bounded_exec_core::classify(facts, &outcome_policy);

    Ok(ExecutionOutput {
        completion,
        exit_code,
        signal,
        stdout,
        stderr,
        disposition,
    })
}

fn validate_request(request: &RunRequest) -> Result<(), RunError> {
    request.limits.validate().map_err(RunError::InvalidLimits)?;
    if request.command.program.as_os_str().is_empty() {
        return Err(RunError::EmptyProgram);
    }
    if !request.command.program.is_absolute() {
        return Err(RunError::ProgramNotAbsolute);
    }
    if request.command.current_dir.as_os_str().is_empty() {
        return Err(RunError::EmptyCurrentDirectory);
    }
    if !request.command.current_dir.is_absolute() {
        return Err(RunError::CurrentDirectoryNotAbsolute);
    }
    let input_bytes = match &request.command.input {
        Input::Null => 0,
        Input::Bytes(bytes) => bytes.len(),
    };
    if input_bytes > request.limits.stdin_max_bytes {
        return Err(RunError::InputLimitExceeded);
    }
    let mut names = BTreeSet::new();
    for (name, _) in &request.command.environment {
        if !names.insert(name.clone()) {
            return Err(RunError::DuplicateEnvironmentName);
        }
    }
    Ok(())
}

fn prepare_command(command_spec: &CommandSpec, termination_scope: TerminationScope) -> Command {
    let mut command = Command::new(&command_spec.program);
    command.args(&command_spec.args);
    command.current_dir(&command_spec.current_dir);
    if command_spec.environment_mode == EnvironmentMode::Clear {
        command.env_clear();
    }
    command.envs(command_spec.environment.iter().cloned());
    match &command_spec.input {
        Input::Null => {
            command.stdin(Stdio::null());
        }
        Input::Bytes(_) => {
            command.stdin(Stdio::piped());
        }
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    configure_termination_scope(&mut command, termination_scope);
    command
}

#[cfg(unix)]
fn configure_termination_scope(command: &mut Command, scope: TerminationScope) {
    use std::os::unix::process::CommandExt;

    if scope == TerminationScope::ProcessGroup {
        command.process_group(0);
    }
}

#[cfg(not(unix))]
fn configure_termination_scope(_command: &mut Command, _scope: TerminationScope) {}

#[derive(Clone, Copy)]
struct TeardownBudget {
    started: Instant,
    timeout: Duration,
}

impl TeardownBudget {
    fn new(timeout_ms: u64) -> Self {
        Self {
            started: Instant::now(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    fn remaining(self) -> Duration {
        self.timeout.saturating_sub(self.started.elapsed())
    }
}

struct WorkerHandle<T> {
    worker: Worker,
    receiver: mpsc::Receiver<Result<T, RunError>>,
    thread: thread::JoinHandle<()>,
}

fn take_required_stdin(
    child: &mut Child,
    scope: TerminationScope,
    limits: ExecutionLimits,
) -> Result<std::process::ChildStdin, RunError> {
    let Some(stdin) = child.stdin.take() else {
        terminate_and_reap(child, scope, limits)?;
        return Err(RunError::MissingPipe(Worker::Stdin));
    };
    Ok(stdin)
}

fn take_required_stdout(
    child: &mut Child,
    scope: TerminationScope,
    limits: ExecutionLimits,
) -> Result<std::process::ChildStdout, RunError> {
    let Some(stdout) = child.stdout.take() else {
        terminate_and_reap(child, scope, limits)?;
        return Err(RunError::MissingPipe(Worker::Stdout));
    };
    Ok(stdout)
}

fn take_required_stderr(
    child: &mut Child,
    scope: TerminationScope,
    limits: ExecutionLimits,
) -> Result<std::process::ChildStderr, RunError> {
    let Some(stderr) = child.stderr.take() else {
        terminate_and_reap(child, scope, limits)?;
        return Err(RunError::MissingPipe(Worker::Stderr));
    };
    Ok(stderr)
}

fn start_stdin_worker(
    stdin: Option<std::process::ChildStdin>,
    input: Input,
) -> Option<WorkerHandle<()>> {
    let (Some(stdin), Input::Bytes(bytes)) = (stdin, input) else {
        return None;
    };
    let (sender, receiver) = mpsc::sync_channel(WORKER_CHANNEL_CAPACITY);
    let worker_thread = thread::spawn(move || {
        let result = write_stdin(stdin, &bytes);
        let _send_result = sender.send(result);
    });
    Some(WorkerHandle {
        worker: Worker::Stdin,
        receiver,
        thread: worker_thread,
    })
}

fn write_stdin(mut stdin: std::process::ChildStdin, bytes: &[u8]) -> Result<(), RunError> {
    stdin.write_all(bytes).map_err(|error| RunError::Io {
        operation: Operation::WriteStdin,
        error,
    })?;
    stdin.flush().map_err(|error| RunError::Io {
        operation: Operation::WriteStdin,
        error,
    })
}

fn start_capture_worker<R>(
    reader: R,
    limit_bytes: usize,
    worker: Worker,
) -> WorkerHandle<CapturedOutput>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(WORKER_CHANNEL_CAPACITY);
    let worker_thread = thread::spawn(move || {
        let result = read_bounded(reader, limit_bytes, worker);
        let _send_result = sender.send(result);
    });
    WorkerHandle {
        worker,
        receiver,
        thread: worker_thread,
    }
}

fn read_bounded<R>(
    mut reader: R,
    limit_bytes: usize,
    worker: Worker,
) -> Result<CapturedOutput, RunError>
where
    R: Read,
{
    let mut capture = CaptureAccumulator::new(limit_bytes);
    let mut buffer = [0_u8; CAPTURE_BUFFER_BYTES];
    loop {
        let read_bytes = reader.read(&mut buffer).map_err(|error| RunError::Io {
            operation: read_operation(worker),
            error,
        })?;
        if read_bytes == 0 {
            break;
        }
        capture
            .observe(&buffer[..read_bytes])
            .map_err(|_error| RunError::CaptureCountOverflow(worker))?;
    }
    Ok(capture.finish())
}

const fn read_operation(worker: Worker) -> Operation {
    match worker {
        Worker::Stdout => Operation::ReadStdout,
        Worker::Stderr => Operation::ReadStderr,
        Worker::Stdin => Operation::ReadStdin,
    }
}

fn wait_for_completion(
    child: &mut Child,
    scope: TerminationScope,
    limits: ExecutionLimits,
    cancellation: Option<&AtomicBool>,
) -> Result<(Completion, Option<ExitStatus>), RunError> {
    let started = Instant::now();
    let timeout = Duration::from_millis(limits.timeout_ms);
    let poll_interval = Duration::from_millis(limits.poll_interval_ms);
    loop {
        if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            let status = terminate_and_reap(child, scope, limits)?;
            return Ok((Completion::Cancelled, status));
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                let wait_error = RunError::Io {
                    operation: Operation::Wait,
                    error,
                };
                terminate_and_reap(child, scope, limits)?;
                return Err(wait_error);
            }
        };
        if status.is_some() {
            return Ok((Completion::Exited, status));
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let status = terminate_and_reap(child, scope, limits)?;
            return Ok((Completion::TimedOut, status));
        }
        thread::sleep(poll_interval.min(timeout.saturating_sub(elapsed)));
    }
}

fn terminate_and_reap(
    child: &mut Child,
    scope: TerminationScope,
    limits: ExecutionLimits,
) -> Result<Option<ExitStatus>, RunError> {
    terminate_owned(child, scope)?;
    let started = Instant::now();
    let timeout = Duration::from_millis(limits.teardown_timeout_ms);
    let poll_interval = Duration::from_millis(limits.poll_interval_ms);
    loop {
        let status = child.try_wait().map_err(|error| RunError::Io {
            operation: Operation::Reap,
            error,
        })?;
        if status.is_some() {
            return Ok(status);
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return Err(RunError::ChildTeardownTimedOut);
        }
        thread::sleep(poll_interval.min(timeout.saturating_sub(elapsed)));
    }
}

#[cfg(unix)]
fn terminate_owned(child: &mut Child, scope: TerminationScope) -> Result<(), RunError> {
    if scope == TerminationScope::ProcessGroup {
        return terminate_process_group(child.id());
    }
    terminate_direct_child(child)
}

#[cfg(not(unix))]
fn terminate_owned(child: &mut Child, _scope: TerminationScope) -> Result<(), RunError> {
    terminate_direct_child(child)
}

fn terminate_direct_child(child: &mut Child) -> Result<(), RunError> {
    let Err(kill_error) = child.kill() else {
        return Ok(());
    };
    match child.try_wait() {
        Ok(Some(_status)) => Ok(()),
        Ok(None) | Err(_) => Err(RunError::Io {
            operation: Operation::Kill,
            error: kill_error,
        }),
    }
}

#[cfg(unix)]
fn terminate_process_group(child_id: u32) -> Result<(), RunError> {
    use nix::errno::Errno;
    use nix::sys::signal::Signal;
    use nix::sys::signal::killpg;
    use nix::unistd::Pid;

    let process_group = i32::try_from(child_id).map_err(|_| RunError::ProcessIdentifierOverflow)?;
    match killpg(Pid::from_raw(process_group), Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(RunError::Io {
            operation: Operation::Kill,
            error: io::Error::from_raw_os_error(error as i32),
        }),
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_child_id: u32) -> Result<(), RunError> {
    Ok(())
}

fn terminate_remaining_process_group(child_id: u32) -> Result<(), RunError> {
    terminate_process_group(child_id)
}

fn finish_stdin_worker(
    worker: Option<WorkerHandle<()>>,
    completion: Completion,
    teardown_budget: TeardownBudget,
) -> Result<(), RunError> {
    let Some(worker) = worker else {
        return Ok(());
    };
    let result = finish_worker(worker, teardown_budget);
    if completion != Completion::Exited && is_expected_stdin_shutdown(&result) {
        return Ok(());
    }
    result
}

fn is_expected_stdin_shutdown(result: &Result<(), RunError>) -> bool {
    match result {
        Err(RunError::Io {
            operation: Operation::WriteStdin,
            error,
        }) => matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
        ),
        _ => false,
    }
}

fn finish_worker<T>(
    worker: WorkerHandle<T>,
    teardown_budget: TeardownBudget,
) -> Result<T, RunError> {
    let received = worker.receiver.recv_timeout(teardown_budget.remaining());
    match received {
        Ok(result) => {
            if worker.thread.join().is_err() {
                return Err(RunError::WorkerPanicked(worker.worker));
            }
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(RunError::WorkerTeardownTimedOut(worker.worker))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if worker.thread.join().is_err() {
                Err(RunError::WorkerPanicked(worker.worker))
            } else {
                Err(RunError::WorkerDisconnected(worker.worker))
            }
        }
    }
}

#[cfg(unix)]
fn exit_signal(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;

    status.signal()
}

#[cfg(not(unix))]
const fn exit_signal(_status: ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    use std::time::Instant;

    use super::*;

    const SUCCESS_EXIT_CODE: i32 = 0;
    const TEST_TIMEOUT_MS: u64 = 1_000;
    const SHORT_TIMEOUT_MS: u64 = 100;
    const TEST_STDIN_MAX_BYTES: usize = 1_024;
    const TEST_OUTPUT_MAX_BYTES: usize = 1_024;
    const SMALL_OUTPUT_MAX_BYTES: usize = 4;
    const TEST_POLL_INTERVAL_MS: u64 = 10;
    const TEST_TEARDOWN_TIMEOUT_MS: u64 = 1_000;
    const TEST_EXECUTION_UPPER_BOUND_SECS: u64 = 2;
    const NON_TERMINATING_SHELL_LOOP: &str = "while :; do :; done";
    const EXPECTED_OUTPUT_BYTES: &[u8] = b"bounded";

    fn request(script: &str) -> RunRequest {
        RunRequest {
            command: CommandSpec {
                program: PathBuf::from("/bin/sh"),
                args: vec![OsString::from("-c"), OsString::from(script)],
                current_dir: PathBuf::from("/"),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms: TEST_TIMEOUT_MS,
                stdin_max_bytes: TEST_STDIN_MAX_BYTES,
                stdout_max_bytes: TEST_OUTPUT_MAX_BYTES,
                stderr_max_bytes: TEST_OUTPUT_MAX_BYTES,
                poll_interval_ms: TEST_POLL_INTERVAL_MS,
                teardown_timeout_ms: TEST_TEARDOWN_TIMEOUT_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true)
                .expect("fixture outcome policy is valid"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn explicit_environment_and_input_round_trip() {
        let mut request = request(
            "if [ \"${HOME+x}\" = x ]; then exit 1; fi; IFS= read -r value; printf '%s:%s' \"$FIXTURE\" \"$value\"",
        );
        request
            .command
            .environment
            .push((OsString::from("FIXTURE"), OsString::from("bounded")));
        request.command.input = Input::Bytes(b"input\n".to_vec());

        let output = run(request).expect("bounded command succeeds");

        assert_eq!(output.disposition, Disposition::Succeeded);
        assert_eq!(output.stdout.bytes, b"bounded:input");
        assert_eq!(output.stdout.observed_bytes, output.stdout.bytes.len());
        assert!(!output.stdout.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn truncation_is_bounded_and_policy_visible() {
        let mut strict_request = request("printf 'bounded-output'");
        strict_request.limits.stdout_max_bytes = SMALL_OUTPUT_MAX_BYTES;
        let mut truncating_request = strict_request.clone();
        truncating_request.outcome_policy =
            OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], false, false)
                .expect("truncating fixture policy is valid");

        let strict =
            run(strict_request).expect("strict output flood is captured as an observation");
        let truncating =
            run(truncating_request).expect("permitted truncation is captured as an observation");

        assert_eq!(strict.stdout.bytes.len(), SMALL_OUTPUT_MAX_BYTES);
        assert!(strict.stdout.observed_bytes > strict.stdout.bytes.len());
        assert!(strict.stdout.truncated);
        assert_eq!(
            strict.disposition,
            Disposition::OutputLimitExceeded(OverflowedStreams::Stdout)
        );
        assert_eq!(truncating.stdout, strict.stdout);
        assert_eq!(truncating.disposition, Disposition::Succeeded);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_and_preexisting_cancellation_tear_down_owned_processes() {
        let mut timeout_request = request(NON_TERMINATING_SHELL_LOOP);
        timeout_request.limits.timeout_ms = SHORT_TIMEOUT_MS;
        let timeout_started = Instant::now();
        let timed_out = run(timeout_request).expect("timeout is a bounded observation");
        let cancellation = AtomicBool::new(true);
        let cancelled = run_with_cancellation(request(NON_TERMINATING_SHELL_LOOP), &cancellation)
            .expect("cancellation is a bounded observation");

        assert_eq!(timed_out.completion, Completion::TimedOut);
        assert_eq!(timed_out.disposition, Disposition::TimedOut);
        assert!(timeout_started.elapsed() < Duration::from_secs(TEST_EXECUTION_UPPER_BOUND_SECS));
        assert_eq!(cancelled.completion, Completion::Cancelled);
        assert_eq!(cancelled.disposition, Disposition::Cancelled);
    }

    #[cfg(unix)]
    #[test]
    fn process_group_cleanup_closes_descendant_held_pipes() {
        let script = format!(
            "({NON_TERMINATING_SHELL_LOOP}) & printf '{}'",
            String::from_utf8_lossy(EXPECTED_OUTPUT_BYTES)
        );
        let started = Instant::now();

        let output =
            run(request(&script)).expect("descendant is terminated after the direct child exits");

        assert_eq!(output.stdout.bytes, EXPECTED_OUTPUT_BYTES);
        assert_eq!(output.disposition, Disposition::Succeeded);
        assert!(started.elapsed() < Duration::from_secs(TEST_EXECUTION_UPPER_BOUND_SECS));
    }

    #[test]
    fn malformed_requests_fail_before_spawn() {
        let mut empty_program = request("exit 0");
        empty_program.command.program = PathBuf::new();
        let mut relative_program = request("exit 0");
        relative_program.command.program = PathBuf::from("sh");
        let mut empty_current_directory = request("exit 0");
        empty_current_directory.command.current_dir = PathBuf::new();
        let mut relative_current_directory = request("exit 0");
        relative_current_directory.command.current_dir = PathBuf::from("relative");
        let mut duplicate_environment = request("exit 0");
        duplicate_environment.command.environment = vec![
            (OsString::from("DUPLICATE"), OsString::from("first")),
            (OsString::from("DUPLICATE"), OsString::from("second")),
        ];
        let mut oversized_input = request("exit 0");
        oversized_input.limits.stdin_max_bytes = SMALL_OUTPUT_MAX_BYTES;
        oversized_input.command.input = Input::Bytes(vec![0_u8; SMALL_OUTPUT_MAX_BYTES + 1]);

        assert!(matches!(run(empty_program), Err(RunError::EmptyProgram)));
        assert!(matches!(
            run(relative_program),
            Err(RunError::ProgramNotAbsolute)
        ));
        assert!(matches!(
            run(empty_current_directory),
            Err(RunError::EmptyCurrentDirectory)
        ));
        assert!(matches!(
            run(relative_current_directory),
            Err(RunError::CurrentDirectoryNotAbsolute)
        ));
        assert!(matches!(
            run(duplicate_environment),
            Err(RunError::DuplicateEnvironmentName)
        ));
        assert!(matches!(
            run(oversized_input),
            Err(RunError::InputLimitExceeded)
        ));
    }
}
