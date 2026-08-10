## Why

Kiln currently plans Radicle broker compatibility and one job COB status
projection. It does not define a first-class model for collaborative objects,
typed profiles, event intake, conflict state, or preview-first mutations.

Cairn now supplies signed Git-backed COB profiles for active changes, RFDs,
ADRs, verification obligations, release decisions, and policy exceptions.
Kiln needs to compose these profiles with native Radicle issues, patches, and CI
status without copying their semantics into the CI core.

## What Changes

- Add typed provider-neutral COB references for repository, type, object,
  operation, semantic-state, and Kiln identities.
- Add a versioned Nickel profile registry for supported COB formats,
  capabilities, bounds, semantic mappings, conflicts, and non-claims.
- Add initial profiles for native Radicle issues, patches, and CI jobs.
- Add initial profiles for all current Cairn COB types, including
  `xyz.onixresearch.adr` and `xyz.onixresearch.cairn-change`.
- Add separate read, event, status, and preview-first mutation contracts.
- Preserve concurrent heads and semantic conflicts without last-writer-wins
  behavior.
- Bind gate, verification, policy, and release inputs to exact object state and
  candidate identities.
- Add independent positive and negative format fixtures and dependency gates.

## Dependencies

Implementation begins after `establish-neutral-ci-control-plane` supplies the
provider ports, canonical identities, event journal, effect plans, and candidate
binding.

The first Cairn adapter profile uses Cairn revision
`837d730799faffe6a088cfd6dc49e08c7e0f1822` as its design baseline. This path
is a review reference, not an ambient sibling dependency.

## Impact

- **Core**: adds pure profile admission, identity mapping, applicability,
  conflict classification, and mutation-plan decisions.
- **Adapters**: adds optional Radicle and Cairn COB shells outside core crates.
- **Configuration**: adds typed Nickel profile and limit contracts.
- **Interoperability**: broadens Radicle support beyond broker messages and job
  status projections.
- **Authority**: keeps signing, Git storage, nodes, transport, profiles,
  credentials, and provider reducers in shells.
- **Testing**: requires positive and negative fixtures for every supported
  profile and capability.

## Out of Scope

- Do not run or manage a Radicle node in the Kiln core.
- Do not copy Cairn reducers or lifecycle authority into Kiln.
- Do not treat a signed operation as actor trust or action authority.
- Do not make a COB the Kiln journal, source of CI truth, or release executor.
- Do not initialize, sign, publish, announce, synchronize, or seed a Radicle
  repository during this change package.
