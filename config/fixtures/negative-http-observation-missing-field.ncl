let Source = import "../http-observation-profile.ncl" in
let changed = std.record.remove "bounds" Source.profile in
changed | std.contract.from_predicate (fun value => value == Source.profile)
