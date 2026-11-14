use serde_json::Value;

use super::*;

const EXPECTED_MAX_STORE_ID_BYTES: usize = 64;
const EXPECTED_INITIAL_PROFILE_COUNT: usize = 3;
const INVALID_MACHINE_SUCCESS_STATUS: u64 = 1;

#[test]
fn rejects_cyclic_generated_workflow_policy_with_a_stable_code() {
    // r[verify cairn.artifact_workflow_contract.policy.invalid]
    // r[verify cairn.lifecycle_workflow_profiles.registry.invalid]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy value");
    value["schemas"][0]["artifacts"][0]["requires"] = serde_json::json!(["tasks"]);
    let error = validate_policy_json(value).expect_err("workflow cycle fails");
    assert!(error
        .to_string()
        .contains(crate::verified::artifact_workflow::WORKFLOW_ISSUE_POLICY_CYCLE));
}

#[test]
fn rejects_agent_guidance_policy_authority_impersonation() {
    // r[verify cairn.agent_guidance_context.authority.reject]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy value");
    value["agent_guidance_policy"]["allowed_authority_classes"] =
        serde_json::json!(["policy_constraint"]);
    let error = validate_policy_json(value).expect_err("policy authority impersonation fails");
    assert!(error
        .to_string()
        .contains("agent_guidance_policy.authority"));
}

#[test]
fn generated_policy_admits_developer_loop_profiles() {
    // r[verify cairn.developer_loop_profiles.policy.valid]
    let policy = parse_policy_json(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy parses");
    assert_eq!(
        policy.developer_loop_policy.schema,
        "cairn.developer-loop-policy.v1"
    );
    assert_eq!(policy.developer_loop_policy.profiles.len(), 1);
    assert_eq!(policy.developer_loop_policy.profiles[0].id, "cairn-local");
    assert_eq!(policy.developer_loop_policy.checks.len(), 3);
}

#[test]
fn rejects_generated_explore_mutation_and_missing_constraint() {
    // r[verify cairn.generated_agent_workflows.descriptors.invalid]
    // r[verify cairn.generated_agent_workflows.explore.invalid]
    let source = include_str!("../../../../../cairn-policy/generated/cairn-policy.json");
    for mutation in ["command", "constraint"] {
        let mut value: Value = serde_json::from_str(source).expect("generated policy value");
        let descriptors = value["generated_agent_workflow_policy"]["descriptors"]
            .as_array_mut()
            .expect("descriptor array");
        let explore = descriptors
            .iter_mut()
            .find(|descriptor| descriptor["id"] == "explore")
            .expect("explore descriptor");
        match mutation {
            "command" => explore["command_classes"] = serde_json::json!(["artifact-edit"]),
            "constraint" => {
                explore["constraints"] = serde_json::json!(["Keep policy authoritative."])
            }
            _ => unreachable!("bounded test mutation"),
        }
        let error = validate_policy_json(value).expect_err("invalid explore policy fails");
        assert!(error
            .to_string()
            .contains("generated_workflow.policy.invalid"));
    }
}

#[test]
fn rejects_unknown_default_workflow_profile() {
    // r[verify cairn.lifecycle_workflow_profiles.selection.invalid]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy value");
    value["workflow_profile_policy"]["default_profile"] = serde_json::json!("unknown-profile");
    let error = validate_policy_json(value).expect_err("unknown default profile fails");
    assert!(error.to_string().contains("default_profile"));
}

#[test]
fn rejects_duplicate_workflow_profiles_and_ambient_sources() {
    // r[verify cairn.lifecycle_workflow_profiles.registry]
    // r[verify cairn.lifecycle_workflow_profiles.selection.invalid]
    // r[verify cairn.lifecycle_workflow_profiles.boundary.ambient]
    let value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy value");

    let mut duplicate = value.clone();
    let schema = duplicate["schemas"][0].clone();
    duplicate["schemas"]
        .as_array_mut()
        .expect("schemas")
        .push(schema);
    let duplicate_error =
        validate_policy_json(duplicate).expect_err("duplicate workflow profile fails");
    assert!(duplicate_error
        .to_string()
        .contains("duplicate workflow schema id"));

    let mut ambient = value;
    ambient["workflow_profile_policy"]["forbidden_selection_sources"] =
        serde_json::json!(["user-global", "agent-prompt", "remote-package"]);
    let ambient_error = validate_policy_json(ambient).expect_err("ambient profile source fails");
    assert!(ambient_error
        .to_string()
        .contains("must forbid ambient source: environment"));
}

#[test]
fn parses_generated_default_policy_text() {
    // r[verify cairn.artifact_workflow_contract.policy.valid]
    // r[verify developer_experience.core_policy_parser_boundary_split.behavior]
    // r[verify cairn.supersede.traceability_ownership.configured_profiles.cairn_default]
    // r[verify cairn.cli_machine_contract.policy.valid]
    // r[verify cairn.lifecycle_workflow_profiles.registry.valid]
    // r[verify cairn.lifecycle_workflow_profiles.initial_profiles]
    // r[verify cairn.lifecycle_workflow_profiles.initial_profiles.spec_driven]
    // r[verify cairn.lifecycle_workflow_profiles.selection.compatibility]
    let text = include_str!("../../../../../cairn-policy/generated/cairn-policy.json");
    let policy = parse_policy_json(text).expect("generated policy parses");
    assert_eq!(policy.name, "cairn-default");
    assert_eq!(policy.version, SUPPORTED_POLICY_VERSION);
    assert!(policy.task_markers_enabled);
    assert_eq!(
        policy.machine_contract_policy.schema_version,
        crate::verified::cli_machine::CLI_MACHINE_SCHEMA_V1
    );
    assert_eq!(
        policy.machine_contract_policy.exit_classes.len(),
        crate::verified::cli_machine::CLI_MACHINE_EXIT_CLASSES.len()
    );
    assert!(
        crate::verified::cli_machine::CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES
            .iter()
            .all(|required| policy
                .machine_contract_policy
                .diagnostics
                .iter()
                .any(|definition| definition.code == *required))
    );
    assert_eq!(policy.gate_stages, vec!["proposal", "design", "tasks"]);
    assert_eq!(
        policy.workflow_schemas.len(),
        EXPECTED_INITIAL_PROFILE_COUNT
    );
    assert_eq!(
        policy.workflow_profile_policy.allowed_profiles,
        vec!["spec-driven", "assurance", "no-spec-delta"]
    );
    let (compatibility, _) = crate::verified::artifact_workflow::select_workflow_profile(
        &policy.workflow_profile_policy,
        &policy.workflow_schemas,
        &crate::verified::artifact_workflow::WorkflowChangeProfileMetadata {
            workflow_profile: None,
            spec_effect: None,
            spec_effect_rationale: None,
        },
    )
    .expect("existing change uses compatibility profile");
    assert_eq!(compatibility.profile_id, "spec-driven");
    assert!(compatibility.compatibility_fallback);
    let workflow = &policy.workflow_schemas[0];
    assert_eq!(workflow.id, "spec-driven");
    assert_eq!(
        workflow.states,
        crate::verified::artifact_workflow::WORKFLOW_SUPPORTED_STATES
            .map(ToOwned::to_owned)
            .to_vec()
    );
    assert_eq!(
        crate::verified::artifact_workflow::workflow_topological_order(workflow)
            .expect("workflow order"),
        vec!["proposal", "specs", "design", "tasks"]
    );
    assert!(policy.gate_ignore_archives);
    assert_eq!(policy.gate_finding_scope, "tasks");
    assert_eq!(policy.gate_finding_promoted, "no");
    assert!(policy.sync_require_gate);
    assert!(policy.archive_require_gate);
    assert_eq!(
        policy
            .artifact_substance_policy
            .proposal_minimum_substantive_lines,
        1
    );
    assert_eq!(
        policy.artifact_substance_policy.dependency_marker_id,
        "depends"
    );
    assert_eq!(
        policy.artifact_substance_policy.dependency_target_class,
        "lifecycle-id"
    );
    assert!(policy.migrate_require_manifest);
    assert!(policy.require_receipt_hashes);
    assert!(policy
        .task_marker_tokens
        .iter()
        .any(|marker| marker.token == "[serial]"));
    assert!(policy
        .task_marker_tokens
        .iter()
        .any(|marker| { marker.id == TASK_IDENTITY_MARKER_ID && marker.kind == "task-id-prefix" }));
    assert!(policy.task_marker_tokens.iter().any(|marker| {
        marker.id == TASK_PREDECESSOR_MARKER_ID && marker.kind == "task-predecessor-prefix"
    }));
    assert!(policy
        .receipt_schemas
        .iter()
        .any(|schema| schema.command == "gate"
            && schema.hash_fields.contains(&"receipt_hash".to_string())));
    assert_eq!(
        policy.exemption_marker_policy.required_fields,
        vec!["reason", "owner", "until"]
    );
    assert!(!policy.steel_orchestration_policy.enabled);
    assert_eq!(policy.steel_orchestration_policy.profiles.len(), 1);
    assert_eq!(
        policy.steel_orchestration_policy.profiles[0].mode,
        "disabled"
    );
    assert_eq!(policy.traceability_policy.default_profile, "cairn-default");
    assert!(policy
        .traceability_policy
        .profiles
        .iter()
        .any(|profile| profile.id == "trellis-rust"));
    assert!(policy.traceability_policy.profiles.iter().all(|profile| {
        profile.assurance_level == "declared" && profile.anchor_kind == "exact_marker_bytes"
    }));
    assert_eq!(policy.stack_provenance_policy.mode, "optional");
    assert!(policy
        .stack_provenance_policy
        .allowed_claim_scopes
        .contains(&"stack_provenance.v1.lifecycle_policy_conformance".to_string()));
    assert_eq!(policy.function_address_policy.mode, "optional");
    assert!(policy
        .function_address_policy
        .allowed_claim_scopes
        .contains(&"function_address.v1.lifecycle_policy_conformance".to_string()));
    assert!(policy
        .function_address_policy
        .required_roles
        .contains(&"kamacite-function-address-receipt".to_string()));
    assert_eq!(
        policy.nominal_identity_policy.runtime.schema_version,
        crate::verified::nominal_identity::NOMINAL_IDENTITY_RUNTIME_SCHEMA_V1
    );
    assert_eq!(
        policy.nominal_identity_policy.octet.schema_version,
        crate::verified::nominal_identity::NOMINAL_IDENTITY_OCTET_SCHEMA_V1
    );
    assert!(
        crate::verified::nominal_identity::nominal_identity_policy_diagnostics(
            &policy.nominal_identity_policy
        )
        .is_empty()
    );
    let lifecycle_store = &policy.lifecycle_store_policy;
    assert_eq!(lifecycle_store.schema, LIFECYCLE_STORE_POLICY_SCHEMA_V1);
    assert_eq!(
        lifecycle_store.bounds.max_store_id_bytes,
        EXPECTED_MAX_STORE_ID_BYTES
    );
    assert_eq!(
        lifecycle_store.store_identity.projection_schema,
        "cairn.lifecycle-store-identity.v1"
    );
    assert_eq!(
        lifecycle_store.project_config.projection_path,
        ".cairn/project.json"
    );
    assert_eq!(lifecycle_store.root_resolution.root_flag, "--root");
    assert_eq!(
        lifecycle_store.root_resolution.lifecycle_store_flag,
        "--lifecycle-store"
    );
    assert_eq!(
        lifecycle_store.root_resolution.eligible_commands,
        vec![
            "status",
            "spec list",
            "spec show",
            "validate",
            "context",
            "workflow status",
            "workflow instructions",
            "doctor",
        ]
    );
    assert_eq!(
        lifecycle_store.locator.schema,
        "cairn.lifecycle-store-locator.v1"
    );
    assert_eq!(
        lifecycle_store.worksets.state.relative_path,
        "cairn/worksets/state.json"
    );
    assert!(lifecycle_store
        .opener_profiles
        .iter()
        .any(|profile| profile.id == "code-new-window"
            && profile.member_argument_mode == "positional_paths"));
    assert!(lifecycle_store
        .issue_codes
        .contains(&"lifecycle_store.root.conflicting_selectors".to_string()));
    assert_eq!(
        lifecycle_store.receipts.portable_hash_field,
        "portable_hash"
    );
    assert!(lifecycle_store
        .receipts
        .portable_excluded_fields
        .contains(&"resolution.root".to_string()));
    assert_eq!(
        lifecycle_store.receipts.required_non_claims,
        lifecycle_store.required_non_claims
    );
    assert_eq!(
        policy.agent_guidance_policy.schema,
        "cairn.agent-guidance-policy.v1"
    );
    assert!(policy
        .agent_guidance_policy
        .known_operations
        .contains(&"workflow-instructions".to_string()));
    assert!(policy
        .agent_guidance_policy
        .portable_excluded_fields
        .contains(&"text".to_string()));
    assert_eq!(
        policy.generated_agent_workflow_policy.descriptors.len(),
        crate::verified::generated_agent_workflow::INITIAL_WORKFLOW_IDS.len()
    );
    assert_eq!(
        policy.generated_agent_workflow_policy.adapters.len(),
        crate::verified::generated_agent_workflow::INITIAL_ADAPTER_IDS.len()
    );
    let metadata = policy
        .policy_schema_compatibility
        .expect("generated policy includes schema compatibility metadata");
    assert_eq!(metadata.schema_version, CURRENT_POLICY_SCHEMA_VERSION);
    assert_eq!(metadata.field_set_digest, CURRENT_POLICY_FIELD_SET_DIGEST);
    assert_eq!(
        crate::verified::evidence::blake3_hex(CURRENT_POLICY_FIELD_SET_DIGEST_MATERIAL.as_bytes()),
        CURRENT_POLICY_FIELD_SET_DIGEST
    );
}

#[test]
fn rejects_invalid_machine_contract_policy() {
    // r[verify cairn.cli_machine_contract.policy.invalid]
    let mut missing = generated_policy_value();
    missing
        .as_object_mut()
        .expect("policy object")
        .remove("machine_contract_policy");
    assert!(matches!(
        validate_policy_json(missing),
        Err(PolicyError::MissingField("machine_contract_policy"))
    ));

    let mut unknown_class = generated_policy_value();
    unknown_class["machine_contract_policy"]["exit_classes"][0]["id"] =
        Value::String("unknown".to_string());
    assert_invalid_field(unknown_class, "machine_contract_policy.exit_classes.id");

    let mut duplicate_code = generated_policy_value();
    let duplicate = duplicate_code["machine_contract_policy"]["diagnostics"][0].clone();
    duplicate_code["machine_contract_policy"]["diagnostics"]
        .as_array_mut()
        .expect("machine diagnostics")
        .push(duplicate);
    assert_invalid_field(duplicate_code, "machine_contract_policy.diagnostics.code");

    let mut unsafe_target = generated_policy_value();
    unsafe_target["machine_contract_policy"]["diagnostics"][0]["target"] =
        Value::String("../secret".to_string());
    assert_invalid_field(unsafe_target, "machine_contract_policy.diagnostics.target");

    let mut unsafe_code = generated_policy_value();
    unsafe_code["machine_contract_policy"]["diagnostics"][0]["code"] =
        Value::String(".hidden".to_string());
    assert_invalid_field(unsafe_code, "machine_contract_policy.diagnostics.code");

    let mut multiline_non_claim = generated_policy_value();
    multiline_non_claim["machine_contract_policy"]["required_non_claims"][0] =
        Value::String("first\rsecond".to_string());
    assert_invalid_field(
        multiline_non_claim,
        "machine_contract_policy.required_non_claims",
    );

    let mut invalid_bound = generated_policy_value();
    invalid_bound["machine_contract_policy"]["bounds"]["max_stdout_bytes"] = Value::from(0);
    assert_invalid_field(
        invalid_bound,
        "machine_contract_policy.bounds.max_stdout_bytes",
    );

    let mut invalid_status = generated_policy_value();
    invalid_status["machine_contract_policy"]["exit_classes"][0]["process_status"] =
        Value::from(INVALID_MACHINE_SUCCESS_STATUS);
    assert_invalid_field(
        invalid_status,
        "machine_contract_policy.exit_classes.process_status",
    );
}

#[test]
fn rejects_missing_or_drifting_nominal_identity_policy() {
    // r[verify cairn.nominal_evidence_identities.policy.deterministic]
    let source = include_str!("../../../../../cairn-policy/generated/cairn-policy.json");

    let mut missing: Value = serde_json::from_str(source).expect("policy JSON");
    missing
        .as_object_mut()
        .expect("policy object")
        .remove("nominal_identity_policy");
    assert!(matches!(
        validate_policy_json(missing),
        Err(PolicyError::MissingField("nominal_identity_policy"))
    ));

    let mut drift: Value = serde_json::from_str(source).expect("policy JSON");
    drift["nominal_identity_policy"]["octet"]["domains"][0]["symbols"] =
        serde_json::json!(["RawString"]);
    assert!(matches!(
        validate_policy_json(drift),
        Err(PolicyError::InvalidField("nominal_identity_policy"))
    ));
}

#[test]
fn rejects_malformed_lifecycle_store_bounds() {
    let mut zero_bound = generated_policy_value();
    zero_bound["lifecycle_store_policy"]["bounds"]["max_store_id_bytes"] = Value::from(0);
    assert_invalid_field(
        zero_bound,
        "lifecycle_store_policy.bounds.max_store_id_bytes",
    );

    let mut wrong_type = generated_policy_value();
    wrong_type["lifecycle_store_policy"]["bounds"]["max_references"] =
        Value::String("many".to_string());
    assert_invalid_field(wrong_type, "lifecycle_store_policy.bounds.max_references");
}

#[test]
fn rejects_duplicate_lifecycle_store_ids_and_issue_codes() {
    let mut duplicate_opener = generated_policy_value();
    let opener = duplicate_opener["lifecycle_store_policy"]["opener_profiles"][0].clone();
    duplicate_opener["lifecycle_store_policy"]["opener_profiles"]
        .as_array_mut()
        .expect("opener profiles")
        .push(opener);
    assert_invalid_field(
        duplicate_opener,
        "lifecycle_store_policy.opener_profiles.id",
    );

    let mut duplicate_issue = generated_policy_value();
    let issue = duplicate_issue["lifecycle_store_policy"]["issue_codes"][0].clone();
    duplicate_issue["lifecycle_store_policy"]["issue_codes"]
        .as_array_mut()
        .expect("issue codes")
        .push(issue);
    assert_invalid_field(duplicate_issue, "lifecycle_store_policy.issue_codes");
}

#[test]
fn rejects_unknown_lifecycle_store_opener_shapes() {
    let mut unknown_mode = generated_policy_value();
    unknown_mode["lifecycle_store_policy"]["opener_profiles"][0]["member_argument_mode"] =
        Value::String("shell_template".to_string());
    assert_invalid_field(
        unknown_mode,
        "lifecycle_store_policy.opener_profiles.member_argument_mode",
    );

    let mut unknown_field = generated_policy_value();
    unknown_field["lifecycle_store_policy"]["opener_profiles"][0]["shell"] =
        Value::String("code {members}".to_string());
    assert_invalid_field(
        unknown_field,
        "lifecycle_store_policy.opener_profiles.shape",
    );
}

#[test]
fn rejects_missing_lifecycle_store_non_claims() {
    let mut missing = generated_policy_value();
    missing["lifecycle_store_policy"]
        .as_object_mut()
        .expect("lifecycle store policy")
        .remove("required_non_claims");
    assert_missing_field(missing, "lifecycle_store_policy.required_non_claims");

    let mut empty_receipt_non_claims = generated_policy_value();
    empty_receipt_non_claims["lifecycle_store_policy"]["receipts"]["required_non_claims"] =
        serde_json::json!([]);
    assert_invalid_field(
        empty_receipt_non_claims,
        "lifecycle_store_policy.receipts.required_non_claims",
    );
}

#[test]
fn rejects_missing_lifecycle_store_required_fields() {
    let mut missing_hash_field = generated_policy_value();
    missing_hash_field["lifecycle_store_policy"]["receipts"]
        .as_object_mut()
        .expect("receipt policy")
        .remove("portable_hash_field");
    assert_missing_field(
        missing_hash_field,
        "lifecycle_store_policy.receipts.portable_hash_field",
    );

    let mut missing_required_name = generated_policy_value();
    missing_required_name["lifecycle_store_policy"]["receipts"]["required_fields"]
        .as_array_mut()
        .expect("required fields")
        .retain(|field| field.as_str() != Some("portable_hash"));
    assert_invalid_field(
        missing_required_name,
        "lifecycle_store_policy.receipts.required_fields",
    );
}

#[test]
fn rejects_lifecycle_store_schema_root_and_local_state_drift() {
    let mut schema = generated_policy_value();
    schema["lifecycle_store_policy"]["schema"] = Value::String("unknown.v1".to_string());
    assert_invalid_field(schema, "lifecycle_store_policy.schema");

    let mut root_flag = generated_policy_value();
    root_flag["lifecycle_store_policy"]["root_resolution"]["root_flag"] =
        Value::String("--workspace".to_string());
    assert_invalid_field(
        root_flag,
        "lifecycle_store_policy.root_resolution.root_flag",
    );

    let mut duplicate_command = generated_policy_value();
    let command = duplicate_command["lifecycle_store_policy"]["root_resolution"]
        ["eligible_commands"][0]
        .clone();
    duplicate_command["lifecycle_store_policy"]["root_resolution"]["eligible_commands"]
        .as_array_mut()
        .expect("eligible commands")
        .push(command);
    assert_invalid_field(
        duplicate_command,
        "lifecycle_store_policy.root_resolution.eligible_commands",
    );

    let mut widened_commands = generated_policy_value();
    widened_commands["lifecycle_store_policy"]["root_resolution"]["eligible_commands"]
        .as_array_mut()
        .expect("eligible commands")
        .push(Value::String("archive".to_string()));
    assert_invalid_field(
        widened_commands,
        "lifecycle_store_policy.root_resolution.eligible_commands",
    );

    let mut locator_schema = generated_policy_value();
    locator_schema["lifecycle_store_policy"]["locator"]["schema"] =
        Value::String("cairn.ambient-locator.v1".to_string());
    assert_invalid_field(locator_schema, "lifecycle_store_policy.locator.schema");
}

#[test]
fn rejects_lifecycle_store_issue_and_portable_field_drift() {
    let mut issue_code = generated_policy_value();
    issue_code["lifecycle_store_policy"]["issue_codes"][0] =
        Value::String("ambient.overclaim".to_string());
    assert_invalid_field(issue_code, "lifecycle_store_policy.issue_codes");

    let mut portable_fields = generated_policy_value();
    portable_fields["lifecycle_store_policy"]["receipts"]["portable_fields"]
        .as_array_mut()
        .expect("portable fields")
        .push(Value::String("root".to_string()));
    assert_invalid_field(
        portable_fields,
        "lifecycle_store_policy.receipts.portable_fields",
    );
}

#[test]
fn rejects_invalid_artifact_substance_policy() {
    // r[verify cairn.lifecycle_validation.tests.negative]
    let source = include_str!("../../../../../cairn-policy/generated/cairn-policy.json");

    let mut zero_threshold: Value = serde_json::from_str(source).expect("policy JSON");
    zero_threshold["gate_policy"]["substance"]["minimum_substantive_tasks"] = Value::from(0);
    assert!(matches!(
        validate_policy_json(zero_threshold),
        Err(PolicyError::InvalidField(
            "gate_policy.substance.minimum_substantive_tasks"
        ))
    ));

    let mut missing_marker: Value = serde_json::from_str(source).expect("policy JSON");
    missing_marker["gate_policy"]["substance"]["dependency_marker_id"] =
        Value::String("unknown-marker".to_string());
    assert!(matches!(
        validate_policy_json(missing_marker),
        Err(PolicyError::InvalidField(
            "gate_policy.substance.dependency_marker_id"
        ))
    ));

    let mut missing_task_marker: Value = serde_json::from_str(source).expect("policy JSON");
    missing_task_marker["task_marker_policy"]["markers"]
        .as_array_mut()
        .expect("marker array")
        .retain(|marker| marker["id"] != TASK_IDENTITY_MARKER_ID);
    assert!(matches!(
        validate_policy_json(missing_task_marker),
        Err(PolicyError::InvalidField("task_marker_policy.markers"))
    ));

    let mut unsupported_target: Value = serde_json::from_str(source).expect("policy JSON");
    unsupported_target["gate_policy"]["substance"]["dependency_target_class"] =
        Value::String("ambient-path".to_string());
    assert!(matches!(
        validate_policy_json(unsupported_target),
        Err(PolicyError::InvalidField(
            "gate_policy.substance.dependency_target_class"
        ))
    ));
}

#[test]
fn policy_module_split_preserves_public_api_exports() {
    // r[verify developer_experience.core_policy_parser_boundary_split.map]
    // r[verify developer_experience.core_policy_parser_boundary_split.modules]
    let _policy: Option<Policy> = None;
    let _task_marker: Option<TaskMarker> = None;
    let _receipt_schema: Option<ReceiptSchema> = None;
    let _exemption_policy: Option<ExemptionMarkerPolicy> = None;
    let _steel_policy: Option<SteelOrchestrationPolicy> = None;
    let _steel_profile: Option<SteelOrchestrationProfile> = None;
    let _traceability_policy: Option<TraceabilityPolicy> = None;
    let _lifecycle_store_policy: Option<LifecycleStorePolicy> = None;
    let _lifecycle_store_bounds: Option<LifecycleStoreBounds> = None;
    let _lifecycle_store_opener: Option<LifecycleStoreOpenerProfile> = None;
    let _stack_provenance_policy: Option<crate::verified::stack_provenance::StackProvenancePolicy> =
        None;
    let _function_address_policy: Option<
        crate::verified::function_address_gates::FunctionAddressPolicy,
    > = None;
    assert_eq!(SUPPORTED_POLICY_VERSION, 1);
}

#[test]
fn rejects_enabled_steel_policy_without_profiles() {
    // r[verify developer_experience.core_policy_parser_boundary_split.tests]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["steel_orchestration_policy"] = serde_json::json!({
        "enabled": true,
        "profiles": []
    });

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "steel_orchestration_policy.profiles"
        ))
    ));
}

#[test]
fn rejects_unknown_steel_profile_mode() {
    // r[verify developer_experience.core_policy_parser_boundary_split.tests]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["steel_orchestration_policy"]["profiles"][0]["mode"] =
        Value::String("ambient".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "steel_orchestration_policy.profiles.mode"
        ))
    ));
}

#[test]
fn rejects_empty_traceability_profile_id() {
    // r[verify cairn.supersede.traceability_ownership.configured_profiles]
    // r[verify cairn.supersede.traceability_ownership.configured_profiles.malformed]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["id"] = Value::String(String::new());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField("traceability_policy.profiles.id"))
    ));
}

#[test]
fn rejects_unsupported_traceability_marker_verb() {
    // r[verify cairn.supersede.traceability_ownership.configured_profiles]
    // r[verify cairn.supersede.traceability_ownership.configured_profiles.malformed]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["marker_verbs"] =
        serde_json::json!(["impl", "claims"]);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "traceability_policy.profiles.marker_verbs"
        ))
    ));
}

#[test]
fn rejects_unsafe_traceability_source_path() {
    // r[verify cairn.supersede.traceability_ownership.configured_profiles]
    // r[verify cairn.supersede.traceability_ownership.configured_profiles.malformed]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["requirement_sources"][0]["root"] =
        Value::String("../requirements".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "traceability_policy.profiles.sources.root"
        ))
    ));
}

#[test]
fn rejects_unsupported_traceability_source_kind() {
    // r[verify cairn.supersede.traceability_ownership.configured_profiles]
    // r[verify cairn.supersede.traceability_ownership.configured_profiles.malformed]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["evidence_sources"][0]["source_kind"] =
        Value::String("ambient_source".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "traceability_policy.profiles.sources.source_kind"
        ))
    ));
}

#[test]
fn rejects_unsupported_traceability_assurance_level() {
    // r[verify cairn.trace_index.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["assurance_level"] =
        Value::String("structural".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "traceability_policy.profiles.assurance_level"
        ))
    ));
}

#[test]
fn rejects_unsupported_traceability_anchor_kind() {
    // r[verify cairn.trace_index.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["traceability_policy"]["profiles"][0]["anchor_kind"] =
        Value::String("line_number".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "traceability_policy.profiles.anchor_kind"
        ))
    ));
}

#[test]
fn rejects_unknown_stack_provenance_mode() {
    // r[verify cairn.stack_provenance_gates.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["stack_provenance_gate"]["mode"] = Value::String("ambient".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField("stack_provenance_gate.mode"))
    ));
}

#[test]
fn rejects_stale_stack_provenance_policy_hash_shape() {
    // r[verify cairn.stack_provenance_gates.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["stack_provenance_gate"]["accepted_valence_policy_hashes"] =
        serde_json::json!(["not-a-hash"]);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "stack_provenance_gate.accepted_valence_policy_hashes"
        ))
    ));
}

#[test]
fn rejects_unknown_function_address_mode() {
    // r[verify cairn.function_address_gates.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["function_address_gate"]["mode"] = Value::String("ambient".to_string());

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField("function_address_gate.mode"))
    ));
}

#[test]
fn rejects_stale_function_address_policy_hash_shape() {
    // r[verify cairn.function_address_gates.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["function_address_gate"]["accepted_valence_policy_hashes"] =
        serde_json::json!(["not-a-hash"]);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "function_address_gate.accepted_valence_policy_hashes"
        ))
    ));
}

#[test]
fn rejects_empty_function_address_required_roles() {
    // r[verify cairn.function_address_gates.policy]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["function_address_gate"]["required_roles"] = serde_json::json!([]);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "function_address_gate.required_roles"
        ))
    ));
}

#[test]
fn rejects_unknown_runtime_capture_origin() {
    // r[verify cairn.runtime_evidence.policy_schema]
    // r[verify cairn.runtime_evidence.policy_schema.public]
    // r[verify cairn.runtime_evidence.policy_schema.unknown]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["runtime_evidence_policy"]["allowed_capture_origins"] =
        serde_json::json!(["boundary_captured", "ambient"]);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "runtime_evidence_policy.allowed_capture_origins"
        ))
    ));
}

#[test]
fn rejects_zero_runtime_summary_limit() {
    // r[verify cairn.runtime_evidence.policy_schema]
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json");
    value["runtime_evidence_policy"]["max_summary_bytes"] = Value::from(0);

    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::InvalidField(
            "runtime_evidence_policy.max_summary_bytes"
        ))
    ));
}

#[test]
fn rejects_unsupported_version() {
    // r[verify developer_experience.core_policy_parser_boundary_split.validation]
    let value = serde_json::json!({
        "name": "bad",
        "version": SUPPORTED_POLICY_VERSION + 1,
        "schemas": [{}],
        "task_marker_policy": { "enabled": true, "strictness": "strict" },
        "change_metadata_policy": {
            "allowed_groups": ["current", "future"],
            "group_prefixes": ["feature:"],
            "statuses": ["current", "planned", "blocked", "deferred"]
        },
        "gate_policy": {
            "stages": ["tasks"],
            "ignore_archives": true,
            "finding": { "scope": "tasks", "promoted": "no" }
        },
        "mutation_policy": {
            "sync": { "require_passing_gate": true },
            "archive": { "require_passing_gate": true },
            "migrate": { "require_manifest": true }
        },
        "receipt_hash_policy": { "required": true }
    });
    assert!(matches!(
        validate_policy_json(value),
        Err(PolicyError::UnsupportedVersion { .. })
    ));
}

#[test]
fn classifies_current_generated_policy_metadata() {
    // r[verify cairn.policy_schema_compatibility.fixtures.positive]
    // r[verify cairn.policy_schema_compatibility.classifier]
    // r[verify cairn.policy_schema_compatibility.validation]
    let value = generated_policy_value();
    let report = classify_policy_schema_compatibility(&value, None);
    assert_eq!(report.status, "current");
    assert_eq!(
        report.schema_version.as_deref(),
        Some(CURRENT_POLICY_SCHEMA_VERSION)
    );
    assert_eq!(
        report.field_set_digest.as_deref(),
        Some(CURRENT_POLICY_FIELD_SET_DIGEST)
    );
}

#[test]
fn classifies_missing_metadata_as_refresh_required() {
    // r[verify cairn.policy_schema_compatibility.fixtures.negative]
    // r[verify cairn.policy_schema_compatibility.validation.fixtures]
    let mut value = generated_policy_value();
    value
        .as_object_mut()
        .unwrap()
        .remove(POLICY_SCHEMA_METADATA_FIELD);
    let error = validate_policy_json(value.clone()).expect_err("missing metadata fails closed");
    let report = classify_policy_schema_compatibility(&value, Some(&error));
    assert_eq!(report.status, "refresh-required");
    assert_eq!(
        report.missing_field.as_deref(),
        Some(POLICY_SCHEMA_METADATA_FIELD)
    );
}

#[test]
fn classifies_missing_new_required_field_as_refresh_required() {
    // r[verify cairn.policy_schema_compatibility.fixtures.negative]
    // r[verify cairn.policy_schema_compatibility.diagnostics]
    let mut value = generated_policy_value();
    value
        .as_object_mut()
        .unwrap()
        .remove("steel_orchestration_policy");
    let error = validate_policy_json(value.clone()).expect_err("missing new field fails");
    let report = classify_policy_schema_compatibility(&value, Some(&error));
    assert_eq!(report.status, "refresh-required");
    assert_eq!(
        report.missing_field.as_deref(),
        Some("steel_orchestration_policy")
    );
    let diagnostic = policy_schema_compatibility_diagnostic(&value, &error)
        .expect("refresh diagnostic is available");
    assert!(diagnostic.contains("policy-refresh-required"));
    assert!(diagnostic.contains("missing_field=steel_orchestration_policy"));
}

#[test]
fn classifies_missing_lifecycle_store_policy_as_refresh_required() {
    let mut value = generated_policy_value();
    value
        .as_object_mut()
        .expect("policy object")
        .remove("lifecycle_store_policy");
    let error = validate_policy_json(value.clone()).expect_err("missing field fails");
    let report = classify_policy_schema_compatibility(&value, Some(&error));
    assert_eq!(report.status, "refresh-required");
    assert_eq!(
        report.missing_field.as_deref(),
        Some("lifecycle_store_policy")
    );
    let diagnostic = policy_schema_compatibility_diagnostic(&value, &error)
        .expect("refresh diagnostic is available");
    assert!(diagnostic.contains("missing_field=lifecycle_store_policy"));
}

#[test]
fn rejects_unsupported_policy_schema_metadata() {
    // r[verify cairn.policy_schema_compatibility.classifier.unsupported]
    // r[verify cairn.policy_schema_compatibility.fixtures.negative]
    let mut value = generated_policy_value();
    value[POLICY_SCHEMA_METADATA_FIELD]["schema_version"] =
        Value::String("cairn.policy-schema.v999".to_string());
    let error = validate_policy_json(value.clone()).expect_err("unsupported schema fails");
    assert!(matches!(error, PolicyError::UnsupportedPolicySchema { .. }));
    let report = classify_policy_schema_compatibility(&value, Some(&error));
    assert_eq!(report.status, "unsupported");
}

#[test]
fn rejects_malformed_policy_schema_metadata() {
    // r[verify cairn.policy_schema_compatibility.fixtures.negative]
    // r[verify cairn.policy_schema_compatibility.validation.fixtures]
    let mut value = generated_policy_value();
    value[POLICY_SCHEMA_METADATA_FIELD]["field_set_digest"] =
        Value::String("not-a-hash".to_string());
    let error = validate_policy_json(value.clone()).expect_err("malformed metadata fails");
    assert!(matches!(
        error,
        PolicyError::InvalidField("policy_schema_compatibility.field_set_digest")
    ));
    let report = classify_policy_schema_compatibility(&value, Some(&error));
    assert_eq!(report.status, "unsupported");
}

fn assert_invalid_field(value: Value, expected_field: &'static str) {
    match validate_policy_json(value) {
        Err(PolicyError::InvalidField(field)) => assert_eq!(field, expected_field),
        other => panic!("expected invalid field {expected_field}, got {other:?}"),
    }
}

fn assert_missing_field(value: Value, expected_field: &'static str) {
    match validate_policy_json(value) {
        Err(PolicyError::MissingField(field)) => assert_eq!(field, expected_field),
        other => panic!("expected missing field {expected_field}, got {other:?}"),
    }
}

fn generated_policy_value() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy json")
}
