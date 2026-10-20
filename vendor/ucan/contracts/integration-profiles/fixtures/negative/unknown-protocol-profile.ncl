let Contracts = import "../../contracts.ncl" in
((import "../../profiles/base.ncl") & {
  id | force = "ucan.negative-unknown-protocol-profile",
  protocol_profile = "hummingbird-v42",
}) | Contracts.IntegrationProfile
