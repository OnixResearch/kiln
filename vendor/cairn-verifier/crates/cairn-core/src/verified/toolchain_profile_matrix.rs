pub const TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1: &str = "cairn.toolchain-profile-matrix.v1";
pub const TOOLCHAIN_PROFILE_MATRIX_BOUNDARY: &str = "toolchain profile matrix validation proves declaration completeness only; it does not prove command success, release eligibility, proof soundness, or compatibility without separate execution receipts";

const FIELD_MATRIX: &str = "matrix";
const FIELD_SCHEMA_VERSION: &str = "schema_version";
const FIELD_PROFILES: &str = "profiles";
const FIELD_REPO: &str = "repo";
const FIELD_PROFILE_ID: &str = "profile_id";
const FIELD_PURPOSE: &str = "purpose";
const FIELD_TOOLCHAIN_PINS: &str = "toolchain_pins";
const FIELD_COMMANDS: &str = "commands";
const FIELD_EXPECTED_ARTIFACTS: &str = "expected_artifacts";
const FIELD_NON_CLAIMS: &str = "non_claims";
const PURPOSE_DEV: &str = "dev";
const PURPOSE_CI: &str = "ci";
const PURPOSE_RELEASE: &str = "release";
const PURPOSE_PROOF: &str = "proof";
const PURPOSE_COMPATIBILITY: &str = "compatibility";
const EMPTY_COUNT: usize = 0;
const ISSUE_INVALID_INPUT: &str = "toolchain_profile.invalid_input";
const ISSUE_SCHEMA: &str = "toolchain_profile.unsupported_schema";
const ISSUE_REQUIRED: &str = "toolchain_profile.required_field";
const ISSUE_PURPOSE: &str = "toolchain_profile.unsupported_purpose";
const ISSUE_MISSING_PINS: &str = "toolchain_profile.missing_pins";
const ISSUE_MISSING_COMMANDS: &str = "toolchain_profile.missing_commands";
const ISSUE_MISSING_ARTIFACTS: &str = "toolchain_profile.missing_artifacts";
const ISSUE_AMBIGUOUS: &str = "toolchain_profile.ambiguous_profile";
const ISSUE_NON_CLAIM: &str = "toolchain_profile.weakened_non_claims";
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "does not prove command success",
    "separate execution receipt",
    "does not prove release eligibility",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainProfileMatrix {
    pub schema_version: String,
    pub profiles: Vec<ToolchainProfileRow>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainProfileRow {
    pub repo: String,
    pub profile_id: String,
    pub purpose: String,
    pub toolchain_pins: Vec<String>,
    pub commands: Vec<String>,
    pub expected_artifacts: Vec<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainProfileIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainProfileReport {
    pub valid: bool,
    pub matrix_hash: String,
    pub profile_count: usize,
    pub invalid_profile_count: usize,
    pub issues: Vec<ToolchainProfileIssue>,
    pub boundary: String,
}

#[must_use]
pub fn validate_toolchain_profile_matrix_value(
    value: &serde_json::Value,
) -> ToolchainProfileReport {
    match parse_toolchain_profile_matrix_input(value) {
        Ok((matrix, matrix_hash)) => validate_toolchain_profile_matrix(&matrix, matrix_hash),
        Err(issue) => ToolchainProfileReport {
            valid: false,
            matrix_hash: crate::verified::evidence::canonical_json_hash(value),
            profile_count: EMPTY_COUNT,
            invalid_profile_count: EMPTY_COUNT,
            issues: vec![issue],
            boundary: TOOLCHAIN_PROFILE_MATRIX_BOUNDARY.to_string(),
        },
    }
}

#[must_use]
pub fn validate_toolchain_profile_matrix(
    matrix: &ToolchainProfileMatrix,
    matrix_hash: String,
) -> ToolchainProfileReport {
    // r[impl cairn.toolchain_profile_matrix.contract]
    // r[impl cairn.toolchain_profile_matrix.validation]
    let mut issues = Vec::new();
    if matrix.schema_version != TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1 {
        push_issue(
            &mut issues,
            ISSUE_SCHEMA,
            FIELD_SCHEMA_VERSION,
            "toolchain profile matrix schema version is unsupported",
        );
    }
    validate_non_claims(FIELD_NON_CLAIMS, &matrix.non_claims, &mut issues);
    if matrix.profiles.is_empty() {
        push_issue(
            &mut issues,
            ISSUE_REQUIRED,
            FIELD_PROFILES,
            "matrix must contain at least one profile",
        );
    }
    let mut seen_profiles = std::collections::BTreeSet::new();
    let mut invalid_profile_indexes = std::collections::BTreeSet::new();
    for (index, profile) in matrix.profiles.iter().enumerate() {
        let before = issues.len();
        validate_profile(index, profile, &mut seen_profiles, &mut issues);
        if issues.len() != before {
            invalid_profile_indexes.insert(index);
        }
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    ToolchainProfileReport {
        valid: issues.is_empty(),
        matrix_hash,
        profile_count: matrix.profiles.len(),
        invalid_profile_count: invalid_profile_indexes.len(),
        issues,
        boundary: TOOLCHAIN_PROFILE_MATRIX_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn toolchain_profile_issues_json(report: &ToolchainProfileReport) -> Vec<serde_json::Value> {
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

fn validate_profile(
    index: usize,
    profile: &ToolchainProfileRow,
    seen_profiles: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<ToolchainProfileIssue>,
) {
    let path = format!("{FIELD_PROFILES}[{index}]");
    validate_required(&format!("{path}.{FIELD_REPO}"), &profile.repo, issues);
    validate_required(
        &format!("{path}.{FIELD_PROFILE_ID}"),
        &profile.profile_id,
        issues,
    );
    validate_purpose(index, &profile.purpose, issues);
    validate_required_list(
        &format!("{path}.{FIELD_TOOLCHAIN_PINS}"),
        &profile.toolchain_pins,
        ISSUE_MISSING_PINS,
        "profile must declare at least one toolchain pin",
        issues,
    );
    validate_required_list(
        &format!("{path}.{FIELD_COMMANDS}"),
        &profile.commands,
        ISSUE_MISSING_COMMANDS,
        "profile must declare at least one validation command",
        issues,
    );
    validate_required_list(
        &format!("{path}.{FIELD_EXPECTED_ARTIFACTS}"),
        &profile.expected_artifacts,
        ISSUE_MISSING_ARTIFACTS,
        "profile must declare at least one expected artifact class",
        issues,
    );
    validate_non_claims(
        &format!("{path}.{FIELD_NON_CLAIMS}"),
        &profile.non_claims,
        issues,
    );
    let key = format!("{}::{}", profile.repo, profile.profile_id);
    if !profile.repo.is_empty() && !profile.profile_id.is_empty() && !seen_profiles.insert(key) {
        push_issue(
            issues,
            ISSUE_AMBIGUOUS,
            &format!("{path}.{FIELD_PROFILE_ID}"),
            "repo/profile ID pair must be unique",
        );
    }
}

fn validate_purpose(index: usize, purpose: &str, issues: &mut Vec<ToolchainProfileIssue>) {
    let path = format!("{FIELD_PROFILES}[{index}].{FIELD_PURPOSE}");
    if !matches!(
        purpose,
        PURPOSE_DEV | PURPOSE_CI | PURPOSE_RELEASE | PURPOSE_PROOF | PURPOSE_COMPATIBILITY
    ) {
        push_issue(
            issues,
            ISSUE_PURPOSE,
            &path,
            "purpose must be dev, ci, release, proof, or compatibility",
        );
    }
}

fn validate_required(field_path: &str, value: &str, issues: &mut Vec<ToolchainProfileIssue>) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIRED,
            field_path,
            "field must not be empty",
        );
    }
}

fn validate_required_list(
    field_path: &str,
    values: &[String],
    code: &str,
    message: &str,
    issues: &mut Vec<ToolchainProfileIssue>,
) {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        push_issue(issues, code, field_path, message);
    }
}

fn validate_non_claims(
    field_path: &str,
    non_claims: &[String],
    issues: &mut Vec<ToolchainProfileIssue>,
) {
    if non_claims.is_empty() {
        push_issue(
            issues,
            ISSUE_NON_CLAIM,
            field_path,
            "profile matrix non-claims are required",
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
                &format!("profile matrix non-claims must mention `{required}`"),
            );
        }
    }
}

fn parse_toolchain_profile_matrix_input(
    value: &serde_json::Value,
) -> Result<(ToolchainProfileMatrix, String), ToolchainProfileIssue> {
    let matrix_value = object_field(value, FIELD_MATRIX)?;
    let matrix_hash = crate::verified::evidence::canonical_json_hash(matrix_value);
    let matrix = ToolchainProfileMatrix {
        schema_version: string_field(matrix_value, FIELD_SCHEMA_VERSION)?,
        profiles: parse_profiles(array_field(matrix_value, FIELD_PROFILES)?)?,
        non_claims: string_array_field(matrix_value, FIELD_NON_CLAIMS)?,
    };
    Ok((matrix, matrix_hash))
}

fn parse_profiles(
    values: &[serde_json::Value],
) -> Result<Vec<ToolchainProfileRow>, ToolchainProfileIssue> {
    values
        .iter()
        .map(|value| {
            Ok(ToolchainProfileRow {
                repo: string_field(value, FIELD_REPO)?,
                profile_id: string_field(value, FIELD_PROFILE_ID)?,
                purpose: string_field(value, FIELD_PURPOSE)?,
                toolchain_pins: string_array_field(value, FIELD_TOOLCHAIN_PINS)?,
                commands: string_array_field(value, FIELD_COMMANDS)?,
                expected_artifacts: string_array_field(value, FIELD_EXPECTED_ARTIFACTS)?,
                non_claims: string_array_field(value, FIELD_NON_CLAIMS)?,
            })
        })
        .collect()
}

fn object_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a serde_json::Value, ToolchainProfileIssue> {
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
) -> Result<&'a [serde_json::Value], ToolchainProfileIssue> {
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

fn string_field(value: &serde_json::Value, field: &str) -> Result<String, ToolchainProfileIssue> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| input_issue(field, &format!("input field `{field}` must be a string")))
}

fn string_array_field(
    value: &serde_json::Value,
    field: &str,
) -> Result<Vec<String>, ToolchainProfileIssue> {
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

fn input_issue(field_path: &str, message: &str) -> ToolchainProfileIssue {
    ToolchainProfileIssue {
        code: ISSUE_INVALID_INPUT.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<ToolchainProfileIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(ToolchainProfileIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {

    use super::*;

    const TEST_MATRIX_HASH: &str = "matrix-hash";

    fn non_claims() -> Vec<String> {
        REQUIRED_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect()
    }

    fn profile(repo: &str, profile_id: &str, purpose: &str) -> ToolchainProfileRow {
        ToolchainProfileRow {
            repo: repo.to_string(),
            profile_id: profile_id.to_string(),
            purpose: purpose.to_string(),
            toolchain_pins: vec!["fenix-2026-07-01".to_string(), "onix-nix".to_string()],
            commands: vec!["cargo test --workspace".to_string()],
            expected_artifacts: vec!["test-receipt".to_string()],
            non_claims: non_claims(),
        }
    }

    fn matrix() -> ToolchainProfileMatrix {
        ToolchainProfileMatrix {
            schema_version: TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1.to_string(),
            profiles: vec![
                profile("cairn", "cairn.dev", PURPOSE_DEV),
                profile("cairn", "cairn.ci", PURPOSE_CI),
                profile("cairn", "cairn.release", PURPOSE_RELEASE),
                profile("cairn", "cairn.proof", PURPOSE_PROOF),
                profile("cairn", "cairn.compat", PURPOSE_COMPATIBILITY),
            ],
            non_claims: non_claims(),
        }
    }

    fn codes(report: &ToolchainProfileReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn complete_profile_classes_pass() {
        // r[verify cairn.toolchain_profile_matrix.fixtures.positive]
        // r[verify cairn.toolchain_profile_matrix.validation.receipt]
        let matrix = matrix();
        let report = validate_toolchain_profile_matrix(&matrix, TEST_MATRIX_HASH.to_string());
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(report.matrix_hash, TEST_MATRIX_HASH);
        assert_eq!(report.profile_count, matrix.profiles.len());
        assert_eq!(report.invalid_profile_count, EMPTY_COUNT);
    }

    #[test]
    fn missing_pins_commands_and_weakened_non_claims_fail() {
        // r[verify cairn.toolchain_profile_matrix.fixtures.negative]
        let mut matrix = matrix();
        matrix.profiles[0].toolchain_pins.clear();
        matrix.profiles[1].commands.clear();
        matrix.profiles[2].non_claims = vec!["claims command success".to_string()];
        let report = validate_toolchain_profile_matrix(&matrix, TEST_MATRIX_HASH.to_string());
        let codes = codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_MISSING_PINS.to_string()));
        assert!(codes.contains(&ISSUE_MISSING_COMMANDS.to_string()));
        assert!(codes.contains(&ISSUE_NON_CLAIM.to_string()));
    }

    #[test]
    fn ambiguous_profile_fails_closed() {
        // r[verify cairn.toolchain_profile_matrix.fixtures.negative]
        let mut matrix = matrix();
        matrix
            .profiles
            .push(profile("cairn", "cairn.dev", PURPOSE_DEV));
        let report = validate_toolchain_profile_matrix(&matrix, TEST_MATRIX_HASH.to_string());
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_AMBIGUOUS.to_string()));
    }

    #[test]
    fn profile_non_claim_boundary_is_visible() {
        // r[verify cairn.toolchain_profile_matrix.docs]
        // r[verify cairn.toolchain_profile_matrix.docs.non_claims]
        let report = validate_toolchain_profile_matrix(&matrix(), TEST_MATRIX_HASH.to_string());
        assert!(report.boundary.contains("declaration completeness only"));
        assert!(report.boundary.contains("does not prove command success"));
        assert!(report.boundary.contains("separate execution receipts"));
    }

    #[test]
    fn value_parser_and_fixture_matrix_cover_success_and_failure() {
        // r[verify cairn.toolchain_profile_matrix.final_validation]
        // r[verify cairn.toolchain_profile_matrix.final_validation.fixtures]
        let value = serde_json::json!({
            "matrix": {
                "schema_version": TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1,
                "non_claims": REQUIRED_NON_CLAIMS,
                "profiles": [{
                    "repo": "cairn",
                    "profile_id": "cairn.dev",
                    "purpose": "dev",
                    "toolchain_pins": ["fenix", "onix"],
                    "commands": ["cargo test --workspace"],
                    "expected_artifacts": ["test-receipt"],
                    "non_claims": REQUIRED_NON_CLAIMS
                }]
            },
            "policy_hash": "policy-test"
        });
        let valid_report = validate_toolchain_profile_matrix_value(&value);
        assert!(
            valid_report.valid,
            "unexpected issues: {:?}",
            valid_report.issues
        );

        let invalid = serde_json::json!({
            "matrix": {
                "schema_version": TOOLCHAIN_PROFILE_MATRIX_SCHEMA_V1,
                "non_claims": REQUIRED_NON_CLAIMS,
                "profiles": [{
                    "repo": "cairn",
                    "profile_id": "cairn.dev",
                    "purpose": "dev",
                    "toolchain_pins": [],
                    "commands": [],
                    "expected_artifacts": [],
                    "non_claims": []
                }]
            }
        });
        let invalid_report = validate_toolchain_profile_matrix_value(&invalid);
        assert!(!invalid_report.valid);
        assert!(codes(&invalid_report).contains(&ISSUE_MISSING_PINS.to_string()));
    }
}
