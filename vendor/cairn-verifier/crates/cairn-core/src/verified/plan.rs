use crate::verified::artifact_workflow::{WORKFLOW_SPEC_EFFECT_DELTA, WORKFLOW_SPEC_EFFECT_NONE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    pub change: String,
    pub dry_run: bool,
    pub mutated: bool,
    pub actions: Vec<PlanAction>,
    pub delta_specs: Vec<String>,
    pub spec_effect: String,
    pub blocked: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivePlan {
    pub change: String,
    pub dry_run: bool,
    pub mutated: bool,
    pub actions: Vec<PlanAction>,
    pub spec_effect: String,
    pub blocked: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanAction {
    pub kind: PlanActionKind,
    pub path: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanActionKind {
    SyncDeltaSpec,
    ArchiveChange,
}

pub fn build_sync_plan(change: &crate::verified::repo::ChangeStatus) -> SyncPlan {
    build_sync_plan_with_dependency_blockers(change, &[])
}

pub fn build_sync_plan_with_dependency_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
) -> SyncPlan {
    build_sync_plan_for_effect_with_dependency_blockers(
        change,
        dependency_blockers,
        WORKFLOW_SPEC_EFFECT_DELTA,
    )
}

pub fn build_sync_plan_for_effect_with_dependency_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
    spec_effect: &str,
) -> SyncPlan {
    // r[impl sync_archive.plan.sync] r[impl sync_archive.plan.sync.delta_specs]
    // r[impl sync_archive.plan.purity] r[impl sync_archive.plan.purity.no_effects]
    // r[impl sync_archive.execute.sync] r[impl sync_archive.execute.sync.applies]
    // r[impl sync_archive.execute.sync.default_dry_run]
    // r[impl cairn.change_dependency_graph.mutation_block]
    // r[impl cairn.lifecycle_workflow_profiles.no_spec_sync]
    // r[impl cairn.lifecycle_workflow_profiles.functional_boundary]
    let reasons = sync_blockers(change, dependency_blockers, spec_effect);
    let actions = if reasons.is_empty() && spec_effect == WORKFLOW_SPEC_EFFECT_DELTA {
        change
            .delta_specs
            .iter()
            .map(|path| PlanAction {
                kind: PlanActionKind::SyncDeltaSpec,
                path: path.clone(),
                description: format!("merge delta spec into main specs: {path}"),
            })
            .collect()
    } else {
        Vec::new()
    };

    SyncPlan {
        change: change.name.clone(),
        dry_run: true,
        mutated: false,
        actions,
        delta_specs: change.delta_specs.clone(),
        spec_effect: spec_effect.to_string(),
        blocked: !reasons.is_empty(),
        reasons,
    }
}

pub fn build_archive_plan(change: &crate::verified::repo::ChangeStatus) -> ArchivePlan {
    build_archive_plan_with_dependency_blockers(change, &[])
}

pub fn build_archive_plan_with_dependency_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
) -> ArchivePlan {
    build_archive_plan_for_effect_with_dependency_blockers(
        change,
        dependency_blockers,
        WORKFLOW_SPEC_EFFECT_DELTA,
    )
}

pub fn build_archive_plan_for_effect_with_dependency_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
    spec_effect: &str,
) -> ArchivePlan {
    // r[impl sync_archive.plan.archive] r[impl sync_archive.plan.archive.blockers]
    // r[impl sync_archive.execute.archive] r[impl sync_archive.execute.archive.moves]
    // r[impl sync_archive.execute.archive.blocked_noop] r[impl sync_archive.execute.boundary]
    // r[impl sync_archive.execute.boundary.pure_core]
    // r[impl cairn.change_dependency_graph.mutation_block]
    let reasons = archive_blockers(change, dependency_blockers, spec_effect);
    let actions = if reasons.is_empty() {
        vec![PlanAction {
            kind: PlanActionKind::ArchiveChange,
            path: change.path.clone(),
            description: format!("move active change to archive: {}", change.name),
        }]
    } else {
        Vec::new()
    };

    ArchivePlan {
        change: change.name.clone(),
        dry_run: true,
        mutated: false,
        actions,
        spec_effect: spec_effect.to_string(),
        blocked: !reasons.is_empty(),
        reasons,
    }
}

fn sync_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
    spec_effect: &str,
) -> Vec<String> {
    let mut reasons = required_artifact_blockers(change);
    match spec_effect {
        WORKFLOW_SPEC_EFFECT_DELTA if change.delta_specs.is_empty() => {
            reasons.push(format!("{}: no delta specs to sync", change.name));
        }
        WORKFLOW_SPEC_EFFECT_NONE if !change.delta_specs.is_empty() => {
            reasons.push(format!(
                "{}: no-spec profile forbids delta specs",
                change.name
            ));
        }
        WORKFLOW_SPEC_EFFECT_DELTA | WORKFLOW_SPEC_EFFECT_NONE => {}
        _ => reasons.push(format!(
            "{}: unsupported workflow spec effect: {spec_effect}",
            change.name
        )),
    }
    reasons.extend(dependency_blockers.iter().cloned());
    reasons.sort();
    reasons.dedup();
    reasons
}

fn archive_blockers(
    change: &crate::verified::repo::ChangeStatus,
    dependency_blockers: &[String],
    spec_effect: &str,
) -> Vec<String> {
    let mut reasons = sync_blockers(change, dependency_blockers, spec_effect);
    if let Some(message) = incomplete_tasks_blocker(change.name.as_str(), change.task_counts) {
        reasons.push(message);
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn required_artifact_blockers(change: &crate::verified::repo::ChangeStatus) -> Vec<String> {
    let mut reasons = Vec::with_capacity(3);
    if let Err(issue) =
        crate::verified::nominal_identity::ChangeId::parse(&change.name, "plan.change_id")
    {
        reasons.push(format!("{}: {}", change.name, issue.code));
    }
    if !change.has_proposal {
        reasons.push(format!("{}: missing proposal.md", change.name));
    }
    if !change.has_tasks {
        reasons.push(format!("{}: missing tasks.md", change.name));
    }
    reasons
}

fn incomplete_tasks_blocker(
    change: &str,
    counts: Option<crate::verified::repo::TaskCounts>,
) -> Option<String> {
    let counts = counts?;
    if counts.todo == 0 && counts.in_progress == 0 && counts.unmarked == 0 {
        return None;
    }
    Some(format!(
        "{change}: tasks not archive-ready (todo: {}, in_progress: {}, unmarked: {})",
        counts.todo, counts.in_progress, counts.unmarked
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_change() -> crate::verified::repo::ChangeStatus {
        crate::verified::repo::ChangeStatus {
            name: "demo".to_string(),
            path: "openspec/changes/demo".to_string(),
            has_proposal: true,
            has_design: true,
            has_tasks: true,
            delta_specs: vec!["openspec/changes/demo/specs/demo/spec.md".to_string()],
            task_counts: Some(crate::verified::repo::TaskCounts {
                total: 2,
                done: 2,
                in_progress: 0,
                todo: 0,
                unmarked: 0,
            }),
            metadata: Default::default(),
            metadata_issues: Vec::new(),
        }
    }

    #[test]
    fn builds_sync_plan_for_delta_specs() {
        let plan = build_sync_plan(&complete_change());
        assert!(!plan.blocked);
        assert!(!plan.mutated);
        assert!(plan.dry_run);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].kind, PlanActionKind::SyncDeltaSpec);
        assert_eq!(plan.delta_specs.len(), 1);
    }

    #[test]
    fn blocks_malformed_change_identity_before_mutation() {
        // r[verify cairn.nominal_evidence_identities.lifecycle.validation]
        let mut change = complete_change();
        change.name = "wrong:change".to_string();
        let plan = build_archive_plan(&change);
        assert!(plan.blocked);
        assert!(plan.actions.is_empty());
        assert!(plan
            .reasons
            .iter()
            .any(|reason| reason.contains("identity_prefix_unsupported")));
    }

    #[test]
    fn blocks_sync_without_delta_specs() {
        let mut change = complete_change();
        change.delta_specs.clear();
        let plan = build_sync_plan(&change);
        assert!(plan.blocked);
        assert!(plan.actions.is_empty());
        assert_eq!(
            plan.reasons,
            vec!["demo: no delta specs to sync".to_string()]
        );
    }

    #[test]
    fn blocks_archive_until_tasks_are_complete_and_marked() {
        let mut change = complete_change();
        change.task_counts = Some(crate::verified::repo::TaskCounts {
            total: 2,
            done: 1,
            in_progress: 1,
            todo: 0,
            unmarked: 1,
        });
        let plan = build_archive_plan(&change);
        assert!(plan.blocked);
        assert!(plan.actions.is_empty());
        assert_eq!(
            plan.reasons,
            vec![
                "demo: tasks not archive-ready (todo: 0, in_progress: 1, unmarked: 1)".to_string()
            ]
        );
    }

    #[test]
    fn builds_archive_action_for_complete_change() {
        let plan = build_archive_plan(&complete_change());
        assert!(!plan.blocked);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].kind, PlanActionKind::ArchiveChange);
    }

    #[test]
    fn no_spec_sync_is_an_explicit_unblocked_no_op() {
        // r[verify cairn.lifecycle_workflow_profiles.no_spec_sync.preview]
        let mut change = complete_change();
        change.delta_specs.clear();
        let plan = build_sync_plan_for_effect_with_dependency_blockers(
            &change,
            &[],
            crate::verified::artifact_workflow::WORKFLOW_SPEC_EFFECT_NONE,
        );
        assert!(!plan.blocked);
        assert!(plan.actions.is_empty());
        assert_eq!(
            plan.spec_effect,
            crate::verified::artifact_workflow::WORKFLOW_SPEC_EFFECT_NONE
        );
    }

    #[test]
    fn no_spec_effect_rejects_conflicting_delta_files() {
        // r[verify cairn.lifecycle_workflow_profiles.no_spec.conflict]
        let plan = build_sync_plan_for_effect_with_dependency_blockers(
            &complete_change(),
            &[],
            crate::verified::artifact_workflow::WORKFLOW_SPEC_EFFECT_NONE,
        );
        assert!(plan.blocked);
        assert!(plan.actions.is_empty());
        assert!(plan
            .reasons
            .iter()
            .any(|reason| reason.contains("no-spec profile forbids delta specs")));
    }

    #[test]
    fn dependency_blockers_prevent_sync_and_archive_actions() {
        // r[verify cairn.change_dependency_graph.mutation_block]
        let blockers = vec![
            "demo: change_dependency.unsatisfied_dependency: dependency prereq is not archived"
                .to_string(),
        ];
        let sync_plan = build_sync_plan_with_dependency_blockers(&complete_change(), &blockers);
        assert!(sync_plan.blocked);
        assert!(sync_plan.actions.is_empty());
        assert_eq!(sync_plan.reasons, blockers);

        let archive_plan =
            build_archive_plan_with_dependency_blockers(&complete_change(), &blockers);
        assert!(archive_plan.blocked);
        assert!(archive_plan.actions.is_empty());
        assert_eq!(archive_plan.reasons, blockers);
    }
}
