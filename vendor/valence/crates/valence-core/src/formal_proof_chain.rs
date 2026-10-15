//! Frozen formal-proof stack-chain fixture validation.
//!
//! The validator consumes parsed in-memory fixture values and checks typed
//! linkage, digest syntax, role preservation, readiness-count boundaries, and
//! non-claims. It does not read live sibling repositories, rerun proof tools,
//! decide Cairn release readiness, or prove verifier soundness.
//!
//! r[impl valence.stack_provenance.formal_proof_chain_fixture.contract]
//! r[impl valence.stack_provenance.formal_proof_chain_fixture.validation]

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::BinaryRole;
use crate::shared_identity::Digest;
use crate::shared_identity::DigestRoleMarker;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::ReceiptRole;
use crate::shared_identity::SidecarRole;
use crate::shared_identity::contains_overclaim;

pub mod aeneas;

const FIELD_BINARY_HASH: &str = "binary_hash";
const FIELD_EDGE_KIND: &str = "edge_kind";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_PRODUCER: &str = "producer";
const FIELD_PROOF_CARD: &str = "proof_card";
const FIELD_READINESS: &str = "readiness";
const FIELD_RECEIPT: &str = "receipt";
const FIELD_ROLE: &str = "verification_role";
const FIELD_SIDECAR_DIGEST: &str = "sidecar_digest";
const FIELD_STATUS: &str = "status";

pub const FORMAL_PROOF_CHAIN_REQUIRED_NON_CLAIM: &str =
    "formal-proof stack-chain fixture proves stack identity and role-preserving linkage only";
const FORMAL_PROOF_CHAIN_SUPPORTED_CLAIM: &str = "formal-proof stack-chain fixture proves stack identity and role-preserving linkage only; it does not prove whole-program correctness, verifier soundness, semantic equivalence, release eligibility, live sibling freshness, or requirement satisfaction outside the declared fixture scope";

const OCTET_PRODUCER: &str = "octet";
const PASS_STATUS: &str = "pass";
const PROPERTY_ROLE: &str = "property";
const RECORDED_ONLY_ROLE: &str = "recorded_only";
const BOUNDARY_ROLE: &str = "boundary";

const SUPPORTED_EDGE_KINDS: &[&str] = &[
    "formal-proof-receipt",
    "proof-card-link",
    "provenance-export",
    "sidecar-inclusion",
    "cairn-readiness-input",
    "manual-review-input",
];

/// Formal proof receipt identity from an Octet export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalProofReceiptNode {
    pub receipt_id: String,
    pub receipt_hash: String,
    pub verification_role: String,
    pub status: String,
    pub producer: String,
    pub non_claims: Vec<String>,
}

/// Proof-card fixture node linked to the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofCardNode {
    pub proof_card_id: String,
    pub linked_receipt_id: String,
    pub proof_card_hash: String,
    pub evidence_tier: String,
    pub non_claims: Vec<String>,
}

/// Provenance record carrying binary and sidecar identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalProofProvenanceNode {
    pub provenance_id: String,
    pub linked_receipt_id: String,
    pub binary_hash: String,
    pub sidecar_digest: String,
    pub non_claims: Vec<String>,
}

/// Graph node in the frozen stack chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalProofGraphNode {
    pub node_id: String,
    pub linked_receipt_id: String,
    pub verification_role: String,
    pub producer: String,
    pub edge_kind: String,
    pub non_claims: Vec<String>,
}

/// Cairn readiness input summary emitted by Valence for Cairn-owned policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnReadinessInputSummary {
    pub property_count: u32,
    pub recorded_only_count: u32,
    pub boundary_count: u32,
    pub verified_count: u32,
    pub non_claims: Vec<String>,
}

/// Complete frozen formal-proof stack-chain fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalProofStackChainFixture {
    pub receipt: FormalProofReceiptNode,
    pub proof_card: ProofCardNode,
    pub provenance: FormalProofProvenanceNode,
    pub graph_nodes: Vec<FormalProofGraphNode>,
    pub readiness: CairnReadinessInputSummary,
    pub non_claims: Vec<String>,
}

/// Field-scoped deterministic fixture issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FormalProofChainIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl FormalProofChainIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Fixture validation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormalProofChainReport {
    pub valid: bool,
    pub property_count: u32,
    pub recorded_only_count: u32,
    pub boundary_count: u32,
    pub issues: Vec<FormalProofChainIssue>,
    pub supported_claim: String,
}

/// Validate the formal-proof fixture through pure in-memory checks.
#[must_use]
pub fn validate_formal_proof_stack_chain(fixture: &FormalProofStackChainFixture) -> FormalProofChainReport {
    let mut issues = Vec::new();
    validate_non_claims(&fixture.non_claims, &mut issues);
    validate_receipt(&fixture.receipt, &mut issues);
    validate_proof_card(&fixture.receipt, &fixture.proof_card, &mut issues);
    validate_provenance(&fixture.receipt, &fixture.provenance, &mut issues);

    let mut property_count = u32::default();
    let mut recorded_only_count = u32::default();
    let mut boundary_count = u32::default();
    for node in &fixture.graph_nodes {
        validate_graph_node(&fixture.receipt, node, &mut issues);
        match node.verification_role.as_str() {
            PROPERTY_ROLE => property_count = property_count.saturating_add(1),
            RECORDED_ONLY_ROLE => recorded_only_count = recorded_only_count.saturating_add(1),
            BOUNDARY_ROLE => boundary_count = boundary_count.saturating_add(1),
            _ => issues.push(FormalProofChainIssue::new(FIELD_ROLE, "unknown_verification_role")),
        }
    }
    validate_readiness(&fixture.readiness, property_count, recorded_only_count, boundary_count, &mut issues);
    finish_report(issues, property_count, recorded_only_count, boundary_count)
}

fn validate_receipt(receipt: &FormalProofReceiptNode, issues: &mut Vec<FormalProofChainIssue>) {
    validate_non_claims(&receipt.non_claims, issues);
    validate_digest::<ReceiptRole>(&receipt.receipt_hash, FIELD_RECEIPT, issues);
    if receipt.status != PASS_STATUS {
        issues.push(FormalProofChainIssue::new(FIELD_STATUS, "receipt_not_passed"));
    }
    if receipt.verification_role != PROPERTY_ROLE {
        issues.push(FormalProofChainIssue::new(FIELD_ROLE, "receipt_not_property"));
    }
    if receipt.producer != OCTET_PRODUCER {
        issues.push(FormalProofChainIssue::new(FIELD_PRODUCER, "property_receipt_not_octet_owned"));
    }
}

fn validate_proof_card(
    receipt: &FormalProofReceiptNode,
    proof_card: &ProofCardNode,
    issues: &mut Vec<FormalProofChainIssue>,
) {
    validate_non_claims(&proof_card.non_claims, issues);
    validate_digest::<ReceiptRole>(&proof_card.proof_card_hash, FIELD_PROOF_CARD, issues);
    if proof_card.linked_receipt_id != receipt.receipt_id {
        issues.push(FormalProofChainIssue::new(FIELD_PROOF_CARD, "stale_receipt_link"));
    }
}

fn validate_provenance(
    receipt: &FormalProofReceiptNode,
    provenance: &FormalProofProvenanceNode,
    issues: &mut Vec<FormalProofChainIssue>,
) {
    validate_non_claims(&provenance.non_claims, issues);
    validate_digest::<BinaryRole>(&provenance.binary_hash, FIELD_BINARY_HASH, issues);
    validate_digest::<SidecarRole>(&provenance.sidecar_digest, FIELD_SIDECAR_DIGEST, issues);
    if provenance.linked_receipt_id != receipt.receipt_id {
        issues.push(FormalProofChainIssue::new(FIELD_RECEIPT, "provenance_receipt_link_mismatch"));
    }
}

fn validate_graph_node(
    receipt: &FormalProofReceiptNode,
    node: &FormalProofGraphNode,
    issues: &mut Vec<FormalProofChainIssue>,
) {
    validate_non_claims(&node.non_claims, issues);
    if !SUPPORTED_EDGE_KINDS.contains(&node.edge_kind.as_str()) {
        issues.push(FormalProofChainIssue::new(FIELD_EDGE_KIND, "unsupported_edge_kind"));
    }
    if node.verification_role == PROPERTY_ROLE
        && (node.producer != OCTET_PRODUCER || node.linked_receipt_id != receipt.receipt_id)
    {
        issues.push(FormalProofChainIssue::new(FIELD_ROLE, "tier_promotion"));
    }
}

fn validate_readiness(
    summary: &CairnReadinessInputSummary,
    property_count: u32,
    recorded_only_count: u32,
    boundary_count: u32,
    issues: &mut Vec<FormalProofChainIssue>,
) {
    validate_non_claims(&summary.non_claims, issues);
    if summary.property_count != property_count {
        issues.push(FormalProofChainIssue::new(FIELD_READINESS, "property_count_mismatch"));
    }
    if summary.recorded_only_count != recorded_only_count {
        issues.push(FormalProofChainIssue::new(FIELD_READINESS, "recorded_only_count_mismatch"));
    }
    if summary.boundary_count != boundary_count {
        issues.push(FormalProofChainIssue::new(FIELD_READINESS, "boundary_count_mismatch"));
    }
    if summary.verified_count != property_count {
        issues.push(FormalProofChainIssue::new(FIELD_READINESS, "verification_role_count"));
    }
}

fn validate_digest<Role: DigestRoleMarker>(digest: &str, field: &'static str, issues: &mut Vec<FormalProofChainIssue>) {
    if Digest::<Role>::parse(digest, MIXED_BLAKE3_PROFILE, field).is_err() {
        issues.push(FormalProofChainIssue::new(field, "malformed_blake3"));
    }
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<FormalProofChainIssue>) {
    if non_claims.is_empty() {
        issues.push(FormalProofChainIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return;
    }
    if !non_claims.iter().any(|claim| claim.contains(FORMAL_PROOF_CHAIN_REQUIRED_NON_CLAIM)) {
        issues.push(FormalProofChainIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(FormalProofChainIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
}

fn finish_report(
    mut issues: Vec<FormalProofChainIssue>,
    property_count: u32,
    recorded_only_count: u32,
    boundary_count: u32,
) -> FormalProofChainReport {
    issues.sort();
    issues.dedup();
    FormalProofChainReport {
        valid: issues.is_empty(),
        property_count,
        recorded_only_count,
        boundary_count,
        issues,
        supported_claim: FORMAL_PROOF_CHAIN_SUPPORTED_CLAIM.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const RECEIPT_ID: &str = "octet.receipt.formal.transition";
    const EXPECTED_PROPERTY_COUNT: u32 = 1;
    const EXPECTED_RECORDED_ONLY_COUNT: u32 = 1;
    const EXPECTED_BOUNDARY_COUNT: u32 = 1;

    fn non_claims() -> Vec<String> {
        vec![FORMAL_PROOF_CHAIN_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn fixture() -> FormalProofStackChainFixture {
        FormalProofStackChainFixture {
            receipt: FormalProofReceiptNode {
                receipt_id: RECEIPT_ID.to_owned(),
                receipt_hash: DIGEST.to_owned(),
                verification_role: PROPERTY_ROLE.to_owned(),
                status: PASS_STATUS.to_owned(),
                producer: OCTET_PRODUCER.to_owned(),
                non_claims: non_claims(),
            },
            proof_card: ProofCardNode {
                proof_card_id: "proof-card.transition".to_owned(),
                linked_receipt_id: RECEIPT_ID.to_owned(),
                proof_card_hash: DIGEST.to_owned(),
                evidence_tier: "formal_proof".to_owned(),
                non_claims: non_claims(),
            },
            provenance: FormalProofProvenanceNode {
                provenance_id: "provenance.transition".to_owned(),
                linked_receipt_id: RECEIPT_ID.to_owned(),
                binary_hash: DIGEST.to_owned(),
                sidecar_digest: DIGEST.to_owned(),
                non_claims: non_claims(),
            },
            graph_nodes: vec![
                FormalProofGraphNode {
                    node_id: "property-node".to_owned(),
                    linked_receipt_id: RECEIPT_ID.to_owned(),
                    verification_role: PROPERTY_ROLE.to_owned(),
                    producer: OCTET_PRODUCER.to_owned(),
                    edge_kind: "formal-proof-receipt".to_owned(),
                    non_claims: non_claims(),
                },
                FormalProofGraphNode {
                    node_id: "manual-review-node".to_owned(),
                    linked_receipt_id: "trellis.manual".to_owned(),
                    verification_role: RECORDED_ONLY_ROLE.to_owned(),
                    producer: "trellis".to_owned(),
                    edge_kind: "manual-review-input".to_owned(),
                    non_claims: non_claims(),
                },
                FormalProofGraphNode {
                    node_id: "boundary-node".to_owned(),
                    linked_receipt_id: "cairn.gate".to_owned(),
                    verification_role: BOUNDARY_ROLE.to_owned(),
                    producer: "cairn".to_owned(),
                    edge_kind: "cairn-readiness-input".to_owned(),
                    non_claims: non_claims(),
                },
            ],
            readiness: CairnReadinessInputSummary {
                property_count: EXPECTED_PROPERTY_COUNT,
                recorded_only_count: EXPECTED_RECORDED_ONLY_COUNT,
                boundary_count: EXPECTED_BOUNDARY_COUNT,
                verified_count: EXPECTED_PROPERTY_COUNT,
                non_claims: non_claims(),
            },
            non_claims: non_claims(),
        }
    }

    #[test]
    fn positive_formal_proof_stack_chain_fixture_passes() {
        // r[verify valence.stack_provenance.formal_proof_chain_fixture.positive]
        let report = validate_formal_proof_stack_chain(&fixture());

        assert!(report.valid);
        assert_eq!(report.property_count, EXPECTED_PROPERTY_COUNT);
        assert!(report.supported_claim.contains("role-preserving linkage only"));
    }

    #[test]
    fn negative_formal_proof_stack_chain_fixture_fails_closed() {
        // r[verify valence.stack_provenance.formal_proof_chain_fixture.negative]
        let mut fixture = fixture();
        fixture.proof_card.linked_receipt_id = "stale".to_owned();
        fixture.graph_nodes[0].producer = "trellis".to_owned();
        fixture.graph_nodes[0].edge_kind = "unsupported".to_owned();
        fixture.readiness.verified_count =
            fixture.readiness.property_count.saturating_add(fixture.readiness.recorded_only_count);
        fixture.non_claims = vec!["proves whole-program safety".to_owned()];

        let report = validate_formal_proof_stack_chain(&fixture);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stale_receipt_link"));
        assert!(report.issues.iter().any(|issue| issue.code == "tier_promotion"));
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_edge_kind"));
        assert!(report.issues.iter().any(|issue| issue.code == "verification_role_count"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
    }
}
