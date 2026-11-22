use super::*;

use super::super::agent_receipt::{
    authoritative_command_hash, authoritative_selection_hash, AgentRequiredCheck,
};
use super::super::ast_grep_gate::ast_grep_receipt_hash;
use super::super::scoped_claim_derivations::{
    claim_identity, ClaimContext, ClaimPolicy, EvaluationRequest, ProducerInput, Proposition,
    ScopedClaim, CLAIM_POLICY_SCHEMA_V1, CLAIM_SCHEMA_V1,
};
use super::super::typed_requirement_contracts::{
    normalize_contract, FiniteDomain, FiniteExpr, SemanticContract, SemanticProfile,
};

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const METHOD: &str = "finite-enumeration-exhaustive.v1";
const REPOSITORY: &str = "onixresearch/cairn";

fn hash(index: u64) -> String {
    format!("{index:064x}")
}

fn native_row(index: u64) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: format!("cairn.test.refinement.{index}"),
        specification_path: ".cairn/specs/refinement/spec.md".into(),
        specification_blake3: HASH_A.into(),
        source_span: super::super::content_bound_requirement_registry::ContentBoundSourceSpan {
            start_byte: 0,
            end_byte: 64,
            start_line: 1,
            end_line: 1,
        },
        requirement_body_blake3: hash(index + 10_000),
        row_blake3: hash(index + 20_000),
    }
}

fn revision(index: u64) -> RequirementRevision {
    let requirement = native_row(index);
    let contract = semantic_contract(&requirement);
    let normalized = normalize_contract(&profile(), &requirement, &contract)
        .expect("refinement fixture contract normalizes");
    RequirementRevision {
        requirement,
        interpretation_blake3: normalized.interpretation_blake3,
        contract_blake3: normalized.contract_blake3,
    }
}

fn scope() -> ArgumentScope {
    ArgumentScope {
        subject_blake3: HASH_C.into(),
        policy_blake3: HASH_B.into(),
        target: REPOSITORY.into(),
        environment: "linux-x86_64".into(),
        bounds: BTreeMap::from([("max_checks".to_string(), "2".to_string())]),
    }
}

fn claim_context() -> ClaimContext {
    ClaimContext {
        subject_blake3: HASH_C.into(),
        policy_blake3: HASH_B.into(),
        target: REPOSITORY.into(),
        environment: "linux-x86_64".into(),
        bounds: BTreeMap::from([("max_checks".to_string(), "2".to_string())]),
    }
}

fn profile() -> SemanticProfile {
    SemanticProfile {
        id: "cairn.requirement-semantics.finite.v1".into(),
        interpretation:
            "finite-enumeration.equality.nominal-domains.total-observables.single-step.v1".into(),
        max_nodes: 32,
        max_depth: 4,
        max_domain_values: 4,
        max_steps: 256,
    }
}

fn semantic_contract(row: &ContentBoundRequirementRow) -> SemanticContract {
    SemanticContract {
        profile: profile().id,
        proposed_by: "refinement-test-author".into(),
        requirement: row.clone(),
        domains: BTreeMap::from([(
            "bool".to_string(),
            FiniteDomain {
                values: vec!["false".into(), "true".into()],
            },
        )]),
        observables: BTreeMap::new(),
        assumptions: Vec::new(),
        activation: FiniteExpr::Bool { value: true },
        guarantee: FiniteExpr::Bool { value: true },
        supported_portion: "refinement guarantee fixture".into(),
        informal_remainder: "not behavioral satisfaction".into(),
    }
}

fn claim_policy() -> ClaimPolicy {
    serde_json::from_value(serde_json::json!({
        "schema": CLAIM_POLICY_SCHEMA_V1,
        "claims": {
            "schema": CLAIM_SCHEMA_V1,
            "kinds": [
                "observation",
                "structural_relationship",
                "bounded_property",
                "checker_accepted_property",
                "reviewed_requirement_correspondence"
            ],
            "propositions": [
                "check_passed",
                "receipt_structurally_valid",
                "native_completion_eligible",
                "reviewed_correspondence",
                "reviewed_completion_decision"
            ]
        },
        "rules": [
            {
                "id": "reviewed_completion_v1",
                "version": 1,
                "assurance_scope": "reviewed completion argument only"
            }
        ],
        "bounds": {
            "max_claims": 8,
            "max_derivations": 8,
            "max_producers": 4,
            "max_premises": 4,
            "max_depth": 4
        }
    }))
    .unwrap()
}

fn structural_receipt() -> serde_json::Value {
    serde_json::json!({
        "tool": {"name": "ast-grep", "version": "0.39.0"},
        "rule_bundle_hash": HASH_A,
        "repository_scope": ".",
        "command": "ast-grep scan --json",
        "output_format": "json",
        "findings_summary": {"total": 0, "errors": 0, "warnings": 0, "infos": 0, "codemods": 0},
        "non_claims": [
            "does not prove source correctness",
            "does not prove requirement satisfaction",
            "does not prove proof discharge",
            "does not prove release eligibility"
        ],
        "findings": []
    })
}

fn structural_case(case_id: &str, revision: &RequirementRevision) -> (GuaranteeCase, String) {
    let receipt = structural_receipt();
    let receipt_blake3 = ast_grep_receipt_hash(&receipt);
    let proposition = Proposition::ReceiptStructurallyValid { receipt_blake3 };
    let context = claim_context();
    let claim = ScopedClaim {
        schema: CLAIM_SCHEMA_V1.into(),
        kind: proposition.claim_kind(),
        requirement: revision.requirement.clone(),
        interpretation_blake3: revision.interpretation_blake3.clone(),
        contract_blake3: revision.contract_blake3.clone(),
        context: context.clone(),
        proposition,
    };
    let claim_id = claim_identity(&claim).unwrap();
    (
        GuaranteeCase {
            id: case_id.into(),
            policy: claim_policy(),
            case: EvaluationRequest {
                profile: profile(),
                native_requirement: revision.requirement.clone(),
                semantic_contract: semantic_contract(&revision.requirement),
                correspondence_review: None,
                authorized_reviewers: Vec::new(),
                claims: vec![claim],
                producers: vec![ProducerInput::StructuralAstGrep {
                    context,
                    repository_scope: ".".into(),
                    receipt,
                    trust_dependencies: BTreeSet::from([
                        "structural-rule-bundle:caller-retained".to_string()
                    ]),
                }],
                derivations: Vec::new(),
            },
        },
        claim_id,
    )
}

fn completion_values(clippy_status: &str) -> (serde_json::Value, serde_json::Value) {
    let tests_command = vec!["cargo".to_string(), "test".to_string()];
    let clippy_command = vec!["cargo".to_string(), "clippy".to_string()];
    let mut checks = vec![
        AgentRequiredCheck {
            check_id: "tests".into(),
            command: tests_command.clone(),
            command_hash: authoritative_command_hash(&tests_command),
            prerequisites: Vec::new(),
        },
        AgentRequiredCheck {
            check_id: "clippy".into(),
            command: clippy_command.clone(),
            command_hash: authoritative_command_hash(&clippy_command),
            prerequisites: vec!["tests".into()],
        },
    ];
    checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    let selection_hash = authoritative_selection_hash(&checks);
    let contract = serde_json::json!({
        "schema": "cairn.agent-completion.v2",
        "repository_id": REPOSITORY,
        "registry_id": "cairn-release-checks",
        "registry_hash": HASH_A,
        "selection_hash": selection_hash.clone(),
        "policy_hash": HASH_B,
        "final_subject_hash": HASH_C,
        "required_checks": checks.clone(),
    });
    let results = serde_json::json!([
        {
            "check_id": "tests", "status": "passed",
            "command": tests_command, "command_hash": checks[1].command_hash,
            "subject_hash": HASH_C, "registry_hash": HASH_A,
            "selection_hash": selection_hash.clone(), "policy_hash": HASH_B,
            "evidence_hash": HASH_A, "execution_receipt_hash": HASH_B
        },
        {
            "check_id": "clippy", "status": clippy_status,
            "command": clippy_command, "command_hash": checks[0].command_hash,
            "subject_hash": HASH_C, "registry_hash": HASH_A,
            "selection_hash": selection_hash.clone(), "policy_hash": HASH_B,
            "evidence_hash": HASH_A, "execution_receipt_hash": HASH_B
        }
    ]);
    (contract, results)
}

fn unresolved_case(case_id: &str, revision: &RequirementRevision) -> (GuaranteeCase, String) {
    let context = claim_context();
    let proposition = Proposition::ReceiptStructurallyValid {
        receipt_blake3: HASH_D.into(),
    };
    let claim = ScopedClaim {
        schema: CLAIM_SCHEMA_V1.into(),
        kind: proposition.claim_kind(),
        requirement: revision.requirement.clone(),
        interpretation_blake3: revision.interpretation_blake3.clone(),
        contract_blake3: revision.contract_blake3.clone(),
        context,
        proposition,
    };
    let claim_id = claim_identity(&claim).unwrap();
    (
        GuaranteeCase {
            id: case_id.into(),
            policy: claim_policy(),
            case: EvaluationRequest {
                profile: profile(),
                native_requirement: revision.requirement.clone(),
                semantic_contract: semantic_contract(&revision.requirement),
                correspondence_review: None,
                authorized_reviewers: Vec::new(),
                claims: vec![claim],
                producers: Vec::new(),
                derivations: Vec::new(),
            },
        },
        claim_id,
    )
}

fn failed_check_case(case_id: &str, revision: &RequirementRevision) -> (GuaranteeCase, String) {
    let (contract, results) = completion_values("failed");
    let context = claim_context();
    let proposition = Proposition::CheckPassed {
        check_id: "clippy".into(),
    };
    let claim = ScopedClaim {
        schema: CLAIM_SCHEMA_V1.into(),
        kind: proposition.claim_kind(),
        requirement: revision.requirement.clone(),
        interpretation_blake3: revision.interpretation_blake3.clone(),
        contract_blake3: revision.contract_blake3.clone(),
        context: context.clone(),
        proposition,
    };
    let claim_id = claim_identity(&claim).unwrap();
    (
        GuaranteeCase {
            id: case_id.into(),
            policy: claim_policy(),
            case: EvaluationRequest {
                profile: profile(),
                native_requirement: revision.requirement.clone(),
                semantic_contract: semantic_contract(&revision.requirement),
                correspondence_review: None,
                authorized_reviewers: Vec::new(),
                claims: vec![claim],
                producers: vec![ProducerInput::AuthoritativeCompletion {
                    context,
                    contract,
                    results,
                    trust_dependencies: BTreeSet::from([
                        "native-test-oracle:shared-cargo-test-harness".to_string(),
                    ]),
                }],
                derivations: Vec::new(),
            },
        },
        claim_id,
    )
}

fn child(
    id: &str,
    revision: &RequirementRevision,
    case_id: &str,
    claim_id: &str,
) -> ChildGuarantee {
    ChildGuarantee {
        id: id.into(),
        requirement: revision.clone(),
        scope: scope(),
        guarantee_case: case_id.into(),
        claim_id: claim_id.into(),
    }
}

fn reviewed_relation() -> ImplicationRelation {
    ImplicationRelation {
        assurance: Assurance::Reviewed,
        witness_blake3: HASH_D.into(),
        statement: "reviewed decomposition argument".into(),
        proof_checker: None,
    }
}

fn feasibility(covered: BTreeSet<String>) -> FeasibilityDisposition {
    FeasibilityDisposition {
        disposition: Feasibility::Feasible,
        method: METHOD.into(),
        assurance: Assurance::Reviewed,
        justification_blake3: HASH_D.into(),
        covered,
        proof_checker: None,
    }
}

fn decomposition(
    id: &str,
    parent: RequirementRevision,
    children: Vec<ChildGuarantee>,
    composition: Composition,
) -> Decomposition {
    let covered: BTreeSet<String> = children.iter().map(|child| child.id.clone()).collect();
    Decomposition {
        schema: REFINEMENT_SCHEMA_V1.into(),
        id: id.into(),
        parent,
        scope: scope(),
        children,
        environment: Vec::new(),
        integration: Vec::new(),
        composition,
        relation: reviewed_relation(),
        feasibility: feasibility(covered),
    }
}

fn root(id: &str, statement: &str) -> RootAssumption {
    RootAssumption {
        id: id.into(),
        statement: statement.into(),
    }
}

fn policy(roots: Vec<RootAssumption>, proof_checkers: Vec<String>) -> RefinementPolicy {
    RefinementPolicy {
        schema: REFINEMENT_POLICY_SCHEMA_V1.into(),
        relations: vec![
            DependencyRelation::OrderingArchive,
            DependencyRelation::VerifiedSpecification,
            DependencyRelation::VerifiedCandidate,
            DependencyRelation::CandidateInput,
            DependencyRelation::IntegratedRevision,
            DependencyRelation::Lifecycle,
            DependencyRelation::Semantic,
        ],
        assurances: vec![Assurance::Reviewed, Assurance::BoundedCheck],
        feasibility_methods: vec![METHOD.into()],
        root_assumptions: roots,
        proof_checkers,
        bounds: RefinementBounds {
            max_decompositions: 16,
            max_requirements: 64,
            max_assumptions: 32,
            max_obligations: 64,
            max_alternatives: 16,
            max_dependencies: 32,
            max_evidence: 64,
            max_guarantee_cases: 32,
            max_depth: 8,
        },
    }
}

fn request(
    decompositions: Vec<Decomposition>,
    guarantee_cases: Vec<GuaranteeCase>,
    dependencies: Vec<TypedDependency>,
    dependency_evidence: Vec<DependencyEvidence>,
    integration_evidence: Vec<IntegrationEvidence>,
    applicability: Vec<ApplicabilityObservation>,
) -> RefinementRequest {
    RefinementRequest {
        decompositions,
        guarantee_cases,
        dependencies,
        dependency_evidence,
        integration_evidence,
        applicability,
        previous_basis: None,
    }
}

fn dependency_baseline(
    id: &str,
    relation: DependencyRelation,
    producer: RequirementRevision,
    consumer: RequirementRevision,
) -> TypedDependency {
    TypedDependency {
        schema: DEPENDENCY_SCHEMA_V1.into(),
        id: id.into(),
        relation,
        producer: DependencyEndpoint {
            requirement: producer,
            work: "producer-change".into(),
            revision: "revision-a".into(),
        },
        consumer: DependencyEndpoint {
            requirement: consumer,
            work: "consumer-change".into(),
            revision: "revision-b".into(),
        },
        context: DependencyContext {
            producer_repository: "onixresearch/kiln".into(),
            consumer_repository: REPOSITORY.into(),
            intended_target: "kiln-main".into(),
            environment: "linux-x86_64".into(),
            policy_blake3: HASH_B.into(),
            bounds: BTreeMap::from([("max_tasks".to_string(), "4".to_string())]),
        },
        coverage: Coverage::WholeChange,
        required_gates: Vec::new(),
        required_input_blake3: None,
        required_lifecycle_stage: None,
        semantic: None,
    }
}

fn archive_evidence(id: &str, coverage: EvidenceCoverage) -> DependencyEvidence {
    DependencyEvidence::Archive {
        dependency_id: id.into(),
        change: "producer-change".into(),
        repository: "onixresearch/kiln".into(),
        revision: "revision-a".into(),
        archive_receipt_blake3: HASH_D.into(),
        coverage,
    }
}

fn composition_child(id: &str) -> Composition {
    Composition::Child { child: id.into() }
}

fn explanation<'a>(report: &'a RefinementReport, id: &str) -> &'a DecompositionExplanation {
    report
        .decompositions
        .iter()
        .find(|explanation| explanation.id == id)
        .expect("decomposition explanation exists")
}

fn dependency<'a>(report: &'a RefinementReport, id: &str) -> &'a DependencyExplanation {
    report
        .dependencies
        .iter()
        .find(|explanation| explanation.id == id)
        .expect("dependency explanation exists")
}

fn supported_pilot() -> (RefinementPolicy, RefinementRequest) {
    let parent = revision(1);
    let durable = revision(2);
    let idempotent = revision(3);
    let recovery = revision(4);

    let (case1, claim1) = structural_case("guarantee.durable", &durable);
    let (case2, claim2) = structural_case("guarantee.idempotent", &idempotent);
    let (case3, claim3) = structural_case("guarantee.recovery", &recovery);

    let mut pilot = decomposition(
        "decomposition.native",
        parent,
        vec![
            child("child.durable", &durable, "guarantee.durable", &claim1),
            child(
                "child.idempotent",
                &idempotent,
                "guarantee.idempotent",
                &claim2,
            ),
            child("child.recovery", &recovery, "guarantee.recovery", &claim3),
        ],
        Composition::AllOf {
            obligations: vec![
                composition_child("child.durable"),
                composition_child("child.idempotent"),
                composition_child("child.recovery"),
            ],
        },
    );
    let statement = "caller retains the reviewed integration harness";
    pilot.environment = vec![EnvironmentAssumption {
        id: "assumption.harness".into(),
        statement: statement.into(),
        atom: AssumptionAtom {
            predicate: "harness-retained".into(),
            asserted: true,
        },
        source: "cairn.test.refinement.1".into(),
    }];
    pilot.integration = vec![IntegrationObligation {
        id: "integration.durable-idempotent-recovery".into(),
        statement: "durable intent, idempotent execution and recovery integrate".into(),
        required: true,
        scope: scope(),
    }];
    pilot
        .feasibility
        .covered
        .insert("assumption.harness".into());

    let integration_evidence = vec![IntegrationEvidence {
        obligation_id: "integration.durable-idempotent-recovery".into(),
        statement: "durable intent, idempotent execution and recovery integrate".into(),
        scope: scope(),
        observation_blake3: HASH_D.into(),
        source: "retained:kiln-integration-observation".into(),
    }];
    (
        policy(vec![root("assumption.harness", statement)], Vec::new()),
        request(
            vec![pilot],
            vec![case1, case2, case3],
            Vec::new(),
            Vec::new(),
            integration_evidence,
            Vec::new(),
        ),
    )
}

#[test]
// r[verify cairn.refinement.decomposition]
// r[verify cairn.refinement.assurance]
fn supported_composition_names_every_premise_and_replays() {
    let (policy, request) = supported_pilot();
    let report = evaluate_refinement(&policy, &request).unwrap();
    assert_eq!(report.schema, REFINEMENT_REPORT_SCHEMA_V1);
    assert!(report.advisory);
    assert_eq!(report.boundary, REFINEMENT_BOUNDARY);
    let pilot = explanation(&report, "decomposition.native");
    assert_eq!(pilot.state, ObligationState::Supported);
    assert_eq!(pilot.assurance, Assurance::Reviewed);
    assert!(pilot.conditional);
    assert_eq!(pilot.obligations.len(), 5);
    assert!(pilot
        .obligations
        .iter()
        .all(|obligation| obligation.state == ObligationState::Supported));
    assert_eq!(pilot.feasibility.state, ObligationState::Supported);
    let residual = report
        .residual_assumptions
        .iter()
        .find(|assumption| assumption.id == "assumption.harness")
        .unwrap();
    assert_eq!(residual.status, AssumptionStatus::PolicyAccepted);
    assert_eq!(residual.source, "cairn.test.refinement.1");

    let replay = evaluate_refinement(&policy, &request).unwrap();
    assert_eq!(report.report_blake3, replay.report_blake3);
    assert_eq!(report.input_blake3, replay.input_blake3);
}

#[test]
// r[verify cairn.refinement.feasibility]
fn parent_stays_pending_until_named_integration_obligation_is_established() {
    let (policy, mut request) = supported_pilot();
    request.integration_evidence.clear();
    let report = evaluate_refinement(&policy, &request).unwrap();
    let pilot = explanation(&report, "decomposition.native");
    assert_eq!(pilot.state, ObligationState::Pending);
    assert!(pilot.reason.contains("integration_obligation_pending"));
    let obligation = pilot
        .obligations
        .iter()
        .find(|obligation| obligation.id == "integration.durable-idempotent-recovery")
        .unwrap();
    assert_eq!(obligation.state, ObligationState::Pending);

    request.integration_evidence.push(IntegrationEvidence {
        obligation_id: "integration.durable-idempotent-recovery".into(),
        statement: "durable intent, idempotent execution and recovery integrate".into(),
        scope: scope(),
        observation_blake3: HASH_D.into(),
        source: "retained:kiln-integration-observation".into(),
    });
    let report = evaluate_refinement(&policy, &request).unwrap();
    assert_eq!(
        explanation(&report, "decomposition.native").state,
        ObligationState::Supported
    );
}

#[test]
// r[verify cairn.refinement.assurance]
fn completing_children_does_not_establish_the_argument() {
    let parent = revision(200);
    let child_revision = revision(201);
    let (case, claim) = unresolved_case("guarantee.unresolved", &child_revision);
    let pilot = decomposition(
        "decomposition.unresolved",
        parent,
        vec![child(
            "child.unresolved",
            &child_revision,
            "guarantee.unresolved",
            &claim,
        )],
        Composition::AllOf {
            obligations: vec![composition_child("child.unresolved")],
        },
    );
    let request = self::request(
        vec![pilot],
        vec![case],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let report = evaluate_refinement(&policy(Vec::new(), Vec::new()), &request).unwrap();
    let pilot = explanation(&report, "decomposition.unresolved");
    assert_eq!(pilot.state, ObligationState::Pending);
    assert!(pilot
        .obligations
        .iter()
        .filter(|obligation| obligation.kind == ObligationKind::Child)
        .all(|obligation| obligation.state == ObligationState::Pending));

    let (policy, mut request) = supported_pilot();
    request.decompositions[0].relation.assurance = Assurance::Unassessed;
    let report = evaluate_refinement(&policy, &request).unwrap();
    let pilot = explanation(&report, "decomposition.native");
    assert_eq!(pilot.state, ObligationState::Unsupported);
    assert!(pilot.reason.contains("relation_unassessed"));
}

#[test]
// r[verify cairn.refinement.decomposition]
fn any_of_uses_only_one_complete_selected_alternative() {
    let parent = revision(20);
    let a1 = revision(21);
    let a2 = revision(22);
    let b1 = revision(23);
    let (case_a1, claim_a1) = structural_case("guarantee.a1", &a1);
    let (case_a2, claim_a2) = unresolved_case("guarantee.a2", &a2);
    let (case_b1, claim_b1) = structural_case("guarantee.b1", &b1);
    let mut pilot = decomposition(
        "decomposition.alternatives",
        parent,
        vec![
            child("child.a1", &a1, "guarantee.a1", &claim_a1),
            child("child.a2", &a2, "guarantee.a2", &claim_a2),
            child("child.b1", &b1, "guarantee.b1", &claim_b1),
        ],
        Composition::AnyOf {
            alternatives: vec![
                Alternative {
                    id: "strategy.a".into(),
                    obligations: vec![composition_child("child.a1"), composition_child("child.a2")],
                },
                Alternative {
                    id: "strategy.b".into(),
                    obligations: vec![composition_child("child.b1")],
                },
            ],
            selected: "strategy.a".into(),
        },
    );
    pilot.feasibility.covered =
        BTreeSet::from(["child.a1".into(), "child.a2".into(), "child.b1".into()]);
    let request = self::request(
        vec![pilot],
        vec![case_a1, case_a2, case_b1],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let policy = policy(Vec::new(), Vec::new());
    let report = evaluate_refinement(&policy, &request).unwrap();
    let selected = explanation(&report, "decomposition.alternatives");
    assert_eq!(selected.state, ObligationState::Pending);
    assert!(selected
        .obligations
        .iter()
        .all(|obligation| obligation.id != "child.b1"));
    assert_eq!(
        selected.selected_alternatives.get("composition"),
        Some(&"strategy.a".to_string())
    );

    let mut request = request;
    if let Composition::AnyOf { selected, .. } = &mut request.decompositions[0].composition {
        *selected = "strategy.b".into();
    }
    let report = evaluate_refinement(&policy, &request).unwrap();
    let selected = explanation(&report, "decomposition.alternatives");
    assert_eq!(selected.state, ObligationState::Supported);
    assert_eq!(
        selected.selected_alternatives.get("composition"),
        Some(&"strategy.b".to_string())
    );
}

#[test]
// r[verify cairn.refinement.feasibility]
fn contradictory_or_unresolved_assumptions_cannot_support_the_parent() {
    let parent = revision(30);
    let one = revision(31);
    let (case_one, claim_one) = structural_case("guarantee.one", &one);
    let mut pilot = decomposition(
        "decomposition.assumptions",
        parent,
        vec![child("child.one", &one, "guarantee.one", &claim_one)],
        Composition::AllOf {
            obligations: vec![composition_child("child.one")],
        },
    );
    pilot.environment = vec![
        EnvironmentAssumption {
            id: "assumption.durable".into(),
            statement: "durable intent holds".into(),
            atom: AssumptionAtom {
                predicate: "durable".into(),
                asserted: true,
            },
            source: "source.one".into(),
        },
        EnvironmentAssumption {
            id: "assumption.not-durable".into(),
            statement: "durable intent does not hold".into(),
            atom: AssumptionAtom {
                predicate: "durable".into(),
                asserted: false,
            },
            source: "source.two".into(),
        },
    ];
    pilot.feasibility.covered = BTreeSet::from([
        "child.one".into(),
        "assumption.durable".into(),
        "assumption.not-durable".into(),
    ]);
    let request = self::request(
        vec![pilot.clone()],
        vec![case_one.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let report = evaluate_refinement(&policy(Vec::new(), Vec::new()), &request).unwrap();
    let infeasible = explanation(&report, "decomposition.assumptions");
    assert_eq!(infeasible.feasibility.state, ObligationState::Infeasible);
    assert!(infeasible
        .feasibility
        .blockers
        .contains(&"contradictory_assumptions".to_string()));
    assert_ne!(infeasible.state, ObligationState::Supported);

    let mut unresolved = pilot;
    unresolved.environment = vec![EnvironmentAssumption {
        id: "assumption.unresolved".into(),
        statement: "unresolved environment premise".into(),
        atom: AssumptionAtom {
            predicate: "unresolved".into(),
            asserted: true,
        },
        source: "source.three".into(),
    }];
    unresolved.feasibility.covered =
        BTreeSet::from(["child.one".into(), "assumption.unresolved".into()]);
    let request = self::request(
        vec![unresolved],
        vec![case_one],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let report = evaluate_refinement(&policy(Vec::new(), Vec::new()), &request).unwrap();
    let pending = explanation(&report, "decomposition.assumptions");
    assert_eq!(pending.state, ObligationState::Pending);
    assert!(pending.reason.contains("assumption_unresolved"));
    let assumption = report
        .residual_assumptions
        .iter()
        .find(|assumption| assumption.id == "assumption.unresolved")
        .unwrap();
    assert_eq!(assumption.status, AssumptionStatus::Residual);
    assert_eq!(assumption.source, "source.three");
}

#[test]
// r[verify cairn.refinement.feasibility]
fn contradictory_child_guarantee_is_infeasible_not_vacuously_true() {
    let parent = revision(40);
    let failed = revision(41);
    let (case_failed, claim_failed) = failed_check_case("guarantee.failed", &failed);
    let pilot = decomposition(
        "decomposition.contradiction",
        parent,
        vec![child(
            "child.failed",
            &failed,
            "guarantee.failed",
            &claim_failed,
        )],
        Composition::AllOf {
            obligations: vec![composition_child("child.failed")],
        },
    );
    let request = self::request(
        vec![pilot],
        vec![case_failed],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let report = evaluate_refinement(&policy(Vec::new(), Vec::new()), &request).unwrap();
    let contradiction = explanation(&report, "decomposition.contradiction");
    assert_eq!(contradiction.feasibility.state, ObligationState::Infeasible);
    assert_eq!(contradiction.state, ObligationState::Contradicted);
    assert_ne!(contradiction.state, ObligationState::Supported);
}

#[test]
// r[verify cairn.refinement.dependencies]
fn relation_contract_names_meaning_and_evidence_for_every_relation() {
    let contract = dependency_relation_contract();
    let relations = contract["relations"].as_object().unwrap();
    assert_eq!(relations.len(), 7);
    for relation in [
        "ordering_archive",
        "verified_specification",
        "verified_candidate",
        "candidate_input",
        "integrated_revision",
        "lifecycle",
        "semantic",
    ] {
        assert!(relations.contains_key(relation));
        assert!(!relations[relation]["meaning"].as_str().unwrap().is_empty());
        assert!(!relations[relation]["evidence"].as_str().unwrap().is_empty());
        assert!(!relations[relation]["non_claims"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}

#[test]
// r[verify cairn.refinement.dependencies]
fn archive_ordering_does_not_discharge_integration_or_affect_independent_c() {
    let (producer_a, consumer_b) = (revision(50), revision(51));
    let (producer_c, consumer_d) = (revision(52), revision(53));
    let mut ordering = dependency_baseline(
        "dependency.a-to-b-ordering",
        DependencyRelation::OrderingArchive,
        producer_a,
        consumer_b,
    );
    ordering.context.producer_repository = "onixresearch/kiln".into();
    let integration = dependency_baseline(
        "dependency.a-to-b-integration",
        DependencyRelation::IntegratedRevision,
        revision(50),
        revision(51),
    );
    let verified = dependency_baseline(
        "dependency.a-to-b-verified",
        DependencyRelation::VerifiedCandidate,
        revision(50),
        revision(51),
    );
    let mut independent = dependency_baseline(
        "dependency.c-to-d-ordering",
        DependencyRelation::OrderingArchive,
        producer_c,
        consumer_d,
    );
    independent.producer.work = "change-c".into();
    independent.producer.revision = "revision-c".into();
    independent.context.producer_repository = "onixresearch/kiln".into();

    let evidence = vec![
        archive_evidence("dependency.a-to-b-ordering", EvidenceCoverage::WholeChange),
        DependencyEvidence::CandidateVerified {
            dependency_id: "dependency.a-to-b-verified".into(),
            repository: "onixresearch/kiln".into(),
            candidate: "producer-change".into(),
            revision: "revision-a".into(),
            verification_receipt_blake3: HASH_C.into(),
        },
        DependencyEvidence::Archive {
            dependency_id: "dependency.c-to-d-ordering".into(),
            change: "change-c".into(),
            repository: "onixresearch/kiln".into(),
            revision: "revision-c".into(),
            archive_receipt_blake3: HASH_A.into(),
            coverage: EvidenceCoverage::WholeChange,
        },
    ];
    let request = self::request(
        Vec::new(),
        Vec::new(),
        vec![ordering, integration, verified, independent],
        evidence,
        Vec::new(),
        Vec::new(),
    );
    let report = evaluate_refinement(&policy(Vec::new(), Vec::new()), &request).unwrap();
    assert_eq!(
        dependency(&report, "dependency.a-to-b-ordering").state,
        ObligationState::Supported
    );
    assert_eq!(
        dependency(&report, "dependency.a-to-b-verified").state,
        ObligationState::Supported
    );
    assert_eq!(
        dependency(&report, "dependency.a-to-b-integration").state,
        ObligationState::Pending
    );
    assert_eq!(
        dependency(&report, "dependency.c-to-d-ordering").state,
        ObligationState::Supported
    );
}

#[test]
// r[verify cairn.refinement.dependencies]
fn integrated_revision_requires_exact_admitted_observation() {
    let producer = revision(60);
    let consumer = revision(61);
    let dependency = dependency_baseline(
        "dependency.integration",
        DependencyRelation::IntegratedRevision,
        producer,
        consumer,
    );
    let exact = DependencyEvidence::Integration {
        dependency_id: "dependency.integration".into(),
        repository: "onixresearch/kiln".into(),
        target: "kiln-main".into(),
        change: "producer-change".into(),
        candidate: "producer-change".into(),
        revision: "revision-a".into(),
        observation_blake3: HASH_D.into(),
    };
    let stale = DependencyEvidence::Integration {
        dependency_id: "dependency.integration".into(),
        repository: "onixresearch/kiln".into(),
        target: "kiln-main".into(),
        change: "producer-change".into(),
        candidate: "producer-change".into(),
        revision: "revision-old".into(),
        observation_blake3: HASH_D.into(),
    };
    let out_of_scope = DependencyEvidence::Integration {
        dependency_id: "dependency.integration".into(),
        repository: "other/repository".into(),
        target: "kiln-main".into(),
        change: "producer-change".into(),
        candidate: "producer-change".into(),
        revision: "revision-a".into(),
        observation_blake3: HASH_D.into(),
    };

    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![exact.clone()],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Supported
    );

    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![stale],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    let stale_explanation = dependency_explanation(&report);
    assert_eq!(stale_explanation.state, ObligationState::Pending);
    assert!(stale_explanation.reason.contains("stale_revision"));
    assert!(report
        .reevaluation_triggers
        .contains("applicability_reevaluation:dependency.integration"));

    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![out_of_scope],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(dependency_explanation(&report)
        .reason
        .contains("out_of_scope"));

    let second_observation = DependencyEvidence::Integration {
        dependency_id: "dependency.integration".into(),
        repository: "onixresearch/kiln".into(),
        target: "kiln-main".into(),
        change: "producer-change".into(),
        candidate: "producer-change".into(),
        revision: "revision-a".into(),
        observation_blake3: HASH_A.into(),
    };
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency],
            vec![exact, second_observation],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(dependency_explanation(&report).reason.contains("ambiguous"));
}

fn dependency_explanation(report: &RefinementReport) -> &DependencyExplanation {
    &report.dependencies[0]
}

#[test]
// r[verify cairn.refinement.dependencies]
fn whole_change_coverage_is_not_a_selected_task_substitute() {
    let producer = revision(70);
    let consumer = revision(71);
    let mut dependency = dependency_baseline(
        "dependency.whole-change",
        DependencyRelation::OrderingArchive,
        producer,
        consumer,
    );
    dependency.context.producer_repository = "onixresearch/kiln".into();
    let partial = archive_evidence(
        "dependency.whole-change",
        EvidenceCoverage::Partial {
            tasks: vec!["task-1".into()],
        },
    );
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![partial],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(dependency_explanation(&report)
        .reason
        .contains("coverage_incomplete"));

    let whole = archive_evidence("dependency.whole-change", EvidenceCoverage::WholeChange);
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![whole],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Supported
    );

    dependency.coverage = Coverage::SelectedTasks {
        tasks: vec!["task-1".into()],
    };
    let partial = archive_evidence(
        "dependency.whole-change",
        EvidenceCoverage::Partial {
            tasks: vec!["task-1".into(), "task-2".into()],
        },
    );
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency],
            vec![partial],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Supported
    );
}

#[test]
// r[verify cairn.refinement.dependencies]
fn verified_specification_is_not_strengthened_to_archival() {
    let mut dependency = dependency_baseline(
        "dependency.spec",
        DependencyRelation::VerifiedSpecification,
        revision(80),
        revision(81),
    );
    dependency.required_gates = vec!["proposal".into(), "design".into(), "tasks".into()];
    let mut evidence = DependencyEvidence::Specification {
        dependency_id: "dependency.spec".into(),
        repository: "onixresearch/kiln".into(),
        revision: "revision-a".into(),
        gates: vec!["proposal".into(), "design".into(), "tasks".into()],
        receipt_blake3: HASH_D.into(),
    };
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency.clone()],
            vec![evidence.clone()],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Supported
    );

    if let DependencyEvidence::Specification { gates, .. } = &mut evidence {
        gates.pop();
    }
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency],
            vec![evidence],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(dependency_explanation(&report)
        .reason
        .contains("missing_specification_gate"));
}

#[test]
// r[verify cairn.refinement.dependencies]
fn candidate_verification_input_and_lifecycle_stay_distinct() {
    let producer = revision(90);
    let consumer = revision(91);
    let mut candidate = dependency_baseline(
        "dependency.candidate",
        DependencyRelation::VerifiedCandidate,
        producer.clone(),
        consumer.clone(),
    );
    candidate.producer.work = "candidate-a".into();
    let mut input = dependency_baseline(
        "dependency.input",
        DependencyRelation::CandidateInput,
        producer,
        consumer.clone(),
    );
    input.required_input_blake3 = Some(HASH_D.into());
    let mut lifecycle = dependency_baseline(
        "dependency.lifecycle",
        DependencyRelation::Lifecycle,
        revision(92),
        consumer,
    );
    lifecycle.required_lifecycle_stage = Some("archive".into());

    let evidence = vec![
        DependencyEvidence::CandidateVerified {
            dependency_id: "dependency.candidate".into(),
            repository: "onixresearch/kiln".into(),
            candidate: "candidate-a".into(),
            revision: "revision-a".into(),
            verification_receipt_blake3: HASH_A.into(),
        },
        DependencyEvidence::Lifecycle {
            dependency_id: "dependency.lifecycle".into(),
            repository: "onixresearch/kiln".into(),
            stage: "archive".into(),
            revision: "revision-a".into(),
            receipt_blake3: HASH_B.into(),
        },
    ];
    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![candidate, input.clone(), lifecycle],
            evidence,
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(report.dependencies[0].state, ObligationState::Supported);
    assert_eq!(report.dependencies[1].state, ObligationState::Pending);
    assert!(report.dependencies[1].reason.contains("missing_evidence"));
    assert_eq!(report.dependencies[2].state, ObligationState::Supported);

    let report = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            Vec::new(),
            Vec::new(),
            vec![input],
            vec![DependencyEvidence::CandidateInput {
                dependency_id: "dependency.input".into(),
                repository: "onixresearch/kiln".into(),
                input_blake3: HASH_D.into(),
                revision: "revision-a".into(),
            }],
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Supported
    );
}

struct SemanticFixture {
    policy: RefinementPolicy,
    request: RefinementRequest,
}

fn semantic_fixture() -> SemanticFixture {
    let producer = revision(100);
    let consumer = revision(101);
    let (case, claim_id) = structural_case("guarantee.producer", &producer);
    let statement = "consumer assumption discharged by producer guarantee";
    let mut dependency = dependency_baseline(
        "dependency.semantic",
        DependencyRelation::Semantic,
        producer.clone(),
        consumer,
    );
    dependency.context.producer_repository = REPOSITORY.into();
    dependency.context.intended_target = REPOSITORY.into();
    // Context identity includes the declared resource envelope, so the fixture's
    // scopes and dependency must agree before the discharge rule is exercised.
    dependency.context.bounds = scope().bounds;
    dependency.semantic = Some(SemanticBinding {
        producer_guarantee: GuaranteeRef {
            guarantee_case: "guarantee.producer".into(),
            claim_id: claim_id.clone(),
        },
        consumer_assumption: AssumptionRef {
            id: "assumption.consumer".into(),
            statement: statement.into(),
        },
        producer_scope: scope(),
        consumer_scope: scope(),
        witness: SemanticWitness {
            assurance: Assurance::Reviewed,
            artifact_blake3: HASH_D.into(),
            rationale: "reviewed implication correspondence".into(),
            guarantee_statement_blake3: guarantee_statement_identity(
                &producer.requirement.row_blake3,
                "receipt_structurally_valid",
            )
            .unwrap(),
            assumption_statement_blake3: assumption_statement_identity(
                "assumption.consumer",
                statement,
            )
            .unwrap(),
        },
    });
    let mut decomposition_parent = revision(102);
    decomposition_parent.requirement.requirement_id = "cairn.test.refinement.consumer".into();
    let mut consumer_decomposition = decomposition(
        "decomposition.consumer",
        decomposition_parent,
        vec![child(
            "child.consumer",
            &revision(101),
            "guarantee.consumer",
            "claim.consumer",
        )],
        Composition::AllOf {
            obligations: vec![composition_child("child.consumer")],
        },
    );
    consumer_decomposition.environment = vec![EnvironmentAssumption {
        id: "assumption.consumer".into(),
        statement: statement.into(),
        atom: AssumptionAtom {
            predicate: "producer-guarantee".into(),
            asserted: true,
        },
        source: "cairn.test.refinement.consumer".into(),
    }];
    consumer_decomposition.feasibility.covered =
        BTreeSet::from(["child.consumer".into(), "assumption.consumer".into()]);
    // The child guarantee itself stays pending; only the semantic discharge is
    // under test, so the decomposition state must not become supported.
    SemanticFixture {
        policy: policy(Vec::new(), Vec::new()),
        request: request(
            vec![consumer_decomposition],
            vec![case],
            vec![dependency],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    }
}

#[test]
// r[verify cairn.refinement.semantic_dependency]
// r[verify cairn.refinement.discharge]
fn semantic_dependency_discharges_assumption_with_path_and_source() {
    let SemanticFixture { policy, request } = semantic_fixture();
    let report = evaluate_refinement(&policy, &request).unwrap();
    let semantic = dependency(&report, "dependency.semantic");
    assert_eq!(semantic.state, ObligationState::Supported);
    let assumption = report
        .residual_assumptions
        .iter()
        .find(|assumption| assumption.id == "assumption.consumer")
        .unwrap();
    assert_eq!(assumption.status, AssumptionStatus::Discharged);
    assert_eq!(assumption.source, "cairn.test.refinement.consumer");
    assert_eq!(assumption.discharge_paths.len(), 1);
    assert_eq!(
        assumption.discharge_paths[0],
        vec![
            "dependency.semantic".to_string(),
            "guarantee.producer".to_string(),
            semantic
                .evidence
                .as_ref()
                .unwrap()
                .split(':')
                .next_back()
                .unwrap()
                .to_string()
        ]
    );
    assert!(report
        .trust_dependencies
        .iter()
        .any(|dependency| dependency.starts_with("semantic-witness:")));
}

#[test]
// r[verify cairn.refinement.semantic_dependency]
fn matching_names_and_receipt_identity_do_not_discharge_without_witness() {
    let mut fixture = semantic_fixture();
    fixture.request.dependencies[0].semantic = None;
    let report = evaluate_refinement(&fixture.policy, &fixture.request).unwrap();
    assert_eq!(
        dependency(&report, "dependency.semantic").reason,
        "unsupported: semantic_binding_missing"
    );

    let mut fixture = semantic_fixture();
    fixture.request.dependencies[0]
        .semantic
        .as_mut()
        .unwrap()
        .witness
        .guarantee_statement_blake3 = HASH_A.into();
    let report = evaluate_refinement(&fixture.policy, &fixture.request).unwrap();
    assert_eq!(
        dependency(&report, "dependency.semantic").reason,
        "unsupported: witness_mismatch"
    );

    let mut fixture = semantic_fixture();
    fixture.request.dependencies[0]
        .semantic
        .as_mut()
        .unwrap()
        .producer_scope
        .target = "other/repository".into();
    let report = evaluate_refinement(&fixture.policy, &fixture.request).unwrap();
    let blocked = dependency(&report, "dependency.semantic");
    assert_eq!(blocked.reason, "unsupported: context_incompatible");
    assert!(report
        .reevaluation_triggers
        .contains("applicability_reevaluation:dependency.semantic"));
}

#[test]
// r[verify cairn.refinement.acyclic]
fn circular_justification_reports_a_specific_cycle() {
    let first = revision(110);
    let second = revision(111);
    let forward = decomposition(
        "decomposition.forward",
        first.clone(),
        vec![child(
            "child.second",
            &second,
            "guarantee.second",
            "claim.second",
        )],
        Composition::AllOf {
            obligations: vec![composition_child("child.second")],
        },
    );
    let backward = decomposition(
        "decomposition.backward",
        second,
        vec![child(
            "child.first",
            &first,
            "guarantee.first",
            "claim.first",
        )],
        Composition::AllOf {
            obligations: vec![composition_child("child.first")],
        },
    );
    let error = evaluate_refinement(
        &policy(Vec::new(), Vec::new()),
        &request(
            vec![forward, backward],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap_err();
    assert_eq!(error.outcome(), "cycle");
    match error {
        RefinementError::Cycle(message) => {
            assert!(message.contains("cairn.test.refinement.110"));
            assert!(message.contains("cairn.test.refinement.111"));
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
// r[verify cairn.refinement.explanations]
fn reevaluation_triggers_cover_requirement_policy_assumption_and_context() {
    let (policy, request) = supported_pilot();
    let report = evaluate_refinement(&policy, &request).unwrap();
    let mut previous = report.basis.clone();
    previous.policy_blake3 = HASH_A.into();
    previous
        .requirement_rows
        .insert("removed-requirement-row".into());
    previous
        .assumption_statements
        .insert("removed:assumption".into());
    previous.contexts.insert("removed-context".into());
    let mut request = request;
    request.previous_basis = Some(previous);
    let report = evaluate_refinement(&policy, &request).unwrap();
    assert!(report.reevaluation_triggers.contains("policy_changed"));
    assert!(report
        .reevaluation_triggers
        .iter()
        .any(|trigger| trigger.starts_with("requirement_changed:")));
    assert!(report
        .reevaluation_triggers
        .iter()
        .any(|trigger| trigger.starts_with("assumption_changed:")));
    assert!(report
        .reevaluation_triggers
        .iter()
        .any(|trigger| trigger.starts_with("context_changed:")));
}

#[test]
// r[verify cairn.refinement.assurance]
fn checked_proof_requires_an_admitted_checker_and_keeps_its_distinction() {
    let (mut policy, mut request) = supported_pilot();
    request.decompositions[0].relation.assurance = Assurance::CheckedProof;
    request.decompositions[0].relation.proof_checker = Some("checker.one".into());
    let error = evaluate_refinement(&policy, &request).unwrap_err();
    assert_eq!(error.outcome(), "unsupported");

    policy.proof_checkers = vec!["checker.one".into()];
    policy.assurances.push(Assurance::CheckedProof);
    let report = evaluate_refinement(&policy, &request).unwrap();
    let pilot = explanation(&report, "decomposition.native");
    assert_eq!(pilot.state, ObligationState::Supported);
    assert_eq!(pilot.assurance, Assurance::Reviewed);
    assert!(report
        .trust_dependencies
        .contains("proof-checker:checker.one:trusted-declaration"));
}

#[test]
// r[verify cairn.refinement.dependencies]
fn unknown_relation_is_an_unsupported_obligation() {
    let mut policy = self::policy(Vec::new(), Vec::new());
    policy
        .relations
        .retain(|relation| *relation != DependencyRelation::Lifecycle);
    let mut dependency = dependency_baseline(
        "dependency.lifecycle",
        DependencyRelation::Lifecycle,
        revision(120),
        revision(121),
    );
    dependency.required_lifecycle_stage = Some("archive".into());
    let report = evaluate_refinement(
        &policy,
        &request(
            Vec::new(),
            Vec::new(),
            vec![dependency],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    assert_eq!(
        dependency_explanation(&report).state,
        ObligationState::Unsupported
    );
    assert!(dependency_explanation(&report)
        .reason
        .contains("relation_not_admitted"));
}

#[test]
fn invalid_policy_bounds_and_unknowns_are_explicit() {
    let mut policy = self::policy(Vec::new(), Vec::new());
    policy.schema = "cairn.refinement-policy.v0".into();
    assert!(matches!(
        admit_refinement_policy(&policy),
        Err(RefinementError::Unsupported(_))
    ));

    let mut policy = self::policy(Vec::new(), Vec::new());
    policy.bounds.max_depth = 0;
    assert!(matches!(
        admit_refinement_policy(&policy),
        Err(RefinementError::BoundExhaustion(_))
    ));

    let mut policy = self::policy(Vec::new(), Vec::new());
    policy.assurances = vec![Assurance::Unassessed];
    assert!(matches!(
        admit_refinement_policy(&policy),
        Err(RefinementError::Unsupported(_))
    ));
}
