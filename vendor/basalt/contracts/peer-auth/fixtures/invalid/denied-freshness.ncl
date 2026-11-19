let H = import "../helpers.ncl" in
H.policy 1 "stale" "fresh" "authenticated-peer-message" "deny" "bounded-comparison-only" H.non_claims
