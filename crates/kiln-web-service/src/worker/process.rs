use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use bounded_exec::{
    CommandSpec, Completion, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run_with_cancellation,
};

use super::profile::{Step, WorkerProfile};
use super::streaming::{self, Chunk, Stream};

pub struct StepOutput {
    pub code: i32,
    #[cfg(test)]
    pub stdout: bounded_exec::CapturedOutput,
    #[cfg(test)]
    pub stderr: bounded_exec::CapturedOutput,
    pub ingress: Result<(), String>,
}

#[cfg(test)]
pub fn execute(
    profile: &WorkerProfile,
    step: &Step,
    checkout: &Path,
    cancellation: &AtomicBool,
) -> Result<StepOutput, String> {
    execute_streamed(
        profile,
        step,
        checkout,
        cancellation,
        streaming::MAX_CHUNK_BYTES,
        |_| Ok(()),
    )
}

/// The calling thread owns ingress. Reader threads and their fixed-capacity
/// channel apply backpressure; a failed upload is never retried, but readers
/// continue draining until Bounded Exec completes owned-process teardown.
pub fn execute_streamed(
    profile: &WorkerProfile,
    step: &Step,
    checkout: &Path,
    cancellation: &AtomicBool,
    chunk_bytes: usize,
    mut upload: impl FnMut(&Chunk) -> Result<(), String>,
) -> Result<StepOutput, String> {
    if chunk_bytes == 0 {
        return Err("worker log chunk bound invalid".into());
    }
    let chunk_bytes = chunk_bytes.min(streaming::MAX_CHUNK_BYTES);
    let endpoints = streaming::Endpoints::new()?;
    let (program, args) = streaming::helper_command(
        &endpoints.stdout_path,
        &endpoints.stderr_path,
        &step.program,
        &step.argv,
    )?;
    let request = RunRequest {
        command: CommandSpec {
            program,
            args,
            current_dir: checkout.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: step
                .environment
                .iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v)))
                .collect(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: profile.command_timeout_ms,
            stdin_max_bytes: 1,
            stdout_max_bytes: profile.maximum_output_bytes,
            stderr_max_bytes: profile.maximum_output_bytes,
            poll_interval_ms: profile.poll_interval_ms,
            teardown_timeout_ms: profile.teardown_timeout_ms,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|_| "worker process policy invalid")?,
    };
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let supervisor = scope.spawn(|| {
            let result = run_with_cancellation(request, cancellation);
            done.store(true, Ordering::Release);
            result
        });
        let (sender, receiver) = mpsc::sync_channel(8);
        let mut readers = Vec::with_capacity(2);
        for (listener, stream) in [
            (&endpoints.stdout, Stream::Stdout),
            (&endpoints.stderr, Stream::Stderr),
        ] {
            let sender = sender.clone();
            let done = &done;
            readers.push(scope.spawn(move || {
                streaming::read_stream(
                    listener,
                    stream,
                    profile.maximum_output_bytes,
                    chunk_bytes,
                    Duration::from_millis(profile.teardown_timeout_ms),
                    done,
                    sender,
                )
            }));
        }
        drop(sender);
        let mut ingress = Ok(());
        for chunk in receiver {
            if ingress.is_ok() {
                ingress = upload(&chunk);
            }
        }
        let output = supervisor
            .join()
            .map_err(|_| "worker supervisor thread failed")?
            .map_err(|_| "worker process execution failed")?;
        let mut captures = Vec::with_capacity(2);
        for reader in readers {
            match reader.join().map_err(|_| "worker stream thread failed")? {
                Ok(capture) => captures.push(capture),
                Err(error) => {
                    if ingress.is_ok() {
                        ingress = Err(error);
                    }
                    captures.push(bounded_exec::CapturedOutput {
                        bytes: Vec::new(),
                        observed_bytes: 0,
                        truncated: false,
                    });
                }
            }
        }
        let mut captures = captures.into_iter();
        let stdout = captures.next().ok_or("worker stdout capture missing")?;
        let stderr = captures.next().ok_or("worker stderr capture missing")?;
        let code = match output.completion {
            Completion::Cancelled => 130,
            Completion::TimedOut => 124,
            Completion::Exited => {
                let code = output
                    .exit_code
                    .unwrap_or_else(|| output.signal.map_or(1, |signal| 128 + signal))
                    .clamp(0, 255);
                if code == 0 && (stdout.truncated || stderr.truncated) {
                    125
                } else {
                    code
                }
            }
        };
        eprintln!(
            "kiln worker: step {} completed; exit={} stdout_truncated={} stderr_truncated={}",
            step.node_id, code, stdout.truncated, stderr.truncated,
        );
        Ok(StepOutput {
            code,
            #[cfg(test)]
            stdout,
            #[cfg(test)]
            stderr,
            ingress,
        })
    })
}
