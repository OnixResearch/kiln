pub(super) fn from_value(
    value: &serde_json::Value,
) -> Result<crate::verified::stack_provenance::StackProvenancePolicy, super::PolicyError> {
    // r[impl cairn.stack_provenance_gates.policy]
    let policy = value
        .get("stack_provenance_gate")
        .ok_or(super::PolicyError::MissingField("stack_provenance_gate"))?;
    let mode = super::fields::string_field(policy, "stack_provenance_gate.mode")?;
    if !crate::verified::stack_provenance::is_stack_provenance_mode(&mode) {
        return Err(super::PolicyError::InvalidField(
            "stack_provenance_gate.mode",
        ));
    }
    let allowed_claim_scopes =
        super::fields::string_array_field(policy, "stack_provenance_gate.allowed_claim_scopes")?;
    if allowed_claim_scopes.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "stack_provenance_gate.allowed_claim_scopes",
        ));
    }
    let accepted_valence_policy_hashes = super::fields::string_array_field(
        policy,
        "stack_provenance_gate.accepted_valence_policy_hashes",
    )?;
    if accepted_valence_policy_hashes
        .iter()
        .any(|hash| !crate::verified::stack_provenance::is_blake3_hex(hash))
    {
        return Err(super::PolicyError::InvalidField(
            "stack_provenance_gate.accepted_valence_policy_hashes",
        ));
    }
    let required_boundary =
        super::fields::string_field(policy, "stack_provenance_gate.required_boundary")?;
    if required_boundary.is_empty() {
        return Err(super::PolicyError::InvalidField(
            "stack_provenance_gate.required_boundary",
        ));
    }
    Ok(crate::verified::stack_provenance::StackProvenancePolicy {
        mode,
        allowed_claim_scopes,
        accepted_valence_policy_hashes,
        required_boundary,
    })
}
