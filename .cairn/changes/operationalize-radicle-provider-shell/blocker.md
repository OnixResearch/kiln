# Blocker: remaining Radicle capability contracts

Recorded: 2026-08-20
Updated: 2026-08-20

## Completed prerequisites

Lattice publishes request schema version 2, bounded Unix transport, dispatch recovery, a durable application handler, and route-bound source-context injection at revision `70496e67c7fd4a8b05914161a8e09de2759bebc8`.

Kiln pins that revision. The Radicle executable now has a source-bound trigger-to-Lattice path with deterministic identities, durable SQLite replay admission, recovery-only restart behavior, and no direct Nix execution.

## Remaining blockers

The active change also requires concrete optional Radicle source, status, and collaboration capabilities.

No reviewed immutable contract currently defines the exact Radicle signed-reference observation used for source acquisition.

No reviewed immutable contract currently defines Radicle CI job status mutation, idempotency, acceptance, and reconciliation for the pinned broker cohort.

Collaboration mutation also lacks an operator-selected signer profile, expected-head policy, and mutation authority. Kiln will not infer these values or load an ambient signer.

## Current safe state

The production profile enables trigger intake only. It rejects source, status, and collaboration capability selection before effects.

`LocalFixtureService` is absent from the executable. Fixture services remain test-only.

Unknown dispatch outcomes use `RecoverDispatch` and never cause automatic redispatch.

## Unblock criteria

Publish exact source and status contracts for the selected Radicle cohort.

Select explicit signer and expected-head authority for collaboration mutation.

Then implement and test each capability as a separate profile without widening trigger authority.

## Non-claims

The trigger shell does not prove source trust, revision availability, event completeness, workflow correctness, provider availability, status publication, mutation success, peer convergence, or release readiness.
