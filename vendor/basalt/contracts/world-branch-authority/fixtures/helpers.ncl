let Contracts = import "../contracts.ncl" in
{
  policy = fun rules default_mode generation non_claims => {
    schema = "basalt.world-branch-authority-policy.v1",
    profile_id = "basalt-world-branch-authority-v1",
    generation = generation,
    default_mode = default_mode,
    rules = rules,
    non_claims = non_claims,
  } | Contracts.Policy,
  rules = Contracts.rules,
  modes = Contracts.modes,
  non_claims = Contracts.non_claims,
  generation = Contracts.identity.generation,
  priority = Contracts.identity.rule_priority,
  mappings = Contracts.consumer_mappings,
}
