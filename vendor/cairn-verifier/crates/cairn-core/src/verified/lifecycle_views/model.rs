use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use crate::verified::{
    change_dependencies::{ChangeDependencyGraph, ChangeDependencyNode},
    repo::{ChangeStatus, TaskCounts},
    traceability::{
        TraceabilityCoverageReport, TRACEABILITY_VERB_DEPENDS, TRACEABILITY_VERB_IMPL,
        TRACEABILITY_VERB_RELATED, TRACEABILITY_VERB_VERIFY,
    },
};

use super::{
    parse::{parse_specs, parse_tasks},
    types::{
        KanbanBoard, KanbanCard, KanbanColumn, KanbanColumnKind, LifecycleMindmapNode,
        LifecycleViewChangeInput, LifecycleViewEdge, LifecycleViewEdgeKind, LifecycleViewInput,
        LifecycleViewIssue, LifecycleViewModel, LifecycleViewNode, LifecycleViewNodeKind,
        ParsedRequirement, ParsedScenario, ParsedSpec, LIFECYCLE_VIEW_BOUNDARY,
        LIFECYCLE_VIEW_NON_CLAIMS, LIFECYCLE_VIEW_SCHEMA_V1,
    },
};

const REPOSITORY_NODE_ID: &str = "repository:root";
const MAX_MINDMAP_DEPTH: usize = 256;
const TRACEABILITY_LINE_PREFIX: &str = "line:";

#[derive(Debug, Clone)]
struct RequirementDeclaration {
    spec_id: String,
    spec_path: String,
    accepted: bool,
    requirement: ParsedRequirement,
}

#[derive(Debug, Clone)]
struct ScenarioDeclaration {
    spec_path: String,
    accepted: bool,
    scenario: ParsedScenario,
}

#[derive(Debug, Default)]
struct ViewBuilder {
    nodes: BTreeMap<String, LifecycleViewNode>,
    edges: BTreeMap<EdgeKey, BTreeSet<String>>,
    issues: Vec<LifecycleViewIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    source: String,
    target: String,
    kind: LifecycleViewEdgeKind,
    state: Option<String>,
}

pub fn build_lifecycle_view_model(input: LifecycleViewInput) -> LifecycleViewModel {
    // r[impl cairn.lifecycle_views.semantic_model]
    // r[impl cairn.lifecycle_views.semantic_model.shared]
    // r[impl cairn.lifecycle_views.kanban]
    // r[impl cairn.lifecycle_views.mindmap]
    // r[impl cairn.lifecycle_views.mindmap.hierarchy]
    // r[impl cairn.lifecycle_views.knowledge_graph]
    // r[impl cairn.lifecycle_views.knowledge_graph.links]
    let LifecycleViewInput {
        repository,
        mut changes,
        specs,
        dependency_graph,
        traceability,
    } = input;
    changes.sort_by(|left, right| left.status.name.cmp(&right.status.name));

    let mut builder = ViewBuilder::default();
    builder.add_node(LifecycleViewNode {
        id: REPOSITORY_NODE_ID.to_string(),
        kind: LifecycleViewNodeKind::Repository,
        label: repository.clone(),
        reference: None,
        state: None,
        path: None,
    });
    add_change_nodes(&changes, &mut builder);
    add_dependency_edges(&dependency_graph, &mut builder);

    let (parsed_specs, parse_issues) = parse_specs(specs);
    builder.issues.extend(parse_issues);
    add_spec_nodes(&parsed_specs, &mut builder);
    let mut reference_targets = add_requirement_nodes(&parsed_specs, &mut builder);
    if let Some(report) = traceability.as_ref() {
        add_traceability_requirement_targets(report, &mut reference_targets, &mut builder);
    }
    add_task_nodes(&changes, &reference_targets, &mut builder);
    if let Some(report) = traceability.as_ref() {
        add_traceability_edges(report, &reference_targets, &mut builder);
    }

    builder.normalize();
    let kanban = build_kanban_board(&changes, &dependency_graph);
    let mindmap = build_mindmap(&builder.nodes, &builder.edges, &mut builder.issues);
    builder.normalize();
    let nodes = builder.nodes.into_values().collect::<Vec<_>>();
    let edges = builder
        .edges
        .into_iter()
        .map(|(key, details)| LifecycleViewEdge {
            source: key.source,
            target: key.target,
            kind: key.kind,
            state: key.state,
            details: details.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    let valid = builder.issues.is_empty();

    LifecycleViewModel {
        schema: LIFECYCLE_VIEW_SCHEMA_V1.to_string(),
        repository,
        valid,
        nodes,
        edges,
        issues: builder.issues,
        kanban,
        mindmap,
        boundary: LIFECYCLE_VIEW_BOUNDARY.to_string(),
        non_claims: LIFECYCLE_VIEW_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
    }
}

fn add_change_nodes(changes: &[LifecycleViewChangeInput], builder: &mut ViewBuilder) {
    for change in changes {
        let node_id = change_node_id(&change.status.name);
        builder.add_node(LifecycleViewNode {
            id: node_id.clone(),
            kind: LifecycleViewNodeKind::Change,
            label: change.status.name.clone(),
            reference: Some(change.status.name.clone()),
            state: change.status.metadata.status.clone(),
            path: Some(change.status.path.clone()),
        });
        builder.add_edge(
            REPOSITORY_NODE_ID,
            &node_id,
            LifecycleViewEdgeKind::Contains,
            Some("active"),
            &[],
        );
    }
}

fn add_dependency_edges(graph: &ChangeDependencyGraph, builder: &mut ViewBuilder) {
    for edge in &graph.edges {
        let dependent_id = change_node_id(&edge.dependent);
        let dependency_id = change_node_id(&edge.dependency);
        ensure_dependency_node(&edge.dependency, edge.satisfied, builder);
        let state = if edge.satisfied {
            "satisfied"
        } else {
            "blocking"
        };
        builder.add_edge(
            &dependent_id,
            &dependency_id,
            LifecycleViewEdgeKind::DependsOn,
            Some(state),
            &[],
        );
        builder.add_edge(
            &dependency_id,
            &dependent_id,
            LifecycleViewEdgeKind::Blocks,
            Some(state),
            &[],
        );
    }
    for issue in &graph.issues {
        builder.issues.push(LifecycleViewIssue {
            code: issue.code.clone(),
            message: issue.message.clone(),
            source: change_node_id(&issue.change),
            target: issue.dependency.as_ref().map(|name| change_node_id(name)),
        });
    }
}

fn ensure_dependency_node(name: &str, satisfied: bool, builder: &mut ViewBuilder) {
    let node_id = change_node_id(name);
    if builder.nodes.contains_key(&node_id) {
        return;
    }
    let state = if satisfied { "archived" } else { "unknown" };
    builder.add_node(LifecycleViewNode {
        id: node_id.clone(),
        kind: LifecycleViewNodeKind::Change,
        label: name.to_string(),
        reference: Some(name.to_string()),
        state: Some(state.to_string()),
        path: None,
    });
    builder.add_edge(
        REPOSITORY_NODE_ID,
        &node_id,
        LifecycleViewEdgeKind::Contains,
        Some(state),
        &[],
    );
}

fn add_spec_nodes(specs: &[ParsedSpec], builder: &mut ViewBuilder) {
    for spec in specs {
        builder.add_node(LifecycleViewNode {
            id: spec.id.clone(),
            kind: LifecycleViewNodeKind::Spec,
            label: spec.label.clone(),
            reference: Some(spec.id.trim_start_matches("spec:").to_string()),
            state: Some(if spec.owner_change.is_some() {
                "proposed".to_string()
            } else {
                "accepted".to_string()
            }),
            path: Some(spec.path.clone()),
        });
        let parent = spec
            .owner_change
            .as_ref()
            .map(|change| change_node_id(change))
            .filter(|change_id| builder.nodes.contains_key(change_id))
            .unwrap_or_else(|| REPOSITORY_NODE_ID.to_string());
        if spec.owner_change.is_some() && parent == REPOSITORY_NODE_ID {
            builder.issues.push(LifecycleViewIssue {
                code: "lifecycle_view.spec.unknown_change".to_string(),
                message: "delta spec owner change is not present in the active change set"
                    .to_string(),
                source: spec.path.clone(),
                target: spec.owner_change.clone(),
            });
        }
        builder.add_edge(
            &parent,
            &spec.id,
            LifecycleViewEdgeKind::Contains,
            None,
            &[],
        );
    }
}

fn add_requirement_nodes(
    specs: &[ParsedSpec],
    builder: &mut ViewBuilder,
) -> BTreeMap<String, String> {
    let mut declarations = BTreeMap::<String, Vec<RequirementDeclaration>>::new();
    for spec in specs {
        for requirement in &spec.requirements {
            declarations
                .entry(requirement.id.clone())
                .or_default()
                .push(RequirementDeclaration {
                    spec_id: spec.id.clone(),
                    spec_path: spec.path.clone(),
                    accepted: spec.owner_change.is_none(),
                    requirement: requirement.clone(),
                });
        }
    }

    let mut reference_targets = BTreeMap::new();
    for (requirement_id, mut rows) in declarations {
        rows.sort_by(|left, right| {
            right
                .accepted
                .cmp(&left.accepted)
                .then(left.spec_path.cmp(&right.spec_path))
        });
        let Some(owner) = rows.first().cloned() else {
            continue;
        };
        builder.add_node(LifecycleViewNode {
            id: requirement_id.clone(),
            kind: LifecycleViewNodeKind::Requirement,
            label: owner.requirement.label.clone(),
            reference: owner.requirement.reference.clone(),
            state: Some(
                if owner.accepted {
                    "accepted"
                } else {
                    "proposed"
                }
                .to_string(),
            ),
            path: Some(owner.spec_path.clone()),
        });
        builder.add_edge(
            &owner.spec_id,
            &requirement_id,
            LifecycleViewEdgeKind::Contains,
            None,
            &[],
        );
        for row in rows.iter().skip(1) {
            builder.add_edge(
                &row.spec_id,
                &requirement_id,
                LifecycleViewEdgeKind::Changes,
                None,
                &[],
            );
        }
        if let Some(reference) = owner.requirement.reference.as_ref() {
            register_reference(reference, &requirement_id, &mut reference_targets);
        }
        add_scenario_nodes(&requirement_id, &rows, &mut reference_targets, builder);
    }
    reference_targets
}

fn add_scenario_nodes(
    requirement_id: &str,
    requirement_rows: &[RequirementDeclaration],
    reference_targets: &mut BTreeMap<String, String>,
    builder: &mut ViewBuilder,
) {
    let mut declarations = BTreeMap::<String, Vec<ScenarioDeclaration>>::new();
    for row in requirement_rows {
        for scenario in &row.requirement.scenarios {
            declarations
                .entry(scenario.id.clone())
                .or_default()
                .push(ScenarioDeclaration {
                    spec_path: row.spec_path.clone(),
                    accepted: row.accepted,
                    scenario: scenario.clone(),
                });
        }
    }
    for (scenario_id, mut rows) in declarations {
        rows.sort_by(|left, right| {
            right
                .accepted
                .cmp(&left.accepted)
                .then(left.spec_path.cmp(&right.spec_path))
        });
        let Some(owner) = rows.first() else {
            continue;
        };
        builder.add_node(LifecycleViewNode {
            id: scenario_id.clone(),
            kind: LifecycleViewNodeKind::Scenario,
            label: owner.scenario.label.clone(),
            reference: owner.scenario.reference.clone(),
            state: Some(
                if owner.accepted {
                    "accepted"
                } else {
                    "proposed"
                }
                .to_string(),
            ),
            path: Some(owner.spec_path.clone()),
        });
        builder.add_edge(
            requirement_id,
            &scenario_id,
            LifecycleViewEdgeKind::Contains,
            None,
            &[],
        );
        if let Some(reference) = owner.scenario.reference.as_ref() {
            register_reference(reference, &scenario_id, reference_targets);
        }
    }
}

fn register_reference(
    reference: &str,
    node_id: &str,
    reference_targets: &mut BTreeMap<String, String>,
) {
    if let Entry::Vacant(entry) = reference_targets.entry(reference.to_string()) {
        entry.insert(node_id.to_string());
    }
}

fn add_traceability_requirement_targets(
    report: &TraceabilityCoverageReport,
    reference_targets: &mut BTreeMap<String, String>,
    builder: &mut ViewBuilder,
) {
    for (reference, occurrences) in &report.requirements {
        if reference_targets.contains_key(reference) {
            continue;
        }
        let node_id = format!("requirement:{reference}");
        let path = occurrences
            .first()
            .map(|occurrence| occurrence.path.clone());
        builder.add_node(LifecycleViewNode {
            id: node_id.clone(),
            kind: LifecycleViewNodeKind::Requirement,
            label: reference.clone(),
            reference: Some(reference.clone()),
            state: Some("unparsed".to_string()),
            path,
        });
        reference_targets.insert(reference.clone(), node_id);
    }
}

fn add_task_nodes(
    changes: &[LifecycleViewChangeInput],
    reference_targets: &BTreeMap<String, String>,
    builder: &mut ViewBuilder,
) {
    for change in changes {
        let Some(markdown) = change.tasks_markdown.as_deref() else {
            continue;
        };
        let task_path = format!("{}/tasks.md", change.status.path.trim_end_matches('/'));
        let (tasks, task_issues) = parse_tasks(&change.status.name, &task_path, markdown);
        builder.issues.extend(task_issues);
        let change_id = change_node_id(&change.status.name);
        for task in tasks {
            builder.add_node(LifecycleViewNode {
                id: task.id.clone(),
                kind: LifecycleViewNodeKind::Task,
                label: task.label,
                reference: None,
                state: Some(task.state.as_str().to_string()),
                path: Some(task.path),
            });
            builder.add_edge(
                &change_id,
                &task.id,
                LifecycleViewEdgeKind::Contains,
                None,
                &[],
            );
            for reference in task.covers {
                let target = reference_targets
                    .get(&reference)
                    .cloned()
                    .unwrap_or_else(|| add_missing_requirement(&reference, builder));
                if !reference_targets.contains_key(&reference) {
                    builder.issues.push(LifecycleViewIssue {
                        code: "lifecycle_view.task.dangling_reference".to_string(),
                        message: format!("task references unknown lifecycle id {reference}"),
                        source: task.id.clone(),
                        target: Some(target.clone()),
                    });
                }
                builder.add_edge(&task.id, &target, LifecycleViewEdgeKind::Covers, None, &[]);
            }
        }
    }
}

fn add_traceability_edges(
    report: &TraceabilityCoverageReport,
    reference_targets: &BTreeMap<String, String>,
    builder: &mut ViewBuilder,
) {
    for (reference, occurrences) in &report.all_references {
        let target = reference_targets
            .get(reference)
            .cloned()
            .unwrap_or_else(|| add_missing_requirement(reference, builder));
        for occurrence in occurrences {
            let artifact_id = source_artifact_node_id(&occurrence.path);
            builder.add_node(LifecycleViewNode {
                id: artifact_id.clone(),
                kind: LifecycleViewNodeKind::SourceArtifact,
                label: occurrence.path.clone(),
                reference: None,
                state: None,
                path: Some(occurrence.path.clone()),
            });
            builder.add_edge(
                &artifact_id,
                &target,
                traceability_edge_kind(&occurrence.verb),
                None,
                &[format!("{TRACEABILITY_LINE_PREFIX}{}", occurrence.line)],
            );
        }
    }
    for reference in &report.missing {
        builder.issues.push(LifecycleViewIssue {
            code: "lifecycle_view.traceability.missing".to_string(),
            message: format!("lifecycle id {reference} has no implementation or verification link"),
            source: reference_targets
                .get(reference)
                .cloned()
                .unwrap_or_else(|| format!("requirement:{reference}")),
            target: None,
        });
    }
    for reference in report
        .dangling
        .iter()
        .filter(|reference| !reference_targets.contains_key(*reference))
    {
        let target = add_missing_requirement(reference, builder);
        builder.issues.push(LifecycleViewIssue {
            code: "lifecycle_view.traceability.dangling".to_string(),
            message: format!("traceability link references unknown lifecycle id {reference}"),
            source: target.clone(),
            target: Some(target),
        });
    }
}

fn add_missing_requirement(reference: &str, builder: &mut ViewBuilder) -> String {
    let node_id = format!("missing_requirement:{reference}");
    builder.add_node(LifecycleViewNode {
        id: node_id.clone(),
        kind: LifecycleViewNodeKind::MissingRequirement,
        label: reference.to_string(),
        reference: Some(reference.to_string()),
        state: Some("missing".to_string()),
        path: None,
    });
    node_id
}

fn traceability_edge_kind(verb: &str) -> LifecycleViewEdgeKind {
    match verb {
        TRACEABILITY_VERB_IMPL => LifecycleViewEdgeKind::Implements,
        TRACEABILITY_VERB_VERIFY => LifecycleViewEdgeKind::Verifies,
        TRACEABILITY_VERB_DEPENDS => LifecycleViewEdgeKind::TraceabilityDepends,
        TRACEABILITY_VERB_RELATED => LifecycleViewEdgeKind::Related,
        _ => LifecycleViewEdgeKind::Related,
    }
}

fn build_kanban_board(
    changes: &[LifecycleViewChangeInput],
    graph: &ChangeDependencyGraph,
) -> KanbanBoard {
    let mut lanes = BTreeMap::<KanbanColumnKind, Vec<KanbanCard>>::new();
    for kind in KanbanColumnKind::ORDERED {
        lanes.insert(kind, Vec::new());
    }
    for change in changes {
        let node = graph.node(&change.status.name);
        let (kind, mut reasons) = classify_change(&change.status, node, graph);
        reasons.sort();
        reasons.dedup();
        let mut dependencies = node
            .map(|node| node.dependencies.clone())
            .unwrap_or_default();
        dependencies.sort();
        dependencies.dedup();
        let mut blocked_by = node.map(|node| node.blocked_by.clone()).unwrap_or_default();
        blocked_by.sort();
        blocked_by.dedup();
        let card = KanbanCard {
            change: change.status.name.clone(),
            metadata_status: change.status.metadata.status.clone(),
            task_counts: change.status.task_counts,
            dependencies,
            blocked_by,
            reasons,
        };
        if let Some(cards) = lanes.get_mut(&kind) {
            cards.push(card);
        }
    }
    KanbanBoard {
        columns: KanbanColumnKind::ORDERED
            .iter()
            .map(|kind| {
                let mut cards = lanes.remove(kind).unwrap_or_default();
                cards.sort_by(|left, right| left.change.cmp(&right.change));
                KanbanColumn { kind: *kind, cards }
            })
            .collect(),
    }
}

fn classify_change(
    change: &ChangeStatus,
    node: Option<&ChangeDependencyNode>,
    graph: &ChangeDependencyGraph,
) -> (KanbanColumnKind, Vec<String>) {
    let mut blockers = Vec::new();
    if change.metadata.status.as_deref() == Some("blocked") {
        blockers.push("metadata status is blocked".to_string());
    }
    if let Some(node) = node {
        blockers.extend(
            node.blocked_by
                .iter()
                .map(|dependency| format!("blocked by active dependency {dependency}")),
        );
    }
    blockers.extend(
        graph
            .issues
            .iter()
            .filter(|issue| issue.change == change.name)
            .map(|issue| format!("{}: {}", issue.code, issue.message)),
    );
    blockers.extend(change.metadata_issues.iter().cloned());
    if !blockers.is_empty() {
        return (KanbanColumnKind::Blocked, blockers);
    }

    if archive_ready(change, node) {
        return (
            KanbanColumnKind::ReadyToArchive,
            vec![
                "all marked tasks are complete".to_string(),
                "required lifecycle artifacts are present".to_string(),
                "declared dependencies are satisfied".to_string(),
            ],
        );
    }

    if work_started(change.task_counts) || change.metadata.status.as_deref() == Some("current") {
        return (
            KanbanColumnKind::InProgress,
            vec!["task work has started or metadata status is current".to_string()],
        );
    }

    if matches!(
        change.metadata.status.as_deref(),
        Some("planned" | "deferred")
    ) || !change.has_tasks
    {
        return (
            KanbanColumnKind::Planned,
            vec!["work is planned, deferred, or not yet task-authored".to_string()],
        );
    }

    (
        KanbanColumnKind::Ready,
        vec!["declared dependencies are satisfied and task work has not started".to_string()],
    )
}

fn archive_ready(change: &ChangeStatus, node: Option<&ChangeDependencyNode>) -> bool {
    change.has_proposal
        && change.has_design
        && change.has_tasks
        && !change.delta_specs.is_empty()
        && node.is_some_and(|node| node.dependency_ready)
        && change.task_counts.is_some_and(tasks_complete)
}

fn tasks_complete(counts: TaskCounts) -> bool {
    counts.total > 0 && counts.todo == 0 && counts.in_progress == 0 && counts.unmarked == 0
}

fn work_started(counts: Option<TaskCounts>) -> bool {
    counts.is_some_and(|counts| counts.done > 0 || counts.in_progress > 0)
}

fn build_mindmap(
    nodes: &BTreeMap<String, LifecycleViewNode>,
    edges: &BTreeMap<EdgeKey, BTreeSet<String>>,
    issues: &mut Vec<LifecycleViewIssue>,
) -> LifecycleMindmapNode {
    let mut parents = BTreeMap::<String, BTreeSet<String>>::new();
    let mut children = BTreeMap::<String, BTreeSet<String>>::new();
    for key in edges
        .keys()
        .filter(|key| key.kind == LifecycleViewEdgeKind::Contains)
    {
        parents
            .entry(key.target.clone())
            .or_default()
            .insert(key.source.clone());
        children
            .entry(key.source.clone())
            .or_default()
            .insert(key.target.clone());
    }
    for (child, parent_ids) in &parents {
        if parent_ids.len() > 1 {
            issues.push(LifecycleViewIssue {
                code: "lifecycle_view.mindmap.multiple_parents".to_string(),
                message: format!("mindmap node has multiple containment parents: {parent_ids:?}"),
                source: child.clone(),
                target: None,
            });
        }
    }
    let mut ancestors = BTreeSet::new();
    build_mindmap_node(
        REPOSITORY_NODE_ID,
        nodes,
        &children,
        &mut ancestors,
        0,
        issues,
    )
    .unwrap_or_else(|| LifecycleMindmapNode {
        id: REPOSITORY_NODE_ID.to_string(),
        kind: LifecycleViewNodeKind::Repository,
        label: "repository".to_string(),
        children: Vec::new(),
    })
}

fn build_mindmap_node(
    node_id: &str,
    nodes: &BTreeMap<String, LifecycleViewNode>,
    children: &BTreeMap<String, BTreeSet<String>>,
    ancestors: &mut BTreeSet<String>,
    depth: usize,
    issues: &mut Vec<LifecycleViewIssue>,
) -> Option<LifecycleMindmapNode> {
    if depth > MAX_MINDMAP_DEPTH {
        issues.push(LifecycleViewIssue {
            code: "lifecycle_view.mindmap.depth_exceeded".to_string(),
            message: format!("mindmap depth exceeds supported bound {MAX_MINDMAP_DEPTH}"),
            source: node_id.to_string(),
            target: None,
        });
        return None;
    }
    if !ancestors.insert(node_id.to_string()) {
        issues.push(LifecycleViewIssue {
            code: "lifecycle_view.mindmap.cycle".to_string(),
            message: "containment cycle detected".to_string(),
            source: node_id.to_string(),
            target: None,
        });
        return None;
    }
    let node = nodes.get(node_id)?;
    let mut child_nodes = Vec::new();
    for child_id in children.get(node_id).into_iter().flatten() {
        if let Some(child) = build_mindmap_node(
            child_id,
            nodes,
            children,
            ancestors,
            depth.saturating_add(1),
            issues,
        ) {
            child_nodes.push(child);
        }
    }
    ancestors.remove(node_id);
    Some(LifecycleMindmapNode {
        id: node.id.clone(),
        kind: node.kind,
        label: node.label.clone(),
        children: child_nodes,
    })
}

fn change_node_id(name: &str) -> String {
    format!("change:{name}")
}

fn source_artifact_node_id(path: &str) -> String {
    format!("source_artifact:{path}")
}

impl ViewBuilder {
    fn add_node(&mut self, node: LifecycleViewNode) {
        match self.nodes.entry(node.id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(node);
            }
            Entry::Occupied(entry) if entry.get() == &node => {}
            Entry::Occupied(entry) => self.issues.push(LifecycleViewIssue {
                code: "lifecycle_view.node.conflict".to_string(),
                message: format!(
                    "node {} has conflicting declarations ({:?} versus {:?})",
                    node.id,
                    entry.get().kind,
                    node.kind
                ),
                source: node.id.clone(),
                target: None,
            }),
        }
    }

    fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        kind: LifecycleViewEdgeKind,
        state: Option<&str>,
        details: &[String],
    ) {
        let key = EdgeKey {
            source: source.to_string(),
            target: target.to_string(),
            kind,
            state: state.map(str::to_string),
        };
        let stored_details = self.edges.entry(key).or_default();
        stored_details.extend(details.iter().cloned());
    }

    fn normalize(&mut self) {
        self.issues.sort();
        self.issues.dedup();
    }
}
