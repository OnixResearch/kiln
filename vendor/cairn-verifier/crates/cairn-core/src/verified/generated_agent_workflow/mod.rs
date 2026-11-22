mod admission;
mod freshness;
mod model;
mod plan;
mod render;

pub use admission::{
    admit_generated_agent_workflow_policy, contains_legacy_layout_reference,
    find_entry_guide_profile, find_generated_workflow_adapter, find_generated_workflow_command,
    find_generated_workflow_descriptor,
};
pub use freshness::{
    generated_workflow_manifest_identity, inspect_copied_generated_workflow_guide,
    validate_generated_workflow_manifest,
};
pub use model::{
    AdmittedGeneratedWorkflowCatalog, GeneratedAgentWorkflowPolicy, GeneratedWorkflowAdapter,
    GeneratedWorkflowAdapterFormat, GeneratedWorkflowBounds, GeneratedWorkflowCommand,
    GeneratedWorkflowCommandClass, GeneratedWorkflowCopiedGuideClassification,
    GeneratedWorkflowCopiedGuideInspection, GeneratedWorkflowDescriptor,
    GeneratedWorkflowDestinationSnapshot, GeneratedWorkflowEntryGuide,
    GeneratedWorkflowFileObservation, GeneratedWorkflowFreshness, GeneratedWorkflowInstallPlan,
    GeneratedWorkflowIssue, GeneratedWorkflowManifest, GeneratedWorkflowManifestFile,
    GeneratedWorkflowManifestObservation, GeneratedWorkflowMutationClass,
    GeneratedWorkflowObservedKind, GeneratedWorkflowPermission, GeneratedWorkflowPlannedFile,
    RenderedGeneratedWorkflowFile, GENERATED_AGENT_WORKFLOW_ENTRY_ID,
    GENERATED_AGENT_WORKFLOW_EXPLORE_ID, GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE,
    GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION, GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
    GENERATED_AGENT_WORKFLOW_ISSUE_CONFLICT, GENERATED_AGENT_WORKFLOW_ISSUE_COPIED_GUIDE,
    GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR, GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
    GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY, GENERATED_AGENT_WORKFLOW_ISSUE_GUIDE_LAYOUT,
    GENERATED_AGENT_WORKFLOW_ISSUE_PLAN_STALE, GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
    GENERATED_AGENT_WORKFLOW_ISSUE_RECOVERY, GENERATED_AGENT_WORKFLOW_MANIFEST_PREFIX,
    GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1, GENERATED_AGENT_WORKFLOW_MANIFEST_SUFFIX,
    GENERATED_AGENT_WORKFLOW_NATIVE_LAYOUT, GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1,
    INITIAL_ADAPTER_IDS, INITIAL_WORKFLOW_IDS,
};
pub use plan::{
    generated_workflow_plan_identity, plan_generated_agent_workflow_install,
    GENERATED_AGENT_WORKFLOW_PLAN_SCHEMA_V1,
};
pub use render::{
    generated_manifest_relative_path, join_generated_destination, render_generated_agent_workflows,
    safe_generated_destination_path,
};

#[cfg(test)]
mod tests;
