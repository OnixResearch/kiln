let Source = import "../nix-eval-jobs-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let limits = replace "max_line_bytes" 0 Source.profile.limits in
replace "limits" limits Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
