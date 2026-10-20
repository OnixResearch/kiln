let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-missing-revocation",
  integrations | force = {
    key_resolution = "caller-supplied",
    proof_loading = "caller-supplied",
    replay = "caller-supplied",
    verification_time = "explicit",
    caveat_policy = "caller-supplied",
    signer_custody = "caller-owned",
    backend_failure = "fail-closed",
  },
}) | Contracts.IntegrationProfile
