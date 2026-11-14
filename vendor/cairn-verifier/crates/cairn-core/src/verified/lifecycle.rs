use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleArtifact {
    pub path: String,
    pub kind: LifecycleArtifactKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleArtifactKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSkeleton {
    pub name: String,
    pub artifacts: Vec<LifecycleArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleLayout {
    Native,
    LegacyCairn,
    LegacyIronspec,
    LegacyOpenSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationInput {
    pub source: String,
    pub source_hash: String,
    pub destination: String,
    pub destination_hash: Option<String>,
    pub destination_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    pub source_layout: LifecycleLayout,
    pub destination_layout: LifecycleLayout,
    pub dry_run: bool,
    pub blocked: bool,
    pub mutated: bool,
    pub actions: Vec<MigrationAction>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationAction {
    pub source: String,
    pub source_hash: String,
    pub destination: String,
    pub destination_hash: Option<String>,
}

pub fn init_artifacts() -> Vec<LifecycleArtifact> {
    // r[impl cairn.supersede.init] r[impl cairn.supersede.init.empty] r[impl cairn.supersede.init.idempotent]
    // r[impl cairn.supersede.native_layout] r[impl cairn.supersede.native_layout.init]
    init_artifacts_for(LifecycleLayout::Native)
}

pub fn init_artifacts_for(layout: LifecycleLayout) -> Vec<LifecycleArtifact> {
    let root = layout_root(layout);
    let archive = archive_dir(layout);
    vec![
        dir(root),
        dir(&format!("{root}/specs")),
        dir(&format!("{root}/changes")),
        dir(archive),
    ]
}

pub fn build_change_skeleton(name: &str) -> Result<ChangeSkeleton, String> {
    // r[impl cairn.supersede.change_create] r[impl cairn.supersede.change_create.skeleton]
    build_change_skeleton_for(LifecycleLayout::Native, name)
}

pub fn build_change_skeleton_for(
    layout: LifecycleLayout,
    name: &str,
) -> Result<ChangeSkeleton, String> {
    validate_change_name(name)?;
    let change_root = format!("{}/changes/{name}", layout_root(layout));
    Ok(ChangeSkeleton {
        name: name.to_string(),
        artifacts: vec![
            dir(&change_root),
            file(&format!("{change_root}/proposal.md")),
            file(&format!("{change_root}/design.md")),
            file(&format!("{change_root}/tasks.md")),
            dir(&format!("{change_root}/specs")),
        ],
    })
}

pub fn layout_name(layout: LifecycleLayout) -> &'static str {
    // r[impl cairn.supersede.native_layout.prefer] r[impl cairn.supersede.native_layout.native_only]
    match layout {
        LifecycleLayout::Native => "cairn",
        LifecycleLayout::LegacyCairn => "legacy-cairn",
        LifecycleLayout::LegacyIronspec => "ironspec",
        LifecycleLayout::LegacyOpenSpec => "openspec",
    }
}

pub fn layout_root(layout: LifecycleLayout) -> &'static str {
    match layout {
        LifecycleLayout::Native => ".cairn",
        LifecycleLayout::LegacyCairn => "cairn",
        LifecycleLayout::LegacyIronspec => "ironspec",
        LifecycleLayout::LegacyOpenSpec => "openspec",
    }
}

pub fn specs_dir(layout: LifecycleLayout) -> String {
    format!("{}/specs", layout_root(layout))
}

pub fn changes_dir(layout: LifecycleLayout) -> String {
    format!("{}/changes", layout_root(layout))
}

pub fn archive_dir(layout: LifecycleLayout) -> &'static str {
    match layout {
        LifecycleLayout::Native => ".cairn/archive",
        LifecycleLayout::LegacyCairn => "cairn/archive",
        LifecycleLayout::LegacyIronspec => "ironspec/archive",
        LifecycleLayout::LegacyOpenSpec => "openspec/changes/archive",
    }
}

pub fn build_cairn_migration_plan(inputs: Vec<MigrationInput>, execute: bool) -> MigrationPlan {
    // r[impl cairn.lifecycle_directory.legacy_migration]
    build_migration_plan_from(LifecycleLayout::LegacyCairn, inputs, execute)
}

pub fn migration_plan_identity(plan: &MigrationPlan) -> Value {
    // r[impl cairn.lifecycle_directory.legacy_migration.plan_binding]
    json!({
        "source_layout": layout_name(plan.source_layout),
        "destination_layout": layout_name(plan.destination_layout),
        "actions": plan.actions.iter().map(|action| {
            json!({
                "source": action.source,
                "source_hash": action.source_hash,
                "destination": action.destination,
                "destination_hash": action.destination_hash,
            })
        }).collect::<Vec<_>>(),
        "conflicts": plan.conflicts,
    })
}

pub fn build_openspec_migration_plan(inputs: Vec<MigrationInput>, execute: bool) -> MigrationPlan {
    // r[impl cairn.supersede.migrate_openspec] r[impl cairn.supersede.migrate_openspec.dry_run]
    // r[impl cairn.supersede.migrate_openspec.execute] r[impl cairn.supersede.migrate_openspec.conflict]
    build_migration_plan_from(LifecycleLayout::LegacyOpenSpec, inputs, execute)
}

pub fn build_ironspec_migration_plan(inputs: Vec<MigrationInput>, execute: bool) -> MigrationPlan {
    // r[impl cairn.supersede.migrate_openspec.ironspec_dry_run]
    // r[impl cairn.supersede.migrate_openspec.ironspec_execute]
    build_migration_plan_from(LifecycleLayout::LegacyIronspec, inputs, execute)
}

fn build_migration_plan_from(
    source_layout: LifecycleLayout,
    inputs: Vec<MigrationInput>,
    execute: bool,
) -> MigrationPlan {
    let mut actions = inputs
        .iter()
        .map(|input| MigrationAction {
            source: input.source.clone(),
            source_hash: input.source_hash.clone(),
            destination: input.destination.clone(),
            destination_hash: input.destination_hash.clone(),
        })
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| left.destination.cmp(&right.destination));
    let mut conflicts = inputs
        .into_iter()
        .filter(|input| input.destination_exists)
        .map(|input| input.destination)
        .collect::<Vec<_>>();
    conflicts.sort();
    MigrationPlan {
        source_layout,
        destination_layout: LifecycleLayout::Native,
        dry_run: !execute,
        blocked: !conflicts.is_empty(),
        mutated: false,
        actions,
        conflicts,
    }
}

pub fn validate_change_name(name: &str) -> Result<(), String> {
    // r[impl cairn.supersede.change_create.invalid_name]
    if name.is_empty() {
        return Err("change name must not be empty".to_string());
    }
    if name == "archive" || name == "_done" || name.starts_with('.') {
        return Err(format!("reserved change name: {name}"));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "invalid change name: {name}; use lowercase letters, digits, and hyphens"
        ));
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err(format!(
            "invalid change name: {name}; hyphens must separate words"
        ));
    }
    Ok(())
}

pub fn proposal_template(name: &str) -> String {
    format!(
        "## Why\n\nCAIRN_PLACEHOLDER: Describe why `{name}` is needed.\n\n## What Changes\n\n- CAIRN_PLACEHOLDER: Describe the user-visible or operator-visible change.\n\n## Impact\n\n- **Files**: CAIRN_PLACEHOLDER: list affected files\n- **Testing**: CAIRN_PLACEHOLDER: list required checks\n"
    )
}

pub fn design_template(name: &str) -> String {
    format!(
        "## Context\n\nCAIRN_PLACEHOLDER: Describe the current state for `{name}`.\n\n## Decisions\n\n### Decision: CAIRN_PLACEHOLDER\n\n**Choice:** CAIRN_PLACEHOLDER: replace with the reviewed choice\n\n**Rationale:** CAIRN_PLACEHOLDER: replace with the rationale\n\n## Risks / Trade-offs\n\n- CAIRN_PLACEHOLDER: replace with a bounded risk or `None identified`\n"
    )
}

pub fn tasks_template() -> String {
    "## Phase 1: Implementation\n\n- [ ] [serial] CAIRN_PLACEHOLDER: replace with a task linked to r[requirement.id].\n- [ ] [serial] CAIRN_PLACEHOLDER: replace with verification and archive work linked to r[requirement.id].\n".to_string()
}

fn dir(path: &str) -> LifecycleArtifact {
    LifecycleArtifact {
        path: path.to_string(),
        kind: LifecycleArtifactKind::Directory,
    }
}

fn file(path: &str) -> LifecycleArtifact {
    LifecycleArtifact {
        path: path.to_string(),
        kind: LifecycleArtifactKind::File,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migration_input(source_hash: &str) -> MigrationInput {
        MigrationInput {
            source: "cairn/specs/demo/spec.md".to_string(),
            source_hash: source_hash.to_string(),
            destination: ".cairn/specs/demo/spec.md".to_string(),
            destination_hash: None,
            destination_exists: false,
        }
    }

    #[test]
    fn migration_plan_identity_ignores_execution_mode() {
        // r[verify cairn.lifecycle_directory.legacy_migration.plan_binding]
        let preview = build_cairn_migration_plan(vec![migration_input("source-a")], false);
        let execution = build_cairn_migration_plan(vec![migration_input("source-a")], true);

        assert_eq!(
            migration_plan_identity(&preview),
            migration_plan_identity(&execution)
        );
    }

    #[test]
    fn migration_plan_identity_changes_with_source_hash() {
        // r[verify cairn.lifecycle_directory.legacy_migration.plan_binding.stale]
        let reviewed = build_cairn_migration_plan(vec![migration_input("source-a")], false);
        let changed = build_cairn_migration_plan(vec![migration_input("source-b")], false);

        assert_ne!(
            migration_plan_identity(&reviewed),
            migration_plan_identity(&changed)
        );
    }

    #[test]
    fn init_artifacts_use_openspec_layout() {
        let paths = init_artifacts_for(LifecycleLayout::LegacyOpenSpec)
            .into_iter()
            .map(|artifact| artifact.path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "openspec",
                "openspec/specs",
                "openspec/changes",
                "openspec/changes/archive"
            ]
        );
    }

    #[test]
    fn init_artifacts_default_to_native_layout() {
        let paths = init_artifacts()
            .into_iter()
            .map(|artifact| artifact.path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![".cairn", ".cairn/specs", ".cairn/changes", ".cairn/archive"]
        );
    }

    #[test]
    fn builds_change_skeleton_for_valid_slug() {
        let skeleton = build_change_skeleton("replace-openspec").expect("valid skeleton");
        assert_eq!(skeleton.name, "replace-openspec");
        assert_eq!(skeleton.artifacts.len(), 5);
        assert_eq!(
            skeleton.artifacts[1].path,
            ".cairn/changes/replace-openspec/proposal.md"
        );
    }

    #[test]
    fn native_layout_uses_hidden_root_and_stable_receipt_name() {
        // r[verify cairn.lifecycle_directory.hidden_default]
        // r[verify cairn.lifecycle_directory.receipt_compatibility]
        // r[verify cairn.lifecycle_directory.receipt_compatibility.paths]
        assert_eq!(layout_root(LifecycleLayout::Native), ".cairn");
        assert_eq!(layout_name(LifecycleLayout::Native), "cairn");
        assert_eq!(layout_root(LifecycleLayout::LegacyCairn), "cairn");
        assert_eq!(layout_name(LifecycleLayout::LegacyCairn), "legacy-cairn");
    }

    #[test]
    fn builds_old_cairn_migration_plan() {
        // r[verify cairn.lifecycle_directory.legacy_migration]
        let plan = build_cairn_migration_plan(
            vec![MigrationInput {
                source: "cairn/specs/demo/spec.md".to_string(),
                source_hash: "source-hash".to_string(),
                destination: ".cairn/specs/demo/spec.md".to_string(),
                destination_hash: None,
                destination_exists: false,
            }],
            false,
        );
        assert_eq!(plan.source_layout, LifecycleLayout::LegacyCairn);
        assert_eq!(plan.destination_layout, LifecycleLayout::Native);
        assert!(plan.dry_run);
        assert!(!plan.blocked);
    }

    #[test]
    fn builds_migration_plan_with_sorted_conflicts() {
        let plan = build_openspec_migration_plan(
            vec![
                MigrationInput {
                    source: "openspec/specs/b/spec.md".to_string(),
                    source_hash: "hash-b".to_string(),
                    destination: ".cairn/specs/b/spec.md".to_string(),
                    destination_hash: None,
                    destination_exists: false,
                },
                MigrationInput {
                    source: "openspec/specs/a/spec.md".to_string(),
                    source_hash: "hash-a".to_string(),
                    destination: ".cairn/specs/a/spec.md".to_string(),
                    destination_hash: Some("existing-a".to_string()),
                    destination_exists: true,
                },
            ],
            false,
        );
        assert!(plan.dry_run);
        assert!(plan.blocked);
        assert_eq!(plan.conflicts, vec![".cairn/specs/a/spec.md"]);
        assert_eq!(plan.actions[0].destination, ".cairn/specs/a/spec.md");
    }

    #[test]
    fn rejects_path_like_or_reserved_names() {
        for name in [
            "",
            "../escape",
            "UPPER",
            "archive",
            "_done",
            ".hidden",
            "bad--name",
        ] {
            assert!(validate_change_name(name).is_err(), "{name}");
        }
    }
}
