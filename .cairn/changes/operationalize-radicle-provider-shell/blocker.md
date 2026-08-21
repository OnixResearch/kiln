# Blocker: remaining Radicle capability contracts

Recorded: 2026-08-20
Updated: 2026-08-21

## Completed prerequisites

Lattice publishes request schema version 2, bounded Unix transport, dispatch recovery, a durable application handler, and route-bound source-context injection at revision `70496e67c7fd4a8b05914161a8e09de2759bebc8`.

Kiln pins that revision. The Radicle executable now has a source-bound trigger-to-Lattice path with deterministic identities, durable SQLite replay admission, recovery-only restart behavior, and no direct Nix execution.

## Completed capability splits

`publish-radicle-ci-status` now provides bounded `xyz.radicle.ci-job` publication through a SecretSpec-resolved signer.

`mutate-radicle-cobs-single-writer` now provides declared, exact-head patch review mutation under the same signer boundary.

Both capabilities use separate profiles. Neither capability widens trigger authority.

## Remaining blocker

The active change now requires only the optional Radicle source capability.

No reviewed immutable contract defines the exact signed-reference observation, revision identity, and acquisition evidence for Radicle source snapshots.

## Current safe state

The production trigger profile enables trigger intake only. It rejects source capability selection before effects.

Separate status and patch-review profiles remain opt-in and require exact target allowlists.

`LocalFixtureService` is absent from the executable. Fixture services remain test-only.

Unknown dispatch outcomes use `RecoverDispatch` and never cause automatic redispatch.

## Unblock criteria

Publish an exact source-observation and acquisition contract for the selected Radicle cohort.

Then implement and test the source capability as a separate profile without widening trigger, status, or patch-review authority.

## Non-claims

The trigger shell does not prove source trust, revision availability, event completeness, workflow correctness, provider availability, status publication, mutation success, peer convergence, or release readiness.
