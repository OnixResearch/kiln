let H = import "../helpers.ncl" in
H.policy H.rules "non-branchable" H.generation (std.array.take 1 H.non_claims)
