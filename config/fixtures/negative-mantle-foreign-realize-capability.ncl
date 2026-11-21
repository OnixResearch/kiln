let Source = import "../mantle-foreign-realize-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "capabilities" ["cancel", "dispatch", "observe", "reconcile"] Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
