pub(super) fn generated_agent_workflow_policy(
    value: &serde_json::Value,
) -> Result<
    crate::verified::generated_agent_workflow::GeneratedAgentWorkflowPolicy,
    super::PolicyError,
> {
    let field =
        value
            .get("generated_agent_workflow_policy")
            .ok_or(super::PolicyError::MissingField(
                "generated_agent_workflow_policy",
            ))?;
    let policy: crate::verified::generated_agent_workflow::GeneratedAgentWorkflowPolicy =
        serde_json::from_value(field.clone())
            .map_err(|_| super::PolicyError::InvalidField("generated_agent_workflow_policy"))?;
    crate::verified::generated_agent_workflow::admit_generated_agent_workflow_policy(&policy).map_err(|issues| {
        let first = issues.into_iter().next().unwrap_or_else(|| {
            crate::verified::generated_agent_workflow::GeneratedWorkflowIssue {
                code: crate::verified::generated_agent_workflow::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY.to_string(),
                field_path: "generated_agent_workflow_policy".to_string(),
                message: "generated workflow policy admission failed".to_string(),
            }
        });
        super::PolicyError::InvalidWorkflow {
            code: crate::verified::generated_agent_workflow::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            path: first.field_path,
            message: format!("{}: {}", first.code, first.message),
        }
    })?;
    Ok(policy)
}
