let Source = import "../observation-profile.ncl" in
let changed = std.record.remove "max_encoded_bytes" Source.profile.bounds in
let profile = std.record.insert "bounds" changed Source.profile in
profile | std.contract.from_predicate (fun value => value == Source.profile)
