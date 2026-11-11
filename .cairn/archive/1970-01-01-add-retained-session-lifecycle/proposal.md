# Add retained session ownership lifecycle

## Why

Issue OnixResearch/kiln#52 requires an opted-in retained Lattice session with
placement ownership, migration-aware lifecycle and stale-owner fencing. The
existing per-job microVM lifecycle ends in destruction and cannot express a
paused, migrated-away or reconciliation-required retained VM.

## What Changes

- Add a pure `kiln-microvm::retained` lifecycle with session/fence/host/generation
  owner tokens.
- Bind bounded resources and add receiving, restoring, paused, running,
  migrating, migrated-away, reconciliation-required and retired states.
- Fail closed when confinement is not observed; refuse stale owners; keep
  migrated-away separate from target placement and capacity release.
- Document non-claims and add restore, confinement, stale-owner, migration and
  serialization tests.

## Impact

- **Files**: `crates/kiln-microvm/src/retained.rs`, `lib.rs`, manifest, docs,
  specification and lifecycle evidence.
- **Testing**: owner/generation fencing, confinement refusal, migration and
  retirement separation, retained-capacity semantics, JSON round trip.
- **Authority**: pure lifecycle only. No VMM launch, migration transport,
  workspace transfer, retry, landing or distributed ownership.
