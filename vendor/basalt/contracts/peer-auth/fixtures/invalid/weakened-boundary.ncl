let H = import "../helpers.ncl" in
H.policy 1 "within-clock-bound" "fresh" "authenticated-peer-message" "deny" "bounded-comparison-only" ["transport is built in"]
