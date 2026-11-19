let Contracts = import "contracts.ncl" in
{
  schema = "basalt.world-branch-authority-policy.v1",
  profile_id = "basalt-world-branch-authority-v1",
  generation = Contracts.identity.generation,
  default_mode = "non-branchable",
  rules = Contracts.rules,
  non_claims = Contracts.non_claims,
} | Contracts.Policy
