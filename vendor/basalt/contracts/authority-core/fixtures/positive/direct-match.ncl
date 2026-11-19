let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & { fixture_id = "allow-direct-match" }
) | Contracts.Fixture
