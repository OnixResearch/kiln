# Add an Exact Nix Build Provider

## Problem

Kiln exposes stock Nix job discovery through `kiln-adapter-nix-eval`, but it has
no provider that executes the discovered derivation. The Mantle adapter proves
the build-provider contract for Mantle, while a Nix job can only be mapped to a
task and dispatched to another provider. That leaves the Nix path incomplete:
the exact `.drv` is not executed, the native realisation is not admitted,
outputs are not retained, and product identities, logs, reconciliation, and an
operator composition are not owned by a Kiln surface.

## Proposed Change

Add `kiln-adapter-nix`, a direct `BuildPort` provider for one exact Nix
derivation, and `kiln-nix-build`, its discovery-task composition and operator
shell:

1. admit one repo-pinned Nix cohort with exact `nix` and `nix-store` identities;
2. bind the Kiln build plan's provider, plan, policy, platform, products, and
   exact expected output store paths before any process starts;
3. execute `nix-store --add-root … --indirect --realise <drv>` through Bounded
   Exec with a cleared environment and fixed `HOME`/`NIX_CONFIG`;
4. retain every realised output under an effect-specific private GC-root
   directory and require every root target to equal the expected store path;
5. admit product, receipt, and bounded stderr log identities and map success,
   failure, timeout, cancellation, provider error, and unknown outcomes;
6. make duplicate dispatch, conflict rejection, observe, reconcile,
   observation-only restore, and explicit root release explicit; and
7. map a discovery `TaskProposal` to the exact profile and build plan, with
   `doctor`, `build`, and `from-task` operator modes.

The first slice supports one exact local derivation with local store inputs.
It does not evaluate flakes, use registries, substitute outputs, or use remote
builders.

## Success Criteria

- The repo-pinned Nix cohort and exact derivation are admitted before execution.
- Executable, `nix-store`, derivation, plan, policy, platform, product, output
  path, or limit drift fails before a process starts.
- A real local `.drv` realisation returns a `Success` observation with every
  expected output admitted exactly once and rooted.
- Roots are released only by explicit idempotent release after durable handoff.
- A nonzero exit is `Failed`; timeout and cancellation remain terminal;
  malformed, missing, duplicate, or path-mismatched output is `ProviderError`;
  ambiguous process start is `Unknown` without a terminal receipt.
- Every non-unknown observation has a bounded receipt, and logs are bounded,
  identity-bound, and unavailable after an observation-only restore.
- A discovery-shaped task maps to the exact profile and plan, builds through
  the real pinned Nix store, and releases its roots.
- Fixture tests, live tests, the repo-pinned cohort check, and the
  fixture-only Nix check pass.

## Impact

- **Core**: no change. The adapter uses the existing `BuildEffectPlan`,
  `BuildEvaluationObservation`, completion admission, and `BuildPort`.
- **Adapter**: exact output binding, `nix-store` rooted realisation, output
  retention/release, and expanded tests.
- **Composition**: new `kiln-nix-build` library and operator binary.
- **Nix**: `nix-build-adapter-fixtures` and `nix-build-adapter-cohort` checks,
  plus the host `tests/nix-build-live.sh` rail.
- **Claims**: a successful observation remains scoped to the exact executable,
  derivation, local inputs, store, rooted outputs, and native realisation. It
  does not prove build correctness, sandbox completeness, derivation
  provenance, output semantics, cache truth, reproducibility, or release
  readiness.

## Out of Scope

- Flake evaluation, registry access, substitution, remote builders, or network
  acquisition.
- A general Nix store observer; `kiln-adapter-nix-eval` retains discovery and
  derivation GC-root handoff, while this adapter owns only build-output roots.
- Changes to the canonical CI state machine or build-product meaning.
- Publishing a Nix package or claiming a deployment.
