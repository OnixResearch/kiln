let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "unsupported-schema",
    input.schema = "basalt.ucan-authority-input.v1",
    expected = {
      accepted = false,
      diagnostic = "unsupported-schema",
    },
  }
) | Contracts.Fixture
