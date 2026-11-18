//! Shell for the off-by-default review-queue wait fixture (#61). One admitted
//! load runs through the real review queue in each declared mode: a disposable
//! bare Git repository, the SQLite review store, the engine's single project
//! CI slot and a scripted workflow provider. Each tick runs these steps in
//! order: enqueue arrivals, withdraw entries past their submitter deadline,
//! apply due CI results, poll the engine, advance the queue, observe. Every
//! time is the first tick boundary at which the public API showed it.
// r[impl kiln.admission.queue_wait]
use std::collections::BTreeMap;
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use git2::{Oid, Repository, Signature, Time};
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{Diagnostic, EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{PortFailure, PortResult, WorkflowPort};
use kiln_web_service::code_review::queue_wait::{
    ArmObservation, CiResult, Load, Mode, ModeSpec, Observed, Transition,
};
use kiln_web_service::code_review::{Profile, RequestContext, Workspace};
use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::profile::ServiceProfile;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;

pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const AUTHOR: &str = "local:author";
const REVIEWER: &str = "local:reviewer";
const SUBMITTER: &str = "local:submitter";
const MAIN: &str = "refs/heads/main";
const PROJECT: &str = "fixture";
/// Synthetic request time of tick 0; setup commands run before it.
const EPOCH_MS: i64 = 1_000_000;
/// Synthetic spacing of setup commands.
const SETUP_STEP_MS: i64 = 1_000;
/// Candidate builds one entry may attempt, as in the default review profile.
const MAX_CANDIDATE_BUILDS: u32 = 16;
/// Concurrent canonical CI executions: the engine dispatches one per project.
const CI_SLOTS: usize = 1;
const TERMINAL: [&str; 3] = ["submitted", "blocked", "cancelled"];
const ACTIVE: [&str; 5] = [
    "queued",
    "preparing",
    "waiting_ci",
    "submitting",
    "uncertain",
];
/// Statuses that show the queue constructing or testing an exact candidate.
const STARTED: [&str; 5] = [
    "preparing",
    "waiting_ci",
    "submitting",
    "uncertain",
    "submitted",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reported {
    Running,
    Passed,
    Failed,
    Unknown,
}

struct Execution {
    plan: WorkflowEffectPlan,
    sequence: u64,
    reported: Reported,
    dispatched_tick: u32,
}

#[derive(Default)]
struct Backend {
    tick: u32,
    executions: BTreeMap<String, Execution>,
    /// Exact candidate commit and tick of every dispatch, in order.
    dispatches: Vec<(String, u32)>,
}

struct Port {
    profile: CapabilityProfile,
    backend: Arc<Mutex<Backend>>,
}

fn reference(kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new("lattice", kind, value)
        .unwrap_or_else(|error| unreachable!("fixture reference: {error:?}"))
}

fn candidate_commit(plan: &WorkflowEffectPlan) -> Option<String> {
    plan.source
        .as_ref()
        .map(|source| source.revision.native_id.as_str().to_owned())
}

fn observation(execution: &Execution) -> WorkflowObservation {
    let plan = &execution.plan;
    let (state, node_state, outcome) = match execution.reported {
        Reported::Running => (
            WorkflowState::Running,
            Some(WorkflowNodeState::Running),
            Outcome::Pending,
        ),
        Reported::Passed => (
            WorkflowState::Completed,
            Some(WorkflowNodeState::Completed),
            Outcome::Success,
        ),
        Reported::Failed => (
            WorkflowState::Failed,
            Some(WorkflowNodeState::Failed),
            Outcome::DomainFailure,
        ),
        // A provider that cannot say what happened reports no node results.
        Reported::Unknown => (WorkflowState::Unknown, None, Outcome::Unknown),
    };
    let terminal = state.is_terminal();
    let successful = execution.reported == Reported::Passed;
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: reference("workflow", "queue-wait-workflow"),
        execution: reference("execution", plan.effect_id.as_str()),
        sequence: execution.sequence,
        state,
        terminal,
        successful,
        outcome,
        nodes: node_state.map_or_else(Vec::new, |node_state| {
            plan.nodes
                .iter()
                .map(|node| WorkflowNodeObservation {
                    node_id: node.id.clone(),
                    state: node_state,
                    terminal,
                    successful,
                    adapter: reference("adapter", "local"),
                    receipt: terminal.then(|| reference("receipt", "queue-wait-node")),
                })
                .collect()
        }),
        logs: Vec::new(),
        receipt: terminal.then(|| reference("receipt", "queue-wait-execution")),
    }
}

fn unsupported() -> PortFailure {
    PortFailure::Domain(Diagnostic::new(
        "unsupported",
        "queue_wait.fixture",
        "The queue-wait fixture admits workflow dispatch and observation only",
    ))
}

impl WorkflowPort for Port {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut backend = self.backend.lock();
        let key = plan.effect_id.as_str().to_owned();
        if backend.executions.contains_key(&key) {
            return Err(unsupported());
        }
        let execution = Execution {
            plan: plan.clone(),
            sequence: 1,
            reported: Reported::Running,
            dispatched_tick: backend.tick,
        };
        let result = observation(&execution);
        if let Some(commit) = candidate_commit(plan) {
            let tick = backend.tick;
            backend.dispatches.push((commit, tick));
        }
        backend.executions.insert(key, execution);
        Ok(result)
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        self.backend
            .lock()
            .executions
            .get(plan.effect_id.as_str())
            .map(observation)
            .ok_or_else(unsupported)
    }
    fn observe(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        self.backend
            .lock()
            .executions
            .get(id.as_str())
            .map(observation)
            .ok_or_else(unsupported)
    }
    fn reconcile(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(id)
    }
    fn control(&mut self, _: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        Err(unsupported())
    }
    fn read_log(&mut self, _: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        Err(unsupported())
    }
}

fn commit(
    repo: &Repository,
    parent: Option<Oid>,
    message: &str,
    files: &[(&str, &str)],
) -> Result<Oid> {
    let mut builder = repo.treebuilder(None)?;
    for (path, text) in files {
        builder.insert(*path, repo.blob(text.as_bytes())?, 0o100644)?;
    }
    let tree = repo.find_tree(builder.write()?)?;
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|id| repo.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repo.commit(None, &signature, &signature, message, &tree, &parents)?)
}

fn body(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap_or(Value::Null)
}

fn accepted(response: ApiResponse) -> Result<Value> {
    if response.status != 200 {
        return Err(format!(
            "status {}: {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )
        .into());
    }
    Ok(body(&response))
}

fn text(value: &Value, pointer: &str) -> Result<String> {
    Ok(value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {pointer}"))?
        .to_owned())
}

fn push_new(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|known| known == value) {
        values.push(value.to_owned());
    }
}

struct Arm<'a> {
    load: &'a Load,
    _root: TempDir,
    repository_path: std::path::PathBuf,
    workspace: Workspace,
    engine: Engine,
    backend: Arc<Mutex<Backend>>,
    changes: Vec<String>,
    observed: Vec<Observed>,
    setup_ms: i64,
}

impl<'a> Arm<'a> {
    fn new(load: &'a Load, mode: &ModeSpec) -> Result<Self> {
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = root.path().join("repository.git");
        let repo = Repository::init_bare(&path)?;
        let base = commit(&repo, None, "base", &[("base.txt", "base\n")])?;
        let mut sources = Vec::with_capacity(load.candidates.len());
        for candidate in &load.candidates {
            let file = format!("{}.txt", candidate.id);
            let source = commit(
                &repo,
                Some(base),
                &candidate.id,
                &[("base.txt", "base\n"), (file.as_str(), "reviewed\n")],
            )?;
            sources.push(source.to_string());
        }
        let target = commit(
            &repo,
            Some(base),
            "target",
            &[("base.txt", "base\n"), ("target.txt", "target\n")],
        )?;
        repo.reference(MAIN, target, false, "disposable fixture")?;
        let canonical = path.canonicalize()?;
        let repository_id = kiln_codec::identity_from_bytes(
            canonical.to_str().ok_or("non-UTF8 fixture")?.as_bytes(),
        )?
        .as_str()
        .to_owned();
        let mut queue = json!({"schema":"kiln.review-queue.v1","max_entries":load.max_entries,
            "max_candidate_builds":MAX_CANDIDATE_BUILDS});
        if let (Mode::Speculative, Some(window), Some(object)) =
            (mode.mode, mode.window, queue.as_object_mut())
        {
            object.insert("mode".into(), json!("speculative"));
            object.insert(
                "window".into(),
                json!({"floor":window.floor,"ceiling":window.ceiling,
                "increase":window.increase,"decrease":window.decrease}),
            );
        }
        let review: Profile = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1", "database":root.path().join("review.sqlite"), "project":PROJECT,
            "provider":{"repository_id":repository_id,"repository_path":path,"target_refs":[MAIN],
                "capabilities":["inspect","compare","register","submit","build_integration","speculative_integration"]},
            "policy":{"id":"queue-wait-policy","eligible_reviewers":[REVIEWER],"submitters":[SUBMITTER],
                "minimum_approvals":1,"require_ci":true},
            "read_principals":[AUTHOR,REVIEWER,SUBMITTER],"write_principals":[AUTHOR,REVIEWER,SUBMITTER],
            "signature_name":"Queue service","signature_email":"queue@example.invalid",
            "queue":queue
        }))?;
        let service: ServiceProfile = serde_json::from_value(json!({
            "schema":"kiln.web-service.v1","service_id":"queue-wait","listen_address":"127.0.0.1:9443",
            "origin":"https://localhost:9443","tls":{"certificate_path":root.path().join("cert.pem"),
                "private_key_path":root.path().join("key.pem"),"min_version":"TLS1.3"},
            "operator_token_file":root.path().join("operator.token"),"worker_token_file":root.path().join("worker.token"),
            "state_directory":root.path().join("state"),
            "projects":[{"id":PROJECT,"label":"Fixture","repository_path":path,"reference":MAIN,
                "source_limits":{"max_entries":32,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":16384},
                "workflow":{"socket_path":root.path().join("lattice.sock"),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],
                    "transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,
                        "read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},
                    "adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},
                    "policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64",
                    "nodes":[{"id":"job","kind":"test","effect_class":"test","adapter_kind":"local",
                        "operation_identity":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "edges":[],"controls":{"cancel":true,"pause":false,"resume":false},
                    "bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}}],
            "bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,
                "timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":128,"max_artifact_bytes":1024,"max_artifacts":2,
                "max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,
                "max_requests":512,"max_events_per_run":32,"max_state_bytes":8388608}
        }))?;
        let backend = Arc::new(Mutex::new(Backend::default()));
        let capability = service
            .projects
            .first()
            .ok_or("missing project")?
            .lattice(service.bounds.max_runs)?
            .provider;
        let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
        ports.insert(
            PROJECT.into(),
            Box::new(Port {
                profile: capability,
                backend: backend.clone(),
            }),
        );
        let mut engine = Engine::open_with_ports(service, ports)?;
        accepted(engine.api(
            "POST",
            "/api/projects",
            &serde_json::to_vec(&json!({"project_id":PROJECT,"request_id":"enroll"}))?,
            Role::Operator,
        )?)?;
        let workspace = Workspace::new(Some(review))?;
        let observed = load
            .candidates
            .iter()
            .zip(&sources)
            .map(|(candidate, source)| Observed {
                candidate: candidate.id.clone(),
                source_commit: source.clone(),
                ..Observed::default()
            })
            .collect();
        let mut arm = Self {
            load,
            _root: root,
            repository_path: path,
            workspace,
            engine,
            backend,
            changes: Vec::new(),
            observed,
            setup_ms: 0,
        };
        for (index, (candidate, source)) in load.candidates.iter().zip(&sources).enumerate() {
            arm.setup_ms += SETUP_STEP_MS;
            let created = accepted(arm.post(
                AUTHOR,
                arm.setup_ms,
                &json!({"request_id":format!("create-{index}"),"change_id":null,"expected_version":null,
                    "action":{"type":"create","commit":source,"target_ref":MAIN,"title":format!("Queue wait {}", candidate.id),
                        "description":"Synthetic parallel candidate","wip":false}}),
            )?)?;
            let change = text(&created, "/change/id")?;
            arm.changes.push(change.clone());
            if let Some(observed) = arm.observed.get_mut(index) {
                observed.change = Some(change);
            }
            for (actor, request, action) in [
                (
                    AUTHOR,
                    "assign",
                    json!({"type":"set_reviewer","principal":REVIEWER,"role":"reviewer"}),
                ),
                (
                    REVIEWER,
                    "approve",
                    json!({"type":"vote","value":"approve"}),
                ),
            ] {
                arm.setup_ms += SETUP_STEP_MS;
                accepted(arm.command(
                    index,
                    actor,
                    &format!("{request}-{index}"),
                    action,
                    arm.setup_ms,
                )?)?;
            }
        }
        if arm.setup_ms >= EPOCH_MS {
            return Err("setup overlaps the measured clock".into());
        }
        Ok(arm)
    }

    fn context(principal: &str, now_ms: i64) -> RequestContext<'_> {
        RequestContext {
            role: Role::Viewer,
            principal,
            now_ms,
            ci: &[],
        }
    }

    fn post(&self, actor: &str, now_ms: i64, request: &Value) -> Result<ApiResponse> {
        Ok(self.workspace.response(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(request)?,
            Self::context(actor, now_ms),
        ))
    }

    fn command(
        &self,
        index: usize,
        actor: &str,
        request: &str,
        action: Value,
        now_ms: i64,
    ) -> Result<ApiResponse> {
        let change = self.changes.get(index).ok_or("change")?;
        let detail = accepted(self.workspace.response(
            "GET",
            &format!("/api/code-review/changes/{change}"),
            &[],
            Self::context(AUTHOR, now_ms),
        ))?;
        let version = detail
            .pointer("/change/version")
            .cloned()
            .unwrap_or(Value::Null);
        self.post(
            actor,
            now_ms,
            &json!({"request_id":request,"change_id":change,"expected_version":version,"action":action}),
        )
    }

    fn target(&self) -> Result<String> {
        Ok(Repository::open_bare(&self.repository_path)?
            .refname_to_id(MAIN)?
            .to_string())
    }

    fn run(mut self, mode: &ModeSpec) -> Result<ArmObservation> {
        for tick in 0..=self.load.horizon_ticks {
            let at_ms = u64::from(tick).saturating_mul(self.load.tick_ms);
            let now_ms = EPOCH_MS
                .checked_add(i64::try_from(at_ms)?)
                .ok_or("synthetic clock overflow")?;
            self.backend.lock().tick = tick;
            self.arrive(tick, at_ms, now_ms)?;
            self.withdraw(tick, at_ms, now_ms)?;
            self.report(tick);
            let _notifications = self.engine.poll()?;
            self.workspace.advance_queue(&mut self.engine, now_ms)?;
            self.observe(at_ms, now_ms)?;
        }
        Ok(ArmObservation {
            mode: mode.clone(),
            ci_slots: CI_SLOTS,
            max_candidate_builds: MAX_CANDIDATE_BUILDS,
            observed: self.observed,
        })
    }

    fn arrive(&mut self, tick: u32, at_ms: u64, now_ms: i64) -> Result {
        let arrivals: Vec<usize> = self
            .load
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.arrival_tick == tick)
            .map(|(index, _)| index)
            .collect();
        for index in arrivals {
            let target = self.target()?;
            let response = self.command(
                index,
                SUBMITTER,
                &format!("enqueue-{index}"),
                json!({"type":"enqueue","expected_target":target}),
                now_ms,
            )?;
            let observed = self.observed.get_mut(index).ok_or("candidate")?;
            observed.arrival_ms = at_ms;
            if response.status == 200 {
                observed.enqueue_ms = Some(at_ms);
            } else {
                let code = body(&response)
                    .get("code")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("http_{}", response.status), str::to_owned);
                observed.refusal = Some(code);
                observed.terminal_ms = Some(at_ms);
            }
        }
        Ok(())
    }

    /// The synthetic submitter withdraws a still-active entry once its deadline passes.
    fn withdraw(&mut self, tick: u32, at_ms: u64, now_ms: i64) -> Result {
        let due: Vec<usize> = self
            .load
            .candidates
            .iter()
            .zip(&self.observed)
            .enumerate()
            .filter(|(_, (candidate, observed))| {
                let deadline = candidate
                    .deadline_ticks
                    .map(|ticks| u64::from(ticks).saturating_mul(self.load.tick_ms));
                match (observed.enqueue_ms, deadline) {
                    (Some(enqueued), Some(deadline)) => {
                        observed.withdrawn_ms.is_none()
                            && observed.terminal_ms.is_none()
                            && at_ms >= enqueued.saturating_add(deadline)
                    }
                    _ => false,
                }
            })
            .map(|(index, _)| index)
            .collect();
        for index in due {
            let response = self.command(
                index,
                SUBMITTER,
                &format!("withdraw-{index}-{tick}"),
                json!({"type":"dequeue"}),
                now_ms,
            )?;
            // A refused withdrawal (reserved operation) is retried next tick.
            if response.status == 200 {
                let observed = self.observed.get_mut(index).ok_or("candidate")?;
                observed.withdrawn_ms = Some(at_ms);
            }
        }
        Ok(())
    }

    /// Report the declared CI result of every execution that has run its `ci_ticks`.
    fn report(&self, tick: u32) {
        let mut backend = self.backend.lock();
        for execution in backend.executions.values_mut() {
            if execution.reported != Reported::Running {
                continue;
            }
            let Some(commit) = candidate_commit(&execution.plan) else {
                continue;
            };
            let Some(candidate) = self
                .observed
                .iter()
                .position(|observed| {
                    observed
                        .candidate_commits
                        .iter()
                        .any(|known| known == &commit)
                })
                .and_then(|index| self.load.candidates.get(index))
            else {
                continue;
            };
            if execution.dispatched_tick.saturating_add(candidate.ci_ticks) <= tick {
                execution.sequence += 1;
                execution.reported = match candidate.ci {
                    CiResult::Pass => Reported::Passed,
                    CiResult::Fail => Reported::Failed,
                    CiResult::Unknown => Reported::Unknown,
                };
            }
        }
    }

    fn observe(&mut self, at_ms: u64, now_ms: i64) -> Result {
        let snapshot = accepted(self.workspace.response(
            "GET",
            "/api/code-review",
            &[],
            Self::context(AUTHOR, now_ms),
        ))?;
        let entries = snapshot
            .get("queue")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let dispatches = self.backend.lock().dispatches.clone();
        for (index, change) in self.changes.iter().enumerate() {
            let Some(entry) = entries
                .iter()
                .rev()
                .find(|entry| entry.get("change").and_then(Value::as_str) == Some(change.as_str()))
            else {
                continue;
            };
            let status = text(entry, "/status")?;
            let observed = self.observed.get_mut(index).ok_or("candidate")?;
            if observed.transitions.last().map(|last| last.status.as_str()) != Some(status.as_str())
            {
                observed.transitions.push(Transition {
                    at_ms,
                    status: status.clone(),
                });
            }
            if STARTED.contains(&status.as_str()) && observed.start_ms.is_none() {
                observed.start_ms = Some(at_ms);
            }
            if TERMINAL.contains(&status.as_str()) && observed.terminal_ms.is_none() {
                observed.terminal_ms = Some(at_ms);
            }
            if let Some(candidate) = entry.get("candidate_commit").and_then(Value::as_str) {
                push_new(&mut observed.candidate_commits, candidate);
            }
            let runs: Vec<String> = match (entry.get("ci_runs"), entry.get("ci_run")) {
                (Some(Value::Array(runs)), _) => runs
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                (_, Some(Value::String(run))) => vec![run.clone()],
                _ => Vec::new(),
            };
            if !runs.is_empty() && observed.ci_requested_ms.is_none() {
                observed.ci_requested_ms = Some(at_ms);
            }
            for run in &runs {
                push_new(&mut observed.ci_runs, run);
            }
            observed.ci_dispatches_ms = dispatches
                .iter()
                .filter(|(commit, _)| {
                    observed
                        .candidate_commits
                        .iter()
                        .any(|known| known == commit)
                })
                .map(|(_, tick)| u64::from(*tick).saturating_mul(self.load.tick_ms))
                .collect();
            let mut unknown = false;
            if ACTIVE.contains(&status.as_str()) {
                for run in &runs {
                    let view = accepted(self.engine.api(
                        "GET",
                        &format!("/api/runs/{run}"),
                        &[],
                        Role::Operator,
                    )?)?;
                    unknown |= view.get("phase").and_then(Value::as_str) == Some("unknown");
                }
            }
            observed.ci_unknown = unknown;
            observed.detail = entry
                .get("detail")
                .and_then(Value::as_str)
                .map(str::to_owned);
            observed.status = Some(status);
        }
        Ok(())
    }
}

/// Run every declared queue mode over a fresh disposable fixture.
pub fn run(load: &Load) -> Result<Vec<ArmObservation>> {
    load.modes
        .iter()
        .map(|mode| Arm::new(load, mode)?.run(mode))
        .collect()
}
