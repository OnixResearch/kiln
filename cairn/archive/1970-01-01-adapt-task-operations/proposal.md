## Why

The initial Kiln change defines CI state, provider ports, durable operations,
and rebuildable projections. It does not define the complete operator contract
for task status, logs, waiting, local capacity, or scoped controls.

Pueue provides a mature reference for these operator behaviors. Its selected
patterns are useful, but its single-user scheduler and mutable daemon model do
not match the Kiln boundary.

Kiln needs its own task-operations contract. The contract must preserve the
pure core, event journal, provider capability admission, canonical Kiln
identities, and global tenant fairness.

## What Changes

- Add read-only operator projections for status, lifecycle facets, dependency
  blocks, delays, capacity blocks, and outcomes.
- Add bounded `log`, `follow`, and `wait` behavior with explicit cursors,
  predicates, deadlines, truncation, gaps, and terminal conditions.
- Add provider-local capacity classes with named concurrency limits that do not
  replace global tenant fairness or provider placement.
- Add pure task-readiness decisions over dependency facts, admitted time
  observations, holds, and provider-local capacity facts.
- Add scoped admission holds, release, cancellation, and optional provider
  pause or resume controls behind declared capabilities.
- Add durable bounded log storage, restart recovery, and secret-redacted
  diagnostics without making logs canonical CI authority.
- Record Pueue only as a pinned design reference. Add no Pueue dependency,
  protocol type, scheduler code, provider adapter, or runtime requirement.

## Dependencies

Implementation begins after `establish-neutral-ci-control-plane` supplies the
canonical task, attempt, event, effect, provider-capability, journal, and
projection foundations.

This change has no runtime, build, protocol, or adapter dependency on Pueue.

## Impact

- **Core**: adds pure readiness, lifecycle-facet, wait-predicate, capacity, and
  control-admission decisions.
- **Shells**: add bounded status, log, follow, wait, hold, release, cancel,
  pause, and resume orchestration.
- **Configuration**: adds typed Nickel policy for capacity classes, log bounds,
  wait bounds, retention, redaction, and control capabilities.
- **Persistence**: adds durable bounded log segments and restart-safe cursors
  while the event journal remains canonical state authority.
- **Providers**: retain process execution, placement, pause, resume,
  cancellation, and output-stream authority.
- **Compatibility**: adds no Pueue protocol compatibility and no Pueue provider.
- **Testing**: requires positive and negative fixtures for every new decision,
  projection, storage, recovery, redaction, and control behavior.
