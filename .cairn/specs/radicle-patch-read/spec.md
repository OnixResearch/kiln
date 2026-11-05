# Radicle Patch Read Specification

## Purpose

Defines the `radicle-patch-read` capability.

## Requirements

### Requirement: Isolated upstream patch reduction

r[kiln.radicle.patch_read] Kiln MUST invoke the pinned upstream Radicle patch
reducer through a separate helper process because its SQLite linkage conflicts
with Kiln's. The adapter MUST bound helper output, reject non-zero exits and
stderr noise, and parse only one JSON projection. The read helper MUST perform no mutation. A separate review command MAY append
one review through the upstream authorization path; its signer seed MUST come
from the child environment, never argv, and it MUST return only a bounded JSON
review identity. The explicit Radicle patch CI profile MUST configure its pinned
helper and native storage root together and MUST expose only bounded native reads;
the default push worker profile MUST NOT inherit patch execution authority.

#### Scenario: Valid patch

- GIVEN a helper that returns one reduced patch JSON object
- WHEN the adapter invokes it
- THEN the object is returned.

#### Scenario: Helper failure

- GIVEN a helper that exits non-zero or writes stderr
- WHEN the adapter invokes it
- THEN the call fails without returning a projection.
