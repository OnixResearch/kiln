# Tasks

## Phase 1: Cohort and baseline

- [x] [serial] Record current stock-profile, request, stream, task-mapping, process, GC-root, and negative-fixture baselines before core changes. r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.fixtures]
- [x] [serial] Select and record the exact Onix `nix-eval-jobs`, Onix Nix, Wasm host ABI, plugin, platform, feature, manifest, source, and license cohort. r[kiln.nix_eval_jobs_onix.cohort]
- [x] [serial] Add a separate named Onix profile with no automatic selection and preserve the existing stock default. r[kiln.nix_eval_jobs_onix.profile] r[kiln.nix_eval_jobs_onix.compatibility]
- [x] [serial] Extend pure request admission and discovery identities with all Onix cohort facts. r[kiln.nix_eval_jobs_onix.admission] r[kiln.nix_eval_jobs_onix.functional_core]

## Phase 2: Bounded shell and classification

- [x] [serial] Reuse the existing Bounded Exec shell with exact executable and plugin paths, clear environment, fixed configuration, stream bounds, deadline, cancellation, teardown, and GC-root ownership. r[kiln.nix_eval_jobs_onix.shell]
- [x] [serial] Add stable stock unsupported-capability and Onix feature, ABI, plugin, and profile-drift outcomes before process launch where observations permit. r[kiln.nix_eval_jobs_onix.fallback]
- [x] [serial] Prohibit stock, fork, plugin, `PATH`, registry, and ambient-command fallback and require a new explicit request for another profile. r[kiln.nix_eval_jobs_onix.fallback]
- [x] [parallel] Add dependency-isolation tests that keep Onix Nix and plugins out of core and stock-only builds. r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.functional_core]

## Phase 3: Fixtures and evidence

- [x] [serial] Import one immutable minimal OnixOS jobset that invokes the actual Nickel plugin through `builtins.wasm` without a sibling-path dependency. r[kiln.nix_eval_jobs_onix.fixtures]
- [x] [parallel] Add positive Onix discovery and existing task-mapping fixtures under the exact cohort. r[kiln.nix_eval_jobs_onix.fixtures]
- [x] [parallel] Add negative stock profile, missing feature, wrong ABI, stale plugin, mutable source, malformed stream, timeout, cleanup, fallback, and overclaim fixtures. r[kiln.nix_eval_jobs_onix.fixtures] r[kiln.nix_eval_jobs_onix.nonclaims]
- [x] [serial] Update adapter, operator, architecture, cohort, license, and non-claim documentation while keeping stock behavior unchanged. r[kiln.nix_eval_jobs_onix.profile] r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.nonclaims]
- [x] [serial] Run focused tests before and after changes, stock and Onix profile conformance, workspace tests, Clippy with warnings denied, Octet, Cairn validation and gates, and relevant Nix checks. Final focused rail passed 20 adapter tests, three process tests and the real ignored-by-default Onix scenario; workspace regression passed 496 tests and strict all-feature Clippy. Named Nix checks cover Nickel, format, no-std, stock cohort and Octet; their exact subjects and outcomes are retained in `evidence/verification.json`, separately from the passing optional Onix cohort measurement. Native sync accepted all nine Onix requirements; traceability passed with 772 requirements and no missing or dangling references. r[kiln.nix_eval_jobs_onix.fixtures]

## Observed pre-change stock baseline and initial external dependency

- Before editing discovery core, `nix develop path:$PWD --command cargo test -p kiln-adapter-nix-eval` passed 13 adapter unit tests, covering stock profile/manifest admission, immutable source, stream decoding, task mapping, process classes, and GC-root ownership. With `--features fixture-process`, the same 13 unit tests and three bounded process fixture tests passed.
- `nix build path:$PWD#checks.x86_64-linux.nix-eval-jobs-cohort -L --option builders ''` built the stock producer cohort and created its result link. The stock fixture exercised successful jobs, the isolated job error, dependency/cache/root records, and the negative `builtins.wasm` case reporting `attribute 'wasm' missing`. Its executable BLAKE3, manifest BLAKE3, fork-excluding Nix revision, and fixed feature set remain the stock pins in `crates/kiln-adapter-nix-eval/src/profile.rs` and `flake.nix`.
- The OnixOS owner verified fork source revision `f93f28ad9dce007baae13aa461c16abf656d1171`, plugin source revision `54bf09574c42a21f9f38dc7e01bc44440e6fbc07`, and the source-level `nix_wasm_init_v1` handshake. The onixpkgs owner independently built `NixOS/nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a` with two measured fork-2.36 patches: the resulting 2.36.4 evaluator BLAKE3 is `4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361`. Its seven `libnix*.so.2.36.0` path and BLAKE3 pairs are recorded in `crates/kiln-adapter-nix-eval/src/onix.rs`, separately from the OnixOS direct CLI closure. The actual fork-linked evaluator produced two JSONL derivation rows (`hello`, `jq`) against a read-only dirty-path OnixOS store snapshot; disabling `wasm-builtin` failed with the missing wasm attribute. A temporary copy with a nonexistent source-bound plugin path failed in pure evaluation.
- At the initial prerequisite check, the candidate checker denied `onix_source_unpublished` after measured-fact checks: a dirty store snapshot and plugin hash were not an approved immutable fixture/manifest. The final closed profile instead binds the operator-approved exact operational cohort and canonical source commit below, now published on its dedicated review ref. The package review remains review material, not a production release; a local path, matching version string or hash alone never authorizes an arbitrary source or establishes ABI equivalence.

## Qualified operational cohort

- The canonical fixture is OnixOS commit `69a131d71a509f9c57646b71113f75f39ab637b1`, published on the dedicated `kiln-wasm-discovery-fixture-20261020` review ref. GitHub acknowledged the legacy `onixcomputer/onix-modules` remote and advertised its canonical `onixcomputer/OnixOS` location; no main or Radicle ref was published.
- `runtime/onix-eval-cohort/jobset.source.json` records the verbatim import. Source BLAKE3 is `dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb`; runtime source is `/nix/store/yxjjwvja5c8qfz10b6qh14bjd2bwrpww-kiln-onix-discovery-jobset-v1`. The complete manifest BLAKE3 is `1e7df147057fde6b439541427a51c2d4ba70f6a42ee15d30021d120e9dda8263`.
- The real final Kiln rail emitted `Complete`, retained the independent nonfatal `broken` row, mapped the two Nickel jobs and report-to-seed dependency, denied premature cleanup and released roots only after durable handoff. The actual observation is `evidence/live-observation.json`; `evidence/verification.json` separates this runtime proof from artifact checks and lifecycle gates.
- Typed producer statistics from nix-eval-jobs 2.36 are admitted only by the Onix decoder; malformed statistics and the same field under stock remain schema failures. Statistics do not become CI task facts or canonical observation fields.
- Final runtime review found the inherited stock substring classifier mislabeled the real `kiln-wasm-discovery` throw. The new cohort regression failed before the fix; profile-bound normalization now keeps Onix job errors as `nix_job_evaluation_error` while preserving stock's unsupported-builtin result. The corrected real live observation, 20 adapter tests, three process tests, 496 workspace tests and strict Clippy all passed afterward.
- Durable capability: repeatable exact-cohort Onix discovery through Kiln's existing bounded adapter and task handoff. Immediate consumer: this OnixOS Nickel/WASM jobset. Kiln maintainers own adapter admission and the live rail; OnixOS owns fixture semantics, and the package/plugin producers own their source cohorts. This is discovery compatibility, not build, deployment, release or source-trust authority.

