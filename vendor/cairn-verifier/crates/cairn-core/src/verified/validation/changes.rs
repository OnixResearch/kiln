const CHANGE_ISSUES_PER_CHANGE_HINT: usize = 3;

pub fn validate_changes(
    changes: &[crate::verified::repo::ChangeStatus],
    policy: &crate::verified::policy::Policy,
) -> Vec<String> {
    validate_changes_with_archives(changes, &[], policy)
}

pub fn validate_changes_with_archives(
    changes: &[crate::verified::repo::ChangeStatus],
    archived_changes: &[String],
    policy: &crate::verified::policy::Policy,
) -> Vec<String> {
    let mut issues =
        Vec::with_capacity(changes.len().saturating_mul(CHANGE_ISSUES_PER_CHANGE_HINT));
    for change in changes {
        issues.extend(change_issues(change, policy));
    }
    let dependency_graph = crate::verified::change_dependencies::build_change_dependency_graph(
        changes,
        archived_changes,
    );
    issues.extend(
        crate::verified::change_dependencies::change_dependency_issue_messages(&dependency_graph),
    );
    issues
}

pub fn change_issues(
    change: &crate::verified::repo::ChangeStatus,
    policy: &crate::verified::policy::Policy,
) -> Vec<String> {
    let mut issues = Vec::with_capacity(CHANGE_ISSUES_PER_CHANGE_HINT);
    if !change.has_proposal {
        issues.push(format!("{}: missing proposal.md", change.name));
    }
    if !change.has_tasks {
        issues.push(format!("{}: missing tasks.md", change.name));
    }
    if let Some(message) = unmarked_tasks_issue(change, policy) {
        issues.push(message);
    }
    issues.extend(crate::verified::repo::validate_change_metadata_policy(
        change,
        &policy.change_metadata_allowed_groups,
        &policy.change_metadata_group_prefixes,
        &policy.change_metadata_statuses,
    ));
    issues
}

fn unmarked_tasks_issue(
    change: &crate::verified::repo::ChangeStatus,
    policy: &crate::verified::policy::Policy,
) -> Option<String> {
    let counts = change.task_counts?;
    if !policy.task_markers_enabled {
        return None;
    }
    if policy.task_marker_strictness != "strict" {
        return None;
    }
    if counts.unmarked == 0 {
        return None;
    }
    Some(format!(
        "{}: {} task(s) missing concurrency markers",
        change.name, counts.unmarked
    ))
}
