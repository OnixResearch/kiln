# Tasks

- [x] Add executable byte-length admission helpers and tests in sibling `verified-logic::ucan_signature`.
- [x] Validate sibling with `cargo fmt`, focused `cargo test --quiet ucan_signature`, and `./scripts/verus.sh src/lib.rs`; commit and push sibling.
- [x] Sync UCAN vendored `verified-logic` and route local signature/key length validators through the helpers.
- [x] Update verified-boundary docs, bridge coverage, and bridge guard patterns.
- [x] Run UCAN validation: fmt, workspace tests, clippy, no-std guard, bridge guard, OpenSpec strict/all, diff check, and flake check.
- [x] Archive the OpenSpec and verify final clean/pushed state.
