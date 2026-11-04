## Why

The Firecracker backend's strongest isolation claim is that each job runs behind
a seccomp-confined VMM. The receipt currently asserts that claim from fixed
literals; it does not observe the running VMM. A changed binary, a lost seccomp
filter, or a unit that gained capabilities would still produce the same receipt.

Issue: [#31](https://github.com/OnixResearch/kiln/issues/31).

## What Changes

- Add `NoNewPrivileges=yes` to the Firecracker unit.
- After spawn, and after `/snapshot/load` for a warm restore, read the attempt
  unit's `cgroup.procs`, require exactly one VMM process, and admit its
  `/proc/<pid>/status` and `/proc/<pid>/exe` against the reviewed executable.
- Add the pure `ConfinementObserved { ok }` lifecycle gate between `Spawned` and
  `GuestConnected`; a failed or unavailable observation journals and fails.
- Replace the VM claim literals with an observed `vm.confinement` record while
  keeping `jailer: false` and `hostile_isolation_claim: false`.

## Impact

- **Files**: `crates/kiln-microvm` lifecycle and confinement parser,
  `crates/kiln-hosted` VMM spawn/observation and receipts, `docs/hosted-ci.md`,
  lifecycle and parser tests.
- **Testing**: pure status parser positive/negative cases, lifecycle ordering
  tests, unit-command property test, and the existing KVM-gated microVM suite
  for cold boot and warm restore.
- **Authority**: no guest networking, no jailer, no root helper, no new
  hostile-isolation claim. Confinement fields remain backend observations
  outside `canonical_outcome`.
