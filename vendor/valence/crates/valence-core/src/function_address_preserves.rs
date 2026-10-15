//! Direct validation of already-loaded Kamacite Preserves function-address receipts.

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::VerificationRole;
use crate::function_address_profile::FunctionAddressDigest;
use crate::function_address_profile::FunctionAddressDigestDomain;
use crate::function_address_profile::FunctionAddressEvidenceInput;
use crate::function_address_profile::FunctionAddressGraphRow;
use crate::function_address_profile::function_address_graph_rows;
use crate::function_address_profile::validate_function_address_evidence;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const FUNCTION_ADDRESS_PRESERVES_SCHEMA: &str = "kamacite.function-address-preserves.v1";
pub const FUNCTION_ADDRESS_PRESERVES_NON_CLAIM: &str = "canonical Preserves function-address identity is recorded-only linkage evidence, not Rust semantics, correctness, proof, or release eligibility";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionAddressPreservesReceipt {
    pub schema: String,
    pub canonical_bytes: Vec<u8>,
    pub receipt_identity: FunctionAddressDigest,
    pub projection_identity: Option<FunctionAddressDigest>,
    pub function_address: FunctionAddressEvidenceInput,
    pub dependency_identity_kind: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionAddressPreservesIssue {
    pub field: &'static str,
    pub code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressPreservesReport {
    pub valid: bool,
    pub issues: Vec<FunctionAddressPreservesIssue>,
    pub receipt_hash: Option<String>,
    pub graph_rows: Vec<FunctionAddressGraphRow>,
}

/// Validate direct canonical Preserves receipt identity and its compatibility projection.
///
/// r[impl valence.evidence_ir.function_address_preserves_ingest.core]
/// r[impl valence.evidence_ir.function_address_preserves_ingest.json_projection]
#[must_use]
pub fn validate_function_address_preserves(
    receipt: &FunctionAddressPreservesReceipt,
) -> FunctionAddressPreservesReport {
    let mut issues = Vec::new();
    if receipt.schema != FUNCTION_ADDRESS_PRESERVES_SCHEMA {
        issues.push(issue("schema", "schema_mismatch"));
    }
    if receipt.receipt_identity.domain != FunctionAddressDigestDomain::KamaciteReceipt {
        issues.push(issue("receipt_identity", "digest_domain_substitution"));
    }
    let receipt_hash = blake3_hex(&receipt.canonical_bytes);
    if normalize(&receipt.receipt_identity.digest) != receipt_hash {
        issues.push(issue("receipt_identity", "canonical_hash_mismatch"));
    }
    if receipt.dependency_identity_kind != "dependency-closure" {
        issues.push(issue("dependency_identity_kind", "dependency_identity_kind_mismatch"));
    }
    if receipt.function_address.verification_role != VerificationRole::RecordedOnly {
        issues.push(issue("verification_role", "role_promotion_denied"));
    }
    if !receipt.non_claims.iter().any(|claim| claim == FUNCTION_ADDRESS_PRESERVES_NON_CLAIM) {
        issues.push(issue("non_claims", "non_claims_weakened"));
    }
    let profile = validate_function_address_evidence(&receipt.function_address);
    if !profile.valid {
        issues.push(issue("function_address", "profile_validation_failed"));
    }
    if let Some(projection_identity) = &receipt.projection_identity {
        if projection_identity.domain != FunctionAddressDigestDomain::ValenceProjection {
            issues.push(issue("projection_identity", "digest_domain_substitution"));
        }
        let actual = serde_json::to_value(&receipt.function_address)
            .ok()
            .and_then(|value| canonical_json_bytes(&value).ok())
            .map(|bytes| blake3_hex(&bytes));
        if actual.as_deref() != Some(normalize(&projection_identity.digest).as_str()) {
            issues.push(issue("projection_identity", "projection_drift"));
        }
    }
    issues.sort_by_key(|item| (item.field, item.code));
    issues.dedup();
    FunctionAddressPreservesReport {
        valid: issues.is_empty(),
        receipt_hash: issues.is_empty().then_some(receipt_hash),
        graph_rows: if issues.is_empty() {
            function_address_graph_rows(&receipt.function_address)
        } else {
            Vec::new()
        },
        issues,
    }
}

fn issue(field: &'static str, code: &'static str) -> FunctionAddressPreservesIssue {
    FunctionAddressPreservesIssue { field, code }
}

fn normalize(value: &str) -> String {
    value.strip_prefix("b3:").unwrap_or(value).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_chain::tests::function_input;

    fn receipt() -> FunctionAddressPreservesReceipt {
        let canonical_bytes = b"(function-address-receipt fixture)".to_vec();
        FunctionAddressPreservesReceipt {
            schema: FUNCTION_ADDRESS_PRESERVES_SCHEMA.to_owned(),
            receipt_identity: FunctionAddressDigest {
                domain: FunctionAddressDigestDomain::KamaciteReceipt,
                digest: blake3_hex(&canonical_bytes),
            },
            canonical_bytes,
            projection_identity: None,
            function_address: function_input(),
            dependency_identity_kind: "dependency-closure".to_owned(),
            non_claims: vec![FUNCTION_ADDRESS_PRESERVES_NON_CLAIM.to_owned()],
        }
    }

    #[test]
    fn valid_preserves_receipt_emits_recorded_graph_rows() {
        // r[verify valence.evidence_ir.function_address_preserves_ingest.positive]
        let report = validate_function_address_preserves(&receipt());
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert!(report.receipt_hash.is_some());
    }

    #[test]
    fn stale_domain_projection_role_and_boundary_fail() {
        // r[verify valence.evidence_ir.function_address_preserves_ingest.negative]
        let mut bad = receipt();
        bad.receipt_identity.domain = FunctionAddressDigestDomain::SourceEnvelope;
        bad.function_address.verification_role = VerificationRole::Property;
        bad.projection_identity = Some(FunctionAddressDigest {
            domain: FunctionAddressDigestDomain::ValenceProjection,
            digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        });
        bad.non_claims.clear();
        let report = validate_function_address_preserves(&bad);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|item| item.code == "projection_drift"));
        assert!(report.issues.iter().any(|item| item.code == "role_promotion_denied"));
    }
}
