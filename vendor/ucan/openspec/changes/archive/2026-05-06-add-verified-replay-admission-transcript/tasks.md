## 1. Implementation
- [x] 1.1 Add a verified-logic module for replay-admission transcript facts and outcomes.
- [x] 1.2 Expose Rust bridge types/functions in `ucan` for replay-admission transcript facts, first-failure outcome selection, and accept/deny decisions.
- [x] 1.3 Route `verify_invocation_with_replay` replay outcomes through the verified transcript bridge before returning allow/deny results.
- [x] 1.4 Preserve caller-owned replay-store/backend semantics; do not claim persistent uniqueness, storage durability, or backend freshness in verified logic.

## 2. Tests
- [x] 2.1 Add verified-logic proof/tests for allow-only acceptance and fail-closed duplicate/stale/malformed/denied/backend outcomes.
- [x] 2.2 Add `ucan` unit tests that exercise allowed replay admission through the verified bridge.
- [x] 2.3 Add `ucan` unit tests that exercise duplicate/stale/malformed/denied/backend replay outcomes as deterministic denials.

## 3. Documentation
- [x] 3.1 Update public API and verified-boundary docs to distinguish verified transcript aggregation from caller-owned replay storage truth.
- [x] 3.2 Update readiness metadata/docs so replay-admission transcript verification is marked controlled-integration-ready without promoting persistent replay-store support.

## 4. Validation
- [x] 4.1 Run `cargo fmt --manifest-path Cargo.toml -p ucan -p ucan-core`.
- [x] 4.2 Run focused replay/verified tests.
- [x] 4.3 Run `cargo test -p ucan --lib` and `cargo test -p ucan --examples`.
- [x] 4.4 Run `cargo clippy -p ucan --all-targets --no-deps -- -D warnings -A clippy::cargo-common-metadata`.
- [x] 4.5 Run `python scripts/check-production-readiness.py`.
- [x] 4.6 Run `openspec validate add-verified-replay-admission-transcript --strict --json`.
- [x] 4.7 Run `nix flake check /home/brittonr/git/ucan -L`.
