# Negative fixture: backend id is outside the supported Rust vocabulary.
let Contracts = import "../../contracts.ncl" in
{
  schema_version = "ucan-nickel-contracts.policy.v1",
  backends = {
    rust = {
      id = "rust",
      description = "Unsupported backend.",
      supports_higher_order = false,
    },
  },
  contracts = {
    document-write = {
      id = "document-write",
      description = "Uses an unsupported backend.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/write"],
    },
  },
} | Contracts.Policy
