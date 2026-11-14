use super::{FrozenProgram, MAX_FILE_BYTES, bounded_file, digest, text};
use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) struct Output {
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(super) fn admit_program(program: &FrozenProgram) -> Result<(), String> {
    // Nix store is the admitted immutable executable cohort, not a worker path.
    if !program.path.is_absolute()
        || !program.path.starts_with("/nix/store")
        || program.blake3.len() != 64
        || !std::fs::canonicalize(&program.path)
            .map_err(text)?
            .starts_with("/nix/store")
        || digest(&bounded_file(&program.path, MAX_FILE_BYTES)?) != program.blake3
    {
        return Err("verification executable path or BLAKE3 differs from immutable profile".into());
    }
    Ok(())
}

pub(super) fn execute(
    program: &Path,
    args: &[String],
    root: &Path,
    environment: &BTreeMap<String, String>,
    timeout_ms: u64,
) -> Result<Output, String> {
    let request = RunRequest {
        command: CommandSpec {
            program: program.into(),
            args: args.iter().map(Into::into).collect(),
            current_dir: root.into(),
            environment_mode: EnvironmentMode::Clear,
            environment: environment
                .iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms,
            stdin_max_bytes: 1,
            stdout_max_bytes: 4 * 1024 * 1024,
            stderr_max_bytes: 4 * 1024 * 1024,
            poll_interval_ms: 10,
            teardown_timeout_ms: 5_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(text)?,
    };
    // Any shell error is conservative unknown. A reservation is already durable;
    // no passing receipt and no automatic rerun can be synthesized after crash.
    match run(request) {
        Ok(output) => Ok(Output {
            status: match output.disposition {
                Disposition::Succeeded => "passed",
                Disposition::ExitFailed if output.exit_code.is_some() => "failed",
                Disposition::ExitFailed => "unknown",
                Disposition::TimedOut => "timed_out",
                Disposition::Cancelled => "cancelled",
                Disposition::OutputLimitExceeded(_) => "unknown",
            }
            .into(),
            exit_code: output.exit_code,
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Err(_) => Ok(Output {
            status: "unknown".into(),
            exit_code: None,
            stdout: vec![],
            stderr: vec![],
        }),
    }
}
