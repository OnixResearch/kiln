# r[verify kiln.nix_eval_jobs_onix.fixtures]
let Source = import "../nix-eval-jobs-onix-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "host_abi" "nix_wasm_init_v2" Source.profile | Source.Profile
