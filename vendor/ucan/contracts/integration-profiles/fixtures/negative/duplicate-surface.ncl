let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-duplicate-surface",
  required_surfaces | force = Base.required_surfaces @ ["ed25519-did-key-compact-token"],
}) | Contracts.IntegrationProfile
