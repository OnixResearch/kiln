use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, Operation as ExecOperation,
    OutcomePolicy, RunError, RunRequest, TerminationScope, run, run_with_cancellation,
};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};

const HASH_BUFFER_BYTES: usize = 65_536;
const SUCCESS_EXIT_STATUS: i32 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub limits: ExecutionLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NixEvalProcessOutput {
    Exited {
        process_status: Option<i32>,
        signal: Option<i32>,
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

pub trait NixEvalRuntime {
    fn file_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId>;
    fn read_file(&self, path: &Path, max_bytes: u64) -> PortResult<Vec<u8>>;
    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()>;
    fn list_gc_roots(&self, path: &Path, max_roots: usize) -> PortResult<Vec<ExternalRef>>;
    fn cleanup_gc_root_directory(&mut self, path: &Path, max_roots: usize) -> PortResult<()>;
    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixEvalProcessOutput>;
    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<NixEvalProcessOutput>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoundedNixEvalRuntime;

impl NixEvalRuntime for BoundedNixEvalRuntime {
    fn file_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        hash_bounded_file(path, max_bytes)
    }

    fn read_file(&self, path: &Path, max_bytes: u64) -> PortResult<Vec<u8>> {
        read_bounded_file(path, max_bytes)
    }

    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()> {
        if !path.is_absolute() || path.exists() {
            return Err(domain(
                "nix_eval_gc_root_path",
                "nix_eval.gc_roots",
                "GC-root directory must be absolute and newly owned by this evaluation",
            ));
        }
        fs::create_dir_all(path).map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_create",
                "GC-root directory could not be created",
            )
        })?;
        set_private_directory_permissions(path)
    }

    fn list_gc_roots(&self, path: &Path, max_roots: usize) -> PortResult<Vec<ExternalRef>> {
        observe_gc_roots(path, max_roots)
    }

    fn cleanup_gc_root_directory(&mut self, path: &Path, max_roots: usize) -> PortResult<()> {
        cleanup_gc_roots(path, max_roots)
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixEvalProcessOutput> {
        execute(invocation, None)
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<NixEvalProcessOutput> {
        execute(invocation, Some(cancellation))
    }
}

fn execute(
    invocation: &ProcessInvocation,
    cancellation: Option<&AtomicBool>,
) -> PortResult<NixEvalProcessOutput> {
    let outcome_policy =
        OutcomePolicy::new(vec![SUCCESS_EXIT_STATUS], true, true).map_err(|_| {
            domain(
                "nix_eval_process_policy",
                "nix_eval.process",
                "Nix evaluation process outcome policy is invalid",
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
            return Ok(NixEvalProcessOutput::Unknown);
        }
        Err(_) => {
            return Err(infrastructure(
                "nix_eval_process_start",
                "Nix evaluation process could not start with the admitted invocation",
            ));
        }
    };
    match output.disposition {
        Disposition::Succeeded | Disposition::ExitFailed => Ok(NixEvalProcessOutput::Exited {
            process_status: output.exit_code,
            signal: output.signal,
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::TimedOut => Ok(NixEvalProcessOutput::TimedOut {
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::Cancelled => Ok(NixEvalProcessOutput::Cancelled {
            stdout: output.stdout.bytes,
            stderr: output.stderr.bytes,
        }),
        Disposition::OutputLimitExceeded(_) => Ok(NixEvalProcessOutput::OutputLimitExceeded {
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

fn hash_bounded_file(path: &Path, max_bytes: u64) -> PortResult<ContentId> {
    if max_bytes == 0 || !path.is_absolute() {
        return Err(domain(
            "nix_eval_file_bound",
            "nix_eval.file",
            "file hashing needs an absolute path and a positive bound",
        ));
    }
    let mut file = File::open(path).map_err(|_| {
        infrastructure(
            "nix_eval_file_open",
            "file is unavailable at the admitted path",
        )
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| infrastructure("nix_eval_file_metadata", "file metadata is unavailable"))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(domain(
            "nix_eval_file_bound",
            "nix_eval.file",
            "file is not a bounded regular file",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|_| {
            infrastructure(
                "nix_eval_file_read",
                "file could not be read for identity verification",
            )
        })?;
        if count == 0 {
            break;
        }
        let count_u64 = u64::try_from(count).map_err(|_| {
            infrastructure(
                "nix_eval_file_count",
                "file byte count is not representable",
            )
        })?;
        observed_bytes = observed_bytes
            .checked_add(count_u64)
            .ok_or_else(|| infrastructure("nix_eval_file_count", "file byte count overflowed"))?;
        if observed_bytes > max_bytes {
            return Err(domain(
                "nix_eval_file_bound",
                "nix_eval.file",
                "file exceeds its named byte bound",
            ));
        }
        let bytes = buffer.get(..count).ok_or_else(|| {
            infrastructure(
                "nix_eval_file_buffer",
                "file read exceeded the fixed hashing buffer",
            )
        })?;
        hasher.update(bytes);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string()).map_err(PortFailure::Domain)
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> PortResult<Vec<u8>> {
    if max_bytes == 0 || !path.is_absolute() {
        return Err(domain(
            "nix_eval_file_bound",
            "nix_eval.file",
            "file read needs an absolute path and a positive bound",
        ));
    }
    let metadata = fs::metadata(path)
        .map_err(|_| infrastructure("nix_eval_file_metadata", "file metadata is unavailable"))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(domain(
            "nix_eval_file_bound",
            "nix_eval.file",
            "file is not a bounded regular file",
        ));
    }
    fs::read(path).map_err(|_| {
        infrastructure(
            "nix_eval_file_read",
            "file could not be read under its admitted bound",
        )
    })
}

fn observe_gc_roots(path: &Path, max_roots: usize) -> PortResult<Vec<ExternalRef>> {
    if max_roots == 0 || !path.is_absolute() || !path.is_dir() {
        return Err(domain(
            "nix_eval_gc_root_bound",
            "nix_eval.gc_roots",
            "GC-root observation needs an absolute directory and a positive bound",
        ));
    }
    let mut entries = fs::read_dir(path)
        .map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_read",
                "GC-root directory could not be read",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_read",
                "GC-root directory entry could not be read",
            )
        })?;
    entries.sort_by_key(fs::DirEntry::file_name);
    if entries.len() > max_roots {
        return Err(domain(
            "nix_eval_gc_root_bound",
            "nix_eval.gc_roots",
            "GC-root count exceeds its named bound",
        ));
    }
    let mut roots = Vec::with_capacity(entries.len());
    for entry in entries {
        let file_type = entry.file_type().map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_type",
                "GC-root entry type could not be observed",
            )
        })?;
        if !file_type.is_symlink() {
            return Err(domain(
                "nix_eval_gc_root_type",
                "nix_eval.gc_roots",
                "GC-root directory contains an entry that is not a symbolic link",
            ));
        }
        let target = fs::read_link(entry.path()).map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_target",
                "GC-root symbolic link could not be read",
            )
        })?;
        let target_text = target.to_str().ok_or_else(|| {
            domain(
                "nix_eval_gc_root_encoding",
                "nix_eval.gc_roots",
                "GC-root target is not valid UTF-8",
            )
        })?;
        if !target.is_absolute()
            || !target_text.starts_with("/nix/store/")
            || !target_text.ends_with(".drv")
        {
            return Err(domain(
                "nix_eval_gc_root_target",
                "nix_eval.gc_roots",
                "GC-root target is not an absolute Nix derivation path",
            ));
        }
        roots.push(ExternalRef::new("nix", "derivation", target_text)?);
    }
    roots.sort();
    Ok(roots)
}

fn cleanup_gc_roots(path: &Path, max_roots: usize) -> PortResult<()> {
    let roots = observe_gc_roots(path, max_roots)?;
    let root_count = roots.len();
    let entries = fs::read_dir(path)
        .map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_read",
                "GC-root directory could not be read for cleanup",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_read",
                "GC-root directory entry could not be read for cleanup",
            )
        })?;
    if entries.len() != root_count {
        return Err(domain(
            "nix_eval_gc_root_changed",
            "nix_eval.gc_roots",
            "GC-root directory changed after bounded observation",
        ));
    }
    for entry in entries {
        fs::remove_file(entry.path()).map_err(|_| {
            infrastructure(
                "nix_eval_gc_root_remove",
                "GC-root symbolic link could not be removed",
            )
        })?;
    }
    fs::remove_dir(path).map_err(|_| {
        infrastructure(
            "nix_eval_gc_root_remove",
            "GC-root directory could not be removed",
        )
    })
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> PortResult<()> {
    use std::os::unix::fs::PermissionsExt;

    const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE)).map_err(|_| {
        infrastructure(
            "nix_eval_gc_root_permissions",
            "GC-root directory permissions could not be restricted",
        )
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> PortResult<()> {
    Err(domain(
        "nix_eval_platform",
        "nix_eval.gc_roots",
        "the pinned Nix evaluation cohort requires Unix directory permissions",
    ))
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
