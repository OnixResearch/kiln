let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-stale-verification-ref",
    input.verification_receipts = [{
      receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      request_ref = "blake3:1111111111111111111111111111111111111111111111111111111111111111",
      verified_grant_refs = [
        "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      ],
      disposition = "stale",
    }],
    expected = { allowed = false, diagnostic = "stale-verification-ref" },
  }
) | Contracts.Fixture
