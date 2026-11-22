//! Root API and wire controls for implementation-only naming repairs.

const POLICY_SOURCE: &str = include_str!("../../../cairn-policy/generated/cairn-policy.json");

fn policy() -> cairn_core::AgentGuidancePolicy {
    cairn_core::parse_policy_json(POLICY_SOURCE)
        .expect("checked policy")
        .agent_guidance_policy
}

fn declaration() -> cairn_core::AgentGuidanceDeclaration {
    cairn_core::AgentGuidanceDeclaration {
        context_refs: vec![cairn_core::AgentContextReference {
            id: "facts".into(),
            source_root: cairn_core::AGENT_GUIDANCE_SOURCE_PROJECT.into(),
            path: "docs/facts.md".into(),
            required: true,
            priority: 1,
            authority: cairn_core::AGENT_GUIDANCE_AUTHORITY_FACT.into(),
            profiles: vec![],
            artifacts: vec![],
            operations: vec![],
        }],
        artifact_guidance: vec![cairn_core::AgentArtifactGuidance {
            id: "artifact-advice".into(),
            text: "Keep the proposal bounded.".into(),
            priority: 2,
            authority: cairn_core::AGENT_GUIDANCE_AUTHORITY_ADVISORY.into(),
            profiles: vec![],
            artifacts: vec![],
        }],
        operation_guidance: vec![cairn_core::AgentOperationGuidance {
            id: "operation-advice".into(),
            text: "Read before editing.".into(),
            priority: 3,
            authority: cairn_core::AGENT_GUIDANCE_AUTHORITY_ADVISORY.into(),
            operations: vec![cairn_core::AGENT_GUIDANCE_OPERATION_CONTEXT.into()],
        }],
    }
}

fn selection(policy: &cairn_core::AgentGuidancePolicy) -> cairn_core::AgentGuidanceSelectionPlan {
    let admitted: cairn_core::AdmittedAgentGuidance =
        cairn_core::admit_agent_guidance_declaration(&declaration(), policy).expect("admitted");
    let request = cairn_core::AgentGuidanceRequest {
        operation: Some(cairn_core::AGENT_GUIDANCE_OPERATION_CONTEXT.into()),
        ..Default::default()
    };
    cairn_core::select_agent_guidance(&admitted, &request, policy).expect("selected")
}

fn observation() -> cairn_core::AgentGuidanceSourceObservation {
    let text = "Declared project fact.";
    cairn_core::AgentGuidanceSourceObservation {
        id: "facts".into(),
        status: cairn_core::AgentGuidanceObservationStatus::Present,
        text: Some(text.into()),
        byte_length: Some(text.len()),
        content_blake3: Some(cairn_core::blake3_hex(text.as_bytes())),
        diagnostic: None,
    }
}

fn evaluate(
    policy: &cairn_core::AgentGuidancePolicy,
    observations: &[cairn_core::AgentGuidanceSourceObservation],
) -> cairn_core::AgentGuidanceResult {
    cairn_core::build_agent_guidance_result(
        &selection(policy),
        observations,
        &["Policy constraint.".into()],
        Some("project-hash".into()),
        "policy-hash".into(),
        policy,
    )
}

#[test]
fn agent_guidance_root_names_preserve_receipt_bytes() {
    // r[verify cairn.octet_integration.evidence]
    let policy = policy();
    assert!(cairn_core::validate_agent_guidance_policy(&policy).is_ok());
    let bounds: &cairn_core::AgentGuidanceBounds = &policy.bounds;
    assert!(cairn_core::safe_relative_context_path(
        "docs/facts.md",
        bounds.max_path_bytes
    ));
    let result = evaluate(&policy, &[observation()]);
    assert!(result.valid);
    let local: &cairn_core::AgentGuidanceEntry = &result.entries[0];
    let portable_entry: &cairn_core::PortableAgentGuidanceEntry = &result.portable_entries[0];
    assert_eq!(local.authority, cairn_core::AGENT_GUIDANCE_AUTHORITY_POLICY);
    assert_eq!(local.id, portable_entry.id);
    let portable: cairn_core::PortableAgentGuidanceResult =
        cairn_core::portable_agent_guidance_result(&result);
    let encoded = serde_json::to_value(&portable).expect("portable value");
    assert_eq!(
        cairn_core::admit_portable_agent_guidance_result(&encoded),
        Ok(portable.clone())
    );
    assert_eq!(
        cairn_core::blake3_hex(&serde_json::to_vec(&(result, portable)).expect("receipt bytes")),
        "43fdaba898cb81cb15b94158581b48b04e071e4263b73fde9e94f599846bd2bd"
    );
}

#[test]
fn agent_guidance_root_rejects_invalid_declarations() {
    // r[verify cairn.agent_guidance_context.admission.invalid]
    let policy = policy();
    let mut invalid = declaration();
    invalid.context_refs[0].path = "../secret".into();
    invalid.context_refs[0].profiles = vec!["unknown".into()];
    invalid.context_refs[0].authority = cairn_core::AGENT_GUIDANCE_AUTHORITY_POLICY.into();
    invalid.artifact_guidance[0].id = "facts".into();
    let issues: Vec<cairn_core::AgentGuidanceIssue> =
        cairn_core::admit_agent_guidance_declaration(&invalid, &policy).expect_err("invalid input");
    for expected in [
        cairn_core::AGENT_GUIDANCE_ISSUE_PATH,
        cairn_core::AGENT_GUIDANCE_ISSUE_SELECTOR,
        cairn_core::AGENT_GUIDANCE_ISSUE_AUTHORITY,
        cairn_core::AGENT_GUIDANCE_ISSUE_DECLARATION,
    ] {
        assert!(issues.iter().any(|issue| issue.code == expected));
    }
    assert_eq!(
        cairn_core::blake3_hex(&serde_json::to_vec(&issues).expect("issue bytes")),
        "bb0ec131cab10d1fc09c4fd65d2a4ec42da60dc1155bb599b18a9646a98310d6"
    );
}

#[test]
fn agent_guidance_root_rejects_stale_and_missing_observations() {
    // r[verify cairn.agent_guidance_context.sources.invalid]
    let policy = policy();
    let mut stale = observation();
    stale.byte_length = Some(0);
    let result = evaluate(&policy, &[stale]);
    assert!(!result.valid);
    assert_eq!(
        result.issues[0].code,
        cairn_core::AGENT_GUIDANCE_ISSUE_OBSERVATION
    );
    let missing = cairn_core::AgentGuidanceSourceObservation {
        status: cairn_core::AgentGuidanceObservationStatus::MissingRequired,
        text: None,
        byte_length: None,
        content_blake3: None,
        ..observation()
    };
    let result = evaluate(&policy, &[missing]);
    assert!(!result.valid);
    assert_eq!(
        result.issues[0].code,
        cairn_core::AGENT_GUIDANCE_ISSUE_SOURCE_MISSING
    );
}

#[test]
fn agent_guidance_root_preserves_serde_shapes_and_rejects_raw_portable_data() {
    // r[verify cairn.agent_guidance_context.receipts.reject_raw]
    let policy = policy();
    let result = evaluate(&policy, &[observation()]);
    let portable = cairn_core::portable_agent_guidance_result(&result);
    let mut value = serde_json::to_value(portable).expect("portable value");
    value["text"] = "private context".into();
    assert_eq!(
        cairn_core::admit_portable_agent_guidance_result(&value)
            .expect_err("raw text")
            .code,
        cairn_core::AGENT_GUIDANCE_ISSUE_PORTABLE_RAW
    );
    let malformed =
        cairn_core::admit_portable_agent_guidance_result(&serde_json::Value::Bool(false))
            .expect_err("wrong shape");
    assert_eq!(
        malformed.code,
        cairn_core::AGENT_GUIDANCE_ISSUE_PORTABLE_INVALID
    );
    assert_eq!(malformed.message, "portable guidance is malformed: invalid type: boolean `false`, expected struct PortableAgentGuidanceResult");
    let short = cairn_core::admit_portable_agent_guidance_result(&serde_json::json!([]))
        .expect_err("short sequence");
    assert_eq!(short.message, "portable guidance is malformed: invalid length 0, expected struct PortableAgentGuidanceResult with 7 elements");
    let mut nested = serde_json::to_value(cairn_core::portable_agent_guidance_result(&result))
        .expect("nested value");
    nested["bounds"] = serde_json::json!([]);
    assert_eq!(cairn_core::admit_portable_agent_guidance_result(&nested).expect_err("short bounds").message,
        "portable guidance is malformed: invalid length 0, expected struct AgentGuidanceBounds with 10 elements");
    assert!(
        serde_json::from_str::<cairn_core::AgentGuidanceDeclaration>("{\"unknown\":[]}").is_err()
    );
    assert_eq!(
        serde_json::from_str::<cairn_core::AgentGuidanceDeclaration>("{}").expect("defaults"),
        cairn_core::AgentGuidanceDeclaration::default()
    );
    let statuses = [
        cairn_core::AgentGuidanceObservationStatus::Present,
        cairn_core::AgentGuidanceObservationStatus::MissingOptional,
        cairn_core::AgentGuidanceObservationStatus::MissingRequired,
        cairn_core::AgentGuidanceObservationStatus::Invalid,
    ];
    assert_eq!(
        serde_json::to_string(&statuses).expect("status bytes"),
        r#"["present","missing_optional","missing_required","invalid"]"#
    );
}

#[test]
fn portable_collaboration_root_names_remain_callable() {
    let _: fn(
        cairn_core::ChangeCollaborationState,
    ) -> Result<
        cairn_core::PortableChangeCollaboration,
        cairn_core::PortableChangeCollaborationError,
    > = cairn_core::build_portable_change_collaboration;
    let _: fn(
        &cairn_core::PortableChangeCollaboration,
    ) -> Result<
        cairn_core::ChangeCollaborationState,
        cairn_core::PortableChangeCollaborationError,
    > = cairn_core::validate_portable_change_collaboration;
}

fn initialize_import(
    init: cairn_core::ChangeCollaborationInit,
) -> Result<cairn_core::ChangeCollaborationState, cairn_core::ChangeCollaborationError> {
    cairn_core::initialize_change_collaboration_import(init, String::new())
}

#[test]
fn collaboration_import_root_names_remain_callable() {
    let _: fn(
        cairn_core::ChangeCollaborationImportSnapshot,
    )
        -> Result<cairn_core::ChangeCollaborationInit, cairn_core::ChangeCollaborationError> =
        cairn_core::plan_change_collaboration_import;
    let _: fn(
        cairn_core::ChangeCollaborationInit,
    ) -> Result<
        cairn_core::ChangeCollaborationState,
        cairn_core::ChangeCollaborationError,
    > = initialize_import;
    let _: fn(
        &cairn_core::ChangeCollaborationInit,
        &str,
    ) -> Result<String, cairn_core::ChangeCollaborationError> =
        cairn_core::change_collaboration_import_operation_id;
}

#[test]
fn collaboration_checkout_root_names_remain_callable() {
    let _: fn(
        &cairn_core::ChangeCollaborationState,
        cairn_core::ChangeCollaborationLocalSnapshot,
    )
        -> Result<cairn_core::ChangeCollaborationCheckoutPlan, cairn_core::ChangeCobError> =
        cairn_core::plan_change_collaboration_checkout;
}

#[test]
fn collaboration_change_root_names_remain_callable() {
    let _: fn(
        cairn_core::ChangeCollaborationOperation,
    ) -> Result<
        cairn_core::ChangeCollaborationState,
        cairn_core::ChangeCollaborationError,
    > = cairn_core::initialize_change_collaboration;
    let _: fn(
        &cairn_core::ChangeCollaborationInit,
    ) -> Result<(), cairn_core::ChangeCollaborationError> =
        cairn_core::validate_change_collaboration_init;
    let _: fn(
        &cairn_core::ChangeCollaborationAction,
    ) -> Result<serde_json::Value, cairn_core::ChangeCollaborationError> =
        cairn_core::change_collaboration_action_json;
    let _: fn(
        &cairn_core::ChangeCollaborationState,
    ) -> Result<
        Vec<cairn_core::ChangeCollaborationEvidence>,
        cairn_core::ChangeCollaborationError,
    > = cairn_core::current_change_collaboration_evidence;
}

#[test]
fn cargo_output_legacy_names_remain_callable_and_fail_closed() {
    let _: fn(&[u8]) -> cairn_core::verified::check_evidence::CargoCoverage =
        cairn_core::verified::check_evidence::cargo_coverage;
    let valid = b"running 1 test\n.\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
    assert!(cairn_core::verified::check_evidence::cargo_coverage(valid).passed);
    assert!(!cairn_core::verified::check_evidence::cargo_coverage(b"invalid\n").passed);
}

#[test]
fn octet_adapter_legacy_names_remain_callable_and_keep_wire_states() {
    let _: fn(
        &serde_json::Value,
    ) -> Result<
        cairn_core::verified::check_evidence::OctetAdmission,
        cairn_core::verified::check_evidence::ProducerIssue,
    > = cairn_core::verified::check_evidence::admit_octet_json;
    let passed = cairn_core::verified::check_evidence::OctetStatus::Passed;
    assert_eq!(
        serde_json::to_string(&passed).expect("status bytes"),
        "\"passed\""
    );
    assert!(
        serde_json::from_str::<cairn_core::verified::check_evidence::OctetStatus>("\"unknown\"")
            .is_err()
    );
}
