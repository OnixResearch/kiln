## Why

Issue [#65](https://github.com/OnixResearch/kiln/issues/65), in its [17 November comment](https://github.com/OnixResearch/kiln/issues/65#issuecomment-6956282283), asks for an optional structural lane on the existing baseline/candidate preview comparison. The comparison binds two exact observations and reports exit status and output equality, but it carries no record of how the source changed between the two revisions. The in-tree structural review (#10) already parses both blob versions with pinned grammars, reports pure reformatting as unchanged and moved items as moved, and falls back with a named reason. What is missing is a record of that result, bound to the comparison's exact identities, that a reviewer can inspect next to the observations without mistaking it for a new engine or for review authority.

## What Changes

- `kiln-highlight` exports its structural rules version and a description of the pinned Tree-sitter runtime, highlight engine and grammars: package, version, Cargo registry checksum and which grammar parses each language. A test fails when a pin disagrees with `Cargo.lock` or with the crate's grammar dependencies.
- `GET .../preview-comparison` accepts an explicit `structural=1`. Without it, the response is unchanged. With it, the response adds a `kiln.preview.structural.v1` record computed by the review's structural function for the exact commit comparison of the two revisions. The record binds the comparison identity, both commits, its settings (rules, engine rules, grammar identity, bounds, deadline and diff limits) and every file's structural rows or named fallback, under a domain-tagged BLAKE3 identity. Incompatible subjects, an unreadable source comparison and an oversized record get explicit statuses and no files.
- The comparison view gets an opt-in checkbox, restored from `review_preview_structural=1`, and a read-only structural section below the two lanes.
- A frozen, Kiln-authored fixture is retained with its comparison settings and expected outcomes. It contains a pure reformat, a moved unchanged function, one edit and named fallbacks. A route-level test requires the preview record to equal the review's structural mode for the same commits.

## Impact

Consumers: authenticated change reviewers. Owners: Kiln review/web and preview maintainers. Files: `kiln-highlight` pins, the review structural module and comparison route, the preview review UI, route parsing, styles, tests, fixtures, operator documentation, the console inventory and the changelog. This change adds no stored state, authority or provider capability.

## Non-goals

This change adds no new diff engine and no semantic-merge checker (#1). It does not run grammars in the browser or fetch them from a CDN, retain structural records on disk, or attach vendor semantic-diff views. It makes no review-time, reviewer-throughput or merge-speed claim. The OmniDiff corpus remains #10's measurement. The graphical cohort and historical-baseline launch rules are unchanged.
