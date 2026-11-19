let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & { fixture_id = "acknowledged-agpl-consumption" }
) | Contracts.Fixture
