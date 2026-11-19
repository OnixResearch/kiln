use iddqd::{id_upcast, IdOrdItem, IdOrdMap};

pub const WORKSPACE_CONTRACT_MATRIX_SCHEMA_V1: &str = "cairn.workspace-contract-matrix.v1";
pub const WORKSPACE_CONTRACT_MATRIX_RECEIPT_SCHEMA_V1: &str =
    "cairn.workspace-contract-matrix.verify-receipt.v1";
pub const WORKSPACE_CONTRACT_MATRIX_BOUNDARY: &str = "workspace contract matrix validation proves declared edge health only; it does not prove provider correctness, does not prove consumer behavioral correctness, does not prove verifier soundness, or does not prove release eligibility";
pub const WORKSPACE_CONTRACT_MODE_DISABLED: &str = "disabled";
pub const WORKSPACE_CONTRACT_MODE_OPTIONAL: &str = "optional";
pub const WORKSPACE_CONTRACT_MODE_REQUIRED: &str = "required";

const FIELD_MATRIX: &str = "matrix";
const FIELD_SCHEMA_VERSION: &str = "schema_version";
const FIELD_EDGES: &str = "edges";
const FIELD_EDGE_ID: &str = "edge_id";
const FIELD_PROVIDER_REPO: &str = "provider_repo";
const FIELD_CONSUMER_REPO: &str = "consumer_repo";
const FIELD_ARTIFACT_FAMILY: &str = "artifact_family";
const FIELD_SCHEMA_ID: &str = "schema_id";
const FIELD_ARTIFACT_PATH: &str = "artifact_path";
const FIELD_ARTIFACT_IDENTITY: &str = "artifact_identity";
const FIELD_VALIDATION_COMMAND: &str = "validation_command";
const FIELD_POSITIVE_FIXTURE: &str = "positive_fixture";
const FIELD_NEGATIVE_FIXTURE: &str = "negative_fixture";
const FIELD_FAILURE_CLASS: &str = "failure_class";
const FIELD_OWNER: &str = "owner";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_OBSERVED_ARTIFACTS: &str = "observed_artifacts";
const FIELD_MODE: &str = "mode";
const FIELD_REQUIRED_BOUNDARY: &str = "required_boundary";
const EMPTY_COUNT: usize = 0;
const ISSUE_INVALID_INPUT: &str = "workspace_contract.invalid_input";
const ISSUE_SCHEMA: &str = "workspace_contract.unsupported_schema";
const ISSUE_REQUIRED: &str = "workspace_contract.required_field";
const ISSUE_UNSUPPORTED_REPO: &str = "workspace_contract.unsupported_repo";
const ISSUE_UNSAFE_PATH: &str = "workspace_contract.unsafe_path";
const ISSUE_STALE_IDENTITY: &str = "workspace_contract.stale_artifact_identity";
const ISSUE_INVALID_IDENTITY: &str = "workspace_contract.invalid_artifact_identity";
const ISSUE_MISSING_NEGATIVE: &str = "workspace_contract.missing_negative_coverage";
const ISSUE_NON_CLAIM: &str = "workspace_contract.weakened_non_claims";
const ISSUE_AMBIGUOUS: &str = "workspace_contract.ambiguous_edge";
const ISSUE_DUPLICATE_OBSERVATION: &str = "workspace_contract.duplicate_observation";
const ISSUE_READINESS_MODE: &str = "workspace_contract.readiness_unsupported_mode";
const ISSUE_READINESS_MISSING: &str = "workspace_contract.readiness_missing_receipt";
const ISSUE_READINESS_INVALID: &str = "workspace_contract.readiness_invalid_receipt";
const ISSUE_READINESS_BOUNDARY: &str = "workspace_contract.readiness_boundary_mismatch";
const ALLOWED_REPOS: &[&str] = &[
    "aspen", "basalt", "cairn", "mantle", "molten", "octet", "trellis", "ucan", "valence",
];
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "declared edge health only",
    "does not prove provider correctness",
    "does not prove consumer behavioral correctness",
    "does not prove release eligibility",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractMatrix {
    pub schema_version: String,
    pub edges: Vec<WorkspaceContractEdge>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractEdge {
    pub edge_id: String,
    pub provider_repo: String,
    pub consumer_repo: String,
    pub artifact_family: String,
    pub schema_id: String,
    pub artifact_path: String,
    pub artifact_identity: String,
    pub validation_command: String,
    pub positive_fixture: String,
    pub negative_fixture: String,
    pub failure_class: String,
    pub owner: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceArtifactObservation {
    pub edge_id: String,
    pub artifact_path: String,
    pub artifact_identity: String,
}

impl IdOrdItem for WorkspaceContractEdge {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.edge_id
    }

    id_upcast!();
}

impl IdOrdItem for WorkspaceArtifactObservation {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.edge_id
    }

    id_upcast!();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractIssue {
    pub code: String,
    pub edge_id: Option<String>,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractReport {
    pub valid: bool,
    pub matrix_hash: String,
    pub edge_count: usize,
    pub stale_edge_count: usize,
    pub checked_edges: Vec<String>,
    pub issues: Vec<WorkspaceContractIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractReadinessPolicy {
    pub mode: String,
    pub required_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContractReadiness {
    pub mode: String,
    pub required: bool,
    pub passed: bool,
    pub skipped: bool,
    pub disposition: String,
    pub receipt_hash: Option<String>,
    pub issues: Vec<WorkspaceContractIssue>,
    pub boundary: String,
}

#[must_use]
pub fn validate_workspace_contract_matrix_value(
    value: &serde_json::Value,
) -> WorkspaceContractReport {
    match parse_workspace_contract_input(value) {
        Ok((matrix, observations, matrix_hash)) => {
            validate_workspace_contract_matrix(&matrix, &observations, matrix_hash)
        }
        Err(issue) => WorkspaceContractReport {
            valid: false,
            matrix_hash: crate::verified::evidence::canonical_json_hash(value),
            edge_count: EMPTY_COUNT,
            stale_edge_count: EMPTY_COUNT,
            checked_edges: Vec::new(),
            issues: vec![issue],
            boundary: WORKSPACE_CONTRACT_MATRIX_BOUNDARY.to_string(),
        },
    }
}

#[must_use]
pub fn validate_workspace_contract_matrix(
    matrix: &WorkspaceContractMatrix,
    observations: &[WorkspaceArtifactObservation],
    matrix_hash: String,
) -> WorkspaceContractReport {
    // r[impl cairn.workspace_contract_matrix.schema]
    // r[impl cairn.workspace_contract_matrix.validation]
    let mut issues = Vec::new();
    if matrix.schema_version != WORKSPACE_CONTRACT_MATRIX_SCHEMA_V1 {
        push_issue(
            &mut issues,
            ISSUE_SCHEMA,
            None,
            FIELD_SCHEMA_VERSION,
            "workspace contract matrix schema version is unsupported",
        );
    }
    validate_non_claims(None, FIELD_NON_CLAIMS, &matrix.non_claims, &mut issues);
    if matrix.edges.is_empty() {
        push_issue(
            &mut issues,
            ISSUE_REQUIRED,
            None,
            FIELD_EDGES,
            "matrix must contain at least one edge",
        );
    }
    // r[impl cairn.workspace_contract_matrix.iddqd_indexes.contract]
    // r[impl cairn.workspace_contract_matrix.iddqd_indexes.validation]
    let _declared = edges_by_id(&matrix.edges, &mut issues);
    let observed = observations_by_edge(observations, &mut issues);
    let mut stale_edges = std::collections::BTreeSet::new();
    for (index, edge) in matrix.edges.iter().enumerate() {
        validate_edge(index, edge, &observed, &mut stale_edges, &mut issues);
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.edge_id.cmp(&right.edge_id))
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    WorkspaceContractReport {
        valid: issues.is_empty(),
        matrix_hash,
        edge_count: matrix.edges.len(),
        stale_edge_count: stale_edges.len(),
        checked_edges: matrix
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect(),
        issues,
        boundary: WORKSPACE_CONTRACT_MATRIX_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn workspace_contract_issues_json(report: &WorkspaceContractReport) -> Vec<serde_json::Value> {
    report
        .issues
        .iter()
        .map(workspace_contract_issue_json)
        .collect()
}

#[must_use]
pub fn workspace_contract_readiness_issues_json(
    readiness: &WorkspaceContractReadiness,
) -> Vec<serde_json::Value> {
    readiness
        .issues
        .iter()
        .map(workspace_contract_issue_json)
        .collect()
}

#[must_use]
pub fn evaluate_workspace_contract_matrix_readiness(
    policy: &WorkspaceContractReadinessPolicy,
    receipt: Option<&serde_json::Value>,
) -> WorkspaceContractReadiness {
    // r[impl cairn.workspace_contract_matrix.release_readiness]
    let mut issues = Vec::new();
    if !is_workspace_contract_mode(&policy.mode) {
        push_issue(
            &mut issues,
            ISSUE_READINESS_MODE,
            None,
            FIELD_MODE,
            "workspace contract matrix readiness mode must be disabled, optional, or required",
        );
        return readiness(policy, false, false, "unsupported-mode", None, issues);
    }
    if policy.mode == WORKSPACE_CONTRACT_MODE_DISABLED {
        return readiness(policy, true, true, "disabled", None, issues);
    }
    let Some(receipt) = receipt else {
        if policy.mode == WORKSPACE_CONTRACT_MODE_OPTIONAL {
            return readiness(policy, true, true, "absent", None, issues);
        }
        push_issue(
            &mut issues,
            ISSUE_READINESS_MISSING,
            None,
            "workspace_contract_matrix_receipt",
            "workspace contract matrix receipt is required by policy",
        );
        return readiness(policy, false, false, "missing", None, issues);
    };
    let receipt_hash = receipt
        .get("receipt_hash")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let valid_receipt = receipt.get("valid").and_then(serde_json::Value::as_bool) == Some(true)
        && receipt.get("schema").and_then(serde_json::Value::as_str)
            == Some(WORKSPACE_CONTRACT_MATRIX_RECEIPT_SCHEMA_V1)
        && receipt_hash
            .as_deref()
            .is_some_and(crate::verified::stack_provenance::is_blake3_hex);
    if !valid_receipt {
        push_issue(
            &mut issues,
            ISSUE_READINESS_INVALID,
            None,
            "workspace_contract_matrix_receipt",
            "workspace contract matrix receipt must be valid and match the expected schema",
        );
    }
    let boundary = receipt
        .get("boundary")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !policy.required_boundary.is_empty() && !boundary.contains(&policy.required_boundary) {
        push_issue(
            &mut issues,
            ISSUE_READINESS_BOUNDARY,
            None,
            "workspace_contract_matrix_receipt.boundary",
            "workspace contract matrix receipt boundary does not satisfy policy",
        );
    }
    readiness(
        policy,
        issues.is_empty(),
        false,
        if issues.is_empty() {
            "accepted"
        } else {
            "invalid"
        },
        receipt_hash,
        issues,
    )
}

#[must_use]
pub fn workspace_contract_readiness_policy_from_value(
    value: &serde_json::Value,
) -> WorkspaceContractReadinessPolicy {
    let gate = value
        .get("workspace_contract_matrix_gate")
        .unwrap_or(&serde_json::Value::Null);
    WorkspaceContractReadinessPolicy {
        mode: gate
            .get(FIELD_MODE)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(WORKSPACE_CONTRACT_MODE_DISABLED)
            .to_string(),
        required_boundary: gate
            .get(FIELD_REQUIRED_BOUNDARY)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(WORKSPACE_CONTRACT_MATRIX_BOUNDARY)
            .to_string(),
    }
}

#[must_use]
pub fn is_workspace_contract_mode(mode: &str) -> bool {
    matches!(
        mode,
        WORKSPACE_CONTRACT_MODE_DISABLED
            | WORKSPACE_CONTRACT_MODE_OPTIONAL
            | WORKSPACE_CONTRACT_MODE_REQUIRED
    )
}

fn readiness(
    policy: &WorkspaceContractReadinessPolicy,
    passed: bool,
    skipped: bool,
    disposition: &str,
    receipt_hash: Option<String>,
    issues: Vec<WorkspaceContractIssue>,
) -> WorkspaceContractReadiness {
    WorkspaceContractReadiness {
        mode: policy.mode.clone(),
        required: policy.mode == WORKSPACE_CONTRACT_MODE_REQUIRED,
        passed,
        skipped,
        disposition: disposition.to_string(),
        receipt_hash,
        issues,
        boundary: WORKSPACE_CONTRACT_MATRIX_BOUNDARY.to_string(),
    }
}

fn validate_edge(
    index: usize,
    edge: &WorkspaceContractEdge,
    observed: &IdOrdMap<WorkspaceArtifactObservation>,
    stale_edges: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    let path = format!("{FIELD_EDGES}[{index}]");
    let edge_id = (!edge.edge_id.is_empty()).then_some(edge.edge_id.clone());
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_EDGE_ID}"),
        &edge.edge_id,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_PROVIDER_REPO}"),
        &edge.provider_repo,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_CONSUMER_REPO}"),
        &edge.consumer_repo,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_ARTIFACT_FAMILY}"),
        &edge.artifact_family,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_SCHEMA_ID}"),
        &edge.schema_id,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_VALIDATION_COMMAND}"),
        &edge.validation_command,
        issues,
    );
    validate_required(
        &edge_id,
        &format!("{path}.{FIELD_OWNER}"),
        &edge.owner,
        issues,
    );
    validate_repo(
        &edge_id,
        &format!("{path}.{FIELD_PROVIDER_REPO}"),
        &edge.provider_repo,
        issues,
    );
    validate_repo(
        &edge_id,
        &format!("{path}.{FIELD_CONSUMER_REPO}"),
        &edge.consumer_repo,
        issues,
    );
    validate_path(
        &edge_id,
        &format!("{path}.{FIELD_ARTIFACT_PATH}"),
        &edge.artifact_path,
        issues,
    );
    validate_path(
        &edge_id,
        &format!("{path}.{FIELD_POSITIVE_FIXTURE}"),
        &edge.positive_fixture,
        issues,
    );
    if !edge.negative_fixture.is_empty() {
        validate_path(
            &edge_id,
            &format!("{path}.{FIELD_NEGATIVE_FIXTURE}"),
            &edge.negative_fixture,
            issues,
        );
    }
    if edge.negative_fixture.trim().is_empty() && edge.failure_class.trim().is_empty() {
        push_issue(
            issues,
            ISSUE_MISSING_NEGATIVE,
            edge_id.clone(),
            &format!("{path}.{FIELD_NEGATIVE_FIXTURE}"),
            "edge must declare a negative fixture or failure class",
        );
    }
    validate_identity(
        &edge_id,
        &format!("{path}.{FIELD_ARTIFACT_IDENTITY}"),
        &edge.artifact_identity,
        issues,
    );
    validate_non_claims(
        edge_id.clone(),
        &format!("{path}.{FIELD_NON_CLAIMS}"),
        &edge.non_claims,
        issues,
    );
    match observed.get(edge.edge_id.as_str()) {
        Some(observation) if observation.artifact_identity == edge.artifact_identity => {}
        Some(_) | None => {
            stale_edges.insert(edge.edge_id.clone());
            push_issue(
                issues,
                ISSUE_STALE_IDENTITY,
                edge_id,
                &format!("{path}.{FIELD_ARTIFACT_IDENTITY}"),
                "artifact identity does not match the observed artifact hash",
            );
        }
    }
}

fn validate_required(
    edge_id: &Option<String>,
    field_path: &str,
    value: &str,
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIRED,
            edge_id.clone(),
            field_path,
            "field must not be empty",
        );
    }
}

fn validate_repo(
    edge_id: &Option<String>,
    field_path: &str,
    repo: &str,
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    if !repo.trim().is_empty() && !ALLOWED_REPOS.contains(&repo) {
        push_issue(
            issues,
            ISSUE_UNSUPPORTED_REPO,
            edge_id.clone(),
            field_path,
            "repo is not part of the supported OnixResearch workspace contract vocabulary",
        );
    }
}

fn validate_path(
    edge_id: &Option<String>,
    field_path: &str,
    path: &str,
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    if !is_safe_relative_path(path) {
        push_issue(
            issues,
            ISSUE_UNSAFE_PATH,
            edge_id.clone(),
            field_path,
            "path must be a safe repo-relative path",
        );
    }
}

fn validate_identity(
    edge_id: &Option<String>,
    field_path: &str,
    identity: &str,
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    if !crate::verified::stack_provenance::is_blake3_hex(identity) {
        push_issue(
            issues,
            ISSUE_INVALID_IDENTITY,
            edge_id.clone(),
            field_path,
            "artifact identity must be BLAKE3 hex",
        );
    }
}

fn validate_non_claims(
    edge_id: Option<String>,
    field_path: &str,
    non_claims: &[String],
    issues: &mut Vec<WorkspaceContractIssue>,
) {
    if non_claims.is_empty() {
        push_issue(
            issues,
            ISSUE_NON_CLAIM,
            edge_id,
            field_path,
            "workspace contract non-claims are required",
        );
        return;
    }
    let joined = non_claims.join("; ").to_ascii_lowercase();
    for required in REQUIRED_NON_CLAIMS {
        if !joined.contains(required) {
            push_issue(
                issues,
                ISSUE_NON_CLAIM,
                edge_id.clone(),
                field_path,
                &format!("workspace contract non-claims must mention `{required}`"),
            );
        }
    }
}

fn edges_by_id(
    edges: &[WorkspaceContractEdge],
    issues: &mut Vec<WorkspaceContractIssue>,
) -> IdOrdMap<WorkspaceContractEdge> {
    let mut indexed = IdOrdMap::with_capacity(edges.len());
    for (index, edge) in edges.iter().cloned().enumerate() {
        let edge_id = edge.edge_id.clone();
        if indexed.insert_unique(edge).is_err() {
            push_issue(
                issues,
                ISSUE_AMBIGUOUS,
                Some(edge_id),
                &format!("{FIELD_EDGES}[{index}].{FIELD_EDGE_ID}"),
                "edge ID must be unique",
            );
        }
    }
    indexed
}

fn observations_by_edge(
    observations: &[WorkspaceArtifactObservation],
    issues: &mut Vec<WorkspaceContractIssue>,
) -> IdOrdMap<WorkspaceArtifactObservation> {
    let mut indexed = IdOrdMap::with_capacity(observations.len());
    for (index, observation) in observations.iter().cloned().enumerate() {
        let edge_id = observation.edge_id.clone();
        if indexed.insert_unique(observation).is_err() {
            push_issue(
                issues,
                ISSUE_DUPLICATE_OBSERVATION,
                Some(edge_id),
                &format!("{FIELD_OBSERVED_ARTIFACTS}[{index}].{FIELD_EDGE_ID}"),
                "observed artifact edge ID must be unique",
            );
        }
    }
    indexed
}

fn parse_workspace_contract_input(
    value: &serde_json::Value,
) -> Result<
    (
        WorkspaceContractMatrix,
        Vec<WorkspaceArtifactObservation>,
        String,
    ),
    WorkspaceContractIssue,
> {
    let matrix_value = object_field(value, FIELD_MATRIX)?;
    let matrix_hash = crate::verified::evidence::canonical_json_hash(matrix_value);
    let matrix = WorkspaceContractMatrix {
        schema_version: string_field(matrix_value, FIELD_SCHEMA_VERSION)?,
        edges: parse_edges(array_field(matrix_value, FIELD_EDGES)?)?,
        non_claims: string_array_field(matrix_value, FIELD_NON_CLAIMS)?,
    };
    let observations = match value.get(FIELD_OBSERVED_ARTIFACTS) {
        Some(_) => parse_observations(array_field(value, FIELD_OBSERVED_ARTIFACTS)?)?,
        None => Vec::new(),
    };
    Ok((matrix, observations, matrix_hash))
}

fn parse_edges(
    values: &[serde_json::Value],
) -> Result<Vec<WorkspaceContractEdge>, WorkspaceContractIssue> {
    values
        .iter()
        .map(|value| {
            Ok(WorkspaceContractEdge {
                edge_id: string_field(value, FIELD_EDGE_ID)?,
                provider_repo: string_field(value, FIELD_PROVIDER_REPO)?,
                consumer_repo: string_field(value, FIELD_CONSUMER_REPO)?,
                artifact_family: string_field(value, FIELD_ARTIFACT_FAMILY)?,
                schema_id: string_field(value, FIELD_SCHEMA_ID)?,
                artifact_path: string_field(value, FIELD_ARTIFACT_PATH)?,
                artifact_identity: string_field(value, FIELD_ARTIFACT_IDENTITY)?,
                validation_command: string_field(value, FIELD_VALIDATION_COMMAND)?,
                positive_fixture: string_field(value, FIELD_POSITIVE_FIXTURE)?,
                negative_fixture: optional_string_field(value, FIELD_NEGATIVE_FIXTURE),
                failure_class: optional_string_field(value, FIELD_FAILURE_CLASS),
                owner: string_field(value, FIELD_OWNER)?,
                non_claims: string_array_field(value, FIELD_NON_CLAIMS)?,
            })
        })
        .collect()
}

fn parse_observations(
    values: &[serde_json::Value],
) -> Result<Vec<WorkspaceArtifactObservation>, WorkspaceContractIssue> {
    values
        .iter()
        .map(|value| {
            Ok(WorkspaceArtifactObservation {
                edge_id: string_field(value, FIELD_EDGE_ID)?,
                artifact_path: string_field(value, FIELD_ARTIFACT_PATH)?,
                artifact_identity: string_field(value, FIELD_ARTIFACT_IDENTITY)?,
            })
        })
        .collect()
}

fn object_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a serde_json::Value, WorkspaceContractIssue> {
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
) -> Result<&'a [serde_json::Value], WorkspaceContractIssue> {
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

fn string_field(value: &serde_json::Value, field: &str) -> Result<String, WorkspaceContractIssue> {
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
) -> Result<Vec<String>, WorkspaceContractIssue> {
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

fn input_issue(field_path: &str, message: &str) -> WorkspaceContractIssue {
    WorkspaceContractIssue {
        code: ISSUE_INVALID_INPUT.to_string(),
        edge_id: None,
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn workspace_contract_issue_json(issue: &WorkspaceContractIssue) -> serde_json::Value {
    serde_json::json!({
        "code": issue.code,
        "edge_id": issue.edge_id,
        "field_path": issue.field_path,
        "message": issue.message,
    })
}

fn push_issue(
    issues: &mut Vec<WorkspaceContractIssue>,
    code: &str,
    edge_id: Option<String>,
    field_path: &str,
    message: &str,
) {
    issues.push(WorkspaceContractIssue {
        code: code.to_string(),
        edge_id,
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('~') || path.contains('\\') {
        return false;
    }
    !path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
}

#[cfg(test)]
mod tests {

    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const TEST_MATRIX_HASH: &str = "matrix-hash";

    fn non_claims() -> Vec<String> {
        REQUIRED_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect()
    }

    fn edge(edge_id: &str, provider_repo: &str, consumer_repo: &str) -> WorkspaceContractEdge {
        WorkspaceContractEdge {
            edge_id: edge_id.to_string(),
            provider_repo: provider_repo.to_string(),
            consumer_repo: consumer_repo.to_string(),
            artifact_family: "proof-manifest".to_string(),
            schema_id: "trellis.proof-artifact-manifest.v1".to_string(),
            artifact_path:
                "fixtures/workspace-contract-matrix/artifacts/trellis-proof-manifest.json"
                    .to_string(),
            artifact_identity: HASH_A.to_string(),
            validation_command: "scripts/check-verified-primitive-manifest.rs".to_string(),
            positive_fixture:
                "fixtures/workspace-contract-matrix/positive/representative-edges.json".to_string(),
            negative_fixture: "fixtures/workspace-contract-matrix/negative/stale-identity.json"
                .to_string(),
            failure_class: "stale_identity".to_string(),
            owner: "cairn.lifecycle".to_string(),
            non_claims: non_claims(),
        }
    }

    fn matrix() -> WorkspaceContractMatrix {
        WorkspaceContractMatrix {
            schema_version: WORKSPACE_CONTRACT_MATRIX_SCHEMA_V1.to_string(),
            edges: vec![
                edge("trellis-to-cairn-proof-manifest", "trellis", "cairn"),
                edge("octet-to-valence-evidence-ir", "octet", "valence"),
            ],
            non_claims: non_claims(),
        }
    }

    fn observations() -> Vec<WorkspaceArtifactObservation> {
        vec![
            WorkspaceArtifactObservation {
                edge_id: "trellis-to-cairn-proof-manifest".to_string(),
                artifact_path:
                    "fixtures/workspace-contract-matrix/artifacts/trellis-proof-manifest.json"
                        .to_string(),
                artifact_identity: HASH_A.to_string(),
            },
            WorkspaceArtifactObservation {
                edge_id: "octet-to-valence-evidence-ir".to_string(),
                artifact_path:
                    "fixtures/workspace-contract-matrix/artifacts/trellis-proof-manifest.json"
                        .to_string(),
                artifact_identity: HASH_A.to_string(),
            },
        ]
    }

    fn codes(report: &WorkspaceContractReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn complete_representative_edges_pass() {
        // r[verify cairn.workspace_contract_matrix.positive]
        // r[verify cairn.workspace_contract_matrix.validation.receipt]
        // r[verify cairn.workspace_contract_matrix.validation.privacy]
        let matrix = matrix();
        let report = validate_workspace_contract_matrix(
            &matrix,
            &observations(),
            TEST_MATRIX_HASH.to_string(),
        );
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(report.edge_count, matrix.edges.len());
        assert_eq!(report.stale_edge_count, EMPTY_COUNT);
        assert_eq!(report.matrix_hash, TEST_MATRIX_HASH);
        assert!(report
            .checked_edges
            .contains(&"octet-to-valence-evidence-ir".to_string()));
    }

    #[test]
    fn missing_owner_unsafe_path_stale_identity_and_missing_negative_fail() {
        // r[verify cairn.workspace_contract_matrix.negative]
        let mut matrix = matrix();
        matrix.edges[0].owner.clear();
        matrix.edges[0].artifact_path = "../outside.json".to_string();
        matrix.edges[0].negative_fixture.clear();
        matrix.edges[0].failure_class.clear();
        matrix.edges[1].artifact_identity = HASH_B.to_string();
        let report = validate_workspace_contract_matrix(
            &matrix,
            &observations(),
            TEST_MATRIX_HASH.to_string(),
        );
        let codes = codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_REQUIRED.to_string()));
        assert!(codes.contains(&ISSUE_UNSAFE_PATH.to_string()));
        assert!(codes.contains(&ISSUE_MISSING_NEGATIVE.to_string()));
        assert!(codes.contains(&ISSUE_STALE_IDENTITY.to_string()));
    }

    #[test]
    fn weakened_non_claim_fails_closed() {
        // r[verify cairn.workspace_contract_matrix.negative]
        let mut matrix = matrix();
        matrix.non_claims = vec!["proves everything".to_string()];
        matrix.edges[0].non_claims = vec!["proves everything".to_string()];
        let report = validate_workspace_contract_matrix(
            &matrix,
            &observations(),
            TEST_MATRIX_HASH.to_string(),
        );
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_NON_CLAIM.to_string()));
    }

    #[test]
    fn duplicate_declared_and_observed_identities_fail_without_overwrite() {
        // r[verify cairn.workspace_contract_matrix.iddqd_indexes.fixtures.negative]
        let mut matrix = matrix();
        matrix.edges.push(matrix.edges[0].clone());
        let mut observed = observations();
        observed.push(observed[0].clone());
        let report =
            validate_workspace_contract_matrix(&matrix, &observed, TEST_MATRIX_HASH.to_string());
        let codes = codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_AMBIGUOUS.to_string()));
        assert!(codes.contains(&ISSUE_DUPLICATE_OBSERVATION.to_string()));
    }

    #[test]
    fn unique_identity_indexes_preserve_public_report_shape() {
        // r[verify cairn.workspace_contract_matrix.iddqd_indexes.fixtures.positive]
        // r[verify cairn.workspace_contract_matrix.iddqd_indexes.receipt_compatibility]
        // r[verify cairn.workspace_contract_matrix.iddqd_indexes.final_validation]
        // r[verify cairn.workspace_contract_matrix.iddqd_indexes.final_validation.fixtures]
        let report = validate_workspace_contract_matrix(
            &matrix(),
            &observations(),
            TEST_MATRIX_HASH.to_string(),
        );
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(report.matrix_hash, TEST_MATRIX_HASH);
        assert_eq!(report.stale_edge_count, EMPTY_COUNT);
        assert_eq!(report.checked_edges.len(), report.edge_count);
    }

    #[test]
    fn release_readiness_modes_fail_closed_when_required_receipt_is_missing() {
        // r[verify cairn.workspace_contract_matrix.release_readiness]
        // r[verify cairn.workspace_contract_matrix.release_readiness.required]
        let policy = WorkspaceContractReadinessPolicy {
            mode: WORKSPACE_CONTRACT_MODE_REQUIRED.to_string(),
            required_boundary: WORKSPACE_CONTRACT_MATRIX_BOUNDARY.to_string(),
        };
        let readiness = evaluate_workspace_contract_matrix_readiness(&policy, None);
        assert!(readiness.required);
        assert!(!readiness.passed);
        assert_eq!(readiness.disposition, "missing");
    }

    #[test]
    fn optional_release_readiness_records_valid_context() {
        // r[verify cairn.workspace_contract_matrix.release_readiness.optional]
        let policy = WorkspaceContractReadinessPolicy {
            mode: WORKSPACE_CONTRACT_MODE_OPTIONAL.to_string(),
            required_boundary: "declared edge health only".to_string(),
        };
        let receipt = serde_json::json!({
            "schema": WORKSPACE_CONTRACT_MATRIX_RECEIPT_SCHEMA_V1,
            "valid": true,
            "receipt_hash": HASH_A,
            "boundary": WORKSPACE_CONTRACT_MATRIX_BOUNDARY
        });
        let readiness = evaluate_workspace_contract_matrix_readiness(&policy, Some(&receipt));
        assert!(!readiness.required);
        assert!(readiness.passed);
        assert_eq!(readiness.receipt_hash.as_deref(), Some(HASH_A));
    }

    #[test]
    fn matrix_non_claim_boundary_is_visible() {
        // r[verify cairn.workspace_contract_matrix.docs]
        // r[verify cairn.workspace_contract_matrix.docs.non_claims]
        let report = validate_workspace_contract_matrix(
            &matrix(),
            &observations(),
            TEST_MATRIX_HASH.to_string(),
        );
        assert!(report.boundary.contains("declared edge health only"));
        assert!(report
            .boundary
            .contains("does not prove provider correctness"));
        assert!(report
            .boundary
            .contains("does not prove consumer behavioral correctness"));
        assert!(report
            .boundary
            .contains("does not prove release eligibility"));
    }

    #[test]
    fn value_parser_covers_fixture_matrix_shape() {
        // r[verify cairn.workspace_contract_matrix.final_validation]
        // r[verify cairn.workspace_contract_matrix.final_validation.fixtures]
        let value = serde_json::json!({
            "matrix": {
                "schema_version": WORKSPACE_CONTRACT_MATRIX_SCHEMA_V1,
                "non_claims": REQUIRED_NON_CLAIMS,
                "edges": [{
                    "edge_id": "trellis-to-cairn-proof-manifest",
                    "provider_repo": "trellis",
                    "consumer_repo": "cairn",
                    "artifact_family": "proof-manifest",
                    "schema_id": "trellis.proof-artifact-manifest.v1",
                    "artifact_path": "fixtures/workspace-contract-matrix/artifacts/trellis-proof-manifest.json",
                    "artifact_identity": HASH_A,
                    "validation_command": "scripts/check-verified-primitive-manifest.rs",
                    "positive_fixture": "fixtures/workspace-contract-matrix/positive/representative-edges.json",
                    "negative_fixture": "fixtures/workspace-contract-matrix/negative/stale-identity.json",
                    "failure_class": "stale_identity",
                    "owner": "cairn.lifecycle",
                    "non_claims": REQUIRED_NON_CLAIMS
                }]
            },
            "observed_artifacts": [{
                "edge_id": "trellis-to-cairn-proof-manifest",
                "artifact_path": "fixtures/workspace-contract-matrix/artifacts/trellis-proof-manifest.json",
                "artifact_identity": HASH_A
            }]
        });
        let report = validate_workspace_contract_matrix_value(&value);
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
    }
}
