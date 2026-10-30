## Why

Reformatting, reordering and renames produce large line diffs that hide small semantic edits (issue #10). The service already runs pinned Tree-sitter grammars server-side for highlighting, but comparison options are only whitespace modes over a line diff.

## What Changes

- **Structural comparison:** a pure `kiln_highlight::structural_diff` parses both sides with the pinned grammars and matches leaf tokens difftastic-style: top-level items with identical token sequences are paired (moved items are reported, not shown as changes), the rest is token-diffed, and only lines holding novel tokens are displayed, with novel spans emphasised. Bounds: bytes per side, token count, a deterministic work budget and a shell wall-clock deadline.
- **Explicit fallback:** a missing grammar, binary content, a parse error or any bound returns a named fallback reason; the shell then shows the plain line diff for that file, labelled with the reason.
- **Exact anchors:** every structural row carries exact blob line numbers on its side, so comments created in structural mode equal line-mode anchors. Moved-unchanged regions are shown as a note without comment controls.
- **Console:** an opt-in "Comparison mode" control next to the whitespace selector (`review_mode=structural`, URL-restored).

## Impact

- **Consumers:** reviewers of reformatting or reordering changes.
- **Owners:** Kiln web-service maintainers (`kiln-highlight`, review shell and console).
- **Affected areas:** `crates/kiln-highlight`, `crates/kiln-web-service` (diff endpoint, `web.js` route key, `code-review.js/.css`), `docs/operator.md`.

## Out of Scope

- Ownership, CI, approvals, suggestions, submit and #1 semantic merge results: this mode is display only.
- Grammars in the browser or from a CDN.
- Structural interdiff: the interdiff old side is an in-memory tree, so structural mode applies to exact commit comparisons only and reports a fallback otherwise.
