## Context

OmniDiff's corpus stores `before.<ext>.test`, `after.<ext>.test` and a
`human_mapping.json` with node entries and `text_mappings`. Its own benchmark
projects tools that cannot emit node mappings to per-line touched labels.

## Decisions

### Projection

The harness marks a line touched when any `Full` text-mapping range covers it.
It marks a Kiln line touched when a structural row carries a non-empty `novel`
span. It compares the old and new sides separately and records the symmetric
difference count.

### Baseline

The CSV is one row per parsed fixture. A run with the baseline fails when a
fixture starts falling back or either mismatch count grows. New fixtures are
reported but not gated until the baseline is refreshed.

## Risks

- The projection throws away moves and cannot distinguish a moved unchanged
  block from an unchanged one.
- Parse-error fallbacks dominate the remaining 116 fixtures.
- The corpus is external and pinned by revision, not vendored.
