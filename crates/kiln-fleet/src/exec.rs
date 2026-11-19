//! Bounded child processes for git, tar, tailscale and the configured
//! verification and publication commands.
//!
//! Every child runs through `bounded-exec` with a cleared environment plus an
//! explicit list, exact argv, a deadline and output bounds. Only an exited
//! child is a result; a deadline, cancellation or overflow is an error.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bounded_exec::{
    CommandSpec, Completion, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy,
    RunRequest, TerminationScope, run,
};

pub const POLL_INTERVAL_MS: u64 = 20;
pub const TEARDOWN_TIMEOUT_MS: u64 = 2_000;
/// No stdin is sent, but `bounded-exec` requires a nonzero bound.
pub const STDIN_MAX_BYTES: usize = 1;
/// Bytes of stderr kept in a diagnostic.
pub const MAX_DETAIL_BYTES: usize = 240;

/// One child invocation.
#[derive(Clone, Debug)]
pub struct Child {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    /// Bytes written to stdin; `None` sends nothing.
    pub input: Option<Vec<u8>>,
}

impl Child {
    #[must_use]
    pub fn new(
        program: &Path,
        current_dir: &Path,
        timeout_ms: u64,
        max_output_bytes: usize,
    ) -> Self {
        Self {
            program: program.to_path_buf(),
            args: Vec::new(),
            current_dir: current_dir.to_path_buf(),
            environment: Vec::new(),
            timeout_ms,
            max_output_bytes,
            input: None,
        }
    }

    #[must_use]
    pub fn input(mut self, bytes: Vec<u8>) -> Self {
        self.input = Some(bytes);
        self
    }

    #[must_use]
    pub fn arg(mut self, value: impl Into<OsString>) -> Self {
        self.args.push(value.into());
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(values.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.push((name.into(), value.into()));
        self
    }

    /// Run to completion.
    ///
    /// # Errors
    /// Returns a bounded diagnostic when the child cannot start, times out,
    /// is cancelled or overflows its output bound.
    pub fn run(self) -> Result<Finished, String> {
        let name = self.program.file_name().map_or_else(
            || "child".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let policy = OutcomePolicy::new(vec![0], true, true)
            .map_err(|_| format!("{name}: execution policy is invalid"))?;
        let stdin_max_bytes = self
            .input
            .as_ref()
            .map_or(STDIN_MAX_BYTES, |bytes| bytes.len().saturating_add(1));
        let input = self.input.map_or(Input::Null, Input::Bytes);
        let request = RunRequest {
            command: CommandSpec {
                program: self.program,
                args: self.args,
                current_dir: self.current_dir,
                environment_mode: EnvironmentMode::Clear,
                environment: self.environment,
                input,
            },
            limits: ExecutionLimits {
                timeout_ms: self.timeout_ms,
                stdin_max_bytes,
                stdout_max_bytes: self.max_output_bytes,
                stderr_max_bytes: self.max_output_bytes,
                poll_interval_ms: POLL_INTERVAL_MS,
                teardown_timeout_ms: TEARDOWN_TIMEOUT_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: policy,
        };
        let output = run(request).map_err(|error| format!("{name} could not run: {error}"))?;
        match (output.completion, output.disposition) {
            (Completion::TimedOut, _) | (_, Disposition::TimedOut) => {
                Err(format!("{name} timed out"))
            }
            (Completion::Cancelled, _) | (_, Disposition::Cancelled) => {
                Err(format!("{name} was cancelled"))
            }
            (_, Disposition::OutputLimitExceeded(_)) => {
                Err(format!("{name} output exceeds its bound"))
            }
            (Completion::Exited, Disposition::Succeeded | Disposition::ExitFailed) => {
                Ok(Finished {
                    program: name,
                    exit_code: output.exit_code,
                    stdout: output.stdout.bytes,
                    stderr: output.stderr.bytes,
                })
            }
        }
    }
}

/// An exited child.
#[derive(Clone, Debug)]
pub struct Finished {
    pub program: String,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Finished {
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Require exit 0.
    ///
    /// # Errors
    /// Returns the exit status and the first bounded stderr line.
    pub fn require_success(self) -> Result<Self, String> {
        if self.succeeded() {
            Ok(self)
        } else {
            Err(format!(
                "{} exited with {}: {}",
                self.program,
                self.exit_code
                    .map_or_else(|| "a signal".to_owned(), |code| code.to_string()),
                detail(&self.stderr)
            ))
        }
    }

    /// Stdout as trimmed UTF-8.
    ///
    /// # Errors
    /// Returns a diagnostic for non-UTF-8 output.
    pub fn stdout_text(&self) -> Result<String, String> {
        std::str::from_utf8(&self.stdout)
            .map(|text| text.trim().to_owned())
            .map_err(|_| format!("{} output is not UTF-8", self.program))
    }
}

/// First line of `bytes`, control characters replaced, bounded.
#[must_use]
pub fn detail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    line.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_DETAIL_BYTES)
        .collect::<String>()
        .trim()
        .to_owned()
}
