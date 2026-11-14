use super::{
    dependency::{
        configured_marker, marker_prefix, observe_marker, valid_lifecycle_id, MarkerObservation,
    },
    placeholder_line, remove_prefixed_marker, substantive_line_count, ArtifactSubstanceInventory,
    REQUIREMENT_MARKER_PREFIX, REQUIREMENT_MARKER_SUFFIX,
};

const TASKS_ARTIFACT_KIND: &str = "tasks";
const TASK_TODO_PREFIX: &str = "- [ ]";
const TASK_DONE_PREFIX_LOWER: &str = "- [x]";
const TASK_DONE_PREFIX_UPPER: &str = "- [X]";
const TASK_IN_PROGRESS_PREFIX: &str = "- [~]";
const MAX_TASK_ISSUES_PER_LINE: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Done,
    InProgress,
    Todo,
}

pub(super) fn task_inventory(
    snapshot: &super::super::gates::GateSnapshot,
    tasks: &super::super::gates::GateArtifact,
    policy: &crate::verified::policy::Policy,
) -> (ArtifactSubstanceInventory, Vec<String>) {
    let known_references = known_delta_references(snapshot);
    let mut inventory = ArtifactSubstanceInventory::text(
        TASKS_ARTIFACT_KIND,
        &tasks.path,
        substantive_line_count(&tasks.markdown, &policy.artifact_substance_policy),
    );
    let line_count = tasks.markdown.lines().count();
    let issue_capacity = line_count.saturating_mul(MAX_TASK_ISSUES_PER_LINE);
    let mut issues = Vec::with_capacity(issue_capacity);
    let dependency_marker = configured_marker(policy);
    if dependency_marker.is_none() {
        issues.push(format!(
            "{}: lifecycle_validation.dependency_marker_shape.policy: configured dependency marker `{}` is absent from task marker policy",
            tasks.path, policy.artifact_substance_policy.dependency_marker_id
        ));
    }
    observe_task_lines(
        &tasks.path,
        &tasks.markdown,
        &known_references,
        dependency_marker,
        policy,
        &mut inventory,
        &mut issues,
    );
    add_minimum_issue(&tasks.path, policy, &inventory, &mut issues);
    issues.sort();
    issues.dedup();
    (inventory, issues)
}

#[allow(clippy::too_many_arguments)]
fn observe_task_lines(
    path: &str,
    markdown: &str,
    known_references: &std::collections::BTreeSet<String>,
    dependency_marker: Option<&crate::verified::policy::TaskMarker>,
    policy: &crate::verified::policy::Policy,
    inventory: &mut ArtifactSubstanceInventory,
    issues: &mut Vec<String>,
) {
    let mut is_fenced = false;
    let mut is_in_comment = false;
    for (line_index, line) in markdown.lines().enumerate() {
        if super::super::spec::fence_boundary(line) && !is_in_comment {
            is_fenced = !is_fenced;
            continue;
        }
        let visible = super::super::spec::visible_text_without_comments(line, &mut is_in_comment);
        if is_fenced {
            continue;
        }
        observe_task_line(
            path,
            line_index,
            visible.trim_start(),
            known_references,
            dependency_marker,
            policy,
            inventory,
            issues,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_task_line(
    path: &str,
    line_index: usize,
    line: &str,
    known_references: &std::collections::BTreeSet<String>,
    dependency_marker: Option<&crate::verified::policy::TaskMarker>,
    policy: &crate::verified::policy::Policy,
    inventory: &mut ArtifactSubstanceInventory,
    issues: &mut Vec<String>,
) {
    let Some((state, label)) = task_state_and_label(line) else {
        return;
    };
    observe_task_state(state, inventory);
    let dependency = dependency_marker.map_or_else(
        || MarkerObservation {
            is_present: false,
            is_valid: false,
            diagnostics: Vec::new(),
        },
        |marker| observe_marker(line, marker, policy),
    );
    inventory.malformed_dependency_markers += dependency.diagnostics.len();
    issues.extend(dependency.diagnostics.into_iter().map(|diagnostic| {
        format!(
            "{path}:{}: lifecycle_validation.dependency_marker_shape.invalid: {diagnostic}",
            line_index + 1
        )
    }));
    let has_known_reference = requirement_markers(line)
        .iter()
        .any(|reference| known_references.contains(reference));
    let semantic_label = task_semantic_label(label, policy);
    let has_substantive_label = !semantic_label.is_empty()
        && !placeholder_line(
            &semantic_label,
            &policy.artifact_substance_policy.placeholder_tokens,
        );
    let is_linked = has_known_reference || (dependency.is_present && dependency.is_valid);
    add_task_row_issues(path, line_index, has_substantive_label, is_linked, issues);
    if has_substantive_label && is_linked {
        inventory.substantive_tasks += 1;
    }
}

fn observe_task_state(state: TaskState, inventory: &mut ArtifactSubstanceInventory) {
    inventory.checkbox_tasks += 1;
    match state {
        TaskState::Done => inventory.task_done += 1,
        TaskState::InProgress => inventory.task_in_progress += 1,
        TaskState::Todo => inventory.task_todo += 1,
    }
}

fn add_task_row_issues(
    path: &str,
    line_index: usize,
    has_substantive_label: bool,
    is_linked: bool,
    issues: &mut Vec<String>,
) {
    if !has_substantive_label {
        issues.push(format!(
            "{path}:{}: lifecycle_validation.task_substance.placeholder: checkbox task has an empty or placeholder-only label",
            line_index + 1
        ));
    }
    if !is_linked {
        issues.push(format!(
            "{path}:{}: lifecycle_validation.task_substance.linkage: checkbox task requires a known r[...] reference or a valid dependency marker",
            line_index + 1
        ));
    }
}

fn add_minimum_issue(
    path: &str,
    policy: &crate::verified::policy::Policy,
    inventory: &ArtifactSubstanceInventory,
    issues: &mut Vec<String>,
) {
    if inventory.substantive_tasks >= policy.artifact_substance_policy.minimum_substantive_tasks {
        return;
    }
    issues.push(format!(
        "{path}: lifecycle_validation.task_substance.minimum: found {} substantive checkbox task(s), expected at least {}",
        inventory.substantive_tasks,
        policy.artifact_substance_policy.minimum_substantive_tasks
    ));
}

fn task_state_and_label(trimmed: &str) -> Option<(TaskState, &str)> {
    for (prefix, state) in [
        (TASK_TODO_PREFIX, TaskState::Todo),
        (TASK_DONE_PREFIX_LOWER, TaskState::Done),
        (TASK_DONE_PREFIX_UPPER, TaskState::Done),
        (TASK_IN_PROGRESS_PREFIX, TaskState::InProgress),
    ] {
        if let Some(label) = trimmed.strip_prefix(prefix) {
            return Some((state, label.trim()));
        }
    }
    None
}

fn task_semantic_label(label: &str, policy: &crate::verified::policy::Policy) -> String {
    let mut semantic = label.to_string();
    for marker in &policy.task_marker_tokens {
        if marker.kind == "prefix" || marker.kind.ends_with("-prefix") {
            remove_prefixed_marker(&mut semantic, marker_prefix(&marker.token));
        } else {
            semantic = semantic.replace(&marker.token, " ");
        }
    }
    remove_prefixed_marker(&mut semantic, REQUIREMENT_MARKER_PREFIX);
    semantic
        .trim_matches(|character: char| character.is_whitespace() || matches!(character, '-' | ':'))
        .to_string()
}

fn known_delta_references(
    snapshot: &super::super::gates::GateSnapshot,
) -> std::collections::BTreeSet<String> {
    snapshot
        .delta_specs
        .iter()
        .flat_map(|artifact| {
            super::super::spec::visible_requirement_reference_set(&artifact.markdown)
        })
        .collect()
}

fn requirement_markers(markdown: &str) -> Vec<String> {
    let marker_capacity = markdown.matches(REQUIREMENT_MARKER_PREFIX).count();
    let mut markers = Vec::with_capacity(marker_capacity);
    let mut rest = markdown;
    while let Some(start) = rest.find(REQUIREMENT_MARKER_PREFIX) {
        let after_prefix = &rest[start + REQUIREMENT_MARKER_PREFIX.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_MARKER_SUFFIX) else {
            break;
        };
        let candidate = after_prefix[..end].trim();
        if valid_lifecycle_id(candidate) {
            markers.push(candidate.to_string());
        }
        rest = &after_prefix[end + REQUIREMENT_MARKER_SUFFIX.len_utf8()..];
    }
    markers
}
