# Attach optional revision-bound session trace identities

## Why

Kiln #91 needs optional trace pointers beside declared authorship and attested
submission. They are untrusted historical context, not verification, ownership,
intent approval or merge authority. The current consumer is Kiln code review;
Kiln maintainers own its journal and display, while capture remains with the
coding harness and its existing adapters.

## What Changes

- Add default-off revision-bound trace observations to the existing review journal.
- Read only the exact commit's locally admitted `refs/notes/traces` note through
  the existing provider boundary; never fetch or render session bodies.
- Parse the documented `traces:<externalId> [sharedUrl]` line format under named
  bounds, retain BLAKE3 pointer/note identities and optional URL host only.
- Distinguish absent notes from unsupported, incompatible or unavailable observations.
- Display identities separately from authorship/attestation without changing gates.

## Impact

Core DTO/admission, the optional review provider port, Git note acquisition,
web profile/Nickel configuration, review UI, contract tests and operator docs.
Old journals and disabled UI remain byte-identical/unchanged. Deterministic
Git/SQLite fixtures and actual browser rehearsal supply repeatability evidence.
No traces.com dependency, remote fetch, trace body, training export or host-wide
setting is added.
