//! Native Lattice node execution with independent Web authority and exact Git admission.
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter};
use kiln_core::source::{validate_resolution, GitSnapshotLimits, SourceNativeFacts};
use kiln_core::workflow::WorkflowEffectPlan;
use kiln_provider_api::{SourceRepositoryPort, WorkflowDispatchBinding};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Durable Web cancellation intent is the only authority the backend observes
/// when the native task itself was destroyed without a signal.
const WEB_CANCEL_POLL_MS: u64 = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatticeExecutionRequest {
    pub policy_id: String,
    pub plan: WorkflowEffectPlan,
    pub binding: WorkflowDispatchBinding,
    pub lattice_execution: kiln_core::types::ExternalRef,
    pub node_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatticeExecutionPolicy {
    pub id: String,
    pub peer: String,
    pub workflow_policy_id: String,
    pub node_id: String,
    pub operation_identity: String,
    pub web_service: kiln_web_service::worker::ServiceConnection,
    pub organization: String,
    pub web_project: String,
    pub web_repository: String,
    pub repository_path: PathBuf,
    pub source_limits: GitSnapshotLimits,
    pub hosted_project: String,
    pub pipeline_id: String,
    pub price_id: String,
    pub class: String,
}
impl LatticeExecutionPolicy {
    pub fn admit(&self) -> Result<(), String> {
        let l = &self.source_limits;
        if [
            &self.id,
            &self.workflow_policy_id,
            &self.node_id,
            &self.organization,
            &self.web_project,
            &self.hosted_project,
            &self.pipeline_id,
            &self.price_id,
            &self.class,
        ]
        .iter()
        .any(|v| !crate::identifier(v))
            || self.operation_identity.len() != 64
            || !self
                .operation_identity
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || self.peer.parse::<iroh::PublicKey>().is_err()
            || self.web_repository.is_empty()
            || self.web_repository.len() > 2048
            || self.web_repository.chars().any(char::is_control)
            || !self.repository_path.is_absolute()
            || l.max_entries == 0
            || l.max_entries > 100_000
            || l.max_path_bytes == 0
            || l.max_path_bytes > 4096
            || l.max_file_bytes == 0
            || l.max_file_bytes > 64 * 1024 * 1024
            || l.max_total_bytes == 0
            || l.max_total_bytes > 256 * 1024 * 1024
        {
            return Err("Web execution policy rejected".into());
        }
        self.web_service.admit()?;
        Ok(())
    }
}

pub(crate) fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_lattice_bindings(organization TEXT NOT NULL,peer TEXT NOT NULL,web_project TEXT NOT NULL,run_id TEXT NOT NULL,node_id TEXT NOT NULL,identity TEXT NOT NULL,attempt TEXT NOT NULL,hosted_project TEXT NOT NULL,policy_identity TEXT NOT NULL,config_identity TEXT NOT NULL,execution_identity TEXT NOT NULL,web_approval_verified INTEGER NOT NULL DEFAULT 0,source_verified INTEGER NOT NULL DEFAULT 0,sequence INTEGER NOT NULL DEFAULT 0,response BLOB,PRIMARY KEY(organization,web_project,run_id,node_id));
        CREATE TRIGGER IF NOT EXISTS hosted_lattice_execution_insert AFTER INSERT ON hosted_execution BEGIN UPDATE hosted_lattice_bindings SET sequence=sequence+1 WHERE organization=NEW.org AND hosted_project=NEW.project AND attempt=NEW.attempt AND execution_identity=NEW.request_identity AND config_identity=NEW.config_identity; END;
        CREATE TRIGGER IF NOT EXISTS hosted_lattice_execution_update AFTER UPDATE OF state,result ON hosted_execution WHEN OLD.state IS NOT NEW.state OR OLD.result IS NOT NEW.result BEGIN UPDATE hosted_lattice_bindings SET sequence=sequence+1 WHERE organization=NEW.org AND hosted_project=NEW.project AND attempt=NEW.attempt AND execution_identity=NEW.request_identity AND config_identity=NEW.config_identity; END;")
        .map_err(|_| "Web binding schema unavailable".into())
}

fn policy<'a>(
    profile: &'a crate::service::Profile,
    actor: &crate::Actor,
    request: &LatticeExecutionRequest,
) -> Result<&'a LatticeExecutionPolicy, String> {
    profile
        .lattice_execution
        .iter()
        .find(|p| {
            p.id == request.policy_id
                && p.peer == actor.peer
                && p.organization == actor.organization
                && p.web_project == request.binding.project_id
        })
        .ok_or_else(|| "current Web approval authority policy denies request".into())
}

type Retained = (String, String, String, String, String, Option<Vec<u8>>);
fn retained(
    db: &Connection,
    actor: &crate::Actor,
    request: &LatticeExecutionRequest,
) -> Result<Option<Retained>, String> {
    db.query_row("SELECT identity,attempt,hosted_project,policy_identity,config_identity,response FROM hosted_lattice_bindings WHERE organization=?1 AND web_project=?2 AND run_id=?3 AND node_id=?4",
        params![actor.organization,request.binding.project_id,request.binding.run_id,request.node_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(|_| "Web binding lookup unavailable".into())
}

struct BoundObservation {
    sequence: u64,
    approval_verified: bool,
    source_verified: bool,
    request_identity: Option<String>,
    config_identity: Option<String>,
    state: Option<String>,
    raw_result: Option<String>,
    expected_request: String,
    expected_config: String,
}

// r[impl kiln.web_hosted.replay]
fn envelope(
    db: &Connection,
    actor: &crate::Actor,
    identity: &str,
    attempt: &str,
    project: &str,
    registry: &crate::service::CancellationRegistry,
) -> Result<Value, String> {
    let row = db.query_row(
        "SELECT b.sequence,b.web_approval_verified,b.source_verified,e.request_identity,e.config_identity,e.state,e.result,b.execution_identity,b.config_identity FROM hosted_lattice_bindings b LEFT JOIN hosted_execution e ON b.organization=e.org AND b.hosted_project=e.project AND b.attempt=e.attempt WHERE b.organization=?1 AND b.hosted_project=?2 AND b.attempt=?3 AND b.identity=?4",
        params![actor.organization, project, attempt, identity], |r| Ok(BoundObservation {
            sequence: crate::sqlite_u64(r, 0)?, approval_verified: r.get(1)?, source_verified: r.get(2)?,
            request_identity: r.get(3)?, config_identity: r.get(4)?, state: r.get(5)?, raw_result: r.get(6)?,
            expected_request: r.get(7)?, expected_config: r.get(8)?,
        }))
        .optional().map_err(|_| "Web execution observation unavailable")?
        .ok_or("verified Web execution record unavailable")?;
    if row.state.is_some()
        && (row.request_identity.as_ref() != Some(&row.expected_request)
            || row.config_identity.as_ref() != Some(&row.expected_config)
            || !row.approval_verified
            || !row.source_verified)
    {
        return Err("verified Web execution record unavailable".into());
    }
    let result = row
        .raw_result
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()
        .map_err(|_| "Web execution receipt malformed")?;
    let (registered, requested) = admission_status(registry, &actor.organization, attempt, false)?;
    // A durable reservation is not evidence of dispatch or source verification.
    // Its absence of an execution row remains Unknown across a crash or restart.
    Ok(
        json!({"binding_identity":identity,"attempt_id":attempt,"sequence":row.sequence,
        "state":row.state.as_deref().unwrap_or("unknown"),"result":result,
        "web_approval_verified":row.approval_verified,"source_verified":row.source_verified,
        "admission_registered":registered,"cancellation_requested":requested}),
    )
}

fn pipeline_request(
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
    attempt: &str,
) -> crate::execution::PipelineRequest {
    let untrusted =
        request
            .binding
            .untrusted
            .as_ref()
            .map(|gate| kiln_core::untrusted::UntrustedProvenance {
                reason: gate.reason,
                source_commit: gate.source_commit.clone(),
                approver_id: gate
                    .approval
                    .as_ref()
                    .map(|a| a.actor_id.clone())
                    .unwrap_or_default(),
            });
    crate::execution::PipelineRequest {
        project: policy.hosted_project.clone(),
        pipeline_id: policy.pipeline_id.clone(),
        attempt_id: attempt.into(),
        price_id: policy.price_id.clone(),
        class: policy.class.clone(),
        // Operator-trusted project enrollment does not waive untrusted provenance
        // admission: that independently enforces safe microVMs and no credentials/cache.
        workspace: attempt.into(),
        trust: "trusted".into(),
        event: None,
        untrusted,
    }
}

// r[impl kiln.web_hosted.admission]
// Registration precedes the durable metadata reservation, so every observable
// pending admission already has the same cancellation flag as its future runner.
struct AdmissionCancellation {
    registry: crate::service::CancellationRegistry,
    key: (String, String),
    flag: Arc<AtomicBool>,
}

impl AdmissionCancellation {
    fn register(
        registry: &crate::service::CancellationRegistry,
        key: (String, String),
    ) -> Result<Self, String> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut active = registry
            .lock()
            .map_err(|_| "execution registry unavailable")?;
        if active.contains_key(&key) {
            return Err("Lattice attempt already has executor".into());
        }
        active.insert(key.clone(), Arc::clone(&flag));
        Ok(Self {
            registry: Arc::clone(registry),
            key,
            flag,
        })
    }

    fn require_active(&self) -> Result<(), String> {
        if self.flag.load(Ordering::Acquire) {
            return Err("Lattice admission cancelled before execution".into());
        }
        Ok(())
    }

    fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

impl Drop for AdmissionCancellation {
    fn drop(&mut self) {
        if let Ok(mut active) = self.registry.lock() {
            active.remove(&self.key);
        }
    }
}

fn admission_status(
    registry: &crate::service::CancellationRegistry,
    organization: &str,
    attempt: &str,
    cancel: bool,
) -> Result<(bool, bool), String> {
    let active = registry
        .lock()
        .map_err(|_| "execution registry unavailable")?;
    if let Some(flag) = active.get(&(organization.into(), attempt.into())) {
        if cancel {
            flag.store(true, Ordering::Release);
        }
        Ok((true, flag.load(Ordering::Acquire)))
    } else {
        Ok((false, false))
    }
}

// No durable binding is not evidence that an independently arriving run cannot
// still register. Retain no orphan intents: the caller keeps its original run
// alive and retries cancellation only while admission remains unregistered.
fn unreserved_observation(
    registry: &crate::service::CancellationRegistry,
    organization: &str,
    identity: &str,
    attempt: &str,
    cancel: bool,
) -> Result<Value, String> {
    let (registered, requested) = admission_status(registry, organization, attempt, cancel)?;
    Ok(json!({
        "binding_identity": identity, "attempt_id": attempt, "sequence": 0,
        "state": "unknown", "result": null,
        "web_approval_verified": false, "source_verified": false,
        "admission_registered": registered, "cancellation_requested": requested,
    }))
}

/// Observes durable Web cancellation intent for an executing backend attempt.
/// The native Lattice task may be destroyed without a signal, so hosted keeps
/// its own bounded observation of the same authoritative Web state.
struct WebCancelWatch {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WebCancelWatch {
    fn start(
        connection: &kiln_web_service::worker::ServiceConnection,
        run_id: &str,
        cancellation: &Arc<AtomicBool>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let watch_stop = Arc::clone(&stop);
        let watch_cancellation = Arc::clone(cancellation);
        let connection = connection.clone();
        let run_id = run_id.to_owned();
        let handle = std::thread::spawn(move || {
            let Ok(client) = kiln_web_service::worker::ServiceClient::connect(&connection) else {
                return;
            };
            while !watch_stop.load(Ordering::Acquire) {
                let cancelled = client.execution_binding(&run_id).is_ok_and(|bound| {
                    bound.cancel_requested || bound.outcome == kiln_core::domain::Outcome::Cancelled
                });
                if cancelled {
                    watch_cancellation.store(true, Ordering::Release);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(WEB_CANCEL_POLL_MS));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    fn finish(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) fn handle(
    service: &mut crate::service::Service,
    actor: &crate::Actor,
    action: &str,
    input: Value,
    now: u64,
) -> Result<Value, String> {
    let request: LatticeExecutionRequest =
        serde_json::from_value(input).map_err(|_| "Web execution request rejected")?;
    let policy = policy(&service.profile, actor, &request)?.clone();
    crate::accounts::require_project(
        &service.db,
        actor,
        &policy.hosted_project,
        &["owner", "admin", "member"],
    )?;
    let identity = crate::identity(&request)?;
    let attempt = format!("lattice-{identity}");
    let policy_identity = crate::identity(&policy)?;
    let config_identity = crate::identity(
        service
            .profile
            .execution
            .as_ref()
            .ok_or("operator execution profile unavailable")?,
    )?;
    let pipeline = pipeline_request(&policy, &request, &attempt);
    let execution_identity = crate::identity(
        &json!({"organization":actor.organization,"request":pipeline,"configuration":config_identity,"plan":policy.pipeline_id}),
    )?;
    if let Some((old, original_attempt, project, original_policy, original_config, response)) =
        retained(&service.db, actor, &request)?
    {
        if old != identity
            || original_attempt != attempt
            || project != policy.hosted_project
            || original_policy != policy_identity
            || original_config != config_identity
        {
            return Err("Web run binding conflicts with retained input".into());
        }
        if let Some(bytes) = response {
            let original = serde_json::from_slice::<Result<Value, String>>(&bytes)
                .map_err(|_| "Web retained response malformed")?;
            return match original {
                Err(error) => Err(error),
                Ok(_) => envelope(
                    &service.db,
                    actor,
                    &identity,
                    &attempt,
                    &project,
                    &service.cancellations,
                ),
            };
        }
        if action == "lattice_cancel" {
            if !request.plan.controls.cancel {
                return Err("Web cancellation not configured".into());
            }
            admission_status(&service.cancellations, &actor.organization, &attempt, true)?;
        }
        // Reserved, not-yet-dispatched and interrupted admissions are observable
        // without promoting their verification flags or permitting redispatch.
        return envelope(
            &service.db,
            actor,
            &identity,
            &attempt,
            &project,
            &service.cancellations,
        );
    }
    if action != "lattice_run" {
        if action == "lattice_cancel" && !request.plan.controls.cancel {
            return Err("Web cancellation not configured".into());
        }
        return unreserved_observation(
            &service.cancellations,
            &actor.organization,
            &identity,
            &attempt,
            action == "lattice_cancel",
        );
    }
    if !crate::identifier(&request.binding.run_id) {
        return Err("Web run identity rejected".into());
    }
    // Local, non-effectful admission precedes any reservation so a tampered
    // node/operation/plan/source cannot consume durable binding retention.
    validate_request(&service.db, actor, &policy, &request)?;
    if crate::operations::is_held(&service.db, &actor.organization)? {
        return Err("organization incident hold denies new consumption".into());
    }
    let cancellation = AdmissionCancellation::register(
        &service.cancellations,
        (actor.organization.clone(), attempt.clone()),
    )?;
    service.db.execute("INSERT INTO hosted_lattice_bindings(organization,peer,web_project,run_id,node_id,identity,attempt,hosted_project,policy_identity,config_identity,execution_identity) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11 WHERE (SELECT COUNT(*) FROM hosted_lattice_bindings)<100000",
        params![actor.organization,actor.peer,request.binding.project_id,request.binding.run_id,request.node_id,identity,attempt,policy.hosted_project,policy_identity,config_identity,execution_identity])
        .map_err(|_| "Web binding reservation failed")?.eq(&1).then_some(()).ok_or("Web binding retention exhausted")?;
    let result = (|| {
        cancellation.require_active()?;
        verify_web_approval(&policy, &request)?;
        cancellation.require_active()?;
        let barrier = Arc::clone(&service.native_effects);
        let _guard = barrier
            .read()
            .map_err(|_| "Web source snapshot barrier unavailable")?;
        cancellation.require_active()?;
        service.db.execute("UPDATE hosted_lattice_bindings SET web_approval_verified=1 WHERE organization=?1 AND web_project=?2 AND run_id=?3 AND node_id=?4", params![actor.organization,request.binding.project_id,request.binding.run_id,request.node_id]).map_err(|_| "Web approval verification durability unknown")?;
        let config = service
            .profile
            .execution
            .as_ref()
            .ok_or("operator execution profile unavailable")?;
        let root = config
            .work_root
            .join(&actor.organization)
            .join(&policy.hosted_project);
        // Tenant roots must already be operator-enrolled, never follow an alias.
        if root
            .canonicalize()
            .map_err(|_| "Web tenant source root unavailable")?
            != root
        {
            return Err("Web tenant source root traverses symlink".into());
        }
        let owner = std::fs::metadata("/proc/self")
            .map_err(|_| "Web source owner unavailable")?
            .uid();
        for ancestor in root.ancestors() {
            let meta = std::fs::symlink_metadata(ancestor)
                .map_err(|_| "Web source ancestor unavailable")?;
            if !meta.is_dir()
                || meta.file_type().is_symlink()
                || (meta.uid() != owner && meta.uid() != 0)
                || (meta.mode() & 0o022 != 0 && meta.mode() & 0o1000 == 0)
                || (ancestor == root && (meta.uid() != owner || meta.mode() & 0o022 != 0))
            {
                return Err("Web source root must have operator-owned safe ancestors".into());
            }
        }
        let source = acquire(&policy, &request, &root, &attempt)?;
        cancellation.require_active()?;
        service.db.execute("UPDATE hosted_lattice_bindings SET source_verified=1 WHERE organization=?1 AND web_project=?2 AND run_id=?3 AND node_id=?4", params![actor.organization,request.binding.project_id,request.binding.run_id,request.node_id]).map_err(|_| "Web source verification durability unknown")?;
        cancellation.require_active()?;
        let watch = WebCancelWatch::start(
            &policy.web_service,
            &request.binding.run_id,
            &cancellation.shared_flag(),
        );
        let executed = crate::execution::run_pipeline(
            &mut service.db,
            actor,
            pipeline,
            now,
            &cancellation.flag,
        );
        // Keep private immutable source alive until all pipeline snapshot reads finish.
        drop(source);
        watch.finish();
        executed?;
        envelope(
            &service.db,
            actor,
            &identity,
            &attempt,
            &policy.hosted_project,
            &service.cancellations,
        )
    })();
    let bytes = serde_json::to_vec(&result).map_err(|_| "Web response encoding failed")?;
    if bytes.len() > crate::MAX_FRAME_BYTES / 2 {
        return Err("Web response exceeds retention bound".into());
    }
    service.db.execute("UPDATE hosted_lattice_bindings SET response=?5 WHERE organization=?1 AND web_project=?2 AND run_id=?3 AND node_id=?4",params![actor.organization,request.binding.project_id,request.binding.run_id,request.node_id,bytes]).map_err(|_| "Web binding response durability unknown")?;
    result
}

fn validate_bound_source(
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
) -> Result<(), String> {
    kiln_core::workflow::validate_workflow_plan(&request.plan)
        .map_err(|_| "Web workflow plan rejected")?;
    if request.plan.workflow_policy_id.as_str() != policy.workflow_policy_id
        || request.node_id != policy.node_id
        || !request.plan.nodes.iter().any(|node| {
            node.id.as_str() == request.node_id
                && node.operation_identity.as_str() == policy.operation_identity
        })
        || request.lattice_execution.provider_namespace.as_str() != "lattice"
    {
        return Err("Lattice workflow policy, node, operation or execution rejected".into());
    }
    let source = &request.binding.source;
    validate_resolution(source).map_err(|_| "Web exact source resolution rejected")?;
    if !matches!(&source.native_facts, SourceNativeFacts::Git(_))
        || source.repository.native_id.as_str() != policy.web_repository
        || source.exact_revision.provider_namespace.as_str() != "git"
        || source.exact_revision.native_kind.as_str() != "commit"
        || !kiln_core::untrusted::valid_commit(source.exact_revision.native_id.as_str())
    {
        return Err("Web source repository or exact Git revision rejected".into());
    }
    let context = request
        .plan
        .source
        .as_ref()
        .ok_or("Web workflow lacks exact source")?;
    if context.provider.as_str() != "git"
        || context.repository != source.repository
        || context.revision.provider_namespace.as_str() != "git"
        || context.revision.native_kind.as_str() != "revision"
        || context.revision.native_id != source.exact_revision.native_id
    {
        return Err("Web workflow source differs from approval binding".into());
    }
    if let Some(gate) = &request.binding.untrusted {
        if !gate.may_dispatch()
            || gate.superseded_by.is_some()
            || gate.pull_request == 0
            || gate.source_commit != source.exact_revision.native_id.as_str()
            || gate.head_repository.is_empty()
            || gate.head_repository.len() > 2048
            || !gate.approval.as_ref().is_some_and(|a| {
                !a.actor_id.is_empty()
                    && a.actor_id.len() <= 256
                    && !a.actor_id.chars().any(char::is_control)
            })
        {
            return Err("Web exact-commit approval rejected".into());
        }
    }
    if request.plan.controls.pause || request.plan.controls.resume {
        return Err("hosted Web pause/resume unsupported".into());
    }
    Ok(())
}

fn verify_web_binding(
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
    authoritative: &kiln_web_service::worker::ExecutionBinding,
) -> Result<(), String> {
    validate_bound_source(policy, request)?;
    if authoritative.phase != "active"
        || authoritative.cancel_requested
        || authoritative.outcome != kiln_core::domain::Outcome::Pending
        || authoritative.execution.as_ref() != Some(&request.lattice_execution)
        || authoritative.plan != request.plan
        || authoritative.binding != request.binding
    {
        return Err("authoritative Web execution binding differs from Lattice request".into());
    }
    Ok(())
}

fn verify_web_approval(
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
) -> Result<(), String> {
    let client = kiln_web_service::worker::ServiceClient::connect(&policy.web_service)?;
    let authoritative = client.execution_binding(&request.binding.run_id)?;
    verify_web_binding(policy, request, &authoritative)
}

fn validate_request(
    db: &Connection,
    actor: &crate::Actor,
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
) -> Result<(), String> {
    validate_bound_source(policy, request)?;
    // The reviewed hosted pipeline is the implementation of one outer Lattice
    // node, not a second copy of the Web workflow graph.
    crate::pipelines::reviewed(db, actor, &policy.hosted_project, &policy.pipeline_id)?;
    Ok(())
}

fn frame<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let raw: [u8; 8] = bytes
        .get(..8)
        .ok_or("Git snapshot framing truncated")?
        .try_into()
        .map_err(|_| "Git snapshot framing rejected")?;
    let size =
        usize::try_from(u64::from_be_bytes(raw)).map_err(|_| "Git snapshot length rejected")?;
    *bytes = bytes.get(8..).ok_or("Git snapshot framing truncated")?;
    let value = bytes.get(..size).ok_or("Git snapshot framing truncated")?;
    *bytes = bytes.get(size..).ok_or("Git snapshot framing truncated")?;
    Ok(value)
}

struct PrivateSource(PathBuf);
impl Drop for PrivateSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn acquire(
    policy: &LatticeExecutionPolicy,
    request: &LatticeExecutionRequest,
    root: &Path,
    attempt: &str,
) -> Result<PrivateSource, String> {
    let profile = GitSourceProfile::pinned_local(
        request.binding.source.source_provider.clone(),
        policy.source_limits.clone(),
    );
    let adapter = LocalGitAdapter::new(
        policy.repository_path.clone(),
        request
            .binding
            .source
            .exact_revision
            .native_id
            .as_str()
            .to_owned(),
        profile,
    )
    .map_err(|_| "hosted Git source profile rejected")?;
    let manifest = adapter
        .acquire_canonical_manifest(&request.binding.source.exact_revision)
        .map_err(|_| "hosted exact Git acquisition failed")?;
    let identity = kiln_codec::identity_from_bytes(&manifest)
        .map_err(|_| "hosted Git tree identity unavailable")?;
    if identity != request.binding.source.canonical_tree {
        return Err("hosted Git canonical tree differs from Web binding".into());
    }
    let staging = tempfile::Builder::new()
        .prefix(".lattice-source-")
        .tempdir_in(root)
        .map_err(|_| "private Web source staging unavailable")?;
    std::fs::set_permissions(staging.path(), std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "Web source permissions unavailable")?;
    materialize(&manifest, staging.path())?;
    let target = root.join(attempt);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err("Web source already exists; refusing mutable reuse".into());
    }
    // Atomic rename preserves private directory ownership and leaves no caller-writable window.
    std::fs::rename(staging.path(), &target)
        .map_err(|_| "private Web source installation failed")?;
    let _old = staging.keep();
    Ok(PrivateSource(target))
}

fn materialize(manifest: &[u8], root: &Path) -> Result<(), String> {
    let mut bytes = manifest
        .strip_prefix(b"kiln.git.snapshot.v1\0")
        .ok_or("Git snapshot format rejected")?;
    while !bytes.is_empty() {
        let (kind, remaining) = bytes.split_first().ok_or("Git snapshot kind missing")?;
        let kind = *kind;
        bytes = remaining;
        let path =
            std::str::from_utf8(frame(&mut bytes)?).map_err(|_| "Git source path is not UTF-8")?;
        let data = frame(&mut bytes)?;
        let relative = Path::new(path);
        if path.is_empty()
            || path.contains('\\')
            || relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || path
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
        {
            return Err("unsafe Git source path".into());
        }
        // Symlinks may reference runtime roots or alias parent entries; fail closed.
        if !matches!(kind, b'f' | b'x') {
            return Err("hosted Web source symlinks and special modes unsupported".into());
        }
        let target = root.join(relative);
        let parent = target.parent().ok_or("Git source parent unavailable")?;
        std::fs::create_dir_all(parent).map_err(|_| "Git source directory collision")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&target)
            .map_err(|_| "Git source file collision")?;
        file.write_all(data)
            .map_err(|_| "Git source write failed")?;
        file.sync_all()
            .map_err(|_| "Git source durability failed")?;
        file.set_permissions(std::fs::Permissions::from_mode(if kind == b'x' {
            0o500
        } else {
            0o400
        }))
        .map_err(|_| "Git source mode failed")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::source::{build_git_snapshot_manifest, GitEntryKind, GitSnapshotEntry};
    use kiln_core::types::ExternalRef;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn cancellation_before_binding_reservation_stops_registered_admission() {
        let registry = crate::service::CancellationRegistry::default();
        let (registered_tx, registered_rx) = std::sync::mpsc::channel();
        let (reserve_tx, reserve_rx) = std::sync::mpsc::channel();
        let runner_registry = Arc::clone(&registry);
        let runner = std::thread::spawn(move || {
            let admission = AdmissionCancellation::register(
                &runner_registry,
                ("org".into(), "lattice-binding".into()),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
            registered_tx
                .send(())
                .unwrap_or_else(|e| unreachable!("{e}"));
            reserve_rx.recv().unwrap_or_else(|e| unreachable!("{e}"));
            // This is the first execution checkpoint after row reservation.
            admission.require_active()
        });
        registered_rx.recv().unwrap_or_else(|e| unreachable!("{e}"));
        let changed =
            unreserved_observation(&registry, "org", "different", "lattice-different", true)
                .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(changed.get("admission_registered"), Some(&json!(false)));
        let unchanged =
            unreserved_observation(&registry, "org", "binding", "lattice-binding", false)
                .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(unchanged.get("cancellation_requested"), Some(&json!(false)));
        let pending = unreserved_observation(&registry, "org", "binding", "lattice-binding", true)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(pending.get("admission_registered"), Some(&json!(true)));
        assert_eq!(pending.get("cancellation_requested"), Some(&json!(true)));
        assert_eq!(pending.get("state"), Some(&json!("unknown")));
        assert_eq!(pending.get("result"), Some(&Value::Null));
        assert_eq!(pending.get("web_approval_verified"), Some(&json!(false)));
        assert_eq!(pending.get("source_verified"), Some(&json!(false)));
        let observed =
            unreserved_observation(&registry, "org", "binding", "lattice-binding", false)
                .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(observed.get("cancellation_requested"), Some(&json!(true)));
        reserve_tx.send(()).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            runner
                .join()
                .unwrap_or_else(|_| unreachable!("runner panicked")),
            Err("Lattice admission cancelled before execution".into())
        );
        assert!(registry
            .lock()
            .unwrap_or_else(|e| unreachable!("{e}"))
            .is_empty());
    }

    #[test]
    fn cancellation_before_registration_requires_retry_without_orphan_intent() {
        let registry = crate::service::CancellationRegistry::default();
        let pending = unreserved_observation(&registry, "org", "binding", "lattice-binding", true)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(pending.get("state"), Some(&json!("unknown")));
        assert_eq!(pending.get("admission_registered"), Some(&json!(false)));
        assert_eq!(pending.get("cancellation_requested"), Some(&json!(false)));
        assert!(registry
            .lock()
            .unwrap_or_else(|e| unreachable!("{e}"))
            .is_empty());
        let admission =
            AdmissionCancellation::register(&registry, ("org".into(), "lattice-binding".into()))
                .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(admission.require_active(), Ok(()));
        unreserved_observation(&registry, "org", "binding", "lattice-binding", true)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            admission.require_active(),
            Err("Lattice admission cancelled before execution".into())
        );
    }

    fn manifest(entries: &[(&str, GitEntryKind, &[u8])]) -> Vec<u8> {
        let values = entries
            .iter()
            .map(|(path, kind, bytes)| GitSnapshotEntry {
                path: (*path).into(),
                kind: *kind,
                native_object: ExternalRef::new("git", "object", &"1".repeat(40))
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                bytes: bytes.to_vec(),
            })
            .collect::<Vec<_>>();
        build_git_snapshot_manifest(
            &values,
            &GitSnapshotLimits {
                max_entries: 8,
                max_path_bytes: 128,
                max_file_bytes: 128,
                max_total_bytes: 256,
            },
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"))
    }

    #[test]
    fn materialized_git_bytes_and_executable_mode_are_exact_and_not_writable() {
        let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        let bytes = manifest(&[
            ("src/data", GitEntryKind::Regular, b"\0exact\xff"),
            ("bin/run", GitEntryKind::Executable, b"#!/bin/sh\nexit 7\n"),
        ]);
        materialize(&bytes, root.path()).unwrap_or_else(|e| unreachable!("{e}"));
        for (path, expected, mode) in [
            ("src/data", b"\0exact\xff".as_slice(), 0o400),
            ("bin/run", b"#!/bin/sh\nexit 7\n".as_slice(), 0o500),
        ] {
            assert_eq!(
                std::fs::read(root.path().join(path)).ok().as_deref(),
                Some(expected)
            );
            assert_eq!(
                std::fs::metadata(root.path().join(path))
                    .map(|m| m.permissions().mode() & 0o777)
                    .ok(),
                Some(mode)
            );
        }
    }

    #[test]
    fn source_symlinks_cannot_alias_runtime_or_escape_staging() {
        for target in [
            b"/etc/shadow".as_slice(),
            b"../../outside".as_slice(),
            b"data".as_slice(),
        ] {
            let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
            let bytes = manifest(&[("link", GitEntryKind::Symlink, target)]);
            assert_eq!(
                materialize(&bytes, root.path()),
                Err("hosted Web source symlinks and special modes unsupported".into())
            );
            assert!(std::fs::symlink_metadata(root.path().join("link")).is_err());
        }
    }

    #[test]
    fn file_directory_alias_and_duplicate_materialization_fail_without_overwriting() {
        let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        let original = manifest(&[("value", GitEntryKind::Regular, b"original")]);
        materialize(&original, root.path()).unwrap_or_else(|e| unreachable!("{e}"));
        let replacement = manifest(&[("value", GitEntryKind::Regular, b"replacement")]);
        assert!(materialize(&replacement, root.path()).is_err());
        let alias = manifest(&[("value/child", GitEntryKind::Regular, b"escaped")]);
        assert!(materialize(&alias, root.path()).is_err());
        assert_eq!(
            std::fs::read(root.path().join("value")).ok().as_deref(),
            Some(b"original".as_slice())
        );
    }

    fn approved_binding() -> (LatticeExecutionPolicy, LatticeExecutionRequest) {
        use kiln_core::source::{GitObjectFormat, GitResolutionFacts, SourceResolution};
        use kiln_core::types::{ContentId, ProviderProfileId};
        let repository =
            ExternalRef::new("git", "repository", "repo").unwrap_or_else(|e| unreachable!("{e:?}"));
        let reference = ExternalRef::new("git", "reference", "refs/heads/main")
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let commit = ExternalRef::new("git", "commit", &"a".repeat(40))
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let source = SourceResolution {
            source_provider: ProviderProfileId::new("git-source")
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            repository: repository.clone(),
            symbolic_reference: reference.clone(),
            exact_revision: commit.clone(),
            canonical_tree: ContentId::from_blake3_hex("1".repeat(64))
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            native_facts: SourceNativeFacts::Git(GitResolutionFacts {
                repository: repository.clone(),
                symbolic_reference: reference,
                exact_commit: commit.clone(),
                native_tree: ExternalRef::new("git", "tree", &"b".repeat(40))
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                observed_ref_before: commit.clone(),
                observed_ref_after: commit,
                object_format: GitObjectFormat::Sha1,
                partial_repository: false,
                replacement_objects: false,
                alternate_object_database: false,
                signature_observed: false,
            }),
        };
        let plan: WorkflowEffectPlan = serde_json::from_value(json!({
            "effect_id":"effect", "attempt_id":"attempt", "candidate_id":"candidate",
            "pipeline_revision_id":"web-pipeline", "workflow_policy_id":"policy", "provider_profile":"bridge",
            "idempotency_key":"1".repeat(64), "platform":"hosted",
            "source":{"provider":"git","repository":repository,
                "revision":ExternalRef::new("git","revision",&"a".repeat(40)).unwrap_or_else(|e| unreachable!("{e:?}"))},
            "nodes":[{"id":"build","kind":"build","effect_class":"Build","adapter_kind":"hosted","operation_identity":"2".repeat(64)}],
            "edges":[], "controls":{"cancel":true,"pause":false,"resume":false},
            "bounds":{"max_nodes":4,"max_edges":4,"max_log_references":4,"max_log_read_bytes":1024,"max_total_log_bytes":4096}
        })).unwrap_or_else(|e| unreachable!("{e}"));
        let mut gate = kiln_core::untrusted::UntrustedGate::new(
            kiln_core::untrusted::UntrustedReason::Fork,
            1,
            "fork/repo".into(),
            "a".repeat(40),
            false,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        gate.approval = Some(kiln_core::untrusted::UntrustedApproval {
            actor_id: "owner".into(),
            actor_label: "Owner".into(),
            source_commit: "a".repeat(40),
            approved_at_ms: 1,
        });
        let request = LatticeExecutionRequest {
            policy_id: "bridge-policy".into(),
            node_id: "build".into(),
            lattice_execution: ExternalRef::new("lattice", "execution", "native-run")
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            plan,
            binding: WorkflowDispatchBinding {
                run_id: "web-run".into(),
                project_id: "web-project".into(),
                source,
                untrusted: Some(gate),
            },
        };
        let policy = LatticeExecutionPolicy {
            id: request.policy_id.clone(),
            workflow_policy_id: "policy".into(),
            node_id: "build".into(),
            operation_identity: "2".repeat(64),
            web_service: kiln_web_service::worker::ServiceConnection {
                service_origin: "https://web.example".into(),
                tls_ca_path: "/operator/ca.pem".into(),
                token_file: "/operator/worker.token".into(),
                http_timeout_ms: 1000,
                maximum_response_bytes: 1048576,
            },
            peer: "0".repeat(64),
            organization: "org".into(),
            web_project: "web-project".into(),
            web_repository: "repo".into(),
            repository_path: "/operator/repo".into(),
            source_limits: GitSnapshotLimits {
                max_entries: 8,
                max_path_bytes: 128,
                max_file_bytes: 128,
                max_total_bytes: 256,
            },
            hosted_project: "hosted-project".into(),
            pipeline_id: "1".repeat(64),
            price_id: "2".repeat(64),
            class: "cpu".into(),
        };
        (policy, request)
    }

    #[test]
    fn authoritative_web_record_must_match_active_exact_native_execution() {
        let (policy, request) = approved_binding();
        let approved = kiln_web_service::worker::ExecutionBinding {
            plan: request.plan.clone(),
            binding: request.binding.clone(),
            execution: Some(request.lattice_execution.clone()),
            phase: "active".into(),
            outcome: kiln_core::domain::Outcome::Pending,
            cancel_requested: false,
            log_bounds: kiln_web_service::worker::LogBounds {
                maximum_chunk_bytes: 1024,
                maximum_total_bytes: 4096,
                maximum_chunks: 4,
            },
        };
        assert_eq!(verify_web_binding(&policy, &request, &approved), Ok(()));
        let mut wrong = approved.clone();
        wrong.outcome = kiln_core::domain::Outcome::Success;
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong.cancel_requested = true;
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong.phase = "waiting".into();
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong.execution = None;
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong.binding.untrusted = None;
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong.binding.source.canonical_tree =
            kiln_core::types::ContentId::from_blake3_hex("3".repeat(64))
                .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        wrong = approved.clone();
        wrong
            .plan
            .nodes
            .first_mut()
            .unwrap_or_else(|| unreachable!("fixture node"))
            .operation_identity = kiln_core::types::ContentId::from_blake3_hex("3".repeat(64))
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert!(verify_web_binding(&policy, &request, &wrong).is_err());
        let mut substituted = request.clone();
        substituted.node_id = "other".into();
        assert!(verify_web_binding(&policy, &substituted, &approved).is_err());
        substituted = request.clone();
        substituted.lattice_execution = ExternalRef::new("relay", "execution", "native-run")
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert!(verify_web_binding(&policy, &substituted, &approved).is_err());
        let mut wrong_policy = policy.clone();
        wrong_policy.workflow_policy_id = "other".into();
        assert!(verify_web_binding(&wrong_policy, &request, &approved).is_err());
        wrong_policy = policy.clone();
        wrong_policy.operation_identity = "3".repeat(64);
        assert!(verify_web_binding(&wrong_policy, &request, &approved).is_err());
    }

    #[test]
    fn web_revision_context_matches_git_commit_but_stale_approval_cannot_dispatch() {
        let (policy, request) = approved_binding();
        assert_eq!(validate_bound_source(&policy, &request), Ok(()));
        let mut stale = request.clone();
        stale
            .binding
            .untrusted
            .as_mut()
            .and_then(|gate| gate.approval.as_mut())
            .unwrap_or_else(|| unreachable!("approved fixture"))
            .source_commit = "b".repeat(40);
        assert_eq!(
            validate_bound_source(&policy, &stale),
            Err("Web exact-commit approval rejected".into())
        );
        let mut changed_context = request;
        changed_context
            .plan
            .source
            .as_mut()
            .unwrap_or_else(|| unreachable!("source fixture"))
            .revision = ExternalRef::new("git", "revision", &"c".repeat(40))
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(
            validate_bound_source(&policy, &changed_context),
            Err("Web workflow source differs from approval binding".into())
        );
    }

    #[test]
    fn recovery_versions_changed_receipts_without_writing_or_rerunning() {
        let db = Connection::open_in_memory().unwrap_or_else(|e| unreachable!("{e}"));
        crate::execution::init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        let actor = crate::Actor {
            peer: "issuer".into(),
            organization: "org".into(),
            role: "owner".into(),
        };
        db.execute("INSERT INTO hosted_lattice_bindings(organization,peer,web_project,run_id,node_id,identity,attempt,hosted_project,policy_identity,config_identity,execution_identity,web_approval_verified,source_verified) VALUES('org','issuer','web','run','build','binding','attempt','project','policy','config','request',1,1)", []).unwrap_or_else(|e| unreachable!("{e}"));
        db.execute("INSERT INTO hosted_execution VALUES('org','project','attempt','pipeline','request','config','unit','running',1,NULL)", []).unwrap_or_else(|e| unreachable!("{e}"));
        let observe = || {
            envelope(
                &db,
                &actor,
                "binding",
                "attempt",
                "project",
                &Default::default(),
            )
            .unwrap_or_else(|e| unreachable!("{e}"))
        };
        let before: i64 = db
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let running = observe();
        assert_eq!(running.get("state"), Some(&json!("running")));
        assert_eq!(running.get("sequence"), Some(&json!(1)));
        assert_eq!(observe(), running);
        let after: i64 = db
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(before, after);
        db.execute(
            "UPDATE hosted_execution SET state='unknown' WHERE org='org' AND attempt='attempt'",
            [],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(observe().get("state"), Some(&json!("unknown")));
        assert_eq!(observe().get("sequence"), Some(&json!(2)));
        db.execute("UPDATE hosted_execution SET state='failed',result=?1 WHERE org='org' AND attempt='attempt'", [json!({"state":"failed","reason":"executor lost"}).to_string()]).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(observe().get("sequence"), Some(&json!(3)));
        db.execute(
            "UPDATE hosted_execution SET result=?1 WHERE org='org' AND attempt='attempt'",
            [json!({"state":"failed","reason":"cleanup failed"}).to_string()],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        let changed = observe();
        assert_eq!(changed.get("sequence"), Some(&json!(4)));
        assert_eq!(
            changed.pointer("/result/reason"),
            Some(&json!("cleanup failed"))
        );
        db.execute("UPDATE hosted_execution SET state=state,result=result WHERE org='org' AND attempt='attempt'", []).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(observe(), changed);
    }

    #[test]
    fn verified_crash_window_cannot_adopt_another_execution_request() {
        let db = Connection::open_in_memory().unwrap_or_else(|e| unreachable!("{e}"));
        crate::execution::init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        let actor = crate::Actor {
            peer: "issuer".into(),
            organization: "org".into(),
            role: "owner".into(),
        };
        // Crash after source verification but before run_pipeline inserted its
        // exact request. The retained authority facts alone are not execution.
        db.execute("INSERT INTO hosted_lattice_bindings(organization,peer,web_project,run_id,node_id,identity,attempt,hosted_project,policy_identity,config_identity,execution_identity,web_approval_verified,source_verified) VALUES('org','issuer','web','run','build','binding','lattice-attempt','project','policy','config','expected-request',1,1)", []).unwrap_or_else(|e| unreachable!("{e}"));
        let verified = envelope(
            &db,
            &actor,
            "binding",
            "lattice-attempt",
            "project",
            &Default::default(),
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(verified.get("state"), Some(&json!("unknown")));
        assert_eq!(verified.get("result"), Some(&Value::Null));
        assert_eq!(verified.get("sequence"), Some(&json!(0)));
        db.execute(
            "UPDATE hosted_lattice_bindings SET web_approval_verified=0,source_verified=0",
            [],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        let pending = envelope(
            &db,
            &actor,
            "binding",
            "lattice-attempt",
            "project",
            &Default::default(),
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(pending.get("state"), Some(&json!("unknown")));
        assert_eq!(pending.get("result"), Some(&Value::Null));
        assert_eq!(pending.get("web_approval_verified"), Some(&json!(false)));
        assert_eq!(pending.get("source_verified"), Some(&json!(false)));
        assert_eq!(pending.get("admission_registered"), Some(&json!(false)));
        assert_eq!(pending.get("cancellation_requested"), Some(&json!(false)));
        let registry = crate::service::CancellationRegistry::default();
        let admission =
            AdmissionCancellation::register(&registry, ("org".into(), "lattice-attempt".into()))
                .unwrap_or_else(|e| unreachable!("{e}"));
        admission_status(&registry, "org", "lattice-attempt", true)
            .unwrap_or_else(|e| unreachable!("{e}"));
        let cancelled_admission = envelope(
            &db,
            &actor,
            "binding",
            "lattice-attempt",
            "project",
            &registry,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            cancelled_admission.get("admission_registered"),
            Some(&json!(true))
        );
        assert_eq!(
            cancelled_admission.get("cancellation_requested"),
            Some(&json!(true))
        );
        assert_eq!(cancelled_admission.get("state"), Some(&json!("unknown")));
        assert_eq!(cancelled_admission.get("result"), Some(&Value::Null));
        drop(admission);
        db.execute(
            "UPDATE hosted_lattice_bindings SET web_approval_verified=1,source_verified=1",
            [],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        // Even if an unrelated row was introduced while policy was removed,
        // restoring policy must never attach the old approval/source to it.
        db.execute("INSERT INTO hosted_execution VALUES('org','project','lattice-attempt','pipeline','substituted-request','config','unit','succeeded',1,?1)", [json!({"state":"succeeded","jobs":[]}).to_string()]).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            envelope(
                &db,
                &actor,
                "binding",
                "lattice-attempt",
                "project",
                &Default::default()
            ),
            Err("verified Web execution record unavailable".into())
        );
        let sequence: i64 = db
            .query_row("SELECT sequence FROM hosted_lattice_bindings", [], |r| {
                r.get(0)
            })
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(sequence, 0);
        db.execute("UPDATE hosted_execution SET request_identity='expected-request',config_identity='substituted-config'", []).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            envelope(
                &db,
                &actor,
                "binding",
                "lattice-attempt",
                "project",
                &Default::default()
            ),
            Err("verified Web execution record unavailable".into())
        );
    }
}
