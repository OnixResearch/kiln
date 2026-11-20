## Why

Cairn main `231d61e` now emits `cairn.convergence.v1`: for one committed
revision it reports, per registry row, whether a declared assurance target has
current Mantle-realized and Octet-judged evidence. Phase 1 deliberately added
no Kiln behavior. Kiln therefore cannot yet answer the next operator question:
which declaration lacks current evidence, and which evidence job should be
built again.

`kiln-adapter-cairn` has no intake for this receipt, the core has no frozen
selection of unsatisfied rows, and no Verify-stage effect request exists for
the evidence build. Without them Kiln would have to treat Cairn `current` or a
Mantle cache hit as CI acceptance, which AGENTS.md forbids.

## What Changes

- Add a pure, no-I/O intake in `kiln-adapter-cairn` for one bounded
  `cairn.convergence.v1` receipt, with typed rows, stable issue codes and exact
  receipt/registry/case/policy identity binding.
- Add pure core selection of the unsatisfied evidence set and minting of one
  frozen evidence campaign revision bound to one candidate-set BLAKE3.
- Add typed Verify-stage evidence build requests that lower to the existing
  `BuildEffectPlan`, and fail closed before effects when the selected provider
  capability or realization route is unsupported by `kiln-adapter-mantle`.
- Retain the real pilot receipt as a fixture and add synthetic negative
  receipts/facts for every rejection path.

## Impact

- **Files**: `crates/kiln-adapter-cairn/src/convergence.rs`,
  `crates/kiln-adapter-cairn/src/convergence/tests.rs`,
  `crates/kiln-adapter-cairn/src/lib.rs`,
  `crates/kiln-core/src/convergence.rs`,
  `crates/kiln-core/src/lib.rs`, `fixtures/cairn/convergence/pilot/`,
  `docs/cairn-convergence.md`, this change package.
- **Testing**: focused `cargo test -p kiln-adapter-cairn -p kiln-core`, focused
  `cargo clippy --all-targets -D warnings` and `cargo fmt --check`,
  `cairn validate --root .`, and the proposal, design and tasks gates.
- **No provider mutation**: no scheduler, dispatch, claim, archive or push is
  added by this change.
