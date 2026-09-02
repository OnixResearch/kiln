//! Trellis proof evidence-chain profile with explicit acceptance roles.

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_chain::EvidenceChainDigest;
use crate::evidence_chain::EvidenceChainGraphRow;
use crate::evidence_ir::EvidenceSpan;
use crate::evidence_ir::VerificationRole;

pub const TRELLIS_PROOF_NON_CLAIM: &str = "Trellis proof evidence preserves scoped proof identity and linkage only, not downstream correctness or release eligibility";
const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrellisProofOutcome {
    ReferenceOnly,
    FormalProofCandidate,
    AcceptedFormalProof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrellisProofEvidence {
    pub source: EvidenceChainDigest,
    pub proof_ir: EvidenceChainDigest,
    pub verifier_receipt: EvidenceChainDigest,
    pub assumptions: Vec<EvidenceChainDigest>,
    pub property_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub spans: Vec<EvidenceSpan>,
    pub verifier_status: String,
    pub policy_accepted: bool,
    pub outcome: TrellisProofOutcome,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrellisProofIssue {
    pub field: &'static str,
    pub code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrellisProofReport {
    pub valid: bool,
    pub issues: Vec<TrellisProofIssue>,
    pub outcome: TrellisProofOutcome,
    pub graph_rows: Vec<EvidenceChainGraphRow>,
}

/// r[impl valence.evidence_ir.trellis_proof_evidence_profile.registry]
/// r[impl valence.evidence_ir.trellis_proof_evidence_profile.validation]
/// r[impl valence.evidence_ir.trellis_proof_evidence_profile.roles]
#[must_use]
pub fn validate_trellis_proof_evidence(input: &TrellisProofEvidence) -> TrellisProofReport {
    let mut issues = Vec::new();
    check_digest(&input.source, "source", "source-envelope", &mut issues);
    check_digest(&input.proof_ir, "proof_ir", "proof-ir", &mut issues);
    check_digest(&input.verifier_receipt, "verifier_receipt", "verifier-receipt", &mut issues);
    if input.assumptions.is_empty() {
        issues.push(issue("assumptions", "assumptions_missing"));
    }
    for assumption in &input.assumptions {
        check_digest(assumption, "assumptions", "assumption", &mut issues);
    }
    if input.property_ids.iter().any(|id| id.trim().is_empty()) {
        issues.push(issue("property_ids", "property_id_malformed"));
    }
    if input.requirement_ids.iter().any(|id| !id.starts_with("r[")) {
        issues.push(issue("requirement_ids", "requirement_id_malformed"));
    }
    if input.spans.is_empty()
        || input.spans.iter().any(|span| !crate::evidence_ir::validate_evidence_span(span).is_empty())
    {
        issues.push(issue("spans", "span_invalid"));
    }
    match input.outcome {
        TrellisProofOutcome::ReferenceOnly => {
            if input.verification_role != VerificationRole::RecordedOnly {
                issues.push(issue("verification_role", "role_promotion_denied"));
            }
        }
        TrellisProofOutcome::FormalProofCandidate => {
            if input.verification_role == VerificationRole::Property {
                issues.push(issue("verification_role", "candidate_property_denied"));
            }
        }
        TrellisProofOutcome::AcceptedFormalProof => {
            if input.verifier_status != "passed"
                || !input.policy_accepted
                || input.verification_role != VerificationRole::Property
            {
                issues.push(issue("outcome", "formal_proof_acceptance_incomplete"));
            }
        }
    }
    if !input.non_claims.iter().any(|claim| claim == TRELLIS_PROOF_NON_CLAIM) {
        issues.push(issue("non_claims", "non_claims_weakened"));
    }
    issues.sort_by_key(|item| (item.field, item.code));
    issues.dedup();
    let graph_rows = if issues.is_empty() {
        graph_rows(input)
    } else {
        Vec::new()
    };
    TrellisProofReport {
        valid: issues.is_empty(),
        issues,
        outcome: input.outcome,
        graph_rows,
    }
}

/// r[impl valence.evidence_ir.trellis_proof_evidence_profile.graph]
fn graph_rows(input: &TrellisProofEvidence) -> Vec<EvidenceChainGraphRow> {
    let mut rows = vec![
        row(&input.proof_ir, &input.source, "proved-from"),
        row(&input.verifier_receipt, &input.proof_ir, "verified"),
    ];
    rows.extend(input.assumptions.iter().map(|assumption| row(&input.proof_ir, assumption, "assumes")));
    rows.sort_by(|left, right| {
        (&left.from_domain, &left.from_digest, &left.to_domain, &left.to_digest, &left.edge_kind).cmp(&(
            &right.from_domain,
            &right.from_digest,
            &right.to_domain,
            &right.to_digest,
            &right.edge_kind,
        ))
    });
    rows
}

fn row(from: &EvidenceChainDigest, to: &EvidenceChainDigest, edge_kind: &str) -> EvidenceChainGraphRow {
    EvidenceChainGraphRow {
        from_domain: from.domain.clone(),
        from_digest: from.digest.clone(),
        to_domain: to.domain.clone(),
        to_digest: to.digest.clone(),
        edge_kind: edge_kind.to_owned(),
    }
}

fn check_digest(value: &EvidenceChainDigest, field: &'static str, domain: &str, issues: &mut Vec<TrellisProofIssue>) {
    if value.domain != domain {
        issues.push(issue(field, "digest_domain_substitution"));
    }
    let digest = value.digest.strip_prefix("b3:").unwrap_or(&value.digest);
    if digest.len() != BLAKE3_HEX_LENGTH || !digest.chars().all(|character| character.is_ascii_hexdigit()) {
        issues.push(issue(field, "malformed_digest"));
    }
}

const fn issue(field: &'static str, code: &'static str) -> TrellisProofIssue {
    TrellisProofIssue { field, code }
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
    fn input() -> TrellisProofEvidence {
        TrellisProofEvidence {
            source: digest("source-envelope"),
            proof_ir: digest("proof-ir"),
            verifier_receipt: digest("verifier-receipt"),
            assumptions: vec![digest("assumption")],
            property_ids: vec!["trellis.property.one".to_owned()],
            requirement_ids: vec!["r[valence.test]".to_owned()],
            spans: vec![EvidenceSpan {
                source_id: "source".to_owned(),
                source_path: "src/lib.rs".to_owned(),
                start_byte: 0,
                end_byte: 1,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
                label: None,
            }],
            verifier_status: "passed".to_owned(),
            policy_accepted: true,
            outcome: TrellisProofOutcome::AcceptedFormalProof,
            verification_role: VerificationRole::Property,
            non_claims: vec![TRELLIS_PROOF_NON_CLAIM.to_owned()],
        }
    }
    #[test]
    fn accepted_proof_and_graph_pass() {
        let report = validate_trellis_proof_evidence(&input());
        assert!(report.valid);
        assert!(!report.graph_rows.is_empty());
    }
    #[test]
    fn missing_assumptions_promotion_domain_and_boundary_fail() {
        let mut bad = input();
        bad.assumptions.clear();
        bad.source.domain = "proof-ir".to_owned();
        bad.policy_accepted = false;
        bad.non_claims.clear();
        let report = validate_trellis_proof_evidence(&bad);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|item| item.code == "formal_proof_acceptance_incomplete"));
    }
}
