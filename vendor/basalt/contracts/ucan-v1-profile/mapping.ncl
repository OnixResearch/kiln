let Contracts = import "contracts.ncl" in
{
  schema = "basalt.ucan-v1-command-mapping.v1",
  protocol_profile = "ucan-wg-1.0.0",
  runtime_revision = Contracts.identity.runtime_revision,
  support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
  entries = Contracts.entries,
  non_claims = Contracts.non_claims,
} | Contracts.Mapping
