let Source = import "../nix-eval-jobs-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "nix_version" "2.34.0" Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
