# Radicle Canonical Specification

## Purpose

Defines the `radicle-canonical` capability.

## Requirements

### Requirement: Vendored canonical provenance

r[kiln.radicle.canonical.provenance]
The repository MUST carry the Radicle canonical-reference computation as a
vendored crate pinned to heartwood
`71a91042d96bb994c4cd4311d85f5cd1a6311b4e`. `vendor/heartwood-canonical.source.json`
MUST record the upstream commit, every upstream-derived file's SHA-256, the removed
test modules, the shim inventory and the license. Upstream-derived files MUST be
byte-identical to that revision except for the recorded test-module removal. The
importer MUST verify every derived digest and MUST refuse to refresh when the shims
are absent or an upstream anchor changed. Licenses MUST be retained.

#### Scenario: Refresh is drift-refusing

r[kiln.radicle.canonical.provenance.scenario.refresh]
- GIVEN a heartwood checkout at the pinned revision
- WHEN `scripts/import-heartwood-canonical.sh` refreshes the derived files
- THEN every derived file MUST match its recorded digest and the refresh MUST be
  idempotent, AND a changed upstream file or anchor MUST fail before any file is
  written

### Requirement: Canonical computation rail

r[kiln.radicle.canonical.rail]
A repository check MUST build the vendored crate and run its retained tests with
warnings denied. The crate MUST declare its own workspace and lock file, and the
Kiln workspace dependency graph MUST not gain the `sqlite` crate.

#### Scenario: Retained upstream tests run in the rail

r[kiln.radicle.canonical.rail.scenario.tests]
- GIVEN the vendored crate at the pinned revision
- WHEN `nix build .#checks.x86_64-linux.heartwood-canonical` runs
- THEN the retained upstream unit tests MUST pass in the check's build profile,
  including the quorum, reference-protection and DID tests
