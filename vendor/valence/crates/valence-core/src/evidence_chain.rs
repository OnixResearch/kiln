//! Generic Kamacite-to-Valence evidence-chain validation.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::evidence_ir::VerificationRole;
use crate::function_address_profile::FunctionAddressEvidenceInput;
use crate::function_address_profile::validate_function_address_evidence;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const EVIDENCE_CHAIN_PROFILE_VERSION: &str = "valence.evidence-chain.v1";
pub const FUNCTION_ADDRESS_CHAIN_KIND: &str = "function-address";
pub const TRELLIS_PROOF_CHAIN_KIND: &str = "trellis-proof";
pub const EVIDENCE_CHAIN_NON_CLAIM: &str =
    "evidence-chain validation proves identity, linkage, role, projection, and non-claim conformance only";
const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceChainDigest {
    pub domain: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceChainLink {
    pub from: EvidenceChainDigest,
    pub to: EvidenceChainDigest,
    pub edge_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChainEnvelope {
    pub profile_version: String,
    pub evidence_kind: String,
    pub canonical_bytes: Vec<u8>,
    pub envelope_identity: EvidenceChainDigest,
    pub profile_root: EvidenceChainDigest,
    pub policy_identity: EvidenceChainDigest,
    pub parent_links: Vec<EvidenceChainLink>,
    pub verification_role: VerificationRole,
    pub compatibility_projection: Option<Value>,
    pub projection_identity: Option<EvidenceChainDigest>,
    pub payload: Value,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceChainIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl EvidenceChainIssue {
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceChainGraphRow {
    pub from_domain: String,
    pub from_digest: String,
    pub to_domain: String,
    pub to_digest: String,
    pub edge_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceChainReport {
    pub valid: bool,
    pub issues: Vec<EvidenceChainIssue>,
    pub graph_rows: Vec<EvidenceChainGraphRow>,
    pub graph_root: Option<String>,
    pub supported_claim: &'static str,
}

/// Validate generic envelope invariants before dispatching the registered profile.
///
/// r[impl valence.evidence_ir.evidence_chain_profile.core]
/// r[impl valence.evidence_ir.evidence_chain_profile.validation_rules]
/// r[impl valence.evidence_ir.evidence_chain_profile.profile_registry]
#[must_use]
pub fn validate_evidence_chain(envelope: &EvidenceChainEnvelope) -> EvidenceChainReport {
    let mut issues = Vec::new();
    if envelope.profile_version != EVIDENCE_CHAIN_PROFILE_VERSION {
        issues.push(EvidenceChainIssue::new("profile_version", "unsupported_profile_version"));
    }
    validate_digest(&envelope.envelope_identity, "envelope_identity", "kamacite-envelope", &mut issues);
    validate_digest(&envelope.profile_root, "profile_root", "profile-root", &mut issues);
    validate_digest(&envelope.policy_identity, "policy_identity", "policy-profile", &mut issues);
    if blake3_hex(&envelope.canonical_bytes) != normalized_digest(&envelope.envelope_identity.digest) {
        issues.push(EvidenceChainIssue::new("envelope_identity", "canonical_envelope_hash_mismatch"));
    }
    if !envelope.non_claims.iter().any(|claim| claim == EVIDENCE_CHAIN_NON_CLAIM) {
        issues.push(EvidenceChainIssue::new("non_claims", "non_claims_weakened"));
    }
    validate_projection(envelope, &mut issues);
    validate_links(envelope, &mut issues);
    validate_registered_profile(envelope, &mut issues);
    issues.sort_by_key(|issue| (issue.field, issue.code));
    issues.dedup();
    if !issues.is_empty() {
        return EvidenceChainReport {
            valid: false,
            issues,
            graph_rows: Vec::new(),
            graph_root: None,
            supported_claim: EVIDENCE_CHAIN_NON_CLAIM,
        };
    }
    let graph_rows = evidence_chain_graph_rows(envelope);
    let graph_root = serde_json::to_value(&graph_rows)
        .ok()
        .and_then(|value| canonical_json_bytes(&value).ok())
        .map(|bytes| blake3_hex(&bytes));
    EvidenceChainReport {
        valid: graph_root.is_some(),
        issues: if graph_root.is_some() {
            Vec::new()
        } else {
            vec![EvidenceChainIssue::new("graph_root", "canonical_serialization_failed")]
        },
        graph_rows,
        graph_root,
        supported_claim: EVIDENCE_CHAIN_NON_CLAIM,
    }
}

/// Emit generic typed graph rows only after envelope validation succeeds.
///
/// r[impl valence.evidence_ir.evidence_chain_profile.graph]
#[must_use]
pub fn evidence_chain_graph_rows(envelope: &EvidenceChainEnvelope) -> Vec<EvidenceChainGraphRow> {
    let mut rows = envelope
        .parent_links
        .iter()
        .map(|link| EvidenceChainGraphRow {
            from_domain: link.from.domain.clone(),
            from_digest: normalized_digest(&link.from.digest),
            to_domain: link.to.domain.clone(),
            to_digest: normalized_digest(&link.to.digest),
            edge_kind: link.edge_kind.clone(),
        })
        .collect::<Vec<_>>();
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

fn validate_registered_profile(envelope: &EvidenceChainEnvelope, issues: &mut Vec<EvidenceChainIssue>) {
    match envelope.evidence_kind.as_str() {
        FUNCTION_ADDRESS_CHAIN_KIND => {
            if envelope.verification_role != VerificationRole::RecordedOnly {
                issues.push(EvidenceChainIssue::new("verification_role", "role_promotion_denied"));
            }
            match serde_json::from_value::<FunctionAddressEvidenceInput>(envelope.payload.clone()) {
                Ok(input) => {
                    let report = validate_function_address_evidence(&input);
                    if !report.valid {
                        issues.push(EvidenceChainIssue::new("payload", "profile_validation_failed"));
                    }
                }
                Err(_) => issues.push(EvidenceChainIssue::new("payload", "profile_payload_malformed")),
            }
        }
        TRELLIS_PROOF_CHAIN_KIND => {
            match serde_json::from_value::<crate::trellis_proof_evidence::TrellisProofEvidence>(
                envelope.payload.clone(),
            ) {
                Ok(input) => {
                    let report = crate::trellis_proof_evidence::validate_trellis_proof_evidence(&input);
                    if !report.valid {
                        issues.push(EvidenceChainIssue::new("payload", "trellis_profile_validation_failed"));
                    }
                }
                Err(_) => issues.push(EvidenceChainIssue::new("payload", "profile_payload_malformed")),
            }
        }
        _ => issues.push(EvidenceChainIssue::new("evidence_kind", "unsupported_profile")),
    }
}

fn validate_projection(envelope: &EvidenceChainEnvelope, issues: &mut Vec<EvidenceChainIssue>) {
    match (&envelope.compatibility_projection, &envelope.projection_identity) {
        (None, None) => {}
        (Some(projection), Some(identity)) => {
            validate_digest(identity, "projection_identity", "compatibility-projection", issues);
            let actual = canonical_json_bytes(projection).ok().map(|bytes| blake3_hex(&bytes));
            if actual.as_deref() != Some(normalized_digest(&identity.digest).as_str()) {
                issues.push(EvidenceChainIssue::new("projection_identity", "projection_drift"));
            }
        }
        _ => issues.push(EvidenceChainIssue::new("compatibility_projection", "projection_binding_missing")),
    }
}

fn validate_links(envelope: &EvidenceChainEnvelope, issues: &mut Vec<EvidenceChainIssue>) {
    let mut links = BTreeSet::new();
    for link in &envelope.parent_links {
        validate_digest(&link.from, "parent_links.from", &link.from.domain, issues);
        validate_digest(&link.to, "parent_links.to", &link.to.domain, issues);
        if link.from.domain == link.to.domain && link.from.digest == link.to.digest {
            issues.push(EvidenceChainIssue::new("parent_links", "self_link_denied"));
        }
        if link.edge_kind.trim().is_empty() {
            issues.push(EvidenceChainIssue::new("parent_links.edge_kind", "edge_kind_empty"));
        }
        if !links.insert((&link.from.domain, &link.from.digest, &link.to.domain, &link.to.digest, &link.edge_kind)) {
            issues.push(EvidenceChainIssue::new("parent_links", "duplicate_link"));
        }
    }
}

fn validate_digest(
    digest: &EvidenceChainDigest,
    field: &'static str,
    expected_domain: &str,
    issues: &mut Vec<EvidenceChainIssue>,
) {
    if digest.domain != expected_domain {
        issues.push(EvidenceChainIssue::new(field, "digest_domain_substitution"));
    }
    let normalized = normalized_digest(&digest.digest);
    if normalized.len() != BLAKE3_HEX_LENGTH || !normalized.chars().all(|character| character.is_ascii_hexdigit()) {
        issues.push(EvidenceChainIssue::new(field, "malformed_blake3"));
    }
}

fn normalized_digest(digest: &str) -> String {
    digest.strip_prefix("b3:").unwrap_or(digest).to_ascii_lowercase()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::function_address_profile::FUNCTION_ADDRESS_PROFILE_VERSION;
    use crate::function_address_profile::FunctionAddressDigest;
    use crate::function_address_profile::FunctionAddressDigestDomain;
    use crate::function_address_profile::FunctionAddressEvidenceEdge;
    use crate::function_address_profile::FunctionAddressEvidenceRecord;
    use crate::function_address_profile::FunctionAddressEvidenceRole;
    use crate::function_address_profile::function_address_policy_identity;

    const HEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_HEX: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    pub(crate) fn function_input() -> FunctionAddressEvidenceInput {
        let source = FunctionAddressDigest {
            domain: FunctionAddressDigestDomain::SourceEnvelope,
            digest: HEX.to_owned(),
        };
        FunctionAddressEvidenceInput {
            profile_version: FUNCTION_ADDRESS_PROFILE_VERSION.to_owned(),
            stable_id: "fn:one".to_owned(),
            verification_role: VerificationRole::RecordedOnly,
            records: vec![FunctionAddressEvidenceRecord {
                locator: "crate::one".to_owned(),
                role: FunctionAddressEvidenceRole::OctetProducer,
                function_address: FunctionAddressDigest {
                    domain: FunctionAddressDigestDomain::FunctionAddress,
                    digest: OTHER_HEX.to_owned(),
                },
                source_identity: source.clone(),
                dependency_closure: None,
                source_span: Some(crate::evidence_ir::EvidenceSpan {
                    source_id: HEX.to_owned(),
                    source_path: "src/lib.rs".to_owned(),
                    start_byte: 0,
                    end_byte: 1,
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 2,
                    label: None,
                }),
                caveats: Vec::new(),
            }],
            dependency_edges: Vec::<FunctionAddressEvidenceEdge>::new(),
            declared_roles: vec!["octet-rust-source-shape-producer".to_owned()],
            octet_producer_receipt_identity: None,
            kamacite_receipt_identity: None,
            supplied_kamacite_receipt_identity: None,
            valence_projection_identity: None,
            mantle_sidecar_digest: None,
            release_binary_identity: None,
            policy_identity: Some(function_address_policy_identity()),
            semantic_claims: Vec::new(),
            non_claims: vec![
                "function-address evidence records source-shape identity only".to_owned(),
                "not Rust semantic correctness".to_owned(),
                "not release eligibility".to_owned(),
            ],
        }
    }

    fn envelope() -> EvidenceChainEnvelope {
        let canonical_bytes = b"canonical-kamacite-envelope".to_vec();
        EvidenceChainEnvelope {
            profile_version: EVIDENCE_CHAIN_PROFILE_VERSION.to_owned(),
            evidence_kind: FUNCTION_ADDRESS_CHAIN_KIND.to_owned(),
            envelope_identity: EvidenceChainDigest {
                domain: "kamacite-envelope".to_owned(),
                digest: blake3_hex(&canonical_bytes),
            },
            canonical_bytes,
            profile_root: EvidenceChainDigest {
                domain: "profile-root".to_owned(),
                digest: HEX.to_owned(),
            },
            policy_identity: EvidenceChainDigest {
                domain: "policy-profile".to_owned(),
                digest: OTHER_HEX.to_owned(),
            },
            parent_links: vec![EvidenceChainLink {
                from: EvidenceChainDigest {
                    domain: "source-envelope".to_owned(),
                    digest: HEX.to_owned(),
                },
                to: EvidenceChainDigest {
                    domain: "function-address".to_owned(),
                    digest: OTHER_HEX.to_owned(),
                },
                edge_kind: "produced".to_owned(),
            }],
            verification_role: VerificationRole::RecordedOnly,
            compatibility_projection: None,
            projection_identity: None,
            payload: serde_json::to_value(function_input()).expect("payload"),
            non_claims: vec![EVIDENCE_CHAIN_NON_CLAIM.to_owned()],
        }
    }

    #[test]
    fn function_address_chain_validates_and_emits_graph_root() {
        // r[verify valence.evidence_ir.evidence_chain_profile.positive]
        let report = validate_evidence_chain(&envelope());
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert!(!report.graph_rows.is_empty());
        assert!(report.graph_root.is_some());
    }

    #[test]
    fn generic_chain_rejects_identity_projection_role_and_profile_failures() {
        // r[verify valence.evidence_ir.evidence_chain_profile.negative]
        let mut bad = envelope();
        bad.envelope_identity.digest = HEX.to_owned();
        bad.evidence_kind = "unknown".to_owned();
        bad.verification_role = VerificationRole::Property;
        bad.compatibility_projection = Some(serde_json::json!({"drift": true}));
        bad.projection_identity = Some(EvidenceChainDigest {
            domain: "compatibility-projection".to_owned(),
            digest: OTHER_HEX.to_owned(),
        });
        bad.non_claims.clear();
        let report = validate_evidence_chain(&bad);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "canonical_envelope_hash_mismatch"));
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_profile"));
        assert!(report.issues.iter().any(|issue| issue.code == "projection_drift"));
        assert!(report.issues.iter().any(|issue| issue.code == "non_claims_weakened"));
    }
}
