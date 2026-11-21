let Source = import "../mantle-foreign-realize-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "revision" "0000000000000000000000000000000000000000" Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
