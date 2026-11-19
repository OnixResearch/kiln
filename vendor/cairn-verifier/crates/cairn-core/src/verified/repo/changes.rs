pub fn build_change_status(mut source: super::ChangeSource) -> super::ChangeStatus {
    // r[impl cairn.supersede.change_metadata.schema]
    // r[impl cairn.supersede.change_metadata.validation]
    source.delta_specs.sort();
    let task_counts = source
        .tasks_markdown
        .as_deref()
        .map(super::tasks::count_tasks);
    let (metadata, metadata_issues) =
        super::metadata::parse_change_metadata(&source.name, source.metadata_json.as_deref());
    super::ChangeStatus {
        name: source.name,
        path: source.path,
        has_proposal: source.has_proposal,
        has_design: source.has_design,
        has_tasks: source.has_tasks,
        delta_specs: source.delta_specs,
        task_counts,
        metadata,
        metadata_issues,
    }
}

pub fn build_change_list(mut changes: Vec<super::ChangeStatus>) -> super::ChangeList {
    // r[impl cairn.supersede.read_parity]
    // r[impl cairn.supersede.read_parity.change_list]
    // r[impl cairn.supersede.change_metadata.read_receipts]
    // r[impl cairn.supersede.change_metadata.filters]
    // r[impl cairn.supersede.boundary]
    // r[impl cairn.supersede.boundary.read_models]
    changes.sort_by(|left, right| left.name.cmp(&right.name));
    super::ChangeList { changes }
}

pub fn filter_changes_by_metadata(
    changes: Vec<super::ChangeStatus>,
    group: Option<&str>,
    target_after: Option<&str>,
    target_before: Option<&str>,
) -> Vec<super::ChangeStatus> {
    // r[impl cairn.supersede.change_metadata.filters]
    changes
        .into_iter()
        .filter(|change| matches_metadata_filter(change, group, target_after, target_before))
        .collect()
}

fn matches_metadata_filter(
    change: &super::ChangeStatus,
    group: Option<&str>,
    target_after: Option<&str>,
    target_before: Option<&str>,
) -> bool {
    if let Some(group) = group {
        if !change
            .metadata
            .groups
            .iter()
            .any(|candidate| candidate == group)
        {
            return false;
        }
    }
    if target_after.is_some() || target_before.is_some() {
        let Some(target_date) = change.metadata.target_date.as_deref() else {
            return false;
        };
        if target_after.is_some_and(|bound| target_date <= bound) {
            return false;
        }
        if target_before.is_some_and(|bound| target_date >= bound) {
            return false;
        }
    }
    true
}

pub fn build_change_detail(change: super::ChangeStatus) -> super::ChangeDetail {
    // r[impl cairn.supersede.read_parity.change_show]
    super::ChangeDetail { change }
}
