# Verification evidence

## Result

Threads now record the exact resolution revision, and agent-opened threads expose
`open`, `addressed`, `dismissed`, or `unknown` outcomes. Addressed requires a
later same-base revision whose hunk overlaps the anchored range on the anchored
side before resolution. Current-change threads are annotated and summarized per
agent in the review panel; a bounded read-only window endpoint provides
per-agent counts.

## Evidence

- Focused outcome tests pass for addressed, dismissed, open, and unknown.
- All 187 core library tests pass.
- All 168 web-service library tests pass (1 ignored).
- All 27 code-review integration tests pass, including the bounded endpoint.
- Review JavaScript syntax and formatting checks pass.
- `cairn validate --root .` reports zero issues.

## Non-claims

Outcome observation does not prove agent usefulness, review quality, or change
correctness. Rebases and unavailable comparisons stay `unknown` rather than
guessing. Outcomes grant no review acceptance, merge, gate, or release authority.
