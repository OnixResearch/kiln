use serde_json::Value;

use crate::verified::runtime_evidence::{
    is_runtime_capture_origin, RuntimeEvidencePolicy, RUNTIME_HASH_ALGORITHM_BLAKE3,
};

use super::{fields::string_array_field, PolicyError};

pub(super) fn runtime_evidence_policy(value: &Value) -> Result<RuntimeEvidencePolicy, PolicyError> {
    // r[impl cairn.runtime_evidence.policy_schema]
    let policy = value
        .get("runtime_evidence_policy")
        .ok_or(PolicyError::MissingField("runtime_evidence_policy"))?;
    let allowed_capture_origins =
        string_array_field(policy, "runtime_evidence_policy.allowed_capture_origins")?;
    if allowed_capture_origins.is_empty()
        || allowed_capture_origins
            .iter()
            .any(|origin| !is_runtime_capture_origin(origin))
    {
        return Err(PolicyError::InvalidField(
            "runtime_evidence_policy.allowed_capture_origins",
        ));
    }
    let allowed_hash_algorithms =
        string_array_field(policy, "runtime_evidence_policy.allowed_hash_algorithms")?;
    if allowed_hash_algorithms.is_empty()
        || allowed_hash_algorithms
            .iter()
            .any(|algorithm| algorithm != RUNTIME_HASH_ALGORITHM_BLAKE3)
    {
        return Err(PolicyError::InvalidField(
            "runtime_evidence_policy.allowed_hash_algorithms",
        ));
    }
    let max_summary_bytes = policy
        .get("max_summary_bytes")
        .and_then(Value::as_u64)
        .ok_or(PolicyError::MissingField(
            "runtime_evidence_policy.max_summary_bytes",
        ))?;
    if max_summary_bytes == 0 {
        return Err(PolicyError::InvalidField(
            "runtime_evidence_policy.max_summary_bytes",
        ));
    }
    Ok(RuntimeEvidencePolicy {
        allowed_capture_origins,
        allowed_hash_algorithms,
        max_summary_bytes,
        forbidden_summary_fragments: string_array_field(
            policy,
            "runtime_evidence_policy.forbidden_summary_fragments",
        )?,
        aggregation_profiles: string_array_field(
            policy,
            "runtime_evidence_policy.aggregation_profiles",
        )?,
    })
}
