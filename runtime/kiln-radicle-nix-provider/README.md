# Kiln Radicle Nix provider

This standalone workspace runs one bounded Nix check for one admitted Radicle repository. It keeps the provider shell, profile admission, bounded process execution, and durable report publication separate from Kiln's CI domain crates.

Use the root `config/radicle-nix-provider.ncl` function to produce its JSON profile. See `../../docs/radicle-nix-provider.md` for the authority and claim boundaries.

The workspace pins Bounded Exec and Durable File Publication by immutable revision. Its Octet hook and Nix check cover the complete workspace, all targets, and all features.
