let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-missing-verification-receipt",
    input.verification_receipts = [],
    expected = { allowed = false, diagnostic = "missing-verification-receipt" },
  }
) | Contracts.Fixture
