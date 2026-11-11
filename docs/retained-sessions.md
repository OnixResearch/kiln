# Retained session lifecycle

`kiln-microvm::retained` is the pure ownership and lifecycle core for an opt-in
retained Lattice session. It is not a second VMM supervisor: the existing
hosted runner and a future migration transport must consult it before restore,
resume, migration or retirement.

A session binds a logical session identity, work/version, attempt/fence,
placement host and monotonic generation, and bounded CPU/memory/slot
reservations. Every command carries an owner token with the exact session,
fence, host and generation; stale owners are refused.

The lifecycle admits receiving, restoring, paused, running, migrating,
migrated-away, reconciliation-required and retired states. Restore observation
fails closed into reconciliation when confinement is not observed. A paused
source is the only migration start point; migration success moves the source to
`migrated-away` but does not prove target placement, cleanup or capacity release.
Only an observed retirement releases capacity.

## Non-claims

This module performs no I/O, starts no VMM, transfers no memory or workspace,
grants no work/retry/landing authority, and does not establish distributed
ownership. Iroh/Aspen transport, sidecar volume fencing, Cloud Hypervisor
invocation and onix-core service composition remain external.
