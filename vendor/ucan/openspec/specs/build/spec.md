# build Specification

## Purpose

Define the build and contributor workflow requirements that keep the vendored `verified-logic` snapshot deterministic, warning-free, and independent of sibling checkouts.

## Requirements

### Requirement: Vendored verified-logic source for builds

The repository MUST track an in-tree `verified-logic` source snapshot under `vendor/verified-logic` so both Cargo and Nix can build without a sibling checkout or a deprecated relative `git+file:` input.
r[build.vendored-verified-logic]

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

- GIVEN a fresh clone of `ucan` with no sibling Trellis checkout present
- WHEN Cargo resolves the root manifest dependency on `verified-logic`
- THEN the build MUST use the in-tree `vendor/verified-logic` crate source
- AND the build MUST not require a sibling checkout to compile

### Requirement: Contributor sync workflow for vendored verified-logic

The repository MUST document and support a deterministic workflow for refreshing `vendor/verified-logic` from a committed upstream Trellis revision.
r[build.verified-logic-sync-workflow]

#### Scenario: Contributor refreshes a stale vendored source
r[build.verified-logic-sync-workflow.refreshes-stale-vendored-source]

- GIVEN the vendored `verified-logic` source no longer matches the intended upstream Trellis revision
- WHEN a contributor runs the documented refresh workflow
- THEN `vendor/verified-logic` MUST contain the committed Trellis source imported from that revision
- AND the repo MUST record which upstream revision was imported
- AND `nix flake check` MUST still exit with status 0 without the relative `git+file:` deprecation warning

#### Scenario: Josh filter matches the vendored source boundary
r[build.verified-logic-sync-workflow.josh-filter]

- GIVEN the repo stores a Josh filter for the vendored `verified-logic` subtree
- WHEN the sync config check runs
- THEN the filter MUST select the same required and optional source paths as the importer
- AND contributors MUST be able to verify the checked-in vendor tree against the recorded Trellis revision
