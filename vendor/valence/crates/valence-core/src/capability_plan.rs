//! Pure filesystem capability access planning.
//!
//! The CLI shell opens declared roots and performs I/O. This module validates
//! only role, operation, root declaration identity, and relative locator shape.

use std::path::Component;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

/// Typed filesystem authority role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRole {
    Artifact,
    Manifest,
    Sidecar,
    Binary,
    Output,
}

/// Operation authorized by an access plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityOperation {
    Read,
    Write,
}

/// Already-parsed plan for one capability-relative filesystem operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAccessPlan {
    pub role: CapabilityRole,
    pub operation: CapabilityOperation,
    pub root_declaration: String,
    pub locator: String,
}

/// Deterministic field-scoped plan issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CapabilityPlanIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl CapabilityPlanIssue {
    const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Validate declared authority independently from the untrusted locator.
///
/// r[impl valence.stack_provenance_adapter.declared_capability_roots.contract]
/// r[impl valence.stack_provenance_adapter.declared_capability_roots.relative_locators]
#[must_use]
pub fn validate_capability_access_plan(plan: &CapabilityAccessPlan) -> Vec<CapabilityPlanIssue> {
    let mut issues = Vec::new();
    if plan.root_declaration.trim().is_empty() {
        issues.push(CapabilityPlanIssue::new("root_declaration", "capability_root_missing"));
    }
    validate_role_operation(plan.role, plan.operation, &mut issues);
    validate_locator(&plan.locator, &mut issues);
    issues.sort();
    issues.dedup();
    issues
}

fn validate_role_operation(
    role: CapabilityRole,
    operation: CapabilityOperation,
    issues: &mut Vec<CapabilityPlanIssue>,
) {
    let permitted = matches!(
        (role, operation),
        (CapabilityRole::Output, CapabilityOperation::Write)
            | (
                CapabilityRole::Artifact | CapabilityRole::Manifest | CapabilityRole::Sidecar | CapabilityRole::Binary,
                CapabilityOperation::Read,
            )
    );
    if !permitted {
        issues.push(CapabilityPlanIssue::new("operation", "capability_role_mismatch"));
    }
}

fn validate_locator(locator: &str, issues: &mut Vec<CapabilityPlanIssue>) {
    if locator.is_empty() {
        issues.push(CapabilityPlanIssue::new("locator", "capability_locator_empty"));
        return;
    }
    let path = Path::new(locator);
    let mut has_normal_component = false;
    for component in path.components() {
        match component {
            Component::Normal(component) if !component.is_empty() => has_normal_component = true,
            Component::CurDir => {}
            Component::ParentDir => {
                issues.push(CapabilityPlanIssue::new("locator", "capability_locator_traversal"));
            }
            Component::RootDir | Component::Prefix(_) => {
                issues.push(CapabilityPlanIssue::new("locator", "capability_locator_absolute"));
            }
            Component::Normal(_) => {
                issues.push(CapabilityPlanIssue::new("locator", "capability_locator_empty_component"));
            }
        }
    }
    if !has_normal_component {
        issues.push(CapabilityPlanIssue::new("locator", "capability_locator_missing_file"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(role: CapabilityRole, operation: CapabilityOperation, locator: &str) -> CapabilityAccessPlan {
        CapabilityAccessPlan {
            role,
            operation,
            root_declaration: "operator-root".to_owned(),
            locator: locator.to_owned(),
        }
    }

    #[test]
    fn explicit_nested_read_and_output_write_plans_pass() {
        // r[verify valence.stack_provenance_adapter.declared_capability_roots.fixtures.positive]
        assert!(
            validate_capability_access_plan(&plan(
                CapabilityRole::Artifact,
                CapabilityOperation::Read,
                "evidence/input.json",
            ))
            .is_empty()
        );
        assert!(
            validate_capability_access_plan(&plan(
                CapabilityRole::Output,
                CapabilityOperation::Write,
                "reports/output.json",
            ))
            .is_empty()
        );
    }

    #[test]
    fn omitted_root_escape_and_wrong_role_plans_fail() {
        // r[verify valence.stack_provenance_adapter.declared_capability_roots.fixtures.negative]
        let mut missing_root = plan(CapabilityRole::Artifact, CapabilityOperation::Read, "input.json");
        missing_root.root_declaration.clear();
        assert!(
            validate_capability_access_plan(&missing_root)
                .iter()
                .any(|issue| issue.code == "capability_root_missing")
        );
        for locator in ["/tmp/input.json", "../input.json", "nested/../../input.json"] {
            assert!(
                !validate_capability_access_plan(&plan(CapabilityRole::Artifact, CapabilityOperation::Read, locator,))
                    .is_empty()
            );
        }
        assert!(
            validate_capability_access_plan(
                &plan(CapabilityRole::Artifact, CapabilityOperation::Write, "output.json",)
            )
            .iter()
            .any(|issue| issue.code == "capability_role_mismatch")
        );
    }
}
