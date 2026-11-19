//! Pure core for the pinned Lattice worker guest (Kiln #51, milestone M1).
//!
//! The immutable guest environment, the outer image manifest, the per-attempt
//! request and the bounded host/guest dispatch protocol are admitted here
//! without I/O, clocks or processes. Host and guest shells supply the observed
//! facts. Admission grants no placement, retry, fencing or landing authority.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use kiln_core::types::{ContentId, Diagnostic};

/// Immutable environment record baked into the initramfs.
pub const ENVIRONMENT_SCHEMA: &str = "kiln.lattice-worker-environment.v1";
/// Outer image manifest published beside the kernel, initramfs and VMM.
pub const MANIFEST_SCHEMA: &str = "kiln.lattice-worker-guest.v1";
/// One admitted attempt request.
pub const ATTEMPT_SCHEMA: &str = "kiln.lattice-worker-attempt.v1";
/// Host/guest dispatch frames.
pub const PROTOCOL_SCHEMA: &str = "kiln.lattice-worker.guest.v1";
/// Label written by the guest into a freshly formatted workspace.
pub const WORKSPACE_LABEL_SCHEMA: &str = "kiln.lattice-worker-workspace.v1";
/// Lattice's standalone effect-authority schema at the pinned revision.
pub const AUTHORITY_SCHEMA: &str = "lattice.standalone-effect-authority.v1";
/// Guest-initiated vsock port, distinct from the per-job agent port so a
/// per-job host never exchanges frames with a worker guest.
pub const LATTICE_WORKER_PORT: u32 = 10_241;
/// The only admitted architecture of the retained CH profile.
pub const ARCHITECTURE: &str = "x86_64";
pub const VMM_KIND: &str = "cloud-hypervisor";
pub const WORKSPACE_DISK: &str = "virtio-blk-raw";
pub const NETWORK_NONE: &str = "none";
pub const MAX_WORKFLOWS: usize = 64;
pub const MAX_GRANTS: usize = 1_024;
pub const MAX_NAME_BYTES: usize = 128;
pub const MAX_TEXT_BYTES: usize = 256;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
/// Bounded stderr tail returned in a report.
pub const MAX_REPORT_TAIL_BYTES: usize = 4_096;
/// Prefix of the workspace image probed for prior use. It covers the ext2/3/4
/// superblock at 1024 bytes and every common partition-table location.
pub const WORKSPACE_PROBE_BYTES: usize = 65_536;
/// The initramfs carries the Lattice closure; smaller guests cannot unpack it.
pub const MIN_GUEST_MEMORY_MIB: u32 = 512;
pub const MIN_WORKSPACE_MIB: u32 = 16;
pub const MAX_WORKSPACE_MIB: u32 = 65_536;
/// machine-id(5) is 128 bits rendered as 32 lowercase hex characters.
pub const GUEST_IDENTITY_HEX: usize = 32;
pub const ENTROPY_BYTES: usize = 32;
const GUEST_IDENTITY_CONTEXT: &str = "kiln lattice worker guest identity v1";
const WORKSPACE_IDENTITY_CONTEXT: &str = "kiln lattice worker workspace identity v1";
const GIT_REVISION_HEX: usize = 40;

fn refused(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

/// Text that is safe both as JSON and as a plain Nickel string literal.
fn plain_text(value: &str, field: &str) -> Result<(), Diagnostic> {
    let admitted = !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'/' | b'.' | b'_' | b'-'));
    if admitted {
        Ok(())
    } else {
        Err(refused(
            "lattice_worker_text",
            field,
            "value must be bounded plain ASCII without quoting or interpolation",
        ))
    }
}

fn workflow_name(value: &str) -> Result<(), Diagnostic> {
    let admitted = !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if admitted {
        Ok(())
    } else {
        Err(refused(
            "lattice_worker_workflow_name",
            "workflow",
            "workflow name must be lowercase ASCII, digits and hyphens",
        ))
    }
}

fn content(value: &ContentId, field: &str) -> Result<(), Diagnostic> {
    value
        .validate()
        .map_err(|_| refused("lattice_worker_identity", field, "invalid BLAKE3 identity"))
}

fn identity_of<T: Serialize>(value: &T, field: &str) -> Result<ContentId, Diagnostic> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| refused("lattice_worker_encoding", field, "record is not encodable"))?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

/// One concrete grant reported by `lattice authority inspect`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub node_id: u64,
    pub operation: String,
    pub resource: String,
    pub ability: String,
}

/// Exact effect requirements of one workflow under the pinned Lattice binary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRequirements {
    pub schema: String,
    pub workflow_revision: String,
    pub machine_identity: String,
    pub policy_ref: String,
    pub grants: Vec<Grant>,
}

impl EffectRequirements {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if self.schema != AUTHORITY_SCHEMA {
            return Err(refused(
                "lattice_worker_authority_schema",
                "requirements.schema",
                "unsupported Lattice effect-authority schema",
            ));
        }
        plain_text(&self.workflow_revision, "requirements.workflow_revision")?;
        plain_text(&self.machine_identity, "requirements.machine_identity")?;
        plain_text(&self.policy_ref, "requirements.policy_ref")?;
        if self.grants.is_empty() || self.grants.len() > MAX_GRANTS {
            return Err(refused(
                "lattice_worker_grants",
                "requirements.grants",
                "grant count outside bound",
            ));
        }
        for grant in &self.grants {
            plain_text(&grant.operation, "requirements.grants.operation")?;
            plain_text(&grant.resource, "requirements.grants.resource")?;
            plain_text(&grant.ability, "requirements.grants.ability")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedWorkflow {
    /// BLAKE3 of the exact Nickel definition bytes baked into the image.
    pub definition: ContentId,
    pub requirements: EffectRequirements,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatticeBuild {
    /// Exact Lattice Git revision of the pinned cohort.
    pub revision: String,
    /// BLAKE3 of the `lattice` executable in the image.
    pub binary: ContentId,
}

/// Immutable environment baked into the initramfs. Mutable workspace, Lattice
/// state, guest identity and effect authority are never part of it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestEnvironment {
    pub schema: String,
    pub architecture: String,
    pub lattice: LatticeBuild,
    pub guest_shell: ContentId,
    pub busybox: ContentId,
    pub workflows: BTreeMap<String, AdmittedWorkflow>,
}

impl GuestEnvironment {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if self.schema != ENVIRONMENT_SCHEMA {
            return Err(refused(
                "lattice_worker_environment_schema",
                "environment.schema",
                "unsupported Lattice worker environment schema",
            ));
        }
        if self.architecture != ARCHITECTURE {
            return Err(refused(
                "lattice_worker_architecture",
                "environment.architecture",
                "only the x86_64 retained CH profile is admitted",
            ));
        }
        if self.lattice.revision.len() != GIT_REVISION_HEX
            || !self
                .lattice
                .revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(refused(
                "lattice_worker_revision",
                "environment.lattice.revision",
                "Lattice revision must be an exact Git commit",
            ));
        }
        content(&self.lattice.binary, "environment.lattice.binary")?;
        content(&self.guest_shell, "environment.guest_shell")?;
        content(&self.busybox, "environment.busybox")?;
        if self.workflows.is_empty() || self.workflows.len() > MAX_WORKFLOWS {
            return Err(refused(
                "lattice_worker_workflows",
                "environment.workflows",
                "admitted workflow count outside bound",
            ));
        }
        for (name, workflow) in &self.workflows {
            workflow_name(name)?;
            content(&workflow.definition, "environment.workflows.definition")?;
            workflow.requirements.validate()?;
        }
        Ok(())
    }

    /// BLAKE3 over the canonical encoding of a valid environment.
    pub fn identity(&self) -> Result<ContentId, Diagnostic> {
        self.validate()?;
        identity_of(self, "environment")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreObject {
    pub path: String,
    pub blake3: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vmm {
    pub kind: String,
    /// Exact trimmed `--version` output observed at image build time.
    pub version: String,
    pub binary: StoreObject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfile {
    pub workspace_disk: String,
    pub vsock_port: u32,
    pub network: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestResources {
    pub vcpus: u8,
    pub memory_mib: u32,
    pub workspace_mib: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestManifest {
    pub schema: String,
    pub architecture: String,
    pub kernel: StoreObject,
    pub initrd: StoreObject,
    pub vmm: Vmm,
    pub environment: GuestEnvironment,
    pub environment_identity: ContentId,
    pub devices: DeviceProfile,
    pub resources: GuestResources,
}

fn store_path(object: &StoreObject, field: &str) -> Result<(), Diagnostic> {
    content(&object.blake3, field)?;
    let admitted = object.path.starts_with("/nix/store/")
        && object.path.len() <= MAX_TEXT_BYTES
        && !object.path.contains("/../")
        && !object.path.contains("/./")
        && !object.path.contains('\0');
    if admitted {
        Ok(())
    } else {
        Err(refused(
            "lattice_worker_store_object",
            field,
            "guest object must be an exact Nix store path",
        ))
    }
}

impl GuestManifest {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if self.schema != MANIFEST_SCHEMA {
            return Err(refused(
                "lattice_worker_manifest_schema",
                "manifest.schema",
                "unsupported Lattice worker guest manifest schema",
            ));
        }
        if self.architecture != ARCHITECTURE || self.environment.architecture != ARCHITECTURE {
            return Err(refused(
                "lattice_worker_architecture",
                "manifest.architecture",
                "only the x86_64 retained CH profile is admitted",
            ));
        }
        store_path(&self.kernel, "manifest.kernel")?;
        store_path(&self.initrd, "manifest.initrd")?;
        store_path(&self.vmm.binary, "manifest.vmm.binary")?;
        if self.vmm.kind != VMM_KIND
            || self.vmm.version.is_empty()
            || self.vmm.version.len() > MAX_TEXT_BYTES
            || self.vmm.version.contains('\0')
        {
            return Err(refused(
                "lattice_worker_vmm",
                "manifest.vmm",
                "manifest must name the exact Cloud Hypervisor build",
            ));
        }
        if self.environment.identity()? != self.environment_identity {
            return Err(refused(
                "lattice_worker_environment_identity",
                "manifest.environment_identity",
                "embedded environment does not hash to the recorded identity",
            ));
        }
        if self.devices.workspace_disk != WORKSPACE_DISK
            || self.devices.vsock_port != LATTICE_WORKER_PORT
            || self.devices.network != NETWORK_NONE
        {
            return Err(refused(
                "lattice_worker_devices",
                "manifest.devices",
                "only a raw workspace disk, the worker vsock port and no NIC are admitted",
            ));
        }
        let resources = self.resources;
        if resources.vcpus == 0
            || resources.memory_mib < MIN_GUEST_MEMORY_MIB
            || !(MIN_WORKSPACE_MIB..=MAX_WORKSPACE_MIB).contains(&resources.workspace_mib)
        {
            return Err(refused(
                "lattice_worker_resources",
                "manifest.resources",
                "guest resources outside the admitted bounds",
            ));
        }
        Ok(())
    }

    /// Image identity: BLAKE3 over the canonical encoding of a valid manifest.
    // r[impl kiln.lattice.worker.guest.scenario.identity]
    pub fn identity(&self) -> Result<ContentId, Diagnostic> {
        self.validate()?;
        identity_of(self, "manifest")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBinding {
    pub identity: ContentId,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptRequest {
    pub schema: String,
    pub attempt: ContentId,
    pub execution: ContentId,
    pub image: ContentId,
    pub environment: ContentId,
    pub workflow: String,
    pub definition: ContentId,
    pub workflow_revision: String,
    pub workspace: WorkspaceBinding,
    pub guest_identity: String,
    /// Original admitted absolute wall deadline; never extended by a move.
    pub deadline_unix_ms: u64,
}

fn derived(context: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key(context);
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

/// Per-attempt workspace identity. A workspace of one attempt can never be
/// named by another attempt or execution.
pub fn workspace_identity(
    attempt: &ContentId,
    execution: &ContentId,
    generation: u64,
) -> Result<ContentId, Diagnostic> {
    let bytes = derived(
        WORKSPACE_IDENTITY_CONTEXT,
        &[
            attempt.as_str().as_bytes(),
            execution.as_str().as_bytes(),
            &generation.to_be_bytes(),
        ],
    );
    ContentId::from_blake3_hex(blake3::Hash::from(bytes).to_hex().to_string())
}

/// Per-attempt guest machine identity; never present in the reusable image.
// r[impl kiln.lattice.worker.guest.scenario.isolated]
#[must_use]
pub fn guest_identity(attempt: &ContentId, execution: &ContentId, workspace: &ContentId) -> String {
    let bytes = derived(
        GUEST_IDENTITY_CONTEXT,
        &[
            attempt.as_str().as_bytes(),
            execution.as_str().as_bytes(),
            workspace.as_str().as_bytes(),
        ],
    );
    let mut text = blake3::Hash::from(bytes).to_hex().to_string();
    text.truncate(GUEST_IDENTITY_HEX);
    text
}

/// Build the exact request for one admitted workflow of a verified manifest.
pub fn request(
    manifest: &GuestManifest,
    attempt: ContentId,
    execution: ContentId,
    workflow: &str,
    generation: u64,
    deadline_unix_ms: u64,
) -> Result<AttemptRequest, Diagnostic> {
    let admitted = manifest
        .environment
        .workflows
        .get(workflow)
        .ok_or_else(|| {
            refused(
                "lattice_worker_workflow_unknown",
                "workflow",
                "workflow is not admitted in this image",
            )
        })?;
    let workspace = workspace_identity(&attempt, &execution, generation)?;
    let guest = guest_identity(&attempt, &execution, &workspace);
    Ok(AttemptRequest {
        schema: ATTEMPT_SCHEMA.into(),
        image: manifest.identity()?,
        environment: manifest.environment_identity.clone(),
        workflow: workflow.into(),
        definition: admitted.definition.clone(),
        workflow_revision: admitted.requirements.workflow_revision.clone(),
        workspace: WorkspaceBinding {
            identity: workspace,
            generation,
        },
        guest_identity: guest,
        attempt,
        execution,
        deadline_unix_ms,
    })
}

/// Checks shared by the host and guest: everything except the outer image.
fn admit_against_environment<'a>(
    environment: &'a GuestEnvironment,
    environment_identity: &ContentId,
    request: &AttemptRequest,
    now_unix_ms: u64,
) -> Result<&'a AdmittedWorkflow, Diagnostic> {
    if request.schema != ATTEMPT_SCHEMA {
        return Err(refused(
            "lattice_worker_attempt_schema",
            "request.schema",
            "unsupported Lattice worker attempt schema",
        ));
    }
    content(&request.attempt, "request.attempt")?;
    content(&request.execution, "request.execution")?;
    content(&request.image, "request.image")?;
    content(&request.environment, "request.environment")?;
    content(&request.definition, "request.definition")?;
    content(&request.workspace.identity, "request.workspace")?;
    if request.environment != *environment_identity {
        return Err(refused(
            "lattice_worker_environment_mismatch",
            "request.environment",
            "attempt names a different guest environment",
        ));
    }
    workflow_name(&request.workflow)?;
    let workflow = environment
        .workflows
        .get(&request.workflow)
        .ok_or_else(|| {
            refused(
                "lattice_worker_workflow_unknown",
                "request.workflow",
                "workflow is not admitted in this environment",
            )
        })?;
    if workflow.definition != request.definition
        || workflow.requirements.workflow_revision != request.workflow_revision
    {
        return Err(refused(
            "lattice_worker_workflow_mismatch",
            "request.definition",
            "attempt names a different workflow definition or revision",
        ));
    }
    if request.workspace.generation == 0
        || workspace_identity(
            &request.attempt,
            &request.execution,
            request.workspace.generation,
        )? != request.workspace.identity
    {
        return Err(refused(
            "lattice_worker_workspace_mismatch",
            "request.workspace",
            "workspace identity is not this attempt's workspace generation",
        ));
    }
    if request.guest_identity
        != guest_identity(
            &request.attempt,
            &request.execution,
            &request.workspace.identity,
        )
    {
        return Err(refused(
            "lattice_worker_guest_identity",
            "request.guest_identity",
            "guest identity is not derived from this attempt",
        ));
    }
    if request.deadline_unix_ms <= now_unix_ms {
        return Err(refused(
            "lattice_worker_deadline",
            "request.deadline_unix_ms",
            "the original attempt deadline is exhausted",
        ));
    }
    Ok(workflow)
}

/// Host admission before any VM, workspace or dispatch effect.
// r[impl kiln.lattice.worker.guest.scenario.refused]
pub fn admit_attempt<'a>(
    manifest: &'a GuestManifest,
    request: &AttemptRequest,
    now_unix_ms: u64,
) -> Result<&'a AdmittedWorkflow, Diagnostic> {
    if manifest.identity()? != request.image {
        return Err(refused(
            "lattice_worker_image_mismatch",
            "request.image",
            "attempt names a different guest image",
        ));
    }
    admit_against_environment(
        &manifest.environment,
        &manifest.environment_identity,
        request,
        now_unix_ms,
    )
}

/// Guest admission of a received dispatch against its baked environment.
// r[impl kiln.lattice.worker.guest.scenario.refused]
pub fn admit_dispatch<'a>(
    environment: &'a GuestEnvironment,
    request: &AttemptRequest,
    now_unix_ms: u64,
) -> Result<&'a AdmittedWorkflow, Diagnostic> {
    let identity = environment.identity()?;
    admit_against_environment(environment, &identity, request, now_unix_ms)
}

/// Render the per-attempt standalone effect-authority profile. Values were
/// validated as plain text, and the expiry is the original attempt deadline.
pub fn authority_profile(
    requirements: &EffectRequirements,
    deadline_unix_ms: u64,
) -> Result<String, Diagnostic> {
    requirements.validate()?;
    let grants = requirements
        .grants
        .iter()
        .map(|grant| {
            format!(
                "    {{ node_id = {}, operation = \"{}\", resource = \"{}\", ability = \"{}\" }},\n",
                grant.node_id, grant.operation, grant.resource, grant.ability
            )
        })
        .collect::<String>();
    Ok(format!(
        "{{\n  schema = \"{}\",\n  workflow_revision = \"{}\",\n  machine_identity = \"{}\",\n  policy_ref = \"{}\",\n  grants = [\n{grants}  ],\n  expires_at_unix_ms = {deadline_unix_ms},\n  roots = {{}},\n}}\n",
        requirements.schema,
        requirements.workflow_revision,
        requirements.machine_identity,
        requirements.policy_ref,
    ))
}

/// Render the per-attempt Lattice application config.
pub fn lattice_config(
    data_dir: &str,
    authority: &str,
    timeout_secs: u64,
) -> Result<String, Diagnostic> {
    plain_text(data_dir, "config.data_dir")?;
    plain_text(authority, "config.effect_authority")?;
    if timeout_secs == 0 {
        return Err(refused(
            "lattice_worker_timeout",
            "config.default_timeout_secs",
            "timeout must be positive",
        ));
    }
    Ok(format!(
        "{{\n  data_dir = \"{data_dir}\",\n  effect_authority = \"{authority}\",\n  default_timeout_secs = {timeout_secs},\n  shell = \"sh\",\n}}\n"
    ))
}

/// A workspace image may be formatted only if its probed prefix was never
/// written. Any byte there means another attempt or tool used it.
#[must_use]
pub fn workspace_blank(prefix: &[u8]) -> bool {
    prefix.len() == WORKSPACE_PROBE_BYTES && prefix.iter().all(|byte| *byte == 0)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLabel {
    pub schema: String,
    pub attempt: ContentId,
    pub workspace: WorkspaceBinding,
    pub guest_identity: String,
}

#[must_use]
pub fn workspace_label(request: &AttemptRequest) -> WorkspaceLabel {
    WorkspaceLabel {
        schema: WORKSPACE_LABEL_SCHEMA.into(),
        attempt: request.attempt.clone(),
        workspace: request.workspace.clone(),
        guest_identity: request.guest_identity.clone(),
    }
}

/// Exact `lattice run --output json` summary at the pinned revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub run_id: u64,
    pub workflow_name: String,
    pub status: String,
    pub node_count: u64,
    pub steps_run: u64,
    pub completed: u64,
    pub failures: u64,
    pub cache_hits: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReportOutcome {
    /// Guest admission refused the dispatch; nothing ran.
    Refused { code: String, field: String },
    /// Preparation failed before Lattice ran the workflow.
    PreparationFailed { stage: String, detail: String },
    /// The Lattice executor ran; `summary` is its own observation.
    Ran {
        exit_code: Option<i32>,
        summary: Option<RunSummary>,
        stderr_tail: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub attempt: ContentId,
    pub guest_identity: String,
    pub outcome: ReportOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    /// The single attempt for this guest incarnation.
    Dispatch {
        request: Box<AttemptRequest>,
        clock_unix_ms: u64,
        entropy_hex: String,
    },
    Ping {
        nonce: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GuestMessage {
    /// Sent on every connection, including after snapshot restore.
    Hello {
        environment: ContentId,
        attempt: Option<ContentId>,
    },
    Report(Report),
    Pong {
        nonce: String,
        attempt: Option<ContentId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame<T> {
    pub schema: String,
    pub message: T,
}

/// Length-prefixed frame, refused above the bound.
pub fn encode<T: Serialize>(message: T) -> Result<Vec<u8>, String> {
    let frame = Frame {
        schema: PROTOCOL_SCHEMA.to_string(),
        message,
    };
    let body = serde_json::to_vec(&frame).map_err(|e| e.to_string())?;
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err("Lattice worker frame exceeds bound".into());
    }
    let length = u32::try_from(body.len()).map_err(|_| "Lattice worker frame length")?;
    let mut bytes = length.to_be_bytes().to_vec();
    bytes.extend(body);
    Ok(bytes)
}

pub fn frame_length(prefix: [u8; 4]) -> Result<usize, String> {
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| "frame length")?;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err("Lattice worker frame length outside bound".into());
    }
    Ok(length)
}

pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err("Lattice worker frame body outside bound".into());
    }
    let frame: Frame<T> =
        serde_json::from_slice(body).map_err(|_| "malformed Lattice worker frame")?;
    if frame.schema != PROTOCOL_SCHEMA {
        return Err("Lattice worker protocol schema mismatch".into());
    }
    Ok(frame.message)
}

/// Bound a diagnostic tail to whole UTF-8 characters without control bytes.
#[must_use]
pub fn bounded_tail(text: &str) -> String {
    let mut start = text.len().saturating_sub(MAX_REPORT_TAIL_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text.get(start..)
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(c: char) -> ContentId {
        ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn requirements(revision: &str) -> EffectRequirements {
        EffectRequirements {
            schema: AUTHORITY_SCHEMA.into(),
            workflow_revision: revision.into(),
            machine_identity: format!("blake3:{}", "9".repeat(64)),
            policy_ref: format!("blake3:{}", "8".repeat(64)),
            grants: vec![Grant {
                node_id: 0,
                operation: "lattice/shell".into(),
                resource: "lattice://process/0".into(),
                ability: "process/execute".into(),
            }],
        }
    }

    fn environment() -> GuestEnvironment {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "log-triage".to_string(),
            AdmittedWorkflow {
                definition: id('d'),
                requirements: requirements(&format!("blake3:{}", "7".repeat(64))),
            },
        );
        workflows.insert(
            "workspace-journal".to_string(),
            AdmittedWorkflow {
                definition: id('e'),
                requirements: requirements(&format!("blake3:{}", "6".repeat(64))),
            },
        );
        GuestEnvironment {
            schema: ENVIRONMENT_SCHEMA.into(),
            architecture: ARCHITECTURE.into(),
            lattice: LatticeBuild {
                revision: "661499d282c2e79ebefbd0f9b35beb5f8d7795cd".into(),
                binary: id('a'),
            },
            guest_shell: id('b'),
            busybox: id('c'),
            workflows,
        }
    }

    fn object(c: char) -> StoreObject {
        StoreObject {
            path: format!("/nix/store/{}-object", c.to_string().repeat(32)),
            blake3: id(c),
        }
    }

    fn manifest() -> GuestManifest {
        let environment = environment();
        GuestManifest {
            schema: MANIFEST_SCHEMA.into(),
            architecture: ARCHITECTURE.into(),
            kernel: object('1'),
            initrd: object('2'),
            vmm: Vmm {
                kind: VMM_KIND.into(),
                version: "cloud-hypervisor v54.0".into(),
                binary: object('3'),
            },
            environment_identity: environment.identity().unwrap_or_else(|_| unreachable!()),
            environment,
            devices: DeviceProfile {
                workspace_disk: WORKSPACE_DISK.into(),
                vsock_port: LATTICE_WORKER_PORT,
                network: NETWORK_NONE.into(),
            },
            resources: GuestResources {
                vcpus: 1,
                memory_mib: 768,
                workspace_mib: 64,
            },
        }
    }

    const NOW: u64 = 1_000;
    const DEADLINE: u64 = 10_000;

    fn attempt(manifest: &GuestManifest, a: char, workflow: &str) -> AttemptRequest {
        request(manifest, id(a), id('f'), workflow, 1, DEADLINE).unwrap_or_else(|_| unreachable!())
    }

    fn code<T: std::fmt::Debug>(result: Result<T, Diagnostic>) -> String {
        match result {
            Err(diagnostic) => diagnostic.code,
            Ok(value) => unreachable!("unexpectedly admitted {value:?}"),
        }
    }

    #[test]
    fn exact_attempt_is_admitted_by_host_and_guest() {
        let manifest = manifest();
        let request = attempt(&manifest, '4', "log-triage");
        assert!(admit_attempt(&manifest, &request, NOW).is_ok());
        assert!(admit_dispatch(&manifest.environment, &request, NOW).is_ok());
        assert_eq!(request.guest_identity.len(), GUEST_IDENTITY_HEX);
    }

    // r[verify kiln.lattice.worker.guest.scenario.identity]
    #[test]
    fn identities_are_canonical_and_bound_to_content() {
        let manifest = manifest();
        let reparsed: GuestManifest = serde_json::from_slice(
            &serde_json::to_vec_pretty(&manifest).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(manifest.identity().ok(), reparsed.identity().ok());
        let mut changed = manifest.clone();
        changed.kernel.blake3 = id('5');
        assert_ne!(manifest.identity().ok(), changed.identity().ok());
        let mut stale = manifest.clone();
        stale.environment.busybox = id('5');
        assert_eq!(
            code(stale.validate()),
            "lattice_worker_environment_identity"
        );
    }

    // r[verify kiln.lattice.worker.guest.scenario.refused]
    #[test]
    fn crossed_identities_are_refused_before_dispatch() {
        let manifest = manifest();
        let good = attempt(&manifest, '4', "log-triage");
        let mut cases: Vec<(AttemptRequest, &str)> = Vec::new();
        let mut image = good.clone();
        image.image = id('5');
        cases.push((image, "lattice_worker_image_mismatch"));
        let mut environment = good.clone();
        environment.environment = id('5');
        cases.push((environment, "lattice_worker_environment_mismatch"));
        let mut workflow = good.clone();
        workflow.workflow = "workspace-journal".into();
        cases.push((workflow, "lattice_worker_workflow_mismatch"));
        let mut unknown = good.clone();
        unknown.workflow = "absent".into();
        cases.push((unknown, "lattice_worker_workflow_unknown"));
        let mut definition = good.clone();
        definition.definition = id('5');
        cases.push((definition, "lattice_worker_workflow_mismatch"));
        let mut revision = good.clone();
        revision.workflow_revision = "blake3:other".into();
        cases.push((revision, "lattice_worker_workflow_mismatch"));
        let other = attempt(&manifest, '5', "log-triage");
        let mut workspace = good.clone();
        workspace.workspace = other.workspace.clone();
        cases.push((workspace, "lattice_worker_workspace_mismatch"));
        let mut generation = good.clone();
        generation.workspace.generation = 2;
        cases.push((generation, "lattice_worker_workspace_mismatch"));
        let mut guest = good.clone();
        guest.guest_identity = other.guest_identity.clone();
        cases.push((guest, "lattice_worker_guest_identity"));
        let mut schema = good.clone();
        schema.schema = "kiln.lattice-worker-attempt.v0".into();
        cases.push((schema, "lattice_worker_attempt_schema"));
        let mut expired = good.clone();
        expired.deadline_unix_ms = NOW;
        cases.push((expired, "lattice_worker_deadline"));
        for (request, expected) in cases {
            assert_eq!(code(admit_attempt(&manifest, &request, NOW)), expected);
            if expected != "lattice_worker_image_mismatch" {
                assert_eq!(
                    code(admit_dispatch(&manifest.environment, &request, NOW)),
                    expected
                );
            }
        }
    }

    #[test]
    fn guest_refuses_a_request_for_another_environment() {
        let manifest = manifest();
        let request = attempt(&manifest, '4', "log-triage");
        let mut other = manifest.environment.clone();
        other.lattice.binary = id('5');
        assert_eq!(
            code(admit_dispatch(&other, &request, NOW)),
            "lattice_worker_environment_mismatch"
        );
    }

    // r[verify kiln.lattice.worker.guest.scenario.isolated]
    #[test]
    fn independent_attempts_share_the_image_but_not_mutable_identities() {
        let manifest = manifest();
        let first = attempt(&manifest, '4', "log-triage");
        let second = attempt(&manifest, '5', "workspace-journal");
        assert_eq!(first.image, second.image);
        assert_eq!(first.environment, second.environment);
        assert_ne!(first.workspace.identity, second.workspace.identity);
        assert_ne!(first.guest_identity, second.guest_identity);
        let next = request(&manifest, id('4'), id('f'), "log-triage", 2, DEADLINE)
            .unwrap_or_else(|_| unreachable!());
        assert_ne!(first.workspace.identity, next.workspace.identity);
        assert_ne!(first.guest_identity, next.guest_identity);
    }

    #[test]
    fn manifest_refuses_unadmitted_devices_resources_and_objects() {
        let mut network = manifest();
        network.devices.network = "tap".into();
        assert_eq!(code(network.validate()), "lattice_worker_devices");
        let mut port = manifest();
        port.devices.vsock_port = crate::AGENT_PORT;
        assert_eq!(code(port.validate()), "lattice_worker_devices");
        let mut memory = manifest();
        memory.resources.memory_mib = MIN_GUEST_MEMORY_MIB - 1;
        assert_eq!(code(memory.validate()), "lattice_worker_resources");
        let mut path = manifest();
        path.kernel.path = "/tmp/vmlinux".into();
        assert_eq!(code(path.validate()), "lattice_worker_store_object");
        let mut vmm = manifest();
        vmm.vmm.kind = "firecracker".into();
        assert_eq!(code(vmm.validate()), "lattice_worker_vmm");
        let mut arch = manifest();
        arch.architecture = "aarch64".into();
        assert_eq!(code(arch.validate()), "lattice_worker_architecture");
        let mut revision = manifest();
        revision.environment.lattice.revision = "main".into();
        assert_eq!(code(revision.validate()), "lattice_worker_revision");
    }

    #[test]
    fn authority_profile_renders_exact_requirements_and_refuses_injection() {
        let requirements = requirements(&format!("blake3:{}", "7".repeat(64)));
        let profile = authority_profile(&requirements, DEADLINE).unwrap_or_else(|_| unreachable!());
        assert!(profile.contains("expires_at_unix_ms = 10000"));
        assert!(profile.contains("resource = \"lattice://process/0\""));
        assert!(profile.contains(AUTHORITY_SCHEMA));
        let mut injected = requirements.clone();
        injected.policy_ref = "x\", roots = { all = true }, y = \"".into();
        assert_eq!(
            code(authority_profile(&injected, DEADLINE)),
            "lattice_worker_text"
        );
        let mut interpolated = requirements.clone();
        interpolated.workflow_revision = "%{std}".into();
        assert_eq!(
            code(authority_profile(&interpolated, DEADLINE)),
            "lattice_worker_text"
        );
        let mut empty = requirements;
        empty.grants.clear();
        assert_eq!(
            code(authority_profile(&empty, DEADLINE)),
            "lattice_worker_grants"
        );
        assert!(lattice_config("/workspace/lattice/state", "/workspace/a.ncl", 60).is_ok());
        assert_eq!(
            code(lattice_config("/workspace/\"x", "/a.ncl", 60)),
            "lattice_worker_text"
        );
    }

    #[test]
    fn only_a_never_written_workspace_prefix_is_blank() {
        let blank = vec![0_u8; WORKSPACE_PROBE_BYTES];
        assert!(workspace_blank(&blank));
        let mut used = blank.clone();
        if let Some(byte) = used.get_mut(1080) {
            *byte = 0x53;
        }
        assert!(!workspace_blank(&used));
        assert!(!workspace_blank(
            blank.get(..WORKSPACE_PROBE_BYTES - 1).unwrap_or_default()
        ));
    }

    #[test]
    fn frames_are_bounded_and_schema_checked() {
        let manifest = manifest();
        let message = HostMessage::Dispatch {
            request: Box::new(attempt(&manifest, '4', "log-triage")),
            clock_unix_ms: NOW,
            entropy_hex: "00".repeat(ENTROPY_BYTES),
        };
        let bytes = encode(message.clone()).unwrap_or_else(|_| unreachable!());
        let body = bytes.get(4..).unwrap_or_default();
        assert_eq!(decode::<HostMessage>(body).ok(), Some(message));
        let wrong = serde_json::to_vec(&Frame {
            schema: "kiln.microvm.guest.v1".to_string(),
            message: HostMessage::Ping { nonce: "n".into() },
        })
        .unwrap_or_default();
        assert!(decode::<HostMessage>(&wrong).is_err());
        assert!(frame_length(0_u32.to_be_bytes()).is_err());
        assert!(frame_length(((MAX_FRAME_BYTES + 1) as u32).to_be_bytes()).is_err());
        assert!(decode::<GuestMessage>(b"{\"schema\":1}").is_err());
    }

    #[test]
    fn report_tail_is_bounded_and_printable() {
        let long = format!("{}\u{7}é", "x".repeat(MAX_REPORT_TAIL_BYTES));
        let tail = bounded_tail(&long);
        assert!(tail.len() <= MAX_REPORT_TAIL_BYTES + 1);
        assert!(tail.ends_with(" é"));
    }
}
