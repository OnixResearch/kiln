# r[verify kiln.nix_eval_jobs_onix.fixtures]
let Source = import "../nix-eval-jobs-onix-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "source_path" "git+https://github.com/onixcomputer/onix-modules?ref=main" Source.profile | Source.Profile
