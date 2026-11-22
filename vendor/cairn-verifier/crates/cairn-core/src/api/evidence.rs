pub use crate::verified::ast_grep_gate::{
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
pub use crate::verified::cli_machine::{
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
pub use crate::verified::content_addressed::{
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
pub use crate::verified::content_bound_requirement_registry::{
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
pub use crate::verified::content_bound_trace_index::{
    evaluate_content_bound_trace_index, evaluate_structural_trace_index,
    parse_content_bound_trace_index_value, query_content_bound_trace_index,
    validate_content_bound_trace_index, verify_content_bound_trace_index,
    verify_structural_trace_index, ContentBoundTraceIndex, ContentBoundTraceIndexEvaluation,
    ContentBoundTraceIndexIssue, ContentBoundTraceIndexVerification, ContentBoundTraceLink,
    ContentBoundTraceMarkerSpan, ContentBoundTraceQuery, ContentBoundTraceQueryEvaluation,
    ContentBoundTraceSourceIdentity, RustItemBinding, RustItemConfiguration, RustItemFact,
    StructuralTraceData, CONTENT_BOUND_TRACE_INDEX_BOUNDARY, CONTENT_BOUND_TRACE_INDEX_NON_CLAIMS,
    CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1, CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2,
    TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES, TRACEABILITY_ANCHOR_RUST_ITEM_V1,
    TRACEABILITY_ASSURANCE_DECLARED,
};
pub use crate::verified::cross_repo_dependencies::{
    build_radicle_issue_receipt, cross_repo_dependency_contract, dependency_blocker_messages,
    evaluate_cross_repo_dependencies, parse_cross_repo_dependencies,
    radicle_issue_receipt_contract, summaries_by_identity, CrossRepoDependency,
    CrossRepoDependencyDiagnostic, CrossRepoDependencyReport, CrossRepoDependencyRowResult,
    CrossRepoReceiptSummary, RadicleIssueObservation, CROSS_REPO_DEPENDENCY_SCHEMA_V1,
    CROSS_REPO_NON_CLAIM_DOWNSTREAM, CROSS_REPO_NON_CLAIM_LINKAGE, CROSS_REPO_NON_CLAIM_UPSTREAM,
    CROSS_REPO_RECEIPT_SCHEMA_V1, RADICLE_ISSUE_NON_CLAIM_AUTHORITY,
    RADICLE_ISSUE_NON_CLAIM_LINKAGE, RADICLE_ISSUE_RECEIPT_KIND, RADICLE_ISSUE_RECEIPT_SCHEMA_V1,
};
pub use crate::verified::evidence::{
    blake3_hex, canonical_json, canonical_json_hash, canonical_preserves_bytes,
    canonical_preserves_hash, preserves_content_hash,
};
pub use crate::verified::global_registry::{
    build_global_registry_snapshot, empty_global_registry_snapshot, evaluate_registry_graph,
    global_registry_snapshot_hash, merge_global_registry_snapshot, query_registry_exact_id,
    query_registry_tags, verify_global_registry_snapshot, GlobalRegistrySnapshot,
    RegistryAdapterMetadata, RegistryCandidateRecords, RegistryCorePolicy, RegistryEdge,
    RegistryGraphDirection, RegistryGraphProfile, RegistryGraphReport, RegistryIssue, RegistryNode,
    RegistryQueryReport, RegistrySnapshotReport, RegistrySourceRecord, RegistryTagAssignment,
    EMPTY_REGISTRY_SNAPSHOT_HASH, GLOBAL_REGISTRY_SCHEMA,
};
pub use crate::verified::kani_receipt::{
    kani_receipt_hash, validate_kani_receipt, KaniReceiptIssue, KaniReceiptProfile,
    KaniReceiptReport, KANI_NON_CLAIM_BOUNDED_ONLY, KANI_NON_CLAIM_FFI_STUB,
    KANI_NON_CLAIM_VERIFIER_SOUNDNESS, KANI_NON_CLAIM_WHOLE_PROGRAM, KANI_RECEIPT_SCHEMA_V1,
};
pub use crate::verified::preserves_evidence_ingest::{
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
pub use crate::verified::probe::{
    all_probe_assertions_passed, evaluate_probe_assertions, probe_declaration_hash,
    probe_issue_json, probe_receipt_hash, validate_probe_completion_status,
    validate_probe_declaration, validate_probe_receipt, ProbeIssue, PROBE_DECLARATION_SCHEMA_V1,
    PROBE_RECEIPT_SCHEMA_V1,
};
pub use crate::verified::receipt_drift::{
    receipt_field_drift, DriftClass, DriftEntry, DriftReport,
};
pub use crate::verified::release_decision::*;
pub use crate::verified::release_run_ledger::{
    positive_release_run_ledger_fixture, release_run_explain, release_run_identity,
    release_run_ledger_hash, release_run_status, validate_release_run_ledger_value,
    ReleaseRunArtifact, ReleaseRunBlocker, ReleaseRunCommandIdentity, ReleaseRunDiagnostic,
    ReleaseRunExplainProjection, ReleaseRunLedger, ReleaseRunLedgerIssue, ReleaseRunLedgerReport,
    ReleaseRunResumeHint, ReleaseRunStatusProjection, ReleaseRunStep, RELEASE_RUN_LEDGER_BOUNDARY,
    RELEASE_RUN_LEDGER_SCHEMA_V1, RELEASE_RUN_STATE_BLOCKED, RELEASE_RUN_STATE_FAILED,
    RELEASE_RUN_STATE_PASSED, RELEASE_RUN_STATE_PENDING, RELEASE_RUN_STATE_RUNNING_OBSERVED,
    RELEASE_RUN_STATE_SKIPPED, RELEASE_RUN_STATE_SUPERSEDED,
};
pub use crate::verified::runtime_evidence::{
    is_runtime_capture_origin, runtime_record_hash, verify_runtime_evidence,
    witness_checkpoint_hash, CaptureOriginSummary, RuntimeEvidenceIssue, RuntimeEvidencePolicy,
    RuntimeEvidenceReport, CAPTURE_ORIGIN_BOUNDARY, CAPTURE_ORIGIN_INGESTED,
    CAPTURE_ORIGIN_UNKNOWN, CAPTURE_ORIGIN_WRAPPER_ASSERTED,
    COMPLETENESS_ALTERED_BEFORE_CHECKPOINT, COMPLETENESS_CHECKPOINT_MALFORMED,
    COMPLETENESS_INTEGRITY_ONLY, COMPLETENESS_SUBJECT_MISMATCH, COMPLETENESS_TRUNCATED,
    COMPLETENESS_WITNESSED_PREFIX, DEFAULT_RUNTIME_SUMMARY_MAX_BYTES, RUNTIME_EVIDENCE_BOUNDARY,
    RUNTIME_EVIDENCE_SCHEMA_V1, RUNTIME_HASH_ALGORITHM_BLAKE3,
};
pub use crate::verified::structured_lifecycle::{
    structured_lifecycle_bundle_hash, structured_lifecycle_issue_json,
    validate_structured_lifecycle_bundle, StructuredLifecycleIssue, StructuredLifecycleReport,
    STRUCTURED_LIFECYCLE_SCHEMA_V1,
};
