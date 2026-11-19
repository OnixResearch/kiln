let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-weakened-non-claims",
    non_claims = [
      "revocation-backend-freshness",
      "key-resolution-truth",
      "ucan-cryptographic-correctness",
      "consumer-replay-store-durability",
    ],
  }
) | Contracts.Fixture
