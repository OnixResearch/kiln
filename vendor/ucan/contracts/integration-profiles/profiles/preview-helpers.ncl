let Contracts = import "../contracts.ncl" in
((import "base.ncl") & {
  id | force = "ucan.preview-typed-caveat-helpers",
  required_surfaces | force = (import "base.ncl").required_surfaces @ ["finite-typed-caveat-helpers"],
  admitted_preview_surfaces | force = ["finite-typed-caveat-helpers"],
}) | Contracts.IntegrationProfile
