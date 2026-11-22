use super::super::readiness::{ReadinessAction, ReadinessScope, ReadinessStage};
use super::*;

// Synthetic contract controls: these are not execution evidence or validator receipts.
fn h(byte: u8) -> String {
    format!("{byte:064x}")
}
fn bind_projection(source: &mut SourceCohort) {
    source.revision.identity = digest(json!({"artifacts": source.artifacts}));
}
fn synthetic_input() -> SelectedWorkInput {
    let policy_hash = h(2);
    let mut source = SourceCohort {
        revision: SourceRevision {
            kind: SourceRevisionKind::Projection,
            identity: String::new(),
        },
        artifacts: vec![SourceArtifact {
            root: SourceRoot::Lifecycle,
            path: ".cairn/changes/demo/tasks.md".into(),
            bytes: 25,
            blake3: h(3),
            status: SourceStatus::Present,
        }],
    };
    bind_projection(&mut source);
    SelectedWorkInput {
        producer: "cairn".into(),
        selection: Selection {
            change: "demo".into(),
            task: Some("A".into()),
        },
        source,
        policy_hash: policy_hash.clone(),
        profile: "spec-driven".into(),
        profile_hash: h(4),
        dependencies: vec![DependencyFact {
            task: "A".into(),
            prerequisite: "B".into(),
            state: Availability::Blocked,
            identity: h(5),
        }],
        completion: CompletionFact {
            state: Availability::Unavailable,
            contract_hash: None,
            results_hash: None,
            subject_hash: None,
        },
        context: ContextFact {
            selection_hash: h(6),
            local_hash: h(7),
            portable_hash: h(8),
            entries: vec![ContextEntry {
                id: "overview".into(),
                authority: "project_fact".into(),
                source_root: Some("project".into()),
                path: Some("docs/overview.md".into()),
                status: Availability::Ready,
                content_hash: Some(h(9)),
                text: Some("local-only context".into()),
            }],
        },
        readiness: ReadinessReport {
            schema: "cairn.readiness.v1".into(),
            scope: ReadinessScope {
                change: "demo".into(),
                policy_hash,
                source_hash: h(10),
                evidence_hash: h(11),
            },
            valid: true,
            classification: "blocked".into(),
            stages: READINESS_STAGES
                .iter()
                .map(|stage| ReadinessStage {
                    stage: (*stage).into(),
                    state: ReadinessState::Blocked,
                    decision_hash: h(12),
                    decision: json!({"synthetic":true}),
                })
                .collect(),
            blockers: vec![],
            actions: vec![ReadinessAction {
                id: "action".into(),
                kind: "diagnostic_inspection".into(),
                causes: vec![],
                target: "/private/checkout/.cairn/changes/demo/tasks.md".into(),
                prerequisites: vec![],
                authority: "advisory".into(),
                expected_observation: "inspect".into(),
                argv: vec!["cairn".into(), "--root".into(), "/private/checkout".into()],
            }],
            diagnostics: vec![],
            readiness_policy_hash: h(13),
            input_hash: h(14),
            receipt_hash: h(15),
            mutation_authorized: false,
            non_claims: vec![],
        },
        units: vec![
            ImplementationUnit {
                task: Some("A".into()),
                state: WorkState::Blocked,
                references: vec![
                    ScopedReference {
                        kind: ReferenceKind::Requirement,
                        id: "impl.demo".into(),
                        scope: ReferenceScope::Task("A".into()),
                    },
                    ScopedReference {
                        kind: ReferenceKind::Acceptance,
                        id: "spec.demo".into(),
                        scope: ReferenceScope::Change,
                    },
                ],
                verification_contracts: vec!["contract.json".into()],
            },
            ImplementationUnit {
                task: Some("B".into()),
                state: WorkState::Complete,
                references: vec![],
                verification_contracts: vec![],
            },
        ],
        remaining: vec![
            RemainingWork {
                kind: RemainingKind::Implementation,
                state: Availability::Blocked,
                subject: Some("A".into()),
            },
            RemainingWork {
                kind: RemainingKind::Completion,
                state: Availability::Unavailable,
                subject: None,
            },
            RemainingWork {
                kind: RemainingKind::Sync,
                state: Availability::Blocked,
                subject: None,
            },
            RemainingWork {
                kind: RemainingKind::Archive,
                state: Availability::Blocked,
                subject: None,
            },
        ],
        enumeration: Enumeration {
            complete: true,
            observed: 2,
            total: Some(2),
            budget_bytes: 67_108_864,
            consumed_bytes: 1024,
        },
        local_root: "/private/checkout".into(),
    }
}
fn round_trip(input: &SelectedWorkInput) -> SelectedWorkSnapshot {
    let policy = SelectedWorkPolicy::bundled().unwrap();
    let portable = compose(&policy, input, None).unwrap().portable();
    let wire = serde_json::to_string(&portable).unwrap();
    let decoded: SelectedWorkSnapshot = serde_json::from_str(&wire).unwrap();
    admit_portable(&policy, &decoded, &portable.cohort).unwrap();
    decoded
}

// r[verify cairn.selected_work.contract]
// r[verify cairn.selected_work.conformance]
#[test]
fn synthetic_unfinished_round_trip_preserves_typed_scope_and_authority() {
    let input = synthetic_input();
    let output = round_trip(&input);
    assert_eq!(
        output.units[0].references[0].kind,
        ReferenceKind::Requirement
    );
    assert_eq!(
        output.units[0].references[0].scope,
        ReferenceScope::Task("A".into())
    );
    assert_eq!(
        output.units[0].references[1].kind,
        ReferenceKind::Acceptance
    );
    assert_eq!(output.units[0].references[1].scope, ReferenceScope::Change);
    assert_eq!(output.readiness.stages[0].state, ReadinessState::Blocked);
    assert_eq!(output.readiness.actions[0].authority, "advisory");
    assert_eq!(output.readiness.actions[0].portable_argv[2], "$ROOT");
    assert_eq!(
        output.readiness.actions[0].target,
        "$ROOT/.cairn/changes/demo/tasks.md"
    );
    assert!(output.local.is_none());
    assert!(!serde_json::to_string(&output)
        .unwrap()
        .contains("local-only context"));
    assert!(!serde_json::to_string(&output)
        .unwrap()
        .contains("/private/checkout"));
    assert_eq!(
        output.semantic_extensions.dependency_meanings,
        SemanticAvailability::Unsupported
    );
    // Without a selected definition, obligations are explicitly not selected,
    // never an empty satisfied set.
    assert_eq!(
        output.semantic_extensions.obligation_explanations,
        SemanticAvailability::NotSelected
    );
    assert_eq!(output.obligations, SelectedObligations::NotSelected);
}
// r[verify cairn.selected_work.conformance]
#[test]
fn synthetic_completion_only_is_not_an_empty_successful_queue() {
    let mut input = synthetic_input();
    input.selection.task = None;
    input.units.clear();
    input.dependencies.clear();
    input.remaining = vec![
        RemainingWork {
            kind: RemainingKind::Completion,
            state: Availability::Blocked,
            subject: None,
        },
        RemainingWork {
            kind: RemainingKind::Sync,
            state: Availability::Blocked,
            subject: None,
        },
        RemainingWork {
            kind: RemainingKind::Archive,
            state: Availability::Blocked,
            subject: None,
        },
    ];
    assert_eq!(
        round_trip(&input).remaining[0].kind,
        RemainingKind::Completion
    );
    input.remaining.clear();
    assert_eq!(
        compose(&SelectedWorkPolicy::bundled().unwrap(), &input, None).unwrap_err(),
        SelectedWorkError::Incomplete
    );
}

#[test]
fn generated_policy_admission_rejects_malformed_and_out_of_range_values() {
    let policy = SelectedWorkPolicy::bundled().unwrap();
    assert_eq!(
        SelectedWorkPolicy::admit_json("{}"),
        Err(SelectedWorkError::InvalidPolicy)
    );
    let mut value = serde_json::to_value(policy).unwrap();
    value["max_observation_bytes"] = json!(67_108_865);
    assert_eq!(
        SelectedWorkPolicy::admit_json(&value.to_string()),
        Err(SelectedWorkError::InvalidPolicy)
    );
    value["max_observation_bytes"] = json!(67_108_864);
    value["unrecognized"] = json!(true);
    assert_eq!(
        SelectedWorkPolicy::admit_json(&value.to_string()),
        Err(SelectedWorkError::InvalidPolicy)
    );
}
// r[verify cairn.selected_work.contract]
#[test]
fn identity_domains_invalidate_without_changing_unit_count_or_stage_state() {
    let base = synthetic_input();
    let original = round_trip(&base).cohort;
    let mut changed = base.clone();
    changed.source.artifacts[0].blake3 = h(20);
    bind_projection(&mut changed.source);
    assert_ne!(round_trip(&changed).cohort.source, original.source);
    changed = base.clone();
    changed.profile_hash = h(21);
    assert_ne!(round_trip(&changed).cohort.intent, original.intent);
    changed = base.clone();
    changed.dependencies[0].identity = h(22);
    assert_ne!(round_trip(&changed).cohort.intent, original.intent);
    changed = base.clone();
    changed.context.portable_hash = h(23);
    assert_ne!(round_trip(&changed).cohort.context, original.context);
    changed = base.clone();
    changed.completion.subject_hash = Some(h(24));
    assert_ne!(
        round_trip(&changed).cohort.verification_subject,
        original.verification_subject
    );
    changed = base.clone();
    changed.readiness.scope.evidence_hash = h(25);
    assert_ne!(
        round_trip(&changed).cohort.verification_subject,
        original.verification_subject
    );
}
// r[verify cairn.selected_work.contract]
#[test]
fn dependency_domain_binds_closure_edges_and_prerequisite_state_only() {
    let mut base = synthetic_input();
    base.dependencies.push(DependencyFact {
        task: "demo".into(),
        prerequisite: "prerequisite-change".into(),
        state: Availability::Blocked,
        identity: h(40),
    });
    let original = round_trip(&base).cohort;

    // A changed prerequisite edge alters the dependency domain.
    let mut edge = base.clone();
    edge.dependencies[1].prerequisite = "other-change".into();
    let edged = round_trip(&edge).cohort;
    assert_ne!(edged.dependencies, original.dependencies);
    assert_ne!(edged.intent, original.intent);

    // A prerequisite completing alters it too.
    let mut completed = base.clone();
    completed.dependencies[0].state = Availability::Ready;
    assert_ne!(
        round_trip(&completed).cohort.dependencies,
        original.dependencies
    );

    // Prose-only edits move source and intent but not dependencies.
    let mut prose = base.clone();
    prose.source.artifacts[0].blake3 = h(41);
    bind_projection(&mut prose.source);
    prose.dependencies[0].identity = h(42);
    let prosed = round_trip(&prose).cohort;
    assert_ne!(prosed.source, original.source);
    assert_ne!(prosed.intent, original.intent);
    assert_eq!(prosed.dependencies, original.dependencies);

    // Reordering unordered units, edges and remaining work alters neither.
    let mut reordered = base.clone();
    reordered.units.reverse();
    reordered.dependencies.reverse();
    reordered.remaining.reverse();
    let permuted = round_trip(&reordered).cohort;
    assert_eq!(permuted.dependencies, original.dependencies);
    assert_eq!(permuted.intent, original.intent);
}
// r[verify cairn.selected_work.contract]
#[test]
fn synthetic_consumer_rejects_stale_crossed_and_malformed_wire() {
    let valid = round_trip(&synthetic_input());
    let policy = SelectedWorkPolicy::bundled().unwrap();
    let mut stale = valid.clone();
    stale.source.artifacts[0].blake3 = h(30);
    assert_eq!(
        admit_portable(&policy, &stale, &valid.cohort),
        Err(SelectedWorkError::InvalidIdentity)
    );
    let mut crossed = valid.clone();
    crossed.completion.subject_hash = Some(h(31));
    assert_eq!(
        admit_portable(&policy, &crossed, &valid.cohort),
        Err(SelectedWorkError::CrossedSubject)
    );
    let mut wire = serde_json::to_value(&valid).unwrap();
    wire["context"]["entries"][0]["text"] = json!("forbidden raw context");
    assert!(serde_json::from_value::<SelectedWorkSnapshot>(wire).is_err());
    let mut wire = serde_json::to_value(&valid).unwrap();
    wire["readiness"]["stages"] = json!([]);
    let malformed: SelectedWorkSnapshot = serde_json::from_value(wire).unwrap();
    assert_eq!(
        admit_portable(&policy, &malformed, &valid.cohort),
        Err(SelectedWorkError::StaleCohort)
    );
    let mut incomplete = valid.clone();
    incomplete.readiness.stages.pop();
    incomplete.cohort = cohort(&incomplete);
    assert_eq!(
        admit_portable(&policy, &incomplete, &incomplete.cohort),
        Err(SelectedWorkError::InvalidReadiness)
    );
    let mut private = valid.clone();
    private.readiness.actions[0].target = "/home/private".into();
    private.cohort = cohort(&private);
    assert_eq!(
        admit_portable(&policy, &private, &private.cohort),
        Err(SelectedWorkError::UnsafePortable)
    );
}
#[test]
fn synthetic_consumer_refuses_partial_budget_and_missing_detail() {
    let policy = SelectedWorkPolicy::bundled().unwrap();
    let mut input = synthetic_input();
    input.enumeration.complete = false;
    assert_eq!(
        compose(&policy, &input, None).unwrap_err(),
        SelectedWorkError::Incomplete
    );
    input.enumeration.complete = true;
    input.enumeration.consumed_bytes = input.enumeration.budget_bytes + 1;
    assert_eq!(
        compose(&policy, &input, None).unwrap_err(),
        SelectedWorkError::BudgetExceeded
    );
    let mut portable = round_trip(&synthetic_input());
    portable.enumeration.total = None;
    portable.cohort = cohort(&portable);
    assert_eq!(
        admit_portable(&policy, &portable, &portable.cohort),
        Err(SelectedWorkError::Incomplete)
    );
    let mut portable = round_trip(&synthetic_input());
    portable.context.entries[0].path = Some("/private/detail".into());
    portable.cohort = cohort(&portable);
    assert_eq!(
        admit_portable(&policy, &portable, &portable.cohort),
        Err(SelectedWorkError::InvalidContext)
    );
}
// r[verify cairn.selected_work.observation]
#[test]
fn scoped_closure_does_not_inherit_independent_malformed_task() {
    let policy = SelectedWorkPolicy::bundled().unwrap();
    let mut input = synthetic_input();
    input.units.push(ImplementationUnit {
        task: Some("C".into()),
        state: WorkState::Unavailable,
        references: vec![],
        verification_contracts: vec![],
    });
    assert_eq!(
        compose(&policy, &input, None).unwrap_err(),
        SelectedWorkError::InvalidSelection
    );
    input.units.pop();
    assert!(compose(&policy, &input, None).is_ok());
    input.units.push(input.units[0].clone());
    assert_eq!(
        compose(&policy, &input, None).unwrap_err(),
        SelectedWorkError::Duplicate
    );
    input.units.pop();
    input.dependencies.push(DependencyFact {
        task: "demo".into(),
        prerequisite: "upstream".into(),
        state: Availability::Unavailable,
        identity: h(27),
    });
    assert!(compose(&policy, &input, None).is_ok());
}

// r[verify cairn.selected_work.conformance]
#[test]
fn scoped_markers_keep_requirement_and_acceptance_domains_distinct() {
    let references = extract_scoped_references(
        concat!(
            "<!-- r[im",
            "pl cairn.demo] a[AC-12] r[im",
            "pl cairn.demo] -->"
        ),
        ReferenceScope::Task("A".into()),
    )
    .unwrap();
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].kind, ReferenceKind::Requirement);
    assert_eq!(references[0].id, "impl cairn.demo");
    assert_eq!(references[1].kind, ReferenceKind::Acceptance);
    assert_eq!(references[1].id, "AC-12");
    assert_eq!(
        extract_scoped_references("r[unfinished", ReferenceScope::Change),
        Err(SelectedWorkError::InvalidSelection)
    );
    assert!(
        extract_scoped_references("notar[not-a-marker]", ReferenceScope::Change)
            .unwrap()
            .is_empty()
    );
}

// r[verify cairn.selected_work.boundary]
#[test]
fn portable_policy_path_is_a_projection_not_a_native_action_rewrite() {
    let mut input = synthetic_input();
    input.readiness.actions[0]
        .argv
        .extend(["--policy".into(), "/opt/cairn/policy.json".into()]);
    let original = input.readiness.actions[0].argv.clone();
    let snapshot = compose(&SelectedWorkPolicy::bundled().unwrap(), &input, None).unwrap();
    assert_eq!(
        snapshot.local.as_ref().unwrap().readiness.actions[0].argv,
        original
    );
    let portable = snapshot.portable();
    assert_eq!(portable.readiness.actions[0].portable_argv[4], "$POLICY");
    assert_eq!(
        portable.readiness.actions[0].original_argv_hash,
        digest(json!(original))
    );
    admit_portable(
        &SelectedWorkPolicy::bundled().unwrap(),
        &portable,
        &portable.cohort,
    )
    .unwrap();
}

#[test]
fn explicit_missing_source_remains_unavailable_not_successful_empty() {
    let mut input = synthetic_input();
    input.source.artifacts[0].status = SourceStatus::Missing;
    input.source.artifacts[0].bytes = 0;
    input.source.artifacts[0].blake3.clear();
    bind_projection(&mut input.source);
    input.readiness.stages[0].state = ReadinessState::Unavailable;
    input.units[0].state = WorkState::Unavailable;
    let portable = round_trip(&input);
    assert_eq!(portable.source.artifacts[0].status, SourceStatus::Missing);
    assert_eq!(
        portable.readiness.stages[0].state,
        ReadinessState::Unavailable
    );
    assert_eq!(portable.units[0].state, WorkState::Unavailable);
}
