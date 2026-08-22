use kiln_radicle_source_contract::{FeatureLevel, SourceReceipt, admit_receipt, fixture_receipt};
use serde::Serialize;

#[derive(Serialize)]
struct RejectedFixture {
    name: &'static str,
    expected_error: &'static str,
    receipt: SourceReceipt,
}

#[derive(Serialize)]
struct FixtureBundle {
    schema: &'static str,
    accepted: SourceReceipt,
    rejected: Vec<RejectedFixture>,
}

fn rejected(name: &'static str, receipt: SourceReceipt) -> Result<RejectedFixture, std::io::Error> {
    match admit_receipt(&receipt) {
        Err(error) => Ok(RejectedFixture {
            name,
            expected_error: error.code(),
            receipt,
        }),
        Ok(()) => Err(std::io::Error::other(
            "negative Radicle source fixture passed admission",
        )),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let accepted = fixture_receipt();
    admit_receipt(&accepted).map_err(|error| std::io::Error::other(error.to_string()))?;

    let mut moved = fixture_receipt();
    moved.observation.sigrefs_commit_after = "7777777777777777777777777777777777777777".to_string();

    let mut signer = fixture_receipt();
    signer.observation.signer_node_id = "z6MkOtherSigner".to_string();

    let mut feature = fixture_receipt();
    feature.observation.feature_level = FeatureLevel::Root;

    let mut revision = fixture_receipt();
    revision.snapshot.revision = "8888888888888888888888888888888888888888".to_string();

    let mut digest = fixture_receipt();
    digest.snapshot.canonical_tree_blake3 = "not-a-digest".to_string();

    let mut bound = fixture_receipt();
    bound.snapshot.entry_count = bound.request.limits.max_entries + 1;

    let bundle = FixtureBundle {
        schema: "kiln.radicle-source-fixtures.v1",
        accepted,
        rejected: vec![
            rejected("moved-sigrefs", moved)?,
            rejected("signer-substitution", signer)?,
            rejected("feature-downgrade", feature)?,
            rejected("revision-substitution", revision)?,
            rejected("malformed-digest", digest)?,
            rejected("snapshot-bound", bound)?,
        ],
    };
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
