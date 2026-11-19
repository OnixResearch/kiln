let H = import "../helpers.ncl" in
H.policy 1 "within-clock-bound" "fresh" "authenticated-peer-message" "deny" "parity" H.non_claims
