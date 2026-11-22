// r[impl cairn.revision_classification.conformance]
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::verified::content_bound_requirement_registry::{
    ContentBoundRequirementRow, ContentBoundSourceSpan,
};
use crate::verified::scoped_claim_derivations::{
    ClaimContext, Proposition, ScopedClaim, CLAIM_SCHEMA_V1,
};
use crate::verified::typed_requirement_contracts::{
    FiniteDomain, FiniteExpr, FiniteTerm, FiniteValue, NormalizedContract, ObservableMapping,
    SemanticContract, SemanticProfile, FINITE_INTERPRETATION_V1, FINITE_SEMANTICS_PROFILE_V1,
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

fn test_policy() -> RevisionClassificationPolicy {
    RevisionClassificationPolicy {
        schema: REVISION_CLASSIFICATION_POLICY_SCHEMA_V1.into(),
        comparison: ComparisonBounds {
            max_domains: 8,
            max_observables: 8,
            max_domain_values: 8,
            max_worlds: 1024,
            max_steps: 100_000,
            max_evidence: 8,
        },
        policy_fragment: PolicyFragmentBounds {
            max_checks: 8,
            max_roles: 8,
            max_assumptions: 8,
            max_assumption_sets: 16,
            max_accepted: 1024,
        },
        transfer: TransferPolicy {
            enabled: false,
            admission_rules: Vec::new(),
        },
    }
}

fn row(id: &str, seed: char) -> ContentBoundRequirementRow {
    ContentBoundRequirementRow {
        requirement_id: id.into(),
        specification_path: ".cairn/specs/revision/spec.md".into(),
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

fn domain(values: &[&str]) -> FiniteDomain {
    FiniteDomain {
        values: values.iter().map(|value| (*value).into()).collect(),
    }
}

fn observable() -> ObservableMapping {
    ObservableMapping {
        arguments: Vec::new(),
        result: "bool".into(),
        phase: "state".into(),
        source: "test observable".into(),
        projection: "test projection".into(),
    }
}

fn contract(
    row: ContentBoundRequirementRow,
    guarantee: FiniteExpr,
    assumptions: Vec<FiniteExpr>,
    domains: BTreeMap<String, FiniteDomain>,
    observables: BTreeMap<String, ObservableMapping>,
) -> SemanticContract {
    SemanticContract {
        profile: FINITE_SEMANTICS_PROFILE_V1.into(),
        proposed_by: "revision-classification-test".into(),
        requirement: row,
        domains,
        observables,
        assumptions,
        activation: FiniteExpr::Bool { value: true },
        guarantee,
        supported_portion: "finite test portion".into(),
        informal_remainder: "test remainder".into(),
    }
}

fn base_contracts(
    guarantee: FiniteExpr,
) -> (
    ContentBoundRequirementRow,
    SemanticContract,
    ContentBoundRequirementRow,
    SemanticContract,
) {
    let old_row = row("test.requirement", 'a');
    let new_row = row("test.requirement", 'b');
    let domains = BTreeMap::from([("bool".to_string(), bool_domain())]);
    let observables = BTreeMap::from([("flag".to_string(), observable())]);
    let old_contract = contract(
        old_row.clone(),
        guarantee.clone(),
        Vec::new(),
        domains.clone(),
        observables.clone(),
    );
    let new_contract = contract(new_row.clone(), guarantee, Vec::new(), domains, observables);
    (old_row, old_contract, new_row, new_contract)
}

fn is_true(name: &str) -> FiniteExpr {
    FiniteExpr::Equal {
        left: FiniteTerm::Observable {
            mapping: name.into(),
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

fn same_mapping(names: &[&str]) -> RevisionMapping {
    let domains = BTreeMap::from([(
        "bool".to_string(),
        DomainCorrespondence {
            old_domain: Some("bool".into()),
            new_domain: Some("bool".into()),
            values: vec![
                ValueCorrespondence {
                    common: "false".into(),
                    old: Some("false".into()),
                    new: Some("false".into()),
                },
                ValueCorrespondence {
                    common: "true".into(),
                    old: Some("true".into()),
                    new: Some("true".into()),
                },
            ],
        },
    )]);
    let observables = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                ObservableCorrespondence {
                    old: Some((*name).into()),
                    new: Some((*name).into()),
                    arguments: Vec::new(),
                    result: "bool".into(),
                },
            )
        })
        .collect();
    RevisionMapping {
        domains,
        observables,
    }
}

fn context() -> RevisionContext {
    RevisionContext {
        subject_blake3: digest('c'),
        policy_blake3: digest('d'),
        target: "test-target".into(),
        environment: "test-environment".into(),
    }
}

fn binding(
    old_row: &ContentBoundRequirementRow,
    old_contract: &SemanticContract,
    new_row: &ContentBoundRequirementRow,
    new_contract: &SemanticContract,
) -> (NormalizedContract, NormalizedContract, RevisionBinding) {
    let old = normalize_contract(&profile(), old_row, old_contract).unwrap();
    let new = normalize_contract(&profile(), new_row, new_contract).unwrap();
    let binding = RevisionBinding {
        old_requirement_row_blake3: old.contract.requirement.row_blake3.clone(),
        new_requirement_row_blake3: new.contract.requirement.row_blake3.clone(),
        old_interpretation_blake3: old.interpretation_blake3.clone(),
        new_interpretation_blake3: new.interpretation_blake3.clone(),
        old_mapping_blake3: old.mapping_blake3.clone(),
        new_mapping_blake3: new.mapping_blake3.clone(),
        old_contract_blake3: old.contract_blake3.clone(),
        new_contract_blake3: new.contract_blake3.clone(),
    };
    (old, new, binding)
}

fn request(
    old_row: &ContentBoundRequirementRow,
    old_contract: &SemanticContract,
    new_row: &ContentBoundRequirementRow,
    new_contract: &SemanticContract,
    mapping: &RevisionMapping,
    retained_evidence: Vec<RetainedEvidence>,
    correspondence: CorrespondenceInputs,
) -> RequirementRevisionRequest {
    let (_, _, binding) = binding(old_row, old_contract, new_row, new_contract);
    RequirementRevisionRequest {
        schema: REQUIREMENT_REVISION_REQUEST_SCHEMA_V1.into(),
        profile: profile(),
        old_native_requirement: old_row.clone(),
        new_native_requirement: new_row.clone(),
        old_contract: old_contract.clone(),
        new_contract: new_contract.clone(),
        binding,
        context: context(),
        mapping: mapping.clone(),
        correspondence,
        retained_evidence,
    }
}

fn compare(
    old_row: &ContentBoundRequirementRow,
    old_contract: &SemanticContract,
    new_row: &ContentBoundRequirementRow,
    new_contract: &SemanticContract,
    mapping: &RevisionMapping,
    retained_evidence: Vec<RetainedEvidence>,
    correspondence: CorrespondenceInputs,
) -> RequirementComparisonReport {
    compare_requirement_revisions(
        &test_policy(),
        &request(
            old_row,
            old_contract,
            new_row,
            new_contract,
            mapping,
            retained_evidence,
            correspondence,
        ),
    )
    .unwrap()
}

fn no_correspondence() -> CorrespondenceInputs {
    CorrespondenceInputs {
        old: None,
        new: None,
        authorized_reviewers: Vec::new(),
    }
}

fn claim(normalized: &NormalizedContract, proposition: Proposition) -> RetainedEvidence {
    RetainedEvidence {
        evidence_blake3: digest('e'),
        receipt_blake3: digest('f'),
        claim: ScopedClaim {
            schema: CLAIM_SCHEMA_V1.into(),
            kind: proposition.claim_kind(),
            requirement: normalized.contract.requirement.clone(),
            interpretation_blake3: normalized.interpretation_blake3.clone(),
            contract_blake3: normalized.contract_blake3.clone(),
            context: ClaimContext {
                subject_blake3: context().subject_blake3,
                policy_blake3: context().policy_blake3,
                target: context().target,
                environment: context().environment,
                bounds: BTreeMap::from([("finite-worlds".into(), "1".into())]),
            },
            proposition,
        },
    }
}

fn observation() -> Proposition {
    Proposition::CheckPassed {
        check_id: "check-a".into(),
    }
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
// r[verify cairn.revision_classification.conformance]
fn editorial_revision_is_equivalent_without_structure_change() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Editorial);
    assert_eq!(report.relation, RevisionRelation::Equivalent);
    assert!(report.structural.as_ref().unwrap().editorial_eligible);
    assert_eq!(
        report.implications[0].holds,
        Some(true),
        "new must imply old"
    );
    assert_eq!(
        report.implications[1].holds,
        Some(true),
        "old must imply new"
    );
    assert!(report.search.as_ref().unwrap().complete);
    assert_eq!(
        report.transfer.automated_transfer,
        "disabled_until_separately_qualified"
    );
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
fn clarifying_revision_is_equivalent_but_structurally_different() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mut old_contract = old_contract;
    let mut new_contract = new_contract;
    old_contract.domains.insert("tag".into(), domain(&["a"]));
    new_contract
        .domains
        .insert("tag".into(), domain(&["a", "b"]));
    let mut mapping = same_mapping(&["flag"]);
    mapping.domains.insert(
        "tag".into(),
        DomainCorrespondence {
            old_domain: Some("tag".into()),
            new_domain: Some("tag".into()),
            values: vec![
                ValueCorrespondence {
                    common: "a".into(),
                    old: Some("a".into()),
                    new: Some("a".into()),
                },
                ValueCorrespondence {
                    common: "b".into(),
                    old: None,
                    new: Some("b".into()),
                },
            ],
        },
    );
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Clarifying);
    assert_eq!(report.relation, RevisionRelation::Equivalent);
    assert!(report.structural.as_ref().unwrap().domains_changed);
    assert!(!report.structural.as_ref().unwrap().editorial_eligible);
}

#[test]
// r[verify cairn.revision_classification.evidence_reuse]
fn strengthening_change_refuses_stale_evidence_reuse() {
    let (old_row, old_contract, new_row, new_contract) =
        base_contracts(FiniteExpr::Bool { value: true });
    let mut new_contract = new_contract;
    new_contract.guarantee = is_true("flag");
    let mapping = same_mapping(&["flag"]);
    let (normalized_old, _, _) = binding(&old_row, &old_contract, &new_row, &new_contract);
    let evidence = claim(&normalized_old, observation());
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        vec![evidence],
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Strengthening);
    assert_eq!(report.relation, RevisionRelation::Stronger);
    let decision = &report.evidence_reuse[0];
    assert_eq!(decision.decision, ReuseDecision::NotReusable);
    assert!(!decision.old_receipt_current);
    assert!(!decision.transfer_allowed);
    assert!(decision.reason.contains("strengthening"));
}

#[test]
// r[verify cairn.revision_classification.evidence_reuse]
fn weakening_change_is_not_silently_promoted() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mut new_contract = new_contract;
    new_contract.guarantee = FiniteExpr::Bool { value: true };
    let mapping = same_mapping(&["flag"]);
    let (normalized_old, _, _) = binding(&old_row, &old_contract, &new_row, &new_contract);
    let evidence = claim(&normalized_old, observation());
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        vec![evidence],
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Weakening);
    assert_eq!(report.relation, RevisionRelation::Weaker);
    let decision = &report.evidence_reuse[0];
    assert_eq!(decision.decision, ReuseDecision::NotReusable);
    assert!(decision.reason.contains("weakens"));
}

#[test]
// r[verify cairn.revision_classification.evidence_reuse]
fn equivalent_change_reuses_only_for_a_fresh_derivation() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let (normalized_old, _, _) = binding(&old_row, &old_contract, &new_row, &new_contract);
    let evidence = claim(&normalized_old, observation());
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        vec![evidence],
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Editorial);
    let decision = &report.evidence_reuse[0];
    assert_eq!(decision.decision, ReuseDecision::Reusable);
    assert!(decision.requires_new_derivation);
    assert!(!decision.old_receipt_current);
    assert!(decision.reason.contains("not current"));
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
fn incompatible_revision_retains_replayable_witnesses() {
    let old_row = row("test.requirement", 'a');
    let new_row = row("test.requirement", 'b');
    let domains = BTreeMap::from([("bool".to_string(), bool_domain())]);
    let observables = BTreeMap::from([
        ("left".to_string(), observable()),
        ("right".to_string(), observable()),
    ]);
    let old_contract = contract(
        old_row.clone(),
        is_true("left"),
        Vec::new(),
        domains.clone(),
        observables.clone(),
    );
    let new_contract = contract(
        new_row.clone(),
        is_true("right"),
        Vec::new(),
        domains,
        observables,
    );
    let mapping = same_mapping(&["left", "right"]);
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Incompatible);
    assert_eq!(report.relation, RevisionRelation::Incomparable);
    for implication in &report.implications {
        assert_eq!(implication.holds, Some(false));
        let witness = implication.witness.as_ref().unwrap();
        let replay = replay_requirement_witness(
            &test_policy(),
            &profile(),
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            witness,
        )
        .unwrap();
        assert!(replay.distinguishing);

        let mut tampered = (*witness).clone();
        tampered.world_blake3 = digest('0');
        assert!(replay_requirement_witness(
            &test_policy(),
            &profile(),
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            &tampered,
        )
        .is_err());
    }
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
fn argument_observables_produce_replayable_witnesses() {
    let old_row = row("test.requirement", 'a');
    let new_row = row("test.requirement", 'b');
    let domains = BTreeMap::from([
        ("bool".to_string(), bool_domain()),
        ("bit".to_string(), domain(&["0", "1"])),
    ]);
    let observables = BTreeMap::from([(
        "flag".to_string(),
        ObservableMapping {
            arguments: vec!["bit".into()],
            result: "bool".into(),
            phase: "state".into(),
            source: "argument test".into(),
            projection: "provide one bit argument".into(),
        },
    )]);
    let guarantee = |argument: &str| FiniteExpr::Equal {
        left: FiniteTerm::Observable {
            mapping: "flag".into(),
            arguments: vec![FiniteTerm::Literal {
                value: FiniteValue {
                    domain: "bit".into(),
                    value: argument.into(),
                },
            }],
        },
        right: FiniteTerm::Literal {
            value: FiniteValue {
                domain: "bool".into(),
                value: "true".into(),
            },
        },
    };
    let old_contract = contract(
        old_row.clone(),
        guarantee("0"),
        Vec::new(),
        domains.clone(),
        observables.clone(),
    );
    let new_contract = contract(
        new_row.clone(),
        guarantee("1"),
        Vec::new(),
        domains,
        observables,
    );
    let mapping = RevisionMapping {
        domains: BTreeMap::from([
            (
                "bool".into(),
                DomainCorrespondence {
                    old_domain: Some("bool".into()),
                    new_domain: Some("bool".into()),
                    values: vec![
                        ValueCorrespondence {
                            common: "false".into(),
                            old: Some("false".into()),
                            new: Some("false".into()),
                        },
                        ValueCorrespondence {
                            common: "true".into(),
                            old: Some("true".into()),
                            new: Some("true".into()),
                        },
                    ],
                },
            ),
            (
                "bit".into(),
                DomainCorrespondence {
                    old_domain: Some("bit".into()),
                    new_domain: Some("bit".into()),
                    values: vec![
                        ValueCorrespondence {
                            common: "0".into(),
                            old: Some("0".into()),
                            new: Some("0".into()),
                        },
                        ValueCorrespondence {
                            common: "1".into(),
                            old: Some("1".into()),
                            new: Some("1".into()),
                        },
                    ],
                },
            ),
        ]),
        observables: BTreeMap::from([(
            "flag".into(),
            ObservableCorrespondence {
                old: Some("flag".into()),
                new: Some("flag".into()),
                arguments: vec!["bit".into()],
                result: "bool".into(),
            },
        )]),
    };
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Incompatible);
    assert_eq!(report.implications.len(), 2);
    for implication in &report.implications {
        let witness = implication.witness.as_ref().unwrap();
        assert!(!witness.world.observables["flag"].is_empty());
        let replay = replay_requirement_witness(
            &test_policy(),
            &profile(),
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            witness,
        )
        .unwrap();
        assert!(replay.distinguishing);
    }
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
fn unsupported_mapping_is_never_editorial() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = RevisionMapping {
        domains: same_mapping(&["flag"]).domains,
        observables: BTreeMap::new(),
    };
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Incompatible);
    assert_eq!(
        report.unsupported.as_ref().unwrap().code,
        "unmapped_observable"
    );
    assert_ne!(report.classification, RevisionClass::Editorial);
}

#[test]
// r[verify cairn.revision_classification.assumption_guarantee]
fn assumption_strengthening_is_not_hidden_by_the_obligation_label() {
    let (old_row, old_contract, new_row, new_contract) =
        base_contracts(FiniteExpr::Bool { value: false });
    let mut new_contract = new_contract;
    new_contract.assumptions = vec![is_true("flag")];
    let mapping = same_mapping(&["flag"]);
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(report.classification, RevisionClass::Weakening);
    let delta = report.assumption_guarantee.as_ref().unwrap();
    assert_eq!(delta.assumptions, FormulaRelation::Strengthened);
    assert_eq!(delta.obligation, RevisionRelation::Weaker);
    assert!(delta.guarantee_structurally_equal);
    assert!(delta.note.contains("stronger environmental assumptions"));
}

#[test]
// r[verify cairn.revision_classification.bounds]
fn step_bound_keeps_the_search_conclusion_undetermined() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let mut policy = test_policy();
    policy.comparison.max_steps = 1;
    let report = compare_requirement_revisions(
        &policy,
        &request(
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            Vec::new(),
            no_correspondence(),
        ),
    )
    .unwrap();
    assert_eq!(report.classification, RevisionClass::Undetermined);
    assert_eq!(report.relation, RevisionRelation::Unknown);
    assert!(!report.search.as_ref().unwrap().complete);
    assert!(report.implications.iter().all(|item| item.holds.is_none()));
    assert_eq!(report.inconclusive.as_ref().unwrap().code, "search_bound");
}

#[test]
// r[verify cairn.revision_classification.bounds]
fn world_bound_makes_the_comparison_undetermined() {
    let old_row = row("test.requirement", 'a');
    let new_row = row("test.requirement", 'b');
    let domains = BTreeMap::from([("bool".to_string(), bool_domain())]);
    let observables = BTreeMap::from([
        ("left".to_string(), observable()),
        ("right".to_string(), observable()),
    ]);
    let old_contract = contract(
        old_row.clone(),
        is_true("left"),
        Vec::new(),
        domains.clone(),
        observables.clone(),
    );
    let new_contract = contract(
        new_row.clone(),
        is_true("right"),
        Vec::new(),
        domains,
        observables,
    );
    let mapping = same_mapping(&["left", "right"]);
    let mut policy = test_policy();
    policy.comparison.max_worlds = 1;
    let report = compare_requirement_revisions(
        &policy,
        &request(
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            Vec::new(),
            no_correspondence(),
        ),
    )
    .unwrap();
    assert_eq!(report.classification, RevisionClass::Undetermined);
    assert_eq!(report.relation, RevisionRelation::Unknown);
    assert!(!report.search.as_ref().unwrap().complete);
    assert!(report.implications.iter().all(|item| item.holds.is_none()));
    assert_eq!(report.inconclusive.as_ref().unwrap().code, "world_bound");
}

#[test]
// r[verify cairn.revision_classification.requirement_relation]
fn correspondence_review_binding_is_admitted_and_stale_records_are_rejected() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let (normalized_old, normalized_new, _) =
        binding(&old_row, &old_contract, &new_row, &new_contract);
    let review = |normalized: &NormalizedContract| CorrespondenceReview {
        requirement_row_blake3: normalized.contract.requirement.row_blake3.clone(),
        contract_blake3: normalized.contract_blake3.clone(),
        interpretation_blake3: normalized.interpretation_blake3.clone(),
        mapping_blake3: normalized.mapping_blake3.clone(),
        proposed_by: normalized.contract.proposed_by.clone(),
        reviewer: "reviewer-1".into(),
        review_artifact_blake3: digest('9'),
        decision: "approved".into(),
    };
    let correspondence = CorrespondenceInputs {
        old: Some(review(&normalized_old)),
        new: Some(review(&normalized_new)),
        authorized_reviewers: vec!["reviewer-1".into()],
    };
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        correspondence.clone(),
    );
    assert_eq!(report.correspondence.old, "admitted_declaration");
    assert!(report.old.correspondence_blake3.is_some());

    let mut stale = correspondence;
    stale.old.as_mut().unwrap().contract_blake3 = digest('0');
    let error = compare_requirement_revisions(
        &test_policy(),
        &request(
            &old_row,
            &old_contract,
            &new_row,
            &new_contract,
            &mapping,
            Vec::new(),
            stale,
        ),
    )
    .unwrap_err();
    assert_eq!(error.outcome(), "invalid_correspondence");
}

#[test]
// r[verify cairn.revision_classification.bounds]
fn comparison_and_witnesses_are_deterministic() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let first = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    let second = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    assert_eq!(first.report_blake3, second.report_blake3);
    assert_eq!(first.input_blake3, second.input_blake3);
    assert_eq!(first.implications, second.implications);
}

#[test]
// r[verify cairn.revision_classification.transfer_disabled]
fn automated_evidence_transfer_is_always_disabled() {
    let (old_row, old_contract, new_row, new_contract) = base_contracts(is_true("flag"));
    let mapping = same_mapping(&["flag"]);
    let report = compare(
        &old_row,
        &old_contract,
        &new_row,
        &new_contract,
        &mapping,
        Vec::new(),
        no_correspondence(),
    );
    let request = EvidenceTransferRequest {
        schema: EVIDENCE_TRANSFER_SCHEMA_V1.into(),
        evidence_blake3: digest('e'),
        old_claim_blake3: digest('1'),
        new_claim_blake3: digest('2'),
        comparison_report_blake3: report.report_blake3.clone(),
        subject_blake3: report.context.subject_blake3.clone(),
        environment: report.context.environment.clone(),
        assumptions: BTreeSet::from(["finite-worlds".into()]),
    };
    let error = plan_evidence_transfer(&test_policy(), &report, &request).unwrap_err();
    assert_eq!(error.outcome(), "unsupported");
    assert!(error.to_string().contains("disabled"));
}

fn universe() -> EvidenceUniverse {
    EvidenceUniverse {
        checks: BTreeSet::from(["c1".into(), "c2".into()]),
        evidence_roles: BTreeSet::from(["r1".into(), "r2".into()]),
        assumptions: BTreeSet::from(["a".into()]),
    }
}

fn fragment(
    required_checks: &[&str],
    roles: &[&str],
    assumption_sets: &[&[&str]],
    universe: EvidenceUniverse,
) -> AcceptanceFragment {
    AcceptanceFragment {
        schema: ACCEPTANCE_FRAGMENT_SCHEMA_V1.into(),
        fragment: SUPPORTED_POLICY_FRAGMENT_V1.into(),
        required_checks: required_checks
            .iter()
            .map(|value| (*value).into())
            .collect(),
        evidence_roles: roles.iter().map(|value| (*value).into()).collect(),
        assumption_sets: assumption_sets
            .iter()
            .map(|set| set.iter().map(|value| (*value).into()).collect())
            .collect(),
        universe,
    }
}

fn policy_request(old: AcceptanceFragment, new: AcceptanceFragment) -> PolicyRevisionRequest {
    PolicyRevisionRequest {
        schema: POLICY_REVISION_REQUEST_SCHEMA_V1.into(),
        binding: PolicyRevisionBinding {
            old_policy_blake3: digest('1'),
            new_policy_blake3: digest('2'),
        },
        old,
        new,
    }
}

fn compare_fragments(old: AcceptanceFragment, new: AcceptanceFragment) -> PolicyComparisonReport {
    compare_policy_revisions(&test_policy(), &policy_request(old, new)).unwrap()
}

#[test]
// r[verify cairn.revision_classification.policy_fragment]
fn policy_tightening_is_distinct_from_widening_and_replays_its_witness() {
    let old = fragment(&["c1"], &["r1", "r2"], &[&["a"][..]], universe());
    let new = fragment(&["c1"], &["r1"], &[&["a"][..]], universe());
    let tightened = compare_fragments(old.clone(), new.clone());
    assert_eq!(tightened.classification, RevisionClass::Strengthening);
    assert_eq!(tightened.relation, SetRelation::Tightened);
    assert_eq!(tightened.supported_fragment, SUPPORTED_POLICY_FRAGMENT_V1);
    assert_eq!(tightened.universe.checks.len(), 2);
    assert_eq!(tightened.universe.evidence_roles.len(), 2);
    assert_eq!(tightened.universe.assumptions.len(), 1);
    assert_eq!(tightened.old.accepted_count, Some(4));
    assert_eq!(tightened.new.accepted_count, Some(2));
    assert!(tightened.inclusion[0].holds);
    assert!(!tightened.inclusion[1].holds);
    let witness = tightened.inclusion[1].witness.as_ref().unwrap();
    let replay = replay_policy_witness(&old, &new, witness).unwrap();
    assert!(replay.distinguishing);

    let widened = compare_fragments(new, old);
    assert_eq!(widened.classification, RevisionClass::Weakening);
    assert_eq!(widened.relation, SetRelation::Widened);
}

#[test]
// r[verify cairn.revision_classification.policy_fragment]
fn policy_incomparable_and_infeasible_sets_stay_visible() {
    let old = fragment(&["c1"], &["r1"], &[&["a"][..]], universe());
    let new = fragment(&["c2"], &["r1"], &[&["a"][..]], universe());
    let incompatible = compare_fragments(old, new);
    assert_eq!(incompatible.classification, RevisionClass::Incompatible);
    assert_eq!(incompatible.relation, SetRelation::Incomparable);
    for inclusion in &incompatible.inclusion {
        assert!(inclusion.witness.is_some());
    }

    let old = fragment(&["c1"], &["r1"], &[&["a"][..]], universe());
    let empty = fragment(&["c1"], &[], &[&["a"][..]], universe());
    let infeasible = compare_fragments(old, empty);
    assert_eq!(infeasible.classification, RevisionClass::Strengthening);
    assert!(infeasible.feasibility.new_accepted_empty);
    assert!(!infeasible.feasibility.new_feasible);
    assert!(infeasible.feasibility.vacuous_inclusion);
    assert_eq!(infeasible.new.accepted_count, Some(0));
    assert!(infeasible.inclusion[0].vacuous);
}

#[test]
// r[verify cairn.revision_classification.policy_fragment]
// r[verify cairn.revision_classification.bounds]
fn policy_universe_mismatch_is_incompatible_and_bounded_fragments_are_undetermined() {
    let mut other_universe = universe();
    other_universe.evidence_roles = BTreeSet::from(["r1".into()]);
    let old = fragment(&["c1"], &["r1"], &[&["a"][..]], universe());
    let new = fragment(&["c1"], &["r1"], &[&["a"][..]], other_universe);
    let mismatch = compare_fragments(old, new);
    assert_eq!(mismatch.classification, RevisionClass::Incompatible);
    assert_eq!(
        mismatch.unsupported.as_ref().unwrap().code,
        "universe_mismatch"
    );

    let mut policy = test_policy();
    policy.policy_fragment.max_checks = 1;
    let bounded = compare_policy_revisions(
        &policy,
        &policy_request(
            fragment(&["c1"], &["r1"], &[&["a"][..]], universe()),
            fragment(&["c1"], &["r1"], &[&["a"][..]], universe()),
        ),
    )
    .unwrap();
    assert_eq!(bounded.classification, RevisionClass::Undetermined);
    assert_eq!(bounded.relation, SetRelation::Unknown);
    assert_eq!(
        bounded.inconclusive.as_ref().unwrap().code,
        "fragment_bound"
    );
}
