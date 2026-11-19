let Contracts = import "../../contracts.ncl" in
{
  schema_version = "ucan-nickel-contracts.policy.v1",
  backends = {
    nickel = {
      id = "nickel",
      description = "Nickel-authored static policy contracts.",
      supports_higher_order = false,
    },
  },
  contracts = {
    document-read = {
      id = "document-read",
      description = "Read bounded document resources.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/read"],
    },
    document-write = {
      id = "document-write",
      description = "Write bounded document resources.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/write"],
    },
  },
} | Contracts.Policy
