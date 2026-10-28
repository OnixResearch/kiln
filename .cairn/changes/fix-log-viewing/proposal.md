## Why

The live log viewer discards previously loaded text when a continued read returns the next chunk or an empty end cursor. Search changes leave a blank viewer, and retained search text makes diagnostic reads fail with `diagnostic_search_unavailable`.

## What Changes

- Append contiguous reads of one exact run/node/stream/search; replace only for an explicit seek or changed selection after an admitted response.
- Preserve displayed bytes on failed reads, provide an explicit read-from-start action, and distinguish empty streams from unavailable reads.
- Load changed searches immediately and disable retained-log search for native diagnostics.
- Keep bounded display retention and byte/native cursor semantics; follow must not pull an operator away from older text they are inspecting.

## Impact

- **Files**: `crates/kiln-web-service/src/web.js`, `web.html`, `web.css`, and `docs/operator.md`.
- **Consumer / owner**: Kiln operators inspecting retained execution output; maintained by Kiln application/dashboard maintainers.
- **Checks**: Real read-only browser reproduction and candidate verification with actual retained chunks, frontend syntax, focused service checks, and native Cairn gates. No execution, credential, API or durable-state changes; no implicit publication or deployment.
