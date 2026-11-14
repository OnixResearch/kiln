use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

use serde_json::Value;

use super::model::*;

const TEMPLATE_OPEN: &str = "{{";
const TEMPLATE_CLOSE: &str = "}}";
const TEMPLATE_TOKEN_CHANGE: &str = "change";
const TEMPLATE_TOKEN_ARTIFACT: &str = "artifact";
const PREREQUISITE_GATE_PREFIX: &str = "gate:";
const PREREQUISITE_ARTIFACT_PREFIX: &str = "artifact:";

// r[impl cairn.artifact_workflow_contract.policy]
// r[impl cairn.artifact_workflow_contract.policy.valid]
// r[impl cairn.artifact_workflow_contract.functional_boundary]
// r[impl cairn.artifact_workflow_contract.functional_boundary.pure]
pub fn workflow_schemas_from_value(
    value: &Value,
) -> Result<Vec<WorkflowSchemaPolicy>, WorkflowAdmissionError> {
    let schemas_value = value.get("schemas").cloned().ok_or_else(|| {
        admission_error(
            WORKFLOW_ISSUE_POLICY_SHAPE,
            "schemas",
            "workflow schemas are missing",
        )
    })?;
    let schemas =
        serde_json::from_value::<Vec<WorkflowSchemaPolicy>>(schemas_value).map_err(|error| {
            admission_error(
                WORKFLOW_ISSUE_POLICY_SHAPE,
                "schemas",
                format!("workflow schema shape is invalid: {error}"),
            )
        })?;
    let allowed_stages = value
        .get("lifecycle_stages")
        .and_then(Value::as_array)
        .map(|stages| {
            stages
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    admit_workflow_schemas(&schemas, &allowed_stages)?;
    Ok(schemas)
}

pub fn workflow_profile_registry_from_value(
    value: &Value,
    schemas: &[WorkflowSchemaPolicy],
) -> Result<WorkflowProfileRegistryPolicy, WorkflowAdmissionError> {
    // r[impl cairn.lifecycle_workflow_profiles.registry]
    // r[impl cairn.lifecycle_workflow_profiles.boundary]
    let registry_value = value
        .get("workflow_profile_policy")
        .cloned()
        .ok_or_else(|| {
            admission_error(
                WORKFLOW_ISSUE_PROFILE_REGISTRY,
                "workflow_profile_policy",
                "workflow profile policy is missing",
            )
        })?;
    let registry = serde_json::from_value::<WorkflowProfileRegistryPolicy>(registry_value)
        .map_err(|error| {
            admission_error(
                WORKFLOW_ISSUE_PROFILE_REGISTRY,
                "workflow_profile_policy",
                format!("workflow profile policy shape is invalid: {error}"),
            )
        })?;
    admit_workflow_profile_registry(&registry, schemas)?;
    Ok(registry)
}

pub fn admit_workflow_profile_registry(
    registry: &WorkflowProfileRegistryPolicy,
    schemas: &[WorkflowSchemaPolicy],
) -> Result<(), WorkflowAdmissionError> {
    let known = schemas
        .iter()
        .map(|schema| schema.id.as_str())
        .collect::<BTreeSet<_>>();
    if !known.contains(registry.default_profile.as_str()) {
        return Err(profile_registry_error(
            "default_profile",
            "default workflow profile is unknown",
        ));
    }
    if registry.allowed_profiles.is_empty() {
        return Err(profile_registry_error(
            "allowed_profiles",
            "at least one workflow profile must be allowed",
        ));
    }
    let mut allowed = BTreeSet::new();
    for profile in &registry.allowed_profiles {
        if !known.contains(profile.as_str()) {
            return Err(profile_registry_error(
                "allowed_profiles",
                format!("allowed workflow profile is unknown: {profile}"),
            ));
        }
        if !allowed.insert(profile.as_str()) {
            return Err(profile_registry_error(
                "allowed_profiles",
                format!("allowed workflow profile is duplicated: {profile}"),
            ));
        }
    }
    if !allowed.contains(registry.default_profile.as_str()) {
        return Err(profile_registry_error(
            "default_profile",
            "default workflow profile must be repository-allowed",
        ));
    }
    validate_effect_profile_mappings(registry, schemas)?;
    validate_forbidden_profile_sources(registry)?;
    validate_initial_profiles(schemas)?;
    Ok(())
}

pub fn admit_workflow_schemas(
    schemas: &[WorkflowSchemaPolicy],
    allowed_stages: &[String],
) -> Result<(), WorkflowAdmissionError> {
    if schemas.is_empty() || schemas.len() > MAX_WORKFLOW_SCHEMAS_LIMIT {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            "schemas",
            "workflow schema count is outside the supported bounds",
        ));
    }
    let mut ids = BTreeSet::new();
    for (schema_index, schema) in schemas.iter().enumerate() {
        let schema_path = format!("schemas[{schema_index}]");
        if !ids.insert(schema.id.clone()) {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_DUPLICATE_SCHEMA,
                format!("{schema_path}.id"),
                format!("duplicate workflow schema id: {}", schema.id),
            ));
        }
        admit_schema(schema, allowed_stages, &schema_path)?;
    }
    Ok(())
}

pub fn workflow_topological_order(
    schema: &WorkflowSchemaPolicy,
) -> Result<Vec<String>, WorkflowAdmissionError> {
    let mut indegree = schema
        .artifacts
        .iter()
        .map(|artifact| (artifact.id.clone(), artifact.requires.len()))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut order = Vec::with_capacity(schema.artifacts.len());

    loop {
        let next = schema
            .artifacts
            .iter()
            .find(|artifact| {
                !emitted.contains(&artifact.id) && indegree.get(&artifact.id).copied() == Some(0)
            })
            .map(|artifact| artifact.id.clone());
        let Some(next) = next else {
            break;
        };
        assert!(emitted.insert(next.clone()));
        order.push(next.clone());
        for artifact in &schema.artifacts {
            if artifact.requires.iter().any(|required| required == &next) {
                let degree = indegree.get_mut(&artifact.id).ok_or_else(|| {
                    admission_error(
                        WORKFLOW_ISSUE_POLICY_UNKNOWN_DEPENDENCY,
                        format!("schemas.{}.artifacts.{}.requires", schema.id, artifact.id),
                        "topological admission lost an artifact",
                    )
                })?;
                *degree = degree.checked_sub(1).ok_or_else(|| {
                    admission_error(
                        WORKFLOW_ISSUE_POLICY_CYCLE,
                        format!("schemas.{}.artifacts", schema.id),
                        "artifact dependency indegree underflow",
                    )
                })?;
            }
        }
    }

    if order.len() != schema.artifacts.len() {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_CYCLE,
            format!("schemas.{}.artifacts", schema.id),
            "workflow artifact dependencies contain a cycle",
        ));
    }
    Ok(order)
}

fn admit_schema(
    schema: &WorkflowSchemaPolicy,
    allowed_stages: &[String],
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    validate_schema_header(schema, path)?;
    validate_bounds(&schema.bounds, path)?;
    validate_artifacts(schema, allowed_stages, path)?;
    validate_review_obligations(schema, path)?;
    validate_profile_shape(schema, path)?;
    workflow_topological_order(schema)?;
    Ok(())
}

fn validate_schema_header(
    schema: &WorkflowSchemaPolicy,
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    if schema.schema_version != WORKFLOW_SCHEMA_VERSION_V1 {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_SCHEMA_VERSION,
            format!("{path}.schema_version"),
            format!(
                "unsupported workflow schema version: {}",
                schema.schema_version
            ),
        ));
    }
    if schema.id.is_empty()
        || schema.description.is_empty()
        || schema.profile_version == 0
        || schema.receipt_fields.is_empty()
    {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_SHAPE,
            path,
            "workflow profile id, version, description, and receipt fields must be present",
        ));
    }
    if !matches!(
        schema.spec_effect.as_str(),
        WORKFLOW_SPEC_EFFECT_DELTA | WORKFLOW_SPEC_EFFECT_NONE
    ) {
        return Err(admission_error(
            WORKFLOW_ISSUE_PROFILE_EFFECT,
            format!("{path}.spec_effect"),
            "workflow profile spec effect is unsupported",
        ));
    }
    if !matches!(
        schema.task_link_class.as_str(),
        WORKFLOW_TASK_LINK_REQUIREMENT | WORKFLOW_TASK_LINK_ACCEPTANCE
    ) {
        return Err(admission_error(
            WORKFLOW_ISSUE_PROFILE_REGISTRY,
            format!("{path}.task_link_class"),
            "workflow profile task-link class is unsupported",
        ));
    }
    validate_unique_nonempty(
        &schema.receipt_fields,
        &format!("{path}.receipt_fields"),
        "receipt field",
    )?;
    for required in [
        "workflow_profile",
        "workflow_profile_version",
        "spec_effect",
        "non_claims",
    ] {
        if !schema.receipt_fields.iter().any(|field| field == required) {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REGISTRY,
                format!("{path}.receipt_fields"),
                format!("workflow profile receipt fields must include {required}"),
            ));
        }
    }
    let expected_states = WORKFLOW_SUPPORTED_STATES.map(ToOwned::to_owned).to_vec();
    if schema.states != expected_states {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_STATES,
            format!("{path}.states"),
            "workflow states must use the supported deterministic order",
        ));
    }
    if schema.required_non_claims.is_empty()
        || schema.required_non_claims.len() > schema.bounds.max_non_claims
    {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            format!("{path}.required_non_claims"),
            "workflow non-claim count is outside the schema bounds",
        ));
    }
    Ok(())
}

fn validate_bounds(bounds: &WorkflowBounds, path: &str) -> Result<(), WorkflowAdmissionError> {
    let checks = [
        (bounds.max_artifacts, MAX_WORKFLOW_ARTIFACTS_LIMIT),
        (bounds.max_dependencies, MAX_WORKFLOW_DEPENDENCIES_LIMIT),
        (bounds.max_files_per_artifact, MAX_WORKFLOW_FILES_LIMIT),
        (bounds.max_path_bytes, MAX_WORKFLOW_PATH_BYTES_LIMIT),
        (bounds.max_artifact_bytes, MAX_WORKFLOW_ARTIFACT_BYTES_LIMIT),
        (bounds.max_template_bytes, MAX_WORKFLOW_TEMPLATE_BYTES_LIMIT),
        (bounds.max_constraints, MAX_WORKFLOW_CONSTRAINTS_LIMIT),
        (bounds.max_non_claims, MAX_WORKFLOW_NON_CLAIMS_LIMIT),
        (bounds.max_issues, MAX_WORKFLOW_ISSUES_LIMIT),
        (
            bounds.max_directory_depth,
            MAX_WORKFLOW_DIRECTORY_DEPTH_LIMIT,
        ),
        (
            bounds.max_directory_entries,
            MAX_WORKFLOW_DIRECTORY_ENTRIES_LIMIT,
        ),
    ];
    if checks
        .into_iter()
        .any(|(selected, limit)| selected == 0 || selected > limit)
    {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            format!("{path}.bounds"),
            "workflow bounds must be positive and within implementation limits",
        ));
    }
    Ok(())
}

fn validate_artifacts(
    schema: &WorkflowSchemaPolicy,
    allowed_stages: &[String],
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    if schema.artifacts.is_empty() || schema.artifacts.len() > schema.bounds.max_artifacts {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            format!("{path}.artifacts"),
            "workflow artifact count is outside the schema bounds",
        ));
    }
    let known = collect_artifact_ids(schema, path)?;
    for (artifact_index, artifact) in schema.artifacts.iter().enumerate() {
        let artifact_path = format!("{path}.artifacts[{artifact_index}]");
        validate_artifact_shape(schema, artifact, allowed_stages, &artifact_path)?;
        validate_artifact_dependencies(schema, artifact, &known, &artifact_path)?;
        validate_prerequisites(artifact, &known, allowed_stages, &artifact_path)?;
    }
    Ok(())
}

fn collect_artifact_ids(
    schema: &WorkflowSchemaPolicy,
    path: &str,
) -> Result<BTreeSet<String>, WorkflowAdmissionError> {
    let mut ids = BTreeSet::new();
    for artifact in &schema.artifacts {
        if artifact.id.is_empty() || !ids.insert(artifact.id.clone()) {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_DUPLICATE_ARTIFACT,
                format!("{path}.artifacts"),
                format!("duplicate or empty artifact id: {}", artifact.id),
            ));
        }
    }
    Ok(ids)
}

fn validate_artifact_shape(
    schema: &WorkflowSchemaPolicy,
    artifact: &WorkflowArtifactPolicy,
    allowed_stages: &[String],
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    validate_safe_path(
        &artifact.output_path,
        schema.bounds.max_path_bytes,
        path,
        false,
    )?;
    validate_safe_path(
        &artifact.generates,
        schema.bounds.max_path_bytes,
        path,
        true,
    )?;
    if !matches!(
        artifact.path_kind.as_str(),
        WORKFLOW_PATH_KIND_FILE | WORKFLOW_PATH_KIND_SPEC_DIRECTORY
    ) {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_PATH_KIND,
            format!("{path}.path_kind"),
            format!("unsupported path kind: {}", artifact.path_kind),
        ));
    }
    if !matches!(
        artifact.structural_admission.as_str(),
        WORKFLOW_ADMISSION_PROPOSAL
            | WORKFLOW_ADMISSION_DELTA_SPECS
            | WORKFLOW_ADMISSION_DESIGN
            | WORKFLOW_ADMISSION_TASKS
            | WORKFLOW_ADMISSION_MARKDOWN
            | WORKFLOW_ADMISSION_ACCEPTANCE
            | WORKFLOW_ADMISSION_RISK
            | WORKFLOW_ADMISSION_VERIFICATION
            | WORKFLOW_ADMISSION_SEMANTIC_REVIEW
    ) {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_ADMISSION,
            format!("{path}.structural_admission"),
            format!(
                "unsupported structural admission: {}",
                artifact.structural_admission
            ),
        ));
    }
    if !allowed_stages.iter().any(|stage| stage == &artifact.stage) {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_STAGE,
            format!("{path}.stage"),
            format!("unknown lifecycle stage: {}", artifact.stage),
        ));
    }
    if artifact.constraints.len() > schema.bounds.max_constraints {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            format!("{path}.constraints"),
            "artifact constraint count exceeds the schema bound",
        ));
    }
    validate_template(
        &artifact.instruction_template,
        schema.bounds.max_template_bytes,
        path,
    )
}

fn validate_artifact_dependencies(
    schema: &WorkflowSchemaPolicy,
    artifact: &WorkflowArtifactPolicy,
    known: &BTreeSet<String>,
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    if artifact.requires.len() > schema.bounds.max_dependencies {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_BOUNDS,
            format!("{path}.requires"),
            "artifact dependency count exceeds the schema bound",
        ));
    }
    let mut unique = BTreeSet::new();
    for required in &artifact.requires {
        if required == &artifact.id {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_SELF_DEPENDENCY,
                format!("{path}.requires"),
                format!("artifact {} depends on itself", artifact.id),
            ));
        }
        if !unique.insert(required.clone()) {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_DUPLICATE_DEPENDENCY,
                format!("{path}.requires"),
                format!("duplicate artifact dependency: {required}"),
            ));
        }
        if !known.contains(required) {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_UNKNOWN_DEPENDENCY,
                format!("{path}.requires"),
                format!("unknown artifact dependency: {required}"),
            ));
        }
    }
    Ok(())
}

fn validate_prerequisites(
    artifact: &WorkflowArtifactPolicy,
    known: &BTreeSet<String>,
    allowed_stages: &[String],
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    for prerequisite in &artifact.apply_prerequisites {
        let is_known_gate = prerequisite
            .strip_prefix(PREREQUISITE_GATE_PREFIX)
            .is_some_and(|stage| allowed_stages.iter().any(|known| known == stage));
        let is_known_artifact = prerequisite
            .strip_prefix(PREREQUISITE_ARTIFACT_PREFIX)
            .is_some_and(|id| known.contains(id));
        if !is_known_gate && !is_known_artifact {
            return Err(admission_error(
                WORKFLOW_ISSUE_POLICY_PREREQUISITE,
                format!("{path}.apply_prerequisites"),
                format!("unsupported apply prerequisite: {prerequisite}"),
            ));
        }
    }
    Ok(())
}

fn validate_safe_path(
    value: &str,
    max_bytes: usize,
    path: &str,
    allow_glob: bool,
) -> Result<(), WorkflowAdmissionError> {
    let has_forbidden_component = Path::new(value).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    let has_forbidden_text = value.is_empty()
        || value.len() > max_bytes
        || value.contains('\\')
        || value.contains('\0')
        || (!allow_glob && (value.contains('*') || value.contains('?')));
    if has_forbidden_component || has_forbidden_text {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_UNSAFE_PATH,
            format!("{path}.output_path"),
            format!("unsafe workflow artifact path: {value}"),
        ));
    }
    Ok(())
}

fn validate_template(
    template: &str,
    max_bytes: usize,
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    if template.is_empty() || template.len() > max_bytes || template.contains('\0') {
        return Err(admission_error(
            WORKFLOW_ISSUE_POLICY_TEMPLATE,
            format!("{path}.instruction_template"),
            "instruction template is empty or exceeds its bound",
        ));
    }
    let mut remaining = template;
    loop {
        let Some(open_index) = remaining.find(TEMPLATE_OPEN) else {
            if remaining.contains(TEMPLATE_CLOSE) {
                return Err(template_error(
                    path,
                    "instruction template has an unmatched close",
                ));
            }
            return Ok(());
        };
        if remaining[..open_index].contains(TEMPLATE_CLOSE) {
            return Err(template_error(
                path,
                "instruction template has an unmatched close",
            ));
        }
        let token_start = open_index + TEMPLATE_OPEN.len();
        let Some(close_offset) = remaining[token_start..].find(TEMPLATE_CLOSE) else {
            return Err(template_error(
                path,
                "instruction template has an unmatched open",
            ));
        };
        let token_end = token_start + close_offset;
        let token = &remaining[token_start..token_end];
        if !matches!(token, TEMPLATE_TOKEN_CHANGE | TEMPLATE_TOKEN_ARTIFACT) {
            return Err(template_error(
                path,
                format!("instruction template uses an unknown token: {token}"),
            ));
        }
        remaining = &remaining[token_end + TEMPLATE_CLOSE.len()..];
    }
}

fn validate_review_obligations(
    schema: &WorkflowSchemaPolicy,
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    let known = schema
        .artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut obligation_ids = BTreeSet::new();
    for (index, obligation) in schema.review_obligations.iter().enumerate() {
        let obligation_path = format!("{path}.review_obligations[{index}]");
        if obligation.id.is_empty() || !obligation_ids.insert(obligation.id.as_str()) {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                format!("{obligation_path}.id"),
                "workflow review obligation IDs must be non-empty and unique",
            ));
        }
        let review_artifact = schema
            .artifacts
            .iter()
            .find(|artifact| artifact.id == obligation.artifact_id);
        if review_artifact.is_none_or(|artifact| {
            artifact.structural_admission != WORKFLOW_ADMISSION_SEMANTIC_REVIEW
        }) {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                format!("{obligation_path}.artifact_id"),
                "workflow review obligation must reference a semantic-review artifact",
            ));
        }
        if !matches!(
            obligation.review_kind.as_str(),
            "plan" | "design" | "code" | "diff"
        ) {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                format!("{obligation_path}.review_kind"),
                "workflow review obligation uses an unsupported semantic review kind",
            ));
        }
        if obligation.required_artifacts.is_empty() {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                format!("{obligation_path}.required_artifacts"),
                "workflow review obligation must bind at least one artifact",
            ));
        }
        validate_unique_nonempty(
            &obligation.required_artifacts,
            &format!("{obligation_path}.required_artifacts"),
            "review artifact",
        )?;
        if let Some(unknown) = obligation
            .required_artifacts
            .iter()
            .find(|artifact| !known.contains(artifact.as_str()))
        {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                format!("{obligation_path}.required_artifacts"),
                format!("workflow review obligation references unknown artifact: {unknown}"),
            ));
        }
    }
    Ok(())
}

fn validate_profile_shape(
    schema: &WorkflowSchemaPolicy,
    path: &str,
) -> Result<(), WorkflowAdmissionError> {
    let admissions = schema
        .artifacts
        .iter()
        .map(|artifact| artifact.structural_admission.as_str())
        .collect::<BTreeSet<_>>();
    let has_delta_specs = admissions.contains(WORKFLOW_ADMISSION_DELTA_SPECS);
    let has_acceptance = admissions.contains(WORKFLOW_ADMISSION_ACCEPTANCE);
    if schema.spec_effect == WORKFLOW_SPEC_EFFECT_NONE {
        if has_delta_specs
            || !has_acceptance
            || schema.task_link_class != WORKFLOW_TASK_LINK_ACCEPTANCE
            || schema.review_obligations.is_empty()
        {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REGISTRY,
                path,
                "no-spec workflow profiles must forbid delta specs and require acceptance links and review",
            ));
        }
    } else if !has_delta_specs || schema.task_link_class != WORKFLOW_TASK_LINK_REQUIREMENT {
        return Err(admission_error(
            WORKFLOW_ISSUE_PROFILE_REGISTRY,
            path,
            "spec-mutating workflow profiles must require delta specs and requirement task links",
        ));
    }
    Ok(())
}

fn validate_effect_profile_mappings(
    registry: &WorkflowProfileRegistryPolicy,
    schemas: &[WorkflowSchemaPolicy],
) -> Result<(), WorkflowAdmissionError> {
    let mut effects = BTreeSet::new();
    for mapping in &registry.required_profiles_by_effect {
        if !matches!(
            mapping.effect.as_str(),
            WORKFLOW_SPEC_EFFECT_DELTA | WORKFLOW_SPEC_EFFECT_NONE
        ) || !effects.insert(mapping.effect.as_str())
        {
            return Err(profile_registry_error(
                "required_profiles_by_effect",
                "effect-to-profile mappings must use unique supported effects",
            ));
        }
        let Some(schema) = schemas.iter().find(|schema| schema.id == mapping.profile) else {
            return Err(profile_registry_error(
                "required_profiles_by_effect",
                format!(
                    "effect mapping references unknown profile: {}",
                    mapping.profile
                ),
            ));
        };
        if schema.spec_effect != mapping.effect {
            return Err(profile_registry_error(
                "required_profiles_by_effect",
                "effect mapping cannot select a profile with a different declared effect",
            ));
        }
    }
    Ok(())
}

fn validate_forbidden_profile_sources(
    registry: &WorkflowProfileRegistryPolicy,
) -> Result<(), WorkflowAdmissionError> {
    let required = [
        "user-global",
        "environment",
        "agent-prompt",
        "remote-package",
    ];
    for source in required {
        if !registry
            .forbidden_selection_sources
            .iter()
            .any(|candidate| candidate == source)
        {
            return Err(profile_registry_error(
                "forbidden_selection_sources",
                format!("workflow profile policy must forbid ambient source: {source}"),
            ));
        }
    }
    validate_unique_nonempty(
        &registry.forbidden_selection_sources,
        "workflow_profile_policy.forbidden_selection_sources",
        "forbidden source",
    )
}

fn validate_initial_profiles(
    schemas: &[WorkflowSchemaPolicy],
) -> Result<(), WorkflowAdmissionError> {
    for required in ["spec-driven", "assurance", "no-spec-delta"] {
        if !schemas.iter().any(|schema| schema.id == required) {
            return Err(profile_registry_error(
                "schemas",
                format!("initial workflow profile is missing: {required}"),
            ));
        }
    }
    Ok(())
}

fn validate_unique_nonempty(
    values: &[String],
    path: &str,
    label: &str,
) -> Result<(), WorkflowAdmissionError> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.is_empty() || !unique.insert(value.as_str()) {
            return Err(admission_error(
                WORKFLOW_ISSUE_PROFILE_REGISTRY,
                path,
                format!("{label} values must be non-empty and unique"),
            ));
        }
    }
    Ok(())
}

fn profile_registry_error(field: &str, message: impl Into<String>) -> WorkflowAdmissionError {
    admission_error(
        WORKFLOW_ISSUE_PROFILE_REGISTRY,
        format!("workflow_profile_policy.{field}"),
        message,
    )
}

fn template_error(path: &str, message: impl Into<String>) -> WorkflowAdmissionError {
    admission_error(
        WORKFLOW_ISSUE_POLICY_TEMPLATE,
        format!("{path}.instruction_template"),
        message,
    )
}

fn admission_error(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> WorkflowAdmissionError {
    WorkflowAdmissionError::new(code, path, message)
}
