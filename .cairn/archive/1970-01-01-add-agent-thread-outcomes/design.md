## Context

Threads already store an anchored revision, side, file and line range. A new
optional `resolved_revision` records the exact revision used at resolution. The
provider can compare the anchor's exact base commit with each later commit; a
hunk overlapping the anchor range on the same side means the anchored region
was touched.

## Decisions

### Outcome derivation

- Unresolved: `open`.
- Resolved without a recorded resolution revision: `unknown` (legacy journal).
- No later revision: `dismissed`.
- A later revision with the same base and an overlapping hunk: `addressed`.
- Later revisions with different bases, provider failure, or an unavailable
  exact comparison: `unknown`.
- Comparisons are capped per thread; an exhausted budget is `unknown`.

The comparison is a shell observation and does not mutate review state.

### Display and window

Current-change threads carry `agent` and `outcome` fields in the detail view.
The submit panel aggregates per-agent counts and addressed rate over decided
threads. A read-only endpoint provides the same counts over a half-open time
window for observation consumers. Unknown and open threads are shown separately
and never counted as addressed.

## Risks

- Rebases change the base, so outcomes for rebased later revisions remain
  `unknown` rather than guessing.
- The outcome is a display observation, not proof of agent utility or review
  quality.
