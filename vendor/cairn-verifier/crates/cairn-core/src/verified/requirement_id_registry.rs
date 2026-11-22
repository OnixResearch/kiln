pub const REQUIREMENT_ID_REGISTRY_SCHEMA_V1: &str = "cairn.requirement-id-registry.v1";
pub const REQUIREMENT_ID_REGISTRY_BOUNDARY: &str = "requirement ID registry resolution proves ownership and linkage only; it does not prove requirement satisfaction, implementation correctness, test coverage, or release eligibility";

const STATUS_ACTIVE: &str = "active";
const STATUS_DEPRECATED: &str = "deprecated";
const STATUS_RETIRED: &str = "retired";
const MIN_NAMESPACE_SEGMENTS: usize = 2;
const EMPTY_COUNT: usize = 0;
const FIELD_REGISTRY: &str = "registry";
const FIELD_REFERENCES: &str = "references";
const FIELD_SCHEMA_VERSION: &str = "schema_version";
const FIELD_BOUNDARY: &str = "boundary";
const FIELD_ROWS: &str = "rows";
const FIELD_REQUIREMENT_ID: &str = "requirement_id";
const FIELD_NAMESPACE: &str = "namespace";
const FIELD_OWNER_REPO: &str = "owner_repo";
const FIELD_ACCEPTED_SPEC: &str = "accepted_spec";
const FIELD_STATUS: &str = "status";
const FIELD_ALIASES: &str = "aliases";
const FIELD_ALLOWED_CONSUMERS: &str = "allowed_consumers";
const FIELD_ID: &str = "id";
const FIELD_REASON: &str = "reason";
const FIELD_CONSUMER: &str = "consumer";
const FIELD_EXPECTED_OWNER: &str = "expected_owner";
const FIELD_NON_CLAIMS: &str = "non_claims";
const ISSUE_INVALID_INPUT: &str = "requirement_registry.invalid_input";
const ISSUE_SCHEMA: &str = "requirement_registry.unsupported_schema";
const ISSUE_REQUIRED: &str = "requirement_registry.required_field";
const ISSUE_DUPLICATE_ID: &str = "requirement_registry.duplicate_id";
const ISSUE_DUPLICATE_ALIAS: &str = "requirement_registry.duplicate_alias";
const ISSUE_ALIAS_SELF: &str = "requirement_registry.alias_self";
const ISSUE_STATUS: &str = "requirement_registry.unsupported_status";
const ISSUE_NAMESPACE: &str = "requirement_registry.namespace_overreach";
const ISSUE_UNKNOWN: &str = "requirement_registry.unknown_requirement";
const ISSUE_WRONG_OWNER: &str = "requirement_registry.wrong_owner";
const ISSUE_STALE_ALIAS: &str = "requirement_registry.stale_alias";
const ISSUE_INACTIVE: &str = "requirement_registry.inactive_requirement";
const ISSUE_CONSUMER: &str = "requirement_registry.consumer_not_allowed";
const ISSUE_NON_CLAIM: &str = "requirement_registry.missing_non_claim";
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "not requirement satisfaction",
    "not implementation correctness",
    "not test coverage",
    "not release eligibility",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementIdRegistry {
    pub schema_version: String,
    pub boundary: String,
    pub rows: Vec<RequirementRegistryRow>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRegistryRow {
    pub requirement_id: String,
    pub namespace: String,
    pub owner_repo: String,
    pub accepted_spec: String,
    pub status: String,
    pub aliases: Vec<RequirementIdAlias>,
    pub allowed_consumers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementIdAlias {
    pub id: String,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementReference {
    pub requirement_id: String,
    pub consumer: String,
    pub expected_owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementIdRegistryIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementIdRegistryReport {
    pub valid: bool,
    pub registry_hash: String,
    pub row_count: usize,
    pub checked_reference_count: usize,
    pub dangling_reference_count: usize,
    pub stale_alias_count: usize,
    pub issues: Vec<RequirementIdRegistryIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AliasResolution {
    row_index: usize,
    alias_index: Option<usize>,
}

#[must_use]
pub fn validate_requirement_id_registry_value(
    value: &serde_json::Value,
) -> RequirementIdRegistryReport {
    match parse_requirement_id_registry_input(value) {
        Ok((registry, references, registry_hash)) => {
            validate_requirement_id_registry(&registry, &references, registry_hash)
        }
        Err(issue) => RequirementIdRegistryReport {
            valid: false,
            registry_hash: crate::verified::evidence::canonical_json_hash(value),
            row_count: EMPTY_COUNT,
            checked_reference_count: EMPTY_COUNT,
            dangling_reference_count: EMPTY_COUNT,
            stale_alias_count: EMPTY_COUNT,
            issues: vec![issue],
            boundary: REQUIREMENT_ID_REGISTRY_BOUNDARY.to_string(),
        },
    }
}

#[must_use]
pub fn validate_requirement_id_registry(
    registry: &RequirementIdRegistry,
    references: &[RequirementReference],
    registry_hash: String,
) -> RequirementIdRegistryReport {
    // r[impl cairn.requirement_id_registry.contract]
    // r[impl cairn.requirement_id_registry.validation]
    let mut issues = Vec::new();
    validate_registry_shape(registry, &mut issues);
    let resolver = build_resolver(registry, &mut issues);
    let mut dangling_reference_count = EMPTY_COUNT;
    let mut stale_alias_count = EMPTY_COUNT;
    for (index, reference) in references.iter().enumerate() {
        validate_reference(
            registry,
            &resolver,
            reference,
            index,
            &mut dangling_reference_count,
            &mut stale_alias_count,
            &mut issues,
        );
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    RequirementIdRegistryReport {
        valid: issues.is_empty(),
        registry_hash,
        row_count: registry.rows.len(),
        checked_reference_count: references.len(),
        dangling_reference_count,
        stale_alias_count,
        issues,
        boundary: REQUIREMENT_ID_REGISTRY_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn requirement_id_registry_issues_json(
    report: &RequirementIdRegistryReport,
) -> Vec<serde_json::Value> {
    report
        .issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "code": issue.code,
                "field_path": issue.field_path,
                "message": issue.message,
            })
        })
        .collect()
}

fn validate_registry_shape(
    registry: &RequirementIdRegistry,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if registry.schema_version != REQUIREMENT_ID_REGISTRY_SCHEMA_V1 {
        push_issue(
            issues,
            ISSUE_SCHEMA,
            FIELD_SCHEMA_VERSION,
            "requirement registry schema version is unsupported",
        );
    }
    validate_required(FIELD_BOUNDARY, &registry.boundary, issues);
    validate_non_claims(FIELD_NON_CLAIMS, &registry.non_claims, issues);
    if registry.rows.is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIRED,
            FIELD_ROWS,
            "registry must contain at least one row",
        );
    }
    for (index, row) in registry.rows.iter().enumerate() {
        validate_row(index, row, issues);
    }
}

fn validate_row(
    index: usize,
    row: &RequirementRegistryRow,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    let path = format!("{FIELD_ROWS}[{index}]");
    validate_required(
        &format!("{path}.{FIELD_REQUIREMENT_ID}"),
        &row.requirement_id,
        issues,
    );
    validate_required(&format!("{path}.{FIELD_NAMESPACE}"), &row.namespace, issues);
    validate_required(
        &format!("{path}.{FIELD_OWNER_REPO}"),
        &row.owner_repo,
        issues,
    );
    validate_required(
        &format!("{path}.{FIELD_ACCEPTED_SPEC}"),
        &row.accepted_spec,
        issues,
    );
    validate_status(&format!("{path}.{FIELD_STATUS}"), &row.status, issues);
    validate_namespace(index, row, issues);
    if row.allowed_consumers.is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIRED,
            &format!("{path}.{FIELD_ALLOWED_CONSUMERS}"),
            "registry row must name at least one allowed consumer",
        );
    }
    for (alias_index, alias) in row.aliases.iter().enumerate() {
        validate_alias(index, row, alias_index, alias, issues);
    }
}

fn validate_alias(
    row_index: usize,
    row: &RequirementRegistryRow,
    alias_index: usize,
    alias: &RequirementIdAlias,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    let path = format!("{FIELD_ROWS}[{row_index}].{FIELD_ALIASES}[{alias_index}]");
    validate_required(&format!("{path}.{FIELD_ID}"), &alias.id, issues);
    validate_required(&format!("{path}.{FIELD_REASON}"), &alias.reason, issues);
    validate_status(&format!("{path}.{FIELD_STATUS}"), &alias.status, issues);
    if alias.id == row.requirement_id {
        push_issue(
            issues,
            ISSUE_ALIAS_SELF,
            &format!("{path}.{FIELD_ID}"),
            "alias must not repeat the canonical requirement ID",
        );
    }
}

fn validate_namespace(
    index: usize,
    row: &RequirementRegistryRow,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    let path = format!("{FIELD_ROWS}[{index}].{FIELD_NAMESPACE}");
    let segment_count = row
        .namespace
        .split('.')
        .filter(|segment| !segment.is_empty())
        .count();
    if segment_count < MIN_NAMESPACE_SEGMENTS {
        push_issue(
            issues,
            ISSUE_NAMESPACE,
            &path,
            "namespace must include an owner and a bounded domain segment",
        );
    }
    let expected_prefix = format!("{}.", row.namespace);
    if !row.requirement_id.starts_with(&expected_prefix) {
        push_issue(
            issues,
            ISSUE_NAMESPACE,
            &path,
            "requirement ID must be inside its declared namespace",
        );
    }
}

fn validate_status(field_path: &str, status: &str, issues: &mut Vec<RequirementIdRegistryIssue>) {
    if !matches!(status, STATUS_ACTIVE | STATUS_DEPRECATED | STATUS_RETIRED) {
        push_issue(
            issues,
            ISSUE_STATUS,
            field_path,
            "status must be active, deprecated, or retired",
        );
    }
}

fn validate_required(field_path: &str, value: &str, issues: &mut Vec<RequirementIdRegistryIssue>) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIRED,
            field_path,
            "field must not be empty",
        );
    }
}

fn validate_non_claims(
    field_path: &str,
    non_claims: &[String],
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if non_claims.is_empty() {
        push_issue(
            issues,
            ISSUE_NON_CLAIM,
            field_path,
            "registry non-claims are required",
        );
        return;
    }
    let joined = non_claims.join("; ").to_ascii_lowercase();
    for required in REQUIRED_NON_CLAIMS {
        if !joined.contains(required) {
            push_issue(
                issues,
                ISSUE_NON_CLAIM,
                field_path,
                &format!("registry non-claims must mention `{required}`"),
            );
        }
    }
}

fn build_resolver(
    registry: &RequirementIdRegistry,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) -> std::collections::BTreeMap<String, AliasResolution> {
    let mut resolver = std::collections::BTreeMap::new();
    for (row_index, row) in registry.rows.iter().enumerate() {
        insert_resolution(
            &mut resolver,
            &row.requirement_id,
            AliasResolution {
                row_index,
                alias_index: None,
            },
            ISSUE_DUPLICATE_ID,
            &format!("{FIELD_ROWS}[{row_index}].{FIELD_REQUIREMENT_ID}"),
            issues,
        );
        for (alias_index, alias) in row.aliases.iter().enumerate() {
            insert_resolution(
                &mut resolver,
                &alias.id,
                AliasResolution {
                    row_index,
                    alias_index: Some(alias_index),
                },
                ISSUE_DUPLICATE_ALIAS,
                &format!("{FIELD_ROWS}[{row_index}].{FIELD_ALIASES}[{alias_index}].{FIELD_ID}"),
                issues,
            );
        }
    }
    resolver
}

fn insert_resolution(
    resolver: &mut std::collections::BTreeMap<String, AliasResolution>,
    id: &str,
    resolution: AliasResolution,
    duplicate_code: &str,
    field_path: &str,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if id.is_empty() {
        return;
    }
    if resolver.insert(id.to_string(), resolution).is_some() {
        push_issue(
            issues,
            duplicate_code,
            field_path,
            "requirement registry ID or alias must be unique",
        );
    }
}

fn validate_reference(
    registry: &RequirementIdRegistry,
    resolver: &std::collections::BTreeMap<String, AliasResolution>,
    reference: &RequirementReference,
    index: usize,
    dangling_reference_count: &mut usize,
    stale_alias_count: &mut usize,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    let path = format!("{FIELD_REFERENCES}[{index}]");
    validate_required(
        &format!("{path}.{FIELD_REQUIREMENT_ID}"),
        &reference.requirement_id,
        issues,
    );
    validate_required(
        &format!("{path}.{FIELD_CONSUMER}"),
        &reference.consumer,
        issues,
    );
    let Some(resolution) = resolver.get(&reference.requirement_id).copied() else {
        *dangling_reference_count += 1;
        push_issue(
            issues,
            ISSUE_UNKNOWN,
            &format!("{path}.{FIELD_REQUIREMENT_ID}"),
            "referenced requirement ID is absent from the registry",
        );
        return;
    };
    let row = &registry.rows[resolution.row_index];
    validate_reference_owner(reference, row, &path, issues);
    validate_reference_status(reference, row, resolution, &path, stale_alias_count, issues);
    validate_reference_consumer(reference, row, &path, issues);
}

fn validate_reference_owner(
    reference: &RequirementReference,
    row: &RequirementRegistryRow,
    path: &str,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if !reference.expected_owner.is_empty() && reference.expected_owner != row.owner_repo {
        push_issue(
            issues,
            ISSUE_WRONG_OWNER,
            &format!("{path}.{FIELD_EXPECTED_OWNER}"),
            "referenced requirement owner does not match the registry row",
        );
    }
}

fn validate_reference_status(
    reference: &RequirementReference,
    row: &RequirementRegistryRow,
    resolution: AliasResolution,
    path: &str,
    stale_alias_count: &mut usize,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if row.status != STATUS_ACTIVE {
        push_issue(
            issues,
            ISSUE_INACTIVE,
            &format!("{path}.{FIELD_REQUIREMENT_ID}"),
            "referenced canonical requirement ID is not active",
        );
    }
    let Some(alias_index) = resolution.alias_index else {
        return;
    };
    let alias = &row.aliases[alias_index];
    if alias.status != STATUS_ACTIVE {
        *stale_alias_count += 1;
        push_issue(
            issues,
            ISSUE_STALE_ALIAS,
            &format!("{path}.{FIELD_REQUIREMENT_ID}"),
            &format!(
                "referenced alias {} is {} for canonical ID {}",
                reference.requirement_id, alias.status, row.requirement_id
            ),
        );
    }
}

fn validate_reference_consumer(
    reference: &RequirementReference,
    row: &RequirementRegistryRow,
    path: &str,
    issues: &mut Vec<RequirementIdRegistryIssue>,
) {
    if !reference.consumer.is_empty()
        && !row
            .allowed_consumers
            .iter()
            .any(|consumer| consumer == &reference.consumer)
    {
        push_issue(
            issues,
            ISSUE_CONSUMER,
            &format!("{path}.{FIELD_CONSUMER}"),
            "reference consumer is not allowed for this requirement ID",
        );
    }
}

fn parse_requirement_id_registry_input(
    value: &serde_json::Value,
) -> Result<(RequirementIdRegistry, Vec<RequirementReference>, String), RequirementIdRegistryIssue>
{
    let registry_value = object_field(value, FIELD_REGISTRY)?;
    let registry_hash = crate::verified::evidence::canonical_json_hash(registry_value);
    let registry = parse_registry(registry_value)?;
    let references = parse_references(array_field(value, FIELD_REFERENCES)?)?;
    Ok((registry, references, registry_hash))
}

fn parse_registry(
    value: &serde_json::Value,
) -> Result<RequirementIdRegistry, RequirementIdRegistryIssue> {
    Ok(RequirementIdRegistry {
        schema_version: string_field(value, FIELD_SCHEMA_VERSION)?,
        boundary: string_field(value, FIELD_BOUNDARY)?,
        rows: parse_rows(array_field(value, FIELD_ROWS)?)?,
        non_claims: string_array_field(value, FIELD_NON_CLAIMS)?,
    })
}

fn parse_rows(
    values: &[serde_json::Value],
) -> Result<Vec<RequirementRegistryRow>, RequirementIdRegistryIssue> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            Ok(RequirementRegistryRow {
                requirement_id: string_field(value, FIELD_REQUIREMENT_ID)?,
                namespace: string_field(value, FIELD_NAMESPACE)?,
                owner_repo: string_field(value, FIELD_OWNER_REPO)?,
                accepted_spec: string_field(value, FIELD_ACCEPTED_SPEC)?,
                status: string_field(value, FIELD_STATUS)?,
                aliases: parse_aliases(array_field(value, FIELD_ALIASES)?).map_err(
                    |mut issue| {
                        issue.field_path = format!("{FIELD_ROWS}[{index}].{}", issue.field_path);
                        issue
                    },
                )?,
                allowed_consumers: string_array_field(value, FIELD_ALLOWED_CONSUMERS)?,
            })
        })
        .collect()
}

fn parse_aliases(
    values: &[serde_json::Value],
) -> Result<Vec<RequirementIdAlias>, RequirementIdRegistryIssue> {
    values
        .iter()
        .map(|value| {
            Ok(RequirementIdAlias {
                id: string_field(value, FIELD_ID)?,
                status: string_field(value, FIELD_STATUS)?,
                reason: string_field(value, FIELD_REASON)?,
            })
        })
        .collect()
}

fn parse_references(
    values: &[serde_json::Value],
) -> Result<Vec<RequirementReference>, RequirementIdRegistryIssue> {
    values
        .iter()
        .map(|value| {
            Ok(RequirementReference {
                requirement_id: string_field(value, FIELD_REQUIREMENT_ID)?,
                consumer: string_field(value, FIELD_CONSUMER)?,
                expected_owner: optional_string_field(value, FIELD_EXPECTED_OWNER),
            })
        })
        .collect()
}

fn object_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a serde_json::Value, RequirementIdRegistryIssue> {
    value
        .get(field)
        .filter(|item| item.is_object())
        .ok_or_else(|| {
            input_issue(
                field,
                &format!("input field `{field}` must be a JSON object"),
            )
        })
}

fn array_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a [serde_json::Value], RequirementIdRegistryIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            input_issue(
                field,
                &format!("input field `{field}` must be a JSON array"),
            )
        })
}

fn string_field(
    value: &serde_json::Value,
    field: &str,
) -> Result<String, RequirementIdRegistryIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| input_issue(field, &format!("input field `{field}` must be a string")))
}

fn optional_string_field(value: &serde_json::Value, field: &str) -> String {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn string_array_field(
    value: &serde_json::Value,
    field: &str,
) -> Result<Vec<String>, RequirementIdRegistryIssue> {
    array_field(value, field)?
        .iter()
        .map(|item| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                input_issue(
                    field,
                    &format!("input field `{field}` must contain only strings"),
                )
            })
        })
        .collect()
}

fn input_issue(field_path: &str, message: &str) -> RequirementIdRegistryIssue {
    RequirementIdRegistryIssue {
        code: ISSUE_INVALID_INPUT.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<RequirementIdRegistryIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(RequirementIdRegistryIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {

    use super::*;

    const TEST_REGISTRY_HASH: &str = "registry-hash";

    fn base_registry() -> RequirementIdRegistry {
        RequirementIdRegistry {
            schema_version: REQUIREMENT_ID_REGISTRY_SCHEMA_V1.to_string(),
            boundary: REQUIREMENT_ID_REGISTRY_BOUNDARY.to_string(),
            rows: vec![RequirementRegistryRow {
                requirement_id: "cairn.requirement_id_registry.contract".to_string(),
                namespace: "cairn.requirement_id_registry".to_string(),
                owner_repo: "cairn".to_string(),
                accepted_spec: ".cairn/specs/requirement-id-registry/spec.md".to_string(),
                status: STATUS_ACTIVE.to_string(),
                aliases: vec![RequirementIdAlias {
                    id: "cairn.req_registry.contract".to_string(),
                    status: STATUS_ACTIVE.to_string(),
                    reason: "explicit compatibility alias".to_string(),
                }],
                allowed_consumers: vec!["cairn".to_string(), "octet".to_string()],
            }],
            non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|claim| (*claim).to_string())
                .collect(),
        }
    }

    fn reference(id: &str, consumer: &str, owner: &str) -> RequirementReference {
        RequirementReference {
            requirement_id: id.to_string(),
            consumer: consumer.to_string(),
            expected_owner: owner.to_string(),
        }
    }

    fn codes(report: &RequirementIdRegistryReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn owned_requirement_and_active_alias_resolve() {
        // r[verify cairn.requirement_id_registry.fixtures.positive]
        // r[verify cairn.requirement_id_registry.validation.receipt]
        let registry = base_registry();
        let references = vec![
            reference("cairn.requirement_id_registry.contract", "octet", "cairn"),
            reference("cairn.req_registry.contract", "cairn", "cairn"),
        ];
        let report = validate_requirement_id_registry(
            &registry,
            &references,
            TEST_REGISTRY_HASH.to_string(),
        );
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(report.registry_hash, TEST_REGISTRY_HASH);
        assert_eq!(report.row_count, registry.rows.len());
        assert_eq!(report.checked_reference_count, references.len());
        assert_eq!(report.dangling_reference_count, EMPTY_COUNT);
        assert_eq!(report.stale_alias_count, EMPTY_COUNT);
    }

    #[test]
    fn unknown_id_wrong_owner_and_stale_alias_fail_closed() {
        // r[verify cairn.requirement_id_registry.fixtures.negative]
        let mut registry = base_registry();
        registry.rows[0].aliases[0].status = STATUS_DEPRECATED.to_string();
        let references = vec![
            reference("cairn.unknown.requirement", "octet", "cairn"),
            reference("cairn.requirement_id_registry.contract", "octet", "valence"),
            reference("cairn.req_registry.contract", "cairn", "cairn"),
        ];
        let report = validate_requirement_id_registry(
            &registry,
            &references,
            TEST_REGISTRY_HASH.to_string(),
        );
        let codes = codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_UNKNOWN.to_string()));
        assert!(codes.contains(&ISSUE_WRONG_OWNER.to_string()));
        assert!(codes.contains(&ISSUE_STALE_ALIAS.to_string()));
        assert_eq!(report.dangling_reference_count, 1);
        assert_eq!(report.stale_alias_count, 1);
    }

    #[test]
    fn namespace_overreach_and_disallowed_consumer_fail_closed() {
        // r[verify cairn.requirement_id_registry.fixtures.negative]
        let mut registry = base_registry();
        registry.rows[0].namespace = "cairn".to_string();
        let references = vec![reference(
            "cairn.requirement_id_registry.contract",
            "mantle",
            "cairn",
        )];
        let report = validate_requirement_id_registry(
            &registry,
            &references,
            TEST_REGISTRY_HASH.to_string(),
        );
        let codes = codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_NAMESPACE.to_string()));
        assert!(codes.contains(&ISSUE_CONSUMER.to_string()));
    }

    #[test]
    fn non_claim_boundary_is_visible() {
        // r[verify cairn.requirement_id_registry.docs]
        // r[verify cairn.requirement_id_registry.docs.non_claims]
        let registry = base_registry();
        let report =
            validate_requirement_id_registry(&registry, &[], TEST_REGISTRY_HASH.to_string());
        assert!(report.boundary.contains("ownership and linkage only"));
        assert!(report
            .boundary
            .contains("does not prove requirement satisfaction"));
        assert!(report.boundary.contains("implementation correctness"));
        assert!(report.boundary.contains("test coverage"));
        assert!(report.boundary.contains("release eligibility"));
    }

    #[test]
    fn value_parser_and_fixture_matrix_cover_success_and_failure() {
        // r[verify cairn.requirement_id_registry.final_validation]
        // r[verify cairn.requirement_id_registry.final_validation.fixtures]
        let valid = serde_json::json!({
            "registry": {
                "schema_version": REQUIREMENT_ID_REGISTRY_SCHEMA_V1,
                "boundary": REQUIREMENT_ID_REGISTRY_BOUNDARY,
                "non_claims": REQUIRED_NON_CLAIMS,
                "rows": [{
                    "requirement_id": "cairn.requirement_id_registry.contract",
                    "namespace": "cairn.requirement_id_registry",
                    "owner_repo": "cairn",
                    "accepted_spec": ".cairn/specs/requirement-id-registry/spec.md",
                    "status": "active",
                    "aliases": [{"id": "cairn.req_registry.contract", "status": "active", "reason": "compatibility"}],
                    "allowed_consumers": ["cairn", "octet"]
                }]
            },
            "references": [{"requirement_id": "cairn.req_registry.contract", "consumer": "octet", "expected_owner": "cairn"}],
            "policy_hash": "policy-test"
        });
        let valid_report = validate_requirement_id_registry_value(&valid);
        assert!(
            valid_report.valid,
            "unexpected issues: {:?}",
            valid_report.issues
        );

        let invalid = serde_json::json!({
            "registry": {
                "schema_version": REQUIREMENT_ID_REGISTRY_SCHEMA_V1,
                "boundary": REQUIREMENT_ID_REGISTRY_BOUNDARY,
                "non_claims": REQUIRED_NON_CLAIMS,
                "rows": [{
                    "requirement_id": "cairn.requirement_id_registry.contract",
                    "namespace": "cairn.requirement_id_registry",
                    "owner_repo": "cairn",
                    "accepted_spec": ".cairn/specs/requirement-id-registry/spec.md",
                    "status": "active",
                    "aliases": [],
                    "allowed_consumers": ["cairn"]
                }]
            },
            "references": [{"requirement_id": "missing.id", "consumer": "octet"}]
        });
        let invalid_report = validate_requirement_id_registry_value(&invalid);
        assert!(!invalid_report.valid);
        assert!(codes(&invalid_report).contains(&ISSUE_UNKNOWN.to_string()));
    }
}
