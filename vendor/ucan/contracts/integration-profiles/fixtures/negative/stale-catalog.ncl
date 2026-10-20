let Contracts = import "../../contracts.ncl" in
((import "../../profiles/base.ncl") & {
  id | force = "ucan.negative-stale-catalog",
  catalog_blake3 | force = "0000000000000000000000000000000000000000000000000000000000000000",
}) | Contracts.IntegrationProfile
