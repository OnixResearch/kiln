## Why

Issue #28 still lacked any reviewed cache-placement declaration. Operators could
not state which runners are expected to share an LLC or cache allocation.

## What Changes

- Add optional `cache_partition` to Firecracker runners.
- Reject a partition containing both untrusted-capable and workload-bearing
  runners, matching the existing host/domain separation.
- Record the label in `vm.cache_partition`.
- Refuse a declared label when the host has no observed resctrl mount.
- Document that this is a placement declaration, not resctrl programming or
  proof of LLC isolation.

## Impact

- **Files**: `crates/kiln-hosted/src/execution.rs`,
  `crates/kiln-hosted/src/microvm.rs`, hosted Nickel profiles,
  `docs/hosted-ci.md`.
- **Testing**: shared-partition rejection and hosted tests.
- **Authority**: static label only. Actual cache allocation, LLC measurement and
  covert-channel evidence remain open.
