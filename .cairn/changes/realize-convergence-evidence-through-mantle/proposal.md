## Why

Phase 2 delivered the Cairn convergence intake, the unsatisfied-set selection,
one frozen campaign revision and Verify lowering to `BuildEffectPlan`. Its
Verify stage refuses `EvidenceRealization::PreserveCacheOnly` with
`unsupported_capability` because `kiln-adapter-mantle` runs only
`mantle build --plan`. The phase-1 evidence jobs are frozen foreign derivation
graphs that Mantle realizes through `foreign-import realize` on the
cache-only preserve route. Kiln therefore cannot build "the same evidence
jobs" that the handoff promises.

The pinned Mantle revision now contains the preserve-route guide fix and the
raised foreign-import bounds, so the operation can be added behind a second,
separate cohort. Without it every preserve-route campaign item stops at the
capability check, and Kiln has no completion record for a realized evidence
root.

## What Changes

- Add one native Cairn change with a delta spec for the Verify-stage preserve
  route and a new Mantle foreign-realize adapter capability.
- Add a second Mantle cohort and typed profile, separate from the build cohort,
  for exactly one operation: `foreign-import realize` on Mantle
  `ead7b8dac087fad0170daca01356afbb3b43b179`.
- The realize profile binds the frozen plan, import receipt, source bundle and
  manifest BLAKE3, execution profile, cache-closure policy and remote endpoint
  binding by BLAKE3 identity, keyed by plan identity, plus the physical store,
  state directory, logical store prefix `/nix/store` and platform.
- Admit `PreserveCacheOnly` in core only when the selected capability profile
  declares the preserve capability, and refuse a profile mismatch for either
  realization before any effect.
- Implement bounded dispatch, closed-world realization-receipt completion,
  product-to-selected-root mapping, distinct failure classes, recorded
  observation replay and non-redispatching reconciliation in
  `kiln-adapter-mantle`.
- Run one live qualification against the retained phase-1 pilot inputs and
  record the commands, exit codes, receipt and comparison in the change
  evidence.
- Update `docs/mantle-foreign-realize.md`, `docs/README.md` and
  `docs/cairn-convergence.md`.

## Impact

- **Files**: `crates/kiln-adapter-mantle/src/{realize.rs,realize/tests.rs,lib.rs,profile.rs}`,
  `crates/kiln-core/src/{convergence.rs,convergence/tests.rs,provider.rs}`,
  `config/mantle-foreign-realize-profile.ncl`,
  `config/fixtures/*mantle-foreign-realize*.ncl`,
  `fixtures/mantle/foreign-realize/`, `docs/mantle-foreign-realize.md`,
  `docs/README.md`, `docs/cairn-convergence.md`, this change package.
- **Testing**: focused `cargo test -p kiln-adapter-mantle -p onix-kiln-core`,
  focused `cargo clippy --all-targets -D warnings`, `cargo fmt --check`,
  `scripts/check-nickel-fixtures.sh`, `cairn validate --root .` and the
  proposal, design and tasks gates, plus one live pilot qualification.
- **No provider mutation** outside the qualified realization: no push, no
  archive, no scheduler or release effect is added by this change.
