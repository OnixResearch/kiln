use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use bounded_exec::ExecutionLimits;
use kiln_core::domain::Outcome;
use kiln_core::remote::{RemoteAction, RemoteCapability, RemoteProfile};
use kiln_core::types::{
    AttemptId, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, PipelineRevisionId,
    ProviderProfileId,
};
use serde::{Deserialize, Serialize};

pub const SLURM_PROVIDER_NAMESPACE: &str = "slurm";
pub const SLURM_JOB_NATIVE_KIND: &str = "job";
pub const SLURM_ACTION_NATIVE_KIND: &str = "action-blake3";
pub const SLURM_PROTOCOL_VERSION: &str = "slurm-cli-v1";
pub const SLURM_PROFILE_SCHEMA: &str = "kiln.slurm-remote.profile.v1";
pub const SLURM_CORRELATION_DOMAIN: &str = "kiln.slurm.correlation.v1";

pub const DEFAULT_MAX_ACTION_BYTES: usize = 65_536;
pub const DEFAULT_MAX_ARGV_ITEMS: usize = 64;
pub const DEFAULT_MAX_ARG_BYTES: usize = 4_096;
pub const DEFAULT_MAX_PATH_BYTES: usize = 4_096;
pub const DEFAULT_MAX_STDOUT_BYTES: usize = 65_536;
pub const DEFAULT_MAX_STDERR_BYTES: usize = 65_536;
pub const DEFAULT_MAX_LOG_BYTES: usize = 1_048_576;
pub const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 15_000;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 25;
pub const DEFAULT_TEARDOWN_TIMEOUT_MS: u64 = 1_000;
pub const DEFAULT_MAX_WAIT_POLLS: usize = 64;
pub const DEFAULT_MAX_IN_FLIGHT: usize = 16;
pub const DEFAULT_STDIN_MAX_BYTES: usize = 1;

pub const DEFAULT_MIN_CPUS: u32 = 1;
pub const DEFAULT_MAX_CPUS: u32 = 64;
pub const DEFAULT_MIN_MEMORY_MB: u64 = 128;
pub const DEFAULT_MAX_MEMORY_MB: u64 = 262_144;
pub const DEFAULT_MIN_TIME_LIMIT_MINUTES: u32 = 1;
pub const DEFAULT_MAX_TIME_LIMIT_MINUTES: u32 = 1_440;

pub const MAX_SLURM_JOB_ID_LEN: usize = 20;
pub const MAX_SLURM_CLUSTER_LEN: usize = 64;
pub const MAX_SLURM_IDENTIFIER_LEN: usize = 128;
pub const MAX_COLLECTION_ITEMS: usize = 256;
pub const JOB_NAME_HASH_PREFIX_LEN: usize = 16;
pub const REQUIRED_SINGLE_NODE_COUNT: u32 = 1;
pub const REQUIRED_SINGLE_TASK_COUNT: u32 = 1;

const SQUEUE_MIN_COLUMNS: usize = 2;
const SQUEUE_MAX_COLUMNS: usize = 5;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlurmRequestedFeature {
    BatchSubmit,
    JobPoll,
    JobWait,
    JobCancel,
    LogRead,
    InteractivePty,
    LiveAttach,
    GpuGres,
    JobArray,
    MultiNode,
    GuestNetworking,
}

impl SlurmRequestedFeature {
    #[must_use]
    pub const fn is_unsupported(self) -> bool {
        matches!(
            self,
            Self::InteractivePty
                | Self::LiveAttach
                | Self::GpuGres
                | Self::JobArray
                | Self::MultiNode
                | Self::GuestNetworking
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlurmExecutablePaths {
    pub sbatch_path: PathBuf,
    pub squeue_path: PathBuf,
    pub scancel_path: PathBuf,
    pub sacct_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlurmPolicyBounds {
    pub min_cpus: u32,
    pub max_cpus: u32,
    pub min_memory_mb: u64,
    pub max_memory_mb: u64,
    pub min_time_limit_minutes: u32,
    pub max_time_limit_minutes: u32,
    pub max_argv_items: usize,
    pub max_arg_bytes: usize,
    pub max_action_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_log_bytes: usize,
    pub max_in_flight: usize,
    pub max_wait_polls: usize,
    pub command_timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub teardown_timeout_ms: u64,
    pub stdin_max_bytes: usize,
}

impl Default for SlurmPolicyBounds {
    fn default() -> Self {
        Self {
            min_cpus: DEFAULT_MIN_CPUS,
            max_cpus: DEFAULT_MAX_CPUS,
            min_memory_mb: DEFAULT_MIN_MEMORY_MB,
            max_memory_mb: DEFAULT_MAX_MEMORY_MB,
            min_time_limit_minutes: DEFAULT_MIN_TIME_LIMIT_MINUTES,
            max_time_limit_minutes: DEFAULT_MAX_TIME_LIMIT_MINUTES,
            max_argv_items: DEFAULT_MAX_ARGV_ITEMS,
            max_arg_bytes: DEFAULT_MAX_ARG_BYTES,
            max_action_bytes: DEFAULT_MAX_ACTION_BYTES,
            max_stdout_bytes: DEFAULT_MAX_STDOUT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
            max_log_bytes: DEFAULT_MAX_LOG_BYTES,
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
            max_wait_polls: DEFAULT_MAX_WAIT_POLLS,
            command_timeout_ms: DEFAULT_COMMAND_TIMEOUT_MS,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            teardown_timeout_ms: DEFAULT_TEARDOWN_TIMEOUT_MS,
            stdin_max_bytes: DEFAULT_STDIN_MAX_BYTES,
        }
    }
}

impl SlurmPolicyBounds {
    #[must_use]
    pub const fn execution_limits(&self) -> ExecutionLimits {
        ExecutionLimits {
            timeout_ms: self.command_timeout_ms,
            stdin_max_bytes: self.stdin_max_bytes,
            stdout_max_bytes: self.max_stdout_bytes,
            stderr_max_bytes: self.max_stderr_bytes,
            poll_interval_ms: self.poll_interval_ms,
            teardown_timeout_ms: self.teardown_timeout_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlurmProfile {
    pub schema: String,
    pub enabled: bool,
    pub provider: ProviderProfileId,
    pub cluster_name: Option<String>,
    pub executables: SlurmExecutablePaths,
    pub allowed_partitions: BTreeSet<String>,
    pub allowed_accounts: BTreeSet<String>,
    pub allowed_working_dir_prefixes: Vec<PathBuf>,
    pub log_root: Option<PathBuf>,
    pub capabilities: BTreeSet<RemoteCapability>,
    pub bounds: SlurmPolicyBounds,
}

impl SlurmProfile {
    /// Disabled-by-default profile template requiring explicit operator opt-in.
    pub fn disabled_default() -> CoreResult<Self> {
        Ok(Self {
            schema: SLURM_PROFILE_SCHEMA.to_string(),
            enabled: false,
            provider: ProviderProfileId::new("slurm-remote-v1")?,
            cluster_name: None,
            executables: SlurmExecutablePaths {
                sbatch_path: PathBuf::from("/usr/bin/sbatch"),
                squeue_path: PathBuf::from("/usr/bin/squeue"),
                scancel_path: PathBuf::from("/usr/bin/scancel"),
                sacct_path: None,
            },
            allowed_partitions: BTreeSet::new(),
            allowed_accounts: BTreeSet::new(),
            allowed_working_dir_prefixes: Vec::new(),
            log_root: None,
            capabilities: BTreeSet::from([
                RemoteCapability::CasUpload,
                RemoteCapability::Execute,
                RemoteCapability::Poll,
                RemoteCapability::Wait,
                RemoteCapability::Cancel,
                RemoteCapability::LogStream,
            ]),
            bounds: SlurmPolicyBounds::default(),
        })
    }

    pub fn to_remote_profile(&self) -> CoreResult<RemoteProfile> {
        let digest = ExternalRef::new("kiln", "digest", "blake3")?;
        Ok(RemoteProfile {
            protocol_version: ExternalRef::new(
                SLURM_PROVIDER_NAMESPACE,
                "version",
                SLURM_PROTOCOL_VERSION,
            )?,
            provider: self.provider.clone(),
            digest_functions: BTreeSet::from([digest]),
            capabilities: self.capabilities.clone(),
            max_action_bytes: self.bounds.max_action_bytes,
            max_log_bytes: self.bounds.max_log_bytes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlurmResources {
    pub partition: String,
    pub account: String,
    pub cpus_per_task: u32,
    pub memory_mb: u64,
    pub time_limit_minutes: u32,
    pub nodes: u32,
    pub ntasks: u32,
    pub gres: Option<String>,
    pub array_spec: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlurmSubmissionSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub resources: SlurmResources,
    pub requested_features: BTreeSet<SlurmRequestedFeature>,
    pub interactive_pty: bool,
    pub live_attach: bool,
    pub guest_networking: bool,
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlurmCorrelation {
    pub correlation_id: ContentId,
    pub action_ref: ExternalRef,
    pub job_name: String,
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub pipeline_revision: PipelineRevisionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedSbatchOutput {
    pub job_id: String,
    pub cluster: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SlurmJobPhase {
    Pending,
    Running,
    Succeeded,
    Failed {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    OutOfMemory,
    TimedOut,
    NodeFail,
    Cancelled,
    Unknown {
        raw_state: Option<String>,
    },
}

impl SlurmJobPhase {
    #[must_use]
    pub const fn to_kiln_outcome(&self) -> Outcome {
        match self {
            Self::Pending | Self::Running => Outcome::Pending,
            Self::Succeeded => Outcome::Success,
            Self::Failed { .. } | Self::OutOfMemory => Outcome::DomainFailure,
            Self::TimedOut => Outcome::TimedOut,
            Self::NodeFail => Outcome::ProviderError,
            Self::Cancelled => Outcome::Cancelled,
            Self::Unknown { .. } => Outcome::Unknown,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlurmJobRecord {
    pub job_id: String,
    pub phase: SlurmJobPhase,
    pub raw_state: Option<String>,
    pub exit_code: Option<String>,
    pub node_list: Option<String>,
    pub reason: Option<String>,
}

/// Validates structural profile configuration without requiring `enabled == true`.
pub fn validate_profile_structure(profile: &SlurmProfile) -> CoreResult<()> {
    if profile.schema != SLURM_PROFILE_SCHEMA {
        return Err(Diagnostic::new(
            "slurm_profile_schema",
            "slurm.schema",
            "Slurm profile schema must match kiln.slurm-remote.profile.v1",
        ));
    }
    if let Some(cluster) = &profile.cluster_name {
        validate_slurm_identifier(cluster, MAX_SLURM_CLUSTER_LEN, "slurm.cluster_name")?;
    }
    validate_clean_absolute_path(&profile.executables.sbatch_path, "slurm.executables.sbatch_path")?;
    validate_clean_absolute_path(&profile.executables.squeue_path, "slurm.executables.squeue_path")?;
    validate_clean_absolute_path(
        &profile.executables.scancel_path,
        "slurm.executables.scancel_path",
    )?;
    if let Some(sacct_path) = &profile.executables.sacct_path {
        validate_clean_absolute_path(sacct_path, "slurm.executables.sacct_path")?;
    }
    if profile.allowed_partitions.len() > MAX_COLLECTION_ITEMS {
        return Err(Diagnostic::new(
            "slurm_partition_limit",
            "slurm.allowed_partitions",
            "Slurm allowlisted partition set exceeds its named collection bound",
        ));
    }
    for partition in &profile.allowed_partitions {
        validate_slurm_identifier(partition, MAX_SLURM_IDENTIFIER_LEN, "slurm.allowed_partitions")?;
    }
    if profile.allowed_accounts.len() > MAX_COLLECTION_ITEMS {
        return Err(Diagnostic::new(
            "slurm_account_limit",
            "slurm.allowed_accounts",
            "Slurm allowlisted account set exceeds its named collection bound",
        ));
    }
    for account in &profile.allowed_accounts {
        validate_slurm_identifier(account, MAX_SLURM_IDENTIFIER_LEN, "slurm.allowed_accounts")?;
    }
    if profile.allowed_working_dir_prefixes.len() > MAX_COLLECTION_ITEMS {
        return Err(Diagnostic::new(
            "slurm_working_dir_limit",
            "slurm.allowed_working_dir_prefixes",
            "Slurm working directory prefix list exceeds its named bound",
        ));
    }
    for prefix in &profile.allowed_working_dir_prefixes {
        validate_clean_absolute_path(prefix, "slurm.allowed_working_dir_prefixes")?;
    }
    if let Some(log_root) = &profile.log_root {
        validate_clean_absolute_path(log_root, "slurm.log_root")?;
    }
    validate_bounds(&profile.bounds)?;
    Ok(())
}

/// Admits an enabled Slurm profile for runtime execution.
pub fn admit_profile(profile: &SlurmProfile) -> CoreResult<()> {
    if !profile.enabled {
        return Err(Diagnostic::new(
            "slurm_adapter_disabled",
            "slurm.enabled",
            "Slurm remote adapter is off by default and must be explicitly enabled",
        ));
    }
    validate_profile_structure(profile)?;
    if profile.allowed_partitions.is_empty() {
        return Err(Diagnostic::new(
            "slurm_partitions_empty",
            "slurm.allowed_partitions",
            "enabled Slurm profile must declare at least one allowlisted partition",
        ));
    }
    if profile.allowed_accounts.is_empty() {
        return Err(Diagnostic::new(
            "slurm_accounts_empty",
            "slurm.allowed_accounts",
            "enabled Slurm profile must declare at least one allowlisted account",
        ));
    }
    Ok(())
}

/// Pure admission of a Slurm submission request and deterministic BLAKE3 correlation.
pub fn admit_submission(
    profile: &SlurmProfile,
    action: &RemoteAction,
    spec: &SlurmSubmissionSpec,
) -> CoreResult<SlurmCorrelation> {
    admit_profile(profile)?;
    validate_remote_action(profile, action)?;
    if !profile.capabilities.contains(&RemoteCapability::Execute) {
        return Err(Diagnostic::new(
            "remote_capability_unsupported",
            "slurm.capabilities",
            "Slurm profile does not declare Execute capability",
        ));
    }
    admit_requested_capabilities(profile, spec)?;
    admit_resources(profile, &spec.resources)?;
    validate_clean_absolute_path(&spec.program, "slurm.submission.program")?;
    let working_dir = validate_clean_absolute_path(&spec.working_dir, "slurm.submission.working_dir")?;
    if !profile.allowed_working_dir_prefixes.is_empty()
        && !profile
            .allowed_working_dir_prefixes
            .iter()
            .any(|prefix| spec.working_dir.starts_with(prefix))
    {
        return Err(Diagnostic::new(
            "slurm_working_dir_denied",
            "slurm.submission.working_dir",
            "submission working directory is outside the allowlisted prefixes",
        ));
    }
    let _ = working_dir;
    if spec.args.len() > profile.bounds.max_argv_items {
        return Err(Diagnostic::new(
            "slurm_argv_bound",
            "slurm.submission.args",
            "submission argument vector exceeds its named item bound",
        ));
    }
    for arg in &spec.args {
        if arg.len() > profile.bounds.max_arg_bytes {
            return Err(Diagnostic::new(
                "slurm_arg_bound",
                "slurm.submission.args",
                "submission argument exceeds its named byte bound",
            ));
        }
        if contains_control_or_nul(arg) {
            return Err(Diagnostic::new(
                "slurm_arg_control_char",
                "slurm.submission.args",
                "submission argument contains control characters or NUL bytes",
            ));
        }
    }
    if let Some(stdout_path) = &spec.stdout_path {
        validate_log_destination(profile, stdout_path, "slurm.submission.stdout_path")?;
    }
    if let Some(stderr_path) = &spec.stderr_path {
        validate_log_destination(profile, stderr_path, "slurm.submission.stderr_path")?;
    }
    let encoded_bytes = serde_json::to_vec(&(action, spec))
        .map_err(|_| {
            Diagnostic::new(
                "slurm_request_encoding",
                "slurm.submission",
                "submission encoding failed",
            )
        })?
        .len();
    if encoded_bytes > profile.bounds.max_action_bytes {
        return Err(Diagnostic::new(
            "slurm_action_bound",
            "slurm.submission",
            "encoded Slurm submission exceeds the named action byte bound",
        ));
    }
    derive_correlation(action, spec)
}

/// Builds the exact discrete `sbatch` argument vector with zero shell interpolation.
pub fn build_sbatch_args(
    spec: &SlurmSubmissionSpec,
    correlation: &SlurmCorrelation,
) -> CoreResult<Vec<String>> {
    let program = validate_clean_absolute_path(&spec.program, "slurm.submission.program")?;
    let working_dir = validate_clean_absolute_path(&spec.working_dir, "slurm.submission.working_dir")?;
    let mut argv = vec![
        "--parsable".to_string(),
        format!("--nodes={}", spec.resources.nodes),
        format!("--ntasks={}", spec.resources.ntasks),
        "--export=NONE".to_string(),
        format!("--partition={}", spec.resources.partition),
        format!("--account={}", spec.resources.account),
        format!("--cpus-per-task={}", spec.resources.cpus_per_task),
        format!("--mem={}M", spec.resources.memory_mb),
        format!("--time={}", spec.resources.time_limit_minutes),
        format!("--chdir={working_dir}"),
        format!("--job-name={}", correlation.job_name),
        format!("--comment={}", correlation.correlation_id.as_str()),
    ];
    if let Some(stdout_path) = &spec.stdout_path {
        let path_str = validate_clean_absolute_path(stdout_path, "slurm.submission.stdout_path")?;
        argv.push(format!("--output={path_str}"));
    }
    if let Some(stderr_path) = &spec.stderr_path {
        let path_str = validate_clean_absolute_path(stderr_path, "slurm.submission.stderr_path")?;
        argv.push(format!("--error={path_str}"));
    }
    argv.push(program.to_string());
    argv.extend(spec.args.iter().cloned());
    Ok(argv)
}

/// Builds exact discrete `squeue` arguments for single-job status observation.
#[must_use]
pub fn build_squeue_args(job_id: &str) -> Vec<String> {
    vec![
        "--noheader".to_string(),
        format!("--jobs={job_id}"),
        "--Format=JobID:|,State:|,ExitCode:|,NodeList:|,Reason".to_string(),
    ]
}

/// Builds exact discrete `scancel` arguments for single-job cancellation.
#[must_use]
pub fn build_scancel_args(job_id: &str) -> Vec<String> {
    vec![job_id.to_string()]
}

/// Validates a Slurm operation `ExternalRef` and returns the native numeric job ID.
pub fn validate_slurm_operation_ref(operation: &ExternalRef) -> CoreResult<&str> {
    operation.validate()?;
    if operation.provider_namespace.as_str() != SLURM_PROVIDER_NAMESPACE {
        return Err(Diagnostic::new(
            "slurm_operation_namespace",
            "operation.provider_namespace",
            "operation reference must belong to the slurm namespace",
        ));
    }
    if operation.native_kind.as_str() != SLURM_JOB_NATIVE_KIND {
        return Err(Diagnostic::new(
            "slurm_operation_kind",
            "operation.native_kind",
            "operation reference must have native kind job",
        ));
    }
    let job_id = operation.native_id.as_str();
    validate_slurm_job_id(job_id, "operation.native_id")?;
    Ok(job_id)
}

/// Parses strict `sbatch --parsable` output (`<job_id>[;<cluster>]`).
pub fn parse_sbatch_output(
    stdout: &str,
    expected_cluster: Option<&str>,
) -> CoreResult<ParsedSbatchOutput> {
    let trimmed = stdout.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return Err(Diagnostic::new(
            "slurm_malformed_sbatch_output",
            "sbatch.stdout",
            "sbatch --parsable produced empty output",
        ));
    }
    if trimmed.contains(['\n', '\r']) || contains_control_or_nul(trimmed) {
        return Err(Diagnostic::new(
            "slurm_malformed_sbatch_output",
            "sbatch.stdout",
            "sbatch --parsable produced multi-line or control-character output",
        ));
    }
    let (job_id, cluster) = match trimmed.split_once(';') {
        Some((job_id, rest)) => {
            if rest.contains(';') {
                return Err(Diagnostic::new(
                    "slurm_malformed_sbatch_output",
                    "sbatch.stdout",
                    "sbatch --parsable contained unexpected delimiter segments",
                ));
            }
            validate_slurm_identifier(rest, MAX_SLURM_CLUSTER_LEN, "sbatch.stdout.cluster")
                .map_err(|_| {
                    Diagnostic::new(
                        "slurm_malformed_sbatch_output",
                        "sbatch.stdout",
                        "sbatch --parsable cluster field is invalid",
                    )
                })?;
            (job_id, Some(rest.to_string()))
        }
        None => (trimmed, None),
    };
    validate_slurm_job_id(job_id, "sbatch.stdout").map_err(|_| {
        Diagnostic::new(
            "slurm_malformed_sbatch_output",
            "sbatch.stdout",
            "sbatch --parsable job identifier must be a positive integer",
        )
    })?;
    if let (Some(expected), Some(observed)) = (expected_cluster, cluster.as_deref())
        && expected != observed
    {
        return Err(Diagnostic::new(
            "slurm_cluster_mismatch",
            "sbatch.stdout.cluster",
            "sbatch reported a different cluster than configured",
        ));
    }
    Ok(ParsedSbatchOutput {
        job_id: job_id.to_string(),
        cluster,
    })
}

/// Parses pipe-delimited `squeue` output (`JobID|State|ExitCode|NodeList|Reason`)
/// for a single expected Slurm job ID.
pub fn parse_squeue_output(expected_job_id: &str, stdout: &str) -> CoreResult<SlurmJobRecord> {
    validate_slurm_job_id(expected_job_id, "squeue.expected_job_id")?;
    if stdout.trim().is_empty() {
        return Ok(SlurmJobRecord {
            job_id: expected_job_id.to_string(),
            phase: SlurmJobPhase::Unknown { raw_state: None },
            raw_state: None,
            exit_code: None,
            node_list: None,
            reason: None,
        });
    }
    if contains_control_chars_except_newlines(stdout) {
        return Err(Diagnostic::new(
            "slurm_malformed_squeue_output",
            "squeue.stdout",
            "squeue output contains forbidden control characters",
        ));
    }
    let lines: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() != 1 {
        return Err(Diagnostic::new(
            "slurm_malformed_squeue_output",
            "squeue.stdout",
            "single-job squeue query returned unexpected multiple lines",
        ));
    }
    let line = lines.first().copied().ok_or_else(|| {
        Diagnostic::new(
            "slurm_malformed_squeue_output",
            "squeue.stdout",
            "squeue line was unexpectedly absent",
        )
    })?;
    parse_delimited_job_line(expected_job_id, line, "squeue.stdout")
}

/// Parses `sacct --parsable2 --noheader` output (`JobIDRaw|State|ExitCode|NodeList|Reason`),
/// selecting the primary job row and ignoring `.batch`/`.extern` sub-steps for the same job ID.
pub fn parse_sacct_output(expected_job_id: &str, stdout: &str) -> CoreResult<SlurmJobRecord> {
    validate_slurm_job_id(expected_job_id, "sacct.expected_job_id")?;
    if stdout.trim().is_empty() {
        return Ok(SlurmJobRecord {
            job_id: expected_job_id.to_string(),
            phase: SlurmJobPhase::Unknown { raw_state: None },
            raw_state: None,
            exit_code: None,
            node_list: None,
            reason: None,
        });
    }
    if contains_control_chars_except_newlines(stdout) {
        return Err(Diagnostic::new(
            "slurm_malformed_sacct_output",
            "sacct.stdout",
            "sacct output contains forbidden control characters",
        ));
    }
    let mut primary_row: Option<SlurmJobRecord> = None;
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < SQUEUE_MIN_COLUMNS || fields.len() > SQUEUE_MAX_COLUMNS {
            return Err(Diagnostic::new(
                "slurm_malformed_sacct_output",
                "sacct.stdout",
                "sacct row has invalid column count",
            ));
        }
        let raw_id = fields.first().copied().unwrap_or_default().trim();
        let root_id = raw_id.split('.').next().unwrap_or(raw_id);
        if root_id != expected_job_id {
            return Err(Diagnostic::new(
                "slurm_job_identity_mismatch",
                "sacct.stdout",
                "sacct returned a different job identity than requested",
            ));
        }
        if raw_id == expected_job_id {
            if primary_row.is_some() {
                return Err(Diagnostic::new(
                    "slurm_malformed_sacct_output",
                    "sacct.stdout",
                    "sacct returned duplicate primary job rows",
                ));
            }
            primary_row = Some(parse_delimited_job_line(expected_job_id, line, "sacct.stdout")?);
        }
    }
    Ok(primary_row.unwrap_or(SlurmJobRecord {
        job_id: expected_job_id.to_string(),
        phase: SlurmJobPhase::Unknown { raw_state: None },
        raw_state: None,
        exit_code: None,
        node_list: None,
        reason: None,
    }))
}

/// Maps a Slurm state string and optional Slurm exit code (`"rc"` or `"rc:sig"`)
/// into a deterministic `SlurmJobPhase`.
#[must_use]
pub fn classify_slurm_state(raw_state: &str, raw_exit_code: Option<&str>) -> SlurmJobPhase {
    let normalized = raw_state.trim().trim_end_matches('+');
    let head = normalized.split_whitespace().next().unwrap_or_default();
    let upper = head.to_ascii_uppercase();
    let parsed_exit = raw_exit_code.and_then(parse_slurm_exit_code);
    match upper.as_str() {
        "PENDING" | "PD" | "CONFIGURING" | "CF" => SlurmJobPhase::Pending,
        "RUNNING" | "R" | "COMPLETING" | "CG" => SlurmJobPhase::Running,
        "COMPLETED" | "CD" => match parsed_exit {
            Some((0, 0)) => SlurmJobPhase::Succeeded,
            Some((code, sig)) => SlurmJobPhase::Failed {
                exit_code: Some(code),
                signal: (sig != 0).then_some(sig),
            },
            None => SlurmJobPhase::Unknown {
                raw_state: Some(raw_state.trim().to_string()),
            },
        },
        "FAILED" | "F" => match parsed_exit {
            Some((code, sig)) => SlurmJobPhase::Failed {
                exit_code: Some(code),
                signal: (sig != 0).then_some(sig),
            },
            None => SlurmJobPhase::Failed {
                exit_code: None,
                signal: None,
            },
        },
        "OUT_OF_MEMORY" | "OOM" => SlurmJobPhase::OutOfMemory,
        "TIMEOUT" | "TO" | "DEADLINE" | "DL" => SlurmJobPhase::TimedOut,
        "NODE_FAIL" | "NF" | "BOOT_FAIL" | "BF" => SlurmJobPhase::NodeFail,
        "CANCELLED" | "CA" => SlurmJobPhase::Cancelled,
        _ => SlurmJobPhase::Unknown {
            raw_state: (!raw_state.trim().is_empty()).then(|| raw_state.trim().to_string()),
        },
    }
}

fn parse_delimited_job_line(
    expected_job_id: &str,
    line: &str,
    field: &'static str,
) -> CoreResult<SlurmJobRecord> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < SQUEUE_MIN_COLUMNS || fields.len() > SQUEUE_MAX_COLUMNS {
        return Err(Diagnostic::new(
            "slurm_malformed_status_output",
            field,
            "Slurm status row must contain 2 to 5 pipe-delimited columns",
        ));
    }
    let observed_job_id = fields.first().copied().unwrap_or_default().trim();
    validate_slurm_job_id(observed_job_id, field).map_err(|_| {
        Diagnostic::new(
            "slurm_malformed_status_output",
            field,
            "Slurm status row contained an invalid job identifier",
        )
    })?;
    if observed_job_id != expected_job_id {
        return Err(Diagnostic::new(
            "slurm_job_identity_mismatch",
            field,
            "Slurm status row job identifier does not match the requested operation",
        ));
    }
    let raw_state = fields.get(1).copied().unwrap_or_default().trim();
    if raw_state.is_empty() {
        return Err(Diagnostic::new(
            "slurm_malformed_status_output",
            field,
            "Slurm status row omitted the job state field",
        ));
    }
    let exit_code_raw = fields
        .get(2)
        .copied()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let node_list = fields
        .get(3)
        .copied()
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "(null)")
        .map(str::to_string);
    let reason = fields
        .get(4)
        .copied()
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "None")
        .map(str::to_string);
    let phase = classify_slurm_state(raw_state, exit_code_raw);
    Ok(SlurmJobRecord {
        job_id: observed_job_id.to_string(),
        phase,
        raw_state: Some(raw_state.to_string()),
        exit_code: exit_code_raw.map(str::to_string),
        node_list,
        reason,
    })
}

fn parse_slurm_exit_code(raw: &str) -> Option<(i32, i32)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.split_once(':') {
        Some((code, signal)) => {
            let code = code.trim().parse::<i32>().ok()?;
            let signal = signal.trim().parse::<i32>().ok()?;
            Some((code, signal))
        }
        None => {
            let code = trimmed.parse::<i32>().ok()?;
            Some((code, 0))
        }
    }
}

fn validate_remote_action(profile: &SlurmProfile, action: &RemoteAction) -> CoreResult<()> {
    if action.provider != profile.provider {
        return Err(Diagnostic::new(
            "slurm_provider_mismatch",
            "remote_action.provider",
            "remote action provider does not match the selected Slurm profile",
        ));
    }
    action.source_or_candidate.validate()?;
    action.command.validate()?;
    action.environment.validate()?;
    action.platform.validate()?;
    action.input_root.validate()?;
    action.canonical_input_root.validate()?;
    action.timeout_policy.validate()?;
    for output in &action.declared_outputs {
        output.validate()?;
    }
    Ok(())
}

fn admit_requested_capabilities(
    profile: &SlurmProfile,
    spec: &SlurmSubmissionSpec,
) -> CoreResult<()> {
    if spec.interactive_pty {
        return Err(Diagnostic::new(
            "slurm_unsupported_capability",
            "slurm.submission.interactive_pty",
            "interactive PTY allocation is unsupported by the Slurm batch adapter",
        ));
    }
    if spec.live_attach {
        return Err(Diagnostic::new(
            "slurm_unsupported_capability",
            "slurm.submission.live_attach",
            "live attach is unsupported by the Slurm batch adapter",
        ));
    }
    if spec.guest_networking {
        return Err(Diagnostic::new(
            "slurm_unsupported_capability",
            "slurm.submission.guest_networking",
            "guest networking is unsupported by the Slurm batch adapter",
        ));
    }
    for feature in &spec.requested_features {
        if feature.is_unsupported() {
            return Err(Diagnostic::new(
                "slurm_unsupported_capability",
                "slurm.submission.requested_features",
                "requested Slurm feature is unsupported by this adapter profile",
            ));
        }
        let required_remote_cap = match feature {
            SlurmRequestedFeature::BatchSubmit => RemoteCapability::Execute,
            SlurmRequestedFeature::JobPoll => RemoteCapability::Poll,
            SlurmRequestedFeature::JobWait => RemoteCapability::Wait,
            SlurmRequestedFeature::JobCancel => RemoteCapability::Cancel,
            SlurmRequestedFeature::LogRead => RemoteCapability::LogStream,
            _ => continue,
        };
        if !profile.capabilities.contains(&required_remote_cap) {
            return Err(Diagnostic::new(
                "remote_capability_unsupported",
                "slurm.capabilities",
                "Slurm profile does not enable the requested capability",
            ));
        }
    }
    Ok(())
}

fn admit_resources(profile: &SlurmProfile, resources: &SlurmResources) -> CoreResult<()> {
    if resources.nodes != REQUIRED_SINGLE_NODE_COUNT {
        return Err(Diagnostic::new(
            "slurm_unsupported_multi_node",
            "slurm.submission.resources.nodes",
            "multi-node allocation is unsupported; nodes must equal 1",
        ));
    }
    if resources.ntasks != REQUIRED_SINGLE_TASK_COUNT {
        return Err(Diagnostic::new(
            "slurm_unsupported_multi_task",
            "slurm.submission.resources.ntasks",
            "multi-task MPI launch is unsupported; ntasks must equal 1",
        ));
    }
    if resources.gres.is_some() {
        return Err(Diagnostic::new(
            "slurm_unsupported_gres",
            "slurm.submission.resources.gres",
            "GPU GRES requests are unsupported in this Slurm adapter profile",
        ));
    }
    if resources.array_spec.is_some() {
        return Err(Diagnostic::new(
            "slurm_unsupported_job_array",
            "slurm.submission.resources.array_spec",
            "Slurm job arrays are unsupported in this Slurm adapter profile",
        ));
    }
    validate_slurm_identifier(
        &resources.partition,
        MAX_SLURM_IDENTIFIER_LEN,
        "slurm.submission.resources.partition",
    )?;
    if !profile.allowed_partitions.contains(&resources.partition) {
        return Err(Diagnostic::new(
            "slurm_partition_denied",
            "slurm.submission.resources.partition",
            "requested Slurm partition is not in the profile allowlist",
        ));
    }
    validate_slurm_identifier(
        &resources.account,
        MAX_SLURM_IDENTIFIER_LEN,
        "slurm.submission.resources.account",
    )?;
    if !profile.allowed_accounts.contains(&resources.account) {
        return Err(Diagnostic::new(
            "slurm_account_denied",
            "slurm.submission.resources.account",
            "requested Slurm account is not in the profile allowlist",
        ));
    }
    if resources.cpus_per_task < profile.bounds.min_cpus
        || resources.cpus_per_task > profile.bounds.max_cpus
    {
        return Err(Diagnostic::new(
            "slurm_cpu_bound",
            "slurm.submission.resources.cpus_per_task",
            "requested CPU count falls outside the configured Slurm bounds",
        ));
    }
    if resources.memory_mb < profile.bounds.min_memory_mb
        || resources.memory_mb > profile.bounds.max_memory_mb
    {
        return Err(Diagnostic::new(
            "slurm_memory_bound",
            "slurm.submission.resources.memory_mb",
            "requested memory falls outside the configured Slurm bounds",
        ));
    }
    if resources.time_limit_minutes < profile.bounds.min_time_limit_minutes
        || resources.time_limit_minutes > profile.bounds.max_time_limit_minutes
    {
        return Err(Diagnostic::new(
            "slurm_time_limit_bound",
            "slurm.submission.resources.time_limit_minutes",
            "requested walltime falls outside the configured Slurm bounds",
        ));
    }
    Ok(())
}

fn validate_log_destination(
    profile: &SlurmProfile,
    path: &Path,
    field: &'static str,
) -> CoreResult<()> {
    let _ = validate_clean_absolute_path(path, field)?;
    if let Some(log_root) = &profile.log_root
        && !path.starts_with(log_root)
    {
        return Err(Diagnostic::new(
            "slurm_log_path_denied",
            field,
            "log destination path must reside under the configured Slurm log_root",
        ));
    }
    Ok(())
}

fn validate_bounds(bounds: &SlurmPolicyBounds) -> CoreResult<()> {
    if bounds.min_cpus == 0 || bounds.min_cpus > bounds.max_cpus {
        return Err(Diagnostic::new(
            "slurm_invalid_bounds",
            "slurm.bounds.cpus",
            "Slurm CPU bounds must be positive and ordered",
        ));
    }
    if bounds.min_memory_mb == 0 || bounds.min_memory_mb > bounds.max_memory_mb {
        return Err(Diagnostic::new(
            "slurm_invalid_bounds",
            "slurm.bounds.memory_mb",
            "Slurm memory bounds must be positive and ordered",
        ));
    }
    if bounds.min_time_limit_minutes == 0
        || bounds.min_time_limit_minutes > bounds.max_time_limit_minutes
    {
        return Err(Diagnostic::new(
            "slurm_invalid_bounds",
            "slurm.bounds.time_limit_minutes",
            "Slurm time-limit bounds must be positive and ordered",
        ));
    }
    if bounds.max_argv_items == 0
        || bounds.max_arg_bytes == 0
        || bounds.max_action_bytes == 0
        || bounds.max_in_flight == 0
        || bounds.max_wait_polls == 0
        || bounds.max_log_bytes == 0
    {
        return Err(Diagnostic::new(
            "slurm_invalid_bounds",
            "slurm.bounds",
            "Slurm policy bounds must all be positive",
        ));
    }
    if bounds.execution_limits().validate().is_err() {
        return Err(Diagnostic::new(
            "slurm_invalid_execution_limits",
            "slurm.bounds",
            "Slurm bounded-exec timing or stream bounds are invalid",
        ));
    }
    Ok(())
}

fn derive_correlation(
    action: &RemoteAction,
    spec: &SlurmSubmissionSpec,
) -> CoreResult<SlurmCorrelation> {
    let mut hasher = blake3::Hasher::new();
    update_len_prefixed(&mut hasher, SLURM_CORRELATION_DOMAIN.as_bytes());
    update_len_prefixed(&mut hasher, action.effect_id.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.attempt_id.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.pipeline_revision.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.source_or_candidate.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.command.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.environment.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.platform.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.canonical_input_root.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.timeout_policy.as_str().as_bytes());
    update_len_prefixed(&mut hasher, action.provider.as_str().as_bytes());
    update_len_prefixed(&mut hasher, spec.program.to_string_lossy().as_bytes());
    update_len_prefixed(&mut hasher, spec.working_dir.to_string_lossy().as_bytes());
    hasher.update(&(spec.args.len() as u64).to_le_bytes());
    for arg in &spec.args {
        update_len_prefixed(&mut hasher, arg.as_bytes());
    }
    update_len_prefixed(&mut hasher, spec.resources.partition.as_bytes());
    update_len_prefixed(&mut hasher, spec.resources.account.as_bytes());
    hasher.update(&spec.resources.cpus_per_task.to_le_bytes());
    hasher.update(&spec.resources.memory_mb.to_le_bytes());
    hasher.update(&spec.resources.time_limit_minutes.to_le_bytes());
    let hex = hasher.finalize().to_hex().to_string();
    let correlation_id = ContentId::from_blake3_hex(hex)?;
    let prefix = correlation_id
        .as_str()
        .get(..JOB_NAME_HASH_PREFIX_LEN)
        .ok_or_else(|| {
            Diagnostic::new(
                "slurm_correlation_hash",
                "slurm.correlation",
                "BLAKE3 correlation digest was shorter than expected",
            )
        })?;
    let job_name = format!("kiln-{prefix}");
    let action_ref = ExternalRef::new(
        SLURM_PROVIDER_NAMESPACE,
        SLURM_ACTION_NATIVE_KIND,
        correlation_id.as_str(),
    )?;
    Ok(SlurmCorrelation {
        correlation_id,
        action_ref,
        job_name,
        effect_id: action.effect_id.clone(),
        attempt_id: action.attempt_id.clone(),
        pipeline_revision: action.pipeline_revision.clone(),
    })
}

fn update_len_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub(crate) fn validate_clean_absolute_path<'a>(
    path: &'a Path,
    field: &'static str,
) -> CoreResult<&'a str> {
    if !path.is_absolute() {
        return Err(Diagnostic::new(
            "slurm_path_not_absolute",
            field,
            "path must be absolute",
        ));
    }
    let text = path.to_str().ok_or_else(|| {
        Diagnostic::new(
            "slurm_path_non_utf8",
            field,
            "path must be valid UTF-8",
        )
    })?;
    if text.is_empty() || text.len() > DEFAULT_MAX_PATH_BYTES || contains_control_or_nul(text) {
        return Err(Diagnostic::new(
            "slurm_path_invalid",
            field,
            "path is empty, oversized, or contains control characters",
        ));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Diagnostic::new(
            "slurm_path_traversal",
            field,
            "path must not contain parent-directory traversal",
        ));
    }
    Ok(text)
}

pub(crate) fn validate_slurm_identifier(
    value: &str,
    max_len: usize,
    field: &'static str,
) -> CoreResult<()> {
    if value.is_empty() || value.len() > max_len {
        return Err(Diagnostic::new(
            "slurm_identifier_bound",
            field,
            "Slurm identifier must be non-empty and within bounds",
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(Diagnostic::new(
            "slurm_identifier_invalid",
            field,
            "Slurm identifier contains disallowed characters",
        ));
    }
    Ok(())
}

pub(crate) fn validate_slurm_job_id(value: &str, field: &'static str) -> CoreResult<()> {
    if value.is_empty() || value.len() > MAX_SLURM_JOB_ID_LEN {
        return Err(Diagnostic::new(
            "slurm_job_id_invalid",
            field,
            "Slurm job identifier must be non-empty and within 20 digits",
        ));
    }
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Diagnostic::new(
            "slurm_job_id_invalid",
            field,
            "Slurm job identifier must contain only ASCII digits",
        ));
    }
    let parsed = value.parse::<u64>().map_err(|_| {
        Diagnostic::new(
            "slurm_job_id_invalid",
            field,
            "Slurm job identifier could not be parsed as an unsigned integer",
        )
    })?;
    if parsed == 0 {
        return Err(Diagnostic::new(
            "slurm_job_id_invalid",
            field,
            "Slurm job identifier must be positive",
        ));
    }
    Ok(())
}

fn contains_control_or_nul(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn contains_control_chars_except_newlines(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\r')
}
