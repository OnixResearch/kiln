let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-missing-operation",
    request_ability = "",
  }
) | Contracts.Fixture
