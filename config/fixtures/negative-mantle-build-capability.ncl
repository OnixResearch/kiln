let Source = import "../mantle-build-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "capabilities" ["dispatch", "observe", "read_logs", "reconcile"] Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
