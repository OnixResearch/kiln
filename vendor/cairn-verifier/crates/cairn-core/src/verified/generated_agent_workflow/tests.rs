use super::*;

const POLICY_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DESTINATION: &str = ".agents/skills";
const NON_CLAIMS: &[&str] = &[
    "deterministic generation proves only a bounded transformation of admitted policy",
    "an adapter changes representation but does not add authority or lifecycle semantics",
    "generated guidance does not grant command, filesystem, network, model, gate, or release authority",
    "workflow success depends on checked project evidence and does not follow from generated text alone",
];

fn policy() -> GeneratedAgentWorkflowPolicy {
    let descriptors = INITIAL_WORKFLOW_IDS
        .iter()
        .map(|id| {
            let read_only = matches!(*id, "explore" | "verify");
            let constraints = if *id == "explore" {
                vec![GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE.to_string()]
            } else {
                vec!["Keep policy authoritative.".to_string()]
            };
            GeneratedWorkflowDescriptor {
                id: (*id).to_string(),
                version: 1,
                goal: format!("Complete the {id} workflow with checked evidence."),
                success_evidence: vec!["The required evidence is present.".to_string()],
                required_evidence: vec!["A checked receipt".to_string()],
                command_classes: if read_only {
                    vec!["read".to_string()]
                } else {
                    vec!["mutate".to_string(), "read".to_string()]
                },
                mutation_class: if read_only {
                    GeneratedWorkflowMutationClass::ReadOnly
                } else {
                    GeneratedWorkflowMutationClass::PreviewFirst
                },
                progress_rule: "Report each checked step.".to_string(),
                immediate_action: "Inspect the selected change.".to_string(),
                output_shape: "Return evidence and blockers.".to_string(),
                constraints,
                non_claims: NON_CLAIMS
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
            }
        })
        .collect();
    GeneratedAgentWorkflowPolicy {
        schema: GENERATED_AGENT_WORKFLOW_POLICY_SCHEMA_V1.to_string(),
        generator_version: GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION.to_string(),
        command_classes: vec![
            GeneratedWorkflowCommandClass {
                id: "mutate".to_string(),
                permission: GeneratedWorkflowPermission::Mutation,
            },
            GeneratedWorkflowCommandClass {
                id: "read".to_string(),
                permission: GeneratedWorkflowPermission::Read,
            },
        ],
        descriptors,
        adapters: vec![
            GeneratedWorkflowAdapter {
                id: "agent-skills".to_string(),
                version: 1,
                format: GeneratedWorkflowAdapterFormat::AgentSkills,
                file_prefix: "cairn-".to_string(),
                file_suffix: String::new(),
                directory_suffix: "/SKILL.md".to_string(),
                front_matter: true,
                max_files: 6,
            },
            GeneratedWorkflowAdapter {
                id: "claude".to_string(),
                version: 1,
                format: GeneratedWorkflowAdapterFormat::Claude,
                file_prefix: "cairn-".to_string(),
                file_suffix: ".md".to_string(),
                directory_suffix: String::new(),
                front_matter: false,
                max_files: 6,
            },
            GeneratedWorkflowAdapter {
                id: "codex".to_string(),
                version: 1,
                format: GeneratedWorkflowAdapterFormat::Codex,
                file_prefix: "cairn-".to_string(),
                file_suffix: ".md".to_string(),
                directory_suffix: String::new(),
                front_matter: false,
                max_files: 6,
            },
        ],
        bounds: GeneratedWorkflowBounds {
            max_descriptors: 6,
            max_adapters: 3,
            max_command_classes: 8,
            max_text_bytes: 4096,
            max_list_items: 32,
            max_files_per_adapter: 8,
            max_path_bytes: 4096,
            max_file_bytes: 65536,
            max_issues: 64,
        },
        receipt_fields: [
            "adapter",
            "destination",
            "files",
            "manifest",
            "plan_hash",
            "policy_hash",
            "non_claims",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        required_non_claims: NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}

fn empty_snapshot() -> GeneratedWorkflowDestinationSnapshot {
    GeneratedWorkflowDestinationSnapshot {
        files: Vec::new(),
        manifest: GeneratedWorkflowManifestObservation::Missing,
    }
}

fn installed_snapshot(plan: &GeneratedWorkflowInstallPlan) -> GeneratedWorkflowDestinationSnapshot {
    GeneratedWorkflowDestinationSnapshot {
        files: plan
            .files
            .iter()
            .map(|file| GeneratedWorkflowFileObservation {
                relative_path: file.relative_path.clone(),
                kind: GeneratedWorkflowObservedKind::Regular,
                content_blake3: Some(file.generated_blake3.clone()),
            })
            .collect(),
        manifest: GeneratedWorkflowManifestObservation::Present(Box::new(plan.manifest.clone())),
    }
}

#[test]
fn admits_and_renders_each_bounded_adapter() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    for adapter in INITIAL_ADAPTER_IDS {
        let files = render_generated_agent_workflows(&catalog, adapter).expect("rendered files");
        assert_eq!(files.len(), INITIAL_WORKFLOW_IDS.len());
        assert!(files.iter().all(|file| file.contents.contains("## Goal")));
        assert!(files
            .iter()
            .all(|file| file.contents.contains("## Non-claims")));
        let explore = files
            .iter()
            .find(|file| file.workflow_id == "explore")
            .expect("rendered explore workflow");
        assert!(explore
            .contents
            .contains(GENERATED_AGENT_WORKFLOW_EXPLORE_NO_WRITE));
    }
    let skill = render_generated_agent_workflows(&catalog, "agent-skills")
        .expect("skills")
        .remove(0);
    assert!(skill.relative_path.ends_with("/SKILL.md"));
    assert!(skill.contents.starts_with("---\nname: cairn-"));
}

#[test]
fn rejects_hidden_explore_mutation_and_unknown_adapter() {
    let mut invalid = policy();
    let explore = invalid
        .descriptors
        .iter_mut()
        .find(|descriptor| descriptor.id == "explore")
        .expect("explore");
    explore.command_classes.push("mutate".to_string());
    let issues = admit_generated_agent_workflow_policy(&invalid).expect_err("must reject");
    assert!(issues
        .iter()
        .any(|issue| issue.field_path.contains("explore") || issue.message.contains("read-only")));

    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    let issues = render_generated_agent_workflows(&catalog, "remote-agent")
        .expect_err("unknown adapter must fail");
    assert_eq!(issues[0].code, GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER);
}

#[test]
fn adapter_projection_rejects_authority_and_runtime_hooks() {
    for field in ["authority", "network_hook", "model_hook"] {
        let mut adapter = serde_json::to_value(&policy().adapters[0]).expect("adapter value");
        adapter[field] = serde_json::json!("forbidden");
        let error = serde_json::from_value::<GeneratedWorkflowAdapter>(adapter)
            .expect_err("adapter must not add authority or runtime hooks");
        assert!(error.to_string().contains("unknown field"));
    }
}

#[test]
fn rejects_unsafe_destination_before_planning() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    for destination in ["../home", "/tmp/skills", "~/.agents", "skills/*"] {
        let issues = plan_generated_agent_workflow_install(
            &catalog,
            "agent-skills",
            destination,
            POLICY_HASH,
            &empty_snapshot(),
        )
        .expect_err("unsafe destination must fail");
        assert_eq!(issues[0].code, GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION);
    }
}

#[test]
fn plans_fresh_install_and_classifies_current_install() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    let fresh = plan_generated_agent_workflow_install(
        &catalog,
        "agent-skills",
        DESTINATION,
        POLICY_HASH,
        &empty_snapshot(),
    )
    .expect("fresh plan");
    assert!(!fresh.blocked());
    assert!(!fresh.current());
    assert!(fresh
        .files
        .iter()
        .all(|file| file.freshness == GeneratedWorkflowFreshness::Missing));
    assert!(validate_generated_workflow_manifest(&fresh.manifest));

    let current = plan_generated_agent_workflow_install(
        &catalog,
        "agent-skills",
        DESTINATION,
        POLICY_HASH,
        &installed_snapshot(&fresh),
    )
    .expect("current plan");
    assert!(current.current());
    assert_eq!(
        fresh.manifest.manifest_identity,
        current.manifest.manifest_identity
    );
}

#[test]
fn blocks_unmanaged_modified_missing_and_symlink_destinations() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    let fresh = plan_generated_agent_workflow_install(
        &catalog,
        "claude",
        ".claude/commands",
        POLICY_HASH,
        &empty_snapshot(),
    )
    .expect("fresh plan");

    let unmanaged = GeneratedWorkflowDestinationSnapshot {
        files: vec![GeneratedWorkflowFileObservation {
            relative_path: fresh.files[0].relative_path.clone(),
            kind: GeneratedWorkflowObservedKind::Regular,
            content_blake3: Some(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
        }],
        manifest: GeneratedWorkflowManifestObservation::Missing,
    };
    let plan = plan_generated_agent_workflow_install(
        &catalog,
        "claude",
        ".claude/commands",
        POLICY_HASH,
        &unmanaged,
    )
    .expect("conflict plan");
    assert!(plan.blocked());
    assert_eq!(
        plan.files[0].freshness,
        GeneratedWorkflowFreshness::UnmanagedConflict
    );

    let mut installed = installed_snapshot(&fresh);
    installed.files[0].kind = GeneratedWorkflowObservedKind::Symlink;
    installed.files[1].content_blake3 =
        Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string());
    installed.files.remove(2);
    let blocked = plan_generated_agent_workflow_install(
        &catalog,
        "claude",
        ".claude/commands",
        POLICY_HASH,
        &installed,
    )
    .expect("blocked plan");
    assert!(blocked.blocked());
    assert!(blocked
        .files
        .iter()
        .any(|file| file.freshness == GeneratedWorkflowFreshness::SymlinkConflict));
    assert!(blocked
        .files
        .iter()
        .any(|file| file.freshness == GeneratedWorkflowFreshness::Modified));
    assert!(blocked
        .files
        .iter()
        .any(|file| file.freshness == GeneratedWorkflowFreshness::Missing));
}

#[test]
fn obsolete_manifest_path_is_reported_without_a_delete_effect() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    let fresh = plan_generated_agent_workflow_install(
        &catalog,
        "codex",
        ".codex/prompts",
        POLICY_HASH,
        &empty_snapshot(),
    )
    .expect("fresh plan");
    let mut snapshot = installed_snapshot(&fresh);
    let prior = match &mut snapshot.manifest {
        GeneratedWorkflowManifestObservation::Present(manifest) => manifest,
        _ => panic!("installed manifest"),
    };
    let mut obsolete = prior.files[0].clone();
    obsolete.workflow_id = "obsolete".to_string();
    obsolete.relative_path = ".codex/prompts/cairn-obsolete.md".to_string();
    prior.files.push(obsolete);
    prior.manifest_identity = generated_workflow_manifest_identity(prior);

    let plan = plan_generated_agent_workflow_install(
        &catalog,
        "codex",
        ".codex/prompts",
        POLICY_HASH,
        &snapshot,
    )
    .expect("obsolete path plan");
    assert_eq!(
        plan.removal_candidates,
        vec![".codex/prompts/cairn-obsolete.md".to_string()]
    );
    assert!(plan
        .files
        .iter()
        .all(|file| { file.relative_path != ".codex/prompts/cairn-obsolete.md" }));
    let repeated = plan_generated_agent_workflow_install(
        &catalog,
        "codex",
        ".codex/prompts",
        POLICY_HASH,
        &installed_snapshot(&plan),
    )
    .expect("repeated obsolete path plan");
    assert_eq!(repeated.removal_candidates, plan.removal_candidates);
}

#[test]
fn unsupported_manifest_version_blocks_installation() {
    let catalog = admit_generated_agent_workflow_policy(&policy()).expect("admitted policy");
    let fresh = plan_generated_agent_workflow_install(
        &catalog,
        "agent-skills",
        DESTINATION,
        POLICY_HASH,
        &empty_snapshot(),
    )
    .expect("fresh plan");
    let mut snapshot = installed_snapshot(&fresh);
    let prior = match &mut snapshot.manifest {
        GeneratedWorkflowManifestObservation::Present(manifest) => manifest,
        _ => panic!("installed manifest"),
    };
    prior.adapter_version += 1;
    prior.manifest_identity = generated_workflow_manifest_identity(prior);
    let plan = plan_generated_agent_workflow_install(
        &catalog,
        "agent-skills",
        DESTINATION,
        POLICY_HASH,
        &snapshot,
    )
    .expect("unsupported plan");
    assert!(plan.blocked());
    assert_eq!(
        plan.manifest_freshness,
        GeneratedWorkflowFreshness::UnsupportedVersion
    );
}

#[test]
fn source_change_is_stale_but_preserves_modified_file_protection() {
    let first_catalog = admit_generated_agent_workflow_policy(&policy()).expect("first policy");
    let first = plan_generated_agent_workflow_install(
        &first_catalog,
        "codex",
        ".codex/prompts",
        POLICY_HASH,
        &empty_snapshot(),
    )
    .expect("first plan");
    let snapshot = installed_snapshot(&first);
    let mut changed_policy = policy();
    changed_policy.descriptors[0]
        .goal
        .push_str(" Keep scope bounded.");
    let changed_catalog =
        admit_generated_agent_workflow_policy(&changed_policy).expect("changed policy");
    let changed = plan_generated_agent_workflow_install(
        &changed_catalog,
        "codex",
        ".codex/prompts",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        &snapshot,
    )
    .expect("changed plan");
    assert!(!changed.blocked());
    assert!(changed
        .files
        .iter()
        .any(|file| file.freshness == GeneratedWorkflowFreshness::StaleSource));
    assert_eq!(
        changed.manifest_freshness,
        GeneratedWorkflowFreshness::StaleSource
    );
}
