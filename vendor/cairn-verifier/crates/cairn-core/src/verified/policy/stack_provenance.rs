use serde_json::Value;

use crate::verified::stack_provenance::{
    is_blake3_hex, is_stack_provenance_mode, StackProvenancePolicy,
};

use super::{fields::string_array_field, fields::string_field, PolicyError};

pub(super) fn stack_provenance_policy(value: &Value) -> Result<StackProvenancePolicy, PolicyError> {
    // r[impl cairn.stack_provenance_gates.policy]
    let policy = value
        .get("stack_provenance_gate")
        .ok_or(PolicyError::MissingField("stack_provenance_gate"))?;
    let mode = string_field(policy, "stack_provenance_gate.mode")?;
    if !is_stack_provenance_mode(&mode) {
        return Err(PolicyError::InvalidField("stack_provenance_gate.mode"));
    }
    let allowed_claim_scopes =
        string_array_field(policy, "stack_provenance_gate.allowed_claim_scopes")?;
    if allowed_claim_scopes.is_empty() {
        return Err(PolicyError::InvalidField(
            "stack_provenance_gate.allowed_claim_scopes",
        ));
    }
    let accepted_valence_policy_hashes = string_array_field(
        policy,
        "stack_provenance_gate.accepted_valence_policy_hashes",
    )?;
    if accepted_valence_policy_hashes
        .iter()
        .any(|hash| !is_blake3_hex(hash))
    {
        return Err(PolicyError::InvalidField(
            "stack_provenance_gate.accepted_valence_policy_hashes",
        ));
    }
    let required_boundary = string_field(policy, "stack_provenance_gate.required_boundary")?;
    if required_boundary.is_empty() {
        return Err(PolicyError::InvalidField(
            "stack_provenance_gate.required_boundary",
        ));
    }
    Ok(StackProvenancePolicy {
        mode,
        allowed_claim_scopes,
        accepted_valence_policy_hashes,
        required_boundary,
    })
}
