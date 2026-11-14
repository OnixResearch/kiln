use serde_json::Value;

use super::document_collaboration::DocumentCollaborationPolicy;
use super::PolicySchemaCompatibilityMetadata;

use crate::verified::{
    agent_guidance::AgentGuidancePolicy,
    artifact_workflow::{WorkflowProfileRegistryPolicy, WorkflowSchemaPolicy},
    cli_machine::MachineContractPolicy,
    developer_loop::DeveloperLoopPolicy,
    function_address_gates::FunctionAddressPolicy,
    generated_agent_workflow::GeneratedAgentWorkflowPolicy,
    nominal_identity::NominalIdentityPolicy,
    runtime_evidence::RuntimeEvidencePolicy,
    stack_provenance::StackProvenancePolicy,
    traceability::TraceabilityProfile,
};

pub const SUPPORTED_POLICY_VERSION: u64 = 1;
pub const TASK_IDENTITY_MARKER_ID: &str = "task";
pub const TASK_PREDECESSOR_MARKER_ID: &str = "after";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub name: String,
    pub version: u64,
    pub task_markers_enabled: bool,
    pub task_marker_strictness: String,
    pub task_marker_tokens: Vec<TaskMarker>,
    pub gate_stages: Vec<String>,
    pub gate_ignore_archives: bool,
    pub gate_finding_scope: String,
    pub gate_finding_promoted: String,
    pub artifact_substance_policy: ArtifactSubstancePolicy,
    pub workflow_schemas: Vec<WorkflowSchemaPolicy>,
    pub workflow_profile_policy: WorkflowProfileRegistryPolicy,
    pub agent_guidance_policy: AgentGuidancePolicy,
    pub developer_loop_policy: DeveloperLoopPolicy,
    pub generated_agent_workflow_policy: GeneratedAgentWorkflowPolicy,
    pub machine_contract_policy: MachineContractPolicy,
    pub document_collaboration_policy: DocumentCollaborationPolicy,
    pub sync_require_gate: bool,
    pub archive_require_gate: bool,
    pub migrate_require_manifest: bool,
    pub require_receipt_hashes: bool,
    pub receipt_schemas: Vec<ReceiptSchema>,
    pub exemption_marker_policy: ExemptionMarkerPolicy,
    pub change_metadata_allowed_groups: Vec<String>,
    pub change_metadata_group_prefixes: Vec<String>,
    pub change_metadata_statuses: Vec<String>,
    pub steel_orchestration_policy: SteelOrchestrationPolicy,
    pub traceability_policy: TraceabilityPolicy,
    pub stack_provenance_policy: StackProvenancePolicy,
    pub function_address_policy: FunctionAddressPolicy,
    pub runtime_evidence_policy: RuntimeEvidencePolicy,
    pub nominal_identity_policy: NominalIdentityPolicy,
    pub lifecycle_store_policy: LifecycleStorePolicy,
    pub policy_schema_compatibility: Option<PolicySchemaCompatibilityMetadata>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSubstancePolicy {
    pub proposal_minimum_substantive_lines: usize,
    pub design_minimum_substantive_lines: usize,
    pub minimum_requirement_blocks: usize,
    pub minimum_substantive_tasks: usize,
    pub placeholder_tokens: Vec<String>,
    pub normative_tokens: Vec<String>,
    pub dependency_marker_id: String,
    pub dependency_target_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStorePolicy {
    pub schema: String,
    pub hash_algorithm: String,
    pub canonicalization: String,
    pub bounds: LifecycleStoreBounds,
    pub store_identity: LifecycleStoreProjectionPolicy,
    pub project_config: LifecycleStoreProjectionPolicy,
    pub root_resolution: LifecycleStoreRootResolutionPolicy,
    pub locator: LifecycleStoreLocalStateSchema,
    pub worksets: LifecycleStoreWorksetPolicy,
    pub opener_profiles: Vec<LifecycleStoreOpenerProfile>,
    pub receipts: LifecycleStoreReceiptPolicy,
    pub issue_codes: Vec<String>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreBounds {
    pub max_store_id_bytes: usize,
    pub max_remote_hint_bytes: usize,
    pub max_references: usize,
    pub max_locator_entries: usize,
    pub max_workset_id_bytes: usize,
    pub max_worksets: usize,
    pub max_workset_members: usize,
    pub max_path_bytes: usize,
    pub max_context_specs_per_store: usize,
    pub max_spec_title_bytes: usize,
    pub max_purpose_summary_bytes: usize,
    pub max_issue_count: usize,
    pub max_non_claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreProjectionPolicy {
    pub source_schema: String,
    pub projection_schema: String,
    pub source_path: String,
    pub projection_path: String,
    pub source_identity_field: String,
    pub runtime_nickel_evaluation: bool,
    pub stale_projection_rejected: bool,
    pub required_fields: Vec<String>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreRootResolutionPolicy {
    pub root_flag: String,
    pub lifecycle_store_flag: String,
    pub selectors_mutually_exclusive: bool,
    pub eligible_commands: Vec<String>,
    pub precedence: Vec<String>,
    pub native_root_wins_over_project_pointer: bool,
    pub current_directory_only_without_vcs_boundary: bool,
    pub consult_global_registry: bool,
    pub machine_default_store: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreLocalStateSchema {
    pub schema: String,
    pub relative_path: String,
    pub state_hash_field: String,
    pub expected_prior_hash_field: String,
    pub hash_algorithm: String,
    pub preview_required: bool,
    pub lock_kind: String,
    pub replacement_kind: String,
    pub symlink_paths_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreWorksetPolicy {
    pub state: LifecycleStoreLocalStateSchema,
    pub required_fields: Vec<String>,
    pub minimum_members: usize,
    pub opener_profile_field: String,
    pub removal_preserves_members: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreOpenerProfile {
    pub id: String,
    pub executable: String,
    pub fixed_args: Vec<String>,
    pub member_argument_mode: String,
    pub primary_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreReceiptPolicy {
    pub schema: String,
    pub evidence_class: String,
    pub full_hash_field: String,
    pub portable_hash_field: String,
    pub required_fields: Vec<String>,
    pub resolution_fields: Vec<String>,
    pub local_display_fields: Vec<String>,
    pub portable_fields: Vec<String>,
    pub portable_excluded_fields: Vec<String>,
    pub local_only_by_default: bool,
    pub release_readiness_consumes_by_default: bool,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelOrchestrationPolicy {
    pub enabled: bool,
    pub profiles: Vec<SteelOrchestrationProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelOrchestrationProfile {
    pub id: String,
    pub mode: String,
    pub allowed_host_functions: Vec<String>,
    pub deterministic_budgets: Vec<String>,
    pub redaction_classes: Vec<String>,
    pub fallback_modes: Vec<String>,
    pub receipt_schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityPolicy {
    pub default_profile: String,
    pub profiles: Vec<TraceabilityProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMarker {
    pub id: String,
    pub token: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptSchema {
    pub command: String,
    pub top_level_keys: Vec<String>,
    pub action_keys: Vec<String>,
    pub issue_keys: Vec<String>,
    pub hash_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExemptionMarkerPolicy {
    pub required_fields: Vec<String>,
    pub date_fields: Vec<String>,
    pub optional_fields: Vec<String>,
}
