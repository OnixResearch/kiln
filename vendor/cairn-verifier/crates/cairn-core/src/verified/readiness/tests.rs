use super::*;

#[test]
fn probe_causes_preserve_exact_targets_without_inventing_validator_issues() {
    let mut snapshot = snapshot();
    let gate = GateReport {
        change: snapshot.scope.change.clone(),
        stage: "tasks".into(),
        verdict: "FAIL".into(),
        issues: vec![crate::GateIssue {
            code: "probe.saved_evidence.invalid".into(),
            message: "receipt missing".into(),
            scope: "local".into(),
            promoted: "blocking".into(),
        }],
        substance: vec![],
        task_ordering: None,
    };
    let causes = ["receipts/first.json", "receipts/second.json"]
        .map(|target| readiness_cause("probe.saved_evidence.invalid", target, "receipt missing"));
    let observation = readiness_gate_with_external_causes(&snapshot.scope, &gate, &causes);
    assert_eq!(observation.state, ReadinessState::Blocked);
    assert_eq!(
        observation.decision,
        readiness_gate(&snapshot.scope, &gate).decision
    );
    assert_eq!(observation.causes, causes);
    snapshot.observations[3] = observation;
    let report = derive_readiness(&snapshot);
    for target in ["receipts/first.json", "receipts/second.json"] {
        assert!(report
            .actions
            .iter()
            .any(|action| action.kind == "evidence_refresh" && action.target == target));
    }
    let forged = [readiness_cause(
        "probe.saved_evidence.invalid",
        "receipts/forged.json",
        "not observed",
    )];
    let rejected = readiness_gate_with_external_causes(&snapshot.scope, &gate, &forged);
    assert_eq!(rejected.state, ReadinessState::Unavailable);
    assert!(!rejected
        .causes
        .iter()
        .any(|cause| cause.target == "receipts/forged.json"));
    assert!(rejected
        .causes
        .iter()
        .any(|cause| cause.code == "observation.cause_conflict"));
}

#[test]
fn missing_merge_observation_does_not_discard_archive_authority() {
    let scope = snapshot().scope;
    let policy = crate::validate_policy_json(
        serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let sync = crate::SyncPlan {
        change: scope.change.clone(),
        dry_run: true,
        mutated: false,
        actions: vec![],
        delta_specs: vec![],
        spec_effect: "none".into(),
        blocked: false,
        reasons: vec![],
    };
    let archive = crate::ArchivePlan {
        change: scope.change.clone(),
        dry_run: true,
        mutated: false,
        actions: vec![],
        spec_effect: "none".into(),
        blocked: false,
        reasons: vec![],
    };
    let graph = crate::build_change_dependency_graph(&[], &[]);
    for merge_binding in [Some(json!({"documents": []})), None] {
        let available = merge_binding.is_some();
        let observations = readiness_mutation_observations(ReadinessMutationInputs {
            scope: &scope,
            sync: &sync,
            archive: &archive,
            merge_binding,
            merge_reasons: &[],
            gate_reasons: &[],
            profile_reasons: &[],
            policy: &policy,
            dependencies: &graph,
        });
        assert_eq!(
            observations[0].state,
            if available {
                ReadinessState::Ready
            } else {
                ReadinessState::Unavailable
            }
        );
        assert_eq!(observations[1].state, ReadinessState::Ready);
        assert_eq!(observations[0].causes.is_empty(), available);
        assert!(observations[1].causes.is_empty());
    }
}

#[test]
fn profile_gate_inputs_include_dependency_closure_without_later_unrelated_artifacts() {
    let policy = crate::validate_policy_json(
        serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for schema in &policy.workflow_schemas {
        for stage in [
            crate::GateStage::Proposal,
            crate::GateStage::Design,
            crate::GateStage::Tasks,
        ] {
            let selected = readiness_gate_artifact_ids(schema, stage);
            for artifact in schema
                .artifacts
                .iter()
                .filter(|artifact| selected.contains(&artifact.id))
            {
                assert!(artifact
                    .requires
                    .iter()
                    .all(|required| selected.contains(required)));
            }
            assert_eq!(selected, readiness_gate_artifact_ids(schema, stage));
        }
    }
    let mut schema = policy
        .workflow_schemas
        .iter()
        .find(|schema| schema.id == "spec-driven")
        .unwrap()
        .clone();
    let proposal = schema
        .artifacts
        .iter()
        .position(|artifact| artifact.stage == "proposal")
        .unwrap();
    let tasks = schema
        .artifacts
        .iter()
        .find(|artifact| artifact.stage == "tasks")
        .unwrap()
        .id
        .clone();
    assert!(!readiness_gate_artifact_ids(&schema, crate::GateStage::Proposal).contains(&tasks));
    schema.artifacts[proposal].requires.push(tasks.clone());
    assert!(readiness_gate_artifact_ids(&schema, crate::GateStage::Proposal).contains(&tasks));
}

#[test]
fn schema_inputs_follow_review_bindings_and_custom_stages() {
    let policy = crate::validate_policy_json(
        serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let stages = [
        crate::GateStage::Proposal,
        crate::GateStage::Design,
        crate::GateStage::Tasks,
    ];
    for profile in ["no-spec-delta", "assurance", "outcome-machine"] {
        let mut schema = policy
            .workflow_schemas
            .iter()
            .find(|schema| schema.id == profile)
            .unwrap()
            .clone();
        // Custom stage assignment is independent of structural admission.
        for artifact in &mut schema.artifacts {
            artifact.stage = "archive".into();
            artifact.requires.clear();
        }
        for stage in stages {
            crate::admit_workflow_schemas(
                &[schema.clone()],
                &[
                    "archive".into(),
                    "proposal".into(),
                    "design".into(),
                    "tasks".into(),
                ],
            )
            .unwrap();
            let selected = readiness_gate_artifact_ids(&schema, stage);
            for artifact in &schema.artifacts {
                let required = matches!(
                    artifact.structural_admission.as_str(),
                    crate::WORKFLOW_ADMISSION_ACCEPTANCE | crate::WORKFLOW_ADMISSION_DELTA_SPECS
                ) || (stage != crate::GateStage::Proposal
                    && artifact.structural_admission == crate::WORKFLOW_ADMISSION_OUTCOME_MACHINE)
                    || (stage == crate::GateStage::Tasks
                        && schema.review_obligations.iter().any(|review| {
                            review.artifact_id == artifact.id
                                || review.required_artifacts.contains(&artifact.id)
                        }));
                assert_eq!(
                    selected.contains(&artifact.id),
                    required,
                    "{profile}/{stage:?}/{}",
                    artifact.id
                );
            }
        }
        if let Some(review) = schema.review_obligations.first().cloned() {
            schema
                .artifacts
                .iter_mut()
                .find(|artifact| artifact.id == review.artifact_id)
                .unwrap()
                .stage = "proposal".into();
            let selected = readiness_gate_artifact_ids(&schema, crate::GateStage::Proposal);
            assert!(selected.contains(&review.artifact_id));
            assert!(review
                .required_artifacts
                .iter()
                .all(|id| selected.contains(id)));
            // A required review input can itself have a later prerequisite.
            let extra = schema
                .artifacts
                .iter()
                .find(|artifact| !selected.contains(&artifact.id))
                .map(|artifact| artifact.id.clone());
            if let Some(extra) = extra {
                schema
                    .artifacts
                    .iter_mut()
                    .find(|artifact| artifact.id == review.required_artifacts[0])
                    .unwrap()
                    .requires
                    .push(extra.clone());
                let expanded = readiness_gate_artifact_ids(&schema, crate::GateStage::Proposal);
                assert!(expanded.contains(&extra));
                assert_eq!(
                    expanded,
                    readiness_gate_artifact_ids(&schema, crate::GateStage::Proposal)
                );
            }
        }
    }
}

#[test]
fn current_and_stale_reviews_select_exact_artifact_repairs() {
    let policy: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .unwrap();
    let schema: crate::WorkflowSchemaPolicy = serde_json::from_value(
        policy["schemas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "assurance")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let mut evidence = crate::WorkflowProfileEvidence {
        spec_effect: Some("delta".into()),
        spec_effect_rationale: None,
        delta_spec_paths: vec!["specs/demo/spec.md".into()],
        acceptance_markdown: None,
        tasks_markdown: None,
        outcome_machine_review: None,
        outcome_machine_evidence: Vec::new(),
        reviews: Vec::new(),
        artifact_hashes: schema
            .artifacts
            .iter()
            .map(|a| crate::WorkflowProfileArtifactHash {
                artifact_id: a.id.clone(),
                path: a.output_path.clone(),
                blake3: "a".repeat(64),
            })
            .collect(),
    };
    evidence.reviews = schema
        .review_obligations
        .iter()
        .map(|obligation| crate::WorkflowReviewEvidence {
            obligation_id: obligation.id.clone(),
            valid: true,
            review_kind: obligation.review_kind.clone(),
            decision: "approved".into(),
            receipt_hash: "b".repeat(64),
            reviewed_inputs: evidence
                .artifact_hashes
                .iter()
                .filter(|a| obligation.required_artifacts.contains(&a.artifact_id))
                .map(|a| crate::WorkflowReviewInputEvidence {
                    path: a.path.clone(),
                    hash: a.blake3.clone(),
                })
                .collect(),
        })
        .collect();
    let mut s = snapshot();
    let evaluation = crate::evaluate_workflow_profile(&schema, &evidence);
    readiness_profile_causes(
        &s.scope,
        &mut s.observations[0],
        &evaluation,
        &schema,
        &evidence,
    );
    assert!(!s.observations[0]
        .causes
        .iter()
        .any(|c| c.code == "cairn.workflow.profile.review"));
    let changed = evidence.reviews[0].reviewed_inputs[0].path.clone();
    evidence.reviews[0].reviewed_inputs[0].hash = "d".repeat(64);
    let evaluation = crate::evaluate_workflow_profile(&schema, &evidence);
    readiness_profile_causes(
        &s.scope,
        &mut s.observations[0],
        &evaluation,
        &schema,
        &evidence,
    );
    let report = derive_readiness(&s);
    assert!(report
        .actions
        .iter()
        .any(|a| a.kind == "evidence_refresh"
            && a.target == format!(".cairn/changes/demo/{changed}")));
}

#[test]
fn generated_policy_bounds_match_enforced_limits() {
    let policy: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/readiness.json"
    ))
    .unwrap();
    assert_eq!(policy["max_blockers"], MAX_READINESS_BLOCKERS);
    assert_eq!(
        policy["max_observation_bytes"],
        MAX_READINESS_OBSERVATION_BYTES
    );
    assert_eq!(policy["max_target_bytes"], MAX_READINESS_TARGET_BYTES);
}

#[test]
fn causal_actions_merge_stages_but_not_distinct_repairs() {
    let mut s = snapshot();
    for stage in [0, 1] {
        s.observations[stage].state = ReadinessState::Blocked;
        s.observations[stage].causes = vec![readiness_cause(
            "artifact.incomplete",
            "tasks.md",
            "missing tasks",
        )];
    }
    let report = derive_readiness(&s);
    let repairs = report
        .actions
        .iter()
        .filter(|a| a.kind == "artifact_repair")
        .collect::<Vec<_>>();
    assert_eq!(repairs.len(), 1);
    assert_eq!(repairs[0].causes.len(), 2);
    assert!(repairs[0]
        .causes
        .iter()
        .all(|id| report.blockers.iter().any(|b| &b.id == id)));
    s.observations[1].causes[0].target = "design.md".into();
    assert_eq!(
        derive_readiness(&s)
            .actions
            .iter()
            .filter(|a| a.kind == "artifact_repair")
            .count(),
        2
    );
    for stage in [0, 1] {
        s.observations[stage].causes = vec![readiness_cause(
            "unknown",
            "demo",
            &format!("cause {stage}"),
        )];
    }
    let report = derive_readiness(&s);
    assert_eq!(
        report
            .actions
            .iter()
            .filter(|a| a.kind == "unresolved")
            .count(),
        2
    );
    s.observations.reverse();
    assert_eq!(report.actions, derive_readiness(&s).actions);
}

fn snapshot() -> ReadinessSnapshot {
    let scope = ReadinessScope {
        change: "demo".into(),
        policy_hash: "a".repeat(64),
        source_hash: "b".repeat(64),
        evidence_hash: "c".repeat(64),
    };
    let observations = READINESS_STAGES
        .iter()
        .map(|stage| ReadinessObservation {
            scope: scope.clone(),
            stage: (*stage).into(),
            state: ReadinessState::Ready,
            decision: json!({"valid": true, "verdict": "PASS"}),
            causes: Vec::new(),
        })
        .collect();
    ReadinessSnapshot {
        scope,
        observations,
    }
}

#[test]
// r[verify cairn.unified_readiness.snapshot]
// r[verify cairn.unified_readiness.conformance]
fn equal_inputs_are_deterministic_and_conflicts_fail_closed() {
    let mut s = snapshot();
    let first = derive_readiness(&s);
    assert_eq!(first, derive_readiness(&s));
    assert_eq!(first.classification, "ready");
    assert!(first.actions.iter().any(|a| a.kind == "sync_preview"));
    s.observations[0].scope.source_hash = "d".repeat(64);
    let rejected = derive_readiness(&s);
    assert!(!rejected.valid);
    assert!(rejected
        .stages
        .iter()
        .all(|s| s.state == ReadinessState::Unavailable));
    assert!(!rejected.actions.iter().any(|a| a.kind == "sync_preview"));
}

#[test]
// r[verify cairn.unified_readiness.actions]
fn repair_targets_and_unknown_causes_remain_advisory() {
    let mut s = snapshot();
    s.observations[0].state = ReadinessState::Blocked;
    s.observations[0].causes = vec![
        readiness_cause(
            "artifact.incomplete",
            ".cairn/changes/demo/tasks.md",
            "missing tasks",
        ),
        readiness_cause("unknown", "demo", "unknown cause"),
    ];
    let report = derive_readiness(&s);
    assert!(report.valid);
    assert_eq!(report.classification, "blocked");
    assert!(report
        .actions
        .iter()
        .any(|a| a.kind == "artifact_repair" && a.target.ends_with("tasks.md")));
    assert!(report
        .actions
        .iter()
        .any(|a| a.kind == "unresolved" && a.argv.is_empty()));
    assert!(report.actions.iter().all(|a| a.authority == "advisory"));
    assert!(!report.mutation_authorized);
}

#[test]
// r[verify cairn.unified_readiness.actions]
fn actions_bind_producer_contracts_predecessors_and_unresolved_probe_declarations() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/agent-completion/contract.json"
    ))
    .unwrap();
    let mut s = snapshot();
    s.scope.policy_hash = contract["policy_hash"].as_str().unwrap().into();
    for observation in &mut s.observations {
        observation.scope = s.scope.clone();
    }
    s.observations[4] = readiness_completion(&s.scope, &contract, &json!([]), &"c".repeat(64));
    s.observations[5].state = ReadinessState::Blocked;
    s.observations[5].causes = vec![readiness_cause(
        "dependency.blocked",
        "upstream-change",
        "The upstream change is still active",
    )];
    s.observations[3].state = ReadinessState::Blocked;
    s.observations[3].causes = vec![readiness_cause(
        "probe.declarations.malformed",
        ".cairn/changes/demo/probes/probes.json",
        "saved probe declarations are not valid JSON",
    )];
    let report = derive_readiness(&s);
    let missing = report
        .actions
        .iter()
        .find(|action| action.kind == "evidence_refresh" && action.target == "tests")
        .expect("missing tests refresh action");
    assert!(missing
        .prerequisites
        .iter()
        .any(|value| value.starts_with("producer-contract:tests:")));
    let dependency = report
        .actions
        .iter()
        .find(|action| action.kind == "dependency_wait")
        .expect("dependency wait action");
    assert!(dependency
        .prerequisites
        .iter()
        .any(|value| value == "predecessor-change:upstream-change"));
    let unresolved = report
        .actions
        .iter()
        .find(|action| {
            action.kind == "unresolved" && action.target == ".cairn/changes/demo/probes/probes.json"
        })
        .expect("declaration-wide probe failure must stay unresolved");
    assert!(unresolved.argv.is_empty());
}

#[test]
// r[verify cairn.unified_readiness.decisions]
fn authoritative_class_does_not_override_stale_subject_or_missing_rows() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/agent-completion/contract.json"
    ))
    .unwrap();
    let results: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/agent-completion/results.json"
    ))
    .unwrap();
    let mut s = snapshot();
    s.scope.policy_hash = contract["policy_hash"].as_str().unwrap().into();
    for o in &mut s.observations {
        o.scope = s.scope.clone();
    }
    let passing = readiness_completion(&s.scope, &contract, &results, &"c".repeat(64));
    assert_eq!(passing.state, ReadinessState::Ready);
    let rejected = readiness_completion(&s.scope, &contract, &results, &"d".repeat(64));
    assert_eq!(rejected.decision["classification"], "authoritative");
    assert_eq!(rejected.decision["valid"], false);
    let mut contradictory = rejected.clone();
    contradictory.state = ReadinessState::Ready;
    contradictory.causes.clear();
    s.observations[4] = contradictory;
    assert!(!derive_readiness(&s).valid);
    s.observations[4] = rejected;
    let report = derive_readiness(&s);
    assert_eq!(report.stages[4].state, ReadinessState::Blocked);
    assert_eq!(report.stages[5].state, ReadinessState::Blocked);
    assert_eq!(report.stages[6].state, ReadinessState::Blocked);
    assert!(!report.actions.iter().any(|a| a.kind == "sync_preview"));
    assert_eq!(
        readiness_completion(&s.scope, &contract, &json!([]), &"c".repeat(64)).state,
        ReadinessState::Blocked
    );
}

#[test]
// r[verify cairn.unified_readiness.boundary]
fn unavailable_unsafe_duplicate_and_overbound_observations_fail_closed() {
    let mut s = snapshot();
    s.observations.remove(4);
    assert_eq!(
        derive_readiness(&s).stages[4].state,
        ReadinessState::Unavailable
    );
    s.observations.push(s.observations[0].clone());
    assert!(!derive_readiness(&s).valid);
    let mut s = snapshot();
    s.observations[0].state = ReadinessState::Blocked;
    s.observations[0].causes = vec![readiness_cause("artifact.incomplete", "../escape", "bad")];
    assert!(!derive_readiness(&s).valid);
    s.observations[0].causes =
        vec![readiness_cause("unknown", "demo", "bad"); MAX_READINESS_BLOCKERS + 1];
    assert!(!derive_readiness(&s).valid);
}

#[test]
// r[verify cairn.agent_readiness.decisions]
// r[verify cairn.agent_readiness.actions]
fn agent_projection_preserves_decisions_complete_actions_and_exact_identity() {
    let mut s = snapshot();
    s.observations[0].state = ReadinessState::Blocked;
    s.observations[0].causes = vec![readiness_cause(
        "artifact.incomplete",
        "tasks.md",
        "missing tasks",
    )];
    let report = derive_readiness(&s);
    let original = serde_json::to_value(&report).unwrap();
    let view = project_agent_readiness(&report, AgentViewRequest::default()).unwrap();
    assert_eq!(view["scope"], original["scope"]);
    assert_eq!(view["valid"], original["valid"]);
    assert_eq!(view["classification"], original["classification"]);
    assert_eq!(view["mutation_authorized"], false);
    assert_eq!(view["full_receipt_hash"], original["receipt_hash"]);
    assert_eq!(
        view["readiness_policy_hash"],
        original["readiness_policy_hash"]
    );
    let view_policy: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/readiness-view.json"
    ))
    .unwrap();
    assert_eq!(view["view_policy_hash"], canonical_json_hash(&view_policy));
    assert_ne!(view["view_policy_hash"], view["readiness_policy_hash"]);
    assert_eq!(view["input_hash"], original["input_hash"]);
    for (short, full) in view["stages"]
        .as_array()
        .unwrap()
        .iter()
        .zip(original["stages"].as_array().unwrap())
    {
        for key in ["stage", "state", "decision_hash"] {
            assert_eq!(short[key], full[key]);
        }
    }
    assert_eq!(view["actions"], original["actions"]);
    assert_eq!(view["blockers"], original["blockers"]);
    assert_eq!(view["omitted_stage_decisions"], 7);
    assert_eq!(view["complete"], false);
    assert_eq!(
        serde_json::to_value(derive_readiness(&s)).unwrap(),
        original
    );
    let first = &report.actions[0];
    let detail =
        readiness_detail(&report, DetailKind::Action, &first.id, &report.receipt_hash).unwrap();
    assert_eq!(detail["detail"], json!(first));
    assert_eq!(detail["full_receipt_hash"], original["receipt_hash"]);
    assert_eq!(
        readiness_detail(
            &report,
            DetailKind::Stage,
            "completion",
            &report.receipt_hash
        )
        .unwrap()["detail"],
        original["stages"][4]
    );
}

#[test]
// r[verify cairn.agent_readiness.detail]
fn agent_budget_boundaries_and_stale_or_unknown_details_fail_closed() {
    let report = derive_readiness(&snapshot());
    let base = project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: None,
            max_entries: Some(0),
        },
    )
    .unwrap();
    let size = serde_json::to_vec(&base).unwrap().len() + 1; // standalone CLI LF
    assert!(project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: Some(size),
            max_entries: Some(0)
        }
    )
    .is_ok());
    assert_eq!(
        project_agent_readiness(
            &report,
            AgentViewRequest {
                max_bytes: Some(size - 1),
                max_entries: Some(0)
            }
        )
        .unwrap_err(),
        "insufficient agent readiness budget for mandatory envelope"
    );
    assert_eq!(base["actions"].as_array().unwrap().len(), 0);
    assert_eq!(base["omitted_actions"], report.actions.len());
    assert_eq!(
        base["detail_selectors"].as_array().unwrap().len(),
        report.stages.len() + report.actions.len()
    );
    assert!(project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: Some(16385),
            max_entries: None
        }
    )
    .is_err());
    assert!(project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: None,
            max_entries: Some(17)
        }
    )
    .is_err());
    let stale = "a".repeat(64);
    assert_eq!(
        readiness_detail(&report, DetailKind::Stage, "artifact", &stale).unwrap_err(),
        "stale readiness detail receipt"
    );
    assert_eq!(
        readiness_detail(&report, DetailKind::Stage, "unknown", &report.receipt_hash).unwrap_err(),
        "unknown readiness detail id"
    );
    assert_eq!(
        readiness_detail(
            &report,
            DetailKind::Blocker,
            "../escape",
            &report.receipt_hash
        )
        .unwrap_err(),
        "unsafe readiness detail id"
    );
}

#[test]
fn agent_projection_never_truncates_oversized_action_argv_or_prerequisites() {
    let mut report = derive_readiness(&snapshot());
    let id = report.actions[0].id.clone();
    report.actions[0].argv = vec!["x".repeat(16384)];
    report.actions[0].prerequisites = vec!["precondition".into()];
    let view = project_agent_readiness(&report, AgentViewRequest::default()).unwrap();
    assert!(view["actions"].as_array().unwrap().is_empty());
    assert_eq!(view["omitted_actions"], report.actions.len());
    assert!(view["detail_selectors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|selector| selector["kind"] == "action" && selector["id"] == id));
    assert_eq!(view["complete"], false);
}

#[test]
fn agent_budget_preserves_omitted_selector_order_and_count_digit_transition() {
    let mut s = snapshot();
    s.observations[0].state = ReadinessState::Blocked;
    s.observations[0].causes = (0..11)
        .map(|index| {
            readiness_cause(
                "artifact.incomplete",
                &format!("tasks-{index:02}.md"),
                "missing task",
            )
        })
        .collect();
    let report = derive_readiness(&s);
    assert_eq!(report.blockers.len(), 11);
    let one = project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: None,
            max_entries: Some(1),
        },
    )
    .unwrap();
    let bytes = serde_json::to_vec(&one).unwrap().len() + 1;
    let bounded = project_agent_readiness(
        &report,
        AgentViewRequest {
            max_bytes: Some(bytes),
            max_entries: None,
        },
    )
    .unwrap();
    assert_eq!(bounded["blockers"], one["blockers"]);
    assert_eq!(bounded["omitted_blockers"], 10);
    assert_eq!(bounded["actions"].as_array().unwrap().len(), 0);
    let selectors = bounded["detail_selectors"].as_array().unwrap();
    let ids: Vec<&str> = selectors
        .iter()
        .map(|selector| selector["id"].as_str().unwrap())
        .collect();
    let expected: Vec<&str> = report
        .stages
        .iter()
        .map(|stage| stage.stage.as_str())
        .chain(
            report
                .blockers
                .iter()
                .skip(1)
                .map(|blocker| blocker.id.as_str()),
        )
        .chain(report.actions.iter().map(|action| action.id.as_str()))
        .collect();
    assert_eq!(ids, expected);
    assert_eq!(serde_json::to_vec(&bounded).unwrap().len() + 1, bytes);
}

#[test]
fn detail_byte_limit_counts_standalone_cli_lf() {
    let mut report = derive_readiness(&snapshot());
    report.actions[0].argv.clear();
    let id = report.actions[0].id.clone();
    let base = readiness_detail(&report, DetailKind::Action, &id, &report.receipt_hash).unwrap();
    let base_bytes = serde_json::to_vec(&base).unwrap().len() + 1;
    let fill = agent_view_policy().max_detail_bytes - base_bytes - 2; // [] becomes ["..."].
    report.actions[0].argv = vec!["x".repeat(fill)];
    let exact = readiness_detail(&report, DetailKind::Action, &id, &report.receipt_hash).unwrap();
    assert_eq!(
        serde_json::to_vec(&exact).unwrap().len() + 1,
        agent_view_policy().max_detail_bytes
    );
    report.actions[0].argv[0].push('x');
    assert_eq!(
        readiness_detail(&report, DetailKind::Action, &id, &report.receipt_hash).unwrap_err(),
        "readiness detail exceeds policy byte limit"
    );
}

#[test]
fn agent_projection_keeps_identity_conflicts_and_unsafe_targets_unavailable() {
    let mut s = snapshot();
    s.observations[0].scope.evidence_hash = "d".repeat(64);
    s.observations[1].state = ReadinessState::Blocked;
    s.observations[1].causes = vec![readiness_cause(
        "artifact.incomplete",
        "../escape",
        "unsafe",
    )];
    let report = derive_readiness(&s);
    let view = project_agent_readiness(&report, AgentViewRequest::default()).unwrap();
    assert_eq!(view["valid"], false);
    assert_eq!(view["classification"], "unavailable");
    assert!(view["stages"]
        .as_array()
        .unwrap()
        .iter()
        .all(|stage| stage["state"] == "unavailable"));
    assert!(view["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d.as_str().unwrap().contains("identity.conflict")));
    assert!(view["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d.as_str().unwrap().contains("target.unsafe")));
    assert_eq!(view["mutation_authorized"], false);
    assert_eq!(view["full_receipt_hash"], report.receipt_hash);
}
