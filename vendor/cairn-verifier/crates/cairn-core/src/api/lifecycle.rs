macro_rules! root_function_compatibility {
    ($module:ident; $($local:ident => $root:ident => $reason:literal;)+) => {$(
        #[cfg_attr(dylint_lib = "octet", allow(renamed_imports, reason = $reason))]
        pub use crate::verified::$module::$local as $root;
    )+};
}

macro_rules! root_type_compatibility {
    ($module:ident; $($local:ident => $root:ident => $reason:literal;)+) => {$(
        #[cfg_attr(dylint_lib = "octet", allow(renamed_imports, reason = $reason))]
        pub use crate::verified::$module::$local as $root;
    )+};
}

pub use crate::verified::archive_compaction::{
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
pub use crate::verified::change_dependencies::{
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
pub use crate::verified::delta_merge::{
    merge_delta_spec, DeltaMerge, DeltaMergeDiagnostic, DeltaOperation, DeltaOperationKind,
    DeltaOperationOutcome, DeltaOperationState,
};
pub use crate::verified::doctor::{
    build_doctor_report, DoctorCheck, DoctorCheckStatus, DoctorReport, DoctorSnapshot,
};
pub use crate::verified::lifecycle::{
    archive_dir, build_cairn_migration_plan, build_change_skeleton, build_change_skeleton_for,
    build_ironspec_migration_plan, build_openspec_migration_plan, changes_dir, design_template,
    init_artifacts, init_artifacts_for, layout_name, layout_root, migration_plan_identity,
    proposal_template, specs_dir, tasks_template, validate_change_name, ChangeSkeleton,
    LifecycleArtifact, LifecycleArtifactKind, LifecycleLayout, MigrationAction, MigrationInput,
    MigrationPlan,
};
pub use crate::verified::lifecycle_orchestration::{
    validate_steel_action_envelope, SteelActionIssue, SteelActionValidationReport,
    STEEL_ACTION_ENVELOPE_SCHEMA_V1, STEEL_ORCHESTRATION_RECEIPT_SCHEMA_V1,
};
pub use crate::verified::lifecycle_receipt::{
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
pub use crate::verified::lifecycle_store_context::{
    normalize_direct_references, AcceptedSpecMetadata, AcceptedSpecSnapshot,
    DirectReferenceNormalization, DoctorFindingStatus, ExplicitRootSelector, FallbackFacts,
    LocatorSnapshot, NativeRootCandidate, ProjectPointerObservation, ReferenceContentBoundary,
    ReferenceTraversalBoundary, RegistryLookupBoundary, RootResolutionObservations,
    RootResolutionResult, RootResolutionSource, VcsBoundaryFacts, REFERENCE_AVAILABLE,
    REFERENCE_DUPLICATE, REFERENCE_SELF, REFERENCE_SNAPSHOT_AMBIGUOUS, REFERENCE_SNAPSHOT_MISSING,
    REFERENCE_SNAPSHOT_STALE, REFERENCE_SPEC_DUPLICATE, REFERENCE_STORE_AMBIGUOUS,
    REFERENCE_STORE_MISSING, REFERENCE_STORE_STALE, ROOT_PROJECT_POINTER_STALE,
    ROOT_SELECTOR_CONFLICT, ROOT_STORE_AMBIGUOUS, ROOT_STORE_MISSING, ROOT_STORE_STALE,
};
pub use crate::verified::readiness::{
    admit_workflow_readiness_policy, select_drain_readiness, workflow_readiness_policy,
    DrainChangeSelection, DrainInstructionRoute, DrainNextTask, DrainReadinessChange,
    DrainReadinessSelection, DrainSelectedAction, DrainTaskOrdering, WorkflowReadinessBounds,
    WorkflowReadinessInstruction, WorkflowReadinessPolicy, WORKFLOW_READINESS_BOUND,
    WORKFLOW_READINESS_POLICY_BOUNDS, WORKFLOW_READINESS_POLICY_INVALID,
};
#[rustfmt::skip]
root_type_compatibility! {
    lifecycle_store_context;
    ExplicitSelector => ExplicitLifecycleStoreSelector => "Root explicit store selector.";
    IdentityObservation => StoreIdentityObservation => "Root store identity observation.";
    LocatorObservation => LocatorStoreObservation => "Root locator store observation.";
    ResolvedRoot => ResolvedLifecycleRoot => "Root resolved lifecycle root.";
    DoctorFinding => LifecycleStoreDoctorFinding => "Root lifecycle store finding.";
    DirectReference => DirectStoreReference => "Root direct store reference.";
    Observations => ContextObservations => "Root context observations.";
    SpecResult => ContextSpecResult => "Root context spec result.";
    AuthorityEffect => LifecycleAuthorityEffect => "Root lifecycle authority effect.";
    Boundary => ContextBoundary => "Root context boundary.";
    BuildResult => ContextResult => "Root context result.";
}
#[rustfmt::skip]
root_function_compatibility! {
    lifecycle_store_context;
    resolve_root => resolve_lifecycle_root => "Root lifecycle root resolution.";
    build => build_lifecycle_store_context => "Root lifecycle store context.";
    qualified_fetch_command => store_qualified_spec_fetch_command => "Root qualified spec fetch command.";
}
#[rustfmt::skip]
root_type_compatibility! {
    lifecycle_store_identity;
    Admission => LifecycleStoreIdentityAdmission => "Root identity admission type.";
    Admitted => LifecycleStoreIdentity => "Root identity type.";
    CheckoutPath => LifecycleStoreCheckoutPath => "Root checkout path type.";
    Diagnostic => LifecycleStoreDiagnostic => "Root diagnostic type.";
    Digest => LifecycleStoreBlake3Identity => "Root BLAKE3 identity type.";
    Id => LifecycleStoreId => "Root store ID type.";
    LocatorAdmission => LifecycleStoreLocatorAdmission => "Root locator admission type.";
    LocatorApplication => LifecycleStoreLocatorApplication => "Root locator application type.";
    LocatorLocalFacts => LifecycleStoreLocatorLocalFacts => "Root locator local facts type.";
    LocatorOperation => LifecycleStoreLocatorOperation => "Root locator operation type.";
    LocatorPlan => LifecycleStoreLocatorPlan => "Root locator plan type.";
    LocatorPortableFacts => LifecycleStoreLocatorPortableFacts => "Root locator portable facts type.";
    LocatorProjection => LifecycleStoreLocatorProjection => "Root locator projection type.";
    LocatorState => LifecycleStoreLocatorState => "Root locator state type.";
    Mapping => LifecycleStoreLocatorMapping => "Root locator mapping type.";
    MappingProjection => LifecycleStoreLocatorMappingProjection => "Root locator mapping projection type.";
    NonClaim => LifecycleStoreNonClaim => "Root non-claim type.";
    Projection => LifecycleStoreIdentityProjection => "Root identity projection type.";
    RemoteHint => LifecycleStoreRemoteHint => "Root remote hint type.";
}
pub use crate::verified::lifecycle_store_identity::{
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
#[rustfmt::skip]
root_function_compatibility! {
    lifecycle_store_identity;
    admit => admit_lifecycle_store_identity => "Root identity admission.";
    admit_locator => admit_lifecycle_store_locator => "Root locator admission.";
    apply_locator_plan => apply_lifecycle_store_locator_plan => "Root locator plan application.";
    empty_locator => empty_lifecycle_store_locator => "Root empty locator.";
    locator_projection => lifecycle_store_locator_projection => "Root locator projection.";
    material => lifecycle_store_identity_material => "Root identity material.";
    plan_hash => lifecycle_store_locator_plan_hash => "Root locator plan hash.";
    plan_registration => plan_lifecycle_store_registration => "Root registration plan.";
    plan_unregistration => plan_lifecycle_store_unregistration => "Root unregistration plan.";
    portable_hash => lifecycle_store_locator_portable_hash => "Root portable hash.";
    source_digest => lifecycle_store_source_identity => "Root source digest.";
}

pub use crate::verified::lifecycle_worksets::{
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
pub use crate::verified::plan::{
    build_archive_plan, build_archive_plan_for_effect_with_dependency_blockers,
    build_archive_plan_with_dependency_blockers, build_sync_plan,
    build_sync_plan_for_effect_with_dependency_blockers, build_sync_plan_with_dependency_blockers,
    ArchivePlan, PlanAction, PlanActionKind, SyncPlan,
};
pub use crate::verified::repo::{
    build_change_detail, build_change_list, build_change_status, build_repo_status,
    build_spec_detail, build_spec_list, count_tasks, filter_changes_by_metadata, spec_title,
    ChangeDetail, ChangeList, ChangeMetadata, ChangeSource, ChangeStatus, RepoStatus, SpecDetail,
    SpecList, SpecSource, SpecSummary, TaskCounts,
};
