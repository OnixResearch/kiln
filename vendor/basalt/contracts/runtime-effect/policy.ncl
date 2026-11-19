let Contracts = import "contracts.ncl" in
{
  schema = "basalt.runtime-effect-policy.v1",
  profile_id = "basalt-runtime-effects-v1",
  generation = Contracts.identity.generation,
  default_decision = "deny",
  require_contract = true,
  require_verified_ucan = true,
  rules = Contracts.rules,
  reference = {
    repository = "https://github.com/rivet-dev/agentos",
    revision = Contracts.identity.agentos_revision,
    vm_config_blob = Contracts.identity.agentos_vm_config_blob,
    evaluation_blob = Contracts.identity.agentos_evaluation_blob,
    default_deny_blob = Contracts.identity.agentos_default_deny_blob,
    license_blob = Contracts.identity.agentos_license_blob,
    license = "Apache-2.0",
    claim = "bounded-comparison-only",
  },
  non_claims = Contracts.non_claims,
} | Contracts.Policy
