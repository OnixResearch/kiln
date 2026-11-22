//! Optional producer observations never narrow declared-link guidance.
//! Octet translation is reused at its existing pinned revision. The reviewed
//! Valence pin has no evidence-admission span contract; present unsupported
//! evidence fails closed rather than borrowing the stack-sidecar contract.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{VerificationImpactInput, VerificationImpactScope};
use crate::canonical_json_hash;
use crate::verified::check_evidence::{admit_octet_json, OctetAdmission, OCTET_REVISION};

pub const STRUCTURAL_EVIDENCE_SCHEMA: &str = "cairn.verification-impact-evidence.v1";
pub const DECLARED_LINK_FALLBACK: &str = "declared_links_only";
const VALENCE_SPAN_UNAVAILABLE: &str = "valence_span_contract_unavailable";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationStructuralEvidence {
    pub schema_version: String,
    pub source_identity: String,
    pub scope: VerificationImpactScope,
    pub octet: Option<BoundOctetReceipt>,
    pub valence: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundOctetReceipt {
    pub producer_revision: String,
    pub receipt_hash: String,
    pub receipt: Value,
}

#[derive(Debug, Serialize)]
pub struct VerificationStructuralAdmission {
    pub valid: bool,
    pub selection_scope: &'static str,
    pub octet: Option<OctetAdmission>,
    pub blockers: Vec<&'static str>,
    pub unavailable_contracts: Vec<&'static str>,
    pub completion_eligible: bool,
    pub non_claims: Vec<&'static str>,
}

/// Source/cohort binding is a checked declaration, not producer authenticity.
/// No observation returned here proves dependency closure or removes a check.
// r[impl cairn.verification_impact.uncertainty]
pub fn admit_verification_structural_evidence(
    input: &VerificationImpactInput,
    evidence: Option<&VerificationStructuralEvidence>,
) -> VerificationStructuralAdmission {
    let mut report = VerificationStructuralAdmission {
        valid: true,
        selection_scope: DECLARED_LINK_FALLBACK,
        octet: None,
        blockers: Vec::new(),
        unavailable_contracts: vec![VALENCE_SPAN_UNAVAILABLE],
        completion_eligible: false,
        non_claims: vec![
            "producer observations do not establish dependency closure or safe omission",
            "receipt hashes and declared scope bindings do not establish authenticity",
            "Valence stack provenance sidecars are not evidence-admission span receipts",
        ],
    };
    let Some(evidence) = evidence else {
        return report;
    };
    if evidence.schema_version != STRUCTURAL_EVIDENCE_SCHEMA {
        report.blockers.push("structural_schema_unsupported");
    }
    if evidence.source_identity != input.after_manifest.identity
        || evidence.scope != input.after_manifest.scope
    {
        report.blockers.push("structural_source_scope_mismatch");
    }
    if let Some(octet) = &evidence.octet {
        if octet.producer_revision != OCTET_REVISION
            || octet.receipt_hash != canonical_json_hash(&octet.receipt)
        {
            report.blockers.push("octet_identity_mismatch");
        } else {
            match admit_octet_json(&octet.receipt) {
                Ok(admitted) => report.octet = Some(admitted),
                Err(_) => report.blockers.push("octet_receipt_invalid"),
            }
        }
    }
    if evidence.valence.is_some() {
        report.blockers.push(VALENCE_SPAN_UNAVAILABLE);
    }
    report.valid = report.blockers.is_empty();
    if !report.valid {
        report.octet = None;
    }
    report
}
