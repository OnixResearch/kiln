let Source = import "../iroh-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let cohort = replace "iroh_version" "0.95.0" Source.profile.cohort in
replace "cohort" cohort Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
