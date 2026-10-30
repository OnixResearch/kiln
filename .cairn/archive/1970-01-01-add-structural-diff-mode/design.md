## Context

- `kiln-highlight` is a pure, bounded crate with pinned Tree-sitter grammars and `language_for_path`; unsupported, oversized or malformed input yields an explicit skipped reason.
- The review diff endpoint returns the provider `CommitComparison` with exact blob line coordinates; comment anchors are `(revision, base_commit, side, file, start_line, end_line)` and are validated against exact blob line counts.

## Goals and Success Contract

For a supported file within bounds, structural mode shows only lines whose tokens changed; a pure reformat shows no change and a moved, unchanged function is reported as moved. Any other case shows the line diff with an explicit reason. A comment created on a structural row has the same anchor as one created on the same line in line mode.

False completion includes: a silently empty structural view after a bound; structural rows whose numbers are display offsets; enabling comments on regions without exact coordinates; grammars running in the browser.

## Decisions

### Decision: Token matching over pinned Tree-sitter trees

**Choice:** `structural_diff(language, old, new, bounds, interrupted)` in `kiln-highlight`. Tokens are leaf nodes, with strings and comments atomic, identified by `(kind, text)`; whitespace is never a token. Top-level items are paired by LCS over their token-sequence hashes; unpaired items with an identical sequence on the other side are moved. Remaining tokens are diffed by LCS within a step budget. Lines with novel tokens become rows with UTF-16 novel spans; matched identical lines supply up to three context rows.

### Decision: Named bounds and explicit fallback

**Choice:** `MAX_STRUCTURAL_BYTES` per side, `MAX_STRUCTURAL_NODES` tokens per side, `MAX_STRUCTURAL_STEPS` work units, plus a shell deadline of `STRUCTURAL_DEADLINE_MS` polled through `interrupted`. Fallback reasons: `unsupported_language`, `binary`, `too_large`, `too_many_nodes`, `work_bound`, `interrupted`, `parse_error`. The shell also reports `not_exact_comparison` for interdiff and `whitespace_filtered` when a whitespace filter is selected, and `truncated` for provider-truncated files. A fallback file renders the line diff with a labelled notice.

### Decision: Anchors stay exact line coordinates

**Choice:** structural rows use exact 1-based blob line numbers; the console builds anchors from the same `(side, line)` coordinates and the same comparison commits as line mode, so anchors are equal. Moved blocks render as a note with both line ranges and no comment controls ("commenting disabled for this region").

### Decision: Opt-in console mode

**Choice:** `review_mode=structural` route key, a "Comparison mode" select (Line / Structural) next to the whitespace selector; the endpoint parameter `mode=structural` adds a `structural` object per file (`{status, reason?, hunks, moved}`) while the line comparison stays in the response for fallback and anchors.

## Risks / Trade-offs

- The matcher is simpler than difftastic's graph search: it can over-report novel tokens around complex reorderings, never under-report changed tokens.
- Large files fall back to the line diff by design.
- One extra blob read pair and parse per displayed textual file.

## Verification Strategy

- **Pure tests:** reformat-only and moved-function fixtures, moved-with-edit, single-line edit anchor equality and UTF-16 spans, added/deleted files, every fallback reason.
- **Web-service tests:** structural response with per-file fallback, anchor equality between structural rows and line-mode rows, unknown mode rejected.
- **Browser:** disposable instance showing the mode select, structural rows and a labelled fallback.
