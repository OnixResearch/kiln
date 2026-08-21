let Source = import "../pijul-source-profile.ncl" in
let changed = std.record.remove "max_total_bytes" Source.profile.limits in
changed | std.contract.from_predicate (fun value => value == Source.profile.limits)
