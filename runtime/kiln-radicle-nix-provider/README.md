# Kiln Radicle Nix provider

This standalone workspace runs one bounded Nix check for one admitted Radicle repository. It keeps the provider shell, profile admission, bounded process execution, and durable report publication separate from Kiln's CI domain crates.

Use the root `config/radicle-nix-provider.ncl` function to produce its JSON profile. See `../../docs/radicle-nix-provider.md` for the authority and claim boundaries.

The workspace pins Bounded Exec and Durable File Publication by immutable revision. Its Octet hook and Nix check cover the complete workspace, all targets, and all features.

The native process-control drills use Rustix signal syscalls for cancellation and emergency cleanup, not an external `/bin/kill`. They exercise real provider processes and owned descendants under Linux with `/proc` and `/bin/sh`; success, timeout, output-limit and SIGTERM cases retain the same bounded teardown assertions in ordinary and Nix-sandboxed runs.
