use std::ffi::OsString;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, Operation, OutcomePolicy,
    RunError, RunRequest, TerminationScope, run,
};

use super::model::{
    HOSTED_NIX_PROVIDER_SCHEMA, HostedNixProviderProfile, HostedProviderError,
    HostedProviderOutcome, HostedProviderRequest, HostedProviderResult,
};

const SUCCESS_EXIT_CODE: i32 = 0;

#[derive(Debug)]
pub(super) struct ProcessObservation {
    pub(super) outcome: HostedProviderOutcome,
    pub(super) exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub(super) fn execute_process(
    profile: &HostedNixProviderProfile,
    source_uri: &str,
) -> HostedProviderResult<ProcessObservation> {
    let limits = profile.limits;
    let stdin_max_bytes = checked_usize(limits.stdin_max_bytes, "stdin bound")?;
    let stdout_max_bytes = checked_usize(limits.stdout_max_bytes, "stdout bound")?;
    let stderr_max_bytes = checked_usize(limits.stderr_max_bytes, "stderr bound")?;
    let request = RunRequest {
        command: CommandSpec {
            program: profile.wrapper_path.clone(),
            args: vec![
                OsString::from("flake"),
                OsString::from("check"),
                OsString::from("--no-update-lock-file"),
                OsString::from(source_uri),
            ],
            current_dir: profile.working_directory.clone(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: limits.timeout_milliseconds,
            stdin_max_bytes,
            stdout_max_bytes,
            stderr_max_bytes,
            poll_interval_ms: limits.poll_interval_milliseconds,
            teardown_timeout_ms: limits.teardown_timeout_milliseconds,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true).map_err(|_| {
            HostedProviderError::Admission("provider outcome policy is invalid".to_string())
        })?,
    };
    match run(request) {
        Ok(output) => {
            let outcome = match output.disposition {
                Disposition::Succeeded => HostedProviderOutcome::Success,
                Disposition::ExitFailed => HostedProviderOutcome::Failure,
                Disposition::TimedOut => HostedProviderOutcome::TimedOut,
                Disposition::Cancelled => HostedProviderOutcome::Cancelled,
                Disposition::OutputLimitExceeded(_) => HostedProviderOutcome::OutputLimitExceeded,
            };
            Ok(ProcessObservation {
                outcome,
                exit_code: output.exit_code,
                stdout: output.stdout.bytes,
                stderr: output.stderr.bytes,
            })
        }
        Err(error) => Ok(ProcessObservation {
            outcome: if may_have_started(&error) {
                HostedProviderOutcome::Unknown
            } else {
                HostedProviderOutcome::ProviderError
            },
            exit_code: None,
            stdout: Vec::new(),
            stderr: error.to_string().into_bytes(),
        }),
    }
}

pub(super) fn render_log(
    profile: &HostedNixProviderProfile,
    request: &HostedProviderRequest,
    source_uri: &str,
    observation: &ProcessObservation,
) -> HostedProviderResult<Vec<u8>> {
    let mut bytes = format!(
        "schema={HOSTED_NIX_PROVIDER_SCHEMA}\nrepository={}\nrevision={}\nsource={}\noutcome={:?}\nexit_code={:?}\n--- stdout ---\n",
        request.repository,
        request.revision,
        source_uri,
        observation.outcome,
        observation.exit_code,
    )
    .into_bytes();
    bytes.extend_from_slice(&observation.stdout);
    bytes.extend_from_slice(b"\n--- stderr ---\n");
    bytes.extend_from_slice(&observation.stderr);
    let maximum = checked_usize(profile.limits.report_max_bytes, "report bound")?;
    if bytes.len() > maximum {
        return Err(HostedProviderError::Admission(
            "combined provider log exceeds its named byte bound".to_string(),
        ));
    }
    Ok(bytes)
}

fn may_have_started(error: &RunError) -> bool {
    match error {
        RunError::InvalidLimits(_)
        | RunError::EmptyProgram
        | RunError::ProgramNotAbsolute
        | RunError::EmptyCurrentDirectory
        | RunError::CurrentDirectoryNotAbsolute
        | RunError::DuplicateEnvironmentName
        | RunError::InputLimitExceeded
        | RunError::Io {
            operation: Operation::Spawn,
            ..
        } => false,
        RunError::MissingPipe(_)
        | RunError::Io { .. }
        | RunError::CaptureCountOverflow(_)
        | RunError::WorkerDisconnected(_)
        | RunError::WorkerPanicked(_)
        | RunError::WorkerTeardownTimedOut(_)
        | RunError::ChildTeardownTimedOut
        | RunError::ProcessIdentifierOverflow => true,
    }
}

fn checked_usize(value: u64, label: &str) -> HostedProviderResult<usize> {
    usize::try_from(value)
        .map_err(|_| HostedProviderError::Admission(format!("{label} does not fit this host")))
}
