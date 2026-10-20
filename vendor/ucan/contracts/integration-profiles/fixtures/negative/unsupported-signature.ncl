let Contracts = import "../../contracts.ncl" in
((import "../../profiles/base.ncl") & {
  id | force = "ucan.negative-unsupported-signature",
  signing_algorithms | force = ["non-EdDSA"],
}) | Contracts.IntegrationProfile
