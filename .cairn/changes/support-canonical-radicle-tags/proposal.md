## Why

Radicle release tags are canonical refs, but Kiln source admission accepts only
`refs/heads/`. A canonical annotated tag cannot be acquired or recorded as
release evidence even when the delegate quorum agrees on it.

Issue: [#21](https://github.com/OnixResearch/kiln/issues/21).

## What Changes

- Accept `refs/tags/` signed and canonical source references.
- Resolve a source reference's target object to its commit for snapshot
  acquisition and `expected_revision` matching.
- Keep the canonical evidence object and kind, including `tag` for annotated
  tags.
- Add contract and native tests for an annotated canonical tag.

## Impact

- **Files**: Radicle source contract, source protocol/canonical verification,
  source shell alias resolution, documentation and native fixtures.
- **Testing**: source contract tests, native canonical-tag fixture, workspace
  tests, Clippy and Cairn validation.
