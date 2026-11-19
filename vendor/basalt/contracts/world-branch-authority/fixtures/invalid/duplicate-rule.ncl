let H = import "../helpers.ncl" in
H.policy (H.rules @ [H.rules.0]) "non-branchable" H.generation H.non_claims
