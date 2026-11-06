## Why

Kiln can publish `xyz.radicle.ci-job` status objects, but the Tangled/Radicle
CI ecosystem reads `xyz.radworks.job` objects. Without that projection, a Kiln
attempt is invisible to `rad`/Tangled tooling, and Kiln cannot join a job that
another CI node already created for the same commit.

The pinned `radicle-cob` cohort already provides the multi-writer mechanism:
`objects()` returns the observed head set across namespaces, the change graph
filters entries with invalid signatures, and `update` creates the new entry on
top of every observed tip. Kiln only needs the `xyz.radworks.job` action schema,
reducer, and bounded publication port.

Issue: [#17](https://github.com/OnixResearch/kiln/issues/17).

## What Changes

- Add a typed Radicle job publisher profile pinned to cohort, type, signer and
  repository allowlist.
- Add `request`, `run` and `finished` actions and a reducer matching the
  `radicle-job` wire schema.
- Add a bounded shell that discovers an existing job by commit, creates one
  only when none exists, appends a run under the signer namespace, and appends
  the terminal reason without duplicating a UUID.
- Add native fixtures for fresh publication, replay, start-then-finish,
  extension of another node's job, divergent multi-head append and rejection
  of unauthorized, mismatched and malformed actions.
- Add a COB browser fixture showing a Kiln-published job.

## Impact

- **Files**: `crates/kiln-adapter-radicle/src/job`, status profile admission,
  adapter exports, native tests, workspace dependencies, web-service COB
  browser fixture, docs.
- **Testing**: focused adapter and web-service tests, Nickel checks, full
  workspace tests, Clippy and rustfmt, Cairn validation and traceability.
- **Authority**: local bounded COB writes only. No network seed, merge,
  identity, delegation, cache, or release authority.
