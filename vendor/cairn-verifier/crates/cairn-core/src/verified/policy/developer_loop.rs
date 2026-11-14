use serde_json::Value;

use crate::verified::developer_loop::{admit_developer_loop_policy, DeveloperLoopPolicy};

use super::PolicyError;

pub(super) fn developer_loop_policy(value: &Value) -> Result<DeveloperLoopPolicy, PolicyError> {
    let field = value
        .get("developer_loop_policy")
        .ok_or(PolicyError::MissingField("developer_loop_policy"))?;
    let policy: DeveloperLoopPolicy = serde_json::from_value(field.clone())
        .map_err(|_| PolicyError::InvalidField("developer_loop_policy"))?;
    admit_developer_loop_policy(policy.clone()).map_err(|first| {
        let first = first.first().expect("admission returns at least one issue");
        PolicyError::InvalidWorkflow {
            code: "developer_loop.policy.invalid",
            path: first.field_path.clone(),
            message: format!("{}: {}", first.code, first.message),
        }
    })?;
    Ok(policy)
}
