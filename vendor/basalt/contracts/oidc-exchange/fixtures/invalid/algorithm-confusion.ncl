let H = import "../helpers.ncl" in
H.policy "HS256" 0 H.bounds.jwks_keys false true true H.non_claims
