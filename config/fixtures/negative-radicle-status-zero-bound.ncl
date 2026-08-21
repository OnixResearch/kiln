let Source = import "../radicle-status-profile.ncl" in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
0 | Positive