const NOMINAL_IDENTITY_POLICY_FIELD: &str = "nominal_identity_policy";

pub(super) fn from_value(
    value: &serde_json::Value,
) -> Result<crate::verified::nominal_identity::NominalIdentityPolicy, super::PolicyError> {
    // r[impl cairn.nominal_evidence_identities.policy]
    let raw = value.get(NOMINAL_IDENTITY_POLICY_FIELD).cloned().ok_or(
        super::PolicyError::MissingField(NOMINAL_IDENTITY_POLICY_FIELD),
    )?;
    let policy =
        serde_json::from_value::<crate::verified::nominal_identity::NominalIdentityPolicy>(raw)
            .map_err(|_| super::PolicyError::InvalidField(NOMINAL_IDENTITY_POLICY_FIELD))?;
    if crate::verified::nominal_identity::nominal_identity_policy_diagnostics(&policy).is_empty() {
        return Ok(policy);
    }
    Err(super::PolicyError::InvalidField(
        NOMINAL_IDENTITY_POLICY_FIELD,
    ))
}
