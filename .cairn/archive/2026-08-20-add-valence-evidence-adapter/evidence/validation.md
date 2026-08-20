# Valence adapter validation

Date: 2026-08-20

## Baseline

- `nix develop path:$PWD -c cargo test --workspace` passed before implementation.

## Focused checks

- `cargo test -p kiln-adapter-valence` passed with seven positive and negative tests.
- Focused Clippy passed with `-D warnings`.
- `nickel export config/valence-profile.ncl` passed.
- `negative-valence-role.ncl` failed as required.

## Repository checks

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo check -p kiln-core --target wasm32-unknown-unknown` passed.
- All negative Nickel fixtures failed as required.
- Cairn validation passed after spec sync.
- Tracey coverage passed at `316/316` referenced requirements.
- `nix flake check path:$PWD -L` passed all checks.
- Nix used `allowBuiltinFetchGit` for the exact private SSH revisions in `Cargo.lock`.

## Pinned cohort

- Valence revision `220307a371e7bc4e3309447b09447d0fe6c4db5f`.
- Artifact Auth revision `e41340bec587b6d049b5cc518ec7db925dde84be`.
- Evidence IR: `valence.evidence-ir.v1`.
- Sidecar: `valence.stack-provenance-sidecar.v1`.
- Receipt: `valence.receipt-transcript.v1`.
- `valence-core` license: `MPL-2.0`.

## Non-claims

These checks prove bounded identity, role, span, linkage, and non-claim validation. They do not prove evidence truth, CI correctness, or release eligibility.
