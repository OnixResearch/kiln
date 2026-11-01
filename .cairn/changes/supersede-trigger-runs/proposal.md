## Why

A new push for the same project and branch can arrive while an older run is
already queued behind the project's active execution. Without supersession the
stale queued run eventually consumes a worker slot and reports an obsolete
result.

Issue: [#27](https://github.com/OnixResearch/kiln/issues/27).

## What Changes

- Add an optional `supersede_queued` trigger policy, default off.
- When a new trigger dispatch creates a run successfully, cancel older runs for
  the same source scope whose exact source revision differs: project and
  requested reference for pushes, or project and pull-request number for pull
  requests.
- Complete older queued runs as `Cancelled` through the durable queued-control
  path, and cancel older running runs through the exact admitted provider
  control path.
- Leave awaiting-approval and terminal runs untouched.
- Add active and queued coverage and document the opt-in.
