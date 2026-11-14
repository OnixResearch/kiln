//! Pure, deterministic core types for cairn.
//!
//! Filesystem access, environment reads, process control, and presentation live
//! in shell crates. Modules under `verified/` are kept pure and are checked by
//! Slotcar's `verified_purity` lint.
//! r[impl cairn.supersede.boundary.pure_core]
//! r[impl cairn.supersede.migrate_openspec.boundary]
//! r[impl cairn.supersede.tracey_coverage.boundary]
//! r[impl cairn.supersede.traceability_ownership.authoritative_checker]
//! r[impl cairn.supersede.traceability_ownership.configured_profiles]
//! r[impl cairn.supersede.gate_policy.references]
//! r[impl cairn.supersede.gate_policy.task_links]
//! r[impl cairn.supersede.gate_policy.exemptions]
//! r[impl cairn.supersede.doctor]
//! r[impl cairn.supersede.doctor.boundary]

pub mod verified;

pub use verified::agent_guidance::{
    admit_agent_guidance_declaration, admit_portable_agent_guidance_result,
    build_agent_guidance_result, portable_agent_guidance_result, safe_relative_context_path,
    select_agent_guidance, validate_agent_guidance_policy, AdmittedAgentGuidance,
    AgentArtifactGuidance, AgentContextReference, AgentGuidanceBounds, AgentGuidanceDeclaration,
    AgentGuidanceEntry, AgentGuidanceIssue, AgentGuidanceObservationStatus, AgentGuidancePolicy,
    AgentGuidanceRequest, AgentGuidanceResult, AgentGuidanceSelectionPlan,
    AgentGuidanceSourceObservation, AgentOperationGuidance, PortableAgentGuidanceEntry,
    PortableAgentGuidanceResult, AGENT_GUIDANCE_AUTHORITY_ADVISORY, AGENT_GUIDANCE_AUTHORITY_FACT,
    AGENT_GUIDANCE_AUTHORITY_POLICY, AGENT_GUIDANCE_ISSUE_AUTHORITY,
    AGENT_GUIDANCE_ISSUE_DECLARATION, AGENT_GUIDANCE_ISSUE_OBSERVATION, AGENT_GUIDANCE_ISSUE_PATH,
    AGENT_GUIDANCE_ISSUE_PORTABLE_INVALID, AGENT_GUIDANCE_ISSUE_PORTABLE_RAW,
    AGENT_GUIDANCE_ISSUE_SELECTOR, AGENT_GUIDANCE_ISSUE_SOURCE,
    AGENT_GUIDANCE_ISSUE_SOURCE_MISSING, AGENT_GUIDANCE_OPERATION_CONTEXT,
    AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS, AGENT_GUIDANCE_POLICY_SCHEMA_V1,
    AGENT_GUIDANCE_SOURCE_LIFECYCLE, AGENT_GUIDANCE_SOURCE_PROJECT,
};
pub use verified::agent_receipt::{
    agent_run_preserves_receipt_hash, agent_run_receipt_hash, agent_run_receipt_issues,
    authoritative_command_hash, authoritative_selection_hash, evaluate_agent_run_completion,
    evaluate_authoritative_agent_completion, positive_agent_run_fixture, AgentArtifactReference,
    AgentAuthoritativeCompletionReport, AgentCompletionDiagnostic, AgentDiagnosticEvidence,
    AgentExecutionResult, AgentNonPassingCheck, AgentRequiredCheck, AgentRunGateIssue,
    AgentRunGateReport, AgentSourceSpan, AgentVerificationContract,
    AGENT_RUN_RECEIPT_SCHEMA_VERSION, NO_MUTATION_PLAN_HASH,
};
pub use verified::ai_spec_draft::{
    ai_spec_draft_hash, validate_ai_spec_draft, AiSpecDraftIssue, AiSpecDraftProvenance,
    AiSpecDraftReport, AI_SPEC_DRAFT_SCHEMA_V1,
};
pub use verified::archive_compaction::{
    append_compaction_receipt, build_archive_manifest, build_compaction_plan,
    build_compaction_receipt, build_unpack_plan, build_verification_receipt, compaction_chain_hash,
    compaction_genesis_hash, compaction_ledger_hash, compaction_plan_hash, compaction_receipt_hash,
    empty_compaction_ledger, ledger_path, mutation_manifest_path, object_path, pack_manifest_path,
    pointer_path, prepared_pointer_path, receipt_path, safe_archive_id, safe_relative_path,
    unpack_plan_hash, validate_compaction_ledger, validate_compaction_plan,
    validate_compaction_receipt, validate_pack_manifest, verification_receipt_hash,
    verify_pack_objects, ArchiveFileInput, ArchiveInput, ArchiveManifest, ArchiveManifestEntry,
    ArchivePointer, CompactionIssue, CompactionLedger, CompactionLedgerEntry, CompactionPlan,
    CompactionReceipt, CompactionVerificationReceipt, MutationAction, PackManifest,
    PackObjectDescriptor, PackObjectInput, UnpackFile, UnpackPlan, COMPACTION_LEDGER_SCHEMA_V1,
    COMPACTION_NON_CLAIM, COMPACTION_PACK_SCHEMA_V1, COMPACTION_PLAN_SCHEMA_V1,
    COMPACTION_POINTER_SCHEMA_V1, COMPACTION_RECEIPT_SCHEMA_V1, COMPACTION_ROOT,
    COMPACTION_UNPACK_SCHEMA_V1, COMPACTION_VERIFY_SCHEMA_V1, HASH_HEX_LENGTH,
    LOSSLESS_PACK_PROFILE, LOSSY_SUMMARY_PROFILE,
};
pub use verified::artifact_auth::adapter::{
    compare_cairn_artifact_auth, evaluate_cairn_artifact_auth, map_cairn_artifact_auth,
    CairnArtifactAuthCompatibilityClassification, CairnArtifactAuthCompatibilityDimension,
    CairnArtifactAuthCompatibilityDisposition, CairnArtifactAuthCompatibilityReport,
    CairnArtifactAuthInput, CairnArtifactAuthReport, CAIRN_ARTIFACT_AUTH_BOUNDARY,
    CAIRN_ARTIFACT_AUTH_COMPATIBILITY_BOUNDARY, CAIRN_ARTIFACT_AUTH_COMPATIBILITY_SCHEMA_V1,
    CAIRN_ARTIFACT_AUTH_PARENT_PROFILE_V1, CAIRN_ARTIFACT_AUTH_PROFILE_V1,
    CAIRN_ARTIFACT_AUTH_VERIFIER_CONTEXT_PROFILE_V1,
};
pub use verified::artifact_workflow::{
    admit_workflow_profile_registry, admit_workflow_schemas, build_workflow_instructions,
    derive_workflow_status, evaluate_workflow_profile, parse_acceptance_ids,
    profile_issue_applies_to_stage, select_workflow_profile, synthetic_acceptance_spec,
    tasks_with_requirement_links, workflow_change_profile_metadata_from_value,
    workflow_markdown_is_structurally_present, workflow_profile_registry_from_value,
    workflow_schemas_from_value, workflow_topological_order, WorkflowAdmissionError,
    WorkflowArtifactFile, WorkflowArtifactObservation, WorkflowArtifactPolicy,
    WorkflowArtifactState, WorkflowArtifactStatus, WorkflowBounds, WorkflowChangeProfileMetadata,
    WorkflowEffectProfilePolicy, WorkflowInstructionEnvelope, WorkflowProfileArtifactHash,
    WorkflowProfileEvaluation, WorkflowProfileEvidence, WorkflowProfileIssue, WorkflowProfileRef,
    WorkflowProfileRegistryPolicy, WorkflowProfileSelection, WorkflowReviewEvidence,
    WorkflowReviewInputEvidence, WorkflowReviewObligationPolicy, WorkflowSchemaPolicy,
    WorkflowStatusReport, WorkflowUpstreamArtifact, MAX_WORKFLOW_ARTIFACTS_LIMIT,
    MAX_WORKFLOW_ARTIFACT_BYTES_LIMIT, MAX_WORKFLOW_CONSTRAINTS_LIMIT,
    MAX_WORKFLOW_DEPENDENCIES_LIMIT, MAX_WORKFLOW_DIRECTORY_DEPTH_LIMIT,
    MAX_WORKFLOW_DIRECTORY_ENTRIES_LIMIT, MAX_WORKFLOW_FILES_LIMIT, MAX_WORKFLOW_ISSUES_LIMIT,
    MAX_WORKFLOW_NON_CLAIMS_LIMIT, MAX_WORKFLOW_PATH_BYTES_LIMIT, MAX_WORKFLOW_SCHEMAS_LIMIT,
    MAX_WORKFLOW_TEMPLATE_BYTES_LIMIT, WORKFLOW_ADMISSION_ACCEPTANCE,
    WORKFLOW_ADMISSION_DELTA_SPECS, WORKFLOW_ADMISSION_DESIGN, WORKFLOW_ADMISSION_MARKDOWN,
    WORKFLOW_ADMISSION_PROPOSAL, WORKFLOW_ADMISSION_RISK, WORKFLOW_ADMISSION_SEMANTIC_REVIEW,
    WORKFLOW_ADMISSION_TASKS, WORKFLOW_ADMISSION_VERIFICATION, WORKFLOW_COMPATIBILITY_PROFILE,
    WORKFLOW_INSTRUCTIONS_SCHEMA_V1, WORKFLOW_ISSUE_ARTIFACT_INVALID,
    WORKFLOW_ISSUE_ARTIFACT_UNKNOWN, WORKFLOW_ISSUE_DEPENDENCY_BLOCKED,
    WORKFLOW_ISSUE_OBSERVATION_DUPLICATE, WORKFLOW_ISSUE_OBSERVATION_INCONSISTENT,
    WORKFLOW_ISSUE_OBSERVATION_UNKNOWN, WORKFLOW_ISSUE_POLICY_ADMISSION,
    WORKFLOW_ISSUE_POLICY_BOUNDS, WORKFLOW_ISSUE_POLICY_CYCLE,
    WORKFLOW_ISSUE_POLICY_DUPLICATE_ARTIFACT, WORKFLOW_ISSUE_POLICY_DUPLICATE_DEPENDENCY,
    WORKFLOW_ISSUE_POLICY_DUPLICATE_SCHEMA, WORKFLOW_ISSUE_POLICY_PATH_KIND,
    WORKFLOW_ISSUE_POLICY_PREREQUISITE, WORKFLOW_ISSUE_POLICY_SCHEMA_VERSION,
    WORKFLOW_ISSUE_POLICY_SELF_DEPENDENCY, WORKFLOW_ISSUE_POLICY_SHAPE,
    WORKFLOW_ISSUE_POLICY_STAGE, WORKFLOW_ISSUE_POLICY_STATES, WORKFLOW_ISSUE_POLICY_TEMPLATE,
    WORKFLOW_ISSUE_POLICY_UNKNOWN_DEPENDENCY, WORKFLOW_ISSUE_POLICY_UNSAFE_PATH,
    WORKFLOW_ISSUE_PROFILE_ACCEPTANCE, WORKFLOW_ISSUE_PROFILE_ARTIFACT,
    WORKFLOW_ISSUE_PROFILE_EFFECT, WORKFLOW_ISSUE_PROFILE_REGISTRY, WORKFLOW_ISSUE_PROFILE_REVIEW,
    WORKFLOW_ISSUE_PROFILE_SELECTION, WORKFLOW_ISSUE_PROFILE_TASK_LINK, WORKFLOW_PATH_KIND_FILE,
    WORKFLOW_PATH_KIND_SPEC_DIRECTORY, WORKFLOW_PROFILE_SOURCE_CREATE, WORKFLOW_SCHEMA_VERSION_V1,
    WORKFLOW_SPEC_EFFECT_DELTA, WORKFLOW_SPEC_EFFECT_NONE, WORKFLOW_STATE_BLOCKED,
    WORKFLOW_STATE_DONE, WORKFLOW_STATE_READY, WORKFLOW_STATE_SKIPPED, WORKFLOW_STATUS_SCHEMA_V1,
    WORKFLOW_SUPPORTED_STATES, WORKFLOW_TASK_LINK_ACCEPTANCE, WORKFLOW_TASK_LINK_REQUIREMENT,
};
pub use verified::ast_grep_gate::{
    ast_grep_receipt_hash, ast_grep_receipt_hash_material, default_ast_grep_gate_policy,
    is_ast_grep_gate_mode, validate_ast_grep_receipt, AstGrepGateIssue, AstGrepGatePolicy,
    AstGrepGateReport, AST_GREP_GATE_BOUNDARY, AST_GREP_GATE_ISSUE_BLOCKING_FINDING,
    AST_GREP_GATE_ISSUE_CODEMOD_REVIEW, AST_GREP_GATE_ISSUE_MALFORMED_FINDING,
    AST_GREP_GATE_ISSUE_MALFORMED_HASH, AST_GREP_GATE_ISSUE_MISSING_FIELD,
    AST_GREP_GATE_ISSUE_OVERCLAIM, AST_GREP_GATE_ISSUE_STALE_SCOPE,
    AST_GREP_GATE_ISSUE_UNSUPPORTED_MODE, AST_GREP_GATE_ISSUE_UNSUPPORTED_OUTPUT_FORMAT,
    AST_GREP_GATE_ISSUE_WEAK_NON_CLAIM, AST_GREP_GATE_MODE_ADVISORY, AST_GREP_GATE_MODE_BLOCKING,
    AST_GREP_GATE_MODE_CODEMOD_REVIEW, AST_GREP_GATE_MODE_INVENTORY,
};
pub use verified::authenticated_inputs::{
    authenticated_evidence_policy_from_value, authenticated_signature_message,
    evaluate_authenticated_evidence, evaluate_authenticated_release_gate,
    AuthenticatedEvidenceInput, AuthenticatedEvidenceIssue, AuthenticatedEvidencePolicy,
    AuthenticatedEvidenceReport, AuthenticatedPolicyExport, AuthenticatedReleaseGateInput,
    AuthenticatedReleaseGateReport, AuthenticatedTrustedProducer, CanonicalReceiptObservation,
    MeasuredEvidenceInput, PolicyExportObservation, SignatureVerificationObservation,
    AUTHENTICATED_EVIDENCE_BOUNDARY, AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1,
    AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS, AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE,
    AUTHENTICATED_EVIDENCE_PROFILE_DIAGNOSTIC, AUTHENTICATED_EVIDENCE_PROFILE_ONIX_RELEASE,
    AUTHENTICATED_EVIDENCE_SCHEMA_V1, AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519,
};
pub use verified::change_dependencies::{
    build_change_dependency_graph, change_dependency_issue_messages,
    change_metadata_with_dependencies, dependency_blockers_for_change, next_ready_change,
    normalized_change_dependencies, plan_change_dependency_edit,
    render_change_dependency_graph_dot, render_change_dependency_graph_mermaid,
    render_change_metadata_json, ChangeDependencyBlockedChange, ChangeDependencyEdge,
    ChangeDependencyEditAction, ChangeDependencyEditPlan, ChangeDependencyGraph,
    ChangeDependencyIssue, ChangeDependencyNextReady, ChangeDependencyNode,
    DEPENDENCY_BLOCKER_UNSATISFIED, DEPENDENCY_EDIT_ISSUE_CHANGE_UNKNOWN,
    DEPENDENCY_EDIT_ISSUE_METADATA_INVALID, DEPENDENCY_EDIT_ISSUE_NOT_DECLARED,
    DEPENDENCY_ISSUE_CYCLE, DEPENDENCY_ISSUE_DUPLICATE, DEPENDENCY_ISSUE_EMPTY_ID,
    DEPENDENCY_ISSUE_INVALID_ID, DEPENDENCY_ISSUE_INVALID_ITEM, DEPENDENCY_ISSUE_INVALID_TYPE,
    DEPENDENCY_ISSUE_SELF, DEPENDENCY_ISSUE_UNKNOWN,
};
pub use verified::cli_machine::{
    build_machine_outcome, is_machine_command_name_safe, sort_machine_diagnostics,
    MachineContractBounds, MachineContractError, MachineContractPolicy, MachineDiagnostic,
    MachineDiagnosticDefinition, MachineDiagnosticSeverity, MachineEnvelope, MachineExitClass,
    MachineExitClassPolicy, MachineFailureKind, MachineOutcome, MachineProcessOutcome,
    CLI_MACHINE_DIAGNOSTIC_BLOCKED, CLI_MACHINE_DIAGNOSTIC_INCOMPATIBLE_OPTION,
    CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR, CLI_MACHINE_DIAGNOSTIC_INVOCATION_FAILED,
    CLI_MACHINE_DIAGNOSTIC_MISSING_COMMAND, CLI_MACHINE_DIAGNOSTIC_NOT_FOUND,
    CLI_MACHINE_DIAGNOSTIC_OUTPUT_INVALID, CLI_MACHINE_DIAGNOSTIC_OUTPUT_TOO_LARGE,
    CLI_MACHINE_DIAGNOSTIC_POLICY_ERROR, CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND,
    CLI_MACHINE_EXIT_CLASSES, CLI_MACHINE_MAX_CAPTURE_BYTES_LIMIT,
    CLI_MACHINE_MAX_COMMAND_BYTES_LIMIT, CLI_MACHINE_MAX_DIAGNOSTICS_LIMIT,
    CLI_MACHINE_MAX_FIELD_BYTES_LIMIT, CLI_MACHINE_MAX_PROCESS_STATUS,
    CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES, CLI_MACHINE_SCHEMA_V1, CLI_MACHINE_UNKNOWN_COMMAND,
};
pub use verified::collaboration_change::{
    apply_change_collaboration_operation, change_collaboration_action_json,
    change_collaboration_content_root, change_collaboration_lifecycle_view,
    change_collaboration_state_hash, current_change_collaboration_evidence,
    initialize_change_collaboration, materialize_change_collaboration,
    validate_change_collaboration_init, validate_change_collaboration_state,
    ChangeCollaborationAction, ChangeCollaborationError, ChangeCollaborationEvidence,
    ChangeCollaborationInit, ChangeCollaborationOperation, ChangeCollaborationState,
    BASE_REVISION_KIND_GIT_OID,
};
pub use verified::collaboration_checkout::{
    plan_change_collaboration_checkout, ChangeCollaborationCheckoutPlan,
    ChangeCollaborationLocalSnapshot,
};
pub use verified::collaboration_import::{
    change_collaboration_import_operation_id, initialize_change_collaboration_import,
    plan_change_collaboration_import, ChangeCollaborationImportSnapshot,
};
pub use verified::collaboration_portable::{
    build_portable_change_collaboration, validate_portable_change_collaboration,
    PortableChangeCollaboration, PortableChangeCollaborationError,
    PORTABLE_COLLABORATION_BACKEND_V1, PORTABLE_COLLABORATION_SCHEMA_V1,
};
pub use verified::collaboration_resolution::{
    apply_planned_change_collaboration_resolution, change_cob_resolution_plan_hash,
    change_collaboration_resolution_plan_hash, inspect_change_cob_conflict_value,
    inspect_change_cob_conflicts, inspect_change_collaboration_conflict_value,
    inspect_change_collaboration_conflicts, plan_change_cob_resolution,
    plan_change_collaboration_resolution, ChangeArtifactConflict, ChangeArtifactConflictHead,
    ChangeArtifactConflictValue, ChangeArtifactHeadSelection, ChangeCobResolutionPlan,
    ChangeCollaborationConflictInspection, ChangeCollaborationResolutionPlan,
    ChangeCollaborationResolutionSnapshot, ChangeLifecycleConflictHead,
};
pub use verified::collaboration_update::{
    apply_planned_change_collaboration_update, change_cob_update_plan_hash,
    change_collaboration_update_plan_hash, plan_change_cob_update,
    plan_change_collaboration_update, ChangeCobUpdatePlan, ChangeCollaborationUpdatePlan,
    ChangeCollaborationUpdateSnapshot,
};
pub use verified::content_addressed::{
    circuit_primitive_graph_hash, circuit_primitive_hash_material, downstream_pilot_hash,
    downstream_pilot_hash_material, export_bundle_hash, export_bundle_hash_material,
    function_object_hash, function_object_hash_material, name_pointer_hash,
    name_pointer_hash_material, validate_circuit_primitive_receipt,
    validate_downstream_pilot_receipt, validate_export_bundle_receipt,
    validate_function_object_receipt, validate_name_pointer_receipt, CircuitPrimitiveIssue,
    CircuitPrimitiveReport, DownstreamPilotIssue, DownstreamPilotReport, ExportBundleIssue,
    ExportBundleReport, FunctionObjectIssue, FunctionObjectReport, NamePointerIssue,
    NamePointerReport, CIRCUIT_PRIMITIVE_SCHEMA_V1, DOWNSTREAM_PILOT_SCHEMA_V1,
    EXPORT_BUNDLE_SCHEMA_V1, FUNCTION_OBJECT_SCHEMA_V1, NAME_POINTER_SCHEMA_V1,
};
pub use verified::content_bound_requirement_registry::{
    content_bound_requirement_registry_issues_json, evaluate_content_bound_requirement_registry,
    parse_content_bound_requirement_registry_value, validate_content_bound_requirement_registry,
    verify_content_bound_requirement_registry, ContentBoundRequirementRegistry,
    ContentBoundRequirementRegistryEvaluation, ContentBoundRequirementRegistryIssue,
    ContentBoundRequirementRegistryReceipt, ContentBoundRequirementRegistryVerification,
    ContentBoundRequirementRow, ContentBoundRevision, ContentBoundSourceSpan,
    ContentBoundSpecificationIdentity, ContentBoundSpecificationInput,
    CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY, CONTENT_BOUND_REQUIREMENT_REGISTRY_NON_CLAIMS,
    CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1,
    CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1, CONTENT_BOUND_REVISION_KIND_GIT,
    CONTENT_BOUND_REVISION_KIND_JUJUTSU, CONTENT_BOUND_REVISION_KIND_OPAQUE,
};
pub use verified::content_bound_trace_index::{
    evaluate_content_bound_trace_index, parse_content_bound_trace_index_value,
    query_content_bound_trace_index, validate_content_bound_trace_index,
    verify_content_bound_trace_index, ContentBoundTraceIndex, ContentBoundTraceIndexEvaluation,
    ContentBoundTraceIndexIssue, ContentBoundTraceIndexVerification, ContentBoundTraceLink,
    ContentBoundTraceMarkerSpan, ContentBoundTraceQuery, ContentBoundTraceQueryEvaluation,
    ContentBoundTraceSourceIdentity, CONTENT_BOUND_TRACE_INDEX_BOUNDARY,
    CONTENT_BOUND_TRACE_INDEX_NON_CLAIMS, CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1,
    TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES, TRACEABILITY_ASSURANCE_DECLARED,
};
pub use verified::cross_repo_dependencies::{
    build_radicle_issue_receipt, cross_repo_dependency_contract, dependency_blocker_messages,
    evaluate_cross_repo_dependencies, parse_cross_repo_dependencies,
    radicle_issue_receipt_contract, summaries_by_identity, CrossRepoDependency,
    CrossRepoDependencyDiagnostic, CrossRepoDependencyReport, CrossRepoDependencyRowResult,
    CrossRepoReceiptSummary, RadicleIssueObservation, CROSS_REPO_DEPENDENCY_SCHEMA_V1,
    CROSS_REPO_NON_CLAIM_DOWNSTREAM, CROSS_REPO_NON_CLAIM_LINKAGE, CROSS_REPO_NON_CLAIM_UPSTREAM,
    CROSS_REPO_RECEIPT_SCHEMA_V1, RADICLE_ISSUE_NON_CLAIM_AUTHORITY,
    RADICLE_ISSUE_NON_CLAIM_LINKAGE, RADICLE_ISSUE_RECEIPT_KIND, RADICLE_ISSUE_RECEIPT_SCHEMA_V1,
};
pub use verified::delta_merge::{
    merge_delta_spec, DeltaMerge, DeltaMergeDiagnostic, DeltaOperation, DeltaOperationKind,
    DeltaOperationOutcome, DeltaOperationState,
};
pub use verified::developer_experience::{
    onboarding_templates, plan_prompt_scaffold, plan_reverse_spec_discovery,
    plan_template_application, DiscoveryCandidate, DiscoveryPlan, DiscoverySnapshot, FilePlan,
    FilePlanEntry, PlannedFile, ScaffoldPlan, TemplateProfile, DISCOVERY_MAX_BYTES,
    DISCOVERY_MAX_FILES, DISCOVERY_POLICY_ID, SCAFFOLD_POLICY_ID, TEMPLATE_POLICY_ID,
};
pub use verified::developer_loop::*;
pub use verified::doctor::{
    build_doctor_report, DoctorCheck, DoctorCheckStatus, DoctorReport, DoctorSnapshot,
};
pub use verified::document_collaboration::*;
pub use verified::evidence::{
    blake3_hex, canonical_json, canonical_json_hash, canonical_preserves_bytes,
    canonical_preserves_hash, preserves_content_hash,
};
pub use verified::evidence_chain_readiness::{
    evaluate_evidence_chain_readiness, evidence_chain_readiness_hash,
    function_address_evidence_chain_profile, EvidenceChainMantleLink, EvidenceChainPolicy,
    EvidenceChainPolicyIdentity, EvidenceChainProfile, EvidenceChainProjection,
    EvidenceChainReadiness, EvidenceChainReadinessInput, EvidenceChainReadinessIssue,
    EvidenceChainValenceLink, EVIDENCE_CHAIN_DISPOSITION_ABSENT,
    EVIDENCE_CHAIN_DISPOSITION_INVALID, EVIDENCE_CHAIN_DISPOSITION_PRESENT,
    EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE, EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND,
    EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1, EVIDENCE_CHAIN_MODE_OPTIONAL,
    EVIDENCE_CHAIN_MODE_REQUIRED, EVIDENCE_CHAIN_READINESS_BOUNDARY,
    EVIDENCE_CHAIN_REQUIRED_NON_CLAIM,
};
pub use verified::formal_proof_readiness_fixture::{
    validate_formal_proof_readiness_fixture, FormalProofReadinessFixture,
    FormalProofReadinessIssue, FormalProofReadinessReport, FORMAL_PROOF_READINESS_BOUNDARY,
};
pub use verified::function_address_gates::{
    evaluate_function_address_evidence, function_address_digest_rules,
    function_address_input_contracts, is_function_address_mode,
    normalize_function_address_evidence_digests, FunctionAddressDigestNormalization,
    FunctionAddressDigestRule, FunctionAddressEvidence, FunctionAddressInputContract,
    FunctionAddressIssue, FunctionAddressPolicy, FunctionAddressReadiness,
    FUNCTION_ADDRESS_BOUNDARY, FUNCTION_ADDRESS_CLAIM_SCOPE_LIFECYCLE,
    FUNCTION_ADDRESS_DIGEST_ISSUE_CASE_UNSUPPORTED, FUNCTION_ADDRESS_DIGEST_ISSUE_MALFORMED,
    FUNCTION_ADDRESS_DIGEST_ISSUE_PREFIX_UNSUPPORTED, FUNCTION_ADDRESS_DIGEST_SPELLING_B3_PREFIX,
    FUNCTION_ADDRESS_DIGEST_SPELLING_BARE, FUNCTION_ADDRESS_GRAPH_SCHEMA_V1,
    FUNCTION_ADDRESS_ISSUE_INPUT_DISPOSITION_UNSUPPORTED,
    FUNCTION_ADDRESS_ISSUE_INPUT_SCHEMA_UNSUPPORTED, FUNCTION_ADDRESS_KAMACITE_SCHEMA_V1,
    FUNCTION_ADDRESS_MANTLE_BINDING_SCHEMA_V1, FUNCTION_ADDRESS_MODE_DISABLED,
    FUNCTION_ADDRESS_MODE_OPTIONAL, FUNCTION_ADDRESS_MODE_REQUIRED,
    FUNCTION_ADDRESS_VALENCE_SCHEMA_V1,
};
pub use verified::function_address_preserves_readiness::{
    evaluate_function_address_preserves_readiness, function_address_preserves_policy,
    FunctionAddressJsonProjection, FunctionAddressPreservesInput,
    FUNCTION_ADDRESS_JSON_PROJECTION_AUTHORITY, FUNCTION_ADDRESS_JSON_PROJECTION_ROLE,
    FUNCTION_ADDRESS_MANTLE_DISPOSITION_PRESENT, FUNCTION_ADDRESS_PRESERVES_BOUNDARY,
    FUNCTION_ADDRESS_PRESERVES_ROLE, FUNCTION_ADDRESS_PRESERVES_SCHEMA_V1,
    FUNCTION_ADDRESS_VALENCE_POLICY_KIND,
};
pub use verified::function_address_stack_smoke::{
    evaluate_function_address_stack_smoke, FunctionAddressStackSmokeInput,
    FunctionAddressStackSmokeIssue, FunctionAddressStackSmokeReport,
};
pub use verified::generated_agent_workflow::*;
pub use verified::global_registry::{
    build_global_registry_snapshot, empty_global_registry_snapshot, evaluate_registry_graph,
    global_registry_snapshot_hash, merge_global_registry_snapshot, query_registry_exact_id,
    query_registry_tags, verify_global_registry_snapshot, GlobalRegistrySnapshot,
    RegistryAdapterMetadata, RegistryCandidateRecords, RegistryCorePolicy, RegistryEdge,
    RegistryGraphDirection, RegistryGraphProfile, RegistryGraphReport, RegistryIssue, RegistryNode,
    RegistryQueryReport, RegistrySnapshotReport, RegistrySourceRecord, RegistryTagAssignment,
    EMPTY_REGISTRY_SNAPSHOT_HASH, GLOBAL_REGISTRY_SCHEMA,
};
pub use verified::hash_algorithm_policy::{
    hash_algorithm_policy_issues_json, validate_hash_algorithm_policy,
    validate_hash_algorithm_policy_value, HashAlgorithmException, HashAlgorithmPolicy,
    HashAlgorithmPolicyIssue, HashAlgorithmPolicyReport, HashFieldObservation,
    HASH_ALGORITHM_POLICY_BOUNDARY, HASH_ALGORITHM_POLICY_SCHEMA_V1,
};
pub use verified::kani_receipt::{
    kani_receipt_hash, validate_kani_receipt, KaniReceiptIssue, KaniReceiptProfile,
    KaniReceiptReport, KANI_NON_CLAIM_BOUNDED_ONLY, KANI_NON_CLAIM_FFI_STUB,
    KANI_NON_CLAIM_VERIFIER_SOUNDNESS, KANI_NON_CLAIM_WHOLE_PROGRAM, KANI_RECEIPT_SCHEMA_V1,
};
pub use verified::lifecycle::{
    archive_dir, build_cairn_migration_plan, build_change_skeleton, build_change_skeleton_for,
    build_ironspec_migration_plan, build_openspec_migration_plan, changes_dir, design_template,
    init_artifacts, init_artifacts_for, layout_name, layout_root, migration_plan_identity,
    proposal_template, specs_dir, tasks_template, validate_change_name, ChangeSkeleton,
    LifecycleArtifact, LifecycleArtifactKind, LifecycleLayout, MigrationAction, MigrationInput,
    MigrationPlan,
};
pub use verified::lifecycle_orchestration::{
    validate_steel_action_envelope, SteelActionIssue, SteelActionValidationReport,
    STEEL_ACTION_ENVELOPE_SCHEMA_V1, STEEL_ORCHESTRATION_RECEIPT_SCHEMA_V1,
};
pub use verified::lifecycle_receipt::{
    adapt_cairn_gate_receipt, adapt_cairn_sync_archive_receipt, adapt_cairn_validate_receipt,
    adapt_downstream_lifecycle_receipt, adapt_lifecycle_receipt_value,
    default_lifecycle_receipt_hash_profile, default_lifecycle_receipt_non_claims,
    lifecycle_receipt_envelope_from_value, lifecycle_receipt_hash, lifecycle_receipt_hash_material,
    validate_lifecycle_receipt_envelope, LifecycleReceiptCommandIdentity,
    LifecycleReceiptDiagnostic, LifecycleReceiptEnvelope, LifecycleReceiptEvidenceSummary,
    LifecycleReceiptHashField, LifecycleReceiptHashProfile, LifecycleReceiptIssue,
    LifecycleReceiptValidationInput, LifecycleReceiptValidationReport,
    LIFECYCLE_RECEIPT_CORE_BOUNDARY, LIFECYCLE_RECEIPT_CORE_SCHEMA_V1,
    LIFECYCLE_RECEIPT_HASH_FIELD_INPUT_HASH, LIFECYCLE_RECEIPT_HASH_FIELD_POLICY_HASH,
    LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH, LIFECYCLE_RECEIPT_HASH_PROFILE_CORE_V1,
    LIFECYCLE_RECEIPT_ISSUE_HASH_PROFILE, LIFECYCLE_RECEIPT_ISSUE_MALFORMED_HASH,
    LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC, LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
    LIFECYCLE_RECEIPT_ISSUE_MISSING_HASH, LIFECYCLE_RECEIPT_ISSUE_OVERCLAIM,
    LIFECYCLE_RECEIPT_ISSUE_SELF_HASH, LIFECYCLE_RECEIPT_ISSUE_STALE_HASH,
    LIFECYCLE_RECEIPT_ISSUE_UNSUPPORTED_VERDICT, LIFECYCLE_RECEIPT_ISSUE_WEAK_NON_CLAIM,
};
pub use verified::lifecycle_store_context::{
    build_lifecycle_store_context, normalize_direct_references, resolve_lifecycle_root,
    store_qualified_spec_fetch_command, AcceptedSpecMetadata, AcceptedSpecSnapshot,
    ContextBoundary, ContextObservations, ContextResult, ContextSpecResult,
    DirectReferenceNormalization, DirectStoreReference, DoctorFindingStatus,
    ExplicitLifecycleStoreSelector, ExplicitRootSelector, FallbackFacts, LifecycleAuthorityEffect,
    LifecycleStoreDoctorFinding, LocatorSnapshot, LocatorStoreObservation, NativeRootCandidate,
    ProjectPointerObservation, ReferenceContentBoundary, ReferenceTraversalBoundary,
    RegistryLookupBoundary, ResolvedLifecycleRoot, RootResolutionObservations,
    RootResolutionResult, RootResolutionSource, StoreIdentityObservation, VcsBoundaryFacts,
    REFERENCE_AVAILABLE, REFERENCE_DUPLICATE, REFERENCE_SELF, REFERENCE_SNAPSHOT_AMBIGUOUS,
    REFERENCE_SNAPSHOT_MISSING, REFERENCE_SNAPSHOT_STALE, REFERENCE_SPEC_DUPLICATE,
    REFERENCE_STORE_AMBIGUOUS, REFERENCE_STORE_MISSING, REFERENCE_STORE_STALE,
    ROOT_PROJECT_POINTER_STALE, ROOT_SELECTOR_CONFLICT, ROOT_STORE_AMBIGUOUS, ROOT_STORE_MISSING,
    ROOT_STORE_STALE,
};
pub use verified::lifecycle_store_identity::{
    admit_lifecycle_store_identity, admit_lifecycle_store_locator,
    apply_lifecycle_store_locator_plan, empty_lifecycle_store_locator,
    lifecycle_store_identity_material, lifecycle_store_locator_plan_hash,
    lifecycle_store_locator_portable_hash, lifecycle_store_locator_projection,
    lifecycle_store_source_identity, plan_lifecycle_store_registration,
    plan_lifecycle_store_unregistration, LifecycleStoreBlake3Identity, LifecycleStoreCheckoutPath,
    LifecycleStoreDiagnostic, LifecycleStoreId, LifecycleStoreIdentity,
    LifecycleStoreIdentityAdmission, LifecycleStoreIdentityProjection,
    LifecycleStoreLocatorAdmission, LifecycleStoreLocatorApplication,
    LifecycleStoreLocatorLocalFacts, LifecycleStoreLocatorMapping,
    LifecycleStoreLocatorMappingProjection, LifecycleStoreLocatorOperation,
    LifecycleStoreLocatorPlan, LifecycleStoreLocatorPortableFacts, LifecycleStoreLocatorProjection,
    LifecycleStoreLocatorState, LifecycleStoreNonClaim, LifecycleStoreRemoteHint,
    BLAKE3_IDENTITY_HEX_LENGTH, LIFECYCLE_STORE_CHECKOUT_PATH_MAX_BYTES,
    LIFECYCLE_STORE_DIAGNOSTIC_MAX_COUNT, LIFECYCLE_STORE_IDENTITY_NON_CLAIMS,
    LIFECYCLE_STORE_IDENTITY_OBSERVATION_NON_CLAIM, LIFECYCLE_STORE_IDENTITY_READINESS_NON_CLAIM,
    LIFECYCLE_STORE_IDENTITY_REMOTE_NON_CLAIM, LIFECYCLE_STORE_IDENTITY_SCHEMA_V1,
    LIFECYCLE_STORE_ID_MAX_BYTES, LIFECYCLE_STORE_LOCAL_EVIDENCE_CLASS,
    LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS, LIFECYCLE_STORE_LOCATOR_NON_CLAIM,
    LIFECYCLE_STORE_LOCATOR_PLAN_SCHEMA_V1, LIFECYCLE_STORE_LOCATOR_SCHEMA_V1,
    LIFECYCLE_STORE_NON_CLAIMS_MAX_COUNT, LIFECYCLE_STORE_NON_CLAIM_MAX_BYTES,
    LIFECYCLE_STORE_PORTABLE_FACT_CLASS, LIFECYCLE_STORE_REMOTE_HINT_MAX_BYTES,
    LIFECYCLE_STORE_REQUIRED_NON_CLAIMS,
};
pub use verified::lifecycle_worksets::{
    apply_workset_transition, opener_profile_identity, plan_workset_create, plan_workset_open,
    plan_workset_remove, recheck_workset_open_plan, workset_identity, workset_open_plan_identity,
    workset_portable_identity, workset_portable_state_identity, workset_state_identity,
    workset_transition_plan_identity, Workset, WorksetArtifactEffect, WorksetArtifactPlan,
    WorksetArtifactRole, WorksetCreateRequest, WorksetError, WorksetExecutionMode, WorksetId,
    WorksetLocalMemberFact, WorksetLocalReceiptFacts, WorksetMember, WorksetMemberLabel,
    WorksetOpenPlan, WorksetOpenRequest, WorksetOpenerPolicy, WorksetOpenerProfile,
    WorksetOpenerProfileId, WorksetPlanAction, WorksetPortableArg, WorksetReceiptOperation,
    WorksetRemoveRequest, WorksetState, WorksetTransitionOperation, WorksetTransitionPlan,
    BLAKE3_HEX_BYTES, MAX_ARG_BYTES, MAX_DISPLAY_PATH_BYTES, MAX_EXECUTABLE_BYTES, MAX_FIXED_ARGS,
    MAX_OPENER_PROFILES, MAX_OPENER_PROFILE_ID_BYTES, MAX_WORKSETS, MAX_WORKSET_ID_BYTES,
    MAX_WORKSET_MEMBERS, MAX_WORKSET_MEMBER_LABEL_BYTES, WORKSET_EVIDENCE_CLASS_LOCAL_ONLY,
    WORKSET_LOCAL_RECEIPT_SCHEMA_V1, WORKSET_NON_CLAIM_EDITOR_STATE,
    WORKSET_NON_CLAIM_LIFECYCLE_AUTHORITY, WORKSET_NON_CLAIM_MEMBER_EFFECTS,
    WORKSET_OPEN_PLAN_SCHEMA_V1, WORKSET_STATE_LOGICAL_PATH, WORKSET_STATE_SCHEMA_V1,
    WORKSET_TRANSITION_PLAN_SCHEMA_V1,
};
pub use verified::nominal_identity::{
    nominal_identity_policy_diagnostics, nominal_octet_projection_hash,
    nominal_runtime_projection_hash, ActorId, ActorIdRole, AnyIdentity, ArchiveDigest,
    ArchiveDigestRole, ArchiveId, ArchiveIdRole, ArtifactDigest, ArtifactDigestRole, BinaryDigest,
    BinaryDigestRole, Blake3Digest, Blake3DigestRef, ChainDigest, ChainDigestRole, ChangeId,
    ChangeIdRole, ContentRootDigest, ContentRootDigestRole, DigestProfile, DigestRoleMarker,
    DigestSpelling, DomainId, DomainIdRef, DomainRoleMarker, EvidenceKind, EvidenceKindRole,
    GitOidSha1Digest, GitOidSha1DigestRole, GraphDigest, GraphDigestRole, IdentityIssue,
    IdentityRole, IdentityWire, ImportedDigest, ImportedDigestAlgorithm, ImportedDigestRoleMarker,
    InputDigest, InputDigestRole, LedgerDigest, LedgerDigestRole, MantleDigest, MantleDigestRole,
    NominalIdentityDomainPolicy, NominalIdentityOctetDomain, NominalIdentityOctetProjection,
    NominalIdentityPolicy, NominalIdentityRuntimeProjection, OperationId, OperationIdRole,
    PackDigest, PackDigestRole, PlanDigest, PlanDigestRole, PolicyDigest, PolicyDigestRole,
    PolicyKind, PolicyKindRole, ProjectionRole, ProjectionRoleMarker, ReceiptDigest,
    ReceiptDigestRole, Sha256Digest, Sha256DigestRole, SidecarDigest, SidecarDigestRole,
    SourceDigest, SourceDigestRole, StateDigest, StateDigestRole, ValenceDigest, ValenceDigestRole,
    WorkspaceId, WorkspaceIdRole, MIXED_BLAKE3_PROFILE, NOMINAL_IDENTITY_OCTET_SCHEMA_V1,
    NOMINAL_IDENTITY_RUNTIME_SCHEMA_V1, RAW_BLAKE3_PROFILE, STRICT_B3_PROFILE,
};
pub use verified::oxide_lifecycle_patterns::{
    positive_oxide_lifecycle_patterns_fixture, validate_oxide_lifecycle_patterns_value,
    EvidenceBundleMember, GoldenSnapshotPlan, LifecycleDocumentPlan, LifecycleSagaStep,
    OxideLifecyclePatternBundle, OxideLifecyclePatternIssue, OxideLifecyclePatternReport,
    OxideReferenceRow, OXIDE_LIFECYCLE_PATTERN_BOUNDARY, OXIDE_LIFECYCLE_PATTERN_SCHEMA_V1,
};
pub use verified::plan::{
    build_archive_plan, build_archive_plan_for_effect_with_dependency_blockers,
    build_archive_plan_with_dependency_blockers, build_sync_plan,
    build_sync_plan_for_effect_with_dependency_blockers, build_sync_plan_with_dependency_blockers,
    ArchivePlan, PlanAction, PlanActionKind, SyncPlan,
};
pub use verified::policy::{
    classify_policy_schema_compatibility, default_policy_schema_metadata, parse_policy_json,
    policy_schema_compatibility_diagnostic, policy_schema_metadata, validate_policy_json,
    ArtifactSubstancePolicy, LifecycleStoreBounds, LifecycleStoreLocalStateSchema,
    LifecycleStoreOpenerProfile, LifecycleStorePolicy, LifecycleStoreProjectionPolicy,
    LifecycleStoreReceiptPolicy, LifecycleStoreRootResolutionPolicy, LifecycleStoreWorksetPolicy,
    Policy, PolicyError, PolicySchemaCompatibilityMetadata, PolicySchemaCompatibilityReport,
    SteelOrchestrationPolicy, SteelOrchestrationProfile, TraceabilityPolicy,
    CURRENT_POLICY_FIELD_SET_DIGEST, CURRENT_POLICY_FIELD_SET_DIGEST_MATERIAL,
    CURRENT_POLICY_SCHEMA_VERSION, DEFAULT_POLICY_REFRESH_COMMAND,
    LIFECYCLE_STORE_POLICY_SCHEMA_V1, POLICY_SCHEMA_METADATA_FIELD, SUPPORTED_POLICY_VERSION,
    TASK_IDENTITY_MARKER_ID, TASK_PREDECESSOR_MARKER_ID,
};
pub use verified::policy_exception::*;
pub use verified::preserves_evidence_ingest::{
    default_preserves_evidence_non_claims, measure_preserves_evidence_envelope_bytes,
    preserves_evidence_ingest_contracts, validate_preserves_evidence_ingest,
    PreservesEvidenceEnvelopeMeasurement, PreservesEvidenceEnvelopeRow,
    PreservesEvidenceIngestContract, PreservesEvidenceIngestInput, PreservesEvidenceIngestIssue,
    PreservesEvidenceIngestReport, PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT,
    PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1, PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE,
    PRESERVES_EVIDENCE_INGEST_BOUNDARY, PRESERVES_EVIDENCE_INGEST_SCHEMA_V1,
    PRESERVES_EVIDENCE_ISSUE_ADAPTER_FOR_OPAQUE, PRESERVES_EVIDENCE_ISSUE_ADAPTER_REQUIRED,
    PRESERVES_EVIDENCE_ISSUE_EMPTY_FIELD, PRESERVES_EVIDENCE_ISSUE_MALFORMED_DIGEST,
    PRESERVES_EVIDENCE_ISSUE_MEASUREMENT_ERROR, PRESERVES_EVIDENCE_ISSUE_NON_CANONICAL_BYTES,
    PRESERVES_EVIDENCE_ISSUE_OVERCLAIM, PRESERVES_EVIDENCE_ISSUE_STALE_DIGEST,
    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ADAPTER, PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_CONTRACT,
    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_OPACITY, PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE,
    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCHEMA, PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCOPE,
    PRESERVES_EVIDENCE_ISSUE_WEAK_NON_CLAIM, PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED,
    PRESERVES_EVIDENCE_OPACITY_OPAQUE, PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1,
    PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED, PRESERVES_EVIDENCE_ROLE_OPAQUE,
};
pub use verified::probe::{
    all_probe_assertions_passed, evaluate_probe_assertions, probe_declaration_hash,
    probe_issue_json, probe_receipt_hash, validate_probe_completion_status,
    validate_probe_declaration, validate_probe_receipt, ProbeIssue, PROBE_DECLARATION_SCHEMA_V1,
    PROBE_RECEIPT_SCHEMA_V1,
};
pub use verified::radicle_change::{
    apply_change_cob_operation, change_cob_action_json, change_cob_content_root,
    change_cob_lifecycle_view, change_cob_state_hash, current_change_cob_evidence,
    decode_base_revision, initialize_change_cob, materialize_change_cob, validate_change_cob_init,
    validate_change_cob_state, ChangeArtifactId, ChangeBaseRevision, ChangeCobAction,
    ChangeCobError, ChangeCobEvidence, ChangeCobInit, ChangeCobOperation, ChangeCobState,
    ChangeLifecycleRecord, ChangeLifecycleStatus, ChangeLifecycleView, ChangeMaterializedFile,
    ChangeRevision, ChangeRevisionSet, ARTIFACT_DESIGN, ARTIFACT_METADATA, ARTIFACT_PROPOSAL,
    ARTIFACT_SPEC_PREFIX, ARTIFACT_TASKS, BLAKE3_HEX_LENGTH, CHANGE_COLLABORATION_SCHEMA_V1,
    GIT_SHA1_HEX_LENGTH, GIT_SHA256_HEX_LENGTH, MAX_ACTIONS_PER_OPERATION, MAX_ACTOR_ID_BYTES,
    MAX_ARTIFACTS, MAX_ARTIFACT_BYTES, MAX_ARTIFACT_ID_BYTES, MAX_BASE_REVISION_KIND_BYTES,
    MAX_BASE_REVISION_VALUE_BYTES, MAX_CHANGE_ID_BYTES, MAX_DELEGATES, MAX_EVIDENCE_NON_CLAIMS,
    MAX_EVIDENCE_RECORDS, MAX_EVIDENCE_TEXT_BYTES, MAX_LIFECYCLE_EVIDENCE_IDS,
    MAX_LIFECYCLE_REVISIONS, MAX_OPERATIONS, MAX_OPERATION_ID_BYTES, MAX_REPOSITORY_ID_BYTES,
    MAX_REVISIONS_PER_ARTIFACT, MAX_REVISION_BASIS_IDS, MAX_WORKSPACE_ID_BYTES,
    RADICLE_CHANGE_COB_SCHEMA_V1, RADICLE_CHANGE_COB_TYPE_NAME,
};
pub use verified::radicle_checkout::{
    plan_change_cob_checkout, ChangeCheckoutDestination, ChangeCheckoutFile, ChangeCobCheckoutPlan,
    ChangeCobLocalSnapshot,
};
pub use verified::radicle_import::{
    change_cob_import_content_root, plan_change_cob_import, ChangeCobImportSnapshot,
};
pub use verified::receipt_drift::{receipt_field_drift, DriftClass, DriftEntry, DriftReport};
pub use verified::release_decision::*;
pub use verified::release_run_ledger::{
    positive_release_run_ledger_fixture, release_run_explain, release_run_identity,
    release_run_ledger_hash, release_run_status, validate_release_run_ledger_value,
    ReleaseRunArtifact, ReleaseRunBlocker, ReleaseRunCommandIdentity, ReleaseRunDiagnostic,
    ReleaseRunExplainProjection, ReleaseRunLedger, ReleaseRunLedgerIssue, ReleaseRunLedgerReport,
    ReleaseRunResumeHint, ReleaseRunStatusProjection, ReleaseRunStep, RELEASE_RUN_LEDGER_BOUNDARY,
    RELEASE_RUN_LEDGER_SCHEMA_V1, RELEASE_RUN_STATE_BLOCKED, RELEASE_RUN_STATE_FAILED,
    RELEASE_RUN_STATE_PASSED, RELEASE_RUN_STATE_PENDING, RELEASE_RUN_STATE_RUNNING_OBSERVED,
    RELEASE_RUN_STATE_SKIPPED, RELEASE_RUN_STATE_SUPERSEDED,
};
pub use verified::repo::{
    build_change_detail, build_change_list, build_change_status, build_repo_status,
    build_spec_detail, build_spec_list, count_tasks, filter_changes_by_metadata, spec_title,
    ChangeDetail, ChangeList, ChangeMetadata, ChangeSource, ChangeStatus, RepoStatus, SpecDetail,
    SpecList, SpecSource, SpecSummary, TaskCounts,
};
pub use verified::requirement_id_registry::{
    requirement_id_registry_issues_json, validate_requirement_id_registry,
    validate_requirement_id_registry_value, RequirementIdAlias, RequirementIdRegistry,
    RequirementIdRegistryIssue, RequirementIdRegistryReport, RequirementReference,
    RequirementRegistryRow, REQUIREMENT_ID_REGISTRY_BOUNDARY, REQUIREMENT_ID_REGISTRY_SCHEMA_V1,
};
pub use verified::runtime_evidence::{
    is_runtime_capture_origin, runtime_record_hash, verify_runtime_evidence,
    witness_checkpoint_hash, CaptureOriginSummary, RuntimeEvidenceIssue, RuntimeEvidencePolicy,
    RuntimeEvidenceReport, CAPTURE_ORIGIN_BOUNDARY, CAPTURE_ORIGIN_INGESTED,
    CAPTURE_ORIGIN_UNKNOWN, CAPTURE_ORIGIN_WRAPPER_ASSERTED,
    COMPLETENESS_ALTERED_BEFORE_CHECKPOINT, COMPLETENESS_CHECKPOINT_MALFORMED,
    COMPLETENESS_INTEGRITY_ONLY, COMPLETENESS_SUBJECT_MISMATCH, COMPLETENESS_TRUNCATED,
    COMPLETENESS_WITNESSED_PREFIX, DEFAULT_RUNTIME_SUMMARY_MAX_BYTES, RUNTIME_EVIDENCE_BOUNDARY,
    RUNTIME_EVIDENCE_SCHEMA_V1, RUNTIME_HASH_ALGORITHM_BLAKE3,
};
pub use verified::semantic_review::{
    completeness_receipt, issues_json as semantic_review_issues_json, review_ledger_analytics,
    review_receipt_hash, semantic_review_policy_profile_json,
    validate_review_metrics_evidence_boundary, validate_review_metrics_fixture_pairs,
    validate_review_metrics_guidance_sync, validate_review_metrics_registry,
    validate_review_receipt, validate_revision_handoff, SemanticReviewIssue, SemanticReviewReport,
    REVIEW_ANALYTICS_SCHEMA_V1, REVIEW_CATEGORIES, REVIEW_COMPLETENESS_SCHEMA_V1, REVIEW_DECISIONS,
    REVIEW_KINDS, REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1,
    REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1, REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1,
    REVIEW_METRICS_REGRESSION_SCHEMA_V1, REVIEW_RECEIPT_SCHEMA_V1, REVIEW_SEVERITIES,
    REVIEW_SOURCES, REVISION_DISPOSITIONS, REVISION_HANDOFF_SCHEMA_V1,
};
pub use verified::stack_provenance::{
    evaluate_stack_provenance, is_blake3_hex, is_stack_provenance_mode, StackProvenanceEvidence,
    StackProvenanceIssue, StackProvenancePolicy, StackProvenanceReadiness,
    STACK_PROVENANCE_BOUNDARY, STACK_PROVENANCE_CLAIM_SCOPE_LIFECYCLE,
    STACK_PROVENANCE_MODE_DISABLED, STACK_PROVENANCE_MODE_OPTIONAL, STACK_PROVENANCE_MODE_REQUIRED,
};
pub use verified::state_machine_receipt::{
    positive_state_machine_receipt_fixture, state_machine_receipt_hash,
    validate_state_machine_receipt_value, StateMachineArtifact, StateMachineChecker,
    StateMachineDiagnostic, StateMachineNamedHash, StateMachineReceipt, StateMachineReceiptIssue,
    StateMachineReceiptReport, STATE_MACHINE_RECEIPT_BOUNDARY, STATE_MACHINE_RECEIPT_SCHEMA_V1,
    STATE_MACHINE_RESULT_PASSED, STATE_MACHINE_VERIFICATION_ROLE_BOUNDED_WITNESS,
};
pub use verified::structured_lifecycle::{
    structured_lifecycle_bundle_hash, structured_lifecycle_issue_json,
    validate_structured_lifecycle_bundle, StructuredLifecycleIssue, StructuredLifecycleReport,
    STRUCTURED_LIFECYCLE_SCHEMA_V1,
};
pub use verified::toolchain_profile_matrix::{
    toolchain_profile_issues_json, validate_toolchain_profile_matrix,
    validate_toolchain_profile_matrix_value, ToolchainProfileIssue, ToolchainProfileMatrix,
    ToolchainProfileReport, ToolchainProfileRow, TOOLCHAIN_PROFILE_MATRIX_BOUNDARY,
    TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1,
};
pub use verified::traceability::{
    adapt_cairn_traceability_markers, adapt_octet_traceability_markers,
    adapt_valence_traceability_markers, classify_traceability_coverage,
    default_traceability_marker_vocabulary, traceability_id_is_valid, traceability_ref_markers,
    traceability_spec_marker, unique_sorted_strings, validate_traceability_marker_links,
    TraceabilityCoverageReport, TraceabilityFile, TraceabilityMarkerIssue, TraceabilityMarkerLink,
    TraceabilityMarkerValidationInput, TraceabilityMarkerValidationReport, TraceabilityProfile,
    TraceabilityReferenceMarker, TraceabilityReferenceOccurrence,
    TraceabilityRequirementOccurrence, TraceabilitySource, TRACEABILITY_COVERAGE_VERBS,
    TRACEABILITY_MARKER_CORE_BOUNDARY, TRACEABILITY_MARKER_NON_CLAIMS,
    TRACEABILITY_MARKER_VERB_REQUIREMENT, TRACEABILITY_PROFILE_CAIRN_DEFAULT,
    TRACEABILITY_PROFILE_TRELLIS_RUST, TRACEABILITY_VERB_DEPENDS, TRACEABILITY_VERB_IMPL,
    TRACEABILITY_VERB_RELATED, TRACEABILITY_VERB_VERIFY,
};
pub use verified::trellis_proof_readiness::{
    evaluate_trellis_proof_readiness, trellis_proof_evidence_chain_profile,
    trellis_proof_readiness_hash, TrellisProofInput, TrellisProofIssue, TrellisProofPolicy,
    TrellisProofReadiness, TRELLIS_PROOF_CLAIM_SCOPE, TRELLIS_PROOF_EVIDENCE_KIND,
    TRELLIS_PROOF_MODE_OPTIONAL, TRELLIS_PROOF_MODE_REQUIRED_FORMAL,
    TRELLIS_PROOF_MODE_REQUIRED_RECORDED, TRELLIS_PROOF_PROFILE_V1,
    TRELLIS_PROOF_REQUIRED_NON_CLAIM, TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL,
    TRELLIS_PROOF_ROLE_RECORDED,
};
pub use verified::validation::{
    build_task_ordering_graph, change_issues, classify_gate_evidence, evaluate_gate,
    evaluate_gate_snapshot, evaluate_lifecycle_substance, evaluate_profile_gate_snapshot,
    finalize_gate_report, lifecycle_substance_issues, validate_changes,
    validate_changes_with_archives, validate_spec_source, validate_spec_source_with_policy,
    validate_spec_sources, validate_spec_sources_with_policy, validation_issue_code,
    ArtifactSubstanceInventory, EvidenceStrengthSummary, GateArtifact, GateIssue, GateReport,
    GateSnapshot, GateStage, LifecycleSubstanceReport, SpecSubstanceInventory, TaskOrderingEdge,
    TaskOrderingGraph, TaskOrderingIssue, TaskOrderingNode, TaskOrderingState,
    TASK_ORDER_ISSUE_CYCLE, TASK_ORDER_ISSUE_ID_DUPLICATE, TASK_ORDER_ISSUE_ID_MALFORMED,
    TASK_ORDER_ISSUE_ID_MISSING, TASK_ORDER_ISSUE_ID_MULTIPLE, TASK_ORDER_ISSUE_MARKER_POLICY,
    TASK_ORDER_ISSUE_PREDECESSOR_DUPLICATE, TASK_ORDER_ISSUE_PREDECESSOR_MALFORMED,
    TASK_ORDER_ISSUE_PREDECESSOR_SELF, TASK_ORDER_ISSUE_PREDECESSOR_UNKNOWN,
    TASK_ORDER_ISSUE_PROGRESS,
};
pub use verified::workflow_collaboration::*;
pub use verified::workspace_contract_matrix::{
    evaluate_workspace_contract_matrix_readiness, is_workspace_contract_mode,
    validate_workspace_contract_matrix, validate_workspace_contract_matrix_value,
    workspace_contract_issues_json, workspace_contract_readiness_issues_json,
    workspace_contract_readiness_policy_from_value, WorkspaceArtifactObservation,
    WorkspaceContractEdge, WorkspaceContractIssue, WorkspaceContractMatrix,
    WorkspaceContractReadiness, WorkspaceContractReadinessPolicy, WorkspaceContractReport,
    WORKSPACE_CONTRACT_MATRIX_BOUNDARY, WORKSPACE_CONTRACT_MATRIX_RECEIPT_SCHEMA_V1,
    WORKSPACE_CONTRACT_MATRIX_SCHEMA_V1, WORKSPACE_CONTRACT_MODE_DISABLED,
    WORKSPACE_CONTRACT_MODE_OPTIONAL, WORKSPACE_CONTRACT_MODE_REQUIRED,
};
