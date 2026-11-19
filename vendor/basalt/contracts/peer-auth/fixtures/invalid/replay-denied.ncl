let H = import "../helpers.ncl" in
H.policy 1 "within-clock-bound" "replayed" "authenticated-peer-message" "deny" "bounded-comparison-only" H.non_claims
