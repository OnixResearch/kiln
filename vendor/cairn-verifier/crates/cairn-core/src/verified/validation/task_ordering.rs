use std::collections::{BTreeMap, BTreeSet};

use crate::verified::{
    dag::topological_order,
    policy::{Policy, TaskMarker, TASK_IDENTITY_MARKER_ID, TASK_PREDECESSOR_MARKER_ID},
};

use super::{
    spec::{fence_boundary, visible_text_without_comments},
    substance::valid_lifecycle_id,
};

pub const TASK_ORDER_ISSUE_MARKER_POLICY: &str = "task_ordering.marker_policy";
pub const TASK_ORDER_ISSUE_ID_MISSING: &str = "task_ordering.task_id.missing";
pub const TASK_ORDER_ISSUE_ID_MALFORMED: &str = "task_ordering.task_id.malformed";
pub const TASK_ORDER_ISSUE_ID_MULTIPLE: &str = "task_ordering.task_id.multiple";
pub const TASK_ORDER_ISSUE_ID_DUPLICATE: &str = "task_ordering.task_id.duplicate";
pub const TASK_ORDER_ISSUE_PREDECESSOR_MALFORMED: &str = "task_ordering.predecessor.malformed";
pub const TASK_ORDER_ISSUE_PREDECESSOR_UNKNOWN: &str = "task_ordering.predecessor.unknown";
pub const TASK_ORDER_ISSUE_PREDECESSOR_DUPLICATE: &str = "task_ordering.predecessor.duplicate";
pub const TASK_ORDER_ISSUE_PREDECESSOR_SELF: &str = "task_ordering.predecessor.self";
pub const TASK_ORDER_ISSUE_CYCLE: &str = "task_ordering.cycle";
pub const TASK_ORDER_ISSUE_PROGRESS: &str = "task_ordering.progress.unsatisfied";

const TASK_IDENTITY_MARKER_KIND: &str = "task-id-prefix";
const TASK_PREDECESSOR_MARKER_KIND: &str = "task-predecessor-prefix";
const MARKER_SUFFIX: char = ']';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOrderingState {
    Done,
    InProgress,
    Todo,
}

impl TaskOrderingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::InProgress => "in_progress",
            Self::Todo => "todo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOrderingGraph {
    pub active: bool,
    pub valid: bool,
    pub nodes: Vec<TaskOrderingNode>,
    pub edges: Vec<TaskOrderingEdge>,
    pub issues: Vec<TaskOrderingIssue>,
    pub order: Vec<String>,
    pub ready_tasks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOrderingNode {
    pub id: String,
    pub line: usize,
    pub state: TaskOrderingState,
    pub predecessors: Vec<String>,
    pub blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOrderingEdge {
    pub task: String,
    pub predecessor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOrderingIssue {
    pub code: String,
    pub line: Option<usize>,
    pub task: Option<String>,
    pub predecessor: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawTask {
    line: usize,
    state: TaskOrderingState,
    identity: MarkerObservation,
    predecessors: MarkerObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkerObservation {
    occurrences: usize,
    targets: Vec<String>,
}

pub fn build_task_ordering_graph(markdown: &str, policy: &Policy) -> TaskOrderingGraph {
    // r[impl cairn.task_ordering.markers]
    // r[impl cairn.task_ordering.graph]
    // r[impl cairn.task_ordering.progress]
    let Some(identity_marker) = configured_marker(policy, TASK_IDENTITY_MARKER_ID) else {
        return marker_policy_failure(TASK_IDENTITY_MARKER_ID);
    };
    let Some(predecessor_marker) = configured_marker(policy, TASK_PREDECESSOR_MARKER_ID) else {
        return marker_policy_failure(TASK_PREDECESSOR_MARKER_ID);
    };
    if identity_marker.kind != TASK_IDENTITY_MARKER_KIND {
        return marker_kind_failure(identity_marker, TASK_IDENTITY_MARKER_KIND);
    }
    if predecessor_marker.kind != TASK_PREDECESSOR_MARKER_KIND {
        return marker_kind_failure(predecessor_marker, TASK_PREDECESSOR_MARKER_KIND);
    }

    let identity_prefix = marker_prefix(&identity_marker.token);
    let predecessor_prefix = marker_prefix(&predecessor_marker.token);
    let mut issues = Vec::new();
    let raw_tasks = observe_tasks(markdown, identity_prefix, predecessor_prefix, &mut issues);
    let active = raw_tasks
        .iter()
        .any(|task| task.identity.occurrences > 0 || task.predecessors.occurrences > 0);
    if !active {
        return TaskOrderingGraph {
            active: false,
            valid: true,
            nodes: Vec::new(),
            edges: Vec::new(),
            issues: Vec::new(),
            order: Vec::new(),
            ready_tasks: Vec::new(),
        };
    }

    let mut nodes = identify_nodes(&raw_tasks, &mut issues);
    let mut edges = admit_edges(&raw_tasks, &mut nodes, &mut issues);
    edges.sort_by(|left, right| {
        left.task
            .cmp(&right.task)
            .then(left.predecessor.cmp(&right.predecessor))
    });
    sort_node_edges(&mut nodes);

    let node_ids = nodes.keys().cloned().collect::<Vec<_>>();
    let topological_edges = edges
        .iter()
        .map(|edge| (edge.predecessor.clone(), edge.task.clone()))
        .collect::<Vec<_>>();
    let topological = topological_order(&node_ids, &topological_edges);
    for task_id in &topological.blocked_nodes {
        issues.push(TaskOrderingIssue {
            code: TASK_ORDER_ISSUE_CYCLE.to_string(),
            line: nodes.get(task_id).map(|node| node.line),
            task: Some(task_id.clone()),
            predecessor: None,
            message: format!("task {task_id} is in or blocked by a task dependency cycle"),
        });
    }

    observe_progress(&mut nodes, &mut issues);
    sort_issues(&mut issues);
    let valid = issues.is_empty();
    let mut node_values = nodes.into_values().collect::<Vec<_>>();
    if !valid {
        for node in &mut node_values {
            node.ready = false;
        }
    }
    let order = if valid { topological.order } else { Vec::new() };
    let ready_tasks = if valid {
        order
            .iter()
            .filter(|task_id| {
                node_values
                    .iter()
                    .find(|node| &node.id == *task_id)
                    .is_some_and(|node| node.ready)
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    TaskOrderingGraph {
        active,
        valid,
        nodes: node_values,
        edges,
        issues,
        order,
        ready_tasks,
    }
}

fn configured_marker<'a>(policy: &'a Policy, marker_id: &str) -> Option<&'a TaskMarker> {
    policy
        .task_marker_tokens
        .iter()
        .find(|marker| marker.id == marker_id)
}

fn marker_policy_failure(marker_id: &str) -> TaskOrderingGraph {
    let issue = TaskOrderingIssue {
        code: TASK_ORDER_ISSUE_MARKER_POLICY.to_string(),
        line: None,
        task: None,
        predecessor: None,
        message: format!("task marker policy does not define marker {marker_id}"),
    };
    TaskOrderingGraph {
        active: false,
        valid: false,
        nodes: Vec::new(),
        edges: Vec::new(),
        issues: vec![issue],
        order: Vec::new(),
        ready_tasks: Vec::new(),
    }
}

fn marker_kind_failure(marker: &TaskMarker, expected_kind: &str) -> TaskOrderingGraph {
    let issue = TaskOrderingIssue {
        code: TASK_ORDER_ISSUE_MARKER_POLICY.to_string(),
        line: None,
        task: None,
        predecessor: None,
        message: format!(
            "task marker {} uses kind {}, expected {expected_kind}",
            marker.id, marker.kind
        ),
    };
    TaskOrderingGraph {
        active: false,
        valid: false,
        nodes: Vec::new(),
        edges: Vec::new(),
        issues: vec![issue],
        order: Vec::new(),
        ready_tasks: Vec::new(),
    }
}

fn observe_tasks(
    markdown: &str,
    identity_prefix: &str,
    predecessor_prefix: &str,
    issues: &mut Vec<TaskOrderingIssue>,
) -> Vec<RawTask> {
    let mut tasks = Vec::new();
    let mut is_fenced = false;
    let mut is_in_comment = false;
    for (line_index, line) in markdown.lines().enumerate() {
        if fence_boundary(line) && !is_in_comment {
            is_fenced = !is_fenced;
            continue;
        }
        let visible = visible_text_without_comments(line, &mut is_in_comment);
        if is_fenced {
            continue;
        }
        let trimmed = visible.trim_start();
        let Some(state) = task_state(trimmed) else {
            continue;
        };
        let line_number = line_index + 1;
        tasks.push(RawTask {
            line: line_number,
            state,
            identity: observe_markers(
                trimmed,
                identity_prefix,
                line_number,
                TASK_ORDER_ISSUE_ID_MALFORMED,
                "task identity",
                issues,
            ),
            predecessors: observe_markers(
                trimmed,
                predecessor_prefix,
                line_number,
                TASK_ORDER_ISSUE_PREDECESSOR_MALFORMED,
                "task predecessor",
                issues,
            ),
        });
    }
    tasks
}

fn observe_markers(
    line: &str,
    prefix: &str,
    line_number: usize,
    issue_code: &str,
    marker_name: &str,
    issues: &mut Vec<TaskOrderingIssue>,
) -> MarkerObservation {
    let mut rest = line;
    let mut occurrences = 0usize;
    let mut targets = Vec::new();
    while let Some(start) = rest.find(prefix) {
        occurrences += 1;
        let after_prefix = &rest[start + prefix.len()..];
        let Some(end) = after_prefix.find(MARKER_SUFFIX) else {
            issues.push(malformed_marker_issue(
                issue_code,
                line_number,
                format!("unclosed {marker_name} marker"),
            ));
            break;
        };
        let target = after_prefix[..end].trim();
        if !valid_lifecycle_id(target) {
            issues.push(malformed_marker_issue(
                issue_code,
                line_number,
                format!("{marker_name} target `{target}` is not a valid lifecycle ID"),
            ));
        } else {
            targets.push(target.to_string());
        }
        rest = &after_prefix[end + MARKER_SUFFIX.len_utf8()..];
    }
    MarkerObservation {
        occurrences,
        targets,
    }
}

fn malformed_marker_issue(code: &str, line: usize, message: String) -> TaskOrderingIssue {
    TaskOrderingIssue {
        code: code.to_string(),
        line: Some(line),
        task: None,
        predecessor: None,
        message,
    }
}

fn identify_nodes(
    raw_tasks: &[RawTask],
    issues: &mut Vec<TaskOrderingIssue>,
) -> BTreeMap<String, TaskOrderingNode> {
    let mut nodes = BTreeMap::<String, TaskOrderingNode>::new();
    for task in raw_tasks {
        if task.identity.occurrences == 0 {
            issues.push(TaskOrderingIssue {
                code: TASK_ORDER_ISSUE_ID_MISSING.to_string(),
                line: Some(task.line),
                task: None,
                predecessor: None,
                message: "task ordering is active, but this checkbox task has no task identity"
                    .to_string(),
            });
            continue;
        }
        if task.identity.occurrences != 1 {
            issues.push(TaskOrderingIssue {
                code: TASK_ORDER_ISSUE_ID_MULTIPLE.to_string(),
                line: Some(task.line),
                task: task.identity.targets.first().cloned(),
                predecessor: None,
                message: "checkbox task must contain exactly one task identity marker".to_string(),
            });
            continue;
        }
        let Some(task_id) = task.identity.targets.first() else {
            continue;
        };
        let node = TaskOrderingNode {
            id: task_id.clone(),
            line: task.line,
            state: task.state,
            predecessors: Vec::new(),
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            ready: false,
        };
        if let Some(existing) = nodes.get(task_id) {
            issues.push(TaskOrderingIssue {
                code: TASK_ORDER_ISSUE_ID_DUPLICATE.to_string(),
                line: Some(task.line),
                task: Some(task_id.clone()),
                predecessor: None,
                message: format!(
                    "task identity {task_id} duplicates the identity on line {}",
                    existing.line
                ),
            });
        } else {
            nodes.insert(task_id.clone(), node);
        }
    }
    nodes
}

fn admit_edges(
    raw_tasks: &[RawTask],
    nodes: &mut BTreeMap<String, TaskOrderingNode>,
    issues: &mut Vec<TaskOrderingIssue>,
) -> Vec<TaskOrderingEdge> {
    let known_ids = nodes.keys().cloned().collect::<BTreeSet<_>>();
    let mut edges = Vec::new();
    for task in raw_tasks {
        if task.identity.occurrences != 1 {
            continue;
        }
        let Some(task_id) = task.identity.targets.first() else {
            continue;
        };
        if !known_ids.contains(task_id) {
            continue;
        }
        let mut seen = BTreeSet::new();
        for predecessor in &task.predecessors.targets {
            if !seen.insert(predecessor.clone()) {
                issues.push(TaskOrderingIssue {
                    code: TASK_ORDER_ISSUE_PREDECESSOR_DUPLICATE.to_string(),
                    line: Some(task.line),
                    task: Some(task_id.clone()),
                    predecessor: Some(predecessor.clone()),
                    message: format!(
                        "task {task_id} declares predecessor {predecessor} more than once"
                    ),
                });
                continue;
            }
            if predecessor == task_id {
                issues.push(TaskOrderingIssue {
                    code: TASK_ORDER_ISSUE_PREDECESSOR_SELF.to_string(),
                    line: Some(task.line),
                    task: Some(task_id.clone()),
                    predecessor: Some(predecessor.clone()),
                    message: format!("task {task_id} depends on itself"),
                });
                continue;
            }
            if !known_ids.contains(predecessor) {
                issues.push(TaskOrderingIssue {
                    code: TASK_ORDER_ISSUE_PREDECESSOR_UNKNOWN.to_string(),
                    line: Some(task.line),
                    task: Some(task_id.clone()),
                    predecessor: Some(predecessor.clone()),
                    message: format!("task {task_id} declares unknown predecessor {predecessor}"),
                });
                continue;
            }
            edges.push(TaskOrderingEdge {
                task: task_id.clone(),
                predecessor: predecessor.clone(),
            });
            if let Some(node) = nodes.get_mut(task_id) {
                node.predecessors.push(predecessor.clone());
            }
            if let Some(node) = nodes.get_mut(predecessor) {
                node.blocks.push(task_id.clone());
            }
        }
    }
    edges
}

fn sort_node_edges(nodes: &mut BTreeMap<String, TaskOrderingNode>) {
    for node in nodes.values_mut() {
        node.predecessors.sort();
        node.predecessors.dedup();
        node.blocks.sort();
        node.blocks.dedup();
    }
}

fn observe_progress(
    nodes: &mut BTreeMap<String, TaskOrderingNode>,
    issues: &mut Vec<TaskOrderingIssue>,
) {
    let states = nodes
        .iter()
        .map(|(task_id, node)| (task_id.clone(), node.state))
        .collect::<BTreeMap<_, _>>();
    for node in nodes.values_mut() {
        node.blocked_by = node
            .predecessors
            .iter()
            .filter(|predecessor| states.get(*predecessor) != Some(&TaskOrderingState::Done))
            .cloned()
            .collect();
        node.ready = node.state == TaskOrderingState::Todo && node.blocked_by.is_empty();
        if matches!(
            node.state,
            TaskOrderingState::Done | TaskOrderingState::InProgress
        ) {
            for predecessor in &node.blocked_by {
                issues.push(TaskOrderingIssue {
                    code: TASK_ORDER_ISSUE_PROGRESS.to_string(),
                    line: Some(node.line),
                    task: Some(node.id.clone()),
                    predecessor: Some(predecessor.clone()),
                    message: format!(
                        "task {} is {} before predecessor {predecessor} is done",
                        node.id,
                        node.state.as_str()
                    ),
                });
            }
        }
    }
}

fn sort_issues(issues: &mut Vec<TaskOrderingIssue>) {
    issues.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then(left.code.cmp(&right.code))
            .then(left.task.cmp(&right.task))
            .then(left.predecessor.cmp(&right.predecessor))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
}

fn marker_prefix(token: &str) -> &str {
    token.split('<').next().unwrap_or(token)
}

fn task_state(trimmed: &str) -> Option<TaskOrderingState> {
    if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
        return Some(TaskOrderingState::Done);
    }
    if trimmed.starts_with("- [~]") {
        return Some(TaskOrderingState::InProgress);
    }
    if trimmed.starts_with("- [ ]") {
        return Some(TaskOrderingState::Todo);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verified::policy::parse_policy_json;

    fn policy() -> Policy {
        parse_policy_json(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .expect("generated policy")
    }

    #[test]
    fn leaves_existing_task_files_in_inactive_compatibility_mode() {
        // r[verify cairn.task_ordering.markers.optional]
        // r[verify cairn.task_ordering.markers.compatibility]
        let graph = build_task_ordering_graph(
            "- [ ] [serial] First r[demo]\n- [ ] [depends:external] Second\n",
            &policy(),
        );

        assert!(!graph.active);
        assert!(graph.valid);
        assert!(graph.nodes.is_empty());
        assert!(graph.issues.is_empty());
    }

    #[test]
    fn ignores_order_markers_in_comments_and_fenced_examples() {
        let markdown = "\
<!-- - [ ] [serial] [task:hidden] Hidden -->\n\
```markdown\n\
- [ ] [serial] [task:example] Example\n\
```\n\
- [ ] [serial] Visible\n";

        let graph = build_task_ordering_graph(markdown, &policy());

        assert!(!graph.active);
        assert!(graph.valid);
        assert!(graph.nodes.is_empty());
    }

    #[test]
    fn orders_valid_tasks_and_reports_only_unblocked_work_as_ready() {
        // r[verify cairn.task_ordering.tests]
        // r[verify cairn.task_ordering.graph.valid]
        // r[verify cairn.task_ordering.progress.ready]
        // r[verify cairn.task_ordering.tests.coverage]
        let markdown = "\
- [x] [serial] [task:build] Build r[demo]\n\
- [ ] [serial] [task:docs] Write docs r[demo]\n\
- [ ] [serial] [task:test] [after:build] Test r[demo]\n";

        let graph = build_task_ordering_graph(markdown, &policy());

        assert!(graph.active);
        assert!(graph.valid, "{:?}", graph.issues);
        assert_eq!(graph.order, vec!["build", "docs", "test"]);
        assert_eq!(graph.ready_tasks, vec!["docs", "test"]);
        assert_eq!(graph.edges.len(), 1);
        let test = graph
            .nodes
            .iter()
            .find(|node| node.id == "test")
            .expect("test node");
        assert_eq!(test.predecessors, vec!["build"]);
        assert!(test.blocked_by.is_empty());
        assert!(test.ready);
    }

    #[test]
    fn rejects_missing_malformed_multiple_and_duplicate_identities() {
        // r[verify cairn.task_ordering.markers.complete]
        // r[verify cairn.task_ordering.graph.invalid]
        let markdown = "\
- [ ] [serial] Missing\n\
- [ ] [serial] [task:] Malformed\n\
- [ ] [serial] [task:a] [task:b] Multiple\n\
- [ ] [serial] [task:duplicate] First\n\
- [ ] [serial] [task:duplicate] Second\n";

        let graph = build_task_ordering_graph(markdown, &policy());
        let codes = graph
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(!graph.valid);
        assert!(graph.order.is_empty());
        assert!(graph.ready_tasks.is_empty());
        for expected in [
            TASK_ORDER_ISSUE_ID_MISSING,
            TASK_ORDER_ISSUE_ID_MALFORMED,
            TASK_ORDER_ISSUE_ID_MULTIPLE,
            TASK_ORDER_ISSUE_ID_DUPLICATE,
        ] {
            assert!(
                codes.contains(expected),
                "missing {expected}: {:?}",
                graph.issues
            );
        }
    }

    #[test]
    fn rejects_malformed_unknown_duplicate_and_self_predecessors() {
        // r[verify cairn.task_ordering.graph.invalid]
        let markdown = "\
- [ ] [serial] [task:a] Root\n\
- [ ] [serial] [task:b] [after:] [after:missing] [after:a] [after:a] Child\n\
- [ ] [serial] [task:c] [after:c] Self\n";

        let graph = build_task_ordering_graph(markdown, &policy());
        let codes = graph
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(!graph.valid);
        for expected in [
            TASK_ORDER_ISSUE_PREDECESSOR_MALFORMED,
            TASK_ORDER_ISSUE_PREDECESSOR_UNKNOWN,
            TASK_ORDER_ISSUE_PREDECESSOR_DUPLICATE,
            TASK_ORDER_ISSUE_PREDECESSOR_SELF,
        ] {
            assert!(
                codes.contains(expected),
                "missing {expected}: {:?}",
                graph.issues
            );
        }
    }

    #[test]
    fn rejects_cycles_and_exposes_no_partial_action_order() {
        // r[verify cairn.task_ordering.graph.invalid]
        let markdown = "\
- [ ] [serial] [task:a] [after:b] A\n\
- [ ] [serial] [task:b] [after:a] B\n";

        let graph = build_task_ordering_graph(markdown, &policy());

        assert!(!graph.valid);
        assert!(graph
            .issues
            .iter()
            .all(|issue| issue.code == TASK_ORDER_ISSUE_CYCLE));
        assert!(graph.order.is_empty());
        assert!(graph.ready_tasks.is_empty());
        assert!(graph.nodes.iter().all(|node| !node.ready));
    }

    #[test]
    fn does_not_report_in_progress_work_as_ready_to_start() {
        let markdown = "- [~] [serial] [task:active] Active\n";

        let graph = build_task_ordering_graph(markdown, &policy());

        assert!(graph.valid);
        assert!(graph.ready_tasks.is_empty());
        assert!(!graph.nodes[0].ready);
    }

    #[test]
    fn rejects_progress_before_predecessor_completion() {
        // r[verify cairn.task_ordering.progress.invalid]
        let markdown = "\
- [ ] [serial] [task:build] Build\n\
- [~] [serial] [task:test] [after:build] Test\n";

        let graph = build_task_ordering_graph(markdown, &policy());

        assert!(!graph.valid);
        assert!(graph
            .issues
            .iter()
            .any(|issue| issue.code == TASK_ORDER_ISSUE_PROGRESS));
        assert!(graph.order.is_empty());
        assert!(graph.ready_tasks.is_empty());
    }
}
