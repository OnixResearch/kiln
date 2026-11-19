let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-replay-backend",
    decisions.replay = "backend-error",
    expected = {
      allowed = false,
      reason_class = "replay-denied",
    },
  }
) | Contracts.Fixture
