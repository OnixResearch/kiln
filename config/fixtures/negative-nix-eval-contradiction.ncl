let Source = import "../nix-eval-jobs-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let policy = replace "partial_admission" true Source.profile.policy in
replace "policy" policy Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
