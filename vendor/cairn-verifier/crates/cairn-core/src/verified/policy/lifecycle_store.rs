use std::collections::BTreeSet;

use serde_json::Value;

use super::{
    LifecycleStoreBounds, LifecycleStoreLocalStateSchema, LifecycleStoreOpenerProfile,
    LifecycleStorePolicy, LifecycleStoreProjectionPolicy, LifecycleStoreReceiptPolicy,
    LifecycleStoreRootResolutionPolicy, LifecycleStoreWorksetPolicy, PolicyError,
};

pub const LIFECYCLE_STORE_POLICY_SCHEMA_V1: &str = "cairn.lifecycle-store-policy.v1";

const POLICY_FIELD: &str = "lifecycle_store_policy";
const HASH_ALGORITHM_BLAKE3: &str = "BLAKE3";
const CANONICALIZATION_SORTED_JSON: &str = "serde_json_compact_sorted_keys";
const ROOT_FLAG: &str = "--root";
const LIFECYCLE_STORE_FLAG: &str = "--lifecycle-store";
const MEMBER_ARGUMENT_MODE_POSITIONAL_PATHS: &str = "positional_paths";
const LOCK_KIND_EXCLUSIVE: &str = "exclusive";
const REPLACEMENT_KIND_ATOMIC: &str = "atomic";
const RECEIPT_EVIDENCE_CLASS_LOCAL_ONLY: &str = "local-only";
const RECEIPT_HASH_FIELD: &str = "receipt_hash";
const PORTABLE_HASH_FIELD: &str = "portable_hash";

const ELIGIBLE_ROOT_COMMANDS: &[&str] = &[
    "status",
    "spec list",
    "spec show",
    "validate",
    "context",
    "workflow status",
    "workflow instructions",
    "doctor",
];
const ROOT_PRECEDENCE: &[&str] = &[
    "explicit_root",
    "explicit_lifecycle_store",
    "native_root",
    "project_pointer",
    "classic_fallback",
];
const STORE_IDENTITY_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "store_id",
    "remote_hint",
    "source_blake3",
    "non_claims",
];
const PROJECT_CONFIG_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "lifecycle_store",
    "references",
    "context_refs",
    "artifact_guidance",
    "operation_guidance",
    "source_blake3",
    "non_claims",
];
const WORKSET_REQUIRED_FIELDS: &[&str] = &["id", "members", "primary_member", "opener_profile"];
const RECEIPT_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "command",
    "valid",
    "verdict",
    "evidence_class",
    "resolution",
    "actions",
    "issues",
    "policy_hash",
    RECEIPT_HASH_FIELD,
    PORTABLE_HASH_FIELD,
    "non_claims",
];
const RECEIPT_RESOLUTION_FIELDS: &[&str] = &[
    "source",
    "root",
    "store_id",
    "store_identity",
    "project_identity",
    "diagnostics",
];
const RECEIPT_LOCAL_DISPLAY_FIELDS: &[&str] = &[
    "root",
    "path",
    "checkout_path",
    "resolution.root",
    "actions.path",
    "members.path",
];
const RECEIPT_PORTABLE_FIELDS: &[&str] = &[
    "schema",
    "command",
    "store_id",
    "store_identity",
    "project_identity",
    "reference_ids",
    "member_labels",
    "opener_profile",
    "actions",
    "policy_hash",
    "non_claims",
];
const OPENER_PROFILE_KEYS: &[&str] = &[
    "id",
    "executable",
    "fixed_args",
    "member_argument_mode",
    "primary_first",
];

const FIELD_SCHEMA: &str = "lifecycle_store_policy.schema";
const FIELD_HASH_ALGORITHM: &str = "lifecycle_store_policy.hash_algorithm";
const FIELD_CANONICALIZATION: &str = "lifecycle_store_policy.canonicalization";
const FIELD_BOUNDS: &str = "lifecycle_store_policy.bounds";
const FIELD_MAX_STORE_ID_BYTES: &str = "lifecycle_store_policy.bounds.max_store_id_bytes";
const FIELD_MAX_REMOTE_HINT_BYTES: &str = "lifecycle_store_policy.bounds.max_remote_hint_bytes";
const FIELD_MAX_REFERENCES: &str = "lifecycle_store_policy.bounds.max_references";
const FIELD_MAX_LOCATOR_ENTRIES: &str = "lifecycle_store_policy.bounds.max_locator_entries";
const FIELD_MAX_WORKSET_ID_BYTES: &str = "lifecycle_store_policy.bounds.max_workset_id_bytes";
const FIELD_MAX_WORKSETS: &str = "lifecycle_store_policy.bounds.max_worksets";
const FIELD_MAX_WORKSET_MEMBERS: &str = "lifecycle_store_policy.bounds.max_workset_members";
const FIELD_MAX_PATH_BYTES: &str = "lifecycle_store_policy.bounds.max_path_bytes";
const FIELD_MAX_CONTEXT_SPECS: &str = "lifecycle_store_policy.bounds.max_context_specs_per_store";
const FIELD_MAX_SPEC_TITLE_BYTES: &str = "lifecycle_store_policy.bounds.max_spec_title_bytes";
const FIELD_MAX_PURPOSE_SUMMARY_BYTES: &str =
    "lifecycle_store_policy.bounds.max_purpose_summary_bytes";
const FIELD_MAX_ISSUE_COUNT: &str = "lifecycle_store_policy.bounds.max_issue_count";
const FIELD_MAX_NON_CLAIMS: &str = "lifecycle_store_policy.bounds.max_non_claims";
const FIELD_STORE_IDENTITY: &str = "lifecycle_store_policy.store_identity";
const FIELD_PROJECT_CONFIG: &str = "lifecycle_store_policy.project_config";
const FIELD_ROOT_RESOLUTION: &str = "lifecycle_store_policy.root_resolution";
const FIELD_ROOT_FLAG: &str = "lifecycle_store_policy.root_resolution.root_flag";
const FIELD_LIFECYCLE_STORE_FLAG: &str =
    "lifecycle_store_policy.root_resolution.lifecycle_store_flag";
const FIELD_SELECTORS_EXCLUSIVE: &str =
    "lifecycle_store_policy.root_resolution.selectors_mutually_exclusive";
const FIELD_ELIGIBLE_COMMANDS: &str = "lifecycle_store_policy.root_resolution.eligible_commands";
const FIELD_PRECEDENCE: &str = "lifecycle_store_policy.root_resolution.precedence";
const FIELD_NATIVE_ROOT_WINS: &str =
    "lifecycle_store_policy.root_resolution.native_root_wins_over_project_pointer";
const FIELD_CURRENT_DIRECTORY_ONLY: &str =
    "lifecycle_store_policy.root_resolution.current_directory_only_without_vcs_boundary";
const FIELD_CONSULT_GLOBAL_REGISTRY: &str =
    "lifecycle_store_policy.root_resolution.consult_global_registry";
const FIELD_MACHINE_DEFAULT_STORE: &str =
    "lifecycle_store_policy.root_resolution.machine_default_store";
const FIELD_LOCATOR: &str = "lifecycle_store_policy.locator";
const FIELD_WORKSETS: &str = "lifecycle_store_policy.worksets";
const FIELD_WORKSET_STATE: &str = "lifecycle_store_policy.worksets.state";
const FIELD_WORKSET_REQUIRED_FIELDS: &str = "lifecycle_store_policy.worksets.required_fields";
const FIELD_MINIMUM_MEMBERS: &str = "lifecycle_store_policy.worksets.minimum_members";
const FIELD_OPENER_PROFILE_FIELD: &str = "lifecycle_store_policy.worksets.opener_profile_field";
const FIELD_REMOVAL_PRESERVES_MEMBERS: &str =
    "lifecycle_store_policy.worksets.removal_preserves_members";
const FIELD_OPENER_PROFILES: &str = "lifecycle_store_policy.opener_profiles";
const FIELD_OPENER_PROFILE_SHAPE: &str = "lifecycle_store_policy.opener_profiles.shape";
const FIELD_OPENER_PROFILE_ID: &str = "lifecycle_store_policy.opener_profiles.id";
const FIELD_OPENER_EXECUTABLE: &str = "lifecycle_store_policy.opener_profiles.executable";
const FIELD_OPENER_FIXED_ARGS: &str = "lifecycle_store_policy.opener_profiles.fixed_args";
const FIELD_OPENER_MEMBER_MODE: &str =
    "lifecycle_store_policy.opener_profiles.member_argument_mode";
const FIELD_OPENER_PRIMARY_FIRST: &str = "lifecycle_store_policy.opener_profiles.primary_first";
const FIELD_RECEIPTS: &str = "lifecycle_store_policy.receipts";
const FIELD_RECEIPT_SCHEMA: &str = "lifecycle_store_policy.receipts.schema";
const FIELD_RECEIPT_EVIDENCE_CLASS: &str = "lifecycle_store_policy.receipts.evidence_class";
const FIELD_RECEIPT_FULL_HASH: &str = "lifecycle_store_policy.receipts.full_hash_field";
const FIELD_RECEIPT_PORTABLE_HASH: &str = "lifecycle_store_policy.receipts.portable_hash_field";
const FIELD_RECEIPT_REQUIRED_FIELDS: &str = "lifecycle_store_policy.receipts.required_fields";
const FIELD_RECEIPT_RESOLUTION_FIELDS: &str = "lifecycle_store_policy.receipts.resolution_fields";
const FIELD_RECEIPT_LOCAL_DISPLAY_FIELDS: &str =
    "lifecycle_store_policy.receipts.local_display_fields";
const FIELD_RECEIPT_PORTABLE_FIELDS: &str = "lifecycle_store_policy.receipts.portable_fields";
const FIELD_RECEIPT_PORTABLE_EXCLUDED_FIELDS: &str =
    "lifecycle_store_policy.receipts.portable_excluded_fields";
const FIELD_RECEIPT_LOCAL_ONLY: &str = "lifecycle_store_policy.receipts.local_only_by_default";
const FIELD_RECEIPT_RELEASE_DEFAULT: &str =
    "lifecycle_store_policy.receipts.release_readiness_consumes_by_default";
const FIELD_RECEIPT_NON_CLAIMS: &str = "lifecycle_store_policy.receipts.required_non_claims";
const FIELD_ISSUE_CODES: &str = "lifecycle_store_policy.issue_codes";
const FIELD_REQUIRED_NON_CLAIMS: &str = "lifecycle_store_policy.required_non_claims";

struct ProjectionExpected {
    source_schema: &'static str,
    projection_schema: &'static str,
    source_path: &'static str,
    projection_path: &'static str,
    required_fields: &'static [&'static str],
}

struct ProjectionFields {
    root: &'static str,
    source_schema: &'static str,
    projection_schema: &'static str,
    source_path: &'static str,
    projection_path: &'static str,
    source_identity_field: &'static str,
    runtime_nickel_evaluation: &'static str,
    stale_projection_rejected: &'static str,
    required_fields: &'static str,
    required_non_claims: &'static str,
}

struct LocalStateExpected {
    schema: &'static str,
    relative_path: &'static str,
}

struct LocalStateFields {
    root: &'static str,
    schema: &'static str,
    relative_path: &'static str,
    state_hash_field: &'static str,
    expected_prior_hash_field: &'static str,
    hash_algorithm: &'static str,
    preview_required: &'static str,
    lock_kind: &'static str,
    replacement_kind: &'static str,
    symlink_paths_allowed: &'static str,
}

const STORE_PROJECTION_EXPECTED: ProjectionExpected = ProjectionExpected {
    source_schema: "cairn.lifecycle-store-source.v1",
    projection_schema: "cairn.lifecycle-store-identity.v1",
    source_path: ".cairn-store/store.ncl",
    projection_path: ".cairn-store/store.json",
    required_fields: STORE_IDENTITY_REQUIRED_FIELDS,
};
const PROJECT_PROJECTION_EXPECTED: ProjectionExpected = ProjectionExpected {
    source_schema: "cairn.lifecycle-store-project-source.v1",
    projection_schema: "cairn.lifecycle-store-project.v1",
    source_path: ".cairn/project.ncl",
    projection_path: ".cairn/project.json",
    required_fields: PROJECT_CONFIG_REQUIRED_FIELDS,
};
const STORE_PROJECTION_FIELDS: ProjectionFields = ProjectionFields {
    root: FIELD_STORE_IDENTITY,
    source_schema: "lifecycle_store_policy.store_identity.source_schema",
    projection_schema: "lifecycle_store_policy.store_identity.projection_schema",
    source_path: "lifecycle_store_policy.store_identity.source_path",
    projection_path: "lifecycle_store_policy.store_identity.projection_path",
    source_identity_field: "lifecycle_store_policy.store_identity.source_identity_field",
    runtime_nickel_evaluation: "lifecycle_store_policy.store_identity.runtime_nickel_evaluation",
    stale_projection_rejected: "lifecycle_store_policy.store_identity.stale_projection_rejected",
    required_fields: "lifecycle_store_policy.store_identity.required_fields",
    required_non_claims: "lifecycle_store_policy.store_identity.required_non_claims",
};
const PROJECT_PROJECTION_FIELDS: ProjectionFields = ProjectionFields {
    root: FIELD_PROJECT_CONFIG,
    source_schema: "lifecycle_store_policy.project_config.source_schema",
    projection_schema: "lifecycle_store_policy.project_config.projection_schema",
    source_path: "lifecycle_store_policy.project_config.source_path",
    projection_path: "lifecycle_store_policy.project_config.projection_path",
    source_identity_field: "lifecycle_store_policy.project_config.source_identity_field",
    runtime_nickel_evaluation: "lifecycle_store_policy.project_config.runtime_nickel_evaluation",
    stale_projection_rejected: "lifecycle_store_policy.project_config.stale_projection_rejected",
    required_fields: "lifecycle_store_policy.project_config.required_fields",
    required_non_claims: "lifecycle_store_policy.project_config.required_non_claims",
};
const LOCATOR_EXPECTED: LocalStateExpected = LocalStateExpected {
    schema: "cairn.lifecycle-store-locator.v1",
    relative_path: "cairn/stores/registry.json",
};
const WORKSET_STATE_EXPECTED: LocalStateExpected = LocalStateExpected {
    schema: "cairn.lifecycle-store-worksets.v1",
    relative_path: "cairn/worksets/state.json",
};
const LOCATOR_FIELDS: LocalStateFields = LocalStateFields {
    root: FIELD_LOCATOR,
    schema: "lifecycle_store_policy.locator.schema",
    relative_path: "lifecycle_store_policy.locator.relative_path",
    state_hash_field: "lifecycle_store_policy.locator.state_hash_field",
    expected_prior_hash_field: "lifecycle_store_policy.locator.expected_prior_hash_field",
    hash_algorithm: "lifecycle_store_policy.locator.hash_algorithm",
    preview_required: "lifecycle_store_policy.locator.preview_required",
    lock_kind: "lifecycle_store_policy.locator.lock_kind",
    replacement_kind: "lifecycle_store_policy.locator.replacement_kind",
    symlink_paths_allowed: "lifecycle_store_policy.locator.symlink_paths_allowed",
};
const WORKSET_STATE_FIELDS: LocalStateFields = LocalStateFields {
    root: FIELD_WORKSET_STATE,
    schema: "lifecycle_store_policy.worksets.state.schema",
    relative_path: "lifecycle_store_policy.worksets.state.relative_path",
    state_hash_field: "lifecycle_store_policy.worksets.state.state_hash_field",
    expected_prior_hash_field: "lifecycle_store_policy.worksets.state.expected_prior_hash_field",
    hash_algorithm: "lifecycle_store_policy.worksets.state.hash_algorithm",
    preview_required: "lifecycle_store_policy.worksets.state.preview_required",
    lock_kind: "lifecycle_store_policy.worksets.state.lock_kind",
    replacement_kind: "lifecycle_store_policy.worksets.state.replacement_kind",
    symlink_paths_allowed: "lifecycle_store_policy.worksets.state.symlink_paths_allowed",
};

pub(super) fn lifecycle_store_policy(value: &Value) -> Result<LifecycleStorePolicy, PolicyError> {
    let policy = object_field(value, "lifecycle_store_policy", POLICY_FIELD)?;
    let schema = exact_string_field(
        policy,
        "schema",
        FIELD_SCHEMA,
        LIFECYCLE_STORE_POLICY_SCHEMA_V1,
    )?;
    let hash_algorithm = exact_string_field(
        policy,
        "hash_algorithm",
        FIELD_HASH_ALGORITHM,
        HASH_ALGORITHM_BLAKE3,
    )?;
    let canonicalization = exact_string_field(
        policy,
        "canonicalization",
        FIELD_CANONICALIZATION,
        CANONICALIZATION_SORTED_JSON,
    )?;
    let bounds = parse_bounds(policy)?;
    let store_identity = parse_projection_policy(
        policy,
        "store_identity",
        &STORE_PROJECTION_FIELDS,
        &STORE_PROJECTION_EXPECTED,
    )?;
    let project_config = parse_projection_policy(
        policy,
        "project_config",
        &PROJECT_PROJECTION_FIELDS,
        &PROJECT_PROJECTION_EXPECTED,
    )?;
    let root_resolution = parse_root_resolution(policy)?;
    let locator = parse_local_state_schema(policy, "locator", &LOCATOR_FIELDS, &LOCATOR_EXPECTED)?;
    let worksets = parse_workset_policy(policy)?;
    let opener_profiles = parse_opener_profiles(policy)?;
    let receipts = parse_receipt_policy(policy)?;
    let issue_codes = nonempty_string_array_field(policy, "issue_codes", FIELD_ISSUE_CODES)?;
    validate_issue_codes(&issue_codes)?;
    let required_non_claims =
        nonempty_string_array_field(policy, "required_non_claims", FIELD_REQUIRED_NON_CLAIMS)?;
    validate_unique(&required_non_claims, FIELD_REQUIRED_NON_CLAIMS)?;

    let parsed = LifecycleStorePolicy {
        schema,
        hash_algorithm,
        canonicalization,
        bounds,
        store_identity,
        project_config,
        root_resolution,
        locator,
        worksets,
        opener_profiles,
        receipts,
        issue_codes,
        required_non_claims,
    };
    validate_policy_relationships(&parsed)?;
    Ok(parsed)
}

fn parse_bounds(policy: &Value) -> Result<LifecycleStoreBounds, PolicyError> {
    let bounds = object_field(policy, "bounds", FIELD_BOUNDS)?;
    Ok(LifecycleStoreBounds {
        max_store_id_bytes: positive_usize_field(
            bounds,
            "max_store_id_bytes",
            FIELD_MAX_STORE_ID_BYTES,
        )?,
        max_remote_hint_bytes: positive_usize_field(
            bounds,
            "max_remote_hint_bytes",
            FIELD_MAX_REMOTE_HINT_BYTES,
        )?,
        max_references: positive_usize_field(bounds, "max_references", FIELD_MAX_REFERENCES)?,
        max_locator_entries: positive_usize_field(
            bounds,
            "max_locator_entries",
            FIELD_MAX_LOCATOR_ENTRIES,
        )?,
        max_workset_id_bytes: positive_usize_field(
            bounds,
            "max_workset_id_bytes",
            FIELD_MAX_WORKSET_ID_BYTES,
        )?,
        max_worksets: positive_usize_field(bounds, "max_worksets", FIELD_MAX_WORKSETS)?,
        max_workset_members: positive_usize_field(
            bounds,
            "max_workset_members",
            FIELD_MAX_WORKSET_MEMBERS,
        )?,
        max_path_bytes: positive_usize_field(bounds, "max_path_bytes", FIELD_MAX_PATH_BYTES)?,
        max_context_specs_per_store: positive_usize_field(
            bounds,
            "max_context_specs_per_store",
            FIELD_MAX_CONTEXT_SPECS,
        )?,
        max_spec_title_bytes: positive_usize_field(
            bounds,
            "max_spec_title_bytes",
            FIELD_MAX_SPEC_TITLE_BYTES,
        )?,
        max_purpose_summary_bytes: positive_usize_field(
            bounds,
            "max_purpose_summary_bytes",
            FIELD_MAX_PURPOSE_SUMMARY_BYTES,
        )?,
        max_issue_count: positive_usize_field(bounds, "max_issue_count", FIELD_MAX_ISSUE_COUNT)?,
        max_non_claims: positive_usize_field(bounds, "max_non_claims", FIELD_MAX_NON_CLAIMS)?,
    })
}

fn parse_projection_policy(
    policy: &Value,
    lookup: &str,
    fields: &ProjectionFields,
    expected: &ProjectionExpected,
) -> Result<LifecycleStoreProjectionPolicy, PolicyError> {
    let projection = object_field(policy, lookup, fields.root)?;
    let source_schema = exact_string_field(
        projection,
        "source_schema",
        fields.source_schema,
        expected.source_schema,
    )?;
    let projection_schema = exact_string_field(
        projection,
        "projection_schema",
        fields.projection_schema,
        expected.projection_schema,
    )?;
    let source_path = exact_string_field(
        projection,
        "source_path",
        fields.source_path,
        expected.source_path,
    )?;
    let projection_path = exact_string_field(
        projection,
        "projection_path",
        fields.projection_path,
        expected.projection_path,
    )?;
    let source_identity_field = exact_string_field(
        projection,
        "source_identity_field",
        fields.source_identity_field,
        "source_blake3",
    )?;
    let runtime_nickel_evaluation = exact_bool_field(
        projection,
        "runtime_nickel_evaluation",
        fields.runtime_nickel_evaluation,
        false,
    )?;
    let stale_projection_rejected = exact_bool_field(
        projection,
        "stale_projection_rejected",
        fields.stale_projection_rejected,
        true,
    )?;
    let required_fields =
        nonempty_string_array_field(projection, "required_fields", fields.required_fields)?;
    validate_string_set(
        &required_fields,
        expected.required_fields,
        fields.required_fields,
    )?;
    let required_non_claims = nonempty_string_array_field(
        projection,
        "required_non_claims",
        fields.required_non_claims,
    )?;
    validate_unique(&required_non_claims, fields.required_non_claims)?;

    Ok(LifecycleStoreProjectionPolicy {
        source_schema,
        projection_schema,
        source_path,
        projection_path,
        source_identity_field,
        runtime_nickel_evaluation,
        stale_projection_rejected,
        required_fields,
        required_non_claims,
    })
}

fn parse_root_resolution(
    policy: &Value,
) -> Result<LifecycleStoreRootResolutionPolicy, PolicyError> {
    let resolution = object_field(policy, "root_resolution", FIELD_ROOT_RESOLUTION)?;
    let root_flag = exact_string_field(resolution, "root_flag", FIELD_ROOT_FLAG, ROOT_FLAG)?;
    let lifecycle_store_flag = exact_string_field(
        resolution,
        "lifecycle_store_flag",
        FIELD_LIFECYCLE_STORE_FLAG,
        LIFECYCLE_STORE_FLAG,
    )?;
    let selectors_mutually_exclusive = exact_bool_field(
        resolution,
        "selectors_mutually_exclusive",
        FIELD_SELECTORS_EXCLUSIVE,
        true,
    )?;
    let eligible_commands =
        nonempty_string_array_field(resolution, "eligible_commands", FIELD_ELIGIBLE_COMMANDS)?;
    validate_unique(&eligible_commands, FIELD_ELIGIBLE_COMMANDS)?;
    if !same_ordered_values(&eligible_commands, ELIGIBLE_ROOT_COMMANDS) {
        return Err(PolicyError::InvalidField(FIELD_ELIGIBLE_COMMANDS));
    }
    let precedence = nonempty_string_array_field(resolution, "precedence", FIELD_PRECEDENCE)?;
    if !same_ordered_values(&precedence, ROOT_PRECEDENCE) {
        return Err(PolicyError::InvalidField(FIELD_PRECEDENCE));
    }
    let native_root_wins_over_project_pointer = exact_bool_field(
        resolution,
        "native_root_wins_over_project_pointer",
        FIELD_NATIVE_ROOT_WINS,
        true,
    )?;
    let current_directory_only_without_vcs_boundary = exact_bool_field(
        resolution,
        "current_directory_only_without_vcs_boundary",
        FIELD_CURRENT_DIRECTORY_ONLY,
        true,
    )?;
    let consult_global_registry = exact_bool_field(
        resolution,
        "consult_global_registry",
        FIELD_CONSULT_GLOBAL_REGISTRY,
        false,
    )?;
    let machine_default_store = exact_bool_field(
        resolution,
        "machine_default_store",
        FIELD_MACHINE_DEFAULT_STORE,
        false,
    )?;

    Ok(LifecycleStoreRootResolutionPolicy {
        root_flag,
        lifecycle_store_flag,
        selectors_mutually_exclusive,
        eligible_commands,
        precedence,
        native_root_wins_over_project_pointer,
        current_directory_only_without_vcs_boundary,
        consult_global_registry,
        machine_default_store,
    })
}

fn parse_local_state_schema(
    parent: &Value,
    lookup: &str,
    fields: &LocalStateFields,
    expected: &LocalStateExpected,
) -> Result<LifecycleStoreLocalStateSchema, PolicyError> {
    let state = object_field(parent, lookup, fields.root)?;
    let schema = exact_string_field(state, "schema", fields.schema, expected.schema)?;
    let relative_path = exact_string_field(
        state,
        "relative_path",
        fields.relative_path,
        expected.relative_path,
    )?;
    let state_hash_field = exact_string_field(
        state,
        "state_hash_field",
        fields.state_hash_field,
        "state_hash",
    )?;
    let expected_prior_hash_field = exact_string_field(
        state,
        "expected_prior_hash_field",
        fields.expected_prior_hash_field,
        "expected_prior_state_hash",
    )?;
    let hash_algorithm = exact_string_field(
        state,
        "hash_algorithm",
        fields.hash_algorithm,
        HASH_ALGORITHM_BLAKE3,
    )?;
    let preview_required =
        exact_bool_field(state, "preview_required", fields.preview_required, true)?;
    let lock_kind = exact_string_field(state, "lock_kind", fields.lock_kind, LOCK_KIND_EXCLUSIVE)?;
    let replacement_kind = exact_string_field(
        state,
        "replacement_kind",
        fields.replacement_kind,
        REPLACEMENT_KIND_ATOMIC,
    )?;
    let symlink_paths_allowed = exact_bool_field(
        state,
        "symlink_paths_allowed",
        fields.symlink_paths_allowed,
        false,
    )?;

    Ok(LifecycleStoreLocalStateSchema {
        schema,
        relative_path,
        state_hash_field,
        expected_prior_hash_field,
        hash_algorithm,
        preview_required,
        lock_kind,
        replacement_kind,
        symlink_paths_allowed,
    })
}

fn parse_workset_policy(policy: &Value) -> Result<LifecycleStoreWorksetPolicy, PolicyError> {
    let worksets = object_field(policy, "worksets", FIELD_WORKSETS)?;
    let state = parse_local_state_schema(
        worksets,
        "state",
        &WORKSET_STATE_FIELDS,
        &WORKSET_STATE_EXPECTED,
    )?;
    let required_fields =
        nonempty_string_array_field(worksets, "required_fields", FIELD_WORKSET_REQUIRED_FIELDS)?;
    validate_string_set(
        &required_fields,
        WORKSET_REQUIRED_FIELDS,
        FIELD_WORKSET_REQUIRED_FIELDS,
    )?;
    let minimum_members = positive_usize_field(worksets, "minimum_members", FIELD_MINIMUM_MEMBERS)?;
    let opener_profile_field = exact_string_field(
        worksets,
        "opener_profile_field",
        FIELD_OPENER_PROFILE_FIELD,
        "opener_profile",
    )?;
    let removal_preserves_members = exact_bool_field(
        worksets,
        "removal_preserves_members",
        FIELD_REMOVAL_PRESERVES_MEMBERS,
        true,
    )?;
    Ok(LifecycleStoreWorksetPolicy {
        state,
        required_fields,
        minimum_members,
        opener_profile_field,
        removal_preserves_members,
    })
}

fn parse_opener_profiles(policy: &Value) -> Result<Vec<LifecycleStoreOpenerProfile>, PolicyError> {
    let values = array_field(policy, "opener_profiles", FIELD_OPENER_PROFILES)?;
    if values.is_empty() {
        return Err(PolicyError::InvalidField(FIELD_OPENER_PROFILES));
    }
    let mut profiles = Vec::with_capacity(values.len());
    let mut ids = BTreeSet::new();
    for value in values {
        let object = value
            .as_object()
            .ok_or(PolicyError::InvalidField(FIELD_OPENER_PROFILE_SHAPE))?;
        let id = string_field(value, "id", FIELD_OPENER_PROFILE_ID)?;
        if !is_store_id(&id) || !ids.insert(id.clone()) {
            return Err(PolicyError::InvalidField(FIELD_OPENER_PROFILE_ID));
        }
        let executable = string_field(value, "executable", FIELD_OPENER_EXECUTABLE)?;
        if !is_safe_executable(&executable) {
            return Err(PolicyError::InvalidField(FIELD_OPENER_EXECUTABLE));
        }
        let fixed_args = string_array_field(value, "fixed_args", FIELD_OPENER_FIXED_ARGS)?;
        if fixed_args
            .iter()
            .any(|argument| !is_safe_argument(argument))
        {
            return Err(PolicyError::InvalidField(FIELD_OPENER_FIXED_ARGS));
        }
        let member_argument_mode = exact_string_field(
            value,
            "member_argument_mode",
            FIELD_OPENER_MEMBER_MODE,
            MEMBER_ARGUMENT_MODE_POSITIONAL_PATHS,
        )?;
        let primary_first = bool_field(value, "primary_first", FIELD_OPENER_PRIMARY_FIRST)?;
        if object.len() != OPENER_PROFILE_KEYS.len()
            || object
                .keys()
                .any(|key| !OPENER_PROFILE_KEYS.contains(&key.as_str()))
        {
            return Err(PolicyError::InvalidField(FIELD_OPENER_PROFILE_SHAPE));
        }
        profiles.push(LifecycleStoreOpenerProfile {
            id,
            executable,
            fixed_args,
            member_argument_mode,
            primary_first,
        });
    }
    Ok(profiles)
}

fn parse_receipt_policy(policy: &Value) -> Result<LifecycleStoreReceiptPolicy, PolicyError> {
    let receipts = object_field(policy, "receipts", FIELD_RECEIPTS)?;
    let schema = exact_string_field(
        receipts,
        "schema",
        FIELD_RECEIPT_SCHEMA,
        "cairn.lifecycle-store-receipt-policy.v1",
    )?;
    let evidence_class = exact_string_field(
        receipts,
        "evidence_class",
        FIELD_RECEIPT_EVIDENCE_CLASS,
        RECEIPT_EVIDENCE_CLASS_LOCAL_ONLY,
    )?;
    let full_hash_field = exact_string_field(
        receipts,
        "full_hash_field",
        FIELD_RECEIPT_FULL_HASH,
        RECEIPT_HASH_FIELD,
    )?;
    let portable_hash_field = exact_string_field(
        receipts,
        "portable_hash_field",
        FIELD_RECEIPT_PORTABLE_HASH,
        PORTABLE_HASH_FIELD,
    )?;
    let required_fields =
        nonempty_string_array_field(receipts, "required_fields", FIELD_RECEIPT_REQUIRED_FIELDS)?;
    validate_string_set(
        &required_fields,
        RECEIPT_REQUIRED_FIELDS,
        FIELD_RECEIPT_REQUIRED_FIELDS,
    )?;
    let resolution_fields = nonempty_string_array_field(
        receipts,
        "resolution_fields",
        FIELD_RECEIPT_RESOLUTION_FIELDS,
    )?;
    validate_string_set(
        &resolution_fields,
        RECEIPT_RESOLUTION_FIELDS,
        FIELD_RECEIPT_RESOLUTION_FIELDS,
    )?;
    let local_display_fields = nonempty_string_array_field(
        receipts,
        "local_display_fields",
        FIELD_RECEIPT_LOCAL_DISPLAY_FIELDS,
    )?;
    validate_string_set(
        &local_display_fields,
        RECEIPT_LOCAL_DISPLAY_FIELDS,
        FIELD_RECEIPT_LOCAL_DISPLAY_FIELDS,
    )?;
    let portable_fields =
        nonempty_string_array_field(receipts, "portable_fields", FIELD_RECEIPT_PORTABLE_FIELDS)?;
    validate_string_set(
        &portable_fields,
        RECEIPT_PORTABLE_FIELDS,
        FIELD_RECEIPT_PORTABLE_FIELDS,
    )?;
    let portable_excluded_fields = nonempty_string_array_field(
        receipts,
        "portable_excluded_fields",
        FIELD_RECEIPT_PORTABLE_EXCLUDED_FIELDS,
    )?;
    validate_string_set(
        &portable_excluded_fields,
        RECEIPT_LOCAL_DISPLAY_FIELDS,
        FIELD_RECEIPT_PORTABLE_EXCLUDED_FIELDS,
    )?;
    if portable_fields
        .iter()
        .any(|field| portable_excluded_fields.contains(field))
    {
        return Err(PolicyError::InvalidField(FIELD_RECEIPT_PORTABLE_FIELDS));
    }
    let local_only_by_default = exact_bool_field(
        receipts,
        "local_only_by_default",
        FIELD_RECEIPT_LOCAL_ONLY,
        true,
    )?;
    let release_readiness_consumes_by_default = exact_bool_field(
        receipts,
        "release_readiness_consumes_by_default",
        FIELD_RECEIPT_RELEASE_DEFAULT,
        false,
    )?;
    let required_non_claims =
        nonempty_string_array_field(receipts, "required_non_claims", FIELD_RECEIPT_NON_CLAIMS)?;
    validate_unique(&required_non_claims, FIELD_RECEIPT_NON_CLAIMS)?;

    Ok(LifecycleStoreReceiptPolicy {
        schema,
        evidence_class,
        full_hash_field,
        portable_hash_field,
        required_fields,
        resolution_fields,
        local_display_fields,
        portable_fields,
        portable_excluded_fields,
        local_only_by_default,
        release_readiness_consumes_by_default,
        required_non_claims,
    })
}

fn validate_issue_codes(issue_codes: &[String]) -> Result<(), PolicyError> {
    validate_unique(issue_codes, FIELD_ISSUE_CODES)?;
    if issue_codes.iter().any(|code| !is_issue_code(code)) {
        return Err(PolicyError::InvalidField(FIELD_ISSUE_CODES));
    }
    Ok(())
}

fn validate_policy_relationships(policy: &LifecycleStorePolicy) -> Result<(), PolicyError> {
    if policy.issue_codes.len() > policy.bounds.max_issue_count {
        return Err(PolicyError::InvalidField(FIELD_ISSUE_CODES));
    }
    if policy.required_non_claims.len() > policy.bounds.max_non_claims {
        return Err(PolicyError::InvalidField(FIELD_REQUIRED_NON_CLAIMS));
    }
    if policy.store_identity.required_non_claims.len() > policy.bounds.max_non_claims {
        return Err(PolicyError::InvalidField(
            STORE_PROJECTION_FIELDS.required_non_claims,
        ));
    }
    if policy.project_config.required_non_claims.len() > policy.bounds.max_non_claims {
        return Err(PolicyError::InvalidField(
            PROJECT_PROJECTION_FIELDS.required_non_claims,
        ));
    }
    if policy.receipts.required_non_claims != policy.required_non_claims {
        return Err(PolicyError::InvalidField(FIELD_RECEIPT_NON_CLAIMS));
    }
    if policy.worksets.minimum_members > policy.bounds.max_workset_members {
        return Err(PolicyError::InvalidField(FIELD_MINIMUM_MEMBERS));
    }
    if policy.opener_profiles.iter().any(|profile| {
        profile.id.len() > policy.bounds.max_workset_id_bytes
            || profile.executable.len() > policy.bounds.max_path_bytes
            || profile
                .fixed_args
                .iter()
                .any(|argument| argument.len() > policy.bounds.max_path_bytes)
    }) {
        return Err(PolicyError::InvalidField(FIELD_OPENER_PROFILES));
    }
    if policy.locator.relative_path.len() > policy.bounds.max_path_bytes {
        return Err(PolicyError::InvalidField(LOCATOR_FIELDS.relative_path));
    }
    if policy.worksets.state.relative_path.len() > policy.bounds.max_path_bytes {
        return Err(PolicyError::InvalidField(
            WORKSET_STATE_FIELDS.relative_path,
        ));
    }
    Ok(())
}

fn object_field<'a>(
    value: &'a Value,
    lookup: &str,
    field: &'static str,
) -> Result<&'a Value, PolicyError> {
    let selected = value.get(lookup).ok_or(PolicyError::MissingField(field))?;
    if !selected.is_object() {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(selected)
}

fn array_field<'a>(
    value: &'a Value,
    lookup: &str,
    field: &'static str,
) -> Result<&'a Vec<Value>, PolicyError> {
    value
        .get(lookup)
        .ok_or(PolicyError::MissingField(field))?
        .as_array()
        .ok_or(PolicyError::InvalidField(field))
}

fn string_field(value: &Value, lookup: &str, field: &'static str) -> Result<String, PolicyError> {
    let text = value
        .get(lookup)
        .ok_or(PolicyError::MissingField(field))?
        .as_str()
        .ok_or(PolicyError::InvalidField(field))?;
    if text.is_empty() {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(text.to_string())
}

fn exact_string_field(
    value: &Value,
    lookup: &str,
    field: &'static str,
    expected: &str,
) -> Result<String, PolicyError> {
    let actual = string_field(value, lookup, field)?;
    if actual != expected {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(actual)
}

fn bool_field(value: &Value, lookup: &str, field: &'static str) -> Result<bool, PolicyError> {
    value
        .get(lookup)
        .ok_or(PolicyError::MissingField(field))?
        .as_bool()
        .ok_or(PolicyError::InvalidField(field))
}

fn exact_bool_field(
    value: &Value,
    lookup: &str,
    field: &'static str,
    expected: bool,
) -> Result<bool, PolicyError> {
    let actual = bool_field(value, lookup, field)?;
    if actual != expected {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(actual)
}

fn positive_usize_field(
    value: &Value,
    lookup: &str,
    field: &'static str,
) -> Result<usize, PolicyError> {
    let raw = value
        .get(lookup)
        .ok_or(PolicyError::MissingField(field))?
        .as_u64()
        .ok_or(PolicyError::InvalidField(field))?;
    if raw == 0 {
        return Err(PolicyError::InvalidField(field));
    }
    usize::try_from(raw).map_err(|_| PolicyError::InvalidField(field))
}

fn string_array_field(
    value: &Value,
    lookup: &str,
    field: &'static str,
) -> Result<Vec<String>, PolicyError> {
    array_field(value, lookup, field)?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .ok_or(PolicyError::InvalidField(field))
        })
        .collect()
}

fn nonempty_string_array_field(
    value: &Value,
    lookup: &str,
    field: &'static str,
) -> Result<Vec<String>, PolicyError> {
    let values = string_array_field(value, lookup, field)?;
    if values.is_empty() {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(values)
}

fn validate_unique(values: &[String], field: &'static str) -> Result<(), PolicyError> {
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(())
}

fn validate_string_set(
    values: &[String],
    expected: &[&str],
    field: &'static str,
) -> Result<(), PolicyError> {
    validate_unique(values, field)?;
    if values.len() != expected.len()
        || values
            .iter()
            .any(|value| !expected.contains(&value.as_str()))
    {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(())
}

fn same_ordered_values(values: &[String], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn is_store_id(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_safe_executable(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
}

fn is_safe_argument(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b'/' | b':' | b'=' | b'+' | b'-')
        })
}

fn is_issue_code(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("lifecycle_store.") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}
