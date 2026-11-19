let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-wrong-ability",
    input.verified_grants = [{
      grant_ref = "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      verification_receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      resource = "did:key:z6Mkresource/docs/report",
      ability = "document/read",
      kind = "direct",
    }],
    expected = { allowed = false, diagnostic = "wrong-ability" },
  }
) | Contracts.Fixture
