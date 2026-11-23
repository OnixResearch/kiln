const FIXTURE_SCHEMA: &str = "cairn.gate-producer-fixtures.v1";
const FIXTURE_JSON: &str =
    include_str!("../../../fixtures/cairn-gate-contract/producer/fixtures-v1.json");

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    schema: String,
    request: cairn_gate_contract::GateRequest,
    pass_result: cairn_gate_contract::GateResult,
    block_result: cairn_gate_contract::GateResult,
}

#[test]
fn decodes_and_admits() -> Result<(), serde_json::Error> {
    // r[verify cairn.gate_interchange.conformance]
    // r[verify cairn.gate_interchange.conformance.scenario.drift]
    let fixtures: Bundle = serde_json::from_str(FIXTURE_JSON)?;
    assert_eq!(fixtures.schema, FIXTURE_SCHEMA);
    assert_eq!(
        cairn_gate_contract::validate_request(&fixtures.request),
        Ok(())
    );
    assert_eq!(
        cairn_gate_contract::validate_result(&fixtures.request, &fixtures.pass_result),
        Ok(())
    );
    assert_eq!(
        cairn_gate_contract::validate_result(&fixtures.request, &fixtures.block_result),
        Ok(())
    );
    let expected: serde_json::Value = serde_json::from_str(FIXTURE_JSON)?;
    assert_eq!(serde_json::to_value(fixtures)?, expected);
    Ok(())
}

#[test]
fn rejects_changed_content_with_original_identity() -> Result<(), serde_json::Error> {
    let mut fixtures: Bundle = serde_json::from_str(FIXTURE_JSON)?;
    fixtures.request.target.stage = cairn_gate_contract::GateStage::Proposal;
    assert_eq!(
        cairn_gate_contract::validate_request(&fixtures.request),
        Err(cairn_gate_contract::ContractError::RequestIdentityMismatch)
    );
    assert_eq!(
        cairn_gate_contract::validate_result(&fixtures.request, &fixtures.pass_result),
        Err(cairn_gate_contract::ContractError::RequestIdentityMismatch)
    );
    Ok(())
}

#[test]
fn rejects_unknown_fields_and_malformed_scalars() -> Result<(), Box<dyn std::error::Error>> {
    let original: serde_json::Value = serde_json::from_str(FIXTURE_JSON)?;
    let mut extra_field = original.clone();
    let request = extra_field
        .get_mut("request")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("fixture request is not an object")?;
    assert!(request
        .insert("undeclared".into(), serde_json::Value::Bool(true))
        .is_none());
    assert!(serde_json::from_value::<Bundle>(extra_field).is_err());

    let mut invalid_identity = original;
    let request = invalid_identity
        .get_mut("request")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("fixture request is not an object")?;
    assert!(request
        .insert(
            "identity".into(),
            serde_json::Value::String("not-an-identity".into()),
        )
        .is_some());
    assert!(serde_json::from_value::<Bundle>(invalid_identity).is_err());
    Ok(())
}
