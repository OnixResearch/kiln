pub const HASH_ALGORITHM_POLICY_SCHEMA_V1: &str = "cairn.hash-algorithm-policy.v1";
pub const HASH_ALGORITHM_POLICY_BOUNDARY: &str = "hash policy validation records declared content-identity algorithm compatibility only; it does not prove artifact truth, producer correctness, digest collision resistance beyond the named algorithm, or semantic correctness";

const ALGORITHM_BLAKE3: &str = "BLAKE3";
const ALGORITHM_SHA256: &str = "SHA256";
const SHA_256_LOWER: &str = "sha256";
const SHA_256_DASHED: &str = "SHA-256";
const BLAKE3_HEX_LENGTH: usize = 64;
const SHA256_HEX_LENGTH: usize = 64;
const FIELD_POLICY: &str = "policy";
const FIELD_FIELDS: &str = "fields";
const FIELD_DEFAULT_ALGORITHM: &str = "default_algorithm";
const FIELD_EXCEPTIONS: &str = "exceptions";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_ALGORITHM: &str = "algorithm";
const FIELD_OWNER: &str = "owner";
const FIELD_SCOPE: &str = "scope";
const FIELD_REASON: &str = "reason";
const FIELD_ALLOWED_CONSUMERS: &str = "allowed_consumers";
const FIELD_MIGRATION_STATUS: &str = "migration_status";
const FIELD_SCHEMA: &str = "schema";
const FIELD_FIELD_PATH: &str = "field_path";
const FIELD_DIGEST: &str = "digest";
const FIELD_STACK_OWNED_IDENTITY: &str = "stack_owned_identity";
const ISSUE_INVALID_INPUT: &str = "hash_policy.invalid_input";
const ISSUE_DEFAULT_ALGORITHM: &str = "hash_policy.default_algorithm_unsupported";
const ISSUE_EXCEPTION_OWNER: &str = "hash_policy.exception_missing_owner";
const ISSUE_EXCEPTION_REASON: &str = "hash_policy.exception_missing_reason";
const ISSUE_EXCEPTION_SCOPE: &str = "hash_policy.exception_missing_scope";
const ISSUE_EXCEPTION_CONSUMERS: &str = "hash_policy.exception_missing_allowed_consumers";
const ISSUE_EXCEPTION_NON_CLAIMS: &str = "hash_policy.exception_missing_non_claims";
const ISSUE_EXCEPTION_ALGORITHM: &str = "hash_policy.exception_algorithm_unsupported";
const ISSUE_FIELD_ALGORITHM: &str = "hash_policy.field_algorithm_unsupported";
const ISSUE_UNDECLARED_HASH: &str = "hash_policy.undeclared_hash";
const ISSUE_MALFORMED_DIGEST: &str = "hash_policy.malformed_digest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashAlgorithmPolicy {
    pub default_algorithm: String,
    pub exceptions: Vec<HashAlgorithmException>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashAlgorithmException {
    pub algorithm: String,
    pub owner: String,
    pub scope: String,
    pub reason: String,
    pub allowed_consumers: Vec<String>,
    pub migration_status: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashFieldObservation {
    pub schema: String,
    pub field_path: String,
    pub algorithm: String,
    pub digest: Option<String>,
    pub stack_owned_identity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashAlgorithmPolicyIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashAlgorithmPolicyReport {
    pub valid: bool,
    pub field_count: usize,
    pub accepted_exception_count: usize,
    pub issues: Vec<HashAlgorithmPolicyIssue>,
    pub boundary: String,
}

#[must_use]
pub fn validate_hash_algorithm_policy(
    policy: &HashAlgorithmPolicy,
    fields: &[HashFieldObservation],
) -> HashAlgorithmPolicyReport {
    // r[impl cairn.hash_algorithm_policy.contract]
    // r[impl cairn.hash_algorithm_policy.validation]
    let mut issues = Vec::new();
    validate_policy_shape(policy, &mut issues);
    let mut accepted_exception_count = 0;
    for field in fields {
        validate_field(policy, field, &mut accepted_exception_count, &mut issues);
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    HashAlgorithmPolicyReport {
        valid: issues.is_empty(),
        field_count: fields.len(),
        accepted_exception_count,
        issues,
        boundary: HASH_ALGORITHM_POLICY_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn validate_hash_algorithm_policy_value(
    value: &serde_json::Value,
) -> HashAlgorithmPolicyReport {
    match parse_hash_algorithm_policy_input(value) {
        Ok((policy, fields)) => validate_hash_algorithm_policy(&policy, &fields),
        Err(issue) => HashAlgorithmPolicyReport {
            valid: false,
            field_count: 0,
            accepted_exception_count: 0,
            issues: vec![issue],
            boundary: HASH_ALGORITHM_POLICY_BOUNDARY.to_string(),
        },
    }
}

#[must_use]
pub fn hash_algorithm_policy_issues_json(
    report: &HashAlgorithmPolicyReport,
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

fn validate_policy_shape(policy: &HashAlgorithmPolicy, issues: &mut Vec<HashAlgorithmPolicyIssue>) {
    if normalized_algorithm(&policy.default_algorithm) != Some(ALGORITHM_BLAKE3) {
        push_issue(
            issues,
            ISSUE_DEFAULT_ALGORITHM,
            FIELD_DEFAULT_ALGORITHM,
            "default stack-owned content identity algorithm must be BLAKE3",
        );
    }
    if policy.non_claims.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_NON_CLAIMS,
            FIELD_NON_CLAIMS,
            "hash policy must carry non-claims",
        );
    }
    for (index, exception) in policy.exceptions.iter().enumerate() {
        validate_exception(index, exception, issues);
    }
}

fn validate_exception(
    index: usize,
    exception: &HashAlgorithmException,
    issues: &mut Vec<HashAlgorithmPolicyIssue>,
) {
    let path = format!("{FIELD_EXCEPTIONS}[{index}]");
    if normalized_algorithm(&exception.algorithm).is_none() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_ALGORITHM,
            &format!("{path}.{FIELD_ALGORITHM}"),
            "exception algorithm is unsupported",
        );
    }
    if exception.owner.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_OWNER,
            &format!("{path}.{FIELD_OWNER}"),
            "exception owner is required",
        );
    }
    if exception.reason.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_REASON,
            &format!("{path}.{FIELD_REASON}"),
            "exception reason is required",
        );
    }
    if exception.scope.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_SCOPE,
            &format!("{path}.{FIELD_SCOPE}"),
            "exception scope is required",
        );
    }
    if exception.allowed_consumers.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_CONSUMERS,
            &format!("{path}.{FIELD_ALLOWED_CONSUMERS}"),
            "exception allowed consumers are required",
        );
    }
    if exception.non_claims.is_empty() {
        push_issue(
            issues,
            ISSUE_EXCEPTION_NON_CLAIMS,
            &format!("{path}.{FIELD_NON_CLAIMS}"),
            "exception non-claims are required",
        );
    }
}

fn validate_field(
    policy: &HashAlgorithmPolicy,
    field: &HashFieldObservation,
    accepted_exception_count: &mut usize,
    issues: &mut Vec<HashAlgorithmPolicyIssue>,
) {
    let Some(algorithm) = normalized_algorithm(&field.algorithm) else {
        push_issue(
            issues,
            ISSUE_FIELD_ALGORITHM,
            &field.field_path,
            "field hash algorithm is unsupported",
        );
        return;
    };
    validate_digest(field, algorithm, issues);
    if !field.stack_owned_identity {
        return;
    }
    if algorithm == ALGORITHM_BLAKE3 {
        return;
    }
    if declared_exception(policy, field, algorithm).is_some() {
        *accepted_exception_count += 1;
        return;
    }
    push_issue(
        issues,
        ISSUE_UNDECLARED_HASH,
        &field.field_path,
        "stack-owned identity field uses non-BLAKE3 algorithm without a bounded exception",
    );
}

fn validate_digest(
    field: &HashFieldObservation,
    algorithm: &str,
    issues: &mut Vec<HashAlgorithmPolicyIssue>,
) {
    let Some(digest) = field.digest.as_deref() else {
        return;
    };
    let expected_len = match algorithm {
        ALGORITHM_BLAKE3 => BLAKE3_HEX_LENGTH,
        ALGORITHM_SHA256 => SHA256_HEX_LENGTH,
        _ => return,
    };
    if digest.len() != expected_len
        || !digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        push_issue(
            issues,
            ISSUE_MALFORMED_DIGEST,
            &field.field_path,
            "digest must be lowercase or uppercase hexadecimal with the algorithm length",
        );
    }
}

fn declared_exception<'a>(
    policy: &'a HashAlgorithmPolicy,
    field: &HashFieldObservation,
    algorithm: &str,
) -> Option<&'a HashAlgorithmException> {
    policy.exceptions.iter().find(|exception| {
        normalized_algorithm(&exception.algorithm) == Some(algorithm)
            && exception.scope == field.field_path
            && !exception.owner.is_empty()
            && !exception.reason.is_empty()
            && !exception.allowed_consumers.is_empty()
            && !exception.non_claims.is_empty()
    })
}

fn parse_hash_algorithm_policy_input(
    value: &serde_json::Value,
) -> Result<(HashAlgorithmPolicy, Vec<HashFieldObservation>), HashAlgorithmPolicyIssue> {
    let policy = value
        .get(FIELD_POLICY)
        .ok_or_else(|| invalid_input(FIELD_POLICY, "policy object is required"))?;
    let fields = value
        .get(FIELD_FIELDS)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_input(FIELD_FIELDS, "fields array is required"))?;
    Ok((parse_policy(policy)?, parse_fields(fields)?))
}

fn parse_policy(
    value: &serde_json::Value,
) -> Result<HashAlgorithmPolicy, HashAlgorithmPolicyIssue> {
    let exceptions = value
        .get(FIELD_EXCEPTIONS)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_input(FIELD_EXCEPTIONS, "exceptions array is required"))?
        .iter()
        .map(parse_exception)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HashAlgorithmPolicy {
        default_algorithm: string_field(value, FIELD_DEFAULT_ALGORITHM)?,
        exceptions,
        non_claims: string_array(value, FIELD_NON_CLAIMS)?,
    })
}

fn parse_exception(
    value: &serde_json::Value,
) -> Result<HashAlgorithmException, HashAlgorithmPolicyIssue> {
    Ok(HashAlgorithmException {
        algorithm: string_field(value, FIELD_ALGORITHM)?,
        owner: string_field(value, FIELD_OWNER)?,
        scope: string_field(value, FIELD_SCOPE)?,
        reason: string_field(value, FIELD_REASON)?,
        allowed_consumers: string_array(value, FIELD_ALLOWED_CONSUMERS)?,
        migration_status: string_field(value, FIELD_MIGRATION_STATUS)?,
        non_claims: string_array(value, FIELD_NON_CLAIMS)?,
    })
}

fn parse_fields(
    values: &[serde_json::Value],
) -> Result<Vec<HashFieldObservation>, HashAlgorithmPolicyIssue> {
    values
        .iter()
        .map(|value| {
            Ok(HashFieldObservation {
                schema: string_field(value, FIELD_SCHEMA)?,
                field_path: string_field(value, FIELD_FIELD_PATH)?,
                algorithm: string_field(value, FIELD_ALGORITHM)?,
                digest: optional_string_field(value, FIELD_DIGEST),
                stack_owned_identity: bool_field(value, FIELD_STACK_OWNED_IDENTITY)?,
            })
        })
        .collect()
}

fn string_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<String, HashAlgorithmPolicyIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| invalid_input(field, "string field is required"))
}

fn optional_string_field(value: &serde_json::Value, field: &'static str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn string_array(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Vec<String>, HashAlgorithmPolicyIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_input(field, "string array field is required"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| invalid_input(field, "array items must be strings"))
        })
        .collect()
}

fn bool_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<bool, HashAlgorithmPolicyIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| invalid_input(field, "boolean field is required"))
}

fn normalized_algorithm(algorithm: &str) -> Option<&'static str> {
    match algorithm {
        ALGORITHM_BLAKE3 => Some(ALGORITHM_BLAKE3),
        ALGORITHM_SHA256 | SHA_256_LOWER | SHA_256_DASHED => Some(ALGORITHM_SHA256),
        _ => None,
    }
}

fn invalid_input(field_path: &str, message: &str) -> HashAlgorithmPolicyIssue {
    HashAlgorithmPolicyIssue {
        code: ISSUE_INVALID_INPUT.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<HashAlgorithmPolicyIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(HashAlgorithmPolicyIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLAKE3_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA256_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn accepts_blake3_default_identity_field() {
        // r[verify cairn.hash_algorithm_policy.fixtures.positive]
        // r[verify cairn.hash_algorithm_policy.final_validation]
        let report =
            validate_hash_algorithm_policy(&policy_without_exceptions(), &[blake3_field()]);
        assert!(report.valid);
        assert_eq!(report.field_count, 1);
        assert_eq!(report.accepted_exception_count, 0);
        assert_eq!(report.boundary, HASH_ALGORITHM_POLICY_BOUNDARY);
    }

    #[test]
    fn accepts_declared_sha256_exception_for_exact_scope() {
        // r[verify cairn.hash_algorithm_policy.fixtures.positive]
        // r[verify cairn.hash_algorithm_policy.validation.exception]
        let report =
            validate_hash_algorithm_policy(&policy_with_sha256_exception(), &[sha256_field()]);
        assert!(report.valid);
        assert_eq!(report.accepted_exception_count, 1);
    }

    #[test]
    fn rejects_undeclared_sha256_identity_field() {
        // r[verify cairn.hash_algorithm_policy.fixtures.negative]
        let report =
            validate_hash_algorithm_policy(&policy_without_exceptions(), &[sha256_field()]);
        assert!(!report.valid);
        assert_issue(&report, ISSUE_UNDECLARED_HASH);
    }

    #[test]
    fn rejects_malformed_digest() {
        // r[verify cairn.hash_algorithm_policy.fixtures.negative]
        let mut field = blake3_field();
        field.digest = Some("not-a-digest".to_string());
        let report = validate_hash_algorithm_policy(&policy_without_exceptions(), &[field]);
        assert!(!report.valid);
        assert_issue(&report, ISSUE_MALFORMED_DIGEST);
    }

    #[test]
    fn rejects_exception_without_owner() {
        // r[verify cairn.hash_algorithm_policy.fixtures.negative]
        let mut policy = policy_with_sha256_exception();
        policy.exceptions[0].owner = String::new();
        let report = validate_hash_algorithm_policy(&policy, &[sha256_field()]);
        assert!(!report.valid);
        assert_issue(&report, ISSUE_EXCEPTION_OWNER);
        assert_issue(&report, ISSUE_UNDECLARED_HASH);
    }

    #[test]
    fn validates_json_fixture_input() {
        // r[verify cairn.hash_algorithm_policy.validation]
        // r[verify cairn.hash_algorithm_policy.final_validation.fixtures]
        // r[verify cairn.hash_algorithm_policy.docs]
        // r[verify cairn.hash_algorithm_policy.docs.non_claims]
        let value = serde_json::json!({
            "policy": {
                "default_algorithm": "BLAKE3",
                "exceptions": [{
                    "algorithm": "SHA256",
                    "owner": "chaoscontrol",
                    "scope": "snapshots.parent_snapshot_ref",
                    "reason": "legacy snapshot references are SHA256-addressed at the protocol boundary",
                    "allowed_consumers": ["chaoscontrol"],
                    "migration_status": "interop-exception",
                    "non_claims": ["scope-bound identity compatibility only"]
                }],
                "non_claims": ["hash policy does not prove artifact truth"]
            },
            "fields": [{
                "schema": "chaoscontrol.snapshot.v1",
                "field_path": "snapshots.parent_snapshot_ref",
                "algorithm": "sha256",
                "digest": SHA256_DIGEST,
                "stack_owned_identity": true
            }]
        });
        let report = validate_hash_algorithm_policy_value(&value);
        assert!(report.valid);
        assert_eq!(report.accepted_exception_count, 1);
        assert!(report.boundary.contains("does not prove artifact truth"));
    }

    fn policy_without_exceptions() -> HashAlgorithmPolicy {
        HashAlgorithmPolicy {
            default_algorithm: ALGORITHM_BLAKE3.to_string(),
            exceptions: Vec::new(),
            non_claims: vec!["hash policy does not prove artifact truth".to_string()],
        }
    }

    fn policy_with_sha256_exception() -> HashAlgorithmPolicy {
        HashAlgorithmPolicy {
            default_algorithm: ALGORITHM_BLAKE3.to_string(),
            exceptions: vec![HashAlgorithmException {
                algorithm: ALGORITHM_SHA256.to_string(),
                owner: "chaoscontrol".to_string(),
                scope: "snapshots.parent_snapshot_ref".to_string(),
                reason: "legacy snapshot references are SHA256-addressed at the protocol boundary"
                    .to_string(),
                allowed_consumers: vec!["chaoscontrol".to_string()],
                migration_status: "interop-exception".to_string(),
                non_claims: vec!["scope-bound identity compatibility only".to_string()],
            }],
            non_claims: vec!["hash policy does not prove artifact truth".to_string()],
        }
    }

    fn blake3_field() -> HashFieldObservation {
        HashFieldObservation {
            schema: "cairn.receipt.v1".to_string(),
            field_path: "receipt_hash".to_string(),
            algorithm: ALGORITHM_BLAKE3.to_string(),
            digest: Some(BLAKE3_DIGEST.to_string()),
            stack_owned_identity: true,
        }
    }

    fn sha256_field() -> HashFieldObservation {
        HashFieldObservation {
            schema: "chaoscontrol.snapshot.v1".to_string(),
            field_path: "snapshots.parent_snapshot_ref".to_string(),
            algorithm: ALGORITHM_SHA256.to_string(),
            digest: Some(SHA256_DIGEST.to_string()),
            stack_owned_identity: true,
        }
    }

    fn assert_issue(report: &HashAlgorithmPolicyReport, code: &str) {
        assert!(report.issues.iter().any(|issue| issue.code == code));
    }
}
