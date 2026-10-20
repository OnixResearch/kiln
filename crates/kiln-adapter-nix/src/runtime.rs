use std::ffi::OsString;
use std::fs::{self, File, Permissions};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
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
pub enum NixBuildProcessOutput {
    Exited {
        process_status: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Cancelled {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    OutputLimitExceeded {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Unknown,
}

pub trait NixBuildRuntime {
    fn file_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId>;
    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()>;
    fn cleanup_gc_root_directory(&mut self, path: &Path, max_roots: usize) -> PortResult<()>;
    fn read_link(&self, path: &Path) -> PortResult<PathBuf>;
    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixBuildProcessOutput>;
    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<NixBuildProcessOutput>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoundedNixBuildRuntime;

impl NixBuildRuntime for BoundedNixBuildRuntime {
    fn file_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        hash_bounded_file(path, max_bytes)
    }

    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()> {
        prepare_gc_root_directory(path)
    }

    fn cleanup_gc_root_directory(&mut self, path: &Path, max_roots: usize) -> PortResult<()> {
        cleanup_gc_root_directory(path, max_roots)
    }

    fn read_link(&self, path: &Path) -> PortResult<PathBuf> {
        read_gc_root_link(path)
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixBuildProcessOutput> {
        execute(invocation, None)
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<NixBuildProcessOutput> {
        execute(invocation, Some(cancellation))
    }
}

fn execute(
    invocation: &ProcessInvocation,
    cancellation: Option<&AtomicBool>,
) -> PortResult<NixBuildProcessOutput> {
    let outcome_policy = OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true).map_err(|_| {
        domain(
            "nix_process_policy",
            "nix.process",
            "Nix process outcome policy is invalid",
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
        Err(error) if execution_may_have_started(&error) => {
            return Ok(NixBuildProcessOutput::Unknown);
        }
        Err(_) => {
            return Err(infrastructure(
                "nix_process_start",
                "Nix process could not start with the admitted invocation",
            ));
        }
    };
    match output.disposition {
        Disposition::Succeeded | Disposition::ExitFailed => Ok(NixBuildProcessOutput::Exited {
            process_status: output.exit_code,
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::TimedOut => Ok(NixBuildProcessOutput::TimedOut {
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::Cancelled => Ok(NixBuildProcessOutput::Cancelled {
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::OutputLimitExceeded(_) => Ok(NixBuildProcessOutput::OutputLimitExceeded {
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
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

fn prepare_gc_root_directory(path: &Path) -> PortResult<()> {
    if !path.is_absolute() {
        return Err(domain(
            "nix_gc_root_path",
            "nix.gc_root",
            "GC-root directory must be absolute",
        ));
    }
    if path.exists() {
        if !path.is_dir() {
            return Err(domain(
                "nix_gc_root_path",
                "nix.gc_root",
                "GC-root path exists but is not a directory",
            ));
        }
    } else {
        fs::create_dir_all(path).map_err(|_| {
            infrastructure(
                "nix_gc_root_create",
                "GC-root directory could not be created",
            )
        })?;
    }
    fs::set_permissions(path, Permissions::from_mode(0o700)).map_err(|_| {
        infrastructure(
            "nix_gc_root_permissions",
            "GC-root directory permissions could not be restricted",
        )
    })
}

fn cleanup_gc_root_directory(path: &Path, max_roots: usize) -> PortResult<()> {
    if !path.is_absolute() || max_roots == 0 {
        return Err(domain(
            "nix_gc_root_path",
            "nix.gc_root",
            "GC-root cleanup needs an absolute directory and a positive bound",
        ));
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(infrastructure(
                "nix_gc_root_read",
                "GC-root directory could not be read",
            ));
        }
    };
    let mut count = 0_usize;
    for entry in entries {
        let entry = entry
            .map_err(|_| infrastructure("nix_gc_root_read", "GC-root entry could not be read"))?;
        count = count.saturating_add(1);
        if count > max_roots {
            return Err(domain(
                "nix_gc_root_bound",
                "nix.gc_root",
                "GC-root directory exceeds its named entry bound",
            ));
        }
        let file_type = entry
            .file_type()
            .map_err(|_| infrastructure("nix_gc_root_type", "GC-root entry type is unavailable"))?;
        if file_type.is_dir() {
            return Err(domain(
                "nix_gc_root_entry",
                "nix.gc_root",
                "GC-root directory contains a nested directory",
            ));
        }
        fs::remove_file(entry.path()).map_err(|_| {
            infrastructure("nix_gc_root_remove", "GC-root entry could not be removed")
        })?;
    }
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(infrastructure(
            "nix_gc_root_remove",
            "GC-root directory could not be removed",
        )),
    }
}

fn read_gc_root_link(path: &Path) -> PortResult<PathBuf> {
    if !path.is_absolute() {
        return Err(domain(
            "nix_gc_root_path",
            "nix.gc_root",
            "GC-root link path must be absolute",
        ));
    }
    let target = fs::read_link(path)
        .map_err(|_| infrastructure("nix_gc_root_link", "GC-root link could not be read"))?;
    if !target.is_absolute() {
        return Err(domain(
            "nix_gc_root_target",
            "nix.gc_root",
            "GC-root target must be absolute",
        ));
    }
    Ok(target)
}

fn hash_bounded_file(path: &Path, max_bytes: u64) -> PortResult<ContentId> {
    if max_bytes == 0 || !path.is_absolute() {
        return Err(domain(
            "nix_file_bound",
            "nix.file",
            "Nix file hashing needs an absolute path and a positive bound",
        ));
    }
    let mut file = File::open(path).map_err(|_| {
        infrastructure(
            "nix_file_open",
            "Nix file is unavailable at the admitted path",
        )
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| infrastructure("nix_file_metadata", "Nix file metadata is unavailable"))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(domain(
            "nix_file_bound",
            "nix.file",
            "Nix file is not a bounded regular file",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|_| {
            infrastructure(
                "nix_file_read",
                "Nix file could not be read for identity verification",
            )
        })?;
        if count == 0 {
            break;
        }
        observed_bytes = observed_bytes
            .checked_add(u64::try_from(count).map_err(|_| {
                infrastructure("nix_file_count", "Nix file byte count is not representable")
            })?)
            .ok_or_else(|| infrastructure("nix_file_count", "Nix file byte count overflowed"))?;
        if observed_bytes > max_bytes {
            return Err(domain(
                "nix_file_bound",
                "nix.file",
                "Nix file exceeds its named byte bound",
            ));
        }
        let bytes = buffer.get(..count).ok_or_else(|| {
            infrastructure(
                "nix_file_buffer",
                "Nix file read exceeded the fixed hashing buffer",
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
