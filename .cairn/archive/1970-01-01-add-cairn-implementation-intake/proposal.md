# Add Cairn implementation-ready snapshot intake

## Why

Cairn now exposes a versioned implementation-ready projection. Kiln needs to
consume it into its own versioned work contract without forking Cairn lifecycle
semantics or granting execution authority.

Issues: OnixResearch/cairn#1 and OnixResearch/kiln#44.

## What Changes

- Add `CairnImplementationPolicy` and `admit_cairn_snapshot` in
  `kiln-adapter-cairn`.
- Accept only complete `cairn.implementation-ready.v1` snapshots; reject
  `incomplete_limit`, forged or archive-ready selections.
- Map ready `task:<id>`/`change:<name>` units to work aliases bound by the change
  snapshot digest and caller-supplied monotonic sequence.
- Add unit requirement references as explicit acceptance obligations.
- Add the `kiln-cairn-intake` binary that admits units into the durable ledger.
- Exclude source-alias versions from the canonical work key so a source edit
  supersedes the same unit instead of creating a new key.

## Impact

- **Files**: `crates/kiln-adapter-cairn/src/implementation.rs`,
  `crates/kiln-adapter-cairn/src/bin/cairn-intake.rs`,
  `crates/kiln-core/src/implementation/mod.rs`, tests and docs.
- **Testing**: ready mapping, non-ready outcomes, incomplete/forged refusal,
  invalid policy, CLI admit/replay, canonical-key supersession.
- **Authority**: intake only. Cairn supplies eligibility; Kiln admits and later
  claims. No worker, dispatch, acceptance or publication is added.
