let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "weakened-non-claims",
    input.non_claims = [
      "legal-advice",
      "license-exception",
      "compatibility-promise",
    ],
    expected = {
      accepted = false,
      diagnostic = "non-claims-weakened",
    },
  }
) | Contracts.Fixture
