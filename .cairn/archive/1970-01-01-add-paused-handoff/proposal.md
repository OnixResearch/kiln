# Add paused handoff manifest and ordering

## Why

Issue #53 requires a consistent mutable workspace and checkpoint handoff where
old memory cannot be restored against a newer disk and the destination cannot
run before ownership/fencing conditions hold. Kiln needs a pure contract before
any transfer transport or VMM migration is wired.

## What Changes

- Add `CheckpointManifest` binding session/work/attempt/fence/execution,
  placement, operation, container profile, disk generation and distinct objects.
- Admit manifests against independently resolved volume facts, rejecting
  old-memory/new-disk and mismatched identities.
- Add ordered `PausedHandoff` phases from intent through quiescence, pause,
  object verification, target admission, resume and source retirement.
- Require the current owner token and target owner; uncertain outcomes enter
  reconciliation without claiming retirement or releasing capacity.
- Document non-claims and add ordered, stale, disk-mismatch and reconciliation
  tests.

## Impact

- **Files**: `crates/kiln-microvm/src/handoff.rs`, `retained.rs`, docs and
  specification.
- **Testing**: ordering, stale target owner, disk generation/object mismatch,
  reconciliation and capacity retention.
- **Authority**: pure admission/ordering only. No copy, transfer, VMM operation,
  ownership commit, resume effect or capacity release.
