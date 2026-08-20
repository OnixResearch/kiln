# OpenTelemetry adapter validation

Date: 2026-08-19

## Baseline

- `nix develop path:$PWD -c cargo test --workspace` passed before implementation.

## Focused checks

- `cargo test -p kiln-adapter-opentelemetry` passed with seven positive and negative tests.
- Focused Clippy passed with `-D warnings`.
- `nickel export config/opentelemetry-profile.ncl` passed.
- `negative-opentelemetry-drift.ncl` failed as required.

## Repository checks

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo check -p kiln-core --target wasm32-unknown-unknown` passed.
- All negative Nickel fixtures failed as required.
- Cairn validation passed after spec sync.
- Tracey coverage passed at `303/303` referenced requirements.
- `nix flake check path:$PWD -L` passed all seven checks.

## Pinned cohort

- OTLP protocol `1.11.0`, revision `790608c4d51e6ffc12210b541e8514cbed9e91a4`.
- Semantic conventions `1.43.0`, revision `89aae438b3b3b0a8dd33003c9d70592baf7dbd0d`.
- CI/CD stability: `release_candidate`.
- License: `Apache-2.0`.

## Non-claims

These checks prove bounded deterministic projection and checked export observations. They do not prove telemetry completeness, backend durability, CI correctness, or release readiness.
