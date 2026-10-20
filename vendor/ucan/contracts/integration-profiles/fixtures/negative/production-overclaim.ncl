let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-production-overclaim",
  non_claims | force = Base.non_claims @ ["This profile proves production readiness."],
}) | Contracts.IntegrationProfile
