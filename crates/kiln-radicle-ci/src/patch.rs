//! Exact Radicle revision CI through the existing authenticated hosted port.
//! No provider payload or approval can turn an untrusted run into a trusted one.
use crate::project::identity;
use crate::{Result, storage};
use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use kiln_adapter_radicle::job::{
    JobDispatch, JobPlan, JobState, RadicleJobProfile, RadicleJobShell,
};
use kiln_core::types::{ContentId, ExternalRef, RunId};
use kiln_core::untrusted::{
    APPROVER_AUTHORITY, ApproveUntrustedRun, UntrustedApproval, UntrustedProvenance,
    UntrustedReason,
};
use kiln_onixos_pilot::{JournalLock, MAX_CONFIG_BYTES, load_json, publish_new};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const PATCH_PROFILE_SCHEMA: &str = "kiln.radicle-ci.patch-profile.v1";
const PATCH_JOURNAL_SCHEMA: &str = "kiln.radicle-ci.patch-journal.v1";
const MAX_PROJECTION_BYTES: usize = 1_048_576;
const RPC_TIMEOUT_MS: u64 = 35_000;
const MAX_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LOG_URL_PREFIX_BYTES: usize = kiln_adapter_radicle::job::DEFAULT_MAX_LOG_URL_BYTES
    - ("radicle-patch-".len() + blake3::OUT_LEN * 2);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub path: PathBuf,
    pub blake3: ContentId,
}
impl Executable {
    pub fn admit(&self) -> Result<()> {
        if !self.path.is_absolute() {
            return Err("reviewed executable must be absolute".into());
        }
        let file = std::fs::File::open(&self.path).map_err(text)?;
        let metadata = file.metadata().map_err(text)?;
        if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_BYTES {
            return Err("executable must be a bounded regular file".into());
        }
        let mut hasher = blake3::Hasher::new();
        let hashed_bytes = std::io::copy(
            &mut std::io::Read::take(file, MAX_EXECUTABLE_BYTES + 1),
            &mut hasher,
        )
        .map_err(text)?;
        if hashed_bytes > MAX_EXECUTABLE_BYTES {
            return Err("executable exceeded its admitted size".into());
        }
        if hasher.finalize().to_hex().as_str() != self.blake3.as_str() {
            return Err("reviewed executable identity drift".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedPatchProfile {
    pub executable: Executable,
    pub client_profile: PathBuf,
    pub client_profile_identity: ContentId,
    pub organization: String,
    pub project: String,
    pub pipeline_id: String,
    pub price_id: String,
    pub class: String,
    /// Existing private work_root/<organization>/<project>, shared with the
    /// hosted service. Each revision gets a new exact-source child directory.
    pub workspace_root: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchProfile {
    pub schema: String,
    pub repository_id: String,
    pub storage_root: PathBuf,
    pub helper: Executable,
    pub state_root: PathBuf,
    pub snapshot_limits: kiln_radicle_source_contract::SnapshotLimits,
    /// Explicit operator-admitted delegate/followed node keys. Native broker
    /// pusher/delegate metadata never enters this set.
    pub trusted_authors: BTreeSet<String>,
    /// Interactive local operators permitted to use patch-approve.
    pub approvers: BTreeSet<String>,
    pub hosted: HostedPatchProfile,
    pub job_profile: RadicleJobProfile,
    pub log_url_prefix: String,
    #[serde(skip)]
    native_fixtures: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionBinding {
    pub repository: String,
    pub patch: String,
    pub revision: String,
    pub base: String,
    pub oid: String,
    pub author: String,
}
#[derive(Clone, Debug, Deserialize)]
struct NativeRevision {
    id: String,
    base: String,
    head: String,
    author: String,
    signed_source: Option<Value>,
}
#[derive(Clone, Debug, Deserialize)]
struct NativePatch {
    patch_id: String,
    heads: BTreeSet<String>,
    revisions: Vec<NativeRevision>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchState {
    Blocked,
    Ready,
    Submitted,
    Running,
    Unknown,
    Succeeded,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchJournal {
    pub schema: String,
    pub profile_identity: ContentId,
    pub binding: RevisionBinding,
    pub trigger: TriggerRequest,
    pub run_id: String,
    pub observed_heads: BTreeSet<String>,
    pub trusted: bool,
    pub approval: Option<UntrustedApproval>,
    pub state: PatchState,
    pub source_tree: Option<String>,
    pub request: Option<Value>,
    pub observation: Option<Value>,
    pub transport_error: Option<String>,
    pub observation_sequence: u32,
    pub job: Option<JobDispatch>,
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn oid(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl PatchProfile {
    #[cfg(feature = "native-fixtures")]
    pub fn enable_native_fixtures(&mut self) {
        self.native_fixtures = true;
    }

    fn job_shell(&self) -> Result<RadicleJobShell> {
        if self.native_fixtures {
            #[cfg(not(feature = "native-fixtures"))]
            return Err("native fixture authority is not compiled into this binary".into());
            #[cfg(feature = "native-fixtures")]
            return RadicleJobShell::new_fixture(
                self.storage_root.clone(),
                self.job_profile.clone(),
            )
            .map_err(|e| format!("{e:?}"));
        }
        RadicleJobShell::new(self.storage_root.clone(), self.job_profile.clone())
            .map_err(|e| format!("{e:?}"))
    }
    pub fn admit(&self) -> Result<()> {
        if self.schema != PATCH_PROFILE_SCHEMA
            || self.repository_id.is_empty()
            || !self.storage_root.is_absolute()
            || !self.state_root.is_absolute()
            || self.approvers.is_empty()
            || self.approvers.len() > 256
            || self.trusted_authors.len() > 256
            || self.approvers.iter().any(|a| !label(a))
            || ![
                &self.hosted.organization,
                &self.hosted.project,
                &self.hosted.pipeline_id,
                &self.hosted.price_id,
                &self.hosted.class,
            ]
            .into_iter()
            .all(|s| label(s))
            || !self.hosted.workspace_root.is_absolute()
            || !self.hosted.client_profile.is_absolute()
            || !self.log_url_prefix.starts_with("https://")
            || self.log_url_prefix.len() > MAX_LOG_URL_PREFIX_BYTES
            || self.snapshot_limits.max_entries == 0
            || self.snapshot_limits.max_entries > 65_536
            || self.snapshot_limits.max_file_bytes == 0
            || self.snapshot_limits.max_file_bytes > 536_870_912
            || self.snapshot_limits.max_total_bytes == 0
            || self.snapshot_limits.max_total_bytes > 1_073_741_824
            || self.snapshot_limits.max_path_bytes == 0
            || self.snapshot_limits.max_path_bytes > 4096
        {
            return Err("invalid explicit Radicle patch profile or finite bounds".into());
        }
        kiln_adapter_radicle::job::admit_log_url(&self.log_url_prefix, MAX_LOG_URL_PREFIX_BYTES)
            .map_err(|error| format!("{error:?}"))?;
        self.helper.admit()?;
        self.hosted.executable.admit()?;
        let client: Value = load_json(&self.hosted.client_profile, MAX_CONFIG_BYTES)?;
        let bytes = std::fs::read(&self.hosted.client_profile).map_err(text)?;
        if blake3::hash(&bytes).to_hex().as_str() != self.hosted.client_profile_identity.as_str()
            || client.get("organization").and_then(Value::as_str)
                != Some(self.hosted.organization.as_str())
        {
            return Err(
                "hosted authenticated client profile identity/organization mismatch".into(),
            );
        }
        let _admitted_jobs = self.job_shell()?;
        if !self.job_profile.allowed_rids.contains(&self.repository_id) {
            return Err("job profile does not allow patch repository".into());
        }
        for root in [&self.state_root, &self.hosted.workspace_root] {
            let meta = std::fs::symlink_metadata(root).map_err(text)?;
            use std::os::unix::fs::PermissionsExt as _;
            if !meta.is_dir()
                || meta.permissions().mode() & 0o077 != 0
                || std::fs::canonicalize(root).map_err(text)? != *root
            {
                return Err(
                    "patch state/workspace roots must be existing canonical private directories"
                        .into(),
                );
            }
        }
        Ok(())
    }
    fn native(&self, patch: &str) -> Result<NativePatch> {
        self.helper.admit()?;
        let value = kiln_adapter_radicle::patch_read::read_patch(
            &self.helper.path,
            &self.storage_root,
            &self.repository_id,
            patch,
        )?;
        if serde_json::to_vec(&value).map_err(text)?.len() > MAX_PROJECTION_BYTES {
            return Err("native projection bound".into());
        }
        let native: NativePatch = serde_json::from_value(value).map_err(text)?;
        if native.patch_id != patch || native.heads.is_empty() {
            return Err("native patch identity/head mismatch".into());
        }
        Ok(native)
    }
    fn binding(&self, trigger: &TriggerRequest) -> Result<(RevisionBinding, BTreeSet<String>)> {
        if trigger.event != TriggerEventKind::Patch
            || trigger.repository.provider_namespace.as_str() != "radicle"
            || trigger.repository.native_id.as_str() != self.repository_id
            || trigger.after.provider_namespace.as_str() != "radicle"
            || trigger.after.native_kind.as_str() != "revision"
        {
            return Err("patch profile requires its exact repository patch event".into());
        }
        let patch = trigger
            .patch
            .as_ref()
            .ok_or("patch event has no patch id")?;
        if patch.provider_namespace.as_str() != "radicle"
            || patch.native_kind.as_str() != "patch"
            || !oid(patch.native_id.as_str())
        {
            return Err("invalid native patch identity".into());
        }
        let native = self.native(patch.native_id.as_str())?;
        let revision = native
            .revisions
            .iter()
            .rev()
            .find(|r| r.head == trigger.after.native_id.as_str())
            .ok_or("event oid is absent from reduced patch revisions")?;
        if !oid(&revision.id)
            || !oid(&revision.base)
            || !oid(&revision.head)
            || revision.signed_source.is_none()
        {
            return Err(
                "revision lacks exact base/head or verified author signed-ref reachability".into(),
            );
        }
        Ok((
            RevisionBinding {
                repository: self.repository_id.clone(),
                patch: native.patch_id,
                revision: revision.id.clone(),
                base: revision.base.clone(),
                oid: revision.head.clone(),
                author: revision.author.clone(),
            },
            native.heads,
        ))
    }
    fn root(&self, run_id: &str) -> Result<PathBuf> {
        if !label(run_id) {
            return Err("invalid patch run id".into());
        }
        Ok(self.state_root.join(run_id))
    }
    fn validate(&self, journal: &PatchJournal) -> Result<()> {
        let key = identity(&(PATCH_JOURNAL_SCHEMA, identity(self)?, &journal.binding))?;
        if journal.schema != PATCH_JOURNAL_SCHEMA
            || journal.profile_identity != identity(self)?
            || journal.run_id != format!("radicle-patch-{}", key.as_str())
            || journal.binding.repository != self.repository_id
            || journal.trusted != self.trusted_authors.contains(&journal.binding.author)
            || journal.trigger != exact_trigger(&journal.binding)?
        {
            return Err("patch journal profile/source/run substitution".into());
        }
        if let Some(approval) = &journal.approval {
            if approval.source_commit != journal.binding.oid
                || !self.approvers.contains(&approval.actor_id)
            {
                return Err("patch approval substitution".into());
            }
        }
        if !journal.trusted && journal.approval.is_none() && journal.state != PatchState::Blocked {
            return Err("untrusted journal bypasses approval".into());
        }
        if let Some(request) = &journal.request {
            if *request != execution_input(self, journal)?
                || journal
                    .source_tree
                    .as_ref()
                    .is_none_or(|tree| ContentId::from_blake3_hex(tree).is_err())
            {
                return Err("patch execution request or source snapshot substitution".into());
            }
        }
        if let Some(JobDispatch::Written(job)) = &journal.job {
            if job.revision.native_id.as_str() != journal.binding.oid
                || job.run_id != journal.run_id
            {
                return Err("patch job result names another exact revision".into());
            }
        }
        Ok(())
    }
    pub fn intake(&self, trigger: &TriggerRequest) -> Result<PatchJournal> {
        self.admit()?;
        let (binding, heads) = self.binding(trigger)?;
        let key = identity(&(PATCH_JOURNAL_SCHEMA, identity(self)?, &binding))?;
        let run_id = format!("radicle-patch-{}", key.as_str());
        let root = self.root(&run_id)?;
        storage::prepare_candidate(&root)?;
        let path = root.join("patch-journal.json");
        let lock = JournalLock::acquire(&path)?;
        if path.exists() {
            let journal: PatchJournal = load_json(&path, MAX_CONFIG_BYTES)?;
            self.validate(&journal)?;
            if journal.binding != binding {
                return Err("patch run reserves another exact revision".into());
            }
            return Ok(journal);
        }
        let trusted = self.trusted_authors.contains(&binding.author);
        let journal = PatchJournal {
            schema: PATCH_JOURNAL_SCHEMA.into(),
            profile_identity: identity(self)?,
            trigger: exact_trigger(&binding)?,
            binding,
            run_id,
            observed_heads: heads,
            trusted,
            approval: None,
            state: if trusted {
                PatchState::Ready
            } else {
                PatchState::Blocked
            },
            source_tree: None,
            request: None,
            observation: None,
            transport_error: None,
            observation_sequence: 0,
            job: None,
        };
        lock.store(&journal, MAX_CONFIG_BYTES)?;
        Ok(journal)
    }
    /// This is an operator-only local CLI, never a producer-owned broker action.
    pub fn approve(&self, command: &ApproveUntrustedRun, now_ms: u64) -> Result<PatchJournal> {
        self.admit()?;
        if command.actor.authority != APPROVER_AUTHORITY
            || !self.approvers.contains(&command.actor.id)
        {
            return Err("interactive patch approver not authorized".into());
        }
        let path = self.root(&command.run)?.join("patch-journal.json");
        let lock = JournalLock::acquire(&path)?;
        let mut journal: PatchJournal = load_json(&path, MAX_CONFIG_BYTES)?;
        self.validate(&journal)?;
        if journal.state != PatchState::Blocked
            || command.source_commit != journal.binding.oid
            || command.actor.label.is_empty()
            || command.actor.label.len() > 256
        {
            return Err("approval run/commit/state mismatch".into());
        }
        let current = self.native(&journal.binding.patch)?;
        if current.revisions.last().map(|r| r.id.as_str())
            != Some(journal.binding.revision.as_str())
        {
            return Err("new revision needs a new approval".into());
        }
        journal.approval = Some(UntrustedApproval {
            actor_id: command.actor.id.clone(),
            actor_label: command.actor.label.clone(),
            source_commit: command.source_commit.clone(),
            approved_at_ms: now_ms,
        });
        journal.state = PatchState::Ready;
        lock.store(&journal, MAX_CONFIG_BYTES)?;
        Ok(journal)
    }
    pub fn load(&self, run_id: &str) -> Result<PatchJournal> {
        self.admit()?;
        let path = self.root(run_id)?.join("patch-journal.json");
        let _lock = JournalLock::acquire(&path)?;
        let journal = load_json(&path, MAX_CONFIG_BYTES)?;
        self.validate(&journal)?;
        Ok(journal)
    }
    /// Submit once. Ambiguous delivery is recovered with execution.get for the
    /// original attempt; never submit a new attempt or discard provenance.
    pub fn drive(&self, run_id: &str) -> Result<PatchJournal> {
        self.admit()?;
        let root = self.root(run_id)?;
        let path = root.join("patch-journal.json");
        let lock = JournalLock::acquire(&path)?;
        let mut journal: PatchJournal = load_json(&path, MAX_CONFIG_BYTES)?;
        self.validate(&journal)?;
        if journal.state == PatchState::Blocked {
            return Ok(journal);
        }
        if matches!(journal.state, PatchState::Succeeded | PatchState::Failed) {
            self.publish_job(&mut journal)?;
            lock.store(&journal, MAX_CONFIG_BYTES)?;
            return Ok(journal);
        }
        let (action, input) = if journal.request.is_none() {
            let current = self.native(&journal.binding.patch)?;
            let (binding, _) = self.binding(&journal.trigger)?;
            if binding != journal.binding
                || current.revisions.last().map(|r| r.id.as_str())
                    != Some(journal.binding.revision.as_str())
            {
                return Err("patch revision moved or was superseded before dispatch".into());
            }
            let workspace = self.hosted.workspace_root.join(&journal.run_id);
            journal.source_tree = Some(
                kiln_adapter_radicle::source::materialize_patch_commit(
                    &self
                        .storage_root
                        .join(self.repository_id.trim_start_matches("rad:")),
                    &journal.binding.oid,
                    &self.snapshot_limits,
                    &workspace,
                )
                .map_err(|e| format!("{e:?}"))?,
            );
            let after = self.native(&journal.binding.patch)?;
            if after.heads != current.heads {
                return Err("patch heads changed during exact source acquisition".into());
            }
            if after.revisions.last().is_none_or(|revision| {
                revision.id != journal.binding.revision
                    || revision.base != journal.binding.base
                    || revision.head != journal.binding.oid
                    || revision.author != journal.binding.author
                    || revision.signed_source.is_none()
            }) {
                return Err(
                    "revision signed-author source changed during exact source acquisition".into(),
                );
            }
            let input = execution_input(self, &journal)?;
            journal.request = Some(input.clone());
            journal.state = PatchState::Submitted;
            // Reservation reaches durable storage before the authenticated RPC.
            lock.store(&journal, MAX_CONFIG_BYTES)?;
            ("execution.run", input)
        } else {
            (
                "execution.get",
                json!({"project": self.hosted.project, "attempt_id": journal.run_id}),
            )
        };
        const MAX_OBSERVATIONS: u32 = 1024;
        if journal.observation_sequence >= MAX_OBSERVATIONS {
            return Err("patch recovery observation bound reached".into());
        }
        journal.observation_sequence += 1;
        lock.store(&journal, MAX_CONFIG_BYTES)?;
        let request = json!({"schema":"kiln.hosted.v1", "organization":self.hosted.organization,
            "request_id": format!("{}-{}-{}", journal.run_id, if action == "execution.run" { "submit" } else { "observe" }, journal.observation_sequence),
            "action":action, "input":input});
        let request_file = root.join(format!("hosted-{}.json", journal.observation_sequence));
        publish_new(&request_file, &request)?;
        match self.rpc(&request_file, &request) {
            Ok(result) => {
                apply_observation(&mut journal, result)?;
                journal.transport_error = None;
            }
            Err(error) => {
                journal.state = PatchState::Unknown;
                journal.transport_error = Some(error);
            }
        }
        lock.store(&journal, MAX_CONFIG_BYTES)?;
        if matches!(journal.state, PatchState::Succeeded | PatchState::Failed) {
            self.publish_job(&mut journal)?;
            lock.store(&journal, MAX_CONFIG_BYTES)?;
        }
        Ok(journal)
    }
    fn rpc(&self, file: &Path, request: &Value) -> Result<Value> {
        self.hosted.executable.admit()?;
        let output = run(RunRequest {
            command: CommandSpec {
                program: self.hosted.executable.path.clone(),
                args: vec![
                    "rpc".into(),
                    "--profile".into(),
                    self.hosted.client_profile.as_os_str().into(),
                    "--request".into(),
                    file.as_os_str().into(),
                ],
                current_dir: PathBuf::from("/"),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms: RPC_TIMEOUT_MS,
                stdin_max_bytes: 1,
                stdout_max_bytes: MAX_PROJECTION_BYTES,
                stderr_max_bytes: 4096,
                poll_interval_ms: 20,
                teardown_timeout_ms: 2000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true)
                .map_err(|e| format!("{e:?}"))?,
        })
        .map_err(text)?;
        if output.disposition != Disposition::Succeeded {
            return Err("authenticated hosted RPC uncertain; reconcile original attempt".into());
        }
        let response: Value = serde_json::from_slice(&output.stdout.bytes).map_err(text)?;
        if response.get("schema").and_then(Value::as_str) != Some("kiln.hosted.v1")
            || response.get("request_id") != request.get("request_id")
            || response.get("error").is_some_and(|error| !error.is_null())
        {
            return Err("hosted RPC response identity or admission failure; no safe retry".into());
        }
        response
            .get("result")
            .filter(|v| !v.is_null())
            .cloned()
            .ok_or("hosted result absent".into())
    }
    fn publish_job(&self, journal: &mut PatchJournal) -> Result<()> {
        if matches!(journal.job, Some(JobDispatch::Written(_))) {
            return Ok(());
        }
        let plan = JobPlan {
            repository: ExternalRef::new("radicle", "repository", &journal.binding.repository)
                .map_err(|e| format!("{e:?}"))?,
            revision: ExternalRef::new("radicle", "revision", &journal.binding.oid)
                .map_err(|e| format!("{e:?}"))?,
            object_id: None,
            run_id: RunId::new(&journal.run_id).map_err(|e| format!("{e:?}"))?,
            log_url: format!("{}{}", self.log_url_prefix, journal.run_id),
            state: JobState::Finished {
                succeeded: journal.state == PatchState::Succeeded,
            },
            max_payload_bytes: self.job_profile.max_payload_bytes,
            max_log_url_bytes: kiln_adapter_radicle::job::DEFAULT_MAX_LOG_URL_BYTES,
        };
        let shell = self.job_shell()?;
        journal.job = Some(
            shell
                .publish(&plan, "publish exact Radicle patch revision CI result")
                .map_err(|e| format!("{e:?}"))?,
        );
        Ok(())
    }
}

pub fn exact_trigger(binding: &RevisionBinding) -> Result<TriggerRequest> {
    let external =
        |kind: &str, id: &str| ExternalRef::new("radicle", kind, id).map_err(|e| format!("{e:?}"));
    Ok(TriggerRequest {
        repository: external("repository", &binding.repository)?,
        actor: external("node", &binding.author)?,
        before: Some(external("revision", &binding.base)?),
        after: external("revision", &binding.oid)?,
        branch: None,
        patch: Some(external("patch", &binding.patch)?),
        event: TriggerEventKind::Patch,
    })
}
// r[impl kiln.radicle.revision_ci]
fn execution_input(profile: &PatchProfile, journal: &PatchJournal) -> Result<Value> {
    let provenance = if journal.trusted {
        None
    } else {
        let approval = journal
            .approval
            .as_ref()
            .filter(|a| a.source_commit == journal.binding.oid)
            .ok_or("untrusted revision has no exact approval")?;
        Some(UntrustedProvenance {
            reason: UntrustedReason::AuthorWithoutPushAccess,
            source_commit: journal.binding.oid.clone(),
            approver_id: approval.actor_id.clone(),
        })
    };
    let mut input = json!({"project":profile.hosted.project, "pipeline_id":profile.hosted.pipeline_id,
        "attempt_id":journal.run_id, "price_id":profile.hosted.price_id, "class":profile.hosted.class,
        "workspace":journal.run_id, "trust":if journal.trusted { "trusted" } else { "hostile" }});
    if let Some(provenance) = provenance {
        input
            .as_object_mut()
            .ok_or("hosted execution input must be an object")?
            .insert(
                "untrusted".into(),
                serde_json::to_value(provenance).map_err(text)?,
            );
    }
    Ok(input)
}
fn apply_observation(journal: &mut PatchJournal, result: Value) -> Result<()> {
    if result.get("attempt_id").and_then(Value::as_str) != Some(journal.run_id.as_str()) {
        return Err("hosted observation names another attempt".into());
    }
    journal.state = match result.get("state").and_then(Value::as_str) {
        Some("succeeded") => PatchState::Succeeded,
        Some("failed" | "cancelled" | "timed_out") => PatchState::Failed,
        Some("running" | "reserved") => PatchState::Running,
        _ => PatchState::Unknown,
    };
    journal.observation = Some(result);
    Ok(())
}

#[cfg(test)]
#[path = "patch_tests.rs"]
mod tests;
