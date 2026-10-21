let Source = import "../dashboard-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let profile = replace "watch_interval_ms" 0 Source.profile in
replace "profile" profile Source | std.contract.from_predicate (fun value => value == Source)
