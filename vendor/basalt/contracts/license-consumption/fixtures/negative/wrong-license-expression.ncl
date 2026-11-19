let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "wrong-license-expression",
    input.license_expression = "MIT",
    expected = {
      accepted = false,
      diagnostic = "wrong-license-expression",
    },
  }
) | Contracts.Fixture
