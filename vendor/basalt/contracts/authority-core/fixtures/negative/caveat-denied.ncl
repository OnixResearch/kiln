let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-caveat",
    input.caveat = "denied",
    expected = { allowed = false, diagnostic = "caveat-denied" },
  }
) | Contracts.Fixture
