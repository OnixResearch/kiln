let Contracts = import "../../contracts.ncl" in
{
  schema = "ucan.integration-fixture-manifest.v1",
  id = "ucan.public-ed25519-allow",
  profile_id = "ucan.controlled-ed25519-did-key",
  public_input_blake3 = "1111111111111111111111111111111111111111111111111111111111111111",
  public_reference_blake3 = "2222222222222222222222222222222222222222222222222222222222222222",
  expected_result = "allow",
  required_surfaces = ["ed25519-did-key-compact-token"],
  non_claims = [
    "Profile conformance does not prove resolver truth.",
    "Profile conformance does not prove key custody or cryptographic correctness.",
    "Profile conformance does not prove revocation or replay freshness.",
    "Profile conformance does not prove backend durability.",
    "Profile conformance does not prove application authorization completeness.",
  ],
} | Contracts.FixtureManifest
