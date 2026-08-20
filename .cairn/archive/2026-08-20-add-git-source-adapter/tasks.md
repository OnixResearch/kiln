## Phase 1: Compatibility pin

- [x] [serial] Select and pin the Git library, source revision, crates, licenses, object formats, repository features, platforms, and non-claims. r[kiln.git.profile] r[kiln.git.nonclaims]
- [x] [depends:git-cohort] Add a typed Nickel read-only profile and named bounds. r[kiln.git.profile]
- [x] [parallel] Add valid profile and invalid format, feature, capability, and bound fixtures. r[kiln.git.conformance]

## Phase 2: Pure source core

- [x] [serial] Add bounded Git repository, reference, object-format, commit, tree, and observation facts. r[kiln.git.resolution]
- [x] [depends:git-facts] Implement pure snapshot entry admission and canonical manifest construction. r[kiln.git.snapshot]
- [x] [parallel] Add positive and negative pure-core tests for paths, entries, duplicates, modes, submodules, and bounds. r[kiln.git.conformance]

## Phase 3: Adapter shell

- [x] [depends:git-profile] Add `kiln-adapter-git` outside core dependency graphs. r[kiln.git.profile] r[kiln.git.nonclaims]
- [x] [depends:git-snapshot-core] Implement local repository observation, exact resolution, consistent acquisition, and canonical identity. r[kiln.git.resolution] r[kiln.git.snapshot]
- [x] [parallel] Prove that the first profile cannot use network, credentials, mutation, or collaboration operations. r[kiln.git.capabilities] r[kiln.git.nonclaims]

## Phase 4: Conformance and evidence

- [x] [serial] Add real positive and negative repositories from the pinned cohort. r[kiln.git.conformance]
- [x] [serial] Document setup, formats, special entries, limits, and non-claims. r[kiln.git.profile] r[kiln.git.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.git.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.git.conformance] r[kiln.git.nonclaims]
