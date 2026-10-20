# Gate review: add-verified-core-structural-validators

## Result
PASS — core structural validators OpenSpec drained.

## Scope
- Verified predicates/helpers for this seam are present in vendored `verified-logic` and bridged through `src/verified.rs`.
- The live UCAN shell/core path routes finite shell-supplied facts through the verified bridge without Aspen/Clankers-specific adapters.
- README, public API, verified-boundary, bridge-coverage, and production-readiness metadata document the verified claim and retained runtime/caller-owned boundaries.
- Readiness remains `controlled-integration-ready`; no parser, codec, crypto, backend, storage, replay-store, or consumer-policy truth was promoted.

## Validation
- `cargo fmt --manifest-path Cargo.toml -p ucan -p ucan-core` — PASS
- `python scripts/check-production-readiness.py` — PASS: `crate_level=controlled-integration-ready surfaces=20`
- Focused seam tests — PASS:
  - `cargo test -p ucan verified --lib`
  - `cargo test -p ucan proof_traversal --lib`
  - `cargo test -p ucan compact --lib`
  - `cargo test -p ucan parsed --lib`
  - `cargo test -p ucan backend_transcript --lib`
- `CARGO_TARGET_DIR=$PWD/target/agent cargo test -p ucan --lib` — PASS: 147 tests
- `CARGO_TARGET_DIR=$PWD/target/agent cargo test -p ucan --examples` — PASS
- `CARGO_TARGET_DIR=$PWD/target/agent cargo clippy -p ucan --all-targets --no-deps -- -D warnings -A clippy::cargo-common-metadata` — PASS
- `openspec validate --all --strict --json` — PASS: 7/7 before archive
- `git diff --check` — PASS

## Notes
- This drain uses already-vendored verified-logic UCAN modules plus the new readiness/doc classification surfaces. It does not add consumer adapters or concrete backends.
