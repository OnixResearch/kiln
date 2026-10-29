use std::collections::{BTreeMap, BTreeSet};

use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_codec::encode_and_identify;
use kiln_core::types::ContentId;
use kiln_provider_api::WorkflowPort;
use serde::Serialize;

use crate::profile::{MAX_PROFILE_BYTES, ServiceProfile};
mod admission;
pub(crate) mod api;
mod artifacts;
mod automation;
mod cache;
mod dashboard;
mod logs;
pub(crate) mod model;
mod queue;
mod registry;
pub use registry::extend_projects;
mod reports;
pub use automation::{NotificationCompletion, NotificationWork};
mod snapshots;
mod storage;
mod telemetry;
mod worker_status;
pub use worker_status::WorkerStatusProfile;
mod change_review;
mod workers;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Operator,
    Worker,
}
#[derive(Clone, Debug)]
pub struct ApiResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub download_name: Option<String>,
}

pub struct Engine {
    profile: ServiceProfile,
    store: storage::Store,
    state: model::DurableState,
    ports: BTreeMap<String, Box<dyn WorkflowPort + Send>>,
    known: BTreeSet<String>,
    storage_failed: bool,
    automation_profile: Option<crate::automation::AutomationProfile>,
    cache_store: Option<crate::cache::CacheStore>,
    worker_status: Option<worker_status::Store>,
    _lock: storage::StateLock,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Engine")
            .field("service_id", &self.profile.service_id)
            .field("runs", &self.state.runs.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn identity<T: Serialize>(value: &T) -> Result<ContentId, String> {
    encode_and_identify(value, MAX_PROFILE_BYTES)
        .map(|(_, id)| id)
        .map_err(|_| "canonical identity exceeds its bound".into())
}

impl Engine {
    pub fn open(profile: ServiceProfile) -> Result<Self, String> {
        profile.admit()?;
        let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
        for project in &profile.projects {
            let lattice = project.lattice(profile.bounds.max_runs)?;
            let exchange = UnixWorkflowExchange::new(
                project.workflow.socket_path.clone(),
                project.transport()?,
                lattice.contract.clone(),
                profile.bounds.max_runs,
            )
            .map_err(|_| "workflow transport could not be admitted")?;
            let port = LatticeAdapter::new(lattice, exchange)
                .map_err(|_| "workflow adapter could not be admitted")?;
            ports.insert(project.id.clone(), Box::new(port));
        }
        Self::open_with_ports(profile, ports)
    }
    pub fn open_with_ports(
        profile: ServiceProfile,
        ports: BTreeMap<String, Box<dyn WorkflowPort + Send>>,
    ) -> Result<Self, String> {
        profile.admit()?;
        if ports.len() != profile.projects.len() {
            return Err("workflow port set differs from reviewed projects".into());
        }
        for project in &profile.projects {
            let expected = project.lattice(profile.bounds.max_runs)?.provider;
            if ports
                .get(&project.id)
                .is_none_or(|port| port.profile() != &expected)
            {
                return Err("workflow port capabilities differ from reviewed project".into());
            }
        }
        let (store, state, lock) = storage::Store::open(&profile)?;
        Ok(Self {
            profile,
            store,
            state,
            ports,
            known: BTreeSet::new(),
            storage_failed: false,
            automation_profile: None,
            cache_store: None,
            worker_status: None,
            _lock: lock,
        })
    }
    fn persist(&mut self) -> Result<(), String> {
        self.state.version = self
            .state
            .version
            .checked_add(1)
            .ok_or("live version exhausted")?;
        if let Err(error) = self.store.save(&self.state, &self.profile.bounds) {
            self.storage_failed = true;
            return Err(error);
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Vec<NotificationWork>, String> {
        if self.storage_failed {
            return Err("durable state is unavailable; restart requires recovery".into());
        }
        let ids: Vec<String> = self
            .state
            .runs
            .iter()
            .filter(|(_, r)| !r.outcome().is_terminal() && r.phase != "queued")
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.refresh(&id)?;
        }
        self.dispatch_queued()?;
        self.tick_automation()
    }
    // r[impl kiln.web.lifecycle]
    fn refresh(&mut self, id: &str) -> Result<(), String> {
        let record = self.state.runs.get(id).ok_or("run missing")?;
        if record.outcome().is_terminal() || record.phase == "queued" {
            return Ok(());
        }
        let plan = record.plan.clone();
        let project = record.project_id.clone();
        let previous = record.observation.clone();
        let unknown = record.phase == "unknown";
        let known = self.known.contains(id);
        let port = self.ports.get_mut(&project).ok_or("project port missing")?;
        let result = if !known {
            match port.recover_dispatch(&plan) {
                Ok(recovered) => {
                    if kiln_core::workflow::admit_workflow_observation(&plan, &recovered).is_err()
                        || previous
                            .as_ref()
                            .is_some_and(|v| v.execution != recovered.execution)
                    {
                        self.fail_unknown(id, "recovery_identity")?;
                        return Ok(());
                    }
                    self.known.insert(id.to_string());
                    port.observe(&plan.effect_id)
                }
                Err(e) => Err(e),
            }
        } else if unknown {
            port.reconcile(&plan.effect_id)
        } else {
            port.observe(&plan.effect_id)
        };
        self.accept(id, result)
    }
    fn fail_unknown(&mut self, id: &str, code: &str) -> Result<(), String> {
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        run.mark_unknown()?;
        run.phase = "unknown".into();
        run.error_code = Some(code.into());
        self.persist()
    }
    // r[impl kiln.web.deploy.cancellation] r[impl kiln.web.deploy.cancellation.scenario.race]
    // Outcomes come only from admitted observations; cancellation intent never rewrites completion.
    fn accept(
        &mut self,
        id: &str,
        result: kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation>,
    ) -> Result<(), String> {
        use kiln_core::workflow::{WorkflowState, admit_workflow_observation};
        match result {
            Err(failure) => {
                use kiln_provider_api::PortFailure;
                let code = match failure {
                    PortFailure::TimedOut(_) => "provider_timed_out",
                    PortFailure::Cancelled(_) => "provider_cancelled",
                    PortFailure::Unknown(_) => "provider_unknown",
                    PortFailure::Domain(_) => "provider_rejected",
                    PortFailure::Retryable(_) => "provider_retryable",
                    PortFailure::Infrastructure(_) => "provider_unavailable",
                };
                self.fail_unknown(id, code)
            }
            Ok(observation) => {
                let now = host_time_ms()?;
                let run = self.state.runs.get_mut(id).ok_or("run missing")?;
                let outcome = admit_workflow_observation(&run.plan, &observation);
                if outcome.is_err()
                    || run.observation.as_ref().is_some_and(|prior| {
                        prior.execution != observation.execution
                            || prior.sequence > observation.sequence
                            || (prior.sequence == observation.sequence && prior != &observation)
                    })
                {
                    return self.fail_unknown(id, "observation_conflict");
                }
                if run.observation.as_ref() == Some(&observation)
                    && run.error_code.is_none()
                    && matches!(run.phase.as_str(), "active" | "terminal" | "unknown")
                {
                    return Ok(());
                }
                if run.created_at_ms.is_some() {
                    if run.started_at_ms.is_none()
                        && matches!(
                            observation.state,
                            WorkflowState::Running | WorkflowState::Paused
                        )
                    {
                        run.started_at_ms = Some(now);
                    }
                    if observation.state.is_terminal() && run.completed_at_ms.is_none() {
                        run.completed_at_ms = Some(now);
                    }
                    for node in &observation.nodes {
                        let timing = run
                            .node_timing
                            .entry(node.node_id.as_str().to_owned())
                            .or_default();
                        if node.state == kiln_core::workflow::WorkflowNodeState::Running
                            && timing.started_at_ms.is_none()
                        {
                            timing.started_at_ms = Some(now);
                        }
                        if node.state.is_terminal() && timing.completed_at_ms.is_none() {
                            timing.completed_at_ms = Some(now);
                        }
                    }
                }
                if observation.state.is_terminal() {
                    run.complete(
                        outcome.map_err(|_| "observation rejected")?,
                        Some(observation.execution.clone()),
                    )?;
                    run.phase = "terminal".into();
                } else if observation.state == WorkflowState::Unknown {
                    run.mark_unknown()?;
                    run.phase = "unknown".into();
                } else {
                    run.phase = "active".into();
                }
                run.observation = Some(observation);
                run.error_code = None;
                self.known.insert(id.to_string());
                self.persist()
            }
        }
    }
}

pub(super) fn host_time_ms() -> Result<u64, String> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "host clock precedes epoch")?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "host clock exceeds bound".into())
}
