mod changes;
mod metadata;
mod specs;
mod status;
mod tasks;
mod types;

// r[impl cli_maintainability.repo_core_modules.focused_modules]
// r[impl cli_maintainability.repo_core_modules.focused_modules.visible_boundaries]
// r[impl cli_maintainability.repo_core_modules.focused_modules.narrow_bridges]
// r[impl cli_maintainability.repo_core_modules.behavior_preserved]

pub use changes::{
    build_change_detail, build_change_list, build_change_status, filter_changes_by_metadata,
};
pub use metadata::validate_change_metadata_policy;
pub use specs::{build_spec_detail, build_spec_list, spec_title};
pub use status::build_repo_status;
pub use tasks::count_tasks;
pub use types::{
    ChangeDetail, ChangeList, ChangeMetadata, ChangeSource, ChangeStatus, RepoStatus, SpecDetail,
    SpecList, SpecSource, SpecSummary, TaskCounts,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_task_states_and_markers() {
        let counts = count_tasks("- [ ] [serial] todo\n- [x] done\n- [~] [depends:a] doing\n");
        assert_eq!(counts.total, 3);
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.done, 1);
        assert_eq!(counts.in_progress, 1);
        assert_eq!(counts.unmarked, 1);
    }

    #[test]
    fn builds_sorted_repo_status_from_shell_snapshot() {
        let status = build_repo_status(
            "fixture".to_string(),
            vec![
                "openspec/specs/z/spec.md".to_string(),
                "openspec/specs/a/spec.md".to_string(),
            ],
            vec![ChangeSource {
                name: "demo".to_string(),
                path: "openspec/changes/demo".to_string(),
                has_proposal: true,
                has_design: false,
                has_tasks: true,
                delta_specs: vec!["openspec/changes/demo/specs/a/spec.md".to_string()],
                tasks_markdown: Some("- [ ] [serial] todo\n- [x] done\n".to_string()),
                metadata_json: Some(r#"{"groups":["future","current"],"target_date":"2026-05-21","status":"planned","depends_on":["blocked-by"]}"#.to_string()),
            }],
        );
        // r[verify cli_maintainability.repo_core_modules.behavior_preserved.focused_checks]
        assert_eq!(status.specs[0], "openspec/specs/a/spec.md");
        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].name, "demo");
        assert_eq!(status.changes[0].task_counts.unwrap().total, 2);
        assert_eq!(status.changes[0].metadata.groups, vec!["current", "future"]);
        assert_eq!(status.changes[0].metadata.depends_on, vec!["blocked-by"]);
    }

    #[test]
    fn filters_changes_by_metadata() {
        let matching = ChangeStatus {
            name: "matching".to_string(),
            path: ".cairn/changes/matching".to_string(),
            has_proposal: true,
            has_design: true,
            has_tasks: true,
            delta_specs: Vec::new(),
            task_counts: None,
            metadata: ChangeMetadata {
                groups: vec!["current".to_string(), "feature:registry".to_string()],
                target_date: Some("2026-06-01".to_string()),
                status: Some("planned".to_string()),
                depends_on: Vec::new(),
                cross_repo_dependencies: Vec::new(),
            },
            metadata_issues: Vec::new(),
        };
        let without_date = ChangeStatus {
            name: "without-date".to_string(),
            metadata: ChangeMetadata {
                groups: vec!["current".to_string()],
                target_date: None,
                status: None,
                depends_on: Vec::new(),
                cross_repo_dependencies: Vec::new(),
            },
            ..matching.clone()
        };
        let filtered = filter_changes_by_metadata(
            vec![matching, without_date],
            Some("current"),
            Some("2026-05-01"),
            Some("2026-07-01"),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "matching");
    }

    #[test]
    fn builds_sorted_spec_list_with_titles() {
        let specs = build_spec_list(vec![
            SpecSource {
                id: "zeta".to_string(),
                path: "openspec/specs/zeta/spec.md".to_string(),
                markdown: "# Zeta Spec\n\nBody".to_string(),
            },
            SpecSource {
                id: "alpha".to_string(),
                path: "openspec/specs/alpha/spec.md".to_string(),
                markdown: "\n# Alpha Spec\n".to_string(),
            },
        ]);
        assert_eq!(specs.specs[0].id, "alpha");
        assert_eq!(specs.specs[0].title.as_deref(), Some("Alpha Spec"));
        assert_eq!(specs.specs[1].id, "zeta");
    }

    #[test]
    fn builds_spec_detail_with_content() {
        let detail = build_spec_detail(SpecSource {
            id: "demo".to_string(),
            path: "openspec/specs/demo/spec.md".to_string(),
            markdown: "# Demo\n\nRequirement".to_string(),
        });
        // r[verify cli_maintainability.repo_core_modules.behavior_preserved.repository_checks]
        assert_eq!(detail.id, "demo");
        assert_eq!(detail.title.as_deref(), Some("Demo"));
        assert!(detail.markdown.contains("Requirement"));
    }
}
