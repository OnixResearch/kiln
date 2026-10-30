//! Pure pipeline planning, Kiln run state, approval, retry and status views.
//!
//! One admitted event and one exact acquisition become one Tangled pipeline
//! holding one Kiln run per matched reviewed workflow. Each run is a
//! `WorkflowEffectPlan` bound to the exact object id, with
//! `kiln_core::domain` events recording its lifecycle.

use kiln_codec::encode_and_identify;
use kiln_core::domain::{Command, CommandEnvelope, Event, Outcome, State, decide, evolve};
use kiln_core::monorepo::ChangedFiles;
use kiln_core::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CandidateId, CommandId, ContentId, EffectId,
    ExternalRef, Limits, PipelineRevisionId, PolicyId, ProviderProfileId, RunId,
};
use kiln_core::untrusted::{
    ApprovalRejection, ApproveUntrustedRun, Approver, ApproverAuthority, UntrustedGate,
    decide_approval,
};
use kiln_core::workflow::{
    WorkflowEffectClass, WorkflowEffectPlan, WorkflowNode, WorkflowObservation,
    WorkflowSourceContext, WorkflowState, admit_workflow_observation, validate_workflow_plan,
};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Rejection, RejectionClass, TangledResult, port_code};
use crate::events::{AdmittedEvent, Trigger, Trust};
use crate::identity::ObjectId;
use crate::nsid;
use crate::profile::{AdmittedProfile, RepositoryPin};
use crate::time::rfc3339;
use crate::when::{TriggerEvent, any_rule_matches};
use crate::workflows::{ReviewedWorkflow, definition_identity, review};
use crate::xrpc::set_field;

pub const PIPELINE_SCHEMA: &str = "kiln.tangled-pipeline.v1";
pub const RUN_SCHEMA: &str = "kiln.tangled-run.v1";
pub const PIPELINE_ID_PREFIX: &str = "kiln-";
/// Hex digits of the pipeline identity used in the pipeline id.
const PIPELINE_ID_HEX: usize = 32;
/// Lexicon bound on workflows in one pipeline.
pub const MAXIMUM_PIPELINE_WORKFLOWS: usize = 50;
/// Lexicon bound on the notifyEvent message.
pub const MAXIMUM_MESSAGE_BYTES: usize = 1000;
/// Status views kept per pipeline; older delivered views are dropped.
pub const MAXIMUM_PUBLICATIONS: usize = 8;
/// StartRun, RecordUnknown and one completion.
const MAXIMUM_RUN_EVENTS: usize = 3;
const MAXIMUM_IDENTITY_BYTES: usize = 1_048_576;
const UNKNOWN_STATUS_ERROR: &str = "kiln: outcome unknown; awaiting reconciliation";
const WORKFLOW_NODE: &str = "workflow";
const WORKFLOW_KIND: &str = "tangled-workflow";

/// One workflow file at the exact commit, as read by the source shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Facts from exact acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredSource {
    pub commit: ObjectId,
    pub files: Vec<WorkflowFile>,
    pub changed: ChangedFiles,
}

/// Milliseconds per second, for #12 approval timestamps.
const MILLISECONDS_PER_SECOND: u64 = 1000;

/// A stable, non-zero #12 pull request number for a Tangled pull request,
/// which has a record key instead of a number: the first eight bytes of the
/// BLAKE3 hash of its AT-URI.
fn pull_request_number(pull: &str) -> u64 {
    let hash = blake3::hash(pull.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(hash.as_bytes().get(..8).unwrap_or(&[1; 8]));
    u64::from_le_bytes(bytes).max(1)
}

/// Whether an untrusted gate still withholds dispatch.
#[must_use]
pub fn held(record: &PipelineRecord) -> bool {
    record
        .untrusted
        .as_ref()
        .is_some_and(|gate| !gate.may_dispatch())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Queued,
    /// Saved before dispatch; after a crash the dispatch is recovered, not repeated.
    DispatchReserved,
    Active,
    Unknown,
    /// The observation budget ran out; the outcome stays unknown and the
    /// worker stops polling.
    Unresolved,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRun {
    pub definition: ReviewedWorkflow,
    pub definition_identity: ContentId,
    pub run_identity: ContentId,
    pub run_id: RunId,
    pub plan: WorkflowEffectPlan,
    pub phase: RunPhase,
    pub polls: usize,
    pub observation: Option<WorkflowObservation>,
    pub events: Vec<Event>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error_code: Option<String>,
    #[serde(skip)]
    pub state: State,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPhase {
    Ready,
    /// Reserved before sending; a crash here reconciles by re-sending.
    Reserved,
    Published,
    /// The delivery outcome is uncertain; the same request is re-sent.
    Unknown,
    Failed,
    /// A newer view replaced this one before it was delivered.
    Superseded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    /// BLAKE3 of the delivered view bytes.
    pub request_identity: ContentId,
    pub phase: PublicationPhase,
    pub attempts: u32,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineRecord {
    pub schema: String,
    pub pipeline_id: String,
    pub identity: ContentId,
    pub replay_key: ContentId,
    pub profile_identity: ContentId,
    pub repo_did: String,
    pub commit: ObjectId,
    pub source_repo: Option<String>,
    pub trigger: Trigger,
    /// Lexicon trigger object, served unchanged.
    pub native: Value,
    pub actor: String,
    pub trust: Trust,
    /// #12 gate of an untrusted pull request; `None` for trusted pipelines.
    pub untrusted: Option<UntrustedGate>,
    pub created_at: i64,
    pub retry_of: Option<String>,
    pub retries: u32,
    pub workflows: Vec<WorkflowRun>,
    pub publications: Vec<Publication>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanOutcome {
    /// No reviewed workflow matched; Tangled starts nothing.
    NoMatch {
        message: String,
    },
    Pipeline(Box<PipelineRecord>),
}

fn internal(code: &'static str) -> Rejection {
    Rejection::internal(code, "Tangled pipeline state is inconsistent")
}

fn identity<T: Serialize>(value: &T) -> TangledResult<ContentId> {
    encode_and_identify(value, MAXIMUM_IDENTITY_BYTES)
        .map(|(_, identity)| identity)
        .map_err(|_| internal("tangled_identity"))
}

fn pipeline_id(identity: &ContentId) -> String {
    let hex = identity.as_str();
    format!(
        "{PIPELINE_ID_PREFIX}{}",
        hex.get(..PIPELINE_ID_HEX).unwrap_or(hex)
    )
}

fn bounded_message(message: String) -> String {
    if message.len() <= MAXIMUM_MESSAGE_BYTES {
        return message;
    }
    let mut end = MAXIMUM_MESSAGE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message.get(..end).unwrap_or_default().to_string()
}

// r[impl kiln.tangled.workflows] r[impl kiln.tangled.source] r[impl kiln.tangled.untrusted]
/// Plan the pipeline of one admitted event from its exact acquisition.
pub fn plan_pipeline(
    profile: &AdmittedProfile,
    event: &AdmittedEvent,
    source: &AcquiredSource,
    now: i64,
) -> TangledResult<PlanOutcome> {
    if source.commit != event.commit {
        return Err(internal("tangled_acquisition_commit"));
    }
    let maximum = profile.profile.limits.maximum_workflow_bytes;
    let mut reviewed = Vec::new();
    let mut diagnostics = Vec::new();
    for file in &source.files {
        match review(&file.path, &file.bytes, &profile.profile.engines, maximum) {
            Ok(workflow) => reviewed.push(workflow),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    let selected = select(event, &reviewed, &source.changed)?;
    if selected.is_empty() {
        let mut message = String::from("kiln: no reviewed workflow matched");
        for diagnostic in &diagnostics {
            message.push_str("; ");
            message.push_str(diagnostic);
        }
        return Ok(PlanOutcome::NoMatch {
            message: bounded_message(message),
        });
    }
    if selected.len() > MAXIMUM_PIPELINE_WORKFLOWS {
        return Err(Rejection::new(
            RejectionClass::Unprocessable,
            "tangled_too_many_workflows",
            "More workflows matched than one pipeline may hold",
        ));
    }
    let identity = identity(&(PIPELINE_SCHEMA, &profile.identity, &event.replay_key))?;
    let untrusted = match (event.trust, &event.trigger) {
        (Trust::Trusted, _) => None,
        (Trust::Untrusted { reason }, Trigger::PullRequest { pull, .. }) => {
            let head = event
                .source_repo
                .clone()
                .unwrap_or_else(|| event.repo_did.clone());
            let gate = UntrustedGate::new(
                reason,
                pull_request_number(pull),
                head,
                event.commit.as_str().to_string(),
                false,
            )
            .map_err(|_| internal("tangled_untrusted_gate"))?;
            Some(gate)
        }
        (Trust::Untrusted { .. }, _) => return Err(internal("tangled_untrusted_trigger")),
    };
    let mut notes = diagnostics;
    for workflow in &selected {
        notes.extend(
            workflow
                .notes
                .iter()
                .map(|note| format!("{}: {note}", workflow.path)),
        );
    }
    let mut record = PipelineRecord {
        schema: PIPELINE_SCHEMA.to_string(),
        pipeline_id: pipeline_id(&identity),
        identity,
        replay_key: event.replay_key.clone(),
        profile_identity: profile.identity.clone(),
        repo_did: event.repo_did.clone(),
        commit: event.commit.clone(),
        source_repo: event.source_repo.clone(),
        trigger: event.trigger.clone(),
        native: event.native.clone(),
        actor: event.actor.clone(),
        trust: event.trust,
        untrusted,
        created_at: now,
        retry_of: None,
        retries: 0,
        workflows: Vec::new(),
        publications: Vec::new(),
        notes,
    };
    for workflow in selected {
        let run = plan_run(profile, &record, workflow)?;
        record.workflows.push(run);
    }
    Ok(PlanOutcome::Pipeline(Box::new(record)))
}

fn select(
    event: &AdmittedEvent,
    reviewed: &[ReviewedWorkflow],
    changed: &ChangedFiles,
) -> TangledResult<Vec<ReviewedWorkflow>> {
    let trigger_event = match &event.trigger {
        Trigger::Push { reference } => TriggerEvent::Push { reference },
        Trigger::PullRequest {
            action,
            target_branch,
            ..
        } => TriggerEvent::PullRequest {
            action: *action,
            base_branch: target_branch,
        },
        Trigger::Manual { .. } => TriggerEvent::Manual,
    };
    let named = |workflow: &ReviewedWorkflow| {
        event.workflows.is_empty() || event.workflows.contains(&workflow.id)
    };
    if matches!(event.trigger, Trigger::Manual { .. }) {
        // Tangled evaluates no `when` filters for manual runs: it runs the
        // named workflows, or every dispatchable one.
        for name in &event.workflows {
            if !reviewed
                .iter()
                .any(|workflow| workflow.id == *name && workflow.manually_dispatchable())
            {
                return Err(Rejection::invalid(
                    "tangled_manual_workflow",
                    "A named workflow is missing, unreviewable or not manually dispatchable",
                ));
            }
        }
        return Ok(reviewed
            .iter()
            .filter(|w| w.manually_dispatchable() && named(w))
            .cloned()
            .collect());
    }
    Ok(reviewed
        .iter()
        .filter(|workflow| {
            named(workflow) && any_rule_matches(&workflow.triggers, &trigger_event, changed)
        })
        .cloned()
        .collect())
}

fn plan_run(
    profile: &AdmittedProfile,
    record: &PipelineRecord,
    definition: ReviewedWorkflow,
) -> TangledResult<WorkflowRun> {
    let binding = profile
        .engine(&definition.engine)
        .ok_or(Rejection::invalid(
            "tangled_engine_unbound",
            "Workflow engine is not bound by the profile",
        ))?;
    let definition_identity =
        definition_identity(&record.repo_did, record.commit.as_str(), &definition)
            .ok_or(internal("tangled_definition_identity"))?;
    let run_identity = identity(&(
        RUN_SCHEMA,
        &record.identity,
        &definition.id,
        &definition_identity,
    ))?;
    let workflow = &profile.profile.workflow;
    let policy = if record.trust == Trust::Trusted {
        &workflow.policy_id
    } else {
        &workflow.untrusted_policy_id
    };
    let core = |_: kiln_core::types::Diagnostic| internal("tangled_run_plan");
    let run = run_identity.as_str();
    let plan = WorkflowEffectPlan {
        effect_id: EffectId::new(format!("tangled-effect-{run}")).map_err(core)?,
        attempt_id: AttemptId::new(format!("tangled-attempt-{run}")).map_err(core)?,
        candidate_id: CandidateId::new(format!("tangled-candidate-{}", record.identity.as_str()))
            .map_err(core)?,
        pipeline_revision_id: PipelineRevisionId::new(format!(
            "tangled-pipeline-{}",
            definition_identity.as_str()
        ))
        .map_err(core)?,
        workflow_policy_id: PolicyId::new(policy.clone()).map_err(core)?,
        provider_profile: ProviderProfileId::new(workflow.provider_profile_id.clone())
            .map_err(core)?,
        idempotency_key: run_identity.clone(),
        platform: Alias::new(binding.platform.clone()).map_err(core)?,
        source: Some(WorkflowSourceContext {
            provider: Alias::new("tangled").map_err(core)?,
            repository: ExternalRef::new("tangled", "repository", &record.repo_did)
                .map_err(core)?,
            revision: ExternalRef::new("tangled", "revision", record.commit.as_str())
                .map_err(core)?,
        }),
        nodes: vec![WorkflowNode {
            id: Alias::new(WORKFLOW_NODE).map_err(core)?,
            kind: Alias::new(WORKFLOW_KIND).map_err(core)?,
            effect_class: WorkflowEffectClass::Test,
            adapter_kind: Alias::new(binding.adapter_kind.clone()).map_err(core)?,
            operation_identity: definition_identity.clone(),
        }],
        edges: Vec::new(),
        controls: workflow.controls,
        bounds: workflow.bounds,
    };
    validate_workflow_plan(&plan).map_err(core)?;
    let run_id = RunId::new(format!("tangled-run-{run}")).map_err(core)?;
    let mut run = WorkflowRun {
        definition,
        definition_identity,
        run_identity,
        run_id,
        plan,
        phase: RunPhase::Queued,
        polls: 0,
        observation: None,
        events: Vec::new(),
        started_at: None,
        finished_at: None,
        error_code: None,
        state: State::default(),
    };
    let start = Command::StartRun {
        run_id: run.run_id.clone(),
        attempt_id: run.plan.attempt_id.clone(),
        provider_profile: run.plan.provider_profile.clone(),
        effect_id: run.plan.effect_id.clone(),
    };
    apply_command(&mut run, "start", start)?;
    Ok(run)
}

fn apply_command(run: &mut WorkflowRun, suffix: &str, command: Command) -> TangledResult<()> {
    let envelope = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new(format!("tangled-{suffix}-{}", run.run_identity.as_str()))
            .map_err(|_| internal("tangled_command_id"))?,
        expected_revision: run.state.revision,
        command,
    };
    let decision = decide(&run.state, &envelope, Limits::default())
        .map_err(|_| internal("tangled_core_decision"))?;
    for event in decision.events {
        run.state = evolve(&run.state, &event).map_err(|_| internal("tangled_core_event"))?;
        run.events.push(event);
    }
    Ok(())
}

impl WorkflowRun {
    pub fn outcome(&self) -> TangledResult<Outcome> {
        self.state
            .runs
            .get(&self.run_id)
            .map(|run| run.outcome)
            .ok_or(internal("tangled_run_state"))
    }

    fn restore(&mut self) -> TangledResult<()> {
        if self.events.is_empty() || self.events.len() > MAXIMUM_RUN_EVENTS {
            return Err(internal("tangled_run_history"));
        }
        validate_workflow_plan(&self.plan).map_err(|_| internal("tangled_run_plan"))?;
        let mut state = State::default();
        for event in &self.events {
            state = evolve(&state, event).map_err(|_| internal("tangled_run_history"))?;
        }
        self.state = state;
        if (self.phase == RunPhase::Terminal) != self.outcome()?.is_terminal() {
            return Err(internal("tangled_run_phase"));
        }
        Ok(())
    }

    fn mark_unknown(&mut self) -> TangledResult<()> {
        if self.outcome()? != Outcome::Unknown {
            let command = Command::RecordUnknown {
                attempt_id: self.plan.attempt_id.clone(),
                external_reference: self
                    .observation
                    .as_ref()
                    .map(|observation| observation.execution.clone()),
            };
            apply_command(self, "unknown", command)?;
        }
        self.phase = RunPhase::Unknown;
        Ok(())
    }

    fn complete(&mut self, outcome: Outcome, reference: Option<ExternalRef>) -> TangledResult<()> {
        let command = if self.outcome()? == Outcome::Unknown {
            Command::ReconcileAttempt {
                attempt_id: self.plan.attempt_id.clone(),
                outcome,
                external_reference: reference,
            }
        } else {
            Command::CompleteAttempt {
                attempt_id: self.plan.attempt_id.clone(),
                outcome,
                external_reference: reference,
                products: Vec::new(),
                metrics: Vec::new(),
            }
        };
        apply_command(self, "complete", command)?;
        self.phase = RunPhase::Terminal;
        Ok(())
    }

    // r[impl kiln.tangled.status]
    /// Fold one dispatch or observation result into the run. Any failure that
    /// does not prove non-acceptance leaves the run `Unknown`, never succeeded.
    pub fn accept(
        &mut self,
        result: PortResult<WorkflowObservation>,
        now: i64,
    ) -> TangledResult<()> {
        let observation = match result {
            Ok(observation) => observation,
            Err(error) => {
                self.error_code = Some(port_code(&error).to_string());
                return self.mark_unknown();
            }
        };
        let outcome = match admit_workflow_observation(&self.plan, &observation) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.error_code = Some(error.code);
                return self.mark_unknown();
            }
        };
        let stale = self.observation.as_ref().is_some_and(|previous| {
            previous.execution != observation.execution
                || observation.sequence < previous.sequence
                || (observation.sequence == previous.sequence && *previous != observation)
        });
        if stale {
            self.error_code = Some("tangled_stale_observation".to_string());
            return self.mark_unknown();
        }
        if self.started_at.is_none() {
            self.started_at = Some(now);
        }
        if observation.state.is_terminal() {
            self.complete(outcome, Some(observation.execution.clone()))?;
            self.finished_at = Some(now);
        } else if observation.state == WorkflowState::Unknown {
            self.mark_unknown()?;
        } else {
            self.phase = RunPhase::Active;
        }
        self.observation = Some(observation);
        self.error_code = None;
        Ok(())
    }

    /// The observation budget is exhausted: the outcome stays unknown.
    pub fn exhaust(&mut self) -> TangledResult<()> {
        self.mark_unknown()?;
        self.error_code = Some("tangled_observation_budget".to_string());
        self.phase = RunPhase::Unresolved;
        Ok(())
    }
}

/// Tangled workflow status for one run; never `success` unless Kiln recorded success.
pub fn workflow_status(
    gate: Option<&UntrustedGate>,
    run: &WorkflowRun,
) -> (&'static str, Option<String>) {
    if gate.is_some_and(|gate| !gate.may_dispatch()) {
        return ("blocked", None);
    }
    match run.phase {
        RunPhase::Queued | RunPhase::DispatchReserved => ("pending", None),
        RunPhase::Active => ("running", None),
        RunPhase::Unknown | RunPhase::Unresolved => {
            ("pending", Some(UNKNOWN_STATUS_ERROR.to_string()))
        }
        RunPhase::Terminal => match run.outcome() {
            Ok(Outcome::Success) => ("success", None),
            Ok(Outcome::TimedOut) => ("timeout", None),
            Ok(Outcome::Cancelled) => ("cancelled", None),
            Ok(Outcome::DomainFailure | Outcome::Denied | Outcome::ProviderError) => (
                "failed",
                run.error_code
                    .clone()
                    .or_else(|| Some("kiln: workflow failed".to_string())),
            ),
            Ok(Outcome::Pending | Outcome::Unknown) | Err(_) => {
                ("pending", Some(UNKNOWN_STATUS_ERROR.to_string()))
            }
        },
    }
}

/// `org.tangled.ci.defs#workflow` view of a reviewed definition at `commit`.
#[must_use]
pub fn workflow_view(repo_did: &str, commit: &str, workflow: &ReviewedWorkflow) -> Value {
    json!({
        "id": workflow.id,
        "name": workflow.id,
        "hash": format!("blake3:{}", workflow.source_blake3),
        "source": {
            "$type": nsid::FILE_SOURCE,
            "repo": repo_did,
            "commit": commit,
            "path": workflow.path,
        },
        "triggers": workflow.trigger_nsids(),
    })
}

// r[impl kiln.tangled.status]
/// `org.tangled.ci.defs#pipeline` view served by getPipeline and delivered to
/// the status endpoint.
#[must_use]
pub fn pipeline_view(record: &PipelineRecord) -> Value {
    let workflows: Vec<Value> = record
        .workflows
        .iter()
        .map(|run| {
            let (status, error) = workflow_status(record.untrusted.as_ref(), run);
            let mut view = json!({
                "id": run.definition.id,
                "name": run.definition.id,
                "status": status,
                "definition": workflow_view(&record.repo_did, record.commit.as_str(), &run.definition),
            });
            if let Some(started) = run.started_at {
                set_field(&mut view, "startedAt", json!(rfc3339(started)));
            }
            if let Some(finished) = run.finished_at {
                set_field(&mut view, "finishedAt", json!(rfc3339(finished)));
            }
            if let Some(error) = error {
                set_field(&mut view, "error", json!(error));
            }
            view
        })
        .collect();
    let mut view = json!({
        "id": record.pipeline_id,
        "repo": record.repo_did,
        "trigger": record.native,
        "commit": record.commit.as_str(),
        "createdAt": rfc3339(record.created_at),
        "workflows": workflows,
    });
    if let Some(source) = &record.source_repo {
        if *source != record.repo_did {
            set_field(&mut view, "sourceRepo", json!(source));
        }
    }
    view
}

/// Canonical view bytes and their request identity.
pub fn view_delivery(record: &PipelineRecord) -> TangledResult<(Vec<u8>, ContentId)> {
    let bytes = serde_json::to_vec(&pipeline_view(record))
        .map_err(|_| internal("tangled_view_encoding"))?;
    let identity =
        kiln_codec::identity_from_bytes(&bytes).map_err(|_| internal("tangled_view_identity"))?;
    Ok((bytes, identity))
}

/// The notifyEvent message for a created pipeline.
#[must_use]
pub fn pipeline_message(record: &PipelineRecord) -> String {
    let mut message = format!(
        "kiln: pipeline {} scheduled {} workflow(s) at {}",
        record.pipeline_id,
        record.workflows.len(),
        record.commit.as_str()
    );
    if held(record) {
        message.push_str("; blocked until an approver approves it");
    }
    for note in &record.notes {
        message.push_str("; ");
        message.push_str(note);
    }
    bounded_message(message)
}

/// Record a new status view when it differs from the newest one.
pub fn refresh_publication(record: &mut PipelineRecord) -> TangledResult<()> {
    let (_, identity) = view_delivery(record)?;
    if record
        .publications
        .last()
        .is_some_and(|last| last.request_identity == identity)
    {
        return Ok(());
    }
    for publication in &mut record.publications {
        if matches!(
            publication.phase,
            PublicationPhase::Ready | PublicationPhase::Unknown
        ) {
            publication.phase = PublicationPhase::Superseded;
        }
    }
    record.publications.push(Publication {
        request_identity: identity,
        phase: PublicationPhase::Ready,
        attempts: 0,
        error_code: None,
    });
    while record.publications.len() > MAXIMUM_PUBLICATIONS {
        let Some(index) = record.publications.iter().position(|publication| {
            matches!(
                publication.phase,
                PublicationPhase::Published
                    | PublicationPhase::Failed
                    | PublicationPhase::Superseded
            )
        }) else {
            break;
        };
        record.publications.remove(index);
    }
    Ok(())
}

/// Whether the worker still has dispatch, observation or delivery work.
#[must_use]
pub fn has_work(record: &PipelineRecord) -> bool {
    let runs = !held(record)
        && record
            .workflows
            .iter()
            .any(|run| !matches!(run.phase, RunPhase::Terminal | RunPhase::Unresolved));
    let deliveries = record.publications.iter().any(|publication| {
        matches!(
            publication.phase,
            PublicationPhase::Ready | PublicationPhase::Reserved | PublicationPhase::Unknown
        )
    });
    runs || deliveries
}

/// Rebuild core state after loading a durable record.
pub fn restore(record: &mut PipelineRecord) -> TangledResult<()> {
    if record.schema != PIPELINE_SCHEMA
        || record.workflows.is_empty()
        || record.workflows.len() > MAXIMUM_PIPELINE_WORKFLOWS
    {
        return Err(internal("tangled_pipeline_record"));
    }
    for run in &mut record.workflows {
        run.restore()?;
    }
    Ok(())
}

fn pipeline_not_found() -> Rejection {
    Rejection::new(
        RejectionClass::NotFound("PipelineNotFound"),
        "tangled_pipeline_unknown",
        "Pipeline not found",
    )
}

/// The record must belong to the named repository; otherwise it does not exist
/// for this caller.
pub fn owned(record: &PipelineRecord, repo_did: &str) -> TangledResult<()> {
    if record.repo_did == repo_did {
        Ok(())
    } else {
        Err(pipeline_not_found())
    }
}

pub fn not_found() -> Rejection {
    pipeline_not_found()
}

// r[impl kiln.tangled.untrusted]
/// `approvePipeline` through the #12 approval decision: only a pinned approver
/// (the Operator authority) may release a blocked pipeline, and only for its
/// exact commit.
pub fn approve(
    record: &mut PipelineRecord,
    pin: &RepositoryPin,
    approver: &str,
    now: i64,
) -> TangledResult<()> {
    owned(record, &pin.repo_did)?;
    let not_blocked = Rejection::new(
        RejectionClass::Precondition("PipelineNotBlocked"),
        "tangled_approval_not_blocked",
        "Pipeline has no workflow waiting for approval",
    );
    let authority = if pin.approvers.iter().any(|did| did == approver) {
        ApproverAuthority::Operator
    } else {
        ApproverAuthority::Viewer
    };
    let command = ApproveUntrustedRun {
        actor: Approver {
            id: approver.to_string(),
            label: approver.to_string(),
            authority,
        },
        run: record.pipeline_id.clone(),
        source_commit: record.commit.as_str().to_string(),
    };
    let approved_at_ms = u64::try_from(now)
        .unwrap_or_default()
        .saturating_mul(MILLISECONDS_PER_SECOND);
    let pipeline_id = record.pipeline_id.clone();
    let Some(gate) = record.untrusted.as_mut() else {
        // Authorization still precedes state, as in #12.
        if authority != ApproverAuthority::Operator {
            return Err(unauthorized());
        }
        return Err(not_blocked);
    };
    match decide_approval(&pipeline_id, gate, &command, approved_at_ms) {
        Ok(approval) => {
            gate.approval = Some(approval);
            Ok(())
        }
        Err(ApprovalRejection::Unauthorized | ApprovalRejection::InvalidActor) => {
            Err(unauthorized())
        }
        Err(ApprovalRejection::AlreadyApproved | ApprovalRejection::Superseded) => Err(not_blocked),
        Err(_) => Err(internal("tangled_approval_binding")),
    }
}

fn unauthorized() -> Rejection {
    Rejection::forbidden(
        "tangled_approval_unauthorized",
        "Only a pinned approver may approve blocked pipelines",
    )
}

// r[impl kiln.tangled.xrpc]
/// `retryPipeline`: duplicate a pipeline's stored, reviewed definitions at
/// the same commit. Approved untrusted pipelines stay untrusted.
pub fn plan_retry(
    profile: &AdmittedProfile,
    original: &mut PipelineRecord,
    pin: &RepositoryPin,
    caller: &str,
    workflows: &[String],
    now: i64,
) -> TangledResult<PipelineRecord> {
    owned(original, &pin.repo_did)?;
    if !pin
        .approvers
        .iter()
        .chain(&pin.trusted_actors)
        .any(|did| did == caller)
    {
        return Err(Rejection::forbidden(
            "tangled_retry_unauthorized",
            "Only a trusted actor or approver may retry pipelines",
        ));
    }
    if held(original) {
        return Err(Rejection::new(
            RejectionClass::Precondition("PipelineNotRetryable"),
            "tangled_retry_blocked",
            "Pipeline is still blocked",
        ));
    }
    let selected: Vec<ReviewedWorkflow> = original
        .workflows
        .iter()
        .filter(|run| workflows.is_empty() || workflows.contains(&run.definition.id))
        .map(|run| run.definition.clone())
        .collect();
    if selected.is_empty()
        || workflows.iter().any(|name| {
            !original
                .workflows
                .iter()
                .any(|run| run.definition.id == *name)
        })
    {
        return Err(Rejection::invalid(
            "tangled_retry_workflows",
            "Named workflows are not part of the pipeline",
        ));
    }
    let ordinal = original
        .retries
        .checked_add(1)
        .ok_or(internal("tangled_retry_ordinal"))?;
    let replay_key = identity(&(
        "kiln.tangled.retry.v1",
        &original.identity,
        ordinal,
        workflows,
    ))?;
    let identity = identity(&(PIPELINE_SCHEMA, &profile.identity, &replay_key))?;
    let mut record = PipelineRecord {
        schema: PIPELINE_SCHEMA.to_string(),
        pipeline_id: pipeline_id(&identity),
        identity,
        replay_key,
        profile_identity: profile.identity.clone(),
        repo_did: original.repo_did.clone(),
        commit: original.commit.clone(),
        source_repo: original.source_repo.clone(),
        trigger: original.trigger.clone(),
        native: original.native.clone(),
        actor: caller.to_string(),
        trust: original.trust,
        // An approved gate stays approved for the same commit: retries run untrusted.
        untrusted: original.untrusted.clone(),
        created_at: now,
        retry_of: Some(original.pipeline_id.clone()),
        retries: 0,
        workflows: Vec::new(),
        publications: Vec::new(),
        notes: Vec::new(),
    };
    for workflow in selected {
        let run = plan_run(profile, &record, workflow)?;
        record.workflows.push(run);
    }
    original.retries = ordinal;
    Ok(record)
}
