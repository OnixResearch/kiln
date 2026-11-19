let H = import "../helpers.ncl" in
H.policy 2 "within-clock-bound" "fresh" "authenticated-peer-message" "deny" "bounded-comparison-only" H.non_claims
