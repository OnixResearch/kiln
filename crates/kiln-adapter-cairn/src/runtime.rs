use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, Operation as ExecOperation,
    OutcomePolicy, RunError, RunRequest, TerminationScope, run, run_with_cancellation,
};
use kiln_core::types::{ContentId, Diagnostic};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};

const HASH_BUFFER_BYTES: usize = 65_536;
const SUCCESS_EXIT_CODE: i32 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub limits: ExecutionLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CairnProcessOutput {
    Exited {
        process_status: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut,
    Cancelled,
    OutputLimitExceeded,
    Unknown,
}

pub trait CairnRuntime {
    fn executable_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId>;
    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<CairnProcessOutput>;
    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<CairnProcessOutput>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoundedCairnRuntime;

impl CairnRuntime for BoundedCairnRuntime {
    fn executable_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        hash_bounded_file(path, max_bytes)
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<CairnProcessOutput> {
        execute(invocation, None)
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<CairnProcessOutput> {
        execute(invocation, Some(cancellation))
    }
}

fn execute(
    invocation: &ProcessInvocation,
    cancellation: Option<&AtomicBool>,
) -> PortResult<CairnProcessOutput> {
    let outcome_policy = OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true).map_err(|_| {
        domain(
            "cairn_process_policy",
            "cairn.process",
            "Cairn process outcome policy is invalid",
        )
    })?;
    let request = RunRequest {
        command: CommandSpec {
            program: invocation.program.clone(),
            args: invocation.args.clone(),
            current_dir: invocation.current_dir.clone(),
            environment_mode: EnvironmentMode::Clear,
            environment: invocation.environment.clone(),
            input: Input::Null,
        },
        limits: invocation.limits,
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy,
    };
    let output = match cancellation {
        Some(cancellation) => run_with_cancellation(request, cancellation),
        None => run(request),
    };
    let output = match output {
        Ok(output) => output,
        Err(error) if execution_may_have_started(&error) => return Ok(CairnProcessOutput::Unknown),
        Err(_error) => {
            return Err(infrastructure(
                "cairn_process_start",
                "Cairn process could not start with the admitted invocation",
            ));
        }
    };
    match output.disposition {
        Disposition::Succeeded | Disposition::ExitFailed => Ok(CairnProcessOutput::Exited {
            process_status: output.exit_code,
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::TimedOut => Ok(CairnProcessOutput::TimedOut),
        Disposition::Cancelled => Ok(CairnProcessOutput::Cancelled),
        Disposition::OutputLimitExceeded(_) => Ok(CairnProcessOutput::OutputLimitExceeded),
    }
}

fn execution_may_have_started(error: &RunError) -> bool {
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

fn hash_bounded_file(path: &Path, max_bytes: u64) -> PortResult<ContentId> {
    if max_bytes == 0 || !path.is_absolute() {
        return Err(domain(
            "cairn_executable_bound",
            "cairn.executable",
            "Cairn executable hashing needs an absolute path and a positive bound",
        ));
    }
    let mut file = File::open(path).map_err(|_| {
        infrastructure(
            "cairn_executable_open",
            "Cairn executable is unavailable at the admitted path",
        )
    })?;
    let metadata = file.metadata().map_err(|_| {
        infrastructure(
            "cairn_executable_metadata",
            "Cairn executable metadata is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(domain(
            "cairn_executable_bound",
            "cairn.executable",
            "Cairn executable is not a bounded regular file",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|_| {
            infrastructure(
                "cairn_executable_read",
                "Cairn executable could not be read for identity verification",
            )
        })?;
        if count == 0 {
            break;
        }
        observed_bytes = observed_bytes
            .checked_add(u64::try_from(count).map_err(|_| {
                infrastructure(
                    "cairn_executable_count",
                    "Cairn executable byte count is not representable",
                )
            })?)
            .ok_or_else(|| {
                infrastructure(
                    "cairn_executable_count",
                    "Cairn executable byte count overflowed",
                )
            })?;
        if observed_bytes > max_bytes {
            return Err(domain(
                "cairn_executable_bound",
                "cairn.executable",
                "Cairn executable exceeds its named byte bound",
            ));
        }
        let bytes = buffer.get(..count).ok_or_else(|| {
            infrastructure(
                "cairn_executable_buffer",
                "Cairn executable read exceeded the fixed hashing buffer",
            )
        })?;
        hasher.update(bytes);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string()).map_err(PortFailure::Domain)
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}
