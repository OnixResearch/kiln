use std::collections::BTreeMap;
use std::str::FromStr;

use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::{Entry, TypeName};
use radicle_oid::Oid;
use serde::{Deserialize, Serialize};

use crate::status::cob::CobError;

use super::plan::{ABSOLUTE_MAX_LOG_URL_BYTES, admit_log_url};
use super::profile::JOB_TYPE_NAME;

/// Bound on runs one node may accumulate in one job object.
pub const MAX_RUNS_PER_NODE: usize = 64;
/// Bound on actions accepted from one entry.
pub const MAX_ACTIONS_PER_ENTRY: usize = 32;

fn cob_error(code: &'static str, message: &'static str) -> CobError {
    CobError {
        diagnostic: kiln_core::types::Diagnostic::new(code, "radicle_job.action", message),
    }
}

fn invalid_action() -> CobError {
    cob_error(
        "job_action_invalid",
        "job entry contained an action outside the pinned xyz.radworks.job schema",
    )
}

/// `xyz.radworks.job` action, wire-compatible with `radicle-job 0.6.0`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub enum JobAction {
    Request { oid: Oid },
    Run { uuid: String, log: String },
    Finished { uuid: String, reason: JobReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum JobReason {
    Failed,
    Succeeded,
}

impl JobReason {
    #[must_use]
    pub const fn from_succeeded(succeeded: bool) -> Self {
        if succeeded {
            Self::Succeeded
        } else {
            Self::Failed
        }
    }

    #[must_use]
    pub const fn succeeded(self) -> bool {
        matches!(self, Self::Succeeded)
    }
}

/// One run under one node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRun {
    pub log: String,
    pub finished: Option<JobReason>,
    pub timestamp: u64,
}

// r[impl kiln.radicle.job]
/// Reduced `xyz.radworks.job` state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobCob {
    oid: Oid,
    runs: BTreeMap<String, BTreeMap<String, JobRun>>,
}

impl JobCob {
    #[must_use]
    pub const fn oid(&self) -> Oid {
        self.oid
    }

    #[must_use]
    pub const fn runs(&self) -> &BTreeMap<String, BTreeMap<String, JobRun>> {
        &self.runs
    }

    #[must_use]
    pub fn run(&self, author: &str, uuid: &str) -> Option<&JobRun> {
        self.runs.get(author).and_then(|runs| runs.get(uuid))
    }

    #[must_use]
    pub fn type_name_static() -> &'static TypeName {
        &JOB_TYPE
    }

    fn apply_action(&mut self, entry: &Entry, action: JobAction) -> Result<(), CobError> {
        match action {
            JobAction::Request { oid } => {
                if oid != self.oid {
                    return Err(cob_error(
                        "job_commit_mismatch",
                        "job object history requests more than one commit",
                    ));
                }
                Ok(())
            }
            JobAction::Run { uuid, log } => {
                admit_action_uuid(&uuid)?;
                admit_log_url(&log, ABSOLUTE_MAX_LOG_URL_BYTES)
                    .map_err(|diagnostic| CobError { diagnostic })?;
                let author = entry.author().to_string();
                let runs = self.runs.entry(author).or_default();
                if runs.contains_key(&uuid) {
                    return Err(cob_error(
                        "job_run_duplicate",
                        "job entry started the same run UUID twice for one node",
                    ));
                }
                if runs.len() >= MAX_RUNS_PER_NODE {
                    return Err(cob_error(
                        "job_run_bound",
                        "job object exceeded the named per-node run bound",
                    ));
                }
                runs.insert(
                    uuid,
                    JobRun {
                        log,
                        finished: None,
                        timestamp: entry.timestamp,
                    },
                );
                Ok(())
            }
            JobAction::Finished { uuid, reason } => {
                admit_action_uuid(&uuid)?;
                let author = entry.author().to_string();
                let run = self
                    .runs
                    .get_mut(&author)
                    .and_then(|runs| runs.get_mut(&uuid))
                    .ok_or_else(|| {
                        cob_error(
                            "job_finish_without_start",
                            "job entry finished a run that this node did not start",
                        )
                    })?;
                if run.finished.replace(reason).is_some() {
                    return Err(cob_error(
                        "job_finish_duplicate",
                        "job entry finished the same run UUID twice",
                    ));
                }
                run.timestamp = entry.timestamp;
                Ok(())
            }
        }
    }
}

fn admit_action_uuid(uuid: &str) -> Result<(), CobError> {
    if uuid.len() != 36 || uuid::Uuid::parse_str(uuid).is_err() {
        return Err(cob_error(
            "job_run_identity_invalid",
            "job action UUID is not a canonical UUID",
        ));
    }
    Ok(())
}

fn decode_actions(entry: &Entry) -> Result<Vec<JobAction>, CobError> {
    if entry.contents().len() > MAX_ACTIONS_PER_ENTRY {
        return Err(cob_error(
            "job_action_bound",
            "job entry contained more actions than the named bound",
        ));
    }
    entry
        .contents()
        .iter()
        .map(|segment| serde_json::from_slice(segment).map_err(|_| invalid_action()))
        .collect()
}

impl radicle_cob::Evaluate<GitCobStore<'_>> for JobCob {
    type Error = CobError;

    fn init(entry: &Entry, _store: &GitCobStore<'_>) -> Result<Self, Self::Error> {
        let mut actions = decode_actions(entry)?.into_iter();
        let root = actions.next().ok_or_else(invalid_action)?;
        let JobAction::Request { oid } = root else {
            return Err(cob_error(
                "job_root_invalid",
                "job object root action is not a Request",
            ));
        };
        let mut job = Self {
            oid,
            runs: BTreeMap::new(),
        };
        for action in actions {
            job.apply_action(entry, action)?;
        }
        Ok(job)
    }

    fn apply<'a, I: Iterator<Item = (&'a Oid, &'a Entry)>>(
        &mut self,
        entry: &Entry,
        _concurrent: I,
        _store: &GitCobStore<'_>,
    ) -> Result<(), Self::Error> {
        for action in decode_actions(entry)? {
            self.apply_action(entry, action)?;
        }
        Ok(())
    }
}

static JOB_TYPE: std::sync::LazyLock<TypeName> = std::sync::LazyLock::new(|| {
    TypeName::from_str(JOB_TYPE_NAME).unwrap_or_else(|_| unreachable!("pinned type name parses"))
});
