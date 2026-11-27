## Context

Cairn authoring and dependencies spans an authenticated operator console and a native authority.

## Decisions

Use explicitly enrolled repository roots, fixed native Cairn verbs and admitted artifacts. Show the exact current revision, reject stale writes and cycles, and preserve the provider result without implicitly committing or starting work.

Artifact editing depends on Cairn's native conditional artifact replacement operation. Enroll it only on profiles explicitly configured for that contract. Read one of the fixed proposal, design, or tasks artifacts with its exact byte digest. Require both the displayed repository revision and artifact digest on a write. Pass bounded UTF-8 content via stdin, then read back the exact bytes and verify the native receipt. Never write lifecycle Markdown directly from Kiln.

## Risks / Trade-offs

Absent capabilities fail closed. Reusing provider admission avoids a second source of truth.
