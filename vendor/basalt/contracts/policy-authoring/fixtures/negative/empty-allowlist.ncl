# Negative fixture: resource and ability allowlists must be non-empty.
let Contracts = import "../../contracts.ncl" in
{
  schema_version = "ucan-nickel-contracts.policy.v1",
  backends = {},
  contracts = {
    document-write = {
      id = "document-write",
      description = "Empty allowlist.",
      resource_prefixes = [],
      abilities = [],
    },
  },
} | Contracts.Policy
