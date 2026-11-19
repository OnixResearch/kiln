let Contracts = import "../../contracts.ncl" in
{
  schema = "basalt.ucan-protocol-cohort.v1",
  protocol_profile = "onix-compact-ed25519-v1",
  runtime_revision = Contracts.identity.runtime_revision,
  support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
  basalt_policy_blake3 = Contracts.identity.basalt_policy_blake3,
  compact_adapter = true,
  final_adapter = true,
  non_claims = Contracts.required_non_claims,
} | Contracts.Cohort
