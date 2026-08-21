let Source = import "../radicle-mutation-profile.ncl" in
let _Profile = Source.profile in
0 | std.contract.from_predicate (fun value => std.is_number value && value > 0)