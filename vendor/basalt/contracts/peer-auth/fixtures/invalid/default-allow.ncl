let H = import "../helpers.ncl" in
H.policy 1 "within-clock-bound" "fresh" "authenticated-peer-message" "allow" "bounded-comparison-only" H.non_claims
