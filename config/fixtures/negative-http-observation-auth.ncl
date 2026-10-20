let Source = import "../http-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let auth = replace "kind" "bearer" Source.profile.auth in
replace "auth" auth Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
