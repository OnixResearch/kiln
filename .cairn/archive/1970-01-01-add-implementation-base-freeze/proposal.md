## Why

The work contract and attempt froze work identity/revision but not the exact
target/base revision. An attempt could therefore be planned against a moving
symbolic reference, and issue #45 requires freezing the resolved base.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44), consumed by
[#45](https://github.com/OnixResearch/kiln/issues/45).

## What Changes

- Add optional exact `base` to `ImplementationWork` and include it in work
  identity.
- Add optional `base` to `ImplementationAttempt`, set from the work at claim.
- Validate the base reference before admission.
- Leave base resolution to the source adapter; absence is explicit.

## Impact

- **Files**: `crates/kiln-core/src/implementation/{mod,attempt}.rs`,
  adapter and ledger fixtures, `docs/implementation-work.md`.
- **Testing**: base changes identity; claim freezes base across a later edit and
  supersession.
- **Authority**: identity/state only; no resolution, dispatch or acceptance.
