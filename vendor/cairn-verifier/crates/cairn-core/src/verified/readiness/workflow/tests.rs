use super::*;
use crate::verified::readiness::{
    derive_readiness, readiness_cause, ReadinessObservation, ReadinessReport, ReadinessScope,
    ReadinessSnapshot, READINESS_STAGES,
};
use crate::{ChangeDependencyGraph, ChangeDependencyNode};

fn scope(change: &str) -> ReadinessScope {
    ReadinessScope {
        change: change.into(),
        policy_hash: "a".repeat(64),
        source_hash: "b".repeat(64),
        evidence_hash: "c".repeat(64),
    }
}

fn snapshot(change: &str) -> ReadinessSnapshot {
    let scope = scope(change);
    ReadinessSnapshot {
        scope: scope.clone(),
        observations: READINESS_STAGES
            .iter()
            .map(|stage| ReadinessObservation {
                scope: scope.clone(),
                stage: (*stage).into(),
                state: ReadinessState::Ready,
                decision: json!({"valid": true, "verdict": "PASS"}),
                causes: Vec::new(),
            })
            .collect(),
    }
}

fn report(change: &str) -> ReadinessReport {
    derive_readiness(&snapshot(change))
}

fn complete_graph(order: &[&str]) -> ChangeDependencyGraph {
    ChangeDependencyGraph {
        valid: true,
        nodes: order
            .iter()
            .map(|name| ChangeDependencyNode {
                name: (*name).to_string(),
                dependencies: Vec::new(),
                satisfied_dependencies: Vec::new(),
                blocked_by: Vec::new(),
                blocks: Vec::new(),
                dependency_ready: true,
            })
            .collect(),
        edges: Vec::new(),
        issues: Vec::new(),
        drain_order: order.iter().map(|name| (*name).to_string()).collect(),
    }
}

fn set_blocked(snapshot: &mut ReadinessSnapshot, stage: usize, code: &str, target: &str) {
    snapshot.observations[stage].state = ReadinessState::Blocked;
    snapshot.observations[stage].causes =
        vec![readiness_cause(code, target, "observed diagnostic")];
}

fn task_order(ready: &[String]) -> DrainTaskOrdering<'_> {
    DrainTaskOrdering {
        available: true,
        active: true,
        valid: true,
        ready_tasks: ready,
    }
}

#[test]
// r[verify cairn.agent_workflow_readiness.compatibility]
fn checked_projection_is_admitted_from_generated_policy() {
    let policy = workflow_readiness_policy().expect("checked workflow readiness projection");
    assert!(policy.views.iter().any(|view| view == &policy.default_view));
    assert!(policy
        .views
        .iter()
        .any(|view| view == &policy.readiness_view));
    assert_ne!(policy.default_view, policy.readiness_view);
    assert!(policy.bounds.max_actions > 0);
    assert!(policy
        .non_claims
        .iter()
        .any(|claim| claim.contains("advisory")));
}

#[test]
// r[verify cairn.agent_workflow_readiness.compatibility]
fn policy_admission_rejects_inconsistent_views_and_non_positive_bounds() {
    let mut policy = workflow_readiness_policy()
        .expect("checked projection")
        .clone();
    policy.views = vec!["legacy".into(), "legacy".into()];
    assert_eq!(
        admit_workflow_readiness_policy(&policy),
        Err(WORKFLOW_READINESS_POLICY_INVALID)
    );

    let mut policy = workflow_readiness_policy()
        .expect("checked projection")
        .clone();
    policy.bounds.max_actions = 0;
    assert_eq!(
        admit_workflow_readiness_policy(&policy),
        Err(WORKFLOW_READINESS_POLICY_BOUNDS)
    );

    let mut policy = workflow_readiness_policy()
        .expect("checked projection")
        .clone();
    policy.action_order.retain(|kind| kind != "task");
    assert_eq!(
        admit_workflow_readiness_policy(&policy),
        Err(WORKFLOW_READINESS_POLICY_INVALID)
    );
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
// r[verify cairn.agent_workflow_readiness.shared]
fn dependency_order_precedes_actions_and_blocked_predecessors_cannot_execute() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let alpha_report = report("alpha");
    let beta_report = report("beta");
    assert!(alpha_report
        .actions
        .iter()
        .any(|action| action.kind == "sync_preview"));
    let mut graph = complete_graph(&["alpha", "beta"]);
    graph.nodes[1].blocked_by = vec!["alpha".into()];
    graph.nodes[1].dependency_ready = false;
    let beta_ready_tasks = vec!["test".to_string()];
    let changes = [
        DrainReadinessChange {
            name: "beta",
            report: &beta_report,
            blocked_by: graph.nodes[1].blocked_by.as_slice(),
            dependency_issue_count: 0,
            task_ordering: task_order(beta_ready_tasks.as_slice()),
        },
        DrainReadinessChange {
            name: "alpha",
            report: &alpha_report,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: DrainTaskOrdering {
                available: false,
                active: false,
                valid: false,
                ready_tasks: &[],
            },
        },
    ];
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    assert_eq!(selection.change_order, vec!["alpha", "beta"]);
    assert_eq!(selection.changes[0].name, "alpha");
    assert!(selection.changes[0]
        .actions
        .iter()
        .any(|action| action.kind == "sync_preview"));
    let beta = &selection.changes[1];
    assert!(beta.ordering_blocked);
    assert!(!beta.task_eligible);
    assert!(beta.next_task.is_none());
    assert!(beta
        .actions
        .iter()
        .all(|action| !policy.preview_kinds.contains(&action.kind)));
    assert!(beta.actions.iter().all(|action| action.kind != "task"));
    assert_eq!(selection.ready_change_count, 1);
    assert_eq!(selection.blocked_change_count, 1);
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
fn task_selection_uses_declared_ready_order_and_exact_instruction_route() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let mut snapshot = snapshot("demo");
    snapshot.observations[4].state = ReadinessState::Unavailable;
    snapshot.observations[4].causes = Vec::new();
    let report = derive_readiness(&snapshot);
    let ready_tasks = vec!["build".to_string(), "test".to_string()];
    let graph = complete_graph(&["demo"]);
    let changes = [DrainReadinessChange {
        name: "demo",
        report: &report,
        blocked_by: &[],
        dependency_issue_count: 0,
        task_ordering: task_order(ready_tasks.as_slice()),
    }];
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    let change = &selection.changes[0];
    assert!(change.task_eligible);
    let task = change.next_task.as_ref().expect("next ready task");
    assert_eq!(task.task, "build");
    assert_eq!(task.instruction.tool, policy.instruction.tool);
    assert_eq!(
        task.instruction.arguments,
        json!({"change": "demo", "artifact": policy.instruction.task_artifact})
    );
    assert_eq!(
        task.instruction.argv,
        vec![
            "cairn",
            "workflow",
            "instructions",
            "tasks",
            "--change",
            "demo"
        ]
    );
    assert_eq!(change.actions[0].kind, "task");
    assert_eq!(change.actions[0].target, "build");
    assert!(change
        .actions
        .iter()
        .all(|action| !policy.preview_kinds.contains(&action.kind)));
}

#[test]
// r[verify cairn.agent_workflow_readiness.shared]
fn unavailable_observations_and_unresolved_causes_stay_explicit_and_advisory() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let mut snapshot = snapshot("demo");
    snapshot.observations.truncate(1);
    set_blocked(&mut snapshot, 0, "workflow.invalid", "demo");
    let report = derive_readiness(&snapshot);
    assert!(report
        .stages
        .iter()
        .skip(1)
        .all(|stage| stage.state == ReadinessState::Unavailable));
    let graph = complete_graph(&["demo"]);
    let changes = [DrainReadinessChange {
        name: "demo",
        report: &report,
        blocked_by: &[],
        dependency_issue_count: 0,
        task_ordering: DrainTaskOrdering {
            available: false,
            active: false,
            valid: false,
            ready_tasks: &[],
        },
    }];
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    let change = &selection.changes[0];
    assert_eq!(selection.unavailable_change_count, 1);
    assert!(change.next_task.is_none());
    assert!(change
        .actions
        .iter()
        .any(|action| action.kind == "unresolved"));
    assert!(change
        .actions
        .iter()
        .all(|action| action.authority == "advisory"));
    assert!(change
        .actions
        .iter()
        .all(|action| !action.argv.iter().any(|part| part == "--execute")));
    assert!(change
        .actions
        .iter()
        .all(|action| !policy.preview_kinds.contains(&action.kind)));
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
fn selected_actions_follow_declared_action_order() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let mut snapshot = snapshot("demo");
    set_blocked(&mut snapshot, 0, "unreviewed.diagnostic", "demo");
    set_blocked(&mut snapshot, 5, "dependency.blocked", "predecessor");
    let report = derive_readiness(&snapshot);
    let graph = complete_graph(&["demo"]);
    let changes = [DrainReadinessChange {
        name: "demo",
        report: &report,
        blocked_by: &[],
        dependency_issue_count: 0,
        task_ordering: DrainTaskOrdering {
            available: false,
            active: false,
            valid: false,
            ready_tasks: &[],
        },
    }];
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    let ranks = selection.changes[0]
        .actions
        .iter()
        .map(|action| {
            policy
                .action_order
                .iter()
                .position(|kind| kind == &action.kind)
                .unwrap_or(policy.action_order.len())
        })
        .collect::<Vec<_>>();
    assert!(ranks.windows(2).all(|pair| pair[0] <= pair[1]), "{ranks:?}");
    assert_eq!(selection.changes[0].actions[0].kind, "dependency_wait");
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
fn action_bounds_truncate_in_dependency_order() {
    let mut policy = workflow_readiness_policy()
        .expect("checked projection")
        .clone();
    policy.bounds.max_actions = 1;
    let first = report("first");
    let second = report("second");
    let empty: Vec<String> = Vec::new();
    let no_tasks = DrainTaskOrdering {
        available: false,
        active: false,
        valid: false,
        ready_tasks: empty.as_slice(),
    };
    let changes = [
        DrainReadinessChange {
            name: "second",
            report: &second,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: no_tasks,
        },
        DrainReadinessChange {
            name: "first",
            report: &first,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: no_tasks,
        },
    ];
    let graph = complete_graph(&["first", "second"]);
    let selection = select_drain_readiness(&changes, &graph, &policy).expect("bounded selection");
    assert!(selection.truncated);
    assert_eq!(selection.changes[0].name, "first");
    assert_eq!(selection.changes[0].actions.len(), 1);
    assert!(selection.changes[1].actions.is_empty());
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
fn changes_absent_from_the_declared_order_follow_it_deterministically() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let first = report("first");
    let second = report("second");
    let empty: Vec<String> = Vec::new();
    let no_tasks = DrainTaskOrdering {
        available: false,
        active: false,
        valid: false,
        ready_tasks: empty.as_slice(),
    };
    let changes = [
        DrainReadinessChange {
            name: "second",
            report: &second,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: no_tasks,
        },
        DrainReadinessChange {
            name: "first",
            report: &first,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: no_tasks,
        },
    ];
    let graph = complete_graph(&["first"]);
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    assert_eq!(selection.change_order, vec!["first", "second"]);
}

#[test]
// r[verify cairn.agent_workflow_readiness.compatibility]
fn admitted_changes_beyond_the_declared_bound_are_rejected() {
    let mut policy = workflow_readiness_policy()
        .expect("checked projection")
        .clone();
    policy.bounds.max_changes = 1;
    let report = report("demo");
    let graph = complete_graph(&["demo"]);
    let changes = [
        DrainReadinessChange {
            name: "demo",
            report: &report,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: DrainTaskOrdering {
                available: false,
                active: false,
                valid: false,
                ready_tasks: &[],
            },
        },
        DrainReadinessChange {
            name: "demo",
            report: &report,
            blocked_by: &[],
            dependency_issue_count: 0,
            task_ordering: DrainTaskOrdering {
                available: false,
                active: false,
                valid: false,
                ready_tasks: &[],
            },
        },
    ];
    assert_eq!(
        select_drain_readiness(&changes, &graph, &policy),
        Err(WORKFLOW_READINESS_BOUND)
    );
}

#[test]
// r[verify cairn.agent_workflow_readiness.order]
fn dependency_graph_issues_block_task_selection_without_inventing_actions() {
    let policy = workflow_readiness_policy().expect("checked projection");
    let report = report("demo");
    let ready = vec!["build".to_string()];
    let graph = complete_graph(&["demo"]);
    let changes = [DrainReadinessChange {
        name: "demo",
        report: &report,
        blocked_by: &[],
        dependency_issue_count: 1,
        task_ordering: task_order(ready.as_slice()),
    }];
    let selection = select_drain_readiness(&changes, &graph, policy).expect("bounded selection");
    let change = &selection.changes[0];
    assert!(change.ordering_blocked);
    assert!(!change.task_eligible);
    assert!(change.next_task.is_none());
    assert!(change
        .actions
        .iter()
        .all(|action| !policy.preview_kinds.contains(&action.kind)));
}
