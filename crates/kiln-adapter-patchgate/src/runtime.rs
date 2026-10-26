use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::sync::atomic::AtomicBool;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, Operation as ExecOperation,
    OutcomePolicy, RunError, RunRequest, TerminationScope, run, run_with_cancellation,
};
use kiln_core::review::ReviewEffectPlan;
use kiln_core::types::ContentId;
use kiln_provider_api::{PortFailure, PortResult};

use crate::{PatchgateProfile, domain, failure};

const HASH_BUFFER_BYTES: usize = 65_536;
const NULL_STDIN_BOUND_BYTES: usize = 1;
pub(crate) const SUCCESS_EXIT_CODE: i32 = 0;

pub(crate) struct ProcessOutput {
    pub disposition: Disposition,
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
}

pub(crate) fn verify_executable(profile: &PatchgateProfile) -> PortResult<()> {
    let mut file = File::open(&profile.program).map_err(|_| {
        PortFailure::Infrastructure(failure(
            "patchgate_executable_open",
            "Pinned Patchgate executable is unavailable",
        ))
    })?;
    let metadata = file.metadata().map_err(|_| {
        PortFailure::Infrastructure(failure(
            "patchgate_executable_metadata",
            "Cannot inspect Patchgate executable",
        ))
    })?;
    if !metadata.is_file() || metadata.len() > profile.limits.max_executable_bytes {
        return Err(domain(
            "patchgate_executable_bound",
            "Patchgate executable must be a bounded regular file",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|_| {
            PortFailure::Infrastructure(failure(
                "patchgate_executable_read",
                "Cannot hash Patchgate executable",
            ))
        })?;
        if count == 0 {
            break;
        }
        observed = observed.checked_add(count as u64).ok_or_else(|| {
            domain(
                "patchgate_executable_bound",
                "Executable byte count overflowed",
            )
        })?;
        if observed > profile.limits.max_executable_bytes {
            return Err(domain(
                "patchgate_executable_bound",
                "Executable exceeds its named hashing bound",
            ));
        }
        let chunk = buffer.get(..count).ok_or_else(|| {
            PortFailure::Infrastructure(failure(
                "patchgate_executable_read",
                "Executable read exceeded the hashing buffer",
            ))
        })?;
        hasher.update(chunk);
    }
    let identity = ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .map_err(PortFailure::Domain)?;
    if identity != profile.executable_identity {
        return Err(domain(
            "patchgate_executable_identity",
            "Executable BLAKE3 differs from the pinned profile",
        ));
    }
    Ok(())
}

pub(crate) fn invocation(profile: &PatchgateProfile, plan: &ReviewEffectPlan) -> RunRequest {
    let mut args: Vec<OsString> = vec![
        "--repo".into(),
        profile.repository_root.clone().into_os_string(),
        "--main".into(),
        profile.native.main.channel.clone().into(),
        "--inbox".into(),
        profile.native.inbox.channel.clone().into(),
        "review".into(),
        "--dry-run".into(),
        "--json".into(),
    ];
    for candidate in &plan.candidate.ordered_changes {
        args.push("--change".into());
        args.push(candidate.change.native_id.as_str().into());
    }
    RunRequest {
        command: CommandSpec {
            program: profile.program.clone(),
            args,
            current_dir: profile.repository_root.clone(),
            environment_mode: EnvironmentMode::Clear,
            environment: profile
                .environment
                .iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: profile.limits.timeout_ms,
            // bounded-exec requires a positive bound even with null stdin.
            stdin_max_bytes: NULL_STDIN_BOUND_BYTES,
            stdout_max_bytes: profile.limits.max_report_bytes,
            stderr_max_bytes: profile.limits.max_stderr_bytes,
            poll_interval_ms: profile.limits.poll_interval_ms,
            teardown_timeout_ms: profile.limits.teardown_timeout_ms,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true)
            .unwrap_or_else(|_| unreachable!("the fixed success policy is valid")),
    }
}

pub(crate) fn execute(
    request: RunRequest,
    cancellation: Option<&AtomicBool>,
) -> PortResult<ProcessOutput> {
    let result = match cancellation {
        Some(cancellation) => run_with_cancellation(request, cancellation),
        None => run(request),
    };
    match result {
        Ok(output) => Ok(ProcessOutput {
            disposition: output.disposition,
            status: output.exit_code,
            stdout: output.stdout.bytes,
        }),
        Err(error) => {
            let detail = failure(
                "patchgate_process",
                "Bounded Patchgate process execution failed",
            );
            Err(if may_have_started(&error) {
                PortFailure::Unknown(detail)
            } else {
                PortFailure::Infrastructure(detail)
            })
        }
    }
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
            operation: ExecOperation::Spawn,
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
