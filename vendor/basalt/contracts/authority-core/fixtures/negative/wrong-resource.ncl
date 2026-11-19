let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-wrong-resource",
    input.verified_grants = [{
      grant_ref = "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      verification_receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      resource = "did:key:z6Mkresource/docs/other",
      ability = "document/write",
      kind = "direct",
    }],
    expected = { allowed = false, diagnostic = "wrong-resource" },
  }
) | Contracts.Fixture
