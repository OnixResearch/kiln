# Raise the workspace Rust toolchain to 1.97.1

## Why

Owner decision O2 raises Kiln's pinned compiler from Rust 1.88.0 to 1.97.1. The verified-authority replay and revocation stores are to adopt `durable-authority-state` with its redb adapter, and that crate declares Rust 1.96. The adoption cannot start while the workspace pin sits below its minimum.

## What Changes

The root `rust-toolchain.toml` MUST select Rust 1.97.1 with its existing components, target and profile. The workspace `rust-version` MUST declare 1.97.1, the only compiler this change tests. Every flake toolchain derived from the workspace pin follows the file, and the isolated Radicle helper toolchain stays on the same release. New compiler and Clippy findings are fixed in code without changing behavior.

## Impact

Kiln maintainers own the root toolchain pin, workspace manifest, flake comments, contributor documentation and any code Clippy 1.97.1 flags. Separately pinned workspaces (`runtime/*`, `web/dashboard-index`, `tools/*`, `checks/*` Octet overlays, fuzz crates and vendored producers) keep their own toolchains. No dependency version, lock entry, wire format, schema or provider behavior changes. Adopting `durable-authority-state` is a later change.
