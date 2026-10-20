let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-implicit-preview",
  required_surfaces | force = Base.required_surfaces @ ["finite-typed-caveat-helpers"],
}) | Contracts.IntegrationProfile
