let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-invalid-disposition",
    input.replay = "ambient",
  }
) | Contracts.Fixture
