## Why

Wave 2 of the Kiln authority plan (S5 restricted source views, S6 grant
lifecycle, S7 projected-workspace authority) needs two Basalt changes that Kiln
does not vendor yet:

- **B1:** the attenuable `kiln://implementation/v2/` framing and the explicit
  `kiln/delegate` chain rule;
- **B2:** the `kiln://source/v1/` operations.

Kiln vendors Basalt `264d2489` and UCAN `417f7466`, which have neither. Kiln #69
also requires the exact Basalt/UCAN cohort that Lattice adopted, so that Kiln
and Lattice verify the same grants with the same code. Lattice `main`
(`39936a19`) vendors Basalt `main` `fac0d7f1fd7ac936ca4d3f251f3287666e93d4bb`,
which contains B1, B2 and the holder series, with Basalt's nested UCAN at
`fca62a6ed6895689ff18112b497bc7a986516c20`.

That UCAN revision moves `ucan-nostr` to `k256` 0.14, which needs stable
`crypto-common` 0.2. Basalt depends on `ucan-nostr` unconditionally. Kiln's Iroh
`0.95.1` pins `aead` 0.6.0-rc.2, which needs a `crypto-common` release
candidate, and Cargo cannot select both. The cohort therefore cannot land
without moving Iroh.

## What Changes

- Re-import `vendor/basalt` at `fac0d7f1` and `vendor/ucan` at `fca62a6e`
  through `scripts/import-pinned-source.sh`, with Basalt's three recorded UCAN
  path rewrites. Update the importer pins and both source records (source
  commit, full-archive and selected-archive BLAKE3) together.
- `crates/kiln-adapter-basalt/tests/cohort.rs` pins both revisions and checks
  that Basalt's compiled UCAN revision equals Kiln's single UCAN snapshot.
- Adapt `kiln-adapter-basalt` to Basalt's new required fields. Kiln binds every
  request with the frozen `ImplementationFraming::V1`, keeps
  `LegacyFraming::Accept`, and sets `max_delegation_depth = 0`, which Basalt
  never consults for v1. Kiln does not adopt v2 or the source operations here.
- Move Iroh from `=0.95.1` to `=1.1.0` in `kiln-adapter-iroh`,
  `kiln-adapter-iroh-observe` and `kiln-hosted`, with only the `tls-ring` and
  `portmapper` features. 1.1.0 is the line `runtime/kiln-aspen-host` already
  pins. Endpoint construction moves to the 1.x builder; ALPNs, frames, bounds,
  peer and relay policy are unchanged. The workflow and observation profiles,
  their Nickel contracts and fixtures, scripts and docs name `1.1.0`.
- Modify `r[kiln.iroh.workflow.profile]` so the accepted spec names the pinned
  Iroh version the code uses.
- Update the reference pins, license map, HTTP observation, Iroh observation,
  migration bridge, hosted CI, operator and swarm-news docs, and the changelog.

## Impact

- **Files:** `vendor/{basalt,ucan}` and their source records,
  `scripts/import-pinned-source.sh`, `Cargo.lock`,
  `crates/kiln-adapter-basalt/{src/lib.rs,src/signer.rs,tests/cohort.rs}`,
  `crates/kiln-adapter-iroh/{Cargo.toml,src/transport.rs,src/migration.rs,src/profile.rs}`,
  `crates/kiln-adapter-iroh-observe/{Cargo.toml,src/shell.rs,src/profile.rs}`,
  `crates/kiln-hosted/{Cargo.toml,src/transport.rs,examples/retained-host.rs}`,
  `config/{iroh-workflow,iroh-observation-profile,live-ci-worker,hosted-client}.ncl`,
  `config/fixtures/iroh-workflow-inputs.ncl`,
  `fixtures/iroh-observation/profile-v1.json`,
  `scripts/{live-ci-controller,web-service-smoke}.py`, docs and changelog.
- **Lock:** 119 package names change: 36 new packages, 22 removed and 61 with
  changed versions (781 to 798 locked packages). All of it comes from the two
  cohorts and the Iroh move they force; `evidence/lock-delta.txt` lists every
  entry.
- **Behavior:** implementation authority decisions for Kiln's v1 requests are
  unchanged except where Basalt itself tightened v1: a v1 chain whose covering
  grant is wider than `kiln://implementation/v1/` now needs explicit
  `kiln/delegate` at every delegator. Iroh peers on `0.95.1` are not promised to
  interoperate with `1.1.0` endpoints, so every Kiln Iroh endpoint upgrades
  together. A workflow or observation profile naming `0.95.1` is refused as
  cohort drift.
- **Testing:** fmt; clippy with and without all features; the wasm build and
  clippy; `kiln-adapter-basalt` tests including `cohort`; the
  `kiln-implementer` authority tests; the Iroh adapters' and `kiln-hosted`
  tests; `scripts/check-nickel-fixtures.sh`; `cairn validate` and the change
  gates.

## Non-claims

- No adoption of `kiln://implementation/v2/`, the delegate rule or
  `kiln://source/v1/` by Kiln; Wave 2 does that.
- No Lattice change, no live qualification, no publication and no release
  eligibility.
- No interoperability between Iroh `0.95.1` and `1.1.0` peers, no WAN or relay
  availability claim, and no qualified composition of the workflow transport
  with `runtime/kiln-aspen-host` merely because both pin Iroh `1.1.0`.
- No new `cargo audit` result, cryptographic audit or security guarantee.
- No review receipt is written by the author.
