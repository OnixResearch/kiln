## Phase 1: Producer contract

- [x] [serial] Record the failed real broker event and exact producer-owned wire contract. r[kiln.radicle.broker_031_wire]
- [x] [serial] Require request kind and version while keeping descriptive metadata non-authoritative. r[kiln.radicle.broker_031_wire]
- [x] [serial] Add the published broker push fixture and negative envelope fixtures. r[kiln.radicle.broker_031_wire.rejected]

## Phase 2: Projection and lifecycle

- [x] [serial] Map the producer commit selection to the existing bounded Kiln trigger. r[kiln.radicle.broker_031_wire.accepted]
- [x] [serial] Preserve Defelo response, terminal failure, replay, and no-fallback behavior. r[kiln.radicle.broker_031_wire]
- [x] [serial] Prove missing authoritative facts fail before Aspen or Lattice effects. r[kiln.radicle.broker_031_wire.rejected]

## Phase 3: Evidence and publication

- [x] [serial] Run baseline and changed tests, strict Clippy, Octet, Cairn, and Nix checks. r[kiln.radicle.broker_031_wire]
- [x] [serial] Publish an immutable Kiln revision and prove a real broker event after explicit cutover. r[kiln.radicle.broker_031_wire.accepted]
- [x] [serial] Sync and archive after the live event and rollback evidence pass. r[kiln.radicle.broker_031_wire]
