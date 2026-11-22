const EXPECTED_KANBAN_COLUMN_COUNT: usize = 5;
const EXPECTED_ACTIVE_CHANGE_COUNT: usize = 5;
const EXPECTED_TRACEABILITY_EDGE_KIND_COUNT: usize = 4;
const EXPECTED_MINDMAP_MINIMUM_CHILDREN: usize = 2;

#[test]
fn valid_lifecycle_facts_build_all_views_deterministically() {
    // r[verify cairn.lifecycle_views.verification.positive]
    // r[verify cairn.lifecycle_views.determinism.repeat]
    let mut input = positive_input();
    let first = super::build_lifecycle_view_model(input.clone());
    input.changes.reverse();
    input.specs.reverse();
    let second = super::build_lifecycle_view_model(input);

    assert!(first.valid, "{:?}", first.issues);
    assert_eq!(first, second);
    assert_eq!(first.kanban.columns.len(), EXPECTED_KANBAN_COLUMN_COUNT);
    assert_eq!(kanban_card_count(&first), EXPECTED_ACTIVE_CHANGE_COUNT);
    assert_eq!(first.mindmap.kind, super::LifecycleViewNodeKind::Repository);
    assert!(first.mindmap.children.len() >= EXPECTED_MINDMAP_MINIMUM_CHILDREN);

    let edge_kinds = first
        .edges
        .iter()
        .map(|edge| edge.kind)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(edge_kinds.contains(&super::LifecycleViewEdgeKind::Contains));
    assert!(edge_kinds.contains(&super::LifecycleViewEdgeKind::Covers));
    assert!(edge_kinds.contains(&super::LifecycleViewEdgeKind::DependsOn));
    assert!(edge_kinds.contains(&super::LifecycleViewEdgeKind::Blocks));
    let traceability_kinds = [
        super::LifecycleViewEdgeKind::Implements,
        super::LifecycleViewEdgeKind::Verifies,
        super::LifecycleViewEdgeKind::TraceabilityDepends,
        super::LifecycleViewEdgeKind::Related,
    ];
    assert_eq!(
        traceability_kinds
            .iter()
            .filter(|kind| edge_kinds.contains(kind))
            .count(),
        EXPECTED_TRACEABILITY_EDGE_KIND_COUNT
    );
}

#[test]
fn kanban_classification_covers_every_active_lane() {
    // r[verify cairn.lifecycle_views.kanban.classification]
    // r[verify cairn.lifecycle_views.kanban.blocked]
    let model = super::build_lifecycle_view_model(positive_input());
    assert_card_in_lane(&model, "planned", super::KanbanColumnKind::Planned);
    assert_card_in_lane(&model, "ready", super::KanbanColumnKind::Ready);
    assert_card_in_lane(&model, "in-progress", super::KanbanColumnKind::InProgress);
    assert_card_in_lane(&model, "blocked", super::KanbanColumnKind::Blocked);
    assert_card_in_lane(
        &model,
        "ready-to-archive",
        super::KanbanColumnKind::ReadyToArchive,
    );
    let blocked = model
        .kanban
        .columns
        .iter()
        .find(|column| column.kind == super::KanbanColumnKind::Blocked)
        .and_then(|column| column.cards.first())
        .expect("blocked card");
    assert!(!blocked.blocked_by.is_empty());
    assert!(!blocked.reasons.is_empty());
}

#[test]
fn malformed_outline_and_dangling_task_link_fail_closed() {
    // r[verify cairn.lifecycle_views.verification.negative]
    // r[verify cairn.lifecycle_views.knowledge_graph.dangling]
    let change = change(
        "broken",
        None,
        &[],
        Some("- [ ] [serial] Implement unknown link. r[demo.unknown]\n"),
    );
    let graph = crate::verified::change_dependencies::build_change_dependency_graph(
        std::slice::from_ref(&change.status),
        &[],
    );
    let model = super::build_lifecycle_view_model(super::LifecycleViewInput {
        repository: "broken-fixture".to_string(),
        changes: vec![change],
        specs: vec![crate::verified::repo::SpecSource {
            id: "broken".to_string(),
            path: ".cairn/specs/broken/spec.md".to_string(),
            markdown: "# Broken\n\n### Requirement: Malformed marker r[bad/id]\nThe system MUST fail closed.\n\n#### Scenario: Compatibility identity\n- GIVEN malformed lifecycle input\n- WHEN Cairn builds a view\n- THEN it reports an issue\n".to_string(),
        }],
        dependency_graph: graph,
        traceability: None,
    });

    assert!(!model.valid);
    let codes = model
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(codes.contains("lifecycle_view.marker.malformed"));
    assert!(codes.contains("lifecycle_view.task.dangling_reference"));
    assert!(model.nodes.iter().any(|node| {
        node.kind == super::LifecycleViewNodeKind::MissingRequirement
            && node.reference.as_deref() == Some("demo.unknown")
    }));
}

fn positive_input() -> super::LifecycleViewInput {
    let changes = vec![
        change("planned", Some("planned"), &[], None),
        change(
            "ready",
            None,
            &[],
            Some("- [ ] [serial] Implement requirement. r[demo.requirement]\n"),
        ),
        change(
            "in-progress",
            Some("current"),
            &[],
            Some("- [~] [serial] Implement scenario. r[demo.requirement.happy]\n"),
        ),
        change(
            "blocked",
            None,
            &["ready-to-archive"],
            Some("- [ ] [serial] Wait for dependency. r[demo.requirement]\n"),
        ),
        change(
            "ready-to-archive",
            None,
            &[],
            Some("- [x] [serial] Complete requirement. r[demo.requirement]\n"),
        ),
    ];
    let statuses = changes
        .iter()
        .map(|change| change.status.clone())
        .collect::<Vec<_>>();
    let graph = crate::verified::change_dependencies::build_change_dependency_graph(&statuses, &[]);
    let spec = accepted_spec();
    let traceability = crate::verified::traceability::classify_traceability_coverage(
        &traceability_profile(),
        &[crate::verified::traceability::TraceabilityFile {
            path: spec.path.clone(),
            text: spec.markdown.clone(),
        }],
        &[crate::verified::traceability::TraceabilityFile {
            path: "src/example.rs".to_string(),
            text: [
                role_marker(
                    crate::verified::traceability::TRACEABILITY_VERB_IMPL,
                    "demo.requirement",
                ),
                role_marker(
                    crate::verified::traceability::TRACEABILITY_VERB_VERIFY,
                    "demo.requirement.happy",
                ),
                role_marker(
                    crate::verified::traceability::TRACEABILITY_VERB_DEPENDS,
                    "demo.requirement",
                ),
                role_marker(
                    crate::verified::traceability::TRACEABILITY_VERB_RELATED,
                    "demo.requirement.happy",
                ),
            ]
            .join("\n"),
        }],
    );
    super::LifecycleViewInput {
        repository: "fixture".to_string(),
        changes,
        specs: vec![spec],
        dependency_graph: graph,
        traceability: Some(traceability),
    }
}

fn accepted_spec() -> crate::verified::repo::SpecSource {
    crate::verified::repo::SpecSource {
        id: "demo".to_string(),
        path: ".cairn/specs/demo/spec.md".to_string(),
        markdown: "# Demo Specification\n\n## Requirements\n\n### Requirement: Demonstrate lifecycle views\nr[demo.requirement] The system MUST expose lifecycle views.\n\n#### Scenario: Happy path\nr[demo.requirement.happy]\n- GIVEN valid lifecycle facts\n- WHEN Cairn builds a view\n- THEN the view is deterministic\n".to_string(),
    }
}

fn change(
    name: &str,
    status: Option<&str>,
    dependencies: &[&str],
    tasks_markdown: Option<&str>,
) -> super::LifecycleViewChangeInput {
    let task_counts = tasks_markdown.map(crate::verified::repo::count_tasks);
    super::LifecycleViewChangeInput {
        status: crate::verified::repo::ChangeStatus {
            name: name.to_string(),
            path: format!(".cairn/changes/{name}"),
            has_proposal: true,
            has_design: true,
            has_tasks: tasks_markdown.is_some(),
            delta_specs: vec![format!(".cairn/changes/{name}/specs/demo/spec.md")],
            task_counts,
            metadata: crate::verified::repo::ChangeMetadata {
                groups: Vec::new(),
                target_date: None,
                status: status.map(str::to_string),
                depends_on: dependencies
                    .iter()
                    .map(|dependency| (*dependency).to_string())
                    .collect(),
                cross_repo_dependencies: Vec::new(),
            },
            metadata_issues: Vec::new(),
        },
        tasks_markdown: tasks_markdown.map(str::to_string),
    }
}

fn role_marker(verb: &str, requirement: &str) -> String {
    format!("// {}[{verb} {requirement}]", "r")
}

fn traceability_profile() -> crate::verified::traceability::TraceabilityProfile {
    crate::verified::traceability::TraceabilityProfile {
        id: "test".to_string(),
        requirement_sources: vec![crate::verified::traceability::TraceabilitySource {
            root: ".cairn/specs".to_string(),
            source_kind: "accepted_spec".to_string(),
            extensions: vec!["md".to_string()],
        }],
        evidence_sources: vec![crate::verified::traceability::TraceabilitySource {
            root: "src".to_string(),
            source_kind: "rust_source".to_string(),
            extensions: vec!["rs".to_string()],
        }],
        marker_verbs: vec![
            crate::verified::traceability::TRACEABILITY_VERB_IMPL.to_string(),
            crate::verified::traceability::TRACEABILITY_VERB_VERIFY.to_string(),
            crate::verified::traceability::TRACEABILITY_VERB_DEPENDS.to_string(),
            crate::verified::traceability::TRACEABILITY_VERB_RELATED.to_string(),
        ],
        assurance_level: "declared".to_string(),
        anchor_kind: "exact_marker_bytes".to_string(),
        rust_configuration_path: None,
    }
}

fn kanban_card_count(model: &super::LifecycleViewModel) -> usize {
    model
        .kanban
        .columns
        .iter()
        .map(|column| column.cards.len())
        .sum()
}

fn assert_card_in_lane(
    model: &super::LifecycleViewModel,
    change: &str,
    expected: super::KanbanColumnKind,
) {
    assert!(
        model
            .kanban
            .columns
            .iter()
            .find(|column| column.kind == expected)
            .is_some_and(|column| column.cards.iter().any(|card| card.change == change)),
        "change {change} missing from {}",
        expected.as_str()
    );
}
