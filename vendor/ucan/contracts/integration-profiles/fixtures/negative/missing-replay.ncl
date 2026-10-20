let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-missing-replay",
  integrations | force = {
    key_resolution = "caller-supplied",
    proof_loading = "caller-supplied",
    revocation = "caller-supplied",
    verification_time = "explicit",
    caveat_policy = "caller-supplied",
    signer_custody = "caller-owned",
    backend_failure = "fail-closed",
  },
}) | Contracts.IntegrationProfile
