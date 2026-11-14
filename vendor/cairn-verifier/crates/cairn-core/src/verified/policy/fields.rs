use serde_json::Value;

use super::{
    ArtifactSubstancePolicy, ExemptionMarkerPolicy, PolicyError, ReceiptSchema, TaskMarker,
    SUPPORTED_POLICY_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskMarkerFields {
    pub(super) is_enabled: bool,
    pub(super) strictness: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GateFields {
    pub(super) stages: Vec<String>,
    pub(super) ignore_archives: bool,
    pub(super) finding_scope: String,
    pub(super) finding_promoted: String,
    pub(super) substance: ArtifactSubstancePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MutationPolicyFields {
    pub(super) sync_require_gate: bool,
    pub(super) archive_require_gate: bool,
    pub(super) migrate_require_manifest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChangeMetadataFields {
    pub(super) allowed_groups: Vec<String>,
    pub(super) group_prefixes: Vec<String>,
    pub(super) statuses: Vec<String>,
}

pub(super) fn version_field(value: &Value) -> Result<u64, PolicyError> {
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(PolicyError::MissingField("version"))?;
    if version != SUPPORTED_POLICY_VERSION {
        return Err(PolicyError::UnsupportedVersion {
            found: version,
            expected: SUPPORTED_POLICY_VERSION,
        });
    }
    Ok(version)
}

pub(super) fn require_nonempty_array(
    value: &Value,
    field: &'static str,
) -> Result<(), PolicyError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(PolicyError::MissingField(field))?;
    if values.is_empty() {
        return Err(PolicyError::InvalidField(field));
    }
    Ok(())
}

pub(super) fn task_marker_fields(value: &Value) -> Result<TaskMarkerFields, PolicyError> {
    let marker_policy = value
        .get("task_marker_policy")
        .ok_or(PolicyError::MissingField("task_marker_policy"))?;
    Ok(TaskMarkerFields {
        is_enabled: marker_policy
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or(PolicyError::MissingField("task_marker_policy.enabled"))?,
        strictness: string_field(marker_policy, "task_marker_policy.strictness")?,
    })
}

pub(super) fn task_markers(value: &Value) -> Result<Vec<TaskMarker>, PolicyError> {
    let marker_policy = value
        .get("task_marker_policy")
        .ok_or(PolicyError::MissingField("task_marker_policy"))?;
    let markers = marker_policy
        .get("markers")
        .and_then(Value::as_array)
        .ok_or(PolicyError::MissingField("task_marker_policy.markers"))?;
    if markers.is_empty() {
        return Err(PolicyError::InvalidField("task_marker_policy.markers"));
    }
    markers
        .iter()
        .map(|marker| {
            Ok(TaskMarker {
                id: string_field(marker, "task_marker_policy.markers.id")?,
                token: string_field(marker, "task_marker_policy.markers.token")?,
                kind: string_field(marker, "task_marker_policy.markers.kind")?,
            })
        })
        .collect()
}

pub(super) fn gate_fields(value: &Value) -> Result<GateFields, PolicyError> {
    let gate_policy = value
        .get("gate_policy")
        .ok_or(PolicyError::MissingField("gate_policy"))?;
    let stages = gate_stages(gate_policy)?;
    let is_gate_ignore_archives = gate_policy
        .get("ignore_archives")
        .and_then(Value::as_bool)
        .ok_or(PolicyError::MissingField("gate_policy.ignore_archives"))?;
    let gate_finding = gate_policy
        .get("finding")
        .ok_or(PolicyError::MissingField("gate_policy.finding"))?;
    let substance = gate_policy
        .get("substance")
        .ok_or(PolicyError::MissingField("gate_policy.substance"))?;
    Ok(GateFields {
        stages,
        ignore_archives: is_gate_ignore_archives,
        finding_scope: string_field(gate_finding, "gate_policy.finding.scope")?,
        finding_promoted: string_field(gate_finding, "gate_policy.finding.promoted")?,
        substance: ArtifactSubstancePolicy {
            proposal_minimum_substantive_lines: usize_field(
                substance,
                "gate_policy.substance.proposal_minimum_substantive_lines",
            )?,
            design_minimum_substantive_lines: usize_field(
                substance,
                "gate_policy.substance.design_minimum_substantive_lines",
            )?,
            minimum_requirement_blocks: usize_field(
                substance,
                "gate_policy.substance.minimum_requirement_blocks",
            )?,
            minimum_substantive_tasks: usize_field(
                substance,
                "gate_policy.substance.minimum_substantive_tasks",
            )?,
            placeholder_tokens: string_array_field(
                substance,
                "gate_policy.substance.placeholder_tokens",
            )?,
            normative_tokens: string_array_field(
                substance,
                "gate_policy.substance.normative_tokens",
            )?,
            dependency_marker_id: string_field(
                substance,
                "gate_policy.substance.dependency_marker_id",
            )?,
            dependency_target_class: string_field(
                substance,
                "gate_policy.substance.dependency_target_class",
            )?,
        },
    })
}

pub(super) fn mutation_policy_fields(value: &Value) -> Result<MutationPolicyFields, PolicyError> {
    let mutation = value
        .get("mutation_policy")
        .ok_or(PolicyError::MissingField("mutation_policy"))?;
    let sync = mutation
        .get("sync")
        .ok_or(PolicyError::MissingField("mutation_policy.sync"))?;
    let archive = mutation
        .get("archive")
        .ok_or(PolicyError::MissingField("mutation_policy.archive"))?;
    let migrate = mutation
        .get("migrate")
        .ok_or(PolicyError::MissingField("mutation_policy.migrate"))?;
    Ok(MutationPolicyFields {
        sync_require_gate: bool_field(sync, "mutation_policy.sync.require_passing_gate")?,
        archive_require_gate: bool_field(archive, "mutation_policy.archive.require_passing_gate")?,
        migrate_require_manifest: bool_field(migrate, "mutation_policy.migrate.require_manifest")?,
    })
}

fn usize_field(value: &Value, field: &'static str) -> Result<usize, PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    let raw = value
        .get(lookup)
        .and_then(Value::as_u64)
        .ok_or(PolicyError::MissingField(field))?;
    if raw == 0 {
        return Err(PolicyError::InvalidField(field));
    }
    usize::try_from(raw).map_err(|_| PolicyError::InvalidField(field))
}

pub(super) fn bool_field(value: &Value, field: &'static str) -> Result<bool, PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(Value::as_bool)
        .ok_or(PolicyError::MissingField(field))
}

pub(super) fn string_array_field(
    value: &Value,
    field: &'static str,
) -> Result<Vec<String>, PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(Value::as_array)
        .ok_or(PolicyError::MissingField(field))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or(PolicyError::InvalidField(field))
        })
        .collect()
}

pub(super) fn receipt_schemas(value: &Value) -> Result<Vec<ReceiptSchema>, PolicyError> {
    let schemas = value
        .get("receipt_schemas")
        .and_then(Value::as_array)
        .ok_or(PolicyError::MissingField("receipt_schemas"))?;
    if schemas.is_empty() {
        return Err(PolicyError::InvalidField("receipt_schemas"));
    }
    schemas
        .iter()
        .map(|schema| {
            Ok(ReceiptSchema {
                command: string_field(schema, "receipt_schemas.command")?,
                top_level_keys: string_array_field(schema, "receipt_schemas.top_level_keys")?,
                action_keys: string_array_field(schema, "receipt_schemas.action_keys")?,
                issue_keys: string_array_field(schema, "receipt_schemas.issue_keys")?,
                hash_fields: string_array_field(schema, "receipt_schemas.hash_fields")?,
            })
        })
        .collect()
}

pub(super) fn exemption_marker_policy(value: &Value) -> Result<ExemptionMarkerPolicy, PolicyError> {
    let marker_policy = value
        .get("exemption_marker_policy")
        .ok_or(PolicyError::MissingField("exemption_marker_policy"))?;
    Ok(ExemptionMarkerPolicy {
        required_fields: string_array_field(
            marker_policy,
            "exemption_marker_policy.required_fields",
        )?,
        date_fields: string_array_field(marker_policy, "exemption_marker_policy.date_fields")?,
        optional_fields: string_array_field(
            marker_policy,
            "exemption_marker_policy.optional_fields",
        )?,
    })
}

pub(super) fn change_metadata_fields(value: &Value) -> Result<ChangeMetadataFields, PolicyError> {
    let change_metadata_policy = value
        .get("change_metadata_policy")
        .ok_or(PolicyError::MissingField("change_metadata_policy"))?;
    Ok(ChangeMetadataFields {
        allowed_groups: string_array_field(
            change_metadata_policy,
            "change_metadata_policy.allowed_groups",
        )?,
        group_prefixes: string_array_field(
            change_metadata_policy,
            "change_metadata_policy.group_prefixes",
        )?,
        statuses: string_array_field(change_metadata_policy, "change_metadata_policy.statuses")?,
    })
}

fn gate_stages(gate_policy: &Value) -> Result<Vec<String>, PolicyError> {
    let stages = gate_policy
        .get("stages")
        .and_then(Value::as_array)
        .ok_or(PolicyError::MissingField("gate_policy.stages"))?
        .iter()
        .map(|stage| {
            stage
                .as_str()
                .map(str::to_string)
                .ok_or(PolicyError::InvalidField("gate_policy.stages"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if stages.is_empty() {
        return Err(PolicyError::InvalidField("gate_policy.stages"));
    }
    Ok(stages)
}

pub(super) fn string_field(value: &Value, field: &'static str) -> Result<String, PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(PolicyError::MissingField(field))
}
