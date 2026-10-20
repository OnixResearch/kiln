let Contracts = import "../../contracts.ncl" in
let Base = import "../../profiles/base.ncl" in
(Base & {
  id | force = "ucan.negative-invalid-limit",
  verification_limits | force = {
    id = "invalid-zero-depth",
    max_proof_depth = 0,
    max_proof_count = Base.verification_limits.max_proof_count,
    max_input_bytes = Base.verification_limits.max_input_bytes,
  },
}) | Contracts.IntegrationProfile
