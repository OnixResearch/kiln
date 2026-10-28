use super::api::error;
use super::model::{RequestKind, RunRecord, selected_plan, workflow_plan};
use super::{ApiResponse, Engine, host_time_ms, identity};
use crate::profile::ProjectProfile;
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::domain::{Command, Outcome, State};
use kiln_core::source::SourceResolution;
use kiln_core::types::{Alias, ExternalRef, ProviderProfileId, RunId};
use kiln_core::workflow::{WorkflowNodeState, WorkflowSourceContext};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartRequest {
    pub project_id: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RerunRequest {
    request_id: String,
    mode: String,
}

struct CandidateSource {
    resolution: SourceResolution,
    dirty_snapshot: Option<crate::snapshot::SnapshotProvenance>,
}

pub(super) fn resolve_source(
    project: &ProjectProfile,
    reference: &str,
    exact: Option<&str>,
) -> Result<SourceResolution, String> {
    if reference != project.reference && !project.admitted_references.iter().any(|r| r == reference)
    {
        return Err("source_reference_not_admitted".into());
    }
    let selected = if let Some(revision) = exact {
        if revision.len() != 40
            || !revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("source_revision_invalid".into());
        }
        let repository =
            git2::Repository::open(&project.repository_path).map_err(|_| "source_unavailable")?;
        let head = repository
            .find_reference(reference)
            .and_then(|r| r.peel_to_commit())
            .map_err(|_| "source_unavailable")?;
        let oid = git2::Oid::from_str(revision).map_err(|_| "source_revision_invalid")?;
        if oid != head.id()
            && !repository
                .graph_descendant_of(head.id(), oid)
                .map_err(|_| "source_revision_unreachable")?
        {
            return Err("source_revision_unreachable".into());
        }
        revision
    } else {
        reference
    };
    let provider = ProviderProfileId::new(format!("web-git-{}", project.id))
        .map_err(|_| "source_profile_invalid")?;
    let adapter = LocalGitAdapter::new(
        project.repository_path.clone(),
        selected.to_owned(),
        GitSourceProfile::pinned_local(provider.clone(), project.source_limits.clone()),
    )
    .map_err(|_| "source_unavailable")?;
    resolve_and_acquire(&adapter, provider).map_err(|_| "source_unavailable".into())
}

impl Engine {
    pub(super) fn start_run(&mut self, body: &[u8]) -> Result<ApiResponse, String> {
        let req: StartRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "request_invalid"),
        };
        if !crate::profile::identifier(&req.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&("/api/runs", &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        let Some(project) = self
            .profile
            .projects
            .iter()
            .find(|p| p.id == req.project_id)
            .cloned()
        else {
            return error(404, "project_not_allowlisted");
        };
        if !self.state.enrolled.contains(&project.id) {
            return error(409, "project_not_enrolled");
        }
        let dirty_snapshot = if let Some(snapshot_id) = &req.snapshot_id {
            let Some(record) = self.state.snapshots.get(snapshot_id).cloned() else {
                return error(404, "snapshot_missing");
            };
            if record.manifest.project_id != project.id
                || req.reference.as_deref().unwrap_or(&project.reference)
                    != record.manifest.reference
                || req.source_revision.as_deref() != Some(record.manifest.base_revision.as_str())
            {
                return error(400, "snapshot_input_mismatch");
            }
            Some(record)
        } else {
            None
        };
        let source = match if let Some(record) = &dirty_snapshot {
            super::snapshots::resolve_snapshot(&project, record)
        } else {
            resolve_source(
                &project,
                req.reference.as_deref().unwrap_or(&project.reference),
                req.source_revision.as_deref(),
            )
        } {
            Ok(v) => v,
            Err(code) => {
                return error(
                    if code == "source_unavailable" {
                        503
                    } else {
                        400
                    },
                    &code,
                );
            }
        };
        self.enqueue(
            &project,
            &req.request_id,
            fingerprint,
            CandidateSource {
                resolution: source,
                dirty_snapshot: dirty_snapshot.map(|record| record.provenance),
            },
            None,
            Vec::new(),
        )
    }
    pub(super) fn rerun(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        let req: RerunRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "rerun_invalid"),
        };
        if !crate::profile::identifier(&req.request_id)
            || !matches!(req.mode.as_str(), "exact" | "latest" | "failed")
        {
            return error(400, "rerun_invalid");
        }
        let fingerprint = identity(&("rerun", id, &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        let prior = self.state.runs.get(id).ok_or("run missing")?;
        if !prior.outcome().is_terminal() {
            return error(409, "rerun_outcome_unknown_or_active");
        }
        let project = self
            .profile
            .projects
            .iter()
            .find(|p| p.id == prior.project_id)
            .cloned()
            .ok_or("project missing")?;
        let mut selected = BTreeSet::new();
        if req.mode == "failed" {
            let Some(observation) = &prior.observation else {
                return error(409, "failed_nodes_unavailable");
            };
            if observation
                .nodes
                .iter()
                .any(|n| n.state == WorkflowNodeState::Unknown)
            {
                return error(409, "failed_nodes_unknown");
            }
            selected.extend(
                observation
                    .nodes
                    .iter()
                    .filter(|n| n.state == WorkflowNodeState::Failed)
                    .map(|n| n.node_id.as_str().to_owned()),
            );
            if selected.is_empty() {
                return error(409, "failed_nodes_unavailable");
            }
            loop {
                let before = selected.len();
                for edge in &project.workflow.edges {
                    if selected.contains(&edge.source) {
                        selected.insert(edge.target.clone());
                    }
                }
                if before == selected.len() {
                    break;
                }
            }
        }
        if req.mode == "latest" && prior.dirty_snapshot.is_some() {
            return error(409, "dirty_retry_requires_exact_source");
        }
        let source = if req.mode == "latest" {
            match resolve_source(&project, &project.reference, None) {
                Ok(v) => v,
                Err(code) => return error(503, &code),
            }
        } else {
            prior.source.clone()
        };
        let dirty_snapshot = if req.mode == "latest" {
            None
        } else {
            prior.dirty_snapshot.clone()
        };
        if let Some(provenance) = &dirty_snapshot {
            let Some(record) = self.state.snapshots.get(&provenance.manifest_id) else {
                return error(409, "snapshot_missing");
            };
            match super::snapshots::resolve_snapshot(&project, record) {
                Ok(resolved) if resolved == source => {}
                _ => return error(409, "snapshot_retry_source_unavailable"),
            }
        }
        self.enqueue(
            &project,
            &req.request_id,
            fingerprint,
            CandidateSource {
                resolution: source,
                dirty_snapshot,
            },
            Some(id.to_owned()),
            selected.into_iter().collect(),
        )
    }
    fn enqueue(
        &mut self,
        project: &ProjectProfile,
        request_id: &str,
        fingerprint: kiln_core::types::ContentId,
        candidate: CandidateSource,
        parent: Option<String>,
        selected: Vec<String>,
    ) -> Result<ApiResponse, String> {
        let CandidateSource {
            resolution: source,
            dirty_snapshot,
        } = candidate;
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }
        if self.state.runs.len() >= self.profile.bounds.max_runs {
            return error(409, "run_retention");
        }
        let pipeline = identity(&project.workflow)?;
        let run_identity = identity(&(
            &self.profile.service_id,
            request_id,
            &source,
            &pipeline,
            &parent,
            &selected,
        ))?;
        let id = format!("web-run-{}", run_identity.as_str());
        let context = WorkflowSourceContext {
            provider: Alias::new("git").map_err(|_| "git alias invalid")?,
            repository: source.repository.clone(),
            revision: ExternalRef::new("git", "revision", source.exact_revision.native_id.as_str())
                .map_err(|_| "source revision invalid")?,
        };
        let plan = selected_plan(
            workflow_plan(project, &run_identity, &pipeline, Some(context))?,
            &selected,
        )?;
        kiln_adapter_lattice::lower_request(&project.lattice(self.profile.bounds.max_runs)?, &plan)
            .map_err(|_| "source-bound workflow rejected")?;
        let mut record = RunRecord {
            id: id.clone(),
            ordinal: self.state.version,
            project_id: project.id.clone(),
            pipeline_identity: pipeline,
            source,
            dirty_snapshot,
            step_telemetry: Vec::new(),
            plan: plan.clone(),
            events: Vec::new(),
            event_recorded_at_ms: Vec::new(),
            phase: "queued".into(),
            observation: None,
            error_code: None,
            artifacts: Vec::new(),
            logs: Vec::new(),
            created_at_ms: Some(host_time_ms()?),
            started_at_ms: None,
            completed_at_ms: None,
            parent_run_id: parent,
            selected_nodes: selected,
            node_timing: BTreeMap::new(),
            reports: Vec::new(),
            state: State::default(),
        };
        record.apply(Command::StartRun {
            run_id: RunId::new(id.clone()).map_err(|_| "run identity invalid")?,
            attempt_id: plan.attempt_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            effect_id: plan.effect_id.clone(),
        })?;
        self.state.runs.insert(id.clone(), record);
        self.reserve_request(
            request_id.to_owned(),
            fingerprint,
            id.clone(),
            RequestKind::Start,
        )?;
        self.dispatch_queued()?;
        let run = self.state.runs.get(&id).ok_or("run missing")?;
        let status = if run.phase == "queued" { 202 } else { 201 };
        self.finish_request(
            request_id,
            status,
            serde_json::to_value(run.view()).map_err(|_| "run view encoding failed")?,
        )
    }
    pub(super) fn dispatch_queued(&mut self) -> Result<(), String> {
        let mut busy: BTreeSet<_> = self
            .state
            .runs
            .values()
            .filter(|r| !r.outcome().is_terminal() && r.phase != "queued")
            .map(|r| r.project_id.clone())
            .collect();
        let mut queued: Vec<_> = self
            .state
            .runs
            .values()
            .filter(|r| r.phase == "queued")
            .map(|r| (r.ordinal, r.id.clone(), r.project_id.clone()))
            .collect();
        queued.sort();
        for (_, id, project) in queued {
            if self.worker_capacity_available(&project, host_time_ms()?)? == Some(false) {
                continue;
            }
            if !busy.insert(project.clone()) {
                continue;
            }
            let run = self.state.runs.get_mut(&id).ok_or("queued run missing")?;
            run.phase = "dispatch_reserved".into();
            let plan = run.plan.clone();
            // Reserve durably before the sole dispatch call. Restart recovers, never dispatches this phase.
            self.persist()?;
            let result = self
                .ports
                .get_mut(&project)
                .ok_or("project port missing")?
                .dispatch(&plan);
            self.accept(&id, result)?;
        }
        Ok(())
    }
    pub(super) fn cancel_queued(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        #[derive(Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        struct Cancel {
            request_id: String,
            action: String,
        }
        let req: Cancel = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "queue_control_invalid"),
        };
        if req.action != "cancel" || !crate::profile::identifier(&req.request_id) {
            return error(400, "queue_control_invalid");
        }
        let fingerprint = identity(&(id, &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        if run.phase != "queued" {
            return error(409, "queue_control_stale");
        }
        run.complete(Outcome::Cancelled, None)?;
        run.phase = "terminal".into();
        run.completed_at_ms = Some(host_time_ms()?);
        self.reserve_request(
            req.request_id.clone(),
            fingerprint,
            id.to_owned(),
            RequestKind::Control,
        )?;
        let value = serde_json::to_value(self.state.runs.get(id).ok_or("run missing")?.view())
            .map_err(|_| "run view encoding failed")?;
        self.finish_request(&req.request_id, 200, value)
    }
}
