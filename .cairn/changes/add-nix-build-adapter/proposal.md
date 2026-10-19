# Add an Exact Nix Build Provider

## Problem

Kiln exposes stock Nix job discovery through `kiln-adapter-nix-eval`, but it has
no provider that executes the discovered derivation. The Mantle adapter proves
the build-provider contract for Mantle, while a Nix job can only be mapped to a
task and dispatched to another provider. That leaves the Nix path incomplete:
the exact `.drv` is not executed, the native Nix JSON result is not admitted,
and product identities, bounded logs, and reconciliation are not owned by a
Kiln adapter.

## Proposed Change

Add `kiln-adapter-nix`, a direct `BuildPort` provider for one exact Nix
derivation:

1. admit one pinned Nix cohort and one exact `.drv` identity;
2. bind the Kiln build plan's provider, plan, policy, platform, and products to
   the profile before any process starts;
3. execute `nix build --no-link --json --offline --expr 'import <drv>'` through
   Bounded Exec with a cleared environment and fixed `HOME`/`NIX_CONFIG`;
4. admit the native JSON result, required output paths, product identities, and
   a bounded stderr log;
5. map success, failure, timeout, cancellation, provider error, and unknown
   outcomes into the canonical build observation; and
6. make duplicate dispatch, conflict rejection, observe, reconcile, and
   observation-only restore explicit.

The first slice supports one exact local derivation with local store inputs.
It does not evaluate flakes, use registries, substitute outputs, or use remote
builders.

## Success Criteria

- The pinned Nix cohort and exact derivation are admitted before execution.
- Executable, derivation, plan, policy, platform, product, or limit drift fails
  before a process starts.
- A real local `.drv` build returns a `Success` observation with every required
  output admitted exactly once.
- A nonzero exit is `Failed`; timeout and cancellation remain terminal;
  malformed or missing output is `ProviderError`; ambiguous process start is
  `Unknown` without a terminal receipt.
- Every non-unknown observation has a bounded receipt, and logs are bounded,
  identity-bound, and unavailable after an observation-only restore.
- Unit fixtures and a real local Nix fixture pass; the fixture-only tests run
  in the Nix check.

## Impact

- **Core**: no change. The adapter uses the existing `BuildEffectPlan`,
  `BuildEvaluationObservation`, completion admission, and `BuildPort`.
- **Adapter**: new crate, profile, runtime, wire, mapping, and tests.
- **Nix**: one new `nix-build-adapter-fixtures` check and one typed Nickel
  profile.
- **Claims**: a successful observation remains scoped to the exact executable,
  derivation, local inputs, store, and native result. It does not prove build
  correctness, sandbox completeness, derivation provenance, output semantics,
  cache truth, reproducibility, or release readiness.

## Out of Scope

- Flake evaluation, registry access, substitution, remote builders, or network
  acquisition.
- A Nix store or GC-root observer; `kiln-adapter-nix-eval` retains discovery and
  GC-root handoff.
- Changes to the canonical CI state machine or build-product meaning.
- Publishing a Nix package or claiming a deployment.
