pub(super) fn from_value(
    value: &serde_json::Value,
) -> Result<crate::verified::developer_loop::DeveloperLoopPolicy, super::PolicyError> {
    let field = value
        .get("developer_loop_policy")
        .ok_or(super::PolicyError::MissingField("developer_loop_policy"))?;
    let policy: crate::verified::developer_loop::DeveloperLoopPolicy =
        serde_json::from_value(field.clone())
            .map_err(|_| super::PolicyError::InvalidField("developer_loop_policy"))?;
    crate::verified::developer_loop::admit_developer_loop_policy(policy.clone()).map_err(
        |first| {
            let first = first.first().expect("admission returns at least one issue");
            super::PolicyError::InvalidWorkflow {
                code: "developer_loop.policy.invalid",
                path: first.field_path.clone(),
                message: format!("{}: {}", first.code, first.message),
            }
        },
    )?;
    Ok(policy)
}
