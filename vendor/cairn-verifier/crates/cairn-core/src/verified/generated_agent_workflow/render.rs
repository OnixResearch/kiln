use serde_json::json;

use crate::verified::evidence::{blake3_hex, canonical_json_hash};

// r[impl cairn.generated_agent_workflows.catalog]
// r[impl cairn.generated_agent_workflows.catalog.contracts]
// r[impl cairn.generated_agent_workflows.adapters]
// r[impl cairn.generated_agent_workflows.adapters.valid]
// r[impl cairn.generated_agent_workflows.adapters.authority]
// r[impl cairn.generated_agent_workflows.adapters.unknown]
use super::{
    admission::{find_generated_workflow_adapter, issue},
    model::{
        AdmittedGeneratedWorkflowCatalog, GeneratedWorkflowAdapter, GeneratedWorkflowAdapterFormat,
        GeneratedWorkflowIssue, RenderedGeneratedWorkflowFile,
        GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER, GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
    },
};

pub fn render_generated_agent_workflows(
    catalog: &AdmittedGeneratedWorkflowCatalog,
    adapter_id: &str,
) -> Result<Vec<RenderedGeneratedWorkflowFile>, Vec<GeneratedWorkflowIssue>> {
    let adapter =
        find_generated_workflow_adapter(catalog, adapter_id).map_err(|issue| vec![issue])?;
    if catalog.policy.descriptors.len() > adapter.max_files {
        return Err(vec![issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
            "adapter.max_files",
            "adapter file count bound is smaller than the workflow catalog",
        )]);
    }
    let mut files = Vec::with_capacity(catalog.policy.descriptors.len());
    for descriptor in &catalog.policy.descriptors {
        let relative_path = adapter_path(adapter, &descriptor.id);
        if !safe_generated_relative_path(&relative_path, catalog.policy.bounds.max_path_bytes) {
            return Err(vec![issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                format!("adapters.{}.path", adapter.id),
                "adapter generated an unsafe relative path",
            )]);
        }
        let contents = render_descriptor(adapter, descriptor);
        if contents.len() > catalog.policy.bounds.max_file_bytes {
            return Err(vec![issue(
                GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                format!("adapters.{}.file_bytes", adapter.id),
                "adapter output exceeds the policy file bound",
            )]);
        }
        let descriptor_identity = canonical_json_hash(&json!({
            "descriptor": descriptor,
            "generator_version": catalog.policy.generator_version,
        }));
        files.push(RenderedGeneratedWorkflowFile {
            workflow_id: descriptor.id.clone(),
            relative_path,
            content_blake3: blake3_hex(contents.as_bytes()),
            contents,
            descriptor_identity,
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

pub fn safe_generated_destination_path(path: &str, max_bytes: usize) -> bool {
    safe_generated_relative_path(path, max_bytes)
        && !path.starts_with('~')
        && !path.contains("${")
        && !path.contains("%USERPROFILE%")
}

pub fn generated_manifest_relative_path(adapter_id: &str) -> String {
    format!(
        "{}{}{}",
        super::model::GENERATED_AGENT_WORKFLOW_MANIFEST_PREFIX,
        adapter_id,
        super::model::GENERATED_AGENT_WORKFLOW_MANIFEST_SUFFIX
    )
}

pub fn join_generated_destination(
    destination: &str,
    relative: &str,
    max_bytes: usize,
) -> Result<String, GeneratedWorkflowIssue> {
    if !safe_generated_destination_path(destination, max_bytes)
        || !safe_generated_relative_path(relative, max_bytes)
    {
        return Err(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
            "destination",
            "destination and generated path must be safe project-relative paths",
        ));
    }
    let joined = format!("{destination}/{relative}");
    if !safe_generated_relative_path(&joined, max_bytes) {
        return Err(issue(
            GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
            "destination",
            "joined generated path exceeds the policy path bound",
        ));
    }
    Ok(joined)
}

fn adapter_path(adapter: &GeneratedWorkflowAdapter, workflow_id: &str) -> String {
    match adapter.format {
        GeneratedWorkflowAdapterFormat::AgentSkills => format!(
            "{}{}{}",
            adapter.file_prefix, workflow_id, adapter.directory_suffix
        ),
        GeneratedWorkflowAdapterFormat::Claude | GeneratedWorkflowAdapterFormat::Codex => {
            format!(
                "{}{}{}",
                adapter.file_prefix, workflow_id, adapter.file_suffix
            )
        }
    }
}

fn render_descriptor(
    adapter: &GeneratedWorkflowAdapter,
    descriptor: &super::model::GeneratedWorkflowDescriptor,
) -> String {
    let mut output = String::new();
    if adapter.front_matter {
        output.push_str("---\nname: ");
        output.push_str(&format!("cairn-{}\n", descriptor.id));
        output.push_str("description: \"");
        output.push_str(&front_matter_escape(&descriptor.goal));
        output.push_str("\"\n---\n\n");
    }
    output.push_str(&format!("# Cairn {} workflow\n\n", descriptor.id));
    output.push_str("## Goal\n\n");
    output.push_str(&descriptor.goal);
    output.push_str("\n\n## Success evidence\n\n");
    append_list(&mut output, &descriptor.success_evidence);
    output.push_str("\n## Required evidence\n\n");
    append_list(&mut output, &descriptor.required_evidence);
    output.push_str("\n## Constraints\n\n");
    append_list(&mut output, &descriptor.constraints);
    output.push_str("\n## Allowed Cairn command classes\n\n");
    append_list(&mut output, &descriptor.command_classes);
    output.push_str("\n## Immediate action\n\n");
    output.push_str(&descriptor.immediate_action);
    output.push_str("\n\n## Progress\n\n");
    output.push_str(&descriptor.progress_rule);
    output.push_str("\n\n## Output\n\n");
    output.push_str(&descriptor.output_shape);
    output.push_str("\n\n## Non-claims\n\n");
    append_list(&mut output, &descriptor.non_claims);
    output
}

fn append_list(output: &mut String, values: &[String]) {
    for value in values {
        output.push_str("- ");
        output.push_str(value);
        output.push('\n');
    }
}

fn front_matter_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

fn safe_generated_relative_path(path: &str, max_bytes: usize) -> bool {
    if path.is_empty()
        || path.len() > max_bytes
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains("//")
        || path.contains("://")
        || path.contains('\\')
        || path.contains(['*', '?', '[', ']', '$', ':', ';', '|', '&', '>', '<'])
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    path.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/')
    })
}
