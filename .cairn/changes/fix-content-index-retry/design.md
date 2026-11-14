## Context

The current map is inserted before `persist_index`; an equal retry verifies stored bytes but does not retry persistence. Index publication synchronizes a temporary file and renames it without synchronizing the parent. The effect port interprets a successful put as completed output publication.

## Decisions

### Decision: Explicitly retain pending publication

Keep one pending-index flag in the single-operation content port. Set it when verified content extends the map; clear it only after complete index publication. Every put, including an equal existing digest, must discharge pending publication before returning a receipt. Existing content remains byte/hash verified. A loaded nonempty index initially requires reconciliation before another acknowledgement, including a visible index left by a failed parent sync. This avoids cloning the entire map or recreating already verified chunks on retries. Existing reads report verified bytes, not a durability acknowledgement.

### Decision: Use the reviewed one-file publisher

Publish the exact JSON index with same-parent replacement, private exclusive no-follow staging, file synchronization and required parent synchronization. Keep the existing `.tmp` stage name and a finite one-attempt collision bound so an obstructed or interrupted stage fails explicitly rather than truncating an unrelated file. Never remove an unowned colliding stage. The caller-owned root remains the filesystem authority boundary. A post-rename parent-sync failure is visible-but-uncertain, not success or a rollback.

### Decision: Exercise real effects with bounded injected failures

A test-only PublicationIo wrapper delegates to the real Linux directory and injects the selected create/write/file-sync/rename/parent-sync failure. Tests call the real MoltenContentPort, hold each fault over repeated puts, retain previously published reads, clear the fault, retry and reopen for exact readback. A real obstructing stage-directory test retains the issue trace. These are ordinary filesystem/process observations, not simulated power loss.

## Risks / Trade-offs

- Pending additions can delay other puts on the same port until publication recovers; this is conservative and bounded by the existing single-operation contract.
- A crashed staging operation can require explicit removal of an operator-owned orphan stage; automatic cleanup must not delete unowned collisions.
- File/parent synchronization observations do not prove underlying chunk-store or device power-loss survival. No distributed, multi-writer or ancestor-root durability claim is added.
- Shared disk is constrained: use owned `/run` scratch and per-command `min-free=0` for Nix; no shared GC, host-policy change, service restart, deployment or publication.
