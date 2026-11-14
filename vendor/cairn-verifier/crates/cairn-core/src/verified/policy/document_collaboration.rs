use std::collections::BTreeSet;

use serde_json::Value;

use super::PolicyError;

pub const DOCUMENT_COLLABORATION_SCHEMA_V1: &str = "cairn.document-collaboration-policy.v1";
pub const DOCUMENT_KIND_RFD: &str = "rfd";
pub const DOCUMENT_KIND_ADR: &str = "adr";

// r[impl cairn.document_collaboration.policy]
// r[impl cairn.document_collaboration.policy.deterministic]
// r[impl cairn.document_collaboration.policy.invalid]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentCollaborationPolicy {
    pub schema: String,
    pub kinds: Vec<String>,
    pub max_document_id_bytes: usize,
    pub max_artifacts: usize,
    pub max_discussion_messages: usize,
    pub max_discussion_text_bytes: usize,
    pub max_links: usize,
    pub max_revisions_per_artifact: usize,
    pub max_path_bytes: usize,
    pub max_number: u64,
    pub required_non_claims: Vec<String>,
}

pub(super) fn document_collaboration_policy(
    value: &Value,
) -> Result<DocumentCollaborationPolicy, PolicyError> {
    let field = value
        .get("document_collaboration_policy")
        .ok_or(PolicyError::MissingField("document_collaboration_policy"))?;
    let schema = str_field(field, "schema", "document_collaboration_policy.schema")?;
    if schema != DOCUMENT_COLLABORATION_SCHEMA_V1 {
        return Err(PolicyError::UnsupportedPolicySchema {
            found: schema,
            expected: DOCUMENT_COLLABORATION_SCHEMA_V1,
        });
    }
    let kinds = str_array_field(field, "kinds")?;
    let kind_set = kinds.iter().collect::<BTreeSet<_>>();
    if kinds.is_empty()
        || kinds.len() != kind_set.len()
        || kinds
            .iter()
            .any(|k| k != DOCUMENT_KIND_RFD && k != DOCUMENT_KIND_ADR)
    {
        return Err(PolicyError::InvalidField(
            "document_collaboration_policy.kinds",
        ));
    }
    let profile = field.get("profile").ok_or(PolicyError::MissingField(
        "document_collaboration_policy.profile",
    ))?;
    let bounds = profile.get("bounds").ok_or(PolicyError::MissingField(
        "document_collaboration_policy.profile.bounds",
    ))?;
    let max_document_id_bytes = pos_usize(bounds, "max_document_id_bytes")?;
    let max_artifacts = pos_usize(bounds, "max_artifacts")?;
    let max_discussion_messages = pos_usize(bounds, "max_discussion_messages")?;
    let max_discussion_text_bytes = pos_usize(bounds, "max_discussion_text_bytes")?;
    let max_links = pos_usize(bounds, "max_links")?;
    let max_revisions_per_artifact = pos_usize(bounds, "max_revisions_per_artifact")?;
    let max_path_bytes = pos_usize(bounds, "max_path_bytes")?;
    let max_number = pos_u64(bounds, "max_number")?;
    let required_non_claims = str_array_field(profile, "required_non_claims")?;
    if required_non_claims.iter().any(|value| value.is_empty()) {
        return Err(PolicyError::InvalidField(
            "document_collaboration_policy.profile.required_non_claims",
        ));
    }
    Ok(DocumentCollaborationPolicy {
        schema,
        kinds,
        max_document_id_bytes,
        max_artifacts,
        max_discussion_messages,
        max_discussion_text_bytes,
        max_links,
        max_revisions_per_artifact,
        max_path_bytes,
        max_number,
        required_non_claims,
    })
}

fn str_field(value: &Value, key: &str, field: &'static str) -> Result<String, PolicyError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(PolicyError::MissingField(field))
}

fn str_array_field(value: &Value, key: &str) -> Result<Vec<String>, PolicyError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .ok_or(PolicyError::MissingField(
            "document_collaboration_policy collection",
        ))
}

fn pos_usize(value: &Value, key: &str) -> Result<usize, PolicyError> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(PolicyError::MissingField(
            "document_collaboration_policy.profile.bounds",
        ))?;
    if raw == 0 {
        return Err(PolicyError::InvalidField(
            "document_collaboration_policy.profile.bounds",
        ));
    }
    usize::try_from(raw)
        .map_err(|_| PolicyError::InvalidField("document_collaboration_policy.profile.bounds"))
}

fn pos_u64(value: &Value, key: &str) -> Result<u64, PolicyError> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(PolicyError::MissingField(
            "document_collaboration_policy.profile.bounds",
        ))?;
    if raw == 0 {
        return Err(PolicyError::InvalidField(
            "document_collaboration_policy.profile.bounds",
        ));
    }
    Ok(raw)
}
