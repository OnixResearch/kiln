# Negative fixture: ability has malformed path segments (empty segment).
let Contracts = import "../../contracts.ncl" in
{
  schema_version = "ucan-nickel-contracts.policy.v1",
  backends = {},
  contracts = {
    document-write = {
      id = "document-write",
      description = "Malformed ability path.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/"],
    },
  },
} | Contracts.Policy
