macro_rules! root_compatibility {
    ($module:ident; $($local:ident => $root:ident => $reason:literal;)+) => {$(
        #[cfg_attr(dylint_lib = "octet", allow(renamed_imports, reason = $reason))]
        pub use crate::verified::$module::$local as $root;
    )+};
}

#[rustfmt::skip]
root_compatibility! {
    collaboration_change;
    action_json => change_collaboration_action_json => "Root collaboration action JSON function.";
    Action => ChangeCollaborationAction => "Root collaboration action type.";
    Error => ChangeCollaborationError => "Root collaboration error type.";
    Evidence => ChangeCollaborationEvidence => "Root collaboration evidence type.";
    Init => ChangeCollaborationInit => "Root collaboration init type.";
    Operation => ChangeCollaborationOperation => "Root collaboration operation type.";
    State => ChangeCollaborationState => "Root collaboration state type.";
}
pub use crate::verified::collaboration_change::{
    apply_change_collaboration_operation, change_collaboration_content_root,
    change_collaboration_lifecycle_view, change_collaboration_state_hash,
    current_change_collaboration_evidence, initialize_change_collaboration,
    materialize_change_collaboration, validate_change_collaboration_init,
    validate_change_collaboration_state, BASE_REVISION_KIND_GIT_OID,
};
pub use crate::verified::collaboration_checkout::plan_change_collaboration_checkout;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration checkout local snapshot type."
    )
)]
pub use crate::verified::collaboration_checkout::LocalSnapshot as ChangeCollaborationLocalSnapshot;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration checkout plan type."
    )
)]
pub use crate::verified::collaboration_checkout::Plan as ChangeCollaborationCheckoutPlan;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration import initialize function."
    )
)]
pub use crate::verified::collaboration_import::initialize as initialize_change_collaboration_import;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration import operation identity function."
    )
)]
pub use crate::verified::collaboration_import::operation_id as change_collaboration_import_operation_id;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration import plan function."
    )
)]
pub use crate::verified::collaboration_import::plan as plan_change_collaboration_import;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented collaboration import snapshot type."
    )
)]
pub use crate::verified::collaboration_import::Snapshot as ChangeCollaborationImportSnapshot;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented portable collaboration build function."
    )
)]
pub use crate::verified::collaboration_portable::build as build_portable_change_collaboration;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented portable collaboration validation function."
    )
)]
pub use crate::verified::collaboration_portable::validate as validate_portable_change_collaboration;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented portable collaboration envelope type."
    )
)]
pub use crate::verified::collaboration_portable::Envelope as PortableChangeCollaboration;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "Cairn core API: preserves the documented portable collaboration error type."
    )
)]
pub use crate::verified::collaboration_portable::Failure as PortableChangeCollaborationError;
pub use crate::verified::collaboration_portable::{
    PORTABLE_COLLABORATION_BACKEND_V1, PORTABLE_COLLABORATION_SCHEMA_V1,
};
#[rustfmt::skip]
root_compatibility! {
    collaboration_resolution;
    Inspection => ChangeCollaborationConflictInspection => "Root collaboration conflict inspection type.";
    Snapshot => ChangeCollaborationResolutionSnapshot => "Root collaboration resolution snapshot type.";
    Plan => ChangeCollaborationResolutionPlan => "Root collaboration resolution plan type.";
    CobPlan => ChangeCobResolutionPlan => "Root COB resolution plan type.";
    inspect => inspect_change_collaboration_conflicts => "Root collaboration conflict inspection function.";
    inspect_value => inspect_change_collaboration_conflict_value => "Root collaboration conflict value function.";
    plan => plan_change_collaboration_resolution => "Root collaboration resolution planning function.";
    apply => apply_planned_change_collaboration_resolution => "Root collaboration resolution apply function.";
    hash_plan => change_collaboration_resolution_plan_hash => "Root collaboration resolution plan hash function.";
    hash_cob_plan => change_cob_resolution_plan_hash => "Root COB resolution plan hash function.";
}
pub use crate::verified::collaboration_resolution::{
    inspect_change_cob_conflict_value, inspect_change_cob_conflicts, plan_change_cob_resolution,
    ChangeArtifactConflict, ChangeArtifactConflictHead, ChangeArtifactConflictValue,
    ChangeArtifactHeadSelection, ChangeLifecycleConflictHead,
};
#[rustfmt::skip]
root_compatibility! {
    collaboration_update;
    apply => apply_planned_change_collaboration_update => "Root collaboration update apply function.";
    build_plan => plan_change_collaboration_update => "Root collaboration update planning function.";
    cob_plan_hash => change_cob_update_plan_hash => "Root COB update plan hash function.";
    plan_hash => change_collaboration_update_plan_hash => "Root collaboration update plan hash function.";
    CobPlan => ChangeCobUpdatePlan => "Root COB update plan type.";
    Plan => ChangeCollaborationUpdatePlan => "Root collaboration update plan type.";
    Snapshot => ChangeCollaborationUpdateSnapshot => "Root collaboration update snapshot type.";
}
pub use crate::verified::collaboration_update::plan_change_cob_update;
pub use crate::verified::document_collaboration::*;
pub use crate::verified::radicle_change::{
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
pub use crate::verified::radicle_checkout::{
    plan_change_cob_checkout, ChangeCheckoutDestination, ChangeCheckoutFile, ChangeCobCheckoutPlan,
    ChangeCobLocalSnapshot,
};
pub use crate::verified::radicle_import::{
    change_cob_import_content_root, plan_change_cob_import, ChangeCobImportSnapshot,
};
