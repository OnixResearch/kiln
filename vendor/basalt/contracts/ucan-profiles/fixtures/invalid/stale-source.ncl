let Contracts = import "../../contracts.ncl" in
{
  schema = "basalt.ucan-protocol-cohort.v1",
  protocol_profile = "onix-compact-ed25519-v1",
  runtime_revision = "277ab768eb532ac0c715a3bbff2a7dc37a702f11",
  support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
  basalt_policy_blake3 = Contracts.identity.basalt_policy_blake3,
  compact_adapter = true,
  final_adapter = false,
  non_claims = Contracts.required_non_claims,
} | Contracts.Cohort
