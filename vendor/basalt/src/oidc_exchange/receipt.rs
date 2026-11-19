use serde::{Deserialize, Serialize};

use super::model::{ExchangeRequest, IssuancePlan, OidcPolicy};

/// OIDC exchange receipt schema.
pub const EXCHANGE_RECEIPT_SCHEMA: &str = "basalt.oidc-exchange.receipt.v1";
const REQUIRED_NON_CLAIMS: [&str; 5] = [
    "receipt is not live authority",
    "receipt does not contain or recreate bearer tokens",
    "receipt does not prove provider or network truth",
    "receipt does not prove replay-store durability",
    "receipt does not grant Mantle execution or release authority",
];

/// Terminal exchange outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExchangeOutcome {
    /// UCAN issuance completed.
    Issued,
    /// Exchange failed closed.
    Denied,
}

/// Metadata-only OIDC exchange receipt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeReceipt {
    /// Receipt schema.
    pub schema: String,
    /// Terminal outcome.
    pub outcome: ExchangeOutcome,
    /// Stable reason.
    pub reason: String,
    /// Exact policy identity.
    pub policy_identity: String,
    /// Issuer profile identifier.
    pub issuer_profile: String,
    /// Optional public subject identity.
    pub subject_identity: Option<String>,
    /// Optional public JWK identifier.
    pub key_id: Option<String>,
    /// Requested capability identity.
    pub requested_capability_identity: String,
    /// Granted capability identity, only after issuance.
    pub granted_capability_identity: Option<String>,
    /// Exact receipt identity.
    pub receipt_identity: String,
    /// Required claim boundaries.
    pub non_claims: Vec<String>,
}

/// Build one success receipt from the exact issued plan.
#[must_use]
pub fn receipt_for_success(policy: &OidcPolicy, plan: &IssuancePlan) -> ExchangeReceipt {
    let capability_identity = capability_identity(
        plan.audience.as_str(),
        plan.project.as_str(),
        plan.operation.ability(),
        plan.resource.as_str(),
        plan.expires_at_seconds,
    );
    finish(ExchangeReceipt {
        schema: EXCHANGE_RECEIPT_SCHEMA.to_owned(),
        outcome: ExchangeOutcome::Issued,
        reason: "issued".to_owned(),
        policy_identity: policy.identity.clone(),
        issuer_profile: policy.issuer_id.clone(),
        subject_identity: Some(plan.subject.identity.clone()),
        key_id: Some(plan.key_id.clone()),
        requested_capability_identity: capability_identity.clone(),
        granted_capability_identity: Some(capability_identity),
        receipt_identity: String::new(),
        non_claims: REQUIRED_NON_CLAIMS.map(str::to_owned).to_vec(),
    })
}

/// Build one failure receipt without bearer or private material.
#[must_use]
pub fn receipt_for_failure(
    policy: &OidcPolicy,
    request: &ExchangeRequest,
    reason: &str,
    key_id: Option<&str>,
) -> ExchangeReceipt {
    finish(ExchangeReceipt {
        schema: EXCHANGE_RECEIPT_SCHEMA.to_owned(),
        outcome: ExchangeOutcome::Denied,
        reason: reason.to_owned(),
        policy_identity: policy.identity.clone(),
        issuer_profile: policy.issuer_id.clone(),
        subject_identity: None,
        key_id: key_id.map(str::to_owned),
        requested_capability_identity: capability_identity(
            request.audience.as_str(),
            request.project.as_str(),
            request.operation.ability(),
            request.resource.as_str(),
            request.lifetime_seconds,
        ),
        granted_capability_identity: None,
        receipt_identity: String::new(),
        non_claims: REQUIRED_NON_CLAIMS.map(str::to_owned).to_vec(),
    })
}

fn finish(mut receipt: ExchangeReceipt) -> ExchangeReceipt {
    let bytes = serde_json::to_vec(&receipt).unwrap_or_default();
    receipt.receipt_identity = format!("blake3:{}", blake3::hash(bytes.as_slice()).to_hex());
    receipt
}

fn capability_identity(
    audience: &str,
    project: &str,
    operation: &str,
    resource: &str,
    lifetime_or_expiration: u64,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"basalt.oidc.capability.v1");
    for value in [audience, project, operation, resource] {
        update_frame(&mut hasher, value.as_bytes());
    }
    update_frame(&mut hasher, &lifetime_or_expiration.to_le_bytes());
    format!("blake3:{}", hasher.finalize().to_hex())
}

fn update_frame(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(&length.to_le_bytes());
    hasher.update(value);
}

/// Reject receipt text that contains prohibited bearer or private values.
///
/// # Errors
///
/// Returns the first forbidden marker that appears in the receipt text.
pub fn validate_receipt_redaction<'a>(
    receipt_text: &str,
    forbidden_values: &'a [&str],
) -> Result<(), &'a str> {
    for value in forbidden_values {
        if !value.is_empty() && receipt_text.contains(value) {
            return Err(value);
        }
    }
    Ok(())
}
