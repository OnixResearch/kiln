let H = import "../helpers.ncl" in
let rule = std.record.merge H.rules.0 { mode = "ambient-copy" } in
H.policy [rule] "non-branchable" H.generation H.non_claims
