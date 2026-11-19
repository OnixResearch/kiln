let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-replay",
    input.replay = "denied",
    expected = { allowed = false, diagnostic = "replay-denied" },
  }
) | Contracts.Fixture
