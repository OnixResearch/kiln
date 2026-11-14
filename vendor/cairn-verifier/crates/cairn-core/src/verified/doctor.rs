use crate::verified::lifecycle::{layout_name, LifecycleLayout};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorSnapshot {
    pub root: String,
    pub selected_layout: LifecycleLayout,
    pub native_layout_present: bool,
    pub legacy_layout_present: bool,
    pub specs_dir_present: bool,
    pub changes_dir_present: bool,
    pub archive_dir_present: bool,
    pub policy_path: String,
    pub policy_name: Option<String>,
    pub policy_error: Option<String>,
    pub spec_count: usize,
    pub change_count: usize,
    pub validation_issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub root: String,
    pub layout: String,
    pub valid: bool,
    pub spec_count: usize,
    pub change_count: usize,
    pub validation_issue_count: usize,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorCheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorCheckStatus {
    Ok,
    Warn,
    Fail,
}

pub fn build_doctor_report(snapshot: DoctorSnapshot) -> DoctorReport {
    // r[impl cairn.supersede.doctor] r[impl cairn.supersede.doctor.boundary]
    let checks = vec![
        layout_check(&snapshot),
        layout_dirs_check(&snapshot),
        policy_check(&snapshot),
        content_check(&snapshot),
        validation_check(&snapshot),
    ];
    let is_valid = checks
        .iter()
        .all(|check| check.status != DoctorCheckStatus::Fail);
    DoctorReport {
        root: snapshot.root,
        layout: layout_name(snapshot.selected_layout).to_string(),
        valid: is_valid,
        spec_count: snapshot.spec_count,
        change_count: snapshot.change_count,
        validation_issue_count: snapshot.validation_issue_count,
        checks,
    }
}

fn layout_check(snapshot: &DoctorSnapshot) -> DoctorCheck {
    if snapshot.native_layout_present && snapshot.legacy_layout_present {
        return doctor_check(
            "layout_selection",
            DoctorCheckStatus::Warn,
            format!(
                "selected {} layout; legacy and native roots are both present",
                layout_name(snapshot.selected_layout)
            ),
        );
    }
    doctor_check(
        "layout_selection",
        DoctorCheckStatus::Ok,
        format!("selected {} layout", layout_name(snapshot.selected_layout)),
    )
}

fn layout_dirs_check(snapshot: &DoctorSnapshot) -> DoctorCheck {
    if snapshot.specs_dir_present && snapshot.changes_dir_present && snapshot.archive_dir_present {
        return doctor_check(
            "layout_directories",
            DoctorCheckStatus::Ok,
            "selected layout directories are present".to_string(),
        );
    }
    if snapshot.specs_dir_present && snapshot.changes_dir_present {
        return doctor_check(
            "layout_directories",
            DoctorCheckStatus::Warn,
            "selected layout has specs and changes directories but no archive directory"
                .to_string(),
        );
    }
    doctor_check(
        "layout_directories",
        DoctorCheckStatus::Fail,
        "selected layout is missing specs or changes directories".to_string(),
    )
}

fn policy_check(snapshot: &DoctorSnapshot) -> DoctorCheck {
    if let Some(error) = &snapshot.policy_error {
        return doctor_check(
            "policy",
            DoctorCheckStatus::Fail,
            format!("{}: {error}", snapshot.policy_path),
        );
    }
    doctor_check(
        "policy",
        DoctorCheckStatus::Ok,
        snapshot
            .policy_name
            .as_ref()
            .map(|name| format!("loaded policy {name}"))
            .unwrap_or_else(|| "policy loaded".to_string()),
    )
}

fn content_check(snapshot: &DoctorSnapshot) -> DoctorCheck {
    if snapshot.spec_count == 0 {
        return doctor_check(
            "content",
            DoctorCheckStatus::Warn,
            "no accepted specs were discovered".to_string(),
        );
    }
    doctor_check(
        "content",
        DoctorCheckStatus::Ok,
        format!(
            "discovered {} accepted spec(s) and {} active change(s)",
            snapshot.spec_count, snapshot.change_count
        ),
    )
}

fn validation_check(snapshot: &DoctorSnapshot) -> DoctorCheck {
    if snapshot.validation_issue_count > 0 {
        return doctor_check(
            "validation",
            DoctorCheckStatus::Fail,
            format!(
                "{} validation issue(s) found",
                snapshot.validation_issue_count
            ),
        );
    }
    doctor_check(
        "validation",
        DoctorCheckStatus::Ok,
        "no validation issues found".to_string(),
    )
}

fn doctor_check(name: &str, status: DoctorCheckStatus, message: String) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_snapshot() -> DoctorSnapshot {
        DoctorSnapshot {
            root: "fixture".to_string(),
            selected_layout: LifecycleLayout::LegacyOpenSpec,
            native_layout_present: false,
            legacy_layout_present: true,
            specs_dir_present: true,
            changes_dir_present: true,
            archive_dir_present: true,
            policy_path: "policy.json".to_string(),
            policy_name: Some("cairn-default".to_string()),
            policy_error: None,
            spec_count: 2,
            change_count: 0,
            validation_issue_count: 0,
        }
    }

    #[test]
    fn builds_valid_report_for_healthy_snapshot() {
        let report = build_doctor_report(healthy_snapshot());
        assert!(report.valid);
        assert_eq!(report.layout, "openspec");
        assert_eq!(report.checks.len(), 5);
    }

    #[test]
    fn fails_missing_policy_and_validation_issues() {
        let mut snapshot = healthy_snapshot();
        snapshot.policy_error = Some("No such file or directory".to_string());
        snapshot.validation_issue_count = 2;
        let report = build_doctor_report(snapshot);
        assert!(!report.valid);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "policy" && check.status == DoctorCheckStatus::Fail));
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "validation" && check.status == DoctorCheckStatus::Fail));
    }
}
