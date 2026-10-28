## Phase 1: Admission

- [x] [serial] Reproduce generic COB rendering and validate proposal, design and task gates. r[kiln.dashboard.cobs.presentation]

## Phase 2: Repair

- [x] [serial] Repair bounded recorded root metadata and readable presentation across all nine supported types without lifecycle reduction. r[kiln.dashboard.cobs.presentation]

## Phase 3: Verification

- [x] [serial] Verify real and producer-shaped content, raw fallback, hostile text, browser transitions and focused checks; update operator guidance. r[kiln.dashboard.cobs.presentation]

## Verification observations

- Live inventory contained 14 Cairn changes, 35 issues, 56 patches and 54 jobs; the other five registered Cairn types were absent. Before repair, `Custom styling support` appeared under the generic `Create issue` heading and all content was JSON-only.
- Candidate browser assets used real read-only APIs for one change, issue, patch and job. All 1/4/3/55 respective action values matched the exact raw response content. Presentations covered artifacts, comments/edits/lifecycle, revisions/merges, and Request/Run/Finished events, without reducing a current state.
- Producer-shaped UI fixtures covered all six Cairn init payload families. Literal hostile HTML and multiline content remained text, with no injected nodes, executable links or script effects. Five of those types were fixture-only verification, not live producer observations.
- Six unknown/malformed action shapes opened exact raw content. Both concurrent artifact variants stayed visible. Exact identity/operation/action disclosures remained available. A 390px mobile viewport had no horizontal overflow; final typography used the application sans-serif font and preserved text whitespace.
- Refresh and document reload restored the selected real issue; session clearing removed its content. No JavaScript page errors were observed. Live-stream request cancellation during navigation was observed separately from page errors. The managed browser tab was closed.
- The second-action title regression failed before repair (search returned zero objects), then passed. Positive/negative cases cover nested document titles and identifiers, absent/non-string/blank titles, oversized root metadata, and unchanged exact detail text. Existing summary-search behavior remains covered by the paging/search contract test.
- A disposable native Git readback invoked the real Browser API with a two-action issue root. Both `/api/cobs?q=readable` and `/api/cobs?q=create` returned title `Readable smoke issue`; exact detail retained both original actions and `reduced_state: null`. Its source and temporary store were removed afterward.
- JavaScript syntax, the web-service test suite, the final 10 COB integration tests and Clippy with warnings denied passed. Initial Clippy findings in new regression assertions were corrected using the repository's checked `member` helper. Kache disabled caching after a host cache I/O warning; checks completed uncached.
- Operator guidance records native encodings, recorded-root title/search scope, exact raw fallback and the 8,192-byte metadata preview bound. Kiln dashboard maintainers own the implementation. No deployment, remote mutation, publication, authority change or producer/lifecycle qualification was performed.
