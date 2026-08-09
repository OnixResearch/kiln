# Add a nix-eval-jobs provider adapter

## Why

Kiln has provider-neutral CI state and narrow provider ports. It does not have an adapter for conventional Nix jobsets that expose many derivation jobs.

`NixOS/nix-eval-jobs` can evaluate a Nix attribute set in parallel and emit one JSON record per job. It also isolates job errors, records derivation facts, reports advisory cache status, and can retain derivations with GC roots.

Kiln needs an optional compatibility adapter without moving Nix evaluation, build authority, or Nix-private types into its core.

## What Changes

- Add an optional workflow-discovery adapter for a pinned `nix-eval-jobs` executable.
- Define a typed Nickel profile for executable, Nix cohort, source, selectors, limits, GC-root handoff, and enabled output fields.
- Parse the upstream JSON-line stream under named record, text, diagnostic, job, and byte bounds.
- Map successful derivation records into provider-neutral Kiln task proposals through a pure core.
- Preserve per-job evaluation errors and fatal process outcomes without false task or evaluation success.
- Keep upstream cache status advisory and separate from build admission or task completion.
- Retain run-scoped Nix GC roots until an admitted downstream provider handoff or explicit failed cleanup result.
- Record exact executable, Nix cohort, source, arguments, output, and adapter identities.
- Add positive, negative, drift, crash, partial-stream, cache, GC-root, and conformance fixtures.

## Dependencies

Implementation begins after `establish-neutral-ci-control-plane` supplies canonical evaluation, task, attempt, event, effect, provider, and projection foundations.

The adapter uses `NixOS/nix-eval-jobs` revision `a0cd02231c58974a6b5aaa3712069b071047162e` as its initial compatibility baseline. The project is GPL-3.0.

The executable remains an optional shell dependency. No Kiln core crate imports its code, Nix private libraries, or Hydra types.

## Non-Goals

- Making Nix the Kiln domain model, journal, scheduler, or build authority.
- Supporting arbitrary Nix or Onix Nix evaluator cohorts.
- Claiming compatibility with Onix-only builtins such as `builtins.wasm`.
- Importing Hydra aggregate or constituent semantics.
- Adding arbitrary `--apply`, unrestricted recursive traversal, or floating source evaluation.
- Treating upstream cache observations as trusted cache hits or completed tasks.
- Using this adapter in Mantle's no-Nix runtime lane.

## Impact

- **Affected specs:** new `nix-eval-jobs-adapter` capability and existing provider-port composition.
- **Planned files:** adapter profile contracts, pure mapping core, process shell, fixture executable, conformance tests, evidence, and docs.
- **Compatibility:** the adapter is optional and unavailable unless its exact executable and Nix cohort pass admission.
- **Testing:** core mapping tests, parser tests, process fixtures, provider conformance, Nickel checks, dependency gates, and Cairn gates.
