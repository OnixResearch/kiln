let H = import "../helpers.ncl" in
H.policy H.identity.algorithm 0 H.bounds.jwks_keys false true true ["signature proves authority"]
