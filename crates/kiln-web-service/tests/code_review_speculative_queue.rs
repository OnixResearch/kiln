//! Speculative review merge queue over real disposable Git, SQLite and the
//! canonical engine with deterministic workflow observations. No network,
//! installed credentials, real refs or client-supplied CI authority.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use git2::{Oid, Repository, Signature, Time};
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{Diagnostic, EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{PortFailure, PortResult, WorkflowPort};
use kiln_web_service::code_review::{Profile, RequestContext, Workspace};
use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::profile::ServiceProfile;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const AUTHOR: &str = "local:author";
const REVIEWER: &str = "local:reviewer";
const SUBMITTER: &str = "local:submitter";
const ADVISOR: &str = "local:advisor";
const MAIN: &str = "refs/heads/main";
const SOURCES: [&str; 3] = ["first", "second", "third"];

struct Execution {
    plan: WorkflowEffectPlan,
    sequence: u64,
    success: Option<bool>,
}
#[derive(Default)]
struct Backend {
    executions: BTreeMap<String, Execution>,
    dispatches: usize,
}
struct Port {
    profile: CapabilityProfile,
    backend: Arc<Mutex<Backend>>,
}
fn checked<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}
fn observation(execution: &Execution) -> WorkflowObservation {
    let plan = &execution.plan;
    let terminal = execution.success.is_some();
    let success = execution.success == Some(true);
    let node_state = match execution.success {
        None => WorkflowNodeState::Running,
        Some(true) => WorkflowNodeState::Completed,
        Some(false) => WorkflowNodeState::Failed,
    };
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: checked(ExternalRef::new("lattice", "workflow", "queue-workflow")),
        execution: checked(ExternalRef::new(
            "lattice",
            "execution",
            plan.effect_id.as_str(),
        )),
        sequence: execution.sequence,
        state: match execution.success {
            None => WorkflowState::Running,
            Some(true) => WorkflowState::Completed,
            Some(false) => WorkflowState::Failed,
        },
        terminal,
        successful: success,
        outcome: match execution.success {
            None => Outcome::Pending,
            Some(true) => Outcome::Success,
            Some(false) => Outcome::DomainFailure,
        },
        nodes: plan
            .nodes
            .iter()
            .map(|node| WorkflowNodeObservation {
                node_id: node.id.clone(),
                state: node_state,
                terminal,
                successful: success,
                adapter: checked(ExternalRef::new("lattice", "adapter", "local")),
                receipt: terminal
                    .then(|| checked(ExternalRef::new("lattice", "receipt", "queue-node"))),
            })
            .collect(),
        logs: Vec::new(),
        receipt: terminal
            .then(|| checked(ExternalRef::new("lattice", "receipt", "queue-execution"))),
    }
}
fn unsupported() -> PortFailure {
    PortFailure::Domain(Diagnostic::new(
        "unsupported",
        "queue.fixture",
        "Queue fixture admits workflow dispatch and observation only",
    ))
}
impl WorkflowPort for Port {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut backend = self.backend.lock();
        backend.dispatches += 1;
        let key = plan.effect_id.as_str().to_owned();
        assert!(
            !backend.executions.contains_key(&key),
            "queue must not redispatch an accepted effect"
        );
        let execution = Execution {
            plan: plan.clone(),
            sequence: 1,
            success: None,
        };
        let result = observation(&execution);
        backend.executions.insert(key, execution);
        Ok(result)
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let backend = self.backend.lock();
        let execution = backend
            .executions
            .get(plan.effect_id.as_str())
            .ok_or_else(unsupported)?;
        assert_eq!(&execution.plan, plan);
        Ok(observation(execution))
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
    let sig = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|id| repo.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repo.commit(None, &sig, &sig, message, &tree, &parents)?)
}
fn accepted(response: ApiResponse) -> Result<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    Ok(serde_json::from_slice(&response.body)?)
}
fn field<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}: {value}").into())
}
fn text(value: &Value, path: &str) -> Result<String> {
    Ok(field(value, path)?.as_str().ok_or("not text")?.to_owned())
}

struct Fixture {
    root: TempDir,
    review: Profile,
    service: ServiceProfile,
    sources: Vec<String>,
    target: String,
}
impl Fixture {
    /// `queue` is the exact `kiln.review-queue.v1` JSON under test.
    fn new(queue: Value, speculative_capability: bool) -> Result<Self> {
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = root.path().join("repository.git");
        let repo = Repository::init_bare(&path)?;
        let base = commit(&repo, None, "base", &[("base.txt", "base\n")])?;
        let mut sources = Vec::new();
        for name in SOURCES {
            let file = format!("{name}.txt");
            sources.push(
                commit(
                    &repo,
                    Some(base),
                    name,
                    &[("base.txt", "base\n"), (file.as_str(), "reviewed\n")],
                )?
                .to_string(),
            );
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
        let mut capabilities = vec![
            "inspect",
            "compare",
            "register",
            "submit",
            "build_integration",
            "apply_suggestions",
        ];
        if speculative_capability {
            capabilities.push("speculative_integration");
        }
        let review = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1", "database":root.path().join("review.sqlite"), "project":"fixture",
            "provider":{"repository_id":repository_id,"repository_path":path,"target_refs":[MAIN],
                "capabilities":capabilities},
            "policy":{"id":"queue-review-policy","eligible_reviewers":[REVIEWER],"submitters":[SUBMITTER],
                "minimum_approvals":1,"require_ci":true},
            "read_principals":[AUTHOR,REVIEWER,SUBMITTER,ADVISOR],"write_principals":[AUTHOR,REVIEWER,SUBMITTER],
            "signature_name":"Queue service","signature_email":"queue@example.invalid",
            "queue":queue
        }))?;
        let service = serde_json::from_value(json!({
            "schema":"kiln.web-service.v1","service_id":"queue-tests","listen_address":"127.0.0.1:9443",
            "origin":"https://localhost:9443","tls":{"certificate_path":root.path().join("cert.pem"),
                "private_key_path":root.path().join("key.pem"),"min_version":"TLS1.3"},
            "operator_token_file":root.path().join("operator.token"),"worker_token_file":root.path().join("worker.token"),
            "state_directory":root.path().join("state"),
            "projects":[{"id":"fixture","label":"Fixture","repository_path":path,"reference":MAIN,
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
                "timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":16,"max_artifact_bytes":1024,"max_artifacts":2,
                "max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,
                "max_requests":256,"max_events_per_run":32,"max_state_bytes":1048576}
        }))?;
        Ok(Self {
            root,
            review,
            service,
            sources,
            target: target.to_string(),
        })
    }
    fn repository(&self) -> Result<Repository> {
        Ok(Repository::open_bare(
            &self.review.provider.repository_path,
        )?)
    }
    fn target(&self) -> Result<String> {
        Ok(self.repository()?.refname_to_id(MAIN)?.to_string())
    }
    fn operation_requests(&self) -> Result<usize> {
        Ok(self
            .repository()?
            .references_glob("refs/kiln/review-operations/*/request")?
            .count())
    }
    fn engine(&self, backend: Arc<Mutex<Backend>>) -> Result<Engine> {
        let capability = self
            .service
            .projects
            .first()
            .ok_or("missing project")?
            .lattice(self.service.bounds.max_runs)?
            .provider;
        let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
        ports.insert(
            "fixture".into(),
            Box::new(Port {
                profile: capability,
                backend,
            }),
        );
        Ok(Engine::open_with_ports(self.service.clone(), ports)?)
    }
}

fn window(floor: usize, ceiling: usize) -> Value {
    json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,
        "mode":"speculative","window":{"floor":floor,"ceiling":ceiling,"increase":1,"decrease":1}})
}
fn serial() -> Value {
    json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16})
}

struct Harness {
    fixture: Fixture,
    workspace: Workspace,
    engine: Engine,
    backend: Arc<Mutex<Backend>>,
    changes: Vec<String>,
    now_ms: i64,
}
impl Harness {
    fn new(queue: Value, count: usize) -> Result<Self> {
        let fixture = Fixture::new(queue, true)?;
        let workspace = Workspace::new(Some(fixture.review.clone()))?;
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = fixture.engine(backend.clone())?;
        accepted(engine.api(
            "POST",
            "/api/projects",
            &serde_json::to_vec(&json!({"project_id":"fixture","request_id":"enroll"}))?,
            Role::Operator,
        )?)?;
        let mut harness = Self {
            fixture,
            workspace,
            engine,
            backend,
            changes: Vec::new(),
            now_ms: 1000,
        };
        for index in 0..count {
            let source = harness.fixture.sources.get(index).ok_or("source")?.clone();
            let created = accepted(harness.post(AUTHOR, &json!({"request_id":format!("create-{index}"),"change_id":null,"expected_version":null,
                "action":{"type":"create","commit":source,"target_ref":MAIN,"title":format!("Queued {index}"),"description":"Exact integration","wip":false}}))?)?;
            harness.changes.push(text(&created, "/change/id")?);
            accepted(harness.action(
                index,
                AUTHOR,
                "assign-reviewer",
                json!({"type":"set_reviewer","principal":REVIEWER,"role":"reviewer"}),
            )?)?;
            accepted(harness.action(
                index,
                REVIEWER,
                "approve",
                json!({"type":"vote","value":"approve"}),
            )?)?;
        }
        Ok(harness)
    }
    fn restart(self) -> Result<Self> {
        let Harness {
            fixture,
            workspace,
            engine,
            backend,
            changes,
            now_ms,
        } = self;
        drop(workspace);
        drop(engine);
        let workspace = Workspace::new(Some(fixture.review.clone()))?;
        let mut engine = fixture.engine(backend.clone())?;
        let _notifications = engine.poll()?;
        Ok(Self {
            fixture,
            workspace,
            engine,
            backend,
            changes,
            now_ms,
        })
    }
    fn post(&mut self, actor: &str, request: &Value) -> Result<ApiResponse> {
        self.now_ms += 1000;
        Ok(self.workspace.response(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(request)?,
            RequestContext {
                role: Role::Viewer,
                principal: actor,
                now_ms: self.now_ms,
                ci: &[],
            },
        ))
    }
    fn detail(&self, index: usize) -> Result<Value> {
        accepted(self.workspace.response(
            "GET",
            &format!(
                "/api/code-review/changes/{}",
                self.changes.get(index).ok_or("change")?
            ),
            &[],
            RequestContext {
                role: Role::Viewer,
                principal: AUTHOR,
                now_ms: self.now_ms,
                ci: &[],
            },
        ))
    }
    fn action(
        &mut self,
        index: usize,
        actor: &str,
        request: &str,
        action: Value,
    ) -> Result<ApiResponse> {
        let detail = self.detail(index)?;
        let command = json!({"request_id":format!("{request}-{index}"),"change_id":self.changes.get(index),
            "expected_version":field(&detail,"/change/version")?,"action":action});
        self.post(actor, &command)
    }
    fn enqueue_all(&mut self) -> Result {
        let target = self.fixture.target.clone();
        for index in 0..self.changes.len() {
            accepted(self.action(
                index,
                SUBMITTER,
                "enqueue",
                json!({"type":"enqueue","expected_target":target}),
            )?)?;
        }
        Ok(())
    }
    fn tick(&mut self) -> Result {
        self.now_ms += 1000;
        self.workspace
            .advance_queue(&mut self.engine, self.now_ms)?;
        Ok(())
    }
    fn queue(&self, index: usize, path: &str) -> Result<Value> {
        Ok(field(&self.detail(index)?, &format!("/queue{path}"))?.clone())
    }
    fn candidate(&self, index: usize) -> Result<String> {
        text(&self.detail(index)?, "/queue/candidate_commit")
    }
    fn run_id(&self, index: usize) -> Result<String> {
        text(&self.detail(index)?, "/queue/ci_run")
    }
    fn status(&self, index: usize) -> Result<String> {
        text(&self.detail(index)?, "/queue/status")
    }
    fn run(&mut self, id: &str) -> Result<Value> {
        accepted(
            self.engine
                .api("GET", &format!("/api/runs/{id}"), &[], Role::Operator)?,
        )
    }
    fn finish(&mut self, candidate: &str, success: bool) -> Result {
        {
            let mut backend = self.backend.lock();
            let execution = backend
                .executions
                .values_mut()
                .find(|execution| {
                    execution
                        .plan
                        .source
                        .as_ref()
                        .is_some_and(|source| source.revision.native_id.as_str() == candidate)
                })
                .ok_or("candidate was not dispatched")?;
            execution.sequence += 1;
            execution.success = Some(success);
        }
        let _notifications = self.engine.poll()?;
        Ok(())
    }
    /// First parent and source parent of an exact merge candidate.
    fn parents(&self, candidate: &str) -> Result<(String, String)> {
        let repo = self.fixture.repository()?;
        let selected = repo.find_commit(Oid::from_str(candidate)?)?;
        assert_eq!(selected.parent_count(), 2);
        Ok((
            selected.parent_id(0)?.to_string(),
            selected.parent_id(1)?.to_string(),
        ))
    }
    fn source(&self, index: usize) -> Result<String> {
        Ok(self.fixture.sources.get(index).ok_or("source")?.clone())
    }
}

// r[verify kiln.review.queue.speculative.window]
// r[verify kiln.review.queue.speculative.window.scenario.three]
#[test]
fn window_of_three_starts_every_candidate_ci_before_the_first_submit_and_lands_in_order() -> Result
{
    let mut h = Harness::new(window(1, 3), 3)?;
    h.enqueue_all()?;
    h.tick()?;
    let target = h.fixture.target.clone();
    assert_eq!(h.fixture.target()?, target, "nothing lands before CI");
    let candidates: Vec<String> = (0..3).map(|i| h.candidate(i)).collect::<Result<_>>()?;
    let runs: Vec<String> = (0..3).map(|i| h.run_id(i)).collect::<Result<_>>()?;
    assert_eq!(runs.iter().collect::<BTreeSet<_>>().len(), 3);
    // Candidate k is stacked on candidate k-1, rooted at the exact target.
    let mut base = target.clone();
    for (index, candidate) in candidates.iter().enumerate() {
        assert_eq!(h.parents(candidate)?, (base.clone(), h.source(index)?));
        assert_eq!(h.status(index)?, "waiting_ci");
        assert_eq!(text(&h.queue(index, "/chain")?, "/root_target")?, target);
        assert_eq!(
            h.queue(index, "/chain/predecessors")?
                .as_array()
                .ok_or("predecessors")?
                .len(),
            index
        );
        assert_eq!(text(&h.queue(index, "")?, "/target_commit")?, base);
        let run = h.run(runs.get(index).ok_or("run")?)?;
        assert_eq!(text(&run, "/source_revision")?, *candidate);
        assert_eq!(text(&run, "/outcome")?, "Pending");
        base = candidate.clone();
    }
    // Every candidate CI run is admitted before anything submits.
    for (index, candidate) in candidates.iter().enumerate() {
        assert!(field(&h.detail(index)?, "/change/submission")?.is_null());
        h.finish(candidate, true)?;
        h.tick()?;
        assert_eq!(h.fixture.target()?, *candidate, "lands in queue order");
        assert_eq!(h.status(index)?, "submitted");
        let detail = h.detail(index)?;
        assert_eq!(text(&detail, "/change/state")?, "submitted");
        assert_eq!(
            text(&detail, "/change/submission/result_commit")?,
            *candidate
        );
        for later in index + 1..3 {
            assert_ne!(h.status(later)?, "submitted");
        }
    }
    assert_eq!(h.backend.lock().dispatches, 3, "no candidate re-tested");
    Ok(())
}

// r[verify kiln.review.queue.speculative.failure]
// r[verify kiln.review.queue.speculative.failure.scenario.middle]
// r[verify kiln.review.queue.speculative.chain.scenario.late]
#[test]
fn failing_middle_entry_ejects_only_itself_and_later_entry_rebuilds_with_fresh_ci() -> Result {
    let mut h = Harness::new(window(1, 3), 3)?;
    h.enqueue_all()?;
    h.tick()?;
    let first = h.candidate(0)?;
    let middle = h.candidate(1)?;
    let stale = h.candidate(2)?;
    let stale_run = h.run_id(2)?;
    let stale_chain = text(&h.queue(2, "/chain")?, "/id")?;
    h.finish(&first, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, first);
    h.finish(&middle, false)?;
    h.tick()?;
    assert_eq!(h.status(1)?, "blocked");
    assert_eq!(
        text(&h.detail(1)?, "/queue/detail")?,
        "review_queue_ci_failed"
    );
    assert_eq!(h.status(0)?, "submitted", "earlier entry unaffected");
    // Only the failing entry is ejected; the later one is rebuilt on the
    // landed target without the failing candidate, with fresh CI.
    assert_ne!(h.status(2)?, "blocked");
    let rebuilt = h.candidate(2)?;
    assert_ne!(rebuilt, stale);
    assert_eq!(h.parents(&rebuilt)?, (first.clone(), h.source(2)?));
    assert_eq!(text(&h.queue(2, "/chain")?, "/root_target")?, first);
    assert!(
        h.queue(2, "/superseded_chains")?
            .as_array()
            .ok_or("superseded")?
            .contains(&json!(stale_chain))
    );
    let fresh_run = h.run_id(2)?;
    assert_ne!(fresh_run, stale_run);
    assert_eq!(text(&h.run(&fresh_run)?, "/source_revision")?, rebuilt);
    // The superseded run was already dispatched; its late success is ignored.
    h.finish(&stale, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, first, "superseded success never lands");
    assert_eq!(h.status(2)?, "waiting_ci");
    assert_eq!(h.candidate(2)?, rebuilt);
    h.finish(&rebuilt, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, rebuilt);
    assert_eq!(h.status(2)?, "submitted");
    assert!(field(&h.detail(1)?, "/change/submission")?.is_null());
    let repo = h.fixture.repository()?;
    assert!(
        !repo.graph_descendant_of(Oid::from_str(&rebuilt)?, Oid::from_str(&h.source(1)?)?)?,
        "failing source never reaches the target"
    );
    Ok(())
}

// r[verify kiln.review.queue.speculative.failure]
#[test]
fn window_shrinks_after_ejection_cancels_queued_runs_and_grows_after_landing() -> Result {
    let mut h = Harness::new(window(1, 2), 3)?;
    h.enqueue_all()?;
    h.tick()?;
    assert!(
        h.queue(2, "/candidate_commit")?.is_null(),
        "outside the window"
    );
    let head = h.candidate(0)?;
    let stale = h.candidate(1)?;
    let stale_run = h.run_id(1)?;
    assert_eq!(text(&h.run(&stale_run)?, "/outcome")?, "Pending");
    // A new head revision revokes its queued authorization while the second
    // candidate's run still waits for the project slot (not dispatched).
    let revised = {
        let repo = h.fixture.repository()?;
        let parent = Oid::from_str(&h.source(0)?)?;
        commit(
            &repo,
            Some(parent),
            "revised",
            &[("base.txt", "base\n"), ("first.txt", "revised\n")],
        )?
        .to_string()
    };
    accepted(h.action(
        0,
        AUTHOR,
        "revise",
        json!({"type":"upload","commit":revised}),
    )?)?;
    h.tick()?;
    assert_eq!(h.status(0)?, "blocked");
    assert_eq!(
        text(&h.detail(0)?, "/queue/detail")?,
        "review_queue_authority_changed"
    );
    assert_eq!(
        text(&h.run(&stale_run)?, "/outcome")?,
        "Cancelled",
        "undispatched superseded candidate run is cancelled"
    );
    let rebuilt = h.candidate(1)?;
    assert_ne!(rebuilt, stale);
    assert_eq!(
        h.parents(&rebuilt)?,
        (h.fixture.target.clone(), h.source(1)?)
    );
    h.tick()?;
    assert!(
        h.queue(2, "/candidate_commit")?.is_null(),
        "window shrank to one after the ejection"
    );
    // The ejected head's still-running CI result is ignored.
    h.finish(&head, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, h.fixture.target);
    h.finish(&rebuilt, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, rebuilt);
    h.tick()?;
    let last = h.candidate(2)?;
    assert_eq!(h.parents(&last)?, (rebuilt, h.source(2)?));
    h.finish(&last, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, last);
    assert!(field(&h.detail(0)?, "/change/submission")?.is_null());
    Ok(())
}

// r[verify kiln.review.queue.speculative.chain]
// r[verify kiln.review.queue.speculative.chain.scenario.restart]
#[test]
fn restart_mid_window_keeps_candidates_and_runs_without_duplicate_dispatch() -> Result {
    let mut h = Harness::new(window(1, 3), 3)?;
    h.enqueue_all()?;
    h.tick()?;
    let candidates: Vec<String> = (0..3).map(|i| h.candidate(i)).collect::<Result<_>>()?;
    let runs: Vec<String> = (0..3).map(|i| h.run_id(i)).collect::<Result<_>>()?;
    let chains: Vec<Value> = (0..3)
        .map(|i| h.queue(i, "/chain"))
        .collect::<Result<_>>()?;
    let operations = h.fixture.operation_requests()?;
    let dispatches = h.backend.lock().dispatches;
    let mut h = h.restart()?;
    h.tick()?;
    for index in 0..3 {
        assert_eq!(h.candidate(index)?, *candidates.get(index).ok_or("c")?);
        assert_eq!(h.run_id(index)?, *runs.get(index).ok_or("r")?);
        assert_eq!(
            h.queue(index, "/chain")?,
            *chains.get(index).ok_or("chain")?
        );
    }
    assert_eq!(h.fixture.operation_requests()?, operations);
    assert_eq!(h.backend.lock().dispatches, dispatches);
    for (index, candidate) in candidates.iter().enumerate() {
        h.finish(candidate, true)?;
        h.tick()?;
        assert_eq!(h.fixture.target()?, *candidate);
        // Restart again after each landing, before the next candidate's result.
        h = h.restart()?;
        h.tick()?;
        assert_eq!(h.status(index)?, "submitted");
        for later in index + 1..3 {
            assert_eq!(h.candidate(later)?, *candidates.get(later).ok_or("c")?);
            assert_eq!(h.run_id(later)?, *runs.get(later).ok_or("r")?);
        }
    }
    assert_eq!(
        h.backend.lock().dispatches,
        3,
        "each candidate dispatched once"
    );
    // Three builds plus three submits; restarts never redispatch an operation.
    assert_eq!(h.fixture.operation_requests()?, operations + 3);
    Ok(())
}

/// Serialized keys of a single-project queue entry before speculative mode.
const LEGACY_ENTRY_KEYS: [&str; 14] = [
    "id",
    "change",
    "revision",
    "actor",
    "target_ref",
    "source_commit",
    "target_commit",
    "candidate_commit",
    "ci_run",
    "status",
    "detail",
    "created_ms",
    "generation",
    "policy_id",
];

// r[verify kiln.review.queue.speculative.serial]
// r[verify kiln.review.queue.speculative.serial.scenario.replay]
#[test]
fn serial_queue_records_keep_exactly_the_previous_shape() -> Result {
    let mut h = Harness::new(serial(), 1)?;
    h.enqueue_all()?;
    h.tick()?;
    let candidate = h.candidate(0)?;
    assert_eq!(
        h.parents(&candidate)?,
        (h.fixture.target.clone(), h.source(0)?)
    );
    h.tick()?;
    h.finish(&candidate, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, candidate);
    let entry = h.queue(0, "")?;
    let keys: BTreeSet<&str> = entry
        .as_object()
        .ok_or("entry")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, LEGACY_ENTRY_KEYS.into_iter().collect::<BTreeSet<_>>());
    let database = h.fixture.root.path().join("review.sqlite");
    let connection = rusqlite::Connection::open_with_flags(
        &database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let state: Vec<u8> =
        connection.query_row("SELECT document FROM review_state", [], |row| row.get(0))?;
    let mut statement = connection.prepare("SELECT document FROM review_requests")?;
    let journals: Vec<Vec<u8>> = statement
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let stored: Value = serde_json::from_slice(&state)?;
    assert!(stored.get("queue_windows").is_none());
    for entry in field(&stored, "/queue")?.as_array().ok_or("queue")? {
        let keys: BTreeSet<&str> = entry
            .as_object()
            .ok_or("entry")?
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, LEGACY_ENTRY_KEYS.into_iter().collect::<BTreeSet<_>>());
    }
    for document in &journals {
        let document = String::from_utf8_lossy(document);
        for key in [
            "\"chain\"",
            "\"superseded_chains\"",
            "\"predecessor\"",
            "build_speculative_integration",
        ] {
            assert!(!document.contains(key), "serial journal gained {key}");
        }
    }
    let build = journals
        .iter()
        .map(|bytes| serde_json::from_slice::<Value>(bytes))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .find(|journal| {
            journal
                .get("request_id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.starts_with("queue-build-"))
        })
        .ok_or("queue build journal")?;
    assert_eq!(field(&build, "/response/status")?, &json!(200));
    assert_eq!(
        text(&build, "/response/body/operation/kind")?,
        "build_integration"
    );
    Ok(())
}

// r[verify kiln.review.queue.speculative.profile]
// r[verify kiln.review.queue.speculative.profile.scenario.invalid]
#[test]
fn speculative_profiles_require_bounded_window_and_provider_capability() -> Result {
    let valid = Fixture::new(window(1, 3), true)?;
    assert!(Workspace::new(Some(valid.review.clone())).is_ok());
    let explicit_serial = Fixture::new(
        json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,"mode":"serial"}),
        false,
    )?;
    assert!(Workspace::new(Some(explicit_serial.review.clone())).is_ok());
    let without_capability = Fixture::new(window(1, 3), false)?;
    assert!(Workspace::new(Some(without_capability.review.clone())).is_err());
    for queue in [
        json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,"mode":"speculative"}),
        json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,
            "window":{"floor":1,"ceiling":3,"increase":1,"decrease":1}}),
        window(4, 3),
        window(1, 17),
        json!({"schema":"kiln.review-queue.v1","max_entries":2,"max_candidate_builds":16,
            "mode":"speculative","window":{"floor":1,"ceiling":3,"increase":1,"decrease":1}}),
        json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,
            "mode":"speculative","window":{"floor":1,"ceiling":3,"increase":0,"decrease":1}}),
    ] {
        let fixture = Fixture::new(queue.clone(), true)?;
        assert!(
            Workspace::new(Some(fixture.review.clone())).is_err(),
            "admitted {queue}"
        );
    }
    let unknown = json!({"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,
        "mode":"speculative","window":{"floor":1,"ceiling":3,"increase":1,"decrease":1,"batch":true}});
    assert!(Fixture::new(unknown, true).is_err(), "unknown window field");
    let target = valid.target()?;
    assert_eq!(
        target, valid.target,
        "admission performs no provider effect"
    );
    Ok(())
}

// r[verify kiln.review.queue.speculative.serial]
#[test]
fn advisory_approvals_never_admit_or_qualify_a_speculative_candidate() -> Result {
    let mut h = Harness::new(window(1, 3), 2)?;
    let target = h.fixture.target.clone();
    // Replace the binding approval of the second change with an advisory one.
    accepted(h.action(
        1,
        REVIEWER,
        "abstain",
        json!({"type":"vote","value":"abstain"}),
    )?)?;
    accepted(h.action(1, ADVISOR, "advise", json!({"type":"advisory_approve"}))?)?;
    let refused = h.action(
        1,
        SUBMITTER,
        "enqueue",
        json!({"type":"enqueue","expected_target":target}),
    )?;
    assert_eq!(refused.status, 409);
    assert_eq!(
        text(&serde_json::from_slice::<Value>(&refused.body)?, "/code")?,
        "review_queue_requirements_unmet"
    );
    // A queued candidate whose binding approval is withdrawn never lands on
    // an advisory approval, even after its exact CI passes.
    accepted(h.action(
        0,
        SUBMITTER,
        "enqueue",
        json!({"type":"enqueue","expected_target":target}),
    )?)?;
    h.tick()?;
    let candidate = h.candidate(0)?;
    accepted(h.action(
        0,
        REVIEWER,
        "abstain",
        json!({"type":"vote","value":"abstain"}),
    )?)?;
    accepted(h.action(0, ADVISOR, "advise", json!({"type":"advisory_approve"}))?)?;
    h.finish(&candidate, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, target);
    assert_eq!(h.status(0)?, "blocked");
    assert_eq!(
        text(&h.detail(0)?, "/queue/detail")?,
        "review_queue_requirements_unmet"
    );
    assert!(field(&h.detail(0)?, "/change/submission")?.is_null());
    Ok(())
}
