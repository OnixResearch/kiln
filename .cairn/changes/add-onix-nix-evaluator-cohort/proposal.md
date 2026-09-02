# Proposal: Add an Onix Nix evaluator cohort

## Why

Kiln's optional `nix-eval-jobs` adapter admits one exact stock Nix cohort. It correctly rejects Onix Nix and `builtins.wasm`, so Kiln cannot discover jobs from OnixOS flakes through the same bounded CI path.

A separately named Onix profile can add that support without changing the stock profile or importing fork implementation code into Kiln.

## What Changes

- Add a second explicit evaluator profile for one exact Onix Nix and `nix-eval-jobs` cohort. r[kiln.nix_eval_jobs_onix.profile]
- Bind the executable, fork revision, Nix version, platform, Wasm host ABI, experimental feature set, plugin bundle, manifest, and argument policy. r[kiln.nix_eval_jobs_onix.cohort]
- Extend pure request admission and observation identity with the selected evaluator profile and plugin identities. r[kiln.nix_eval_jobs_onix.admission]
- Launch only the exact profile through the existing Bounded Exec shell with no ambient `PATH` lookup or stock fallback. r[kiln.nix_eval_jobs_onix.shell] r[kiln.nix_eval_jobs_onix.fallback]
- Add a positive OnixOS `builtins.wasm` jobset and negative stock, missing-feature, wrong-ABI, stale-plugin, malformed-stream, timeout, and overclaim fixtures. r[kiln.nix_eval_jobs_onix.fixtures]
- Preserve the current stock cohort unchanged. r[kiln.nix_eval_jobs_onix.compatibility]

## Impact

- **Core**: profile selection, request admission, identities, and outcome classification remain pure.
- **Shell**: exact executable and plugin file observations remain provider effects.
- **OnixOS**: supplies a frozen fixture and cohort facts. It does not become a Kiln dependency or authority source.
- **Claims**: passing fixtures prove bounded compatibility only, not evaluator correctness or build success.

## Out of Scope

- Replacing the stock profile, linking Nix libraries into Kiln, or copying fork source.
- General support for arbitrary Nix forks, alternate stores, or ambient plugins.
- Build execution, output correctness, deployment safety, reproducibility, or release eligibility.

## Affected Specs

- `nix-eval-jobs-onix-cohort`: profile, cohort, admission, shell, fallback, compatibility, fixtures, and non-claims.
