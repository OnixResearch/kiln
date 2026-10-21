let Source = import "../dashboard-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let profile = replace "unexpected_field" true Source.profile in
replace "profile" profile Source | std.contract.from_predicate (fun value => value == Source)
