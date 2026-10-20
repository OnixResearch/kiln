# r[verify kiln.nix_eval_jobs_onix.nonclaims]
let Source = import "../nix-eval-jobs-onix-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "required_non_claims" ["discovery proves deployment safety"] Source.profile | Source.Profile
