let Source = import "../http-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let origin = replace "scheme" "http" Source.profile.origin in
replace "origin" origin Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
