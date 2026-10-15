use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use kiln_core::types::{CoreResult, Diagnostic};
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::{Entry, TypeName};
use radicle_oid::Oid;

use super::plan::{StatusOutcome, StatusPlan};

/// Reducer error carrying a Kiln diagnostic through `radicle-cob::Evaluate`.
#[derive(Debug)]
pub struct CobError {
    pub diagnostic: Diagnostic,
}

impl fmt::Display for CobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.diagnostic.message)
    }
}

impl std::error::Error for CobError {}

impl From<CobError> for Diagnostic {
    fn from(error: CobError) -> Self {
        error.diagnostic
    }
}

fn cob_error(code: &'static str) -> CobError {
    CobError {
        diagnostic: Diagnostic::new(
            code,
            "radicle_status.action",
            "ci-job entry contained an action outside the pinned schema",
        ),
    }
}

/// `xyz.radicle.ci-job` typed COB state.
///
/// The state is a bounded map of idempotency keys to the latest projected
/// outcome for that key. Repeats of one key overwrite the same entry, which
/// makes writes idempotent and the projection rebuildable from history.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiJobStatus {
    pub runs: BTreeMap<String, ProjectedOutcome>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedOutcome {
    pub run_id: String,
    pub outcome: StatusOutcome,
}

/// One admitted action. The serialized action is the whole change payload.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CiJobAction {
    /// Project or replace the outcome bound to one idempotency key.
    Project {
        key: String,
        run_id: String,
        outcome: StatusOutcome,
    },
}

// r[impl kiln.radicle.status_allowlist]
pub fn ci_job_type_name() -> CoreResult<TypeName> {
    TypeName::from_str(super::profile::CI_JOB_TYPE_NAME).map_err(|_| {
        Diagnostic::new(
            "radicle_status_type_name",
            "radicle_status.object_type",
            "The pinned ci-job type name failed to parse",
        )
    })
}

const MAX_ACTION_KEY_BYTES: usize = 256;

fn admit_action_key(key: &str) -> CoreResult<()> {
    if key.is_empty() || key.len() > MAX_ACTION_KEY_BYTES {
        return Err(Diagnostic::new(
            "radicle_status_payload_bound",
            "radicle_status.action.key",
            "ci-job action key is empty or exceeds its named bound",
        ));
    }
    Ok(())
}

impl radicle_cob::Evaluate<GitCobStore<'_>> for CiJobStatus {
    type Error = CobError;

    fn init(entry: &Entry, _store: &GitCobStore<'_>) -> Result<Self, Self::Error> {
        let mut state = Self::default();
        state.apply_entry(entry)?;
        Ok(state)
    }

    fn apply<'a, I: Iterator<Item = (&'a Oid, &'a Entry)>>(
        &mut self,
        entry: &Entry,
        _concurrent: I,
        _store: &GitCobStore<'_>,
    ) -> Result<(), Self::Error> {
        self.apply_entry(entry)
    }
}

impl CiJobStatus {
    fn apply_entry(&mut self, entry: &Entry) -> Result<(), CobError> {
        let bytes: Vec<u8> = entry
            .contents()
            .iter()
            .flat_map(|segment| segment.iter())
            .copied()
            .collect();
        let action: CiJobAction = serde_json::from_slice(&bytes)
            .map_err(|_| cob_error("radicle_status_action_invalid"))?;
        match action {
            CiJobAction::Project {
                key,
                run_id,
                outcome,
            } => {
                admit_action_key(&key).map_err(|diagnostic| CobError { diagnostic })?;
                if run_id.is_empty() {
                    return Err(cob_error("radicle_status_run_identity_invalid"));
                }
                self.runs.insert(key, ProjectedOutcome { run_id, outcome });
                Ok(())
            }
        }
    }

    // r[impl kiln.radicle.shell_status.scenario.success]
    #[must_use]
    pub fn projected(&self, plan: &StatusPlan) -> Option<&ProjectedOutcome> {
        self.runs.get(plan.idempotency_key.native_id.as_str())
    }

    #[must_use]
    pub fn type_name_static() -> &'static TypeName {
        &CI_JOB_TYPE
    }
}

static CI_JOB_TYPE: std::sync::LazyLock<TypeName> = std::sync::LazyLock::new(|| {
    ci_job_type_name().unwrap_or_else(|_| unreachable!("pinned type name must parse"))
});
