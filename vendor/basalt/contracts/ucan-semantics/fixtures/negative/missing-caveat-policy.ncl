let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-missing-caveat-policy",
    decisions.caveat_evaluation = "missing-policy",
    expected = {
      allowed = false,
      reason_class = "caveat-denied",
    },
  }
) | Contracts.Fixture
