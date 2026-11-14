use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::verified::evidence::canonical_json_hash;

use super::model::{
    AdmittedGeneratedWorkflowCatalog, GeneratedAgentWorkflowPolicy, GeneratedWorkflowAdapter,
    GeneratedWorkflowAdapterFormat, GeneratedWorkflowDescriptor, GeneratedWorkflowIssue,
    GeneratedWorkflowMutationClass, GeneratedWorkflowPermission,
    GENERATED_AGENT_WORKFLOW_EXPLORE_ID, GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE,
    GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION, GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
    GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR, GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
    GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1, INITIAL_ADAPTER_IDS, INITIAL_WORKFLOW_IDS,
};

const REQUIRED_RECEIPT_FIELDS: &[&str] = &[
    "adapter",
    "destination",
    "files",
    "manifest",
    "plan_hash",
    "policy_hash",
    "non_claims",
];
const MINIMUM_ADAPTER_VERSION: u32 = 1;
const MINIMUM_DESCRIPTOR_VERSION: u32 = 1;

// r[impl cairn.generated_agent_workflows.descriptors]
// r[impl cairn.generated_agent_workflows.descriptors.valid]
// r[impl cairn.generated_agent_workflows.descriptors.invalid]
// r[impl cairn.generated_agent_workflows.explore]
// r[impl cairn.generated_agent_workflows.explore.valid]
// r[impl cairn.generated_agent_workflows.explore.invalid]
pub fn admit_generated_agent_workflow_policy(
    policy: &GeneratedAgentWorkflowPolicy,
) -> Result<AdmittedGeneratedWorkflowCatalog, Vec<GeneratedWorkflowIssue>> {
    let mut issues = validate_policy_header(policy);
    let command_permissions = validate_command_classes(policy, &mut issues);
    validate_descriptors(policy, &command_permissions, &mut issues);
    validate_adapters(policy, &mut issues);
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let issue_limit = policy.bounds.max_issues.max(1);
    issues.truncate(issue_limit);
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut normalized = policy.clone();
    normalized
        .command_classes
        .sort_by(|left, right| left.id.cmp(&right.id));
    normalized
        .descriptors
        .sort_by(|left, right| left.id.cmp(&right.id));
    normalized
        .adapters
        .sort_by(|left, right| left.id.cmp(&right.id));
    for descriptor in &mut normalized.descriptors {
        normalize_strings(&mut descriptor.success_evidence);
        normalize_strings(&mut descriptor.required_evidence);
        normalize_strings(&mut descriptor.command_classes);
        normalize_strings(&mut descriptor.constraints);
        normalize_strings(&mut descriptor.non_claims);
    }
    let catalog_identity = canonical_json_hash(&json!({
        "generator_version": normalized.generator_version,
        "command_classes": normalized.command_classes,
        "descriptors": normalized.descriptors,
        "adapters": normalized.adapters,
        "bounds": normalized.bounds,
        "non_claims": normalized.required_non_claims,
    }));
    Ok(AdmittedGeneratedWorkflowCatalog {
        policy: normalized,
        catalog_identity,
    })
}

pub fn find_generated_workflow_descriptor<'a>(
    catalog: &'a AdmittedGeneratedWorkflowCatalog,
    id: &str,
) -> Result<&'a GeneratedWorkflowDescriptor, GeneratedWorkflowIssue> {
    catalog
        .policy
        .descriptors
        .iter()
        .find(|descriptor| descriptor.id == id)
        .ok_or_else(|| {
            issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                "workflow",
                format!(
                    "unknown workflow {id}; known workflows: {}",
                    INITIAL_WORKFLOW_IDS.join(", ")
                ),
            )
        })
}

pub fn find_generated_workflow_adapter<'a>(
    catalog: &'a AdmittedGeneratedWorkflowCatalog,
    id: &str,
) -> Result<&'a GeneratedWorkflowAdapter, GeneratedWorkflowIssue> {
    catalog
        .policy
        .adapters
        .iter()
        .find(|adapter| adapter.id == id)
        .ok_or_else(|| {
            issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                "adapter",
                format!(
                    "unknown adapter {id}; known adapters: {}",
                    INITIAL_ADAPTER_IDS.join(", ")
                ),
            )
        })
}

fn validate_policy_header(policy: &GeneratedAgentWorkflowPolicy) -> Vec<GeneratedWorkflowIssue> {
    let mut issues = Vec::new();
    if policy.schema != GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1 {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.schema",
            "generated workflow policy schema is unsupported",
        ));
    }
    if policy.generator_version != GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.generator_version",
            "generated workflow generator version is unsupported",
        ));
    }
    let bounds = &policy.bounds;
    if bounds.max_descriptors == 0
        || bounds.max_adapters == 0
        || bounds.max_command_classes == 0
        || bounds.max_text_bytes == 0
        || bounds.max_list_items == 0
        || bounds.max_files_per_adapter == 0
        || bounds.max_path_bytes == 0
        || bounds.max_file_bytes == 0
        || bounds.max_issues == 0
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.bounds",
            "all generated workflow bounds must be positive",
        ));
    }
    if policy.descriptors.len() > bounds.max_descriptors
        || policy.adapters.len() > bounds.max_adapters
        || policy.command_classes.len() > bounds.max_command_classes
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy",
            "generated workflow policy exceeds a collection bound",
        ));
    }
    if !contains_required(&policy.receipt_fields, REQUIRED_RECEIPT_FIELDS)
        || !unique_nonempty(&policy.receipt_fields)
        || !unique_nonempty(&policy.required_non_claims)
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.receipt_fields",
            "generated workflow receipt fields or non-claims are incomplete",
        ));
    }
    issues
}

fn validate_command_classes(
    policy: &GeneratedAgentWorkflowPolicy,
    issues: &mut Vec<GeneratedWorkflowIssue>,
) -> BTreeMap<String, GeneratedWorkflowPermission> {
    let mut permissions = BTreeMap::new();
    for (index, command) in policy.command_classes.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.command_classes[{index}]");
        if !valid_id(&command.id)
            || permissions
                .insert(command.id.clone(), command.permission.clone())
                .is_some()
        {
            issues.push(issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
                format!("{path}.id"),
                "command class IDs must be safe and unique",
            ));
        }
    }
    if permissions.is_empty() {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.command_classes",
            "at least one command class is required",
        ));
    }
    permissions
}

fn validate_descriptors(
    policy: &GeneratedAgentWorkflowPolicy,
    command_permissions: &BTreeMap<String, GeneratedWorkflowPermission>,
    issues: &mut Vec<GeneratedWorkflowIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, descriptor) in policy.descriptors.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.descriptors[{index}]");
        validate_descriptor_shape(descriptor, &path, policy, &mut ids, issues);
        let mut has_mutation = false;
        for command in &descriptor.command_classes {
            match command_permissions.get(command) {
                Some(GeneratedWorkflowPermission::Mutation) => has_mutation = true,
                Some(GeneratedWorkflowPermission::Read) => {}
                None => issues.push(issue(
                    GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                    format!("{path}.command_classes"),
                    format!("unknown command class: {command}"),
                )),
            }
        }
        if descriptor.mutation_class == GeneratedWorkflowMutationClass::ReadOnly && has_mutation {
            issues.push(issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                format!("{path}.mutation_class"),
                "read-only workflow references a mutation command class",
            ));
        }
        if descriptor.id == GENERATED_AGENT_WORKFLOW_EXPLORE_ID
            && (has_mutation
                || descriptor.mutation_class != GeneratedWorkflowMutationClass::ReadOnly
                || !descriptor
                    .constraints
                    .iter()
                    .any(|constraint| constraint == GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE))
        {
            issues.push(issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                path,
                "explore must be read-only and include the exact no-write constraint",
            ));
        }
    }
    let actual = ids.iter().map(String::as_str).collect::<Vec<_>>();
    if actual != INITIAL_WORKFLOW_IDS {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.descriptors",
            format!(
                "initial workflow catalog must be: {}",
                INITIAL_WORKFLOW_IDS.join(", ")
            ),
        ));
    }
}

fn validate_descriptor_shape(
    descriptor: &GeneratedWorkflowDescriptor,
    path: &str,
    policy: &GeneratedAgentWorkflowPolicy,
    ids: &mut BTreeSet<String>,
    issues: &mut Vec<GeneratedWorkflowIssue>,
) {
    if !valid_id(&descriptor.id)
        || !ids.insert(descriptor.id.clone())
        || descriptor.version < MINIMUM_DESCRIPTOR_VERSION
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
            format!("{path}.id"),
            "workflow descriptor ID, version, or uniqueness is invalid",
        ));
    }
    let scalar_text = [
        descriptor.goal.as_str(),
        descriptor.progress_rule.as_str(),
        descriptor.immediate_action.as_str(),
        descriptor.output_shape.as_str(),
    ];
    if scalar_text
        .iter()
        .any(|text| !bounded_text(text, policy.bounds.max_text_bytes))
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
            path,
            "workflow descriptor scalar text is empty or out of bounds",
        ));
    }
    let lists = [
        &descriptor.success_evidence,
        &descriptor.required_evidence,
        &descriptor.command_classes,
        &descriptor.constraints,
        &descriptor.non_claims,
    ];
    if lists.iter().any(|values| {
        values.is_empty()
            || values.len() > policy.bounds.max_list_items
            || values
                .iter()
                .any(|value| !bounded_text(value, policy.bounds.max_text_bytes))
    }) || !contains_all(&descriptor.non_claims, &policy.required_non_claims)
    {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
            path,
            "workflow descriptor lists or non-claims are incomplete or out of bounds",
        ));
    }
}

fn validate_adapters(
    policy: &GeneratedAgentWorkflowPolicy,
    issues: &mut Vec<GeneratedWorkflowIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, adapter) in policy.adapters.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.adapters[{index}]");
        let expected_shape = match adapter.format {
            GeneratedWorkflowAdapterFormat::AgentSkills => {
                adapter.front_matter
                    && adapter.file_suffix.is_empty()
                    && adapter.directory_suffix == "/SKILL.md"
            }
            GeneratedWorkflowAdapterFormat::Claude | GeneratedWorkflowAdapterFormat::Codex => {
                !adapter.front_matter
                    && adapter.file_suffix == ".md"
                    && adapter.directory_suffix.is_empty()
            }
        };
        if !valid_id(&adapter.id)
            || !ids.insert(adapter.id.clone())
            || adapter.version < MINIMUM_ADAPTER_VERSION
            || !safe_path_fragment(&adapter.file_prefix)
            || adapter.max_files == 0
            || adapter.max_files > policy.bounds.max_files_per_adapter
            || !expected_shape
        {
            issues.push(issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                path,
                "adapter identity, version, format, paths, or bounds are invalid",
            ));
        }
    }
    let actual = ids.iter().map(String::as_str).collect::<Vec<_>>();
    if actual != INITIAL_ADAPTER_IDS {
        issues.push(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.adapters",
            format!(
                "initial adapter catalog must be: {}",
                INITIAL_ADAPTER_IDS.join(", ")
            ),
        ));
    }
}

fn normalize_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
}

fn safe_path_fragment(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn contains_required(values: &[String], required: &[&str]) -> bool {
    required
        .iter()
        .all(|required| values.iter().any(|value| value == required))
}

fn contains_all(values: &[String], required: &[String]) -> bool {
    required.iter().all(|required| values.contains(required))
}

pub(super) fn issue(
    code: impl Into<String>,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> GeneratedWorkflowIssue {
    GeneratedWorkflowIssue {
        code: code.into(),
        field_path: field_path.into(),
        message: message.into(),
    }
}
