# Add the operator surfaces: store input, CLI command, and CLI rail

## Why

The operator dashboard landed with one input, one entry point, and one rail.

- It read a canonical Kiln journal document only. The SQLite event store, the
  other local canonical record surface Kiln owns, was not observable.
- It was reachable only as the separate `kiln-dashboard` binary. The operator
  CLI, which is the documented entry point, could not reach it.
- The property rail covered only the dashboard. The `kiln` operator CLI, which
  prints canonical state and mints evidence identities, had no property
  coverage.
- The journal fixtures were all produced by the dashboard's own example
  generator. Nothing proved that the dashboard reads a journal written by a
  Kiln shell.

## What Changes

- Add a canonical SQLite event store as a second dashboard input, opened read
  only, with the store's own event identity verification and canonical replay.
- Add `kiln dashboard` to the operator CLI, sharing one implementation with the
  standalone binary.
- Extend the property-based terminal rail to the `kiln` CLI's demo and
  evidence surfaces, including the rejection of a malformed evidence input.
- Check in a journal written by the OnixOS pilot's own writer and add it to the
  rail, so the dashboard is exercised against a shell-written journal.
- Add a canonical event store profile to the store adapter, so a reader admits
  the same profile the store was created with instead of inventing one.

## Impact

- **Affected specs:** new `dashboard-operator-surfaces` capability, extending
  the `observation-dashboard` capability.
- **Planned files:** the dashboard crate's shell and store modules, the
  operator CLI, the store adapter's read-only open and canonical profile, the
  property rail specifications and fixtures, the operator documentation, and
  the README.
- **Compatibility:** additive only. The dashboard binary keeps its journal
  input and its report. The store adapter keeps its write path and gains a
  read-only one. No existing command changes behavior.
- **Testing:** the dashboard crate's tests, the conformance boundary audit, the
  property rail with its negative controls, the workspace gates, and the Cairn
  gates.

## Non-Goals

- No Celld worker event store input. That store is a remote authenticated
  HTTP service, and reading it would give the dashboard a network path that
  its read-only local boundary forbids.
- No new view vocabulary, no second projection, and no change to the admitted
  observation profile.
- No claim that the pilot journal fixture is a production journal. It is a
  recorded artifact of the pilot's own writer.
- No claim that the property rail explored every input. It states properties
  over the fixtures it runs.
