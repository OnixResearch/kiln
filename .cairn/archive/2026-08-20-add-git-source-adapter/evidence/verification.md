# Verification evidence

## Baseline

The proposal, design, and tasks gates passed before implementation. The existing core, provider API, and Jujutsu adapter tests also passed.

## Implementation

The adapter pins `git2` version `0.20.2` and `libgit2` version `1.9.6`. Cargo records their exact package checksums.

The first profile supports local SHA-1 repositories with repository format version `0`. It rejects network, credentials, mutation, signing, collaboration, Git links, partial clones, replacements, and alternates.

The pure core validates exact resolution and builds the canonical snapshot manifest. This manifest uses paths, entry kinds, and content bytes without Git object IDs.

Real repositories produced by the pinned library cover stable acquisition, executable files, symbolic links, moved references, missing repositories, and unsupported features.

## Gates

These commands passed:

```console
nix develop path:$PWD -c cargo fmt --all -- --check
nix develop path:$PWD -c cargo check -p kiln-core --target wasm32-unknown-unknown
nix develop path:$PWD -c cargo test --workspace
nix develop path:$PWD -c cargo clippy --workspace --all-targets -- -D warnings
nix develop path:$PWD -c nickel export config/git-source-profile.ncl
nix flake check path:$PWD -L
```

The Nix gate includes positive Nickel profiles and negative authority and bound fixtures.

The final Tracey gate passed with `377/377` referenced requirements.
