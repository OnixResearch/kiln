## Why

`LICENSES.md` states that the MIT and Apache-2.0 texts of the vendored Radicle canonical computation are retained at `vendor/heartwood-canonical/LICENSE-MIT` and `vendor/heartwood-canonical/LICENSE-APACHE`. Neither file exists or has existed in this repository, and `scripts/import-heartwood-canonical.sh` does not extract them. The `radicle 0.25.1` crate archive carries no license file. Accepted requirement `kiln.radicle.canonical.provenance` ends "Licenses MUST be retained." The link check of launch preparation found the gap.

## What Changes

- Extract `LICENSE-MIT` and `LICENSE-APACHE` from the repository root of heartwood at the pinned commit `71a91042d96bb994c4cd4311d85f5cd1a6311b4e` through the existing importer.
- Record both in `vendor/heartwood-canonical.source.json` as upstream-derived files with their upstream paths and SHA-256, so the existing drift-refusing loop verifies and refreshes them.

The canonical algorithm files, shims, crate manifest, lock file and every Kiln behavior are unchanged.

## Impact

- **Current consumer:** recipients of the published source, the license map and the launch checklist.
- **Durable capability:** the importer keeps the vendored license texts bound to the pinned commit on every refresh.
- **Owner:** Kiln maintainers.
- **Files**: `scripts/import-heartwood-canonical.sh`, `vendor/heartwood-canonical.source.json`, `vendor/heartwood-canonical/LICENSE-MIT`, `vendor/heartwood-canonical/LICENSE-APACHE`.
- **Testing**: run the importer against a checkout holding the pinned commit and confirm an idempotent refresh; run it in an isolated copy with a tampered license digest and confirm refusal without a write; confirm the `LICENSES.md` links resolve; run the vendored crate's tests.
- **No accepted-spec effect:** the change restores conformance to an existing requirement and adds none.
