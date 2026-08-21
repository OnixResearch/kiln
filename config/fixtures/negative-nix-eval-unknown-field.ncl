let Source = import "../nix-eval-jobs-profile.ncl" in
std.record.insert "ambient_fallback" true Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
