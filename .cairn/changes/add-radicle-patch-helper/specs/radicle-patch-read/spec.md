# Radicle Patch Read Specification

## ADDED Requirements

### Requirement: Isolated upstream patch reduction

r[kiln.radicle.patch_read] Kiln MUST invoke the pinned upstream Radicle patch
reducer through a separate helper process because its SQLite linkage conflicts
with Kiln's. The adapter MUST bound helper output, reject non-zero exits and
stderr noise, and parse only one JSON projection. The helper MUST perform no
mutation.

#### Scenario: Valid patch

- GIVEN a helper that returns one reduced patch JSON object
- WHEN the adapter invokes it
- THEN the object is returned.

#### Scenario: Helper failure

- GIVEN a helper that exits non-zero or writes stderr
- WHEN the adapter invokes it
- THEN the call fails without returning a projection.
