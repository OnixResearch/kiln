let Source = import "../iroh-observation-profile.ncl" in
let changed = std.record.remove "endpoint" Source.profile in
changed | std.contract.from_predicate (fun value => value == Source.profile)
