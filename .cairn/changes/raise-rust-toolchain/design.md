## Context

Kiln pinned Rust 1.88.0 in `rust-toolchain.toml` and declared `rust-version = "1.88"`. The flake builds packages, checks and the default development shell from that file through `rust-bin.fromRustupToolchainFile`. Only the isolated Radicle helper cohort named Rust 1.97.1 directly.

## Decision

Change the channel to `1.97.1` and keep `clippy`, `rustfmt`, `wasm32-unknown-unknown` and the minimal profile. Declare `rust-version = "1.97.1"`: the tested release, not a proven minimum. Packages, checks and the development shell follow the root file without edit. The flake `clippy` check took Clippy from nixpkgs; it now uses the pinned toolchain's Clippy, so the check and the development shell run the same linter. The Radicle helper keeps its explicit `stable."1.97.1".minimal` selection so its cohort does not move with later root pins, and its comment now names the shared release. `tools/http-contract-drift`, which the flake builds with the root toolchain, declares the same `rust-version`.

Octet lints the root `kiln-core`, `kiln-provider-api` and `kiln-codec` crates inside its overlay workspaces with its own pinned nightly (rustc 1.96.0-nightly-2026-03-21). Cargo refuses those crates' new `rust-version` there. The two overlay checks pass `--ignore-rust-version`: the declaration states which compiler Kiln builds are tested with, and the Octet gate is a lint gate under Octet's compiler, not a build-support claim. The overlays' own `rust-version = "1.88"` declarations and the separately pinned runtime, browser and fuzz workspaces are unchanged.

Fix new rustc warnings and `clippy --workspace --all-targets -D warnings` findings in code. An `#[allow]` is used only where a lint is wrong for that code, with the reason beside it. Formatting stays stable under rustfmt 1.97.1.

Dependencies held back for 1.88 stay where they are. The Tree-sitter 0.26 pins and the `detect-coding-agent` metadata patch no longer need the old compiler; moving them is separate follow-up work, and the manifest comments say so.

Some fixtures are bound to the workspace toolchain and must be re-qualified on every bump:

- the `pijul-cohort` check pins the Pijul fixture producer's store path and BLAKE3, and the producer is built with `rust-toolchain.toml`. Take the new values from an actual build of `packages.x86_64-linux.pijul-cohort-fixture-producer`; `pijul-native-fixtures` qualifies the new binary.
- the build-input corpus (`config/build-input-contracts.ncl`, exported to `fixtures/build-inputs/corpus.json`) declares the toolchain its planned commands run under. Re-declare it, re-export, and refresh `fixtures/build-inputs/expected.json` from an actual `build-input-qualification --execute` run. Only toolchain-derived identities may change. Only the `build-input-fixtures` flake check exercises this; the workspace and all-features gates do not enable its feature.

## Verification

Run the maintained root gates under the 1.97.1 development shell: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace --no-fail-fast`, the no-std wasm32 core checks, rustdoc, the Nickel fixtures, Octet, and the Cairn validation, traceability and change gates. Record each exit status in the change evidence.

## Risks and non-claims

A newer compiler can change codegen, inference or lint behavior without a source change. The workspace test suite is the behavioral check. The pin does not claim 1.97.1 is Kiln's minimum supported compiler, and it does not qualify separately pinned runtime, browser, fuzz or vendored workspaces.
