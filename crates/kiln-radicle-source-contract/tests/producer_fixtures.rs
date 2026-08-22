use kiln_radicle_source_contract::{SourceReceipt, admit_receipt};
use serde::Deserialize;

const FIXTURES: &str =
    include_str!("../../../fixtures/radicle-source-contract/producer/fixtures-v1.json");

#[derive(Deserialize)]
struct RejectedFixture {
    expected_error: String,
    receipt: SourceReceipt,
}

#[derive(Deserialize)]
struct FixtureBundle {
    schema: String,
    accepted: SourceReceipt,
    rejected: Vec<RejectedFixture>,
}

#[test]
fn producer_fixtures_match_independent_admission() -> Result<(), Box<dyn std::error::Error>> {
    let bundle: FixtureBundle = serde_json::from_str(FIXTURES)?;
    if bundle.schema != "kiln.radicle-source-fixtures.v1" {
        return Err(std::io::Error::other("unexpected fixture schema").into());
    }
    admit_receipt(&bundle.accepted).map_err(|error| std::io::Error::other(error.to_string()))?;
    if bundle.rejected.is_empty() {
        return Err(std::io::Error::other("negative fixture set is empty").into());
    }
    for fixture in bundle.rejected {
        match admit_receipt(&fixture.receipt) {
            Ok(()) => {
                return Err(std::io::Error::other("negative producer fixture passed").into());
            }
            Err(error) if error.code() != fixture.expected_error => {
                return Err(std::io::Error::other("negative fixture error changed").into());
            }
            Err(_) => {}
        }
    }
    Ok(())
}
