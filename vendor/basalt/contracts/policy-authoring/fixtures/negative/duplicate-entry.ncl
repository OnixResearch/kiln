# Negative fixture: allowlist contains a duplicate normalized entry.
let Contracts = import "../../contracts.ncl" in
{
  schema_version = "ucan-nickel-contracts.policy.v1",
  backends = {},
  contracts = {
    document-write = {
      id = "document-write",
      description = "Duplicate ability.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/write", "document/write"],
    },
  },
} | Contracts.Policy
