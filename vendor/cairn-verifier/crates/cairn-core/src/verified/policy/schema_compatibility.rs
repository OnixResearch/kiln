use serde_json::Value;

use super::PolicyError;

pub const POLICY_SCHEMA_METADATA_FIELD: &str = "policy_schema_compatibility";
pub const CURRENT_POLICY_SCHEMA_VERSION: &str = "cairn.policy-schema.v1";
pub const CURRENT_POLICY_FIELD_SET_DIGEST_MATERIAL: &str = concat!(
    "agent_completion_policy\n",
    "agent_guidance_policy\n",
    "authenticated_evidence_policy\n",
    "cross_repo_dependency_policy\n",
    "developer_loop_policy\n",
    "document_collaboration_policy\n",
    "generated_agent_workflow_policy\n",
    "global_registry_policy\n",
    "lifecycle_store_policy\n",
    "machine_contract_policy\n",
    "nominal_identity_policy\n",
    "policy_schema_compatibility\n",
    "runtime_evidence_policy\n",
    "steel_orchestration_policy\n",
    "traceability_policy\n",
    "workflow_profile_policy\n",
);
pub const CURRENT_POLICY_FIELD_SET_DIGEST: &str =
    "9f44153637df26127588d81e70bb2095f07f64c4e7d5c009608f6c70b6ca961a";
pub const DEFAULT_POLICY_REFRESH_COMMAND: &str =
    "nix run path:$CAIRN_SOURCE#cairn -- policy export --output cairn-policy/generated/cairn-policy.json";

const BLAKE3_HEX_LENGTH: usize = 64;
const FIELD_SCHEMA_VERSION: &str = "schema_version";
const FIELD_PRODUCER: &str = "producer";
const FIELD_FIELD_SET_DIGEST: &str = "field_set_digest";
const FIELD_REFRESH_COMMAND: &str = "refresh_command";
const COMPAT_CURRENT: &str = "current";
const COMPAT_COMPATIBLE_WITH_WARNING: &str = "compatible-with-warning";
const COMPAT_REFRESH_REQUIRED: &str = "refresh-required";
const COMPAT_UNSUPPORTED: &str = "unsupported";
const DIAGNOSTIC_MISSING_METADATA: &str = "policy-metadata-missing";
const DIAGNOSTIC_MALFORMED_METADATA: &str = "policy-metadata-malformed";
const DIAGNOSTIC_UNSUPPORTED_SCHEMA: &str = "policy-schema-unsupported";
const DIAGNOSTIC_STALE_FIELD_SET: &str = "policy-refresh-required";

const REQUIRED_METADATA_FIELDS: &[&str] = &[
    FIELD_SCHEMA_VERSION,
    FIELD_PRODUCER,
    FIELD_FIELD_SET_DIGEST,
    FIELD_REFRESH_COMMAND,
];

const REFRESH_REQUIRED_FIELDS: &[&str] = &[
    "agent_completion_policy",
    "agent_guidance_policy",
    "developer_loop_policy",
    "document_collaboration_policy",
    "generated_agent_workflow_policy",
    "authenticated_evidence_policy",
    "cross_repo_dependency_policy",
    "global_registry_policy",
    "lifecycle_store_policy",
    "machine_contract_policy",
    "nominal_identity_policy",
    "policy_schema_compatibility",
    "runtime_evidence_policy",
    "steel_orchestration_policy",
    "traceability_policy",
    "workflow_profile_policy",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySchemaCompatibilityMetadata {
    pub schema_version: String,
    pub producer: String,
    pub field_set_digest: String,
    pub refresh_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySchemaCompatibilityReport {
    pub status: String,
    pub diagnostic_code: Option<String>,
    pub missing_field: Option<String>,
    pub schema_version: Option<String>,
    pub field_set_digest: Option<String>,
    pub refresh_command: Option<String>,
}

#[must_use]
pub fn default_policy_schema_metadata() -> PolicySchemaCompatibilityMetadata {
    // r[impl cairn.policy_schema_compatibility.metadata]
    PolicySchemaCompatibilityMetadata {
        schema_version: CURRENT_POLICY_SCHEMA_VERSION.to_string(),
        producer: "cairn policy export".to_string(),
        field_set_digest: CURRENT_POLICY_FIELD_SET_DIGEST.to_string(),
        refresh_command: DEFAULT_POLICY_REFRESH_COMMAND.to_string(),
    }
}

pub fn policy_schema_metadata(
    value: &Value,
) -> Result<Option<PolicySchemaCompatibilityMetadata>, PolicyError> {
    let Some(metadata) = value.get(POLICY_SCHEMA_METADATA_FIELD) else {
        return Ok(None);
    };
    let schema_version = metadata_string_field(metadata, FIELD_SCHEMA_VERSION)?;
    let producer = metadata_string_field(metadata, FIELD_PRODUCER)?;
    let field_set_digest = metadata_string_field(metadata, FIELD_FIELD_SET_DIGEST)?;
    if !is_blake3_hex(&field_set_digest) {
        return Err(PolicyError::InvalidField(
            "policy_schema_compatibility.field_set_digest",
        ));
    }
    let refresh_command = metadata_string_field(metadata, FIELD_REFRESH_COMMAND)?;
    if schema_version != CURRENT_POLICY_SCHEMA_VERSION {
        return Err(PolicyError::UnsupportedPolicySchema {
            found: schema_version,
            expected: CURRENT_POLICY_SCHEMA_VERSION,
        });
    }
    Ok(Some(PolicySchemaCompatibilityMetadata {
        schema_version,
        producer,
        field_set_digest,
        refresh_command,
    }))
}

#[must_use]
pub fn classify_policy_schema_compatibility(
    value: &Value,
    parse_error: Option<&PolicyError>,
) -> PolicySchemaCompatibilityReport {
    if let Some(error) = parse_error {
        return classify_policy_schema_parse_error(value, error);
    }
    match policy_schema_metadata(value) {
        Ok(Some(metadata)) => current_report(metadata),
        Ok(None) => missing_metadata_report(),
        Err(PolicyError::UnsupportedPolicySchema { found, .. }) => unsupported_schema_report(found),
        Err(_) => malformed_metadata_report(),
    }
}

#[must_use]
pub fn policy_schema_compatibility_diagnostic(
    value: &Value,
    parse_error: &PolicyError,
) -> Option<String> {
    let report = classify_policy_schema_compatibility(value, Some(parse_error));
    (report.status == COMPAT_REFRESH_REQUIRED).then(|| {
        let field = report
            .missing_field
            .unwrap_or_else(|| "<unknown>".to_string());
        let refresh_command = report
            .refresh_command
            .unwrap_or_else(|| DEFAULT_POLICY_REFRESH_COMMAND.to_string());
        format!(
            "{parse_error}; diagnostic=policy-refresh-required; missing_field={field}; refresh_command={refresh_command}"
        )
    })
}

fn classify_policy_schema_parse_error(
    value: &Value,
    parse_error: &PolicyError,
) -> PolicySchemaCompatibilityReport {
    match parse_error {
        PolicyError::MissingField(field) if refresh_required_field(field) => {
            refresh_required_report((*field).to_string(), refresh_command_from_value(value))
        }
        PolicyError::UnsupportedPolicySchema { found, .. } => {
            unsupported_schema_report(found.clone())
        }
        _ => classify_policy_schema_compatibility(value, None),
    }
}

fn current_report(metadata: PolicySchemaCompatibilityMetadata) -> PolicySchemaCompatibilityReport {
    PolicySchemaCompatibilityReport {
        status: COMPAT_CURRENT.to_string(),
        diagnostic_code: None,
        missing_field: None,
        schema_version: Some(metadata.schema_version),
        field_set_digest: Some(metadata.field_set_digest),
        refresh_command: Some(metadata.refresh_command),
    }
}

fn missing_metadata_report() -> PolicySchemaCompatibilityReport {
    PolicySchemaCompatibilityReport {
        status: COMPAT_COMPATIBLE_WITH_WARNING.to_string(),
        diagnostic_code: Some(DIAGNOSTIC_MISSING_METADATA.to_string()),
        missing_field: Some(POLICY_SCHEMA_METADATA_FIELD.to_string()),
        schema_version: None,
        field_set_digest: None,
        refresh_command: Some(DEFAULT_POLICY_REFRESH_COMMAND.to_string()),
    }
}

fn malformed_metadata_report() -> PolicySchemaCompatibilityReport {
    PolicySchemaCompatibilityReport {
        status: COMPAT_UNSUPPORTED.to_string(),
        diagnostic_code: Some(DIAGNOSTIC_MALFORMED_METADATA.to_string()),
        missing_field: None,
        schema_version: None,
        field_set_digest: None,
        refresh_command: Some(DEFAULT_POLICY_REFRESH_COMMAND.to_string()),
    }
}

fn unsupported_schema_report(found: String) -> PolicySchemaCompatibilityReport {
    PolicySchemaCompatibilityReport {
        status: COMPAT_UNSUPPORTED.to_string(),
        diagnostic_code: Some(DIAGNOSTIC_UNSUPPORTED_SCHEMA.to_string()),
        missing_field: None,
        schema_version: Some(found),
        field_set_digest: None,
        refresh_command: Some(DEFAULT_POLICY_REFRESH_COMMAND.to_string()),
    }
}

fn refresh_required_report(
    missing_field: String,
    refresh_command: String,
) -> PolicySchemaCompatibilityReport {
    PolicySchemaCompatibilityReport {
        status: COMPAT_REFRESH_REQUIRED.to_string(),
        diagnostic_code: Some(DIAGNOSTIC_STALE_FIELD_SET.to_string()),
        missing_field: Some(missing_field),
        schema_version: None,
        field_set_digest: None,
        refresh_command: Some(refresh_command),
    }
}

fn metadata_string_field(value: &Value, field: &'static str) -> Result<String, PolicyError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| metadata_field_error(field))
}

fn metadata_field_error(field: &'static str) -> PolicyError {
    for required in REQUIRED_METADATA_FIELDS {
        if *required == field {
            return PolicyError::MissingField(metadata_field_path(field));
        }
    }
    PolicyError::MissingField(POLICY_SCHEMA_METADATA_FIELD)
}

fn metadata_field_path(field: &'static str) -> &'static str {
    match field {
        FIELD_SCHEMA_VERSION => "policy_schema_compatibility.schema_version",
        FIELD_PRODUCER => "policy_schema_compatibility.producer",
        FIELD_FIELD_SET_DIGEST => "policy_schema_compatibility.field_set_digest",
        FIELD_REFRESH_COMMAND => "policy_schema_compatibility.refresh_command",
        _ => POLICY_SCHEMA_METADATA_FIELD,
    }
}

fn refresh_required_field(field: &str) -> bool {
    REFRESH_REQUIRED_FIELDS.contains(&field)
}

fn refresh_command_from_value(value: &Value) -> String {
    value
        .get(POLICY_SCHEMA_METADATA_FIELD)
        .and_then(|metadata| metadata.get(FIELD_REFRESH_COMMAND))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_POLICY_REFRESH_COMMAND.to_string())
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}
