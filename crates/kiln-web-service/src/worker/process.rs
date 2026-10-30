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
    pub cpu_time_us: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
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
        checkout,
        cancellation,
        streaming::MAX_CHUNK_BYTES,
        |_| Ok(()),
    )
}

/// The calling thread owns ingress. Reader threads and their fixed-capacity
/// channel apply backpressure; a failed upload is never retried, but readers
/// continue draining until Bounded Exec completes owned-process teardown.
/// `directory` is the job working directory inside `checkout`.
pub fn execute_streamed(
    profile: &WorkerProfile,
    step: &Step,
    checkout: &Path,
    directory: &Path,
    cancellation: &AtomicBool,
    chunk_bytes: usize,
    mut upload: impl FnMut(&Chunk) -> Result<(), String>,
) -> Result<StepOutput, String> {
    if chunk_bytes == 0 {
        return Err("worker log chunk bound invalid".into());
    }
    let chunk_bytes = chunk_bytes.min(streaming::MAX_CHUNK_BYTES);
    let endpoints = streaming::Endpoints::new()?;
    let (isolated_program, isolated_args) =
        super::sandbox::command(profile, step, checkout, directory, &endpoints.usage_path)?;
    let (program, args) = streaming::helper_command(
        &endpoints.stdout_path,
        &endpoints.stderr_path,
        &isolated_program,
        &isolated_args,
        &endpoints.usage_path,
        profile.execution.as_ref(),
    )?;
    let request = RunRequest {
        command: CommandSpec {
            program,
            args,
            current_dir: directory.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: if profile.execution.is_some() {
                Vec::new()
            } else {
                step.environment
                    .iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
                    .collect()
            },
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
        // Cancellation/timeout can terminate the accounting adapter before it
        // writes. Unknown usage stays absent rather than guessed from the host.
        let resources = if output.completion == Completion::Exited
            && !(profile.execution.is_some() && code == 126)
        {
            // 126 is the reserved accountant/launcher failure exit. Never
            // consume a preexisting metrics file after failed publication.
            super::files::regular(&endpoints.usage_path, 4096, false)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<streaming::ResourceRecord>(&bytes).ok())
                .filter(|record| record.exit_code == code)
        } else {
            None
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
            cpu_time_us: resources.as_ref().map(|r| r.cpu_time_us),
            peak_memory_bytes: resources.as_ref().map(|r| r.peak_memory_bytes),
        })
    })
}

// r[verify kiln.web.developer_loop.workers]
#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> WorkerProfile {
        serde_json::from_slice(include_bytes!(
            "../../../../fixtures/web-worker/profile.json"
        ))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "reviewed worker fixture profile", error))
    }

    #[test]
    fn child_cpu_and_memory_are_observed_for_success_and_failure() {
        let root = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "private execution fixture directory", error)
        });
        let profile = profile();
        for expected in [0, 7] {
            let mut step = profile
                .steps
                .first()
                .unwrap_or_else(|| unreachable!("{}", "reviewed fixture step"))
                .clone();
            step.program = "/bin/sh".into();
            step.argv = vec![
                "-c".into(),
                format!("i=0; while [ $i -lt 10000 ]; do i=$((i+1)); done; exit {expected}"),
            ];
            let output = execute(&profile, &step, root.path(), &AtomicBool::new(false))
                .unwrap_or_else(|error| {
                    unreachable!("{}: {:?}", "bounded fixture execution", error)
                });
            assert_eq!(output.code, expected);
            assert!(output.cpu_time_us.is_some_and(|cpu| cpu > 0));
            assert!(output.peak_memory_bytes.is_some_and(|bytes| bytes > 0));
        }
    }

    #[test]
    fn explicit_unavailable_isolation_never_executes_command() {
        let root = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "private isolation fixture directory", error)
        });
        let mut profile = profile();
        profile.execution = Some(crate::execution::IsolationPolicy {
            bubblewrap: "/missing/kiln-bubblewrap".into(),
            probe_program: "/usr/bin/true".into(),
            read_only_mounts: vec!["/usr".into()],
            network: crate::execution::NetworkPolicy::Deny,
            cpu_seconds: 60,
            address_space_bytes: 268_435_456,
            maximum_file_bytes: 16_777_216,
            maximum_open_files: 64,
        });
        let mut step = profile
            .steps
            .first()
            .unwrap_or_else(|| unreachable!("{}", "reviewed fixture step"))
            .clone();
        step.program = "/bin/sh".into();
        step.argv = vec!["-c".into(), "printf executed > marker".into()];
        assert!(execute(&profile, &step, root.path(), &AtomicBool::new(false)).is_err());
        assert!(!root.path().join("marker").exists());
    }
}
