pub(super) fn parse(
    value: &serde_json::Value,
) -> Result<crate::verified::agent_guidance::Policy, super::PolicyError> {
    // r[impl cairn.agent_guidance_context.project_config]
    let policy = value
        .get("agent_guidance_policy")
        .cloned()
        .ok_or(super::PolicyError::MissingField("agent_guidance_policy"))?;
    let parsed = serde_json::from_value::<crate::verified::agent_guidance::Policy>(policy)
        .map_err(|_| super::PolicyError::InvalidField("agent_guidance_policy"))?;
    crate::verified::agent_guidance::validate_policy(&parsed)
        .map_err(super::PolicyError::InvalidField)?;
    Ok(parsed)
}
