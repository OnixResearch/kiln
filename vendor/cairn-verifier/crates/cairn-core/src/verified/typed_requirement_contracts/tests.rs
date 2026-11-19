use super::*;
use crate::{
    evaluate_content_bound_requirement_registry, ContentBoundRevision,
    ContentBoundSpecificationInput,
};

fn inventory() -> TypedRequirementInventory {
    parse_typed_requirement_inventory_value(
        &serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/typed-requirement-contracts.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

fn example(contract: &SemanticContract) -> SemanticExample {
    fn arguments(contract: &SemanticContract, domains: &[String]) -> Vec<Vec<FiniteValue>> {
        if let Some((first, rest)) = domains.split_first() {
            let suffixes = arguments(contract, rest);
            contract.domains[first]
                .values
                .iter()
                .flat_map(|value| {
                    suffixes.iter().map(move |suffix| {
                        let mut row = vec![FiniteValue {
                            domain: first.clone(),
                            value: value.clone(),
                        }];
                        row.extend(suffix.clone());
                        row
                    })
                })
                .collect()
        } else {
            vec![vec![]]
        }
    }
    SemanticExample {
        observables: contract
            .observables
            .iter()
            .map(|(name, mapping)| {
                (
                    name.clone(),
                    arguments(contract, &mapping.arguments)
                        .into_iter()
                        .map(|arguments| ObservableRow {
                            arguments,
                            value: FiniteValue {
                                domain: mapping.result.clone(),
                                value: contract.domains[&mapping.result].values[0].clone(),
                            },
                        })
                        .collect(),
                )
            })
            .collect(),
    }
}

fn set(example: &mut SemanticExample, mapping: &str, arguments: &[&str], value: &str) {
    let row = example
        .observables
        .get_mut(mapping)
        .unwrap()
        .iter_mut()
        .find(|row| {
            row.arguments
                .iter()
                .map(|v| v.value.as_str())
                .eq(arguments.iter().copied())
        })
        .unwrap();
    row.value.value = value.into();
}

fn normalized(inventory: &TypedRequirementInventory, index: usize) -> NormalizedContract {
    let contract = &inventory.contracts[index];
    normalize_contract(&inventory.profile, &contract.requirement, contract).unwrap()
}

#[test]
// r[verify cairn.typed_requirements.finite_contract]
fn five_pilots_bind_exact_native_registry_rows() {
    let inventory = inventory();
    assert_eq!(inventory.contracts.len(), 5);
    let specs = vec![
        ContentBoundSpecificationInput {
            path: ".cairn/specs/agent-completion/spec.md".into(),
            markdown: include_str!("../../../../../.cairn/specs/agent-completion/spec.md").into(),
        },
        ContentBoundSpecificationInput {
            path: ".cairn/specs/verification-obligation-radicle-cob/spec.md".into(),
            markdown: include_str!(
                "../../../../../.cairn/specs/verification-obligation-radicle-cob/spec.md"
            )
            .into(),
        },
    ];
    let registry = evaluate_content_bound_requirement_registry(
        specs,
        b"policy",
        "typed-contract-tests",
        ContentBoundRevision {
            kind: "opaque".into(),
            value: "fixture".into(),
        },
    )
    .registry
    .unwrap();
    for contract in &inventory.contracts {
        let row = registry
            .rows
            .iter()
            .find(|row| row.requirement_id == contract.requirement.requirement_id)
            .unwrap();
        assert_eq!(row, &contract.requirement);
        assert!(normalize_contract(&inventory.profile, row, contract).is_ok());
        assert!(!contract.informal_remainder.is_empty());
    }
}

#[test]
fn normalization_is_deterministic_and_identity_covers_every_interpretation_input() {
    let inventory = inventory();
    let before = normalized(&inventory, 0);
    let mut reordered = inventory.contracts[0].clone();
    for domain in reordered.domains.values_mut() {
        domain.values.reverse();
    }
    if let FiniteExpr::And { expressions } = &mut reordered.guarantee {
        expressions.reverse();
    }
    assert_eq!(
        before,
        normalize_contract(&inventory.profile, &reordered.requirement, &reordered).unwrap()
    );
    let mut changed = reordered.clone();
    changed
        .observables
        .values_mut()
        .next()
        .unwrap()
        .projection
        .push_str(" changed");
    let after = normalize_contract(&inventory.profile, &changed.requirement, &changed).unwrap();
    assert_ne!(before.mapping_blake3, after.mapping_blake3);
    assert_ne!(before.contract_blake3, after.contract_blake3);
    let mut profile = inventory.profile.clone();
    profile.max_steps -= 1;
    assert_ne!(
        before.interpretation_blake3,
        normalize_contract(&profile, &reordered.requirement, &reordered)
            .unwrap()
            .interpretation_blake3
    );
    let mut row = reordered.requirement.clone();
    row.requirement_body_blake3 = "f".repeat(64);
    assert_eq!(
        normalize_contract(&inventory.profile, &row, &reordered),
        Err(SemanticError::RequirementRevisionMismatch)
    );
    changed = reordered.clone();
    changed.guarantee = FiniteExpr::Bool { value: false };
    assert_ne!(
        before.contract_blake3,
        normalize_contract(&inventory.profile, &changed.requirement, &changed)
            .unwrap()
            .contract_blake3
    );
}

#[test]
fn typing_binding_unsupported_profile_and_resource_errors_are_explicit() {
    let inventory = inventory();
    let mut contract = inventory.contracts[0].clone();
    let errors = [
        (
            FiniteExpr::Equal {
                left: FiniteTerm::Variable {
                    name: "free".into(),
                },
                right: FiniteTerm::Literal {
                    value: FiniteValue {
                        domain: "identity".into(),
                        value: "a".into(),
                    },
                },
            },
            "unbound",
        ),
        (
            FiniteExpr::Equal {
                left: FiniteTerm::Literal {
                    value: FiniteValue {
                        domain: "bool".into(),
                        value: "true".into(),
                    },
                },
                right: FiniteTerm::Literal {
                    value: FiniteValue {
                        domain: "identity".into(),
                        value: "a".into(),
                    },
                },
            },
            "ill_typed",
        ),
        (
            FiniteExpr::Unsupported {
                construct: "always".into(),
            },
            "unsupported",
        ),
    ];
    for (expression, expected) in errors {
        contract.guarantee = expression;
        let error =
            normalize_contract(&inventory.profile, &contract.requirement, &contract).unwrap_err();
        assert!(matches!(
            (&error, expected),
            (SemanticError::UnboundVariable(_), "unbound")
                | (SemanticError::IllTyped(_), "ill_typed")
                | (SemanticError::UnsupportedConstruct(_), "unsupported")
        ));
    }
    contract = inventory.contracts[0].clone();
    contract.profile = "unknown".into();
    assert!(matches!(
        normalize_contract(&inventory.profile, &contract.requirement, &contract),
        Err(SemanticError::UnknownProfile(_))
    ));
    let mut profile = inventory.profile.clone();
    profile.max_nodes = 1;
    assert!(matches!(
        normalize_contract(
            &profile,
            &inventory.contracts[0].requirement,
            &inventory.contracts[0]
        ),
        Err(SemanticError::BoundExhaustion(_))
    ));
    let mut wire = serde_json::to_value(&inventory).unwrap();
    wire["contracts"][0]["guarantee"] = serde_json::json!({"kind": "add", "left": 1, "right": 2});
    assert_eq!(
        parse_typed_requirement_inventory_value(&wire),
        Err(SemanticError::UnsupportedConstruct("add".into()))
    );
}

#[test]
// r[verify cairn.typed_requirements.correspondence]
fn correspondence_admits_only_independent_current_declared_reviews() {
    let inventory = inventory();
    let contract = normalized(&inventory, 0);
    let review = CorrespondenceReview {
        requirement_row_blake3: contract.contract.requirement.row_blake3.clone(),
        contract_blake3: contract.contract_blake3.clone(),
        interpretation_blake3: contract.interpretation_blake3.clone(),
        mapping_blake3: contract.mapping_blake3.clone(),
        proposed_by: contract.contract.proposed_by.clone(),
        reviewer: "independent-reviewer".into(),
        review_artifact_blake3: "a".repeat(64),
        decision: "approved".into(),
    };
    let reviewers = vec![review.reviewer.clone()];
    assert!(admit_correspondence_review(&contract, &review, &reviewers).is_ok());
    assert!(admit_correspondence_review(&contract, &review, &[]).is_err());
    let mut modified = contract.clone();
    modified
        .contract
        .observables
        .values_mut()
        .next()
        .unwrap()
        .projection
        .push_str(" tampered");
    assert!(admit_correspondence_review(&modified, &review, &reviewers).is_err());
    for field in [
        "requirement",
        "contract",
        "interpretation",
        "mapping",
        "author",
        "self",
        "decision",
        "artifact",
    ] {
        let mut changed = review.clone();
        match field {
            "requirement" => changed.requirement_row_blake3 = "b".repeat(64),
            "contract" => changed.contract_blake3 = "b".repeat(64),
            "interpretation" => changed.interpretation_blake3 = "b".repeat(64),
            "mapping" => changed.mapping_blake3 = "b".repeat(64),
            "author" => changed.proposed_by = "invented-other-author".into(),
            "self" => changed.reviewer = changed.proposed_by.clone(),
            "decision" => changed.decision = "proposed".into(),
            "artifact" => changed.review_artifact_blake3.clear(),
            _ => unreachable!(),
        }
        assert!(
            admit_correspondence_review(&contract, &changed, &reviewers).is_err(),
            "{field}"
        );
    }
}

#[test]
// r[verify cairn.typed_requirements.examples]
fn coverage_distinguishes_passed_failed_duplicate_missing_and_unknown_rows() {
    let inventory = inventory();
    let normalized = normalized(&inventory, 1);
    let mut good = example(&normalized.contract);
    set(&mut good, "selected", &["check-a"], "true");
    set(&mut good, "present", &["row-a"], "true");
    set(&mut good, "row_check", &["row-a"], "check-a");
    set(&mut good, "row_status", &["row-a"], "passed");
    set(&mut good, "eligible", &["row-a"], "true");
    assert!(
        evaluate_example(&inventory.profile, &normalized, &good)
            .unwrap()
            .guarantee_holds
    );
    for failure in ["failed", "duplicate", "missing", "unknown", "ineligible"] {
        let mut bad = good.clone();
        match failure {
            "failed" => set(&mut bad, "row_status", &["row-a"], "failed"),
            "duplicate" => {
                set(&mut bad, "present", &["row-b"], "true");
                set(&mut bad, "row_check", &["row-b"], "check-a");
                set(&mut bad, "row_status", &["row-b"], "passed");
                set(&mut bad, "eligible", &["row-b"], "true");
            }
            "missing" => set(&mut bad, "present", &["row-a"], "false"),
            "unknown" => set(&mut bad, "row_check", &["row-a"], "unknown"),
            "ineligible" => set(&mut bad, "eligible", &["row-a"], "false"),
            _ => unreachable!(),
        }
        assert!(
            !evaluate_example(&inventory.profile, &normalized, &bad)
                .unwrap()
                .guarantee_holds,
            "{failure}"
        );
    }
}

#[test]
fn identity_freshness_authority_and_claim_transition_have_positive_and_negative_examples() {
    let inventory = inventory();
    for index in [0, 2, 3, 4] {
        let normalized = normalized(&inventory, index);
        let mut good = example(&normalized.contract);
        match index {
            0 => set(&mut good, "repository_contract_present", &[], "true"),
            2 => set(&mut good, "eligible", &["row-a"], "true"),
            3 => {
                set(&mut good, "completion_claimed", &[], "true");
                set(&mut good, "receipt_present", &[], "true");
                set(&mut good, "receipt_passing", &[], "true");
            }
            4 => {
                set(&mut good, "revision_after", &[], "b");
                set(&mut good, "lifecycle_after", &[], "open");
                set(&mut good, "operation_before", &["operation-a"], "true");
                set(&mut good, "operation_after", &["operation-a"], "true");
            }
            _ => unreachable!(),
        }
        let result = evaluate_example(&inventory.profile, &normalized, &good).unwrap();
        assert!(
            result.activated && result.guarantee_holds && result.implication_holds,
            "pilot {index}"
        );
        let mut bad = good.clone();
        match index {
            0 => set(&mut bad, "decision_identity", &["policy"], "b"),
            2 => set(&mut bad, "final_subject", &[], "b"),
            3 => set(&mut bad, "receipt_present", &[], "false"),
            4 => set(&mut bad, "lifecycle_after", &[], "discharged"),
            _ => unreachable!(),
        }
        assert!(
            !evaluate_example(&inventory.profile, &normalized, &bad)
                .unwrap()
                .implication_holds,
            "pilot {index}"
        );
    }
}

#[test]
fn evaluation_rejects_incomplete_duplicate_ill_typed_tables_tampering_and_exhaustion() {
    let inventory = inventory();
    let normalized = normalized(&inventory, 0);
    let good = example(&normalized.contract);
    let mut bad = good.clone();
    bad.observables
        .get_mut("repository_identity")
        .unwrap()
        .pop();
    assert!(matches!(
        evaluate_example(&inventory.profile, &normalized, &bad),
        Err(SemanticError::InvalidExample(_))
    ));
    bad = good.clone();
    let rows = bad.observables.get_mut("repository_identity").unwrap();
    rows[1] = rows[0].clone();
    assert!(matches!(
        evaluate_example(&inventory.profile, &normalized, &bad),
        Err(SemanticError::InvalidExample(_))
    ));
    bad = good.clone();
    set(&mut bad, "repository_contract_present", &[], "unknown");
    assert!(matches!(
        evaluate_example(&inventory.profile, &normalized, &bad),
        Err(SemanticError::IllTyped(_))
    ));
    let mut tampered = normalized.clone();
    tampered.contract_blake3 = "f".repeat(64);
    assert!(matches!(
        evaluate_example(&inventory.profile, &tampered, &good),
        Err(SemanticError::InvalidExample(_))
    ));
    let mut profile = inventory.profile.clone();
    profile.max_steps = 1;
    let limited = normalize_contract(
        &profile,
        &normalized.contract.requirement,
        &normalized.contract,
    )
    .unwrap();
    assert!(matches!(
        evaluate_example(&profile, &limited, &good),
        Err(SemanticError::BoundExhaustion(_))
    ));
}

#[test]
fn membership_negation_disjunction_and_existential_domains_are_evaluated_not_named() {
    let inventory = inventory();
    let mut contract = inventory.contracts[0].clone();
    contract.guarantee = FiniteExpr::Exists {
        variable: "x".into(),
        domain: "identity".into(),
        expression: Box::new(FiniteExpr::Or {
            expressions: vec![
                FiniteExpr::Bool { value: false },
                FiniteExpr::Not {
                    expression: Box::new(FiniteExpr::Member {
                        term: FiniteTerm::Variable { name: "x".into() },
                        values: vec![FiniteValue {
                            domain: "identity".into(),
                            value: "a".into(),
                        }],
                    }),
                },
            ],
        }),
    };
    let normalized =
        normalize_contract(&inventory.profile, &contract.requirement, &contract).unwrap();
    assert!(
        evaluate_example(&inventory.profile, &normalized, &example(&contract))
            .unwrap()
            .guarantee_holds
    );
    contract.domains.get_mut("identity").unwrap().values = vec!["a".into()];
    let normalized =
        normalize_contract(&inventory.profile, &contract.requirement, &contract).unwrap();
    assert!(
        !evaluate_example(&inventory.profile, &normalized, &example(&contract))
            .unwrap()
            .guarantee_holds
    );
}
