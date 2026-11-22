use super::*;

use serde_json::json;

use super::super::agent_receipt::{
    authoritative_command_hash, authoritative_selection_hash, AgentRequiredCheck,
};
use super::super::ast_grep_gate::ast_grep_receipt_hash;

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const REVIEWER: &str = "independent-test-reviewer";
const COMPLETION_SCOPE: &str =
    "native completion accepted with reviewed correspondence; no behavioral satisfaction";

struct Case {
    request: EvaluationRequest,
    policy: ClaimPolicy,
    context: ClaimContext,
    ids: BTreeMap<&'static str, String>,
}

fn row() -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: "cairn.test.scoped_claims".into(),
        specification_path: ".cairn/specs/scoped-claims/spec.md".into(),
        specification_blake3: HASH_A.into(),
        source_span: super::super::content_bound_requirement_registry::ContentBoundSourceSpan {
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
        max_nodes: 64,
        max_depth: 8,
        max_domain_values: 8,
        max_steps: 1000,
    }
}

fn semantic_contract(row: &ContentBoundRequirementRow) -> SemanticContract {
    SemanticContract {
        profile: profile().id,
        proposed_by: "scoped-claims-test-author".into(),
        requirement: row.clone(),
        domains: BTreeMap::from([(
            "bool".to_string(),
            super::super::typed_requirement_contracts::FiniteDomain {
                values: vec!["false".into(), "true".into()],
            },
        )]),
        observables: BTreeMap::new(),
        assumptions: Vec::new(),
        activation: super::super::typed_requirement_contracts::FiniteExpr::Bool { value: true },
        guarantee: super::super::typed_requirement_contracts::FiniteExpr::Bool { value: true },
        supported_portion: "scoped claim derivation test".into(),
        informal_remainder: "not an admitted behavior claim".into(),
    }
}

fn context() -> ClaimContext {
    ClaimContext {
        subject_blake3: HASH_C.into(),
        policy_blake3: HASH_B.into(),
        target: "onixresearch/cairn".into(),
        environment: "linux-x86_64".into(),
        bounds: BTreeMap::from([("max_checks".to_string(), "2".to_string())]),
    }
}

fn policy() -> ClaimPolicy {
    serde_json::from_value(json!({
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
            {"id": "reviewed_completion_v1", "version": 1, "assurance_scope": COMPLETION_SCOPE}
        ],
        "bounds": {
            "max_claims": 16,
            "max_derivations": 16,
            "max_producers": 8,
            "max_premises": 4,
            "max_depth": 8
        }
    }))
    .unwrap()
}

fn claim(
    row: &ContentBoundRequirementRow,
    interpretation_blake3: &str,
    contract_blake3: &str,
    context: &ClaimContext,
    kind: ClaimKind,
    proposition: Proposition,
) -> ScopedClaim {
    ScopedClaim {
        schema: CLAIM_SCHEMA_V1.into(),
        kind,
        requirement: row.clone(),
        interpretation_blake3: interpretation_blake3.into(),
        contract_blake3: contract_blake3.into(),
        context: context.clone(),
        proposition,
    }
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
    let contract = json!({
        "schema": "cairn.agent-completion.v2",
        "repository_id": "onixresearch/cairn",
        "registry_id": "cairn-release-checks",
        "registry_hash": HASH_A,
        "selection_hash": selection_hash.clone(),
        "policy_hash": HASH_B,
        "final_subject_hash": HASH_C,
        "required_checks": checks.clone(),
    });
    let results = json!([
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

fn completion_producer(clippy_status: &str, context: &ClaimContext) -> ProducerInput {
    let (contract, results) = completion_values(clippy_status);
    ProducerInput::AuthoritativeCompletion {
        context: context.clone(),
        contract,
        results,
        trust_dependencies: BTreeSet::from([
            "native-execution-provenance:caller-retained".to_string(),
            "native-test-oracle:shared-cargo-test-harness".to_string(),
        ]),
    }
}

fn structural_receipt() -> serde_json::Value {
    json!({
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

fn structural_producer(context: &ClaimContext) -> ProducerInput {
    ProducerInput::StructuralAstGrep {
        context: context.clone(),
        repository_scope: ".".into(),
        receipt: structural_receipt(),
        trust_dependencies: BTreeSet::from(["structural-rule-bundle:caller-retained".to_string()]),
    }
}

fn review(
    row: &ContentBoundRequirementRow,
    normalized: &super::super::typed_requirement_contracts::NormalizedContract,
) -> CorrespondenceReview {
    CorrespondenceReview {
        requirement_row_blake3: row.row_blake3.clone(),
        contract_blake3: normalized.contract_blake3.clone(),
        interpretation_blake3: normalized.interpretation_blake3.clone(),
        mapping_blake3: normalized.mapping_blake3.clone(),
        proposed_by: normalized.contract.proposed_by.clone(),
        reviewer: REVIEWER.into(),
        review_artifact_blake3: "d".repeat(64),
        decision: "approved".into(),
    }
}

fn case() -> Case {
    let row = row();
    let profile = profile();
    let contract = semantic_contract(&row);
    let normalized = normalize_contract(&profile, &row, &contract).unwrap();
    let context = context();
    let make = |kind, proposition| {
        claim(
            &row,
            &normalized.interpretation_blake3,
            &normalized.contract_blake3,
            &context,
            kind,
            proposition,
        )
    };
    let clippy = make(
        ClaimKind::Observation,
        Proposition::CheckPassed {
            check_id: "clippy".into(),
        },
    );
    let tests = make(
        ClaimKind::Observation,
        Proposition::CheckPassed {
            check_id: "tests".into(),
        },
    );
    let eligible = make(
        ClaimKind::CheckerAcceptedProperty,
        Proposition::NativeCompletionEligible,
    );
    let reviewed = make(
        ClaimKind::ReviewedRequirementCorrespondence,
        Proposition::ReviewedCorrespondence,
    );
    let decision = make(
        ClaimKind::BoundedProperty,
        Proposition::ReviewedCompletionDecision,
    );
    let ids = BTreeMap::from([
        ("clippy", claim_identity(&clippy).unwrap()),
        ("tests", claim_identity(&tests).unwrap()),
        ("eligible", claim_identity(&eligible).unwrap()),
        ("reviewed", claim_identity(&reviewed).unwrap()),
        ("decision", claim_identity(&decision).unwrap()),
    ]);
    let derivation = Derivation {
        id: "derivation.completion".into(),
        rule: Rule::ReviewedCompletionV1,
        premises: vec![ids["eligible"].clone(), ids["reviewed"].clone()],
        conclusion: ids["decision"].clone(),
        applicability: context.clone(),
        assurance_scope: COMPLETION_SCOPE.into(),
        residual_assumptions: BTreeSet::from([
            "reviewed-correspondence:accepted-declaration".into()
        ]),
    };
    Case {
        request: EvaluationRequest {
            profile,
            native_requirement: row.clone(),
            semantic_contract: contract,
            correspondence_review: Some(review(&row, &normalized)),
            authorized_reviewers: vec![REVIEWER.into()],
            claims: vec![clippy, tests, eligible, reviewed, decision],
            producers: vec![completion_producer("passed", &context)],
            derivations: vec![derivation],
        },
        policy: policy(),
        context,
        ids,
    }
}

fn explanation<'a>(report: &'a DerivationReport, id: &str) -> &'a ClaimExplanation {
    report
        .claims
        .iter()
        .find(|explanation| explanation.claim_id == id)
        .expect("claim is explained")
}

#[test]
// r[verify cairn.scoped_claims.states]
// r[verify cairn.scoped_claims.determinism]
fn native_completion_case_explains_decision_and_replays_deterministically() {
    let case = case();
    let report = evaluate_derivations(&case.policy, &case.request).unwrap();
    assert!(report.advisory);
    assert_eq!(report.schema, DERIVATION_REPORT_SCHEMA_V1);
    assert_eq!(report.independence, INDEPENDENCE_NOT_ESTABLISHED);
    for key in ["clippy", "tests", "eligible", "reviewed", "decision"] {
        assert_eq!(
            explanation(&report, &case.ids[key]).state,
            ClaimState::Supported,
            "{key}"
        );
    }
    let decision = explanation(&report, &case.ids["decision"]);
    assert_eq!(decision.kind, ClaimKind::BoundedProperty);
    assert_eq!(decision.support.len(), 1);
    let derived = &decision.support[0];
    assert_eq!(derived.rule, "reviewed_completion_v1");
    assert_eq!(derived.version, RULE_VERSION_V1);
    assert_eq!(
        derived.premises,
        vec![case.ids["eligible"].clone(), case.ids["reviewed"].clone()]
    );
    assert_eq!(derived.applicability, case.context);
    assert_eq!(derived.assurance_scope, COMPLETION_SCOPE);
    assert!(derived
        .residual_assumptions
        .contains("native-test-oracle:adequacy-not-established"));
    assert!(derived
        .residual_assumptions
        .contains("native-execution-honesty:not-established"));
    assert!(report
        .trust_dependencies
        .contains("native-test-oracle:shared-cargo-test-harness"));
    let clippy = explanation(&report, &case.ids["clippy"]);
    let tests = explanation(&report, &case.ids["tests"]);
    let shared = "native-test-oracle:shared-cargo-test-harness";
    assert!(clippy.support[0].residual_assumptions.contains(shared));
    assert!(tests.support[0].residual_assumptions.contains(shared));
    assert_eq!(
        report
            .trust_dependencies
            .iter()
            .filter(|dependency| *dependency == shared)
            .count(),
        1,
        "shared oracle dependency stays one shared entry"
    );
    assert!(report.unresolved.is_empty());

    let replay = evaluate_derivations(&case.policy, &case.request).unwrap();
    assert_eq!(report.report_blake3, replay.report_blake3);
    assert_eq!(report.input_blake3, replay.input_blake3);
    assert_eq!(report, replay);
}

#[test]
// r[verify cairn.scoped_claims.native_producers]
fn timeout_and_unsupported_statuses_establish_neither_polarity() {
    for status in ["timed_out", "exploded"] {
        let mut case = case();
        case.request.producers = vec![completion_producer(status, &case.context)];
        let report = evaluate_derivations(&case.policy, &case.request).unwrap();
        let observation = explanation(&report, &case.ids["clippy"]);
        assert_eq!(observation.state, ClaimState::Unresolved, "{status}");
        assert!(observation.support.is_empty());
        assert!(observation.counterevidence.is_empty());
        assert_eq!(
            explanation(&report, &case.ids["eligible"]).state,
            ClaimState::Unresolved
        );
        let decision = explanation(&report, &case.ids["decision"]);
        assert_eq!(decision.state, ClaimState::Unresolved);
        assert!(decision.support.is_empty());
        assert!(decision.counterevidence.is_empty());
        assert!(decision.unresolved_premises.contains(&case.ids["eligible"]));
        assert!(report.unresolved.contains(&case.ids["decision"]));
    }
}

#[test]
// r[verify cairn.scoped_claims.native_producers]
// r[verify cairn.scoped_claims.rules]
fn observations_alone_do_not_establish_the_completion_property() {
    let mut case = case();
    case.request.correspondence_review = None;
    case.request.authorized_reviewers.clear();
    let report = evaluate_derivations(&case.policy, &case.request).unwrap();
    assert_eq!(
        explanation(&report, &case.ids["clippy"]).state,
        ClaimState::Supported
    );
    assert_eq!(
        explanation(&report, &case.ids["tests"]).state,
        ClaimState::Supported
    );
    assert_eq!(
        explanation(&report, &case.ids["eligible"]).state,
        ClaimState::Supported
    );
    assert_eq!(
        explanation(&report, &case.ids["reviewed"]).state,
        ClaimState::Unresolved
    );
    let decision = explanation(&report, &case.ids["decision"]);
    assert_eq!(decision.state, ClaimState::Unresolved);
    assert!(decision.support.is_empty());
    assert!(decision.unresolved_premises.contains(&case.ids["reviewed"]));
}

#[test]
// r[verify cairn.scoped_claims.states]
fn failed_check_refutes_only_its_exact_observation() {
    let mut case = case();
    case.request.producers = vec![completion_producer("failed", &case.context)];
    let report = evaluate_derivations(&case.policy, &case.request).unwrap();
    let clippy = explanation(&report, &case.ids["clippy"]);
    assert_eq!(clippy.state, ClaimState::Refuted);
    assert_eq!(clippy.kind, ClaimKind::Observation);
    assert!(clippy.support.is_empty());
    assert_eq!(clippy.counterevidence.len(), 1);
    assert_eq!(
        clippy.counterevidence[0].rule,
        "native-check-observation.v1"
    );
    assert_eq!(
        clippy.claim.proposition,
        Proposition::CheckPassed {
            check_id: "clippy".into()
        }
    );
    assert_eq!(
        explanation(&report, &case.ids["tests"]).state,
        ClaimState::Supported
    );
    let decision = explanation(&report, &case.ids["decision"]);
    assert_eq!(decision.state, ClaimState::Unresolved);
    assert!(decision.counterevidence.is_empty());
}

#[test]
// r[verify cairn.scoped_claims.states]
fn support_and_refutation_remain_visible_together() {
    let mut case = case();
    case.request
        .producers
        .push(completion_producer("failed", &case.context));
    let report = evaluate_derivations(&case.policy, &case.request).unwrap();
    let clippy = explanation(&report, &case.ids["clippy"]);
    assert_eq!(clippy.state, ClaimState::Disputed);
    assert_eq!(clippy.support.len(), 1);
    assert_eq!(clippy.counterevidence.len(), 1);
    assert_ne!(
        clippy.support[0].source, clippy.counterevidence[0].source,
        "support and counterevidence come from distinct admitted receipts"
    );
    assert!(clippy.state_reason.contains("neither cancels"));
    assert_eq!(
        explanation(&report, &case.ids["tests"]).state,
        ClaimState::Supported
    );
}

#[test]
// r[verify cairn.scoped_claims.authority]
fn structural_receipts_stay_recorded_only_and_cannot_establish_completion() {
    let mut case = case();
    let receipt = structural_receipt();
    let receipt_hash = ast_grep_receipt_hash(&receipt);
    let structural = claim(
        &case.request.native_requirement,
        &case.request.claims[0].interpretation_blake3,
        &case.request.claims[0].contract_blake3,
        &case.context,
        ClaimKind::StructuralRelationship,
        Proposition::ReceiptStructurallyValid {
            receipt_blake3: receipt_hash.clone(),
        },
    );
    let structural_id = claim_identity(&structural).unwrap();
    case.request.claims.push(structural);
    case.request.producers = vec![structural_producer(&case.context)];
    let report = evaluate_derivations(&case.policy, &case.request).unwrap();
    let structural_explanation = explanation(&report, &structural_id);
    assert_eq!(structural_explanation.state, ClaimState::Supported);
    assert_eq!(
        structural_explanation.kind,
        ClaimKind::StructuralRelationship
    );
    assert_eq!(structural_explanation.support[0].source, receipt_hash);
    assert!(structural_explanation.support[0]
        .assurance_scope
        .starts_with("recorded_only:"));
    assert!(report
        .trust_dependencies
        .contains("recorded-only:no-behavior"));
    let decision = explanation(&report, &case.ids["decision"]);
    assert_eq!(decision.state, ClaimState::Unresolved);
    assert!(decision.counterevidence.is_empty());

    // A structural link cannot replace the admitted correspondence premise.
    case.request.derivations = vec![Derivation {
        id: "derivation.structural".into(),
        rule: Rule::ReviewedCompletionV1,
        premises: vec![case.ids["eligible"].clone(), structural_id.clone()],
        conclusion: case.ids["decision"].clone(),
        applicability: case.context.clone(),
        assurance_scope: COMPLETION_SCOPE.into(),
        residual_assumptions: BTreeSet::new(),
    }];
    let error = evaluate_derivations(&case.policy, &case.request).unwrap_err();
    assert_eq!(error.outcome(), "unsupported");
}

#[test]
// r[verify cairn.scoped_claims.scope]
fn different_revisions_targets_environments_and_bounds_cannot_combine() {
    let mut revision = case();
    revision.request.claims[0].requirement.row_blake3 = "e".repeat(64);
    assert_eq!(
        evaluate_derivations(&revision.policy, &revision.request)
            .unwrap_err()
            .outcome(),
        "scope_mismatch"
    );

    let mut applicability = case();
    applicability.request.derivations[0]
        .applicability
        .environment = "other-os".into();
    assert_eq!(
        evaluate_derivations(&applicability.policy, &applicability.request)
            .unwrap_err()
            .outcome(),
        "scope_mismatch"
    );

    let mut premise_scope = case();
    let mut foreign = premise_scope.request.claims[2].clone();
    foreign
        .context
        .bounds
        .insert("max_checks".into(), "1".into());
    let foreign_id = claim_identity(&foreign).unwrap();
    premise_scope.request.claims.push(foreign);
    premise_scope.request.derivations[0].premises =
        vec![foreign_id, premise_scope.ids["reviewed"].clone()];
    assert_eq!(
        evaluate_derivations(&premise_scope.policy, &premise_scope.request)
            .unwrap_err()
            .outcome(),
        "scope_mismatch"
    );

    let mut target = case();
    let mut foreign_context = target.context.clone();
    foreign_context.target = "other/repository".into();
    target.request.producers = vec![completion_producer("passed", &foreign_context)];
    assert_eq!(
        evaluate_derivations(&target.policy, &target.request)
            .unwrap_err()
            .outcome(),
        "scope_mismatch"
    );
}

#[test]
// r[verify cairn.scoped_claims.rules]
fn unsupported_rules_missing_premises_and_duplicates_are_explicit() {
    let mut scope = case();
    scope.request.derivations[0].assurance_scope = "unbounded behavior".into();
    assert_eq!(
        evaluate_derivations(&scope.policy, &scope.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut shape = case();
    shape.request.derivations[0].premises = vec![shape.ids["reviewed"].clone()];
    assert_eq!(
        evaluate_derivations(&shape.policy, &shape.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut missing = case();
    missing.request.derivations = vec![Derivation {
        id: "derivation.missing".into(),
        rule: Rule::ReviewedCompletionV1,
        premises: vec!["claim.not-declared".into()],
        conclusion: missing.ids["decision"].clone(),
        applicability: missing.context.clone(),
        assurance_scope: COMPLETION_SCOPE.into(),
        residual_assumptions: BTreeSet::new(),
    }];
    let report = evaluate_derivations(&missing.policy, &missing.request).unwrap();
    let decision = explanation(&report, &missing.ids["decision"]);
    assert_eq!(decision.state, ClaimState::Unresolved);
    assert!(decision.unresolved_premises.contains("claim.not-declared"));

    let mut duplicate = case();
    let repeated = duplicate.request.derivations[0].clone();
    duplicate.request.derivations.push(repeated);
    assert_eq!(
        evaluate_derivations(&duplicate.policy, &duplicate.request)
            .unwrap_err()
            .outcome(),
        "invalid_input"
    );
}

#[test]
// r[verify cairn.scoped_claims.bounds]
fn policy_vocabulary_and_bounds_are_admitted_explicitly() {
    let mut bounded = case();
    bounded.policy.bounds.max_claims = 1;
    assert_eq!(
        evaluate_derivations(&bounded.policy, &bounded.request)
            .unwrap_err()
            .outcome(),
        "bound_exhaustion"
    );

    let mut version = case();
    version.policy.rules[0].version = 2;
    assert_eq!(
        evaluate_derivations(&version.policy, &version.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut duplicate_rule = case();
    let repeated = duplicate_rule.policy.rules[0].clone();
    duplicate_rule.policy.rules.push(repeated);
    assert_eq!(
        evaluate_derivations(&duplicate_rule.policy, &duplicate_rule.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut unknown_proposition = case();
    unknown_proposition
        .policy
        .claims
        .propositions
        .push("invented_proposition".into());
    assert_eq!(
        evaluate_derivations(&unknown_proposition.policy, &unknown_proposition.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut unadmitted_kind = case();
    unadmitted_kind
        .policy
        .claims
        .kinds
        .retain(|kind| *kind != ClaimKind::Observation);
    assert_eq!(
        evaluate_derivations(&unadmitted_kind.policy, &unadmitted_kind.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    let mut unsupported_profile = case();
    unsupported_profile.request.profile.interpretation = "unbounded-program-semantics".into();
    assert_eq!(
        evaluate_derivations(&unsupported_profile.policy, &unsupported_profile.request)
            .unwrap_err()
            .outcome(),
        "unsupported"
    );

    // Unknown rule identifiers are rejected at policy deserialization rather
    // than silently ignored.
    let unknown_rule = serde_json::from_value::<ClaimPolicy>(json!({
        "schema": CLAIM_POLICY_SCHEMA_V1,
        "claims": {"schema": CLAIM_SCHEMA_V1, "kinds": ["observation"], "propositions": ["check_passed"]},
        "rules": [{"id": "invented_rule_v1", "version": 1, "assurance_scope": "x"}],
        "bounds": {"max_claims": 1, "max_derivations": 1, "max_producers": 1, "max_premises": 1, "max_depth": 1}
    }));
    assert!(unknown_rule.is_err());
}

#[test]
// r[verify cairn.scoped_claims.identity]
fn claim_identity_rejects_unknown_schemas_and_kind_substitution() {
    let case = case();
    let mut claim = case.request.claims[0].clone();
    claim.schema = "cairn.scoped-claim.v2".into();
    assert_eq!(claim_identity(&claim).unwrap_err().outcome(), "unsupported");

    let mut claim = case.request.claims[0].clone();
    claim.kind = ClaimKind::BoundedProperty;
    assert_eq!(
        claim_identity(&claim).unwrap_err().outcome(),
        "invalid_input"
    );

    let mut claim = case.request.claims[0].clone();
    claim.context.environment.clear();
    assert_eq!(
        claim_identity(&claim).unwrap_err().outcome(),
        "invalid_input"
    );
}

#[test]
// r[verify cairn.scoped_claims.rules]
fn defensive_cycle_detection_reports_a_cycle_outcome() {
    // v1 rule shapes cannot produce a cycle; this exercises the replay guard
    // that future rule additions and malformed internal graphs rely on.
    let context = context();
    let first = Derivation {
        id: "derivation.a".into(),
        rule: Rule::ReviewedCompletionV1,
        premises: vec!["derivation.b".into()],
        conclusion: "derivation.a".into(),
        applicability: context.clone(),
        assurance_scope: COMPLETION_SCOPE.into(),
        residual_assumptions: BTreeSet::new(),
    };
    let second = Derivation {
        id: "derivation.b".into(),
        rule: Rule::ReviewedCompletionV1,
        premises: vec!["derivation.a".into()],
        conclusion: "derivation.b".into(),
        applicability: context,
        assurance_scope: COMPLETION_SCOPE.into(),
        residual_assumptions: BTreeSet::new(),
    };
    let incoming = BTreeMap::from([
        ("derivation.a", vec![&first]),
        ("derivation.b", vec![&second]),
    ]);
    let mut evidence = BTreeMap::from([
        ("derivation.a".to_string(), Evidence::default()),
        ("derivation.b".to_string(), Evidence::default()),
    ]);
    let declared = BTreeSet::from(["derivation.a".to_string(), "derivation.b".to_string()]);
    let error = replay(
        "derivation.a",
        &incoming,
        &mut evidence,
        &declared,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
        4,
    )
    .unwrap_err();
    assert_eq!(error.outcome(), "cycle");
}
