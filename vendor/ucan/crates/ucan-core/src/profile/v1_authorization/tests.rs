//! Positive and negative no-std authorization-core tests.

use alloc::vec;

use super::*;

fn command(value: &str) -> FinalCommand {
    FinalCommand::new(value).expect("command should parse")
}

fn dlg(
    issuer: &str,
    audience: &str,
    subject: Option<&str>,
    command: Option<FinalCommand>,
    policy: Option<Policy>,
    revoked: bool,
    cid_byte: u8,
) -> DelegationFacts {
    DelegationFacts {
        issuer: issuer.to_owned(),
        audience: audience.to_owned(),
        subject: subject.map(str::to_owned),
        command,
        policy,
        not_before: Some(1_000),
        expires_at: Some(2_000),
        cid: vec![cid_byte],
        revoked,
    }
}

fn invocation(issuer: &str, command: FinalCommand, proof_bytes: &[u8]) -> InvocationFacts {
    InvocationFacts {
        issuer: issuer.to_owned(),
        executor: "did:key:z6Mkexecutor".to_owned(),
        executor_allowed: true,
        subject: Some("did:key:z6Mksubject".to_owned()),
        command,
        args: IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]),
        proofs: proof_bytes.iter().map(|&byte| vec![byte]).collect(),
        not_before: Some(1_000),
        expires_at: Some(2_000),
    }
}

fn work_budget() -> usize {
    MAX_CHAIN_WORK
}

// ---- commands ----

#[test]
fn valid_commands_parse() {
    for value in ["/crud", "/crud/create", "/msg/send", "/", "/a/b/c", "/rôle/кафе"] {
        assert!(FinalCommand::new(value).is_ok(), "{value}");
    }
}

#[test]
fn invalid_commands_fail_closed() {
    for (value, expected) in [
        ("crud", CommandError::MustStartWithSlash),
        ("/crud/", CommandError::TrailingSlash),
        ("/crud//x", CommandError::EmptySegment),
        ("/Crud", CommandError::UppercaseAscii),
        ("/cr ud", CommandError::ControlOrSpace),
    ] {
        assert_eq!(FinalCommand::new(value), Err(expected), "{value}");
    }
}

#[test]
fn parent_command_covers_complete_descendant_segments() {
    assert!(command("/crud").covers(&command("/crud/create")));
    assert!(command("/").covers(&command("/crud/create")));
    assert!(command("/crud").covers(&command("/crud")));
    assert!(!command("/crud/create").covers(&command("/crud")));
}

#[test]
fn lexical_prefix_is_not_a_parent() {
    assert!(!command("/crypto").covers(&command("/cryptocurrency")));
    assert!(!command("/cryptocurrency").covers(&command("/crypto")));
}

// ---- selectors and policy ----

#[test]
fn selector_parse_and_resolution() {
    let selector = Selector::parse(".a.b").expect("selector");
    let args = IpldValue::Map(vec![("a".to_owned(), IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(7))]))]);
    assert_eq!(selector.resolve(&args), Some(&IpldValue::Int(7)));
    assert_eq!(Selector::parse(".").expect("root selector").resolve(&args), Some(&args));
    assert_eq!(Selector::parse(".missing").expect("selector").resolve(&args), None);
}

#[test]
fn malformed_and_unsupported_selectors_fail() {
    for value in ["..", ".a..b", ".a.b.", ".[", ".a|.b", ".a?.b", ".a~b"] {
        assert!(Selector::parse(value).is_err(), "{value}");
    }
}

fn ipld_from_json(value: serde_json::Value) -> IpldValue {
    // `serde_json` is a dev-only dependency of ucan-core's test build.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::option_if_let_else,
        reason = "fixture numeric projection keeps unsupported numeric classes explicit"
    )]
    fn projection(value: serde_json::Value) -> IpldValue {
        match value {
            serde_json::Value::Null => IpldValue::Null,
            serde_json::Value::Bool(value) => IpldValue::Bool(value),
            serde_json::Value::Number(number) => {
                if let Some(integer) = number.as_i64() {
                    IpldValue::Int(integer)
                } else if let Some(float) = number.as_f64() {
                    if float.fract() == 0.0 {
                        IpldValue::Int(float as i64)
                    } else {
                        IpldValue::Float(0)
                    }
                } else {
                    IpldValue::Int(0)
                }
            }
            serde_json::Value::String(text) => IpldValue::Text(text),
            serde_json::Value::Array(items) => IpldValue::List(items.into_iter().map(projection).collect()),
            serde_json::Value::Object(map) => {
                IpldValue::Map(map.into_iter().map(|(key, item)| (key, projection(item))).collect())
            }
        }
    }
    projection(value)
}

fn policy(json: serde_json::Value) -> Policy {
    let value = ipld_from_json(json);
    match Policy::parse(&value) {
        Ok(parsed) => parsed,
        Err(error) => panic!("policy parse failed: {error:?}\nvalue={value:?}"),
    }
}

#[test]
fn policy_matches_arguments() {
    let value = serde_json::json!([[["==", ".b", 1], [">", ".b", 0], ["<", ".b", 2], ["<=", ".b", 1], [
        ">=", ".b", 1
    ]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn policy_supports_like_connective_and_negation() {
    let value = serde_json::json!([[["like", ".name", "Ka*e"]], [["not", ["==", ".age", 40]]], [["and", [
        ["==", ".name", "Katie"],
        ["==", ".age", 35]
    ]]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![
        ("name".to_owned(), IpldValue::Text("Katie".to_owned())),
        ("age".to_owned(), IpldValue::Int(35)),
    ]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn like_uses_rhs_pattern_and_honors_escaped_wildcards() {
    let matching = policy(serde_json::json!([[["like", ".name", "Alice\\*, Bob*, Carol."]]]));
    let matching_args =
        IpldValue::Map(vec![("name".to_owned(), IpldValue::Text("Alice*, Bob, Dan, Erin, Carol.".to_owned()))]);
    let mut matching_budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(matching.satisfied(&matching_args, &mut matching_budget).expect("matching like"));

    let denied_args = IpldValue::Map(vec![("name".to_owned(), IpldValue::Text("Alice, Bob, Carol.".to_owned()))]);
    let mut denied_budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(!matching.satisfied(&denied_args, &mut denied_budget).expect("denied like"));
}

#[test]
fn unresolved_selector_evaluates_false_without_panic() {
    let value = serde_json::json!([[["==", ".missing", 1]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(!parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn type_mismatch_compare_is_false() {
    let value = serde_json::json!([[["==", ".name", 1]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("name".to_owned(), IpldValue::Text("Katie".to_owned()))]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(!parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn optional_selector_within_or_is_false_for_missing() {
    let value = serde_json::json!([[["or", [["==", ".missing", 1]]]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(!parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn unknown_operator_is_rejected() {
    let value = serde_json::json!([[["glorp", ".a", 1]]]);
    assert_eq!(Policy::parse(&ipld_from_json(value)), Err(PolicyError::UnknownOperator("glorp".to_owned())));
}

#[test]
fn unsupported_jq_features_are_rejected() {
    let value = serde_json::json!([[["==", ".a|.b", 1]], [["==", ".a?.b", 1]]]);
    let parsed = Policy::parse(&ipld_from_json(value));
    assert!(matches!(parsed, Err(PolicyError::UnsupportedJq { .. })));
}

#[test]
fn policy_work_limit_is_enforced() {
    let value = serde_json::json!([[["==", ".b", 1]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]);
    let mut budget = 0_usize;
    assert_eq!(parsed.satisfied(&args, &mut budget), Err(PolicyError::WorkLimitExceeded));
}

#[test]
fn collection_and_byte_equality_are_supported() {
    let value = serde_json::json!([[["==", ".list", [1, 2]], ["==", ".bytes", [104, 105]]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![
        ("list".to_owned(), IpldValue::List(vec![IpldValue::Int(1), IpldValue::Int(2)])),
        ("bytes".to_owned(), IpldValue::Bytes(vec![104, 105])),
    ]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    assert!(parsed.satisfied(&args, &mut budget).expect("evaluate"));
}

#[test]
fn numeric_equality_treats_1_and_1dot0_as_equal() {
    let value = serde_json::json!([[["==", ".b", 1.0]]]);
    let parsed = policy(value);
    let args = IpldValue::Map(vec![("b".to_owned(), IpldValue::Int(1))]);
    let mut budget = MAX_POLICY_EVALUATION_STEPS;
    let result = parsed.satisfied(&args, &mut budget).expect("evaluate");
    assert!(result, "numeric equality must hold");
}

// ---- chain composition and admission ----

#[test]
fn direct_chain_is_admitted() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/crud")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    let authority = compose_chain(&facts, &[delegation], 1_500, 60, &mut budget).expect("direct chain admits");
    assert_eq!(authority.root_issuer, "did:key:z6Mkroot");
    assert_eq!(authority.effective_command, command("/crud/create"));
    assert_eq!(authority.proof_cids.len(), 1);
}

#[test]
fn delegated_two_hop_chain_is_admitted() {
    let root = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkmid",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let middle = dlg("did:key:z6Mkmid", "did:key:z6Mkinvoker", None, Some(command("/crud")), None, false, 0x12);
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11, 0x12]);
    let mut budget = work_budget();
    let authority = compose_chain(&facts, &[root, middle], 1_500, 60, &mut budget).expect("two-hop chain admits");
    assert_eq!(authority.subject, "did:key:z6Mksubject");
}

#[test]
fn alternate_valid_path_is_admitted_without_reordering() {
    let delegation = dlg(
        "did:key:z6Mkother",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    let authority = compose_chain(&facts, &[delegation], 1_500, 60, &mut budget).expect("alternate path admits");
    assert_eq!(authority.root_issuer, "did:key:z6Mkother");
}

#[test]
fn empty_chain_is_rejected() {
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[]);
    let mut budget = work_budget();
    assert_eq!(compose_chain(&facts, &[], 1_500, 60, &mut budget), Err(ChainError::EmptyChain));
}

#[test]
fn duplicate_and_reordered_proofs_fail_closed() {
    // Duplicate proof CIDs are rejected.
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let duplicated = vec![delegation.clone(), delegation];
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11, 0x11]);
    let mut budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &duplicated, 1_500, 60, &mut budget),
        Err(ChainError::DuplicateProof | ChainError::CycleProof)
    ));

    // Reordering breaks adjacent alignment without silent repair.
    let out_of_order_first = dlg(
        "did:key:z6Mkmis",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let out_of_order_second = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkmis",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x12,
    );
    let order_facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11, 0x12]);
    let mut budget = work_budget();
    let error = compose_chain(&order_facts, &[out_of_order_first, out_of_order_second], 1_500, 60, &mut budget)
        .expect_err("order must fail");
    assert!(matches!(error, ChainError::OrderViolation { .. }));
}

#[test]
fn root_without_subject_is_rejected() {
    let delegation = dlg("did:key:z6Mkroot", "did:key:z6Mkinvoker", None, Some(command("/")), None, false, 0x11);
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert_eq!(compose_chain(&facts, &[delegation], 1_500, 60, &mut budget), Err(ChainError::RootWithoutSubject));
}

#[test]
fn subject_mismatch_is_rejected() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mkother"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &[delegation], 1_500, 60, &mut budget),
        Err(ChainError::SubjectMismatch)
    ));
}

#[test]
fn non_invoker_final_audience_is_rejected() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6MkWrong",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert_eq!(compose_chain(&facts, &[delegation], 1_500, 60, &mut budget), Err(ChainError::NotInvoker));
}

#[test]
fn executor_mismatch_denies() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let mut facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    facts.executor_allowed = false;
    let mut budget = work_budget();
    assert_eq!(compose_chain(&facts, &[delegation], 1_500, 60, &mut budget), Err(ChainError::ExecutorMismatch));
}

#[test]
fn lexical_prefix_command_denies() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/crypto")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/cryptocurrency"), &[0x11]);
    let mut budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &[delegation], 1_500, 60, &mut budget),
        Err(ChainError::CommandNotCovered { .. })
    ));
}

#[test]
fn time_invalid_chain_denies() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &[delegation], 5_000, 60, &mut budget),
        Err(ChainError::TimeInvalid { .. })
    ));
}

#[test]
fn invalid_or_overflowing_clock_observations_deny() {
    const NEGATIVE_CLOCK_SKEW: i64 = -1;
    const POSITIVE_CLOCK_SKEW: i64 = 60;
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut negative_budget = work_budget();
    assert_eq!(
        compose_chain(&facts, core::slice::from_ref(&delegation), 1_500, NEGATIVE_CLOCK_SKEW, &mut negative_budget),
        Err(ChainError::InvalidClockSkew)
    );

    let mut overflow_budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &[delegation], i64::MAX, POSITIVE_CLOCK_SKEW, &mut overflow_budget),
        Err(ChainError::TimeInvalid { .. })
    ));
}

#[test]
fn policy_denied_chain_fails_closed() {
    let value = serde_json::json!([[["==", ".b", 2]]]);
    let parsed = policy(value);
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        Some(parsed),
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert!(matches!(
        compose_chain(&facts, &[delegation], 1_500, 60, &mut budget),
        Err(ChainError::PolicyDenied { .. })
    ));
}

#[test]
fn revoked_delegation_denies() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        true,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = work_budget();
    assert_eq!(compose_chain(&facts, &[delegation], 1_500, 60, &mut budget), Err(ChainError::Revoked { index: 0 }));
}

#[test]
fn chain_work_limit_is_enforced() {
    let delegation = dlg(
        "did:key:z6Mkroot",
        "did:key:z6Mkinvoker",
        Some("did:key:z6Mksubject"),
        Some(command("/")),
        None,
        false,
        0x11,
    );
    let facts = invocation("did:key:z6Mkinvoker", command("/crud/create"), &[0x11]);
    let mut budget = 0_usize;
    assert!(matches!(
        compose_chain(&facts, &[delegation], 1_500, 60, &mut budget),
        Err(ChainError::LimitExceeded)
    ));
}
