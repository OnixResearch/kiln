// r[impl cairn.concurrent_footprints.conformance]
// r[verify cairn.concurrent_footprints.conformance]
use std::collections::BTreeMap;

use super::*;
use crate::verified::content_bound_requirement_registry::ContentBoundSourceSpan;
use crate::verified::typed_requirement_contracts::{
    normalize_contract, FiniteDomain, FiniteExpr, FiniteTerm, FiniteValue, ObservableMapping,
    SemanticContract, FINITE_INTERPRETATION_V1, FINITE_SEMANTICS_PROFILE_V1,
};

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}

fn profile() -> SemanticProfile {
    SemanticProfile {
        id: FINITE_SEMANTICS_PROFILE_V1.into(),
        interpretation: FINITE_INTERPRETATION_V1.into(),
        max_nodes: 4096,
        max_depth: 64,
        max_domain_values: 256,
        max_steps: 1_000_000,
    }
}

fn policy() -> FootprintPolicy {
    FootprintPolicy {
        schema: POLICY_SCHEMA_V1.into(),
        bounds: FootprintBounds {
            max_changes: 8,
            max_revisions: 8,
            max_assumptions: 8,
            max_guarantees: 8,
            max_invariants: 8,
            max_invalidations: 8,
            max_interfaces: 8,
            max_unknown: 8,
            max_compositions: 8,
            max_evidence: 8,
            max_conditions: 8,
            max_domain_values: 8,
            max_worlds: 1024,
            max_steps: 100_000,
            max_depth: 32,
        },
    }
}

fn row(requirement_id: &str, seed: char) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: requirement_id.into(),
        specification_path: ".cairn/specs/concurrent/spec.md".into(),
        specification_blake3: digest(seed),
        source_span: ContentBoundSourceSpan {
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            end_line: 1,
        },
        requirement_body_blake3: digest(seed),
        row_blake3: digest(seed),
    }
}

fn bool_domain() -> FiniteDomain {
    FiniteDomain {
        values: vec!["false".into(), "true".into()],
    }
}

fn boolean_observable() -> ObservableMapping {
    ObservableMapping {
        arguments: Vec::new(),
        result: "bool".into(),
        phase: "state".into(),
        source: "test observable".into(),
        projection: "test projection".into(),
    }
}

fn observable(result: &str, arguments: &[&str]) -> ObservableMapping {
    ObservableMapping {
        arguments: arguments
            .iter()
            .map(|argument| (*argument).into())
            .collect(),
        result: result.into(),
        phase: "state".into(),
        source: "test observable".into(),
        projection: "test projection".into(),
    }
}

fn revision(
    requirement: ContentBoundRequirementRow,
    domains: BTreeMap<String, FiniteDomain>,
    observables: BTreeMap<String, ObservableMapping>,
    guarantee: FiniteExpr,
) -> FootprintRevision {
    let contract = SemanticContract {
        profile: FINITE_SEMANTICS_PROFILE_V1.into(),
        proposed_by: "concurrent-footprints-test".into(),
        requirement: requirement.clone(),
        domains,
        observables,
        assumptions: Vec::new(),
        activation: FiniteExpr::Bool { value: true },
        guarantee,
        supported_portion: "finite test portion".into(),
        informal_remainder: "test remainder".into(),
    };
    let normalized = normalize_contract(&profile(), &requirement, &contract).unwrap();
    FootprintRevision {
        requirement,
        contract,
        interpretation_blake3: normalized.interpretation_blake3,
        mapping_blake3: normalized.mapping_blake3,
        contract_blake3: normalized.contract_blake3,
    }
}

fn simple_revision(requirement_id: &str, seed: char) -> FootprintRevision {
    revision(
        row(requirement_id, seed),
        BTreeMap::from([("bool".to_string(), bool_domain())]),
        BTreeMap::from([("flag".to_string(), boolean_observable())]),
        FiniteExpr::Bool { value: true },
    )
}

fn is_true(mapping: &str) -> FiniteExpr {
    FiniteExpr::Equal {
        left: FiniteTerm::Observable {
            mapping: mapping.into(),
            arguments: Vec::new(),
        },
        right: FiniteTerm::Literal {
            value: FiniteValue {
                domain: "bool".into(),
                value: "true".into(),
            },
        },
    }
}

fn and(left: FiniteExpr, right: FiniteExpr) -> FiniteExpr {
    FiniteExpr::And {
        expressions: vec![left, right],
    }
}

fn attempt_observable() -> ObservableMapping {
    observable("bool", &["attempt"])
}

fn attempt_domain() -> FiniteDomain {
    FiniteDomain {
        values: vec!["first".into(), "second".into()],
    }
}

fn authorized(variable: &str) -> FiniteExpr {
    FiniteExpr::Equal {
        left: FiniteTerm::Observable {
            mapping: "authorized".into(),
            arguments: vec![FiniteTerm::Variable {
                name: variable.into(),
            }],
        },
        right: FiniteTerm::Literal {
            value: FiniteValue {
                domain: "bool".into(),
                value: "true".into(),
            },
        },
    }
}

fn authorized_first() -> FiniteExpr {
    FiniteExpr::Equal {
        left: FiniteTerm::Observable {
            mapping: "authorized".into(),
            arguments: vec![FiniteTerm::Literal {
                value: FiniteValue {
                    domain: "attempt".into(),
                    value: "first".into(),
                },
            }],
        },
        right: FiniteTerm::Literal {
            value: FiniteValue {
                domain: "bool".into(),
                value: "true".into(),
            },
        },
    }
}

fn forall_attempt() -> FiniteExpr {
    FiniteExpr::Forall {
        variable: "attempt".into(),
        domain: "attempt".into(),
        expression: Box::new(authorized("attempt")),
    }
}

fn context() -> CaseContext {
    CaseContext {
        subject_blake3: digest('a'),
        policy_blake3: digest('b'),
        target: "test-target".into(),
        environment: "test-environment".into(),
        configurations: BTreeMap::from([("effect.dispatch".into(), digest('c'))]),
    }
}

fn interface(
    id: &str,
    configuration: &str,
    change_kind: InterfaceChange,
    order_sensitive: bool,
) -> AffectedInterface {
    AffectedInterface {
        id: id.into(),
        configuration_blake3: configuration.into(),
        change_kind,
        operation: format!("{}-operation", change_kind.tag()),
        order_sensitive,
    }
}

fn footprint(change_id: &str, revisions: Vec<FootprintRevision>) -> ChangeFootprint {
    ChangeFootprint {
        schema: FOOTPRINT_SCHEMA_V1.into(),
        change: ChangeIdentity {
            change_id: change_id.into(),
            source_revision: format!("source-{change_id}"),
            intent_revision: format!("intent-{change_id}"),
        },
        revisions,
        assumptions: Vec::new(),
        guarantees: Vec::new(),
        invariants: Vec::new(),
        invalidated: Vec::new(),
        interfaces: Vec::new(),
        evidence_basis: Vec::new(),
        provided_conditions: Vec::new(),
        unknown: Vec::new(),
    }
}

fn composition(
    id: &str,
    participants: &[&str],
    order: &[&str],
    fixed: bool,
    admitted: bool,
) -> CompositionCandidate {
    CompositionCandidate {
        id: id.into(),
        participants: participants.iter().map(|value| (*value).into()).collect(),
        order: order.iter().map(|value| (*value).into()).collect(),
        fixed,
        admitted,
    }
}

fn case(
    context: CaseContext,
    changes: Vec<ChangeFootprint>,
    compositions: Vec<CompositionCandidate>,
) -> ConcurrentChangeCase {
    ConcurrentChangeCase {
        schema: CASE_SCHEMA_V1.into(),
        profile: profile(),
        context,
        changes,
        compositions,
        joint_evidence: Vec::new(),
        individual_receipts: Vec::new(),
        prior_obligations: Vec::new(),
    }
}

fn compatible_contracts() -> (ContentBoundRequirementRow, FootprintRevision) {
    let requirement = row("alpha.requirement", '1');
    let revision = revision(
        requirement.clone(),
        BTreeMap::from([("bool".to_string(), bool_domain())]),
        BTreeMap::from([
            ("flag".to_string(), boolean_observable()),
            ("guard".to_string(), boolean_observable()),
        ]),
        and(is_true("flag"), is_true("guard")),
    );
    (requirement, revision)
}

fn compatible_case() -> ConcurrentChangeCase {
    let (alpha_row, alpha_revision) = compatible_contracts();
    let previous_form = StatementForm::Finite {
        expression: and(is_true("flag"), is_true("guard")),
    };
    let previous_identity = statement_identity(
        GUARANTEE_IDENTITY_DOMAIN_V1,
        &alpha_row.row_blake3,
        "alpha.feature",
        &previous_form,
    )
    .unwrap();

    let mut alpha = footprint("alpha", vec![alpha_revision]);
    alpha.guarantees.push(ChangedGuarantee {
        id: "alpha.feature".into(),
        previous: Some(GuaranteeStatement {
            requirement_id: "alpha.requirement".into(),
            form: previous_form,
        }),
        current: Some(GuaranteeStatement {
            requirement_id: "alpha.requirement".into(),
            form: StatementForm::Finite {
                expression: is_true("flag"),
            },
        }),
        change: GuaranteeChange::Replaced,
        affected_claims: vec!["claim.alpha".into()],
        interfaces: vec!["effect.dispatch".into()],
        configurations: vec![digest('c')],
        unknown: None,
    });
    alpha.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        false,
    ));

    let mut beta = footprint("beta", vec![simple_revision("beta.requirement", '2')]);
    beta.assumptions.push(ConsumedAssumption {
        id: "beta.feature".into(),
        requirement_id: "beta.requirement".into(),
        consumed_change: "alpha".into(),
        consumed_guarantee: "alpha.feature".into(),
        consumed_identity: previous_identity,
        dependency: AssumptionDependency::Monotone,
        meaning: "the feature flag stays enabled".into(),
        finite_predicate: Some(is_true("flag")),
        evidence_basis: vec!["claim.beta".into()],
        configurations: vec![digest('c')],
        unknown: None,
    });
    beta.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        false,
    ));

    case(
        context(),
        vec![alpha, beta],
        vec![composition(
            "c1",
            &["alpha", "beta"],
            &["alpha", "beta"],
            true,
            true,
        )],
    )
}

fn supported_propositions() -> Vec<PropositionEvidence> {
    vec![
        PropositionEvidence {
            proposition: PropositionKind::Coexistence,
            state: PropositionState::Supported,
            basis: "bounded joint check of the declared composition".into(),
        },
        PropositionEvidence {
            proposition: PropositionKind::InvariantPreservation,
            state: PropositionState::Supported,
            basis: "bounded joint check that shared invariants remain established".into(),
        },
    ]
}

fn attach_evidence(
    case: &mut ConcurrentChangeCase,
    report: &CompatibilityReport,
    scope: EvidenceScope,
    propositions: Vec<PropositionEvidence>,
) {
    let obligation = &report.obligations[0];
    case.joint_evidence = vec![JointEvidence {
        id: "joint.evidence".into(),
        obligation: obligation.id.clone(),
        combined_candidate_blake3: obligation.candidate.combined_candidate_blake3.clone(),
        participants: obligation.participants.clone(),
        compositions: obligation.candidate.compositions.clone(),
        context: case.context.clone(),
        affected_claims: obligation.affected_claims.clone(),
        propositions,
        assurance: AssuranceScope::BoundedCheck,
        scope,
        artifact_blake3: digest('e'),
    }];
}

fn retry_case() -> ConcurrentChangeCase {
    let authorization_row = row("authorization.requirement", '3');
    let authorization_revision = revision(
        authorization_row.clone(),
        BTreeMap::from([
            ("bool".to_string(), bool_domain()),
            ("attempt".to_string(), attempt_domain()),
        ]),
        BTreeMap::from([("authorized".to_string(), attempt_observable())]),
        authorized_first(),
    );
    let previous_form = StatementForm::Finite {
        expression: forall_attempt(),
    };
    let previous_identity = statement_identity(
        GUARANTEE_IDENTITY_DOMAIN_V1,
        &authorization_row.row_blake3,
        "authorization.dispatch",
        &previous_form,
    )
    .unwrap();

    let mut authorization = footprint("authorization", vec![authorization_revision]);
    authorization.guarantees.push(ChangedGuarantee {
        id: "authorization.dispatch".into(),
        previous: Some(GuaranteeStatement {
            requirement_id: "authorization.requirement".into(),
            form: previous_form,
        }),
        current: Some(GuaranteeStatement {
            requirement_id: "authorization.requirement".into(),
            form: StatementForm::Finite {
                expression: authorized_first(),
            },
        }),
        change: GuaranteeChange::Weakened,
        affected_claims: vec!["claim.authorization".into()],
        interfaces: vec!["effect.dispatch".into()],
        configurations: vec![digest('c')],
        unknown: None,
    });
    authorization.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        true,
    ));

    let mut retry = footprint("retry", vec![simple_revision("retry.requirement", '4')]);
    retry.assumptions.push(ConsumedAssumption {
        id: "retry.authorization".into(),
        requirement_id: "retry.requirement".into(),
        consumed_change: "authorization".into(),
        consumed_guarantee: "authorization.dispatch".into(),
        consumed_identity: previous_identity,
        dependency: AssumptionDependency::Monotone,
        meaning: "authorization is established immediately before every attempted effect".into(),
        finite_predicate: Some(forall_attempt()),
        evidence_basis: vec!["claim.retry".into()],
        configurations: vec![digest('c')],
        unknown: None,
    });
    retry.invariants.push(PreservedInvariant {
        id: "authorization-before-effect".into(),
        requirement_id: "retry.requirement".into(),
        commitment: StatementForm::Informal {
            statement: "authorization precedes every attempted effect".into(),
        },
        interfaces: vec!["effect.dispatch".into()],
        configurations: vec![digest('c')],
        under_assumptions: vec!["retry.authorization".into()],
        required_conditions: vec![IntegrationCondition {
            id: "revalidate-authorization-per-attempt".into(),
            statement: "retry must revalidate authorization immediately before each retry attempt"
                .into(),
        }],
    });
    retry.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        true,
    ));

    case(
        context(),
        vec![authorization, retry],
        vec![composition(
            "authorization-then-retry",
            &["authorization", "retry"],
            &["authorization", "retry"],
            true,
            true,
        )],
    )
}

fn invalidation_case() -> ConcurrentChangeCase {
    let mut gamma = footprint("gamma", vec![simple_revision("gamma.requirement", '5')]);
    gamma.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        false,
    ));
    gamma.invalidated.push(InvalidatedObligation {
        id: "gamma.invalidation".into(),
        requirement_id: "gamma.requirement".into(),
        obligation: "delta.retained-obligation".into(),
        interfaces: vec!["effect.dispatch".into()],
        configurations: vec![digest('c')],
        reason: "the revised requirement replaces the retained obligation".into(),
    });

    let mut delta = footprint("delta", vec![simple_revision("delta.requirement", '6')]);
    delta.interfaces.push(interface(
        "effect.dispatch",
        &digest('c'),
        InterfaceChange::Modify,
        false,
    ));
    delta.evidence_basis.push(EvidenceBasis {
        id: "delta.basis".into(),
        requirement_id: "delta.requirement".into(),
        obligations: vec!["delta.retained-obligation".into()],
        claims: vec!["claim.delta".into()],
        assumptions: Vec::new(),
        configurations: vec![digest('c')],
    });

    case(
        context(),
        vec![gamma, delta],
        vec![composition(
            "gamma-then-delta",
            &["gamma", "delta"],
            &["gamma", "delta"],
            true,
            true,
        )],
    )
}

#[test]
// r[verify cairn.concurrent_footprints.derivation]
fn disjoint_changes_interacting_through_assumption_emit_joint_obligation() {
    let report = analyze_concurrent_changes(&policy(), &compatible_case()).unwrap();
    assert!(report.advisory);
    assert_eq!(report.obligations.len(), 1);
    assert_eq!(report.interactions.len(), 1);
    let interaction = &report.interactions[0];
    assert_eq!(
        interaction.kind,
        InteractionKind::ChangedGuaranteeAffectsAssumption
    );
    assert_eq!(interaction.producer_change, "alpha");
    assert_eq!(interaction.consumer_change, "beta");
    let check = interaction.finite.as_ref().unwrap();
    assert_eq!(check.previous_implies_assumption, Some(true));
    assert_eq!(check.current_implies_assumption, Some(true));
    assert_eq!(
        report.obligations[0].outcome,
        ObligationOutcome::NeedsJointEvidence
    );
}

#[test]
// r[verify cairn.concurrent_footprints.explanation]
fn report_explains_the_path_from_guarantee_to_assumption_to_obligation() {
    let report = analyze_concurrent_changes(&policy(), &compatible_case()).unwrap();
    let path = &report.obligations[0].path;
    let kinds: Vec<&str> = path.iter().map(|step| step.kind.as_str()).collect();
    assert_eq!(kinds[0], "guarantee_change");
    assert_eq!(kinds[1], "assumption_consumption");
    assert!(kinds.contains(&"finite_check"));
    assert_eq!(kinds.last().copied(), Some("outcome"));
    let detail = path
        .iter()
        .map(|step| step.detail.clone())
        .collect::<Vec<String>>()
        .join("\n");
    assert!(detail.contains("the feature flag stays enabled"));
    assert!(detail.contains("alpha.requirement"), "{detail}");
    assert!(detail.contains("current_implies_assumption=Some(true)"));
}

#[test]
// r[verify cairn.concurrent_footprints.outcomes]
fn compatible_fixture_names_assumptions_meaning_evidence_and_composition() {
    let mut case = compatible_case();
    let first = analyze_concurrent_changes(&policy(), &case).unwrap();
    attach_evidence(
        &mut case,
        &first,
        EvidenceScope::AllAdmittedCompositions,
        supported_propositions(),
    );
    let report = analyze_concurrent_changes(&policy(), &case).unwrap();
    let obligation = &report.obligations[0];
    assert_eq!(
        obligation.outcome,
        ObligationOutcome::SupportedWithinDeclaredScope
    );
    assert_eq!(
        obligation.propositions.coexistence,
        PropositionState::Supported
    );
    assert_eq!(
        obligation.propositions.invariant_preservation,
        PropositionState::Supported
    );
    assert_eq!(
        obligation.propositions.commutativity,
        PropositionState::NotApplicable
    );
    assert_eq!(
        obligation.evidence.admitted,
        vec!["joint.evidence".to_string()]
    );
    assert_eq!(obligation.candidate.compositions.len(), 1);
    assert_eq!(obligation.candidate.compositions[0].id, "c1");
    assert_eq!(
        obligation.candidate.compositions[0].order,
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert!(obligation.outcome_basis.contains("joint.evidence"));
    assert!(obligation.outcome_basis.contains("c1"));
    assert!(report.individual_receipts_do_not_discharge);
}

#[test]
// r[verify cairn.concurrent_footprints.outcomes]
fn individually_valid_receipts_do_not_discharge_the_combined_obligation() {
    let mut case = compatible_case();
    case.individual_receipts = vec![
        IndividualReceipt {
            id: "alpha.receipt".into(),
            change: "alpha".into(),
            claim: "claim.alpha".into(),
            requirement_id: "alpha.requirement".into(),
            evidence_blake3: digest('7'),
            receipt_blake3: digest('8'),
        },
        IndividualReceipt {
            id: "beta.receipt".into(),
            change: "beta".into(),
            claim: "claim.beta".into(),
            requirement_id: "beta.requirement".into(),
            evidence_blake3: digest('9'),
            receipt_blake3: digest('0'),
        },
    ];
    let report = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert_eq!(
        report.obligations[0].outcome,
        ObligationOutcome::NeedsJointEvidence
    );
    assert!(report.individual_receipts_do_not_discharge);
    assert!(report
        .individual_receipts
        .iter()
        .all(|receipt| !receipt.discharges_combined_obligation && receipt.change_known));
}

#[test]
// r[verify cairn.concurrent_footprints.derivation]
// r[verify cairn.concurrent_footprints.outcomes]
fn retry_authorization_fixture_names_invariant_and_missing_integration_condition() {
    let report = analyze_concurrent_changes(&policy(), &retry_case()).unwrap();
    assert_eq!(report.obligations.len(), 1);
    let obligation = &report.obligations[0];
    assert_eq!(obligation.outcome, ObligationOutcome::Conflict);
    assert!(obligation
        .affected_invariants
        .contains(&"authorization-before-effect".to_string()));
    let missing = obligation
        .integration_conditions
        .iter()
        .find(|condition| condition.id == "revalidate-authorization-per-attempt")
        .unwrap();
    assert_eq!(missing.state, "missing");
    assert!(missing.provided_by.is_empty());
    assert!(obligation
        .outcome_basis
        .contains("previous guarantee established assumption retry.authorization"));
    let witness = report.interactions[0]
        .finite
        .as_ref()
        .and_then(|check| check.witness.as_ref())
        .expect("a distinguishing finite observation table is retained");
    let rows = witness.observables.get("authorized").unwrap();
    let first = rows
        .iter()
        .find(|row| row.arguments[0].value == "first")
        .unwrap();
    let second = rows
        .iter()
        .find(|row| row.arguments[0].value == "second")
        .unwrap();
    assert_eq!(first.value.value, "true");
    assert_eq!(second.value.value, "false");
}

#[test]
// r[verify cairn.concurrent_footprints.obligation]
// r[verify cairn.concurrent_footprints.outcomes]
fn order_sensitive_composition_keeps_commutativity_separate() {
    let mut case = compatible_case();
    case.compositions.push(composition(
        "c2",
        &["alpha", "beta"],
        &["beta", "alpha"],
        false,
        true,
    ));
    for change in &mut case.changes {
        if let Some(interface) = change.interfaces.first_mut() {
            interface.order_sensitive = true;
        }
    }
    let first = analyze_concurrent_changes(&policy(), &case).unwrap();
    let obligation = &first.obligations[0];
    assert!(obligation.order_sensitive);
    assert!(obligation.commutativity_required);
    assert_eq!(
        obligation.propositions.commutativity,
        PropositionState::NotEstablished
    );
    assert_eq!(obligation.outcome, ObligationOutcome::NeedsJointEvidence);

    attach_evidence(
        &mut case,
        &first,
        EvidenceScope::AllAdmittedCompositions,
        supported_propositions(),
    );
    let second = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert_eq!(
        second.obligations[0].outcome,
        ObligationOutcome::NeedsJointEvidence,
        "invariant preservation must not imply commutativity"
    );
    assert_eq!(
        second.obligations[0].propositions.invariant_preservation,
        PropositionState::Supported
    );
    assert_eq!(
        second.obligations[0].propositions.commutativity,
        PropositionState::NotEstablished
    );

    case.joint_evidence[0]
        .propositions
        .push(PropositionEvidence {
            proposition: PropositionKind::Commutativity,
            state: PropositionState::Supported,
            basis: "both declared orders were checked".into(),
        });
    let third = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert_eq!(
        third.obligations[0].outcome,
        ObligationOutcome::SupportedWithinDeclaredScope
    );
    assert_eq!(
        third.obligations[0].propositions.commutativity,
        PropositionState::Supported
    );
}

#[test]
// r[verify cairn.concurrent_footprints.outcomes]
fn satisfiability_witness_cannot_establish_preservation_over_admitted_compositions() {
    let mut case = compatible_case();
    let first = analyze_concurrent_changes(&policy(), &case).unwrap();
    attach_evidence(
        &mut case,
        &first,
        EvidenceScope::Witness,
        supported_propositions(),
    );
    let report = analyze_concurrent_changes(&policy(), &case).unwrap();
    let obligation = &report.obligations[0];
    assert_eq!(obligation.outcome, ObligationOutcome::NeedsJointEvidence);
    assert_eq!(
        obligation.propositions.invariant_preservation,
        PropositionState::NotEstablished
    );
    assert!(obligation.evidence.admitted.is_empty());
    assert!(obligation
        .evidence
        .insufficient
        .iter()
        .any(|rejection| rejection
            .reasons
            .contains(&"witness_scope_insufficient".to_string())));
}

#[test]
// r[verify cairn.concurrent_footprints.derivation]
fn invalidation_of_another_changes_evidence_basis_needs_joint_evidence() {
    let report = analyze_concurrent_changes(&policy(), &invalidation_case()).unwrap();
    assert_eq!(report.obligations.len(), 1);
    let obligation = &report.obligations[0];
    assert_eq!(obligation.outcome, ObligationOutcome::NeedsJointEvidence);
    assert_eq!(obligation.affected_claims, vec!["claim.delta".to_string()]);
    let interaction = report
        .interactions
        .iter()
        .find(|interaction| interaction.kind == InteractionKind::InvalidationAffectsEvidenceBasis)
        .unwrap();
    assert_eq!(interaction.producer_change, "gamma");
    assert_eq!(interaction.consumer_change, "delta");
    assert_eq!(
        interaction.invalidation.as_deref(),
        Some("gamma.invalidation")
    );
    assert_eq!(interaction.evidence_basis.as_deref(), Some("delta.basis"));
}

#[test]
// r[verify cairn.concurrent_footprints.invalidation]
fn changed_participants_dependencies_configuration_or_composition_invalidate_prior_applicability() {
    let mut case = invalidation_case();
    let report = analyze_concurrent_changes(&policy(), &case).unwrap();
    let fingerprint = report.obligations[0].fingerprint.clone();
    assert_eq!(report.obligations[0].id, fingerprint.obligation_id);

    case.prior_obligations = vec![fingerprint.clone()];
    let unchanged = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(unchanged.prior_applicability[0].applicable);

    let mut participants = fingerprint.clone();
    participants
        .participants
        .insert("delta".into(), "intent-delta-revised".into());
    case.prior_obligations = vec![participants];
    let changed = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(!changed.prior_applicability[0].applicable);
    assert!(changed.prior_applicability[0]
        .reasons
        .contains(&"participants_changed".to_string()));

    let mut dependencies = fingerprint.clone();
    dependencies.dependencies.insert(
        "invalidation.gamma.gamma.invalidation".into(),
        "other.obligation".into(),
    );
    case.prior_obligations = vec![dependencies];
    let changed = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(changed.prior_applicability[0]
        .reasons
        .contains(&"dependencies_changed".to_string()));

    let mut configurations = fingerprint.clone();
    configurations
        .configurations
        .insert("effect.dispatch".into(), digest('d'));
    case.prior_obligations = vec![configurations];
    let changed = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(changed.prior_applicability[0]
        .reasons
        .contains(&"configuration_changed".to_string()));

    let mut composition = fingerprint.clone();
    composition.composition = vec!["another:order:true".into()];
    case.prior_obligations = vec![composition];
    let changed = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(changed.prior_applicability[0]
        .reasons
        .contains(&"composition_changed".to_string()));

    let mut absent = fingerprint.clone();
    absent.obligation_id = "absent.obligation".into();
    case.prior_obligations = vec![absent];
    let changed = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(changed.prior_applicability[0]
        .reasons
        .contains(&"obligation_absent".to_string()));
}

#[test]
// r[verify cairn.concurrent_footprints.outcomes]
fn unknown_scope_unsupported_interpretation_and_bounds_stay_explicit() {
    let mut unknown = compatible_case();
    unknown.changes[0].unknown.push(UnknownScope {
        scope: "scheduler interaction".into(),
        kind: UnknownKind::IncompleteFootprint,
        detail: "scheduling behaviour is not declared".into(),
    });
    let report = analyze_concurrent_changes(&policy(), &unknown).unwrap();
    assert_eq!(report.obligations[0].outcome, ObligationOutcome::Unknown);
    assert!(!report.declaration_complete);
    assert_eq!(report.scoped_unknown.len(), 1);

    let mut unsupported = compatible_case();
    unsupported.changes[1].assumptions[0].finite_predicate = Some(FiniteExpr::Unsupported {
        construct: "affine-arithmetic".into(),
    });
    let report = analyze_concurrent_changes(&policy(), &unsupported).unwrap();
    assert_eq!(report.obligations[0].outcome, ObligationOutcome::Unknown);
    assert!(report.obligations[0]
        .outcome_basis
        .contains("unsupported finite interpretation"));

    let mut bounded = policy();
    bounded.bounds.max_worlds = 1;
    let report = analyze_concurrent_changes(&bounded, &compatible_case()).unwrap();
    assert_eq!(report.obligations[0].outcome, ObligationOutcome::Unknown);
    assert!(!report.exploration.bound_exhausted.is_empty());
    assert!(!report.exploration.complete);
}

#[test]
// r[verify cairn.concurrent_footprints.footprint]
fn incomplete_declaration_is_reported_instead_of_silently_ignored() {
    let mut case = compatible_case();
    case.changes[1].assumptions[0].consumed_change = "not-declared".into();
    let report = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert!(report.obligations.is_empty());
    assert_eq!(report.gaps.len(), 1);
    assert_eq!(report.gaps[0].kind, "unresolved_change");
    assert!(!report.declaration_complete);
    assert!(!report.footprints.iter().all(|footprint| footprint.complete));
}

#[test]
// r[verify cairn.concurrent_footprints.obligation]
fn interface_configuration_and_operation_conflicts_are_explicit() {
    let mut mismatch = compatible_case();
    mismatch.changes[1].interfaces[0].configuration_blake3 = digest('d');
    mismatch.context.configurations.clear();
    let report = analyze_concurrent_changes(&policy(), &mismatch).unwrap();
    assert_eq!(report.obligations[0].outcome, ObligationOutcome::Conflict);
    assert!(report
        .interactions
        .iter()
        .any(|interaction| interaction.kind == InteractionKind::InterfaceConfigurationMismatch));

    let mut operation = compatible_case();
    operation.changes[0].interfaces[0].change_kind = InterfaceChange::Remove;
    operation.changes[1].interfaces[0].change_kind = InterfaceChange::Modify;
    let report = analyze_concurrent_changes(&policy(), &operation).unwrap();
    assert_eq!(report.obligations[0].outcome, ObligationOutcome::Conflict);
    assert!(report
        .interactions
        .iter()
        .any(|interaction| interaction.kind == InteractionKind::InterfaceOperationConflict));
}

#[test]
// r[verify cairn.concurrent_footprints.footprint]
fn malformed_declarations_are_rejected() {
    let mut unsupported = compatible_case();
    unsupported.schema = "cairn.unknown-case.v1".into();
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &unsupported),
        Err(FootprintError::Unsupported(_))
    ));

    let mut duplicate = compatible_case();
    let extra = duplicate.changes[0].clone();
    duplicate.changes.push(extra);
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &duplicate),
        Err(FootprintError::InvalidInput(_))
    ));

    let mut tampered = compatible_case();
    tampered.changes[0].revisions[0].contract_blake3 = digest('9');
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &tampered),
        Err(FootprintError::RevisionMismatch(_))
    ));

    let mut self_consuming = compatible_case();
    self_consuming.changes[1].assumptions[0].consumed_change = "beta".into();
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &self_consuming),
        Err(FootprintError::InvalidInput(_))
    ));

    let mut bad_order = compatible_case();
    bad_order.compositions[0].order = vec!["alpha".into(), "alpha".into()];
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &bad_order),
        Err(FootprintError::InvalidInput(_))
    ));

    let mut bad_evidence = compatible_case();
    bad_evidence.joint_evidence.push(JointEvidence {
        id: "bad.evidence".into(),
        obligation: "unused".into(),
        combined_candidate_blake3: digest('1'),
        participants: vec!["alpha".into(), "beta".into()],
        compositions: Vec::new(),
        context: context(),
        affected_claims: Vec::new(),
        propositions: Vec::new(),
        assurance: AssuranceScope::Reviewed,
        scope: EvidenceScope::DeclaredComposition,
        artifact_blake3: "not-a-digest".into(),
    });
    assert!(matches!(
        analyze_concurrent_changes(&policy(), &bad_evidence),
        Err(FootprintError::InvalidInput(_))
    ));

    let mut empty_bounds = policy();
    empty_bounds.bounds.max_worlds = 0;
    assert!(matches!(
        analyze_concurrent_changes(&empty_bounds, &compatible_case()),
        Err(FootprintError::BoundExhaustion(_))
    ));
}

#[test]
// r[verify cairn.concurrent_footprints.bounds]
fn identical_inputs_produce_identical_reports_and_identities() {
    let case = compatible_case();
    let first = analyze_concurrent_changes(&policy(), &case).unwrap();
    let second = analyze_concurrent_changes(&policy(), &case).unwrap();
    assert_eq!(first.input_blake3, second.input_blake3);
    assert_eq!(first.policy_blake3, second.policy_blake3);
    assert_eq!(first.report_blake3, second.report_blake3);
    assert_eq!(first.obligations[0].id, second.obligations[0].id);
    assert_eq!(
        first.obligations[0].candidate.combined_candidate_blake3,
        second.obligations[0].candidate.combined_candidate_blake3
    );
}

#[test]
// r[verify cairn.concurrent_footprints.independence]
fn textual_collaboration_and_verification_impact_reports_remain_independent() {
    // These references are compile-time evidence that the existing reports
    // keep their own public surfaces and schemas.
    let _inspection: Option<crate::verified::collaboration_resolution::Inspection> = None;
    assert_eq!(
        crate::verified::verification_impact::VERIFICATION_IMPACT_SCHEMA_V1,
        "cairn.verification-impact.v1"
    );
    let report = analyze_concurrent_changes(&policy(), &compatible_case()).unwrap();
    assert_eq!(report.schema, REPORT_SCHEMA_V1);
    assert!(report
        .independent_reports
        .contains(&"textual_collaboration_conflict_resolution".to_string()));
    assert!(report
        .independent_reports
        .contains(&"verification_impact".to_string()));
    assert!(report.boundary.contains("remain independently usable"));
}
