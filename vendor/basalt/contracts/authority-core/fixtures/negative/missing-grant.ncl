let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-missing-grant",
    input.verified_grants = [],
    expected = { allowed = false, diagnostic = "missing-verified-grant" },
  }
) | Contracts.Fixture
