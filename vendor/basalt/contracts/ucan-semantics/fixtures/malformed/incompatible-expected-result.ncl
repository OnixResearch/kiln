let Contracts = import "../../contracts.ncl" in
(
  (import "../negative/over-delegation.ncl")
  & {
    fixture_id = "malformed-incompatible-expected-result",
    expected = {
      allowed = true,
      reason_class = "allowed",
    },
  }
) | Contracts.Fixture
