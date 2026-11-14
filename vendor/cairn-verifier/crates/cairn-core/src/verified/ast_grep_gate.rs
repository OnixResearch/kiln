use serde_json::{json, Value};

use super::evidence::canonical_json_hash;

pub const AST_GREP_GATE_MODE_INVENTORY: &str = "inventory";
pub const AST_GREP_GATE_MODE_ADVISORY: &str = "advisory";
pub const AST_GREP_GATE_MODE_BLOCKING: &str = "blocking";
pub const AST_GREP_GATE_MODE_CODEMOD_REVIEW: &str = "codemod_review";
pub const AST_GREP_GATE_BOUNDARY: &str = "ast-grep lifecycle gate validation proves only receipt shape, rule-bundle identity, scope conformance, finding-count posture, and declared non-claims; it does not prove source correctness, does not prove requirement satisfaction, does not prove proof discharge, and does not prove release eligibility";

pub const AST_GREP_GATE_ISSUE_MISSING_FIELD: &str = "ast_grep_gate.missing_field";
pub const AST_GREP_GATE_ISSUE_MALFORMED_HASH: &str = "ast_grep_gate.malformed_hash";
pub const AST_GREP_GATE_ISSUE_UNSUPPORTED_MODE: &str = "ast_grep_gate.unsupported_mode";
pub const AST_GREP_GATE_ISSUE_UNSUPPORTED_OUTPUT_FORMAT: &str =
    "ast_grep_gate.unsupported_output_format";
pub const AST_GREP_GATE_ISSUE_STALE_SCOPE: &str = "ast_grep_gate.stale_scope";
pub const AST_GREP_GATE_ISSUE_WEAK_NON_CLAIM: &str = "ast_grep_gate.weak_non_claim";
pub const AST_GREP_GATE_ISSUE_OVERCLAIM: &str = "ast_grep_gate.overclaim";
pub const AST_GREP_GATE_ISSUE_BLOCKING_FINDING: &str = "ast_grep_gate.blocking_finding";
pub const AST_GREP_GATE_ISSUE_CODEMOD_REVIEW: &str = "ast_grep_gate.codemod_review";
pub const AST_GREP_GATE_ISSUE_MALFORMED_FINDING: &str = "ast_grep_gate.malformed_finding";

const AST_GREP_TOOL_NAME: &str = "ast-grep";
const BLAKE3_HEX_LENGTH: usize = 64;
const OUTPUT_FORMAT_JSON: &str = "json";
const OUTPUT_FORMAT_JSONL: &str = "jsonl";
const OUTPUT_FORMAT_SARIF: &str = "sarif";
const FIELD_RECEIPT_HASH: &str = "receipt_hash";
const FIELD_TOOL: &str = "tool";
const FIELD_NAME: &str = "name";
const FIELD_VERSION: &str = "version";
const FIELD_RULE_BUNDLE_HASH: &str = "rule_bundle_hash";
const FIELD_REPOSITORY_SCOPE: &str = "repository_scope";
const FIELD_COMMAND: &str = "command";
const FIELD_OUTPUT_FORMAT: &str = "output_format";
const FIELD_FINDINGS_SUMMARY: &str = "findings_summary";
const FIELD_TOTAL: &str = "total";
const FIELD_ERRORS: &str = "errors";
const FIELD_WARNINGS: &str = "warnings";
const FIELD_INFOS: &str = "infos";
const FIELD_CODEMODS: &str = "codemods";
const FIELD_CODEMOD_REVIEW: &str = "codemod_review";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_FINDINGS: &str = "findings";
const FIELD_RULE_ID: &str = "rule_id";
const FIELD_SOURCE_SPAN: &str = "source_span";
const FIELD_VERIFICATION_ROLE: &str = "verification_role";
const FIELD_PATH: &str = "path";
const FIELD_START_LINE: &str = "start_line";
const FIELD_END_LINE: &str = "end_line";
const CODEMOD_REVIEWED: &str = "reviewed";
const CODEMOD_NOT_REQUIRED: &str = "not_required";
const EXPECTED_SCOPE_RELATIVE: &str = "safe relative repository scope";

const REQUIRED_NON_CLAIM_FRAGMENTS: &[&str] = &[
    "source correctness",
    "requirement satisfaction",
    "proof discharge",
    "release eligibility",
];

const OVERCLAIM_PHRASES: &[&str] = &[
    "proves source correctness",
    "proves requirement satisfaction",
    "proves proof discharge",
    "proves release eligibility",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstGrepGatePolicy {
    pub mode: String,
    pub allowed_output_formats: Vec<String>,
    pub expected_repository_scope: String,
    pub required_non_claim_fragments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstGrepGateIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstGrepGateReport {
    pub valid: bool,
    pub passed: bool,
    pub mode: String,
    pub disposition: String,
    pub issues: Vec<AstGrepGateIssue>,
    pub finding_count: u64,
    pub error_count: u64,
    pub advisory_count: u64,
    pub codemod_count: u64,
    pub valence_compatible_finding_count: u64,
    pub rule_bundle_hash: Option<String>,
    pub receipt_hash: String,
    pub boundary: String,
}

impl AstGrepGateIssue {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "code": self.code,
            "field_path": self.field_path,
            "expected": self.expected,
            "actual": self.actual,
        })
    }
}

impl AstGrepGateReport {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "valid": self.valid,
            "passed": self.passed,
            "mode": self.mode,
            "disposition": self.disposition,
            "issues": self.issues.iter().map(AstGrepGateIssue::to_json).collect::<Vec<_>>(),
            "finding_count": self.finding_count,
            "error_count": self.error_count,
            "advisory_count": self.advisory_count,
            "codemod_count": self.codemod_count,
            "valence_compatible_finding_count": self.valence_compatible_finding_count,
            "rule_bundle_hash": self.rule_bundle_hash,
            "receipt_hash": self.receipt_hash,
            "boundary": self.boundary,
        })
    }
}

#[must_use]
pub fn default_ast_grep_gate_policy(mode: &str) -> AstGrepGatePolicy {
    // r[impl cairn.ast_grep_gate.profile]
    AstGrepGatePolicy {
        mode: mode.to_string(),
        allowed_output_formats: vec![
            OUTPUT_FORMAT_JSON.to_string(),
            OUTPUT_FORMAT_JSONL.to_string(),
            OUTPUT_FORMAT_SARIF.to_string(),
        ],
        expected_repository_scope: ".".to_string(),
        required_non_claim_fragments: REQUIRED_NON_CLAIM_FRAGMENTS
            .iter()
            .map(|fragment| (*fragment).to_string())
            .collect(),
    }
}

#[must_use]
pub fn ast_grep_receipt_hash(receipt: &Value) -> String {
    canonical_json_hash(&ast_grep_receipt_hash_material(receipt))
}

#[must_use]
pub fn ast_grep_receipt_hash_material(receipt: &Value) -> Value {
    let mut material = receipt.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove(FIELD_RECEIPT_HASH);
    }
    material
}

#[must_use]
pub fn validate_ast_grep_receipt(policy: &AstGrepGatePolicy, receipt: &Value) -> AstGrepGateReport {
    // r[impl cairn.ast_grep_gate.receipt]
    // r[impl cairn.ast_grep_gate.shell_boundary]
    let mut issues = Vec::new();
    validate_policy(policy, &mut issues);
    validate_tool_identity(receipt, &mut issues);
    validate_rule_bundle_hash(receipt, &mut issues);
    validate_repository_scope(policy, receipt, &mut issues);
    validate_command_identity(receipt, &mut issues);
    validate_output_format(policy, receipt, &mut issues);
    let summary = findings_summary(receipt, &mut issues);
    validate_non_claims(policy, receipt, &mut issues);
    validate_mode_posture(policy, &summary, receipt, &mut issues);
    let valence_compatible_finding_count = validate_valence_findings(receipt, &mut issues);
    sort_ast_grep_issues(&mut issues);

    let valid = !issues.iter().any(|issue| {
        issue.code != AST_GREP_GATE_ISSUE_BLOCKING_FINDING
            || policy.mode == AST_GREP_GATE_MODE_BLOCKING
    });
    let passed = issues.is_empty();
    AstGrepGateReport {
        valid,
        passed,
        mode: policy.mode.clone(),
        disposition: disposition(policy, &issues),
        issues,
        finding_count: summary.total,
        error_count: summary.errors,
        advisory_count: summary.warnings + summary.infos,
        codemod_count: summary.codemods,
        valence_compatible_finding_count,
        rule_bundle_hash: receipt_string(receipt, FIELD_RULE_BUNDLE_HASH),
        receipt_hash: ast_grep_receipt_hash(receipt),
        boundary: AST_GREP_GATE_BOUNDARY.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FindingSummary {
    total: u64,
    errors: u64,
    warnings: u64,
    infos: u64,
    codemods: u64,
}

fn validate_policy(policy: &AstGrepGatePolicy, issues: &mut Vec<AstGrepGateIssue>) {
    if !is_ast_grep_gate_mode(&policy.mode) {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_UNSUPPORTED_MODE,
            "policy.mode",
            "inventory, advisory, blocking, or codemod_review",
            &policy.mode,
        ));
    }
}

fn validate_tool_identity(receipt: &Value, issues: &mut Vec<AstGrepGateIssue>) {
    let Some(tool) = receipt.get(FIELD_TOOL).and_then(Value::as_object) else {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_TOOL,
            "ast-grep tool identity",
            "missing",
        ));
        return;
    };
    if tool.get(FIELD_NAME).and_then(Value::as_str) != Some(AST_GREP_TOOL_NAME) {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            "tool.name",
            AST_GREP_TOOL_NAME,
            tool.get(FIELD_NAME)
                .and_then(Value::as_str)
                .unwrap_or("missing"),
        ));
    }
    if tool
        .get(FIELD_VERSION)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            "tool.version",
            "ast-grep version",
            "missing",
        ));
    }
}

fn validate_rule_bundle_hash(receipt: &Value, issues: &mut Vec<AstGrepGateIssue>) {
    let hash = receipt
        .get(FIELD_RULE_BUNDLE_HASH)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if hash.is_empty() {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_RULE_BUNDLE_HASH,
            "BLAKE3 rule bundle hash",
            "missing",
        ));
    } else if !is_blake3_hex(hash) {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MALFORMED_HASH,
            FIELD_RULE_BUNDLE_HASH,
            "lowercase BLAKE3 rule bundle hash",
            "malformed",
        ));
    }
}

fn validate_repository_scope(
    policy: &AstGrepGatePolicy,
    receipt: &Value,
    issues: &mut Vec<AstGrepGateIssue>,
) {
    let scope = receipt
        .get(FIELD_REPOSITORY_SCOPE)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if scope.is_empty() {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_REPOSITORY_SCOPE,
            EXPECTED_SCOPE_RELATIVE,
            "missing",
        ));
        return;
    }
    if scope.starts_with('/') || scope.contains("..") || scope != policy.expected_repository_scope {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_STALE_SCOPE,
            FIELD_REPOSITORY_SCOPE,
            &policy.expected_repository_scope,
            scope,
        ));
    }
}

fn validate_command_identity(receipt: &Value, issues: &mut Vec<AstGrepGateIssue>) {
    if receipt
        .get(FIELD_COMMAND)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_COMMAND,
            "ast-grep command identity",
            "missing",
        ));
    }
}

fn validate_output_format(
    policy: &AstGrepGatePolicy,
    receipt: &Value,
    issues: &mut Vec<AstGrepGateIssue>,
) {
    let output_format = receipt
        .get(FIELD_OUTPUT_FORMAT)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if output_format.is_empty() {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_OUTPUT_FORMAT,
            "ast-grep output format",
            "missing",
        ));
        return;
    }
    if !policy
        .allowed_output_formats
        .iter()
        .any(|allowed| allowed == output_format)
    {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_UNSUPPORTED_OUTPUT_FORMAT,
            FIELD_OUTPUT_FORMAT,
            "policy-declared ast-grep output format",
            output_format,
        ));
    }
}

fn findings_summary(receipt: &Value, issues: &mut Vec<AstGrepGateIssue>) -> FindingSummary {
    let Some(summary) = receipt.get(FIELD_FINDINGS_SUMMARY) else {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_FINDINGS_SUMMARY,
            "ast-grep findings summary",
            "missing",
        ));
        return empty_findings_summary();
    };
    let result = FindingSummary {
        total: summary_count(summary, FIELD_TOTAL, issues),
        errors: summary_count(summary, FIELD_ERRORS, issues),
        warnings: summary_count(summary, FIELD_WARNINGS, issues),
        infos: summary_count(summary, FIELD_INFOS, issues),
        codemods: summary_count(summary, FIELD_CODEMODS, issues),
    };
    if result.errors + result.warnings + result.infos < result.total {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            FIELD_FINDINGS_SUMMARY,
            "finding count covered by severity buckets",
            "inconsistent",
        ));
    }
    result
}

fn empty_findings_summary() -> FindingSummary {
    FindingSummary {
        total: 0,
        errors: 0,
        warnings: 0,
        infos: 0,
        codemods: 0,
    }
}

fn summary_count(summary: &Value, field: &str, issues: &mut Vec<AstGrepGateIssue>) -> u64 {
    let Some(value) = summary.get(field).and_then(Value::as_u64) else {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MISSING_FIELD,
            &format!("findings_summary.{field}"),
            "non-negative finding count",
            "missing",
        ));
        return 0;
    };
    value
}

fn validate_non_claims(
    policy: &AstGrepGatePolicy,
    receipt: &Value,
    issues: &mut Vec<AstGrepGateIssue>,
) {
    let Some(non_claims) = receipt.get(FIELD_NON_CLAIMS).and_then(Value::as_array) else {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_WEAK_NON_CLAIM,
            FIELD_NON_CLAIMS,
            "explicit ast-grep gate non-claims",
            "missing",
        ));
        return;
    };
    let joined = non_claims
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("; ")
        .to_ascii_lowercase();
    for fragment in &policy.required_non_claim_fragments {
        let expected = format!("does not prove {fragment}");
        if !joined.contains(&expected) {
            issues.push(ast_grep_issue(
                AST_GREP_GATE_ISSUE_WEAK_NON_CLAIM,
                FIELD_NON_CLAIMS,
                &expected,
                "missing",
            ));
        }
    }
    let claim_text = receipt
        .get("claim")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    for phrase in OVERCLAIM_PHRASES {
        if claim_text.contains(phrase) || joined.contains(phrase) {
            issues.push(ast_grep_issue(
                AST_GREP_GATE_ISSUE_OVERCLAIM,
                "claim",
                "bounded ast-grep non-claim text",
                "overclaim",
            ));
        }
    }
}

fn validate_mode_posture(
    policy: &AstGrepGatePolicy,
    summary: &FindingSummary,
    receipt: &Value,
    issues: &mut Vec<AstGrepGateIssue>,
) {
    match policy.mode.as_str() {
        AST_GREP_GATE_MODE_BLOCKING if summary.errors > 0 => issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_BLOCKING_FINDING,
            "findings_summary.errors",
            "zero blocking ast-grep errors",
            "blocking findings present",
        )),
        AST_GREP_GATE_MODE_CODEMOD_REVIEW if summary.codemods > 0 => {
            let review = receipt
                .get(FIELD_CODEMOD_REVIEW)
                .and_then(Value::as_str)
                .unwrap_or_default();
            if review != CODEMOD_REVIEWED && review != CODEMOD_NOT_REQUIRED {
                issues.push(ast_grep_issue(
                    AST_GREP_GATE_ISSUE_CODEMOD_REVIEW,
                    FIELD_CODEMOD_REVIEW,
                    "reviewed codemod evidence",
                    if review.is_empty() { "missing" } else { review },
                ));
            }
        }
        _ => {}
    }
}

fn validate_valence_findings(receipt: &Value, issues: &mut Vec<AstGrepGateIssue>) -> u64 {
    let Some(findings) = receipt.get(FIELD_FINDINGS).and_then(Value::as_array) else {
        return 0;
    };
    let mut compatible = 0;
    for (index, finding) in findings.iter().enumerate() {
        let field_path = format!("findings.{index}");
        let has_rule = finding
            .get(FIELD_RULE_ID)
            .and_then(Value::as_str)
            .map(|rule| !rule.is_empty())
            .unwrap_or(false);
        let has_role = finding
            .get(FIELD_VERIFICATION_ROLE)
            .and_then(Value::as_str)
            .map(|role| !role.is_empty())
            .unwrap_or(false);
        let has_span = validate_source_span(finding.get(FIELD_SOURCE_SPAN), &field_path, issues);
        if has_rule && has_role && has_span {
            compatible += 1;
        } else {
            issues.push(ast_grep_issue(
                AST_GREP_GATE_ISSUE_MALFORMED_FINDING,
                &field_path,
                "rule id, verification role, and source span",
                "missing",
            ));
        }
    }
    compatible
}

fn validate_source_span(
    span: Option<&Value>,
    parent_path: &str,
    issues: &mut Vec<AstGrepGateIssue>,
) -> bool {
    let Some(span) = span else {
        return false;
    };
    let path = span
        .get(FIELD_PATH)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let start_line = span.get(FIELD_START_LINE).and_then(Value::as_u64);
    let end_line = span.get(FIELD_END_LINE).and_then(Value::as_u64);
    let path_ok = !path.is_empty() && !path.starts_with('/') && !path.contains("..");
    let line_ok = matches!((start_line, end_line), (Some(start), Some(end)) if start <= end);
    if !path_ok || !line_ok {
        issues.push(ast_grep_issue(
            AST_GREP_GATE_ISSUE_MALFORMED_FINDING,
            &format!("{parent_path}.source_span"),
            "safe relative source span",
            "malformed",
        ));
    }
    path_ok && line_ok
}

fn disposition(policy: &AstGrepGatePolicy, issues: &[AstGrepGateIssue]) -> String {
    if issues.is_empty() {
        return "passed".to_string();
    }
    if policy.mode == AST_GREP_GATE_MODE_ADVISORY
        && issues
            .iter()
            .all(|issue| issue.code == AST_GREP_GATE_ISSUE_BLOCKING_FINDING)
    {
        return "advisory".to_string();
    }
    "failed".to_string()
}

fn ast_grep_issue(code: &str, field_path: &str, expected: &str, actual: &str) -> AstGrepGateIssue {
    AstGrepGateIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn receipt_string(receipt: &Value, field: &str) -> Option<String> {
    receipt
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn sort_ast_grep_issues(issues: &mut [AstGrepGateIssue]) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.expected.cmp(&right.expected))
    });
}

#[must_use]
pub fn is_ast_grep_gate_mode(mode: &str) -> bool {
    matches!(
        mode,
        AST_GREP_GATE_MODE_INVENTORY
            | AST_GREP_GATE_MODE_ADVISORY
            | AST_GREP_GATE_MODE_BLOCKING
            | AST_GREP_GATE_MODE_CODEMOD_REVIEW
    )
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const EXPECTED_VALENCE_FINDINGS: u64 = 1;

    fn receipt() -> Value {
        json!({
            "tool": {"name": "ast-grep", "version": "0.39.0"},
            "rule_bundle_hash": RULE_HASH,
            "repository_scope": ".",
            "command": "ast-grep scan --json",
            "output_format": "json",
            "findings_summary": {
                "total": 1,
                "errors": 0,
                "warnings": 1,
                "infos": 0,
                "codemods": 0
            },
            "non_claims": [
                "does not prove source correctness",
                "does not prove requirement satisfaction",
                "does not prove proof discharge",
                "does not prove release eligibility"
            ],
            "findings": [{
                "rule_id": "cairn.no-unwrap",
                "verification_role": "advisory",
                "source_span": {"path": "crates/demo/src/lib.rs", "start_line": 1, "end_line": 1}
            }]
        })
    }

    fn issue_codes(report: &AstGrepGateReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn advisory_receipt_with_findings_passes_as_inventory() {
        // r[verify cairn.ast_grep_gate.profile]
        // r[verify cairn.ast_grep_gate.receipt]
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_ADVISORY),
            &receipt(),
        );

        assert!(report.valid, "{:?}", report.issues);
        assert!(report.passed, "{:?}", report.issues);
        assert_eq!(report.finding_count, 1);
        assert_eq!(report.rule_bundle_hash.as_deref(), Some(RULE_HASH));
        assert!(report
            .boundary
            .contains("does not prove source correctness"));
    }

    #[test]
    fn blocking_mode_fails_on_error_findings() {
        let mut receipt = receipt();
        receipt["findings_summary"]["errors"] = json!(1);
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_BLOCKING),
            &receipt,
        );

        assert!(!report.passed);
        assert!(issue_codes(&report).contains(&AST_GREP_GATE_ISSUE_BLOCKING_FINDING.to_string()));
    }

    #[test]
    fn malformed_and_overclaiming_receipts_fail_closed() {
        // r[verify cairn.ast_grep_gate.negative_fixtures]
        let mut receipt = receipt();
        receipt["tool"] = json!({"name": "other"});
        receipt["rule_bundle_hash"] = json!("sha256:not-blake3");
        receipt["repository_scope"] = json!("/tmp/repo");
        receipt["output_format"] = json!("text");
        receipt["non_claims"] = json!(["proves release eligibility"]);
        receipt["claim"] = json!("proves source correctness");
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_BLOCKING),
            &receipt,
        );
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_MISSING_FIELD.to_string()));
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_MALFORMED_HASH.to_string()));
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_STALE_SCOPE.to_string()));
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_UNSUPPORTED_OUTPUT_FORMAT.to_string()));
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_WEAK_NON_CLAIM.to_string()));
        assert!(codes.contains(&AST_GREP_GATE_ISSUE_OVERCLAIM.to_string()));
    }

    #[test]
    fn codemod_review_mode_requires_reviewed_codemod_evidence() {
        let mut receipt = receipt();
        receipt["findings_summary"]["codemods"] = json!(1);
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_CODEMOD_REVIEW),
            &receipt,
        );
        assert!(!report.passed);
        assert!(issue_codes(&report).contains(&AST_GREP_GATE_ISSUE_CODEMOD_REVIEW.to_string()));

        receipt["codemod_review"] = json!(CODEMOD_REVIEWED);
        let reviewed = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_CODEMOD_REVIEW),
            &receipt,
        );
        assert!(reviewed.passed, "{:?}", reviewed.issues);
    }

    #[test]
    fn valence_compatible_fields_are_preserved_and_counted() {
        // r[verify cairn.ast_grep_gate.valence_compat]
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_INVENTORY),
            &receipt(),
        );

        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(
            report.valence_compatible_finding_count,
            EXPECTED_VALENCE_FINDINGS
        );
    }

    #[test]
    fn shell_boundary_and_validation_non_claims_are_visible() {
        // r[verify cairn.ast_grep_gate.shell_boundary]
        // r[verify cairn.ast_grep_gate.validation]
        let report = validate_ast_grep_receipt(
            &default_ast_grep_gate_policy(AST_GREP_GATE_MODE_INVENTORY),
            &receipt(),
        );
        let rendered = report.to_json();

        assert!(report.valid, "{:?}", report.issues);
        assert!(rendered["boundary"]
            .as_str()
            .expect("boundary")
            .contains("does not prove release eligibility"));
        assert_eq!(
            rendered["receipt_hash"].as_str().unwrap().len(),
            BLAKE3_HEX_LENGTH
        );
    }
}
