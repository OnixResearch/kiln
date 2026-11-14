mod agent_guidance;
mod developer_loop;
mod document_collaboration;
mod error;
mod fields;
mod function_address;
mod generated_agent_workflow;
mod lifecycle_store;
mod machine;
mod nominal_identity;
mod parser;
mod runtime_evidence;
mod schema_compatibility;
mod stack_provenance;
mod steel;
mod traceability;
mod types;

pub use error::PolicyError;
pub use lifecycle_store::LIFECYCLE_STORE_POLICY_SCHEMA_V1;
pub use parser::{parse_policy_json, validate_policy_json};
pub use schema_compatibility::{
    classify_policy_schema_compatibility, default_policy_schema_metadata,
    policy_schema_compatibility_diagnostic, policy_schema_metadata,
    PolicySchemaCompatibilityMetadata, PolicySchemaCompatibilityReport,
    CURRENT_POLICY_FIELD_SET_DIGEST, CURRENT_POLICY_FIELD_SET_DIGEST_MATERIAL,
    CURRENT_POLICY_SCHEMA_VERSION, DEFAULT_POLICY_REFRESH_COMMAND, POLICY_SCHEMA_METADATA_FIELD,
};
pub use types::{
    ArtifactSubstancePolicy, ExemptionMarkerPolicy, LifecycleStoreBounds,
    LifecycleStoreLocalStateSchema, LifecycleStoreOpenerProfile, LifecycleStorePolicy,
    LifecycleStoreProjectionPolicy, LifecycleStoreReceiptPolicy,
    LifecycleStoreRootResolutionPolicy, LifecycleStoreWorksetPolicy, Policy, ReceiptSchema,
    SteelOrchestrationPolicy, SteelOrchestrationProfile, TaskMarker, TraceabilityPolicy,
    SUPPORTED_POLICY_VERSION, TASK_IDENTITY_MARKER_ID, TASK_PREDECESSOR_MARKER_ID,
};

#[cfg(test)]
mod tests;
