## Why

Radicle issues #18 and #20 require exact revision CI and native multi-writer COB projections without giving producer metadata CI authority.

## What Changes

Extend the existing isolated pinned upstream helper with bounded issue/patch/job history admission, explicit observed head sets, native authorization, and stale-preview refusal for own review append. Add a separately selected patch CI intake with exact base/head/revision bindings, signed-author reachability, revision-bound approval, deduplication and exact job results. Preserve default push bytes.

## Impact

Radicle adapters, isolated helper, Radicle CI shell, typed configuration and local fixtures. No core provider dependency, Lattice/hosted changes, remote publication or real signer operations.
