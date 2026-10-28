use crate::profile::ProjectProfile;
use kiln_core::domain::{Command, CommandEnvelope, Event, Outcome, State, decide, evolve};
use kiln_core::source::SourceResolution;
use kiln_core::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CandidateId, CommandId, ContentId, EffectId,
    ExternalRef, Limits, PipelineRevisionId, PolicyId, ProviderProfileId, RunId,
};
use kiln_core::workflow::{
    WorkflowControls, WorkflowEdge, WorkflowEffectPlan, WorkflowNode, WorkflowNodeState,
    WorkflowObservation, WorkflowSourceContext, WorkflowState,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableState {
    pub version: u64,
    pub enrolled: BTreeSet<String>,
    pub runs: BTreeMap<String, RunRecord>,
    pub requests: BTreeMap<String, RequestRecord>,
    #[serde(default)]
    pub audit: Vec<AuditRecord>,
    #[serde(default)]
    pub automation: crate::automation::AutomationState,
    #[serde(default)]
    pub access_policy_identity: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub snapshots: BTreeMap<String, crate::snapshot::SnapshotRecord>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Enroll,
    Start,
    Control,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestRecord {
    pub fingerprint: ContentId,
    pub resource: String,
    pub kind: RequestKind,
    pub response: Option<serde_json::Value>,
    pub status: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRecord {
    pub ordinal: u64,
    pub recorded_at_ms: u64,
    pub actor_id: String,
    pub actor_label: String,
    pub role: String,
    pub method: String,
    pub target: String,
    pub decision: String,
    pub status: u16,
    pub run_id: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTiming {
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportRecord {
    pub id: String,
    pub name: String,
    pub node_id: String,
    pub attempt_id: AttemptId,
    pub source_revision: String,
    pub execution: ExternalRef,
    pub kind: crate::reports::ReportKind,
    pub artifact: Option<ArtifactView>,
    pub report: Option<crate::reports::Report>,
    pub unavailable: Option<String>,
    pub recorded_at_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactView {
    pub id: String,
    pub name: String,
    pub node_id: String,
    pub attempt_id: AttemptId,
    pub bytes: usize,
    pub blake3: String,
    pub download_path: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogChunk {
    pub id: String,
    pub attempt_id: AttemptId,
    pub execution: ExternalRef,
    pub source_revision: String,
    pub node_id: String,
    pub stream: String,
    pub cursor: u64,
    pub bytes: usize,
    pub blake3: String,
    pub truncated: bool,
}
impl LogChunk {
    pub fn identity(&self, run_id: &str) -> Result<ContentId, String> {
        super::identity(&(
            "kiln.web-worker-log.v1",
            run_id,
            &self.attempt_id,
            &self.execution,
            &self.source_revision,
            &self.node_id,
            &self.stream,
            self.cursor,
            self.bytes,
            &self.blake3,
            self.truncated,
        ))
    }
}
#[derive(Serialize)]
pub struct LogBounds {
    pub maximum_chunk_bytes: usize,
    pub maximum_total_bytes: usize,
    pub maximum_chunks: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub id: String,
    pub ordinal: u64,
    pub project_id: String,
    pub pipeline_identity: ContentId,
    pub source: SourceResolution,
    pub plan: WorkflowEffectPlan,
    pub events: Vec<Event>,
    #[serde(default)]
    pub event_recorded_at_ms: Vec<Option<u64>>,
    pub phase: String,
    pub observation: Option<WorkflowObservation>,
    pub error_code: Option<String>,
    pub artifacts: Vec<ArtifactView>,
    #[serde(default)]
    pub logs: Vec<LogChunk>,
    #[serde(default)]
    pub created_at_ms: Option<u64>,
    #[serde(default)]
    pub started_at_ms: Option<u64>,
    #[serde(default)]
    pub completed_at_ms: Option<u64>,
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub selected_nodes: Vec<String>,
    #[serde(default)]
    pub node_timing: BTreeMap<String, NodeTiming>,
    #[serde(default)]
    pub reports: Vec<ReportRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_telemetry: Vec<crate::telemetry::TelemetryRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_snapshot: Option<crate::snapshot::SnapshotProvenance>,
    #[serde(skip)]
    pub state: State,
}
#[derive(Serialize)]
pub struct NodeView {
    pub id: String,
    pub state: WorkflowNodeState,
    pub kind: String,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}
#[derive(Serialize)]
pub struct RunView {
    pub id: String,
    pub project_id: String,
    pub source_revision: String,
    pub canonical_tree: ContentId,
    pub repository: ExternalRef,
    pub revision: u64,
    pub attempt_id: AttemptId,
    pub outcome: Outcome,
    pub phase: String,
    pub execution: Option<ExternalRef>,
    pub sequence: u64,
    pub nodes: Vec<NodeView>,
    pub controls: WorkflowControls,
    pub error_code: Option<String>,
    pub log_bounds: LogBounds,
    pub ordinal: u64,
    pub parent_run_id: Option<String>,
    pub selected_nodes: Vec<String>,
    pub edges: Vec<WorkflowEdge>,
    pub created_at_ms: Option<u64>,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub report_summary: serde_json::Value,
    pub step_telemetry: Vec<crate::telemetry::TelemetryRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_snapshot: Option<crate::snapshot::SnapshotProvenance>,
}
impl RunRecord {
    pub fn restore(&mut self) -> Result<(), String> {
        kiln_core::source::validate_resolution(&self.source)
            .map_err(|_| "stored source resolution rejected")?;
        kiln_core::workflow::validate_workflow_plan(&self.plan)
            .map_err(|_| "stored workflow plan rejected")?;
        self.state = State::default();
        for event in &self.events {
            self.state = evolve(&self.state, event)
                .map_err(|_| "stored canonical event history rejected")?;
        }
        if self.state.runs.len() != 1
            || self.state.attempts.len() != 1
            || !self.state.attempts.contains_key(&self.plan.attempt_id)
            || !self
                .state
                .runs
                .contains_key(&RunId::new(self.id.clone()).map_err(|_| "stored run id rejected")?)
        {
            return Err("stored canonical identity conflict".into());
        }
        if let Some(obs) = &self.observation {
            kiln_core::workflow::admit_workflow_observation(&self.plan, obs)
                .map_err(|_| "stored observation rejected")?;
        }
        super::telemetry::validate(self)?;
        Ok(())
    }
    pub fn outcome(&self) -> Outcome {
        self.state
            .attempts
            .get(&self.plan.attempt_id)
            .map(|a| a.outcome)
            .unwrap_or(Outcome::Unknown)
    }
    pub fn apply(&mut self, command: Command) -> Result<(), String> {
        let envelope = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new(format!(
                "web-command-{}-{}",
                self.plan.idempotency_key.as_str(),
                self.state.revision
            ))
            .map_err(|_| "command identity invalid")?,
            expected_revision: self.state.revision,
            command,
        };
        let decision = decide(&self.state, &envelope, Limits::default())
            .map_err(|_| "canonical command rejected")?;
        let recorded_at = if self.created_at_ms.is_some() {
            Some(super::host_time_ms()?)
        } else {
            None
        };
        self.event_recorded_at_ms.resize(self.events.len(), None);
        for event in decision.events {
            self.state = evolve(&self.state, &event).map_err(|_| "canonical event rejected")?;
            self.events.push(event);
            self.event_recorded_at_ms.push(recorded_at);
        }
        Ok(())
    }
    pub fn mark_unknown(&mut self) -> Result<(), String> {
        if self.outcome() != Outcome::Unknown {
            self.apply(Command::RecordUnknown {
                attempt_id: self.plan.attempt_id.clone(),
                external_reference: self.observation.as_ref().map(|v| v.execution.clone()),
            })?;
        }
        Ok(())
    }
    pub fn complete(
        &mut self,
        outcome: Outcome,
        reference: Option<ExternalRef>,
    ) -> Result<(), String> {
        let command = if self.outcome() == Outcome::Unknown {
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
        self.apply(command)
    }
    pub fn capabilities(&self) -> WorkflowControls {
        let available = self.phase == "active"
            && self.outcome() == Outcome::Pending
            && self.error_code.is_none();
        let state = self.observation.as_ref().map(|o| o.state);
        WorkflowControls {
            cancel: self.phase == "queued" || available && self.plan.controls.cancel,
            pause: available && self.plan.controls.pause && state == Some(WorkflowState::Running),
            resume: available && self.plan.controls.resume && state == Some(WorkflowState::Paused),
        }
    }
    pub fn view(&self) -> RunView {
        RunView {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            source_revision: self.source.exact_revision.native_id.as_str().to_string(),
            canonical_tree: self.source.canonical_tree.clone(),
            repository: self.source.repository.clone(),
            revision: self.state.revision,
            attempt_id: self.plan.attempt_id.clone(),
            outcome: self.outcome(),
            phase: self.phase.clone(),
            execution: self.observation.as_ref().map(|o| o.execution.clone()),
            sequence: self.observation.as_ref().map(|o| o.sequence).unwrap_or(0),
            nodes: self
                .plan
                .nodes
                .iter()
                .map(|n| NodeView {
                    id: n.id.as_str().to_string(),
                    kind: n.kind.as_str().to_owned(),
                    started_at_ms: self
                        .node_timing
                        .get(n.id.as_str())
                        .and_then(|t| t.started_at_ms),
                    completed_at_ms: self
                        .node_timing
                        .get(n.id.as_str())
                        .and_then(|t| t.completed_at_ms),
                    duration_ms: self
                        .node_timing
                        .get(n.id.as_str())
                        .and_then(|t| duration(t.started_at_ms, t.completed_at_ms)),
                    state: self
                        .observation
                        .as_ref()
                        .and_then(|o| o.nodes.iter().find(|v| v.node_id == n.id))
                        .map(|v| v.state)
                        .unwrap_or(if self.phase == "queued" {
                            WorkflowNodeState::Pending
                        } else {
                            WorkflowNodeState::Unknown
                        }),
                })
                .collect(),
            controls: self.capabilities(),
            error_code: self.error_code.clone(),
            ordinal: self.ordinal,
            parent_run_id: self.parent_run_id.clone(),
            selected_nodes: self.selected_nodes.clone(),
            edges: self.plan.edges.clone(),
            created_at_ms: self.created_at_ms,
            started_at_ms: self.started_at_ms,
            completed_at_ms: self.completed_at_ms,
            duration_ms: duration(self.started_at_ms, self.completed_at_ms),
            report_summary: super::reports::summary(self),
            step_telemetry: self.step_telemetry.clone(),
            dirty_snapshot: self.dirty_snapshot.clone(),
            log_bounds: LogBounds {
                maximum_chunk_bytes: self.plan.bounds.max_log_read_bytes,
                maximum_total_bytes: self.plan.bounds.max_total_log_bytes,
                maximum_chunks: self.plan.bounds.max_log_references,
            },
        }
    }
}

pub fn workflow_plan(
    project: &ProjectProfile,
    run: &ContentId,
    pipeline: &ContentId,
    source: Option<WorkflowSourceContext>,
) -> Result<WorkflowEffectPlan, String> {
    let config = &project.workflow;
    let result = (|| -> Result<_, kiln_core::types::Diagnostic> {
        Ok(WorkflowEffectPlan {
            effect_id: EffectId::new(format!("web-effect-{}", run.as_str()))?,
            attempt_id: AttemptId::new(format!("web-attempt-{}", run.as_str()))?,
            candidate_id: CandidateId::new(format!("web-candidate-{}", run.as_str()))?,
            pipeline_revision_id: PipelineRevisionId::new(format!(
                "web-pipeline-{}",
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
                .map(|n| {
                    Ok(WorkflowNode {
                        id: Alias::new(n.id.clone())?,
                        kind: Alias::new(n.kind.clone())?,
                        effect_class: n.effect_class.into(),
                        adapter_kind: Alias::new(n.adapter_kind.clone())?,
                        operation_identity: ContentId::from_blake3_hex(
                            n.operation_identity.clone(),
                        )?,
                    })
                })
                .collect::<Result<_, kiln_core::types::Diagnostic>>()?,
            edges: config
                .edges
                .iter()
                .map(|e| {
                    Ok(WorkflowEdge {
                        source: Alias::new(e.source.clone())?,
                        target: Alias::new(e.target.clone())?,
                    })
                })
                .collect::<Result<_, kiln_core::types::Diagnostic>>()?,
            controls: config.controls,
            bounds: config.bounds,
        })
    })();
    let plan = result.map_err(|_| "workflow identity rejected")?;
    kiln_core::workflow::validate_workflow_plan(&plan).map_err(|_| "workflow plan rejected")?;
    Ok(plan)
}

pub fn duration(start: Option<u64>, end: Option<u64>) -> Option<u64> {
    end?.checked_sub(start?)
}

pub(crate) fn selected_plan(
    mut plan: WorkflowEffectPlan,
    selected: &[String],
) -> Result<WorkflowEffectPlan, String> {
    if !selected.is_empty() {
        let ids: BTreeSet<_> = selected.iter().map(String::as_str).collect();
        if ids.len() != selected.len()
            || ids
                .iter()
                .any(|id| !plan.nodes.iter().any(|n| n.id.as_str() == *id))
        {
            return Err("retry node selection rejected".into());
        }
        plan.nodes.retain(|n| ids.contains(n.id.as_str()));
        plan.edges
            .retain(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()));
        kiln_core::workflow::validate_workflow_plan(&plan).map_err(|_| "retry graph rejected")?;
    }
    Ok(plan)
}
