let Source = import "../mantle-build-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "plan_identity" "not-a-blake3-identity" Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
