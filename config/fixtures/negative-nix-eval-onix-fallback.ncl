# r[verify kiln.nix_eval_jobs_onix.fallback]
let Source = import "../nix-eval-jobs-onix-profile.ncl" in
std.record.insert "ambient_fallback" true Source.profile | Source.Profile
