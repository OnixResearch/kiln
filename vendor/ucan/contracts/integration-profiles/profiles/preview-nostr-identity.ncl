let Contracts = import "../contracts.ncl" in
((import "base.ncl") & {
  id | force = "ucan.preview-nostr-identity-adapter",
  required_surfaces | force = (import "base.ncl").required_surfaces @ ["nostr-identity-adapter"],
  admitted_preview_surfaces | force = ["nostr-identity-adapter"],
}) | Contracts.IntegrationProfile
