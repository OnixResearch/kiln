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
    policy: &super::model::GeneratedAgentWorkflowPolicy,
) -> Result<super::model::AdmittedGeneratedWorkflowCatalog, Vec<super::model::GeneratedWorkflowIssue>>
{
    let mut issues = validate_policy_header(policy);
    let command_permissions = validate_command_classes(policy, &mut issues);
    validate_command_catalog(policy, &command_permissions, &mut issues);
    validate_entry_guide(policy, &mut issues);
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
        .command_catalog
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
    let command_catalog_identity =
        crate::verified::evidence::canonical_json_hash(&serde_json::json!({
            "command_catalog": normalized.command_catalog,
        }));
    let catalog_identity = crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "generator_version": normalized.generator_version,
        "command_classes": normalized.command_classes,
        "command_catalog": normalized.command_catalog,
        "command_catalog_identity": command_catalog_identity,
        "entry": normalized.entry,
        "descriptors": normalized.descriptors,
        "adapters": normalized.adapters,
        "bounds": normalized.bounds,
        "non_claims": normalized.required_non_claims,
    }));
    Ok(super::model::AdmittedGeneratedWorkflowCatalog {
        policy: normalized,
        command_catalog_identity,
        catalog_identity,
    })
}

pub fn find_generated_workflow_descriptor<'a>(
    catalog: &'a super::model::AdmittedGeneratedWorkflowCatalog,
    id: &str,
) -> Result<&'a super::model::GeneratedWorkflowDescriptor, super::model::GeneratedWorkflowIssue> {
    catalog
        .policy
        .descriptors
        .iter()
        .find(|descriptor| descriptor.id == id)
        .ok_or_else(|| {
            issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                "workflow",
                format!(
                    "unknown workflow {}; known workflows: {}",
                    diagnostic_label(id),
                    super::model::INITIAL_WORKFLOW_IDS.join(", ")
                ),
            )
        })
}

pub fn find_generated_workflow_adapter<'a>(
    catalog: &'a super::model::AdmittedGeneratedWorkflowCatalog,
    id: &str,
) -> Result<&'a super::model::GeneratedWorkflowAdapter, super::model::GeneratedWorkflowIssue> {
    catalog
        .policy
        .adapters
        .iter()
        .find(|adapter| adapter.id == id)
        .ok_or_else(|| {
            issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                "adapter",
                format!(
                    "unknown adapter {}; known adapters: {}",
                    diagnostic_label(id),
                    super::model::INITIAL_ADAPTER_IDS.join(", ")
                ),
            )
        })
}

pub fn find_generated_workflow_command<'a>(
    catalog: &'a super::model::AdmittedGeneratedWorkflowCatalog,
    id: &str,
) -> Result<&'a super::model::GeneratedWorkflowCommand, super::model::GeneratedWorkflowIssue> {
    catalog
        .policy
        .command_catalog
        .iter()
        .find(|command| command.id == id)
        .ok_or_else(|| {
            issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                "entry.routes",
                format!("unknown command catalog route {}", diagnostic_label(id)),
            )
        })
}

pub fn find_entry_guide_profile<'a>(
    catalog: &'a super::model::AdmittedGeneratedWorkflowCatalog,
    profile_id: &str,
) -> Result<&'a str, super::model::GeneratedWorkflowIssue> {
    catalog
        .policy
        .entry
        .profiles
        .iter()
        .find(|profile| profile.as_str() == profile_id)
        .map(String::as_str)
        .ok_or_else(|| {
            issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                "entry.profiles",
                format!(
                    "unknown workflow profile {}; supported profiles: {}",
                    diagnostic_label(profile_id),
                    catalog.policy.entry.profiles.join(", ")
                ),
            )
        })
}

fn validate_command_catalog(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    command_permissions: &std::collections::BTreeMap<
        String,
        super::model::GeneratedWorkflowPermission,
    >,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) {
    let mut ids = std::collections::BTreeSet::new();
    if policy.command_catalog.len() > policy.bounds.max_command_classes {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.command_catalog",
            "command catalog exceeds the policy collection bound",
        ));
    }
    for (index, command) in policy.command_catalog.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.command_catalog[{index}]");
        if !valid_id(&command.id) || !ids.insert(command.id.clone()) {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                format!("{path}.id"),
                "command catalog IDs must be safe and unique",
            ));
        }
        if !command_permissions.contains_key(&command.command_class) {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                format!("{path}.command_class"),
                format!(
                    "unknown command class: {}",
                    diagnostic_label(&command.command_class)
                ),
            ));
        }
        if !bounded_text(&command.invocation, policy.bounds.max_text_bytes)
            || !command.invocation.starts_with("cairn ")
            || contains_legacy_layout_reference(&command.invocation)
        {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                format!("{path}.invocation"),
                "command invocation must be bounded, start with cairn, and use the native layout",
            ));
        }
    }
    if ids.is_empty() {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            "generated_agent_workflow_policy.command_catalog",
            "at least one command catalog entry is required",
        ));
    }
}

fn validate_entry_guide(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) {
    let entry = &policy.entry;
    let path = "generated_agent_workflow_policy.entry";
    if entry.id != super::model::GENERATED_AGENT_WORKFLOW_ENTRY_ID
        || entry.version < MINIMUM_DESCRIPTOR_VERSION
        || entry.layout != super::model::GENERATED_AGENT_WORKFLOW_NATIVE_LAYOUT
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            path,
            "entry guide identity, version, or native layout is invalid",
        ));
    }
    let scalar_text = [entry.goal.as_str(), entry.layout.as_str()];
    if scalar_text
        .iter()
        .any(|text| !bounded_text(text, policy.bounds.max_text_bytes))
        || contains_legacy_layout_reference(&entry.goal)
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            path,
            "entry guide text is empty, out of bounds, or uses a legacy layout",
        ));
    }
    let commands = policy
        .command_catalog
        .iter()
        .map(|command| command.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let routes = [
        (&entry.discovery_routes, "discovery_routes"),
        (&entry.readiness_routes, "readiness_routes"),
    ];
    for (values, name) in routes {
        if values.is_empty()
            || values.len() > policy.bounds.max_list_items
            || !unique_nonempty(values)
            || values
                .iter()
                .any(|value| !commands.contains(value.as_str()))
        {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                format!("{path}.{name}"),
                "entry routes must be unique, bounded, and present in the command catalog",
            ));
        }
    }
    for route in [&entry.stage_route, &entry.profile_route] {
        if !commands.contains(route.as_str()) {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
                format!("{path}.stage_route"),
                format!("unknown command catalog route {}", diagnostic_label(route)),
            ));
        }
    }
    if entry.profiles.is_empty()
        || entry.profiles.len() > policy.bounds.max_list_items
        || !unique_nonempty(&entry.profiles)
        || entry.profiles.iter().any(|profile| {
            !valid_id(profile) || !bounded_text(profile, policy.bounds.max_text_bytes)
        })
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            format!("{path}.profiles"),
            "entry profiles must be safe, unique, and bounded",
        ));
    }
    if entry.boundaries.is_empty()
        || entry.boundaries.len() > policy.bounds.max_list_items
        || !unique_nonempty(&entry.boundaries)
        || entry.boundaries.iter().any(|text| {
            !bounded_text(text, policy.bounds.max_text_bytes)
                || contains_legacy_layout_reference(text)
        })
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            format!("{path}.boundaries"),
            "entry boundaries must be unique, bounded, and use the native layout",
        ));
    }
    if entry.required_evidence.is_empty()
        || entry.required_evidence.len() > policy.bounds.max_list_items
        || !unique_nonempty(&entry.required_evidence)
        || entry.required_evidence.iter().any(|text| {
            !bounded_text(text, policy.bounds.max_text_bytes)
                || contains_legacy_layout_reference(text)
        })
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            format!("{path}.required_evidence"),
            "entry evidence constraints must be unique, bounded, and use the native layout",
        ));
    }
    let stage_command = policy
        .command_catalog
        .iter()
        .find(|command| command.id == entry.stage_route);
    if entry.max_stage_route_bytes == 0
        || stage_command.is_none_or(|command| {
            command.invocation.is_empty()
                || command.invocation.len() > entry.max_stage_route_bytes
                || command.invocation.len() > policy.bounds.max_text_bytes
        })
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            format!("{path}.max_stage_route_bytes"),
            "stage route text exceeds the entry stage-route bound",
        ));
    }
    let required_bytes = entry_required_bytes(policy);
    if entry.max_bytes == 0
        || entry.max_bytes > policy.bounds.max_file_bytes
        || entry.max_bytes < required_bytes
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ENTRY,
            format!("{path}.max_bytes"),
            "entry text bound cannot carry mandatory boundaries, evidence, and routes",
        ));
    }
}

fn entry_required_bytes(policy: &super::model::GeneratedAgentWorkflowPolicy) -> usize {
    let entry = &policy.entry;
    let mut total = entry.goal.len() + entry.layout.len() + 512;
    for values in [
        &entry.boundaries,
        &entry.required_evidence,
        &entry.profiles,
        &policy.required_non_claims,
    ] {
        total = values
            .iter()
            .fold(total, |total, value| total.saturating_add(value.len() + 4));
    }
    for route in entry
        .discovery_routes
        .iter()
        .chain(entry.readiness_routes.iter())
        .chain(std::iter::once(&entry.stage_route))
        .chain(std::iter::once(&entry.profile_route))
    {
        if let Some(command) = policy
            .command_catalog
            .iter()
            .find(|command| &command.id == route)
        {
            total = total.saturating_add(command.invocation.len() + 4);
        }
    }
    total
}

fn validate_policy_header(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
) -> Vec<super::model::GeneratedWorkflowIssue> {
    let mut issues = Vec::new();
    if policy.schema != super::model::GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1 {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.schema",
            "generated workflow policy schema is unsupported",
        ));
    }
    if policy.generator_version != super::model::GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
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
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.bounds",
            "all generated workflow bounds must be positive",
        ));
    }
    if policy.descriptors.len() > bounds.max_descriptors
        || policy.adapters.len() > bounds.max_adapters
        || policy.command_classes.len() > bounds.max_command_classes
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy",
            "generated workflow policy exceeds a collection bound",
        ));
    }
    if !contains_required(&policy.receipt_fields, REQUIRED_RECEIPT_FIELDS)
        || !unique_nonempty(&policy.receipt_fields)
        || !unique_nonempty(&policy.required_non_claims)
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.receipt_fields",
            "generated workflow receipt fields or non-claims are incomplete",
        ));
    }
    issues
}

fn validate_command_classes(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) -> std::collections::BTreeMap<String, super::model::GeneratedWorkflowPermission> {
    let mut permissions = std::collections::BTreeMap::new();
    for (index, command) in policy.command_classes.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.command_classes[{index}]");
        if !valid_id(&command.id)
            || permissions
                .insert(command.id.clone(), command.permission.clone())
                .is_some()
        {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
                format!("{path}.id"),
                "command class IDs must be safe and unique",
            ));
        }
    }
    if permissions.is_empty() {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.command_classes",
            "at least one command class is required",
        ));
    }
    permissions
}

fn validate_descriptors(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    command_permissions: &std::collections::BTreeMap<
        String,
        super::model::GeneratedWorkflowPermission,
    >,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) {
    let mut ids = std::collections::BTreeSet::new();
    for (index, descriptor) in policy.descriptors.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.descriptors[{index}]");
        validate_descriptor_shape(descriptor, &path, policy, &mut ids, issues);
        let mut has_mutation = false;
        for command in &descriptor.command_classes {
            match command_permissions.get(command) {
                Some(super::model::GeneratedWorkflowPermission::Mutation) => has_mutation = true,
                Some(super::model::GeneratedWorkflowPermission::Read) => {}
                None => issues.push(issue(
                    super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                    format!("{path}.command_classes"),
                    format!("unknown command class: {}", diagnostic_label(command)),
                )),
            }
        }
        if descriptor.mutation_class == super::model::GeneratedWorkflowMutationClass::ReadOnly
            && has_mutation
        {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                format!("{path}.mutation_class"),
                "read-only workflow references a mutation command class",
            ));
        }
        if descriptor.id == super::model::GENERATED_AGENT_WORKFLOW_EXPLORE_ID
            && (has_mutation
                || descriptor.mutation_class
                    != super::model::GeneratedWorkflowMutationClass::ReadOnly
                || !descriptor.constraints.iter().any(|constraint| {
                    constraint == super::model::GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE
                }))
        {
            issues.push(issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
                path,
                "explore must be read-only and include the exact no-write constraint",
            ));
        }
    }
    let actual = ids.iter().map(String::as_str).collect::<Vec<_>>();
    if actual != super::model::INITIAL_WORKFLOW_IDS {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.descriptors",
            format!(
                "initial workflow catalog must be: {}",
                super::model::INITIAL_WORKFLOW_IDS.join(", ")
            ),
        ));
    }
}

/// Longest policy-supplied identifier that an admission or lookup diagnostic
/// copies; longer values are truncated with a marker.
const DIAGNOSTIC_LABEL_BYTES: usize = 64;

/// Copies at most [`DIAGNOSTIC_LABEL_BYTES`] of a policy-supplied value into
/// a diagnostic, marking a truncated value with its full length.
fn diagnostic_label(value: &str) -> std::borrow::Cow<'_, str> {
    if value.len() <= DIAGNOSTIC_LABEL_BYTES {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut end = DIAGNOSTIC_LABEL_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!(
        "{}...(truncated, {} bytes)",
        &value[..end],
        value.len()
    ))
}

fn validate_descriptor_shape(
    descriptor: &super::model::GeneratedWorkflowDescriptor,
    path: &str,
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    ids: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) {
    if !valid_id(&descriptor.id)
        || !ids.insert(descriptor.id.clone())
        || descriptor.version < MINIMUM_DESCRIPTOR_VERSION
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
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
        || scalar_text
            .iter()
            .any(|text| contains_legacy_layout_reference(text))
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
            path,
            "workflow descriptor scalar text is empty, out of bounds, or uses a legacy layout",
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
            || values.iter().any(|value| {
                !bounded_text(value, policy.bounds.max_text_bytes)
                    || contains_legacy_layout_reference(value)
            })
    }) || !contains_all(&descriptor.non_claims, &policy.required_non_claims)
    {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESCRIPTOR,
            path,
            "workflow descriptor lists or non-claims are incomplete, out of bounds, or use a legacy layout",
        ));
    }
}

fn validate_adapters(
    policy: &super::model::GeneratedAgentWorkflowPolicy,
    issues: &mut Vec<super::model::GeneratedWorkflowIssue>,
) {
    let mut ids = std::collections::BTreeSet::new();
    for (index, adapter) in policy.adapters.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.adapters[{index}]");
        let expected_shape = match adapter.format {
            super::model::GeneratedWorkflowAdapterFormat::AgentSkills => {
                adapter.front_matter
                    && adapter.file_suffix.is_empty()
                    && adapter.directory_suffix == "/SKILL.md"
            }
            super::model::GeneratedWorkflowAdapterFormat::Claude
            | super::model::GeneratedWorkflowAdapterFormat::Codex => {
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
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER,
                path,
                "adapter identity, version, format, paths, or bounds are invalid",
            ));
        }
    }
    let actual = ids.iter().map(String::as_str).collect::<Vec<_>>();
    if actual != super::model::INITIAL_ADAPTER_IDS {
        issues.push(issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "generated_agent_workflow_policy.adapters",
            format!(
                "initial adapter catalog must be: {}",
                super::model::INITIAL_ADAPTER_IDS.join(", ")
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
        && values
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == values.len()
}

fn contains_required(values: &[String], required: &[&str]) -> bool {
    required
        .iter()
        .all(|required| values.iter().any(|value| value == required))
}

fn contains_all(values: &[String], required: &[String]) -> bool {
    required.iter().all(|required| values.contains(required))
}

pub fn contains_legacy_layout_reference(text: &str) -> bool {
    const MARKER: &str = "cairn/";
    let mut offset = 0;
    while let Some(relative) = text[offset..].find(MARKER) {
        let start = offset + relative;
        let previous = text[..start].chars().next_back();
        if previous.is_none_or(|character| {
            !(character == '.'
                || character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_')
        }) {
            return true;
        }
        offset = start + MARKER.len();
    }
    false
}

pub(super) fn issue(
    code: impl Into<String>,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> super::model::GeneratedWorkflowIssue {
    super::model::GeneratedWorkflowIssue {
        code: code.into(),
        field_path: field_path.into(),
        message: message.into(),
    }
}
