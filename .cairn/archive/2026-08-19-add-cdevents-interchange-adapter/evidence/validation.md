# CDEvents adapter validation

Date: 2026-08-19

## Baseline

- `nix develop path:$PWD -c cargo test --workspace` passed before implementation.

## Focused checks

- `cargo test -p kiln-adapter-cdevents` passed with six positive and negative tests.
- `nickel export config/cdevents-profile.ncl` passed.
- `negative-cdevents-lossy.ncl` failed as required.

## Repository checks

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo check -p kiln-core --target wasm32-unknown-unknown` passed.
- All negative Nickel fixtures failed as required.
- Cairn validation passed after spec sync.
- Tracey coverage passed at `287/287` referenced requirements.
- `nix flake check path:$PWD -L` passed all seven checks.

## Pinned cohort

- CDEvents `0.5.1`, revision `df02ce0b6563a13bf35cc109993b03b18da8a14a`.
- CloudEvents specification `1.0.2`, context version `1.0`.
- License: `Apache-2.0`.

## Non-claims

These checks prove bounded format admission, deterministic mapping, and preserved identity links. They do not prove external event truth, delivery, or CI success.
