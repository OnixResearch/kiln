let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "allow-delegated-match",
    input.verified_grants = [{
      grant_ref = "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      verification_receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      resource = "did:key:z6Mkresource/docs/report",
      ability = "document/write",
      kind = "delegated",
    }],
  }
) | Contracts.Fixture
