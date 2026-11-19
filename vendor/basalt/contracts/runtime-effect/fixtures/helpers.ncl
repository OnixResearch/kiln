let Contracts = import "../contracts.ncl" in
{
  policy = fun rules default_decision generation reference_revision non_claims =>
    {
      schema = "basalt.runtime-effect-policy.v1",
      profile_id = "basalt-runtime-effects-v1",
      generation = generation,
      default_decision = default_decision,
      require_contract = true,
      require_verified_ucan = true,
      rules = rules,
      reference = {
        repository = "https://github.com/rivet-dev/agentos",
        revision = reference_revision,
        vm_config_blob = Contracts.identity.agentos_vm_config_blob,
        evaluation_blob = Contracts.identity.agentos_evaluation_blob,
        default_deny_blob = Contracts.identity.agentos_default_deny_blob,
        license_blob = Contracts.identity.agentos_license_blob,
        license = "Apache-2.0",
        claim = "bounded-comparison-only",
      },
      non_claims = non_claims,
    } | Contracts.Policy,
  identity = Contracts.identity,
  rules = Contracts.rules,
  non_claims = Contracts.non_claims,
  mappings = Contracts.consumer_mappings,
  rule_priority = Contracts.identity.rule_priority,
}
