## Context

Lattice workflow authoring spans an authenticated operator console and a native authority.

## Decisions

Forward through a configured Lattice authoring boundary. Display immutable revisions and source; submit a validated draft against the expected head. Preserve validation receipts and show stale or unsupported commands without publishing or executing automatically.

Editable drafts depend on Lattice's native conditional draft-save transaction. Keep pinned archives read-only. An operator must explicitly select an enrolled owner catalog and workflow. Read its exact source and head/draft identities, submit bounded source with those identities, and relay the native commit or conflict receipt. Do not call readmission or publish, and do not persist a parallel Kiln draft.

## Risks / Trade-offs

Absent capabilities fail closed. Reusing provider admission avoids a second source of truth.
