use serde::Serialize;

use super::identity::receipt_identity;
use super::{
    BranchAuthorityDecision, BranchAuthorityDiagnostic, BranchMode, BranchObligation,
    POLICY_NON_CLAIMS, RECEIPT_SCHEMA,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchAuthorityReceipt {
    pub schema: &'static str,
    pub decision_ref: String,
    pub allowed: bool,
    pub policy_ref: String,
    pub request_ref: String,
    pub authority_input_ref: String,
    pub selected_rule_ref: Option<String>,
    pub mode: Option<BranchMode>,
    pub obligations: Vec<BranchObligation>,
    pub diagnostic: BranchAuthorityDiagnostic,
    pub non_claims: Vec<String>,
}

#[derive(Serialize)]
struct ReceiptBody<'a> {
    schema: &'static str,
    allowed: bool,
    policy_ref: &'a str,
    request_ref: &'a str,
    authority_input_ref: &'a str,
    selected_rule_ref: Option<&'a str>,
    mode: Option<BranchMode>,
    obligations: &'a [BranchObligation],
    diagnostic: BranchAuthorityDiagnostic,
    non_claims: Vec<String>,
}

/// Project one decision into a metadata-only, domain-bound receipt.
///
/// # Errors
///
/// Returns a serialization error if the closed receipt body cannot be encoded.
pub fn branch_authority_receipt(
    decision: &BranchAuthorityDecision,
) -> Result<BranchAuthorityReceipt, serde_json::Error> {
    let non_claims = POLICY_NON_CLAIMS
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let body = ReceiptBody {
        schema: RECEIPT_SCHEMA,
        allowed: decision.is_allowed(),
        policy_ref: decision.policy_ref(),
        request_ref: decision.request_ref(),
        authority_input_ref: decision.authority_input_ref(),
        selected_rule_ref: decision.selected_rule_ref(),
        mode: decision.mode(),
        obligations: decision.obligations(),
        diagnostic: decision.diagnostic(),
        non_claims: non_claims.clone(),
    };
    let canonical_json = serde_json::to_vec(&body)?;
    Ok(BranchAuthorityReceipt {
        schema: RECEIPT_SCHEMA,
        decision_ref: receipt_identity(&canonical_json),
        allowed: decision.is_allowed(),
        policy_ref: decision.policy_ref().to_string(),
        request_ref: decision.request_ref().to_string(),
        authority_input_ref: decision.authority_input_ref().to_string(),
        selected_rule_ref: decision.selected_rule_ref().map(ToString::to_string),
        mode: decision.mode(),
        obligations: decision.obligations().to_vec(),
        diagnostic: decision.diagnostic(),
        non_claims,
    })
}
