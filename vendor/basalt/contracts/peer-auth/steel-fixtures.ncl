let Contracts = import "contracts.ncl" in
{
  schema = "basalt.peer-auth-steel-fixtures.v1",
  policy_id = "peer-control-v1",
  cases = Contracts.steel_cases,
  non_claims = Contracts.non_claims,
} | Contracts.SteelFixtureSuite
