//! Real bounded transport to the explicitly adopted Lattice implementation CLI.
use crate::lineage::{self, Route};
use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use kiln_core::fair_admission::ClaimRef;
use kiln_core::implementation::{
    Enrollment, FrozenRequest, ImplementationDispatchPlan, TransportLaunch, TransportStart,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[path = "worker_binding.rs"]
mod binding;
#[path = "worker_metrics.rs"]
mod metrics;
#[path = "worker_settlement.rs"]
mod settlement;

const MAX_OMP_OBSERVATION_BYTES: u64 = 16 * 1024;

fn read_metrics_artifact(
    state: &Path,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    response: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    // This is the native producer's per-attempt directory identity. An artifact
    // pointer from another attempt cannot supply this attempt's observations.
    let key = state.join(format!("{}-{}", attempt.identity.as_str(), attempt.fence));
    let expected = state
        .join(
            blake3::hash(key.as_os_str().as_encoded_bytes())
                .to_hex()
                .as_str(),
        )
        .join("omp-observation.json");
    let path = response
        .pointer("/artifacts/omp_observation")
        .and_then(serde_json::Value::as_str)
        .map(Path::new)
        .ok_or("worker metrics artifact is unavailable")?;
    if path != expected
        || std::fs::canonicalize(path).map_err(|error| error.to_string())? != expected
    {
        return Err("worker metrics artifact belongs to another attempt or path".into());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_OMP_OBSERVATION_BYTES {
        return Err("worker metrics artifact is not a bounded ordinary file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_OMP_OBSERVATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_OMP_OBSERVATION_BYTES {
        return Err("worker metrics artifact exceeds its bound".into());
    }
    Ok(bytes)
}

const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
fn transport_profile_mode_matches(
    schema: &str,
    native_request_profile: Option<&ContentId>,
) -> bool {
    matches!(
        (schema, native_request_profile),
        ("kiln.lattice-implementation-cli.v1", None)
            | ("kiln.lattice-implementation-cli.v2", Some(_))
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerTransportProfile {
    pub schema: String,
    pub lattice_program: PathBuf,
    pub executable_blake3: ContentId,
    pub state_directory: PathBuf,
    pub output_directory: PathBuf,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return Err("transport input is not a bounded ordinary file".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}
fn required(flags: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    flags
        .get(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is required"))
}
fn clock() -> Result<u64, String> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    u64::try_from(ms).map_err(|error| error.to_string())
}
fn program_identity(path: &Path) -> Result<ContentId, String> {
    if !path.is_absolute()
        || !path.starts_with("/nix/store")
        || !std::fs::canonicalize(path)
            .map_err(|error| error.to_string())?
            .starts_with("/nix/store")
    {
        return Err("worker transport executable is not an admitted immutable Nix program".into());
    }
    let mut input = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..count)
                .ok_or("worker executable read boundary invalid")?,
        );
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .map_err(|error| format!("{error:?}"))
}
fn binding<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a serde_json::Value, String> {
    value
        .get(field)
        .ok_or_else(|| format!("worker request lacks {field}"))
}
/// One Lattice CLI command with a cleared ambient environment plus exactly the
/// supplied entries.
fn lattice_command(
    program: PathBuf,
    args: Vec<std::ffi::OsString>,
    current_dir: PathBuf,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> CommandSpec {
    CommandSpec {
        program,
        args,
        current_dir,
        environment_mode: EnvironmentMode::Clear,
        environment,
        input: Input::Null,
    }
}

/// The frozen request file for sent bytes with BLAKE3 `digest`. Without a
/// lineage the sent bytes are the admitted request, so the name is the one
/// earlier launches used.
#[must_use]
pub fn frozen_request_path(output_directory: &Path, digest: &ContentId) -> PathBuf {
    output_directory.join(format!("request-{}.json", digest.as_str()))
}

/// The request file Lattice ran for an attempt: the one recorded for its
/// launch (`Implementer::dispatch_request`), or, for a launch recorded
/// before Kiln recorded its sent request, the admitted request frozen in the
/// profile's output directory. Lattice binds its state to these exact bytes,
/// so `recover`, `cancel`, `inspect` and `export-candidate` must name this
/// file, not the admitted request. [`sent_request_and_state`] pairs it with
/// the launch's state root; callers in another process read both, verified,
/// from `kiln-lattice-dispatch --worker-request <admitted> --sent-request
/// true` ([`sent_request`], schema [`SENT_REQUEST_SCHEMA`]).
///
/// # Errors
/// Only if BLAKE3 hex were not a content identity.
pub fn sent_request_path(
    request: Option<&FrozenRequest>,
    profile: &WorkerTransportProfile,
    admitted: &[u8],
) -> Result<PathBuf, String> {
    match request {
        Some(request) => Ok(PathBuf::from(&request.path)),
        None => Ok(frozen_request_path(
            &profile.output_directory,
            &blake3_identity(admitted)?,
        )),
    }
}

/// The request file and state root an attempt's recorded launch ran, which
/// `recover`, `cancel`, `inspect` and `export-candidate` must name: the
/// request is [`sent_request_path`]; the state root is the recorded launch's
/// (`Implementer::dispatch_launch`), or the profile's for a transport intent
/// recorded without a launch. `sent` is the request recorded for that
/// launch and `admitted` the admitted request's bytes.
///
/// # Errors
/// Only if BLAKE3 hex were not a content identity.
pub fn sent_request_and_state(
    launch: Option<&TransportLaunch>,
    sent: Option<&FrozenRequest>,
    profile: &WorkerTransportProfile,
    admitted: &[u8],
) -> Result<(PathBuf, PathBuf), String> {
    let request = sent_request_path(sent, profile, admitted)?;
    let state = launch.map_or_else(
        || profile.state_directory.clone(),
        |launch| PathBuf::from(&launch.state_directory),
    );
    Ok((request, state))
}

/// `schema` of the one object `--sent-request true` prints.
pub const SENT_REQUEST_SCHEMA: &str = "kiln.lattice-sent-request.v1";

/// What `--sent-request true` prints. `request` and `state_directory` are
/// exactly as recorded (or resolved for a legacy launch), never
/// canonicalized.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SentRequestReport {
    schema: String,
    attempt: String,
    fence: u64,
    recorded: bool,
    request: String,
    request_blake3: String,
    state_directory: String,
}

/// Whether `record` speaks for the admitted request: the attempt holds
/// `fence`, its request seal from bind-dispatch is these exact bytes
/// (`admitted`, their BLAKE3), and its transport intent is recorded.
fn sent_request_admitted(
    record: &kiln_implementer::DispatchRecord,
    fence: u64,
    admitted: &ContentId,
) -> Result<(), String> {
    let attempt = &record.attempt;
    if attempt.fence != fence {
        return Err(format!(
            "fence {fence} is unknown to the ledger for attempt {}",
            attempt.identity.as_str()
        ));
    }
    let bound = attempt
        .dispatch_request_binding
        .as_ref()
        .is_some_and(|binding| {
            binding.attempt == attempt.identity
                && binding.fence == fence
                && binding.request_digest == *admitted
        });
    if !bound {
        return Err("the admitted request is not the request bound at bind-dispatch".into());
    }
    if !record.transport_started {
        return Err(
            "no dispatch transport intent is recorded for the attempt; nothing was dispatched"
                .into(),
        );
    }
    Ok(())
}

// r[impl kiln.admission.lattice_lineage.scenario.reported]
/// `kiln-lattice-dispatch --database DB --enrollment FILE --worker-request
/// ADMITTED --worker-transport-profile FILE --sent-request true`: one
/// [`SENT_REQUEST_SCHEMA`] object naming the request file and state root the
/// attempt's recorded launch ran, for callers in another process that must
/// name them ([`sent_request_path`]). The attempt and fence are the admitted
/// request's `envelope.attempt_id` and `envelope.fence`. The ledger is opened
/// read-only (`Implementer::open_read_only`) and read in one snapshot; the
/// file is checked as recovery checks it (a bounded ordinary file hashing to
/// the recorded BLAKE3, or the admitted bytes' for a legacy launch, that
/// `verify_frozen` accepts). Nothing is written, no credential is read and
/// Lattice is not started; `--local-operator`, `--dispatch-plan` and
/// `--admission-observations` are not consulted.
///
/// # Errors
/// A flag other than `--sent-request true`, an unreadable input, a ledger in
/// WAL mode (`Implementer::open_read_only`), an attempt or fence unknown to
/// the ledger, admitted bytes that are not the request bound at
/// bind-dispatch, no recorded transport intent, or a recorded file that is
/// missing, not a bounded ordinary file or crossed.
pub fn sent_request(flags: &BTreeMap<String, String>) -> Result<String, String> {
    if flags.get("--sent-request").map(String::as_str) != Some("true") {
        return Err("--sent-request accepts only true".into());
    }
    let admitted = read_bounded(&required(flags, "--worker-request")?)
        .map_err(|error| format!("the admitted worker request is unreadable ({error})"))?;
    let request: serde_json::Value =
        serde_json::from_slice(&admitted).map_err(|error| error.to_string())?;
    let envelope = binding(&request, "envelope")?;
    let attempt = envelope
        .get("attempt_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("worker request lacks envelope.attempt_id")?;
    let attempt = ContentId::from_blake3_hex(attempt).map_err(|error| format!("{error:?}"))?;
    let fence = envelope
        .get("fence")
        .and_then(serde_json::Value::as_u64)
        .ok_or("worker request lacks envelope.fence")?;
    let profile: WorkerTransportProfile =
        read_json(&required(flags, "--worker-transport-profile")?)?;
    let enrollment: Enrollment = read_json(&required(flags, "--enrollment")?)?;
    let record = Implementer::open_read_only(&required(flags, "--database")?, enrollment)?
        .dispatch_record(&attempt)?
        .ok_or_else(|| format!("attempt {} is unknown to the ledger", attempt.as_str()))?;
    let admitted_blake3 = blake3_identity(&admitted)?;
    sent_request_admitted(&record, fence, &admitted_blake3)?;
    let (path, state) = sent_request_and_state(
        record.launch.as_ref(),
        record.request.as_ref(),
        &profile,
        &admitted,
    )?;
    let expected = record
        .request
        .as_ref()
        .map_or(&admitted_blake3, |sent| &sent.blake3);
    let bytes = read_bounded(&path).map_err(|error| {
        format!(
            "the recorded request {} is unavailable ({error})",
            path.display()
        )
    })?;
    let crossed = || {
        format!(
            "the recorded request {} is crossed ({})",
            path.display(),
            lineage::LineageRefusal::FrozenRequestCrossed.code()
        )
    };
    if blake3_identity(&bytes)? != *expected {
        return Err(crossed());
    }
    lineage::verify_frozen(&admitted, &bytes).map_err(|_| crossed())?;
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| "the recorded request and state paths must be UTF-8".to_string())
    };
    serde_json::to_string(&SentRequestReport {
        schema: SENT_REQUEST_SCHEMA.into(),
        attempt: attempt.as_str().to_owned(),
        fence,
        recorded: record.request.is_some(),
        request: text(&path)?,
        request_blake3: expected.as_str().to_owned(),
        state_directory: text(&state)?,
    })
    .map_err(|error| error.to_string())
}

fn blake3_identity(bytes: &[u8]) -> Result<ContentId, String> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string())
        .map_err(|error| format!("{error:?}"))
}

/// Where a launch's request comes from.
#[derive(Debug)]
pub enum RequestSource<'a> {
    /// A first launch: the budget lineage is derived from this snapshot.
    Fresh {
        ledger: &'a kiln_core::fair_admission::AdmissionLedger,
        work: &'a ContentId,
        claim: &'a ClaimRef,
        now_ms: u64,
    },
    /// Recovery: the request recorded for the launch that won the transport
    /// intent, `None` when an earlier binary recorded that launch.
    Recover { request: Option<&'a FrozenRequest> },
}

/// The exact bytes one launch sends and the file they are frozen in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchRequest {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub blake3: ContentId,
}

impl LaunchRequest {
    /// `bytes` frozen under their own BLAKE3 in `output_directory`.
    ///
    /// # Errors
    /// Only if BLAKE3 hex were not a content identity.
    pub fn new(output_directory: &Path, bytes: Vec<u8>) -> Result<Self, String> {
        let blake3 = blake3_identity(&bytes)?;
        Ok(Self {
            path: frozen_request_path(output_directory, &blake3),
            bytes,
            blake3,
        })
    }

    /// The record the transport intent keeps of this request.
    fn frozen(&self) -> Result<FrozenRequest, String> {
        Ok(FrozenRequest {
            path: self
                .path
                .to_str()
                .ok_or("the frozen request path must be UTF-8")?
                .to_owned(),
            blake3: self.blake3.clone(),
        })
    }
}

/// Choose the request a launch sends.
///
/// A first launch appends the lineage derived from the ledger to the
/// admitted bytes when the attempt holds an admitted reservation; an
/// inconsistent lineage is refused before any transport start. Recovery
/// never derives again: the grants are unknown by then. It reads exactly the
/// file recorded for its launch, which must still hash to the recorded
/// BLAKE3 and be the admitted bytes, or them plus one well-formed lineage. A
/// launch recorded before Kiln recorded its request sent the admitted bytes.
///
/// # Errors
/// A lineage refusal, an oversized request, or on recovery a missing,
/// changed or crossed frozen file.
pub fn launch_request(
    source: RequestSource<'_>,
    profile: &WorkerTransportProfile,
    admitted: &[u8],
) -> Result<LaunchRequest, String> {
    let request = match source {
        RequestSource::Fresh {
            ledger,
            work,
            claim,
            now_ms,
        } => {
            let bytes =
                lineage::dispatched_request(Route::Worker, ledger, work, claim, now_ms, admitted)
                    .map_err(|refusal| refusal.to_string())?;
            LaunchRequest::new(&profile.output_directory, bytes.into_owned())?
        }
        RequestSource::Recover { request } => {
            let Some(recorded) = request else {
                return LaunchRequest::new(&profile.output_directory, admitted.to_vec());
            };
            let path = PathBuf::from(&recorded.path);
            let bytes = match read_bounded(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return Err(format!(
                        "the recorded frozen request {} is unavailable ({error}); {}",
                        path.display(),
                        lineage::RECOVERY_NOT_INVOKED
                    ));
                }
            };
            if blake3_identity(&bytes)? != recorded.blake3 {
                return Err(lineage::LineageRefusal::FrozenRequestCrossed.to_string());
            }
            lineage::verify_frozen(admitted, &bytes).map_err(|refusal| refusal.to_string())?;
            LaunchRequest {
                path,
                bytes,
                blake3: recorded.blake3.clone(),
            }
        }
    };
    if request.bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err("dispatched request exceeds its byte bound; nothing was started".into());
    }
    Ok(request)
}

/// One ordinary file of at most [`MAX_REQUEST_BYTES`].
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return Err("not a bounded ordinary file".into());
    }
    std::fs::read(path).map_err(|error| error.to_string())
}

/// Prepare every launch file before any owner grant is consumed: the private
/// output directory, the frozen request and, for the first launch, its state
/// root. Returns the frozen request path. Each step is idempotent. A frozen
/// request is written to a temporary file, synced and renamed onto its name,
/// so a crash never leaves a partial file under it; one found whose bytes do
/// not hash to its name (left by an earlier, non-atomic writer) is replaced
/// the same way, and any other conflicting file is refused. Nothing here
/// binds the launch: only the transport intent records it.
fn prepare_launch_files(
    profile: &WorkerTransportProfile,
    request: &LaunchRequest,
    first_launch: bool,
) -> Result<PathBuf, String> {
    let output_dir = &profile.output_directory;
    if !output_dir.is_absolute() || !profile.state_directory.is_absolute() {
        return Err("worker state/output paths must be absolute".into());
    }
    let mut directories = std::fs::DirBuilder::new();
    directories.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directories.mode(0o700);
    }
    directories
        .create(output_dir)
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(output_dir)
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("worker transport observations require a private output directory".into());
        }
    }
    freeze_request(request)?;
    let frozen_request = request.path.clone();
    if first_launch {
        // The root exists before the transport intent records it, so recovery
        // after a loss before Lattice started still runs in it, never in a
        // fresh root.
        directories
            .create(&profile.state_directory)
            .map_err(|error| error.to_string())?;
    }
    Ok(frozen_request)
}

/// The launch a fresh run records with its transport intent.
fn first_launch(profile: &WorkerTransportProfile) -> Result<TransportLaunch, String> {
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| "Lattice program and state paths must be UTF-8".to_string())
    };
    Ok(TransportLaunch {
        lattice_program: text(&profile.lattice_program)?,
        executable_blake3: profile.executable_blake3.clone(),
        state_directory: text(&profile.state_directory)?,
    })
}

/// Freeze `request` at its path; see [`prepare_launch_files`].
fn freeze_request(request: &LaunchRequest) -> Result<(), String> {
    match std::fs::symlink_metadata(&request.path) {
        Ok(metadata) if !metadata.is_file() => {
            return Err("frozen request path is not an ordinary file".into());
        }
        Ok(_) => {
            let existing = read_bounded(&request.path)?;
            if existing == request.bytes {
                return Ok(());
            }
            // The name is the BLAKE3 of the bytes it must hold: other bytes
            // that still hash to it are a conflict, bytes that do not are a
            // torn write, which no launch can have recorded.
            if blake3_identity(&existing)? == request.blake3 {
                return Err("frozen request conflict".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    write_atomically(&request.path, &request.bytes)
}

/// Write `bytes` to a temporary file beside `path` (mode 0400), sync it,
/// rename it onto `path` and sync the directory.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let directory = path
        .parent()
        .ok_or("frozen request path has no directory")?;
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or("frozen request name must be UTF-8")?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = directory.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o400);
    }
    let written = options
        .open(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&temporary, path))
        .and_then(|()| std::fs::File::open(directory).and_then(|directory| directory.sync_all()));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.map_err(|error| error.to_string())
}

/// Exact files for the guarded existing implementation-worker consumer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerInvocation {
    pub database: PathBuf,
    pub enrollment: PathBuf,
    pub dispatch_plan: PathBuf,
    pub request: PathBuf,
    pub transport_profile: PathBuf,
    pub admission_observations: Option<PathBuf>,
}

/// Call the same guarded worker path as `kiln-lattice-dispatch` without
/// converting its stdout into completion evidence.
pub fn run_invocation(invocation: &WorkerInvocation) -> Result<String, String> {
    let mut flags = BTreeMap::from([
        (
            "--database".into(),
            invocation.database.display().to_string(),
        ),
        (
            "--enrollment".into(),
            invocation.enrollment.display().to_string(),
        ),
        (
            "--dispatch-plan".into(),
            invocation.dispatch_plan.display().to_string(),
        ),
        (
            "--worker-request".into(),
            invocation.request.display().to_string(),
        ),
        (
            "--worker-transport-profile".into(),
            invocation.transport_profile.display().to_string(),
        ),
    ]);
    if let Some(path) = &invocation.admission_observations {
        flags.insert(
            "--admission-observations".into(),
            path.display().to_string(),
        );
    }
    run(&flags)
}

// r[impl kiln.implementation.authority.selection.scenario.scoped]
/// Holder roles the Lattice worker and `kiln-lattice-dispatch` load. They
/// reserve, admit transport for and record the run of one dispatched attempt:
/// coordinator operations only. The verifier and publisher seeds are never
/// opened.
#[must_use]
pub fn worker_holder_roles()
-> std::collections::BTreeSet<kiln_core::implementation::grant::GrantRole> {
    std::collections::BTreeSet::from([kiln_core::implementation::grant::GrantRole::Coding])
}

// r[impl kiln.implementation.current_authority] r[impl kiln.implementation.unknown_barrier]
// r[impl kiln.implementation.lattice_cli]
pub fn run(flags: &BTreeMap<String, String>) -> Result<String, String> {
    run_with(flags, &|name| std::env::var_os(name))
}

/// [`run`] with the credential source injected: `read_credential` returns
/// the value of the one credential variable the admitted request names.
// r[impl kiln.implementation.authority.dispatch]
fn run_with(
    flags: &BTreeMap<String, String>,
    read_credential: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<String, String> {
    let request_path = required(flags, "--worker-request")?;
    let request: serde_json::Value = read_json(&request_path)?;
    let request_bytes = std::fs::read(&request_path).map_err(|error| error.to_string())?;
    let request_id = ContentId::from_blake3_hex(blake3::hash(&request_bytes).to_hex().to_string())
        .map_err(|error| format!("{error:?}"))?;
    let plan: ImplementationDispatchPlan = read_json(&required(flags, "--dispatch-plan")?)?;
    let profile: WorkerTransportProfile =
        read_json(&required(flags, "--worker-transport-profile")?)?;
    let enrollment: Enrollment = read_json(&required(flags, "--enrollment")?)?;
    // The admitted request alone names the one variable that may reach Lattice.
    let credential =
        settlement::credential_variable(&request).map_err(|error| error.to_string())?;
    // One resolved selection: the profile this run selects and the projection
    // authority its route check reads come from the same configuration.
    let selection = kiln_implementer::authority_config::ImplementationAuthorityConfig::resolve(
        flags.get("--local-operator").map(String::as_str),
        flags.get("--authority-config").map(Path::new),
    )?;
    let mut implementer = Implementer::open(&required(flags, "--database")?, enrollment)?;
    implementer.select_authority(&selection, &worker_holder_roles())?;
    let attempt = implementer.authorize_current(&plan.attempt_identity, plan.fence, clock()?)?;
    // The exact Dispatch request the plan reservation below makes is admitted
    // first, changing nothing, so an authority denial reads no credential.
    // The reservation revalidates this session in its own transaction.
    implementer.authorize_dispatch_plan(&attempt.identity, attempt.fence, clock()?, &plan)?;
    // A pinned projected workspace runs only under a profile that enables
    // projection and only with its exact route. The route is checked after
    // the authority pre-check and before the credential read, so a refused
    // route reads no credential. Only a selected verified profile with
    // `projection_enabled` enables projection; the local-operator profile
    // never does.
    let projection_authority =
        crate::projected_workspace::ProjectionAuthority::of_selected(&selection);
    crate::projected_workspace::check_route(
        flags
            .get(crate::projected_workspace::ROUTE_FLAG)
            .map(Path::new),
        &plan,
        &request,
        projection_authority,
    )?;
    // Before any durable step (plan reservation, request seal, admission or
    // transport intent), a fresh run needs its one credential variable.
    let recovery = implementer.dispatch_requires_recovery(&attempt.identity)?;
    let mut environment = if recovery {
        Vec::new()
    } else {
        settlement::credential_environment(credential, credential.and_then(read_credential))
            .map_err(|error| error.to_string())?
    };
    if program_identity(&profile.lattice_program)? != profile.executable_blake3 {
        return Err("unqualified or changed Lattice implementation executable".into());
    }
    let execution = plan
        .execution
        .as_ref()
        .ok_or("candidate worker requires explicit bound execution, not legacy configured route")?;
    if !transport_profile_mode_matches(&profile.schema, execution.native_request_profile.as_ref()) {
        return Err(
            "transport profile generation does not match the native request profile binding".into(),
        );
    }
    let envelope = binding(&request, "envelope")?;
    let text_matches = |key: &str, expected: &str| {
        envelope.get(key).and_then(serde_json::Value::as_str) == Some(expected)
    };
    if !text_matches("work_id", plan.work_identity.as_str())
        || envelope
            .get("work_version")
            .and_then(serde_json::Value::as_u64)
            != Some(plan.work_revision)
        || !text_matches("attempt_id", plan.attempt_identity.as_str())
        || envelope.get("fence").and_then(serde_json::Value::as_u64) != Some(plan.fence)
        || !text_matches("repository", &plan.repository)
        || !text_matches(
            "base",
            plan.base
                .as_ref()
                .ok_or("exact base required")?
                .native_id
                .as_str(),
        )
        || !text_matches("workspace_id", plan.workspace.identity.as_str())
        || !text_matches(
            "policy_digest",
            execution.requirements.policy_identity.as_str(),
        )
        || !text_matches(
            "worker_profile",
            execution
                .native_request_profile
                .as_ref()
                .map(kiln_core::types::ContentId::as_str)
                .unwrap_or(execution.requirements.needs.profile_revision.as_str()),
        )
        // Lattice request v3 dropped `network` (Lattice b46deb36).
        || crate::request_network::request_network(&request) != Some(plan.workspace.network)
    {
        return Err(
            "worker request is crossed with admitted work, attempt, workspace or model policy"
                .into(),
        );
    }
    let ledger = implementer.work()?;
    let work = ledger
        .current()
        .find(|work| work.identity().is_ok_and(|id| id == plan.work_identity))
        .ok_or("work version is not current")?;
    // The task/spec/verification identity is the admitted work, not a worker field.
    if !text_matches("verification_profile", work.verification_profile.as_str()) {
        return Err("verification profile changed".into());
    }
    let acceptance = kiln_core::implementation::acceptance_digest(work)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if !text_matches("task_spec_digest", acceptance.as_str())
        || !work.aliases.iter().any(|source| {
            let kind = match source.kind {
                kiln_core::implementation::WorkSourceKind::GitHubIssue => "github_issue",
                kiln_core::implementation::WorkSourceKind::CairnTask => "cairn_task",
                kiln_core::implementation::WorkSourceKind::CairnArtifact => "cairn_artifact",
            };
            text_matches("source_kind", kind)
                && text_matches("source_id", &source.native_id)
                && text_matches("readiness_snapshot", source.version.as_str())
        })
    {
        return Err("worker source/spec association differs from admitted work".into());
    }
    if request
        .get("allow_unchanged")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        && work.terminal_policy
            != kiln_core::implementation::TerminalPolicy::VerifiedOrAlreadySatisfied
    {
        return Err("work does not admit an unchanged proposal".into());
    }
    if flags
        .get("--recover")
        .is_some_and(|value| matches!(value.as_str(), "true" | "1"))
        && !implementer.dispatch_requires_recovery(&attempt.identity)?
    {
        return Err(
            "recovery requires prior persisted transport intent; no execution was started".into(),
        );
    }
    // The exact request bytes must bind the plan Kiln reserves below.
    let plan_identity = kiln_core::implementation::admit_dispatch_plan(work, &attempt, &plan)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if !settlement::request_binds_plan(&request, plan_identity.as_str()) {
        return Err(
            "worker request does not bind the reserved dispatch plan; nothing was dispatched"
                .into(),
        );
    }
    let reservation = implementer.reserve_dispatch_plan(
        work,
        &attempt.identity,
        attempt.fence,
        clock()?,
        &plan,
    )?;
    let request_binding = implementer.bind_campaign_request(
        work,
        &attempt.identity,
        attempt.fence,
        clock()?,
        &request_bytes,
    )?;
    if request_binding.plan != reservation.plan
        || reservation.plan != plan_identity
        || request_binding.request_digest != request_id
    {
        return Err(
            "retained request seal differs from the exact worker bytes and dispatch plan".into(),
        );
    }
    let admission = implementer.admission_snapshot()?;
    if admission.policy.is_none() {
        return Err(
            "bound worker execution requires an explicit owner-grant admission policy".into(),
        );
    }
    let fresh: Vec<kiln_core::fair_admission::EligibilityInput> = if recovery {
        Vec::new()
    } else {
        let item = admission
            .items
            .get(&plan.work_identity)
            .ok_or("queued work is missing")?;
        let expires = request
            .get("expires_at_ms")
            .and_then(serde_json::Value::as_u64)
            .ok_or("worker admission lacks deadline")?;
        let runtime_ms = u64::from(plan.deadline_minutes) * 60 * 1000;
        if expires > item.deadline_ms
            || expires > attempt.lease_until_ms
            || clock()?
                .checked_add(runtime_ms)
                .is_none_or(|end| end > item.deadline_ms)
            || envelope
                .pointer("/budget/max_seconds")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|seconds| seconds > runtime_ms / 1000)
            || envelope
                .pointer("/budget/max_turns")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|turns| turns > u64::from(plan.invocation_budget))
        {
            return Err(
                "worker runtime/invocations exceed parent queue, lease or dispatch budget".into(),
            );
        }
        match flags.get("--admission-observations") {
            Some(path) => read_json(Path::new(path))?,
            None => Vec::new(),
        }
    };
    // The transport intent records the launch, with the exact request it
    // sends, in the same ledger mutation; an existing intent returns the
    // launch that won it, never this profile's.
    let (fresh_request, start) = if recovery {
        let start = implementer.begin_dispatch_transport(
            &attempt.identity,
            attempt.fence,
            clock()?,
            &reservation.plan,
            None,
        )?;
        (None, start)
    } else {
        // A first launch derives the budget lineage from this ledger snapshot
        // and freezes the bytes it sends under their own BLAKE3, so a retry
        // whose lineage changed before any transport start writes its own
        // file. Every fallible filesystem step (output directory, frozen
        // request and state root) precedes the owner-grant gate, which moves
        // committed owner reservations to unknown: a failure here leaves the
        // grants committed, so a retry after fixing the cause proceeds.
        let claim = ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        };
        let source = RequestSource::Fresh {
            ledger: &admission,
            work: &plan.work_identity,
            claim: &claim,
            now_ms: clock()?,
        };
        let request = launch_request(source, &profile, &request_bytes)?;
        let launch = first_launch(&profile)?;
        let sent = request.frozen()?;
        let path = prepare_launch_files(&profile, &request, true)?;
        // Open item (predates this change): a crash between this gate and
        // `begin_dispatch_transport` leaves unknown grants with no transport
        // intent; neither a fresh run nor recovery can then proceed.
        implementer.admission_before_transport(
            &attempt.identity,
            attempt.fence,
            &fresh,
            clock()?,
        )?;
        let start = implementer.begin_dispatch_transport_sending(
            &attempt.identity,
            attempt.fence,
            clock()?,
            &reservation.plan,
            Some(&launch),
            Some(&sent),
        )?;
        (Some(path), start)
    };
    let output_dir = &profile.output_directory;
    let recovery = start.is_recovery();
    let (lattice_program, state_directory, frozen_request) = match (start, fresh_request) {
        (TransportStart::Fresh, Some(path)) => (
            profile.lattice_program.clone(),
            profile.state_directory.clone(),
            path,
        ),
        (TransportStart::Fresh, None) => {
            return Err(format!(
                "the ledger started a transport the caller expected to recover; {}",
                lineage::RECOVERY_NOT_INVOKED
            ));
        }
        (TransportStart::Recover(recorded), _) => {
            // Recovery never starts a worker, so it never receives the credential.
            environment.clear();
            let state_is_directory = recorded
                .as_ref()
                .is_some_and(|launch| binding::is_directory(Path::new(&launch.state_directory)));
            let program_matches = recorded.as_ref().is_some_and(|launch| {
                program_identity(Path::new(&launch.lattice_program))
                    .is_ok_and(|identity| identity == launch.executable_blake3)
            });
            let recorded = binding::recovery_launch(recorded, state_is_directory, program_matches)?;
            // The exact bytes the first launch sent, from the file recorded
            // for it, never from the current profile's output directory.
            let sent = implementer.dispatch_request(&attempt.identity)?;
            let request = launch_request(
                RequestSource::Recover {
                    request: sent.as_ref(),
                },
                &profile,
                &request_bytes,
            )?;
            let path = prepare_launch_files(&profile, &request, false)?;
            (
                PathBuf::from(recorded.lattice_program),
                PathBuf::from(recorded.state_directory),
                path,
            )
        }
    };
    let launch_kind = if recovery {
        settlement::Launch::Recover
    } else {
        settlement::Launch::Run
    };
    let command = if recovery { "recover" } else { "run" };
    let args: Vec<std::ffi::OsString> = vec![
        "implementation-attempt".into(),
        command.into(),
        "--request".into(),
        frozen_request.as_os_str().into(),
        "--state".into(),
        state_directory.as_os_str().into(),
    ];
    let launch = RunRequest {
        command: lattice_command(
            lattice_program.clone(),
            args,
            output_dir.clone(),
            environment,
        ),
        limits: ExecutionLimits {
            timeout_ms: u64::from(plan.deadline_minutes) * 60 * 1000 + 10_000,
            stdin_max_bytes: 1,
            stdout_max_bytes: 4 * 1024 * 1024,
            stderr_max_bytes: 4 * 1024 * 1024,
            poll_interval_ms: 10,
            teardown_timeout_ms: 5_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|error| format!("{error:?}"))?,
    };
    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handlers = Vec::new();
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        handlers.push(
            signal_hook::flag::register(signal, cancellation.clone())
                .map_err(|error| error.to_string())?,
        );
    }
    let cancel_flag = cancellation.clone();
    let done = finished.clone();
    let cancel_program = lattice_program.clone();
    let cancel_state = state_directory.clone();
    let cancel_root = output_dir.clone();
    let cancel_request = frozen_request.clone();
    let watcher = std::thread::spawn(move || {
        while !done.load(std::sync::atomic::Ordering::SeqCst) {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                let cancel_args: Vec<std::ffi::OsString> = vec![
                    "implementation-attempt".into(),
                    "cancel".into(),
                    "--request".into(),
                    cancel_request.as_os_str().into(),
                    "--state".into(),
                    cancel_state.as_os_str().into(),
                ];
                if let Ok(outcome_policy) = OutcomePolicy::new(vec![0], true, true) {
                    let result = bounded_exec::run(RunRequest {
                        // Cancellation starts no worker and never receives the credential.
                        command: lattice_command(
                            cancel_program.clone(),
                            cancel_args,
                            cancel_root.clone(),
                            Vec::new(),
                        ),
                        limits: ExecutionLimits {
                            timeout_ms: 5_000,
                            stdin_max_bytes: 1,
                            stdout_max_bytes: 64 * 1024,
                            stderr_max_bytes: 64 * 1024,
                            poll_interval_ms: 10,
                            teardown_timeout_ms: 5_000,
                        },
                        termination_scope: TerminationScope::ProcessGroup,
                        outcome_policy,
                    });
                    if result.is_ok_and(|output| output.disposition == Disposition::Succeeded) {
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    let outcome = bounded_exec::run(launch);
    finished.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = watcher.join();
    for handler in handlers {
        signal_hook::low_level::unregister(handler);
    }
    let outcome = outcome.map_err(|_| {
        "Lattice outcome unknown after possible dispatch; retain state and recover".to_string()
    })?;
    // Raw transport observations remain separate from accepted candidate evidence.
    let prefix = format!(
        "{}-{}-{}",
        attempt.identity.as_str(),
        attempt.fence,
        if recovery { "recovery" } else { "dispatch" }
    );
    for (suffix, bytes) in [
        ("stdout", outcome.stdout.bytes.as_slice()),
        ("stderr", outcome.stderr.bytes.as_slice()),
    ] {
        use std::io::Write;
        let path = output_dir.join(format!(
            "{prefix}-{}.{}",
            blake3::hash(bytes).to_hex(),
            suffix
        ));
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&path).map_err(|error| error.to_string())? != bytes {
                    return Err("retained transport observation conflicts".into());
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    if outcome.disposition == Disposition::ExitFailed {
        // Only Lattice can witness that no worker effect of this attempt began,
        // and only the first `run` speaks for the attempt's only launch.
        if let settlement::RefusalOutcome::BeforeEffect { reason } =
            settlement::classify_refusal(envelope, &outcome.stdout.bytes, launch_kind)
        {
            implementer.settle_dispatch_without_execution(
                &attempt.identity,
                attempt.fence,
                clock()?,
                &format!("lattice_refused_before_effect:{reason}"),
            )?;
            return Err(format!(
                "Lattice refused the attempt before any effect ({reason}); the attempt is settled without execution"
            ));
        }
    }
    if outcome.disposition != Disposition::Succeeded {
        return Err("worker has no complete observed result; retain state and reconcile".into());
    }
    let observed: serde_json::Value = serde_json::from_slice(&outcome.stdout.bytes)
        .map_err(|_| "worker response malformed after possible dispatch")?;
    if settlement::classify_observation(&request, &outcome.stdout.bytes, reservation.plan.as_str())?
        == settlement::Observed::ClosedBeforeExecution
    {
        if launch_kind != settlement::Launch::Recover {
            return Err(
                "closure reported outside bound recovery; retain state and reconcile".into(),
            );
        }
        // Recovery in the bound root exclusively closed the attempt: it never executes.
        implementer.settle_dispatch_without_execution(
            &attempt.identity,
            attempt.fence,
            clock()?,
            "lattice_closed_before_execution",
        )?;
        return serde_json::to_string(&observed).map_err(|error| error.to_string());
    }
    let workflow = ExternalRef::new("lattice", "implementation-attempt", request_id.as_str())
        .map_err(|error| format!("{error:?}"))?;
    implementer.reconcile_dispatch(&attempt.identity, attempt.fence, clock()?, &workflow)?;
    implementer.admission_execution_observed(&attempt.identity, attempt.fence, clock()?)?;
    // Observation failure cannot turn a valid native result into a failed
    // dispatch or grant candidate authority. Raw transport evidence is retained
    // above; absent/malformed telemetry is explicit in this diagnostic.
    let metrics_result = read_metrics_artifact(&state_directory, &attempt, &observed)
        .and_then(|bytes| metrics::normalize(&request, &observed, &bytes))
        .and_then(|observation| {
            implementer.record_worker_observation(&attempt.identity, attempt.fence, &observation)
        });
    if let Err(error) = metrics_result {
        eprintln!("kiln worker metrics unavailable: {error}");
    }
    if cancellation.load(std::sync::atomic::Ordering::SeqCst)
        || observed
            .pointer("/result/disposition")
            .and_then(serde_json::Value::as_str)
            == Some("cancelled")
    {
        implementer.cancel(
            &attempt.identity,
            attempt.fence,
            clock()?,
            "provider cancellation observed; retained usage is not refunded",
        )?;
    }
    serde_json::to_string(&observed).map_err(|error| error.to_string())
}

#[cfg(test)]
mod transport_profile_tests {
    use super::transport_profile_mode_matches;
    use kiln_core::types::ContentId;

    #[test]
    fn profile_generation_and_native_binding_must_be_paired() {
        let native = ContentId::from_blake3_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v1",
            None
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v1",
            Some(&native)
        ));
        assert!(transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v2",
            Some(&native)
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v2",
            None
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v3",
            Some(&native)
        ));
    }
}

#[cfg(test)]
mod launch_environment_tests {
    use super::{lattice_command, settlement};
    use bounded_exec::{Disposition, ExecutionLimits, OutcomePolicy, RunRequest, TerminationScope};
    use std::ffi::OsString;

    /// What a real child launched like the Lattice process can read.
    fn observe(environment: Vec<(OsString, OsString)>) -> String {
        let probe = "printf '%s|%s|%s' \"${KILN_TEST_MODEL_TOKEN-unset}\" \"${HOME-unset}\" \"${CARGO_MANIFEST_DIR-unset}\"";
        let outcome = bounded_exec::run(RunRequest {
            command: lattice_command(
                "/bin/sh".into(),
                vec!["-c".into(), probe.into()],
                std::env::temp_dir(),
                environment,
            ),
            limits: ExecutionLimits {
                timeout_ms: 10_000,
                stdin_max_bytes: 1,
                stdout_max_bytes: 4096,
                stderr_max_bytes: 4096,
                poll_interval_ms: 10,
                teardown_timeout_ms: 5_000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true)
                .unwrap_or_else(|_| unreachable!()),
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(outcome.disposition, Disposition::Succeeded);
        String::from_utf8(outcome.stdout.bytes).unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.implementation.lattice_cli.scenario.credential]
    #[test]
    fn only_the_named_credential_reaches_the_lattice_process() {
        let request = serde_json::json!({"omp": {"model_broker": {
            "credential": {"provider": "env", "key": "KILN_TEST_MODEL_TOKEN"}
        }}});
        let name = settlement::credential_variable(&request).unwrap_or_else(|_| unreachable!());
        let environment =
            settlement::credential_environment(name, Some(OsString::from("fake-value")))
                .unwrap_or_else(|_| unreachable!());
        // The named value is delivered; ambient HOME and Cargo variables are withheld.
        assert_eq!(observe(environment), "fake-value|unset|unset");
        // No declared credential keeps the fully cleared environment.
        let undeclared = serde_json::json!({"omp": {"model_broker": {"credential": null}}});
        let none = settlement::credential_variable(&undeclared).unwrap_or_else(|_| unreachable!());
        let cleared = settlement::credential_environment(none, Some(OsString::from("fake-value")))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(observe(cleared), "unset|unset|unset");
    }
}

#[cfg(test)]
mod credential_ordering_tests {
    use super::run_with;
    use kiln_core::implementation::{
        AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace,
        Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationDispatchPlan, ImplementationWork,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
    };
    use kiln_core::projection::materialization::{
        WORKSPACE_PIN_SCHEMA, WorkspacePin, overlay_identity,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_implementer::Implementer;
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::path::Path;

    const LEASE_MS: u64 = 600_000;
    /// The credential variable the admitted request names.
    const VARIABLE: &str = "KILN_TEST_LATTICE_CREDENTIAL_45";
    /// The local-operator selection a campaign drive passes its worker.
    const OPERATOR: Option<(&str, &str)> = Some(("--local-operator", "test-operator"));

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn enrollment() -> Enrollment {
        Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        }
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: Some(
                ExternalRef::new("git", "commit", &"a".repeat(40))
                    .unwrap_or_else(|_| unreachable!()),
            ),
            title: "Credential ordering".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#45")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#45".to_string(),
                version: content('1'),
            }],
        }
    }

    fn write(path: &Path, value: &impl serde::Serialize) {
        std::fs::write(
            path,
            serde_json::to_vec(value).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
    }

    /// Run the transport for one claimed attempt under the `authority`
    /// selection flag, or under the absent profile when it is `None`, reading
    /// the named credential only through `read_credential`. Reports the result
    /// and whether any dispatch plan was reserved.
    fn dispatch(
        credential: serde_json::Value,
        authority: Option<(&str, &str)>,
        read_credential: &dyn Fn(&str) -> Option<OsString>,
    ) -> (Result<String, String>, bool) {
        dispatch_with(credential, authority, false, read_credential)
    }

    /// [`dispatch`], with a plan that pins a projected workspace for the
    /// claimed attempt when `pinned`. No route is supplied.
    fn dispatch_with(
        credential: serde_json::Value,
        authority: Option<(&str, &str)>,
        pinned: bool,
        read_credential: &dyn Fn(&str) -> Option<OsString>,
    ) -> (Result<String, String>, bool) {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let root = directory.path();
        let database = root.join("kiln.sqlite");
        let now = super::clock().unwrap_or_else(|_| unreachable!());
        let work = work();
        let attempt = {
            let mut owner =
                Implementer::open_with_local_operator(&database, enrollment(), "test-operator")
                    .unwrap_or_else(|error| unreachable!("{error}"));
            owner
                .admit(&work, None)
                .unwrap_or_else(|error| unreachable!("{error}"));
            owner
                .claim_next(now, LEASE_MS)
                .unwrap_or_else(|error| unreachable!("{error}"))
                .unwrap_or_else(|| unreachable!())
        };
        let mut plan = ImplementationDispatchPlan {
            schema: DISPATCH_PLAN_SCHEMA.into(),
            work_identity: attempt.work_identity.clone(),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            workspace: DispatchWorkspace {
                identity: content('e'),
                isolation: WorkspaceIsolation::IsolatedSnapshot,
                network: false,
                credentials: false,
                projection: None,
            },
            provider_profile: ProviderProfileId::new("lattice-implementation")
                .unwrap_or_else(|_| unreachable!()),
            permitted_paths: work.permitted_paths.clone(),
            permitted_effects: work.permitted_effects.clone(),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 1,
            token_budget: 1000,
            invocation_budget: 1,
            expected_output: content('f'),
            execution: None,
        };
        if pinned {
            let input = content('5');
            let pin = WorkspacePin {
                schema: WORKSPACE_PIN_SCHEMA.into(),
                repository: content('1'),
                source: work.base.clone().unwrap_or_else(|| unreachable!()),
                definition: content('2'),
                manifest: content('3'),
                profile: content('4'),
                overlay: overlay_identity(&input, &attempt.identity, attempt.fence)
                    .unwrap_or_else(|_| unreachable!()),
                input,
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
            };
            plan.workspace.identity = pin.overlay.clone();
            plan.workspace.projection = Some(pin);
        }
        write(&root.join("plan.json"), &plan);
        write(&root.join("enrollment.json"), &enrollment());
        write(
            &root.join("request.json"),
            &serde_json::json!({"envelope": {}, "omp": {"model_broker": {"credential": credential}}}),
        );
        write(
            &root.join("transport.json"),
            &serde_json::json!({
                "schema": "kiln.lattice-implementation-cli.v2",
                "lattice_program": root.join("not-a-store-program"),
                "executable_blake3": "b".repeat(64),
                "state_directory": root.join("state"),
                "output_directory": root.join("transport"),
            }),
        );
        let mut flags = BTreeMap::from([
            ("--database".to_string(), database.display().to_string()),
            (
                "--enrollment".to_string(),
                root.join("enrollment.json").display().to_string(),
            ),
            (
                "--dispatch-plan".to_string(),
                root.join("plan.json").display().to_string(),
            ),
            (
                "--worker-request".to_string(),
                root.join("request.json").display().to_string(),
            ),
            (
                "--worker-transport-profile".to_string(),
                root.join("transport.json").display().to_string(),
            ),
        ]);
        if let Some((flag, value)) = authority {
            flags.insert(flag.to_string(), value.to_string());
        }
        let result = run_with(&flags, read_credential);
        let reserved = Implementer::open(&database, enrollment())
            .and_then(|mut owner| owner.dispatch_reservation(&attempt.identity))
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some();
        (result, reserved)
    }

    /// A credential source that counts its reads and returns `value`.
    fn counted(reads: &Cell<usize>, value: Option<&str>) -> impl Fn(&str) -> Option<OsString> {
        let value = value.map(OsString::from);
        move |name| {
            assert_eq!(name, VARIABLE, "only the named variable is read");
            reads.set(reads.get() + 1);
            value.clone()
        }
    }

    fn declared() -> serde_json::Value {
        serde_json::json!({"provider": "env", "key": VARIABLE})
    }

    /// Write under `dir` an exported verified-UCAN configuration whose
    /// root-issued holder grants and revocation feed are valid now, with
    /// `projection_enabled` as given. Returns the configuration path.
    fn verified_config(dir: &Path, projection_enabled: bool) -> String {
        use kiln_adapter_basalt::test_support::{self, Fixture};
        use kiln_core::implementation::grant::GrantRole;
        use std::os::unix::fs::PermissionsExt;
        let hex = |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
        let now_ms = super::clock().unwrap_or_else(|_| unreachable!());
        let fixture = Fixture::direct_from(test_support::ROOT_SEED, now_ms / 1_000 + 3_600);
        let proofs = dir.join("proofs");
        std::fs::create_dir_all(&proofs).unwrap_or_else(|_| unreachable!());
        for (index, (_, token)) in fixture.grants.iter().enumerate() {
            std::fs::write(proofs.join(format!("{index}.ucan")), token)
                .unwrap_or_else(|_| unreachable!());
        }
        let mut holders = serde_json::Map::new();
        for (name, role) in [
            ("coordinator", GrantRole::Coding),
            ("verifier", GrantRole::Verification),
            ("publisher", GrantRole::Publication),
        ] {
            let path = dir.join(format!("{name}.seed"));
            std::fs::write(&path, test_support::seed(test_support::role_seed(role)))
                .unwrap_or_else(|_| unreachable!());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .unwrap_or_else(|_| unreachable!());
            holders.insert(name.into(), serde_json::json!({ "seed_path": path }));
        }
        let feed = dir.join("revocations.json");
        // The feed's producer observed it this second.
        write(
            &feed,
            &serde_json::json!({
                "schema": "kiln.implementation-authority-revocations.v1",
                "generation": 1,
                "observed_at_ms": now_ms / 1_000 * 1_000,
                "revoked": [],
            }),
        );
        let keys: Vec<_> = [
            test_support::ROOT_SEED,
            test_support::COORDINATOR_SEED,
            test_support::VERIFIER_SEED,
            test_support::PUBLISHER_SEED,
        ]
        .into_iter()
        .map(|seed| {
            serde_json::json!({
                "did": test_support::did(seed),
                "ed25519": hex(&test_support::public_key(seed)),
            })
        })
        .collect();
        let path = dir.join("authority.json");
        write(
            &path,
            &serde_json::json!({
                "schema": "kiln.implementation-authority-config.v1",
                "profile": { "verified_ucan": {
                    "audience": test_support::AUDIENCE,
                    "generation": test_support::GENERATION,
                    "policy": "packaged",
                    "trusted_roots": fixture.roots,
                    "keys": keys,
                    "proofs_dir": proofs,
                    "revocation": { "path": feed, "max_age_ms": 600_000 },
                    "replay": { "retention_ms": 60_000 },
                    "holders": holders,
                    "projection_enabled": projection_enabled,
                } },
            }),
        );
        path.display().to_string()
    }

    // r[verify kiln.implementation.lattice_cli.scenario.credential]
    #[test]
    fn a_refused_credential_is_rejected_before_any_plan_reservation() {
        let reads = Cell::new(0);
        let (missing, reserved) = dispatch(declared(), OPERATOR, &counted(&reads, None));
        assert!(
            missing
                .as_ref()
                .is_err_and(|error| error.starts_with("worker_credential_missing:")),
            "{missing:?}"
        );
        assert_eq!(reads.get(), 1, "the admitted run reads its one credential");
        assert!(!reserved);
        let reads = Cell::new(0);
        let (reserved_name, reserved) = dispatch(
            serde_json::json!({"provider": "env", "key": "PATH"}),
            OPERATOR,
            &counted(&reads, Some("value")),
        );
        assert!(
            reserved_name
                .as_ref()
                .is_err_and(|error| error.starts_with("worker_credential_reserved_name:")),
            "{reserved_name:?}"
        );
        assert_eq!(reads.get(), 0);
        assert!(!reserved);
        // Without a declared credential the credential stage passes, and the
        // next check (the immutable executable) refuses, still before any
        // reservation.
        let reads = Cell::new(0);
        let (undeclared, reserved) = dispatch(
            serde_json::Value::Null,
            OPERATOR,
            &counted(&reads, Some("value")),
        );
        assert!(
            undeclared
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{undeclared:?}"
        );
        assert_eq!(reads.get(), 0);
        assert!(!reserved);
    }

    // r[verify kiln.implementation.authority.dispatch.scenario.credential]
    #[test]
    fn an_authority_denial_precedes_the_credential_read() {
        // The absent profile denies the plan's Dispatch pre-check: no
        // credential is read and nothing is reserved.
        let reads = Cell::new(0);
        let (denied, reserved) = dispatch(declared(), None, &counted(&reads, Some("token")));
        assert!(
            denied
                .as_ref()
                .is_err_and(|error| error.starts_with("authority:")),
            "{denied:?}"
        );
        assert_eq!(reads.get(), 0, "a denial never reads the credential");
        assert!(!reserved);
        // Admitted, the same run reads its credential once, after the
        // pre-check, and the executable check still refuses before any
        // reservation.
        let reads = Cell::new(0);
        let (admitted, reserved) = dispatch(declared(), OPERATOR, &counted(&reads, Some("token")));
        assert!(
            admitted
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{admitted:?}"
        );
        assert_eq!(reads.get(), 1);
        assert!(!reserved);
    }

    // r[verify kiln.source.projection.route.scenario.dispatch]
    #[test]
    fn a_refused_projected_route_precedes_the_credential_read() {
        // The pre-check admits the pinned plan under the local operator; the
        // route check then refuses it (the profile never enables projection
        // and no route is supplied) before the credential read and any
        // reservation.
        let reads = Cell::new(0);
        let (refused, reserved) =
            dispatch_with(declared(), OPERATOR, true, &counted(&reads, Some("token")));
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.starts_with("projection-requires-operator:")),
            "{refused:?}"
        );
        assert_eq!(reads.get(), 0, "a refused route never reads the credential");
        assert!(!reserved);
        // The same plan without its pin passes the route check and reads its
        // credential once.
        let reads = Cell::new(0);
        let (unpinned, reserved) =
            dispatch_with(declared(), OPERATOR, false, &counted(&reads, Some("token")));
        assert!(
            unpinned
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{unpinned:?}"
        );
        assert_eq!(reads.get(), 1);
        assert!(!reserved);
    }

    // r[verify kiln.source.projection.route.scenario.dispatch]
    #[test]
    fn the_selected_profile_decides_projection_at_the_route_check() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let profile = |name: &str, projection_enabled: bool| {
            let dir = directory.path().join(name);
            std::fs::create_dir_all(&dir).unwrap_or_else(|_| unreachable!());
            verified_config(&dir, projection_enabled)
        };
        // A selected verified profile with `projection_enabled` passes the
        // pre-check and the route check's profile gate: the pinned plan is
        // refused only for its missing route.
        let enabled = profile("enabled", true);
        let reads = Cell::new(0);
        let (unrouted, reserved) = dispatch_with(
            declared(),
            Some(("--authority-config", &enabled)),
            true,
            &counted(&reads, Some("token")),
        );
        assert!(
            unrouted.as_ref().is_err_and(|error| error
                .starts_with("the plan pins a projected workspace and requires its route")),
            "{unrouted:?}"
        );
        assert_eq!(reads.get(), 0);
        assert!(!reserved);
        // The same verified profile without `projection_enabled`, and the
        // local operator, never enable projection.
        let disabled = profile("disabled", false);
        for authority in [Some(("--authority-config", disabled.as_str())), OPERATOR] {
            let reads = Cell::new(0);
            let (refused, reserved) =
                dispatch_with(declared(), authority, true, &counted(&reads, Some("token")));
            assert!(
                refused
                    .as_ref()
                    .is_err_and(|error| error.starts_with("projection-requires-operator:")),
                "{authority:?}: {refused:?}"
            );
            assert_eq!(reads.get(), 0);
            assert!(!reserved);
        }
        // A verified profile without projection still admits an unpinned
        // plan, which reads its credential once.
        let reads = Cell::new(0);
        let (unpinned, reserved) = dispatch_with(
            declared(),
            Some(("--authority-config", &disabled)),
            false,
            &counted(&reads, Some("token")),
        );
        assert!(
            unpinned
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{unpinned:?}"
        );
        assert_eq!(reads.get(), 1);
        assert!(!reserved);
    }

    // r[verify kiln.implementation.authority.config.scenario.roles]
    #[test]
    fn the_drives_exported_authority_config_admits_the_worker() {
        // A campaign drive passes its exported configuration file unchanged;
        // the worker selects that profile, passes the Dispatch pre-check and
        // reads its credential once.
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let exported = directory.path().join("authority.json");
        write(
            &exported,
            &kiln_implementer::authority_config::ImplementationAuthorityConfig::local_operator(
                "test-operator",
            ),
        );
        let path = exported.display().to_string();
        let reads = Cell::new(0);
        let (admitted, reserved) = dispatch(
            declared(),
            Some(("--authority-config", &path)),
            &counted(&reads, Some("token")),
        );
        assert!(
            admitted
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{admitted:?}"
        );
        assert_eq!(reads.get(), 1);
        assert!(!reserved);
    }
}

#[cfg(test)]
#[path = "worker_preparation_tests.rs"]
mod preparation_tests;

#[cfg(test)]
#[path = "worker_sent_request_tests.rs"]
mod sent_request_tests;
