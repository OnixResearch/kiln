use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_adapter_aspen::{
    AdmittedAspenRuntimeProfile, AspenRuntimeProfileDocument, AspenServiceDecision,
    AspenServiceTransport, ServiceTransportOutcome, UnixAspenServiceTransport, admit_profile,
};
use kiln_adapter_radicle::source::RadicleSourceShell;
use kiln_adapter_radicle::status::{RadicleStatusProfile, RadicleStatusShell, StatusDispatch};
use kiln_core::ContentId;
use kiln_core::domain::State;
use kiln_onixos_pilot::{JournalLock, MAX_CONFIG_BYTES, load_json};
use kiln_provider_api::TriggerRequest;
use serde::{Deserialize, Serialize};

use crate::journal::{CiJournal, admit_native, terminal_outcome};
use crate::project::{
    PROJECT_RESULT_FILE, ProjectBindings, ProjectSourceIntent, ProjectWorkerProfile,
    ProjectWorkerResult, SOURCE_INTENT_FILE, SOURCE_INTENT_SCHEMA, SourceContext, admit_result,
};
use crate::{Result, storage};

pub const PROFILE_SCHEMA: &str = "kiln.radicle-ci.profile.v1";
pub const JOURNAL_FILE: &str = "ci-journal.json";
const MAXIMUM_OBSERVATION_MILLISECONDS: u64 = 600_000;
const MAXIMUM_JOURNAL_RECORDS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiProfile {
    pub schema: String,
    pub worker_profile_path: PathBuf,
    pub worker_profile_identity: ContentId,
    pub runtime_profile: AspenRuntimeProfileDocument,
    pub socket_path: PathBuf,
    pub maximum_frame_bytes: usize,
    pub transport_timeout_seconds: u64,
    pub max_journal_bytes: usize,
    pub max_journal_records: usize,
    pub maximum_polls: usize,
    pub poll_interval_milliseconds: u64,
    pub status_storage_root: PathBuf,
    pub status_profile: RadicleStatusProfile,
}

impl CiProfile {
    // r[impl kiln.radicle_ci.runtime] r[impl kiln.radicle_ci.evidence]
    pub fn admit(&self) -> Result<AdmittedAspenRuntimeProfile> {
        if self.schema != PROFILE_SCHEMA
            || !self.worker_profile_path.is_absolute()
            || !self.socket_path.is_absolute()
            || !self.status_storage_root.is_absolute()
            || self.maximum_frame_bytes == 0
            || self.maximum_frame_bytes > MAX_CONFIG_BYTES
            || self.transport_timeout_seconds == 0
            || self.transport_timeout_seconds > MAXIMUM_OBSERVATION_MILLISECONDS / 1000
            || self.max_journal_bytes == 0
            || self.max_journal_bytes > MAX_CONFIG_BYTES
            || self.max_journal_records < 2
            || self.max_journal_records > MAXIMUM_JOURNAL_RECORDS
            || self.maximum_polls == 0
            || self.maximum_polls >= self.max_journal_records
            || self.poll_interval_milliseconds == 0
        {
            return Err("invalid Radicle CI profile schema, paths or finite bounds".into());
        }
        let polls = u64::try_from(self.maximum_polls).map_err(text)?;
        if self
            .poll_interval_milliseconds
            .checked_mul(polls)
            .ok_or("observation duration overflow")?
            > MAXIMUM_OBSERVATION_MILLISECONDS
        {
            return Err("Radicle CI observation duration exceeds its named bound".into());
        }
        let runtime = self
            .runtime_profile
            .clone()
            .into_runtime_profile()
            .map_err(text)?;
        admit_profile(&runtime).map_err(text)
    }
}

#[derive(Debug)]
pub struct CiService {
    profile: CiProfile,
    worker: ProjectWorkerProfile,
    runtime: AdmittedAspenRuntimeProfile,
    status: RadicleStatusShell,
}

impl CiService {
    pub fn new(profile: CiProfile) -> Result<Self> {
        let status = RadicleStatusShell::new(
            profile.status_storage_root.clone(),
            profile.status_profile.clone(),
        )
        .map_err(text)?;
        Self::with_status(profile, status)
    }

    #[cfg(feature = "native-fixtures")]
    pub fn new_fixture(profile: CiProfile) -> Result<Self> {
        let status = RadicleStatusShell::new_fixture(
            profile.status_storage_root.clone(),
            profile.status_profile.clone(),
        )
        .map_err(text)?;
        Self::with_status(profile, status)
    }

    fn with_status(profile: CiProfile, status: RadicleStatusShell) -> Result<Self> {
        let runtime = profile.admit()?;
        let worker = storage::load_worker(
            &profile.worker_profile_path,
            &profile.worker_profile_identity,
        )?;
        if !profile
            .status_profile
            .allowed_rids
            .iter()
            .any(|rid| rid == &worker.source.repository_id)
        {
            return Err(
                "dedicated status profile does not admit the configured source repository".into(),
            );
        }
        Ok(Self {
            profile,
            worker,
            runtime,
            status,
        })
    }

    pub const fn profile(&self) -> &CiProfile {
        &self.profile
    }
    pub const fn worker_profile(&self) -> &ProjectWorkerProfile {
        &self.worker
    }

    pub fn source_context(&self, trigger: &TriggerRequest) -> Result<SourceContext> {
        let source = SourceContext {
            provider: trigger.repository.provider_namespace.as_str().to_owned(),
            repository: trigger.repository.native_id.as_str().to_owned(),
            revision: trigger.after.native_id.as_str().to_owned(),
        };
        if self.worker.trigger(&source)? != *trigger {
            return Err("broker event is not the configured exact repository/branch push".into());
        }
        Ok(source)
    }

    fn bindings(&self, source: &SourceContext) -> Result<ProjectBindings> {
        self.worker
            .bindings(&self.profile.worker_profile_identity, source)
    }

    fn client(&self) -> Result<UnixAspenServiceTransport> {
        UnixAspenServiceTransport::new(
            self.profile.socket_path.clone(),
            self.profile.maximum_frame_bytes,
            Duration::from_secs(self.profile.transport_timeout_seconds),
        )
        .map_err(str::to_owned)
    }

    // r[impl kiln.radicle_ci.source] r[impl kiln.radicle_ci.journal]
    pub fn accept(&self, trigger: TriggerRequest) -> Result<CiJournal> {
        let source = self.source_context(&trigger)?;
        let bindings = self.bindings(&source)?;
        storage::prepare_candidate(&bindings.root)?;
        let path = bindings.root.join(JOURNAL_FILE);
        let lock = JournalLock::acquire(&path)?;
        if path.try_exists().map_err(text)? {
            let journal = self.load_journal(&path)?;
            if journal.trigger != trigger {
                return Err("candidate reserves another broker trigger".into());
            }
            return Ok(journal);
        }
        let source_path = bindings.root.join(SOURCE_INTENT_FILE);
        if source_path.try_exists().map_err(text)? {
            return Err("orphan source intent lacks canonical start reservation".into());
        }
        let pinned_profile = self.worker.source.pin(&trigger.after).map_err(text)?;
        let resolution = RadicleSourceShell::new(pinned_profile.clone())
            .map_err(text)?
            .resolve_for_materialization()
            .map_err(text)?;
        let intent = ProjectSourceIntent {
            schema: SOURCE_INTENT_SCHEMA.into(),
            profile_identity: self.profile.worker_profile_identity.clone(),
            source,
            pinned_profile,
            resolution,
        };
        let mut journal =
            CiJournal::fresh(&self.profile, &self.worker, &self.runtime, trigger, intent)?;
        let mut client = self.client()?;
        lock.store(&journal, self.profile.max_journal_bytes)?;
        storage::publish_source_intent(&source_path, &journal.source_intent)?;
        let native = client.submit(&self.runtime, &journal.ingress);
        self.record_native(&mut journal, native)?;
        journal.seal()?;
        lock.store(&journal, self.profile.max_journal_bytes)?;
        journal.validate(&self.profile, &self.worker, &self.runtime)?;
        Ok(journal)
    }

    // r[impl kiln.radicle_ci.recovery] r[impl kiln.radicle_ci.status]
    pub fn reconcile(&self, source: &SourceContext) -> Result<CiJournal> {
        let bindings = self.bindings(source)?;
        let path = bindings.root.join(JOURNAL_FILE);
        let lock = JournalLock::acquire(&path)?;
        let mut journal = self.load_journal(&path)?;
        let state = journal.validate(&self.profile, &self.worker, &self.runtime)?;
        if terminal_outcome(&state, &bindings.attempt_id)?.is_none() {
            storage::publish_source_intent(
                &bindings.root.join(SOURCE_INTENT_FILE),
                &journal.source_intent,
            )?;
            let native = self
                .client()?
                .reconcile(&self.runtime, &journal.ingress.operation_id);
            self.record_native(&mut journal, native)?;
            self.load_completed_worker(&mut journal, &bindings)?;
            let admission = self.admit_completion(&mut journal);
            journal.seal()?;
            lock.store(&journal, self.profile.max_journal_bytes)?;
            admission?;
        }
        crate::status::drive(
            &self.profile,
            &self.worker,
            &self.status,
            &lock,
            &mut journal,
        )?;
        journal.validate(&self.profile, &self.worker, &self.runtime)?;
        Ok(journal)
    }

    pub fn replay(&self, source: &SourceContext) -> Result<State> {
        Ok(self.load_replayed(source)?.1)
    }

    pub fn load_replayed(&self, source: &SourceContext) -> Result<(CiJournal, State)> {
        let bindings = self.bindings(source)?;
        let path = bindings.root.join(JOURNAL_FILE);
        let _lock = JournalLock::acquire(&path)?;
        let journal: CiJournal = load_json(&path, self.profile.max_journal_bytes)?;
        let state = journal.validate(&self.profile, &self.worker, &self.runtime)?;
        Ok((journal, state))
    }

    pub fn observe(&self, source: &SourceContext) -> Result<CiJournal> {
        for poll in 0..self.profile.maximum_polls {
            let journal = self.reconcile(source)?;
            if matches!(journal.status_observation, Some(StatusDispatch::Written(_))) {
                return Ok(journal);
            }
            let state = journal.validate(&self.profile, &self.worker, &self.runtime)?;
            let bindings = self.bindings(source)?;
            if terminal_outcome(&state, &bindings.attempt_id)?.is_some() {
                return Ok(journal);
            }
            if poll + 1 < self.profile.maximum_polls {
                std::thread::sleep(Duration::from_millis(
                    self.profile.poll_interval_milliseconds,
                ));
            }
        }
        let bindings = self.bindings(source)?;
        let path = bindings.root.join(JOURNAL_FILE);
        let _lock = JournalLock::acquire(&path)?;
        self.load_journal(&path)
    }

    fn load_journal(&self, path: &Path) -> Result<CiJournal> {
        let journal: CiJournal = load_json(path, self.profile.max_journal_bytes)?;
        journal.validate(&self.profile, &self.worker, &self.runtime)?;
        Ok(journal)
    }

    fn record_native(
        &self,
        journal: &mut CiJournal,
        native: ServiceTransportOutcome,
    ) -> Result<()> {
        let bindings = self.bindings(&journal.source)?;
        admit_native(&journal.ingress, &bindings.run_id, &native)?;
        // Preserve the original submit reply, latest progress and first receipt of
        // each terminal decision/variant. Polling must not exhaust recovery.
        if let Some(kind) = native_terminal_kind(&native) {
            if journal
                .native_observations
                .iter()
                .any(|prior| native_terminal_kind(prior) == Some(kind))
            {
                return Ok(());
            }
            if let Some(index) = journal
                .native_observations
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(index, prior)| (!native_terminal(prior)).then_some(index))
            {
                journal.native_observations.remove(index);
            }
        } else if let Some(progress) = journal
            .native_observations
            .iter_mut()
            .skip(1)
            .find(|prior| !native_terminal(prior))
        {
            *progress = native;
            return Ok(());
        }
        if journal.native_observations.len() >= self.profile.max_journal_records {
            return Err("native observation bound reached; no redispatch is authorized".into());
        }
        journal.native_observations.push(native);
        Ok(())
    }

    fn load_completed_worker(
        &self,
        journal: &mut CiJournal,
        bindings: &ProjectBindings,
    ) -> Result<()> {
        if journal.worker_result.is_some()
            || !journal.native_observations.iter().any(native_terminal)
        {
            return Ok(());
        }
        let path = bindings.root.join(PROJECT_RESULT_FILE);
        if path.try_exists().map_err(text)? {
            let result = load_json::<ProjectWorkerResult>(&path, self.profile.max_journal_bytes)?;
            admit_result(
                &self.worker,
                &self.profile.worker_profile_identity,
                &journal.trigger,
                &result,
            )?;
            if result.source_intent != journal.source_intent {
                return Err("worker changed the reserved signed source".into());
            }
            journal.worker_result = Some(result);
        }
        Ok(())
    }

    fn admit_completion(&self, journal: &mut CiJournal) -> Result<()> {
        if let Some(command) = journal.completion(&self.profile, &self.worker)? {
            journal.append(&self.profile, command, "complete")?;
        }
        Ok(())
    }
}

fn native_terminal_kind(native: &ServiceTransportOutcome) -> Option<(bool, AspenServiceDecision)> {
    let (rejected, ack) = match native {
        ServiceTransportOutcome::Acknowledged(ack) => (false, ack),
        ServiceTransportOutcome::Rejected(ack) => (true, ack),
        _ => return None,
    };
    matches!(
        ack.decision,
        AspenServiceDecision::Completed
            | AspenServiceDecision::Failed
            | AspenServiceDecision::Denied
            | AspenServiceDecision::Rejected
            | AspenServiceDecision::Cancelled
            | AspenServiceDecision::TimedOut
    )
    .then_some((rejected, ack.decision))
}

fn native_terminal(native: &ServiceTransportOutcome) -> bool {
    native_terminal_kind(native).is_some()
}

fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;
