# Implementation verification

## Implemented surfaces

- Public generic hook set remains consumer-agnostic: `KeyResolver`, `ProofStore`, `RevocationChecker`, and new `ReplayAdmission`.
- Hook error taxonomy now distinguishes missing/unsupported/malformed/stale/backend resolver failures; missing/stale/malformed/backend proof failures; stale/malformed/backend revocation failures; duplicate/stale/malformed/denied/backend replay admission failures.
- `verify_invocation_with_replay` verifies and authorizes the invocation first, then calls caller-owned replay admission and denies fail-closed on admission errors.
- `AllowAllReplayAdmission` preserves compatibility for consumers that enforce replay elsewhere.
- In-memory replay admission is used only in tests/example code, not as a production backend.
- Readiness docs/report/check distinguish generic hook support from concrete remote DID, proof transport, registry revocation, and replay-persistence backend support.
- README/public API/verified-boundaries document the Aspen/Clankers responsibility split without adding consumer-specific adapters.

## Verification commands

From `/home/brittonr/git/ucan` unless noted:

```bash
cargo fmt --manifest-path Cargo.toml -p ucan -p ucan-core
CARGO_TARGET_DIR=$PWD/target/agent cargo test -p ucan --lib
CARGO_TARGET_DIR=$PWD/target/agent cargo test -p ucan --examples
CARGO_TARGET_DIR=$PWD/target/agent cargo clippy -p ucan --all-targets --no-deps -- -D warnings -A clippy::cargo-common-metadata
python scripts/check-production-readiness.py
openspec validate add-consumer-integration-hooks --strict --json
git diff --check
```

All commands passed. The readiness check reported:

```text
production-readiness: PASS crate_level=controlled-integration-ready surfaces=14
```

From `/tmp`:

```bash
nix flake check /home/brittonr/git/ucan -L
```

Passed with `all checks passed!`; the Nix check also reported `ucan-nextest` summary `169 tests run: 169 passed, 0 skipped` and `ucan-production-readiness` reported `production-readiness: PASS crate_level=controlled-integration-ready surfaces=14`.
