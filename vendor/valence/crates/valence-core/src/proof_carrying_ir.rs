//! Opaque proof-carrying IR lineage profile.

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_chain::EvidenceChainDigest;
use crate::evidence_ir::VerificationRole;

pub const PROOF_CARRYING_IR_NON_CLAIM: &str = "proof-carrying IR lineage proves identity and linkage only, not IR semantics, proof discharge, compiler correctness, or semantic equivalence";
const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofCarryingIrLineageRow {
    pub source_ref: EvidenceChainDigest,
    pub ir_ref: EvidenceChainDigest,
    pub node_id: String,
    pub proof_obligation_id: String,
    pub transform_step_id: String,
    pub dialect_id: String,
    pub dialect_version: String,
    pub opaque_payload_hash: String,
    pub proof_sidecar_ref: EvidenceChainDigest,
    pub expected_sidecar_digest: String,
    pub verification_role: VerificationRole,
    pub semantic_claims: Vec<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofCarryingIrIssue {
    pub field: &'static str,
    pub code: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProofCarryingIrReport {
    pub valid: bool,
    pub issues: Vec<ProofCarryingIrIssue>,
    pub accepted_rows: usize,
}

/// r[impl valence.evidence_ir.proof_carrying_ir_lineage.profile]
/// r[impl valence.evidence_ir.proof_carrying_ir_lineage.validation]
/// r[impl valence.evidence_ir.proof_carrying_ir_lineage.opaque_dialects]
#[must_use]
pub fn validate_proof_carrying_ir_lineage(rows: &[ProofCarryingIrLineageRow]) -> ProofCarryingIrReport {
    let mut issues = Vec::new();
    if rows.is_empty() {
        issues.push(issue("rows", "rows_missing"));
    }
    for row in rows {
        check_digest(&row.source_ref, "source_ref", "source-artifact", &mut issues);
        check_digest(&row.ir_ref, "ir_ref", "ir-artifact", &mut issues);
        check_digest(&row.proof_sidecar_ref, "proof_sidecar_ref", "proof-sidecar", &mut issues);
        for (field, value) in [
            ("node_id", &row.node_id),
            ("proof_obligation_id", &row.proof_obligation_id),
            ("transform_step_id", &row.transform_step_id),
            ("dialect_id", &row.dialect_id),
            ("dialect_version", &row.dialect_version),
        ] {
            if value.trim().is_empty() {
                issues.push(issue(field, "identifier_missing"));
            }
        }
        if normalize(&row.proof_sidecar_ref.digest) != normalize(&row.expected_sidecar_digest) {
            issues.push(issue("proof_sidecar_ref", "stale_sidecar"));
        }
        if !valid_hex(&row.opaque_payload_hash) {
            issues.push(issue("opaque_payload_hash", "malformed_digest"));
        }
        if row.verification_role != VerificationRole::RecordedOnly {
            issues.push(issue("verification_role", "role_promotion_denied"));
        }
        if !row.semantic_claims.is_empty() {
            issues.push(issue("semantic_claims", "semantic_equivalence_overclaim"));
        }
        if !row.non_claims.iter().any(|claim| claim == PROOF_CARRYING_IR_NON_CLAIM) {
            issues.push(issue("non_claims", "non_claims_weakened"));
        }
    }
    issues.sort_by_key(|item| (item.field, item.code));
    issues.dedup();
    ProofCarryingIrReport {
        valid: issues.is_empty(),
        accepted_rows: if issues.is_empty() { rows.len() } else { 0 },
        issues,
    }
}
fn check_digest(
    value: &EvidenceChainDigest,
    field: &'static str,
    domain: &str,
    issues: &mut Vec<ProofCarryingIrIssue>,
) {
    if value.domain != domain {
        issues.push(issue(field, "cross_artifact_substitution"));
    }
    if !valid_hex(&value.digest) {
        issues.push(issue(field, "malformed_digest"));
    }
}
fn normalize(value: &str) -> &str {
    value.strip_prefix("b3:").unwrap_or(value)
}
fn valid_hex(value: &str) -> bool {
    let value = normalize(value);
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}
const fn issue(field: &'static str, code: &'static str) -> ProofCarryingIrIssue {
    ProofCarryingIrIssue { field, code }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn digest(domain: &str) -> EvidenceChainDigest {
        EvidenceChainDigest {
            domain: domain.to_owned(),
            digest: HEX.to_owned(),
        }
    }
    fn row() -> ProofCarryingIrLineageRow {
        ProofCarryingIrLineageRow {
            source_ref: digest("source-artifact"),
            ir_ref: digest("ir-artifact"),
            node_id: "node-1".to_owned(),
            proof_obligation_id: "po-1".to_owned(),
            transform_step_id: "step-1".to_owned(),
            dialect_id: "unknown.experimental".to_owned(),
            dialect_version: "v1".to_owned(),
            opaque_payload_hash: HEX.to_owned(),
            proof_sidecar_ref: digest("proof-sidecar"),
            expected_sidecar_digest: HEX.to_owned(),
            verification_role: VerificationRole::RecordedOnly,
            semantic_claims: Vec::new(),
            non_claims: vec![PROOF_CARRYING_IR_NON_CLAIM.to_owned()],
        }
    }
    #[test]
    fn valid_unknown_dialect_is_preserved_recorded_only() {
        assert!(validate_proof_carrying_ir_lineage(&[row()]).valid);
    }
    #[test]
    fn stale_sidecar_role_cross_artifact_and_overclaim_fail() {
        let mut bad = row();
        bad.source_ref.domain = "ir-artifact".to_owned();
        bad.expected_sidecar_digest = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        bad.verification_role = VerificationRole::Property;
        bad.semantic_claims.push("semantic equivalence".to_owned());
        let report = validate_proof_carrying_ir_lineage(&[bad]);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|item| item.code == "stale_sidecar"));
    }
}
