## ADDED Requirements

### Requirement: Vendored verified-logic source for builds
r[build.vendored-verified-logic]

The repository MUST track an in-tree `verified-logic` source snapshot under `vendor/verified-logic` so both Cargo and Nix can build without a sibling checkout or a deprecated relative `git+file:` input.

#### Scenario: Warning-free flake evaluation
r[build.vendored-verified-logic.warning-free-flake-evaluation]

- GIVEN a checkout with `vendor/verified-logic` present in-tree
- WHEN `nix flake check` evaluates `flake.nix`
- THEN the command MUST exit with status 0
- AND evaluation MUST not emit the relative `git+file:` deprecation warning

#### Scenario: Warning-free dev shell verification
r[build.vendored-verified-logic.warning-free-dev-shell-verification]

- GIVEN a checkout with `vendor/verified-logic` present in-tree
- WHEN `nix develop -c cargo test --workspace` runs
- THEN the command MUST exit with status 0
- AND the command MUST not emit the relative `git+file:` deprecation warning

#### Scenario: Fresh clone resolves verified-logic dependency
r[build.vendored-verified-logic.fresh-clone-resolves-dependency]

- GIVEN a fresh clone of `ucan` with no sibling `../verified-logic` checkout present
- WHEN Cargo resolves the root manifest dependency on `verified-logic`
- THEN the build MUST use the in-tree `vendor/verified-logic` crate source
- AND the build MUST not require a sibling checkout to compile

### Requirement: Contributor sync workflow for vendored verified-logic
r[build.verified-logic-sync-workflow]

The repository MUST document and support a deterministic workflow for refreshing `vendor/verified-logic` from a committed upstream `verified-logic` revision.

#### Scenario: Contributor refreshes a stale vendored source
r[build.verified-logic-sync-workflow.refreshes-stale-vendored-source]

- GIVEN the vendored `verified-logic` source no longer matches the intended upstream revision
- WHEN a contributor runs the documented refresh workflow
- THEN `vendor/verified-logic` MUST contain the committed source imported from that revision
- AND the repo MUST record which upstream revision was imported
- AND `nix flake check` MUST still exit with status 0 without the relative `git+file:` deprecation warning
