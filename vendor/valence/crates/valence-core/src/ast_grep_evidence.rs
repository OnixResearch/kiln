//! Pure adapter for bounded ast-grep source-shape evidence.

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::EvidenceSpan;
use crate::evidence_ir::VerificationRole;
use crate::evidence_ir::validate_evidence_span;
use crate::shared_identity::blake3_hex;

pub const AST_GREP_PROFILE: &str = "valence.ast-grep-evidence.v1";
pub const AST_GREP_REQUIRED_NON_CLAIM: &str = "ast-grep evidence records syntax-aware source shape and does not prove semantic correctness, semantic equivalence, proof discharge, or release eligibility";
const DIGEST_PREFIX: &str = "b3:";
const ONE_BASED_OFFSET: u32 = 1;
const OVERCLAIM_MARKERS: &[&str] = &[
    "behavior preserving",
    "behavior-preserving",
    "semantic equivalence",
    "proves correctness",
    "proof discharged",
    "release eligible",
    "release readiness",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstGrepPosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstGrepRange {
    pub start_byte: u64,
    pub end_byte: u64,
    pub start: AstGrepPosition,
    pub end: AstGrepPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstGrepFinding {
    pub profile: String,
    pub tool_version: String,
    pub rule_id: String,
    pub rule_bundle: serde_json::Value,
    pub rule_bundle_identity: String,
    pub scan_scope: serde_json::Value,
    pub scan_scope_identity: String,
    pub finding_identity: String,
    pub source_id: String,
    pub source_path: String,
    pub range: Option<AstGrepRange>,
    pub label: Option<String>,
    pub replacement_preview: Option<String>,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AstGrepIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl AstGrepIssue {
    const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AstGrepReport {
    pub valid: bool,
    pub source_span: Option<EvidenceSpan>,
    pub issues: Vec<AstGrepIssue>,
    pub non_claims: Vec<String>,
}

#[derive(Serialize)]
struct FindingIdentityMaterial<'a> {
    profile: &'a str,
    tool_version: &'a str,
    rule_id: &'a str,
    rule_bundle_identity: &'a str,
    scan_scope_identity: &'a str,
    source_id: &'a str,
    source_path: &'a str,
    range: &'a Option<AstGrepRange>,
    label: &'a Option<String>,
    replacement_preview: &'a Option<String>,
    verification_role: &'a str,
    non_claims: &'a [String],
}

#[must_use]
pub fn ast_grep_value_identity(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).expect("JSON value serialization is infallible");
    format!("{DIGEST_PREFIX}{}", blake3_hex(&bytes))
}

#[must_use]
pub fn ast_grep_finding_identity(finding: &AstGrepFinding) -> String {
    let material = FindingIdentityMaterial {
        profile: &finding.profile,
        tool_version: &finding.tool_version,
        rule_id: &finding.rule_id,
        rule_bundle_identity: &finding.rule_bundle_identity,
        scan_scope_identity: &finding.scan_scope_identity,
        source_id: &finding.source_id,
        source_path: &finding.source_path,
        range: &finding.range,
        label: &finding.label,
        replacement_preview: &finding.replacement_preview,
        verification_role: finding.verification_role.as_str(),
        non_claims: &finding.non_claims,
    };
    let bytes = serde_json::to_vec(&material).expect("typed identity serialization is infallible");
    format!("{DIGEST_PREFIX}{}", blake3_hex(&bytes))
}

#[must_use]
pub fn validate_ast_grep_finding(finding: &AstGrepFinding) -> AstGrepReport {
    let mut issues = Vec::new();
    validate_required_identity(finding, &mut issues);
    validate_role_and_claims(finding, &mut issues);
    let source_span = map_source_span(finding, &mut issues);
    issues.sort();
    issues.dedup();
    AstGrepReport {
        valid: issues.is_empty(),
        source_span,
        issues,
        non_claims: finding.non_claims.clone(),
    }
}

fn validate_required_identity(finding: &AstGrepFinding, issues: &mut Vec<AstGrepIssue>) {
    if finding.profile != AST_GREP_PROFILE {
        issues.push(AstGrepIssue::new("profile", "unsupported_profile"));
    }
    for (field, value) in [
        ("tool_version", finding.tool_version.as_str()),
        ("rule_id", finding.rule_id.as_str()),
        ("source_id", finding.source_id.as_str()),
        ("source_path", finding.source_path.as_str()),
    ] {
        if value.trim().is_empty() || value.trim() != value || value.chars().any(char::is_control) {
            issues.push(AstGrepIssue::new(field, "invalid_required_identity"));
        }
    }
    if finding.rule_bundle_identity != ast_grep_value_identity(&finding.rule_bundle) {
        issues.push(AstGrepIssue::new("rule_bundle_identity", "stale_rule_bundle_identity"));
    }
    if finding.scan_scope_identity != ast_grep_value_identity(&finding.scan_scope) {
        issues.push(AstGrepIssue::new("scan_scope_identity", "stale_scan_scope_identity"));
    }
    if finding.finding_identity != ast_grep_finding_identity(finding) {
        issues.push(AstGrepIssue::new("finding_identity", "stale_finding_identity"));
    }
}

fn validate_role_and_claims(finding: &AstGrepFinding, issues: &mut Vec<AstGrepIssue>) {
    if finding.verification_role == VerificationRole::Property {
        issues.push(AstGrepIssue::new("verification_role", "unsupported_role_promotion"));
    }
    if !finding.non_claims.iter().any(|claim| claim == AST_GREP_REQUIRED_NON_CLAIM) {
        issues.push(AstGrepIssue::new("non_claims", "required_non_claim_missing"));
    }
    let metadata = finding
        .non_claims
        .iter()
        .filter(|claim| claim.as_str() != AST_GREP_REQUIRED_NON_CLAIM)
        .map(String::as_str)
        .chain(finding.replacement_preview.iter().map(String::as_str));
    if metadata
        .map(str::to_ascii_lowercase)
        .any(|text| OVERCLAIM_MARKERS.iter().any(|marker| text.contains(marker)))
    {
        issues.push(AstGrepIssue::new("non_claims", "unsupported_overclaim"));
    }
}

fn map_source_span(finding: &AstGrepFinding, issues: &mut Vec<AstGrepIssue>) -> Option<EvidenceSpan> {
    let Some(range) = &finding.range else {
        issues.push(AstGrepIssue::new("range", "source_span_missing"));
        return None;
    };
    let Some(start_line) = range.start.line.checked_add(ONE_BASED_OFFSET) else {
        issues.push(AstGrepIssue::new("range", "source_coordinate_overflow"));
        return None;
    };
    let Some(start_column) = range.start.column.checked_add(ONE_BASED_OFFSET) else {
        issues.push(AstGrepIssue::new("range", "source_coordinate_overflow"));
        return None;
    };
    let Some(end_line) = range.end.line.checked_add(ONE_BASED_OFFSET) else {
        issues.push(AstGrepIssue::new("range", "source_coordinate_overflow"));
        return None;
    };
    let Some(end_column) = range.end.column.checked_add(ONE_BASED_OFFSET) else {
        issues.push(AstGrepIssue::new("range", "source_coordinate_overflow"));
        return None;
    };
    let span = EvidenceSpan {
        source_id: finding.source_id.clone(),
        source_path: finding.source_path.clone(),
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        start_line,
        start_column,
        end_line,
        end_column,
        label: finding.label.clone(),
    };
    for issue in validate_evidence_span(&span) {
        issues.push(AstGrepIssue::new(issue.field, issue.code));
    }
    Some(span)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding() -> AstGrepFinding {
        let bundle = serde_json::json!({"rules": [{"id": "no-unwrap", "pattern": "$X.unwrap()"}]});
        let scope = serde_json::json!({"root": "crates/valence-core", "languages": ["rust"]});
        let mut finding = AstGrepFinding {
            profile: AST_GREP_PROFILE.to_owned(),
            tool_version: "0.40.0".to_owned(),
            rule_id: "no-unwrap".to_owned(),
            rule_bundle_identity: ast_grep_value_identity(&bundle),
            scan_scope_identity: ast_grep_value_identity(&scope),
            rule_bundle: bundle,
            scan_scope: scope,
            finding_identity: String::new(),
            source_id: "ast-grep:no-unwrap:001".to_owned(),
            source_path: "src/lib.rs".to_owned(),
            range: Some(AstGrepRange {
                start_byte: 10,
                end_byte: 20,
                start: AstGrepPosition { line: 2, column: 4 },
                end: AstGrepPosition { line: 2, column: 14 },
            }),
            label: Some("no-unwrap".to_owned()),
            replacement_preview: Some("match value { ... }".to_owned()),
            verification_role: VerificationRole::RecordedOnly,
            non_claims: vec![AST_GREP_REQUIRED_NON_CLAIM.to_owned()],
        };
        finding.finding_identity = ast_grep_finding_identity(&finding);
        finding
    }

    #[test]
    fn valid_finding_maps_exact_span() {
        let report = validate_ast_grep_finding(&finding());
        assert!(report.valid, "{:?}", report.issues);
        let span = report.source_span.expect("valid finding has span");
        assert_eq!(span.start_line, 3);
        assert_eq!(span.start_column, 5);
        assert_eq!(span.start_byte, 10);
    }

    #[test]
    fn stale_bundle_missing_span_and_role_promotion_fail() {
        let mut candidate = finding();
        candidate.rule_bundle["rules"][0]["pattern"] = serde_json::json!("different");
        candidate.range = None;
        candidate.verification_role = VerificationRole::Property;
        candidate.finding_identity = ast_grep_finding_identity(&candidate);
        let report = validate_ast_grep_finding(&candidate);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stale_rule_bundle_identity"));
        assert!(report.issues.iter().any(|issue| issue.code == "source_span_missing"));
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_role_promotion"));
    }

    #[test]
    fn malformed_span_and_rewrite_overclaim_fail() {
        let mut candidate = finding();
        candidate.range.as_mut().unwrap().start_byte = 30;
        candidate.replacement_preview = Some("behavior-preserving rewrite".to_owned());
        candidate.finding_identity = ast_grep_finding_identity(&candidate);
        let report = validate_ast_grep_finding(&candidate);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "span_bytes_reversed"));
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_overclaim"));
    }

    #[test]
    fn finding_identity_binds_review_metadata() {
        let mut candidate = finding();
        candidate.replacement_preview = Some("changed preview".to_owned());
        let report = validate_ast_grep_finding(&candidate);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stale_finding_identity"));
    }
}
