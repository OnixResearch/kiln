use cairn_gate_contract::{validate_request, validate_result, GateRequest, GateResult};
use serde::Deserialize;

const PRODUCER_FIXTURE_SCHEMA: &str = "cairn.gate-producer-fixtures.v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerFixtures {
    schema: String,
    request: GateRequest,
    pass_result: GateResult,
    block_result: GateResult,
}

#[test]
fn producer_fixtures_decode_and_admit() -> Result<(), serde_json::Error> {
    // r[verify cairn.gate_interchange.conformance]
    // r[verify cairn.gate_interchange.conformance.scenario.drift]
    let fixtures: ProducerFixtures = serde_json::from_str(include_str!(
        "../../../fixtures/cairn-gate-contract/producer/fixtures-v1.json"
    ))?;
    assert_eq!(fixtures.schema, PRODUCER_FIXTURE_SCHEMA);
    assert_eq!(validate_request(&fixtures.request), Ok(()));
    assert_eq!(
        validate_result(&fixtures.request, &fixtures.pass_result),
        Ok(())
    );
    assert_eq!(
        validate_result(&fixtures.request, &fixtures.block_result),
        Ok(())
    );
    Ok(())
}
