let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-stale-proof",
    decisions.proof_traversal = "stale",
    expected = {
      allowed = false,
      reason_class = "proof-denied",
    },
  }
) | Contracts.Fixture
