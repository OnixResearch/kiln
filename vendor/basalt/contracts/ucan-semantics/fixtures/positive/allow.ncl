let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "allow-direct-grant",
  }
) | Contracts.Fixture
