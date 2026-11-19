// r[impl cairn.generated_agent_workflows.catalog]
// r[impl cairn.generated_agent_workflows.catalog.contracts]
// r[impl cairn.generated_agent_workflows.adapters]
// r[impl cairn.generated_agent_workflows.adapters.valid]
// r[impl cairn.generated_agent_workflows.adapters.authority]
// r[impl cairn.generated_agent_workflows.adapters.unknown]
use super::admission::issue;

pub fn render_generated_agent_workflows(
    catalog: &super::model::AdmittedGeneratedWorkflowCatalog,
    adapter_id: &str,
) -> Result<
    Vec<super::model::RenderedGeneratedWorkflowFile>,
    Vec<super::model::GeneratedWorkflowIssue>,
> {
    let adapter = super::admission::find_generated_workflow_adapter(catalog, adapter_id)
        .map_err(|issue| vec![issue])?;
    let expected_files = catalog.policy.descriptors.len() + 1;
    if expected_files > adapter.max_files {
        return Err(vec![issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
            "adapter.max_files",
            "adapter file count bound is smaller than the workflow catalog",
        )]);
    }
    let mut files = Vec::with_capacity(expected_files);
    for descriptor in &catalog.policy.descriptors {
        let relative_path = adapter_path(adapter, &descriptor.id);
        if !safe_generated_relative_path(&relative_path, catalog.policy.bounds.max_path_bytes) {
            return Err(vec![issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                format!("adapters.{}.path", adapter.id),
                "adapter generated an unsafe relative path",
            )]);
        }
        let contents = render_descriptor(adapter, descriptor);
        if contents.len() > catalog.policy.bounds.max_file_bytes {
            return Err(vec![issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                format!("adapters.{}.file_bytes", adapter.id),
                "adapter output exceeds the policy file bound",
            )]);
        }
        let descriptor_identity =
            crate::verified::evidence::canonical_json_hash(&serde_json::json!({
                "descriptor": descriptor,
                "generator_version": catalog.policy.generator_version,
            }));
        files.push(super::model::RenderedGeneratedWorkflowFile {
            workflow_id: descriptor.id.clone(),
            relative_path,
            content_blake3: crate::verified::evidence::blake3_hex(contents.as_bytes()),
            contents,
            descriptor_identity,
        });
    }
    let entry = &catalog.policy.entry;
    let entry_path = adapter_path(adapter, &entry.id);
    if !safe_generated_relative_path(&entry_path, catalog.policy.bounds.max_path_bytes) {
        return Err(vec![issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
            format!("adapters.{}.path", adapter.id),
            "adapter generated an unsafe entry path",
        )]);
    }
    let entry_contents = render_entry(adapter, catalog).map_err(|issue| vec![issue])?;
    if entry_contents.len() > catalog.policy.bounds.max_file_bytes {
        return Err(vec![issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
            format!("adapters.{}.file_bytes", adapter.id),
            "entry guide output exceeds the policy file bound",
        )]);
    }
    let entry_identity = crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "entry": entry,
        "generator_version": catalog.policy.generator_version,
        "command_catalog_identity": catalog.command_catalog_identity,
    }));
    let entry_descriptor_identity =
        crate::verified::evidence::canonical_json_hash(&serde_json::json!({
            "entry_identity": entry_identity,
            "generator_version": catalog.policy.generator_version,
            "command_catalog_identity": catalog.command_catalog_identity,
            "adapter": {
                "id": adapter.id,
                "version": adapter.version,
                "format": adapter.format,
            },
        }));
    files.push(super::model::RenderedGeneratedWorkflowFile {
        workflow_id: entry.id.clone(),
        relative_path: entry_path,
        content_blake3: crate::verified::evidence::blake3_hex(entry_contents.as_bytes()),
        contents: entry_contents,
        descriptor_identity: entry_descriptor_identity,
    });
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
) -> Result<String, super::model::GeneratedWorkflowIssue> {
    if !safe_generated_destination_path(destination, max_bytes)
        || !safe_generated_relative_path(relative, max_bytes)
    {
        return Err(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
            "destination",
            "destination and generated path must be safe project-relative paths",
        ));
    }
    let joined = format!("{destination}/{relative}");
    if !safe_generated_relative_path(&joined, max_bytes) {
        return Err(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
            "destination",
            "joined generated path exceeds the policy path bound",
        ));
    }
    Ok(joined)
}

fn adapter_path(adapter: &super::model::GeneratedWorkflowAdapter, workflow_id: &str) -> String {
    match adapter.format {
        super::model::GeneratedWorkflowAdapterFormat::AgentSkills => format!(
            "{}{}{}",
            adapter.file_prefix, workflow_id, adapter.directory_suffix
        ),
        super::model::GeneratedWorkflowAdapterFormat::Claude
        | super::model::GeneratedWorkflowAdapterFormat::Codex => {
            format!(
                "{}{}{}",
                adapter.file_prefix, workflow_id, adapter.file_suffix
            )
        }
    }
}

fn render_descriptor(
    adapter: &super::model::GeneratedWorkflowAdapter,
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

fn render_entry(
    adapter: &super::model::GeneratedWorkflowAdapter,
    catalog: &super::model::AdmittedGeneratedWorkflowCatalog,
) -> Result<String, super::model::GeneratedWorkflowIssue> {
    let entry = &catalog.policy.entry;
    let mut output = String::new();
    if adapter.front_matter {
        output.push_str("---\nname: ");
        output.push_str(&format!("cairn-{}\n", entry.id));
        output.push_str("description: \"");
        output.push_str(&front_matter_escape(&entry.goal));
        output.push_str("\"\n---\n\n");
    }
    output.push_str("# Cairn entry guidance\n\n## Goal\n\n");
    output.push_str(&entry.goal);
    output.push_str("\n\n## Native layout\n\nUse the project-local `");
    output.push_str(&entry.layout);
    output.push_str("/` layout at the explicit project root. Treat any other lifecycle layout as a migration input, never a runtime fallback.\n\n## Discover current state\n\n");
    for route in &entry.discovery_routes {
        let command = super::admission::find_generated_workflow_command(catalog, route)?;
        output.push_str("- `");
        output.push_str(&command.invocation);
        output.push_str("`\n");
    }
    output.push_str("\n## Check readiness\n\n");
    for route in &entry.readiness_routes {
        let command = super::admission::find_generated_workflow_command(catalog, route)?;
        output.push_str("- `");
        output.push_str(&command.invocation);
        output.push_str("`\n");
    }
    output.push_str("\n## Workflow profiles\n\n");
    append_list(&mut output, &entry.profiles);
    output.push_str("\nSelect a profile only through `");
    output.push_str(
        &super::admission::find_generated_workflow_command(catalog, &entry.profile_route)?
            .invocation,
    );
    output.push_str(
        "`.\n\n## Stage instructions\n\nRequest one artifact's stage instructions with `",
    );
    output.push_str(
        &super::admission::find_generated_workflow_command(catalog, &entry.stage_route)?.invocation,
    );
    output.push_str("`. Load only that selected stage guide; do not concatenate every stage description.\n\n## Authority limits\n\n");
    append_list(&mut output, &entry.boundaries);
    output.push_str("\n## Required evidence\n\n");
    append_list(&mut output, &entry.required_evidence);
    output.push_str("\n## Non-claims\n\n");
    append_list(&mut output, &catalog.policy.required_non_claims);
    if output.len() > entry.max_bytes
        || !entry
            .boundaries
            .iter()
            .all(|boundary| output.contains(boundary))
        || !entry
            .required_evidence
            .iter()
            .all(|evidence| output.contains(evidence))
    {
        return Err(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            "entry.max_bytes",
            "rendered entry guide exceeds its bound or omits required authority and evidence",
        ));
    }
    if super::admission::contains_legacy_layout_reference(&output) {
        return Err(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_GUIDE_LAYOUT,
            "entry",
            "rendered entry guide references a legacy lifecycle layout",
        ));
    }
    Ok(output)
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
