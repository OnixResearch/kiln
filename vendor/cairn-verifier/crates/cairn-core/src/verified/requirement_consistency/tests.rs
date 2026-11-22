use super::*;
use crate::verified::content_bound_requirement_registry::ContentBoundSourceSpan;
use crate::verified::typed_requirement_contracts::{
    parse_typed_requirement_inventory_value, FiniteDomain, ObservableMapping,
};

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn row(requirement_id: &str) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: requirement_id.into(),
        specification_path: ".cairn/specs/requirement-consistency/spec.md".into(),
        specification_blake3: HASH_A.into(),
        source_span: ContentBoundSourceSpan {
            start_byte: 0,
            end_byte: 64,
            start_line: 1,
            end_line: 1,
        },
        requirement_body_blake3: HASH_B.into(),
        row_blake3: HASH_C.into(),
    }
}

fn profile() -> SemanticProfile {
    SemanticProfile {
        id: "cairn.requirement-semantics.finite.v1".into(),
        interpretation:
            "finite-enumeration.equality.nominal-domains.total-observables.single-step.v1".into(),
        max_nodes: 512,
        max_depth: 16,
        max_domain_values: 8,
        max_steps: 10_000,
    }
}

fn domain(values: &[&str]) -> FiniteDomain {
    FiniteDomain {
        values: values.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn literal(domain: &str, value: &str) -> FiniteTerm {
    FiniteTerm::Literal {
        value: FiniteValue {
            domain: domain.into(),
            value: value.into(),
        },
    }
}

fn variable(name: &str) -> FiniteTerm {
    FiniteTerm::Variable { name: name.into() }
}

fn observation(mapping: &str, arguments: Vec<FiniteTerm>) -> FiniteTerm {
    FiniteTerm::Observable {
        mapping: mapping.into(),
        arguments,
    }
}

fn equal(left: FiniteTerm, right: FiniteTerm) -> FiniteExpr {
    FiniteExpr::Equal { left, right }
}

fn mapping(arguments: &[&str], result: &str) -> ObservableMapping {
    ObservableMapping {
        arguments: arguments.iter().map(|name| (*name).to_string()).collect(),
        result: result.into(),
        phase: "state".into(),
        source: "crates/cairn-core/src/verified/requirement_consistency.rs".into(),
        projection: "finite test observation".into(),
    }
}

struct ContractSpec<'a> {
    requirement_id: &'a str,
    domains: BTreeMap<String, FiniteDomain>,
    observables: BTreeMap<String, ObservableMapping>,
    assumptions: Vec<FiniteExpr>,
    activation: FiniteExpr,
    guarantee: FiniteExpr,
}

fn contract(spec: ContractSpec<'_>) -> SemanticContract {
    SemanticContract {
        profile: profile().id,
        proposed_by: "requirement-consistency-test-author".into(),
        requirement: row(spec.requirement_id),
        domains: spec.domains,
        observables: spec.observables,
        assumptions: spec.assumptions,
        activation: spec.activation,
        guarantee: spec.guarantee,
        supported_portion: "finite consistency analysis test contract".into(),
        informal_remainder: "not an admitted behavior claim".into(),
    }
}

fn case(contracts: Vec<SemanticContract>, selected: Vec<&str>) -> ConsistencyCase {
    ConsistencyCase {
        schema: CONSISTENCY_CASE_SCHEMA_V1.into(),
        profile: profile(),
        subject: AnalysisSubject {
            subject_blake3: HASH_C.into(),
            policy_blake3: HASH_B.into(),
            target: "onixresearch/cairn".into(),
            environment: "linux-x86_64".into(),
        },
        contracts: contracts
            .into_iter()
            .map(|contract| CaseContract {
                native_requirement: contract.requirement.clone(),
                contract,
                correspondence_review: None,
            })
            .collect(),
        selected: selected.iter().map(|id| (*id).to_string()).collect(),
        correspondence: Vec::new(),
        authorized_reviewers: Vec::new(),
        native_obligations: vec![NativeObligationBinding {
            requirement_id: selected[0].to_string(),
            obligation_id: "obligation.requirement-consistency".into(),
            finding_id: "finding.requirement-consistency".into(),
        }],
        producer: ProducerIdentity {
            producer: "cairn-core".into(),
            adapter: "finite-enumeration".into(),
            version: "1".into(),
        },
    }
}

fn bounds() -> ConsistencyBounds {
    ConsistencyBounds {
        max_contracts: 8,
        max_guarantees: 16,
        max_cells: 4_096,
        max_states: 100_000,
        max_evaluations: 1_000_000,
        max_depth: 32,
    }
}

fn policy(queries: Vec<AnalysisQueryKind>, bounds: ConsistencyBounds) -> ConsistencyPolicy {
    ConsistencyPolicy {
        schema: CONSISTENCY_POLICY_SCHEMA_V1.into(),
        queries,
        bounds,
    }
}

fn all_queries() -> Vec<AnalysisQueryKind> {
    vec![
        AnalysisQueryKind::AssumptionsSatisfiable,
        AnalysisQueryKind::ActivationReachable,
        AnalysisQueryKind::GuaranteesCompatible,
        AnalysisQueryKind::GuaranteeConstraining,
    ]
}

fn query(report: &ConsistencyReport, kind: AnalysisQueryKind) -> Vec<&QueryOutcome> {
    report
        .queries
        .iter()
        .filter(|outcome| outcome.query.kind() == kind)
        .collect()
}

fn findings(report: &ConsistencyReport, kind: FindingKind) -> Vec<&AnalysisFinding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .collect()
}

fn bool_domain() -> FiniteDomain {
    domain(&["false", "true"])
}

fn identity_domain() -> FiniteDomain {
    domain(&["a", "b"])
}

/// One requirement whose guarantee both duplicates an assumption (redundant)
/// and constrains a subject identity (meaningful).
fn conjunction_contract() -> SemanticContract {
    contract(ContractSpec {
        requirement_id: "cairn.test.requirement_consistency.conjunction",
        domains: BTreeMap::from([
            ("bool".to_string(), bool_domain()),
            ("identity".to_string(), identity_domain()),
        ]),
        observables: BTreeMap::from([
            ("present".to_string(), mapping(&[], "bool")),
            ("final_subject".to_string(), mapping(&[], "identity")),
            ("receipt_subject".to_string(), mapping(&[], "identity")),
        ]),
        assumptions: vec![equal(
            observation("receipt_subject", vec![]),
            observation("final_subject", vec![]),
        )],
        activation: equal(observation("present", vec![]), literal("bool", "true")),
        guarantee: FiniteExpr::And {
            expressions: vec![
                equal(
                    observation("receipt_subject", vec![]),
                    observation("final_subject", vec![]),
                ),
                equal(
                    observation("final_subject", vec![]),
                    literal("identity", "a"),
                ),
            ],
        },
    })
}

#[test]
// r[verify cairn.requirement_consistency.queries]
fn satisfiable_constraints_yield_a_replayable_model() {
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![conjunction_contract()],
        vec!["cairn.test.requirement_consistency.conjunction"],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");
    assert!(report.advisory);
    assert_eq!(report.schema, CONSISTENCY_REPORT_SCHEMA_V1);
    assert_eq!(
        report.scope.subject_blake3.as_str(),
        HASH_C,
        "the exact subject is recorded"
    );
    assert_eq!(
        report.scope.environmental_assumptions.len(),
        1,
        "the shared assumption is named exactly once"
    );
    assert_eq!(report.scope.guarantees.len(), 2);
    assert_eq!(
        report.scope.contracts[0].quantified_variables.len(),
        0,
        "an unquantified contract binds no variables"
    );

    let assumptions = &query(&report, AnalysisQueryKind::AssumptionsSatisfiable)[0];
    assert_eq!(assumptions.status, AnalysisStatus::Consistent);
    assert_eq!(assumptions.basis.kind, SearchBasisKind::Witness);
    let witness = assumptions
        .witness
        .as_ref()
        .expect("a consistent query retains its model");
    assert_eq!(witness.query, assumptions.query.label());
    assert_eq!(witness.tables.len(), 1);
    assert_eq!(
        witness.tables[0].requirement_id,
        "cairn.test.requirement_consistency.conjunction"
    );

    let activation = &query(&report, AnalysisQueryKind::ActivationReachable)[0];
    assert_eq!(activation.status, AnalysisStatus::Consistent);
    let compatible = &query(&report, AnalysisQueryKind::GuaranteesCompatible)[0];
    assert_eq!(compatible.status, AnalysisStatus::Consistent);

    let replay = replay_witness(
        &policy,
        &case,
        compatible.witness.as_ref().expect("compatible witness"),
    )
    .expect("replay admission");
    assert!(replay.replays, "{}", replay.detail);
    assert_eq!(replay.contract_evaluations.len(), 1);
    assert_eq!(replay.query, compatible.query.label());
    assert_eq!(
        replay.contract_evaluations[0].requirement_id,
        "cairn.test.requirement_consistency.conjunction"
    );

    let reachable = findings(&report, FindingKind::ActivationReachable);
    assert_eq!(reachable.len(), 1);
    assert_eq!(reachable[0].disposition, UNRESOLVED_DISPOSITION);
    assert_eq!(
        reachable[0].native_obligations[0].obligation_id, "obligation.requirement-consistency",
        "findings attach to the caller-named native obligation without resolving it"
    );
    assert!(reachable[0]
        .fragments
        .iter()
        .any(|fragment| fragment.location == "activation"));
}

#[test]
// r[verify cairn.requirement_consistency.nonvacuity]
fn redundant_and_constraining_guarantees_are_distinguished() {
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![conjunction_contract()],
        vec!["cairn.test.requirement_consistency.conjunction"],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");

    let redundant = findings(&report, FindingKind::RedundantGuarantee);
    assert_eq!(
        redundant.len(),
        1,
        "the guarantee duplicated by an assumption adds no excluded behavior"
    );
    let redundant_expression = &redundant[0]
        .fragments
        .last()
        .expect("guarantee fragment")
        .expression;
    assert_eq!(
        redundant_expression,
        &equal(
            observation("receipt_subject", vec![]),
            observation("final_subject", vec![])
        ),
        "the finding names the exact redundant formula fragment"
    );
    assert!(redundant[0].witness.is_none());
    assert_eq!(redundant[0].status, Some(AnalysisStatus::Inconsistent));
    assert!(
        redundant[0]
            .detail
            .contains("does not make the requirement useless"),
        "redundancy is not reported as removal guidance"
    );

    let constraining = findings(&report, FindingKind::ConstrainingGuarantee);
    assert_eq!(
        constraining.len(),
        1,
        "removing the constraining guarantee admits a previously excluded behavior"
    );
    let withheld = query(&report, AnalysisQueryKind::GuaranteeConstraining)
        .into_iter()
        .find(|outcome| outcome.status == AnalysisStatus::Consistent)
        .expect("one guarantee is constraining");
    let witness = constraining[0]
        .witness
        .as_ref()
        .expect("a distinguishing behavior is retained");
    assert_eq!(
        witness.query,
        withheld.query.label(),
        "the witness names the withheld guarantee"
    );
    let replay = replay_witness(&policy, &case, witness).expect("replay admission");
    assert!(replay.replays, "{}", replay.detail);
    assert!(
        constraining[0]
            .fragments
            .iter()
            .any(|fragment| fragment.location == "assumptions[0]"),
        "the reproducing assumption is named with the finding"
    );
}

fn contradictory_contract() -> SemanticContract {
    contract(ContractSpec {
        requirement_id: "cairn.test.requirement_consistency.contradiction",
        domains: BTreeMap::from([("bool".to_string(), bool_domain())]),
        observables: BTreeMap::from([("present".to_string(), mapping(&[], "bool"))]),
        assumptions: vec![
            equal(observation("present", vec![]), literal("bool", "true")),
            equal(observation("present", vec![]), literal("bool", "false")),
        ],
        activation: equal(observation("present", vec![]), literal("bool", "true")),
        guarantee: FiniteExpr::Bool { value: true },
    })
}

#[test]
// r[verify cairn.requirement_consistency.statuses]
fn contradictory_assumptions_are_inconsistent_and_exhaustive() {
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![contradictory_contract()],
        vec!["cairn.test.requirement_consistency.contradiction"],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");

    let assumptions = &query(&report, AnalysisQueryKind::AssumptionsSatisfiable)[0];
    assert_eq!(assumptions.status, AnalysisStatus::Inconsistent);
    assert_eq!(
        assumptions.basis.kind,
        SearchBasisKind::Exhaustive,
        "a negative result needs a complete-search basis"
    );
    assert!(assumptions.basis.detail.contains("complete exploration"));
    assert!(assumptions.witness.is_none());
    assert_eq!(assumptions.basis.model_cells, 1);

    let contradictions = findings(&report, FindingKind::ContradictoryAssumptions);
    assert!(
        !contradictions.is_empty(),
        "the unsatisfiable conjunction is reported explicitly"
    );
    let whole_set = contradictions
        .iter()
        .find(|finding| finding.contracts.len() == 1 && finding.conflicting.len() == 1)
        .expect("one scoped contradictory-assumption finding");
    assert!(whole_set.conflicting_exploration_complete);
    let conflict = &whole_set.conflicting[0];
    assert!(conflict.minimal, "both singletons were shown satisfiable");
    assert_eq!(conflict.members.len(), 2);
    assert!(conflict
        .members
        .iter()
        .any(|member| member.ends_with("::assumptions[0]")));
    assert!(conflict
        .members
        .iter()
        .any(|member| member.ends_with("::assumptions[1]")));

    let activation = &query(&report, AnalysisQueryKind::ActivationReachable)[0];
    assert_eq!(
        activation.status,
        AnalysisStatus::Vacuous,
        "no model activates the requirement, so the implication is vacuous"
    );
    assert_eq!(
        query(&report, AnalysisQueryKind::GuaranteesCompatible)[0].status,
        AnalysisStatus::Vacuous,
        "no model of the shared premise leaves every guarantee vacuously implied"
    );
    assert_eq!(
        findings(&report, FindingKind::VacuousRequirementSet).len(),
        1
    );
    assert!(
        findings(&report, FindingKind::UnreachableActivation).is_empty(),
        "an unreachable activation must stay distinct from contradictory assumptions"
    );
}

#[test]
// r[verify cairn.requirement_consistency.nonvacuity]
fn identical_guarantee_text_cannot_hide_vacuous_activation() {
    let vacuous = contract(ContractSpec {
        requirement_id: "cairn.test.requirement_consistency.vacuous",
        domains: BTreeMap::from([
            ("bool".to_string(), bool_domain()),
            ("identity".to_string(), identity_domain()),
        ]),
        observables: BTreeMap::from([
            ("left".to_string(), mapping(&[], "identity")),
            ("right".to_string(), mapping(&[], "identity")),
            ("gate".to_string(), mapping(&[], "bool")),
        ]),
        assumptions: vec![equal(observation("gate", vec![]), literal("bool", "false"))],
        activation: equal(observation("gate", vec![]), literal("bool", "true")),
        guarantee: equal(observation("left", vec![]), observation("right", vec![])),
    });
    let reachable = contract(ContractSpec {
        requirement_id: "cairn.test.requirement_consistency.reachable",
        domains: BTreeMap::from([
            ("bool".to_string(), bool_domain()),
            ("identity".to_string(), identity_domain()),
        ]),
        observables: BTreeMap::from([
            ("left".to_string(), mapping(&[], "identity")),
            ("right".to_string(), mapping(&[], "identity")),
            ("gate".to_string(), mapping(&[], "bool")),
        ]),
        assumptions: Vec::new(),
        activation: equal(observation("gate", vec![]), literal("bool", "true")),
        guarantee: equal(observation("left", vec![]), observation("right", vec![])),
    });
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![vacuous.clone(), reachable.clone()],
        vec![
            "cairn.test.requirement_consistency.vacuous",
            "cairn.test.requirement_consistency.reachable",
        ],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");

    let vacuous_guarantee = &report.scope.guarantees[0];
    let reachable_guarantee = &report.scope.guarantees[1];
    assert_eq!(
        vacuous_guarantee.expression, reachable_guarantee.expression,
        "both requirements state the same guarantee"
    );
    assert_eq!(
        vacuous_guarantee.fragment_blake3, reachable_guarantee.fragment_blake3,
        "a predicate-name or fragment-name check cannot distinguish these requirements"
    );

    let activation = query(&report, AnalysisQueryKind::ActivationReachable);
    let vacuous_activation = activation
        .iter()
        .find(|outcome| {
            outcome.query.label()
                == "activation_reachable:cairn.test.requirement_consistency.vacuous"
        })
        .expect("vacuous activation query");
    let reachable_activation = activation
        .iter()
        .find(|outcome| {
            outcome.query.label()
                == "activation_reachable:cairn.test.requirement_consistency.reachable"
        })
        .expect("reachable activation query");
    assert_eq!(vacuous_activation.status, AnalysisStatus::Vacuous);
    assert_eq!(reachable_activation.status, AnalysisStatus::Consistent);
    assert!(reachable_activation.witness.is_some());

    let unreachable = findings(&report, FindingKind::UnreachableActivation);
    assert_eq!(unreachable.len(), 1);
    assert_eq!(
        unreachable[0].contracts[0].requirement_id,
        "cairn.test.requirement_consistency.vacuous"
    );
    assert_eq!(unreachable[0].status, Some(AnalysisStatus::Vacuous));
    assert!(
        unreachable[0].detail.contains("no model activates"),
        "the vacuity cause is stated"
    );
}

#[test]
// r[verify cairn.requirement_consistency.nonvacuity]
fn single_valued_and_unobserved_domains_are_named() {
    let contract = contract(ContractSpec {
        requirement_id: "cairn.test.requirement_consistency.trivial",
        domains: BTreeMap::from([
            ("flag".to_string(), domain(&["only"])),
            ("identity".to_string(), identity_domain()),
        ]),
        observables: BTreeMap::from([("value".to_string(), mapping(&[], "identity"))]),
        assumptions: Vec::new(),
        activation: FiniteExpr::Bool { value: true },
        guarantee: FiniteExpr::Forall {
            variable: "f".into(),
            domain: "flag".into(),
            expression: Box::new(equal(variable("f"), literal("flag", "only"))),
        },
    });
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![contract],
        vec!["cairn.test.requirement_consistency.trivial"],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");

    let trivial = findings(&report, FindingKind::TrivialQuantification);
    assert_eq!(trivial.len(), 1);
    assert!(trivial[0].detail.contains("single-valued domain flag"));
    assert_eq!(
        trivial[0]
            .domains
            .get("cairn.test.requirement_consistency.trivial#flag"),
        Some(&vec!["only".to_string()])
    );
    assert_eq!(
        trivial[0].fragments[0].location.as_str(),
        "guarantee[0]",
        "the finding names the exact formula fragment"
    );

    let indistinguishable = findings(&report, FindingKind::IndistinguishableStates);
    assert_eq!(indistinguishable.len(), 1);
    assert!(indistinguishable[0]
        .detail
        .contains("cannot distinguish states that differ only in flag"));
    assert!(indistinguishable[0]
        .domains
        .contains_key("cairn.test.requirement_consistency.trivial#flag"));
}

#[test]
// r[verify cairn.requirement_consistency.statuses]
fn exhausted_bounds_and_disabled_queries_never_conclude_consistent() {
    let exhausted_policy = policy(
        all_queries(),
        ConsistencyBounds {
            max_states: 1,
            ..bounds()
        },
    );
    let case = case(
        vec![contradictory_contract()],
        vec!["cairn.test.requirement_consistency.contradiction"],
    );
    let report = analyze_consistency(&exhausted_policy, &case).expect("analysis is admitted");
    let assumptions = &query(&report, AnalysisQueryKind::AssumptionsSatisfiable)[0];
    assert_eq!(
        assumptions.status,
        AnalysisStatus::Undetermined,
        "exhausted bounds are not a verdict"
    );
    assert_eq!(assumptions.basis.kind, SearchBasisKind::Exhausted);
    assert_eq!(assumptions.basis.states_explored, 1);
    assert!(assumptions.witness.is_none());
    assert!(!findings(&report, FindingKind::IncompleteAnalysis).is_empty());

    let disabled_policy = policy(vec![AnalysisQueryKind::AssumptionsSatisfiable], bounds());
    let disabled = analyze_consistency(&disabled_policy, &case).expect("analysis is admitted");
    for outcome in &disabled.queries {
        assert_ne!(
            outcome.status,
            AnalysisStatus::Consistent,
            "an analysis that did not run cannot conclude consistency"
        );
        if outcome.query.kind() != AnalysisQueryKind::AssumptionsSatisfiable {
            assert_eq!(outcome.status, AnalysisStatus::Unsupported);
            assert_eq!(outcome.basis.kind, SearchBasisKind::Unsupported);
            assert!(outcome.witness.is_none());
        }
    }
    assert_eq!(
        query(&disabled, AnalysisQueryKind::AssumptionsSatisfiable)[0].status,
        AnalysisStatus::Inconsistent,
        "the enabled query still decides the same exact case"
    );

    let admitted =
        analyze_consistency(&policy(all_queries(), bounds()), &case).expect("analysis is admitted");
    assert_eq!(
        query(&admitted, AnalysisQueryKind::AssumptionsSatisfiable)[0].status,
        AnalysisStatus::Inconsistent,
        "the same case is inconsistent under a complete bound"
    );
}

#[test]
// r[verify cairn.requirement_consistency.policy]
fn policy_and_case_admission_reject_unsupported_input() {
    let case = case(
        vec![conjunction_contract()],
        vec!["cairn.test.requirement_consistency.conjunction"],
    );
    let unsupported = ConsistencyPolicy {
        schema: "cairn.requirement-consistency-policy.v2".into(),
        ..policy(all_queries(), bounds())
    };
    assert!(matches!(
        analyze_consistency(&unsupported, &case),
        Err(ConsistencyError::Unsupported(_))
    ));
    let empty = policy(Vec::new(), bounds());
    assert!(matches!(
        analyze_consistency(&empty, &case),
        Err(ConsistencyError::Unsupported(_))
    ));
    let duplicated = policy(
        vec![
            AnalysisQueryKind::AssumptionsSatisfiable,
            AnalysisQueryKind::AssumptionsSatisfiable,
        ],
        bounds(),
    );
    assert!(matches!(
        analyze_consistency(&duplicated, &case),
        Err(ConsistencyError::Unsupported(_))
    ));
    let beyond = policy(
        all_queries(),
        ConsistencyBounds {
            max_states: MAX_STATES_CEILING + 1,
            ..bounds()
        },
    );
    assert!(matches!(
        analyze_consistency(&beyond, &case),
        Err(ConsistencyError::BoundExhaustion(_))
    ));
    let mut wrong_case_schema = case.clone();
    wrong_case_schema.schema = "cairn.requirement-consistency-case.v2".into();
    assert!(matches!(
        analyze_consistency(&policy(all_queries(), bounds()), &wrong_case_schema),
        Err(ConsistencyError::Unsupported(_))
    ));
    let mut missing_selection = case.clone();
    missing_selection.selected = vec!["cairn.test.requirement_consistency.absent".into()];
    assert!(matches!(
        analyze_consistency(&policy(all_queries(), bounds()), &missing_selection),
        Err(ConsistencyError::ScopeMismatch(_))
    ));
    let mut empty_selection = case.clone();
    empty_selection.selected = Vec::new();
    assert!(matches!(
        analyze_consistency(&policy(all_queries(), bounds()), &empty_selection),
        Err(ConsistencyError::BoundExhaustion(_))
    ));
    let mut bad_subject = case.clone();
    bad_subject.subject.subject_blake3 = "not-a-digest".into();
    assert!(matches!(
        analyze_consistency(&policy(all_queries(), bounds()), &bad_subject),
        Err(ConsistencyError::InvalidInput(_))
    ));
    let mut stale = case.clone();
    stale.contracts[0].native_requirement = row("cairn.test.requirement_consistency.other");
    assert!(matches!(
        analyze_consistency(&policy(all_queries(), bounds()), &stale),
        Err(ConsistencyError::ScopeMismatch(_))
    ));
}

#[test]
// r[verify cairn.requirement_consistency.replay]
fn tampered_witnesses_are_rejected() {
    let policy = policy(all_queries(), bounds());
    let case = case(
        vec![conjunction_contract()],
        vec!["cairn.test.requirement_consistency.conjunction"],
    );
    let report = analyze_consistency(&policy, &case).expect("analysis is admitted");
    let witness = query(&report, AnalysisQueryKind::GuaranteesCompatible)[0]
        .witness
        .clone()
        .expect("compatible witness");

    let mut altered_value = witness.clone();
    let present = altered_value.tables[0]
        .observables
        .get_mut("present")
        .expect("present table");
    present[0].value.value = "false".into();
    let replay = replay_witness(&policy, &case, &altered_value).expect("replay admission");
    assert!(!replay.replays);
    assert!(replay.detail.contains("does not satisfy the named query"));

    let mut altered_contract = witness.clone();
    altered_contract.tables[0].contract_blake3 = HASH_A.into();
    let replay = replay_witness(&policy, &case, &altered_contract).expect("replay admission");
    assert!(!replay.replays);
    assert!(replay.detail.contains("different contract revision"));

    let mut altered_identity = witness.clone();
    altered_identity.model_blake3 = HASH_B.into();
    let replay = replay_witness(&policy, &case, &altered_identity).expect("replay admission");
    assert!(!replay.replays);
    assert!(replay.detail.contains("model identity"));

    let mut altered_query = witness.clone();
    altered_query.query = "activation_reachable:cairn.unknown".into();
    let replay = replay_witness(&policy, &case, &altered_query).expect("replay admission");
    assert!(!replay.replays);
    assert!(replay.detail.contains("outside this exact case scope"));
}

const PILOT_INVENTORY: &str =
    include_str!("../../../../../cairn-policy/generated/typed-requirement-contracts.json");

#[test]
// r[verify cairn.requirement_consistency.queries]
fn pilot_contracts_produce_consistent_models_and_guarantee_findings() {
    let inventory = parse_typed_requirement_inventory_value(
        &serde_json::from_str(PILOT_INVENTORY).expect("packaged inventory parses"),
    )
    .expect("packaged inventory is admitted");
    let selected: Vec<String> = inventory
        .contracts
        .iter()
        .map(|contract| contract.requirement.requirement_id.clone())
        .collect();
    let case = ConsistencyCase {
        schema: CONSISTENCY_CASE_SCHEMA_V1.into(),
        profile: inventory.profile.clone(),
        subject: AnalysisSubject {
            subject_blake3: HASH_C.into(),
            policy_blake3: HASH_B.into(),
            target: "onixresearch/cairn".into(),
            environment: "linux-x86_64".into(),
        },
        contracts: inventory
            .contracts
            .into_iter()
            .map(|contract| CaseContract {
                native_requirement: contract.requirement.clone(),
                contract,
                correspondence_review: None,
            })
            .collect(),
        selected: selected.clone(),
        correspondence: Vec::new(),
        authorized_reviewers: Vec::new(),
        native_obligations: Vec::new(),
        producer: ProducerIdentity {
            producer: "cairn-core".into(),
            adapter: "finite-enumeration".into(),
            version: "1".into(),
        },
    };
    let policy = policy(
        all_queries(),
        ConsistencyBounds {
            max_contracts: 8,
            max_guarantees: 16,
            max_cells: 4_096,
            max_states: 500_000,
            max_evaluations: 4_000_000,
            max_depth: 64,
        },
    );
    let report = analyze_consistency(&policy, &case).expect("pilot analysis is admitted");
    assert_eq!(report.scope.selected.len(), 5);
    assert_eq!(report.scope.guarantees.len(), 13);
    assert_eq!(
        report.scope.contracts.len(),
        5,
        "each pilot contract keeps its exact revision binding"
    );
    for contract in &report.scope.contracts {
        assert!(
            selected.contains(&contract.requirement_id),
            "scope names an exact selected requirement"
        );
        assert_eq!(contract.correspondence, "not_reviewed");
        assert!(!contract.requirement_row_blake3.is_empty());
    }
    assert_eq!(
        query(&report, AnalysisQueryKind::AssumptionsSatisfiable)[0].status,
        AnalysisStatus::Consistent
    );
    for outcome in query(&report, AnalysisQueryKind::ActivationReachable) {
        assert_eq!(outcome.status, AnalysisStatus::Consistent);
    }
    let compatible = query(&report, AnalysisQueryKind::GuaranteesCompatible);
    assert_eq!(compatible.len(), 1);
    assert_eq!(compatible[0].status, AnalysisStatus::Consistent);
    let replay = replay_witness(
        &policy,
        &case,
        compatible[0].witness.as_ref().expect("pilot witness"),
    )
    .expect("pilot witness replay admission");
    assert!(replay.replays, "{}", replay.detail);
    assert_eq!(replay.contract_evaluations.len(), 5);
    for outcome in query(&report, AnalysisQueryKind::GuaranteeConstraining) {
        assert!(
            matches!(
                outcome.status,
                AnalysisStatus::Consistent
                    | AnalysisStatus::Inconsistent
                    | AnalysisStatus::Undetermined
            ),
            "withholding status is explicit: {:?}",
            outcome.status
        );
        if outcome.status == AnalysisStatus::Consistent {
            assert_eq!(outcome.basis.kind, SearchBasisKind::Witness);
            assert!(outcome.witness.is_some());
        }
    }
    assert!(
        !findings(&report, FindingKind::ConstrainingGuarantee).is_empty(),
        "the pilot corpus contains meaningful guarantees"
    );
    assert!(
        findings(&report, FindingKind::TrivialQuantification).is_empty(),
        "no selected pilot domain is single-valued"
    );
    assert!(
        findings(&report, FindingKind::IndistinguishableStates).is_empty(),
        "every quantified pilot domain is accepted by an observable argument"
    );
    assert!(
        findings(&report, FindingKind::ConflictingGuarantees).is_empty(),
        "the selected pilot guarantees are jointly satisfiable"
    );
    for report_finding in &report.findings {
        assert_eq!(report_finding.disposition, UNRESOLVED_DISPOSITION);
        for fragment in &report_finding.fragments {
            assert!(
                selected.contains(&fragment.requirement_id),
                "fragments name the exact requirement"
            );
            assert_eq!(fragment.fragment_blake3.len(), 64);
        }
    }
}

fn verdict_contract(requirement_id: &str, identities: &[&str], verdict: &str) -> SemanticContract {
    contract(ContractSpec {
        requirement_id,
        domains: BTreeMap::from([
            ("bool".to_string(), bool_domain()),
            ("identity".to_string(), domain(identities)),
        ]),
        observables: BTreeMap::from([("verdict".to_string(), mapping(&[], "identity"))]),
        assumptions: Vec::new(),
        activation: FiniteExpr::Bool { value: true },
        guarantee: equal(observation("verdict", vec![]), literal("identity", verdict)),
    })
}

fn correspondence(members: &[(&str, &str)]) -> ObservablesCorrespondence {
    ObservablesCorrespondence {
        identity: "shared.verdict".into(),
        members: members
            .iter()
            .map(|(requirement_id, mapping)| ObservableMember {
                requirement_id: (*requirement_id).into(),
                mapping: (*mapping).into(),
            })
            .collect(),
    }
}

#[test]
// r[verify cairn.requirement_consistency.queries]
fn contradictory_guarantees_conflict_only_when_correspondence_is_declared() {
    let first = verdict_contract("cairn.test.requirement_consistency.first", &["a", "b"], "a");
    let second = verdict_contract(
        "cairn.test.requirement_consistency.second",
        &["a", "b"],
        "b",
    );
    let policy = policy(all_queries(), bounds());

    let independent = case(
        vec![first.clone(), second.clone()],
        vec![
            "cairn.test.requirement_consistency.first",
            "cairn.test.requirement_consistency.second",
        ],
    );
    let report = analyze_consistency(&policy, &independent).expect("analysis is admitted");
    assert_eq!(
        query(&report, AnalysisQueryKind::GuaranteesCompatible)[0].status,
        AnalysisStatus::Consistent,
        "without a declared correspondence the observations are independent"
    );
    assert!(findings(&report, FindingKind::ConflictingGuarantees).is_empty());

    let mut corresponded = independent.clone();
    corresponded.correspondence = vec![correspondence(&[
        ("cairn.test.requirement_consistency.first", "verdict"),
        ("cairn.test.requirement_consistency.second", "verdict"),
    ])];
    let report = analyze_consistency(&policy, &corresponded).expect("analysis is admitted");
    assert_eq!(
        report.scope.correspondence.len(),
        1,
        "the declared correspondence is part of the scope identity"
    );
    assert_eq!(
        report.scope.contracts[0].observable_states["verdict"],
        "shared.verdict"
    );
    let compatible = &query(&report, AnalysisQueryKind::GuaranteesCompatible)[0];
    assert_eq!(compatible.status, AnalysisStatus::Inconsistent);
    assert!(compatible.witness.is_none());
    let conflicting = findings(&report, FindingKind::ConflictingGuarantees);
    assert_eq!(conflicting.len(), 1);
    assert_eq!(conflicting[0].status, Some(AnalysisStatus::Inconsistent));
    let conflict = conflicting[0]
        .conflicting
        .first()
        .expect("the conflicting subset is retained");
    assert!(conflict.minimal);
    assert_eq!(conflict.members.len(), 2);
    assert!(conflict.members[0].ends_with("::guarantee[0]"));
    assert!(conflicting[0].conflicting_exploration_complete);

    let mut mismatched = independent.clone();
    mismatched.contracts[1].contract = verdict_contract(
        "cairn.test.requirement_consistency.second",
        &["a", "b", "c"],
        "b",
    );
    mismatched.contracts[1].native_requirement =
        mismatched.contracts[1].contract.requirement.clone();
    mismatched.correspondence = vec![correspondence(&[
        ("cairn.test.requirement_consistency.first", "verdict"),
        ("cairn.test.requirement_consistency.second", "verdict"),
    ])];
    assert!(matches!(
        analyze_consistency(&policy, &mismatched),
        Err(ConsistencyError::InvalidInput(detail)) if detail.contains("different declared domains")
    ));
}
