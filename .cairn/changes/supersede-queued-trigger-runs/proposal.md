## Why

A new push for the same project and branch can arrive while an older run is
already queued behind the project's active execution. Without supersession the
stale queued run eventually consumes a worker slot and reports an obsolete
result.

Issue: [#27](https://github.com/OnixResearch/kiln/issues/27).

## What Changes

- Add an optional `supersede_queued` trigger policy, default off.
- When a new trigger dispatch creates a run successfully, cancel older queued
  runs for the same project and requested reference whose exact source revision
  differs.
- Record the supersession by completing each older run as `Cancelled` through
  the existing durable queued-control path.
- Keep running runs untouched; this is a queued-only behavior.
- Add positive and negative automation tests and document the opt-in.
