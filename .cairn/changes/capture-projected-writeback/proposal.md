## Why

Issue [#79](https://github.com/OnixResearch/kiln/issues/79) needs an explicit reverse mapping before writable projections can be accepted. A projected workspace omits and rearranges source files, so its directory diff is not a repository diff: treating it as one can delete hidden content, edit the wrong origin through an alias or apply a stale mapping. #78 materializes a writable view into a pinned attempt overlay and #77 plans its build, but nothing turns what a worker wrote in that overlay into exact source operations, seals them before verification or rebuilds the full candidate without touching what the view does not hold.

## What Changes

- A pure capture in `kiln-core` (`projection::writeback`) accepts a completeness witness for a write, its admitted definition, the overlay's workspace pin, the shell's observation of the overlay and the attempt's current fence and source facts. It maps every changed, created or removed overlay file back through the manifest and mounts to one typed source operation (`edit`, `create` or `delete`) naming the expected base object. It refuses, before any effect: a moved source, a superseded fence, a pin for another view, aliases that disagree, new files outside the declared creation destinations, new files that map to no source path or to several, new files outside the selection or over a source file the view does not hold, and unsupported entries.
- A sealed candidate (`kiln.projection.writeback-candidate.v1`) carries a BLAKE3 identity over the base, the definition, manifest and overlay pin and the ordered operations. Its pure reconstruction applies the operations to the base entries and keeps every other entry.
- A worker shell observes an overlay by hashing each regular file as a Git blob and seals a candidate. Sealing re-reads the captured bytes and refuses an overlay that changed since capture, writes only those blobs, and builds the candidate tree from the base tree. It checks that the tree differs from the base at exactly the operation paths, then commits it without moving any reference.

## Capabilities

### Modified Capabilities

- `source-projection`: edits in a writable projected workspace become a sealed full-source candidate of typed operations.

## Impact

New `kiln_core::projection::writeback` module, a `mounts` accessor on `AdmittedDefinition`, a `kiln-web-service` worker writeback shell with round-trip tests over a temporary Git repository, and documentation. No worker job, dispatch, verification or publication route changes; nothing is pushed or published.

## Non-goals

Rename detection (a rename is captured as a delete and a create), alias, membership and fork operations that change a projection definition or a source graph, explicit rebase of a candidate over a moved source, generated-entry inverses, multi-repository writeback, current-authority checks and restricted results (#69/#70), handing candidates to independent verification (#46), build qualification of the candidate (#77) and publication (#47/#63).
