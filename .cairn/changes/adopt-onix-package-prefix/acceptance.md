# Change-local acceptance

a[adopt-onix-package-prefix.names] `cargo metadata` reports packages `onix-kiln-core` and `onix-kiln-cli`, library target `kiln_core` and binary target `kiln`; no package named `kiln-core` or `kiln-cli` remains in the workspace, and no Rust source changes.

a[adopt-onix-package-prefix.references] Every package selector in the flake, the boundary script, the Octet check templates, the lock files and the documented commands names the renamed packages. No stale `kiln-core` or `kiln-cli` selector remains in tracked files outside archived evidence and retained historical receipts.

a[adopt-onix-package-prefix.verification] The locked workspace tests and Clippy pass, each regenerated lock file is consistent with its manifest, the boundary script passes with its negative case in an isolated copy, lifecycle gates are exercised, and the evidence names the Nix checks that were not run.
