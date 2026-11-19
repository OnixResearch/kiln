//! ast-grep policy-boundary audit rule-pack core.
//!
//! This pure module defines the audit rule pack shape, validates that a rule
//! may only be release-blocking when it carries both a positive and a negative
//! fixture, and binds a deterministic BLAKE3 identity plus explicit non-claims
//! into the audit receipt. It performs no process execution, no filesystem
//! scans, and no ast-grep state. The release verifier shell owns running
//! ast-grep and capturing candidate findings.

use serde::{Deserialize, Serialize};

pub use crate::ast_grep_audit_identity::{
    AST_GREP_AUDIT_SCHEMA, DEFAULT_AST_GREP_TOOL, DEFAULT_AST_GREP_VERSION,
};

/// Default repository scan scope.
pub const DEFAULT_REPOSITORY_SCOPE: &str = "src";
/// Candidate-evidence non-claim shared by every rule.
pub const CANDIDATE_EVIDENCE_NON_CLAIM: &str = "candidate-boundary-evidence-only";

/// Release posture a rule may declare.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Posture {
    /// Advisory findings reported but never block release by default.
    Advisory,
    /// Blocking posture requires a positive and a negative fixture up front.
    Blocking,
}

/// A single structural audit finding class.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuditFinding {
    /// Stable rule id.
    pub rule_id: String,
    /// Machine-readable span/severity marker.
    pub severity: String,
    /// Remediation note for operators.
    pub remediation: String,
}

/// One declared policy-boundary rule.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditRule {
    /// Stable rule id.
    pub id: String,
    /// Declared release posture.
    pub posture: Posture,
    /// Declared severity class.
    pub severity: String,
    /// Whether a positive fixture is declared.
    pub has_positive_fixture: bool,
    /// Whether a negative fixture is declared.
    pub has_negative_fixture: bool,
    /// Ordered non-claims this rule carries.
    pub non_claims: Vec<String>,
}

/// A validated rule pack over Basalt policy-boundary source shapes.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditRulePack {
    /// Rule pack schema identifier.
    pub schema: String,
    /// Scan tool identity.
    pub tool: String,
    /// Scan tool version.
    pub tool_version: String,
    /// Repository scan scope.
    pub repository_scope: String,
    /// Default release posture.
    pub default_posture: Posture,
    /// Declared rules.
    pub rules: Vec<AuditRule>,
}

impl AuditRulePack {
    /// Construct an advisory rule pack with the reviewed schema default.
    #[must_use]
    pub fn new(rules: Vec<AuditRule>) -> Self {
        Self {
            schema: AST_GREP_AUDIT_SCHEMA.to_owned(),
            tool: DEFAULT_AST_GREP_TOOL.to_owned(),
            tool_version: DEFAULT_AST_GREP_VERSION.to_owned(),
            repository_scope: DEFAULT_REPOSITORY_SCOPE.to_owned(),
            default_posture: Posture::Advisory,
            rules,
        }
    }
}

/// Deterministic issue found when validating a rule pack.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RulePackIssue {
    /// Stable issue code.
    pub code: String,
    /// Stable human-readable explanation.
    pub message: String,
}

/// The audit receipt bound to one rule pack scan.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditReceipt {
    /// Receipt schema marker.
    pub schema: String,
    /// ast-grep tool identity.
    pub tool: String,
    /// ast-grep tool version.
    pub tool_version: String,
    /// Repository scan scope.
    pub repository_scope: String,
    /// BLAKE3 identity of the reviewed rule pack.
    pub rule_pack_blake3: String,
    /// Declared release posture.
    pub posture: Posture,
    /// Candidate findings reported in the scan (empty until shell populates).
    pub findings: Vec<AuditFinding>,
    /// Explicit non-claims preserved on every receipt.
    pub non_claims: Vec<String>,
}

impl AuditReceipt {
    /// Return true when the audit is advisory (never blocking by default).
    #[must_use]
    pub fn is_advisory(&self) -> bool {
        self.posture == Posture::Advisory
    }
}

/// Validate a rule pack and refuse to let an unpaired rule become blocking.
///
/// # Errors
///
/// Returns a non-empty ordered `Vec<RulePackIssue>` when the pack violates any
/// structural boundary, in which case the pack must remain advisory.
#[must_use]
pub fn validate_rule_pack(pack: &AuditRulePack) -> Vec<RulePackIssue> {
    let mut issues = Vec::new();
    if pack.schema != AST_GREP_AUDIT_SCHEMA {
        issues.push(RulePackIssue {
            code: "rulepack.schema".to_owned(),
            message: "unsupported ast-grep audit rule-pack schema".to_owned(),
        });
    }
    for rule in &pack.rules {
        if rule.id.is_empty() {
            issues.push(RulePackIssue {
                code: "rulepack.rule.empty-id".to_owned(),
                message: "audit rule id must be non-empty".to_owned(),
            });
        }
        if rule.posture == Posture::Blocking
            && (!rule.has_positive_fixture || !rule.has_negative_fixture)
        {
            issues.push(RulePackIssue {
                code: "rulepack.blocking-requires-fixtures".to_owned(),
                message: format!(
                    "rule {} may not block release without a positive and a negative fixture",
                    rule.id
                ),
            });
        }
        for non_claim in &rule.non_claims {
            if non_claim.is_empty() {
                issues.push(RulePackIssue {
                    code: "rulepack.empty-non-claim".to_owned(),
                    message: "audit rule non-claims must be non-empty".to_owned(),
                });
            }
        }
    }
    issues
}

/// Return the deterministic BLAKE3 identity of a rule pack's canonical bytes.
///
/// # Panics
///
/// Panics only if the crate-owned rule-pack shape fails to serialize.
#[must_use]
pub fn rule_pack_blake3(pack: &AuditRulePack) -> String {
    let bytes = serde_json::to_vec(pack).expect("rule pack serialization is infallible");
    format!("blake3:{}", blake3::hash(bytes.as_slice()).to_hex())
}

/// Build an advisory receipt for a validated rule pack.
///
/// The receipt binds tool identity, rule-pack BLAKE3, scan scope, posture, and
/// non-claims. It never claims authorization.
#[must_use]
pub fn advisory_receipt(pack: &AuditRulePack) -> AuditReceipt {
    let mut non_claims = vec![CANDIDATE_EVIDENCE_NON_CLAIM.to_owned()];
    for rule in &pack.rules {
        for non_claim in &rule.non_claims {
            if !non_claims.contains(non_claim) {
                non_claims.push(non_claim.clone());
            }
        }
    }
    AuditReceipt {
        schema: AST_GREP_AUDIT_SCHEMA.to_owned(),
        tool: pack.tool.clone(),
        tool_version: pack.tool_version.clone(),
        repository_scope: pack.repository_scope.clone(),
        rule_pack_blake3: rule_pack_blake3(pack),
        posture: pack.default_posture,
        findings: Vec::new(),
        non_claims,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advisory_rule() -> AuditRule {
        AuditRule {
            id: "no-direct-enforcement-bypass".to_owned(),
            posture: Posture::Advisory,
            severity: "medium".to_owned(),
            has_positive_fixture: true,
            has_negative_fixture: true,
            non_claims: vec!["does-not-prove-ucan-authority".to_owned()],
        }
    }

    #[test]
    fn positive_advisory_rule_pack_validates() {
        let pack = AuditRulePack::new(vec![advisory_rule()]);
        assert!(validate_rule_pack(&pack).is_empty());
    }

    #[test]
    fn positive_identity_is_deterministic() {
        let pack = AuditRulePack::new(vec![advisory_rule()]);
        assert_eq!(rule_pack_blake3(&pack), rule_pack_blake3(&pack));
        assert!(rule_pack_blake3(&pack).starts_with("blake3:"));
    }

    #[test]
    fn negative_blocking_rule_without_fixtures_fails_closed() {
        let mut rule = advisory_rule();
        rule.posture = Posture::Blocking;
        rule.has_negative_fixture = false;

        let pack = AuditRulePack::new(vec![rule]);
        let issues = validate_rule_pack(&pack);
        assert!(
            issues
                .iter()
                .any(|i| i.code == "rulepack.blocking-requires-fixtures")
        );
    }

    #[test]
    fn negative_blocking_rule_without_positive_fixture_fails_closed() {
        let mut rule = advisory_rule();
        rule.posture = Posture::Blocking;
        rule.has_positive_fixture = false;

        let pack = AuditRulePack::new(vec![rule]);
        let issues = validate_rule_pack(&pack);
        assert!(
            issues
                .iter()
                .any(|i| i.code == "rulepack.blocking-requires-fixtures")
        );
    }

    #[test]
    fn negative_unknown_schema_fails_closed() {
        let mut pack = AuditRulePack::new(vec![advisory_rule()]);
        pack.schema = "other".to_owned();
        assert!(
            validate_rule_pack(&pack)
                .iter()
                .any(|i| i.code == "rulepack.schema")
        );
    }

    #[test]
    fn receipt_is_advisory_and_preserves_non_claims() {
        let pack = AuditRulePack::new(vec![advisory_rule()]);
        let receipt = advisory_receipt(&pack);
        assert!(receipt.is_advisory());
        assert!(
            receipt
                .non_claims
                .contains(&CANDIDATE_EVIDENCE_NON_CLAIM.to_owned())
        );
        assert!(
            receipt
                .non_claims
                .contains(&"does-not-prove-ucan-authority".to_owned())
        );
    }
}
