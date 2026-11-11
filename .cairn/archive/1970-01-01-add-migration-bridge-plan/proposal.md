# Add a bound migration bridge plan

## Why

Issue #54 requires the VM-specific Iroh migration bridge to bind durable
operation, endpoint, placement, attempt and protocol identities before opening
protected Cloud Hypervisor sockets. Kiln needs a pure admission contract before
the transport shell is wired.

## What Changes

- Add a versioned `MigrationBridgePlan` and exact expectation admission.
- Bind endpoint public keys, protected source/target sockets, checkpoint
  manifest, placements, protocol generation, precopy mode and resource bounds.
- Refuse crossed/stale bindings, zero/equal endpoints, escaping sockets,
  same-host/non-advancing placements, postcopy and multiple connections.
- Document non-claims and add exact/refused tests.

## Impact

- **Files**: `crates/kiln-microvm/src/bridge.rs`, library export, docs and
  specification.
- **Testing**: exact admission, wrong peer/socket/protocol, stale placement,
  escaping path, zero endpoint and connection bound.
- **Authority**: pure admission only. No I/O, transfer, verification, placement
  commit, resume or cleanup.
