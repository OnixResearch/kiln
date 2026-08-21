let Source = import "../nix-eval-jobs-profile.ncl" in
let changed = std.record.remove "nix_version" Source.profile in
changed | std.contract.from_predicate (fun value => value == Source.profile)
