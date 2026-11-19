let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-verification-request-mismatch",
    input.verification_receipts = [{
      receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      request_ref = "blake3:4444444444444444444444444444444444444444444444444444444444444444",
      verified_grant_refs = [
        "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      ],
      disposition = "current",
    }],
    expected = { allowed = false, diagnostic = "verification-request-mismatch" },
  }
) | Contracts.Fixture
