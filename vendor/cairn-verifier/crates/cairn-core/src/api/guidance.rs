pub use crate::verified::adoption_report::*;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented admit_agent_guidance_declaration root function."
    )
)]
pub use crate::verified::agent_guidance::admit_declaration as admit_agent_guidance_declaration;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented admit_portable_agent_guidance_result root function."
    )
)]
pub use crate::verified::agent_guidance::admit_portable_result as admit_portable_agent_guidance_result;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented build_agent_guidance_result root function."
    )
)]
pub use crate::verified::agent_guidance::build_result as build_agent_guidance_result;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented portable_agent_guidance_result root function."
    )
)]
pub use crate::verified::agent_guidance::portable_result as portable_agent_guidance_result;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented select_agent_guidance root function."
    )
)]
pub use crate::verified::agent_guidance::select as select_agent_guidance;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented validate_agent_guidance_policy root function."
    )
)]
pub use crate::verified::agent_guidance::validate_policy as validate_agent_guidance_policy;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AdmittedAgentGuidance root type."
    )
)]
pub use crate::verified::agent_guidance::Admitted as AdmittedAgentGuidance;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentArtifactGuidance root type."
    )
)]
pub use crate::verified::agent_guidance::ArtifactAdvice as AgentArtifactGuidance;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceBounds root type."
    )
)]
pub use crate::verified::agent_guidance::Bounds as AgentGuidanceBounds;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentContextReference root type."
    )
)]
pub use crate::verified::agent_guidance::ContextReference as AgentContextReference;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceDeclaration root type."
    )
)]
pub use crate::verified::agent_guidance::Declaration as AgentGuidanceDeclaration;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceEntry root type."
    )
)]
pub use crate::verified::agent_guidance::Entry as AgentGuidanceEntry;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceResult root type."
    )
)]
pub use crate::verified::agent_guidance::Evaluation as AgentGuidanceResult;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceIssue root type."
    )
)]
pub use crate::verified::agent_guidance::Issue as AgentGuidanceIssue;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceObservationStatus root type."
    )
)]
pub use crate::verified::agent_guidance::ObservationStatus as AgentGuidanceObservationStatus;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentOperationGuidance root type."
    )
)]
pub use crate::verified::agent_guidance::OperationAdvice as AgentOperationGuidance;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidancePolicy root type."
    )
)]
pub use crate::verified::agent_guidance::Policy as AgentGuidancePolicy;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented PortableAgentGuidanceEntry root type."
    )
)]
pub use crate::verified::agent_guidance::PortableEntry as PortableAgentGuidanceEntry;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented PortableAgentGuidanceResult root type."
    )
)]
pub use crate::verified::agent_guidance::PortableEvaluation as PortableAgentGuidanceResult;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceRequest root type."
    )
)]
pub use crate::verified::agent_guidance::Request as AgentGuidanceRequest;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceSelectionPlan root type."
    )
)]
pub use crate::verified::agent_guidance::SelectionPlan as AgentGuidanceSelectionPlan;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented AgentGuidanceSourceObservation root type."
    )
)]
pub use crate::verified::agent_guidance::SourceObservation as AgentGuidanceSourceObservation;
pub use crate::verified::agent_guidance::{
    safe_relative_context_path, AGENT_GUIDANCE_AUTHORITY_ADVISORY, AGENT_GUIDANCE_AUTHORITY_FACT,
    AGENT_GUIDANCE_AUTHORITY_POLICY, AGENT_GUIDANCE_ISSUE_AUTHORITY,
    AGENT_GUIDANCE_ISSUE_DECLARATION, AGENT_GUIDANCE_ISSUE_OBSERVATION, AGENT_GUIDANCE_ISSUE_PATH,
    AGENT_GUIDANCE_ISSUE_PORTABLE_INVALID, AGENT_GUIDANCE_ISSUE_PORTABLE_RAW,
    AGENT_GUIDANCE_ISSUE_SELECTOR, AGENT_GUIDANCE_ISSUE_SOURCE,
    AGENT_GUIDANCE_ISSUE_SOURCE_MISSING, AGENT_GUIDANCE_OPERATION_CONTEXT,
    AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS, AGENT_GUIDANCE_POLICY_SCHEMA_V1,
    AGENT_GUIDANCE_SOURCE_LIFECYCLE, AGENT_GUIDANCE_SOURCE_PROJECT,
};
pub use crate::verified::agent_receipt::{
    agent_run_preserves_receipt_hash, agent_run_receipt_hash, agent_run_receipt_issues,
    authoritative_command_hash, authoritative_selection_hash, evaluate_agent_run_completion,
    evaluate_authoritative_agent_completion, positive_agent_run_fixture, AgentArtifactReference,
    AgentAuthoritativeCompletionReport, AgentCompletionDiagnostic, AgentDiagnosticEvidence,
    AgentExecutionResult, AgentNonPassingCheck, AgentRequiredCheck, AgentRunGateIssue,
    AgentRunGateReport, AgentSourceSpan, AgentVerificationContract,
    AGENT_RUN_RECEIPT_SCHEMA_VERSION, NO_MUTATION_PLAN_HASH,
};
pub use crate::verified::ai_spec_draft::{
    ai_spec_draft_hash, validate_ai_spec_draft, AiSpecDraftIssue, AiSpecDraftProvenance,
    AiSpecDraftReport, AI_SPEC_DRAFT_SCHEMA_V1,
};
pub use crate::verified::artifact_workflow::{
    admit_workflow_profile_registry, admit_workflow_schemas, build_workflow_instructions,
    derive_workflow_status, evaluate_workflow_profile, outcome_machine_evidence_paths,
    parse_acceptance_ids, profile_issue_applies_to_stage, select_workflow_profile,
    synthetic_acceptance_spec, tasks_with_requirement_links,
    workflow_change_profile_metadata_from_value, workflow_markdown_is_structurally_present,
    workflow_profile_registry_from_value, workflow_schemas_from_value, workflow_topological_order,
    WorkflowAdmissionError, WorkflowArtifactFile, WorkflowArtifactObservation,
    WorkflowArtifactPolicy, WorkflowArtifactState, WorkflowArtifactStatus, WorkflowBounds,
    WorkflowChangeProfileMetadata, WorkflowEffectProfilePolicy, WorkflowInstructionEnvelope,
    WorkflowOutcomeMachineEvidenceObservation, WorkflowProfileArtifactHash,
    WorkflowProfileEvaluation, WorkflowProfileEvidence, WorkflowProfileIssue, WorkflowProfileRef,
    WorkflowProfileRegistryPolicy, WorkflowProfileSelection, WorkflowReviewEvidence,
    WorkflowReviewInputEvidence, WorkflowReviewObligationPolicy, WorkflowSchemaPolicy,
    WorkflowStatusReport, WorkflowUpstreamArtifact, MAX_WORKFLOW_ARTIFACTS_LIMIT,
    MAX_WORKFLOW_ARTIFACT_BYTES_LIMIT, MAX_WORKFLOW_CONSTRAINTS_LIMIT,
    MAX_WORKFLOW_DEPENDENCIES_LIMIT, MAX_WORKFLOW_DIRECTORY_DEPTH_LIMIT,
    MAX_WORKFLOW_DIRECTORY_ENTRIES_LIMIT, MAX_WORKFLOW_FILES_LIMIT, MAX_WORKFLOW_ISSUES_LIMIT,
    MAX_WORKFLOW_NON_CLAIMS_LIMIT, MAX_WORKFLOW_PATH_BYTES_LIMIT, MAX_WORKFLOW_SCHEMAS_LIMIT,
    MAX_WORKFLOW_TEMPLATE_BYTES_LIMIT, OUTCOME_MACHINE_REVIEW_SCHEMA_V1,
    WORKFLOW_ADMISSION_ACCEPTANCE, WORKFLOW_ADMISSION_DELTA_SPECS, WORKFLOW_ADMISSION_DESIGN,
    WORKFLOW_ADMISSION_MARKDOWN, WORKFLOW_ADMISSION_OUTCOME_MACHINE, WORKFLOW_ADMISSION_PROPOSAL,
    WORKFLOW_ADMISSION_RISK, WORKFLOW_ADMISSION_SEMANTIC_REVIEW, WORKFLOW_ADMISSION_TASKS,
    WORKFLOW_ADMISSION_VERIFICATION, WORKFLOW_COMPATIBILITY_PROFILE,
    WORKFLOW_INSTRUCTIONS_SCHEMA_V1, WORKFLOW_ISSUE_ARTIFACT_INVALID,
    WORKFLOW_ISSUE_ARTIFACT_UNKNOWN, WORKFLOW_ISSUE_DEPENDENCY_BLOCKED,
    WORKFLOW_ISSUE_OBSERVATION_DUPLICATE, WORKFLOW_ISSUE_OBSERVATION_INCONSISTENT,
    WORKFLOW_ISSUE_OBSERVATION_UNKNOWN, WORKFLOW_ISSUE_OUTCOME_MACHINE_AUTHORITY,
    WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION, WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE,
    WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE, WORKFLOW_ISSUE_OUTCOME_MACHINE_SHAPE,
    WORKFLOW_ISSUE_POLICY_ADMISSION, WORKFLOW_ISSUE_POLICY_BOUNDS, WORKFLOW_ISSUE_POLICY_CYCLE,
    WORKFLOW_ISSUE_POLICY_DUPLICATE_ARTIFACT, WORKFLOW_ISSUE_POLICY_DUPLICATE_DEPENDENCY,
    WORKFLOW_ISSUE_POLICY_DUPLICATE_SCHEMA, WORKFLOW_ISSUE_POLICY_PATH_KIND,
    WORKFLOW_ISSUE_POLICY_PREREQUISITE, WORKFLOW_ISSUE_POLICY_SCHEMA_VERSION,
    WORKFLOW_ISSUE_POLICY_SELF_DEPENDENCY, WORKFLOW_ISSUE_POLICY_SHAPE,
    WORKFLOW_ISSUE_POLICY_STAGE, WORKFLOW_ISSUE_POLICY_STATES, WORKFLOW_ISSUE_POLICY_TEMPLATE,
    WORKFLOW_ISSUE_POLICY_UNKNOWN_DEPENDENCY, WORKFLOW_ISSUE_POLICY_UNSAFE_PATH,
    WORKFLOW_ISSUE_PROFILE_ACCEPTANCE, WORKFLOW_ISSUE_PROFILE_ARTIFACT,
    WORKFLOW_ISSUE_PROFILE_EFFECT, WORKFLOW_ISSUE_PROFILE_REGISTRY, WORKFLOW_ISSUE_PROFILE_REVIEW,
    WORKFLOW_ISSUE_PROFILE_SELECTION, WORKFLOW_ISSUE_PROFILE_TASK_LINK, WORKFLOW_PATH_KIND_FILE,
    WORKFLOW_PATH_KIND_SPEC_DIRECTORY, WORKFLOW_PROFILE_SOURCE_CREATE,
    WORKFLOW_PROFILE_STAGE_COMPLETION, WORKFLOW_SCHEMA_VERSION_V1, WORKFLOW_SPEC_EFFECT_DELTA,
    WORKFLOW_SPEC_EFFECT_NONE, WORKFLOW_STATE_BLOCKED, WORKFLOW_STATE_DONE, WORKFLOW_STATE_READY,
    WORKFLOW_STATE_SKIPPED, WORKFLOW_STATUS_SCHEMA_V1, WORKFLOW_SUPPORTED_STATES,
    WORKFLOW_TASK_LINK_ACCEPTANCE, WORKFLOW_TASK_LINK_REQUIREMENT,
};
pub use crate::verified::developer_experience::{
    onboarding_templates, plan_prompt_scaffold, plan_reverse_spec_discovery,
    plan_template_application, DiscoveryCandidate, DiscoveryPlan, DiscoverySnapshot, FilePlan,
    FilePlanEntry, PlannedFile, ScaffoldPlan, TemplateProfile, DISCOVERY_MAX_BYTES,
    DISCOVERY_MAX_FILES, DISCOVERY_POLICY_ID, SCAFFOLD_POLICY_ID, TEMPLATE_POLICY_ID,
};
pub use crate::verified::developer_loop::*;
pub use crate::verified::generated_agent_workflow::*;
pub use crate::verified::verification_impact::{
    derive_verification_impact, VerificationCheckDefinition, VerificationCheckMap,
    VerificationEvidenceRefresh, VerificationImpactCausalPath, VerificationImpactInput,
    VerificationImpactIssue, VerificationImpactReport, VerificationImpactScope,
    VerificationImpactUncertainty, VerificationPriorEvidence, VerificationRequirementCheck,
    VerificationRequirementEdge, VerificationSourceChange, VerificationSourceChangeKind,
    VerificationSourceManifest, VerificationSourceMember, MAX_IMPACT_CHANGED_PATHS,
    MAX_IMPACT_CHECKS, MAX_IMPACT_EDGES, MAX_IMPACT_FEATURES, MAX_IMPACT_PATH_BYTES,
    MAX_IMPACT_PRIOR_EVIDENCE, MAX_IMPACT_REQUIREMENT_BINDINGS, MAX_IMPACT_SOURCE_MEMBERS,
    MAX_IMPACT_TEXT_BYTES, VERIFICATION_CHECK_MAP_SCHEMA_V1, VERIFICATION_IMPACT_BOUNDARY,
    VERIFICATION_IMPACT_SCHEMA_V1,
};
pub use crate::verified::workflow_collaboration::*;
