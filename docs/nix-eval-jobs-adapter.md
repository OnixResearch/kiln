# Nix evaluation job discovery

<!-- r[impl kiln.nix_eval_jobs.nonclaims] -->

Kiln uses `nix-eval-jobs` only as an optional discovery provider. The adapter does not build tasks or change canonical CI state directly.

## Selected cohort

The initial profile admits this exact stock Nix cohort:

| Fact | Value |
|---|---|
| Upstream project | `NixOS/nix-eval-jobs` |
| Upstream revision | `a0cd02231c58974a6b5aaa3712069b071047162e` |
| Upstream license | `GPL-3.0` |
| Package version | `2.35.0` |
| Source version | `0.1.6` |
| Nix revision | `aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47` |
| Nix version | `2.35.2` |
| Nixpkgs revision | `346dd96ad74dc4457a9db9de4f4f57dab2e5731d` |
| Platform | `x86_64-linux` |
| Executable | `/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs` |
| Executable BLAKE3 | `dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063` |
| Cohort manifest | `/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json` |
| Manifest BLAKE3 | `102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31` |
| Bounded Exec revision | `29dac88ecded94457572db3fdfaaaab95fa91525` |

The upstream executable has no `--version` option. The immutable cohort manifest binds both version values to the exact executable path.

The adapter verifies the executable bytes and manifest bytes before each new discovery. It does not use `PATH` or another evaluator.

## Approach decision

The selected mechanism runs one pinned process behind an application-owned discovery port. Bounded Exec owns process-group timeout, cancellation, output limits, and teardown.

Kiln rejected these mechanisms:

- Linking Nix libraries into `kiln-core` would add private ABI and provider dependencies.
- Copying upstream evaluator source would add maintenance and GPL source-distribution scope to Kiln.
- Converting every jobset to Mantle first would not support conventional external Nix jobsets.
- Using the host `nix-eval-jobs` command would permit silent executable and ABI drift.

## Source admission

The initial profile accepts only flake evaluation from an immutable `/nix/store` source path. Each request also binds these values:

- the source content identity;
- the root selector;
- the target system;
- the pipeline and policy identities;
- the discovery and build-provider profiles;
- the worker and memory bounds;
- the task, record, dependency, metadata, and output bounds.

Floating Git references, registries, expression mode, `--apply`, impurity, forced recursion, and undeclared imports are unavailable.

## Process and stream behavior

The shell clears the process environment. It supplies only the admitted home path and a fixed Nix configuration.

The operator provisions the home path. Nix can retain evaluator cache data there under operator policy.

The fixed argument policy can enable these upstream fields:

- `attr` and `attrPath`;
- `name`, `system`, `storeDir`, and `drvPath`;
- `outputs` and `inputDrvs`;
- `meta` and `requiredSystemFeatures`;
- `cacheStatus`, `isCached`, `neededBuilds`, and `neededSubstitutes`;
- per-job `error` and `fatal` values.

Bounded Exec captures a named maximum byte count. The schema decoder then consumes fixed-size chunks and admits complete newline-delimited records.

Malformed JSON, a partial final line, unknown fields, oversized data, duplicate jobs, conflicting derivations, missing outputs, and dependency cycles fail closed.

A successful process exit does not complete discovery by itself. The stream, stderr, process, and GC-root facts must also be complete and consistent.

Timeout, cancellation, signal loss, output overflow, provider error, and unknown remain separate outcomes. An unknown observation can only reconcile to its recorded value.

## Task mapping

The pure core maps admitted successful jobs into task proposals. Each BLAKE3 mapping identity binds these facts:

- the discovery schema;
- the source, selector, and system;
- the pipeline and mapping policy;
- the upstream attribute and derivation;
- the output names and paths;
- the selected build provider.

Input derivations create task dependencies only when they match another discovered job. External build inputs remain provider facts.

Per-job evaluation errors remain visible. They do not suppress independent successful jobs after a complete process outcome.

The process shell cannot create tasks directly. `admit_discovery_observation` repeats the canonical mapping before it returns task proposals.

The core also recomputes the complete observation identity. Restored observations cannot substitute process, stream, task, or GC-root facts.

## Cache advisories

`cacheStatus` remains advisory. The deprecated `isCached` field must agree with the current field.

Cache observations cannot complete tasks, skip build-provider admission, authorize substitution, or prove output presence.

## GC-root handoff

The adapter gives each discovery an owned GC-root directory. The observed roots must exactly match the successful derivation records.

The adapter retains roots until every proposed task has a durable downstream handoff. Cleanup before that handoff fails.

A cleanup error changes the root state to `CleanupFailed`. It does not become clean completion.

## Onix boundary

The selected operational cohort is stock Nix only. A discovery plan can carry separate evaluator, manifest, host ABI, feature, plugin, and linked-library identities for an explicitly named non-stock cohort; the stock adapter denies such a plan before creating GC roots or launching a process. Absent cohort facts retain the stock observation identity and serialized request shape.

`onix-nix-eval-v1` currently exposes **candidate-only measured-fact checks, not an executable provider profile**: `admit_onix_candidate` rejects evaluator, feature, plugin, ABI-label, and linked-library drift, then **always returns `onix_source_unpublished`**, even if those facts match. There is no approved Onix manifest hash or published OnixOS jobset identity to match against `manifest_identity` and `source_identity`. A `/nix/store/` path prefix is not evidence of immutable publication. Linux `verify_onix_candidate_files` hashes the measured evaluator, plugin, and seven evaluator-linked libraries using bounded, no-final-symlink file opens without launching a jobset; it cannot authorize a source. The evaluator derives from `NixOS/nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a` with two explicit patches against `onixcomputer/nix@f93f28ad9dce007baae13aa461c16abf656d1171`. Its seven library files are **not** the separately built OnixOS fork CLI closure, and matching Nix version strings do not prove ABI equivalence.

The fork-linked evaluator produced two JSONL derivation rows (`hello`, `jq`) against a read-only dirty-path OnixOS store snapshot. Disabling `wasm-builtin` failed as expected, and substituting a nonexistent plugin path into a temporary source copy failed in pure evaluation. The onixpkgs fork-linked package has a GitHub review commit `3bdf51c00947ffd6b8bd3355ff83509a03752042`, but that unmerged review branch is not a production release pin. These trials do not establish a published immutable OnixOS Git jobset, authenticated source-bound plugin expression, or an immutable evaluator/fixture manifest. No Onix profile is admitted until those publication prerequisites exist. ABI equivalence with the separately built OnixOS CLI is neither asserted nor required. Hashing an installed plugin does not constrain an arbitrary remote flake's own `builtins.wasm` path; the path belongs in the published source-bound expression, not a speculative `nix-eval-jobs --plugin` flag.

## Evidence and non-claims

The pinned upstream package passes its unit suite. The Kiln Nix check also runs positive job, job-error, dependency, cache, GC-root, and `builtins.wasm` fixtures.

This evidence does not prove these properties:

- Nix evaluation correctness;
- source trust;
- complete dependency semantics;
- cache truth;
- build execution or output correctness;
- Onix Nix compatibility;
- reproducibility;
- release readiness.
