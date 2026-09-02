use std::collections::BTreeSet;

use kiln_codec::encode_and_identify;
use kiln_core::domain::{Command, CommandEnvelope, Event, Outcome, State, decide, evolve};
use kiln_core::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CandidateId, CommandId, ContentId, EffectId,
    ExternalRef, Limits, PipelineRevisionId, PolicyId, ProviderProfileId, RunId,
};
use kiln_core::workflow::{
    WorkflowEdge, WorkflowEffectPlan, WorkflowNode, WorkflowObservation, WorkflowSourceContext,
    WorkflowState, admit_workflow_observation, validate_workflow_plan,
};
use kiln_provider_api::{PortFailure, PortResult, WorkflowPort};
use serde::{Deserialize, Serialize};

use crate::domain;
use crate::events::VerifiedEvent;
use crate::profile::{AdmittedGithubConfig, GithubConfig, MAXIMUM_PROFILE_BYTES, WorkflowConfig};
use crate::replay::SqliteStore;
use crate::status::{Publication, StatusPlan, StatusPort, StatusState, plan_status};

pub const RUN_SCHEMA: &str = "kiln.github-run.v1";
const MAXIMUM_CI_EVENTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Queued,
    DispatchReserved,
    Active,
    Unknown,
    Unresolved,
    Terminal,
}

impl RunPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::DispatchReserved => "dispatch_reserved",
            Self::Active => "active",
            Self::Unknown => "unknown",
            Self::Unresolved => "unresolved",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPhase {
    Ready,
    Reserved,
    Published,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusRecord {
    pub plan: StatusPlan,
    pub phase: PublicationPhase,
    pub publication: Option<Publication>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub schema: String,
    pub run_identity: ContentId,
    pub immutable_identity: ContentId,
    pub profile_identity: ContentId,
    pub plan: WorkflowEffectPlan,
    pub run_id: RunId,
    pub phase: RunPhase,
    pub workflow_polls: usize,
    pub observation: Option<WorkflowObservation>,
    pub events: Vec<Event>,
    pub statuses: Vec<StatusRecord>,
    pub error_code: Option<String>,
    #[serde(skip)]
    pub version: u64,
    #[serde(skip)]
    pub ci_state: State,
}

impl RunRecord {
    pub fn outcome(&self) -> PortResult<Outcome> {
        self.ci_state
            .runs
            .get(&self.run_id)
            .map(|run| run.outcome)
            .ok_or_else(|| {
                domain(
                    "github_run_state",
                    "github.run",
                    "GitHub run has no canonical CI state",
                )
            })
    }

    pub fn has_work(&self) -> bool {
        !matches!(self.phase, RunPhase::Terminal | RunPhase::Unresolved)
            || self.statuses.iter().any(|status| {
                matches!(
                    status.phase,
                    PublicationPhase::Ready | PublicationPhase::Reserved
                )
            })
    }

    pub fn restore_state(&mut self) -> PortResult<()> {
        if self.schema != RUN_SCHEMA
            || self.events.is_empty()
            || self.events.len() > MAXIMUM_CI_EVENTS
            || self.statuses.is_empty()
            || self.statuses.len() > MAXIMUM_CI_EVENTS
        {
            return Err(domain(
                "github_run_record",
                "github.run",
                "GitHub durable run has an invalid schema or bounded history",
            ));
        }
        validate_workflow_plan(&self.plan)?;
        let expected = immutable_identity(
            &self.run_identity,
            &self.profile_identity,
            &self.plan,
            &self.run_id,
        )?;
        if expected != self.immutable_identity {
            return Err(domain(
                "github_run_identity",
                "github.run",
                "GitHub durable run immutable material changed",
            ));
        }
        let mut state = State::default();
        for event in &self.events {
            state = evolve(&state, event)?;
        }
        self.ci_state = state;
        let outcome = self.outcome()?;
        if self.ci_state.runs.len() != 1
            || self.ci_state.attempts.len() != 1
            || !self.ci_state.attempts.contains_key(&self.plan.attempt_id)
            || (self.phase == RunPhase::Terminal) != outcome.is_terminal()
        {
            return Err(domain(
                "github_run_history",
                "github.run",
                "GitHub durable history disagrees with its run phase",
            ));
        }
        if let Some(observation) = &self.observation {
            admit_workflow_observation(&self.plan, observation)?;
        }
        Ok(())
    }
}

pub fn workflow_plan(
    config: &WorkflowConfig,
    run: &ContentId,
    pipeline: &ContentId,
    candidate: &ContentId,
    source: Option<WorkflowSourceContext>,
) -> PortResult<WorkflowEffectPlan> {
    let plan = WorkflowEffectPlan {
        effect_id: EffectId::new(format!("github-effect-{}", run.as_str()))?,
        attempt_id: AttemptId::new(format!("github-attempt-{}", run.as_str()))?,
        candidate_id: CandidateId::new(format!("github-candidate-{}", candidate.as_str()))?,
        pipeline_revision_id: PipelineRevisionId::new(format!(
            "github-pipeline-{}",
            pipeline.as_str()
        ))?,
        workflow_policy_id: PolicyId::new(config.policy_id.clone())?,
        provider_profile: ProviderProfileId::new(config.provider_profile_id.clone())?,
        idempotency_key: run.clone(),
        platform: Alias::new(config.platform.clone())?,
        source,
        nodes: config
            .nodes
            .iter()
            .map(|node| {
                Ok(WorkflowNode {
                    id: Alias::new(node.id.clone())?,
                    kind: Alias::new(node.kind.clone())?,
                    effect_class: node.effect_class.into(),
                    adapter_kind: Alias::new(node.adapter_kind.clone())?,
                    operation_identity: ContentId::from_blake3_hex(
                        node.operation_identity.clone(),
                    )?,
                })
            })
            .collect::<Result<_, kiln_core::types::Diagnostic>>()?,
        edges: config
            .edges
            .iter()
            .map(|edge| {
                Ok(WorkflowEdge {
                    source: Alias::new(edge.source.clone())?,
                    target: Alias::new(edge.target.clone())?,
                })
            })
            .collect::<Result<_, kiln_core::types::Diagnostic>>()?,
        controls: config.controls,
        bounds: config.bounds,
    };
    validate_workflow_plan(&plan)?;
    Ok(plan)
}

// r[impl kiln.github.workflow]
// r[impl kiln.github.replay]
pub fn plan_event(admitted: &AdmittedGithubConfig, event: &VerifiedEvent) -> PortResult<RunRecord> {
    let (_, identity) = encode_and_identify(
        &(
            RUN_SCHEMA,
            &admitted.pipeline_identity,
            &event.candidate_identity,
        ),
        MAXIMUM_PROFILE_BYTES,
    )
    .map_err(|_| {
        domain(
            "github_run_identity",
            "github.run",
            "GitHub run identity could not be encoded",
        )
    })?;
    let source = WorkflowSourceContext {
        provider: Alias::new("github")?,
        repository: ExternalRef::new("github", "repository", &event.repository)?,
        revision: ExternalRef::new("github", "revision", &event.revision)?,
    };
    let plan = workflow_plan(
        &admitted.config.workflow,
        &identity,
        &admitted.pipeline_identity,
        &event.candidate_identity,
        Some(source),
    )?;
    let run_id = RunId::new(format!("github-run-{}", identity.as_str()))?;
    let immutable_identity = immutable_identity(&identity, &admitted.identity, &plan, &run_id)?;
    let pending = plan_status(
        &admitted.config.status,
        &event.revision,
        identity.clone(),
        StatusState::Pending,
    )?;
    let mut record = RunRecord {
        schema: RUN_SCHEMA.to_string(),
        run_identity: identity,
        immutable_identity,
        profile_identity: admitted.identity.clone(),
        plan,
        run_id,
        phase: RunPhase::Queued,
        workflow_polls: 0,
        observation: None,
        events: Vec::new(),
        statuses: vec![StatusRecord {
            plan: pending,
            phase: PublicationPhase::Ready,
            publication: None,
            error_code: None,
        }],
        error_code: None,
        version: 0,
        ci_state: State::default(),
    };
    let start = Command::StartRun {
        run_id: record.run_id.clone(),
        attempt_id: record.plan.attempt_id.clone(),
        provider_profile: record.plan.provider_profile.clone(),
        effect_id: record.plan.effect_id.clone(),
    };
    apply_command(&mut record, "start", start)?;
    Ok(record)
}

fn immutable_identity(
    run: &ContentId,
    profile: &ContentId,
    plan: &WorkflowEffectPlan,
    run_id: &RunId,
) -> PortResult<ContentId> {
    encode_and_identify(
        &(RUN_SCHEMA, run, profile, plan, run_id),
        MAXIMUM_PROFILE_BYTES,
    )
    .map(|(_, identity)| identity)
    .map_err(|_| {
        domain(
            "github_run_identity",
            "github.run",
            "GitHub immutable run material exceeds its identity bound",
        )
    })
}

fn apply_command(record: &mut RunRecord, suffix: &str, command: Command) -> PortResult<()> {
    let envelope = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new(format!("github-{suffix}-{}", record.run_identity.as_str()))?,
        expected_revision: record.ci_state.revision,
        command,
    };
    let decision = decide(&record.ci_state, &envelope, Limits::default())?;
    for event in decision.events {
        record.ci_state = evolve(&record.ci_state, &event)?;
        record.events.push(event);
    }
    Ok(())
}

#[derive(Debug)]
pub struct GithubService<W, P> {
    workflow: W,
    status: P,
    store: SqliteStore,
    config: GithubConfig,
    known_executions: BTreeSet<ContentId>,
    cursor: Option<String>,
}

impl<W: WorkflowPort, P: StatusPort> GithubService<W, P> {
    pub fn new(workflow: W, status: P, store: SqliteStore, config: GithubConfig) -> Self {
        Self {
            workflow,
            status,
            store,
            config,
            known_executions: BTreeSet::new(),
            cursor: None,
        }
    }

    pub fn tick(&mut self) -> PortResult<bool> {
        let Some(mut record) = self.store.next_work(self.cursor.as_deref())? else {
            return Ok(false);
        };
        self.cursor = Some(record.run_identity.as_str().to_string());
        if !matches!(record.phase, RunPhase::Terminal | RunPhase::Unresolved) {
            self.advance_workflow(&mut record)?;
        }
        self.advance_status(&mut record)?;
        Ok(true)
    }

    pub fn reconcile_run(&mut self, run: &ContentId) -> PortResult<RunRecord> {
        let mut record = self.store.load(run)?.ok_or_else(|| {
            domain(
                "github_run_missing",
                "github.run",
                "GitHub run does not exist",
            )
        })?;
        if record.phase == RunPhase::Unresolved {
            record.phase = RunPhase::Unknown;
            record.workflow_polls = 0;
            self.store.save(&mut record)?;
        }
        if !matches!(record.phase, RunPhase::Queued | RunPhase::Terminal) {
            self.advance_workflow(&mut record)?;
        }
        self.advance_status(&mut record)?;
        Ok(record)
    }

    // r[impl kiln.github.workflow]
    fn advance_workflow(&mut self, record: &mut RunRecord) -> PortResult<()> {
        if record.phase == RunPhase::Queued {
            if self.store.active_count()? >= self.config.maximum_active_runs {
                return Ok(());
            }
            record.phase = RunPhase::DispatchReserved;
            self.store.save(record)?;
            let result = self.workflow.dispatch(&record.plan);
            return self.accept_result(record, result);
        }
        if record.workflow_polls >= self.config.maximum_workflow_polls {
            mark_unknown(record)?;
            record.phase = RunPhase::Unresolved;
            record.error_code = Some("github_observation_budget".to_string());
            append_status(record, &self.config, StatusState::Error)?;
            return self.store.save(record);
        }
        record.workflow_polls += 1;
        let result = if !self.known_executions.contains(&record.run_identity) {
            match self.workflow.recover_dispatch(&record.plan) {
                Ok(recovered) => {
                    if let Err(error) = admit_workflow_observation(&record.plan, &recovered) {
                        return self.reject_observation(record, error.into());
                    }
                    if record
                        .observation
                        .as_ref()
                        .is_some_and(|previous| previous.execution != recovered.execution)
                    {
                        return self.reject_observation(
                            record,
                            domain(
                                "github_recovery_execution",
                                "github.workflow",
                                "Recovered execution differs from the durable execution",
                            ),
                        );
                    }
                    self.known_executions.insert(record.run_identity.clone());
                    // Recovery reconstructs acceptance; it is not a newer lifecycle observation.
                    self.workflow.observe(&record.plan.effect_id)
                }
                Err(error) => Err(error),
            }
        } else if record.phase == RunPhase::Unknown {
            self.workflow.reconcile(&record.plan.effect_id)
        } else {
            self.workflow.observe(&record.plan.effect_id)
        };
        self.accept_result(record, result)
    }

    fn accept_result(
        &mut self,
        record: &mut RunRecord,
        result: PortResult<WorkflowObservation>,
    ) -> PortResult<()> {
        match result {
            Ok(observation) => {
                let outcome = match admit_workflow_observation(&record.plan, &observation) {
                    Ok(outcome) => outcome,
                    Err(error) => return self.reject_observation(record, error.into()),
                };
                if record.observation.as_ref().is_some_and(|previous| {
                    previous.execution != observation.execution
                        || observation.sequence < previous.sequence
                        || (observation.sequence == previous.sequence && previous != &observation)
                }) {
                    return self.reject_observation(
                        record,
                        domain(
                            "github_stale_observation",
                            "github.workflow",
                            "Workflow observation is stale or conflicts with durable evidence",
                        ),
                    );
                }
                self.known_executions.insert(record.run_identity.clone());
                if observation.state.is_terminal() {
                    complete(record, outcome, Some(observation.execution.clone()))?;
                    record.phase = RunPhase::Terminal;
                    let state = if outcome == Outcome::Success {
                        StatusState::Success
                    } else if outcome == Outcome::DomainFailure {
                        StatusState::Failure
                    } else {
                        StatusState::Error
                    };
                    append_status(record, &self.config, state)?;
                } else if observation.state == WorkflowState::Unknown {
                    mark_unknown(record)?;
                    record.phase = RunPhase::Unknown;
                } else {
                    record.phase = RunPhase::Active;
                }
                record.observation = Some(observation);
                record.error_code = None;
            }
            Err(error) => {
                // Dispatch may include recovery internally; no PortFailure proves non-acceptance.
                mark_unknown(record)?;
                record.phase = RunPhase::Unknown;
                record.error_code = Some(error_code(&error).to_string());
            }
        }
        self.store.save(record)
    }

    fn reject_observation(&mut self, record: &mut RunRecord, error: PortFailure) -> PortResult<()> {
        mark_unknown(record)?;
        record.phase = RunPhase::Unknown;
        record.error_code = Some(error_code(&error).to_string());
        self.store.save(record)?;
        Err(error)
    }

    // r[impl kiln.github.status]
    fn advance_status(&mut self, record: &mut RunRecord) -> PortResult<()> {
        let Some(index) = record.statuses.iter().position(|status| {
            matches!(
                status.phase,
                PublicationPhase::Ready | PublicationPhase::Reserved
            )
        }) else {
            return Ok(());
        };
        let phase = record.statuses.get(index).ok_or_else(status_missing)?.phase;
        if phase == PublicationPhase::Ready {
            record
                .statuses
                .get_mut(index)
                .ok_or_else(status_missing)?
                .phase = PublicationPhase::Reserved;
            self.store.save(record)?;
        }
        let plan = &record.statuses.get(index).ok_or_else(status_missing)?.plan;
        let result = if phase == PublicationPhase::Ready {
            self.status.publish(plan).map(Some)
        } else {
            self.status.reconcile(plan)
        };
        let status = record.statuses.get_mut(index).ok_or_else(status_missing)?;
        match result {
            Ok(Some(publication)) => {
                if publication.request_identity != crate::status::request_identity(&status.plan)? {
                    status.phase = PublicationPhase::Unknown;
                    status.error_code = Some("github_status_linkage".to_string());
                } else {
                    status.phase = PublicationPhase::Published;
                    status.publication = Some(publication);
                    status.error_code = None;
                }
            }
            Ok(None) => {
                status.phase = PublicationPhase::Unknown;
            }
            Err(error) => {
                status.phase = if phase == PublicationPhase::Ready
                    && matches!(error, PortFailure::Unknown(_))
                {
                    PublicationPhase::Reserved
                } else if phase == PublicationPhase::Reserved {
                    PublicationPhase::Unknown
                } else {
                    PublicationPhase::Failed
                };
                status.error_code = Some(error_code(&error).to_string());
            }
        }
        self.store.save(record)
    }
}

fn status_missing() -> PortFailure {
    domain(
        "github_status_missing",
        "github.status",
        "GitHub status intent disappeared",
    )
}

fn append_status(
    record: &mut RunRecord,
    config: &GithubConfig,
    state: StatusState,
) -> PortResult<()> {
    if record
        .statuses
        .last()
        .is_some_and(|status| status.plan.state == state)
    {
        return Ok(());
    }
    let source = record.plan.source.as_ref().ok_or_else(|| {
        domain(
            "github_source_missing",
            "github.run",
            "GitHub workflow has no exact source binding",
        )
    })?;
    let plan = plan_status(
        &config.status,
        source.revision.native_id.as_str(),
        record.run_identity.clone(),
        state,
    )?;
    record.statuses.push(StatusRecord {
        plan,
        phase: PublicationPhase::Ready,
        publication: None,
        error_code: None,
    });
    Ok(())
}

fn mark_unknown(record: &mut RunRecord) -> PortResult<()> {
    if record.outcome()? != Outcome::Unknown {
        let command = Command::RecordUnknown {
            attempt_id: record.plan.attempt_id.clone(),
            external_reference: record
                .observation
                .as_ref()
                .map(|observation| observation.execution.clone()),
        };
        apply_command(record, "unknown", command)?;
    }
    Ok(())
}

fn complete(
    record: &mut RunRecord,
    outcome: Outcome,
    reference: Option<ExternalRef>,
) -> PortResult<()> {
    let command = if record.outcome()? == Outcome::Unknown {
        Command::ReconcileAttempt {
            attempt_id: record.plan.attempt_id.clone(),
            outcome,
            external_reference: reference,
        }
    } else {
        Command::CompleteAttempt {
            attempt_id: record.plan.attempt_id.clone(),
            outcome,
            external_reference: reference,
            products: Vec::new(),
            metrics: Vec::new(),
        }
    };
    apply_command(record, "complete", command)
}

pub(crate) fn error_code(error: &PortFailure) -> &str {
    match error {
        PortFailure::Domain(error) => &error.code,
        PortFailure::Infrastructure(error)
        | PortFailure::Retryable(error)
        | PortFailure::TimedOut(error)
        | PortFailure::Cancelled(error)
        | PortFailure::Unknown(error) => &error.code,
    }
}
