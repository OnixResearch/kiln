use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run, run_with_cancellation,
};
use git2::{Cred, FetchOptions, Oid, RemoteCallbacks, Repository};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::source::GitSnapshotLimits;
use kiln_core::types::{ContentId, ProviderProfileId};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

use crate::secret::{ResolvedSecret, SecretReference, admit_secret_reference, resolve_secret};
use crate::{domain, infrastructure};

pub const JOB_SCHEMA_VERSION: u32 = 1;
pub const MAXIMUM_PROFILE_BYTES: usize = 1_048_576;
const SOURCE_RESULT_BYTES: usize = 4096;
const POLL_INTERVAL_MS: u64 = 10;
const MAXIMUM_DECLARATIONS: usize = 4096;
const MAXIMUM_TEXT_BYTES: usize = 65_536;
const MAXIMUM_SOURCE_SECRET_BYTES: usize = 4096;

// r[impl kiln.github.profile] r[impl kiln.github.source]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobProfile {
    pub schema_version: u32,
    pub owner: String,
    pub repository: String,
    pub program_path: PathBuf,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub source_secret: Option<SecretReference>,
    #[serde(deserialize_with = "deserialize_snapshot_limits")]
    pub snapshot_limits: GitSnapshotLimits,
    pub maximum_transfer_bytes: usize,
    pub acquisition_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub maximum_output_bytes: usize,
    pub teardown_grace_ms: u64,
}

fn deserialize_snapshot_limits<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<GitSnapshotLimits, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Limits {
        max_entries: usize,
        max_path_bytes: usize,
        max_file_bytes: usize,
        max_total_bytes: usize,
    }
    let limits = Limits::deserialize(deserializer)?;
    Ok(GitSnapshotLimits {
        max_entries: limits.max_entries,
        max_path_bytes: limits.max_path_bytes,
        max_file_bytes: limits.max_file_bytes,
        max_total_bytes: limits.max_total_bytes,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceContext {
    pub repository: String,
    pub revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum JobTermination {
    Exited {
        code: Option<i32>,
        signal: Option<i32>,
    },
    TimedOut,
    Cancelled,
    OutputLimitExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobOutcome {
    pub revision: String,
    pub canonical_tree: ContentId,
    pub termination: JobTermination,
}

impl JobOutcome {
    #[must_use]
    pub fn successful(&self) -> bool {
        matches!(
            self.termination,
            JobTermination::Exited {
                code: Some(0),
                signal: None
            }
        )
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.".contains(&byte)
        })
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn valid_environment_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 256
        && name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index != 0 && byte.is_ascii_digit())
        })
        && !upper.starts_with("LATTICE_")
        && !upper.starts_with("GITHUB_")
        && !upper.starts_with("GIT_")
        && !upper.starts_with("SECRETSPEC_")
        && !["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"]
            .iter()
            .any(|part| upper.contains(part))
}

fn valid_text(value: &str) -> bool {
    value.len() <= MAXIMUM_TEXT_BYTES && !value.contains('\0')
}

pub fn admit_job_profile(profile: &JobProfile) -> PortResult<()> {
    let limits = &profile.snapshot_limits;
    if profile.schema_version != JOB_SCHEMA_VERSION
        || !valid_component(&profile.owner)
        || !valid_component(&profile.repository)
        || !profile.program_path.is_absolute()
        || profile
            .program_path
            .to_str()
            .is_none_or(|value| !valid_text(value))
        || profile.args.len() > MAXIMUM_DECLARATIONS
        || profile.args.iter().any(|value| !valid_text(value))
        || profile.environment.len() > MAXIMUM_DECLARATIONS
        || profile
            .environment
            .iter()
            .any(|(key, value)| !valid_environment_name(key) || !valid_text(value))
        || limits.max_entries == 0
        || limits.max_path_bytes == 0
        || limits.max_file_bytes == 0
        || limits.max_total_bytes == 0
        || profile.maximum_transfer_bytes == 0
        || profile.acquisition_timeout_ms == 0
        || profile.command_timeout_ms == 0
        || profile.maximum_output_bytes == 0
        || profile.teardown_grace_ms == 0
    {
        return Err(domain(
            "github_job_profile",
            "github.job",
            "GitHub job profile is invalid",
        ));
    }
    if let Some(reference) = &profile.source_secret {
        admit_secret_reference(reference)?;
    }
    Ok(())
}

pub fn admit_source_context(
    profile: &JobProfile,
    provider: &str,
    repository: &str,
    revision: &str,
) -> PortResult<SourceContext> {
    admit_job_profile(profile)?;
    if provider != "github"
        || repository != format!("{}/{}", profile.owner, profile.repository)
        || !valid_revision(revision)
    {
        return Err(domain(
            "github_job_source",
            "github.job.source",
            "GitHub job source context is invalid",
        ));
    }
    Ok(SourceContext {
        repository: repository.to_owned(),
        revision: revision.to_owned(),
    })
}

// The digest covers the original bounded bytes, not a JSON reserialization.
pub fn load_job_profile(path: &Path, identity: &str) -> PortResult<JobProfile> {
    if identity.len() != 64
        || !identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(domain(
            "github_job_identity",
            "github.job.identity",
            "GitHub job profile identity is invalid",
        ));
    }
    let file = File::open(path).map_err(|_| job_io())?;
    if !file.metadata().map_err(|_| job_io())?.is_file() {
        return Err(job_io());
    }
    let mut bytes = Vec::new();
    file.take((MAXIMUM_PROFILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| job_io())?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES || blake3::hash(&bytes).to_hex().as_str() != identity {
        return Err(domain(
            "github_job_identity",
            "github.job.identity",
            "GitHub job profile identity does not match",
        ));
    }
    let profile: JobProfile = serde_json::from_slice(&bytes).map_err(|_| {
        domain(
            "github_job_profile",
            "github.job",
            "GitHub job profile could not be decoded",
        )
    })?;
    admit_job_profile(&profile)?;
    Ok(profile)
}

fn job_io() -> kiln_provider_api::PortFailure {
    infrastructure(
        "github_job_io",
        "GitHub job local resource operation failed",
    )
}

fn source_failure() -> kiln_provider_api::PortFailure {
    infrastructure(
        "github_job_acquisition",
        "GitHub exact source acquisition failed",
    )
}

fn execution_limits(profile: &JobProfile, timeout_ms: u64, output_bytes: usize) -> ExecutionLimits {
    ExecutionLimits {
        timeout_ms,
        stdin_max_bytes: 1,
        stdout_max_bytes: output_bytes,
        stderr_max_bytes: output_bytes,
        poll_interval_ms: POLL_INTERVAL_MS.min(timeout_ms),
        teardown_timeout_ms: profile.teardown_grace_ms,
    }
}

fn request(command: CommandSpec, limits: ExecutionLimits) -> PortResult<RunRequest> {
    Ok(RunRequest {
        command,
        limits,
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|_| job_io())?,
    })
}

// r[impl kiln.github.source] r[impl kiln.github.secrets]
// This child is the only network owner. Its parent can enforce a hard deadline
// even while libgit2 is blocked in DNS, TLS, or a remote read.
fn acquire_source(
    profile: &JobProfile,
    revision: &str,
    root: &Path,
    fixture: Option<&Path>,
    secret: Option<ResolvedSecret>,
) -> PortResult<ContentId> {
    let url = format!(
        "https://github.com/{}/{}.git",
        profile.owner, profile.repository
    );
    let mut init = git2::RepositoryInitOptions::new();
    init.bare(true).external_template(false);
    let repository =
        Repository::init_opts(root.join("objects"), &init).map_err(|_| source_failure())?;
    // A newly created repository needs no external configuration. Fail closed
    // rather than honor system URL rewrites, helpers, proxies or TLS overrides.
    let config = repository.config().map_err(|_| source_failure())?;
    let mut entries = config.entries(None).map_err(|_| source_failure())?;
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|_| source_failure())?;
        if entry.level() != git2::ConfigLevel::Local
            || !matches!(
                entry.name(),
                Some(
                    "core.repositoryformatversion"
                        | "core.filemode"
                        | "core.bare"
                        | "core.logallrefupdates"
                )
            )
        {
            return Err(source_failure());
        }
    }
    drop(entries);
    drop(config);
    let mut callbacks = RemoteCallbacks::new();
    callbacks
        .transfer_progress(|progress| progress.received_bytes() <= profile.maximum_transfer_bytes);
    callbacks.credentials(|requested_url, _, allowed| {
        if requested_url != url || !allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            return Err(git2::Error::from_str("source authentication denied"));
        }
        match secret.as_ref() {
            Some(token) => Cred::userpass_plaintext(
                "x-access-token",
                token
                    .as_str()
                    .map_err(|_| git2::Error::from_str("source authentication denied"))?,
            ),
            None => Err(git2::Error::from_str("source authentication unavailable")),
        }
    });
    let mut options = FetchOptions::new();
    options.remote_callbacks(callbacks);
    options.follow_redirects(git2::RemoteRedirect::None);
    options.download_tags(git2::AutotagOption::None);
    if fixture.is_none() {
        options.depth(1);
    }
    let fixture_url = fixture.and_then(Path::to_str);
    let selected_url = fixture_url.unwrap_or(&url);
    let mut remote = repository
        .remote_anonymous(selected_url)
        .map_err(|_| source_failure())?;
    remote
        .fetch(&[revision], Some(&mut options), None)
        .map_err(|_| source_failure())?;
    drop(remote);
    drop(options);
    drop(secret);
    // No remote is saved and credentials never reach the checkout or command.
    prepare_checkout(profile, revision, &repository, &root.join("checkout"))
}

fn prepare_checkout(
    profile: &JobProfile,
    revision: &str,
    repository: &Repository,
    checkout: &Path,
) -> PortResult<ContentId> {
    let oid = Oid::from_str(revision).map_err(|_| source_failure())?;
    let commit = repository.find_commit(oid).map_err(|_| source_failure())?;
    if commit.id().to_string() != revision {
        return Err(source_failure());
    }
    let provider = ProviderProfileId::new("github-job-git-v1")?;
    let adapter = LocalGitAdapter::new(
        repository.path().to_path_buf(),
        revision.to_owned(),
        GitSourceProfile::pinned_local(provider.clone(), profile.snapshot_limits.clone()),
    )?;
    let resolution = resolve_and_acquire(&adapter, provider).map_err(|_| source_failure())?;
    // Verify all filesystem-specific restrictions before writing any source.
    let tree = commit.tree().map_err(|_| source_failure())?;
    validate_checkout_tree(repository, &tree, 0, profile.snapshot_limits.max_path_bytes)?;
    fs::create_dir(checkout).map_err(|_| job_io())?;
    materialize_tree(repository, &tree, checkout)?;
    Ok(resolution.canonical_tree)
}

fn validate_checkout_tree(
    repository: &Repository,
    tree: &git2::Tree<'_>,
    prefix_bytes: usize,
    maximum_path_bytes: usize,
) -> PortResult<()> {
    for entry in tree {
        let name = entry.name().ok_or_else(source_failure)?;
        let path_bytes = prefix_bytes
            .checked_add(name.len())
            .ok_or_else(source_failure)?;
        if path_bytes > maximum_path_bytes
            || name.eq_ignore_ascii_case(".git")
            || name.is_empty()
            || name == "."
            || name == ".."
            || name.contains(['/', '\\', '\0'])
        {
            return Err(source_failure());
        }
        match entry.filemode() {
            0o040_000 => {
                let child = repository
                    .find_tree(entry.id())
                    .map_err(|_| source_failure())?;
                validate_checkout_tree(
                    repository,
                    &child,
                    path_bytes.checked_add(1).ok_or_else(source_failure)?,
                    maximum_path_bytes,
                )?;
            }
            0o100_644 | 0o100_755 => {}
            // Symlinks and gitlinks are not materialized: they could escape the
            // owned checkout or require a second, unadmitted source acquisition.
            _ => return Err(source_failure()),
        }
    }
    Ok(())
}

fn materialize_tree(
    repository: &Repository,
    tree: &git2::Tree<'_>,
    destination: &Path,
) -> PortResult<()> {
    use std::os::unix::fs::PermissionsExt;
    for entry in tree {
        let path = destination.join(entry.name().ok_or_else(source_failure)?);
        if entry.filemode() == 0o040_000 {
            fs::create_dir(&path).map_err(|_| job_io())?;
            let child = repository
                .find_tree(entry.id())
                .map_err(|_| source_failure())?;
            materialize_tree(repository, &child, &path)?;
        } else {
            let blob = repository
                .find_blob(entry.id())
                .map_err(|_| source_failure())?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| job_io())?;
            file.write_all(blob.content()).map_err(|_| job_io())?;
            let mode = if entry.filemode() == 0o100_755 {
                0o755
            } else {
                0o644
            };
            file.set_permissions(fs::Permissions::from_mode(mode))
                .map_err(|_| job_io())?;
        }
    }
    Ok(())
}

fn execute_command(
    profile: &JobProfile,
    context: &SourceContext,
    canonical_tree: ContentId,
    checkout: &Path,
    cancellation: Option<&AtomicBool>,
) -> PortResult<JobOutcome> {
    let request = request(
        CommandSpec {
            program: profile.program_path.clone(),
            args: profile.args.iter().map(OsString::from).collect(),
            current_dir: checkout.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: profile
                .environment
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect(),
            input: Input::Null,
        },
        execution_limits(
            profile,
            profile.command_timeout_ms,
            profile.maximum_output_bytes,
        ),
    )?;
    let result = match cancellation {
        Some(flag) => run_with_cancellation(request, flag),
        None => run(request),
    }
    .map_err(|_| {
        infrastructure(
            "github_job_execution",
            "GitHub job process could not be observed to completion",
        )
    })?;
    let termination = match result.disposition {
        Disposition::Succeeded | Disposition::ExitFailed => JobTermination::Exited {
            code: result.exit_code,
            signal: result.signal,
        },
        Disposition::TimedOut => JobTermination::TimedOut,
        Disposition::Cancelled => JobTermination::Cancelled,
        Disposition::OutputLimitExceeded(_) => JobTermination::OutputLimitExceeded,
    };
    // Deliberately discard stdout/stderr: commands may print source secrets.
    Ok(JobOutcome {
        revision: context.revision.clone(),
        canonical_tree,
        termination,
    })
}

pub fn run_job(profile: &JobProfile, context: &SourceContext) -> PortResult<JobOutcome> {
    supervised_job(profile, context, None, &AtomicBool::new(false))
}

pub fn run_job_with_cancellation(
    profile: &JobProfile,
    context: &SourceContext,
    cancellation: &AtomicBool,
) -> PortResult<JobOutcome> {
    supervised_job(profile, context, None, cancellation)
}

fn run_job_inner(
    profile: &JobProfile,
    context: &SourceContext,
    fixture: Option<&Path>,
    cancellation: Option<&AtomicBool>,
) -> PortResult<JobOutcome> {
    let _ = admit_source_context(profile, "github", &context.repository, &context.revision)?;
    if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err(job_io());
    }
    let root = tempfile::Builder::new()
        .prefix("kiln-github-job-")
        .tempdir()
        .map_err(|_| job_io())?;
    let profile_path = root.path().join("profile.json");
    let bytes = serde_json::to_vec(profile).map_err(|_| job_io())?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES {
        return Err(job_io());
    }
    fs::write(&profile_path, &bytes).map_err(|_| job_io())?;
    let mut args = vec![
        OsString::from("--acquire"),
        profile_path.into_os_string(),
        OsString::from(blake3::hash(&bytes).to_hex().as_str()),
        root.path().as_os_str().to_owned(),
        OsString::from(&context.revision),
    ];
    if let Some(path) = fixture {
        args.push(path.as_os_str().to_owned());
    }
    // Resolve outside the network child so killing that child cannot orphan a
    // nested resolver process group. Credentials cross only its bounded stdin.
    if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err(job_io());
    }
    let secret = profile
        .source_secret
        .as_ref()
        .map(|reference| resolve_secret(reference, "GitHub read-only exact source acquisition"))
        .transpose()?;
    let input = secret.as_ref().map_or(Input::Null, |secret| {
        Input::Bytes(secret.as_bytes().to_vec())
    });
    let mut limits = execution_limits(profile, profile.acquisition_timeout_ms, SOURCE_RESULT_BYTES);
    limits.stdin_max_bytes = MAXIMUM_SOURCE_SECRET_BYTES;
    let acquisition_request = request(
        CommandSpec {
            program: std::env::current_exe().map_err(|_| job_io())?,
            args,
            current_dir: root.path().to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: vec![
                (OsString::from("HOME"), root.path().as_os_str().to_owned()),
                (
                    OsString::from("XDG_CONFIG_HOME"),
                    root.path().as_os_str().to_owned(),
                ),
                (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            ],
            input,
        },
        limits,
    )?;
    let output = match cancellation {
        Some(flag) => run_with_cancellation(acquisition_request, flag),
        None => run(acquisition_request),
    }
    .map_err(|_| source_failure())?;
    drop(secret);
    if output.disposition != Disposition::Succeeded {
        return Err(source_failure());
    }
    let canonical_tree: ContentId =
        serde_json::from_slice(&output.stdout.bytes).map_err(|_| source_failure())?;
    execute_command(
        profile,
        context,
        canonical_tree,
        &root.path().join("checkout"),
        cancellation,
    )
}

// The workflow runtime may SIGKILL the entire outer job group. A detached
// supervisor retains ownership of the Bounded Exec groups, while an EOF-only
// lifeline converts even that uncatchable parent death into cancellation.
struct SocketRootCleanup(PathBuf);

impl Drop for SocketRootCleanup {
    fn drop(&mut self) {
        // Never recursively remove a caller-supplied path.
        let _ = fs::remove_file(self.0.join("lifeline.sock"));
        let _ = fs::remove_file(self.0.join("profile.json"));
        let _ = fs::remove_dir(&self.0);
    }
}

fn supervisor(args: &[OsString], cancellation: &AtomicBool) -> PortResult<i32> {
    let fixture = fixture_argument(args, 6)?;
    if args.len() != 6 && !(args.len() == 7 && fixture.is_some()) {
        return Err(job_io());
    }
    let [
        _,
        profile_path,
        identity,
        socket_argument,
        repository,
        revision,
        ..,
    ] = args
    else {
        return Err(job_io());
    };
    let socket = Path::new(socket_argument);
    let root = socket.parent().ok_or_else(job_io)?;
    if socket
        .file_name()
        .is_none_or(|name| name != "lifeline.sock")
        || !root.is_absolute()
        || root
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.starts_with("kiln-github-lifeline-"))
        || !fs::symlink_metadata(root)
            .map_err(|_| job_io())?
            .file_type()
            .is_dir()
    {
        return Err(job_io());
    }
    let _cleanup = SocketRootCleanup(root.to_path_buf());
    let mut stream = UnixStream::connect(socket).map_err(|_| job_io())?;
    // External effects require an explicit start byte from an accepted parent.
    // Bootstrap failure can therefore safely kill this still-childless process.
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| job_io())?;
    let mut start = [0u8; 1];
    stream.read_exact(&mut start).map_err(|_| job_io())?;
    if start != *b"S" {
        return Err(job_io());
    }
    stream
        .set_read_timeout(Some(Duration::from_millis(POLL_INTERVAL_MS)))
        .map_err(|_| job_io())?;
    let mut monitor = stream.try_clone().map_err(|_| job_io())?;
    let stop = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        let lifeline = scope.spawn(|| {
            let mut byte = [0u8; 1];
            while !stop.load(Ordering::Acquire) {
                match monitor.read(&mut byte) {
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock
                                | std::io::ErrorKind::TimedOut
                                | std::io::ErrorKind::Interrupted
                        ) => {}
                    _ => {
                        cancellation.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        });
        let result = (|| {
            let profile = load_job_profile(
                Path::new(profile_path),
                identity.to_str().ok_or_else(job_io)?,
            )?;
            let context = admit_source_context(
                &profile,
                "github",
                repository.to_str().ok_or_else(job_io)?,
                revision.to_str().ok_or_else(job_io)?,
            )?;
            run_job_inner(&profile, &context, fixture, Some(cancellation))
        })();
        stop.store(true, Ordering::Release);
        lifeline.join().map_err(|_| job_io())?;
        Ok::<_, kiln_provider_api::PortFailure>(result.ok())
    })?;
    let bytes = serde_json::to_vec(&result).map_err(|_| job_io())?;
    stream.write_all(&bytes).map_err(|_| job_io())?;
    stream.shutdown(Shutdown::Write).map_err(|_| job_io())?;
    Ok(0)
}

fn supervised_job(
    profile: &JobProfile,
    context: &SourceContext,
    fixture: Option<&Path>,
    cancellation: &AtomicBool,
) -> PortResult<JobOutcome> {
    let _ = admit_source_context(profile, "github", &context.repository, &context.revision)?;
    if cancellation.load(Ordering::Acquire) {
        return Err(job_io());
    }
    let root = tempfile::Builder::new()
        .prefix("kiln-github-lifeline-")
        .tempdir()
        .map_err(|_| job_io())?;
    let profile_path = root.path().join("profile.json");
    let bytes = serde_json::to_vec(profile).map_err(|_| job_io())?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES {
        return Err(job_io());
    }
    fs::write(&profile_path, &bytes).map_err(|_| job_io())?;
    let identity = blake3::hash(&bytes).to_hex().to_string();
    let socket = root.path().join("lifeline.sock");
    let listener = UnixListener::bind(&socket).map_err(|_| job_io())?;
    listener.set_nonblocking(true).map_err(|_| job_io())?;
    let mut command = Command::new(std::env::current_exe().map_err(|_| job_io())?);
    command
        .arg("--supervise")
        .arg(profile_path)
        .arg(identity)
        .arg(&socket)
        .arg(&context.repository)
        .arg(&context.revision);
    if let Some(fixture) = fixture {
        command.arg(fixture.canonicalize().map_err(|_| job_io())?);
    }
    let mut child = command
        .env_clear()
        .current_dir(root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|_| job_io())?;
    // No secret or external child exists before the supervisor connects. It is
    // safe to terminate a failed bootstrap directly; after connection, only the
    // lifeline may request teardown of its separately owned process groups.
    let bootstrap_deadline = Instant::now() + Duration::from_secs(5);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break Ok(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break Err(job_io()),
        }
        if cancellation.load(Ordering::Acquire) || Instant::now() >= bootstrap_deadline {
            break Err(job_io());
        }
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break Err(job_io()),
            Ok(None) => std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS)),
        }
    };
    let mut stream = match stream {
        Ok(stream) => stream,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let result = (|| {
        stream
            .set_read_timeout(Some(Duration::from_millis(POLL_INTERVAL_MS)))
            .map_err(|_| job_io())?;
        stream.write_all(b"S").map_err(|_| job_io())?;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; SOURCE_RESULT_BYTES];
        loop {
            if cancellation.load(Ordering::Acquire) {
                let _ = stream.shutdown(Shutdown::Write);
            }
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if bytes.len() + count > SOURCE_RESULT_BYTES {
                        return Err(job_io());
                    }
                    bytes.extend_from_slice(buffer.get(..count).ok_or_else(job_io)?);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(job_io()),
            }
        }
        let result: Option<JobOutcome> = serde_json::from_slice(&bytes).map_err(|_| job_io())?;
        if cancellation.load(Ordering::Acquire) {
            return Err(job_io());
        }
        result.ok_or_else(source_failure)
    })();
    let _ = stream.shutdown(Shutdown::Both);
    drop(stream);
    // The connection is closed before waiting, including every error path.
    // Thus the supervisor's monitor always has permission to cancel and reap.
    let status = child.wait().map_err(|_| job_io())?;
    if !status.success() {
        return Err(job_io());
    }
    result
}

// Fixed diagnostics only: neither Git/library errors nor process output is logged.
pub fn cli_main() -> i32 {
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    let mut registrations = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, cancellation.clone()) {
            Ok(id) => registrations.push(id),
            Err(_) => {
                for id in registrations {
                    signal_hook::low_level::unregister(id);
                }
                eprintln!("GitHub job failed");
                return 1;
            }
        }
    }
    let result = cli(std::env::args_os().skip(1).collect(), &cancellation);
    for id in registrations {
        signal_hook::low_level::unregister(id);
    }
    match result {
        Ok(code) => code,
        Err(_) => {
            eprintln!("GitHub job failed");
            1
        }
    }
}

fn cli(args: Vec<OsString>, cancellation: &AtomicBool) -> PortResult<i32> {
    if args.first().is_some_and(|arg| arg == "--supervise") {
        return supervisor(&args, cancellation);
    }
    if args.first().is_some_and(|arg| arg == "--acquire") {
        let fixture = fixture_argument(&args, 5)?;
        if args.len() != 5 && !(fixture.is_some() && args.len() == 6) {
            return Err(job_io());
        }
        let [_, profile_path, identity, root, revision, ..] = args.as_slice() else {
            return Err(job_io());
        };
        let profile = load_job_profile(
            Path::new(profile_path),
            identity.to_str().ok_or_else(job_io)?,
        )?;
        let revision = revision.to_str().ok_or_else(job_io)?;
        if !valid_revision(revision) {
            return Err(source_failure());
        }
        let secret = if profile.source_secret.is_some() {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take((MAXIMUM_SOURCE_SECRET_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| source_failure())?;
            Some(ResolvedSecret::from_bytes(bytes)?)
        } else {
            None
        };
        let canonical = acquire_source(&profile, revision, Path::new(root), fixture, secret)?;
        let output = serde_json::to_vec(&canonical).map_err(|_| job_io())?;
        std::io::stdout().write_all(&output).map_err(|_| job_io())?;
        return Ok(0);
    }
    let fixture = fixture_argument(&args, 5)?;
    if args.len() != 4 && !(fixture.is_some() && args.len() == 6) {
        return Err(job_io());
    }
    let [profile_flag, profile_path, identity_flag, identity, ..] = args.as_slice() else {
        return Err(job_io());
    };
    if profile_flag != "--profile" || identity_flag != "--identity" {
        return Err(job_io());
    }
    if fixture.is_some()
        && args
            .get(4)
            .is_none_or(|argument| argument != "--fixture-source")
    {
        return Err(job_io());
    }
    let profile = load_job_profile(
        Path::new(profile_path),
        identity.to_str().ok_or_else(job_io)?,
    )?;
    let provider = std::env::var("LATTICE_SOURCE_PROVIDER").map_err(|_| source_failure())?;
    let repository = std::env::var("LATTICE_SOURCE_REPOSITORY").map_err(|_| source_failure())?;
    let revision = std::env::var("LATTICE_SOURCE_REVISION").map_err(|_| source_failure())?;
    let context = admit_source_context(&profile, &provider, &repository, &revision)?;
    let outcome = supervised_job(&profile, &context, fixture, cancellation)?;
    Ok(match outcome.termination {
        JobTermination::Exited {
            code: Some(code),
            signal: None,
        } if (0..=255).contains(&code) => code,
        _ => 1,
    })
}

fn fixture_argument(args: &[OsString], index: usize) -> PortResult<Option<&Path>> {
    #[cfg(feature = "protocol-fixtures")]
    {
        Ok(args.get(index).map(Path::new))
    }
    #[cfg(not(feature = "protocol-fixtures"))]
    {
        if args.len() > index {
            return Err(job_io());
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
