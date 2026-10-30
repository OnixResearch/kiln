use std::collections::BTreeSet;

use kiln_core::domain::Outcome;
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef, RunId};
use serde::{Deserialize, Serialize};

use super::reducer::{JobAction, JobReason, JobRun};
use crate::status::plan::admit_revision;

/// Default bound on serialized job actions admitted for one plan.
pub const DEFAULT_MAX_JOB_PAYLOAD_BYTES: usize = super::profile::DEFAULT_MAX_JOB_PAYLOAD_BYTES;
/// Default bound on one job log URL.
pub const DEFAULT_MAX_LOG_URL_BYTES: usize = super::profile::DEFAULT_MAX_LOG_URL_BYTES;
const ABSOLUTE_MAX_JOB_PAYLOAD_BYTES: usize = 1_048_576;
pub(crate) const ABSOLUTE_MAX_LOG_URL_BYTES: usize = 8_192;

/// The desired state of one Kiln attempt in the job object.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JobState {
    Started,
    Finished { succeeded: bool },
}

impl JobState {
    #[must_use]
    pub const fn succeeded(self) -> Option<bool> {
        match self {
            Self::Started => None,
            Self::Finished { succeeded } => Some(succeeded),
        }
    }
}

// r[impl kiln.radicle.job]
// r[impl kiln.radicle.job.identity]
/// One admitted Radicle job publication plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobPlan {
    /// The admitted repository identity (`rad:<RID>`).
    pub repository: ExternalRef,
    /// Exact source commit described by this run.
    pub revision: ExternalRef,
    /// Exact job object identity when the plan does not rely on discovery.
    pub object_id: Option<ExternalRef>,
    /// The Kiln attempt this run describes.
    pub run_id: RunId,
    /// Bounded log URL for this run.
    pub log_url: String,
    /// Desired state; starting a finished run fails.
    pub state: JobState,
    /// Bound on serialized actions admitted for one publication.
    pub max_payload_bytes: usize,
    /// Bound on the log URL accepted by this plan.
    pub max_log_url_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobObservation {
    pub repository: ExternalRef,
    pub revision: ExternalRef,
    pub object_id: ExternalRef,
    pub head: ExternalRef,
    pub run_uuid: String,
    pub run_id: String,
    pub state: JobState,
    pub signer_identity: ExternalRef,
}

/// A write may have reached storage even when its final result is unavailable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobUnknownObservation {
    pub repository: ExternalRef,
    pub revision: ExternalRef,
    pub object_id: Option<ExternalRef>,
    pub run_uuid: String,
    pub run_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    tag = "disposition",
    content = "observation"
)]
pub enum JobDispatch {
    Written(JobObservation),
    Unknown(JobUnknownObservation),
}

/// Map a terminal Kiln outcome to a finished job state.
pub fn classify_job_outcome(outcome: Outcome) -> CoreResult<JobState> {
    match outcome {
        Outcome::Success => Ok(JobState::Finished { succeeded: true }),
        Outcome::DomainFailure
        | Outcome::Denied
        | Outcome::Cancelled
        | Outcome::TimedOut
        | Outcome::ProviderError => Ok(JobState::Finished { succeeded: false }),
        Outcome::Pending | Outcome::Unknown => Err(Diagnostic::new(
            "job_outcome_unknown",
            "job_plan.state",
            "Non-terminal or unknown Kiln outcome cannot be published as a finished job",
        )),
    }
}

/// Deterministic UUID for one Kiln attempt in one repository revision.
#[must_use]
pub fn run_uuid(repository: &ExternalRef, revision: &ExternalRef, run_id: &RunId) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln-radicle-job-v1\0");
    hasher.update(repository.native_id.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(revision.native_id.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(run_id.as_str().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    uuid::Uuid::from_bytes(bytes).to_string()
}

// r[impl kiln.radicle.job.bounds]
pub fn admit_job_plan(plan: &JobPlan, allowed_rids: &BTreeSet<String>) -> CoreResult<()> {
    if plan.repository.provider_namespace.as_str() != "radicle"
        || plan.repository.native_kind.as_str() != "repository"
        || plan.repository.native_id.as_str().is_empty()
    {
        return Err(Diagnostic::new(
            "job_repository_identity",
            "job_plan.repository",
            "Radicle job plan names an invalid repository identity",
        ));
    }
    if !allowed_rids.contains(plan.repository.native_id.as_str()) {
        return Err(Diagnostic::new(
            "job_repository_not_admitted",
            "job_plan.repository",
            "Radicle job plan targets a repository outside its allowlist",
        ));
    }
    admit_revision(&plan.revision)?;
    if let Some(object_id) = &plan.object_id {
        if object_id.provider_namespace.as_str() != "radicle"
            || object_id.native_kind.as_str() != "cob"
            || object_id.native_id.as_str().is_empty()
        {
            return Err(Diagnostic::new(
                "job_object_identity",
                "job_plan.object_id",
                "Radicle job plan names an invalid COB identity",
            ));
        }
        object_id.validate()?;
    }
    if plan.max_payload_bytes == 0 || plan.max_payload_bytes > ABSOLUTE_MAX_JOB_PAYLOAD_BYTES {
        return Err(Diagnostic::new(
            "job_payload_bound",
            "job_plan.max_payload_bytes",
            "Radicle job payload bound is zero or exceeds its absolute maximum",
        ));
    }
    if plan.max_log_url_bytes == 0 || plan.max_log_url_bytes > ABSOLUTE_MAX_LOG_URL_BYTES {
        return Err(Diagnostic::new(
            "job_log_url_bound",
            "job_plan.max_log_url_bytes",
            "Radicle job log URL bound is zero or exceeds its absolute maximum",
        ));
    }
    admit_log_url(&plan.log_url, plan.max_log_url_bytes)
}

pub fn admit_log_url(url: &str, maximum: usize) -> CoreResult<()> {
    let invalid = || {
        Diagnostic::new(
            "job_log_url_invalid",
            "job_plan.log_url",
            "Radicle job log URL must be a bounded http or https URL with a host",
        )
    };
    if url.is_empty()
        || url.len() > maximum
        || url
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(invalid());
    }
    let authority = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(invalid)?;
    let host = authority.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() || host.starts_with(':') || host.contains('@') {
        return Err(invalid());
    }
    Ok(())
}

// r[impl kiln.radicle.job.replay]
/// Actions needed to move an observed run to the plan's desired state.
///
/// An empty vector is an idempotent replay.
pub fn required_actions(plan: &JobPlan, existing: Option<&JobRun>) -> CoreResult<Vec<JobAction>> {
    let uuid = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
    let desired = plan.state;
    match (existing, desired) {
        (None, JobState::Started) => Ok(vec![JobAction::Run {
            uuid,
            log: plan.log_url.clone(),
        }]),
        (None, JobState::Finished { succeeded }) => Ok(vec![
            JobAction::Run {
                uuid: uuid.clone(),
                log: plan.log_url.clone(),
            },
            JobAction::Finished {
                uuid,
                reason: JobReason::from_succeeded(succeeded),
            },
        ]),
        (Some(run), _) if run.log != plan.log_url => Err(Diagnostic::new(
            "job_replay_log_conflict",
            "job_plan.log_url",
            "The deterministic run UUID is already bound to a different log URL",
        )),
        (Some(run), JobState::Started) => match run.finished {
            Some(_) => Err(Diagnostic::new(
                "job_replay_start_after_finish",
                "job_plan.state",
                "A finished job run cannot be restarted with the same UUID",
            )),
            None => Ok(Vec::new()),
        },
        (Some(run), JobState::Finished { succeeded }) => match run.finished {
            None => Ok(vec![JobAction::Finished {
                uuid,
                reason: JobReason::from_succeeded(succeeded),
            }]),
            Some(existing) if existing == JobReason::from_succeeded(succeeded) => Ok(Vec::new()),
            Some(_) => Err(Diagnostic::new(
                "job_replay_reason_conflict",
                "job_plan.state",
                "The deterministic run UUID is already finished with another reason",
            )),
        },
    }
}
