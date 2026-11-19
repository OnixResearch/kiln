let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-token-possession-overclaim",
    input.verified_grants = [],
    input.verification_receipts = [],
    input.claims = ["token-possession"],
    expected = { allowed = false, diagnostic = "authority-overclaim" },
  }
) | Contracts.Fixture
