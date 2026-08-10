# Add a Mantle build adapter

## Why

Kiln declares `mantle-build-v1`, but the workspace has no Mantle adapter crate. The declared profile cannot dispatch or reconcile a real build.

Mantle owns derivation evaluation, bounded builds, cache facts, and build evidence. Kiln must keep CI state and outcome admission.

## What Changes

- Add optional `kiln-adapter-mantle` support for the `BuildPort`.
- Pin one exact Mantle request, observation, and receipt contract.
- Bind every dispatch to one Kiln effect, attempt, candidate, pipeline revision, plan, and idempotency key.
- Preserve Mantle builder, worker, store, product, cache, log, and receipt identities.
- Map completion through a pure outcome-admission core.
- Support cancellation and reconciliation only when the selected profile declares them.
- Keep cache observations advisory until required products pass Kiln admission.
- Add typed Nickel profiles, bounds, conformance fixtures, and runtime independence checks.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and a pinned published Mantle adapter contract.

## Impact

- **Adapters**: adds `kiln-adapter-mantle`.
- **Core**: adds provider-neutral build facts only when the current model lacks them.
- **Configuration**: replaces the profile declaration with an executable compatibility contract.
- **Testing**: adds producer, consumer, cancellation, timeout, drift, and reconciliation fixtures.
