//! Local workspace initialization (`kiln init`).
//!
//! Pure configuration planning and validation live in [`plan_workspace`] and
//! its helpers; filesystem inspection, token/TLS generation, and atomic
//! directory creation stay in the imperative shell [`run`].

use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use git2::{Repository, RepositoryOpenFlags};
use kiln_codec::identity_from_bytes;
use serde_json::{Value, json};

pub const INIT_USAGE: &str = "\
usage: kiln init --repo PATH --out DIR [--project ID]

options:
  --repo PATH     existing local Git repository directory to bind
  --out DIR       new directory to create for generated workspace files (refuses existing paths)
  --project ID    optional project identifier (alphanumeric, '-', '_', '.'; max 100 chars)
  --help, -h      print this text";

const DEFAULT_SERVICE_ID: &str = "kiln-local";
const DEFAULT_LISTEN_PORT: u16 = 9443;
const DEFAULT_PROJECT_ID: &str = "local-repo";
const MAX_PROJECT_ID_BYTES: usize = 100;
const MAX_LABEL_BYTES: usize = 256;
const MAX_REFERENCE_BYTES: usize = 1024;
const OPENSSL_TIMEOUT_MS: u64 = 15_000;
const OPENSSL_POLL_MS: u64 = 25;
const OPENSSL_TEARDOWN_MS: u64 = 2_000;
const OPENSSL_STDIN_BYTES: usize = 1_024;
const OPENSSL_STDOUT_BYTES: usize = 16_384;
const OPENSSL_STDERR_BYTES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitAction {
    Help,
    Init {
        repo: PathBuf,
        out: PathBuf,
        project: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitError {
    Usage(String),
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspacePlanInput {
    pub repo_path: PathBuf,
    pub out_dir: PathBuf,
    pub project_id: String,
    pub project_label: String,
    pub reference: String,
    pub owner_uid: u32,
    pub listen_port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedWorkspace {
    pub service_json: String,
    pub access_json: String,
    pub sources_json: String,
    pub start_here_txt: String,
    pub summary_text: String,
}

/// Parse `kiln init` arguments without touching the filesystem.
pub fn parse_init_args(arguments: impl Iterator<Item = String>) -> Result<InitAction, InitError> {
    let mut args: Vec<String> = arguments.collect();
    if args.len() == 1
        && args
            .first()
            .is_some_and(|arg| matches!(arg.as_str(), "--help" | "-h" | "help"))
    {
        return Ok(InitAction::Help);
    }
    let mut repo: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut project: Option<String> = None;
    let mut iter = args.drain(..);
    while let Some(flag) = iter.next() {
        if matches!(flag.as_str(), "--help" | "-h") {
            return Err(InitError::Usage(INIT_USAGE.to_owned()));
        }
        match flag.as_str() {
            "--repo" => {
                if repo.is_some() {
                    return Err(InitError::Usage(INIT_USAGE.to_owned()));
                }
                let value = next_flag_value(&mut iter)?;
                repo = Some(PathBuf::from(value));
            }
            "--out" => {
                if out.is_some() {
                    return Err(InitError::Usage(INIT_USAGE.to_owned()));
                }
                let value = next_flag_value(&mut iter)?;
                out = Some(PathBuf::from(value));
            }
            "--project" => {
                if project.is_some() {
                    return Err(InitError::Usage(INIT_USAGE.to_owned()));
                }
                let value = next_flag_value(&mut iter)?;
                project = Some(value);
            }
            _ => return Err(InitError::Usage(INIT_USAGE.to_owned())),
        }
    }
    let (Some(repo), Some(out)) = (repo, out) else {
        return Err(InitError::Usage(INIT_USAGE.to_owned()));
    };
    Ok(InitAction::Init { repo, out, project })
}

fn next_flag_value(iter: &mut impl Iterator<Item = String>) -> Result<String, InitError> {
    iter.next()
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .ok_or_else(|| InitError::Usage(INIT_USAGE.to_owned()))
}

/// Reject empty, non-UTF-8, or control-character paths before any filesystem effect.
pub fn validate_path_text(path: &Path, flag: &str) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{flag} path must be valid UTF-8"))?;
    if text.is_empty() {
        return Err(format!("{flag} path cannot be empty"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!("{flag} path must not contain control characters"));
    }
    Ok(text.to_owned())
}

/// Validate a Kiln project identifier (`[A-Za-z0-9_.-]+`, 1..=100 bytes, not `.` or `..`).
pub fn validate_project_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > MAX_PROJECT_ID_BYTES {
        return Err(format!(
            "invalid --project identifier: must be 1..={MAX_PROJECT_ID_BYTES} ASCII characters"
        ));
    }
    if matches!(id, "." | "..") {
        return Err("invalid --project identifier: '.' and '..' are not allowed".to_owned());
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(
            "invalid --project identifier: only ASCII alphanumeric, '-', '_', and '.' are allowed"
                .to_owned(),
        );
    }
    Ok(())
}

/// Derive a safe project identifier from a repository directory name.
pub fn derive_project_id(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches(|ch| ch == '-' || ch == '.');
    if trimmed.is_empty() {
        return DEFAULT_PROJECT_ID.to_owned();
    }
    let clipped: String = trimmed.chars().take(MAX_PROJECT_ID_BYTES).collect();
    if matches!(clipped.as_str(), "." | "..") || clipped.is_empty() {
        DEFAULT_PROJECT_ID.to_owned()
    } else {
        clipped
    }
}

/// Validate a clean label (1..=256 chars, no control characters).
pub fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty() || label.len() > MAX_LABEL_BYTES || label.chars().any(char::is_control) {
        return Err("project label must be 1..=256 characters without control bytes".to_owned());
    }
    Ok(())
}

/// Validate a clean Git reference string compatible with `ServiceProfile`.
pub fn validate_reference(reference: &str) -> Result<(), String> {
    if reference.is_empty()
        || reference.len() > MAX_REFERENCE_BYTES
        || reference.starts_with('-')
        || reference
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("repository reference is invalid".to_owned());
    }
    Ok(())
}

fn is_clean_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

/// Purely plan every generated file and operator next-step summary.
pub fn plan_workspace(input: &WorkspacePlanInput) -> Result<PlannedWorkspace, String> {
    if !is_clean_absolute(&input.repo_path) {
        return Err("resolved repository path must be clean and absolute".to_owned());
    }
    if !is_clean_absolute(&input.out_dir) {
        return Err("resolved output directory must be clean and absolute".to_owned());
    }
    validate_path_text(&input.repo_path, "--repo")?;
    validate_path_text(&input.out_dir, "--out")?;
    validate_project_id(&input.project_id)?;
    validate_label(&input.project_label)?;
    validate_reference(&input.reference)?;
    if input.listen_port == 0 {
        return Err("listen port must be non-zero".to_owned());
    }

    let op_seed = format!("kiln.init.local-node.v1:{}:build", input.project_id);
    let operation_identity = identity_from_bytes(op_seed.as_bytes())
        .map_err(|error| format!("failed to derive node operation identity: {error}"))?;

    let cert_path = input.out_dir.join("cert.pem");
    let key_path = input.out_dir.join("key.pem");
    let operator_token_path = input.out_dir.join("operator.token");
    let worker_token_path = input.out_dir.join("worker.token");
    let state_dir = input.out_dir.join("state");
    let socket_path = input.out_dir.join("lattice.sock");
    let service_path = input.out_dir.join("service.json");
    let access_path = input.out_dir.join("access.json");
    let sources_path = input.out_dir.join("sources.json");

    let listen_address = format!("127.0.0.1:{}", input.listen_port);
    let origin = format!("https://127.0.0.1:{}", input.listen_port);

    let service_value: Value = json!({
        "schema": "kiln.web-service.v1",
        "service_id": DEFAULT_SERVICE_ID,
        "listen_address": listen_address,
        "origin": origin,
        "tls": {
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "min_version": "TLS1.3"
        },
        "operator_token_file": operator_token_path,
        "worker_token_file": worker_token_path,
        "state_directory": state_dir,
        "projects": [{
            "id": input.project_id,
            "label": input.project_label,
            "repository_path": input.repo_path,
            "reference": input.reference,
            "source_limits": {
                "max_entries": 4096,
                "max_path_bytes": 1024,
                "max_file_bytes": 1_048_576,
                "max_total_bytes": 16_777_216
            },
            "workflow": {
                "socket_path": socket_path,
                "allowed_user_ids": [input.owner_uid],
                "transport_limits": {
                    "maximum_frame_bytes": 1_048_576,
                    "maximum_log_bytes": 65_536,
                    "maximum_connections": 4,
                    "read_deadline_milliseconds": 1000,
                    "write_deadline_milliseconds": 1000
                },
                "adapter_limits": {
                    "maximum_request_bytes": 1_048_576,
                    "maximum_response_bytes": 1_048_576
                },
                "policy_id": "local-default",
                "provider_profile_id": "local-lattice",
                "platform": "linux-x86_64",
                "nodes": [{
                    "id": "build",
                    "kind": "build",
                    "effect_class": "build",
                    "adapter_kind": "local",
                    "operation_identity": operation_identity.as_str()
                }],
                "edges": [],
                "controls": {
                    "cancel": true,
                    "pause": false,
                    "resume": false
                },
                "bounds": {
                    "max_nodes": 8,
                    "max_edges": 16,
                    "max_log_references": 32,
                    "max_log_read_bytes": 16_384,
                    "max_total_log_bytes": 65_536
                }
            }
        }],
        "bounds": {
            "max_request_bytes": 524_288,
            "max_response_bytes": 4_194_304,
            "max_header_bytes": 16_384,
            "max_connections": 8,
            "timeout_seconds": 5,
            "poll_interval_ms": 500,
            "max_projects": 16,
            "max_runs": 256,
            "max_artifact_bytes": 65_536,
            "max_artifacts": 32,
            "max_run_artifact_bytes": 524_288,
            "max_total_artifact_bytes": 8_388_608,
            "max_log_read_bytes": 16_384,
            "max_requests": 4096,
            "max_events_per_run": 128,
            "max_state_bytes": 16_777_216
        }
    });

    let access_value: Value = json!({
        "schema": "kiln.web-access.v1",
        "viewers": []
    });

    let sources_value: Value = json!({
        "schema": "kiln.web-sources.v1",
        "sources": [{
            "id": input.project_id,
            "label": input.project_label,
            "kind": "git",
            "path": input.repo_path
        }]
    });

    let service_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&service_value)
            .map_err(|error| format!("service.json serialization failed: {error}"))?
    );
    let access_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&access_value)
            .map_err(|error| format!("access.json serialization failed: {error}"))?
    );
    let sources_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&sources_value)
            .map_err(|error| format!("sources.json serialization failed: {error}"))?
    );

    let check_cmd = format!(
        "kiln-web-service check --profile {} --access {} --sources {}",
        service_path.display(),
        access_path.display(),
        sources_path.display()
    );
    let serve_cmd = format!(
        "kiln-web-service serve --profile {} --access {} --sources {}",
        service_path.display(),
        access_path.display(),
        sources_path.display()
    );

    let start_here_txt = format!(
        "\
Kiln Local Workspace
====================
Project:    {}
Repository: {}
Reference:  {}
Console:    {origin}

Generated files:
  service.json     Service profile (HTTPS listen address, project allowlist, bounds)
  access.json      Viewer access policy (operator uses operator.token by default)
  sources.json     Read-only local source browser configuration
  cert.pem         Self-signed localhost TLS 1.3 certificate (for local use only)
  key.pem          Private TLS key (mode 0600)
  operator.token   Operator bearer credential (mode 0600)
  worker.token     Worker bearer credential (mode 0600)
  state/           Private SQLite + artifact directory (mode 0700)

1. Validate the workspace (offline; contacts no external providers):
   {check_cmd}

2. Start the local HTTPS web console:
   {serve_cmd}

   (If running from a source checkout instead of installed binaries, prefix with:
    cargo run --locked -p kiln-web-service -- ...)

3. Open {origin} in your browser (accept the local self-signed certificate
   from {cert}) and sign in with the token inside:
   {op_token}

Note: `kiln-web-service` owns CI state and local read-only source browsing.
Starting real workflow jobs also requires a separate Lattice worker listening at:
  {sock}
",
        input.project_id,
        input.repo_path.display(),
        input.reference,
        cert = cert_path.display(),
        op_token = operator_token_path.display(),
        sock = socket_path.display(),
    );

    let summary_text = format!(
        "\
Initialized local Kiln workspace at {}
  Project: {} ({} @ {})

Next steps:
  1. Validate configuration (no network or providers contacted):
     {check_cmd}
  2. Start the local web console:
     {serve_cmd}
  3. Open {origin} and sign in with the operator token in:
     {}
     (See {} for file details.)",
        input.out_dir.display(),
        input.project_id,
        input.repo_path.display(),
        input.reference,
        operator_token_path.display(),
        input.out_dir.join("START_HERE.txt").display(),
    );

    Ok(PlannedWorkspace {
        service_json,
        access_json,
        sources_json,
        start_here_txt,
        summary_text,
    })
}

/// Imperative shell entry point for `kiln init`.
pub fn run(arguments: impl Iterator<Item = String>) -> Result<String, InitError> {
    let action = parse_init_args(arguments)?;
    let InitAction::Init { repo, out, project } = action else {
        return Ok(INIT_USAGE.to_owned());
    };
    init_workspace(&repo, &out, project.as_deref()).map_err(InitError::Failed)
}

fn init_workspace(repo: &Path, out: &Path, project: Option<&str>) -> Result<String, String> {
    validate_path_text(repo, "--repo")?;
    validate_path_text(out, "--out")?;
    if let Some(id) = project {
        validate_project_id(id)?;
    }

    // Fail closed if `--out` already exists (including symlinks).
    match fs::symlink_metadata(out) {
        Ok(_) => {
            return Err(format!(
                "--out path already exists; refusing to overwrite: {}",
                out.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot inspect --out path {}: {error}",
                out.display()
            ));
        }
    }

    // Validate `--repo` exists and is a directory containing a Git repository with a resolvable revision.
    let repo_meta = fs::metadata(repo)
        .map_err(|error| format!("--repo path is unavailable ({}): {error}", repo.display()))?;
    if !repo_meta.is_dir() {
        return Err(format!("--repo must be a directory: {}", repo.display()));
    }
    let repo_abs = fs::canonicalize(repo)
        .map_err(|error| format!("--repo path canonicalization failed: {error}"))?;
    let reference = resolve_git_reference(&repo_abs)?;

    let repo_name = repo_abs
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(DEFAULT_PROJECT_ID);
    let project_id = match project {
        Some(explicit) => explicit.to_owned(),
        None => derive_project_id(repo_name),
    };
    let project_label = if project_id == DEFAULT_PROJECT_ID {
        repo_name.to_owned()
    } else {
        project_id.clone()
    };

    // Resolve `--out` parent to a symlink-free canonical absolute path before creating anything.
    let out_abs = resolve_new_output_path(out)?;
    let openssl_exe = find_executable("openssl").ok_or_else(|| {
        "openssl executable not found in PATH (required to generate local TLS cert/key); run inside 'nix develop path:$PWD' or install openssl".to_owned()
    })?;

    // Create `--out` atomically with private permissions (0700).
    let mut dir_builder = DirBuilder::new();
    dir_builder.mode(0o700);
    dir_builder.create(&out_abs).map_err(|error| {
        format!(
            "failed to create --out directory {}: {error}",
            out_abs.display()
        )
    })?;

    let result = populate_workspace(
        &openssl_exe,
        &repo_abs,
        &out_abs,
        &project_id,
        &project_label,
        &reference,
    );
    if result.is_err() {
        let _ = fs::remove_dir_all(&out_abs);
    }
    result
}

fn populate_workspace(
    openssl_exe: &Path,
    repo_abs: &Path,
    out_abs: &Path,
    project_id: &str,
    project_label: &str,
    reference: &str,
) -> Result<String, String> {
    let out_meta = fs::symlink_metadata(out_abs)
        .map_err(|error| format!("created output directory metadata unavailable: {error}"))?;
    let owner_uid = out_meta.uid();

    let planned = plan_workspace(&WorkspacePlanInput {
        repo_path: repo_abs.to_path_buf(),
        out_dir: out_abs.to_path_buf(),
        project_id: project_id.to_owned(),
        project_label: project_label.to_owned(),
        reference: reference.to_owned(),
        owner_uid,
        listen_port: DEFAULT_LISTEN_PORT,
    })?;

    let mut state_builder = DirBuilder::new();
    state_builder.mode(0o700);
    state_builder
        .create(out_abs.join("state"))
        .map_err(|error| format!("failed to create state directory: {error}"))?;

    kiln_web_service::profile::write_feed_token(&out_abs.join("operator.token"))
        .map_err(|error| format!("failed to create operator.token: {error}"))?;
    kiln_web_service::profile::write_feed_token(&out_abs.join("worker.token"))
        .map_err(|error| format!("failed to create worker.token: {error}"))?;

    generate_local_tls(
        openssl_exe,
        out_abs,
        &out_abs.join("cert.pem"),
        &out_abs.join("key.pem"),
    )?;

    write_new_file(
        &out_abs.join("service.json"),
        planned.service_json.as_bytes(),
        0o600,
    )?;
    write_new_file(
        &out_abs.join("access.json"),
        planned.access_json.as_bytes(),
        0o600,
    )?;
    write_new_file(
        &out_abs.join("sources.json"),
        planned.sources_json.as_bytes(),
        0o600,
    )?;
    write_new_file(
        &out_abs.join("START_HERE.txt"),
        planned.start_here_txt.as_bytes(),
        0o644,
    )?;

    Ok(planned.summary_text)
}

fn resolve_git_reference(repo_abs: &Path) -> Result<String, String> {
    let repository = Repository::open_ext(
        repo_abs,
        RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&Path>(),
    )
    .map_err(|_| {
        format!(
            "--repo is not a readable Git repository: {}",
            repo_abs.display()
        )
    })?;
    if let Ok(head) = repository.head()
        && let Ok(Some(target)) = head.symbolic_target()
        && validate_reference(target).is_ok()
        && repository.revparse_single(target).is_ok()
    {
        return Ok(target.to_owned());
    }
    if repository.revparse_single("HEAD").is_ok() {
        return Ok("HEAD".to_owned());
    }
    Err(format!(
        "--repo has no commits yet (HEAD cannot be resolved): {}",
        repo_abs.display()
    ))
}

fn resolve_new_output_path(out: &Path) -> Result<PathBuf, String> {
    let joined = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot determine current directory: {error}"))?
            .join(out)
    };
    let name = joined
        .file_name()
        .filter(|name| !name.is_empty() && *name != OsStr::new(".") && *name != OsStr::new(".."))
        .ok_or_else(|| format!("--out path must end in a directory name: {}", out.display()))?
        .to_os_string();
    let parent = joined
        .parent()
        .ok_or_else(|| format!("--out path must have a parent directory: {}", out.display()))?;
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent directory for --out ({}): {error}",
                parent.display()
            )
        })?;
    }
    let parent_abs = fs::canonicalize(parent).map_err(|error| {
        format!(
            "cannot resolve parent directory for --out ({}): {error}",
            parent.display()
        )
    })?;
    let candidate = parent_abs.join(name);
    if fs::symlink_metadata(&candidate).is_ok() {
        return Err(format!(
            "--out path already exists; refusing to overwrite: {}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync {}: {error}", path.display()))?;
    Ok(())
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if let Ok(meta) = fs::metadata(&candidate)
            && meta.is_file()
            && meta.permissions().mode() & 0o111 != 0
        {
            return Some(candidate);
        }
    }
    None
}

fn generate_local_tls(
    openssl_exe: &Path,
    out_abs: &Path,
    cert_path: &Path,
    key_path: &Path,
) -> Result<(), String> {
    let args = vec![
        OsString::from("req"),
        OsString::from("-x509"),
        OsString::from("-newkey"),
        OsString::from("ec"),
        OsString::from("-pkeyopt"),
        OsString::from("ec_paramgen_curve:prime256v1"),
        OsString::from("-nodes"),
        OsString::from("-keyout"),
        key_path.as_os_str().to_os_string(),
        OsString::from("-out"),
        cert_path.as_os_str().to_os_string(),
        OsString::from("-days"),
        OsString::from("365"),
        OsString::from("-subj"),
        OsString::from("/CN=localhost"),
        OsString::from("-addext"),
        OsString::from("subjectAltName=DNS:localhost,IP:127.0.0.1"),
    ];
    let output = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program: openssl_exe.to_path_buf(),
            args,
            current_dir: out_abs.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: OPENSSL_TIMEOUT_MS,
            stdin_max_bytes: OPENSSL_STDIN_BYTES,
            stdout_max_bytes: OPENSSL_STDOUT_BYTES,
            stderr_max_bytes: OPENSSL_STDERR_BYTES,
            poll_interval_ms: OPENSSL_POLL_MS,
            teardown_timeout_ms: OPENSSL_TEARDOWN_MS,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|error| format!("TLS generator policy error: {error:?}"))?,
    })
    .map_err(|error| format!("TLS generator execution failed: {error}"))?;

    match output.disposition {
        Disposition::Succeeded => {
            fs::set_permissions(key_path, fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("failed to protect key.pem permissions: {error}"))?;
            fs::set_permissions(cert_path, fs::Permissions::from_mode(0o644))
                .map_err(|error| format!("failed to set cert.pem permissions: {error}"))?;
            Ok(())
        }
        Disposition::ExitFailed => {
            let stderr = String::from_utf8_lossy(&output.stderr.bytes);
            Err(format!("openssl TLS generation failed: {stderr}"))
        }
        Disposition::TimedOut => Err("openssl TLS generation timed out".to_owned()),
        Disposition::Cancelled => Err("openssl TLS generation cancelled".to_owned()),
        Disposition::OutputLimitExceeded(_) => {
            Err("openssl TLS generation output exceeded bound".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_init_flags_in_any_order() {
        let parsed = parse_init_args(
            [
                "--out",
                "/tmp/out",
                "--project",
                "my-app",
                "--repo",
                "/tmp/repo",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert_eq!(
            parsed,
            Ok(InitAction::Init {
                repo: PathBuf::from("/tmp/repo"),
                out: PathBuf::from("/tmp/out"),
                project: Some("my-app".to_owned()),
            })
        );
    }

    #[test]
    fn parses_help_and_rejects_missing_or_duplicate_flags() {
        assert_eq!(
            parse_init_args(["--help"].into_iter().map(str::to_owned)),
            Ok(InitAction::Help)
        );
        assert!(matches!(
            parse_init_args(["--repo", "/tmp/repo"].into_iter().map(str::to_owned)),
            Err(InitError::Usage(_))
        ));
        assert!(matches!(
            parse_init_args(
                ["--repo", "/tmp/a", "--repo", "/tmp/b", "--out", "/tmp/out"]
                    .into_iter()
                    .map(str::to_owned)
            ),
            Err(InitError::Usage(_))
        ));
        assert!(matches!(
            parse_init_args(
                ["--repo", "--out", "/tmp/out"]
                    .into_iter()
                    .map(str::to_owned)
            ),
            Err(InitError::Usage(_))
        ));
    }

    #[test]
    fn project_id_validation_rejects_invalid_names_and_allows_valid_ones() {
        assert!(validate_project_id("kiln-app_01.v2").is_ok());
        assert!(validate_project_id("").is_err());
        assert!(validate_project_id(".").is_err());
        assert!(validate_project_id("..").is_err());
        assert!(validate_project_id("bad/name").is_err());
        assert!(validate_project_id("bad name").is_err());
        assert!(validate_project_id("bad\nname").is_err());
        assert!(validate_project_id(&"a".repeat(101)).is_err());
    }

    #[test]
    fn derive_project_id_normalizes_directory_names() {
        assert_eq!(derive_project_id("my repo!"), "my-repo");
        assert_eq!(derive_project_id("..."), "local-repo");
        assert_eq!(derive_project_id("kiln"), "kiln");
    }

    #[test]
    fn validate_path_text_rejects_control_chars() {
        assert!(validate_path_text(Path::new("/tmp/ok"), "--repo").is_ok());
        assert!(validate_path_text(Path::new("/tmp/bad\npath"), "--repo").is_err());
        assert!(validate_path_text(Path::new(""), "--out").is_err());
    }

    #[test]
    fn plan_workspace_emits_valid_schemas_and_next_steps() {
        let planned = plan_workspace(&WorkspacePlanInput {
            repo_path: PathBuf::from("/tmp/sample-repo"),
            out_dir: PathBuf::from("/tmp/sample-kiln"),
            project_id: "sample-repo".to_owned(),
            project_label: "sample-repo".to_owned(),
            reference: "refs/heads/main".to_owned(),
            owner_uid: 1000,
            listen_port: 9443,
        });
        let Ok(planned) = planned else {
            unreachable!("unexpected plan failure: {planned:?}");
        };

        let Ok(service) = serde_json::from_str::<Value>(&planned.service_json) else {
            unreachable!("service_json invalid");
        };
        assert_eq!(
            service.get("schema").and_then(Value::as_str),
            Some("kiln.web-service.v1")
        );
        let first_project = service
            .get("projects")
            .and_then(Value::as_array)
            .and_then(|projects| projects.first());
        assert_eq!(
            first_project
                .and_then(|p| p.get("id"))
                .and_then(Value::as_str),
            Some("sample-repo")
        );
        assert_eq!(
            first_project
                .and_then(|p| p.get("repository_path"))
                .and_then(Value::as_str),
            Some("/tmp/sample-repo")
        );
        assert_eq!(
            first_project
                .and_then(|p| p.get("reference"))
                .and_then(Value::as_str),
            Some("refs/heads/main")
        );

        let Ok(access) = serde_json::from_str::<Value>(&planned.access_json) else {
            unreachable!("access_json invalid");
        };
        assert_eq!(
            access.get("schema").and_then(Value::as_str),
            Some("kiln.web-access.v1")
        );

        let Ok(sources) = serde_json::from_str::<Value>(&planned.sources_json) else {
            unreachable!("sources_json invalid");
        };
        assert_eq!(
            sources.get("schema").and_then(Value::as_str),
            Some("kiln.web-sources.v1")
        );
        assert_eq!(
            sources
                .get("sources")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("kind"))
                .and_then(Value::as_str),
            Some("git")
        );

        assert!(planned.summary_text.contains(
            "kiln-web-service check --profile /tmp/sample-kiln/service.json --access /tmp/sample-kiln/access.json --sources /tmp/sample-kiln/sources.json"
        ));
        assert!(planned.summary_text.contains(
            "kiln-web-service serve --profile /tmp/sample-kiln/service.json --access /tmp/sample-kiln/access.json --sources /tmp/sample-kiln/sources.json"
        ));
        assert!(planned.start_here_txt.contains("https://127.0.0.1:9443"));
    }

    #[test]
    fn plan_workspace_rejects_relative_or_dirty_paths() {
        let relative = plan_workspace(&WorkspacePlanInput {
            repo_path: PathBuf::from("relative/repo"),
            out_dir: PathBuf::from("/tmp/out"),
            project_id: "sample".to_owned(),
            project_label: "sample".to_owned(),
            reference: "refs/heads/main".to_owned(),
            owner_uid: 1000,
            listen_port: 9443,
        });
        assert!(relative.is_err());

        let dirty = plan_workspace(&WorkspacePlanInput {
            repo_path: PathBuf::from("/tmp/repo/../other"),
            out_dir: PathBuf::from("/tmp/out"),
            project_id: "sample".to_owned(),
            project_label: "sample".to_owned(),
            reference: "refs/heads/main".to_owned(),
            owner_uid: 1000,
            listen_port: 9443,
        });
        assert!(dirty.is_err());
    }
}
