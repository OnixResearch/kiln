use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const HOSTED_NIX_PROVIDER_SCHEMA: &str = "kiln.radicle-nix-provider.v1";
pub const PINNED_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_DURABLE_FILE_PUBLICATION_REVISION: &str =
    "8e05e74e24b45f752d77145c4455385daaf6d6ab";
pub const REQUIRED_PROVIDER_NON_CLAIMS: [&str; 7] = [
    "a local source path does not prove source trust or revision authority",
    "bounded execution does not prove sandboxing or hermeticity",
    "a process exit does not prove CI correctness",
    "report publication does not prove external report delivery",
    "filesystem synchronization does not prove storage-device persistence",
    "a provider receipt does not prove distributed exactly-once execution",
    "provider execution does not establish release eligibility",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedProviderLimits {
    pub timeout_milliseconds: u64,
    pub stdin_max_bytes: u64,
    pub stdout_max_bytes: u64,
    pub stderr_max_bytes: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
    pub wrapper_max_bytes: u64,
    pub report_max_bytes: u64,
    pub stage_collision_attempts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedNixProviderProfile {
    pub schema: String,
    pub bounded_exec_revision: String,
    pub durable_file_publication_revision: String,
    pub allowed_user_id: u32,
    pub repository: String,
    pub source_root: PathBuf,
    pub wrapper_path: PathBuf,
    pub wrapper_blake3: String,
    pub working_directory: PathBuf,
    pub report_root: PathBuf,
    pub report_namespace: String,
    pub report_base_url: String,
    pub limits: HostedProviderLimits,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedProviderRequest {
    pub repository: String,
    pub revision: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedProviderOutcome {
    Success,
    Failure,
    TimedOut,
    Cancelled,
    OutputLimitExceeded,
    ProviderError,
    Unknown,
}

impl HostedProviderOutcome {
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedProviderReceipt {
    pub schema: String,
    pub run_identity: String,
    pub repository: String,
    pub revision: String,
    pub wrapper_blake3: String,
    pub outcome: HostedProviderOutcome,
    pub exit_code: Option<i32>,
    pub log_blake3: String,
    pub report_blake3: String,
    pub log_url: String,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HostedProviderReport<'a> {
    pub(super) schema: &'static str,
    pub(super) run_identity: &'a str,
    pub(super) repository: &'a str,
    pub(super) revision: &'a str,
    pub(super) wrapper_blake3: &'a str,
    pub(super) outcome: HostedProviderOutcome,
    pub(super) exit_code: Option<i32>,
    pub(super) log_blake3: &'a str,
    pub(super) non_claims: &'a [String],
}

#[derive(Debug)]
pub enum HostedProviderError {
    Admission(String),
    Infrastructure(String),
}

impl std::fmt::Display for HostedProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(message) => write!(formatter, "provider admission failed: {message}"),
            Self::Infrastructure(message) => {
                write!(formatter, "provider infrastructure failed: {message}")
            }
        }
    }
}

impl std::error::Error for HostedProviderError {}

pub type HostedProviderResult<T> = Result<T, HostedProviderError>;
