let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "missing-license-review",
    input.review_action = "deferred",
    expected = {
      accepted = false,
      diagnostic = "missing-review",
    },
  }
) | Contracts.Fixture
