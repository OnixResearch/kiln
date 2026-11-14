// r[impl cli_maintainability.core_validation_modules.focused_modules]
// r[impl cli_maintainability.core_validation_modules.focused_modules.visible_boundaries]
// r[impl cli_maintainability.core_validation_modules.focused_modules.narrow_bridges]
// r[impl cli_maintainability.core_validation_modules.behavior_preserved]
// r[verify cli_maintainability.core_validation_modules.behavior_preserved.focused_checks]
// r[verify cli_maintainability.core_validation_modules.behavior_preserved.repository_checks]
mod changes;
mod gates;
mod markdown;
mod references;
mod spec;
mod substance;
mod task_ordering;

pub use changes::{change_issues, validate_changes, validate_changes_with_archives};
pub use gates::{
    classify_gate_evidence, evaluate_gate, evaluate_gate_snapshot, evaluate_profile_gate_snapshot,
    finalize_gate_report, validation_issue_code, EvidenceStrengthSummary, GateArtifact, GateIssue,
    GateReport, GateSnapshot, GateStage,
};
pub use spec::{
    validate_spec_source, validate_spec_source_with_policy, validate_spec_sources,
    validate_spec_sources_with_policy, SpecSubstanceInventory,
};
pub use substance::{
    evaluate_lifecycle_substance, lifecycle_substance_issues, ArtifactSubstanceInventory,
    LifecycleSubstanceReport,
};
pub use task_ordering::{
    build_task_ordering_graph, TaskOrderingEdge, TaskOrderingGraph, TaskOrderingIssue,
    TaskOrderingNode, TaskOrderingState, TASK_ORDER_ISSUE_CYCLE, TASK_ORDER_ISSUE_ID_DUPLICATE,
    TASK_ORDER_ISSUE_ID_MALFORMED, TASK_ORDER_ISSUE_ID_MISSING, TASK_ORDER_ISSUE_ID_MULTIPLE,
    TASK_ORDER_ISSUE_MARKER_POLICY, TASK_ORDER_ISSUE_PREDECESSOR_DUPLICATE,
    TASK_ORDER_ISSUE_PREDECESSOR_MALFORMED, TASK_ORDER_ISSUE_PREDECESSOR_SELF,
    TASK_ORDER_ISSUE_PREDECESSOR_UNKNOWN, TASK_ORDER_ISSUE_PROGRESS,
};

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::verified::function_address_gates::{
        FunctionAddressPolicy, FUNCTION_ADDRESS_BOUNDARY, FUNCTION_ADDRESS_CLAIM_SCOPE_LIFECYCLE,
        FUNCTION_ADDRESS_MODE_DISABLED,
    };
    use crate::verified::policy::{
        parse_policy_json, ArtifactSubstancePolicy, ExemptionMarkerPolicy, Policy, ReceiptSchema,
        SteelOrchestrationPolicy, TaskMarker, TraceabilityPolicy,
    };
    use crate::verified::repo::{ChangeStatus, SpecSource, TaskCounts};
    use crate::verified::runtime_evidence::{
        RuntimeEvidencePolicy, CAPTURE_ORIGIN_BOUNDARY, CAPTURE_ORIGIN_INGESTED,
        CAPTURE_ORIGIN_UNKNOWN, CAPTURE_ORIGIN_WRAPPER_ASSERTED, DEFAULT_RUNTIME_SUMMARY_MAX_BYTES,
        RUNTIME_HASH_ALGORITHM_BLAKE3,
    };
    use crate::verified::stack_provenance::{
        StackProvenancePolicy, STACK_PROVENANCE_BOUNDARY, STACK_PROVENANCE_CLAIM_SCOPE_LIFECYCLE,
        STACK_PROVENANCE_MODE_DISABLED,
    };

    fn policy(enabled: bool, strictness: &str) -> Policy {
        let generated_policy = parse_policy_json(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .expect("generated policy");
        Policy {
            name: "test".to_string(),
            version: 1,
            task_markers_enabled: enabled,
            task_marker_strictness: strictness.to_string(),
            task_marker_tokens: vec![
                TaskMarker {
                    id: "parallel".to_string(),
                    token: "[parallel]".to_string(),
                    kind: "literal".to_string(),
                },
                TaskMarker {
                    id: "serial".to_string(),
                    token: "[serial]".to_string(),
                    kind: "literal".to_string(),
                },
                TaskMarker {
                    id: "depends".to_string(),
                    token: "[depends:".to_string(),
                    kind: "prefix".to_string(),
                },
                TaskMarker {
                    id: "task".to_string(),
                    token: "[task:<id>]".to_string(),
                    kind: "task-id-prefix".to_string(),
                },
                TaskMarker {
                    id: "after".to_string(),
                    token: "[after:<task-id>]".to_string(),
                    kind: "task-predecessor-prefix".to_string(),
                },
            ],
            gate_stages: vec![
                "proposal".to_string(),
                "design".to_string(),
                "tasks".to_string(),
            ],
            gate_ignore_archives: true,
            gate_finding_scope: "tasks".to_string(),
            gate_finding_promoted: "no".to_string(),
            machine_contract_policy: generated_policy.machine_contract_policy,
            document_collaboration_policy: generated_policy.document_collaboration_policy,
            workflow_schemas: generated_policy.workflow_schemas,
            workflow_profile_policy: generated_policy.workflow_profile_policy,
            agent_guidance_policy: generated_policy.agent_guidance_policy,
            developer_loop_policy: generated_policy.developer_loop_policy,
            generated_agent_workflow_policy: generated_policy.generated_agent_workflow_policy,
            artifact_substance_policy: ArtifactSubstancePolicy {
                proposal_minimum_substantive_lines: 1,
                design_minimum_substantive_lines: 1,
                minimum_requirement_blocks: 1,
                minimum_substantive_tasks: 1,
                placeholder_tokens: vec!["cairn_placeholder".to_string()],
                normative_tokens: vec![
                    "MUST".to_string(),
                    "SHALL".to_string(),
                    "SHOULD".to_string(),
                    "MAY".to_string(),
                ],
                dependency_marker_id: "depends".to_string(),
                dependency_target_class: "lifecycle-id".to_string(),
            },
            sync_require_gate: true,
            archive_require_gate: true,
            migrate_require_manifest: true,
            require_receipt_hashes: true,
            receipt_schemas: vec![ReceiptSchema {
                command: "gate".to_string(),
                top_level_keys: vec!["verdict".to_string()],
                action_keys: Vec::new(),
                issue_keys: vec![
                    "message".to_string(),
                    "scope".to_string(),
                    "promoted".to_string(),
                ],
                hash_fields: vec!["receipt_hash".to_string()],
            }],
            exemption_marker_policy: ExemptionMarkerPolicy {
                required_fields: vec![
                    "reason".to_string(),
                    "owner".to_string(),
                    "until".to_string(),
                ],
                date_fields: vec!["until".to_string()],
                optional_fields: vec!["scope".to_string()],
            },
            change_metadata_allowed_groups: vec!["current".to_string(), "future".to_string()],
            change_metadata_group_prefixes: vec!["feature:".to_string()],
            change_metadata_statuses: vec![
                "current".to_string(),
                "planned".to_string(),
                "blocked".to_string(),
                "deferred".to_string(),
            ],
            steel_orchestration_policy: SteelOrchestrationPolicy {
                enabled: false,
                profiles: Vec::new(),
            },
            traceability_policy: TraceabilityPolicy {
                default_profile: "cairn-default".to_string(),
                profiles: Vec::new(),
            },
            stack_provenance_policy: StackProvenancePolicy {
                mode: STACK_PROVENANCE_MODE_DISABLED.to_string(),
                allowed_claim_scopes: vec![STACK_PROVENANCE_CLAIM_SCOPE_LIFECYCLE.to_string()],
                accepted_valence_policy_hashes: Vec::new(),
                required_boundary: STACK_PROVENANCE_BOUNDARY.to_string(),
            },
            function_address_policy: FunctionAddressPolicy {
                mode: FUNCTION_ADDRESS_MODE_DISABLED.to_string(),
                allowed_claim_scopes: vec![FUNCTION_ADDRESS_CLAIM_SCOPE_LIFECYCLE.to_string()],
                accepted_valence_policy_hashes: Vec::new(),
                required_roles: vec!["function-address-evidence-sidecar".to_string()],
                required_schemas: vec!["valence.function-address-evidence.v1".to_string()],
                required_boundary: FUNCTION_ADDRESS_BOUNDARY.to_string(),
            },
            runtime_evidence_policy: RuntimeEvidencePolicy {
                allowed_capture_origins: vec![
                    CAPTURE_ORIGIN_BOUNDARY.to_string(),
                    CAPTURE_ORIGIN_INGESTED.to_string(),
                    CAPTURE_ORIGIN_WRAPPER_ASSERTED.to_string(),
                    CAPTURE_ORIGIN_UNKNOWN.to_string(),
                ],
                allowed_hash_algorithms: vec![RUNTIME_HASH_ALGORITHM_BLAKE3.to_string()],
                max_summary_bytes: DEFAULT_RUNTIME_SUMMARY_MAX_BYTES,
                forbidden_summary_fragments: Vec::new(),
                aggregation_profiles: Vec::new(),
            },
            nominal_identity_policy: generated_policy.nominal_identity_policy,
            lifecycle_store_policy: generated_policy.lifecycle_store_policy,
            policy_schema_compatibility: None,
            raw: json!({}),
        }
    }

    fn change() -> ChangeStatus {
        ChangeStatus {
            name: "demo".to_string(),
            path: "openspec/changes/demo".to_string(),
            has_proposal: true,
            has_design: false,
            has_tasks: true,
            delta_specs: Vec::new(),
            task_counts: Some(TaskCounts {
                total: 2,
                done: 0,
                in_progress: 0,
                todo: 2,
                unmarked: 0,
            }),
            metadata: Default::default(),
            metadata_issues: Vec::new(),
        }
    }

    #[test]
    fn accepts_complete_marked_change() {
        assert!(change_issues(&change(), &policy(true, "strict")).is_empty());
    }

    #[test]
    fn rejects_missing_required_change_documents() {
        let mut change = change();
        change.has_proposal = false;
        change.has_tasks = false;
        assert_eq!(
            change_issues(&change, &policy(true, "strict")),
            vec![
                "demo: missing proposal.md".to_string(),
                "demo: missing tasks.md".to_string(),
            ]
        );
    }

    #[test]
    fn rejects_unmarked_tasks_only_when_strict_markers_are_enabled() {
        let mut change = change();
        change.task_counts = Some(TaskCounts {
            total: 1,
            done: 0,
            in_progress: 0,
            todo: 1,
            unmarked: 1,
        });
        assert_eq!(
            change_issues(&change, &policy(true, "strict")),
            vec!["demo: 1 task(s) missing concurrency markers".to_string()]
        );
        assert!(change_issues(&change, &policy(false, "strict")).is_empty());
        assert!(change_issues(&change, &policy(true, "advisory")).is_empty());
    }

    #[test]
    fn evaluates_stage_gates_from_change_snapshot() {
        let policy = policy(true, "strict");
        let proposal = evaluate_gate(&change(), GateStage::Proposal, &policy);
        assert_eq!(proposal.verdict, "PASS");

        let mut incomplete = change();
        incomplete.has_design = false;
        incomplete.delta_specs = Vec::new();
        let design = evaluate_gate(&incomplete, GateStage::Design, &policy);
        assert_eq!(design.verdict, "FAIL");
        assert_eq!(
            design
                .issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>(),
            vec!["demo: missing delta specs", "demo: missing design.md"]
        );
    }

    #[test]
    fn authoritative_gate_finalizer_cannot_emit_a_contradictory_outcome() {
        // r[verify cairn.probe_execution_boundary.authoritative_gate_composition]
        let report = evaluate_gate(&change(), GateStage::Proposal, &policy(true, "strict"));
        assert_eq!(report.verdict, "PASS");
        let finalized = finalize_gate_report(
            report,
            vec![GateIssue {
                code: "probe.saved_receipt.missing".to_string(),
                message: "saved probe receipt is missing".to_string(),
                scope: "tasks".to_string(),
                promoted: "no".to_string(),
            }],
        );
        assert_eq!(finalized.verdict, "FAIL");
        assert!(!finalized.issues.is_empty());
    }

    #[test]
    fn gate_snapshot_rejects_unknown_refs_unlinked_tasks_and_malformed_exemptions() {
        let mut change = change();
        change.has_design = true;
        change.delta_specs = vec!["openspec/changes/demo/specs/demo/spec.md".to_string()];
        let snapshot = GateSnapshot {
            change,
            proposal: Some(GateArtifact {
                path: "proposal.md".to_string(),
                markdown: "r[demo.unknown] gate-exempt[reason=test owner=agent]".to_string(),
            }),
            design: Some(GateArtifact {
                path: "design.md".to_string(),
                markdown: "r[demo.behavior]".to_string(),
            }),
            tasks: Some(GateArtifact {
                path: "tasks.md".to_string(),
                markdown: "- [ ] [serial] missing link\n- [ ] [parallel] r[demo.behavior] linked"
                    .to_string(),
            }),
            delta_specs: vec![GateArtifact {
                path: "spec.md".to_string(),
                markdown: "r[demo.behavior] The system MUST behave.".to_string(),
            }],
        };

        let report = evaluate_gate_snapshot(&snapshot, GateStage::Tasks, &policy(true, "strict"));
        assert_eq!(report.verdict, "FAIL");
        let messages = report
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages
            .iter()
            .any(|message| message.contains("unknown requirement reference r[demo.unknown]")));
        assert!(messages
            .iter()
            .any(|message| message.contains("malformed gate exemption")));
        assert!(messages
            .iter()
            .any(|message| message.contains("task missing known requirement reference")));
    }

    #[test]
    fn gate_snapshot_accepts_known_refs_dependencies_and_bounded_exemptions() {
        // r[verify cairn.lifecycle_validation.artifact_substance.positive]
        // r[verify cairn.lifecycle_validation.task_substance]
        // r[verify cairn.lifecycle_validation.dependency_marker_shape.valid]
        let mut change = change();
        change.has_design = true;
        change.delta_specs = vec!["openspec/changes/demo/specs/demo/spec.md".to_string()];
        let snapshot = GateSnapshot {
            change,
            proposal: Some(GateArtifact {
                path: "proposal.md".to_string(),
                markdown: "This proposal defines bounded behavior. gate-exempt[reason=test owner=agent until=2026-12-31]".to_string(),
            }),
            design: Some(GateArtifact {
                path: "design.md".to_string(),
                markdown: "r[demo.behavior] The design chooses an explicit implementation."
                    .to_string(),
            }),
            tasks: Some(GateArtifact {
                path: "tasks.md".to_string(),
                markdown: "- [ ] [serial] r[demo.behavior] linked\n- [ ] [depends:demo.behavior] dependency".to_string(),
            }),
            delta_specs: vec![GateArtifact {
                path: "spec.md".to_string(),
                markdown: "### Requirement: Demo behavior\nr[demo.behavior] The system MUST behave."
                    .to_string(),
            }],
        };

        let report = evaluate_gate_snapshot(&snapshot, GateStage::Tasks, &policy(true, "strict"));
        assert_eq!(report.verdict, "PASS");
        assert!(report.issues.is_empty());
    }

    #[test]
    fn accepts_ears_style_requirement_and_scenario_structure() {
        let source = SpecSource {
            id: "demo".to_string(),
            path: "openspec/specs/demo/spec.md".to_string(),
            markdown: "### Requirement: Demo\nr[demo] The system MUST do the thing.\n\n#### Scenario: Happy path\n- GIVEN a precondition\n- WHEN an action happens\n- THEN an outcome occurs\n".to_string(),
        };
        assert!(validate_spec_source(&source).is_empty());
    }

    #[test]
    fn rejects_weak_requirements_and_incomplete_scenarios() {
        let source = SpecSource {
            id: "demo".to_string(),
            path: "openspec/specs/demo/spec.md".to_string(),
            markdown: "### Requirement: Demo\nThis feature works.\n\n#### Scenario: Missing when\n- GIVEN a precondition\n- THEN an outcome occurs\n".to_string(),
        };
        assert_eq!(
            validate_spec_source(&source),
            vec![
                "openspec/specs/demo/spec.md: Requirement: Demo missing requirement strength (MUST/SHALL/SHOULD/MAY)".to_string(),
                "openspec/specs/demo/spec.md: Scenario: Missing when missing WHEN clause(s)".to_string(),
            ]
        );
    }

    #[test]
    fn policy_aware_spec_substance_accepts_a_well_formed_requirement() {
        // r[verify cairn.lifecycle_validation.spec_substance]
        // r[verify cairn.lifecycle_validation.tests]
        // r[verify cairn.lifecycle_validation.tests.positive]
        let source = SpecSource {
            id: "demo".to_string(),
            path: "spec.md".to_string(),
            markdown: include_str!("../../../../../fixtures/lifecycle-substance/positive-spec.md")
                .to_string(),
        };
        let (inventory, issues) = validate_spec_source_with_policy(
            &source,
            &policy(true, "strict").artifact_substance_policy,
        );
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(inventory.requirement_blocks, 1);
        assert_eq!(inventory.substantive_requirement_blocks, 1);
        assert_eq!(inventory.scenario_blocks, 1);
    }

    #[test]
    fn policy_aware_spec_substance_rejects_vacuity_borrowed_strength_and_bad_identity() {
        // r[verify cairn.lifecycle_validation.spec_substance.empty]
        // r[verify cairn.lifecycle_validation.spec_substance.borrowed_strength]
        // r[verify cairn.lifecycle_validation.spec_substance.identity]
        // r[verify cairn.lifecycle_validation.tests.negative]
        let fixtures = [
            (
                "empty",
                include_str!("../../../../../fixtures/lifecycle-substance/negative-empty-spec.md"),
            ),
            (
                "fenced",
                include_str!("../../../../../fixtures/lifecycle-substance/negative-fenced-spec.md"),
            ),
            (
                "commented",
                include_str!(
                    "../../../../../fixtures/lifecycle-substance/negative-commented-spec.md"
                ),
            ),
            (
                "borrowed",
                include_str!(
                    "../../../../../fixtures/lifecycle-substance/negative-borrowed-strength.md"
                ),
            ),
            (
                "multiple",
                include_str!(
                    "../../../../../fixtures/lifecycle-substance/negative-multiple-ids.md"
                ),
            ),
        ];
        let substance_policy = policy(true, "strict").artifact_substance_policy;
        for (id, markdown) in fixtures {
            let source = SpecSource {
                id: id.to_string(),
                path: format!("{id}.md"),
                markdown: markdown.to_string(),
            };
            let (inventory, issues) = validate_spec_source_with_policy(&source, &substance_policy);
            assert!(!issues.is_empty(), "{id}: {inventory:?}");
            assert_eq!(inventory.substantive_requirement_blocks, 0, "{id}");
        }
    }

    #[test]
    fn vacuous_artifacts_and_malformed_dependencies_fail_the_tasks_gate() {
        // r[verify cairn.lifecycle_validation.artifact_substance.empty]
        // r[verify cairn.lifecycle_validation.spec_substance.empty]
        // r[verify cairn.lifecycle_validation.task_substance.empty]
        // r[verify cairn.lifecycle_validation.task_substance.placeholder]
        // r[verify cairn.lifecycle_validation.dependency_marker_shape.invalid]
        // r[verify cairn.lifecycle_validation.mutation_block.consistent]
        let mut vacuous_change = change();
        vacuous_change.has_design = true;
        vacuous_change.delta_specs = vec!["spec.md".to_string()];
        vacuous_change.task_counts = Some(TaskCounts {
            total: 1,
            done: 1,
            in_progress: 0,
            todo: 0,
            unmarked: 0,
        });
        let snapshot = GateSnapshot {
            change: vacuous_change,
            proposal: Some(GateArtifact {
                path: "proposal.md".to_string(),
                markdown: "CAIRN_PLACEHOLDER".to_string(),
            }),
            design: Some(GateArtifact {
                path: "design.md".to_string(),
                markdown: "   \n".to_string(),
            }),
            tasks: Some(GateArtifact {
                path: "tasks.md".to_string(),
                markdown: "```markdown\n- [x] [depends:ignored] fenced example\n```\n<!-- - [x] [depends:ignored] commented example -->\n- [x] [depends:] CAIRN_PLACEHOLDER\n- [x] [depends:unclosed task\n- [x] [depends:../unsupported] invalid target"
                    .to_string(),
            }),
            delta_specs: vec![GateArtifact {
                path: "spec.md".to_string(),
                markdown: String::new(),
            }],
        };
        let report = evaluate_gate_snapshot(&snapshot, GateStage::Tasks, &policy(true, "strict"));
        assert_eq!(report.verdict, "FAIL");
        let messages = report
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        for code in [
            "lifecycle_validation.artifact_substance.proposal",
            "lifecycle_validation.artifact_substance.design",
            "lifecycle_validation.spec_substance.zero_requirements",
            "lifecycle_validation.task_substance.minimum",
            "lifecycle_validation.dependency_marker_shape.invalid",
        ] {
            assert!(
                messages.iter().any(|message| message.contains(code)),
                "missing {code}: {messages:?}"
            );
        }
        assert_eq!(report.substance.len(), 4);
    }

    #[test]
    fn checked_task_progress_cannot_override_a_substance_blocker() {
        // r[verify cairn.lifecycle_validation.task_progress_non_authority.reduced]
        // r[verify cairn.lifecycle_validation.task_progress_non_authority.blocked]
        let mut blocked_change = change();
        blocked_change.has_design = true;
        blocked_change.delta_specs = vec!["spec.md".to_string()];
        blocked_change.task_counts = Some(TaskCounts {
            total: 1,
            done: 1,
            in_progress: 0,
            todo: 0,
            unmarked: 0,
        });
        let snapshot = GateSnapshot {
            change: blocked_change,
            proposal: Some(GateArtifact {
                path: "proposal.md".to_string(),
                markdown: "Substantive proposal".to_string(),
            }),
            design: Some(GateArtifact {
                path: "design.md".to_string(),
                markdown: "Substantive design".to_string(),
            }),
            tasks: Some(GateArtifact {
                path: "tasks.md".to_string(),
                markdown: "- [x] [serial] CAIRN_PLACEHOLDER r[demo.behavior]".to_string(),
            }),
            delta_specs: vec![GateArtifact {
                path: "spec.md".to_string(),
                markdown: "### Requirement: Demo\nr[demo.behavior] The system MUST behave."
                    .to_string(),
            }],
        };
        let report = evaluate_gate_snapshot(&snapshot, GateStage::Tasks, &policy(true, "strict"));
        assert_eq!(report.verdict, "FAIL");
        assert!(report.issues.iter().any(|issue| issue
            .message
            .contains("lifecycle_validation.task_substance")));
        let tasks = report
            .substance
            .iter()
            .find(|inventory| inventory.kind == "tasks")
            .expect("tasks inventory");
        assert_eq!(tasks.task_done, 1);
        assert_eq!(tasks.substantive_tasks, 0);
    }
}
