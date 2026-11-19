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
    pub(super) substance: super::ArtifactSubstancePolicy,
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

pub(super) fn version_field(value: &serde_json::Value) -> Result<u64, super::PolicyError> {
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(super::PolicyError::MissingField("version"))?;
    if version != super::SUPPORTED_POLICY_VERSION {
        return Err(super::PolicyError::UnsupportedVersion {
            found: version,
            expected: super::SUPPORTED_POLICY_VERSION,
        });
    }
    Ok(version)
}

pub(super) fn require_nonempty_array(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<(), super::PolicyError> {
    let values = value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(field))?;
    if values.is_empty() {
        return Err(super::PolicyError::InvalidField(field));
    }
    Ok(())
}

pub(super) fn task_marker_fields(
    value: &serde_json::Value,
) -> Result<TaskMarkerFields, super::PolicyError> {
    let marker_policy = value
        .get("task_marker_policy")
        .ok_or(super::PolicyError::MissingField("task_marker_policy"))?;
    Ok(TaskMarkerFields {
        is_enabled: marker_policy
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or(super::PolicyError::MissingField(
                "task_marker_policy.enabled",
            ))?,
        strictness: string_field(marker_policy, "task_marker_policy.strictness")?,
    })
}

pub(super) fn task_markers(
    value: &serde_json::Value,
) -> Result<Vec<super::TaskMarker>, super::PolicyError> {
    let marker_policy = value
        .get("task_marker_policy")
        .ok_or(super::PolicyError::MissingField("task_marker_policy"))?;
    let markers = marker_policy
        .get("markers")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(
            "task_marker_policy.markers",
        ))?;
    if markers.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "task_marker_policy.markers",
        ));
    }
    markers
        .iter()
        .map(|marker| {
            Ok(super::TaskMarker {
                id: string_field(marker, "task_marker_policy.markers.id")?,
                token: string_field(marker, "task_marker_policy.markers.token")?,
                kind: string_field(marker, "task_marker_policy.markers.kind")?,
            })
        })
        .collect()
}

pub(super) fn gate_fields(value: &serde_json::Value) -> Result<GateFields, super::PolicyError> {
    let gate_policy = value
        .get("gate_policy")
        .ok_or(super::PolicyError::MissingField("gate_policy"))?;
    let stages = gate_stages(gate_policy)?;
    let is_gate_ignore_archives = gate_policy
        .get("ignore_archives")
        .and_then(serde_json::Value::as_bool)
        .ok_or(super::PolicyError::MissingField(
            "gate_policy.ignore_archives",
        ))?;
    let gate_finding = gate_policy
        .get("finding")
        .ok_or(super::PolicyError::MissingField("gate_policy.finding"))?;
    let substance = gate_policy
        .get("substance")
        .ok_or(super::PolicyError::MissingField("gate_policy.substance"))?;
    Ok(GateFields {
        stages,
        ignore_archives: is_gate_ignore_archives,
        finding_scope: string_field(gate_finding, "gate_policy.finding.scope")?,
        finding_promoted: string_field(gate_finding, "gate_policy.finding.promoted")?,
        substance: super::ArtifactSubstancePolicy {
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

pub(super) fn mutation_policy_fields(
    value: &serde_json::Value,
) -> Result<MutationPolicyFields, super::PolicyError> {
    let mutation = value
        .get("mutation_policy")
        .ok_or(super::PolicyError::MissingField("mutation_policy"))?;
    let sync = mutation
        .get("sync")
        .ok_or(super::PolicyError::MissingField("mutation_policy.sync"))?;
    let archive = mutation
        .get("archive")
        .ok_or(super::PolicyError::MissingField("mutation_policy.archive"))?;
    let migrate = mutation
        .get("migrate")
        .ok_or(super::PolicyError::MissingField("mutation_policy.migrate"))?;
    Ok(MutationPolicyFields {
        sync_require_gate: bool_field(sync, "mutation_policy.sync.require_passing_gate")?,
        archive_require_gate: bool_field(archive, "mutation_policy.archive.require_passing_gate")?,
        migrate_require_manifest: bool_field(migrate, "mutation_policy.migrate.require_manifest")?,
    })
}

fn usize_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<usize, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    let raw = value
        .get(lookup)
        .and_then(serde_json::Value::as_u64)
        .ok_or(super::PolicyError::MissingField(field))?;
    if raw == 0 {
        return Err(super::PolicyError::InvalidField(field));
    }
    usize::try_from(raw).map_err(|_| super::PolicyError::InvalidField(field))
}

pub(super) fn bool_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<bool, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(serde_json::Value::as_bool)
        .ok_or(super::PolicyError::MissingField(field))
}

pub(super) fn string_array_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Vec<String>, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(field))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or(super::PolicyError::InvalidField(field))
        })
        .collect()
}

pub(super) fn receipt_schemas(
    value: &serde_json::Value,
) -> Result<Vec<super::ReceiptSchema>, super::PolicyError> {
    let schemas = value
        .get("receipt_schemas")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField("receipt_schemas"))?;
    if schemas.is_empty() {
        return Err(super::PolicyError::InvalidField("receipt_schemas"));
    }
    schemas
        .iter()
        .map(|schema| {
            Ok(super::ReceiptSchema {
                command: string_field(schema, "receipt_schemas.command")?,
                top_level_keys: string_array_field(schema, "receipt_schemas.top_level_keys")?,
                action_keys: string_array_field(schema, "receipt_schemas.action_keys")?,
                issue_keys: string_array_field(schema, "receipt_schemas.issue_keys")?,
                hash_fields: string_array_field(schema, "receipt_schemas.hash_fields")?,
            })
        })
        .collect()
}

pub(super) fn exemption_marker_policy(
    value: &serde_json::Value,
) -> Result<super::ExemptionMarkerPolicy, super::PolicyError> {
    let marker_policy = value
        .get("exemption_marker_policy")
        .ok_or(super::PolicyError::MissingField("exemption_marker_policy"))?;
    Ok(super::ExemptionMarkerPolicy {
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

pub(super) fn change_metadata_fields(
    value: &serde_json::Value,
) -> Result<ChangeMetadataFields, super::PolicyError> {
    let change_metadata_policy = value
        .get("change_metadata_policy")
        .ok_or(super::PolicyError::MissingField("change_metadata_policy"))?;
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

fn gate_stages(gate_policy: &serde_json::Value) -> Result<Vec<String>, super::PolicyError> {
    let stages = gate_policy
        .get("stages")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField("gate_policy.stages"))?
        .iter()
        .map(|stage| {
            stage
                .as_str()
                .map(str::to_string)
                .ok_or(super::PolicyError::InvalidField("gate_policy.stages"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if stages.is_empty() {
        return Err(super::PolicyError::InvalidField("gate_policy.stages"));
    }
    Ok(stages)
}

pub(super) fn string_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<String, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or(super::PolicyError::MissingField(field))
}
