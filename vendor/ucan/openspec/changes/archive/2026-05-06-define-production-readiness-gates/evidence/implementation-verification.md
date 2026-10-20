# Production-readiness gate implementation verification

Status: captured

## Implemented artifacts

- `src/readiness.rs`: public static readiness report, support labels, integration profile, and unit tests.
- `src/lib.rs`: public exports for `production_readiness_report`, support labels, surfaces, and profile types.
- `docs/production-readiness.md`: operator-facing support-label report and caller-wired integration profile.
- `docs/production-readiness.json`: machine-readable support report.
- `scripts/check-production-readiness.py`: aggregate readiness guard that checks docs/report consistency and prevents unsupported surfaces from being promoted.
- `flake.nix`: wires the readiness guard into `checks.x86_64-linux.production-readiness`, therefore `nix flake check` runs it.
- README/public API/verified-boundary docs now link the readiness report and keep the crate classified as controlled-integration ready, not turnkey production-supported.

## Commands

- command: `python scripts/check-production-readiness.py`
- result: pass — `production-readiness: PASS crate_level=controlled-integration-ready surfaces=12`

- command: `cargo test -p ucan readiness --lib`
- result: pass — 4 readiness tests passed.

- command: `CARGO_TARGET_DIR=$PWD/target/agent cargo test -p ucan --lib`
- result: pass — 143 library tests passed.

- command: `CARGO_TARGET_DIR=$PWD/target/agent cargo clippy -p ucan --all-targets --no-deps -- -D warnings -A clippy::cargo-common-metadata`
- result: pass.

- command: `nix flake check /home/brittonr/git/ucan -L` from `/tmp`
- result: pass — package, fmt, clippy, nextest, and production-readiness checks passed; nextest ran 167 tests.

## Scope notes

No Aspen-specific adapter code was added to `ucan`. Aspen remains a downstream consumer that should implement its own mapping from UCAN resource/ability/caveats into Aspen identity, capability, operation, revocation, replay, and RPC envelopes.
