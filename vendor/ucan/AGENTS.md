# Agent Notes

## Architecture
- `crates/ucan-core` is the `#![no_std]` functional core. Keep deterministic parsing/validation there and keep it free of `std` imports.
- The repo-root `ucan` crate is the `std` shell. Put file/path/reader/writer integration there and delegate validation into `ucan-core`.
- UCAN semantic primitives in the shell should bridge into the vendored `vendor/verified-logic` crate instead of reimplementing ability, temporal, signature, or DID logic locally.
- Current public surface includes end-to-end token issuance, signer-based issuance (`TokenSigner`, `Ed25519InMemorySigner`, `issue_token_with_signer`), parsing, verification, and proof-chain APIs (`issue_token`, `verify_compact_token`, `IssueRequest`, `VerificationContext`) plus two runnable examples. Treat “make it usable” follow-up work as adding interoperability, ergonomics, key/DID resolver abstractions, caveats/invocations, and production integration examples rather than only polishing helper exports.

## Build
- `nix develop -c cargo test --workspace` is the fastest verification path for ordinary changes.
- The dev shell includes `uv`; Hegel property tests install into `.hegel/` on first run, so keep that directory gitignored. Sandboxed `nix flake check` skips Hegel integration tests and runs lib/example checks because Hegel's Python server bootstrap is not sandbox-friendly.
- `flake.nix` uses Crane, so a tracked `Cargo.lock` must exist before `nix develop` or `nix flake check` can evaluate successfully.
- New workspace files must be at least staged with `git add -N` before Crane-based checks can see them.
- `flake.nix` now builds directly from the repo's cleaned Cargo source because `vendor/verified-logic` is checked in alongside the root manifest.
- Full `nix flake check` now scopes `cargo fmt` and `cargo clippy` to `ucan`/`ucan-core` and allows `clippy::cargo-common-metadata`; otherwise the vendored `verified-logic` snapshot or package metadata gets linted against this repo's policy instead of upstream's.
- Refresh `vendor/verified-logic` with `./scripts/sync-verified-logic.sh ../trellis`; the script imports the committed `HEAD` revision from the sibling Trellis checkout and records the imported revision in `vendor/verified-logic-revision.txt`.
- `josh/verified-logic.josh` is the canonical Josh filter for the vendored subtree. Run `rustc scripts/check-verified-logic-josh-sync.rs -o /tmp/check-verified-logic-josh-sync && /tmp/check-verified-logic-josh-sync --source-repo ../trellis` to verify the vendored tree against the recorded Trellis revision.
- `.pre-commit-config.yaml` intentionally runs only the focused local Nix check for Josh sync config, unit tests, and revision metadata. Do not make pre-commit require a sibling `../trellis` checkout.
- After archiving an OpenSpec change, validate repository state with `openspec validate --all --strict`; bare `openspec validate --strict` exits with “Nothing to validate”.

## Gotchas
- Avoid SSH-only Cargo dependencies unless the environment is guaranteed to have matching GitHub SSH credentials; anonymous or CI-like environments will fail before tests run.
- Keep wildcard abilities aligned with `verified-logic::ucan_ability`: `*` is valid only as a full path segment, not inside another segment.
- Nix `path:` flake inputs cannot point outside the current flake tree, so `path:../trellis` is not a replacement for the old relative source input. The repo avoids that constraint by vendoring the Trellis `verified-logic` crate in-tree.
- `.gitignore`'s `result*` pattern will also ignore `vendor/verified-logic/src/result.rs` unless that path is explicitly unignored; without it, Crane's cleaned source misses the module and full Nix builds fail.
- While `flake.nix` is changing, repo-local `direnv` reevaluations can stall shell commands. For verification commands, prefer invoking Nix from outside the repo root (for example from `/tmp` with an explicit flake path) to avoid hanging on `nix print-dev-env`.
- Focused Cargo follow-up checks can block behind the shared `/home/brittonr/.cargo-target` lock from unrelated repos; use a repo-local ignored target such as `CARGO_TARGET_DIR=$PWD/target/agent` for quick `cargo test -p ucan-core` / `cargo clippy -p ucan-core` loops.
