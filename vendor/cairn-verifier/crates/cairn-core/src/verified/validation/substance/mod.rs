mod dependency;
mod tasks;

pub(crate) use dependency::valid_lifecycle_id;
use tasks::task_inventory;

const PROPOSAL_ARTIFACT_KIND: &str = "proposal";
const DESIGN_ARTIFACT_KIND: &str = "design";
const DELTA_SPEC_ARTIFACT_KIND: &str = "delta_spec";
pub(super) const REQUIREMENT_MARKER_PREFIX: &str = "r[";
pub(super) const REQUIREMENT_MARKER_SUFFIX: char = ']';
const GATE_EXEMPTION_MARKER_PREFIX: &str = "gate-exempt[";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSubstanceInventory {
    pub kind: String,
    pub path: String,
    pub substantive_lines: usize,
    pub requirement_blocks: usize,
    pub substantive_requirement_blocks: usize,
    pub scenario_blocks: usize,
    pub checkbox_tasks: usize,
    pub substantive_tasks: usize,
    pub task_done: usize,
    pub task_in_progress: usize,
    pub task_todo: usize,
    pub malformed_dependency_markers: usize,
}

impl ArtifactSubstanceInventory {
    pub(super) fn text(kind: &str, path: &str, substantive_lines: usize) -> Self {
        Self {
            kind: kind.to_string(),
            path: path.to_string(),
            substantive_lines,
            requirement_blocks: 0,
            substantive_requirement_blocks: 0,
            scenario_blocks: 0,
            checkbox_tasks: 0,
            substantive_tasks: 0,
            task_done: 0,
            task_in_progress: 0,
            task_todo: 0,
            malformed_dependency_markers: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleSubstanceReport {
    pub issues: Vec<String>,
    pub artifacts: Vec<ArtifactSubstanceInventory>,
}

pub(super) fn artifact_substance_issues(
    snapshot: &super::gates::GateSnapshot,
    stage: super::gates::GateStage,
    policy: &crate::verified::policy::Policy,
) -> (
    Vec<super::gates::GateIssue>,
    Vec<ArtifactSubstanceInventory>,
) {
    // r[impl cairn.lifecycle_validation.artifact_substance]
    // r[impl cairn.lifecycle_validation.task_substance]
    // r[impl cairn.lifecycle_validation.dependency_marker_shape]
    // r[impl cairn.lifecycle_validation.task_progress_non_authority]
    let mut issues = Vec::new();
    let mut inventories = Vec::new();
    add_proposal(snapshot, policy, &mut issues, &mut inventories);
    if matches!(
        stage,
        super::gates::GateStage::Design | super::gates::GateStage::Tasks
    ) {
        add_design(snapshot, policy, &mut issues, &mut inventories);
        add_delta_specs(snapshot, policy, &mut issues, &mut inventories);
    }
    if matches!(stage, super::gates::GateStage::Tasks) {
        add_tasks(snapshot, policy, &mut issues, &mut inventories);
    }
    issues.sort_by(|left, right| left.message.cmp(&right.message));
    issues.dedup_by(|left, right| left.message == right.message);
    inventories.sort_by(|left, right| left.path.cmp(&right.path).then(left.kind.cmp(&right.kind)));
    (issues, inventories)
}

fn add_proposal(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
    issues: &mut Vec<super::gates::GateIssue>,
    inventories: &mut Vec<ArtifactSubstanceInventory>,
) {
    let Some(proposal) = &snapshot.proposal else {
        return;
    };
    let substance_policy = &policy.artifact_substance_policy;
    let inventory = text_inventory(
        PROPOSAL_ARTIFACT_KIND,
        &proposal.path,
        &proposal.markdown,
        substance_policy,
    );
    if inventory.substantive_lines < substance_policy.proposal_minimum_substantive_lines {
        issues.push(super::gates::gate_issue(
            format!(
                "{}: lifecycle_validation.artifact_substance.proposal: found {} substantive line(s), expected at least {} after placeholder filtering",
                proposal.path,
                inventory.substantive_lines,
                substance_policy.proposal_minimum_substantive_lines
            ),
            policy,
        ));
    }
    inventories.push(inventory);
}

fn add_design(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
    issues: &mut Vec<super::gates::GateIssue>,
    inventories: &mut Vec<ArtifactSubstanceInventory>,
) {
    let Some(design) = &snapshot.design else {
        return;
    };
    let substance_policy = &policy.artifact_substance_policy;
    let inventory = text_inventory(
        DESIGN_ARTIFACT_KIND,
        &design.path,
        &design.markdown,
        substance_policy,
    );
    if inventory.substantive_lines < substance_policy.design_minimum_substantive_lines {
        issues.push(super::gates::gate_issue(
            format!(
                "{}: lifecycle_validation.artifact_substance.design: found {} substantive line(s), expected at least {} after placeholder filtering",
                design.path,
                inventory.substantive_lines,
                substance_policy.design_minimum_substantive_lines
            ),
            policy,
        ));
    }
    inventories.push(inventory);
}

fn add_delta_specs(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
    issues: &mut Vec<super::gates::GateIssue>,
    inventories: &mut Vec<ArtifactSubstanceInventory>,
) {
    for delta_spec in &snapshot.delta_specs {
        let source = crate::verified::repo::SpecSource {
            id: delta_spec.path.clone(),
            path: delta_spec.path.clone(),
            markdown: delta_spec.markdown.clone(),
        };
        let (spec_inventory, spec_issues) = super::spec::validate_spec_source_with_policy(
            &source,
            &policy.artifact_substance_policy,
        );
        inventories.push(ArtifactSubstanceInventory {
            kind: DELTA_SPEC_ARTIFACT_KIND.to_string(),
            path: spec_inventory.path,
            substantive_lines: substantive_line_count(
                &delta_spec.markdown,
                &policy.artifact_substance_policy,
            ),
            requirement_blocks: spec_inventory.requirement_blocks,
            substantive_requirement_blocks: spec_inventory.substantive_requirement_blocks,
            scenario_blocks: spec_inventory.scenario_blocks,
            checkbox_tasks: 0,
            substantive_tasks: 0,
            task_done: 0,
            task_in_progress: 0,
            task_todo: 0,
            malformed_dependency_markers: 0,
        });
        issues.extend(
            spec_issues
                .into_iter()
                .map(|message| super::gates::gate_issue(message, policy)),
        );
    }
}

fn add_tasks(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
    issues: &mut Vec<super::gates::GateIssue>,
    inventories: &mut Vec<ArtifactSubstanceInventory>,
) {
    let Some(tasks) = &snapshot.tasks else {
        return;
    };
    let (inventory, task_issues) = task_inventory(snapshot, tasks, policy);
    issues.extend(
        task_issues
            .into_iter()
            .map(|message| super::gates::gate_issue(message, policy)),
    );
    inventories.push(inventory);
}

pub fn evaluate_lifecycle_substance(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
) -> LifecycleSubstanceReport {
    let (issues, artifacts) =
        artifact_substance_issues(snapshot, super::gates::GateStage::Tasks, policy);
    LifecycleSubstanceReport {
        issues: issues.into_iter().map(|issue| issue.message).collect(),
        artifacts,
    }
}

pub fn lifecycle_substance_issues(
    snapshot: &super::gates::GateSnapshot,
    policy: &crate::verified::policy::Policy,
) -> Vec<String> {
    evaluate_lifecycle_substance(snapshot, policy).issues
}

fn text_inventory(
    kind: &str,
    path: &str,
    markdown: &str,
    policy: &crate::verified::policy::ArtifactSubstancePolicy,
) -> ArtifactSubstanceInventory {
    ArtifactSubstanceInventory::text(kind, path, substantive_line_count(markdown, policy))
}

pub(super) fn substantive_line_count(
    markdown: &str,
    policy: &crate::verified::policy::ArtifactSubstancePolicy,
) -> usize {
    let mut count = 0;
    let mut is_fenced = false;
    let mut is_in_comment = false;
    for line in markdown.lines() {
        if super::spec::fence_boundary(line) && !is_in_comment {
            is_fenced = !is_fenced;
            continue;
        }
        let visible = super::spec::visible_text_without_comments(line, &mut is_in_comment);
        let trimmed = visible.trim();
        if is_fenced
            || trimmed.is_empty()
            || trimmed.starts_with('#')
            || placeholder_line(trimmed, &policy.placeholder_tokens)
        {
            continue;
        }
        let semantic = line_semantic_text(trimmed);
        if semantic.bytes().any(|byte| byte.is_ascii_alphanumeric()) {
            count += 1;
        }
    }
    count
}

fn line_semantic_text(line: &str) -> String {
    let mut semantic = line.to_string();
    remove_prefixed_marker(&mut semantic, REQUIREMENT_MARKER_PREFIX);
    remove_prefixed_marker(&mut semantic, GATE_EXEMPTION_MARKER_PREFIX);
    semantic
}

pub(super) fn placeholder_line(line: &str, placeholder_tokens: &[String]) -> bool {
    let normalized = line.to_ascii_lowercase();
    placeholder_tokens
        .iter()
        .any(|token| normalized.contains(&token.to_ascii_lowercase()))
}

pub(super) fn remove_prefixed_marker(text: &mut String, prefix: &str) {
    loop {
        let Some(start) = text.find(prefix) else {
            return;
        };
        let Some(relative_end) = text[start + prefix.len()..].find(REQUIREMENT_MARKER_SUFFIX)
        else {
            text.replace_range(start.., " ");
            return;
        };
        let end = start + prefix.len() + relative_end + REQUIREMENT_MARKER_SUFFIX.len_utf8();
        text.replace_range(start..end, " ");
    }
}
