let Source = import "../cairn-gate-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "command" "release-readiness" Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
