# Vendored UCAN Source Boundary

Basalt builds against the reviewed UCAN snapshot in `vendor/ucan/`.

The snapshot records integrated UCAN commit `417f74662b9de2de3bdabe93ad5db996e1bd2861`.
That revision is published on UCAN `main`. The checked-in snapshot remains the durable build boundary.

## Imported source

The snapshot contains the complete Cargo build closure for these UCAN packages:

- the `ucan` standard-library shell
- the `ucan-core` functional core
- the optional `ucan-nostr` adapter crate
- the vendored `verified-logic` dependency
- the two UCAN repository check binaries that its manifest declares
- the static Aeneas proof-checker inputs required by those binaries

The snapshot also contains the reviewed support catalog, holder-observation documentation, and license boundary.
It preserves these license files:

- `vendor/ucan/LICENSES/AGPL-3.0-or-later.txt`
- `vendor/ucan/LICENSES/MPL-2.0.txt`
- `vendor/ucan/crates/ucan-core/LICENSE`
- `vendor/ucan/crates/ucan-nostr/LICENSE`
- `vendor/ucan/vendor/verified-logic/LICENSE`

Basalt remains `AGPL-3.0-or-later`. Each vendored UCAN package retains its declared license.

## Private package boundary

Basalt has `publish = false` because the reviewed UCAN closure has no safe registry identity.
A normal Cargo package operation replaces the path dependency with the unrelated crates.io package `ucan ^0.1.0`.
Do not create or publish that tarball.

The release verifier checks the private package list and the required vendored source files separately.
The vendored checker validates the manifest, revision, licenses, file modes, lengths, and BLAKE3 identities.

## Offline check

Run the snapshot check from the Basalt repository root:

```bash
nix develop -c rustc --edition=2024 tools/sync-vendored-ucan.rs -o /tmp/basalt-sync-vendored-ucan
nix develop -c /tmp/basalt-sync-vendored-ucan --check
```

The check compares every selected file with `vendor/ucan.snapshot`.
The manifest records file modes, byte lengths, and BLAKE3 identities.
It rejects extra files, missing files, changed files, symlinks, missing license files, and a changed revision record.

## Source parity check

If a local Git repository contains the reviewed commit, run this command:

```bash
nix develop -c /tmp/basalt-sync-vendored-ucan --check-source-revision \
  /path/to/ucan FULL_GIT_OBJECT_ID
```

The compatibility form `--check-source /path/to/ucan` reads only commit `417f74662b9de2de3bdabe93ad5db996e1bd2861`.
Uncommitted files and the checked-out branch do not affect the result.

## Import

Use an explicit source repository path to refresh the snapshot:

```bash
tools/sync-vendored-ucan.rs --import-revision \
  /path/to/ucan FULL_GIT_OBJECT_ID
```

The importer accepts only a full lowercase Git object ID. It rejects moving references and malformed values before snapshot replacement.

The compatibility form `--import /path/to/ucan` selects the current `SOURCE_REVISION` value.

The importer archives the exact commit, creates the snapshot in a staging directory, and checks all required license files.
It then replaces `vendor/ucan/` and writes the revision and BLAKE3 manifest.

A BLAKE3-bound coordinator plan can supply the exact reviewed revision. The operator must still review the new UCAN source boundary before import.

If an import or check fails, inspect the first diagnostic. Do not continue to another consumer or commit partial snapshot changes.
