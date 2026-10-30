//! The reviewed spindle profile, authored in `config/tangled-spindle.ncl` and
//! exported to JSON. Admission is pure and fails closed.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;

use kiln_codec::encode_and_identify;
use kiln_core::types::{Alias, ContentId, PolicyId, ProviderProfileId};
use kiln_core::workflow::{WorkflowBounds, WorkflowControls};
use serde::{Deserialize, Serialize};

use crate::error::{Rejection, TangledResult};
use crate::identity::{valid_did, web_authority};

pub const PROFILE_SCHEMA: &str = "kiln.tangled-spindle.v1";
/// Encoded profiles larger than this are refused.
pub const MAXIMUM_PROFILE_BYTES: usize = 1_048_576;
/// Upper bound on pinned repositories in one profile.
pub const MAXIMUM_REPOSITORIES: usize = 256;
/// Upper bound on DIDs in one trusted-actor or approver list.
pub const MAXIMUM_ACTORS: usize = 256;
/// Upper bound on engines, images and packages declared per profile.
pub const MAXIMUM_ENGINE_ENTRIES: usize = 1024;
const MAXIMUM_TEXT_BYTES: usize = 4096;

/// Non-claims every admitted profile must carry verbatim.
pub const REQUIRED_NON_CLAIMS: [&str; 3] = [
    "Tangled admission proves a verified knot or actor signature, not source trust",
    "Tangled workflow files run only through the reviewed Kiln mapping",
    "Tangled status deliveries are Kiln projections, not Tangled records",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryPin {
    /// The repository's own DID, minted by its knot.
    pub repo_did: String,
    /// The `did:web` of the knot that hosts it; the only accepted push issuer.
    pub knot_did: String,
    /// HTTPS clone URL, normally `https://<knot host>/<repo DID>`.
    pub clone_url: String,
    /// DIDs with push access: they may trigger manual runs and vouch for
    /// same-repository pull requests.
    pub trusted_actors: Vec<String>,
    /// DIDs that may approve blocked pipelines and retry pipelines.
    pub approvers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineBinding {
    /// Tangled `engine` value, for example `microvm` or `nixery`.
    pub engine: String,
    /// `image` values this binding provides; empty means `image` must be absent.
    pub images: Vec<String>,
    /// `dependencies` this binding provides.
    pub packages: Vec<String>,
    /// Kiln platform alias for runs of this engine.
    pub platform: String,
    /// Kiln adapter kind that executes the workflow node.
    pub adapter_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KilnWorkflowBinding {
    pub policy_id: String,
    /// Policy for untrusted runs released by approval; grants no secrets.
    pub untrusted_policy_id: String,
    pub provider_profile_id: String,
    pub controls: WorkflowControls,
    pub bounds: WorkflowBounds,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusBinding {
    /// HTTPS endpoint receiving signed pipeline views.
    pub endpoint: String,
    /// File holding the HMAC secret (mode 0600 or 0400, owned by the service).
    pub secret_path: PathBuf,
    pub timeout_ms: u64,
    /// Deliveries of one view attempted before it is recorded as failed.
    pub maximum_attempts: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub maximum_body_bytes: usize,
    pub maximum_header_bytes: usize,
    pub read_timeout_ms: u64,
    pub write_timeout_ms: u64,
    /// Longest accepted service-auth token lifetime (Tangled spindles use 300).
    pub maximum_token_lifetime_seconds: i64,
    pub clock_skew_seconds: i64,
    pub maximum_tokens: usize,
    pub maximum_pipelines: usize,
    pub maximum_record_bytes: usize,
    pub maximum_workflow_files: usize,
    pub maximum_workflow_bytes: usize,
    pub maximum_changed_paths: usize,
    pub maximum_changed_path_bytes: usize,
    pub acquisition_timeout_ms: u64,
    pub resolver_timeout_ms: u64,
    pub maximum_did_document_bytes: usize,
    pub key_cache_seconds: i64,
    pub maximum_active_runs: usize,
    pub maximum_workflow_polls: usize,
    pub worker_poll_interval_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TangledProfile {
    pub schema: String,
    /// This service's `did:web`; the required token audience.
    pub spindle_did: String,
    pub listen_address: SocketAddr,
    pub database_path: PathBuf,
    /// Private object stores for exact acquisition.
    pub source_directory: PathBuf,
    pub git_program: PathBuf,
    /// HTTPS base URL of the `did:plc` directory.
    pub plc_directory_url: String,
    pub repositories: Vec<RepositoryPin>,
    pub engines: Vec<EngineBinding>,
    pub workflow: KilnWorkflowBinding,
    pub status: StatusBinding,
    pub limits: Limits,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedProfile {
    pub profile: TangledProfile,
    /// BLAKE3 identity of the canonical profile encoding.
    pub identity: ContentId,
}

impl AdmittedProfile {
    #[must_use]
    pub fn repository(&self, repo_did: &str) -> Option<&RepositoryPin> {
        self.profile
            .repositories
            .iter()
            .find(|pin| pin.repo_did == repo_did)
    }

    #[must_use]
    pub fn engine(&self, engine: &str) -> Option<&EngineBinding> {
        self.profile
            .engines
            .iter()
            .find(|binding| binding.engine == engine)
    }
}

fn invalid(code: &'static str, message: &'static str) -> Rejection {
    Rejection::invalid(code, message)
}

fn https_url(value: &str) -> bool {
    value.len() <= MAXIMUM_TEXT_BYTES
        && value.strip_prefix("https://").is_some_and(|rest| {
            let authority = rest.split('/').next().unwrap_or_default();
            !authority.is_empty()
                && !authority.contains('@')
                && !rest.contains(['?', '#'])
                && !rest.bytes().any(|byte| byte <= b' ' || byte == 127)
        })
}

fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_TEXT_BYTES && !value.chars().any(char::is_control)
}

fn did_list(values: &[String]) -> bool {
    values.len() <= MAXIMUM_ACTORS
        && values.iter().all(|did| valid_did(did))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

// r[impl kiln.tangled.profile] r[impl kiln.tangled.identity]
pub fn admit_profile(profile: TangledProfile) -> TangledResult<AdmittedProfile> {
    if profile.schema != PROFILE_SCHEMA {
        return Err(invalid(
            "tangled_profile_schema",
            "Tangled profile schema is not supported",
        ));
    }
    if web_authority(&profile.spindle_did).is_none() {
        return Err(invalid(
            "tangled_profile_spindle",
            "Spindle DID must be a did:web service DID",
        ));
    }
    if !https_url(&profile.plc_directory_url) {
        return Err(invalid(
            "tangled_profile_plc",
            "PLC directory must be an HTTPS URL",
        ));
    }
    for path in [
        &profile.database_path,
        &profile.source_directory,
        &profile.git_program,
    ] {
        if !path.is_absolute() {
            return Err(invalid(
                "tangled_profile_path",
                "Profile paths must be absolute",
            ));
        }
    }
    admit_repositories(&profile.repositories)?;
    admit_engines(&profile.engines)?;
    admit_workflow(&profile.workflow)?;
    if !https_url(&profile.status.endpoint) {
        return Err(invalid(
            "tangled_profile_status",
            "Status endpoint must be an HTTPS URL",
        ));
    }
    if !profile.status.secret_path.is_absolute()
        || profile.status.timeout_ms == 0
        || profile.status.maximum_attempts == 0
    {
        return Err(invalid(
            "tangled_profile_status",
            "Status binding is incomplete or unbounded",
        ));
    }
    admit_limits(&profile.limits)?;
    if profile
        .non_claims
        .iter()
        .map(String::as_str)
        .ne(REQUIRED_NON_CLAIMS)
    {
        return Err(invalid(
            "tangled_profile_non_claims",
            "Tangled profile non-claims differ from the reviewed set",
        ));
    }
    let (_, identity) = encode_and_identify(&(PROFILE_SCHEMA, &profile), MAXIMUM_PROFILE_BYTES)
        .map_err(|_| {
            invalid(
                "tangled_profile_identity",
                "Tangled profile exceeds its identity bound",
            )
        })?;
    Ok(AdmittedProfile { profile, identity })
}

fn admit_repositories(repositories: &[RepositoryPin]) -> TangledResult<()> {
    if repositories.is_empty() || repositories.len() > MAXIMUM_REPOSITORIES {
        return Err(invalid(
            "tangled_profile_repositories",
            "Profile must pin between one and the bounded number of repositories",
        ));
    }
    let mut seen = BTreeSet::new();
    for pin in repositories {
        if !valid_did(&pin.repo_did) || !seen.insert(pin.repo_did.as_str()) {
            return Err(invalid(
                "tangled_profile_repository",
                "Repository DIDs must be valid and unique",
            ));
        }
        if web_authority(&pin.knot_did).is_none() {
            return Err(invalid(
                "tangled_profile_knot",
                "Knot DID must be a did:web service DID",
            ));
        }
        if !https_url(&pin.clone_url) {
            return Err(invalid(
                "tangled_profile_clone_url",
                "Clone URL must be an HTTPS URL",
            ));
        }
        if !did_list(&pin.trusted_actors) || pin.approvers.is_empty() || !did_list(&pin.approvers) {
            return Err(invalid(
                "tangled_profile_actors",
                "Trusted actors and approvers must be unique DIDs, with at least one approver",
            ));
        }
    }
    Ok(())
}

fn admit_engines(engines: &[EngineBinding]) -> TangledResult<()> {
    let entries = engines
        .iter()
        .map(|binding| 1 + binding.images.len() + binding.packages.len())
        .sum::<usize>();
    if engines.is_empty() || entries > MAXIMUM_ENGINE_ENTRIES {
        return Err(invalid(
            "tangled_profile_engines",
            "Profile must bind at least one engine within bounds",
        ));
    }
    let mut seen = BTreeSet::new();
    for binding in engines {
        let names_ok = [&binding.engine]
            .into_iter()
            .chain(&binding.images)
            .chain(&binding.packages)
            .all(|value| text(value));
        if !names_ok
            || !seen.insert(binding.engine.as_str())
            || Alias::new(binding.platform.clone()).is_err()
            || Alias::new(binding.adapter_kind.clone()).is_err()
        {
            return Err(invalid(
                "tangled_profile_engine",
                "Engine bindings must be unique and well formed",
            ));
        }
    }
    Ok(())
}

fn admit_workflow(workflow: &KilnWorkflowBinding) -> TangledResult<()> {
    if PolicyId::new(workflow.policy_id.clone()).is_err()
        || PolicyId::new(workflow.untrusted_policy_id.clone()).is_err()
        || workflow.policy_id == workflow.untrusted_policy_id
        || ProviderProfileId::new(workflow.provider_profile_id.clone()).is_err()
        || workflow.bounds.max_nodes == 0
        || workflow.bounds.max_log_references == 0
        || workflow.bounds.max_log_read_bytes == 0
        || workflow.bounds.max_total_log_bytes == 0
    {
        return Err(invalid(
            "tangled_profile_workflow",
            "Kiln workflow binding is invalid; trusted and untrusted policies must differ",
        ));
    }
    Ok(())
}

fn admit_limits(limits: &Limits) -> TangledResult<()> {
    let sizes = [
        limits.maximum_body_bytes,
        limits.maximum_header_bytes,
        limits.maximum_tokens,
        limits.maximum_pipelines,
        limits.maximum_record_bytes,
        limits.maximum_workflow_files,
        limits.maximum_workflow_bytes,
        limits.maximum_changed_paths,
        limits.maximum_changed_path_bytes,
        limits.maximum_did_document_bytes,
        limits.maximum_active_runs,
        limits.maximum_workflow_polls,
    ];
    let times = [
        limits.read_timeout_ms,
        limits.write_timeout_ms,
        limits.acquisition_timeout_ms,
        limits.resolver_timeout_ms,
        limits.worker_poll_interval_ms,
    ];
    if sizes.contains(&0)
        || times.contains(&0)
        || limits.maximum_token_lifetime_seconds <= 0
        || limits.clock_skew_seconds < 0
        || limits.key_cache_seconds <= 0
    {
        return Err(invalid(
            "tangled_profile_limits",
            "Every Tangled limit must be positive",
        ));
    }
    Ok(())
}
