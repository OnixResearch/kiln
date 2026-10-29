## Why

The live COB inspector renders all nine supported types as operation metadata and JSON. Actual issue titles live in action 1 rather than the generic `Create issue` commit summary; Cairn artifacts and patch descriptions appear as escaped JSON strings; externally tagged Radicle job events have no readable presentation.

## What Changes

- Display bounded recorded root titles in inventory/search while preserving summary fallback and exact history.
- Present each supported type's recorded content: Cairn artifacts, documents, obligation fields, release candidates, exception requests, issue discussion, patch revisions and job events.
- Keep technical identities and raw action JSON available in disclosures. Unknown/malformed actions remain inspectable rather than being dropped or interpreted as lifecycle decisions.
- Preserve inert text, exact selection, concurrency, read bounds and read-only authority.

## Impact

Kiln dashboard maintainers own this repair for operators browsing local collaboration records. Changes are confined to the web-service reader, UI, focused behavior checks and operator guidance. Cairn owns the producer schemas consulted for presentation; Kiln does not import its reducers. No remote mutation, deployment, publication or provider requalification.
