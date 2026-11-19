let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-weakened-non-claims",
    input.non_claims = [
      "transport-authority",
      "storage-authority",
      "persistence-authority",
      "replay-store-truth",
      "key-resolution-trust",
      "downstream-business-authority",
      "release-eligibility",
    ],
  }
) | Contracts.Fixture
