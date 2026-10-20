let Source = import "../observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "views" ["state_summary", "unknown_view"] Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
