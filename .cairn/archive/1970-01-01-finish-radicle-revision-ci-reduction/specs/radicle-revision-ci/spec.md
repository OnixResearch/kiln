# Native Radicle Revision CI

## ADDED Requirements

### Requirement: Native multi-writer projection

r[kiln.radicle.multiwriter] Kiln MUST bound native history before reduction, use pinned upstream signature and authorization rules, and bind own review append to the current observed head set.

#### Scenario: Concurrent histories

- GIVEN concurrent authorized reviews
- WHEN reduced through the helper
- THEN native state retains both reviews and a stale head set refuses mutation.

### Requirement: Exact revision CI

r[kiln.radicle.revision_ci] Patch CI MUST retain patch, revision, base and oid, require author signed-ref reachability and revision-bound approval for untrusted authors, deduplicate revision dispatch, and preserve push serialization.

#### Scenario: Revision replacement

- GIVEN an approved untrusted revision
- WHEN a new revision arrives
- THEN the new revision remains blocked until separately approved.
