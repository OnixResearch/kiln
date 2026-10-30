//! Real disposable Git and SQLite queue conformance with deterministic workflow observations.
//! No network, installed credentials, real refs or client-supplied CI authority.
// r[verify kiln.review.parity.queue]
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
const MAIN: &str = "refs/heads/main";

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

struct Fixture {
    _root: TempDir,
    review: Profile,
    service: ServiceProfile,
    source: String,
    second: String,
    target: String,
}
impl Fixture {
    fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = root.path().join("repository.git");
        let repo = Repository::init_bare(&path)?;
        let base = commit(&repo, None, "base", &[("base.txt", "base\n")])?;
        let source = commit(
            &repo,
            Some(base),
            "source",
            &[("base.txt", "base\n"), ("source.txt", "reviewed\n")],
        )?;
        let second = commit(
            &repo,
            Some(source),
            "new source",
            &[
                ("base.txt", "base\n"),
                ("source.txt", "changed after review\n"),
            ],
        )?;
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
        let review = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1", "database":root.path().join("review.sqlite"), "project":"fixture",
            "provider":{"repository_id":repository_id,"repository_path":path,"target_refs":[MAIN],
                "capabilities":["inspect","compare","register","submit","build_integration","apply_suggestions"]},
            "policy":{"id":"queue-review-policy","eligible_reviewers":[REVIEWER],"submitters":[SUBMITTER],
                "minimum_approvals":1,"require_ci":true},
            "read_principals":[AUTHOR,REVIEWER,SUBMITTER],"write_principals":[AUTHOR,REVIEWER,SUBMITTER],
            "signature_name":"Queue service","signature_email":"queue@example.invalid",
            "queue":{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16}
        }))?;
        // Same bounded service/WorkflowPort admission pattern as tests/engine.rs.
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
                "timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":8,"max_artifact_bytes":1024,"max_artifacts":2,
                "max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,
                "max_requests":128,"max_events_per_run":32,"max_state_bytes":1048576}
        }))?;
        Ok(Self {
            _root: root,
            review,
            service,
            source: source.to_string(),
            second: second.to_string(),
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
fn engine_api(engine: &mut Engine, method: &str, path: &str, body: Value) -> Result<ApiResponse> {
    let bytes = if body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(&body)?
    };
    Ok(engine.api(method, path, &bytes, Role::Operator)?)
}

struct Harness {
    fixture: Fixture,
    workspace: Workspace,
    engine: Engine,
    backend: Arc<Mutex<Backend>>,
    change_id: String,
    now_ms: i64,
}
impl Harness {
    fn new() -> Result<Self> {
        let fixture = Fixture::new()?;
        let workspace = Workspace::new(Some(fixture.review.clone()))?;
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = fixture.engine(backend.clone())?;
        accepted(engine_api(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id":"fixture","request_id":"enroll"}),
        )?)?;
        let mut harness = Self {
            fixture,
            workspace,
            engine,
            backend,
            change_id: String::new(),
            now_ms: 1000,
        };
        let created = accepted(harness.post(AUTHOR, &json!({"request_id":"create","change_id":null,"expected_version":null,
            "action":{"type":"create","commit":harness.fixture.source,"target_ref":MAIN,"title":"Queued review","description":"Exact integration","wip":false}}))?)?;
        harness.change_id = text(&created, "/change/id")?;
        accepted(harness.action(
            AUTHOR,
            "assign-reviewer",
            json!({"type":"set_reviewer","principal":REVIEWER,"role":"reviewer"}),
        )?)?;
        accepted(harness.action(
            REVIEWER,
            "approve",
            json!({"type":"vote","value":"approve"}),
        )?)?;
        Ok(harness)
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
    fn detail(&self) -> Result<Value> {
        accepted(self.workspace.response(
            "GET",
            &format!("/api/code-review/changes/{}", self.change_id),
            &[],
            RequestContext {
                role: Role::Viewer,
                principal: AUTHOR,
                now_ms: self.now_ms,
                ci: &[],
            },
        ))
    }
    fn action(&mut self, actor: &str, request: &str, action: Value) -> Result<ApiResponse> {
        let detail = self.detail()?;
        let command = json!({"request_id":request,"change_id":self.change_id,
            "expected_version":field(&detail,"/change/version")?,"action":action});
        self.post(actor, &command)
    }
    fn enqueue(&mut self) -> Result {
        accepted(self.action(
            SUBMITTER,
            "enqueue",
            json!({"type":"enqueue","expected_target":self.fixture.target}),
        )?)?;
        Ok(())
    }
    fn tick(&mut self) -> Result {
        self.now_ms += 1000;
        self.workspace
            .advance_queue(&mut self.engine, self.now_ms)?;
        Ok(())
    }
    fn candidate(&self) -> Result<String> {
        text(&self.detail()?, "/queue/candidate_commit")
    }
    fn run_id(&self) -> Result<String> {
        text(&self.detail()?, "/queue/ci_run")
    }
    fn run(&mut self, id: &str) -> Result<Value> {
        accepted(engine_api(
            &mut self.engine,
            "GET",
            &format!("/api/runs/{id}"),
            Value::Null,
        )?)
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
    fn assert_merge(&self, candidate: &str, target: &str) -> Result {
        let repo = self.fixture.repository()?;
        let selected = repo.find_commit(Oid::from_str(candidate)?)?;
        assert_eq!(selected.parent_count(), 2);
        assert_eq!(selected.parent_id(0)?.to_string(), target);
        assert_eq!(selected.parent_id(1)?.to_string(), self.fixture.source);
        let tree = selected.tree()?;
        for (path, expected) in [
            ("source.txt", b"reviewed\n".as_slice()),
            ("target.txt", b"target\n".as_slice()),
        ] {
            let entry = tree.get_path(std::path::Path::new(path))?;
            assert_eq!(repo.find_blob(entry.id())?.content(), expected);
        }
        Ok(())
    }
}

#[test]
fn queue_tests_and_submits_exact_combined_commit_without_transferring_source_votes() -> Result {
    let mut h = Harness::new()?;
    let original = h.detail()?;
    let revision = text(&original, "/change/revisions/0/id")?;
    let votes = field(&original, "/change/votes")?.clone();
    h.enqueue()?;
    h.tick()?;
    let candidate = h.candidate()?;
    assert_ne!(candidate, h.fixture.source);
    h.assert_merge(&candidate, &h.fixture.target)?;
    assert_eq!(h.fixture.target()?, h.fixture.target);
    h.tick()?;
    let run_id = h.run_id()?;
    let run = h.run(&run_id)?;
    assert_eq!(text(&run, "/source_revision")?, candidate);
    assert_eq!(text(&run, "/outcome")?, "Pending");
    assert_eq!(h.fixture.target()?, h.fixture.target);
    h.finish(&candidate, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, candidate);
    let submitted = h.detail()?;
    assert_eq!(text(&submitted, "/queue/status")?, "submitted");
    assert_eq!(text(&submitted, "/change/state")?, "submitted");
    assert_eq!(text(&submitted, "/change/revisions/0/id")?, revision);
    assert_eq!(
        text(&submitted, "/change/revisions/0/commit")?,
        h.fixture.source
    );
    assert_eq!(field(&submitted, "/change/votes")?, &votes);
    assert_eq!(text(&submitted, "/change/votes/0/revision")?, revision);
    assert_eq!(
        text(&submitted, "/change/submission/result_commit")?,
        candidate
    );
    Ok(())
}

#[test]
fn failed_candidate_ci_never_advances_target() -> Result {
    let mut h = Harness::new()?;
    h.enqueue()?;
    h.tick()?;
    let candidate = h.candidate()?;
    h.tick()?;
    let run_id = h.run_id()?;
    h.finish(&candidate, false)?;
    assert_eq!(text(&h.run(&run_id)?, "/outcome")?, "DomainFailure");
    h.tick()?;
    let detail = h.detail()?;
    assert_eq!(text(&detail, "/queue/status")?, "blocked");
    assert_eq!(text(&detail, "/queue/detail")?, "review_queue_ci_failed");
    assert_eq!(h.fixture.target()?, h.fixture.target);
    assert!(field(&detail, "/change/submission")?.is_null());
    assert_eq!(
        h.action(
            SUBMITTER,
            "cannot-prepare-failed",
            json!({"type":"prepare_submit"})
        )?
        .status,
        409
    );
    Ok(())
}

#[test]
fn moved_target_invalidates_old_success_and_requires_a_new_exact_candidate_run() -> Result {
    let mut h = Harness::new()?;
    h.enqueue()?;
    h.tick()?;
    let old_candidate = h.candidate()?;
    h.tick()?;
    let old_run = h.run_id()?;
    h.finish(&old_candidate, true)?;
    let moved = {
        let repo = h.fixture.repository()?;
        let id = commit(
            &repo,
            Some(Oid::from_str(&h.fixture.target)?),
            "target moves",
            &[
                ("base.txt", "base\n"),
                ("target.txt", "target\n"),
                ("later.txt", "later\n"),
            ],
        )?;
        repo.reference(MAIN, id, true, "disposable competing writer")?;
        id.to_string()
    };
    h.tick()?;
    let invalidated = h.detail()?;
    assert_eq!(text(&invalidated, "/queue/target_commit")?, moved);
    assert!(field(&invalidated, "/queue/candidate_commit")?.is_null());
    assert!(field(&invalidated, "/queue/ci_run")?.is_null());
    assert_eq!(h.fixture.target()?, moved);
    h.tick()?;
    let replacement = h.candidate()?;
    assert_ne!(replacement, old_candidate);
    h.assert_merge(&replacement, &moved)?;
    h.tick()?;
    let replacement_run = h.run_id()?;
    assert_ne!(replacement_run, old_run);
    assert_eq!(
        text(&h.run(&replacement_run)?, "/source_revision")?,
        replacement
    );
    h.tick()?;
    assert_eq!(
        h.fixture.target()?,
        moved,
        "old passing CI cannot qualify a new candidate"
    );
    assert_eq!(text(&h.detail()?, "/queue/status")?, "waiting_ci");
    h.finish(&replacement, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, replacement);
    assert_eq!(text(&h.detail()?, "/queue/status")?, "submitted");
    Ok(())
}

#[test]
fn uploading_a_new_revision_blocks_the_queued_approved_source_even_after_ci_success() -> Result {
    let mut h = Harness::new()?;
    let original = h.detail()?;
    let original_revision = text(&original, "/change/revisions/0/id")?;
    h.enqueue()?;
    h.tick()?;
    let candidate = h.candidate()?;
    h.tick()?;
    accepted(h.action(
        AUTHOR,
        "new-revision",
        json!({"type":"upload","commit":h.fixture.second}),
    )?)?;
    h.finish(&candidate, true)?;
    h.tick()?;
    let stale = h.detail()?;
    assert_eq!(text(&stale, "/queue/status")?, "blocked");
    assert_eq!(
        text(&stale, "/queue/detail")?,
        "review_queue_authority_changed"
    );
    assert_eq!(text(&stale, "/queue/revision")?, original_revision);
    assert_eq!(text(&stale, "/change/votes/0/revision")?, original_revision);
    assert_ne!(text(&stale, "/change/revisions/1/id")?, original_revision);
    assert_eq!(h.fixture.target()?, h.fixture.target);
    assert!(field(&stale, "/change/submission")?.is_null());
    Ok(())
}

#[test]
fn restart_recovers_same_queue_candidate_and_run_without_dispatching_or_approving_again() -> Result
{
    let mut h = Harness::new()?;
    h.enqueue()?;
    h.tick()?;
    let candidate = h.candidate()?;
    h.tick()?;
    let run = h.run_id()?;
    let before = h.detail()?;
    // Drop both persistence owners before reopening the same admitted profiles.
    let Harness {
        fixture,
        workspace,
        engine,
        backend,
        change_id,
        now_ms,
    } = h;
    drop(workspace);
    drop(engine);
    let workspace = Workspace::new(Some(fixture.review.clone()))?;
    let engine = fixture.engine(backend.clone())?;
    let mut h = Harness {
        fixture,
        workspace,
        engine,
        backend,
        change_id,
        now_ms,
    };
    let _notifications = h.engine.poll()?;
    h.tick()?;
    let recovered = h.detail()?;
    assert_eq!(text(&recovered, "/queue/id")?, text(&before, "/queue/id")?);
    assert_eq!(h.candidate()?, candidate);
    assert_eq!(h.run_id()?, run);
    assert_eq!(
        field(&recovered, "/change/votes")?,
        field(&before, "/change/votes")?
    );
    assert_eq!(h.backend.lock().dispatches, 1);
    assert_eq!(h.fixture.target()?, h.fixture.target);
    h.finish(&candidate, true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, candidate);
    assert_eq!(text(&h.detail()?, "/queue/status")?, "submitted");
    assert_eq!(h.backend.lock().dispatches, 1);
    Ok(())
}

#[test]
fn only_submitters_can_enqueue_and_owner_can_cancel_without_any_ci_dispatch() -> Result {
    let mut h = Harness::new()?;
    assert_eq!(
        h.action(
            AUTHOR,
            "author-cannot-enqueue",
            json!({"type":"enqueue","expected_target":h.fixture.target})
        )?
        .status,
        403
    );
    h.enqueue()?;
    assert_eq!(
        h.action(
            REVIEWER,
            "reviewer-cannot-dequeue",
            json!({"type":"dequeue"})
        )?
        .status,
        403
    );
    accepted(h.action(AUTHOR, "owner-dequeue", json!({"type":"dequeue"}))?)?;
    h.tick()?;
    assert_eq!(text(&h.detail()?, "/queue/status")?, "cancelled");
    assert_eq!(h.fixture.target()?, h.fixture.target);
    assert_eq!(h.backend.lock().dispatches, 0);
    Ok(())
}

#[test]
fn queue_admission_requires_ci_and_explicit_integration_capability() -> Result {
    let fixture = Fixture::new()?;
    let mut no_ci = fixture.review.clone();
    no_ci.policy.require_ci = false;
    assert!(Workspace::new(Some(no_ci)).is_err());
    let mut no_integration = fixture.review.clone();
    no_integration
        .provider
        .capabilities
        .retain(|cap| *cap != kiln_provider_api::change_review::ReviewCapability::BuildIntegration);
    assert!(Workspace::new(Some(no_integration)).is_err());
    assert_eq!(fixture.target()?, fixture.target);
    Ok(())
}

/// Three projects on the review repository: the primary `fixture` (source.txt)
/// and `docs` (declares source.txt) are affected by the reviewed source;
/// `other` (other.txt) is skipped.
struct Monorepo {
    fixture: Fixture,
    service: ServiceProfile,
    workspace: Workspace,
    engine: Engine,
    backends: BTreeMap<String, Arc<Mutex<Backend>>>,
    change_id: String,
    now_ms: i64,
}
impl Monorepo {
    fn new() -> Result<Self> {
        let fixture = Fixture::new()?;
        let mut service = serde_json::to_value(&fixture.service)?;
        let template = field(&service, "/projects/0")?.clone();
        let project = |id: &str, scope: Value| -> Result<Value> {
            let mut project = template.clone();
            let object = project.as_object_mut().ok_or("project")?;
            object.insert("id".into(), json!(id));
            object.insert("label".into(), json!(format!("Project {id}")));
            object.insert("scope".into(), scope);
            Ok(project)
        };
        let projects = json!([
            project("fixture", json!({"root":"","include":["source.txt"]}))?,
            project(
                "docs",
                json!({"root":"","include":["docs.txt"],"depends_on":["source.txt"]})
            )?,
            project("other", json!({"root":"","include":["other.txt"]}))?,
        ]);
        let object = service.as_object_mut().ok_or("service")?;
        object.insert("projects".into(), projects);
        let bounds = object
            .get_mut("bounds")
            .and_then(Value::as_object_mut)
            .ok_or("bounds")?;
        bounds.insert("max_projects".into(), json!(4));
        bounds.insert("max_runs".into(), json!(16));
        let service: ServiceProfile = serde_json::from_value(service)?;
        service.admit()?;
        let workspace = Workspace::for_service(Some(fixture.review.clone()), &service)?;
        let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
        let mut backends = BTreeMap::new();
        for project in &service.projects {
            let backend = Arc::new(Mutex::new(Backend::default()));
            backends.insert(project.id.clone(), backend.clone());
            ports.insert(
                project.id.clone(),
                Box::new(Port {
                    profile: project.lattice(service.bounds.max_runs)?.provider,
                    backend,
                }),
            );
        }
        let mut engine = Engine::open_with_ports(service.clone(), ports)?;
        for id in ["fixture", "docs", "other"] {
            accepted(engine_api(
                &mut engine,
                "POST",
                "/api/projects",
                json!({"project_id":id,"request_id":format!("enroll-{id}")}),
            )?)?;
        }
        let mut harness = Self {
            fixture,
            service,
            workspace,
            engine,
            backends,
            change_id: String::new(),
            now_ms: 1000,
        };
        let created = accepted(harness.post(AUTHOR, &json!({"request_id":"create","change_id":null,"expected_version":null,
            "action":{"type":"create","commit":harness.fixture.source,"target_ref":MAIN,"title":"Monorepo review","description":"Exact integration","wip":false}}))?)?;
        harness.change_id = text(&created, "/change/id")?;
        accepted(harness.action(
            AUTHOR,
            "assign-reviewer",
            json!({"type":"set_reviewer","principal":REVIEWER,"role":"reviewer"}),
        )?)?;
        accepted(harness.action(
            REVIEWER,
            "approve",
            json!({"type":"vote","value":"approve"}),
        )?)?;
        accepted(harness.action(
            SUBMITTER,
            "enqueue",
            json!({"type":"enqueue","expected_target":harness.fixture.target}),
        )?)?;
        Ok(harness)
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
    fn detail_with(&self, ci: &[kiln_core::change_review::CiFact]) -> Result<Value> {
        accepted(self.workspace.response(
            "GET",
            &format!("/api/code-review/changes/{}", self.change_id),
            &[],
            RequestContext {
                role: Role::Viewer,
                principal: AUTHOR,
                now_ms: self.now_ms,
                ci,
            },
        ))
    }
    fn action(&mut self, actor: &str, request: &str, action: Value) -> Result<ApiResponse> {
        let detail = self.detail_with(&[])?;
        let command = json!({"request_id":request,"change_id":self.change_id,
            "expected_version":field(&detail,"/change/version")?,"action":action});
        self.post(actor, &command)
    }
    fn tick(&mut self) -> Result {
        self.now_ms += 1000;
        self.workspace
            .advance_queue(&mut self.engine, self.now_ms)?;
        Ok(())
    }
    fn backend(&self, project: &str) -> Result<&Arc<Mutex<Backend>>> {
        Ok(self.backends.get(project).ok_or("backend")?)
    }
    fn finish(&mut self, project: &str, success: bool) -> Result {
        {
            let mut backend = self.backend(project)?.lock();
            let execution = backend
                .executions
                .values_mut()
                .next()
                .ok_or("project candidate was not dispatched")?;
            execution.sequence += 1;
            execution.success = Some(success);
        }
        let _notifications = self.engine.poll()?;
        Ok(())
    }
    fn scope(&self, project: &str) -> Result<String> {
        Ok(self
            .service
            .projects
            .iter()
            .find(|p| p.id == project)
            .ok_or("project")?
            .admitted_scope()?
            .ok_or("scope")?
            .identity()
            .as_str()
            .to_owned())
    }
    fn run(&mut self, id: &str) -> Result<Value> {
        accepted(engine_api(
            &mut self.engine,
            "GET",
            &format!("/api/runs/{id}"),
            Value::Null,
        )?)
    }
    /// Start CI for the constructed candidate; returns (candidate, runs).
    fn start(&mut self) -> Result<(String, Vec<String>)> {
        self.tick()?;
        self.tick()?;
        let detail = self.detail_with(&[])?;
        let candidate = text(&detail, "/queue/candidate_commit")?;
        let runs = field(&detail, "/queue/ci_runs")?
            .as_array()
            .ok_or("ci_runs")?
            .iter()
            .map(|run| run.as_str().map(str::to_owned).ok_or("run id"))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok((candidate, runs))
    }
}

fn row<'a>(detail: &'a Value, project: &str) -> Result<&'a Value> {
    Ok(field(detail, "/checklist/items")?
        .as_array()
        .ok_or("items")?
        .iter()
        .find(|item| item.get("key") == Some(&json!("ci")))
        .and_then(|item| item.get("ci_projects"))
        .and_then(Value::as_array)
        .ok_or("ci_projects")?
        .iter()
        .find(|row| row.get("project_id") == Some(&json!(project)))
        .ok_or("project row")?)
}

// r[verify kiln.monorepo.gating]
#[test]
fn monorepo_queue_tests_every_affected_project_at_its_current_scope() -> Result {
    let mut h = Monorepo::new()?;
    let (candidate, runs) = h.start()?;
    assert_eq!(runs.len(), 2, "one candidate run per affected project");
    let dispatches =
        |h: &Monorepo, id: &str| -> Result<usize> { Ok(h.backend(id)?.lock().dispatches) };
    assert_eq!(dispatches(&h, "fixture")?, 1);
    assert_eq!(dispatches(&h, "docs")?, 1);
    assert_eq!(dispatches(&h, "other")?, 0, "skipped projects never run");
    let mut by_project = BTreeMap::new();
    for id in &runs {
        let run = h.run(id)?;
        assert_eq!(text(&run, "/source_revision")?, candidate);
        by_project.insert(text(&run, "/project_id")?, run);
    }
    let docs = by_project.get("docs").ok_or("docs run")?;
    assert_eq!(text(docs, "/scope_identity")?, h.scope("docs")?);
    assert_eq!(text(docs, "/affected/reasons/0/kind")?, "declared");
    let primary = by_project.get("fixture").ok_or("fixture run")?;
    assert_eq!(text(primary, "/affected/reasons/0/kind")?, "path");

    // Canonical facts for the exact candidate: another project's success or an
    // older docs scope never satisfies docs; a current-scope success does.
    let fact = |project: &str, run: &str, ordinal: u64, scope: Option<String>| {
        kiln_core::change_review::CiFact {
            project: project.into(),
            repository: h.fixture.review.provider.repository_id.clone(),
            revision: candidate.clone(),
            run_id: run.into(),
            ordinal,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: scope,
            terminal: true,
        }
    };
    let primary_id = text(primary, "/id")?;
    let mut ci = vec![
        fact("fixture", &primary_id, 100, Some(h.scope("fixture")?)),
        fact("other", "other-run", 101, Some(h.scope("other")?)),
        fact("docs", "stale-docs-run", 102, Some("0".repeat(64))),
    ];
    let stale = h.detail_with(&ci)?;
    assert_eq!(field(&stale, "/checklist/ready")?, false);
    assert_eq!(text(row(&stale, "docs")?, "/state")?, "stale_scope");
    assert_eq!(text(row(&stale, "fixture")?, "/state")?, "passed");
    assert_eq!(text(row(&stale, "other")?, "/outcome")?, "skipped");
    ci.push(fact(
        "docs",
        "current-docs-run",
        103,
        Some(h.scope("docs")?),
    ));
    let current = h.detail_with(&ci)?;
    assert_eq!(text(row(&current, "docs")?, "/state")?, "passed");
    assert_eq!(field(&current, "/checklist/ready")?, true);

    // Real CI: the target moves only after every affected project passed.
    h.finish("fixture", true)?;
    h.tick()?;
    let waiting = h.detail_with(&[])?;
    assert_eq!(text(&waiting, "/queue/status")?, "waiting_ci");
    assert_eq!(h.fixture.target()?, h.fixture.target);
    h.finish("docs", true)?;
    h.tick()?;
    assert_eq!(h.fixture.target()?, candidate);
    assert_eq!(text(&h.detail_with(&[])?, "/queue/status")?, "submitted");
    assert_eq!(dispatches(&h, "other")?, 0);
    Ok(())
}

// r[verify kiln.monorepo.gating]
#[test]
fn monorepo_queue_failure_of_any_affected_project_blocks_submission() -> Result {
    let mut h = Monorepo::new()?;
    let (_candidate, runs) = h.start()?;
    assert_eq!(runs.len(), 2);
    h.finish("fixture", true)?;
    h.finish("docs", false)?;
    h.tick()?;
    let detail = h.detail_with(&[])?;
    assert_eq!(text(&detail, "/queue/status")?, "blocked");
    assert_eq!(text(&detail, "/queue/detail")?, "review_queue_ci_failed");
    assert_eq!(h.fixture.target()?, h.fixture.target);
    assert!(field(&detail, "/change/submission")?.is_null());
    Ok(())
}
