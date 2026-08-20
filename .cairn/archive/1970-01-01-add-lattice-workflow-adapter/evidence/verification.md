# Verification: Add the Lattice workflow adapter

Date: 2026-08-20

## Source contract

Kiln consumes `lattice-workflow-contract` from:

- Repository: `ssh://git@github.com/OnixResearch/lattice.git`
- Revision: `66fe7b9b43567490caacc797ebda307e734c4419`
- License: `MIT`

The producer branch and remote `main` contain the archived Lattice contract change.

`scripts/sync-lattice-workflow-fixtures.rs` synchronized the producer fixture. Its `--check` mode passed against the pinned checkout.

## Rust checks

These commands passed:

```text
nix develop path:$PWD -c cargo test --workspace
nix develop path:$PWD -c cargo clippy --workspace --all-targets -- -D warnings
nix develop path:$PWD -c cargo check -p kiln-core --target wasm32-unknown-unknown
nix develop path:$PWD -c rustfmt --check scripts/sync-lattice-workflow-fixtures.rs
```

The focused Lattice adapter rail passed 12 positive and negative tests.

The tests cover deterministic lowering, profile drift, unsupported nodes, provider unavailability, dispatch, observation, cancellation, pause, resume, logs, disconnect ambiguity, stale reconciliation, and independent producer fixtures.

## Nickel checks

The Lattice profile and positive fixture exported successfully.

The node, effect, control, and zero-limit fixtures failed as required.

## Lifecycle and Nix checks

Proposal, design, and tasks gates passed. Cairn validation passed.

Tracey coverage passed with `393/393` referenced requirements after specification sync.

`nix flake check path:$PWD -L` passed. It built and tested the exact Git dependency, strict Clippy, `wasm32` core, Nickel profiles, and release evidence.

## Boundaries

`kiln-core` owns workflow plan and observation admission. `kiln-provider-api` owns workflow execution and exchange ports.

`kiln-adapter-lattice` owns pure contract mapping and shell orchestration. It does not import Lattice runtime implementations into core crates.

The checks do not prove workflow correctness, host confinement, external effect success, credential freshness, provider availability, or release readiness.
