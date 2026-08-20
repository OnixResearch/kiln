# Proposal: Add the concrete Lattice workflow transport client

## Problem

`kiln-adapter-lattice` accepts an abstract `WorkflowExchangePort`, but Kiln has no production transport implementation. The Radicle shell cannot compose a real workflow client.

## Outcome

Add a bounded Linux Unix-domain client pinned to the Lattice transport and dispatch-recovery cohort at revision `e3148ec59fa12081c270d9800a060d94214b9405`.

## Scope

- Implement `WorkflowExchangePort` with `lattice-workflow-transport`.
- Map every exchange operation to producer-owned commands.
- Preserve request, acceptance, observation, log, and rejection linkage.
- Classify incomplete writes separately from unknown post-write outcomes.
- Recover unknown dispatch acceptance without redispatch.
- Add positive and negative local socket conformance tests.
- Update Nickel profiles, fixtures, documentation, and Nix checks.

## Exclusions

- No Lattice server deployment.
- No automatic workflow retry.
- No Radicle trigger composition in this change.
- No claim of runtime availability or external effect success.
