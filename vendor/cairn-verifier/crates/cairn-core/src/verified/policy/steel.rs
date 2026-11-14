use serde_json::Value;

use super::{fields::*, PolicyError, SteelOrchestrationPolicy, SteelOrchestrationProfile};

pub(super) fn steel_orchestration_policy(
    value: &Value,
) -> Result<SteelOrchestrationPolicy, PolicyError> {
    let policy = value
        .get("steel_orchestration_policy")
        .ok_or(PolicyError::MissingField("steel_orchestration_policy"))?;
    let enabled = bool_field(policy, "steel_orchestration_policy.enabled")?;
    let profiles = steel_orchestration_profiles(policy)?;
    if enabled && profiles.is_empty() {
        return Err(PolicyError::InvalidField(
            "steel_orchestration_policy.profiles",
        ));
    }
    Ok(SteelOrchestrationPolicy { enabled, profiles })
}

fn steel_orchestration_profiles(
    policy: &Value,
) -> Result<Vec<SteelOrchestrationProfile>, PolicyError> {
    let profiles =
        policy
            .get("profiles")
            .and_then(Value::as_array)
            .ok_or(PolicyError::MissingField(
                "steel_orchestration_policy.profiles",
            ))?;
    profiles
        .iter()
        .map(|profile| {
            let mode = string_field(profile, "steel_orchestration_policy.profiles.mode")?;
            if !matches!(
                mode.as_str(),
                "disabled" | "comparison" | "opt_in" | "selected_default" | "broad_default"
            ) {
                return Err(PolicyError::InvalidField(
                    "steel_orchestration_policy.profiles.mode",
                ));
            }
            Ok(SteelOrchestrationProfile {
                id: string_field(profile, "steel_orchestration_policy.profiles.id")?,
                mode,
                allowed_host_functions: string_array_field(
                    profile,
                    "steel_orchestration_policy.profiles.allowed_host_functions",
                )?,
                deterministic_budgets: string_array_field(
                    profile,
                    "steel_orchestration_policy.profiles.deterministic_budgets",
                )?,
                redaction_classes: string_array_field(
                    profile,
                    "steel_orchestration_policy.profiles.redaction_classes",
                )?,
                fallback_modes: string_array_field(
                    profile,
                    "steel_orchestration_policy.profiles.fallback_modes",
                )?,
                receipt_schema_version: string_field(
                    profile,
                    "steel_orchestration_policy.profiles.receipt_schema_version",
                )?,
            })
        })
        .collect()
}
