pub(super) fn from_value(
    value: &serde_json::Value,
) -> Result<crate::verified::function_address_gates::FunctionAddressPolicy, super::PolicyError> {
    // r[impl cairn.function_address_gates.policy]
    let policy = value
        .get("function_address_gate")
        .ok_or(super::PolicyError::MissingField("function_address_gate"))?;
    let mode = super::fields::string_field(policy, "function_address_gate.mode")?;
    if !crate::verified::function_address_gates::is_function_address_mode(&mode) {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.mode",
        ));
    }
    let allowed_claim_scopes =
        super::fields::string_array_field(policy, "function_address_gate.allowed_claim_scopes")?;
    if allowed_claim_scopes.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.allowed_claim_scopes",
        ));
    }
    let accepted_valence_policy_hashes = super::fields::string_array_field(
        policy,
        "function_address_gate.accepted_valence_policy_hashes",
    )?;
    if accepted_valence_policy_hashes
        .iter()
        .any(|hash| !crate::verified::function_address_gates::is_blake3_hex(hash))
    {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.accepted_valence_policy_hashes",
        ));
    }
    let required_roles =
        super::fields::string_array_field(policy, "function_address_gate.required_roles")?;
    if required_roles.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.required_roles",
        ));
    }
    let required_schemas =
        super::fields::string_array_field(policy, "function_address_gate.required_schemas")?;
    if required_schemas.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.required_schemas",
        ));
    }
    let required_boundary =
        super::fields::string_field(policy, "function_address_gate.required_boundary")?;
    if required_boundary.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "function_address_gate.required_boundary",
        ));
    }
    Ok(
        crate::verified::function_address_gates::FunctionAddressPolicy {
            mode,
            allowed_claim_scopes,
            accepted_valence_policy_hashes,
            required_roles,
            required_schemas,
            required_boundary,
        },
    )
}
