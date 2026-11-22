## Implementation

- [x] [serial] Pin Rust 1.97.1 in `rust-toolchain.toml`, declare it as the workspace `rust-version`, and keep the flake and Radicle helper toolchains consistent. r[kiln.toolchain.pin]
- [x] [serial] Correct manifest and contributor documentation that described the 1.88 pin, and record held-back dependencies as follow-ups. r[kiln.toolchain.pin]
- [x] [serial] Fix new rustc and Clippy findings without behavior changes. r[kiln.toolchain.gates.scenario.lint]
- [x] [serial] Re-pin the Pijul fixture producer, which builds with the workspace toolchain, from `/nix/store/qr2sja5n9r9m0j9q2pyn60cyb6lg3vnp-pijul-1.0.0-beta.11/bin/pijul` (BLAKE3 `9b8d58a1…`) to `/nix/store/wlm2nasarbcaji3y0v4gz39ckvqdmhgn-pijul-1.0.0-beta.11/bin/pijul` (BLAKE3 `22cf118c…`); `pijul-native-fixtures` qualifies the new binary against the Pijul adapter. r[kiln.toolchain.pin]
- [x] [serial] Re-qualify the build-input fixture corpus under Rust 1.97.1: re-declare the corpus toolchain and its `rust-toolchain.toml` channel in `config/build-input-contracts.ncl`, re-export `fixtures/build-inputs/corpus.json`, and refresh `expected.json` from an actual `--execute` harness run. Only the declared toolchain, the alpha and beta plan and materialized view identities, the corpus identity and the synthetic revision changed; inputs, views and executions are unchanged. r[kiln.toolchain.pin]

## Qualification

- [x] [serial] Enter the development shell and confirm Rust 1.97.1. r[kiln.toolchain.pin.scenario.shell]
- [x] [serial] Run fmt, Clippy, workspace tests, wasm32, rustdoc, Nickel, Octet and Cairn gates and record exit statuses. r[kiln.toolchain.gates]
- [x] [serial] Rerun `nix flake check` at integration: integration head `16e573553c40eda639f3a6956131fe51ab06d824`, all 47 `checks.x86_64-linux`, exit 0 (`evidence/verification.json` `integration_flake_check`). r[kiln.toolchain.gates]
