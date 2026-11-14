use serde_json::Value;

use crate::verified::generated_agent_workflow::{
    admit_generated_agent_workflow_policy, GeneratedAgentWorkflowPolicy,
    GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
};

use super::PolicyError;

pub(super) fn generated_agent_workflow_policy(
    value: &Value,
) -> Result<GeneratedAgentWorkflowPolicy, PolicyError> {
    let field = value
        .get("generated_agent_workflow_policy")
        .ok_or(PolicyError::MissingField("generated_agent_workflow_policy"))?;
    let policy: GeneratedAgentWorkflowPolicy = serde_json::from_value(field.clone())
        .map_err(|_| PolicyError::InvalidField("generated_agent_workflow_policy"))?;
    admit_generated_agent_workflow_policy(&policy).map_err(|issues| {
        let first = issues.into_iter().next().unwrap_or_else(|| {
            crate::verified::generated_agent_workflow::GeneratedWorkflowIssue {
                code: GENERATED_AGENT_WORKFLOW_ISSUE_POLICY.to_string(),
                field_path: "generated_agent_workflow_policy".to_string(),
                message: "generated workflow policy admission failed".to_string(),
            }
        });
        PolicyError::InvalidWorkflow {
            code: GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            path: first.field_path,
            message: format!("{}: {}", first.code, first.message),
        }
    })?;
    Ok(policy)
}
