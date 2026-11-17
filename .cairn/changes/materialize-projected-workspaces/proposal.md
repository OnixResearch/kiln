## Why

Issue [#78](https://github.com/OnixResearch/kiln/issues/78) needs a workspace provider that materializes an admitted projection reproducibly while preserving attempt isolation and exact source binding. The worker today writes the complete pinned Git tree and chooses a project directory; the projection contract (#75) can already resolve a frozen manifest and a completeness witness, but nothing turns that witness into a tree, separates reusable immutable inputs from an attempt's writable state, or refuses partial or mismatched preparation.

## What Changes

- A pure, versioned materialization plan in `kiln-core` turns a `CompleteProjection` and a materializer profile into planned entries and a reusable input identity, and derives an attempt overlay identity from that input, the attempt and its fence. It refuses symbolic links and generated entries in this first eager-local profile, and a platform that cannot represent the manifest's case rules, executable bits or path lengths.
- An eager local shell in the worker verifies every provider object against its native identity, writes only the planned entries and modes into a private preparing directory, publishes the immutable input by rename with a separate binding, and copies it into an attempt's private writable overlay. Existing directories with a missing or mismatched binding are refused, never reused or overwritten; reused inputs are re-verified.

## Capabilities

### Modified Capabilities

- `source-projection`: admitted complete projections can be materialized into immutable inputs and attempt overlays.

## Impact

New `kiln_core::projection::materialization` module and `kiln-web-service` worker materializer with tests. Full-tree worker preparation is unchanged and stays the current route.

## Non-goals

Dispatch pinning of the manifest and overlay generation, restricted-runtime confinement, current-authority checks at retrieval (#69/#70), generated-entry providers, symbolic links, lazy or remote content and the two-project build profile (#77) are later tasks of this issue.
