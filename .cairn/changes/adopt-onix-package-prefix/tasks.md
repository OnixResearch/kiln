## Phase 1: Rename

- [x] [serial] Rename the two packages, keep the library and binary names, and rename the workspace dependency under its existing key. a[adopt-onix-package-prefix.names]
- [x] [serial] Update the check templates, lock files, flake selectors, boundary script and documented commands, and state the package names and the pinned-consumer note. a[adopt-onix-package-prefix.references]

## Phase 2: Verification

- [x] [serial] Check names with Cargo metadata, run locked workspace tests and Clippy, check nested lock files, run the boundary script in an isolated copy, search for stale selectors, validate lifecycle gates and retain bounded evidence. a[adopt-onix-package-prefix.verification]

`evidence/verification.json` records each Nix check that selects a renamed
package, with its result or under `not_exercised`. Cairn, OnixOS and the Kiln
Aspen host keep their earlier pins and were not changed. This package is not
synchronized or archived.
