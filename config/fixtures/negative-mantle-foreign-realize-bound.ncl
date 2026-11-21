let Source = import "../mantle-foreign-realize-profile.ncl" in
let limits = std.record.insert "max_roots" 2 (std.record.remove "max_roots" Source.profile.limits) in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "limits" limits Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
