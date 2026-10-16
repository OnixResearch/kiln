## Why

The deployable Kiln Aspen host proves the reviewed process-scoped canary, but its acknowledgements, uncertain-operation set, callback values, and host composition are process-local. The current Lattice workflow also proves only a bounded shell fixture. Production Radicle CI needs durable restart behavior and one bounded Kiln-owned build provider before Onix can route broker traffic through Aspen.

## What Changes

- Replace process-local hosted operation state with a durable, identity-checked ledger and recoverable Aspen journal/value adapters.
- Reconcile an accepted or uncertain hosted operation after process loss without provider redispatch.
- Add one typed, bounded Radicle Nix provider executable for Lattice workflow execution and bounded report publication.
- Add positive and negative restart, corruption, timeout, teardown, report, replay, and load tests.
- Publish a new immutable Kiln host cohort for Onix deployment.

## Impact

- **Files**: `runtime/kiln-aspen-host`, `crates/kiln-adapter-radicle`, typed Nickel profiles, Nix checks, host documentation, and Cairn evidence.
- **Testing**: workspace tests, strict Clippy, Octet deny-all, nested-host all-target/all-feature tests, Nix package checks, restart/corruption fixtures, and strict Cairn validation.
