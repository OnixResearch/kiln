# Tasks

## Phase 1: Cohort and baseline

- [x] [serial] Record current stock-profile, request, stream, task-mapping, process, GC-root, and negative-fixture baselines before core changes. r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.fixtures]
- [ ] [serial] Select and record the exact Onix `nix-eval-jobs`, Onix Nix, Wasm host ABI, plugin, platform, feature, manifest, source, and license cohort. r[kiln.nix_eval_jobs_onix.cohort]
- [ ] [serial] Add a separate named Onix profile with no automatic selection and preserve the existing stock default. r[kiln.nix_eval_jobs_onix.profile] r[kiln.nix_eval_jobs_onix.compatibility]
- [ ] [serial] Extend pure request admission and discovery identities with all Onix cohort facts. r[kiln.nix_eval_jobs_onix.admission] r[kiln.nix_eval_jobs_onix.functional_core]

## Phase 2: Bounded shell and classification

- [ ] [serial] Reuse the existing Bounded Exec shell with exact executable and plugin paths, clear environment, fixed configuration, stream bounds, deadline, cancellation, teardown, and GC-root ownership. r[kiln.nix_eval_jobs_onix.shell]
- [ ] [serial] Add stable stock unsupported-capability and Onix feature, ABI, plugin, and profile-drift outcomes before process launch where observations permit. r[kiln.nix_eval_jobs_onix.fallback]
- [ ] [serial] Prohibit stock, fork, plugin, `PATH`, registry, and ambient-command fallback and require a new explicit request for another profile. r[kiln.nix_eval_jobs_onix.fallback]
- [ ] [parallel] Add dependency-isolation tests that keep Onix Nix and plugins out of core and stock-only builds. r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.functional_core]

## Phase 3: Fixtures and evidence

- [ ] [serial] Import one immutable minimal OnixOS jobset that invokes the actual Nickel plugin through `builtins.wasm` without a sibling-path dependency. r[kiln.nix_eval_jobs_onix.fixtures]
- [ ] [parallel] Add positive Onix discovery and existing task-mapping fixtures under the exact cohort. r[kiln.nix_eval_jobs_onix.fixtures]
- [ ] [parallel] Add negative stock profile, missing feature, wrong ABI, stale plugin, mutable source, malformed stream, timeout, cleanup, fallback, and overclaim fixtures. r[kiln.nix_eval_jobs_onix.fixtures] r[kiln.nix_eval_jobs_onix.nonclaims]
- [ ] [serial] Update adapter, operator, architecture, cohort, license, and non-claim documentation while keeping stock behavior unchanged. r[kiln.nix_eval_jobs_onix.profile] r[kiln.nix_eval_jobs_onix.compatibility] r[kiln.nix_eval_jobs_onix.nonclaims]
- [ ] [serial] Run focused tests before and after changes, stock and Onix profile conformance, workspace tests, Clippy with warnings denied, Octet, Cairn validation and gates, and relevant Nix checks. r[kiln.nix_eval_jobs_onix.fixtures]

## Observed pre-change stock baseline and external dependency

- Before editing discovery core, `nix develop path:$PWD --command cargo test -p kiln-adapter-nix-eval` passed 13 adapter unit tests, covering stock profile/manifest admission, immutable source, stream decoding, task mapping, process classes, and GC-root ownership. With `--features fixture-process`, the same 13 unit tests and three bounded process fixture tests passed.
- `nix build path:$PWD#checks.x86_64-linux.nix-eval-jobs-cohort -L --option builders ''` built the stock producer cohort and created its result link. The stock fixture exercised successful jobs, the isolated job error, dependency/cache/root records, and the negative `builtins.wasm` case reporting `attribute 'wasm' missing`. Its executable BLAKE3, manifest BLAKE3, fork-excluding Nix revision, and fixed feature set remain the stock pins in `crates/kiln-adapter-nix-eval/src/profile.rs` and `flake.nix`.
- The OnixOS owner verified fork source revision `f93f28ad9dce007baae13aa461c16abf656d1171`, plugin source revision `54bf09574c42a21f9f38dc7e01bc44440e6fbc07`, and the source-level `nix_wasm_init_v1` handshake. The onixpkgs owner independently built `NixOS/nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a` with two measured fork-2.36 patches: the resulting 2.36.4 evaluator BLAKE3 is `4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361`. Its seven `libnix*.so.2.36.0` path and BLAKE3 pairs are recorded in `crates/kiln-adapter-nix-eval/src/onix.rs`, separately from the OnixOS direct CLI closure. The actual fork-linked evaluator produced two JSONL derivation rows (`hello`, `jq`) against a read-only dirty-path OnixOS store snapshot; disabling `wasm-builtin` failed with the missing wasm attribute. A temporary copy with a nonexistent source-bound plugin path failed in pure evaluation.
- Kiln records candidate-linked libraries in discovery observation identity; the exact candidate checker **always denies `onix_source_unpublished` after measured-fact checks**, because there is no approved Onix manifest or published OnixOS source identity. Even `/nix/store/` prefix or a stock manifest hash proves no Onix source authority. The read-only Linux file checker uses bounded no-follow opens for the evaluator, plugin, and its seven libraries. Stock rejects every Onix request before process launch. The onixpkgs package has a GitHub review commit `3bdf51c00947ffd6b8bd3355ff83509a03752042`, but this unmerged review branch is not the positive production pin; the OnixOS snapshot is **not** a published immutable Git fixture and must not be pinned. Neither a local plugin hash nor a separate direct CLI build authorizes an arbitrary remote flake or proves ABI equivalence; published source must bind the plugin path before positive admission.
