## Why

Issue #57 identifies a real persistence-boundary defect: `put_exact` inserts a verified digest-to-manifest mapping before index publication, then an equal retry can bypass the failed publication and acknowledge content that disappears from the reference index after reopen. The source-derived failure trace is the reproduction; no successful failure drill is claimed before verification.

## What Changes

- Track verified-but-unpublished index state explicitly and acknowledge puts only after the index publication barrier succeeds. Reconcile a reopened visible index before a new acknowledgement.
- Reuse the runtime-pinned capability-relative Durable File Publication mechanism for exclusive no-follow staging, exact replacement, payload synchronization and parent synchronization. Preserve publication uncertainty after rename rather than rolling back visible state.
- Exercise the real content port under persistent create/write/file-sync/rename/parent-sync faults, recovery, reopen and a prepopulated index. Preserve exact hash/read-bound checks.

## Impact

- **Consumer/outcome:** Aspen effect materialization cannot convert an index persistence error into a successful retry acknowledgement.
- **Durable capability/owner:** repeatable exact-content publication/recovery, maintained by Kiln Aspen-host maintainers.
- **Dependencies:** reuse the unchanged reviewed Durable File Publication `8e05e74e24b45f752d77145c4455385daaf6d6ab` and Molten `22f8ded26ca1907c29948e08b53f35df23080733` cohorts; no dependency or port API migration.
- **Verification:** focused real-port filesystem/fault regressions, runtime formatting/lints and applicable native lifecycle/traceability gates.
- **Non-claims:** no power-cut/storage-medium guarantee, multi-writer transaction, global durability, VM transfer, authority grant, deployment, publication or release approval.
