## Phase 1: Audited implementation

- [x] [parallel] Correct the model-only replay change and use platform HMAC verification with real Worker freshness, signature, and durable nonce regression scenarios for r[kiln.celld.transport].
- [x] [parallel] Apply compatible targeted registry security updates while preserving Rust 1.88 and the Celld transport dependency boundary for r[kiln.celld.transport].
- [x] [serial] Restore ordinary public-identity comparisons and retain owner-only socket behavior with operator documentation and an exercised runtime permission check for r[kiln.aspen_host.transport].

## Phase 2: Verification

- [x] [serial] Exercise Worker conformance, provider/authentication regressions, socket permissions, root checks, and lifecycle gates; record exact successes and blockers without claiming a new qualified deployment for r[kiln.celld.transport] and r[kiln.aspen_host.transport].

## Observed verification

- Root `nix develop path:. --command cargo test --locked --workspace`: 555 passed, zero failed, three environment-gated tests ignored. Includes the real local TLS observation-server and authenticated native Iroh paths.
- Root all-target Clippy with `--locked --workspace --all-targets -- -D warnings`: passed on Rust 1.88. Root and standalone-host formatting checks passed.
- `node --test tests/celld-worker.test.mjs`: 17 passed. The production Worker admits the inclusive 300-second boundaries, rejects stale/future and malformed/bad-MAC requests before object resolution, rejects a mutation nonce after SQLite reopen, and accepts an exact-operation retry with a fresh nonce.
- Standalone-host `cargo test --locked --workspace --all-targets --all-features` with the installed nightly compiler: 30 passed, zero failed, zero ignored. Includes real owner-only socket admission, Aspen-to-Lattice execution, durable restart, crash/uncertainty recovery without redispatch, finite load, damaged-state rejection, backup/restore and exact-cohort rollback drills.
- A throwaway native socket executable accepted a same-owner connection, observed socket mode `0600`, and rejected shared and symlinked final parents before socket creation. The executable was removed after use.
- Standalone-host all-target/all-feature Clippy with matching installed nightly Cargo, rustc and `clippy-driver`: passed. The initial mixed-tool command selected the root 1.88 Clippy driver and failed MSRV admission; explicitly selecting the nightly `cargo-clippy` and driver corrected the command, without changing manifests or locks. The pinned Aspen malformed-manifest fixture emits a diagnostic even on the successful command.
- `nix build path:.#checks.x86_64-linux.octet --no-link -L`: passed, including the standalone-host deny-all gate with zero findings, warnings or errors. Radicle seed HEAD lookups returned HTTP 504 and used cached refs; the selected immutable inputs remained pinned.
- `nix flake check path:. --no-write-lock-file -L`: all `x86_64-linux` checks passed, including root package tests, Clippy, Nickel, no-std, aggregate Octet and the standalone-host all-feature package tests. `aarch64-linux` was omitted as incompatible with this host; it is not covered by this run. Nix used the configured remote builder and immutable cached source inputs, not fresh-source availability proof.
- `cairn sync harden-audit-followup --root . --policy cairn-policy/generated/cairn-policy.json --execute`: applied both transport contracts. Explicit-policy `cairn validate` passed after synchronization (50 specs, no findings).
- Explicit-policy `cairn gate proposal`, `cairn gate design` and `cairn gate tasks` for this change: passed. `cairn release-readiness --root . --policy cairn-policy/generated/cairn-policy.json`: local verdict pass, including traceability, deterministic coverage and read-only MCP smoke. Its `onix_release_ready` remains false; full determinism replay and optional authenticated release evidence are not supplied, and the command does not execute the heavyweight repository gates.
- Root `cargo audit --no-fetch --json`: exit 1, 10 vulnerabilities and 9 warnings, down from 17 and 10 with no ignores. Exact fixes, remaining dependency constraints and repeat commands are in `docs/reference-pins.md`.

These observations belong to this local source change, not a previously qualified executable cohort. Kiln maintainers own the repeatable Worker, socket and transport rails. No source-host publication, deployment, signing or provider-cohort migration was performed.
