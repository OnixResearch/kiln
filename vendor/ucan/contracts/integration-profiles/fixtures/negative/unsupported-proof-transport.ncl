let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-unsupported-proof-transport",
  required_surfaces | force = Base.required_surfaces @ ["proof-transport-backends"],
}) | Contracts.IntegrationProfile
