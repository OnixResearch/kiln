## Why

`flake.nix` currently relies on `git+file:../verified-logic` so Nix can copy the sibling `verified-logic` checkout into the synthetic Crane source tree. That works today, but current Nix warns that relative `git+file:` inputs are deprecated and will stop working.

A direct `path:../verified-logic` replacement is not viable because Nix path inputs in `flake.nix` must stay inside the current flake tree. The warning-free fix therefore requires moving the source that Nix consumes into the repo tree instead of pointing at an external sibling path.

## What Changes

- Vendor a repo-managed `verified-logic` crate snapshot under `vendor/verified-logic` so both Cargo and Nix can consume an in-tree dependency.
- Update the root manifest and `flake.nix` to build against the vendored path instead of a sibling `../verified-logic` checkout.
- Add a deterministic refresh workflow that imports a committed revision from the sibling `../verified-logic` repo and records which revision was vendored.

## Non-Goals

- Change the public Rust API exposed by `ucan` or `ucan-core`
- Change the upstream `verified-logic` repository's internal layout or release process
- Teach Nix `path:` flake inputs to reach outside the current flake tree

## Capabilities

### Modified Capabilities
- `nix-build-portability`: Local and CI Nix builds no longer depend on deprecated relative `git+file:` behavior.
- `verified-logic-integration`: The shell crate builds against a checked-in `verified-logic` snapshot that Nix and plain Cargo resolve the same way.
- `verified-logic-refresh`: Contributors can refresh the vendored snapshot from a known upstream revision without hand-copying files.

## Impact

- **Files**: `Cargo.toml`, `flake.nix`, `vendor/verified-logic/**`, refresh tooling under `scripts/`, docs in `README.md` and `AGENTS.md`
- **APIs**: No Rust API changes expected
- **Dependencies**: Adds a vendored `verified-logic` source snapshot tracked in this repo
- **Testing**: `nix flake check` and `nix develop -c cargo test --workspace` should both pass without the relative `git+file:` warning
