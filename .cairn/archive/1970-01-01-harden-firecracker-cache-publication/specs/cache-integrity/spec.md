## ADDED Requirements

### Requirement: Immutable cache publication

r[kiln.hosted.microvm.cache_integrity]

For any guest behaviour and interruption, existing entries MUST remain byte-identical to their recorded BLAKE3; a new key MUST be absent or complete and verified. Corruption MUST become a measured miss. Publication MUST use verified synced staging, no-replace atomic rename and directory fsync; restart MUST remove owned orphan staging. Receipts MUST identify entry hash and local tier without claiming hostile isolation or remote performance.

#### Scenario: Interrupted publication

- GIVEN an existing immutable entry and a different new key
- WHEN publication is killed at receiving, verifying, before rename or after rename
- THEN restart preserves the existing entry and exposes no partial key.

#### Scenario: Hostile return

- GIVEN an existing immutable entry
- WHEN oversized, hash-invalid or conflicting replacement content is submitted
- THEN the existing entry remains unchanged.
