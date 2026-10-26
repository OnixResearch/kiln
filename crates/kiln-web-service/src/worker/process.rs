use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use bounded_exec::{
    CommandSpec, Completion, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy,
    RunRequest, TerminationScope, run_with_cancellation,
};

use super::profile::{Step, WorkerProfile};

pub struct StepOutput {
    pub code: i32,
    pub stdout: bounded_exec::CapturedOutput,
    pub stderr: bounded_exec::CapturedOutput,
}

pub fn execute(
    profile: &WorkerProfile,
    step: &Step,
    checkout: &Path,
    cancellation: &AtomicBool,
) -> Result<StepOutput, String> {
    eprintln!(
        "kiln worker: step {} started; command output is published on step completion",
        step.node_id
    );
    let request = RunRequest {
        command: CommandSpec {
            program: step.program.clone(),
            args: step.argv.iter().map(OsString::from).collect(),
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
    let output = run_with_cancellation(request, cancellation)
        .map_err(|_| "worker process execution failed")?;
    let code = match output.completion {
        Completion::Cancelled => 130,
        Completion::TimedOut => 124,
        Completion::Exited => {
            let code = output
                .exit_code
                .unwrap_or_else(|| output.signal.map_or(1, |s| 128 + s))
                .clamp(0, 255);
            if code == 0 && matches!(output.disposition, Disposition::OutputLimitExceeded(_)) {
                125
            } else {
                code
            }
        }
    };
    eprintln!(
        "kiln worker: step {} completed; exit={} stdout_truncated={} stderr_truncated={}",
        step.node_id, code, output.stdout.truncated, output.stderr.truncated
    );
    Ok(StepOutput {
        code,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}
