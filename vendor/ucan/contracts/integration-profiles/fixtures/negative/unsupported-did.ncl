let Contracts = import "../../contracts.ncl" in
((import "../../profiles/base.ncl") & {
  id | force = "ucan.negative-unsupported-did",
  did_methods | force = ["remote-did-method"],
}) | Contracts.IntegrationProfile
