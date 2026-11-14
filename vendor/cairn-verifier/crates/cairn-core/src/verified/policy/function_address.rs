use serde_json::Value;

use crate::verified::function_address_gates::{
    is_blake3_hex, is_function_address_mode, FunctionAddressPolicy,
};

use super::{fields::string_array_field, fields::string_field, PolicyError};

pub(super) fn function_address_policy(value: &Value) -> Result<FunctionAddressPolicy, PolicyError> {
    // r[impl cairn.function_address_gates.policy]
    let policy = value
        .get("function_address_gate")
        .ok_or(PolicyError::MissingField("function_address_gate"))?;
    let mode = string_field(policy, "function_address_gate.mode")?;
    if !is_function_address_mode(&mode) {
        return Err(PolicyError::InvalidField("function_address_gate.mode"));
    }
    let allowed_claim_scopes =
        string_array_field(policy, "function_address_gate.allowed_claim_scopes")?;
    if allowed_claim_scopes.is_empty() {
        return Err(PolicyError::InvalidField(
            "function_address_gate.allowed_claim_scopes",
        ));
    }
    let accepted_valence_policy_hashes = string_array_field(
        policy,
        "function_address_gate.accepted_valence_policy_hashes",
    )?;
    if accepted_valence_policy_hashes
        .iter()
        .any(|hash| !is_blake3_hex(hash))
    {
        return Err(PolicyError::InvalidField(
            "function_address_gate.accepted_valence_policy_hashes",
        ));
    }
    let required_roles = string_array_field(policy, "function_address_gate.required_roles")?;
    if required_roles.is_empty() {
        return Err(PolicyError::InvalidField(
            "function_address_gate.required_roles",
        ));
    }
    let required_schemas = string_array_field(policy, "function_address_gate.required_schemas")?;
    if required_schemas.is_empty() {
        return Err(PolicyError::InvalidField(
            "function_address_gate.required_schemas",
        ));
    }
    let required_boundary = string_field(policy, "function_address_gate.required_boundary")?;
    if required_boundary.is_empty() {
        return Err(PolicyError::InvalidField(
            "function_address_gate.required_boundary",
        ));
    }
    Ok(FunctionAddressPolicy {
        mode,
        allowed_claim_scopes,
        accepted_valence_policy_hashes,
        required_roles,
        required_schemas,
        required_boundary,
    })
}
