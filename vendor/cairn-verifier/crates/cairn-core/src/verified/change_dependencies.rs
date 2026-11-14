use std::collections::{BTreeMap, BTreeSet};

use crate::verified::{
    dag::topological_order,
    lifecycle::validate_change_name,
    repo::{ChangeMetadata, ChangeStatus},
};

pub const DEPENDENCY_ISSUE_DUPLICATE: &str = "change_dependency.duplicate_dependency";
pub const DEPENDENCY_ISSUE_UNKNOWN: &str = "change_dependency.unknown_dependency";
pub const DEPENDENCY_ISSUE_SELF: &str = "change_dependency.self_dependency";
pub const DEPENDENCY_ISSUE_CYCLE: &str = "change_dependency.cycle";
pub const DEPENDENCY_ISSUE_INVALID_TYPE: &str =
    "change_dependency.metadata.depends_on.invalid_type";
pub const DEPENDENCY_ISSUE_INVALID_ITEM: &str =
    "change_dependency.metadata.depends_on.invalid_item";
pub const DEPENDENCY_ISSUE_EMPTY_ID: &str = "change_dependency.metadata.depends_on.empty_id";
pub const DEPENDENCY_ISSUE_INVALID_ID: &str = "change_dependency.metadata.depends_on.invalid_id";
pub const DEPENDENCY_BLOCKER_UNSATISFIED: &str = "change_dependency.unsatisfied_dependency";
pub const DEPENDENCY_EDIT_ISSUE_CHANGE_UNKNOWN: &str = "change_dependency.authoring.unknown_change";
pub const DEPENDENCY_EDIT_ISSUE_METADATA_INVALID: &str =
    "change_dependency.authoring.invalid_metadata";
pub const DEPENDENCY_EDIT_ISSUE_NOT_DECLARED: &str = "change_dependency.authoring.not_declared";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDependencyEditAction {
    Add,
    Remove,
}

impl ChangeDependencyEditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyEditPlan {
    pub action: ChangeDependencyEditAction,
    pub change: String,
    pub dependency: String,
    pub valid: bool,
    pub changed: bool,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub issues: Vec<ChangeDependencyIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyBlockedChange {
    pub change: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyNextReady {
    pub ready: Option<String>,
    pub blocked: Vec<ChangeDependencyBlockedChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyGraph {
    pub valid: bool,
    pub nodes: Vec<ChangeDependencyNode>,
    pub edges: Vec<ChangeDependencyEdge>,
    pub issues: Vec<ChangeDependencyIssue>,
    pub drain_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyNode {
    pub name: String,
    pub dependencies: Vec<String>,
    pub satisfied_dependencies: Vec<String>,
    pub blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    pub dependency_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyEdge {
    pub dependent: String,
    pub dependency: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDependencyIssue {
    pub change: String,
    pub code: String,
    pub dependency: Option<String>,
    pub message: String,
}

struct GraphBuildContext {
    known_set: BTreeSet<String>,
    active_set: BTreeSet<String>,
    archived_set: BTreeSet<String>,
    nodes: BTreeMap<String, ChangeDependencyNode>,
    edges: Vec<ChangeDependencyEdge>,
    topological_edges: Vec<(String, String)>,
    issues: Vec<ChangeDependencyIssue>,
}

pub fn normalized_change_dependencies(change: &ChangeStatus) -> Vec<String> {
    let mut dependencies = change.metadata.depends_on.clone();
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

pub fn plan_change_dependency_edit(
    changes: &[ChangeStatus],
    archived_changes: &[String],
    change_name: &str,
    dependency: &str,
    action: ChangeDependencyEditAction,
) -> ChangeDependencyEditPlan {
    // r[impl cairn.change_dependency_graph.authoring_cli]
    // r[impl cairn.change_dependency_graph.authoring_cli.invalid]
    let mut issues = Vec::new();
    let Some(change) = changes
        .iter()
        .find(|candidate| candidate.name == change_name)
    else {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_EDIT_ISSUE_CHANGE_UNKNOWN.to_string(),
            dependency: None,
            message: format!("change {change_name} is not active"),
        });
        return dependency_edit_plan(
            action,
            change_name,
            dependency,
            Vec::new(),
            Vec::new(),
            issues,
        );
    };

    let before = normalized_change_dependencies(change);
    for metadata_issue in &change.metadata_issues {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_EDIT_ISSUE_METADATA_INVALID.to_string(),
            dependency: None,
            message: metadata_issue.clone(),
        });
    }
    if let Err(error) = validate_change_name(dependency) {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_INVALID_ID.to_string(),
            dependency: Some(dependency.to_string()),
            message: format!("invalid dependency id {dependency}: {error}"),
        });
    }
    if dependency == change_name {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_SELF.to_string(),
            dependency: Some(dependency.to_string()),
            message: format!("change {change_name} depends on itself"),
        });
    }

    let known_changes = known_change_names_for_status(changes, archived_changes);
    if !known_changes.contains(dependency) {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_UNKNOWN.to_string(),
            dependency: Some(dependency.to_string()),
            message: format!("dependency {dependency} is not active or archived"),
        });
    }

    match action {
        ChangeDependencyEditAction::Add => {
            if before.iter().any(|existing| existing == dependency) {
                issues.push(ChangeDependencyIssue {
                    change: change_name.to_string(),
                    code: DEPENDENCY_ISSUE_DUPLICATE.to_string(),
                    dependency: Some(dependency.to_string()),
                    message: format!("dependency {dependency} is already declared"),
                });
            }
        }
        ChangeDependencyEditAction::Remove => {
            if !before.iter().any(|existing| existing == dependency) {
                issues.push(ChangeDependencyIssue {
                    change: change_name.to_string(),
                    code: DEPENDENCY_EDIT_ISSUE_NOT_DECLARED.to_string(),
                    dependency: Some(dependency.to_string()),
                    message: format!("dependency {dependency} is not declared"),
                });
            }
        }
    }

    let current_graph = build_change_dependency_graph(changes, archived_changes);
    issues.extend(
        current_graph
            .issues
            .into_iter()
            .filter(|issue| issue.change == change_name),
    );

    issues.sort_by(|left, right| {
        left.change
            .cmp(&right.change)
            .then(left.code.cmp(&right.code))
            .then(left.dependency.cmp(&right.dependency))
    });
    issues.dedup_by(|left, right| {
        left.change == right.change
            && left.code == right.code
            && left.dependency == right.dependency
    });
    if !issues.is_empty() {
        return dependency_edit_plan(
            action,
            change_name,
            dependency,
            before.clone(),
            before,
            issues,
        );
    }

    let after = apply_dependency_edit(&before, dependency, action);
    let candidate_changes = changes_with_dependency_update(changes, change_name, &after);
    let candidate_graph = build_change_dependency_graph(&candidate_changes, archived_changes);
    let mut candidate_issues = candidate_graph
        .issues
        .into_iter()
        .filter(|issue| issue.change == change_name)
        .collect::<Vec<_>>();
    candidate_issues.sort_by(|left, right| {
        left.change
            .cmp(&right.change)
            .then(left.code.cmp(&right.code))
            .then(left.dependency.cmp(&right.dependency))
    });
    if !candidate_issues.is_empty() {
        return dependency_edit_plan(
            action,
            change_name,
            dependency,
            before.clone(),
            before,
            candidate_issues,
        );
    }

    dependency_edit_plan(action, change_name, dependency, before, after, Vec::new())
}

pub fn change_metadata_with_dependencies(
    change: &ChangeStatus,
    depends_on: Vec<String>,
) -> ChangeMetadata {
    let mut dependencies = depends_on;
    dependencies.sort();
    dependencies.dedup();
    ChangeMetadata {
        groups: change.metadata.groups.clone(),
        target_date: change.metadata.target_date.clone(),
        status: change.metadata.status.clone(),
        depends_on: dependencies,
        cross_repo_dependencies: change.metadata.cross_repo_dependencies.clone(),
    }
}

pub fn render_change_metadata_json(metadata: &ChangeMetadata) -> Result<String, serde_json::Error> {
    // r[impl cairn.change_dependency_graph.authoring_cli]
    let mut object = serde_json::Map::new();
    if !metadata.groups.is_empty() {
        object.insert("groups".to_string(), serde_json::json!(metadata.groups));
    }
    if let Some(target_date) = &metadata.target_date {
        object.insert("target_date".to_string(), serde_json::json!(target_date));
    }
    if let Some(status) = &metadata.status {
        object.insert("status".to_string(), serde_json::json!(status));
    }
    if !metadata.depends_on.is_empty() {
        object.insert(
            "depends_on".to_string(),
            serde_json::json!(metadata.depends_on),
        );
    }
    if !metadata.cross_repo_dependencies.is_empty() {
        object.insert(
            "cross_repo_dependencies".to_string(),
            serde_json::json!(metadata.cross_repo_dependencies),
        );
    }
    let mut text = serde_json::to_string_pretty(&serde_json::Value::Object(object))?;
    text.push('\n');
    Ok(text)
}

pub fn next_ready_change(graph: &ChangeDependencyGraph) -> ChangeDependencyNextReady {
    // r[impl cairn.change_dependency_graph.next_ready]
    let ready = graph
        .drain_order
        .iter()
        .filter_map(|name| graph.node(name))
        .find(|node| node.dependency_ready)
        .map(|node| node.name.clone());
    let blocked = graph
        .nodes
        .iter()
        .filter(|node| !node.dependency_ready)
        .map(|node| ChangeDependencyBlockedChange {
            change: node.name.clone(),
            blockers: dependency_blockers_for_change(graph, &node.name),
        })
        .collect();
    ChangeDependencyNextReady { ready, blocked }
}

pub fn render_change_dependency_graph_mermaid(graph: &ChangeDependencyGraph) -> String {
    // r[impl cairn.change_dependency_graph.render_formats]
    let names = graph_render_names(graph);
    let ids = graph_node_ids(&names);
    let mut lines = vec!["flowchart TD".to_string()];
    for name in &names {
        if let Some(id) = ids.get(name) {
            lines.push(format!("  {id}[\"{}\"]", escape_graph_label(name)));
        }
    }
    for edge in sorted_graph_edges(graph) {
        if let (Some(dependency_id), Some(dependent_id)) =
            (ids.get(&edge.dependency), ids.get(&edge.dependent))
        {
            let label = if edge.satisfied {
                "satisfied archived"
            } else {
                "active blocker"
            };
            lines.push(format!("  {dependency_id} -->|\"{label}\"| {dependent_id}"));
        }
    }
    render_lines(lines)
}

pub fn render_change_dependency_graph_dot(graph: &ChangeDependencyGraph) -> String {
    // r[impl cairn.change_dependency_graph.render_formats]
    let mut lines = vec!["digraph change_dependencies {".to_string()];
    for name in graph_render_names(graph) {
        lines.push(format!("  \"{}\";", escape_graph_label(&name)));
    }
    for edge in sorted_graph_edges(graph) {
        let label = if edge.satisfied {
            "satisfied archived"
        } else {
            "active blocker"
        };
        lines.push(format!(
            "  \"{}\" -> \"{}\" [label=\"{label}\"];",
            escape_graph_label(&edge.dependency),
            escape_graph_label(&edge.dependent)
        ));
    }
    lines.push("}".to_string());
    render_lines(lines)
}

fn dependency_edit_plan(
    action: ChangeDependencyEditAction,
    change: &str,
    dependency: &str,
    before: Vec<String>,
    after: Vec<String>,
    issues: Vec<ChangeDependencyIssue>,
) -> ChangeDependencyEditPlan {
    ChangeDependencyEditPlan {
        action,
        change: change.to_string(),
        dependency: dependency.to_string(),
        valid: issues.is_empty(),
        changed: before != after,
        before,
        after,
        issues,
    }
}

fn known_change_names_for_status(
    changes: &[ChangeStatus],
    archived_changes: &[String],
) -> BTreeSet<String> {
    let active_set = sorted_active_names(changes)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let archived_set = sorted_archived_names(archived_changes)
        .into_iter()
        .collect::<BTreeSet<_>>();
    known_change_names(&active_set, &archived_set)
}

fn apply_dependency_edit(
    before: &[String],
    dependency: &str,
    action: ChangeDependencyEditAction,
) -> Vec<String> {
    let mut dependencies = before.to_vec();
    match action {
        ChangeDependencyEditAction::Add => dependencies.push(dependency.to_string()),
        ChangeDependencyEditAction::Remove => {
            dependencies.retain(|existing| existing != dependency);
        }
    }
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn changes_with_dependency_update(
    changes: &[ChangeStatus],
    change_name: &str,
    dependencies: &[String],
) -> Vec<ChangeStatus> {
    changes
        .iter()
        .map(|change| {
            let mut updated = change.clone();
            if updated.name == change_name {
                updated.metadata.depends_on = dependencies.to_vec();
            }
            updated
        })
        .collect()
}

fn graph_render_names(graph: &ChangeDependencyGraph) -> Vec<String> {
    let mut names = BTreeSet::new();
    for node in &graph.nodes {
        names.insert(node.name.clone());
    }
    for edge in &graph.edges {
        names.insert(edge.dependent.clone());
        names.insert(edge.dependency.clone());
    }
    names.into_iter().collect()
}

fn graph_node_ids(names: &[String]) -> BTreeMap<String, String> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), format!("n{index}")))
        .collect()
}

fn sorted_graph_edges(graph: &ChangeDependencyGraph) -> Vec<ChangeDependencyEdge> {
    let mut edges = graph.edges.clone();
    edges.sort_by(|left, right| {
        left.dependency
            .cmp(&right.dependency)
            .then(left.dependent.cmp(&right.dependent))
            .then(left.satisfied.cmp(&right.satisfied))
    });
    edges
}

fn escape_graph_label(value: &str) -> String {
    value.replace('"', "\\\"")
}

fn render_lines(lines: Vec<String>) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

pub fn build_change_dependency_graph(
    changes: &[ChangeStatus],
    archived_changes: &[String],
) -> ChangeDependencyGraph {
    // r[impl cairn.change_dependency_graph.graph_validation]
    // r[impl cairn.change_dependency_graph.trellis_primitives]
    // r[impl cairn.change_dependency_graph.trellis_primitives.portable]
    let active_names = sorted_active_names(changes);
    let active_set = active_names.iter().cloned().collect::<BTreeSet<_>>();
    let archived_set = sorted_archived_names(archived_changes)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let known_set = known_change_names(&active_set, &archived_set);
    let mut context = GraphBuildContext {
        known_set,
        active_set,
        archived_set,
        nodes: initial_nodes(&active_names),
        edges: Vec::new(),
        topological_edges: Vec::new(),
        issues: Vec::new(),
    };

    for change in sorted_changes(changes) {
        let declared_dependencies = sorted_dependencies(&change.metadata.depends_on);
        let unique_dependencies =
            unique_dependencies(&declared_dependencies, &change.name, &mut context.issues);
        for dependency in unique_dependencies {
            record_dependency_edge(&change.name, &dependency, &mut context);
        }
    }

    sort_node_fields(&mut context.nodes);
    context.edges.sort_by(|left, right| {
        left.dependent
            .cmp(&right.dependent)
            .then(left.dependency.cmp(&right.dependency))
    });

    let topological = topological_order(&active_names, &context.topological_edges);
    for cyclic_node in &topological.blocked_nodes {
        context.issues.push(ChangeDependencyIssue {
            change: cyclic_node.clone(),
            code: DEPENDENCY_ISSUE_CYCLE.to_string(),
            dependency: None,
            message: format!("change {cyclic_node} is in a dependency cycle"),
        });
    }
    context.issues.sort_by(|left, right| {
        left.change
            .cmp(&right.change)
            .then(left.code.cmp(&right.code))
            .then(left.dependency.cmp(&right.dependency))
    });

    mark_dependency_readiness(&mut context.nodes, &context.issues);
    let nodes = context.nodes.into_values().collect::<Vec<_>>();

    ChangeDependencyGraph {
        valid: context.issues.is_empty(),
        nodes,
        edges: context.edges,
        issues: context.issues,
        drain_order: topological.order,
    }
}

pub fn change_dependency_issue_messages(graph: &ChangeDependencyGraph) -> Vec<String> {
    graph
        .issues
        .iter()
        .map(change_dependency_issue_message)
        .collect()
}

pub fn dependency_blockers_for_change(
    graph: &ChangeDependencyGraph,
    change_name: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if let Some(node) = graph.nodes.iter().find(|node| node.name == change_name) {
        for dependency in &node.blocked_by {
            blockers.push(format!(
                "{change_name}: {DEPENDENCY_BLOCKER_UNSATISFIED}: dependency {dependency} is not archived"
            ));
        }
    }
    blockers.extend(
        graph
            .issues
            .iter()
            .filter(|issue| issue.change == change_name)
            .map(change_dependency_issue_message),
    );
    blockers.sort();
    blockers.dedup();
    blockers
}

fn change_dependency_issue_message(issue: &ChangeDependencyIssue) -> String {
    format!("{}: {}: {}", issue.change, issue.code, issue.message)
}

fn sorted_active_names(changes: &[ChangeStatus]) -> Vec<String> {
    let mut names = changes
        .iter()
        .map(|change| change.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn sorted_archived_names(archived_changes: &[String]) -> Vec<String> {
    let mut names = archived_changes.to_vec();
    names.sort();
    names.dedup();
    names
}

fn known_change_names(
    active_set: &BTreeSet<String>,
    archived_set: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut known = active_set.clone();
    known.extend(archived_set.iter().cloned());
    known
}

fn initial_nodes(active_names: &[String]) -> BTreeMap<String, ChangeDependencyNode> {
    active_names
        .iter()
        .map(|name| {
            (
                name.clone(),
                ChangeDependencyNode {
                    name: name.clone(),
                    dependencies: Vec::new(),
                    satisfied_dependencies: Vec::new(),
                    blocked_by: Vec::new(),
                    blocks: Vec::new(),
                    dependency_ready: true,
                },
            )
        })
        .collect()
}

fn sorted_changes(changes: &[ChangeStatus]) -> Vec<&ChangeStatus> {
    let mut sorted = changes.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.name.cmp(&right.name));
    sorted
}

fn sorted_dependencies(dependencies: &[String]) -> Vec<String> {
    let mut sorted = dependencies.to_vec();
    sorted.sort();
    sorted
}

fn unique_dependencies(
    sorted_dependencies: &[String],
    change_name: &str,
    issues: &mut Vec<ChangeDependencyIssue>,
) -> Vec<String> {
    let mut unique = Vec::new();
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for dependency in sorted_dependencies {
        if !seen.insert(dependency.clone()) {
            duplicates.insert(dependency.clone());
            continue;
        }
        unique.push(dependency.clone());
    }
    for dependency in duplicates {
        issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_DUPLICATE.to_string(),
            dependency: Some(dependency.clone()),
            message: format!("dependency {dependency} is declared more than once"),
        });
    }
    unique
}

fn record_dependency_edge(change_name: &str, dependency: &str, context: &mut GraphBuildContext) {
    if dependency == change_name {
        context.issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_SELF.to_string(),
            dependency: Some(dependency.to_string()),
            message: format!("change {change_name} depends on itself"),
        });
        return;
    }
    if !context.known_set.contains(dependency) {
        context.issues.push(ChangeDependencyIssue {
            change: change_name.to_string(),
            code: DEPENDENCY_ISSUE_UNKNOWN.to_string(),
            dependency: Some(dependency.to_string()),
            message: format!("dependency {dependency} is not active or archived"),
        });
        return;
    }

    let is_archived = context.archived_set.contains(dependency);
    context.edges.push(ChangeDependencyEdge {
        dependent: change_name.to_string(),
        dependency: dependency.to_string(),
        satisfied: is_archived,
    });
    if let Some(node) = context.nodes.get_mut(change_name) {
        node.dependencies.push(dependency.to_string());
        if is_archived {
            node.satisfied_dependencies.push(dependency.to_string());
        }
    }
    if context.active_set.contains(dependency) {
        if let Some(node) = context.nodes.get_mut(change_name) {
            node.blocked_by.push(dependency.to_string());
        }
        if let Some(dependency_node) = context.nodes.get_mut(dependency) {
            dependency_node.blocks.push(change_name.to_string());
        }
        context
            .topological_edges
            .push((dependency.to_string(), change_name.to_string()));
    }
}

fn sort_node_fields(nodes: &mut BTreeMap<String, ChangeDependencyNode>) {
    for node in nodes.values_mut() {
        node.dependencies.sort();
        node.dependencies.dedup();
        node.satisfied_dependencies.sort();
        node.satisfied_dependencies.dedup();
        node.blocked_by.sort();
        node.blocked_by.dedup();
        node.blocks.sort();
        node.blocks.dedup();
    }
}

fn mark_dependency_readiness(
    nodes: &mut BTreeMap<String, ChangeDependencyNode>,
    issues: &[ChangeDependencyIssue],
) {
    let issue_changes = issues
        .iter()
        .map(|issue| issue.change.clone())
        .collect::<BTreeSet<_>>();
    for node in nodes.values_mut() {
        node.dependency_ready = node.blocked_by.is_empty() && !issue_changes.contains(&node.name);
    }
}

impl ChangeDependencyGraph {
    pub fn node(&self, name: &str) -> Option<&ChangeDependencyNode> {
        self.nodes.iter().find(|node| node.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verified::repo::{ChangeMetadata, TaskCounts};

    fn change(name: &str, depends_on: &[&str]) -> ChangeStatus {
        ChangeStatus {
            name: name.to_string(),
            path: format!(".cairn/changes/{name}"),
            has_proposal: true,
            has_design: true,
            has_tasks: true,
            delta_specs: vec![format!(".cairn/changes/{name}/specs/demo/spec.md")],
            task_counts: Some(TaskCounts {
                total: TASK_TOTAL,
                done: TASK_TOTAL,
                in_progress: 0,
                todo: 0,
                unmarked: 0,
            }),
            metadata: ChangeMetadata {
                groups: Vec::new(),
                target_date: None,
                status: None,
                depends_on: depends_on
                    .iter()
                    .map(|dependency| (*dependency).to_string())
                    .collect(),
                cross_repo_dependencies: Vec::new(),
            },
            metadata_issues: Vec::new(),
        }
    }

    const TASK_TOTAL: usize = 2;

    #[test]
    fn empty_graph_is_ready() {
        // r[verify cairn.change_dependency_graph.positive_negative_tests]
        // r[verify cairn.change_dependency_graph.positive_negative_tests.positive]
        let graph = build_change_dependency_graph(&[change("alpha", &[])], &[]);
        assert!(graph.valid);
        assert_eq!(graph.drain_order, vec!["alpha".to_string()]);
        assert!(graph
            .node("alpha")
            .is_some_and(|node| node.dependency_ready));
        assert!(graph.issues.is_empty());
    }

    #[test]
    fn archived_dependencies_are_satisfied() {
        // r[verify cairn.change_dependency_graph.positive_negative_tests]
        // r[verify cairn.change_dependency_graph.positive_negative_tests.positive]
        // r[verify cairn.change_dependency_graph.drain_order.archived]
        let graph =
            build_change_dependency_graph(&[change("beta", &["alpha"])], &["alpha".to_string()]);
        assert!(graph.valid);
        let node = graph.node("beta");
        assert!(node.is_some_and(|node| node.blocked_by.is_empty()));
        assert!(graph.edges[0].satisfied);
        assert_eq!(graph.drain_order, vec!["beta".to_string()]);
    }

    #[test]
    fn active_dependencies_order_before_dependents() {
        // r[verify cairn.change_dependency_graph.positive_negative_tests]
        // r[verify cairn.change_dependency_graph.positive_negative_tests.positive]
        // r[verify cairn.change_dependency_graph.graph_validation.acyclic]
        let graph =
            build_change_dependency_graph(&[change("beta", &["alpha"]), change("alpha", &[])], &[]);
        assert!(graph.valid);
        assert_eq!(
            graph.drain_order,
            vec!["alpha".to_string(), "beta".to_string()]
        );
        let beta = graph.node("beta");
        assert!(beta.is_some_and(|node| node.blocked_by == vec!["alpha".to_string()]));
        let alpha = graph.node("alpha");
        assert!(alpha.is_some_and(|node| node.blocks == vec!["beta".to_string()]));
    }

    #[test]
    fn invalid_edges_report_issue_codes() {
        // r[verify cairn.change_dependency_graph.positive_negative_tests]
        // r[verify cairn.change_dependency_graph.positive_negative_tests.negative]
        let graph = build_change_dependency_graph(
            &[
                change("alpha", &["missing", "missing"]),
                change("beta", &["beta"]),
            ],
            &[],
        );
        assert!(!graph.valid);
        let codes = graph
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&DEPENDENCY_ISSUE_DUPLICATE));
        assert!(codes.contains(&DEPENDENCY_ISSUE_UNKNOWN));
        assert!(codes.contains(&DEPENDENCY_ISSUE_SELF));
    }

    #[test]
    fn cycles_are_rejected_without_ready_nodes() {
        // r[verify cairn.change_dependency_graph.positive_negative_tests]
        // r[verify cairn.change_dependency_graph.positive_negative_tests.negative]
        // r[verify cairn.change_dependency_graph.graph_validation.cycle]
        let graph = build_change_dependency_graph(
            &[change("alpha", &["beta"]), change("beta", &["alpha"])],
            &[],
        );
        assert!(!graph.valid);
        assert!(graph.drain_order.is_empty());
        let codes = graph
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.iter().all(|code| *code == DEPENDENCY_ISSUE_CYCLE));
        assert!(graph
            .node("alpha")
            .is_some_and(|node| !node.dependency_ready));
    }

    #[test]
    fn dependency_edit_add_and_remove_are_sorted_and_metadata_safe() {
        // r[verify cairn.change_dependency_graph.authoring_cli.tests.positive]
        // r[verify cairn.change_dependency_graph.authoring_cli]
        // r[verify cairn.change_dependency_graph.authoring_cli.execute]
        // r[verify cairn.change_dependency_graph.authoring_cli.remove]
        let changes = [
            change("gamma", &[]),
            change("alpha", &[]),
            change("beta", &[]),
        ];
        let add = plan_change_dependency_edit(
            &changes,
            &[],
            "gamma",
            "alpha",
            ChangeDependencyEditAction::Add,
        );

        assert!(add.valid, "{:?}", add.issues);
        assert!(add.changed);
        assert_eq!(add.after, vec!["alpha".to_string()]);

        let metadata = change_metadata_with_dependencies(&changes[0], add.after);
        let rendered = render_change_metadata_json(&metadata).expect("metadata renders");
        assert!(rendered.contains("depends_on"));
        assert!(!rendered.contains("[]"));

        let changes = [change("gamma", &["alpha"]), change("alpha", &[])];
        let remove = plan_change_dependency_edit(
            &changes,
            &[],
            "gamma",
            "alpha",
            ChangeDependencyEditAction::Remove,
        );
        assert!(remove.valid, "{:?}", remove.issues);
        assert!(remove.changed);
        assert!(remove.after.is_empty());
        let metadata = change_metadata_with_dependencies(&changes[0], remove.after);
        let rendered = render_change_metadata_json(&metadata).expect("metadata renders");
        assert!(!rendered.contains("depends_on"));
    }

    #[test]
    fn dependency_edit_rejects_duplicate_unknown_self_and_cycles() {
        // r[verify cairn.change_dependency_graph.authoring_cli.tests.negative]
        // r[verify cairn.change_dependency_graph.authoring_cli.invalid]
        let changes = [change("alpha", &["beta"]), change("beta", &[])];
        let duplicate = plan_change_dependency_edit(
            &changes,
            &[],
            "alpha",
            "beta",
            ChangeDependencyEditAction::Add,
        );
        assert!(!duplicate.valid);
        assert!(duplicate
            .issues
            .iter()
            .any(|issue| issue.code == DEPENDENCY_ISSUE_DUPLICATE));

        let unknown = plan_change_dependency_edit(
            &changes,
            &[],
            "alpha",
            "missing",
            ChangeDependencyEditAction::Add,
        );
        assert!(!unknown.valid);
        assert!(unknown
            .issues
            .iter()
            .any(|issue| issue.code == DEPENDENCY_ISSUE_UNKNOWN));

        let self_dependency = plan_change_dependency_edit(
            &changes,
            &[],
            "alpha",
            "alpha",
            ChangeDependencyEditAction::Add,
        );
        assert!(!self_dependency.valid);
        assert!(self_dependency
            .issues
            .iter()
            .any(|issue| issue.code == DEPENDENCY_ISSUE_SELF));

        let cycle = plan_change_dependency_edit(
            &changes,
            &[],
            "beta",
            "alpha",
            ChangeDependencyEditAction::Add,
        );
        assert!(!cycle.valid);
        assert!(cycle
            .issues
            .iter()
            .any(|issue| issue.code == DEPENDENCY_ISSUE_CYCLE));
    }

    #[test]
    fn graph_rendering_and_next_ready_are_deterministic() {
        // r[verify cairn.change_dependency_graph.authoring_cli.tests.positive]
        // r[verify cairn.change_dependency_graph.render_formats]
        // r[verify cairn.change_dependency_graph.render_formats.human]
        // r[verify cairn.change_dependency_graph.next_ready]
        // r[verify cairn.change_dependency_graph.next_ready.ready]
        let graph =
            build_change_dependency_graph(&[change("beta", &["alpha"]), change("alpha", &[])], &[]);
        let mermaid = render_change_dependency_graph_mermaid(&graph);
        let dot = render_change_dependency_graph_dot(&graph);
        assert!(mermaid.contains("flowchart TD"));
        assert!(mermaid.contains("active blocker"));
        assert!(dot.contains("digraph change_dependencies"));
        assert!(dot.contains("active blocker"));

        let next = next_ready_change(&graph);
        assert_eq!(next.ready, Some("alpha".to_string()));
        assert_eq!(next.blocked[0].change, "beta");
        assert!(next.blocked[0].blockers[0].contains("alpha"));

        // r[verify cairn.change_dependency_graph.next_ready.blocked]
        let cycle = build_change_dependency_graph(
            &[change("alpha", &["beta"]), change("beta", &["alpha"])],
            &[],
        );
        let blocked = next_ready_change(&cycle);
        assert_eq!(blocked.ready, None);
        assert_eq!(blocked.blocked.len(), 2);
    }
}
