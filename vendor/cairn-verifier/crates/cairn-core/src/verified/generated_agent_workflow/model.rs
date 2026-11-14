use serde::{Deserialize, Serialize};

pub const GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1: &str =
    "cairn.generated-agent-workflow-policy.v1";
pub const GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1: &str =
    "cairn.generated-agent-workflow-manifest.v1";
pub const GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION: &str = "cairn-agent-workflow.v1";
pub const GENERATED_AGENT_WORKFLOW_MANIFEST_PREFIX: &str = ".cairn-agent-workflows-";
pub const GENERATED_AGENT_WORKFLOW_MANIFEST_SUFFIX: &str = ".json";
pub const GENERATED_AGENT_WORKFLOW_EXPLORE_ID: &str = "explore";
pub const GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE: &str =
    "Do not create a change, edit lifecycle artifacts, execute mutation plans, or claim implementation completion.";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_POLICY: &str = "generated_workflow.policy.invalid";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR: &str = "generated_workflow.descriptor.invalid";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER: &str = "generated_workflow.adapter.invalid";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION: &str =
    "generated_workflow.destination.invalid";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_CONFLICT: &str = "generated_workflow.destination.conflict";
pub const GENERATED_AGENT_WORKFLOW_ISSUE_PLAN_STALE: &str = "generated_workflow.plan.stale";

pub const INITIAL_WORKFLOW_IDS: &[&str] =
    &["apply", "archive", "drain", "explore", "propose", "verify"];
pub const INITIAL_ADAPTER_IDS: &[&str] = &["agent-skills", "claude", "codex"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedWorkflowPermission {
    Read,
    Mutation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedWorkflowMutationClass {
    ReadOnly,
    PreviewFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedWorkflowAdapterFormat {
    AgentSkills,
    Claude,
    Codex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedWorkflowFreshness {
    Current,
    Missing,
    StaleSource,
    Modified,
    UnmanagedConflict,
    UnsupportedVersion,
    SymlinkConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedWorkflowObservedKind {
    Missing,
    Regular,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowBounds {
    pub max_descriptors: usize,
    pub max_adapters: usize,
    pub max_command_classes: usize,
    pub max_text_bytes: usize,
    pub max_list_items: usize,
    pub max_files_per_adapter: usize,
    pub max_path_bytes: usize,
    pub max_file_bytes: usize,
    pub max_issues: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowCommandClass {
    pub id: String,
    pub permission: GeneratedWorkflowPermission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowDescriptor {
    pub id: String,
    pub version: u32,
    pub goal: String,
    pub success_evidence: Vec<String>,
    pub required_evidence: Vec<String>,
    pub command_classes: Vec<String>,
    pub mutation_class: GeneratedWorkflowMutationClass,
    pub progress_rule: String,
    pub immediate_action: String,
    pub output_shape: String,
    pub constraints: Vec<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowAdapter {
    pub id: String,
    pub version: u32,
    pub format: GeneratedWorkflowAdapterFormat,
    pub file_prefix: String,
    pub file_suffix: String,
    pub directory_suffix: String,
    pub front_matter: bool,
    pub max_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedAgentWorkflowPolicy {
    pub schema: String,
    pub generator_version: String,
    pub command_classes: Vec<GeneratedWorkflowCommandClass>,
    pub descriptors: Vec<GeneratedWorkflowDescriptor>,
    pub adapters: Vec<GeneratedWorkflowAdapter>,
    pub bounds: GeneratedWorkflowBounds,
    pub receipt_fields: Vec<String>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl std::fmt::Display for GeneratedWorkflowIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.field_path, self.message
        )
    }
}

impl std::error::Error for GeneratedWorkflowIssue {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedGeneratedWorkflowCatalog {
    pub policy: GeneratedAgentWorkflowPolicy,
    pub catalog_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderedGeneratedWorkflowFile {
    pub workflow_id: String,
    pub relative_path: String,
    pub contents: String,
    pub content_blake3: String,
    pub descriptor_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowFileObservation {
    pub relative_path: String,
    pub kind: GeneratedWorkflowObservedKind,
    pub content_blake3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowManifestFile {
    pub workflow_id: String,
    pub relative_path: String,
    pub generated_blake3: String,
    pub expected_prior_blake3: Option<String>,
    pub descriptor_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowManifest {
    pub schema: String,
    pub generator_version: String,
    pub policy_hash: String,
    pub catalog_identity: String,
    pub adapter_id: String,
    pub adapter_version: u32,
    pub destination: String,
    pub files: Vec<GeneratedWorkflowManifestFile>,
    pub removal_candidates: Vec<String>,
    pub non_claims: Vec<String>,
    pub manifest_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedWorkflowManifestObservation {
    Missing,
    Invalid(String),
    Symlink,
    Other,
    Present(Box<GeneratedWorkflowManifest>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedWorkflowDestinationSnapshot {
    pub files: Vec<GeneratedWorkflowFileObservation>,
    pub manifest: GeneratedWorkflowManifestObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowPlannedFile {
    pub workflow_id: String,
    pub relative_path: String,
    pub generated_blake3: String,
    pub descriptor_identity: String,
    pub expected_prior_blake3: Option<String>,
    pub observed_blake3: Option<String>,
    pub freshness: GeneratedWorkflowFreshness,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWorkflowInstallPlan {
    pub schema: String,
    pub adapter_id: String,
    pub adapter_version: u32,
    pub destination: String,
    pub policy_hash: String,
    pub catalog_identity: String,
    pub max_file_bytes: usize,
    pub files: Vec<GeneratedWorkflowPlannedFile>,
    pub manifest_path: String,
    pub manifest: GeneratedWorkflowManifest,
    pub manifest_bytes: String,
    pub expected_prior_manifest_identity: Option<String>,
    pub manifest_freshness: GeneratedWorkflowFreshness,
    pub removal_candidates: Vec<String>,
    pub blockers: Vec<String>,
    pub plan_hash: String,
    pub non_claims: Vec<String>,
}

impl GeneratedWorkflowInstallPlan {
    pub fn blocked(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn current(&self) -> bool {
        !self.blocked()
            && self.manifest_freshness == GeneratedWorkflowFreshness::Current
            && self
                .files
                .iter()
                .all(|file| file.freshness == GeneratedWorkflowFreshness::Current)
            && self.removal_candidates.is_empty()
    }
}
