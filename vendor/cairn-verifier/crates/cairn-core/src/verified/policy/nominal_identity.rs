use serde_json::Value;

use super::PolicyError;
use crate::verified::nominal_identity::{
    nominal_identity_policy_diagnostics, NominalIdentityPolicy,
};

const NOMINAL_IDENTITY_POLICY_FIELD: &str = "nominal_identity_policy";

pub(super) fn nominal_identity_policy(value: &Value) -> Result<NominalIdentityPolicy, PolicyError> {
    // r[impl cairn.nominal_evidence_identities.policy]
    let raw = value
        .get(NOMINAL_IDENTITY_POLICY_FIELD)
        .cloned()
        .ok_or(PolicyError::MissingField(NOMINAL_IDENTITY_POLICY_FIELD))?;
    let policy = serde_json::from_value::<NominalIdentityPolicy>(raw)
        .map_err(|_| PolicyError::InvalidField(NOMINAL_IDENTITY_POLICY_FIELD))?;
    if nominal_identity_policy_diagnostics(&policy).is_empty() {
        return Ok(policy);
    }
    Err(PolicyError::InvalidField(NOMINAL_IDENTITY_POLICY_FIELD))
}
