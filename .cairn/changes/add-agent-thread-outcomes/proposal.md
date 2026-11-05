## Why

Issue #35 still lacks a way to tell whether agent review objections led to
changes or were dismissed. Threads carry no resolution revision and the review
page has no non-binding outcome observation.

## What Changes

- Record the exact revision on which a thread is resolved.
- Derive each agent-opened thread's outcome:
  - `open` while unresolved;
  - `addressed` when a later revision before resolution touches the anchored
    hunk;
  - `dismissed` when resolved without such a change;
  - `unknown` when exact history is unavailable.
- Compare the thread's exact base commit to each later revision with the same
  base and detect overlapping hunks on the anchored side.
- Annotate current-change threads and show per-agent addressed/dismissed/open
  counts in the review panel.
- Add a bounded read-only endpoint
  `/api/code-review/agent-thread-outcomes?start_ms&end_ms` for per-window
  observation.

## Impact

- **Files**: `crates/kiln-core/src/change_review.rs`,
  `crates/kiln-web-service/src/code_review.rs`,
  `crates/kiln-web-service/src/code_review/outcome_tests.rs`,
  `crates/kiln-web-service/src/code-review.js`, `docs/operator.md`.
- **Testing**: anchored overlap addressed/dismissed, open, unknown, endpoint
  syntax/read-only bounds, full review tests.
- **Authority**: observation only. Outcomes never grant review acceptance,
  merge, gate, release, or source authority.
